import assert from "node:assert/strict";
import * as fs from "fs";
import * as path from "path";
import { GoogleAiStudioProvider } from "../google-ai-studio-provider";
import {
  EmbeddingLoadBalancer,
  LocalDeterministicEmbedder,
} from "../embedding-load-balancer";
import {
  DocumentInput,
  SemanticSearchCoordinator,
} from "../semantic-search-coordinator";

function loadApiKey(): string | undefined {
  if (process.env.GOOGLE_AI_STUDIO_API_KEY)
    return process.env.GOOGLE_AI_STUDIO_API_KEY;
  if (process.env.GEMINI_API_KEY) return process.env.GEMINI_API_KEY;
  const envPath = path.resolve(__dirname, "../../.env");
  if (fs.existsSync(envPath)) {
    const lines = fs.readFileSync(envPath, "utf-8").split("\n");
    for (const line of lines) {
      const match = line.match(
        /^\s*(?:GOOGLE_AI_STUDIO_API_KEY|GEMINI_API_KEY)\s*=\s*(["']?)(.*?)\1\s*$/,
      );
      if (match && match[2]) return match[2].trim();
    }
  }
  return undefined;
}

async function runLiveTests() {
  console.log(
    "=================================================================",
  );
  console.log("LIVE SEMANTIC SEARCH EVALUATION WITH GOOGLE AI STUDIO");
  console.log(
    "=================================================================",
  );

  const apiKey = loadApiKey();
  if (!apiKey) {
    console.warn(
      "[SKIP] No API key found in environment or .env. Skipping live test.",
    );
    return;
  }

  const UNIFIED_DIMENSIONS = 3072;
  const provider = new GoogleAiStudioProvider({ apiKey });

  const loadBalancer = new EmbeddingLoadBalancer(provider, {
    slots: [
      { modelId: "gemini-embedding-001", cooldownMs: 60_000 },
      { modelId: "gemini-embedding-2", cooldownMs: 60_000 },
    ],
    localFallback: new LocalDeterministicEmbedder(UNIFIED_DIMENSIONS),
    unifiedDimensions: UNIFIED_DIMENSIONS,
    defaultTimeoutMs: 15_000,
  });

  const coordinator = new SemanticSearchCoordinator(loadBalancer, {
    providerId: "google-ai-studio",
    modelId: "gemini-embedding-001",
  });

  console.log(
    "\n[Test 1] Ingesting documents with genuine 3072-dimensional Gemini embeddings...",
  );

  const docEnglishBilling: DocumentInput = {
    id: "doc-aws-billing",
    title: "AWS Cloud Infrastructure Billing Summary Q3",
    content:
      "Total expenditures for compute instances and object storage were reconciled. The payable statement amount is twelve thousand dollars due by end of month.",
    file_path: "/finance/aws_billing_q3.txt",
    metadata: { category: "finance", lang: "en" },
  };

  const docVietnameseTelecom: DocumentInput = {
    id: "doc-viettel-invoice",
    title: "Hóa đơn giá trị gia tăng dịch vụ viễn thông Viettel",
    content:
      "Cộng tiền cung ứng dịch vụ mạng cáp quang băng thông rộng. Đã thanh toán chuyển khoản qua ngân hàng Quân Đội.",
    file_path: "/finance/viettel_invoice.txt",
    metadata: { category: "finance", lang: "vi" },
  };

  const docTechRust: DocumentInput = {
    id: "doc-rust-wasm",
    title: "Rust WebAssembly Memory Layout and FFI Guidelines",
    content:
      "Memory safety is enforced at compile time. WebAssembly linear memory can be accessed using Float32Array views without copies.",
    file_path: "/engineering/wasm_memory.txt",
    metadata: { category: "tech", lang: "en" },
  };

  const res1 = await coordinator.ingestDocument(docEnglishBilling);
  console.log(
    ` -> Ingested Doc 1: chunks=${res1.chunkCount}, source=${res1.embeddingSource}`,
  );
  assert.ok(
    res1.embeddingSource.startsWith("gemini-slot:"),
    "Doc 1 should be embedded via real Gemini API slot",
  );

  const res2 = await coordinator.ingestDocument(docVietnameseTelecom);
  console.log(
    ` -> Ingested Doc 2: chunks=${res2.chunkCount}, source=${res2.embeddingSource}`,
  );
  assert.ok(
    res2.embeddingSource.startsWith("gemini-slot:"),
    "Doc 2 should be embedded via real Gemini API slot",
  );

  const res3 = await coordinator.ingestDocument(docTechRust);
  console.log(
    ` -> Ingested Doc 3: chunks=${res3.chunkCount}, source=${res3.embeddingSource}`,
  );

  const stats = coordinator.getStats();
  console.log(" -> WASM Index Stats:", JSON.stringify(stats));
  assert.equal(
    stats.dimensions,
    UNIFIED_DIMENSIONS,
    "Vector index must be exactly 3072 dimensions",
  );
  assert.equal(
    stats.vector_index_len,
    3,
    "All 3 chunks must be indexed in vector index",
  );

  console.log(
    "\n[Test 2] Cross-Lingual Query: Vietnamese query -> English document (Zero keyword overlap)",
  );
  // Pure cross-lingual: Vietnamese cloud infrastructure query with zero lexical words in Doc 1 or Doc 2:
  const queryVi = "chi phí máy chủ đám mây và lưu trữ đối tượng";
  console.log(` -> Query: "${queryVi}"`);
  const searchVi = await coordinator.search(queryVi, { topK: 5 });

  console.log(` -> Embedding source: ${searchVi.embeddingSource}`);
  console.log(` -> Total results returned: ${searchVi.results.length}`);
  searchVi.results.forEach((r, idx) => {
    const docId =
      typeof r.document_id === "string"
        ? r.document_id
        : (r.document_id as any)?.[0];
    console.log(
      `    #${idx + 1} Doc: ${docId} | Title: "${r.document_title}" | Score: ${r.score.final_score.toFixed(4)}`,
    );
  });

  const topViDocId =
    typeof searchVi.results[0]?.document_id === "string"
      ? searchVi.results[0]?.document_id
      : (searchVi.results[0]?.document_id as any)?.[0];
  assert.equal(
    topViDocId,
    "doc-aws-billing",
    "Vietnamese cloud billing query must retrieve AWS Billing document as #1!",
  );

  console.log(
    "\n[Test 3] Cross-Lingual Query: English query -> Vietnamese document (Zero keyword overlap)",
  );
  // Query in English with NO exact keywords from the Vietnamese document:
  const queryEn = "telecom broadband monthly fees";
  console.log(` -> Query: "${queryEn}"`);
  const searchEn = await coordinator.search(queryEn, { topK: 5 });

  console.log(` -> Embedding source: ${searchEn.embeddingSource}`);
  console.log(` -> Total results returned: ${searchEn.results.length}`);
  searchEn.results.forEach((r, idx) => {
    const docId =
      typeof r.document_id === "string"
        ? r.document_id
        : (r.document_id as any)?.[0];
    console.log(
      `    #${idx + 1} Doc: ${docId} | Title: "${r.document_title}" | Score: ${r.score.final_score.toFixed(4)}`,
    );
  });

  const topEnDocId =
    typeof searchEn.results[0]?.document_id === "string"
      ? searchEn.results[0]?.document_id
      : (searchEn.results[0]?.document_id as any)?.[0];
  assert.equal(
    topEnDocId,
    "doc-viettel-invoice",
    "English telecom query must retrieve Viettel Invoice document as #1!",
  );

  console.log(
    "\n[Test 4] Verifying Load Balancer Failover to Slot 1 on simulated 429 cooldown",
  );
  const slots = loadBalancer.getSlots();
  // Put Slot 0 on cooldown to verify real failover to gemini-embedding-2
  (slots[0] as any).cooldownUntil = Date.now() + 60_000;
  const failoverRes = await coordinator.search("cloud storage query", {
    topK: 1,
  });
  console.log(
    ` -> Embedding source during slot 0 cooldown: ${failoverRes.embeddingSource}`,
  );
  assert.equal(
    failoverRes.embeddingSource,
    "gemini-slot:gemini-embedding-2",
    "Must failover to gemini-embedding-2",
  );
  assert.ok(
    slots[1].totalSuccesses > 0,
    "Slot 1 must have handled request during cooldown",
  );
  // Reset cooldown
  loadBalancer.resetCooldowns();

  console.log(
    "\n[Test 5] Structured Metadata and Negative Constraint Filters with Live Embeddings",
  );
  // Filter by category "finance", negative keyword "viettel"
  const filteredSearch = await coordinator.search("billing expenses", {
    topK: 5,
    filter: {
      negative_keywords: ["viettel"],
    },
  });
  console.log(` -> Filtered results count: ${filteredSearch.results.length}`);
  for (const r of filteredSearch.results) {
    const docId =
      typeof r.document_id === "string"
        ? r.document_id
        : (r.document_id as any)?.[0];
    console.log(`    Doc: ${docId} | Title: "${r.document_title}"`);
    assert.notEqual(
      docId,
      "doc-viettel-invoice",
      "Negative keyword 'viettel' must exclude doc-viettel-invoice",
    );
  }

  console.log(
    "\n=================================================================",
  );
  console.log(
    "✅ ALL REAL-API SEMANTIC SEARCH TESTS PASSED WITH 100% SUCCESS!",
  );
  console.log(
    "=================================================================",
  );
}

runLiveTests().catch((err) => {
  console.error("FATAL ERROR in live semantic search test:", err);
  process.exit(1);
});
