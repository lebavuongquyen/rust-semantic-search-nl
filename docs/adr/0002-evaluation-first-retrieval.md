# ADR 0002: Evaluation-First Retrieval and Ground Truth Metrics

## Status
Accepted

## Context
A major risk in AI/semantic search prototypes is subjective verification: claiming high relevance or cross-lingual capability based on cherry-picked queries or passing unit tests with mock values. Without a versioned corpus, labeled queries, and hard negatives, regressions in ranking or retrieval cannot be detected.

## Decision
1. **Evaluation Harness**: Implement an automated, versioned evaluation harness that evaluates retrieval against a standard golden dataset before accepting algorithmic changes.
2. **Standard Relevance Metrics**: Retain and compute standard information retrieval metrics:
   - Recall@K: proportion of relevant documents retrieved in top-K candidates.
   - Precision@K: proportion of top-K retrieved documents that are relevant.
   - MRR (Mean Reciprocal Rank): reciprocal rank of the first relevant document.
   - nDCG@K (Normalized Discounted Cumulative Gain): ranking quality when graded relevance judgments exist.
3. **Cross-Lingual Golden Dataset**: Maintain paired test cases in both directions (Vietnamese Query -> English Document, English Query -> Vietnamese Document) alongside hard negatives (distinguishing semantically adjacent but distinct concepts such as invoices vs receipts).
4. **Regression Gate**: No claim of retrieval improvement or cross-lingual support will be accepted without reproducible metrics run on the evaluation dataset.

## Consequences
- Positive: Objective, quantifiable measurement of lexical, semantic, and hybrid retrieval.
- Positive: Guards against catastrophic forgetting or regressions when tuning thresholds or fusion weights.
- Trade-off: Requires maintaining and curating golden benchmark datasets and relevance judgments.
