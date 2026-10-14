import {
  AiEmbeddingRequest,
  AiEmbeddingResponse,
  AiProvider,
  AiProviderError,
} from "../ai-provider-contract";
import {
  EmbeddingLoadBalancer,
  LocalDeterministicEmbedder,
} from "../embedding-load-balancer";

async function runTests() {
  console.log("\n=======================================================");
  console.log("TS EMBEDDING LOAD BALANCER & FALLBACK TEST SUITE");
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

  const dummy768 = () => new Array(768).fill(0.1);

  // 1. Slot 0 success test
  await test("Slot 0 returns vectors successfully without triggering fallback", async () => {
    let calledModel = "";
    const fakeProvider: AiProvider = {
      id: "google-ai-studio",
      displayName: "Google AI",
      async generate() {
        throw new Error("not used");
      },
      async embed(req) {
        calledModel = req.model;
        return {
          providerId: "google-ai-studio",
          model: req.model,
          vectors: [dummy768()],
          dimensions: 768,
        };
      },
    };

    const balancer = new EmbeddingLoadBalancer(fakeProvider, {
      slots: [
        { modelId: "text-embedding-004", cooldownMs: 1000 },
        { modelId: "embedding-001", cooldownMs: 1000 },
      ],
      localFallback: new LocalDeterministicEmbedder(768),
      unifiedDimensions: 768,
    });

    const res = await balancer.embed(["test query"]);
    if (res.source !== "gemini-slot:text-embedding-004") {
      throw new Error(
        `Expected gemini-slot:text-embedding-004, got ${res.source}`,
      );
    }
    if (calledModel !== "text-embedding-004") {
      throw new Error(
        `Expected calledModel text-embedding-004, got ${calledModel}`,
      );
    }
    if (res.vectors.length !== 1 || res.vectors[0].length !== 768) {
      throw new Error("Invalid vector dimensions");
    }
  });

  // 2. Round-robin load distribution across multiple requests
  await test("Load balancer rotates slots in round-robin fashion", async () => {
    const calledModels: string[] = [];
    const fakeProvider: AiProvider = {
      id: "google-ai-studio",
      displayName: "Google AI",
      async generate() {
        throw new Error("not used");
      },
      async embed(req) {
        calledModels.push(req.model);
        return {
          providerId: "google-ai-studio",
          model: req.model,
          vectors: [dummy768()],
          dimensions: 768,
        };
      },
    };

    const balancer = new EmbeddingLoadBalancer(fakeProvider, {
      slots: [
        { modelId: "slot-A", cooldownMs: 1000 },
        { modelId: "slot-B", cooldownMs: 1000 },
      ],
      localFallback: new LocalDeterministicEmbedder(768),
      unifiedDimensions: 768,
      strategy: "round_robin",
    });

    await balancer.embed(["q1"]);
    await balancer.embed(["q2"]);

    if (calledModels[0] !== "slot-A" || calledModels[1] !== "slot-B") {
      throw new Error(
        `Expected [slot-A, slot-B], got ${JSON.stringify(calledModels)}`,
      );
    }
  });

  // 3. Rate-limit (429) cooldown triggers failover to next slot
  await test("Rate-limit 429 on Slot 0 sets cooldown and seamlessly uses Slot 1", async () => {
    let callCount = 0;
    const fakeProvider: AiProvider = {
      id: "google-ai-studio",
      displayName: "Google AI",
      async generate() {
        throw new Error("not used");
      },
      async embed(req) {
        callCount++;
        if (req.model === "slot-A") {
          throw new AiProviderError({
            message: "Resource exhausted (quota exceeded)",
            code: "QUOTA_EXCEEDED",
            statusCode: 429,
            providerId: "google-ai-studio",
            retryable: true,
          });
        }
        return {
          providerId: "google-ai-studio",
          model: req.model,
          vectors: [dummy768()],
          dimensions: 768,
        };
      },
    };

    const balancer = new EmbeddingLoadBalancer(fakeProvider, {
      slots: [
        { modelId: "slot-A", cooldownMs: 60_000 },
        { modelId: "slot-B", cooldownMs: 60_000 },
      ],
      localFallback: new LocalDeterministicEmbedder(768),
      unifiedDimensions: 768,
    });

    const res = await balancer.embed(["q1"]);
    if (res.source !== "gemini-slot:slot-B") {
      throw new Error(`Expected source gemini-slot:slot-B, got ${res.source}`);
    }
    if (
      !res.attemptedSlots.includes("slot-A") ||
      !res.attemptedSlots.includes("slot-B")
    ) {
      throw new Error("Expected both slot-A and slot-B in attemptedSlots");
    }

    // Next immediate call should skip slot-A because it's in cooldown!
    const res2 = await balancer.embed(["q2"]);
    if (res2.source !== "gemini-slot:slot-B") {
      throw new Error(`Expected res2 to stay on slot-B, got ${res2.source}`);
    }
    if (res2.attemptedSlots.includes("slot-A")) {
      throw new Error("slot-A should have been bypassed during cooldown");
    }
  });

  // 4. Fallback to Local Deterministic Embedder when all slots fail
  await test("Seamless fallback to local offline engine when all remote slots fail", async () => {
    const fakeProvider: AiProvider = {
      id: "google-ai-studio",
      displayName: "Google AI",
      async generate() {
        throw new Error("not used");
      },
      async embed(req) {
        throw new AiProviderError({
          message: `Slot ${req.model} down`,
          code: "PROVIDER_UNAVAILABLE",
          statusCode: 503,
          providerId: "google-ai-studio",
          retryable: true,
        });
      },
    };

    const localEmbedder = new LocalDeterministicEmbedder(768);
    const balancer = new EmbeddingLoadBalancer(fakeProvider, {
      slots: [
        { modelId: "slot-1", cooldownMs: 5000 },
        { modelId: "slot-2", cooldownMs: 5000 },
      ],
      localFallback: localEmbedder,
      unifiedDimensions: 768,
    });

    const res = await balancer.embed(["tìm hóa đơn"]);
    if (res.source !== "local_fallback") {
      throw new Error(`Expected source local_fallback, got ${res.source}`);
    }
    if (res.vectors.length !== 1 || res.vectors[0].length !== 768) {
      throw new Error("Fallback vector dimensions incorrect");
    }
    if (!res.errorDetails || res.errorDetails.length !== 2) {
      throw new Error("Expected 2 error details logged from failed slots");
    }
  });

  // 5. LocalDeterministicEmbedder mathematical consistency check (Zero dictionary)
  await test("LocalDeterministicEmbedder produces deterministic normalized vectors without dictionary", async () => {
    const local = new LocalDeterministicEmbedder(768);
    const [vec1, vec2, vecOther] = await local.embed([
      "search query document",
      "search query document",
      "completely different text",
    ]);

    if (vec1.length !== 768) throw new Error("Incorrect dimensions");

    // Identical texts produce identical vectors
    let dotSame = 0;
    for (let i = 0; i < 768; i++) dotSame += vec1[i] * vec2[i];
    if (Math.abs(dotSame - 1.0) > 1e-4) {
      throw new Error(`Expected identical text cosine ~ 1.0, got ${dotSame}`);
    }

    // Distinct texts produce lower cosine
    let dotDiff = 0;
    for (let i = 0; i < 768; i++) dotDiff += vec1[i] * vecOther[i];
    if (dotDiff > 0.8) {
      throw new Error(`Expected distinct text cosine < 0.8, got ${dotDiff}`);
    }
  });

  console.log("-------------------------------------------------------");
  console.log(
    `Embedding Load Balancer Tests: ${passed} passed, ${failed} failed`,
  );
  console.log("=======================================================\n");

  if (failed > 0) {
    process.exit(1);
  }
}

runTests();
