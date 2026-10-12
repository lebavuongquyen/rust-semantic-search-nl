use semantic_search_agent::adapters::fakes::{InMemoryDocumentStore, ReciprocalRankFusion};
use semantic_search_agent::adapters::lexical_bm25::Bm25LexicalRetriever;
use semantic_search_agent::adapters::semantic_embedder::MultilingualSemanticEmbedder;
use semantic_search_agent::adapters::vector_index::MemoryVectorIndex;
use semantic_search_agent::agent::{AgentExecutionPolicy, ControlledSearchAgent};
use semantic_search_agent::application::SearchOrchestrator;
use semantic_search_agent::domain::{Document, Query};
use semantic_search_agent::ports::EmbeddingProvider;

#[test]
fn test_m07_compound_query_decomposition_and_provenance() {
    let policy = AgentExecutionPolicy {
        max_iterations: 3,
        max_subqueries: 3,
    };
    let agent = ControlledSearchAgent::new(policy);

    let raw = "tìm hóa đơn dịch vụ và hợp đồng đã ký";
    let subqueries = agent.decompose_query(raw);

    assert_eq!(subqueries.len(), 2);
    assert_eq!(subqueries[0], "tìm hóa đơn dịch vụ");
    assert_eq!(subqueries[1], "hợp đồng đã ký");

    // Ingest documents
    let store = InMemoryDocumentStore::new();
    let bm25 = Bm25LexicalRetriever::default();
    let embedder = MultilingualSemanticEmbedder::new();
    let space = embedder.embedding_space().clone();
    let vector_index = MemoryVectorIndex::new(space);
    let fusion = ReciprocalRankFusion::default();

    let mut orchestrator = SearchOrchestrator::new(
        store,
        Some(bm25),
        Some(vector_index),
        Some(embedder),
        fusion,
    );

    let doc_inv = Document::new("doc-invoice", "Invoice Consulting 2026", "Invoice amount due 4500 USD")
        .with_file_path("finance/invoice.pdf");
    let doc_contract = Document::new("doc-contract", "Signed Agreement 2026", "Signed contract between parties")
        .with_file_path("legal/contract.pdf");

    orchestrator.ingest_document(doc_inv, 200).unwrap();
    orchestrator.ingest_document(doc_contract, 200).unwrap();

    // Execute multi-step search through agent
    let merged = agent
        .execute_bounded_search(raw, |subq| {
            let q = Query::new(subq).with_top_k(3);
            orchestrator.search(&q)
        })
        .expect("Agent search executed");

    assert_eq!(merged.len(), 2, "Agent should merge results from both subqueries");

    // Verify subquery origin is tracked in provenance trace
    let origins: Vec<Option<&String>> = merged
        .iter()
        .map(|r| r.provenance.trace_details.get("subquery_origin"))
        .collect();
    assert!(origins.iter().all(|o| o.is_some()));
}

#[test]
fn test_m07_bounded_iterations_limit() {
    let policy = AgentExecutionPolicy {
        max_iterations: 1, // Strict limit: 1 iteration only
        max_subqueries: 5,
    };
    let agent = ControlledSearchAgent::new(policy);

    let mut executed_count = 0;
    let _ = agent.execute_bounded_search("subq1 và subq2 và subq3", |_| {
        executed_count += 1;
        Ok(Vec::new())
    });

    assert_eq!(executed_count, 1, "Agent execution must stop at max_iterations limit");
}
