import * as fs from "fs";
import * as path from "path";
import * as os from "os";
import { AiProvider } from "../ai-provider-contract";
import {
  EmbeddingLoadBalancer,
  LocalDeterministicEmbedder,
} from "../embedding-load-balancer";
import { SemanticSearchCoordinator } from "../semantic-search-coordinator";
import { DirectoryCrawler } from "../directory-crawler";

async function runTests() {
  console.log("\n=======================================================");
  console.log("TS DIRECTORY CRAWLER & AUTOMATED INGESTION TEST SUITE");
  console.log("=======================================================");

  let passed = 0;
  let failed = 0;

  async function test(name: string, fn: () => Promise<void>) {
    try {
      await fn();
      console.log(`[PASS] ${name}`);
      passed++;
    } catch (err) {
      console.error(`[FAIL] ${name}:`, err);
      failed++;
    }
  }

  const localEmbedder = new LocalDeterministicEmbedder(768);
  const fakeProvider: AiProvider = {
    id: "google-ai-studio",
    displayName: "Google AI",
    async generate() {
      throw new Error("not used");
    },
    async embed(req) {
      const vecs = await localEmbedder.embed(req.inputs);
      return {
        providerId: "google-ai-studio",
        model: req.model,
        vectors: vecs,
        dimensions: 768,
      };
    },
  };

  const balancer = new EmbeddingLoadBalancer(fakeProvider, {
    slots: [{ modelId: "text-embedding-004", cooldownMs: 5000 }],
    localFallback: localEmbedder,
    unifiedDimensions: 768,
  });

  const coordinator = new SemanticSearchCoordinator(balancer);

  // Create temporary directory fixture
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), "crawler-test-"));
  const subDir = path.join(tempDir, "finance");
  fs.mkdirSync(subDir, { recursive: true });

  const invoiceFilePath = path.join(subDir, "invoice_august_2026.md");
  fs.writeFileSync(
    invoiceFilePath,
    "# Monthly Invoice\nCloud hosting invoice for August 2026. Total payment due: $4,500 via wire transfer.",
    "utf-8",
  );

  const contractFilePath = path.join(tempDir, "contract_signed.txt");
  fs.writeFileSync(
    contractFilePath,
    "Master services agreement and legal contract signed between the parties.",
    "utf-8",
  );

  const ignoredFilePath = path.join(tempDir, "binary_data.bin");
  fs.writeFileSync(ignoredFilePath, Buffer.from([0, 1, 2, 3, 4]));

  try {
    // 1. File discovery test
    await test("DirectoryCrawler discovers eligible text and markdown files, skipping binaries", async () => {
      const crawler = new DirectoryCrawler(coordinator, {
        includeExtensions: ["md", "txt"],
      });
      const files = crawler.discoverFiles(tempDir);

      const paths = files.map((f) => path.basename(f.filePath));
      if (
        !paths.includes("invoice_august_2026.md") ||
        !paths.includes("contract_signed.txt")
      ) {
        throw new Error(
          `Expected markdown and txt files, got: ${JSON.stringify(paths)}`,
        );
      }
      if (paths.includes("binary_data.bin")) {
        throw new Error("Should not discover unapproved binary extension");
      }
    });

    // 2. Automated directory crawl and index test
    await test("DirectoryCrawler crawls directory and indexes documents into WASM engine", async () => {
      const crawler = new DirectoryCrawler(coordinator);
      const { indexedCount, errors } = await crawler.crawlAndIndex(tempDir);

      if (errors.length > 0) {
        throw new Error(
          `Encountered indexing errors: ${JSON.stringify(errors)}`,
        );
      }
      if (indexedCount < 2) {
        throw new Error(
          `Expected at least 2 indexed files, got ${indexedCount}`,
        );
      }
    });

    // 3. Search crawled documents
    await test("Search finds documents indexed by directory crawler via cross-lingual query", async () => {
      const { results } = await coordinator.search("tìm hóa đơn tháng 8", {
        topK: 3,
      });
      if (results.length === 0)
        throw new Error("Expected search results for crawled documents");

      const topTitle = results[0].document_title;
      if (!topTitle.includes("invoice_august_2026")) {
        throw new Error(
          `Expected invoice document to rank top, got: ${topTitle}`,
        );
      }
    });
  } finally {
    // Clean up temporary test files
    fs.rmSync(tempDir, { recursive: true, force: true });
  }

  console.log("-------------------------------------------------------");
  console.log(`Directory Crawler Tests: ${passed} passed, ${failed} failed`);
  console.log("=======================================================\n");

  if (failed > 0) {
    process.exit(1);
  }
}

runTests();
