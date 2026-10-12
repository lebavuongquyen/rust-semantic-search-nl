use semantic_search_agent::adapters::fakes::{InMemoryDocumentStore, ReciprocalRankFusion};
use semantic_search_agent::adapters::lexical_bm25::Bm25LexicalRetriever;
use semantic_search_agent::adapters::semantic_embedder::MultilingualSemanticEmbedder;
use semantic_search_agent::adapters::vector_index::MemoryVectorIndex;
use semantic_search_agent::application::SearchOrchestrator;
use semantic_search_agent::domain::Query;
use semantic_search_agent::evaluation::{
    EvaluationHarness, QueryEvaluationMetric, RegressionGateConfig, RegressionGateOutcome,
    RegressionViolation,
};
use semantic_search_agent::ports::EmbeddingProvider;
use std::time::Instant;

#[test]
fn test_m09_continuous_evaluation_and_sliced_benchmarks() {
    let corpus_json = include_str!("data/golden_corpus.json");
    let queries_json = include_str!("data/golden_queries.json");

    let docs = EvaluationHarness::parse_golden_corpus(corpus_json).expect("Parsed golden corpus");
    let golden_queries =
        EvaluationHarness::parse_golden_queries(queries_json).expect("Parsed golden queries");

    let store = InMemoryDocumentStore::new();
    let lexical = Bm25LexicalRetriever::default();
    let embedder = MultilingualSemanticEmbedder::new();
    let space = embedder.embedding_space().clone();
    let vector_index = MemoryVectorIndex::new(space);
    let fusion = ReciprocalRankFusion::default();

    let mut orchestrator = SearchOrchestrator::new(
        store,
        Some(lexical),
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
    println!("M09: CONTINUOUS EVALUATION WITH LATENCY & SLICES");
    println!("=======================================================");

    for golden in &golden_queries {
        let query = Query::new(&golden.query).with_top_k(k);

        let start = Instant::now();
        let results = orchestrator.search(&query).expect("Search executed");
        let duration_ms = start.elapsed().as_micros() as f64 / 1000.0;

        let metric =
            EvaluationHarness::evaluate_query_results_with_duration(golden, &results, k, duration_ms);

        println!(
            "Query: {:<32} | Latency: {:.3}ms | Recall@{}: {:.2} | MRR: {:.2}",
            metric.query_text, metric.duration_ms, k, metric.recall_at_k, metric.reciprocal_rank
        );
        query_metrics.push(metric);
    }

    let slices = EvaluationHarness::compute_slices(&golden_queries, &query_metrics);
    let report =
        EvaluationHarness::generate_report_with_slices("hybrid_bm25_vector", k, query_metrics, slices);

    println!("-------------------------------------------------------");
    println!("OVERALL M09 SUMMARY:");
    println!("  Total Queries:         {}", report.total_queries);
    println!("  Mean Recall@{}:         {:.4}", k, report.mean_recall);
    println!("  Mean Reciprocal Rank:  {:.4}", report.mean_reciprocal_rank);
    println!("  Mean Latency:          {:.3}ms", report.mean_duration_ms);
    println!("  p50 Latency:           {:.3}ms", report.p50_duration_ms);
    println!("  p95 Latency:           {:.3}ms", report.p95_duration_ms);
    println!("-------------------------------------------------------");
    println!("SLICED BREAKDOWNS:");
    for (slice_name, s) in &report.slices {
        println!(
            "  [{:<22}] N={:<2} | Recall: {:.2} | MRR: {:.2} | Latency: {:.3}ms",
            slice_name, s.count, s.mean_recall, s.mean_mrr, s.mean_duration_ms
        );
    }
    println!("=======================================================\n");

    // Verify slicing invariants
    assert!(report.slices.contains_key("lang:vi"));
    assert!(report.slices.contains_key("target_lang:en"));
    assert!(report.slices.contains_key("slice:cross_lingual"));

    let cross_slice = report.slices.get("slice:cross_lingual").unwrap();
    assert!(cross_slice.count >= 4, "Cross-lingual slice should contain at least 4 queries");
    assert!(cross_slice.mean_recall >= 0.80, "Cross-lingual recall should be >= 0.80");

    // Regression Gate: Baseline comparing to itself must pass
    let config = RegressionGateConfig::default();
    let outcome = EvaluationHarness::evaluate_regression(&report, &report, &config);
    assert_eq!(outcome, RegressionGateOutcome::Passed);
}

#[test]
fn test_m09_regression_gate_blocks_degradations() {
    // 1. Create a baseline report
    let baseline_metrics = vec![
        QueryEvaluationMetric {
            query_id: "q1".to_string(),
            query_text: "invoice".to_string(),
            recall_at_k: 1.0,
            precision_at_k: 0.5,
            reciprocal_rank: 1.0,
            ndcg_at_k: 1.0,
            retrieved_doc_ids: vec!["d1".to_string()],
            duration_ms: 10.0,
        },
        QueryEvaluationMetric {
            query_id: "q2".to_string(),
            query_text: "contract".to_string(),
            recall_at_k: 1.0,
            precision_at_k: 0.5,
            reciprocal_rank: 1.0,
            ndcg_at_k: 1.0,
            retrieved_doc_ids: vec!["d2".to_string()],
            duration_ms: 12.0,
        },
    ];
    let baseline = EvaluationHarness::generate_report("baseline_v1", 5, baseline_metrics);

    // 2. Candidate with severe recall drop (e.g. buggy retriever)
    let degraded_recall_metrics = vec![
        QueryEvaluationMetric {
            query_id: "q1".to_string(),
            query_text: "invoice".to_string(),
            recall_at_k: 0.5, // dropped from 1.0
            precision_at_k: 0.2,
            reciprocal_rank: 0.5,
            ndcg_at_k: 0.5,
            retrieved_doc_ids: vec!["d3".to_string()],
            duration_ms: 11.0,
        },
        QueryEvaluationMetric {
            query_id: "q2".to_string(),
            query_text: "contract".to_string(),
            recall_at_k: 0.0, // dropped from 1.0
            precision_at_k: 0.0,
            reciprocal_rank: 0.0,
            ndcg_at_k: 0.0,
            retrieved_doc_ids: vec!["d3".to_string()],
            duration_ms: 10.0,
        },
    ];
    let degraded_recall_candidate =
        EvaluationHarness::generate_report("degraded_recall", 5, degraded_recall_metrics);

    let config = RegressionGateConfig {
        max_recall_drop: 0.05,
        max_mrr_drop: 0.05,
        max_ndcg_drop: 0.05,
        max_latency_increase_ratio: 0.50,
        max_p95_duration_ms: Some(100.0),
        min_canary_recall: Some(0.80),
    };

    let outcome =
        EvaluationHarness::evaluate_regression(&baseline, &degraded_recall_candidate, &config);
    match outcome {
        RegressionGateOutcome::Rejected(violations) => {
            assert!(violations.iter().any(|v| matches!(v, RegressionViolation::RecallDrop { .. })));
            assert!(violations.iter().any(|v| matches!(v, RegressionViolation::CanaryRecallTooLow { .. })));
        }
        RegressionGateOutcome::Passed => panic!("Expected regression gate to block recall drop!"),
    }

    // 3. Candidate with severe latency spike
    let latency_spike_metrics = vec![
        QueryEvaluationMetric {
            query_id: "q1".to_string(),
            query_text: "invoice".to_string(),
            recall_at_k: 1.0,
            precision_at_k: 0.5,
            reciprocal_rank: 1.0,
            ndcg_at_k: 1.0,
            retrieved_doc_ids: vec!["d1".to_string()],
            duration_ms: 250.0, // Baseline was 10ms -> 2400% spike
        },
        QueryEvaluationMetric {
            query_id: "q2".to_string(),
            query_text: "contract".to_string(),
            recall_at_k: 1.0,
            precision_at_k: 0.5,
            reciprocal_rank: 1.0,
            ndcg_at_k: 1.0,
            retrieved_doc_ids: vec!["d2".to_string()],
            duration_ms: 280.0,
        },
    ];
    let latency_candidate =
        EvaluationHarness::generate_report("latency_spike", 5, latency_spike_metrics);

    let latency_outcome =
        EvaluationHarness::evaluate_regression(&baseline, &latency_candidate, &config);
    match latency_outcome {
        RegressionGateOutcome::Rejected(violations) => {
            assert!(violations.iter().any(|v| matches!(v, RegressionViolation::LatencySpike { .. })));
            assert!(violations.iter().any(|v| matches!(v, RegressionViolation::P95LatencyExceeded { .. })));
        }
        RegressionGateOutcome::Passed => panic!("Expected regression gate to block latency spike!"),
    }
}
