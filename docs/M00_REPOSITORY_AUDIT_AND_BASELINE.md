# M00: Repository Audit and Architecture Baseline Report

**Execution Date:** 2026-10-12  
**Auditor:** Pair Programmer Agent  
**Status:** Completed & Reconciled

---

## 1. Verified Repository State

### 1.1 Git & Working Tree
- **Branch:** `master` (initial state, no commits yet).
- **Tracked Files:** Untracked repository baseline files (`Cargo.toml`, `src/`, `ts/`, `docs/`, documentation).
- **Hygiene Requirement:** Changes must be verified against real compiler checks (`cargo test`, `cargo check --target wasm32-unknown-unknown`, TypeScript typechecks) before marking any milestone complete.

### 1.2 Toolchain & Targets
- **Rust Toolchain:** `rustc 1.99.0`, `cargo 1.99.0`
- **Node.js & Package Manager:** `node v24.12.0`, `pnpm 12.6.0`, `typescript 7.0.2` (via pnpm dlx)
- **Target Targets Verified:**
  - Native x86_64: `cargo test` passed (4 passed, 0 failed).
  - WebAssembly: `cargo check --target wasm32-unknown-unknown` passed.
  - TypeScript: `pnpm dlx typescript --noEmit ts/*.ts` passed (0 errors).

---

## 2. Source Code & Coupling Inventory

| Module / File | Current Responsibility | Identified Architecture Risks & Hardcoded Assumptions |
|---|---|---|
| `src/lib.rs` | WASM entry point (`parseQuery`, `scoreDocument`, `computeCosineSimilarity`) | Directly glues hardcoded thesaurus and weighted scorer without provider-neutral domain models (`Document`, `Chunk`, `Provenance`). |
| `src/query/mod.rs` | Rule-based query normalization, temporal extraction, code pattern heuristics | Hardcoded typo replacements (e.g. `"biên lại"` -> `"biên lai"`). Hardcoded temporal month 8 string (`"2608"`). Lacks query-intent schema validation and negation handling. |
| `src/thesaurus/mod.rs` | Concept catalogue (`ConceptRegistry`) | Hardcoded compile-time thesaurus for 5 concepts (Receipt, Auth, Contract, Salary, Error). Violates rule against hardcoded translation dictionaries for cross-lingual semantics. |
| `src/fusion/mod.rs` | Document scoring (`HybridScorer`) | Arbitrary fixed weights (`350.0` for filename, `200.0` for content, `250.0` for temporal). Not BM25, not Reciprocal Rank Fusion (RRF). Lacks normalization and score calibration. |
| `src/similarity/mod.rs` | Cosine similarity calculation | Mathematical primitive only. Lacks vector space identity checking (dimensions, model, normalization). |
| `ts/ai-provider-contract.ts` | TypeScript provider-neutral contracts | Clean contract foundation for text generation and embeddings. Needs contract tests and mock provider. |
| `ts/google-ai-studio-provider.ts` | REST adapter for Google AI Studio | Clean fetch-based adapter. Error mapping implemented. Needs unit tests with mocked fetch. |
| `ts/google-ai-studio-orchestrator.ts` | Legacy query planner | Hardcoded Google model list and hardcoded prompt examples. Needs migration to `AiRouter`. |

---

## 3. Architecture Violations Identified & Remediation Plan

1. **Hardcoded Translation & Thesaurus in Core Engine**:
   - *Violation:* `src/thesaurus/mod.rs` hardcodes Vietnamese-to-English synonyms in code.
   - *Remediation (M01 & M03):* Make domain concepts an optional external data source. Semantic cross-lingual retrieval must be driven by vector embeddings in a shared semantic space or declared multilingual models, not fixed code constants.
2. **Missing Core Domain Models**:
   - *Violation:* Search operates directly on raw string slices `(&str, &str)` instead of structured `Document`, `Chunk`, `Query`, `SearchResult`, and `Provenance` domain types.
   - *Remediation (M01):* Establish `domain` module with first-class data models and traits.
3. **No Evaluation Harness or Golden Dataset**:
   - *Violation:* Search quality is tested against single ad-hoc unit tests without standard information retrieval metrics (Recall@K, MRR, nDCG).
   - *Remediation (M02):* Implement golden test corpus, query suite with relevance annotations, and repeatable benchmark runner.
4. **Uncalibrated Weighted Scorer Misleadingly Treated as Hybrid Search**:
   - *Violation:* Hand-crafted point additions (`+350`, `+200`, `+250`) without BM25 lexical foundations or rank-based fusion (RRF).
   - *Remediation (M02 & M04):* Implement BM25 lexical retriever port (M02) and genuine Reciprocal Rank Fusion (M04).

---

## 4. File-by-File Implementation Plan for M01 & Beyond

### Phase 1: M01 — Provider-Neutral Search Core
- **`src/domain/mod.rs`** (New): Define core domain types: `Document`, `DocumentId`, `Chunk`, `ChunkId`, `Query`, `QueryFilter`, `SearchResult`, `Score`, `ScoreComponent`, `Provenance`, `EmbeddingVector`, `EmbeddingSpace`.
- **`src/ports/mod.rs`** (New): Define provider-neutral trait interfaces:
  - `LexicalRetriever`: Index and retrieve documents/chunks using lexical match.
  - `VectorRetriever`: Search chunks using vector similarity with space compatibility checks.
  - `EmbeddingProvider`: Generate embeddings with declared model metadata.
  - `RankFusionPolicy`: Fuse multiple candidate lists deterministically (e.g. RRF).
- **`src/adapters/fakes.rs`** (New): Implement deterministic in-memory fakes for all ports (`FakeLexicalRetriever`, `FakeVectorRetriever`, `FakeEmbeddingProvider`).
- **`src/application/orchestrator.rs`** (New): Provider-neutral search coordinator coordinating retrieval, filtering, and provenance tracking.
- **Contract & Unit Tests:** Full test suite verifying that switching adapters does not modify domain rules and unsupported configurations fail fast.

### Phase 2: M02 — Evaluation Harness and Lexical Baseline
- **`tests/data/corpus.json`** & **`tests/data/queries.json`**: Golden multilingual corpus (Vietnamese & English documents) with annotated relevance judgments, including catalyst case (BIDV receipt) and coding agent cases.
- **`src/evaluation/mod.rs`**: Repeatable evaluation harness calculating Recall@K, Precision@K, MRR, and nDCG@K.
- **`src/adapters/lexical_bm25.rs`**: Standard BM25 tokenized lexical retriever satisfying the `LexicalRetriever` port.

### Phase 3: M03 — Semantic and Cross-Lingual Retrieval; Embedding Lifecycle
- Implement vector index with explicit space validation (`EmbeddingSpace`).
- Cross-lingual retrieval evaluation suite verifying Vietnamese -> English and English -> Vietnamese retrieval with hard negatives.

### Phase 4: M04 — Hybrid Retrieval and Structured Constraints
- Reciprocal Rank Fusion (RRF) implementation.
- Hard metadata filtering and content-temporal matching without dropping constraints.

### Phase 5: M12 / TypeScript Modernization
- Migrate `ts/google-ai-studio-orchestrator.ts` to use `AiRouter`.
- Add deterministic TypeScript test suite for provider contracts.
