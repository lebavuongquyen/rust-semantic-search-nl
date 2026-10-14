import {
  AiProvider,
  AiProviderError,
  AiProviderRegistry,
  AiRouter,
} from "../ai-provider-contract";
import {
  GoogleAiStudioSearchOrchestrator,
  ParsedSearchPlan,
} from "../google-ai-studio-orchestrator";

declare const process: any;

// Simple test runner without external dependencies
async function runTests() {
  console.log("\n=======================================================");
  console.log("TS PROVIDER CONTRACT & ORCHESTRATOR TEST SUITE");
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

  // 1. Registry capability rejection test
  await test("AiProviderRegistry throws on unsupported capability", async () => {
    const registry = new AiProviderRegistry();
    const fakeProvider: AiProvider = {
      id: "fake-ai",
      displayName: "Fake AI",
      async describeModel(modelId) {
        return {
          id: modelId,
          providerId: "fake-ai",
          capabilities: new Set(["text_generation"]),
        };
      },
      async generate() {
        return { providerId: "fake-ai", model: "m1", text: "hello" };
      },
    };
    registry.register(fakeProvider);

    let threw = false;
    try {
      await registry.requireCapability("fake-ai", "m1", "text_embedding");
    } catch (e: any) {
      threw = true;
      if (e.code !== "CAPABILITY_UNSUPPORTED") {
        throw new Error(`Expected CAPABILITY_UNSUPPORTED, got ${e.code}`);
      }
    }
    if (!threw) throw new Error("Expected requireCapability to throw");
  });

  // 2. Orchestrator local-only fallback when apiKey is missing
  await test("Orchestrator runs standard pure plan locally without API key", async () => {
    const orchestrator = new GoogleAiStudioSearchOrchestrator({ apiKey: "" });
    const fallbackFn = (q: string): ParsedSearchPlan => ({
      provider: "standard_pure",
      raw_query: q,
      language: "vi",
      normalized_query: q,
      semantic_keywords: ["hóa đơn", "invoice"],
      name_patterns: [],
      code_routes: [],
      temporal_info: null,
      target_extensions: [],
      negative_keywords: [],
    });

    const envelope = await orchestrator.generateSearchPlan(
      "tìm hóa đơn",
      fallbackFn,
    );
    if (envelope.status !== "SUCCESS")
      throw new Error("Expected status SUCCESS");
    if (envelope.plan?.provider !== "standard_pure")
      throw new Error("Expected provider standard_pure");
    if (envelope.diagnostic.attempted !== false)
      throw new Error("Expected attempted=false");
  });

  // 3. Orchestrator fallback on API error (e.g. 429 quota)
  await test("Orchestrator falls back to standard_fallback on 429 quota error", async () => {
    const mockFetcher: typeof fetch = async () => {
      return new Response("Resource exhausted: quota exceeded", {
        status: 429,
        headers: { "x-request-id": "req-429" },
      });
    };

    const orchestrator = new GoogleAiStudioSearchOrchestrator({
      apiKey: "dummy-key",
      fallbackPolicy: "fallback_to_standard",
      fetchImplementation: mockFetcher,
    });

    const fallbackFn = (q: string): ParsedSearchPlan => ({
      provider: "standard_fallback",
      raw_query: q,
      language: "en",
      normalized_query: q,
      semantic_keywords: ["fallback"],
      name_patterns: [],
      code_routes: [],
      temporal_info: null,
      target_extensions: [],
      negative_keywords: [],
    });

    const envelope = await orchestrator.generateSearchPlan(
      "test query",
      fallbackFn,
    );
    if (envelope.status !== "SUCCESS")
      throw new Error("Expected status SUCCESS");
    if (envelope.plan?.provider !== "standard_fallback")
      throw new Error("Expected provider standard_fallback");
    if (envelope.diagnostic.errorCode !== "QUOTA_EXCEEDED") {
      throw new Error(
        `Expected QUOTA_EXCEEDED, got ${envelope.diagnostic.errorCode}`,
      );
    }
  });

  // 4. Orchestrator strict_fail policy
  await test("Orchestrator strict_fail policy immediately rejects on error", async () => {
    const mockFetcher: typeof fetch = async () => {
      return new Response("Unauthorized", { status: 401 });
    };

    const orchestrator = new GoogleAiStudioSearchOrchestrator({
      apiKey: "invalid-key",
      fallbackPolicy: "strict_fail",
      fetchImplementation: mockFetcher,
    });

    const envelope = await orchestrator.generateSearchPlan("test query", () => {
      throw new Error("Should not call fallback");
    });

    if (envelope.status !== "FAILED") throw new Error("Expected status FAILED");
    if (envelope.diagnostic.errorCode !== "UNAUTHORIZED") {
      throw new Error(
        `Expected UNAUTHORIZED, got ${envelope.diagnostic.errorCode}`,
      );
    }
  });

  // 5. Successful mock plan generation
  await test("Orchestrator generates and validates plan from Gemini JSON response", async () => {
    const mockPlanJson = JSON.stringify({
      language: "vi",
      normalized_query: "tìm hóa đơn",
      semantic_keywords: ["hóa đơn", "invoice", "receipt"],
      name_patterns: ["*invoice*.*"],
      code_routes: [],
      temporal_info: { month: 8, year: 2026, patterns: ["tháng 08"] },
      target_extensions: ["pdf"],
      negative_keywords: [],
    });

    const mockFetcher: typeof fetch = async () => {
      return new Response(
        JSON.stringify({
          candidates: [
            {
              content: { parts: [{ text: mockPlanJson }] },
              finishReason: "STOP",
            },
          ],
        }),
        { status: 200, headers: { "Content-Type": "application/json" } },
      );
    };

    const orchestrator = new GoogleAiStudioSearchOrchestrator({
      apiKey: "valid-key",
      fetchImplementation: mockFetcher,
    });

    const envelope = await orchestrator.generateSearchPlan(
      "tìm hóa đơn",
      () => {
        throw new Error("Fallback should not be called");
      },
    );

    if (envelope.status !== "SUCCESS")
      throw new Error("Expected status SUCCESS");
    if (envelope.plan?.provider !== "gemini")
      throw new Error("Expected provider gemini");
    if (!envelope.plan?.semantic_keywords.includes("invoice")) {
      throw new Error("Expected semantic keywords to include invoice");
    }
  });

  // 6. Direct GoogleAiStudioProvider contract test: Authentication error (401)
  await test("GoogleAiStudioProvider maps 401 response to AUTHENTICATION error code", async () => {
    const { GoogleAiStudioProvider } =
      await import("../google-ai-studio-provider");
    const mockFetcher: typeof fetch = async () => {
      return new Response(
        JSON.stringify({ error: { message: "API key invalid" } }),
        { status: 401 },
      );
    };
    const provider = new GoogleAiStudioProvider({
      apiKey: "bad-key",
      fetchImplementation: mockFetcher,
    });
    let threw = false;
    try {
      await provider.generate({
        model: "gemini-2.0-flash",
        messages: [{ role: "user", content: "test" }],
      });
    } catch (e: any) {
      threw = true;
      if (e.code !== "AUTHENTICATION")
        throw new Error(`Expected AUTHENTICATION, got ${e.code}`);
      if (e.statusCode !== 401)
        throw new Error(`Expected 401, got ${e.statusCode}`);
      if (e.retryable !== false) throw new Error("401 must not be retryable");
    }
    if (!threw) throw new Error("Expected 401 to throw");
  });

  // 7. Direct GoogleAiStudioProvider contract test: Rate limit / Quota error (429)
  await test("GoogleAiStudioProvider maps 429 rate limit to RATE_LIMITED or QUOTA_EXCEEDED with retryable=true", async () => {
    const { GoogleAiStudioProvider } =
      await import("../google-ai-studio-provider");
    const mockFetcher: typeof fetch = async () => {
      return new Response("Too Many Requests", { status: 429 });
    };
    const provider = new GoogleAiStudioProvider({
      apiKey: "test-key",
      fetchImplementation: mockFetcher,
    });
    let threw = false;
    try {
      await provider.generate({
        model: "gemini-2.0-flash",
        messages: [{ role: "user", content: "test" }],
      });
    } catch (e: any) {
      threw = true;
      if (e.code !== "RATE_LIMITED" && e.code !== "QUOTA_EXCEEDED")
        throw new Error(`Unexpected code: ${e.code}`);
      if (e.retryable !== true) throw new Error("429 must be retryable");
    }
    if (!threw) throw new Error("Expected 429 to throw");
  });

  // 8. Direct GoogleAiStudioProvider contract test: Embedding invalid vector validation
  await test("GoogleAiStudioProvider rejects malformed embedding vector containing NaN", async () => {
    const { GoogleAiStudioProvider } =
      await import("../google-ai-studio-provider");
    const mockFetcher: typeof fetch = async () => {
      return new Response(
        JSON.stringify({ embedding: { values: [0.1, null, 0.3] } }),
        { status: 200 },
      );
    };
    const provider = new GoogleAiStudioProvider({
      apiKey: "test-key",
      fetchImplementation: mockFetcher,
    });
    let threw = false;
    try {
      await provider.embed({
        model: "text-embedding-004",
        inputs: ["test text"],
      });
    } catch (e: any) {
      threw = true;
      if (e.code !== "INVALID_RESPONSE")
        throw new Error(`Expected INVALID_RESPONSE, got ${e.code}`);
    }
    if (!threw) throw new Error("Expected invalid embedding to throw");
  });

  // 9. Direct GoogleAiStudioProvider contract test: AbortSignal / Timeout handling
  await test("GoogleAiStudioProvider respects AbortSignal and marks error as TIMEOUT/NETWORK", async () => {
    const { GoogleAiStudioProvider } =
      await import("../google-ai-studio-provider");
    const controller = new AbortController();
    controller.abort(); // already aborted

    const provider = new GoogleAiStudioProvider({ apiKey: "test-key" });
    let threw = false;
    try {
      await provider.generate({
        model: "gemini-2.0-flash",
        messages: [{ role: "user", content: "test" }],
        options: { signal: controller.signal },
      });
    } catch (e: any) {
      threw = true;
      if (e.code !== "TIMEOUT" && e.code !== "NETWORK")
        throw new Error(`Expected TIMEOUT or NETWORK, got ${e.code}`);
    }
    if (!threw) throw new Error("Expected aborted request to throw");
  });

  console.log("-------------------------------------------------------");
  console.log(`TS Test Results: ${passed} passed, ${failed} failed`);
  console.log("=======================================================\n");

  if (failed > 0) {
    process.exit(1);
  }
}

runTests();
