use crate::domain::{
    Chunk, ChunkId, DistanceMetric, Document, DocumentId, EmbeddingSpace, EmbeddingVector,
    Provenance, Query, Score, ScoreComponent, SearchError,
};
use crate::ports::{DocumentStore, EmbeddingProvider, LexicalRetriever, RankFusionPolicy, VectorRetriever};
use crate::similarity::cosine_similarity;
use std::collections::HashMap;

/// In-memory document and chunk store for fast test execution.
#[derive(Default)]
pub struct InMemoryDocumentStore {
    documents: HashMap<DocumentId, Document>,
    chunks: HashMap<ChunkId, Chunk>,
}

impl InMemoryDocumentStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl DocumentStore for InMemoryDocumentStore {
    fn put_document(&mut self, doc: Document) -> Result<(), SearchError> {
        self.documents.insert(doc.id.clone(), doc);
        Ok(())
    }

    fn get_document(&self, id: &DocumentId) -> Result<Option<Document>, SearchError> {
        Ok(self.documents.get(id).cloned())
    }

    fn delete_document(&mut self, id: &DocumentId) -> Result<bool, SearchError> {
        Ok(self.documents.remove(id).is_some())
    }

    fn put_chunk(&mut self, chunk: Chunk) -> Result<(), SearchError> {
        self.chunks.insert(chunk.id.clone(), chunk);
        Ok(())
    }

    fn get_chunk(&self, id: &ChunkId) -> Result<Option<Chunk>, SearchError> {
        Ok(self.chunks.get(id).cloned())
    }

    fn delete_chunk(&mut self, id: &ChunkId) -> Result<bool, SearchError> {
        Ok(self.chunks.remove(id).is_some())
    }
}

/// In-memory token-based lexical retriever double.
#[derive(Default)]
pub struct FakeLexicalRetriever {
    chunks: HashMap<ChunkId, String>,
}

impl FakeLexicalRetriever {
    pub fn new() -> Self {
        Self::default()
    }
}

impl LexicalRetriever for FakeLexicalRetriever {
    fn index_chunk(&mut self, chunk: &Chunk) -> Result<(), SearchError> {
        self.chunks.insert(chunk.id.clone(), chunk.text.to_lowercase());
        Ok(())
    }

    fn remove_chunk(&mut self, chunk_id: &ChunkId) -> Result<(), SearchError> {
        self.chunks.remove(chunk_id);
        Ok(())
    }

    fn search(&self, query: &Query, top_k: usize) -> Result<Vec<(ChunkId, f32)>, SearchError> {
        let query_terms: Vec<&str> = query.normalized_query.split_whitespace().collect();
        let mut scored: Vec<(ChunkId, f32)> = Vec::new();

        for (id, text) in &self.chunks {
            let mut matches = 0.0f32;
            for term in &query_terms {
                if text.contains(term) {
                    matches += 1.0;
                }
            }
            if matches > 0.0 {
                // Normalize by query term count
                let score = matches / (query_terms.len().max(1) as f32);
                scored.push((id.clone(), score));
            }
        }

        // Sort descending by score
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        if scored.len() > top_k {
            scored.truncate(top_k);
        }

        Ok(scored)
    }
}

/// Deterministic mock embedding provider for tests without external network or models.
pub struct FakeEmbeddingProvider {
    space: EmbeddingSpace,
}

impl FakeEmbeddingProvider {
    pub fn new(space: EmbeddingSpace) -> Self {
        Self { space }
    }

    pub fn standard_test_space() -> EmbeddingSpace {
        EmbeddingSpace {
            provider_id: "test-fake-provider".to_string(),
            model_id: "deterministic-mock-v1".to_string(),
            revision: "1.0.0".to_string(),
            dimensions: 16,
            distance_metric: DistanceMetric::Cosine,
            normalized: true,
            supported_languages: vec!["en".to_string(), "vi".to_string()],
        }
    }
}

impl Default for FakeEmbeddingProvider {
    fn default() -> Self {
        Self::new(Self::standard_test_space())
    }
}

impl EmbeddingProvider for FakeEmbeddingProvider {
    fn embedding_space(&self) -> &EmbeddingSpace {
        &self.space
    }

    fn embed_text(&self, text: &str) -> Result<EmbeddingVector, SearchError> {
        // Deterministic pseudo-embedding from character frequencies
        let mut vec = vec![0.0f32; self.space.dimensions];
        for (i, b) in text.as_bytes().iter().enumerate() {
            vec[i % self.space.dimensions] += (*b as f32) / 255.0;
        }

        // Normalize if required
        if self.space.normalized {
            let norm: f32 = vec.iter().map(|v| v * v).sum::<f32>().sqrt();
            if norm > 0.0 {
                for v in &mut vec {
                    *v /= norm;
                }
            }
        }

        EmbeddingVector::new(self.space.clone(), vec)
    }
}

/// In-memory vector retriever with strict vector space validation.
pub struct FakeVectorRetriever {
    space: EmbeddingSpace,
    vectors: HashMap<ChunkId, Vec<f32>>,
}

impl FakeVectorRetriever {
    pub fn new(space: EmbeddingSpace) -> Self {
        Self {
            space,
            vectors: HashMap::new(),
        }
    }
}

impl VectorRetriever for FakeVectorRetriever {
    fn embedding_space(&self) -> &EmbeddingSpace {
        &self.space
    }

    fn index_vector(&mut self, chunk_id: &ChunkId, vector: &EmbeddingVector) -> Result<(), SearchError> {
        if !self.space.is_compatible_with(&vector.space) {
            return Err(SearchError::IncompatibleEmbeddingSpace {
                expected: format!("{}/{}-dim{}", self.space.provider_id, self.space.model_id, self.space.dimensions),
                actual: format!("{}/{}-dim{}", vector.space.provider_id, vector.space.model_id, vector.space.dimensions),
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
                expected: format!("{}/{}-dim{}", self.space.provider_id, self.space.model_id, self.space.dimensions),
                actual: format!("{}/{}-dim{}", query_vector.space.provider_id, query_vector.space.model_id, query_vector.space.dimensions),
            });
        }

        let mut scored: Vec<(ChunkId, f32)> = Vec::new();
        for (id, vec) in &self.vectors {
            let sim = cosine_similarity(&query_vector.values, vec);
            scored.push((id.clone(), sim));
        }

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        if scored.len() > top_k {
            scored.truncate(top_k);
        }

        Ok(scored)
    }
}

/// Standard Reciprocal Rank Fusion (RRF) policy.
pub struct ReciprocalRankFusion {
    pub k: f32,
}

impl ReciprocalRankFusion {
    pub fn new(k: f32) -> Self {
        Self { k }
    }
}

impl Default for ReciprocalRankFusion {
    fn default() -> Self {
        Self { k: 60.0 }
    }
}

impl RankFusionPolicy for ReciprocalRankFusion {
    fn fuse(
        &self,
        lexical_candidates: &[(ChunkId, f32)],
        vector_candidates: &[(ChunkId, f32)],
        top_k: usize,
    ) -> Result<Vec<(ChunkId, Score, Provenance)>, SearchError> {
        let mut rrf_scores: HashMap<ChunkId, f32> = HashMap::new();
        let mut components_map: HashMap<ChunkId, Vec<ScoreComponent>> = HashMap::new();
        let mut trace_map: HashMap<ChunkId, HashMap<String, String>> = HashMap::new();
        let mut rank_in_path_map: HashMap<ChunkId, usize> = HashMap::new();
        let mut path_map: HashMap<ChunkId, String> = HashMap::new();

        // 1. Process lexical candidates
        for (rank_0, (id, score)) in lexical_candidates.iter().enumerate() {
            let rank = rank_0 + 1;
            let rrf_inc = 1.0 / (self.k + rank as f32);
            *rrf_scores.entry(id.clone()).or_insert(0.0) += rrf_inc;

            components_map.entry(id.clone()).or_default().push(ScoreComponent {
                name: "lexical_rrf".to_string(),
                raw_value: *score,
                weighted_value: rrf_inc,
                description: format!("Lexical rank {} -> RRF +{:.5}", rank, rrf_inc),
            });

            path_map.insert(id.clone(), "lexical".to_string());
            rank_in_path_map.insert(id.clone(), rank);

            let trace = trace_map.entry(id.clone()).or_default();
            trace.insert("lexical_rank".to_string(), rank.to_string());
            trace.insert("lexical_raw_score".to_string(), score.to_string());
        }

        // 2. Process vector candidates
        for (rank_0, (id, score)) in vector_candidates.iter().enumerate() {
            let rank = rank_0 + 1;
            let rrf_inc = 1.0 / (self.k + rank as f32);
            *rrf_scores.entry(id.clone()).or_insert(0.0) += rrf_inc;

            components_map.entry(id.clone()).or_default().push(ScoreComponent {
                name: "vector_rrf".to_string(),
                raw_value: *score,
                weighted_value: rrf_inc,
                description: format!("Vector rank {} -> RRF +{:.5}", rank, rrf_inc),
            });

            let current_path = path_map.get(id);
            if current_path == Some(&"lexical".to_string()) {
                path_map.insert(id.clone(), "hybrid_rrf".to_string());
            } else {
                path_map.insert(id.clone(), "vector".to_string());
                rank_in_path_map.insert(id.clone(), rank);
            }

            let trace = trace_map.entry(id.clone()).or_default();
            trace.insert("vector_rank".to_string(), rank.to_string());
            trace.insert("vector_similarity".to_string(), score.to_string());
        }

        let mut fused: Vec<(ChunkId, Score, Provenance)> = Vec::new();
        for (id, total_score) in rrf_scores {
            let components = components_map.remove(&id).unwrap_or_default();
            let trace = trace_map.remove(&id).unwrap_or_default();
            let path = path_map.remove(&id).unwrap_or_else(|| "unknown".to_string());
            let rank = rank_in_path_map.remove(&id).unwrap_or(1);

            let score = Score {
                final_score: total_score,
                components,
            };

            let provenance = Provenance {
                retrieval_path: path,
                chunk_id: id.clone(),
                document_id: DocumentId("unknown".to_string()), // updated by orchestrator
                matched_terms: Vec::new(),
                rank_in_path: rank,
                trace_details: trace,
            };

            fused.push((id, score, provenance));
        }

        fused.sort_by(|a, b| b.1.final_score.partial_cmp(&a.1.final_score).unwrap_or(std::cmp::Ordering::Equal));
        if fused.len() > top_k {
            fused.truncate(top_k);
        }

        Ok(fused)
    }
}
