import {
  AiEmbeddingRequest,
  AiEmbeddingResponse,
  AiProvider,
  AiProviderError,
} from "./ai-provider-contract";

export interface LocalEmbedder {
  embed(texts: readonly string[]): Promise<readonly (readonly number[])[]>;
  readonly dimensions: number;
}

/**
 * Local deterministic pseudo-embedding generator ensuring zero-cost, offline fallback
 * compatible with the configured vector dimension.
 */
export class LocalDeterministicEmbedder implements LocalEmbedder {
  constructor(readonly dimensions: number = 768) {}

  async embed(
    texts: readonly string[],
  ): Promise<readonly (readonly number[])[]> {
    return texts.map((text) => {
      const vec = new Array<number>(this.dimensions).fill(0);
      const normalized = text.toLowerCase().trim();
      if (!normalized) return vec;

      // Pure mathematical deterministic n-gram hash projection (zero dictionary, zero word hardcoding)
      const runes = Array.from(normalized);
      for (let n = 1; n <= 3; n++) {
        for (let i = 0; i <= runes.length - n; i++) {
          const ngram = runes.slice(i, i + n).join("");
          let hash = 2166136261;
          for (let j = 0; j < ngram.length; j++) {
            hash ^= ngram.charCodeAt(j);
            hash = Math.imul(hash, 16777619);
          }
          const dim = Math.abs(hash) % this.dimensions;
          vec[dim] += 1.0 / n;
        }
      }

      // Continuous byte dispersal
      const bytes = Buffer.from(text, "utf-8");
      for (let i = 0; i < bytes.length; i++) {
        const dim =
          (384 + ((i * 13) % (this.dimensions - 384))) % this.dimensions;
        vec[dim] += (bytes[i] / 255) * 0.15;
      }

      // L2 Normalize
      let normSq = 0;
      for (let i = 0; i < this.dimensions; i++) normSq += vec[i] * vec[i];
      const norm = Math.sqrt(normSq);
      if (norm > 0) {
        for (let i = 0; i < this.dimensions; i++) vec[i] /= norm;
      }

      return vec;
    });
  }
}

export interface EmbeddingSlotConfig {
  readonly modelId: string;
  readonly cooldownMs?: number;
}

export interface EmbeddingLoadBalancerOptions {
  readonly slots?: readonly EmbeddingSlotConfig[];
  readonly localFallback?: LocalEmbedder;
  readonly unifiedDimensions?: number;
  readonly defaultTimeoutMs?: number;
  readonly strategy?: "priority" | "round_robin";
}

export interface EmbeddingSlotState {
  readonly modelId: string;
  readonly cooldownMs: number;
  cooldownUntil: number;
  consecutiveFailures: number;
  totalSuccesses: number;
  totalFailures: number;
}

export interface LoadBalancedEmbedResult {
  readonly vectors: readonly (readonly number[])[];
  readonly dimensions: number;
  readonly source: string; // e.g. "gemini-slot:gemini-embedding-001" | "local_fallback" | "local_offline"
  readonly attemptedSlots: readonly string[];
  readonly errorDetails?: readonly string[];
}

export interface LoadBalancerEmbedOptions {
  readonly timeoutMs?: number;
  readonly signal?: AbortSignal;
  readonly taskType?:
    "RETRIEVAL_DOCUMENT" | "RETRIEVAL_QUERY" | "SEMANTIC_SIMILARITY";
  readonly title?: string;
}

export class EmbeddingLoadBalancer {
  private readonly slotStates: EmbeddingSlotState[];
  private currentSlotIndex = 0;
  private readonly localFallback: LocalEmbedder;
  readonly unifiedDimensions: number;
  private readonly defaultTimeoutMs: number;
  readonly strategy: "priority" | "round_robin";

  constructor(
    private readonly provider?: AiProvider,
    options?: EmbeddingLoadBalancerOptions,
  ) {
    this.unifiedDimensions = options?.unifiedDimensions ?? 3072;
    this.localFallback =
      options?.localFallback ??
      new LocalDeterministicEmbedder(this.unifiedDimensions);
    this.defaultTimeoutMs = options?.defaultTimeoutMs ?? 8000;
    this.strategy = options?.strategy ?? "priority";

    this.slotStates = (options?.slots ?? []).map((s) => ({
      modelId: s.modelId,
      cooldownMs: s.cooldownMs ?? 60_000,
      cooldownUntil: 0,
      consecutiveFailures: 0,
      totalSuccesses: 0,
      totalFailures: 0,
    }));
  }

  getSlots(): readonly Readonly<EmbeddingSlotState>[] {
    return this.slotStates;
  }

  getProvider(): AiProvider | undefined {
    return this.provider;
  }

  /**
   * Reset cooldowns (used in test or recovery procedures)
   */
  resetCooldowns(): void {
    for (const s of this.slotStates) {
      s.cooldownUntil = 0;
      s.consecutiveFailures = 0;
    }
  }

  /**
   * Execute embedding generation with round-robin load balancing, rate-limit cooldown,
   * permanent failure fast-rejection, and fallback-to-local step execution.
   */
  async embed(
    texts: readonly string[],
    options?: LoadBalancerEmbedOptions,
  ): Promise<LoadBalancedEmbedResult> {
    if (texts.length === 0) {
      return {
        vectors: [],
        dimensions: this.unifiedDimensions,
        source: "noop",
        attemptedSlots: [],
      };
    }

    if (this.slotStates.length === 0 || !this.provider) {
      const fallbackVectors = await this.localFallback.embed(texts);
      return {
        vectors: fallbackVectors,
        dimensions: this.localFallback.dimensions,
        source: "local_offline",
        attemptedSlots: [],
      };
    }

    const attemptedSlots: string[] = [];
    const errorDetails: string[] = [];
    const now = Date.now();
    const timeoutMs = options?.timeoutMs ?? this.defaultTimeoutMs;

    // Iterate through slots starting from round-robin pointer or priority index 0
    const n = this.slotStates.length;
    const startIndex =
      this.strategy === "round_robin" ? this.currentSlotIndex : 0;
    for (let i = 0; i < n; i++) {
      const slotIdx = (startIndex + i) % n;
      const slot = this.slotStates[slotIdx];

      if (slot.cooldownUntil > now) {
        // Slot is in rate-limit or failure cooldown
        continue;
      }

      attemptedSlots.push(slot.modelId);

      try {
        if (!this.provider.embed) {
          throw new AiProviderError({
            message: `Provider ${this.provider.id} does not support embedding`,
            code: "CAPABILITY_UNSUPPORTED",
            providerId: this.provider.id,
          });
        }

        const res: AiEmbeddingResponse = await this.provider.embed({
          model: slot.modelId,
          inputs: texts,
          taskType: options?.taskType,
          title: options?.title,
          options: {
            timeoutMs,
            signal: options?.signal,
          },
        });

        // Advance round-robin pointer if using round_robin strategy
        if (this.strategy === "round_robin") {
          this.currentSlotIndex = (slotIdx + 1) % n;
        }
        slot.consecutiveFailures = 0;
        slot.totalSuccesses++;

        // Enforce or validate dimensions
        if (res.dimensions !== this.unifiedDimensions) {
          // If remote returned different dimension, warn and adapt/pad/truncate or validate
          if (res.vectors.some((v) => v.length !== this.unifiedDimensions)) {
            throw new AiProviderError({
              message: `Dimension mismatch from ${slot.modelId}: expected ${this.unifiedDimensions}, got ${res.dimensions}`,
              code: "INVALID_RESPONSE",
              providerId: this.provider.id,
            });
          }
        }

        return {
          vectors: res.vectors,
          dimensions: this.unifiedDimensions,
          source: `gemini-slot:${slot.modelId}`,
          attemptedSlots,
        };
      } catch (err: any) {
        slot.totalFailures++;
        slot.consecutiveFailures++;

        const isRateLimit =
          err?.code === "RATE_LIMITED" ||
          err?.code === "QUOTA_EXCEEDED" ||
          err?.statusCode === 429 ||
          /quota|rate limit/i.test(err?.message ?? "");

        const isAuthError =
          err?.code === "AUTHENTICATION" ||
          err?.code === "PERMISSION_DENIED" ||
          err?.statusCode === 401 ||
          err?.statusCode === 403;

        errorDetails.push(
          `Slot ${slot.modelId} error [${err?.code || "UNKNOWN"}]: ${err?.message || err}`,
        );

        if (isRateLimit) {
          // Apply cooldown window for 429
          slot.cooldownUntil = Date.now() + slot.cooldownMs;
        } else if (isAuthError) {
          // Auth error is permanent for this key/slot, cool down longer
          slot.cooldownUntil = Date.now() + slot.cooldownMs * 5;
        } else {
          // Transient error: short cooldown
          slot.cooldownUntil = Date.now() + 5000;
        }

        // Continue to next slot
      }
    }

    // All slots exhausted or on cooldown -> Granular Fallback at Step 3 to Local Engine
    const fallbackVectors = await this.localFallback.embed(texts);
    return {
      vectors: fallbackVectors,
      dimensions: this.localFallback.dimensions,
      source: "local_fallback",
      attemptedSlots,
      errorDetails,
    };
  }
}
