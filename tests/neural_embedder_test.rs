use semantic_search_agent::adapters::cache::CachedEmbeddingProvider;
use semantic_search_agent::adapters::fakes::{InMemoryDocumentStore, ReciprocalRankFusion};
use semantic_search_agent::adapters::neural_embedder::{
    EmbeddingTransport, ModelEndpointEmbeddingAdapter, NeuralModelConfig,
};
use semantic_search_agent::adapters::vector_index::MemoryVectorIndex;
use semantic_search_agent::application::SearchOrchestrator;
use semantic_search_agent::domain::{Query, SearchError};
use semantic_search_agent::evaluation::EvaluationHarness;
use semantic_search_agent::ports::EmbeddingProvider;

#[test]
fn test_neural_embedder_multilingual_mpnet_evaluation() {
    let corpus_json = include_str!("data/golden_corpus.json");
    let queries_json = include_str!("data/golden_queries.json");

    let docs = EvaluationHarness::parse_golden_corpus(corpus_json).expect("Parsed golden corpus");
    let golden_queries =
        EvaluationHarness::parse_golden_queries(queries_json).expect("Parsed golden queries");

    let config = NeuralModelConfig::all_multilingual_mpnet_base();
    assert_eq!(config.dimensions, 768);
    assert!(config.supported_languages.contains(&"vi".to_string()));
    assert!(config.supported_languages.contains(&"en".to_string()));

    let neural_adapter = ModelEndpointEmbeddingAdapter::new_offline(config);
    let space = neural_adapter.embedding_space().clone();
    assert_eq!(space.dimensions, 768);

    let store = InMemoryDocumentStore::new();
    let vector_index = MemoryVectorIndex::new(space);
    let fusion = ReciprocalRankFusion::default();

    let mut orchestrator = SearchOrchestrator::<
        InMemoryDocumentStore,
        semantic_search_agent::adapters::fakes::FakeLexicalRetriever,
        MemoryVectorIndex,
        ModelEndpointEmbeddingAdapter,
        ReciprocalRankFusion,
    >::new(store, None, Some(vector_index), Some(neural_adapter), fusion);

    for doc in docs {
        orchestrator
            .ingest_document(doc, 500)
            .expect("Ingested document");
    }

    let k = 5;
    let mut query_metrics = Vec::new();

    println!("\n=======================================================");
    println!("NEURAL EMBEDDER: all-multilingual-mpnet-base (768-dim)");
    println!("=======================================================");

    for golden in &golden_queries {
        let query = Query::new(&golden.query).with_top_k(k);
        let results = orchestrator.search(&query).expect("Search executed");

        let metric = EvaluationHarness::evaluate_query_results(golden, &results, k);
        println!(
            "Query: {:<32} | Recall@{}: {:.2} | MRR: {:.2} | Top: {:?}",
            metric.query_text, k, metric.recall_at_k, metric.reciprocal_rank, metric.retrieved_doc_ids
        );
        query_metrics.push(metric);
    }

    let report = EvaluationHarness::generate_report("neural_mpnet_768d", k, query_metrics);
    println!("-------------------------------------------------------");
    println!("Mean Recall@{}: {:.4}", k, report.mean_recall);
    println!("Mean MRR:      {:.4}", report.mean_reciprocal_rank);
    println!("=======================================================\n");

    assert!(report.mean_recall >= 0.80, "Neural model recall must be >= 0.80");
    assert!(report.mean_reciprocal_rank >= 0.80, "Neural model MRR must be >= 0.80");
}

#[test]
fn test_neural_embedder_dimension_mismatch_fails_explicitly() {
    struct BrokenTransport;
    impl EmbeddingTransport for BrokenTransport {
        fn execute_embedding(
            &self,
            _config: &NeuralModelConfig,
            texts: &[&str],
        ) -> Result<Vec<Vec<f32>>, SearchError> {
            // Return wrong dimension (3 values instead of 768)
            Ok(vec![vec![1.0, 2.0, 3.0]; texts.len()])
        }
    }

    let config = NeuralModelConfig::all_multilingual_mpnet_base();
    let adapter = ModelEndpointEmbeddingAdapter::with_transport(config, BrokenTransport);

    let err = adapter.embed_text("test dimension mismatch").unwrap_err();
    match err {
        SearchError::DimensionMismatch { expected, actual } => {
            assert_eq!(expected, 768);
            assert_eq!(actual, 3);
        }
        _ => panic!("Expected SearchError::DimensionMismatch, got: {:?}", err),
    }
}

#[test]
fn test_neural_embedder_composition_with_cache() {
    let config = NeuralModelConfig::bge_m3();
    assert_eq!(config.dimensions, 1024);

    let neural_adapter = ModelEndpointEmbeddingAdapter::new_offline(config);
    let cached_adapter = CachedEmbeddingProvider::new(neural_adapter, 100);

    let v1 = cached_adapter.embed_text("tìm kiếm ngữ nghĩa đa ngôn ngữ").unwrap();
    assert_eq!(v1.values.len(), 1024);
    assert_eq!(cached_adapter.stats().misses, 1);
    assert_eq!(cached_adapter.stats().hits, 0);

    let v2 = cached_adapter.embed_text("tìm kiếm ngữ nghĩa đa ngôn ngữ").unwrap();
    assert_eq!(v1, v2);
    assert_eq!(cached_adapter.stats().hits, 1);
}
