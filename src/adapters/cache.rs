use crate::domain::{
    EmbeddingSpace, EmbeddingVector, QueryFilter, SearchError, SearchResult,
};
use crate::ports::EmbeddingProvider;
use crate::similarity::cosine_similarity;
use std::collections::{HashMap, VecDeque};
use std::sync::RwLock;

/// Cache performance and operational statistics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CacheStats {
    pub hits: usize,
    pub misses: usize,
    pub evictions: usize,
    pub current_size: usize,
    pub capacity: usize,
}

impl CacheStats {
    pub fn hit_ratio(&self) -> f32 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f32 / total as f32
        }
    }
}

/// In-memory LRU-bounded embedding vector cache.
pub struct EmbeddingCache {
    capacity: usize,
    entries: HashMap<String, EmbeddingVector>,
    order: VecDeque<String>,
    hits: usize,
    misses: usize,
    evictions: usize,
}

impl EmbeddingCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: HashMap::with_capacity(capacity.min(1024)),
            order: VecDeque::with_capacity(capacity.min(1024)),
            hits: 0,
            misses: 0,
            evictions: 0,
        }
    }

    fn make_key(space: &EmbeddingSpace, text: &str) -> String {
        format!("{}:{}:{}:{}", space.provider_id, space.model_id, space.revision, text)
    }

    pub fn get(&mut self, space: &EmbeddingSpace, text: &str) -> Option<EmbeddingVector> {
        let key = Self::make_key(space, text);
        if let Some(vec) = self.entries.get(&key) {
            self.hits += 1;
            // Promote to most recently used
            if let Some(pos) = self.order.iter().position(|k| k == &key) {
                self.order.remove(pos);
            }
            self.order.push_back(key);
            Some(vec.clone())
        } else {
            self.misses += 1;
            None
        }
    }

    pub fn put(&mut self, text: &str, vector: EmbeddingVector) {
        let key = Self::make_key(&vector.space, text);
        if self.entries.contains_key(&key) {
            self.entries.insert(key.clone(), vector);
            if let Some(pos) = self.order.iter().position(|k| k == &key) {
                self.order.remove(pos);
            }
            self.order.push_back(key);
            return;
        }

        if self.entries.len() >= self.capacity {
            if let Some(oldest_key) = self.order.pop_front() {
                self.entries.remove(&oldest_key);
                self.evictions += 1;
            }
        }

        self.order.push_back(key.clone());
        self.entries.insert(key, vector);
    }

    pub fn stats(&self) -> CacheStats {
        CacheStats {
            hits: self.hits,
            misses: self.misses,
            evictions: self.evictions,
            current_size: self.entries.len(),
            capacity: self.capacity,
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
    }
}

/// A Decorator that transparently caches text embeddings for any underlying `EmbeddingProvider`.
pub struct CachedEmbeddingProvider<E: EmbeddingProvider> {
    inner: E,
    cache: RwLock<EmbeddingCache>,
}

impl<E: EmbeddingProvider> CachedEmbeddingProvider<E> {
    pub fn new(inner: E, capacity: usize) -> Self {
        Self {
            inner,
            cache: RwLock::new(EmbeddingCache::new(capacity)),
        }
    }

    pub fn inner(&self) -> &E {
        &self.inner
    }

    pub fn stats(&self) -> CacheStats {
        self.cache.read().unwrap().stats()
    }

    pub fn clear_cache(&self) {
        self.cache.write().unwrap().clear();
    }
}

impl<E: EmbeddingProvider> EmbeddingProvider for CachedEmbeddingProvider<E> {
    fn embedding_space(&self) -> &EmbeddingSpace {
        self.inner.embedding_space()
    }

    fn embed_text(&self, text: &str) -> Result<EmbeddingVector, SearchError> {
        let space = self.inner.embedding_space();

        // 1. Fast read lock check
        {
            let mut cache = self.cache.write().unwrap();
            if let Some(cached) = cache.get(space, text) {
                return Ok(cached);
            }
        }

        // 2. Miss: Compute embedding via inner provider
        let vector = self.inner.embed_text(text)?;

        // 3. Store in cache
        {
            let mut cache = self.cache.write().unwrap();
            cache.put(text, vector.clone());
        }

        Ok(vector)
    }

    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<EmbeddingVector>, SearchError> {
        let space = self.inner.embedding_space();
        let mut results = Vec::with_capacity(texts.len());
        let mut uncached_indices = Vec::new();
        let mut uncached_texts = Vec::new();

        {
            let mut cache = self.cache.write().unwrap();
            for (idx, &text) in texts.iter().enumerate() {
                if let Some(cached) = cache.get(space, text) {
                    results.push(Some(cached));
                } else {
                    results.push(None);
                    uncached_indices.push(idx);
                    uncached_texts.push(text);
                }
            }
        }

        if !uncached_texts.is_empty() {
            let computed = self.inner.embed_batch(&uncached_texts)?;
            let mut cache = self.cache.write().unwrap();
            for (orig_idx, (text, vec)) in uncached_indices.into_iter().zip(uncached_texts.into_iter().zip(computed)) {
                cache.put(text, vec.clone());
                results[orig_idx] = Some(vec);
            }
        }

        Ok(results.into_iter().map(|opt| opt.expect("All vectors populated")).collect())
    }
}

/// An entry stored in the semantic query cache.
#[derive(Debug, Clone)]
struct SemanticQueryCacheEntry {
    query_text: String,
    query_vector: EmbeddingVector,
    filter: Option<QueryFilter>,
    results: Vec<SearchResult>,
}

/// Semantic query cache matching repeat or near-duplicate queries via vector cosine similarity.
pub struct SemanticQueryCache {
    capacity: usize,
    similarity_threshold: f32,
    entries: VecDeque<SemanticQueryCacheEntry>,
    hits: usize,
    misses: usize,
    evictions: usize,
}

impl SemanticQueryCache {
    pub fn new(capacity: usize, similarity_threshold: f32) -> Self {
        Self {
            capacity: capacity.max(1),
            similarity_threshold: similarity_threshold.clamp(0.0, 1.0),
            entries: VecDeque::with_capacity(capacity.min(256)),
            hits: 0,
            misses: 0,
            evictions: 0,
        }
    }

    /// Attempt to retrieve cached search results for a query vector and optional filter.
    pub fn get(
        &mut self,
        query_vector: &EmbeddingVector,
        filter: &Option<QueryFilter>,
    ) -> Option<Vec<SearchResult>> {
        for (idx, entry) in self.entries.iter().enumerate() {
            // Must match filter constraints exactly
            if entry.filter.as_ref() != filter.as_ref() {
                continue;
            }

            // Must share compatible embedding space
            if !entry.query_vector.space.is_compatible_with(&query_vector.space) {
                continue;
            }

            // Check cosine similarity against cached query vector
            let sim = cosine_similarity(&query_vector.values, &entry.query_vector.values);
            if sim >= self.similarity_threshold {
                self.hits += 1;
                let results = entry.results.clone();
                // Promote entry
                let entry_val = self.entries.remove(idx).unwrap();
                self.entries.push_back(entry_val);
                return Some(results);
            }
        }

        self.misses += 1;
        None
    }

    /// Insert or update query results in the cache.
    pub fn put(
        &mut self,
        query_text: String,
        query_vector: EmbeddingVector,
        filter: Option<QueryFilter>,
        results: Vec<SearchResult>,
    ) {
        if self.entries.len() >= self.capacity {
            self.entries.pop_front();
            self.evictions += 1;
        }

        self.entries.push_back(SemanticQueryCacheEntry {
            query_text,
            query_vector,
            filter,
            results,
        });
    }

    pub fn peek_latest_query(&self) -> Option<&str> {
        self.entries.back().map(|e| e.query_text.as_str())
    }

    pub fn stats(&self) -> CacheStats {
        CacheStats {
            hits: self.hits,
            misses: self.misses,
            evictions: self.evictions,
            current_size: self.entries.len(),
            capacity: self.capacity,
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}
