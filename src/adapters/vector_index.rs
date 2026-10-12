use crate::domain::{
    ChunkId, DistanceMetric, EmbeddingSpace, EmbeddingVector, SearchError,
};
use crate::ports::VectorRetriever;
use crate::similarity::cosine_similarity;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// In-memory vector index with space validation, version tracking, and migration support.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryVectorIndex {
    space: EmbeddingSpace,
    index_version: u32,
    vectors: HashMap<ChunkId, Vec<f32>>,
}

impl MemoryVectorIndex {
    pub fn new(space: EmbeddingSpace) -> Self {
        Self {
            space,
            index_version: 1,
            vectors: HashMap::new(),
        }
    }

    pub fn index_version(&self) -> u32 {
        self.index_version
    }

    pub fn len(&self) -> usize {
        self.vectors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.vectors.is_empty()
    }

    /// Serialize index to JSON string for durable snapshots.
    pub fn to_json(&self) -> Result<String, SearchError> {
        serde_json::to_string(self)
            .map_err(|e| SearchError::Internal(format!("Failed to serialize vector index: {}", e)))
    }

    /// Deserialize vector index from JSON string.
    pub fn from_json(json_str: &str) -> Result<Self, SearchError> {
        serde_json::from_str(json_str).map_err(|e| {
            SearchError::Internal(format!("Failed to deserialize vector index: {}", e))
        })
    }

    /// Persist vector index snapshot to disk atomically.
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
            SearchError::Internal(format!("Failed to write vector index snapshot tmp file: {}", e))
        })?;
        std::fs::rename(&tmp_path, path).map_err(|e| {
            SearchError::Internal(format!("Failed to commit vector index snapshot: {}", e))
        })?;
        Ok(())
    }

    /// Load vector index snapshot from disk.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_snapshot(path: impl AsRef<std::path::Path>) -> Result<Self, SearchError> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path).map_err(|e| {
            SearchError::Internal(format!("Failed to read vector index snapshot from {}: {}", path.display(), e))
        })?;
        Self::from_json(&content)
    }

    /// Atomically migrate the vector index to a new embedding space.
    /// If reembedding any chunk fails, the original index remains untouched (rollback).
    pub fn migrate_to<F>(
        &mut self,
        new_space: EmbeddingSpace,
        chunk_texts: &HashMap<ChunkId, String>,
        mut embed_fn: F,
    ) -> Result<(), SearchError>
    where
        F: FnMut(&str) -> Result<EmbeddingVector, SearchError>,
    {
        let mut new_vectors = HashMap::with_capacity(self.vectors.len());

        for (id, _) in &self.vectors {
            let text = chunk_texts.get(id).ok_or_else(|| {
                SearchError::Internal(format!("Missing text for chunk {} during migration", id))
            })?;

            let new_vec = embed_fn(text)?;
            if !new_space.is_compatible_with(&new_vec.space) {
                return Err(SearchError::IncompatibleEmbeddingSpace {
                    expected: format!("{}/{}", new_space.provider_id, new_space.model_id),
                    actual: format!("{}/{}", new_vec.space.provider_id, new_vec.space.model_id),
                });
            }

            new_vectors.insert(id.clone(), new_vec.values);
        }

        // Atomic swap
        self.space = new_space;
        self.vectors = new_vectors;
        self.index_version += 1;

        Ok(())
    }
}

impl VectorRetriever for MemoryVectorIndex {
    fn embedding_space(&self) -> &EmbeddingSpace {
        &self.space
    }

    fn index_vector(&mut self, chunk_id: &ChunkId, vector: &EmbeddingVector) -> Result<(), SearchError> {
        if !self.space.is_compatible_with(&vector.space) {
            return Err(SearchError::IncompatibleEmbeddingSpace {
                expected: format!(
                    "{}/{}:d{}",
                    self.space.provider_id, self.space.model_id, self.space.dimensions
                ),
                actual: format!(
                    "{}/{}:d{}",
                    vector.space.provider_id, vector.space.model_id, vector.space.dimensions
                ),
            });
        }

        self.vectors.insert(chunk_id.clone(), vector.values.clone());
        Ok(())
    }

    fn remove_vector(&mut self, chunk_id: &ChunkId) -> Result<(), SearchError> {
        self.vectors.remove(chunk_id);
        Ok(())
    }

    fn search_vector(
        &self,
        query_vector: &EmbeddingVector,
        top_k: usize,
    ) -> Result<Vec<(ChunkId, f32)>, SearchError> {
        if !self.space.is_compatible_with(&query_vector.space) {
            return Err(SearchError::IncompatibleEmbeddingSpace {
                expected: format!(
                    "{}/{}:d{}",
                    self.space.provider_id, self.space.model_id, self.space.dimensions
                ),
                actual: format!(
                    "{}/{}:d{}",
                    query_vector.space.provider_id, query_vector.space.model_id, query_vector.space.dimensions
                ),
            });
        }

        let mut scored: Vec<(ChunkId, f32)> = Vec::with_capacity(self.vectors.len());

        for (id, doc_vec) in &self.vectors {
            let score = match self.space.distance_metric {
                DistanceMetric::Cosine => cosine_similarity(&query_vector.values, doc_vec),
                DistanceMetric::DotProduct => {
                    query_vector.values.iter().zip(doc_vec.iter()).map(|(a, b)| a * b).sum()
                }
                DistanceMetric::Euclidean => {
                    let dist_sq: f32 = query_vector
                        .values
                        .iter()
                        .zip(doc_vec.iter())
                        .map(|(a, b)| (a - b).powi(2))
                        .sum();
                    1.0 / (1.0 + dist_sq.sqrt())
                }
            };
            scored.push((id.clone(), score));
        }

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        if scored.len() > top_k {
            scored.truncate(top_k);
        }

        Ok(scored)
    }
}
