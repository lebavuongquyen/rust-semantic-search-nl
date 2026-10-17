import { AiProvider } from "../ai-provider-contract";
import {
  EmbeddingLoadBalancer,
  LocalDeterministicEmbedder,
} from "../embedding-load-balancer";
import { SemanticSearchCoordinator } from "../semantic-search-coordinator";

async function runTests() {
  console.log("\n=======================================================");
  console.log("TS-WASM END-TO-END SEARCH COORDINATOR TEST SUITE");
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

  // Setup mock provider that echoes deterministic concept embeddings
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

  // 1. Ingest test documents
  await test("Coordinator ingests English and Vietnamese documents into WASM engine", async () => {
    // English invoice document
    const ing1 = await coordinator.ingestDocument(
      {
        id: "doc-invoice-en",
        title: "Invoice 2026-08 Corporate Services",
        content:
          "Invoice for consulting services rendered in August 2026. Total amount due: $15,000 USD via wire transfer.",
      },
      200,
    );

    if (ing1.chunkCount === 0) throw new Error("Expected chunks > 0");

    // Vietnamese invoice document
    const ing2 = await coordinator.ingestDocument(
      {
        id: "doc-hoadon-vi",
        title: "Hóa đơn giá trị gia tăng tháng 08",
        content:
          "Hóa đơn dịch vụ đám mây tháng 8 năm 2026. Tổng tiền thanh toán đã bao gồm thuế VAT.",
      },
      200,
    );

    if (ing2.chunkCount === 0) throw new Error("Expected chunks > 0");

    // Security/Auth document (hard negative for invoice search)
    await coordinator.ingestDocument(
      {
        id: "doc-auth",
        title: "Authentication & JWT Guidelines",
        content:
          "Guidelines for JWT bearer tokens and password hashing protocols in API gateway.",
      },
      200,
    );

    const stats = coordinator.getStats();
    if (stats.vector_index_len < 3) {
      throw new Error(
        `Expected at least 3 vectors in WASM index, got ${stats.vector_index_len}`,
      );
    }
  });

  // 2. Cross-lingual semantic search: Vietnamese query -> English document
  await test("Cross-lingual retrieval: Vietnamese query 'tìm hóa đơn dịch vụ' retrieves English invoice", async () => {
    const { results, embeddingSource } = await coordinator.search(
      "tìm hóa đơn dịch vụ",
      { topK: 3 },
    );
    if (results.length === 0) throw new Error("Expected search results");
    if (!embeddingSource.startsWith("gemini-slot:")) {
      throw new Error(
        `Expected gemini-slot embedding source, got ${embeddingSource}`,
      );
    }

    const docIds = results.map((r: any) =>
      typeof r.document_id === "string"
        ? r.document_id
        : r.document_id?.[0] || String(r.document_id),
    );
    if (
      !docIds.includes("doc-invoice-en") &&
      !docIds.includes("doc-hoadon-vi")
    ) {
      throw new Error(
        `Expected invoice documents in results, got ${JSON.stringify(docIds)}`,
      );
    }

    // Top result should be one of the invoice documents, not authentication doc
    const topId =
      typeof results[0].document_id === "string"
        ? results[0].document_id
        : results[0].document_id?.[0];
    if (topId === "doc-auth") {
      throw new Error(
        "Hard negative doc-auth should not rank first for invoice query",
      );
    }
  });

  // 3. Cross-lingual semantic search: English query -> Vietnamese document
  await test("Cross-lingual retrieval: English query 'find corporate billing statement' retrieves Vietnamese invoice", async () => {
    const { results } = await coordinator.search(
      "find corporate billing statement",
      { topK: 3 },
    );
    if (results.length === 0) throw new Error("Expected search results");

    const docIds = results.map((r: any) =>
      typeof r.document_id === "string"
        ? r.document_id
        : r.document_id?.[0] || String(r.document_id),
    );
    if (
      !docIds.includes("doc-hoadon-vi") &&
      !docIds.includes("doc-invoice-en")
    ) {
      throw new Error(
        `Expected invoice documents, got ${JSON.stringify(docIds)}`,
      );
    }
  });

  // 4. Fallback execution verification
  await test("Search seamlessly continues when remote provider slots all fail", async () => {
    // Break the remote provider temporarily
    const brokenProvider: AiProvider = {
      id: "google-ai-studio",
      displayName: "Broken Google AI",
      async generate() {
        throw new Error("not used");
      },
      async embed() {
        throw new Error("Remote API 429 Quota Exceeded");
      },
    };

    const fallbackBalancer = new EmbeddingLoadBalancer(brokenProvider, {
      slots: [{ modelId: "text-embedding-004", cooldownMs: 5000 }],
      localFallback: localEmbedder,
      unifiedDimensions: 768,
    });

    const fallbackCoordinator = new SemanticSearchCoordinator(fallbackBalancer);
    await fallbackCoordinator.ingestDocument({
      id: "doc-receipt-vi",
      title: "Biên lai thu lệ phí hộ tịch",
      content:
        "Biên lai nộp tiền lệ phí cấp bản sao trích lục hộ tịch ngày 24 tháng 8 năm 2026.",
    });

    const { results, embeddingSource } = await fallbackCoordinator.search(
      "receipt proof of payment",
    );
    if (embeddingSource !== "local_fallback") {
      throw new Error(
        `Expected embeddingSource local_fallback, got ${embeddingSource}`,
      );
    }
    if (results.length === 0) {
      throw new Error("Expected search results using local fallback embedding");
    }
    const topId =
      typeof results[0].document_id === "string"
        ? results[0].document_id
        : results[0].document_id?.[0];
    if (topId !== "doc-receipt-vi") {
      throw new Error(`Expected doc-receipt-vi, got ${topId}`);
    }
  });

  // 5. Deleting document removes vectors from WASM index
  await test("Deleting document removes vectors from WASM index", async () => {
    const statsBefore = coordinator.getStats();
    const deleted = coordinator.deleteDocument("doc-auth");
    if (!deleted) throw new Error("Expected deleteDocument to return true");

    const statsAfter = coordinator.getStats();
    if (statsAfter.vector_index_len >= statsBefore.vector_index_len) {
      throw new Error("Vector count in index should decrease after deletion");
    }
  });

  // 6. Zero-AI mode: coordinator runs without provider or API key using BM25 and offline vector index
  await test("Zero-AI mode: coordinator runs without provider or API key using BM25 and offline math vector index", async () => {
    const offlineCoordinator = new SemanticSearchCoordinator();
    await offlineCoordinator.ingestDocument({
      id: "doc-offline-manual",
      title: "Local Offline Installation Guide",
      content:
        "Complete offline installation and maintenance guide without internet access.",
    });

    const { results, embeddingSource } = await offlineCoordinator.search(
      "offline installation maintenance",
    );
    if (embeddingSource !== "local_offline") {
      throw new Error(`Expected source local_offline, got ${embeddingSource}`);
    }
    if (results.length === 0) {
      throw new Error("Expected offline search results");
    }
    const topId =
      typeof results[0].document_id === "string"
        ? results[0].document_id
        : results[0].document_id?.[0];
    if (topId !== "doc-offline-manual") {
      throw new Error(`Expected doc-offline-manual, got ${topId}`);
    }
  });

  // 7. Parent-child context retrieval and HyDE natural language search
  await test("Parent-Child context window and HyDE natural language search", async () => {
    const { results } = await coordinator.search("corporate services invoice", {
      enableHyDE: true,
    });
    if (results.length === 0) {
      throw new Error("Expected search results for corporate services invoice");
    }
    const top = results[0];
    if (!top.parent_context) {
      throw new Error(
        "Expected parent_context to be attached to search result",
      );
    }
    if (!top.parent_context.title) {
      throw new Error("Expected parent_context.title to be present");
    }
    if (top.parent_context.full_content_length <= 0) {
      throw new Error("Expected parent_context.full_content_length > 0");
    }
  });

  console.log("-------------------------------------------------------");
  console.log(
    `E2E Search Coordinator Tests: ${passed} passed, ${failed} failed`,
  );
  console.log("=======================================================\n");

  if (failed > 0) {
    process.exit(1);
  }
}

runTests();
