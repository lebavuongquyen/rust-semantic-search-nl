export type AiCapability =
  | "text_generation"
  | "structured_output"
  | "streaming"
  | "text_embedding"
  | "image_input"
  | "audio_input"
  | "tool_calling"
  | "model_discovery";

export type AiProviderId = string;
export type AiModelId = string;

export interface AiModelDescriptor {
  id: AiModelId;
  providerId: AiProviderId;
  displayName?: string;
  capabilities: ReadonlySet<AiCapability>;
  inputModalities?: readonly string[];
  outputModalities?: readonly string[];
  contextWindow?: number;
  maxOutputTokens?: number;
  dimensions?: number;
  languages?: readonly string[];
  metadata?: Readonly<Record<string, string | number | boolean>>;
}

export interface AiRequestOptions {
  timeoutMs?: number;
  signal?: AbortSignal;
  temperature?: number;
  maxOutputTokens?: number;
  responseFormat?: "text" | "json";
  metadata?: Readonly<Record<string, string>>;
}

export interface AiMessage {
  role: "system" | "user" | "assistant";
  content: string;
}

export interface AiGenerateRequest {
  model: AiModelId;
  messages: readonly AiMessage[];
  options?: AiRequestOptions;
}

export interface AiGenerateResponse {
  providerId: AiProviderId;
  model: AiModelId;
  text: string;
  usage?: {
    inputTokens?: number;
    outputTokens?: number;
    totalTokens?: number;
  };
  finishReason?: string;
  requestId?: string;
  rawMetadata?: Readonly<Record<string, string | number | boolean>>;
}

export interface AiEmbeddingRequest {
  model: AiModelId;
  inputs: readonly string[];
  options?: AiRequestOptions;
}

export interface AiEmbeddingResponse {
  providerId: AiProviderId;
  model: AiModelId;
  vectors: readonly (readonly number[])[];
  dimensions: number;
  usage?: {
    inputTokens?: number;
    totalTokens?: number;
  };
}

export type AiErrorCode =
  | "AUTHENTICATION"
  | "PERMISSION_DENIED"
  | "RATE_LIMITED"
  | "QUOTA_EXCEEDED"
  | "TIMEOUT"
  | "NETWORK"
  | "INVALID_REQUEST"
  | "MODEL_NOT_FOUND"
  | "CAPABILITY_UNSUPPORTED"
  | "PROVIDER_UNAVAILABLE"
  | "INVALID_RESPONSE"
  | "UNKNOWN";

export class AiProviderError extends Error {
  readonly code: AiErrorCode;
  readonly providerId: AiProviderId;
  readonly statusCode?: number;
  readonly retryable: boolean;
  readonly requestId?: string;
  readonly causeValue?: unknown;

  constructor(args: {
    message: string;
    code: AiErrorCode;
    providerId: AiProviderId;
    statusCode?: number;
    retryable?: boolean;
    requestId?: string;
    cause?: unknown;
  }) {
    super(args.message);
    this.name = "AiProviderError";
    this.code = args.code;
    this.providerId = args.providerId;
    this.statusCode = args.statusCode;
    this.retryable = args.retryable ?? false;
    this.requestId = args.requestId;
    this.causeValue = args.cause;
  }
}

export interface AiProvider {
  readonly id: AiProviderId;
  readonly displayName: string;
  listModels?(options?: { signal?: AbortSignal }): Promise<readonly AiModelDescriptor[]>;
  describeModel?(modelId: AiModelId, options?: { signal?: AbortSignal }): Promise<AiModelDescriptor | undefined>;
  generate(request: AiGenerateRequest): Promise<AiGenerateResponse>;
  embed?(request: AiEmbeddingRequest): Promise<AiEmbeddingResponse>;
}

export class AiProviderRegistry {
  private readonly providers = new Map<AiProviderId, AiProvider>();

  register(provider: AiProvider): void {
    if (!provider.id.trim()) {
      throw new Error("AI provider id must not be empty");
    }
    if (this.providers.has(provider.id)) {
      throw new Error(`AI provider already registered: ${provider.id}`);
    }
    this.providers.set(provider.id, provider);
  }

  replace(provider: AiProvider): void {
    if (!provider.id.trim()) {
      throw new Error("AI provider id must not be empty");
    }
    this.providers.set(provider.id, provider);
  }

  get(providerId: AiProviderId): AiProvider {
    const provider = this.providers.get(providerId);
    if (!provider) {
      throw new AiProviderError({
        message: `AI provider is not registered: ${providerId}`,
        code: "INVALID_REQUEST",
        providerId,
      });
    }
    return provider;
  }

  list(): readonly Pick<AiProvider, "id" | "displayName">[] {
    return [...this.providers.values()].map(({ id, displayName }) => ({ id, displayName }));
  }

  async requireCapability(
    providerId: AiProviderId,
    modelId: AiModelId,
    capability: AiCapability,
  ): Promise<AiModelDescriptor> {
    const provider = this.get(providerId);
    const descriptor = await provider.describeModel?.(modelId);
    if (!descriptor) {
      throw new AiProviderError({
        message: `Model metadata is unavailable for ${providerId}/${modelId}; configure or discover model capabilities before use.`,
        code: "MODEL_NOT_FOUND",
        providerId,
      });
    }
    if (!descriptor.capabilities.has(capability)) {
      throw new AiProviderError({
        message: `Model ${providerId}/${modelId} does not support required capability: ${capability}`,
        code: "CAPABILITY_UNSUPPORTED",
        providerId,
      });
    }
    return descriptor;
  }
}

export interface AiRoute {
  providerId: AiProviderId;
  modelId: AiModelId;
  requiredCapabilities: readonly AiCapability[];
  fallbackRoutes?: readonly Omit<AiRoute, "fallbackRoutes">[];
}

export interface AiRouterOptions {
  allowFallback?: boolean;
  onFallback?: (error: AiProviderError, route: Omit<AiRoute, "fallbackRoutes">) => void;
}

export class AiRouter {
  constructor(
    private readonly registry: AiProviderRegistry,
    private readonly options: AiRouterOptions = {},
  ) {}

  async generate(route: AiRoute, request: Omit<AiGenerateRequest, "model">): Promise<AiGenerateResponse> {
    const candidates: Omit<AiRoute, "fallbackRoutes">[] = [
      { providerId: route.providerId, modelId: route.modelId, requiredCapabilities: route.requiredCapabilities },
      ...(this.options.allowFallback === false ? [] : route.fallbackRoutes ?? []),
    ];
    let lastError: AiProviderError | undefined;

    for (const candidate of candidates) {
      try {
        const provider = this.registry.get(candidate.providerId);
        for (const capability of candidate.requiredCapabilities) {
          await this.registry.requireCapability(candidate.providerId, candidate.modelId, capability);
        }
        return await provider.generate({ ...request, model: candidate.modelId });
      } catch (error) {
        lastError = error instanceof AiProviderError
          ? error
          : new AiProviderError({
              message: error instanceof Error ? error.message : String(error),
              code: "UNKNOWN",
              providerId: candidate.providerId,
              cause: error,
            });
        if (!lastError.retryable || candidate === candidates[candidates.length - 1]) {
          if (!this.options.allowFallback || candidate === candidates[candidates.length - 1]) {
            throw lastError;
          }
        }
        const next = candidates[candidates.indexOf(candidate) + 1];
        if (!next) throw lastError;
        this.options.onFallback?.(lastError, next);
      }
    }

    throw lastError ?? new Error("No AI route could be selected");
  }
}
