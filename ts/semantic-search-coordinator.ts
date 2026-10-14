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

export interface SearchQueryFilter {
  readonly required_extensions?: readonly string[];
  readonly required_tags?: readonly string[];
  readonly path_prefix?: string;
  readonly negative_keywords?: readonly string[];
  readonly temporal_year?: number;
  readonly temporal_month?: number;
}

export interface SearchExecutionResult {
  readonly rank: number;
  readonly chunk_id: { 0: string };
  readonly document_id: { 0: string };
  readonly document_title: string;
  readonly snippet: string;
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

    return {
      chunkCount: chunkTexts.length,
      embeddingSource: embedRes.source,
    };
  }

  /**
   * Search query:
   * 1. Generate query vector via LoadBalancer
   * 2. Invoke Rust WASM hybrid retriever (BM25 + Vector + RRF + Filters + Reranker)
   * 3. Annotate provenance with actual embedding source
   */
  async search(
    rawQuery: string,
    options?: {
      topK?: number;
      filter?: SearchQueryFilter;
    },
  ): Promise<{ results: SearchExecutionResult[]; embeddingSource: string }> {
    const topK = options?.topK ?? 10;
    const filterJson = options?.filter ? JSON.stringify(options.filter) : null;

    // Step 1: Embed the search query
    const embedRes: LoadBalancedEmbedResult = await this.embedder.embed([
      rawQuery,
    ]);
    const queryVector =
      embedRes.vectors.length > 0
        ? new Float32Array(embedRes.vectors[0])
        : null;

    // Step 2: Execute search in Rust WASM
    const results: SearchExecutionResult[] = this.wasmAgent.searchWithVector(
      rawQuery,
      queryVector,
      topK,
      filterJson,
    );

    // Annotate trace with embedding provenance
    for (const res of results) {
      if (res.provenance && res.provenance.trace_details) {
        res.provenance.trace_details["embedding_source"] = embedRes.source;
        if (embedRes.attemptedSlots.length > 0) {
          res.provenance.trace_details["attempted_embedding_slots"] =
            embedRes.attemptedSlots.join(",");
        }
      }
    }

    return {
      results,
      embeddingSource: embedRes.source,
    };
  }

  deleteDocument(docId: string): boolean {
    return this.wasmAgent.deleteDocument(docId);
  }

  getStats(): any {
    return this.wasmAgent.getIndexStats();
  }
}
