use crate::domain::{
    Chunk, ChunkId, Document, DocumentId, NormalizedQueryIntent, Query, QueryFilter, SearchError, SearchResult,
};
use crate::ports::{
    DocumentStore, EmbeddingProvider, LexicalRetriever, NoopReranker, RankFusionPolicy,
    RerankerPort, VectorRetriever,
};
use std::collections::HashMap;

/// Application search orchestrator implementing provider-neutral search workflows.
pub struct SearchOrchestrator<
    S: DocumentStore,
    L: LexicalRetriever,
    V: VectorRetriever,
    E: EmbeddingProvider,
    F: RankFusionPolicy,
    R: RerankerPort = NoopReranker,
> {
    store: S,
    lexical: Option<L>,
    vector: Option<V>,
    embedder: Option<E>,
    fusion_policy: F,
    reranker: Option<R>,
    doc_chunks_map: HashMap<DocumentId, Vec<ChunkId>>,
}

impl<
    S: DocumentStore,
    L: LexicalRetriever,
    V: VectorRetriever,
    E: EmbeddingProvider,
    F: RankFusionPolicy,
> SearchOrchestrator<S, L, V, E, F, NoopReranker> {
    pub fn new(
        store: S,
        lexical: Option<L>,
        vector: Option<V>,
        embedder: Option<E>,
        fusion_policy: F,
    ) -> Self {
        Self {
            store,
            lexical,
            vector,
            embedder,
            fusion_policy,
            reranker: None,
            doc_chunks_map: HashMap::new(),
        }
    }

    pub fn with_reranker<R2: RerankerPort>(
        self,
        reranker: R2,
    ) -> SearchOrchestrator<S, L, V, E, F, R2> {
        SearchOrchestrator {
            store: self.store,
            lexical: self.lexical,
            vector: self.vector,
            embedder: self.embedder,
            fusion_policy: self.fusion_policy,
            reranker: Some(reranker),
            doc_chunks_map: self.doc_chunks_map,
        }
    }
}

impl<
    S: DocumentStore,
    L: LexicalRetriever,
    V: VectorRetriever,
    E: EmbeddingProvider,
    F: RankFusionPolicy,
    R: RerankerPort,
> SearchOrchestrator<S, L, V, E, F, R> {
    pub fn store(&self) -> &S {
        &self.store
    }

    pub fn store_mut(&mut self) -> &mut S {
        &mut self.store
    }

    pub fn lexical(&self) -> Option<&L> {
        self.lexical.as_ref()
    }

    pub fn lexical_mut(&mut self) -> Option<&mut L> {
        self.lexical.as_mut()
    }

    pub fn vector(&self) -> Option<&V> {
        self.vector.as_ref()
    }

    pub fn vector_mut(&mut self) -> Option<&mut V> {
        self.vector.as_mut()
    }

    pub fn embedder(&self) -> Option<&E> {
        self.embedder.as_ref()
    }

    /// Ingest a full document, automatically chunking its content on UTF-8 char boundaries and updating index ports.
    /// If chunk_size == 0, returns an InvalidConfiguration error.
    /// If an error occurs midway, previous indexed chunks for this document are rolled back.
    pub fn ingest_document(
        &mut self,
        doc: Document,
        chunk_size: usize,
    ) -> Result<Vec<ChunkId>, SearchError> {
        if chunk_size == 0 {
            return Err(SearchError::InvalidConfiguration(
                "chunk_size must be greater than zero".to_string(),
            ));
        }

        let doc_id = doc.id.clone();

        // If document was previously ingested, clean up old chunks and indexes first (re-ingestion)
        if self.doc_chunks_map.contains_key(&doc_id) {
            let _ = self.delete_document(&doc_id);
        }

        let content = doc.content.clone();
        let metadata = doc.metadata.clone();

        // Store the document
        self.store.put_document(doc)?;

        let mut chunks = Vec::new();
        let mut char_indices: Vec<usize> = content.char_indices().map(|(idx, _)| idx).collect();
        char_indices.push(content.len());

        let total_chars = char_indices.len().saturating_sub(1);
        let mut char_start = 0;
        let mut chunk_idx = 0;

        let mut indexed_lexical_chunks: Vec<ChunkId> = Vec::new();
        let mut indexed_vector_chunks: Vec<ChunkId> = Vec::new();
        let mut stored_chunks: Vec<ChunkId> = Vec::new();

        let mut ingest_result: Result<(), SearchError> = Ok(());

        while char_start < total_chars {
            let char_end = (char_start + chunk_size).min(total_chars);
            let byte_start = char_indices[char_start];
            let byte_end = char_indices[char_end];

            let chunk_text = &content[byte_start..byte_end];
            let chunk_id = ChunkId(format!("{}-c{}", doc_id, chunk_idx));
            let chunk = Chunk {
                id: chunk_id.clone(),
                document_id: doc_id.clone(),
                chunk_index: chunk_idx,
                text: chunk_text.to_string(),
                start_offset: byte_start,
                end_offset: byte_end,
                metadata: metadata.clone(),
            };

            // Index in lexical port
            if let Some(lex) = &mut self.lexical {
                if let Err(e) = lex.index_chunk(&chunk) {
                    ingest_result = Err(e);
                    break;
                }
                indexed_lexical_chunks.push(chunk_id.clone());
            }

            // Index in vector port
            if let (Some(vec_retriever), Some(emb_provider)) = (&mut self.vector, &self.embedder) {
                let vector = match emb_provider.embed_text(&chunk.text) {
                    Ok(v) => v,
                    Err(e) => {
                        ingest_result = Err(e);
                        break;
                    }
                };
                if let Err(e) = vec_retriever.index_vector(&chunk.id, &vector) {
                    ingest_result = Err(e);
                    break;
                }
                indexed_vector_chunks.push(chunk_id.clone());
            }

            if let Err(e) = self.store.put_chunk(chunk) {
                ingest_result = Err(e);
                break;
            }
            stored_chunks.push(chunk_id.clone());
            chunks.push(chunk_id);

            char_start = char_end;
            chunk_idx += 1;
        }

        if let Err(err) = ingest_result {
            // Rollback on partial failure
            if let Some(lex) = &mut self.lexical {
                for cid in &indexed_lexical_chunks {
                    let _ = lex.remove_chunk(cid);
                }
            }
            if let Some(vec_retriever) = &mut self.vector {
                for cid in &indexed_vector_chunks {
                    let _ = vec_retriever.remove_vector(cid);
                }
            }
            for cid in &stored_chunks {
                let _ = self.store.delete_chunk(cid);
            }
            let _ = self.store.delete_document(&doc_id);
            return Err(err);
        }

        self.doc_chunks_map.insert(doc_id, chunks.clone());
        Ok(chunks)
    }

    /// Delete a document and safely propagate deletion to all chunks, storage, and index ports.
    pub fn delete_document(&mut self, doc_id: &DocumentId) -> Result<bool, SearchError> {
        let chunk_ids = self.doc_chunks_map.remove(doc_id);
        let mut had_data = false;

        if let Some(cids) = chunk_ids {
            had_data = true;
            for cid in &cids {
                if let Some(lex) = &mut self.lexical {
                    lex.remove_chunk(cid)?;
                }
                if let Some(vec_retriever) = &mut self.vector {
                    vec_retriever.remove_vector(cid)?;
                }
                let _ = self.store.delete_chunk(cid)?;
            }
        }

        let doc_deleted = self.store.delete_document(doc_id)?;
        Ok(had_data || doc_deleted)
    }

    /// Search based on a validated query intent, ensuring mandatory constraints are strictly preserved.
    pub fn search_with_intent(
        &self,
        intent: &NormalizedQueryIntent,
        top_k: usize,
    ) -> Result<Vec<SearchResult>, SearchError> {
        // Enforce invariants
        intent.validate()?;

        let mut query = Query::new(&intent.raw_query)
            .with_filter(intent.mandatory_filter.clone())
            .with_top_k(top_k);

        query.language = intent.detected_language.clone();

        self.search(&query)
    }

    /// Search across available retrieval ports, fuse scores, enforce hard constraints, and return results.
    pub fn search(&self, query: &Query) -> Result<Vec<SearchResult>, SearchError> {
        if query.top_k == 0 {
            return Ok(Vec::new());
        }

        let candidate_limit = query.top_k.saturating_mul(3).max(10).min(500);

        let lexical_candidates = match &self.lexical {
            Some(lex) => lex.search(query, candidate_limit)?,
            None => Vec::new(),
        };

        let vector_candidates = match (&self.vector, &self.embedder) {
            (Some(vec_retriever), Some(emb_provider)) => {
                let query_vector = emb_provider.embed_text(&query.raw_query)?;
                vec_retriever.search_vector(&query_vector, candidate_limit)?
            }
            _ => Vec::new(),
        };

        if lexical_candidates.is_empty() && vector_candidates.is_empty() {
            return Ok(Vec::new());
        }

        // Fuse candidates using rank fusion policy
        let fused = self.fusion_policy.fuse(
            &lexical_candidates,
            &vector_candidates,
            candidate_limit,
        )?;

        // Apply structured filters and assemble SearchResult models
        let mut results = Vec::new();
        let mut rank = 1;

        for (chunk_id, score, mut provenance) in fused {
            let chunk = match self.store.get_chunk(&chunk_id)? {
                Some(c) => c,
                None => continue,
            };

            let doc = match self.store.get_document(&chunk.document_id)? {
                Some(d) => d,
                None => continue,
            };

            // Evaluate hard filters
            if !self.passes_filter(&chunk, &doc, &query.filter) {
                continue;
            }

            provenance.document_id = doc.id.clone();
            for (k, v) in &chunk.metadata {
                provenance.trace_details.insert(k.clone(), v.clone());
            }

            results.push(SearchResult {
                rank,
                chunk_id: chunk.id,
                document_id: doc.id,
                document_title: doc.title,
                snippet: chunk.text,
                score,
                provenance,
            });

            rank += 1;
            if results.len() >= query.top_k * 2 {
                break;
            }
        }

        // Apply optional precision reranker
        if let Some(reranker) = &self.reranker {
            results = reranker.rerank(&query.raw_query, results, query.top_k)?;
        } else if results.len() > query.top_k {
            results.truncate(query.top_k);
        }

        Ok(results)
    }

    fn passes_filter(&self, chunk: &Chunk, doc: &Document, filter: &QueryFilter) -> bool {
        let chunk_lower = chunk.text.to_lowercase();
        let title_lower = doc.title.to_lowercase();

        // 1. Negative keywords constraint
        for neg in &filter.negative_keywords {
            let neg_lower = neg.to_lowercase();
            if chunk_lower.contains(&neg_lower) || title_lower.contains(&neg_lower) {
                return false;
            }
        }

        // 2. Extension constraint
        if !filter.required_extensions.is_empty() {
            if let Some(path) = &doc.file_path {
                let path_lower = path.to_lowercase();
                let matches_ext = filter
                    .required_extensions
                    .iter()
                    .any(|ext| path_lower.ends_with(&format!(".{}", ext.to_lowercase())));
                if !matches_ext {
                    return false;
                }
            } else {
                return false;
            }
        }

        // 3. Path prefix constraint
        if let Some(prefix) = &filter.path_prefix {
            if let Some(path) = &doc.file_path {
                if !path.starts_with(prefix) {
                    return false;
                }
            } else {
                return false;
            }
        }

        // 4. Required tags constraint (matches against metadata keys, values, or comma-separated "tags" metadata)
        if !filter.required_tags.is_empty() {
            let doc_tags_str = doc.metadata.get("tags").cloned().unwrap_or_default().to_lowercase();
            let chunk_tags_str = chunk.metadata.get("tags").cloned().unwrap_or_default().to_lowercase();
            let category_str = doc.metadata.get("category").cloned().unwrap_or_default().to_lowercase();

            for req_tag in &filter.required_tags {
                let tag_lower = req_tag.to_lowercase();
                let matches_doc = doc_tags_str.split(',').any(|t| t.trim() == tag_lower);
                let matches_chunk = chunk_tags_str.split(',').any(|t| t.trim() == tag_lower);
                let matches_cat = category_str == tag_lower;
                let matches_meta_val = doc.metadata.values().any(|v| v.to_lowercase() == tag_lower)
                    || chunk.metadata.values().any(|v| v.to_lowercase() == tag_lower);

                if !matches_doc && !matches_chunk && !matches_cat && !matches_meta_val {
                    return false;
                }
            }
        }

        // 5. Temporal year constraint
        if let Some(req_year) = filter.temporal_year {
            let req_year_str = req_year.to_string();
            let matches_doc_year = doc.metadata.get("year").map(|y| y == &req_year_str).unwrap_or(false);
            let matches_chunk_year = chunk.metadata.get("year").map(|y| y == &req_year_str).unwrap_or(false);
            let text_contains_year = chunk_lower.contains(&req_year_str) || title_lower.contains(&req_year_str);

            if !matches_doc_year && !matches_chunk_year && !text_contains_year {
                return false;
            }
        }

        // 6. Temporal month constraint
        if let Some(req_month) = filter.temporal_month {
            let month_num_str = req_month.to_string();
            let month_pad_str = format!("{:02}", req_month);
            let matches_doc_month = doc.metadata.get("month").map(|m| m == &month_num_str || m == &month_pad_str).unwrap_or(false);
            let matches_chunk_month = chunk.metadata.get("month").map(|m| m == &month_num_str || m == &month_pad_str).unwrap_or(false);

            // Also check standard month names and formats in text
            let month_names = match req_month {
                1 => vec!["january", "tháng 1", "tháng 01"],
                2 => vec!["february", "tháng 2", "tháng 02"],
                3 => vec!["march", "tháng 3", "tháng 03"],
                4 => vec!["april", "tháng 4", "tháng 04"],
                5 => vec!["may", "tháng 5", "tháng 05"],
                6 => vec!["june", "tháng 6", "tháng 06"],
                7 => vec!["july", "tháng 7", "tháng 07"],
                8 => vec!["august", "tháng 8", "tháng 08"],
                9 => vec!["september", "tháng 9", "tháng 09"],
                10 => vec!["october", "tháng 10"],
                11 => vec!["november", "tháng 11"],
                12 => vec!["december", "tháng 12"],
                _ => vec![],
            };

            let text_contains_month = month_names.iter().any(|m| chunk_lower.contains(m) || title_lower.contains(m));

            if !matches_doc_month && !matches_chunk_month && !text_contains_month {
                return false;
            }
        }

        true
    }
}
