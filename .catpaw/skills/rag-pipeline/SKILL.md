---
name: rag-pipeline
description: Production RAG pipeline optimization for EchoMind's local RAG knowledge base. Use when improving retrieval quality, tuning chunking strategy, optimizing hybrid search (vector + BM25 RRF fusion), configuring rerankers, building eval harnesses, or iterating on RAGAS-style metrics. Covers fastembed ONNX embeddings, SQLite vector store, HNSW index, ColBERT multi-vector, HyDE query rewriting, and agentic RAG ReAct loops.
---

# RAG Pipeline Engineer

Adapted from [agency-agents](https://github.com/msitarzewski/agency-agents) RAG Pipeline Engineer, tailored for EchoMind's pure-Rust RAG architecture (no Python/LangChain).

## EchoMind RAG Architecture

```
chat IPC -> chat_inner()
  -> empty KB check (count_documents == 0 -> error)
  -> init embedder (lazy, fastembed/ONNX all-MiniLM-L6-v2, 384-dim)
  -> ChatEngine.chat()
    -> HybridRetriever.retrieve() (if hybrid_search enabled)
      -> Embedder.embed(query) -> Storage.vector_search(cosine sim, top-k=5)
      -> Storage.keyword_search(BM25 via FTS5, top-k=5)
      -> rrf_fuse() merge (k=60, vector=1.0, BM25=0.8)
    -> threshold filter (score > 0.1)
    -> (optional) LocalReranker rerank (bge-reranker-base Cross-Encoder)
    -> chunk expansion: expand to adjacent chunks (+/-1)
    -> build RAG prompt -> LLMProvider.chat_stream(SSE)
  -> chat_token events -> frontend typing
  -> chat_done event -> persist_exchange()
```

## Critical Rules

1. **Never skip evals.** "It feels better" is not a metric. Every architectural change gets a before/after eval run. EchoMind has `crates/core/src/rag_eval.rs` (9 RAGAS-style metrics: hit_rate, mrr, ndcg, keyword_overlap, context_embedding_similarity, faithfulness, answer_relevance, context_precision, context_recall).
2. **Chunk for retrieval, not ingestion.** EchoMind has 3 splitters: `TextSplitter` (token-based), `SemanticSplitter` (paragraph->sentence->clause), `SectionAwareSplitter` (markdown heading hierarchy). Choose based on document type.
3. **Validate embeddings on your corpus.** all-MiniLM-L6-v2 (384-dim) is the default. Pro supports custom ONNX models. Test on actual data, not MTEB benchmarks.
4. **Re-ranking is not free.** `LocalReranker` (bge-reranker-base) adds ~200ms. Default is OFF (`rerank_enabled = false`). Only enable when retrieval precision is the bottleneck.
5. **Metadata matters.** EchoMind supports `workspace_id` filtering, document tags, and entity-based graph retrieval. Design metadata schema before index schema.
6. **Async by default.** All DB operations via `tokio::task::spawn_blocking`. Batch embedding via `rayon` thread pool.
7. **Rust ecosystem first.** No Python dependencies. Pure Rust: `fastembed` (ONNX), `rusqlite` (SQLite), `hnsw_rs` (HNSW), `tiktoken-rs` (tokenization).

## Retrieval Pipeline Components

| Component | File | Strategy |
|---|---|---|
| `VectorRetriever` | `crates/core/src/retriever.rs` | Pure cosine similarity vector search |
| `HybridRetriever` | `crates/core/src/hybrid_retriever.rs` | Vector + keyword (BM25 FTS5) -> RRF fusion (k=60). Runtime toggle via `set_hybrid_enabled(bool)`. |
| `GraphRetriever` | `crates/core/src/graph_retriever.rs` | Entity BFS traversal, 2-hop max, 4-way RRF fusion |
| `LocalReranker` | `crates/infra/src/local_reranker.rs` | bge-reranker-base Cross-Encoder (REQ-RAG-020) |
| `HydeRewriter` | `crates/infra/src/hyde_rewriter.rs` | HyDE query rewriting, graceful degradation (REQ-RAG-021) |
| `HNSW Index` | `crates/infra/src/hnsw_index.rs` | Approximate nearest neighbor (Pro, REQ-NFR-005) |
| `PropositionSplitter` | `crates/core/src/proposition_splitter.rs` | Sentence + pronoun resolution + context completion (REQ-PERF-007) |
| `SummaryTreeBuilder` | `crates/core/src/summary_tree.rs` | RAPTOR multi-level summary tree (REQ-PERF-009) |
| `AgentEngine` | `crates/core/src/agent.rs` | ReAct agentic RAG, max 5 iterations (REQ-RAG-022) |

## Advanced RAG Features (Toggleable)

| Feature | Setting Key | Default | Description |
|---|---|---|---|
| Hybrid Search | `rag.hybrid_search` | true | Vector + BM25 RRF fusion |
| Reranker | `rag.rerank_enabled` | false | bge-reranker-base Cross-Encoder |
| HyDE | `rag.hyd_enabled` | false | Query rewriting |
| Graph Retriever | `rag.graph_retriever_enabled` | false | Entity graph traversal |
| Progressive Injection | `rag.progressive_injection` | false | Start with top-2 chunks, expand if insufficient |
| Speculative RAG | `rag.speculative_enabled` | false | Draft model + verify model |
| Retrieval Memory | `rag.retrieval_memory_enabled` | false | Self-evolving method selection |
| Response Cache | `cache.enabled` | true | 3-tier: exact / semantic / retrieval |

## Eval Harness

EchoMind has a built-in RAGAS-style eval system (`crates/core/src/rag_eval.rs`):

- **5 pure Rust retrieval metrics** (zero LLM cost): hit_rate, mrr, ndcg, keyword_overlap, context_embedding_similarity
- **4 LLM-as-Judge metrics** (uses `LLMProvider::one_shot`): faithfulness, answer_relevance, context_precision, context_recall
- LLM metric failures degrade gracefully (skip, don't crash)
- 45 TDD tests in `rag_eval_tests.rs`

Run eval via IPC: `run_rag_evaluation(samples, settings)` -> `RagEvalReport`

## Workflow

### 1. Document Analysis (before any code change)
- Audit corpus: document types, average length, languages (CJK support critical)
- Define query distribution: factual / analytical / code / conversational
- Identify metadata: workspace_id, tags, entities, source_path

### 2. Embedding & Index Selection
- Default: all-MiniLM-L6-v2 (384-dim, ~30MB, fastembed/ONNX)
- Pro: custom ONNX model upload (REQ-VEC-014)
- HNSW (Pro): 32 max connections, 16 max layers, cosine distance metric
- Vector cache: in-memory `Arc<RwLock<Option<Arc<Vec<(chunk_id, vector)>>>>` for O(1) reuse

### 3. Retrieval Pipeline Tuning
- Run baseline eval on golden dataset
- Tune RRF `k` parameter (default 60), vector weight (1.0), BM25 weight (0.8)
- Tune threshold filter (default score > 0.1)
- Tune top-k (default 5, expandable via progressive injection)
- Ablation: toggle each feature on/off, measure impact

### 4. Re-ranking Decision
- Analyze baseline retrieval precision on golden dataset
- If precision < 0.75, trial `LocalReranker`; measure latency delta (~200ms)
- Only deploy if: precision gain > 10% AND latency stays within SLA

### 5. Eval-Driven Iteration
- Run RAGAS eval suite (`run_rag_evaluation` IPC command)
- Identify lowest-scoring metric
- Hypothesize cause; change one variable at a time
- Rerun eval; only keep changes that improve target metric without degrading others

## Key Files

- `crates/core/src/chat.rs` — ChatEngine (RAG query orchestration)
- `crates/core/src/retriever.rs` — VectorRetriever
- `crates/core/src/hybrid_retriever.rs` — HybridRetriever (vector + BM25 RRF)
- `crates/core/src/rag_eval.rs` — RAGAS-style eval (9 metrics)
- `crates/infra/src/local_embedder.rs` — fastembed/ONNX (384-dim)
- `crates/infra/src/local_reranker.rs` — bge-reranker-base Cross-Encoder
- `crates/infra/src/hnsw_index.rs` — HNSW ANN index (Pro)
- `crates/infra/src/sqlite_storage.rs` — SQLite vector BLOB + FTS5
- `docs/SRS_v1.0.md` — REQ-RAG-* requirements (verification criteria)
