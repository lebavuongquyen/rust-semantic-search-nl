/**
 * Google AI Studio Integration for SemanticSearchAgent (Migrated to Provider-Neutral Architecture)
 *
 * Supports Free-tier quota models:
 * - gemini-2.0-flash (Recommended default)
 * - gemini-1.5-flash
 * - gemini-1.5-flash-8b
 *
 * Configurable Fallback Policy:
 * - 'fallback_to_standard': If API call fails (429 Quota, network, 401, timeout), automatically fall back to local WASM engine
 * - 'strict_fail': If API call fails, immediately reject/return FAILED status with detailed HTTP/error code telemetry
 */

import {
  AiProviderError,
  AiProviderRegistry,
  AiRouter,
} from "./ai-provider-contract";
import { GoogleAiStudioProvider } from "./google-ai-studio-provider";

export type FallbackPolicy = "fallback_to_standard" | "strict_fail";

export type GoogleAiStudioModel =
  | "gemini-2.0-flash"
  | "gemini-1.5-flash"
  | "gemini-1.5-flash-8b"
  | (string & {});

export interface GoogleAiStudioConfig {
  apiKey: string;
  model?: GoogleAiStudioModel;
  fallbackPolicy?: FallbackPolicy;
  timeoutMs?: number;
  fetchImplementation?: typeof fetch;
}

export interface ApiDiagnostic {
  attempted: boolean;
  modelUsed?: string;
  statusCode?: number; // e.g. 429 (Too Many Requests), 401, 503
  errorCode?: string; // 'QUOTA_EXCEEDED' | 'UNAUTHORIZED' | 'TIMEOUT' | 'NETWORK_ERROR' | 'UNKNOWN'
  errorMessage?: string;
  durationMs?: number;
}

export interface ParsedSearchPlan {
  provider: "gemini" | "standard_fallback" | "standard_pure";
  raw_query: string;
  language: string;
  normalized_query: string;
  semantic_keywords: VecString;
  name_patterns: VecString;
  code_routes: VecString;
  temporal_info?: {
    month?: number;
    year?: number;
    patterns: string[];
  } | null;
  target_extensions?: string[];
  negative_keywords?: string[];
}

type VecString = string[];

export interface SearchPlanEnvelope {
  status: "SUCCESS" | "FAILED";
  plan?: ParsedSearchPlan;
  diagnostic: ApiDiagnostic;
  error?: string;
}

export class GoogleAiStudioSearchOrchestrator {
  private config: Required<
    Omit<GoogleAiStudioConfig, "fetchImplementation">
  > & {
    fetchImplementation?: typeof fetch;
  };
  private router: AiRouter;

  constructor(config: GoogleAiStudioConfig) {
    this.config = {
      apiKey: config.apiKey,
      model: config.model || "gemini-2.0-flash",
      fallbackPolicy: config.fallbackPolicy || "fallback_to_standard",
      timeoutMs: config.timeoutMs || 4000,
      fetchImplementation: config.fetchImplementation,
    };

    const registry = new AiProviderRegistry();
    if (this.config.apiKey && this.config.apiKey.trim() !== "") {
      registry.register(
        new GoogleAiStudioProvider({
          apiKey: this.config.apiKey,
          fetchImplementation: this.config.fetchImplementation,
          defaultTimeoutMs: this.config.timeoutMs,
        }),
      );
    }
    this.router = new AiRouter(registry, { allowFallback: false });
  }

  /**
   * Tạo Search Plan kết hợp giữa Google AI Studio & Fallback Engine
   */
  async generateSearchPlan(
    query: string,
    standardFallbackFn: (query: string) => ParsedSearchPlan,
  ): Promise<SearchPlanEnvelope> {
    const startTime = Date.now();

    // 1. Nếu không có API Key, chạy luôn workflow chuẩn
    if (!this.config.apiKey || this.config.apiKey.trim() === "") {
      const plan = standardFallbackFn(query);
      plan.provider = "standard_pure";
      return {
        status: "SUCCESS",
        plan,
        diagnostic: {
          attempted: false,
          durationMs: Date.now() - startTime,
        },
      };
    }

    // 2. Thử gọi qua Provider Router
    try {
      const planFromGemini = await this.callGeminiApi(query);
      return {
        status: "SUCCESS",
        plan: planFromGemini,
        diagnostic: {
          attempted: true,
          modelUsed: this.config.model,
          statusCode: 200,
          durationMs: Date.now() - startTime,
        },
      };
    } catch (err: any) {
      const durationMs = Date.now() - startTime;
      const statusCode = err.statusCode || 500;
      let errorCode = err.code || "UNKNOWN";

      if (statusCode === 429 || errorCode === "QUOTA_EXCEEDED") {
        errorCode = "QUOTA_EXCEEDED";
      } else if (
        statusCode === 401 ||
        statusCode === 403 ||
        errorCode === "AUTHENTICATION" ||
        errorCode === "PERMISSION_DENIED"
      ) {
        errorCode = "UNAUTHORIZED";
      } else if (
        err.name === "AbortError" ||
        err.message?.includes("timeout") ||
        errorCode === "TIMEOUT"
      ) {
        errorCode = "TIMEOUT";
      } else if (
        err.message?.includes("network") ||
        err.message?.includes("fetch failed") ||
        errorCode === "NETWORK"
      ) {
        errorCode = "NETWORK_ERROR";
      }

      const diagnostic: ApiDiagnostic = {
        attempted: true,
        modelUsed: this.config.model,
        statusCode,
        errorCode,
        errorMessage: err.message || String(err),
        durationMs,
      };

      // 3. Xử lý theo Chính sách Fallback đã chọn
      if (this.config.fallbackPolicy === "strict_fail") {
        return {
          status: "FAILED",
          diagnostic,
          error: `Google AI Studio API failed with status ${statusCode} (${errorCode}): ${err.message}`,
        };
      }

      // Default: 'fallback_to_standard'
      const fallbackPlan = standardFallbackFn(query);
      fallbackPlan.provider = "standard_fallback";

      return {
        status: "SUCCESS",
        plan: fallbackPlan,
        diagnostic,
      };
    }
  }

  private async callGeminiApi(query: string): Promise<ParsedSearchPlan> {
    const systemPrompt = `You are a high-speed Multilingual Semantic Search Query Planner.
Your job is to analyze the user's natural language search query and output a strict JSON search specification.
Rules:
1. Translate and expand concepts cross-lingually across relevant languages (e.g. Vietnamese, English) based strictly on semantic context.
2. Extract temporal patterns if mentioned (e.g. month, year, quarters).
3. Infer technical naming patterns or coding file routes if query is technical.
Output MUST be strict JSON only matching this format:
{
  "language": "vi|en|ja|zh",
  "normalized_query": "string",
  "semantic_keywords": ["keyword1", "keyword2"],
  "name_patterns": ["*pattern*.*"],
  "code_routes": ["/route1", "/api/route1"],
  "temporal_info": {
    "month": 8,
    "year": 2026,
    "patterns": ["tháng 08", "08/2026", "2608"]
  },
  "target_extensions": ["pdf", "jpg", "png", "docx"],
  "negative_keywords": []
}`;

    // Execute via Provider Contract and Router
    const response = await this.router.generate(
      {
        providerId: "google-ai-studio",
        modelId: this.config.model,
        requiredCapabilities: ["text_generation", "structured_output"],
      },
      {
        messages: [
          { role: "system", content: systemPrompt },
          { role: "user", content: query },
        ],
        options: {
          responseFormat: "json",
          temperature: 0.1,
          timeoutMs: this.config.timeoutMs,
        },
      },
    );

    const parsed = JSON.parse(response.text);

    // Validate plan schema
    return {
      provider: "gemini",
      raw_query: query,
      language: typeof parsed.language === "string" ? parsed.language : "en",
      normalized_query:
        typeof parsed.normalized_query === "string"
          ? parsed.normalized_query
          : query,
      semantic_keywords: Array.isArray(parsed.semantic_keywords)
        ? parsed.semantic_keywords
        : [],
      name_patterns: Array.isArray(parsed.name_patterns)
        ? parsed.name_patterns
        : [],
      code_routes: Array.isArray(parsed.code_routes) ? parsed.code_routes : [],
      temporal_info: parsed.temporal_info || null,
      target_extensions: Array.isArray(parsed.target_extensions)
        ? parsed.target_extensions
        : [],
      negative_keywords: Array.isArray(parsed.negative_keywords)
        ? parsed.negative_keywords
        : [],
    };
  }
}
