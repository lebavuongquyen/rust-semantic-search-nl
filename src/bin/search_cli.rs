use semantic_search_agent::adapters::fakes::ReciprocalRankFusion;
use semantic_search_agent::adapters::lexical_bm25::Bm25LexicalRetriever;
use semantic_search_agent::adapters::neural_embedder::{ModelEndpointEmbeddingAdapter, NeuralModelConfig};
use semantic_search_agent::adapters::vector_index::MemoryVectorIndex;
use semantic_search_agent::application::SearchOrchestrator;
use semantic_search_agent::domain::Query;
use semantic_search_agent::evaluation::EvaluationHarness;
use semantic_search_agent::ports::EmbeddingProvider;
use std::env;
use std::time::Instant;

fn main() {
    let args: Vec<String> = env::args().collect();
    let query_str = if args.len() > 1 {
        args[1..].join(" ")
    } else {
        "tìm hóa đơn dịch vụ".to_string()
    };

    println!("===============================================================");
    println!("  RUST SEMANTIC SEARCH NL - CLI RUNNER");
    println!("===============================================================");
    println!("Query: \"{}\"\n", query_str);

    // Initialize golden corpus
    let corpus_json = include_str!("../../tests/data/golden_corpus.json");
    let docs = EvaluationHarness::parse_golden_corpus(corpus_json)
        .expect("Failed to parse default corpus");

    // Initialize components
    let store = semantic_search_agent::adapters::fakes::InMemoryDocumentStore::new();
    let lexical = Bm25LexicalRetriever::default();
    let config = NeuralModelConfig::all_multilingual_mpnet_base();
    let embedder = ModelEndpointEmbeddingAdapter::new_offline(config);
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

    // Ingest documents
    let start_ingest = Instant::now();
    for doc in docs {
        orchestrator
            .ingest_document(doc, 500)
            .expect("Ingested document");
    }
    let ingest_dur = start_ingest.elapsed();
    println!("Ingested golden documents in {:.2?}", ingest_dur);

    // Execute search
    let query = Query::new(&query_str).with_top_k(5);
    let start_search = Instant::now();
    let results = orchestrator.search(&query).expect("Search failed");
    let search_dur = start_search.elapsed();

    println!("Search executed in {:.3}ms\n", search_dur.as_micros() as f64 / 1000.0);
    println!("---------------------------------------------------------------");
    println!("  TOP RESULTS (Rank | Score | Path | Document Title)");
    println!("---------------------------------------------------------------");

    if results.is_empty() {
        println!("No matching documents found.");
    } else {
        for res in &results {
            println!(
                "#{:<2} [Score: {:.4}] [{:<10}] {}",
                res.rank, res.score.final_score, res.provenance.retrieval_path, res.document_title
            );
            println!("    Doc ID : {}", res.document_id);
            println!("    Snippet: {}\n", res.snippet.replace('\n', " "));
        }
    }
    println!("===============================================================");
}
