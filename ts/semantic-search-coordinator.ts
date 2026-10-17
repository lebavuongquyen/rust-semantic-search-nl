import {
  EmbeddingLoadBalancer,
  LoadBalancedEmbedResult,
} from "./embedding-load-balancer";
// Import compiled WebAssembly agent
// @ts-ignore
const { SemanticSearchAgent } = require("../pkg/semantic_search_agent.js");

export interface DocumentInput {
  readonly id: string;
  readonly title: string;
  readonly content: string;
  readonly file_path?: string;
  readonly metadata?: Record<string, string>;
}

export interface ChunkDefinition {
  readonly text: string;
  readonly startOffset: number;
  readonly endOffset: number;
}

export interface TemporalRangeFilter {
  readonly start_epoch_seconds?: number;
  readonly end_epoch_seconds?: number;
}

export interface SearchQueryFilter {
  readonly required_extensions?: readonly string[];
  readonly required_tags?: readonly string[];
  readonly path_prefix?: string;
  readonly negative_keywords?: readonly string[];
  readonly temporal_year?: number;
  readonly temporal_month?: number;
  readonly temporal_range?: TemporalRangeFilter;
}

export interface SearchExecutionResult {
  readonly rank: number;
  readonly chunk_id: { 0: string };
  readonly document_id: { 0: string };
  readonly document_title: string;
  readonly snippet: string;
  readonly parent_context?: {
    readonly title: string;
    readonly full_content_length: number;
    readonly preview: string;
  };
  readonly score: {
    readonly final_score: number;
    readonly components: readonly {
      name: string;
      raw_value: number;
      weighted_value: number;
      description: string;
    }[];
  };
  readonly provenance: {
    readonly retrieval_path: string;
    readonly chunk_id: { 0: string };
    readonly document_id: { 0: string };
    readonly matched_terms: readonly string[];
    readonly rank_in_path: number;
    readonly trace_details: Record<string, string>;
  };
}

export class SemanticSearchCoordinator {
  private readonly wasmAgent: any;
  private readonly embedder: EmbeddingLoadBalancer;
  private readonly parentDocs = new Map<string, DocumentInput>();

  constructor(
    embedder?: EmbeddingLoadBalancer,
    options?: { providerId?: string; modelId?: string },
  ) {
    this.embedder = embedder ?? new EmbeddingLoadBalancer();
    this.wasmAgent = new SemanticSearchAgent();
    // Sync embedding space to match balancer dimensions
    this.wasmAgent.configureEmbeddingSpace(
      options?.providerId ?? (embedder ? "google-ai-studio" : "local-offline"),
      options?.modelId ??
        (embedder ? "gemini-embedding-001" : "offline-math-embedder"),
      this.embedder.unifiedDimensions,
    );
  }

  getWasmAgent(): any {
    return this.wasmAgent;
  }

  getEmbedder(): EmbeddingLoadBalancer {
    return this.embedder;
  }

  /**
   * Smart UTF-8 chunking with paragraph and sentence boundary awareness
   */
  static chunkContent(content: string, chunkSizeChars = 300): string[] {
    if (!content) return [];
    if (chunkSizeChars <= 0) chunkSizeChars = 300;

    const trimmed = content.trim();
    if (trimmed.length <= chunkSizeChars) {
      return [trimmed];
    }

    // Try splitting by paragraphs first
    const paragraphs = content.split(/\n\s*\n/);
    const chunks: string[] = [];
    let currentChunk = "";

    for (const para of paragraphs) {
      const p = para.trim();
      if (!p) continue;

      if (p.length > chunkSizeChars) {
        if (currentChunk.trim()) {
          chunks.push(currentChunk.trim());
          currentChunk = "";
        }
        const sentences = p.split(/(?<=[.!?\n])\s+/);
        for (const sent of sentences) {
          const s = sent.trim();
          if (!s) continue;
          if (s.length > chunkSizeChars) {
            if (currentChunk.trim()) {
              chunks.push(currentChunk.trim());
              currentChunk = "";
            }
            let start = 0;
            while (start < s.length) {
              const end = Math.min(start + chunkSizeChars, s.length);
              chunks.push(s.substring(start, end));
              start = end;
            }
          } else if (currentChunk.length + s.length + 1 <= chunkSizeChars) {
            currentChunk = currentChunk ? `${currentChunk} ${s}` : s;
          } else {
            chunks.push(currentChunk.trim());
            currentChunk = s;
          }
        }
      } else if (currentChunk.length + p.length + 2 <= chunkSizeChars) {
        currentChunk = currentChunk ? `${currentChunk}\n\n${p}` : p;
      } else {
        chunks.push(currentChunk.trim());
        currentChunk = p;
      }
    }

    if (currentChunk.trim()) {
      chunks.push(currentChunk.trim());
    }

    return chunks.length > 0 ? chunks : [content.substring(0, chunkSizeChars)];
  }

  /**
   * Ingest a full document:
   * 1. Chunk content into UTF-8 pieces
   * 2. Generate embeddings via LoadBalancer (with slot cooldown & local fallback)
   * 3. Pass chunks & flattened Float32Array to Rust WASM engine with metadata and file_path
   */
  async ingestDocument(
    doc: DocumentInput,
    chunkSizeChars = 300,
  ): Promise<{ chunkCount: number; embeddingSource: string }> {
    const chunkTexts = SemanticSearchCoordinator.chunkContent(
      doc.content,
      chunkSizeChars,
    );
    if (chunkTexts.length === 0) {
      return { chunkCount: 0, embeddingSource: "empty" };
    }

    // Step 3: Embed chunks via LoadBalancer
    const embedRes: LoadBalancedEmbedResult =
      await this.embedder.embed(chunkTexts);

    // Flatten vectors into single contiguous Float32Array for zero-copy FFI into WASM
    const totalFloats =
      embedRes.vectors.length * this.embedder.unifiedDimensions;
    const flatBuffer = new Float32Array(totalFloats);
    for (let c = 0; c < embedRes.vectors.length; c++) {
      const vec = embedRes.vectors[c];
      flatBuffer.set(vec, c * this.embedder.unifiedDimensions);
    }

    // Pass into WASM engine with file_path and metadata
    const metadataJson = doc.metadata ? JSON.stringify(doc.metadata) : null;
    this.wasmAgent.ingestDocumentWithVectors(
      doc.id,
      doc.title,
      doc.content,
      chunkTexts,
      flatBuffer,
      this.embedder.unifiedDimensions,
      doc.file_path ?? null,
      metadataJson,
    );

    // Store parent document for rich context retrieval
    this.parentDocs.set(doc.id, doc);

    return {
      chunkCount: chunkTexts.length,
      embeddingSource: embedRes.source,
    };
  }

  /**
   * Zero-hardcode natural language hypothetical document generator (HyDE).
   * Generates a hypothetical excerpt that directly answers the query to project into document embedding space.
   */
  async generateHypotheticalPassage(
    query: string,
    modelId = "gemini-1.5-flash",
  ): Promise<string | undefined> {
    const provider = this.embedder.getProvider();
    if (!provider || !provider.generate) return undefined;
    try {
      const res = await provider.generate({
        model: modelId,
        messages: [
          {
            role: "system",
            content:
              "You are a helpful search assistant. Given the search query, write a realistic, hypothetical passage or excerpt from an ideal document that directly satisfies this query. Do not include introductory remarks or pleasantries. Output only the passage text.",
          },
          { role: "user", content: query },
        ],
        options: { temperature: 0.3, maxOutputTokens: 200, timeoutMs: 5000 },
      });
      const text = res.text?.trim();
      return text && text.length > 10 ? text : undefined;
    } catch {
      return undefined;
    }
  }

  /**
   * Search query:
   * 1. Optional HyDE (Hypothetical Document Embeddings) expansion for deep natural language understanding
   * 2. Generate query vector via LoadBalancer with RETRIEVAL_QUERY taskType
   * 3. Invoke Rust WASM hybrid retriever (BM25 + Vector + RRF + Filters + Reranker)
   * 4. Attach parent document context window to each result for full situational awareness
   */
  async search(
    rawQuery: string,
    options?: {
      topK?: number;
      filter?: SearchQueryFilter;
      enableHyDE?: boolean;
      hydeModel?: string;
    },
  ): Promise<{ results: SearchExecutionResult[]; embeddingSource: string }> {
    const topK = options?.topK ?? 10;
    const filterJson = options?.filter ? JSON.stringify(options.filter) : null;

    let textToEmbed = rawQuery;
    let hydePassage: string | undefined = undefined;

    // Step 1: If HyDE is enabled, synthesize a hypothetical document passage first
    if (options?.enableHyDE) {
      hydePassage = await this.generateHypotheticalPassage(
        rawQuery,
        options.hydeModel,
      );
      if (hydePassage) {
        textToEmbed = hydePassage;
      }
    }

    // Step 2: Embed the search query (or hypothetical passage) with RETRIEVAL_QUERY taskType
    const embedRes: LoadBalancedEmbedResult = await this.embedder.embed(
      [textToEmbed],
      {
        taskType: "RETRIEVAL_QUERY",
      },
    );
    const queryVector =
      embedRes.vectors.length > 0
        ? new Float32Array(embedRes.vectors[0])
        : null;

    // Step 3: Execute search in Rust WASM
    const results: SearchExecutionResult[] = this.wasmAgent.searchWithVector(
      rawQuery,
      queryVector,
      topK,
      filterJson,
    );

    // Step 4: Annotate trace with embedding provenance and attach parent context
    for (const res of results) {
      if (res.provenance && res.provenance.trace_details) {
        res.provenance.trace_details["embedding_source"] = embedRes.source;
        if (hydePassage) {
          res.provenance.trace_details["hyde_applied"] = "true";
          res.provenance.trace_details["hypothetical_passage"] =
            hydePassage.substring(0, 120);
        }
        if (embedRes.attemptedSlots.length > 0) {
          res.provenance.trace_details["attempted_embedding_slots"] =
            embedRes.attemptedSlots.join(",");
        }
      }

      // Attach rich parent document context
      const docIdStr =
        typeof res.document_id === "object"
          ? (res.document_id as any)[0] || String(res.document_id)
          : String(res.document_id);
      const parentDoc = this.parentDocs.get(docIdStr);
      if (parentDoc) {
        (res as any).parent_context = {
          title: parentDoc.title,
          full_content_length: parentDoc.content.length,
          preview: parentDoc.content.substring(0, 250),
        };
      }
    }

    return {
      results,
      embeddingSource: embedRes.source,
    };
  }

  deleteDocument(docId: string): boolean {
    this.parentDocs.delete(docId);
    return this.wasmAgent.deleteDocument(docId);
  }

  getStats(): any {
    return this.wasmAgent.getIndexStats();
  }
}
