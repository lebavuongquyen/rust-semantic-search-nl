pub mod intent;

pub use intent::{NormalizedQueryIntent, QueryIntentCategory};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique identifier for a document.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DocumentId(pub String);

impl std::fmt::Display for DocumentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Core Document entity representing an ingested document.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Document {
    pub id: DocumentId,
    pub title: String,
    pub file_path: Option<String>,
    pub mime_type: Option<String>,
    pub metadata: HashMap<String, String>,
    pub content: String,
}

impl Document {
    pub fn new(id: impl Into<String>, title: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            id: DocumentId(id.into()),
            title: title.into(),
            file_path: None,
            mime_type: None,
            metadata: HashMap::new(),
            content: content.into(),
        }
    }

    pub fn with_file_path(mut self, path: impl Into<String>) -> Self {
        self.file_path = Some(path.into());
        self
    }

    pub fn with_metadata(mut self, key: impl Into<String>, val: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), val.into());
        self
    }
}

/// Unique identifier for a chunk within a document.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChunkId(pub String);

impl std::fmt::Display for ChunkId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A chunk of a document for indexing and fine-grained retrieval.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Chunk {
    pub id: ChunkId,
    pub document_id: DocumentId,
    pub chunk_index: usize,
    pub text: String,
    pub start_offset: usize,
    pub end_offset: usize,
    pub metadata: HashMap<String, String>,
}

impl Chunk {
    pub fn new(
        id: impl Into<String>,
        doc_id: DocumentId,
        chunk_index: usize,
        text: impl Into<String>,
    ) -> Self {
        let text = text.into();
        let end_offset = text.len();
        Self {
            id: ChunkId(id.into()),
            document_id: doc_id,
            chunk_index,
            text,
            start_offset: 0,
            end_offset,
            metadata: HashMap::new(),
        }
    }
}

/// Vector distance/similarity metric
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DistanceMetric {
    Cosine,
    DotProduct,
    Euclidean,
}

/// Explicit identity of an embedding vector space to prevent incompatible mixing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddingSpace {
    pub provider_id: String,
    pub model_id: String,
    pub revision: String,
    pub dimensions: usize,
    pub distance_metric: DistanceMetric,
    pub normalized: bool,
    pub supported_languages: Vec<String>,
}

impl EmbeddingSpace {
    pub fn is_compatible_with(&self, other: &EmbeddingSpace) -> bool {
        self.provider_id == other.provider_id
            && self.model_id == other.model_id
            && self.revision == other.revision
            && self.dimensions == other.dimensions
            && self.distance_metric == other.distance_metric
            && self.normalized == other.normalized
            && self.supported_languages == other.supported_languages
    }
}

/// An embedding vector with explicit space identity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EmbeddingVector {
    pub space: EmbeddingSpace,
    pub values: Vec<f32>,
}

impl EmbeddingVector {
    pub fn new(space: EmbeddingSpace, values: Vec<f32>) -> Result<Self, SearchError> {
        if values.len() != space.dimensions {
            return Err(SearchError::DimensionMismatch {
                expected: space.dimensions,
                actual: values.len(),
            });
        }
        if values.iter().any(|v| !v.is_finite()) {
            return Err(SearchError::InvalidConfiguration(
                "Embedding vector values must be finite numbers (no NaN or Infinity)".to_string(),
            ));
        }
        Ok(Self { space, values })
    }
}

/// Exact numerical epoch timestamp range for zero-hardcode temporal filtering.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct TemporalRange {
    pub start_epoch_seconds: Option<i64>,
    pub end_epoch_seconds: Option<i64>,
}

/// Structured filter criteria for hard constraints.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct QueryFilter {
    pub required_extensions: Vec<String>,
    pub required_tags: Vec<String>,
    pub path_prefix: Option<String>,
    pub negative_keywords: Vec<String>,
    pub temporal_year: Option<u16>,
    pub temporal_month: Option<u8>,
    pub temporal_range: Option<TemporalRange>,
}

/// Natural-language search query with optional structured constraints.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Query {
    pub raw_query: String,
    pub normalized_query: String,
    pub language: Option<String>,
    pub filter: QueryFilter,
    pub top_k: usize,
}

impl Query {
    pub fn new(raw: impl Into<String>) -> Self {
        let raw_str = raw.into();
        Self {
            normalized_query: raw_str.trim().to_lowercase(),
            raw_query: raw_str,
            language: None,
            filter: QueryFilter::default(),
            top_k: 10,
        }
    }

    pub fn with_top_k(mut self, k: usize) -> Self {
        self.top_k = k;
        self
    }

    pub fn with_filter(mut self, filter: QueryFilter) -> Self {
        self.filter = filter;
        self
    }
}

/// Component contribution to a score.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScoreComponent {
    pub name: String,
    pub raw_value: f32,
    pub weighted_value: f32,
    pub description: String,
}

/// Overall score with traceability breakdown.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Score {
    pub final_score: f32,
    pub components: Vec<ScoreComponent>,
}

impl Score {
    pub fn simple(value: f32, description: impl Into<String>) -> Self {
        Self {
            final_score: value,
            components: vec![ScoreComponent {
                name: "base".to_string(),
                raw_value: value,
                weighted_value: value,
                description: description.into(),
            }],
        }
    }
}

/// Provenance metadata tracking how and why a result was retrieved.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Provenance {
    pub retrieval_path: String, // e.g. "lexical", "vector", "hybrid_rrf"
    pub chunk_id: ChunkId,
    pub document_id: DocumentId,
    pub matched_terms: Vec<String>,
    pub rank_in_path: usize,
    pub trace_details: HashMap<String, String>,
}

/// Final normalized search result item.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SearchResult {
    pub rank: usize,
    pub chunk_id: ChunkId,
    pub document_id: DocumentId,
    pub document_title: String,
    pub snippet: String,
    pub score: Score,
    pub provenance: Provenance,
}

/// Domain errors with explicit semantics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SearchError {
    DimensionMismatch { expected: usize, actual: usize },
    IncompatibleEmbeddingSpace { expected: String, actual: String },
    UnsupportedCapability(String),
    InvalidConfiguration(String),
    DocumentNotFound(DocumentId),
    ChunkNotFound(ChunkId),
    RetrieverError(String),
    FilterViolation(String),
    Internal(String),
}

impl std::fmt::Display for SearchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SearchError::DimensionMismatch { expected, actual } => {
                write!(f, "Dimension mismatch: expected {}, got {}", expected, actual)
            }
            SearchError::IncompatibleEmbeddingSpace { expected, actual } => {
                write!(f, "Incompatible embedding space: expected {}, got {}", expected, actual)
            }
            SearchError::UnsupportedCapability(cap) => write!(f, "Unsupported capability: {}", cap),
            SearchError::InvalidConfiguration(msg) => write!(f, "Invalid configuration: {}", msg),
            SearchError::DocumentNotFound(id) => write!(f, "Document not found: {}", id),
            SearchError::ChunkNotFound(id) => write!(f, "Chunk not found: {}", id),
            SearchError::RetrieverError(msg) => write!(f, "Retriever error: {}", msg),
            SearchError::FilterViolation(msg) => write!(f, "Filter violation: {}", msg),
            SearchError::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for SearchError {}
