use crate::domain::{
    Chunk, ChunkId, Document, DocumentId, EmbeddingSpace, EmbeddingVector, Query,
    Score, SearchError, Provenance,
};

/// Port for lexical retrieval (e.g. BM25, inverted index, trigram match).
pub trait LexicalRetriever: Send + Sync {
    /// Add or update a chunk in the lexical index.
    fn index_chunk(&mut self, chunk: &Chunk) -> Result<(), SearchError>;

    /// Remove a chunk from the index.
    fn remove_chunk(&mut self, chunk_id: &ChunkId) -> Result<(), SearchError>;

    /// Search for candidate chunks matching the query.
    /// Returns a list of (ChunkId, lexical_score) ordered by relevance descending.
    fn search(&self, query: &Query, top_k: usize) -> Result<Vec<(ChunkId, f32)>, SearchError>;
}

/// Port for vector similarity retrieval.
pub trait VectorRetriever: Send + Sync {
    /// Embedding space identity supported by this vector index.
    fn embedding_space(&self) -> &EmbeddingSpace;

    /// Add a chunk embedding vector to the index.
    fn index_vector(&mut self, chunk_id: &ChunkId, vector: &EmbeddingVector) -> Result<(), SearchError>;

    /// Remove a chunk embedding vector from the index.
    fn remove_vector(&mut self, chunk_id: &ChunkId) -> Result<(), SearchError>;

    /// Search for nearest chunks by query vector.
    /// Returns a list of (ChunkId, similarity_score) ordered by similarity descending.
    fn search_vector(
        &self,
        query_vector: &EmbeddingVector,
        top_k: usize,
    ) -> Result<Vec<(ChunkId, f32)>, SearchError>;
}

/// Port for generating text embeddings.
pub trait EmbeddingProvider: Send + Sync {
    /// Embedding space identity produced by this provider.
    fn embedding_space(&self) -> &EmbeddingSpace;

    /// Embed a single text string into an embedding vector.
    fn embed_text(&self, text: &str) -> Result<EmbeddingVector, SearchError>;

    /// Embed a batch of text strings into embedding vectors.
    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<EmbeddingVector>, SearchError> {
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.embed_text(text)?);
        }
        Ok(results)
    }
}

/// Port for fusing ranked candidate lists (e.g. Reciprocal Rank Fusion / RRF).
pub trait RankFusionPolicy: Send + Sync {
    /// Fuse lexical and vector candidates into a consolidated list with provenance and scores.
    fn fuse(
        &self,
        lexical_candidates: &[(ChunkId, f32)],
        vector_candidates: &[(ChunkId, f32)],
        top_k: usize,
    ) -> Result<Vec<(ChunkId, Score, Provenance)>, SearchError>;
}

/// Port for document storage and lookup.
pub trait DocumentStore: Send + Sync {
    fn put_document(&mut self, doc: Document) -> Result<(), SearchError>;
    fn get_document(&self, id: &DocumentId) -> Result<Option<Document>, SearchError>;
    fn delete_document(&mut self, id: &DocumentId) -> Result<bool, SearchError>;
    fn put_chunk(&mut self, chunk: Chunk) -> Result<(), SearchError>;
    fn get_chunk(&self, id: &ChunkId) -> Result<Option<Chunk>, SearchError>;
    fn delete_chunk(&mut self, id: &ChunkId) -> Result<bool, SearchError>;
}

/// Port for natural-language query understanding, entity extraction, and constraint parsing.
pub trait QueryUnderstandingPort: Send + Sync {
    fn parse_intent(&self, raw_query: &str) -> Result<crate::domain::NormalizedQueryIntent, SearchError>;
}

/// Port for candidate reranking and precision optimization.
pub trait RerankerPort: Send + Sync {
    fn rerank(
        &self,
        query: &str,
        candidates: Vec<crate::domain::SearchResult>,
        top_k: usize,
    ) -> Result<Vec<crate::domain::SearchResult>, SearchError>;
}

/// Default no-op reranker that preserves candidate order.
#[derive(Debug, Clone, Default)]
pub struct NoopReranker;

impl RerankerPort for NoopReranker {
    fn rerank(
        &self,
        _query: &str,
        candidates: Vec<crate::domain::SearchResult>,
        _top_k: usize,
    ) -> Result<Vec<crate::domain::SearchResult>, SearchError> {
        Ok(candidates)
    }
}

/// Extracted text content from a document or scanned asset.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExtractedContent {
    pub text: String,
    pub title: Option<String>,
    pub mime_type: String,
    pub confidence: Option<f32>,
    pub extraction_method: String, // e.g. "plain_text", "pdf_text", "ocr_tesseract", "multimodal_vision"
    pub metadata: std::collections::HashMap<String, String>,
}

/// Port for text extraction and multimodal processing (plain-text, PDF, OCR).
pub trait TextExtractorPort: Send + Sync {
    /// List of MIME types or file extensions supported by this extractor.
    fn supported_formats(&self) -> &[&str];

    /// Extract text content from raw bytes.
    fn extract_text(&self, file_name: &str, bytes: &[u8]) -> Result<ExtractedContent, SearchError>;
}

