# Changelog

All notable changes to the SemanticSearchAgent project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-10-14

### Added
- **Genuine Neural Multilingual Embedding**: Discovered and integrated native 3072-dimensional Google AI Studio models (`gemini-embedding-001` and `gemini-embedding-2`) with proven cross-lingual semantic similarity (Vietnamese $\leftrightarrow$ English).
- **TypeScript Embedding Load Balancer**: Implemented `EmbeddingLoadBalancer` supporting both `priority` (ensuring consistent index embedding spaces with cooldown failover) and `round_robin` strategies, rate-limit 429 cooldowns, and seamless step-level fallback to local offline embedder without aborting pipelines.
- **Rust WebAssembly Search Core**: Exposed 5 high-performance WASM bindings on `SemanticSearchAgent`: `configureEmbeddingSpace`, `ingestDocumentWithVectors`, `searchWithVector`, `deleteDocument`, and `getIndexStats`.
- **TypeScript-WASM Bridge**: Implemented `SemanticSearchCoordinator` with smart paragraph and sentence boundary chunking, zero-copy Float32Array vector marshalling, and search provenance annotation.
- **Automated Directory Crawler**: Implemented `DirectoryCrawler` for recursive filesystem scanning, filtering, and automated batch ingestion into the WASM hybrid index.
- **Live Real-API Evaluation Suite**: Added `ts/tests/live-semantic-search.test.ts` verifying real 3072-dimensional embeddings, cross-lingual retrieval with zero keyword overlap, 429 failover, and structured constraint filtering.
- **API Model Prober**: Created `ts/scripts/probe-embedding-dims.ts` and recorded benchmark findings in `docs/EMBEDDING_PROBE_RESULTS.md`.

### Fixed
- Fixed binary PDF corruption in `DirectoryCrawler` by implementing `extractPdfText` stream text run parser.
- Fixed partial filter deserialization failure in Rust by adding `#[serde(default)]` to `QueryFilter`.
- Fixed document metadata and file path loss across the TypeScript $\rightarrow$ WebAssembly FFI boundary.
- Fixed `getIndexStats` JSON serialization in WASM by introducing a typed `IndexStats` struct.
- Resolved model space mismatch by prioritizing primary model slots and isolating fallback transitions.

### Changed
- Bumped crate and package versions to `0.2.0`.
- Updated test verification suite to 62 passing tests (35 Rust native + 22 TypeScript contracts/integration + 5 Live Google AI Studio tests).

## [0.1.0] - 2026-10-12

### Added
- Initial project release with Okapi BM25 lexical retriever, linear vector index, RRF rank fusion, and bounded agent orchestrator.
