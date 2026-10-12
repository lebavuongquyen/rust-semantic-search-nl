# SemanticSearchAgent — Roadmap to a Provider-Agnostic Natural-Language Semantic Search System

> **Status:** Proposed roadmap  
> **Project root:** `E:\10_Learning\Rust\SemanticSearchAgent`  
> **Purpose:** Define an implementation roadmap for a genuine natural-language semantic search system without hard-coding dependence on any specific library, framework, model, database, search engine, or AI provider.

## 1. Vision

Build **SemanticSearchAgent** as a search system that understands natural-language intent, retrieves semantically relevant information, respects explicit constraints, explains the basis of its results, and can evolve as technologies change.

The goal is not merely to add embeddings to keyword search. The system should support a measured progression from reliable retrieval foundations to semantic and hybrid retrieval, query understanding, reranking, and controlled agentic search.

### 1.1 Product goals

- Accept natural-language queries, including paraphrases, synonyms, and multi-condition requests.
- Retrieve relevant results based on meaning as well as exact terms and structured constraints.
- Preserve source provenance and make result generation traceable.
- Support Vietnamese and English, with language coverage measured explicitly.
- Allow technologies to be replaced through stable contracts and adapters.
- Evaluate search quality with repeatable datasets and metrics.
- Operate reliably under provider failures, data changes, restarts, and changing workloads.

### 1.2 Non-goals

- Choosing a single mandatory AI provider, embedding model, vector database, or application framework.
- Treating an LLM call or vector similarity score as proof of relevance.
- Adding an agent framework before the underlying search capabilities are reliable.
- Creating an abstraction for every class regardless of whether a real change boundary exists.
- Claiming quality targets without representative corpora and relevance judgments.

## 2. Non-Negotiable Architecture Principles

### 2.1 Provider-agnostic core

Domain and application logic must not import or depend directly on SDKs or implementation-specific types from external providers. Integrations belong in adapters behind project-owned contracts.

### 2.2 Capability-driven design

Represent required behaviors as explicit capabilities. Examples include:

- Text embedding
- Lexical retrieval
- Vector retrieval
- Metadata filtering
- Document ingestion and extraction
- Reranking
- Query understanding
- Language-model inference
- Result fusion
- Persistence and index lifecycle management

A provider may support only a subset of capabilities. The application must validate capability requirements rather than assume every implementation behaves identically.

### 2.3 Configuration over scattered constants

Provider identities, model identities, index settings, thresholds, timeouts, retry limits, and strategy selection belong in validated configuration or policy. Do not scatter provider-specific names, dimensions, thresholds, or assumptions throughout the codebase.

Configuration must not be used to conceal incompatible semantics. Validate that the selected components can work together.

### 2.4 Contract-first integration

For each replaceable boundary, define:

- Input and output types
- Error semantics
- Timeout and cancellation behavior
- Batch and pagination behavior, where applicable
- Consistency and lifecycle expectations
- Capability declarations
- Compatibility rules
- Observability requirements

Add an adapter only after the contract is clear enough to test.

### 2.5 Evaluation-first development

Maintain a repeatable evaluation harness early in development. Any change to retrieval, ranking, chunking, query processing, or provider selection must be evaluated against a known baseline.

### 2.6 Explicit ownership of core behavior

The project owns its domain types, orchestration policies, validation, evaluation, and compatibility rules. External libraries may implement capabilities, but they must not define the application's domain model.

### 2.7 Appropriate abstraction

Introduce interfaces where implementations can realistically vary, where isolation improves testing, or where a boundary has independent lifecycle or operational concerns. Avoid unnecessary abstraction layers and premature plugin complexity.

## 3. Target Architecture

The following is a logical architecture, not a commitment to any particular library or framework.

```text
Natural-Language Interface
        |
        v
Query Understanding and Planning
(intent, entities, constraints, context, decomposition)
        |
        v
Search Orchestrator
(policy, capability selection, budgets, cancellation)
        |
        +--------------------+---------------------+
        |                    |                     |
        v                    v                     v
 Lexical Retrieval     Semantic Retrieval     Structured Filters
        |                    |                     |
        +--------------------+---------------------+
                             |
                             v
                  Fusion and Reranking
                             |
                             v
                  Normalized Search Results
              (provenance, scores, evidence, trace)
```

Cross-cutting components:

- Domain model and contracts
- Provider adapters and capability registry
- Configuration validation
- Evaluation and regression testing
- Logging, metrics, tracing, and cost accounting
- Access control and data lifecycle management
- Index versioning, migration, recovery, and consistency checks

### 3.1 Architectural boundaries

**Domain**
- Owns document, chunk, query, filter, result, score, provenance, and relevant policy types.
- Contains business invariants.
- Does not depend on provider SDKs.

**Application**
- Implements use cases and coordinates ingestion, indexing, retrieval, fusion, and result assembly.
- Depends on project-owned contracts, not concrete adapters.

**Contracts / ports**
- Define capabilities and behavioral expectations.
- Use provider-neutral types.
- Make errors and unsupported operations explicit.

**Adapters**
- Translate between project contracts and external technology APIs.
- Keep provider-specific configuration and quirks inside the integration boundary.

**Configuration and registry**
- Validate component compatibility.
- Resolve implementations and policies at runtime.
- Fail clearly when required capabilities are unavailable.

**Evaluation**
- Runs independently of production provider choices where practical.
- Stores dataset, configuration, pipeline, and result metadata required to reproduce comparisons.

**Observability**
- Records pipeline stages, latency, errors, selected strategies, and cost.
- Redacts sensitive content and credentials.

## 4. Versioned Roadmap

Milestones are sequential quality gates, not calendar promises. Advance only when the acceptance criteria for the current milestone pass.

### M00 — Repository Audit and Architecture Baseline

**Objectives**
- Establish the actual state of the repository before planning file-level changes.
- Identify hard-coded technology dependencies and architecture risks.
- Document the target boundaries and implementation sequence.

**Work**
- Inspect project structure, dependencies, module boundaries, runtime entry points, tests, and existing documentation.
- Trace ingestion, chunking, indexing, query handling, retrieval, ranking, and result output.
- Identify direct provider imports in domain or application logic.
- Inventory fixed model names, vector dimensions, backend assumptions, score thresholds, and implicit defaults.
- Record relevant risks and decisions in Architecture Decision Records (ADRs).

**Deliverables**
- Repository and dependency audit
- Current-state architecture map
- Hard-code and coupling inventory
- Initial ADRs
- Updated, repository-specific implementation plan

**Acceptance criteria**
- Findings are based on inspected code and reproducible commands.
- Existing behavior and test baseline are documented.
- Every high-risk dependency has a proposed remediation or an explicit reason to retain it.

### M01 — Provider-Neutral Search Core

**Objectives**
- Establish stable domain and application boundaries.

**Work**
- Define core types such as `Document`, `Chunk`, `Query`, `Filter`, `SearchResult`, `Score`, and `Provenance`.
- Define ingestion, retrieval, persistence, and result-assembly contracts as justified by current requirements.
- Standardize validation errors, operational errors, cancellation, timeout, and pagination semantics.
- Separate domain and application logic from implementation-specific types.
- Add deterministic fake implementations for core tests.

**Deliverables**
- Domain model and contract documentation
- Dependency direction rules
- Unit and contract tests
- Minimal end-to-end search flow using test doubles or a minimal implementation

**Acceptance criteria**
- Core tests run without requiring a live AI or search provider.
- Replacing an adapter does not require changes to domain rules.
- Unsupported capabilities and invalid configurations fail explicitly.

### M02 — Evaluation Harness and Lexical Baseline

**Objectives**
- Establish an objective way to measure relevance before adding complexity.

**Work**
- Create a representative corpus and query set.
- Record relevance judgments, including difficult and ambiguous queries.
- Implement or adapt a lexical baseline behind a provider-neutral contract.
- Measure retrieval and ranking quality.
- Store benchmark metadata and make runs repeatable.

**Recommended metrics**
- Recall@K: whether relevant items are retrieved.
- Precision@K: how many top results are relevant.
- MRR: how highly the first relevant result is ranked.
- nDCG@K: ranking quality when graded relevance judgments exist.
- Latency, resource use, and cost where applicable.

**Deliverables**
- Versioned evaluation dataset
- Relevance-judgment format and annotation guidance
- Repeatable benchmark command or test workflow
- Baseline report

**Acceptance criteria**
- Benchmark runs can be repeated with recorded configuration and dataset versions.
- Metrics are calculated consistently and have documented limitations.
- No quality claim is accepted without evidence from the evaluation set.

### M03 — Semantic and Cross-Lingual Retrieval; Embedding Lifecycle

**Objectives**
- Add semantic retrieval across languages without coupling the core to one embedding model, inference runtime, or vector technology.
- Make cross-lingual retrieval a first-class capability, not a later dictionary-based patch.

**Work**
- Define provider-neutral text-embedding and vector-retrieval contracts.
- Model embedding capabilities and language coverage as declared metadata; do not assume every model is multilingual or cross-lingual.
- Add adapters only for implementations justified by privacy, runtime, language coverage, performance, and licensing requirements.
- Track model identity, model revision, dimensions, normalization behavior, supported languages, and vector-space compatibility.
- Validate compatibility between query vectors, document vectors, and the target index.
- Define re-embedding, index migration, rollback, and partial-failure recovery.
- Evaluate query/document language pairs independently, including Vietnamese queries against English documents and the reverse direction.
- Preserve the local-first / zero-knowledge requirement: document text and query content must not leave the user's device unless a future explicit product decision and user consent change that requirement.

**Required cross-lingual examples**
- Vietnamese query `tìm hóa đơn` must be able to retrieve English documents titled or describing `invoice`.
- Vietnamese `biên lai thanh toán` should retrieve relevant English documents such as `payment receipt` or `proof of payment` when their actual content supports that meaning.
- English `find the signed contract` should retrieve relevant Vietnamese documents describing a `hợp đồng đã ký`.
- Tests must also include hard negatives, such as `invoice` versus `receipt` where the domain makes them distinct, so broad semantic similarity does not collapse different document types.

**Deliverables**
- Embedding and vector retrieval contracts
- Declared language/capability metadata and model/index compatibility validation
- Embedding lifecycle and migration design
- Cross-lingual evaluation set with labeled relevance and hard negatives
- Semantic and cross-lingual retrieval evaluation report

**Acceptance criteria**
- Incompatible vector/index configurations are rejected before use.
- Model changes cannot silently mix incompatible embeddings in the same index.
- Cross-lingual tests pass in both query-to-document directions for every language pair claimed as supported.
- Semantic retrieval demonstrates measurable value against lexical-only baselines.
- No specific embedding model, runtime, or vector backend is mandatory in the domain core.

### M04 — Hybrid Retrieval and Structured Constraints

**Objectives**
- Combine complementary retrieval methods while preserving hard constraints.

**Work**
- Run lexical and semantic retrieval through separate capabilities.
- Add a fusion policy, such as Reciprocal Rank Fusion, where evaluation supports it.
- Support metadata filters, mandatory constraints, and access-control filters.
- Avoid combining incomparable raw scores without calibration.
- Record which retrieval paths contributed to each result.

**Deliverables**
- Fusion contract and policy
- Filter and access-control semantics
- Hybrid retrieval tests and benchmark report

**Acceptance criteria**
- Hard constraints are never treated as optional ranking hints.
- Fusion behavior is deterministic for a fixed configuration and input where feasible.
- Hybrid retrieval is compared against lexical and semantic baselines.

### M05 — Natural-Language and Cross-Lingual Query Understanding

**Objectives**
- Interpret user intent and constraints rather than relying on similarity alone.
- Support users querying in one language while relevant files use another, without requiring the user to know the document's language.

**Work**
- Represent query intent, entities, time ranges, mandatory constraints, soft preferences, and context when relevant.
- Add optional query rewriting, expansion, or decomposition behind explicit capabilities.
- Distinguish required conditions from ranking preferences.
- Detect or infer language as a supporting signal only; language detection must not be treated as the semantic matching mechanism.
- Define language coverage from actual component capabilities and benchmark results, not from hardcoded assumptions.
- Do not rely on a manually maintained Vietnamese-English synonym dictionary as the primary cross-lingual solution. A configurable domain vocabulary may supplement retrieval for specialized terms, but it must be optional, data-driven, replaceable, and must not be required for general cross-lingual matching.
- Define when ambiguity requires clarification and when the system should proceed conservatively.
- Test negation, synonyms, misspellings, dates, numbers, conflicting constraints, mixed-language queries, and query/document language mismatches.
- Keep query rewriting bounded and ensure it cannot silently remove mandatory constraints or broaden access permissions.

**Deliverables**
- Query-understanding contract and normalized query representation
- Policy for clarification and query transformation
- Cross-lingual query suite with query language, document language, intent, constraints, relevance labels, and hard negatives
- Evaluation report for intent preservation and cross-lingual retrieval quality

**Acceptance criteria**
- Query transformations do not silently remove mandatory constraints.
- A user can query in Vietnamese and retrieve semantically relevant English content, including `hóa đơn` → `invoice`, without adding that pair as a hardcoded code constant.
- The same capability is tested in the reverse direction and across every additional language pair the project claims to support.
- Supported languages and known limitations are documented from measured evidence.
- Quality is measured against labeled examples rather than subjective demonstrations alone.

### Cross-Lingual Acceptance Matrix

Cross-lingual support must be tested as a matrix, not demonstrated with a single example. The initial evaluation set must include at least:

| Query language | Query example | Relevant document language | Expected behavior |
|---|---|---|---|
| Vietnamese | `tìm hóa đơn` | English | Retrieve relevant `invoice` documents without a hardcoded translation pair |
| English | `find an invoice` | Vietnamese | Retrieve relevant `hóa đơn` documents |
| Vietnamese | `tìm biên lai thanh toán` | English | Retrieve relevant `payment receipt` / `proof of payment` content when context supports it |
| English | `find the signed contract` | Vietnamese | Retrieve relevant `hợp đồng đã ký` content |
| Vietnamese | `tìm tài liệu hướng dẫn` | English | Retrieve relevant `guide`, `manual`, or `documentation` content based on semantics |
| Mixed | `find hóa đơn tháng 8` | Vietnamese and English | Preserve both document-type intent and the month constraint |

Add hard negatives and ambiguous cases for each domain. These examples are benchmark data, not permission to embed fixed translation pairs into the search algorithm. Expand the matrix as new languages and domains are declared supported.

### M06 — Reranking and Relevance Optimization

**Objectives**
- Improve ordering of retrieved candidates when evidence shows ranking is the bottleneck.

**Work**
- Define a reranker contract.
- Separate candidate recall from ranking quality.
- Evaluate reranking against the same candidate sets and datasets.
- Consider relevance, freshness, source quality, and business signals only when justified.
- Add latency budgets, candidate limits, cancellation, and cost controls.

**Deliverables**
- Reranking contract and policy
- Comparative ranking evaluation
- Performance and cost report

**Acceptance criteria**
- Reranking improvement is measured independently from retrieval.
- Added latency and cost are documented.
- The system can operate without optional reranking when it is unavailable or not justified.

### M07 — Controlled Search Agent

**Objectives**
- Add multi-step planning only after underlying search capabilities are reliable.

**Work**
- Decompose complex questions into bounded subqueries where useful.
- Select strategies using capability declarations and policy, not provider-specific conditionals spread across the codebase.
- Merge and deduplicate results while retaining provenance.
- Validate evidence and distinguish retrieved facts from generated interpretations.
- Enforce limits on iterations, tool calls, time, tokens, and monetary cost.
- Provide deterministic non-agentic fallbacks for supported workflows.

**Deliverables**
- Agent orchestration policy
- Bounded execution and stopping rules
- Provenance-aware result aggregation
- Tests for loops, failed tools, contradictory results, and unsupported capabilities

**Acceptance criteria**
- Agent execution is bounded and observable.
- The agent cannot invent sources or present unsupported statements as retrieved facts.
- Simpler queries do not incur unnecessary multi-step processing.

### M08 — Production Reliability, Security, and Operations

**Objectives**
- Make indexing and search resilient, secure, and operable.

**Work**
- Test restart recovery, partial indexing failures, retry safety, and consistency checks.
- Define index versioning, rebuild, migration, cleanup, and rollback procedures.
- Enforce tenant isolation and document-level access control where required.
- Ensure deletions propagate through chunks, indexes, caches, and derived artifacts.
- Protect against prompt injection in retrieved content, unsafe query transformations, and resource exhaustion.
- Instrument latency, errors, throughput, resource use, provider failures, and cost.
- Redact secrets and sensitive user/document content from logs.

**Deliverables**
- Recovery and migration procedures
- Security and access-control tests
- Fault-injection suite
- Operational dashboards or documented telemetry
- Deployment and rollback guide

**Acceptance criteria**
- Failure scenarios have explicit expected outcomes and tests.
- Access-control filtering is applied before unauthorized content can leak into results or generated responses.
- Recovery and rollback procedures are tested, not merely documented.

### M09 — Continuous Evaluation and Optimization

**Objectives**
- Improve search quality safely as data, queries, and providers evolve.

**Work**
- Categorize failed queries and missing relevant documents.
- Track quality by language, query class, data source, and strategy where sample sizes allow.
- Compare new providers or strategies using the same evaluation protocol.
- Add regression guards and controlled rollout for ranking or pipeline changes.
- Support canary evaluation and rollback.
- Use feedback only under documented privacy, retention, and data-quality rules.

**Deliverables**
- Ongoing evaluation workflow
- Search-quality regression gates
- Strategy comparison reports
- Rollout and rollback procedures

**Acceptance criteria**
- Quality changes are reproducible and attributable to known configuration or pipeline changes.
- Regressions block rollout according to agreed thresholds.
- Optimization decisions consider relevance, latency, reliability, and cost together.



## 5. Hard-Coding Prevention Checklist

Apply this checklist to every milestone and code review.

### AI and language models
- [ ] Domain logic does not call a provider SDK directly.
- [ ] Model identity and supported capabilities are explicit.
- [ ] Provider-specific request/response types stay inside adapters.
- [ ] Timeouts, limits, failures, and unsupported features are handled explicitly.

### Embeddings and cross-lingual behavior
- [ ] Model names are not scattered through source code.
- [ ] Dimensions and vector metadata are validated, not assumed.
- [ ] Model changes trigger compatibility checks and a migration plan.
- [ ] Incompatible embeddings cannot be silently mixed.
- [ ] Cross-lingual support is declared per model/capability and verified by evaluation.
- [ ] General cross-lingual matching does not depend on hardcoded translation pairs or a mandatory hand-authored synonym dictionary.
- [ ] Any domain vocabulary is externalized/configurable, optional, and treated as supplemental evidence rather than the core semantic engine.
- [ ] Client-side inference and local data handling preserve the project's zero-knowledge requirement unless the product specification explicitly changes.

### Storage and retrieval
- [ ] Domain logic does not depend on a database-specific API.
- [ ] Filter, delete, pagination, consistency, and lifecycle semantics are documented.
- [ ] Backend capabilities and limitations are declared.
- [ ] Fallback behavior never weakens access control or mandatory filters.

### Ranking and orchestration
- [ ] Retrieval strategy is selected through policy or explicit configuration.
- [ ] Raw scores from different systems are not combined without justification.
- [ ] Optional capabilities can be omitted safely.
- [ ] Thresholds are evaluated and configured rather than universally assumed.

### Build, runtime, and dependencies
- [ ] Dependency direction is enforced by tests or automated checks where practical.
- [ ] A specific DI container, plugin framework, or application framework is not required by the domain.
- [ ] Dependency versions and compatibility constraints are documented.
- [ ] Configuration is validated at startup or before the relevant operation.

### Evaluation and tests
- [ ] Core tests run without live external providers.
- [ ] Adapter contract tests exist.
- [ ] Integration tests cover real provider behavior where practical.
- [ ] Search-quality benchmarks are independent of a single implementation.
- [ ] Negative tests cover missing capabilities, malformed data, provider failure, and incompatible configuration.

## 6. Quality Gates

Every milestone must pass applicable gates before it is considered complete.

### G1 — Functional correctness
- Ingestion, update, delete, filtering, pagination, and index lifecycle behave as specified.
- Deleted or unauthorized content does not reappear in results.
- Error paths and invalid inputs are tested.

### G2 — Retrieval quality
- A representative corpus and labeled query set exist.
- Retrieval and ranking metrics are reported with dataset and configuration versions.
- Hard constraints, negation, multilingual queries, and difficult examples have dedicated tests.
- No universal score threshold is assumed without empirical support.

### G3 — Replaceability
- Adapters can be changed without rewriting domain logic.
- Contract tests document behavior that must remain consistent.
- Provider-specific limitations and semantic differences are visible.

### G4 — Reliability and security
- Fault injection covers partial failure, restarts, retries, and inconsistent state.
- Authorization and tenant boundaries are tested.
- Resource limits and untrusted retrieved content are handled safely.

### G5 — Performance and cost
- p50/p95 latency, throughput, resource use, indexing cost, and query cost are measured where applicable.
- Targets are set after establishing a representative baseline.
- Optional expensive capabilities have explicit budgets and fallback behavior.

## 7. Implementation Workflow

For each milestone:

1. Inspect the current implementation and relevant tests.
2. Write or update the ADR and acceptance criteria.
3. Identify the smallest coherent set of files to change.
4. Implement contracts and behavior incrementally.
5. Add unit, contract, integration, regression, and negative tests as appropriate.
6. Run the relevant test suite and record exact results.
7. Audit architecture boundaries and search-quality impact.
8. Update documentation and changelog before release.
9. Commit only after the working tree and release checks meet the project's policy.
10. Update this roadmap with completed work, evidence, known limitations, and the next milestone.

Do not report tests, screenshots, benchmarks, or repository changes as completed unless they were actually performed and their results were inspected.

## 8. Release Readiness

A milestone is ready only when:

- [ ] Acceptance criteria are satisfied.
- [ ] Relevant tests pass and failures are explained.
- [ ] Architecture and dependency audits are reviewed.
- [ ] Search-quality changes are measured when retrieval or ranking changes.
- [ ] Configuration, migration, failure, and rollback behavior are documented where applicable.
- [ ] Documentation and changelog are updated.
- [ ] Known limitations and deferred work are explicit.
- [ ] The working tree and commit state match the intended release policy.

## 9. Definition of Done: Genuine Natural-Language Semantic Search

SemanticSearchAgent can be considered mature when it demonstrates all of the following:

1. Users can search using natural-language paraphrases instead of knowing exact document wording.
2. Users can query in one supported language and retrieve relevant documents written in another, in both directions, without hardcoded translation pairs.
3. Supported languages and multi-condition queries are evaluated with representative tests.
4. Retrieval quality is demonstrated with relevance judgments and repeatable benchmarks.
5. Lexical, semantic, filtering, fusion, and reranking capabilities can evolve independently.
6. Models, providers, and storage implementations can be replaced without rewriting domain rules.
7. Results preserve source provenance and expose enough trace information for debugging.
8. Mandatory constraints and authorization rules cannot be silently relaxed by query rewriting or agent planning.
9. Failures, migrations, restarts, and index inconsistencies have tested recovery behavior.
10. Performance, reliability, and cost are measured against explicit targets.
11. New capabilities pass contract, regression, security, and quality gates.

## Immediate Next Step — M00

Start with **M00 — Repository Audit and Architecture Baseline** for:

`E:\10_Learning\Rust\SemanticSearchAgent`

The audit should establish the real repository state before producing a file-by-file implementation plan. It must identify current dependencies, existing behavior, test coverage, architecture violations, and the smallest safe sequence of changes.

This document defines the target roadmap. It does **not** claim that the repository has already been audited or that any milestone has been implemented.

## Cross-Cutting Track — M12: Provider-Agnostic AI Model Integration

**Objective**

Provide production-quality, optional AI-assisted query understanding and embedding integration without binding the search core to a vendor, SDK, or fixed model catalogue.

**Required contracts**

- Provider-neutral model descriptors and capability declarations.
- Text generation, structured output, streaming, embeddings, and multimodal capabilities as independent contracts; only advertise a capability when its adapter actually implements and tests it.
- Model discovery and capability validation at runtime where the provider supports discovery; configurable descriptors where discovery is unavailable.
- Normalized errors for authentication, permission, quota, rate limits, timeouts, cancellation, network failures, invalid responses, unsupported capabilities, and provider outages.
- Explicit model routing, fallback routes, retry policy, timeout/cancellation, request correlation, cost/usage accounting, and secret-safe diagnostics.
- Embedding-space identity including provider, model, revision, dimensions, normalization, and compatible index version.
- Provider contract tests using deterministic fake adapters and mocked HTTP responses; no live API key required for CI.

**Google AI Studio adapter**

- Integrate through the provider contract, not by adding Google-specific conditionals to search-core logic.
- Discover available models and capabilities rather than compiling a permanent union of model IDs.
- Support model generation and structured JSON output for query planning; support embeddings only through models that declare and pass embedding capability checks.
- Normalize Google API errors and respect quotas, rate limits, cancellation, and request deadlines.
- Never silently fall back to a different provider if doing so changes privacy, data residency, or consent semantics.
- Keep API keys out of source control and logs. Browser-shipped keys are not secret; production deployments must use a trusted backend or a clearly user-controlled secret mechanism.

**Additional providers**

- Prove the abstraction with at least one independently implemented second provider or a standards-based/custom endpoint adapter.
- Support provider configuration by endpoint, authentication strategy, model mapping, and capability metadata where appropriate.
- Do not assume APIs that share a superficially similar schema have identical behavior or capability support.

**Acceptance criteria**

- Core search tests pass with no network, credentials, or provider SDK.
- Google AI Studio can be selected by configuration and its models/capabilities are discovered or explicitly configured.
- Unsupported capabilities fail before sending a request.
- Tests cover success, malformed response, 400, 401/403, 404, 429/quota, 5xx, timeout, cancellation, and network failure.
- Fallback is explicit, observable, capability-compatible, and privacy-policy-compatible.
- Query planning output is schema-validated; required filters, negation, date constraints, and access rules cannot be silently dropped.
- Remote AI calls are opt-in, and local-only operation remains available.
- Embeddings from incompatible model spaces cannot be mixed in one index.
- Provider changes are compared against a stable cross-lingual relevance evaluation set, including Vietnamese "hóa đơn" ↔ English "invoice" and hard-negative cases.

**Current implementation note (2026-10-12)**

The first TypeScript contract layer and Google AI Studio REST adapter have been added in ts/ai-provider-contract.ts and ts/google-ai-studio-provider.ts, with design notes in docs/AI_PROVIDER_ARCHITECTURE.md. This is an initial foundation, not completion of M12. The legacy google-ai-studio-orchestrator.ts still needs migration onto these contracts, and provider contract tests, a second adapter, production routing policy, and embedding index identity remain gates before this milestone can be considered complete.

## Repository Reality Check — Audit Findings (2026-10-12)

**Audit status: M00 is partially inspected, but not complete.** This section records findings verified against the current repository. Passing compilation/tests does not mean semantic-search acceptance criteria are met.

### Verified current state

- The Rust crate exposes a WASM-facing query parser, a rule-based search-plan builder, keyword/filename/content scoring, time-pattern matching, and cosine similarity for vectors passed by the caller.
- Current Rust modules are limited to `lib.rs`, `query`, `thesaurus`, `fusion`, and `similarity`. There is no implemented document ingestion pipeline, persistent document index, embedding generation adapter, vector retrieval index, OCR pipeline, or end-to-end folder search in this crate.
- `query/mod.rs` uses hardcoded language/script checks, fixed month alias lists and year range 2020–2030, and explicit typo replacements such as `biên lại` → `biên lai`.
- `thesaurus/mod.rs` contains a compile-time multilingual synonym catalogue for a small set of domains. It is not an externally configured vocabulary yet.
- `fusion/mod.rs` is not Reciprocal Rank Fusion or dense-vector + BM25 fusion. It is a hand-weighted substring scorer: filename keyword match +350, content substring match +200, temporal substring match +250, candidate threshold 150. The current implementation has no BM25 index and does not retrieve or compare document embeddings.
- `computeCosineSimilarity` computes a mathematical similarity from supplied vectors; it does not generate embeddings or perform semantic retrieval by itself.
- `ts/google-ai-studio-orchestrator.ts` is a separate legacy Gemini-only planner. It hardcodes a small model union/default, directly calls the Google REST API, includes fixed cross-language expansions and date-pattern examples in the prompt, and parses model JSON without schema validation. It is not wired through the newly added provider contract.
- `ts/ai-provider-contract.ts` and `ts/google-ai-studio-provider.ts` are an initial integration foundation. They are not yet covered by provider contract tests and do not establish production-ready provider portability. The Google adapter currently implements generation and embeddings but not streaming, tool calling, or multimodal requests. Its model capability discovery only reflects the currently mapped methods.
- The roadmap's M12 provider milestone is appended after the “Immediate Next Step: M00” section. This ordering is confusing: M12 is a cross-cutting capability track and must not imply that it is the next sequential implementation milestone or that earlier quality gates can be skipped.
- `ARCHITECTURE_PLAN.md` conflicts with the provider-agnostic roadmap in several places: it mandates named algorithms/libraries in task descriptions, contains a compile-time administrative thesaurus despite the no-hardcoded-vocabulary principle, sets unmeasured latency/score targets, and uses a weighted-sum formula while labeling it RRF. These are proposals, not verified capabilities, and must be reconciled before implementation.
- The Rust test `test_workspace_files_guidance_documentation` manually constructs a plan that claims to be Gemini-produced. It does not call Google AI Studio. Its passing result proves the scorer can score a prepared keyword list against a few local files, not that the live AI integration or cross-lingual semantic retrieval works.

### Verification commands executed on 2026-10-12

- `cargo test`: 4 passed, 0 failed.
- `cargo check --target wasm32-unknown-unknown`: passed.
- TypeScript no-emit check for the provider contract, Google adapter, and legacy orchestrator: passed.
- These checks are compile/unit-test evidence only. No live Google API request, real embedding generation, real cross-lingual retrieval benchmark, 10,000-file performance test, OCR test, browser/WASM end-to-end test, or provider-failure contract suite was executed in this audit.

### Material gaps and corrective priorities

1. **M00 remains open:** inventory the actual runtime/integration entry points and all project files, document the current data flow, and reconcile the two architecture documents.
2. **Remove false semantic claims:** until embeddings are generated and used for retrieval, describe the current engine as rule-based multilingual keyword expansion and weighted lexical scoring, not genuine semantic search.
3. **Eliminate mandatory hardcoded domain knowledge:** move optional thesaurus/domain vocabulary and typo rules into versioned external configuration, or replace them with general corpus-based correction and measured semantic retrieval. Keep language-independent algorithms and generic date parsing separate from domain terms.
4. **Correct retrieval architecture terminology:** implement actual BM25 and/or a documented lexical ranker; implement vector retrieval only after a compatible embedding provider and index exist; use true RRF only if ranks from multiple result lists are fused. Do not label a weighted sum “RRF.”
5. **Finish AI adapter contracts before claiming portability:** migrate the legacy orchestrator; add mocked contract tests and negative tests; fix cancellation/timeout classification and error redaction; validate planner JSON against a schema; and add a second adapter or standards-based endpoint adapter.
6. **Protect embedding compatibility:** persist provider/model/revision/dimension/normalization metadata with every vector index and reject incompatible queries/indexes.
7. **Replace unverified performance assertions:** remove claims such as “10,000 files under one second,” “Top 50 under 50ms,” “absolute accuracy,” fixed score bonuses, and hard thresholds unless supported by reproducible benchmark data on representative hardware and documents.
8. **Preserve privacy policy:** Google AI Studio is a remote service. Any query or document content sent to it is not local-only/zero-knowledge. Keep remote planning/embedding opt-in, disclose what leaves the device, and ensure local-only mode works without credentials or network.

### Revised status

- M00 Repository Audit: **IN PROGRESS**
- Provider-neutral search core: **PARTIAL / NOT ACCEPTED**
- Lexical baseline: **PROTOTYPE; no benchmark report**
- Semantic/cross-lingual retrieval: **NOT IMPLEMENTED**
- Hybrid retrieval / actual RRF: **NOT IMPLEMENTED**
- Natural-language planning: **RULE-BASED LOCAL PLAN + UNMIGRATED GEMINI PLANNER**
- AI provider integration: **FOUNDATION ADDED; NOT PRODUCTION-READY**
- OCR, persistent indexing, 10,000-file evaluation, end-to-end WASM validation: **NOT VERIFIED / NOT IMPLEMENTED IN THE INSPECTED RUST CRATE**

Do not mark any milestone complete based only on successful compilation or the four current Rust unit tests. Update this section only when a behavior is implemented and its acceptance evidence has been recorded.
