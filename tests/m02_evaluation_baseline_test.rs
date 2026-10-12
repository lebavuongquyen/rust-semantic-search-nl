use semantic_search_agent::adapters::fakes::{InMemoryDocumentStore, ReciprocalRankFusion};
use semantic_search_agent::adapters::lexical_bm25::Bm25LexicalRetriever;
use semantic_search_agent::application::SearchOrchestrator;
use semantic_search_agent::domain::Query;
use semantic_search_agent::evaluation::EvaluationHarness;

#[test]
fn test_m02_lexical_bm25_baseline_evaluation() {
    let corpus_json = include_str!("data/golden_corpus.json");
    let queries_json = include_str!("data/golden_queries.json");

    let docs = EvaluationHarness::parse_golden_corpus(corpus_json)
        .expect("Parsed golden corpus");
    let golden_queries = EvaluationHarness::parse_golden_queries(queries_json)
        .expect("Parsed golden queries");

    let store = InMemoryDocumentStore::new();
    let bm25 = Bm25LexicalRetriever::default();
    let fusion = ReciprocalRankFusion::default();

    // Type annotations: None for VectorRetriever and EmbeddingProvider
    let mut orchestrator = SearchOrchestrator::<
        InMemoryDocumentStore,
        Bm25LexicalRetriever,
        semantic_search_agent::adapters::fakes::FakeVectorRetriever,
        semantic_search_agent::adapters::fakes::FakeEmbeddingProvider,
        ReciprocalRankFusion,
    >::new(store, Some(bm25), None, None, fusion);

    // Ingest all golden documents
    for doc in docs {
        orchestrator
            .ingest_document(doc, 500)
            .expect("Ingested document");
    }

    let k = 5;
    let mut query_metrics = Vec::new();

    println!("\n=======================================================");
    println!("M02: EVALUATION HARNESS - LEXICAL BM25 BASELINE REPORT");
    println!("=======================================================");

    for golden in &golden_queries {
        let query = Query::new(&golden.query).with_top_k(k);
        let results = orchestrator.search(&query).expect("Search executed");

        let metric = EvaluationHarness::evaluate_query_results(golden, &results, k);
        println!(
            "Query: {:<32} | Recall@{}: {:.2} | Prec@{}: {:.2} | MRR: {:.2} | nDCG: {:.2} | Retrieved: {:?}",
            metric.query_text, k, metric.recall_at_k, k, metric.precision_at_k, metric.reciprocal_rank, metric.ndcg_at_k, metric.retrieved_doc_ids
        );
        query_metrics.push(metric);
    }

    let report = EvaluationHarness::generate_report("bm25_lexical_baseline", k, query_metrics);

    println!("-------------------------------------------------------");
    println!("OVERALL BASELINE SUMMARY:");
    println!("  Total Queries:         {}", report.total_queries);
    println!("  Mean Recall@{}:         {:.4}", k, report.mean_recall);
    println!("  Mean Precision@{}:      {:.4}", k, report.mean_precision);
    println!("  Mean Reciprocal Rank:  {:.4}", report.mean_reciprocal_rank);
    println!("  Mean nDCG@{}:           {:.4}", k, report.mean_ndcg);
    println!("=======================================================\n");

    // Acceptance criteria check:
    // Lexical baseline works for exact keyword matches (e.g. login credentials, signed contract),
    // but clearly demonstrates the documented limitations on cross-lingual vocabulary mismatches
    assert_eq!(report.total_queries, 6);
    assert!(report.mean_recall > 0.0, "Lexical baseline should retrieve matching keyword queries");
    // Verify report serialization round-trip
    let serialized = serde_json::to_string_pretty(&report).expect("Serialized report");
    assert!(!serialized.is_empty());
}
