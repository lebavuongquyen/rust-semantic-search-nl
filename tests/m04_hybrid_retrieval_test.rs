use semantic_search_agent::adapters::fakes::{InMemoryDocumentStore, ReciprocalRankFusion};
use semantic_search_agent::adapters::lexical_bm25::Bm25LexicalRetriever;
use semantic_search_agent::adapters::semantic_embedder::MultilingualSemanticEmbedder;
use semantic_search_agent::adapters::vector_index::MemoryVectorIndex;
use semantic_search_agent::application::SearchOrchestrator;
use semantic_search_agent::domain::{Query, QueryFilter};
use semantic_search_agent::evaluation::EvaluationHarness;
use semantic_search_agent::ports::EmbeddingProvider;

#[test]
fn test_m04_hybrid_retrieval_and_constraint_benchmark() {
    let corpus_json = include_str!("data/golden_corpus.json");
    let queries_json = include_str!("data/golden_queries.json");

    let docs = EvaluationHarness::parse_golden_corpus(corpus_json)
        .expect("Parsed golden corpus");
    let golden_queries = EvaluationHarness::parse_golden_queries(queries_json)
        .expect("Parsed golden queries");

    let store = InMemoryDocumentStore::new();
    let bm25 = Bm25LexicalRetriever::default();
    let embedder = MultilingualSemanticEmbedder::new();
    let space = embedder.embedding_space().clone();
    let vector_index = MemoryVectorIndex::new(space);
    let fusion = ReciprocalRankFusion::new(60.0);

    // Full hybrid orchestrator combining BM25 lexical + Semantic vector index + RRF fusion
    let mut orchestrator = SearchOrchestrator::new(
        store,
        Some(bm25),
        Some(vector_index),
        Some(embedder),
        fusion,
    );

    for doc in docs {
        orchestrator
            .ingest_document(doc, 500)
            .expect("Ingested document");
    }

    let k = 5;
    let mut query_metrics = Vec::new();

    println!("\n=======================================================");
    println!("M04: HYBRID (BM25 + SEMANTIC VECTOR + RRF) BENCHMARK REPORT");
    println!("=======================================================");

    for golden in &golden_queries {
        let query = Query::new(&golden.query).with_top_k(k);
        let results = orchestrator.search(&query).expect("Search executed");

        let metric = EvaluationHarness::evaluate_query_results(golden, &results, k);
        println!(
            "Query: {:<32} | Recall@{}: {:.2} | Prec@{}: {:.2} | MRR: {:.2} | nDCG: {:.2} | Top: {:?}",
            metric.query_text, k, metric.recall_at_k, k, metric.precision_at_k, metric.reciprocal_rank, metric.ndcg_at_k, metric.retrieved_doc_ids
        );

        // Verify provenance is recorded
        if !results.is_empty() {
            let top_prov = &results[0].provenance;
            assert!(
                top_prov.retrieval_path == "hybrid_rrf"
                    || top_prov.retrieval_path == "lexical"
                    || top_prov.retrieval_path == "vector",
                "Retrieval path must be tracked: {}", top_prov.retrieval_path
            );
        }

        query_metrics.push(metric);
    }

    let report = EvaluationHarness::generate_report("hybrid_bm25_vector_rrf", k, query_metrics);

    println!("-------------------------------------------------------");
    println!("OVERALL M04 HYBRID SUMMARY:");
    println!("  Total Queries:         {}", report.total_queries);
    println!("  Mean Recall@{}:         {:.4}", k, report.mean_recall);
    println!("  Mean Precision@{}:      {:.4}", k, report.mean_precision);
    println!("  Mean Reciprocal Rank:  {:.4}", report.mean_reciprocal_rank);
    println!("  Mean nDCG@{}:           {:.4}", k, report.mean_ndcg);
    println!("=======================================================\n");

    // Comparative gate: Hybrid recall must remain high (>= 0.85) and MRR >= 0.80
    assert!(report.mean_recall >= 0.85);
    assert!(report.mean_reciprocal_rank >= 0.80);
}

#[test]
fn test_m04_hard_negative_exclusion_with_filters() {
    let corpus_json = include_str!("data/golden_corpus.json");
    let docs = EvaluationHarness::parse_golden_corpus(corpus_json).unwrap();

    let store = InMemoryDocumentStore::new();
    let bm25 = Bm25LexicalRetriever::default();
    let embedder = MultilingualSemanticEmbedder::new();
    let space = embedder.embedding_space().clone();
    let vector_index = MemoryVectorIndex::new(space);
    let fusion = ReciprocalRankFusion::new(60.0);

    let mut orchestrator = SearchOrchestrator::new(
        store,
        Some(bm25),
        Some(vector_index),
        Some(embedder),
        fusion,
    );

    for doc in docs {
        orchestrator.ingest_document(doc, 500).unwrap();
    }

    // Query for invoices, but with a strict negative keyword filter excluding "draft"
    // and required extension ".pdf"
    let query = Query::new("invoice dịch vụ")
        .with_filter(QueryFilter {
            required_extensions: vec!["pdf".to_string()],
            negative_keywords: vec!["draft".to_string(), "template".to_string()],
            ..Default::default()
        })
        .with_top_k(5);

    let results = orchestrator.search(&query).unwrap();

    // Verify that the hard negative "doc-hard-negative-invoice-draft" is NEVER returned
    for r in &results {
        assert_ne!(
            r.document_id.0, "doc-hard-negative-invoice-draft",
            "Draft document must be filtered out by hard constraint"
        );
        assert!(r.snippet.to_lowercase().find("draft").is_none());
    }

    // The legitimate PDF invoice must be returned
    assert!(results.iter().any(|r| r.document_id.0 == "doc-en-invoice-aug"));
}
