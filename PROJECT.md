# SemanticSearchAgent — Project Specification and Status

**Status as of 2026-10-12:** Milestones M00 to M08 and M12 completed; verified with empirical benchmarks and 21 passing automated tests.

## 1. Mission

Build a provider-agnostic natural-language search system that retrieves relevant documents by meaning, supports cross-lingual queries (for example Vietnamese `hóa đơn` ↔ English `invoice`) without hardcoded translation pairs, preserves constraints and provenance, and can evolve without locking the domain core to a specific technology.

## 2. Non-Negotiable Requirements

- No specific library, framework, embedding model, inference runtime, OCR engine, vector database, search backend, AI provider, or agent framework is mandatory.
- All replaceable implementations must sit behind project-owned contracts/adapters and expose validated capabilities.
- Local-only operation must work without credentials or network.
- Remote AI processing is opt-in, disclosed, and consent-based; fallback must not silently change privacy semantics.
- Cross-lingual retrieval must be demonstrated in both directions with a representative, labeled evaluation set and hard negatives.
- No accuracy, scale, or latency claims without reproducible evidence.
- Mandatory filters, negation, access control, and index/model compatibility must be preserved.

## 3. Implemented Architecture Modules

- `src/domain/`: Domain entities (`Document`, `Chunk`, `EmbeddingSpace`, `EmbeddingVector`, `Query`, `QueryFilter`, `Score`, `Provenance`, `SearchResult`, `NormalizedQueryIntent`).
- `src/ports/`: Trait interfaces (`LexicalRetriever`, `VectorRetriever`, `EmbeddingProvider`, `RankFusionPolicy`, `QueryUnderstandingPort`, `RerankerPort`, `DocumentStore`).
- `src/adapters/`:
  - `lexical_bm25`: Okapi BM25 indexer & retriever.
  - `semantic_embedder`: Multilingual semantic continuous space embedder.
  - `vector_index`: Memory vector index with space validation and safe migration.
  - `query_analyzer`: Natural-language intent parser preserving mandatory constraints and negations.
  - `reranker`: Cross-encoder precision reranker.
  - `fakes`: Deterministic in-memory test doubles.
- `src/agent/`: Controlled search agent with bounded decomposition and loop detection.
- `src/security/`: Tenant isolation access control policy and telemetry secret redaction.
- `src/evaluation/`: Benchmark harness evaluating Recall@K, Precision@K, MRR, and nDCG@K on golden datasets.
- `ts/`: Provider-neutral AI contracts (`ai-provider-contract.ts`), Google AI Studio REST adapter (`google-ai-studio-provider.ts`), and migrated orchestrator (`google-ai-studio-orchestrator.ts`).

## 4. Verified Baseline (2026-10-12)

- `cargo test`: **21 passed, 0 failed, 0 warnings** across unit, contract, lexical baseline, semantic cross-lingual, hybrid fusion, query understanding, reranking, agent, and security suites.
- `cargo check --target wasm32-unknown-unknown`: passed.
- `cargo build --target wasm32-unknown-unknown --release`: passed, generating 607 KB `.wasm` binary.
- TypeScript verification: `pnpm dlx typescript --noEmit` passed (0 errors); `node target/ts_dist/tests/provider-contract.test.js` passed (5/5 tests).

## 5. Source-of-Truth Documents

1. [ROADMAP.md](ROADMAP.md) — current status and next step.
2. [AGENTS.md](AGENTS.md) — agent/contributor instructions.
3. [ARCHITECTURE_PLAN.md](ARCHITECTURE_PLAN.md) — target architecture.
4. [Detailed roadmap](docs/SEMANTIC_SEARCH_AGENT_ROADMAP.md) — milestones, acceptance criteria, quality gates.
5. [AI provider architecture](docs/AI_PROVIDER_ARCHITECTURE.md) — provider contracts and adapter status.
6. [README.md](README.md) — quick start and verified feature summary.

## 6. Next Step

Proceed with **M09 — Continuous Evaluation and Optimization** (regression gates, canary thresholds, and ongoing strategy evaluation).
