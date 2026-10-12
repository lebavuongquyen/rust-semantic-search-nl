# AI Provider Integration Architecture

## Goal

Allow SemanticSearchAgent to use AI model APIs through replaceable adapters while keeping search policy, query planning, relevance scoring, and retrieval independent of vendor SDKs.

## Contracts

- `AiProvider` defines model discovery, text generation, and optional embedding operations.
- `AiModelDescriptor` declares capabilities per model. A provider name or API compatibility is not proof that a model supports a feature.
- `AiProviderRegistry` owns provider registration and model capability validation.
- `AiRouter` selects a configured route and can attempt explicit fallback routes.
- `AiProviderError` normalizes operational failures while retaining provider identity, HTTP status, retryability, and request ID when available.

## Current adapter

`GoogleAiStudioProvider` uses the Google Generative Language REST API directly through the standard Fetch API. It does not require a Google SDK or a third-party runtime library. It supports model discovery, text generation, JSON response mode, and text embeddings.

Model IDs are discovered from the API rather than compiled into a union type. The model descriptor only advertises capabilities that the adapter currently implements. Additional modalities and streaming must not be advertised until the request and response contracts implement them and tests cover them.

## Usage

```ts
import { AiProviderRegistry, AiRouter } from "./ai-provider-contract";
import { GoogleAiStudioProvider } from "./google-ai-studio-provider";

const registry = new AiProviderRegistry();
registry.register(new GoogleAiStudioProvider({
  apiKey: readSecretFromSecureConfiguration(),
}));

const models = await registry.get("google-ai-studio").listModels?.();
const selected = models?.find((model) =>
  model.capabilities.has("text_generation") &&
  model.capabilities.has("structured_output")
);

if (!selected) {
  throw new Error("No compatible Google AI Studio model is available");
}

const router = new AiRouter(registry, { allowFallback: false });
const response = await router.generate({
  providerId: selected.providerId,
  modelId: selected.id,
  requiredCapabilities: ["text_generation", "structured_output"],
}, {
  messages: [
    { role: "system", content: "Return a JSON search plan." },
    { role: "user", content: "Tìm hóa đơn" },
  ],
  options: { responseFormat: "json", temperature: 0.1, timeoutMs: 10000 },
});
```

## Security and privacy

- Never commit API keys or print them in logs.
- Browser-side API keys are visible to users. Production applications should use a trusted backend or a user-controlled local secret store, and should clearly disclose when query/document content is sent to a remote provider.
- The local-only search path must continue to work without a network connection or AI provider.
- Do not automatically upload document contents to generate embeddings. Remote embedding requires an explicit, separately configured consent/policy decision.
- Redact query text and provider response bodies from telemetry unless the user explicitly enables content diagnostics.

## Error and fallback policy

Errors are normalized into authentication, permission, quota, rate limit, timeout, network, invalid request, missing model, unsupported capability, provider unavailable, invalid response, and unknown categories. Only retryable failures should be retried automatically. Fallback providers/models must be explicitly configured and must satisfy the same required capabilities. A fallback must not silently change privacy policy or move private content to another provider.

## Next implementation gates

1. Add deterministic contract tests for registration, capability rejection, timeout, abort, malformed responses, and fallback selection.
2. Add a second provider adapter to prove that the contract is genuinely vendor-neutral.
3. Add query-planner schema validation and reject malformed or constraint-dropping plans.
4. Add embedding dimension/model identity tracking and prevent vectors from incompatible embedding spaces from being mixed.
5. Add bounded retries with backoff, rate limiting, cost budgets, request correlation, and secret-safe diagnostics.
6. Add explicit consent and policy enforcement for every remote-content operation.
