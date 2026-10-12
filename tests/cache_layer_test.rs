use semantic_search_agent::adapters::cache::{
    CachedEmbeddingProvider, EmbeddingCache, SemanticQueryCache,
};
use semantic_search_agent::adapters::semantic_embedder::MultilingualSemanticEmbedder;
use semantic_search_agent::domain::{
    ChunkId, DocumentId, Provenance, QueryFilter, Score, SearchResult,
};
use semantic_search_agent::ports::EmbeddingProvider;

#[test]
fn test_cached_embedding_provider_hits_and_misses() {
    let embedder = MultilingualSemanticEmbedder::new();
    let cached_provider = CachedEmbeddingProvider::new(embedder, 5);

    // Initial state
    let stats0 = cached_provider.stats();
    assert_eq!(stats0.hits, 0);
    assert_eq!(stats0.misses, 0);
    assert_eq!(stats0.current_size, 0);

    // First embed -> cache miss
    let vec1 = cached_provider
        .embed_text("tìm hóa đơn dịch vụ")
        .expect("Embedded text 1");
    let stats1 = cached_provider.stats();
    assert_eq!(stats1.hits, 0);
    assert_eq!(stats1.misses, 1);
    assert_eq!(stats1.current_size, 1);

    // Second embed of identical text -> cache hit
    let vec2 = cached_provider
        .embed_text("tìm hóa đơn dịch vụ")
        .expect("Embedded text 2");
    assert_eq!(vec1, vec2);
    let stats2 = cached_provider.stats();
    assert_eq!(stats2.hits, 1);
    assert_eq!(stats2.misses, 1);
    assert_eq!(stats2.hit_ratio(), 0.5);

    // Batch embedding with partial cache hits
    let texts = vec![
        "tìm hóa đơn dịch vụ", // Cached
        "hợp đồng kinh tế",     // Uncached
        "biên lai thu tiền",    // Uncached
    ];
    let batch_vecs = cached_provider
        .embed_batch(&texts)
        .expect("Batch embedded");
    assert_eq!(batch_vecs.len(), 3);
    assert_eq!(batch_vecs[0], vec1);

    let stats3 = cached_provider.stats();
    assert_eq!(stats3.hits, 2); // 1 from previous single embed + 1 from batch
    assert_eq!(stats3.misses, 3); // 1 from previous single embed + 2 from batch
    assert_eq!(stats3.current_size, 3);
}

#[test]
fn test_embedding_cache_eviction_bounded_capacity() {
    let mut cache = EmbeddingCache::new(3);
    let embedder = MultilingualSemanticEmbedder::new();
    let space = embedder.embedding_space().clone();

    let v1 = embedder.embed_text("text 1").unwrap();
    let v2 = embedder.embed_text("text 2").unwrap();
    let v3 = embedder.embed_text("text 3").unwrap();
    let v4 = embedder.embed_text("text 4").unwrap();

    cache.put("text 1", v1);
    cache.put("text 2", v2);
    cache.put("text 3", v3);

    let stats = cache.stats();
    assert_eq!(stats.current_size, 3);
    assert_eq!(stats.evictions, 0);

    // Inserting 4th item evicts oldest (text 1)
    cache.put("text 4", v4);
    let stats = cache.stats();
    assert_eq!(stats.current_size, 3);
    assert_eq!(stats.evictions, 1);

    assert!(cache.get(&space, "text 1").is_none());
    assert!(cache.get(&space, "text 2").is_some());
    assert!(cache.get(&space, "text 4").is_some());
}

#[test]
fn test_semantic_query_cache_near_match_and_filter_isolation() {
    let embedder = MultilingualSemanticEmbedder::new();
    let mut query_cache = SemanticQueryCache::new(10, 0.95);

    let q1_text = "tìm hóa đơn dịch vụ";
    let q1_vec = embedder.embed_text(q1_text).unwrap();

    let fake_results = vec![SearchResult {
        rank: 1,
        document_id: DocumentId("doc-invoice-1".to_string()),
        document_title: "Invoice #1".to_string(),
        chunk_id: ChunkId("doc-invoice-1-c0".to_string()),
        score: Score::simple(0.95, "semantic_vector"),
        provenance: Provenance {
            retrieval_path: "vector".to_string(),
            chunk_id: ChunkId("doc-invoice-1-c0".to_string()),
            document_id: DocumentId("doc-invoice-1".to_string()),
            matched_terms: vec![],
            rank_in_path: 1,
            trace_details: std::collections::HashMap::new(),
        },
        snippet: "Invoice #1".to_string(),
    }];

    // Put into semantic cache with no filters
    query_cache.put(
        q1_text.to_string(),
        q1_vec.clone(),
        None,
        fake_results.clone(),
    );
    assert_eq!(query_cache.peek_latest_query(), Some(q1_text));

    // 1. Exact query hits cache
    let cached_res1 = query_cache.get(&q1_vec, &None);
    assert!(cached_res1.is_some());
    assert_eq!(cached_res1.unwrap().len(), 1);

    // 2. Near-semantic query ("tìm hóa đơn dịch vụ tháng 8")
    // Should have very high cosine similarity with q1_vec
    let q2_vec = embedder.embed_text("hóa đơn dịch vụ").unwrap();
    let sim = semantic_search_agent::similarity::cosine_similarity(&q1_vec.values, &q2_vec.values);
    println!("Similarity between q1 and q2: {}", sim);
    let cached_res2 = query_cache.get(&q2_vec, &None);
    assert!(cached_res2.is_some(), "Near-duplicate semantic query should hit cache (sim={})", sim);

    // 3. Different filter must NOT hit cache even if query vector is identical
    let mut filter_with_pdf = QueryFilter::default();
    filter_with_pdf.required_extensions = vec![".pdf".to_string()];
    let filtered_res = query_cache.get(&q1_vec, &Some(filter_with_pdf));
    assert!(filtered_res.is_none(), "Filter mismatch must prevent cache hit");

    // 4. Semantically distant query must miss
    let distant_vec = embedder.embed_text("lỗi đăng nhập credentials").unwrap();
    let distant_res = query_cache.get(&distant_vec, &None);
    assert!(distant_res.is_none(), "Semantically distant query must miss");

    let stats = query_cache.stats();
    assert_eq!(stats.hits, 2);
    assert_eq!(stats.misses, 2);
}
