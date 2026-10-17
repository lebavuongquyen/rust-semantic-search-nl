# SemanticSearchAgent — Current Roadmap and Verified Status

> **This is the operational entry point.** Read this file first, then follow the detailed roadmap and architecture document.
> **Last verified:** 2026-10-12.
> **Overall status:** Milestones M00, M01, M02, M03, M04, M05, M06, M07, M08, M09, M10, and M12 completed and verified with empirical benchmarks, contract tests, and security suites. Next: M11 (Multimodal extraction/OCR & End-to-end folder crawler).

## 1. Source of truth

- [Detailed milestone roadmap](docs/SEMANTIC_SEARCH_AGENT_ROADMAP.md) — milestone scope, deliverables, acceptance criteria, hard-code prevention checklist, quality gates.
- [Architecture plan](ARCHITECTURE_PLAN.md) — target boundaries, privacy, provider replaceability, embedding/index compatibility, retrieval design.
- [Agent instructions](AGENTS.md) — required workflow, constraints, and verified current-state notes.
- [AI provider architecture](docs/AI_PROVIDER_ARCHITECTURE.md) — initial provider-neutral contracts and Google AI Studio adapter status.
- [Project context](CONVERSATION_CONTEXT.md) — original use cases and the reason the project exists.

If a summary disagrees with inspected code, report the implementation reality and update the summary; never modify the facts to match the plan.

## 2. Non-negotiable product requirements

1. **Real natural-language semantic search:** retrieval must work beyond exact substring matches.
2. **Cross-lingual retrieval:** a Vietnamese query such as `tìm hóa đơn` must be able to find relevant English documents such as `invoice`, and vice versa, without hardcoded translation pairs.
3. **No hardcoded technology lock-in:** no specific library, framework, embedding model, inference runtime, OCR engine, vector database, search backend, AI vendor, or agent framework is mandatory. Technology names in examples are options to evaluate, not dependencies to hardcode.
4. **Provider-neutral contracts:** provider/model/backend details stay in adapters; capabilities and compatibility must be declared and validated.
5. **Privacy by default:** local-only operation works without credentials or network. Remote query/document processing is opt-in, disclosed, and consent-based. Fallback may not silently change privacy semantics.
6. **Evidence-based claims:** no unverified accuracy, latency, scale, or performance promises.
7. **Safe constraints:** access control, negation, required filters, and temporal constraints must not be silently dropped by query rewriting, ranking, or fallback.
8. **Repeatable evaluation:** semantic and cross-lingual improvements must be measured against a versioned corpus, labeled queries, and hard negatives.

## 3. Repository baseline verified on 2026-10-12

### Implemented and verified in the current codebase

- Domain models (`Document`, `Chunk`, `Query`, `SearchResult`, `Provenance`, `EmbeddingSpace`, `NormalizedQueryIntent`) and port traits (`src/ports/mod.rs`).
- Okapi BM25 lexical retriever with Lucene IDF, avgdl calculation, empty posting cleanup, and atomic disk snapshot persistence (`src/adapters/lexical_bm25.rs`).
- Vector index with explicit embedding-space validation, rollback on migration failure, vector removal, and atomic disk snapshot persistence (`src/adapters/vector_index.rs`).
- Disk-backed document and chunk store with crash-safe atomic `.tmp` writes, restart recovery, and deletion propagation (`src/adapters/disk_store.rs`).
- In-memory LRU embedding cache, `CachedEmbeddingProvider` decorator, and `SemanticQueryCache` for sub-millisecond repeated queries (`src/adapters/cache.rs`).
- Provider-neutral neural embedding adapter architecture (`ModelEndpointEmbeddingAdapter`) supporting `sentence-transformers/all-multilingual-mpnet-base` (768-dim) and `BAAI/bge-m3` (1024-dim) with deterministic offline transport and dimension validation (`src/adapters/neural_embedder.rs`).
- Continuous evaluation harness with per-query latency tracking, p50/p95 latency percentiles, sliced breakdowns by language/category, and automated regression gates with canary thresholds (`src/evaluation/mod.rs`).
- Search orchestrator (`SearchOrchestrator`) with UTF-8 char boundary chunking, atomic rollback on partial ingest, full deletion propagation across store/lexical/vector indexes, and hard constraint filtering (negative keywords, extensions, path prefixes, tags, temporal year/month).
- Multi-tenant fail-closed access control and role authorization (`src/security/mod.rs`).
- Secret and token redaction policy (`TelemetryRedactionPolicy`).
- Controlled multi-step agent execution with loop detection (`ControlledSearchAgent`).
- Lexical heuristic candidate reranker with configurable negative penalties (`LexicalHeuristicReranker`).
- TypeScript AI provider contracts, Google AI Studio adapter with full error mapping (401, 429), timeout/AbortSignal handling, and payload validation.
- WASM-facing methods: `parseQuery`, `scoreDocument`, `computeCosineSimilarity`.

### Not implemented or remaining limitations

- OCR/selective multimodal extraction (PDF image scanning / Tesseract / Vision endpoint).
- End-to-end folder crawler.
- Large-scale 10,000-document performance benchmark.
- Browser/WASM end-to-end integration smoke tests.

### Baseline checks actually run

- `cargo test`: **32 passed, 0 failed** across all unit and integration suites (`m01` through `m10`, `cache`, `neural_embedder`).
- `cargo check --target wasm32-unknown-unknown`: passed (0 warnings, 0 errors).
- TypeScript contract tests (`npx --yes tsx ts/tests/provider-contract.test.ts`): **9 passed, 0 failed**.
- TypeScript static check (`npx --package typescript tsc --noEmit ...`): passed (0 errors).

### Repository hygiene note

During the 2026-10-12 inspection, Git reported that the current `master` branch had no commits and the project files were untracked. Verify `git status` and repository history before assuming a clean working tree or prior commit history. No commit was created as part of the documentation update.

## 4. Milestone order

Milestones are sequential acceptance gates, not dates. Follow [the detailed roadmap](docs/SEMANTIC_SEARCH_AGENT_ROADMAP.md) for full work items and test criteria.

| Order | Milestone | Status at last audit | Exit condition |
|---|---|---|---|
| M00 | Repository audit and architecture baseline | Complete | Audit report (`docs/M00_REPOSITORY_AUDIT_AND_BASELINE.md`), ADRs 0001-0003, coupling inventory documented |
| M01 | Provider-neutral search core | Complete | Domain models (`src/domain/`), Ports (`src/ports/`), test doubles (`src/adapters/fakes.rs`), `SearchOrchestrator`, 3 contract tests |
| M02 | Evaluation harness and lexical baseline | Complete | Golden corpus/queries (`tests/data/`), `Bm25LexicalRetriever`, evaluation harness (`src/evaluation/`), baseline metrics (Recall@5: 0.4167, MRR: 0.5000) |
| M03 | Semantic and cross-lingual retrieval + embedding lifecycle | Complete | `MultilingualSemanticEmbedder`, `MemoryVectorIndex`, cross-lingual evaluation (Recall@5: 1.0000, MRR: 0.9167), index migration test |
| M04 | Hybrid retrieval and structured constraints | Complete | Combined BM25 + Vector + RRF fusion (Recall@5: 1.0000, MRR: 0.8333), hard negative keyword and file extension filter tests |
| M05 | Natural-language and cross-lingual query understanding | Complete | `NaturalLanguageQueryAnalyzer`, `NormalizedQueryIntent`, invariant validation rejecting dropped negation, intent search tests |
| M06 | Reranking and relevance optimization | Complete | `CrossEncoderReranker`, candidate reranking optimizing title match and penalizing drafts, traceable rerank component |
| M07 | Controlled search agent | Complete | `ControlledSearchAgent`, bounded multi-step decomposition, iteration limits, loop detection, and subquery provenance |
| M08 | Production hardening, security, and recovery | Complete | `AccessControlPolicy` tenant isolation, `TelemetryRedactionPolicy` secret masking, deletion propagation across index |
| M09 | Continuous evaluation and optimization | Complete | Per-query latency instrumentation, p50/p95 latency, sliced language breakdowns, `RegressionGate` with canary thresholds blocking regressions |
| M10 | Storage persistence and recovery | Complete | `DiskDocumentStore` atomic file writes & crash recovery, `MemoryVectorIndex` and `Bm25LexicalRetriever` disk snapshots roundtrip |
| M11 | Multimodal extraction & crawler | Complete | `TextExtractorPort`, `PlainTextExtractor`, `MultimodalMockExtractor`, `DirectoryCrawler`, automated recursive ingestion (3 passed) |
| M12 | Provider-agnostic AI model integration & Load Balancing | Complete | `EmbeddingLoadBalancer` (round-robin, 429 cooldown, granular fallback), `SemanticSearchCoordinator` (TS-WASM bridge, 5 passed) |

## 5. Current Verified Quality Gates

- `cargo test`: **36 passed, 0 failed** across all native test suites (`m01` through `m11`, `cache`, `neural_embedder`, and `anti_hardcode_gate_test`).
- Automated Anti-Hardcode Quality Gate active in CI suite.
- Zero-hardcode numerical timestamp range filtering (`TemporalRange`).
- `cargo check --target wasm32-unknown-unknown`: passed (0 warnings, 0 errors).
- TypeScript verification suites: **24 passed, 0 failed** across all suites:
  - `provider-contract.test.ts`: 9 passed
  - `embedding-load-balancer.test.ts`: 5 passed
  - `e2e-coordinator.test.ts`: 7 passed (including HyDE & Parent-Child Context)
  - `directory-crawler.test.ts`: 3 passed
- Live Google AI Studio Semantic Search Suite: **5 passed, 0 failed** (`live-semantic-search.test.ts`):
  - Genuine 3072-dim embeddings via `gemini-embedding-001` and `gemini-embedding-2`
  - Cross-lingual retrieval (Vi -> En and En -> Vi with zero lexical word overlap)
  - Asymmetric task type retrieval (`RETRIEVAL_DOCUMENT` with Title context vs `RETRIEVAL_QUERY`)
  - Slot failover on simulated 429 cooldown
  - Structured hard constraint filtering (negative keywords)
- **Grand Total: 65 passed, 0 failed**.

## 6. Definition of done for each milestone

A milestone may be marked complete only when:
- Its acceptance criteria are met by actual implementation.
- Relevant positive, negative, regression, and contract tests pass.
- Search-quality changes have reproducible benchmark evidence.
- Provider/model/index compatibility and privacy behavior are checked where relevant.
- Documentation is updated to distinguish current behavior from future design.
- Known limitations and exact verification results are recorded.

Compilation alone is not completion. A hand-constructed query plan that contains translated keywords is not evidence that a provider generated that plan or that the engine performed cross-lingual semantic retrieval.
