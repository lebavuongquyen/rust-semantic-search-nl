# SemanticSearchAgent — Full Repository Audit

**Audit date:** 2026-10-12  
**Repository:** `E:\10_Learning\Rust\SemanticSearchAgent`  
**Scope:** Rust source and tests, TypeScript AI integration, architecture and roadmap documents, build/test status, security-sensitive code paths.  
**Mode:** Read-only code audit; no production source files changed.

## Executive verdict

**Not ready to claim production-grade, provider-agnostic, genuine cross-lingual semantic search.** The repository now contains a meaningful search architecture skeleton and several working prototype components, but important runtime correctness, security, semantic quality, lifecycle, and documentation consistency gaps remain.

The test suites pass, but tests currently validate selected in-memory/test-double flows and hand-authored examples. They do not prove real model-generated embeddings, general Vietnamese-English semantic retrieval, production access control, durable indexing, or 10,000-file performance.

## Verification evidence

| Check | Result | Notes |
|---|---|---|
| `cargo test` | PASS | 21 Rust tests passed, 0 failed |
| `cargo test --all-features` | PASS | 21 Rust tests passed, 0 failed |
| `cargo check --target wasm32-unknown-unknown` | PASS | Compile check only; no browser/WASM runtime test |
| TypeScript provider-contract test runner (`npx --yes tsx ts/tests/provider-contract.test.ts`) | PASS | 5 tests passed, 0 failed |
| TypeScript static type check (`tsc --noEmit ...`) | PASS | The four TypeScript source/test files checked |
| `cargo fmt --all -- --check` | NOT RUN SUCCESSFULLY | `rustfmt` component is not installed in the active toolchain |
| `cargo clippy --all-targets --all-features -- -D warnings` | NOT RUN SUCCESSFULLY | `clippy` component is not installed in the active toolchain |
| Live Google AI Studio request | NOT TESTED | No real credential/network acceptance test performed |
| 10,000-document benchmark | NOT TESTED | No reproducible large-corpus performance evidence |
| Browser/WASM end-to-end | NOT TESTED | No generated WASM package/runtime smoke test |

Repository inventory at audit time: 20 Rust source files, 8 Rust integration-test files, 4 TypeScript files, and 12 Markdown files. Git reports no commits on `master` and zero tracked files; the repository content is all untracked.

## Findings

### P1 — Tenant authorization fails open when tenant metadata is absent

**Location:** `src/security/mod.rs`, `AccessControlPolicy::filter_authorized_results`.

The filter rejects a result only when `tenant_id` exists in provenance and differs from the active tenant. If `tenant_id` is absent, the result is allowed. `user_roles` is currently not consulted. This is fail-open behavior and is not sufficient for a production multi-tenant security boundary.

**Required correction:** deny results with missing tenant identity by default; authorize roles/permissions against explicit document policy; apply authorization before results can be observed; add negative tests for missing tenant, mismatched tenant, missing roles, and unauthorized roles.

### P1 — Document deletion does not fully propagate through storage and vector index

**Location:** `src/application/orchestrator.rs`, `delete_document`.

Deletion removes the document-to-chunk map entry and attempts lexical chunk removal, but ignores removal errors and does not remove vector entries or stored documents/chunks through `DocumentStore`. A later vector search can therefore return stale chunk IDs, and the API can report success while residual data remains.

**Required correction:** define deletion semantics across store, lexical index, vector index, and mapping; propagate failures or implement a recoverable transaction/outbox; add regression tests proving deleted content is unreachable through every retrieval path.

### P1 — Ingestion has unsafe chunk boundaries and can partially mutate indexes

**Location:** `src/application/orchestrator.rs`, `ingest_document`.

- `chunk_size == 0` causes the loop to make no progress and can hang indefinitely for non-empty content.
- Chunks use byte offsets directly in Rust string slicing (`content[start..end]`). With UTF-8 text, a chunk boundary can fall inside a multibyte character and panic. This is especially relevant for Vietnamese text.
- The document is stored before indexing finishes; if embedding, vector indexing, or later storage fails, earlier chunks/indexes can remain partially written.
- Re-ingesting a document does not first reconcile prior chunks, so old chunk IDs may remain indexed when content/chunk counts change.

**Required correction:** reject zero chunk sizes; chunk on valid UTF-8 boundaries or character-aware offsets; make ingestion transactional or explicitly recoverable; implement replace/reindex semantics and fault-injection tests.

### P1 — The current multilingual embedder is a hardcoded concept encoder, not a general semantic embedding model

**Location:** `src/adapters/semantic_embedder.rs`.

`MultilingualSemanticEmbedder` manually maps a small set of finance, receipt, contract, authentication, documentation, and August terms into fixed vector dimensions. It also hashes bytes into remaining dimensions. The implementation hardcodes vocabulary, concepts, and a particular month. It does not invoke a trained embedding model or a provider adapter.

This can pass curated Vietnamese/English tests because the same hand-selected signals are encoded in the test corpus. It cannot substantiate a general semantic-search claim and directly conflicts with the project's no-hardcoded-domain-knowledge requirement.

**Required correction:** keep the vector interface provider-neutral, but generate vectors through a real configurable embedding provider/model or a genuinely general local model. Keep deterministic fakes strictly in test code. Evaluate on held-out cross-lingual queries, ambiguous terms, unseen domains, and hard negatives.

### P1 — Hard constraints are incompletely enforced

**Location:** `src/application/orchestrator.rs`, `passes_filter`.

The filter currently evaluates negative keywords, file extensions, and path prefix. `QueryFilter` also exposes `required_tags`, `temporal_year`, and `temporal_month`, but those fields are not enforced here. A caller can supply these constraints and still receive results that violate them.

**Required correction:** implement every declared hard constraint or reject unsupported filters explicitly. Add one positive and multiple negative tests per filter, including missing metadata and case/path normalization.

### P1 — Embedding-space identity compatibility ignores language capability metadata

**Location:** `src/domain/mod.rs`, `EmbeddingSpace::is_compatible_with`.

Compatibility compares provider, model, revision, dimensions, distance metric, and normalization, but ignores `supported_languages`. More importantly, the current compatibility contract does not itself prove that the vectors were actually generated by the declared model/revision. Metadata consistency is necessary but cannot substitute for provider-backed embedding provenance.

**Required correction:** define compatibility semantics explicitly; ensure actual adapter identity and model revision are authoritative; validate vector finiteness and dimensions; persist identity with durable index metadata when persistence is introduced.

### P1 — Provider-neutral AI contract is not yet proven end-to-end

**Location:** `ts/ai-provider-contract.ts`, `ts/google-ai-studio-provider.ts`, `ts/google-ai-studio-orchestrator.ts`.

The provider registry/router and Google adapter are a useful foundation, and TypeScript type-checks. However, current tests focus on registry/orchestrator behavior using stubs. The Google adapter's live HTTP/error/timeout behavior is not covered by a dedicated mocked-fetch contract suite in the test file audited. The older Google-specific orchestrator still owns provider-specific model/prompt assumptions and is not fully replaced by the neutral routing layer. There is only one real provider adapter in this repository.

**Required correction:** test the adapter against mocked fetch responses for success, malformed payload, HTTP 400/401/403/408/429/5xx, network failure, timeout/abort, redaction, and cancellation. Validate generated plan JSON against a runtime schema. Route orchestration through provider-neutral contracts and prove portability with a second independently implemented adapter or compatible endpoint adapter.

### P1 — Claimed cross-encoder reranker is heuristic logic

**Location:** `src/adapters/reranker.rs`.

`CrossEncoderReranker` uses title substring matching, whitespace token overlap, and a hardcoded `draft` penalty. It does not run a cross-encoder model. The class name overstates the implementation and the fixed domain-specific penalty conflicts with configurable, provider-neutral behavior.

**Required correction:** rename it to reflect lexical heuristic reranking, make policy configurable, or implement an actual reranker adapter. Evaluate its effect against a baseline using held-out judgments and nDCG/MRR.

### P1 — Multilingual text and query rules contain apparent mojibake

**Locations:** `src/lib.rs`, `src/adapters/semantic_embedder.rs`, `src/query/mod.rs`, `src/thesaurus/mod.rs`, `src/agent/mod.rs`, and parts of the roadmap.

UTF-8 inspection found suspicious mojibake sequences in source literals and comments, including Vietnamese concept strings and conjunction/negation cues. This is not merely cosmetic where the text appears inside matching rules: it can cause correctly encoded Vietnamese user input to fail to match the intended patterns. Tests using the same corrupted literals can pass while real user input fails.

**Required correction:** inspect and repair actual UTF-8 source literals; add tests with canonical Vietnamese strings copied from real UTF-8 inputs; add a source hygiene check to prevent recurrence. Avoid replacing strings based solely on terminal rendering—verify the file bytes and behavior.

### P2 — BM25 implementation needs lifecycle and correctness hardening

**Location:** `src/adapters/lexical_bm25.rs`.

The adapter implements an inverted index and BM25-style scoring, but tokenization is basic lowercasing and splitting; there is no language-aware analysis or configurable analyzer port. Removing a chunk scans every posting list and leaves empty posting maps. Constructor parameters are not validated (`k1`, `b`), and large token/document counts are narrowed to `u32`.

**Required correction:** validate configuration; define Unicode/tokenization behavior; clean empty postings; test update/remove/reinsert and empty documents; benchmark index/update/query costs at representative corpus sizes.

### P2 — Retrieval can do unnecessary or unbounded work for edge values

**Location:** `src/application/orchestrator.rs`, `search`.

Candidate counts use `query.top_k * 3` without checked/saturating arithmetic and the reranker has an independently configured candidate cap. `top_k == 0` and extreme values need explicit, consistent semantics. A brute-force in-memory vector index is O(N) per query and is a prototype, not evidence of scalable approximate nearest-neighbor retrieval.

**Required correction:** validate query limits and use bounded configuration; benchmark candidate-generation, fusion, filtering, and reranking separately; select a vector-index implementation through a port when scale requires it.

### P2 — Query intent validation is cue-based and does not prove semantic constraint extraction

**Location:** `src/domain/intent.rs` and `src/agent/mod.rs`.

Negation validation relies on a small fixed list of Vietnamese/English strings, and the controlled agent decomposes using hardcoded conjunctions. This is a guardrail, not broad multilingual natural-language understanding. It can miss paraphrases and languages outside the list.

**Required correction:** separate mandatory structured constraints from soft query rewrites; use a validated planner output schema; reject ambiguous/unsupported mandatory constraints rather than silently dropping them; add multilingual paraphrase and adversarial tests.

### P2 — Audit documentation is internally inconsistent and stale

**Locations:** `docs/M00_REPOSITORY_AUDIT_AND_BASELINE.md`, `docs/SEMANTIC_SEARCH_AGENT_ROADMAP.md`, `AGENTS.md`, `ARCHITECTURE_PLAN.md`, `ROADMAP.md`.

- M00 declares itself “Completed & Reconciled,” while the roadmap says M00 remains in progress.
- `AGENTS.md` describes a four-test baseline and says there is no ingestion/vector pipeline, while the current tree contains application orchestration, BM25/vector adapters, and 21 passing Rust tests. Those components still have serious limitations, but the inventory/status wording is stale.
- The roadmap's repository reality section also reports 4 Rust tests and states that the new provider contract has no contract tests, while the current suite now has 21 Rust tests and 5 TypeScript tests. The TS tests do not yet cover the real Google adapter comprehensively, so the precise gap should be stated rather than the entire suite being described as absent.
- The repository is entirely untracked with no initial commit, making changes hard to review or safely compare.

**Required correction:** reconcile all status documents against this audit; distinguish implemented prototype, tested contract, production acceptance, and not implemented; record test commands/results; preserve the no-hardcoded-provider/domain-knowledge requirements. Establish a reviewed baseline commit only after the owner approves the current files and intended repository contents.

### P2 — Formatting and lint quality gates are currently unavailable

`cargo fmt --check` and Clippy could not run because `rustfmt` and `clippy` are absent from the active Rust toolchain. The failure is environmental, not a source diagnostic. Until those components are installed and checks pass, formatting and lint status must remain unverified.

## Positive findings

- Rust domain types and ports create a reasonable starting point for provider-neutral architecture.
- BM25 and in-memory vector-index prototypes exist, rather than only the original weighted substring scorer.
- Embedding-space metadata and migration rollback intent are represented in code.
- Query-intent invariants, bounded agent iterations, result provenance, and basic secret redaction have tests.
- Rust native tests, all-features tests, WASM target compilation, TypeScript static checking, and the current TypeScript test runner all pass.

## Recommended remediation order

1. Fix fail-open tenant authorization and deletion consistency.
2. Fix ingestion zero-size, UTF-8 boundary, partial-write, and re-ingestion behavior.
3. Repair corrupted Vietnamese source literals and add real-input regression tests.
4. Replace the hardcoded concept embedder with a genuine configurable embedding adapter; retain fakes only for tests.
5. Enforce every declared hard filter and validate top-k/configuration boundaries.
6. Complete provider-adapter negative tests, schema validation, timeout/cancellation behavior, and provider-neutral orchestration.
7. Rename or replace heuristic reranking; validate BM25/vector compatibility and index lifecycle.
8. Reconcile roadmap/architecture/agent docs with current source and acceptance evidence; then install and run rustfmt/Clippy.
9. Add held-out retrieval-quality evaluation, live-provider opt-in tests, browser/WASM integration tests, and reproducible scale benchmarks before claiming production readiness.

## Final assessment

The repository has progressed beyond the earlier four-test skeleton, but the core product requirement—general, configurable, cross-lingual semantic retrieval without hardcoded domain vocabularies—is not yet met. The current result is best described as a provider-neutral search prototype with BM25 and in-memory vector-index scaffolding, a hardcoded concept-vector prototype, and an initial Google AI Studio integration. Production acceptance should remain blocked until the P1 findings are addressed and demonstrated with regression evidence.
