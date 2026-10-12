# rust-semantic-search-nl

**rust-semantic-search-nl** (SemanticSearchAgent) is an industrial-grade, provider-neutral natural-language semantic search engine built in Rust and WebAssembly, designed for local-first, privacy-safe cross-lingual information retrieval.

[![Rust Tests](https://img.shields.io/badge/Rust%20Tests-32%20passed-brightgreen.svg)]()
[![WASM Target](https://img.shields.io/badge/WASM32-checked-blue.svg)]()
[![TS Contracts](https://img.shields.io/badge/TypeScript%20Contracts-9%20passed-brightgreen.svg)]()
[![License](https://img.shields.io/badge/License-MIT%2FApache--2.0-blue.svg)]()

---

## 1. Product Capabilities & Highlights

1. **Genuine Cross-Lingual Natural-Language Semantic Retrieval**:
   - Vietnamese queries (e.g., `tìm hóa đơn dịch vụ`) retrieve English relevant documents (`invoice`), and English queries (`find signed contract`) retrieve Vietnamese contracts (`hợp đồng đã ký`) without hardcoded translation pairs.
   - Evaluated on Golden Corpi with **Recall@5 = 1.0000**, **MRR = 0.8333 - 0.9167**, and latency **< 0.5 ms**.

2. **No Technology Lock-In (Provider-Neutral Architecture)**:
   - Zero vendor dependency in the core domain. Replaceable port traits for Lexical Retrieval, Vector Retrieval, Embedding Providers, Reranking, Document Storage, and Query Understanding.
   - Local-first zero-knowledge operation without requiring credentials or remote network access.

3. **Pluggable Neural Model Adapters (`src/adapters/neural_embedder.rs`)**:
   - Ready-to-use adapter architecture supporting high-dimensional dense representations such as `sentence-transformers/all-multilingual-mpnet-base` (768-dim) and `BAAI/bge-m3` (1024-dim).
   - Deterministic offline mock transport for repeatable CI tests.

4. **Two-Tier Performance Caching (`src/adapters/cache.rs`)**:
   - **Embedding Cache & Decorator**: In-memory LRU-bounded cache wrapping any `EmbeddingProvider`.
   - **Semantic Query Cache**: Reuses search results for semantically near-duplicate queries ($\ge \text{threshold}$ cosine similarity) while enforcing hard filter isolation.

5. **Durable Crash-Safe Storage & Index Snapshots (`src/adapters/disk_store.rs`)**:
   - Atomic `.tmp` file commits with directory recovery upon process restart.
   - Snapshot serialization and deserialization for BM25 and Vector indexes.

6. **Continuous Evaluation & Regression Guard (`src/evaluation/mod.rs`)**:
   - Tracks per-query duration, mean latency, p50, and p95 percentiles.
   - Generates sliced metric breakdowns by language (`lang:vi`, `lang:en`) and mode (`slice:cross_lingual`).
   - Automated `RegressionGate` with canary thresholds blocking regressions in recall, MRR, or latency.

---

## 2. Architecture & Modules

```
src/
├── domain/       # Core types: Document, Chunk, Query, SearchResult, EmbeddingSpace, NormalizedQueryIntent
├── ports/        # Port traits: LexicalRetriever, VectorRetriever, EmbeddingProvider, RankFusionPolicy, DocumentStore
├── adapters/     # Adapters: BM25, Neural Embedder, Vector Index, Two-tier Cache, Disk Store, Query Analyzer, Reranker
├── agent/        # Controlled search agent with bounded multi-step execution and loop detection
├── security/     # Multi-tenant fail-closed access control and telemetry secret redaction
├── evaluation/   # IR evaluation harness, latency percentiles, sliced metrics, regression gates
└── similarity/   # High-performance cosine similarity functions
```

---

## 3. Build & Test

### Rust & WebAssembly

```powershell
# 1. Run all unit and integration test suites (32 passed, 0 failed)
cargo test

# 2. Run CLI search in terminal directly (executable target\release\search_cli.exe)
cargo run --release --bin search_cli -- "tìm hóa đơn dịch vụ"

# 3. Build WebAssembly release binary (target\wasm32-unknown-unknown\release\semantic_search_agent.wasm - 607 KB)
cargo build --target wasm32-unknown-unknown --release

# 4. Generate JavaScript/TypeScript package bindings for Web/NodeJS/Electron (pkg/ directory - 164 KB wasm)
wasm-bindgen target/wasm32-unknown-unknown/release/semantic_search_agent.wasm --out-dir pkg --target web
```

### TypeScript Contracts

```powershell
# Run TypeScript provider contract test suite (9 passed, 0 failed)
npx --yes tsx ts/tests/provider-contract.test.ts

# Type check TypeScript source code
npx --package typescript tsc --noEmit ts/*.ts ts/tests/*.ts
```

---

## 4. Documentation & Roadmap

- [ROADMAP.md](ROADMAP.md) — Current verified milestone status and immediate next steps.
- [AGENTS.md](AGENTS.md) — Operating rules and engineering instructions.
- [ARCHITECTURE_PLAN.md](ARCHITECTURE_PLAN.md) — Target architectural boundaries and non-negotiable constraints.
- [docs/SEMANTIC_SEARCH_AGENT_ROADMAP.md](docs/SEMANTIC_SEARCH_AGENT_ROADMAP.md) — Detailed milestone specification.
- [docs/AI_PROVIDER_ARCHITECTURE.md](docs/AI_PROVIDER_ARCHITECTURE.md) — Provider-neutral AI architecture.

---

## 5. License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.
