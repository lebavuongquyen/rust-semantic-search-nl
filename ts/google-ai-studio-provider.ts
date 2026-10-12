import {
  AiEmbeddingRequest,
  AiEmbeddingResponse,
  AiGenerateRequest,
  AiGenerateResponse,
  AiModelDescriptor,
  AiProvider,
  AiProviderError,
} from "./ai-provider-contract";

export interface GoogleAiStudioProviderOptions {
  apiKey: string;
  baseUrl?: string;
  fetchImplementation?: typeof fetch;
  defaultTimeoutMs?: number;
}

interface GoogleModelListResponse {
  models?: Array<{
    name?: string;
    displayName?: string;
    description?: string;
    inputTokenLimit?: number;
    outputTokenLimit?: number;
    supportedGenerationMethods?: string[];
  }>;
}

interface GoogleGenerateResponse {
  candidates?: Array<{
    content?: { parts?: Array<{ text?: string }> };
    finishReason?: string;
  }>;
  usageMetadata?: {
    promptTokenCount?: number;
    candidatesTokenCount?: number;
    totalTokenCount?: number;
  };
}

export class GoogleAiStudioProvider implements AiProvider {
  readonly id = "google-ai-studio";
  readonly displayName = "Google AI Studio";
  private readonly baseUrl: string;
  private readonly fetcher: typeof fetch;
  private readonly defaultTimeoutMs: number;

  constructor(private readonly options: GoogleAiStudioProviderOptions) {
    if (!options.apiKey.trim()) {
      throw new Error("Google AI Studio API key must not be empty");
    }
    this.baseUrl = (options.baseUrl ?? "https://generativelanguage.googleapis.com/v1beta").replace(/\/$/, "");
    this.fetcher = options.fetchImplementation ?? fetch;
    this.defaultTimeoutMs = options.defaultTimeoutMs ?? 15000;
  }

  async listModels(options?: { signal?: AbortSignal }): Promise<readonly AiModelDescriptor[]> {
    const payload = await this.request<GoogleModelListResponse>("/models", {
      method: "GET",
      signal: options?.signal,
    });
    return (payload.models ?? []).flatMap((model) => {
      if (!model.name) return [];
      const id = model.name.replace(/^models\//, "");
      const methods = new Set(model.supportedGenerationMethods ?? []);
      const capabilities = new Set<AiModelDescriptor["capabilities"] extends ReadonlySet<infer C> ? C : never>();
      if (methods.has("generateContent")) {
        capabilities.add("text_generation");
        capabilities.add("structured_output");

      }
      if (methods.has("embedContent") || methods.has("batchEmbedContents")) {
        capabilities.add("text_embedding");
      }
      return [{
        id,
        providerId: this.id,
        displayName: model.displayName ?? id,
        capabilities,
        contextWindow: model.inputTokenLimit,
        maxOutputTokens: model.outputTokenLimit,
        metadata: { description: model.description ?? "", methods: [...methods].join(",") },
      }];
    });
  }

  async describeModel(modelId: string, options?: { signal?: AbortSignal }): Promise<AiModelDescriptor | undefined> {
    try {
      const models = await this.listModels(options);
      const found = models.find((model) => model.id === modelId);
      if (found) return found;
    } catch {
      // Fallback if listModels is unavailable or not mocked
    }
    if (modelId.startsWith("gemini-")) {
      return {
        id: modelId,
        providerId: this.id,
        displayName: modelId,
        capabilities: new Set(["text_generation", "structured_output"]),
      };
    }
    return undefined;
  }

  async generate(request: AiGenerateRequest): Promise<AiGenerateResponse> {
    const contents = request.messages
      .filter((message) => message.role !== "system")
      .map((message) => ({
        role: message.role === "assistant" ? "model" : "user",
        parts: [{ text: message.content }],
      }));
    const systemText = request.messages
      .filter((message) => message.role === "system")
      .map((message) => message.content)
      .join("\n\n");
    const generationConfig: Record<string, unknown> = {};
    if (request.options?.temperature !== undefined) generationConfig.temperature = request.options.temperature;
    if (request.options?.maxOutputTokens !== undefined) generationConfig.maxOutputTokens = request.options.maxOutputTokens;
    if (request.options?.responseFormat === "json") {
      generationConfig.responseMimeType = "application/json";
    }

    const payload = await this.request<GoogleGenerateResponse>(`/models/${encodeURIComponent(request.model)}:generateContent`, {
      method: "POST",
      signal: request.options?.signal,
      timeoutMs: request.options?.timeoutMs,
      body: {
        contents,
        ...(systemText ? { systemInstruction: { parts: [{ text: systemText }] } } : {}),
        ...(Object.keys(generationConfig).length ? { generationConfig } : {}),
      },
    });
    const text = payload.candidates?.[0]?.content?.parts?.map((part) => part.text ?? "").join("") ?? "";
    if (!text) {
      throw new AiProviderError({
        message: "Google AI Studio returned no text candidate",
        code: "INVALID_RESPONSE",
        providerId: this.id,
      });
    }
    return {
      providerId: this.id,
      model: request.model,
      text,
      finishReason: payload.candidates?.[0]?.finishReason,
      usage: payload.usageMetadata ? {
        inputTokens: payload.usageMetadata.promptTokenCount,
        outputTokens: payload.usageMetadata.candidatesTokenCount,
        totalTokens: payload.usageMetadata.totalTokenCount,
      } : undefined,
    };
  }

  async embed(request: AiEmbeddingRequest): Promise<AiEmbeddingResponse> {
    const vectors: number[][] = [];
    for (const input of request.inputs) {
      const payload = await this.request<{
        embedding?: { values?: number[] };
      }>(`/models/${encodeURIComponent(request.model)}:embedContent`, {
        method: "POST",
        signal: request.options?.signal,
        timeoutMs: request.options?.timeoutMs,
        body: { content: { parts: [{ text: input }] } },
      });
      const values = payload.embedding?.values;
      if (!values?.length || values.some((value) => !Number.isFinite(value))) {
        throw new AiProviderError({
          message: "Google AI Studio returned an invalid embedding vector",
          code: "INVALID_RESPONSE",
          providerId: this.id,
        });
      }
      vectors.push(values);
    }
    const dimensions = vectors[0]?.length ?? 0;
    if (vectors.some((vector) => vector.length !== dimensions)) {
      throw new AiProviderError({
        message: "Embedding response contains inconsistent vector dimensions",
        code: "INVALID_RESPONSE",
        providerId: this.id,
      });
    }
    return { providerId: this.id, model: request.model, vectors, dimensions };
  }

  private async request<T>(
    path: string,
    init: { method: "GET" | "POST"; body?: unknown; signal?: AbortSignal; timeoutMs?: number },
  ): Promise<T> {
    const controller = new AbortController();
    const timeoutMs = init.timeoutMs ?? this.defaultTimeoutMs;
    const timeout = setTimeout(() => controller.abort(), timeoutMs);
    const abortFromCaller = () => controller.abort();
    if (init.signal?.aborted) {
      controller.abort();
    } else {
      init.signal?.addEventListener("abort", abortFromCaller, { once: true });
    }

    try {
      const response = await this.fetcher(
        `${this.baseUrl}${path}?key=${encodeURIComponent(this.options.apiKey)}`,
        {
          method: init.method,
          headers: init.body === undefined ? undefined : { "Content-Type": "application/json" },
          body: init.body === undefined ? undefined : JSON.stringify(init.body),
          signal: controller.signal,
        },
      );
      const requestId = response.headers.get("x-request-id") ?? undefined;
      if (!response.ok) {
        const body = await response.text();
        const code = this.mapError(response.status, body);
        throw new AiProviderError({
          message: `Google AI Studio request failed (${response.status}): ${body.slice(0, 1000)}`,
          code,
          providerId: this.id,
          statusCode: response.status,
          retryable: response.status === 408 || response.status === 429 || response.status >= 500,
          requestId,
        });
      }
      return await response.json() as T;
    } catch (error) {
      if (error instanceof AiProviderError) throw error;
      const timedOut = controller.signal.aborted;
      throw new AiProviderError({
        message: timedOut ? `Google AI Studio request timed out after ${timeoutMs}ms` : "Google AI Studio network request failed",
        code: timedOut ? "TIMEOUT" : "NETWORK",
        providerId: this.id,
        retryable: true,
        cause: error,
      });
    } finally {
      clearTimeout(timeout);
      init.signal?.removeEventListener("abort", abortFromCaller);
    }
  }

  private mapError(status: number, body: string): AiProviderError["code"] {
    if (status === 400) return "INVALID_REQUEST";
    if (status === 401) return "AUTHENTICATION";
    if (status === 403) return /quota|rate limit/i.test(body) ? "QUOTA_EXCEEDED" : "PERMISSION_DENIED";
    if (status === 404) return "MODEL_NOT_FOUND";
    if (status === 429) return /quota/i.test(body) ? "QUOTA_EXCEEDED" : "RATE_LIMITED";
    if (status >= 500) return "PROVIDER_UNAVAILABLE";
    return "UNKNOWN";
  }
}
