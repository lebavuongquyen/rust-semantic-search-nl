use crate::domain::{Chunk, ChunkId, Query, SearchError};
use crate::ports::LexicalRetriever;
use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

/// Standard Okapi BM25 Lexical Retriever.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bm25LexicalRetriever {
    pub k1: f32,
    pub b: f32,
    /// Inverted index: term -> map of (ChunkId -> term_frequency)
    inverted_index: HashMap<String, HashMap<ChunkId, u32>>,
    /// Length of each document/chunk in terms
    doc_lengths: HashMap<ChunkId, u32>,
    /// Total number of indexed chunks
    total_docs: usize,
    /// Sum of all doc lengths for avgdl calculation
    total_doc_length: u64,
}

impl Default for Bm25LexicalRetriever {
    fn default() -> Self {
        Self::new(1.2, 0.75)
    }
}

impl Bm25LexicalRetriever {
    pub fn new(k1: f32, b: f32) -> Self {
        Self {
            k1,
            b,
            inverted_index: HashMap::new(),
            doc_lengths: HashMap::new(),
            total_docs: 0,
            total_doc_length: 0,
        }
    }

    /// Serialize BM25 lexical index to JSON string.
    pub fn to_json(&self) -> Result<String, SearchError> {
        serde_json::to_string(self)
            .map_err(|e| SearchError::Internal(format!("Failed to serialize BM25 index: {}", e)))
    }

    /// Deserialize BM25 lexical index from JSON string.
    pub fn from_json(json_str: &str) -> Result<Self, SearchError> {
        serde_json::from_str(json_str).map_err(|e| {
            SearchError::Internal(format!("Failed to deserialize BM25 index: {}", e))
        })
    }

    /// Persist BM25 index snapshot to disk atomically.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn save_snapshot(&self, path: impl AsRef<std::path::Path>) -> Result<(), SearchError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                SearchError::Internal(format!("Failed to create snapshot directory: {}", e))
            })?;
        }
        let tmp_path = path.with_extension("tmp");
        let json_str = self.to_json()?;
        std::fs::write(&tmp_path, json_str).map_err(|e| {
            SearchError::Internal(format!("Failed to write BM25 index snapshot tmp file: {}", e))
        })?;
        std::fs::rename(&tmp_path, path).map_err(|e| {
            SearchError::Internal(format!("Failed to commit BM25 index snapshot: {}", e))
        })?;
        Ok(())
    }

    /// Load BM25 index snapshot from disk.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_snapshot(path: impl AsRef<std::path::Path>) -> Result<Self, SearchError> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path).map_err(|e| {
            SearchError::Internal(format!("Failed to read BM25 index snapshot from {}: {}", path.display(), e))
        })?;
        Self::from_json(&content)
    }

    /// Tokenize text into lowercased alphanumeric words.
    pub fn tokenize(text: &str) -> Vec<String> {
        text.to_lowercase()
            .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect()
    }

    fn avgdl(&self) -> f32 {
        if self.total_docs == 0 {
            1.0
        } else {
            (self.total_doc_length as f32) / (self.total_docs as f32)
        }
    }

    fn idf(&self, term: &str) -> f32 {
        let n = match self.inverted_index.get(term) {
            Some(docs) => docs.len() as f32,
            None => 0.0,
        };
        let n_total = self.total_docs as f32;
        // Standard Lucene / BM25 IDF with smoothing
        let idf_val = ((n_total - n + 0.5) / (n + 0.5) + 1.0).ln();
        idf_val.max(0.0)
    }
}

impl LexicalRetriever for Bm25LexicalRetriever {
    fn index_chunk(&mut self, chunk: &Chunk) -> Result<(), SearchError> {
        // If chunk is already present, remove it first
        if self.doc_lengths.contains_key(&chunk.id) {
            self.remove_chunk(&chunk.id)?;
        }

        let tokens = Self::tokenize(&chunk.text);
        let doc_len = tokens.len() as u32;

        for token in tokens {
            let entry = self.inverted_index.entry(token).or_default();
            *entry.entry(chunk.id.clone()).or_insert(0) += 1;
        }

        self.doc_lengths.insert(chunk.id.clone(), doc_len);
        self.total_docs += 1;
        self.total_doc_length += doc_len as u64;

        Ok(())
    }

    fn remove_chunk(&mut self, chunk_id: &ChunkId) -> Result<(), SearchError> {
        if let Some(doc_len) = self.doc_lengths.remove(chunk_id) {
            self.total_docs = self.total_docs.saturating_sub(1);
            self.total_doc_length = self.total_doc_length.saturating_sub(doc_len as u64);

            self.inverted_index.retain(|_, postings| {
                postings.remove(chunk_id);
                !postings.is_empty()
            });
        }
        Ok(())
    }

    fn search(&self, query: &Query, top_k: usize) -> Result<Vec<(ChunkId, f32)>, SearchError> {
        if self.total_docs == 0 {
            return Ok(Vec::new());
        }

        let query_tokens = Self::tokenize(&query.normalized_query);
        if query_tokens.is_empty() {
            return Ok(Vec::new());
        }

        let avgdl = self.avgdl();
        let mut candidate_ids: HashSet<&ChunkId> = HashSet::new();

        // Find candidate chunks containing at least one query token
        for token in &query_tokens {
            if let Some(postings) = self.inverted_index.get(token) {
                for id in postings.keys() {
                    candidate_ids.insert(id);
                }
            }
        }

        let mut scored: Vec<(ChunkId, f32)> = Vec::new();

        for chunk_id in candidate_ids {
            let doc_len = *self.doc_lengths.get(chunk_id).unwrap_or(&0) as f32;
            let mut bm25_score = 0.0f32;

            for token in &query_tokens {
                if let Some(postings) = self.inverted_index.get(token) {
                    if let Some(&tf) = postings.get(chunk_id) {
                        let idf = self.idf(token);
                        let tf = tf as f32;
                        let denom = tf + self.k1 * (1.0 - self.b + self.b * (doc_len / avgdl));
                        let term_score = idf * (tf * (self.k1 + 1.0)) / denom;
                        bm25_score += term_score;
                    }
                }
            }

            if bm25_score > 0.0 {
                scored.push((chunk_id.clone(), bm25_score));
            }
        }

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        if scored.len() > top_k {
            scored.truncate(top_k);
        }

        Ok(scored)
    }
}
