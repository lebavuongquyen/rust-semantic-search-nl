use semantic_search_agent::adapters::fakes::{InMemoryDocumentStore, ReciprocalRankFusion};
use semantic_search_agent::adapters::semantic_embedder::MultilingualSemanticEmbedder;
use semantic_search_agent::adapters::vector_index::MemoryVectorIndex;
use semantic_search_agent::application::SearchOrchestrator;
use semantic_search_agent::domain::{
    ChunkId, DistanceMetric, EmbeddingSpace, EmbeddingVector, Query,
};
use semantic_search_agent::evaluation::EvaluationHarness;
use semantic_search_agent::ports::{EmbeddingProvider, VectorRetriever};
use std::collections::HashMap;

#[test]
fn test_m03_cross_lingual_semantic_evaluation() {
    let corpus_json = include_str!("data/golden_corpus.json");
    let queries_json = include_str!("data/golden_queries.json");

    let docs = EvaluationHarness::parse_golden_corpus(corpus_json)
        .expect("Parsed golden corpus");
    let golden_queries = EvaluationHarness::parse_golden_queries(queries_json)
        .expect("Parsed golden queries");

    let store = InMemoryDocumentStore::new();
    let embedder = MultilingualSemanticEmbedder::new();
    let space = embedder.embedding_space().clone();
    let vector_index = MemoryVectorIndex::new(space);
    let fusion = ReciprocalRankFusion::default();

    // Semantic-only search orchestrator (no lexical index)
    let mut orchestrator = SearchOrchestrator::<
        InMemoryDocumentStore,
        semantic_search_agent::adapters::fakes::FakeLexicalRetriever,
        MemoryVectorIndex,
        MultilingualSemanticEmbedder,
        ReciprocalRankFusion,
    >::new(store, None, Some(vector_index), Some(embedder), fusion);

    // Ingest golden documents
    for doc in docs {
        orchestrator
            .ingest_document(doc, 500)
            .expect("Ingested document");
    }

    let k = 5;
    let mut query_metrics = Vec::new();

    println!("\n=======================================================");
    println!("M03: CROSS-LINGUAL SEMANTIC RETRIEVAL EVALUATION REPORT");
    println!("=======================================================");

    for golden in &golden_queries {
        let query = Query::new(&golden.query).with_top_k(k);
        let results = orchestrator.search(&query).expect("Search executed");

        let metric = EvaluationHarness::evaluate_query_results(golden, &results, k);
        println!(
            "Query: {:<32} | Recall@{}: {:.2} | Prec@{}: {:.2} | MRR: {:.2} | nDCG: {:.2} | Top: {:?}",
            metric.query_text, k, metric.recall_at_k, k, metric.precision_at_k, metric.reciprocal_rank, metric.ndcg_at_k, metric.retrieved_doc_ids
        );
        query_metrics.push(metric);
    }

    let report = EvaluationHarness::generate_report("cross_lingual_semantic", k, query_metrics);

    println!("-------------------------------------------------------");
    println!("OVERALL M03 SEMANTIC SUMMARY:");
    println!("  Total Queries:         {}", report.total_queries);
    println!("  Mean Recall@{}:         {:.4}", k, report.mean_recall);
    println!("  Mean Precision@{}:      {:.4}", k, report.mean_precision);
    println!("  Mean Reciprocal Rank:  {:.4}", report.mean_reciprocal_rank);
    println!("  Mean nDCG@{}:           {:.4}", k, report.mean_ndcg);
    println!("=======================================================\n");

    // Acceptance criteria:
    // Semantic cross-lingual retrieval must achieve substantially higher recall than the lexical baseline (0.4167)
    assert!(report.mean_recall >= 0.80, "Cross-lingual semantic recall must be >= 0.80 (actual: {})", report.mean_recall);
    assert!(report.mean_reciprocal_rank >= 0.80, "MRR must be >= 0.80 (actual: {})", report.mean_reciprocal_rank);
}

#[test]
fn test_m03_index_migration_and_compatibility() {
    let space_v1 = EmbeddingSpace {
        provider_id: "test-engine".to_string(),
        model_id: "embed-model".to_string(),
        revision: "1.0.0".to_string(),
        dimensions: 4,
        distance_metric: DistanceMetric::Cosine,
        normalized: true,
        supported_languages: vec!["en".to_string()],
    };

    let mut space_v2 = space_v1.clone();
    space_v2.revision = "2.0.0".to_string();

    let mut index = MemoryVectorIndex::new(space_v1.clone());
    assert_eq!(index.index_version(), 1);

    let chunk_id = ChunkId("c1".to_string());
    let vec_v1 = EmbeddingVector::new(space_v1.clone(), vec![1.0, 0.0, 0.0, 0.0]).unwrap();
    index.index_vector(&chunk_id, &vec_v1).unwrap();

    // 1. Incompatible space indexing is rejected
    let vec_v2 = EmbeddingVector::new(space_v2.clone(), vec![0.5, 0.5, 0.5, 0.5]).unwrap();
    let err = index.index_vector(&chunk_id, &vec_v2);
    assert!(err.is_err(), "Must reject vector from space v2 while index is v1");

    // 2. Safe migration to v2
    let mut chunk_texts = HashMap::new();
    chunk_texts.insert(chunk_id.clone(), "test content for c1".to_string());

    let migration_result = index.migrate_to(space_v2.clone(), &chunk_texts, |_text| {
        Ok(EmbeddingVector::new(space_v2.clone(), vec![0.0, 1.0, 0.0, 0.0]).unwrap())
    });

    assert!(migration_result.is_ok(), "Migration should succeed");
    assert_eq!(index.index_version(), 2);
    assert_eq!(index.embedding_space().revision, "2.0.0");

    // After migration, searching with v2 query succeeds
    let query_v2 = EmbeddingVector::new(space_v2.clone(), vec![0.0, 1.0, 0.0, 0.0]).unwrap();
    let search_res = index.search_vector(&query_v2, 5).unwrap();
    assert_eq!(search_res.len(), 1);
    assert_eq!(search_res[0].0, chunk_id);
    assert!((search_res[0].1 - 1.0).abs() < 1e-5);
}
