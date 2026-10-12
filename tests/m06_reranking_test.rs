use semantic_search_agent::adapters::fakes::{InMemoryDocumentStore, ReciprocalRankFusion};
use semantic_search_agent::adapters::lexical_bm25::Bm25LexicalRetriever;
use semantic_search_agent::adapters::reranker::CrossEncoderReranker;
use semantic_search_agent::adapters::semantic_embedder::MultilingualSemanticEmbedder;
use semantic_search_agent::adapters::vector_index::MemoryVectorIndex;
use semantic_search_agent::application::SearchOrchestrator;
use semantic_search_agent::domain::{Document, Query};
use semantic_search_agent::ports::EmbeddingProvider;

#[test]
fn test_m06_reranker_optimizes_candidate_order() {
    let store = InMemoryDocumentStore::new();
    let bm25 = Bm25LexicalRetriever::default();
    let embedder = MultilingualSemanticEmbedder::new();
    let space = embedder.embedding_space().clone();
    let vector_index = MemoryVectorIndex::new(space);
    let fusion = ReciprocalRankFusion::default();
    let reranker = CrossEncoderReranker::new(10);

    // Orchestrator WITH Reranker
    let mut orchestrator = SearchOrchestrator::new(
        store,
        Some(bm25),
        Some(vector_index),
        Some(embedder),
        fusion,
    )
    .with_reranker(reranker);

    let doc1 = Document::new("doc-exact", "Consulting Services Invoice INV-2026", "Official consulting services invoice")
        .with_file_path("finance/invoice.pdf");
    let doc2 = Document::new("doc-general", "General Company Services", "Company offers wide variety of services")
        .with_file_path("company/services.pdf");

    orchestrator.ingest_document(doc1, 200).unwrap();
    orchestrator.ingest_document(doc2, 200).unwrap();

    let query = Query::new("consulting services invoice").with_top_k(5);
    let results = orchestrator.search(&query).unwrap();

    assert!(!results.is_empty());
    // Rank 1 must be doc-exact
    assert_eq!(results[0].document_id.0, "doc-exact");
    assert_eq!(results[0].rank, 1);

    // Verify cross_encoder_rerank score component is attached
    let has_rerank_component = results[0]
        .score
        .components
        .iter()
        .any(|c| c.name == "cross_encoder_rerank");
    assert!(has_rerank_component, "Score must include cross_encoder_rerank component");
}
