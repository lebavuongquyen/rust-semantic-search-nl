# Gemini Embedding Models Probe Results

> **Date:** 2026-10-14  
> **Source:** Live probe against Google AI Studio API (`v1beta`) using genuine API credentials.

## 1. Discovered Models & Dimensions

| Model ID | Display Name | Supported Methods | Dimensions | Cross-lingual Cosine (`tìm hóa đơn` <-> `invoice`) |
| :--- | :--- | :--- | :---: | :---: |
| `gemini-embedding-001` | Gemini Embedding 001 | `embedContent`, `asyncBatchEmbedContent` | **3072** | **0.6946** |
| `gemini-embedding-2` | Gemini Embedding 2 | `embedContent`, `asyncBatchEmbedContent` | **3072** | **0.6284** |
| `gemini-embedding-2-preview` | Gemini Embedding 2 Preview | `embedContent`, `asyncBatchEmbedContent` | **3072** | **0.6284** |

## 2. Key Architecture Findings

1. **Embedding Dimensions:**
   - The native embedding output dimension for `gemini-embedding-001` and `gemini-embedding-2` is **3072** float32 values.
   - Unified dimension setting across TypeScript and Rust WASM must be configured to **3072**.

2. **Genuine Multilingual Semantic Capability:**
   - Both models produce strong positive cross-lingual semantic similarity between Vietnamese queries and English terms without any manual dictionary or translation pairs.
   - Baseline cross-lingual cosine: ~0.63 - 0.70.

3. **Load Balancer Slot Configuration:**
   - Slot 0: `gemini-embedding-001` (100 RPM quota)
   - Slot 1: `gemini-embedding-2` (100 RPM quota)
   - Fallback: Local deterministic embedder (dimension scaled to 3072).
