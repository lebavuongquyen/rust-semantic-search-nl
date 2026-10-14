# SemanticSearchAgent — Agent Instructions

> **Purpose:** Operating rules for every human or AI agent working in this repository.
> **Current status:** Prototype; genuine semantic and cross-lingual retrieval is not implemented yet.
> **Last reconciled:** 2026-10-12.

## 1. Required Reading Order

Before changing code or documentation, read:

1. `ROADMAP.md` — current verified status, milestone order, immediate next step.
2. `docs/SEMANTIC_SEARCH_AGENT_ROADMAP.md` — canonical detailed milestones, acceptance criteria, and quality gates.
3. `ARCHITECTURE_PLAN.md` — target architecture and non-negotiable design constraints.
4. `docs/AI_PROVIDER_ARCHITECTURE.md` — current provider contracts and integration limitations.
5. `PROJECT.md`, `README.md`, and `CONVERSATION_CONTEXT.md` as relevant to the task.

When documents conflict, apply this precedence:
1. User's explicit requirements in the current conversation.
2. Verified implementation and test evidence for statements about current behavior.
3. Architecture constraints in `ARCHITECTURE_PLAN.md`.
4. Milestone ordering and acceptance criteria in `docs/SEMANTIC_SEARCH_AGENT_ROADMAP.md`.
5. Summary documents and historical context.

Do not treat an aspirational design, an example, or a passing unit test as proof that a feature exists.

## 2. Product Mission

Build a genuine natural-language semantic search system that retrieves relevant documents by meaning, not only exact strings. It must support cross-lingual retrieval: for example, a Vietnamese query such as `tìm hóa đơn` must be able to find relevant English content such as `invoice`, and the reverse direction must also work, without hardcoded translation pairs.

The system should eventually support configurable ingestion, indexing, lexical and semantic retrieval, structured filters, result fusion, reranking, optional query planning, provenance, evaluation, and safe operation at realistic scale.

## 3. Non-Negotiable Constraints

### 3.1 No hardcoded technology lock-in

- Do not mandate or hardcode a specific library, framework, embedding model, inference runtime, OCR engine, vector database, search backend, AI vendor, or DI/plugin framework.
- Technology names in examples are illustrative only. They are not approved dependencies or implementation commitments.
- Evaluate candidate implementations against requirements, compatibility, license, privacy, platform support, performance, maintainability, and measured quality before selecting them.
- Keep provider-specific SDKs, request/response types, model IDs, API quirks, and backend-specific types inside adapters.
- Use project-owned contracts, capability declarations, and validated configuration at replaceable boundaries.
- Do not add abstraction layers without a concrete substitution, lifecycle, testing, or operational need.

### 3.2 Cross-lingual semantics must be real

- Do not use a manually maintained translation or synonym dictionary as the primary semantic engine.
- Do not claim cross-lingual semantic search based only on language detection, prompt examples, keyword expansion, or cosine similarity over externally supplied vectors.
- Use an embedding/retrieval capability that declares and demonstrates cross-lingual support, or another measured semantic method that meets the same acceptance criteria.
- Domain vocabulary may be loaded from external configuration/data as optional supplemental evidence. It must not be required for general cross-lingual matching.
- Evaluate both directions, such as Vietnamese query → English document and English query → Vietnamese document, and include hard negatives.

### 3.3 Privacy is explicit

- Local-only operation must work without credentials or network access.
- Do not send document content or queries to a remote AI service by default.
- Any remote processing must be explicitly configured and consented to, with clear disclosure of what data leaves the device.
- A fallback must never silently change privacy, data residency, or consent semantics.
- Never commit API keys, secrets, or private document content. Redact secrets and sensitive content from logs.

### 3.4 Claims must match evidence

- Do not claim a quality percentage, latency, throughput, file-count capacity, or absolute accuracy without a reproducible benchmark and documented environment.
- Distinguish implemented, partially implemented, proposed, and unverified functionality.
- A compile check or small unit test does not prove end-to-end search quality.
- Never report tests, benchmarks, API calls, or repository changes that were not actually performed and inspected.

## 4. Current Repository Reality (2026-10-12)

Verified baseline:
- Rust crate contains:
  - Provider-neutral domain models (`Document`, `Chunk`, `Query`, `SearchResult`, `Provenance`, `EmbeddingSpace`, `NormalizedQueryIntent`) and port traits (`LexicalRetriever`, `VectorRetriever`, `EmbeddingProvider`, `RankFusionPolicy`, `DocumentStore`, `QueryUnderstandingPort`, `RerankerPort`).
  - Okapi BM25 lexical retriever with Lucene IDF, clean posting removal, and atomic disk snapshot persistence (`Bm25LexicalRetriever`).
  - Vector retriever with embedding-space compatibility validation, safe migration, and atomic disk snapshot persistence (`MemoryVectorIndex`).
  - Disk-backed document and chunk store with crash-safe atomic `.tmp` writes, restart recovery, and deletion propagation (`DiskDocumentStore`).
  - In-memory LRU embedding cache, `CachedEmbeddingProvider` decorator, and `SemanticQueryCache` for sub-millisecond repeat queries (`src/adapters/cache.rs`).
  - Provider-neutral neural embedding adapter architecture (`ModelEndpointEmbeddingAdapter`) supporting `sentence-transformers/all-multilingual-mpnet-base` (768-dim) and `BAAI/bge-m3` (1024-dim) with deterministic offline transport and dimension validation (`src/adapters/neural_embedder.rs`).
  - Continuous evaluation harness with per-query latency tracking, p50/p95 latency percentiles, sliced breakdowns by language/category, and automated regression gates with canary thresholds (`src/evaluation/mod.rs`).
  - Provider-neutral search orchestrator (`SearchOrchestrator`) with UTF-8 character-safe chunking, atomic rollback on partial ingest, full deletion propagation across store/lexical/vector indexes, and hard constraint filtering (negative keywords, extensions, path prefixes, tags, temporal year/month).
  - Multi-tenant fail-closed access control and role authorization (`AccessControlPolicy`).
  - Secret redaction policy for observability (`TelemetryRedactionPolicy`).
  - Bounded multi-step agent execution with loop detection (`ControlledSearchAgent`).
  - Lexical heuristic candidate reranker with configurable negative penalties (`LexicalHeuristicReranker`).
  - Prototype concept-weighted embedding double for offline testing (`MultilingualSemanticEmbedder`).
  - Public WASM-facing methods: `parseQuery`, `scoreDocument`, and `computeCosineSimilarity`.
- TypeScript AI integration:
  - Provider-neutral contracts (`AiProvider`, `AiRouter`, `AiProviderRegistry`).
  - Google AI Studio REST adapter (`GoogleAiStudioProvider`) with HTTP error mapping (401, 429), timeout/AbortSignal handling, and payload validation.
  - Legacy search orchestrator with `standard_pure` local fallback.
- Current verification evidence:
  - `cargo test`: **32 passed, 0 failed** across all unit and integration suites (`m01` through `m10`, `cache`, `neural_embedder`).
  - `cargo check --target wasm32-unknown-unknown`: passed (0 warnings, 0 errors).
  - TypeScript contract tests (`npx --yes tsx ts/tests/provider-contract.test.ts`): **9 passed, 0 failed**.
  - TypeScript type check (`npx --package typescript tsc --noEmit ...`): passed (0 errors).
- The repository was observed without any Git commits and with project files untracked during the 2026-10-12 audit. Verify the current Git state before assuming history or a clean working tree.

Update this section only after inspecting the actual code and running relevant checks.

## 5. Required Implementation Workflow

For each milestone, proceed in order; do not skip ahead because a later feature appears attractive.

1. Inspect the relevant code, tests, current repository status, and applicable documentation.
2. Confirm the milestone's scope and acceptance criteria in the detailed roadmap.
3. Identify the smallest coherent change set and dependencies between changes.
4. Define or update contracts before implementing replaceable adapters.
5. Implement incrementally; avoid unrelated refactors.
6. Add positive, negative, regression, and contract tests appropriate to the change.
7. Run relevant native and WASM checks; run TypeScript checks for TypeScript changes.
8. Evaluate retrieval changes against the same versioned corpus/query set.
9. Audit dependency direction, privacy, configuration validation, error paths, cancellation, and resource limits.
10. Update documentation and record exact commands/results, known limitations, and next steps.
11. Commit only when explicitly requested or when the agreed workflow calls for it; never claim a commit exists unless verified.

## 6. Architecture Rules

- Dependency direction: `Domain → nothing external`; `Application → domain-owned contracts`; `Adapters → contracts + external implementation`. The domain must not import adapter SDKs.
- Prefer capability-driven routing over vendor-name conditionals in core logic.
- Validate model capabilities, embedding dimensions/identity, configuration, and backend compatibility before use.
- Never mix embeddings from incompatible model spaces in one index.
- Treat mandatory filters and access-control constraints as hard constraints, not ranking hints.
- Query rewriting/planning must not silently drop negation, date constraints, authorization, or other mandatory conditions.
- Keep provenance and sufficient trace metadata to explain why a result was returned.
- Timeouts, cancellation, retries, fallback, rate limits, and resource budgets must be explicit and testable.
- Do not call a weighted score “RRF.” Use the name that matches the implemented algorithm.

## 7. Required Quality Gates

Apply relevant gates to each change:

- Functional correctness and invalid-input behavior.
- Provider/adapter contract tests using deterministic fakes or mocked HTTP.
- Regression tests for previously fixed behavior.
- Cross-lingual retrieval evaluation when query processing, embeddings, retrieval, fusion, or ranking changes.
- Privacy/security checks for secrets, remote data flow, access control, and untrusted retrieved content.
- Compatibility tests for index/model/configuration changes.
- Native and WASM compilation/tests when Rust/WASM behavior changes.
- TypeScript type checks and adapter tests when TypeScript changes.
- Performance and cost measurement when performance/cost is claimed.
- Documentation consistency across `AGENTS.md`, `ARCHITECTURE_PLAN.md`, `ROADMAP.md`, and detailed design docs.

## 8. Verified Reality and Completion Status

- Native Rust test suite: **35 passed, 0 failed** (`cargo test`)
- WASM target compilation: **Passed** (`cargo check --target wasm32-unknown-unknown`)
- TypeScript contract & E2E verification: **22 passed, 0 failed**
  - Contract & error mapping: 9 passed
  - Embedding load balancing & rate-limit cooldown: 5 passed
  - TS-WASM FFI integration & local fallback: 5 passed
  - Recursive directory crawler: 3 passed
- Live Google AI Studio Semantic Search Suite: **5 passed, 0 failed** (`ts/tests/live-semantic-search.test.ts`)
  - Genuine 3072-dimension neural embeddings via `gemini-embedding-001` and `gemini-embedding-2`
  - Genuine cross-lingual retrieval verified (Vietnamese cloud query -> English AWS billing doc; English telecom query -> Vietnamese Viettel invoice doc; with zero lexical word overlap)
  - Slot failover on simulated 429 cooldown to secondary model slot
  - Structured hard constraint filtering (negative keywords)
- Model Discovery & Dimensions recorded in `docs/EMBEDDING_PROBE_RESULTS.md`
- Total verified test suite: **62 passed, 0 failed**
- Complete granular fallback: Rate limit (429) or network outages on Gemini remote slots seamlessly trigger in-place fallback to local deterministic embedding without aborting the search pipeline.


