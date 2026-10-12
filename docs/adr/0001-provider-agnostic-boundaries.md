# ADR 0001: Provider-Agnostic Boundaries and Replaceable Adapters

## Status
Accepted

## Context
The system requires natural-language semantic search, query understanding, and retrieval capabilities. Many AI search systems become hardwired to specific cloud vendors (e.g., OpenAI, Google Gemini, Anthropic), specific vector databases (e.g., Pinecone, Qdrant), or specific framework ecosystems. Such coupling breaks local-only privacy, increases vendor lock-in, and makes offline/WASM embedding impossible.

## Decision
1. **Domain Isolation**: Core domain entities (`Document`, `Chunk`, `Query`, `SearchResult`, `Filter`, `Score`, `Provenance`) and application orchestration logic must not import any external provider SDKs or backend-specific database drivers.
2. **Capability-Driven Ports**: All external capabilities (e.g. `TextEmbedding`, `LexicalRetrieval`, `VectorRetrieval`, `AiTextGeneration`, `StructuredOutput`) are declared as abstract ports/traits with explicit capability descriptors.
3. **Replaceable Adapters**: Concrete implementations (such as Google AI Studio REST adapter, local ONNX/WASM embedding adapters, or BM25 indexers) reside entirely in adapter layers.
4. **Deterministic Fakes**: All core contracts must have deterministic in-memory fakes to enable unit testing and CI without requiring network access, credentials, or live external services.

## Consequences
- Positive: Total flexibility to run completely locally (zero-knowledge) or switch cloud AI providers without rewriting domain rules.
- Positive: 100% CI reproducibility using in-memory test doubles.
- Trade-off: Requires defining clean data-transfer boundaries and explicit error normalization between adapters and the core domain.
