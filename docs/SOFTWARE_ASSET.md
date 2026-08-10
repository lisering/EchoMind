# EchoMind（灵犀）— 软件资产文档

> **本文档是 EchoMind 项目的权威软件资产清单，覆盖全部源代码、架构、数据流、依赖、测试、构建与部署细节。后续迭代开发必须同步更新此文档。**
>
> **版本**: v1.5.0 | **更新日期**: 2026-08-10 | **维护者**: EchoMind 团队

---

## 目录

1. [项目概览](#1-项目概览)
2. [架构体系](#2-架构体系)
3. [Workspace 与 Crate 清单](#3-workspace-与-crate-清单)
4. [数据模型层 (crates/models)](#4-数据模型层-cratesmodels)
5. [Prompt 构建层 (crates/prompt)](#5-prompt-构建层-cratesprompt)
6. [上下文压缩层 (crates/compact)](#6-上下文压缩层-cratescompact)
7. [核心业务逻辑层 (crates/core)](#7-核心业务逻辑层-cratescore)
8. [基础设施适配层 (crates/infra)](#8-基础设施适配层-cratesinfra)
9. [Tauri 应用层 (crates/tauri-app)](#9-tauri-应用层-cratestauri-app)
10. [许可证签发 CLI (crates/license-issuer)](#10-许可证签发-cli-crateslicense-issuer)
11. [前端 (ui/)](#11-前端-ui)
12. [构建系统](#12-构建系统)
13. [测试体系](#13-测试体系)
14. [数据库 Schema](#14-数据库-schema)
15. [IPC 命令清单](#15-ipc-命令清单)
16. [设置键清单](#16-设置键清单)
17. [安全架构](#17-安全架构)
18. [Pro 功能门控](#18-pro-功能门控)
19. [数据流图](#19-数据流图)
20. [依赖清单](#20-依赖清单)
21. [文档同步规则](#21-文档同步规则)

---

## 1. 项目概览

### 1.1 产品定位

EchoMind（灵犀）是一个 **本地 RAG 知识库桌面应用**，基于 Rust + Tauri v2 构建，面向律师、研究人员等高隐私需求场景。

### 1.2 核心能力

| 能力 | 技术方案 |
|---|---|
| 文档导入 | Markdown / 纯文本 / PDF（含多模态 OCR + VLM） |
| 文本分块 | Token 窗口 / 语义分块 / 章节感知分块 / Proposition 原子分割 |
| 嵌入向量 | fastembed ONNX (all-MiniLM-L6-v2, 384-dim)，多模型运行时切换 |
| 向量检索 | 余弦相似度全表扫描 / HNSW 近似最近邻（Pro） |
| 混合检索 | 向量 + BM25 关键词 → RRF 融合 |
| 重排序 | bge-reranker-base Cross-Encoder（Pro 可选） |
| 查询改写 | HyDE 假设性文档嵌入（Pro） |
| LLM 推理 | OpenAI 兼容 SSE 流式 / 本地 mistral.rs GGUF（Pro） |
| 数据库加密 | SQLCipher AES-256 + Argon2id KDF |
| 隐私保护 | 8 类 PII 检测/脱敏 + 审计哈希链 |
| 安全状态机 | 自动锁屏 / 暴力破解防护 / 紧急销毁 / 剪贴板清除 |
| 多代理 RAG | ReAct 循环 / Coordinator 多步协调 |
| 性能优化 | 三级缓存 / Prompt 压缩 / 渐进注入 / Speculative RAG / 检索记忆 / Prompt Prefix 磁盘缓存 / 上下文压缩增强（原文尾部保留） |

### 1.3 技术栈

- **后端**: Rust Edition 2024 (≥1.85), Tauri v2
- **前端**: Vanilla JS ES Modules (56 个模块), esbuild 打包 IIFE, TypeScript checkJs 类型检查 (tsconfig.json + types/*.d.ts), Tailwind CSS (本地 JIT), 无框架无 CDN
- **数据库**: SQLite (rusqlite + r2d2 连接池, WAL 模式, FTS5, SQLCipher)
- **嵌入模型**: fastembed (ONNX Runtime, all-MiniLM-L6-v2 384-dim)
- **本地 LLM**: mistral.rs v0.9.0 (GGUF 格式, GPU Metal/CUDA 可选)
- **许可证**: Ed25519 签名验证

---

## 2. 架构体系

### 2.1 六边形架构（端口与适配器）

```
crates/models  →  crates/prompt  →  crates/core  →  crates/infra  →  crates/tauri-app
  (contracts)     (prompts)       (ports+logic)  (adapters)     (assembly)
                                     ↑
                              crates/compact
                              (compaction)
```

**依赖方向铁律**: `tauri-app → core → models`，严禁反向依赖。`crates/infra` 实现 `crates/core` 定义的端口 Trait。

### 2.2 三条规则体系

1. **纪律体系**: TDD 驱动、代码注释、真实数据测试、生产级代码质量、最新版本依赖、文档实时同步
2. **工程体系**: Cargo workspace lint deny (`unwrap_used`/`expect_used`/`panic`/`unreachable`/`todo`/`unimplemented`)、CI 强制 clippy
3. **架构体系**: 六边形分层、依赖方向、`#[cfg(feature = "pro")]` 条件编译、原生 `async fn` in trait（无 `async-trait` 宏）

### 2.3 原生 async trait

所有 `crates/core` 中的 Trait 使用 Edition 2024 原生 `async fn`，不使用 `#[async_trait]` 宏。`#![allow(async_fn_in_trait)]` 是刻意豁免——所有 Trait 都有 `Send + Sync` supertrait 约束，且端口仅在 workspace 内消费。

---

## 3. Workspace 与 Crate 清单

### 3.1 Cargo.toml workspace

```toml
[workspace]
resolver = "3"
members = [
    "crates/models",       # 数据模型契约
    "crates/prompt",       # Prompt 构建
    "crates/compact",      # 上下文压缩
    "crates/core",         # 端口 Trait + 业务逻辑
    "crates/infra",        # 适配器实现
    "crates/tauri-app",    # Tauri 组装层
    "crates/license-issuer", # License 签发 CLI
]

[workspace.lints.clippy]
unwrap_used = "deny"
expect_used = "deny"
panic = "deny"
unreachable = "deny"
todo = "deny"
unimplemented = "deny"

[profile.dev.package."*"]
opt-level = 3  # ML 依赖在 dev 模式以 opt-level 3 编译，避免调试时不可用
```

### 3.2 Crate 一览

| Crate | 角色 | 关键依赖 |
|---|---|---|
| `echomind-models` | 数据模型契约 | serde, chrono, uuid |
| `echomind-prompt` | Prompt 构建 | echomind-core (bpe) |
| `echomind-compact` | 上下文压缩 | echomind-core (LLMProvider) |
| `echomind-core` | 端口 Trait + 业务逻辑 | tokio, pulldown-cmark, tiktoken-rs, argon2, zeroize, ed25519-dalek |
| `echomind-infra` | 适配器实现 | rusqlite, r2d2, fastembed, reqwest, mistral.rs (pro) |
| `echomind-tauri-app` | Tauri 组装层 | tauri, 所有内部 crate |
| `license-issuer` | License 签发 CLI | ed25519-dalek, clap |

---

## 4. 数据模型层 (crates/models)

**文件**: `crates/models/src/lib.rs` (~1095 行)

### 4.1 核心结构体

| 结构体 | 用途 | 关键字段 |
|---|---|---|
| `Document` | 导入文档元数据 | id, file_path, file_hash, status, domain, original_path, created_at |
| `Chunk` | 文本分块 | id, doc_id, content, token_count, sequence |
| `ChatMessage` | 对话消息 | id, conversation_id, role, content, sources, created_at, reasoning |
| `Conversation` | 会话 | id, workspace_id, title, created_at |
| `LlmConfig` | LLM 配置 | api_key, base_url, model, proxy_url |
| `LlmMode` | 推理模式 | Remote, Local |
| `RetrievalResult` | 检索结果 | chunk, doc_id, doc_name, score, sequence |
| `DocStatus` | 文档状态枚举 | Ready, Processing, Failed, Duplicate |
| `SettingsPayload` | 设置载荷 | api_key, base_url, model, hybrid_search, rerank_enabled, hyde_enabled, agent_enabled, vlm_enabled, embedding_model, context_token_limit |
| `CacheStats` | 缓存统计 | exact_hits, semantic_hits, misses, total_queries, hit_rate |
| `CacheSettingsPayload` | 缓存设置 | cache_enabled, cache_ttl_secs, cache_semantic_threshold, cache_privacy_mode |
| `Proposition` | 原子命题 | id, chunk_id, content, sequence |
| `SummaryNode` | RAPTOR 摘要树节点 | id, doc_id, level, content, children_ids |
| `Entity` | 实体 | id, chunk_id, entity_text, entity_type |
| `EntityRelation` | 实体关系（知识图谱边） | subject, relation_type, object, chunk_id, confidence |
| `GraphTriple` | 关系三元组（批量导入用） | subject, relation_type, object, chunk_id, confidence |
| `GraphStats` | 图统计 | entity_count, relation_count, relation_types |
| `GraphPath` | 最短路径 | path (Vec<String>), hops (usize) |
| `GraphCommunity` | 社区检测结果 | communities (HashMap<String, usize>), community_count |
| `MemoryRecord` | 检索记忆记录 | query_type, method, hit_rate, avg_score, count |
| `ModelInfo` | 本地模型信息 | filename, size_bytes, architecture, quantization |
| `ModelDownloadProgressPayload` | 模型下载进度 | Downloading{current,total}, Done, Error |
| `ModelLoadStatusPayload` | 模型加载状态 | loading, ready, error |
| `AgentStepPayload` | Agent 步骤 | iteration, thought, action, observation, is_final |
| `TokenUsage` | Token 用量 | prompt_tokens, completion_tokens, total_tokens |
| `LlmSamplingParams` | 采样参数 | temperature, top_p, top_k, max_tokens, penalty |
| `KvCacheStatus` | KV cache 状态 | enabled, cache_dir, file_count, total_size_bytes |
| `SyncProgressPayload` | 同步进度 | Started, Syncing, Completed |
| `SyncResult` | 同步结果 | added, updated, deleted, skipped, errors |

### 4.2 枚举类型

| 枚举 | 变体 | 用途 |
|---|---|---|
| `DocStatus` | Ready, Processing, Failed, Duplicate | 文档状态 |
| `LlmMode` | Remote, Local | LLM 推理模式 |
| `CacheLevel` | Exact, Semantic, Retrieval | 三级缓存层级 |
| `QueryType` | Factual, Analytical, Code, Conversational | 查询类型分类 |
| `RetrievalMethod` | VectorOnly, Hybrid, HybridRerank, Proposition, Colbert | 检索方法 |

---

## 5. Prompt 构建层 (crates/prompt)

**文件**: `crates/prompt/src/lib.rs`

### 5.1 核心功能

| 函数 | 用途 |
|---|---|
| `build_rag_prompt_segmented()` | 分段式 RAG prompt 构建（Prompt Caching 优化：static_prefix + dynamic_context 分离） |
| `build_agent_prompt()` | ReAct Agent prompt（Thought/Action/Observation 循环） |
| `build_final_rag_prompt()` | Agent 最终答案 RAG prompt |
| `truncate_text()` | 文本截断工具 |

### 5.2 SegmentedPrompt

```rust
pub struct SegmentedPrompt {
    pub static_prefix: String,    // 不变部分（系统指令 + 约束），命中 API prompt cache
    pub dynamic_context: String,  // 可变部分（检索片段 + 用户查询）
}
```

---

## 6. 上下文压缩层 (crates/compact)

**文件**: `crates/compact/src/lib.rs`

### 6.1 CompactionEngine

基于 LLM 的历史对话摘要引擎，替代简单截断。

| 方法 | 用途 |
|---|---|
| `compact(history, max_messages)` | 将超出 `max_messages` 的历史消息通过 LLM 摘要压缩 |
| `bpe()` | 复用 `echomind-core::splitter::bpe()` 的全局 BPE 编码器 |

### 6.2 设计原则

- 依赖 `echomind-core` 的 `LLMProvider` trait 和 `bpe()` tokenizer
- 摘要后的历史以系统消息形式注入，保持上下文连贯性
- 零新依赖：复用已有 LLM 端口

---

## 7. 核心业务逻辑层 (crates/core)

### 7.1 端口 Trait 定义 (`lib.rs`)

十五个端口 Trait 定义整个系统契约：

| Trait | 方法签名 | 用途 |
|---|---|---|
| `Loader` | `async fn load(&self, file_path: &str) -> Result<String>` | 文件 → 纯文本 |
| `Splitter` | `async fn split(&self, text: &str) -> Result<Vec<String>>` | 文本 → 分块 |
| `Embedder` | `async fn embed(&self, text: &str) -> Result<Vec<f32>>` + `embed_batch` | 文本 → 向量 |
| `MultiVectorEmbedder` | `async fn embed_multi(&self, text: &str) -> Result<Vec<Vec<f32>>>` | ColBERT 多向量（Pro） |
| `Storage` | CRUD for documents, chunks, embeddings, conversations, messages, settings; vector_search, keyword_search, pagination, zombie cleanup | 持久化 |
| `LLMProvider` | `async fn chat_stream(...) -> Result<BoxStream<Result<String>>>` + `chat_stream_segmented` + `complete` | 流式 LLM |
| `PromptCompressor` | `async fn compress(&self, text: &str, ratio: f32) -> Result<String>` + `compress_batch` | 上下文压缩 |
| `ResponseCache` | 7 个方法：exact_get/semantic_get/retrieval_get/put/list/clear/stats | 语义缓存 |
| `Retriever` | `async fn retrieve(&self, query: &str, top_k: usize) -> Result<Vec<RetrievalResult>>` | 检索 |
| `Reranker` | `async fn rerank(&self, query, results) -> Result<Vec<RetrievalResult>>` | 重排序 |
| `QueryRewriter` | `async fn rewrite(&self, query: &str) -> Result<String>` | 查询改写（HyDE） |
| `PageRenderer` | `async fn render_page(&self, file_path, page_num, dpi) -> Result<Vec<u8>>` | PDF 页面渲染（Pro） |
| `OcrEngine` | `async fn recognize(&self, image_bytes: &[u8]) -> Result<String>` | OCR（Pro） |
| `VisionLanguageModel` | `async fn describe_image(&self, image_bytes, prompt) -> Result<String>` | VLM 图片理解（Pro） |
| `DomainClassifier` | `async fn classify(&self, text: &str) -> Result<String>` | 领域分类 |

**辅助类型**:
- `NoVlm` — 零大小 `VisionLanguageModel` 实现，返回空字符串（VLM 禁用时使用）
- `NoCompressor` — 空操作 `PromptCompressor` 实现

### 7.2 模块清单

| 模块 | 文件 | 核心类型/函数 | 用途 |
|---|---|---|---|
| `chat` | `chat.rs` | `ChatEngine`, `ChatOutcome` | RAG 查询编排（检索→压缩→Prompt→流式） |
| `agent` | `agent.rs` | `AgentEngine<R, L>`, `AgentStepInfo` | ReAct 代理 RAG 多步推理 |
| `coordinator` | `coordinator.rs` | `CoordinatorEngine<R, L>`, `CoordinatorOutcome` | 多代理协调（Research→Synthesis→Implementation→Verification） |
| `import` | `import.rs` | `ImportService<S>`, `ImportOutcome` | 文件导入 + 索引 + 嵌入管线 |
| `sync` | `sync.rs` | `SyncService<S>` | 文件监听增量同步 |
| `retriever` | `retriever.rs` | `VectorRetriever`, `build_contextual_text`, `expand_neighbors` | 向量检索 + 相邻扩展 |
| `hybrid_retriever` | `hybrid_retriever.rs` | `HybridRetriever`, `rrf_fuse` | 混合检索（向量 + BM25 → RRF） |
| `loader` | `loader.rs` | `MarkdownLoader`, `TextLoader`, `PdfLoader`, `MultimodalPdfLoader` | 文件加载 |
| `splitter` | `splitter.rs` | `TextSplitter`, `bpe()`, `token_window_split` | Token 窗口分块 |
| `semantic_splitter` | `semantic_splitter.rs` | `SemanticSplitter` | 语义分块（段落→句子→子句→Token） |
| `section_aware_splitter` | `section_aware_splitter.rs` | `SectionAwareSplitter` | 章节感知分块 |
| `stream_parse` | `stream_parse.rs` | `SseParser`, `StreamItem`, `parse_openai_payload` | SSE 流式解析 |
| `license` | `license.rs` | `verify_license()`, `verify_license_with_key()` | Ed25519 离线验证 |
| `encryption` | `encryption.rs` | `KeyDerivation`, `SecureString`, `PasswordStrengthChecker`, `BruteForceProtection` | Argon2id KDF + 密码强度 + 暴力破解防护 |
| `privacy` | `privacy.rs` | `PiiRedactor`, `PrivacyManager`, `AuditLogger`, `InMemoryAuditLogger` | PII 检测/脱敏 + 审计日志 |
| `security` | `security.rs` | `SecurityManager`, `ClipboardGuard`, `SecurityState`, `LockReason` | 安全状态机 + 自动锁屏 + 剪贴板清除 |
| `auto_dream` | `auto_dream.rs` | `DreamEngine`, `DreamResult`, `DreamSuggestion` | 后台空闲整理引擎 |
| `domain` | `domain.rs` | `EmbeddingDomainClassifier`, `DOMAINS`, `DOMAIN_KEYWORDS` | 16 领域质心分类 |
| `export` | `export.rs` | `export_conversation_to_markdown()` | 对话导出 |
| `errors` | `errors.rs` | `ERR_*` 常量, `prefix_error()`, `classify_llm_error()` | 统一错误前缀 |
| `cache` | `cache.rs` | `normalize_query`, `query_hash`, `cosine_similarity`, `is_expired` | 缓存逻辑 |
| `prompt_compressor` | `prompt_compressor.rs` | `RuleBasedCompressor`, `NoCompressor` | 规则压缩（停用词 + 信息密度评分） |
| `progressive_injector` | `progressive_injector.rs` | `ProgressiveInjector`, `detect_insufficient_info` | 渐进式上下文注入 |
| `speculative_rag` | `speculative_rag.rs` | `SpeculativeRagEngine<D, L>`, `text_similarity` | Speculative RAG（小模型草稿→大模型验证） |
| `retrieval_memory` | `retrieval_memory.rs` | `RetrievalMemoryEngine`, `QueryType`, `RetrievalMethod`, `classify_query_type` | 自进化检索记忆 |
| `proposition_splitter` | `proposition_splitter.rs` | `PropositionSplitter` | 原子命题分割 + 代词消解 |
| `summary_tree` | `summary_tree.rs` | `SummaryTreeBuilder` | RAPTOR 多级摘要树 |
| `entity_extractor` | `entity_extractor.rs` | `EntityExtractor`, `extract_relations()` | NER 纯规则抽取 + 实体关系抽取（8 种关系类型，零 LLM） |
| `graph_retriever` | `graph_retriever.rs` | `GraphRetriever<S>`, `rrf_fuse_four_way()` | 知识图谱图遍历检索（BFS 最大 2 跳 + 四路 RRF 融合，REQ-RAG-027） |
| `graph_analyzer` | `graph_analyzer.rs` | `GraphAnalyzer` | 图分析引擎（最短路径 BFS + 社区检测 Label Propagation + 度中心性，REQ-RAG-027 AC-32~40） |
| `step_cache` | `step_cache.rs` | `StepCache` trait, `InMemoryStepCache`, `step_cache_key` | Agent/Coordinator 步骤缓存 |
| `audit` | `audit.rs` | `AuditEngine`, `AuditReport`, `ContradictionPair` | 文档一致性审计（Pro） |
| `vlm_prompt` | `vlm_prompt.rs` | `VLM_TIERED_PROMPT` | VLM 分级图表理解提示词（Pro） |

### 7.3 关键设计

#### ChatEngine (`chat.rs`)

```rust
pub struct ChatEngine<R: Retriever, L: LLMProvider> {
    retriever: R,
    llm: L,
    top_k: usize,
    compressor: Option<Arc<dyn PromptCompressor>>,  // 上下文压缩
    compression_ratio: f32,
    progressive_initial_count: Option<usize>,        // 渐进注入
}
```

`ChatEngine.chat()` 流程：
1. 检索 top-k 片段
2. （可选）压缩每个 chunk 内容
3. （可选）渐进式注入初始子集
4. 构建分段式 RAG prompt
5. LLM 流式生成
6. 返回 `ChatOutcome::Answered { answer_stream, sources, progressive_info }` 或 `NoContext`

#### AgentEngine (`agent.rs`)

ReAct 循环：LLM 生成 Thought + Action(s) → 并行工具执行 → Observation → 最多 5 轮 → 降级到标准 RAG。

#### SecurityManager (`security.rs`)

安全状态机：
```
Unencrypted → EncryptedUnlocked →(超时/手动)→ Locked →(密码)→ EncryptedUnlocked
```
- 自动锁屏：空闲超时触发（默认 180s）
- 系统睡眠检测：两次 tick 间隔 >5s 判定为睡眠唤醒
- 暴力破解防护：5 次失败 → 30s 锁定，10 次触发紧急销毁
- 剪贴板清除：复制后定时清除（默认 30s）
- 紧急销毁密码：Argon2id 哈希存储，恒定时间比较

#### KeyDerivation (`encryption.rs`)

- **Argon2id** 参数：内存 64MB, 时间 3 轮, 并行度 4, 输出 32 字节
- **PBKDF2** 向后兼容：HMAC-SHA256, 100K iterations
- **SecureString**: `#[derive(Zeroize)]`，Drop 时物理擦零
- **密码强度**: 5 级（VeryWeak→VeryStrong），含 Top 100 弱密码字典

#### PiiRedactor (`privacy.rs`)

8 类 PII 检测：
| 类型 | 正则 | 脱敏示例 |
|---|---|---|
| Email | `[a-zA-Z0-9._%+-]+@...` | `j***@example.com` |
| Phone (CN) | `1[3-9]\d{9}` | `138****5678` |
| IdCard | `\d{17}[\dXx]` | `110***********1234` |
| BankCard | `\d{16,19}` + Luhn | `6222****1234` |
| IP | `\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}` | `192.168.**.**` |
| IntlPhone | `\+\d{7,15}` | `+86***5678` |
| SSN | `\d{3}-\d{2}-\d{4}` | `XXX-**-XXXX` |
| Passport | `[EGeg]\d{8}` | `E****5678` |

审计日志哈希链：`entry_hash = SHA256(prev_hash || entry_data)`

---

## 8. 基础设施适配层 (crates/infra)

### 8.1 模块清单

| 模块 | 文件 | 核心类型 | 用途 |
|---|---|---|---|
| `sqlite_storage` | `sqlite_storage.rs` | `SqliteStorage`, `IntegrityCheckResult` | SQLite 持久化（WAL + r2d2 池 + FTS5 + SQLCipher） |
| `sqlite_cache` | `sqlite_cache.rs` | `SqliteCache` | 三级语义缓存 SQLite 实现 |
| `local_embedder` | `local_embedder.rs` | `LocalEmbedder`, `DownloadEvent` | fastembed ONNX 嵌入（多模型 + 并行推理池） |
| `local_reranker` | `local_reranker.rs` | `LocalReranker` | bge-reranker-base Cross-Encoder |
| `openai_provider` | `openai_provider.rs` | `OpenAIProvider` | OpenAI 兼容 SSE 流式 LLM |
| `hyde_rewriter` | `hyde_rewriter.rs` | `HydeRewriter` | HyDE 查询改写 |
| `duckduckgo_provider` | `duckduckgo_provider.rs` | `DuckDuckGoProvider` | DuckDuckGo Instant Answer API 网页搜索（REQ-RAG-036） |
| `model_manager` | `model_manager.rs` | `ModelManager`, `RecommendedModel` | GGUF 模型文件管理 |
| `file_watcher` | `file_watcher.rs` | `FileWatcher`, `FileWatcherHandle` | 文件监听（notify + debouncer） |
| `local_logger` | `local_logger.rs` | `LocalLogger` | tracing 结构化日志 + JSON Lines + 轮转 |
| `perf` | `perf.rs` | `PerfReport`, `PerfRunner` | 性能基准框架 |
| `local_llm` (Pro) | `local_llm.rs` | `LocalLlmEngine`, `Quantization`, `KernelMode` | mistral.rs 本地 LLM 推理 |
| `hnsw_index` (Pro) | `hnsw_index.rs` | `HnswIndex` | HNSW 近似最近邻索引 |
| `ocrs_engine` (Pro) | `ocrs_engine.rs` | `OcrsEngine` | ocrs + rten 本地 OCR |
| `pdfium_renderer` (Pro) | `pdfium_renderer.rs` | `PdfiumRenderer` | pdfium PDF 页面渲染 |
| `openai_vision` (Pro) | `openai_vision.rs` | `OpenAIVisionProvider` | OpenAI 兼容 VLM API |
| `colbert_embedder` (Pro) | `colbert_embedder.rs` | `ColbertEmbedder` | ColBERT 多向量嵌入 |
| `onnx_compressor` (Pro) | `onnx_compressor.rs` | `OnnxCompressor` | ONNX 嵌入相似度压缩 |
| `gguf_reader` (Pro) | `gguf_reader.rs` | `GgufFile` | GGUF 文件解析（mmap 零拷贝） |
| `quant_blocks` (Pro) | `quant_blocks.rs` | `BlockQ4_0`, `BlockQ4K`, `BlockQ8_0`, `BlockQ8K` | 量化块结构 + 反量化 |
| `gemv_kernel` (Pro) | `gemv_kernel.rs` | `gemv_dispatch`, `quantize_row_q8_0` | 自研 GEMV 内核 |
| `weight_repack` (Pro) | `weight_repack.rs` | `repack_for_gemv`, `RepackedWeights` | CPU cache 友好权重重排 |
| `layer_prefetch` (Pro) | `layer_prefetch.rs` | `LayerPrefetcher`, `SendPtr` | Layer 级流式预取（madvise） |
| `memory_budget` (Pro) | `memory_budget.rs` | `MemoryBudget`, `get_system_memory` | RAM 预算管理（LRU 驱逐） |
| `kv_cache` (Pro) | `kv_cache.rs` | `KvCacheSnapshot` | KV cache 序列化/反序列化 |

### 8.2 SqliteStorage 关键设计

- **连接池**: r2d2 SqliteConnectionManager
- **PRAGMA**: WAL 模式, foreign_keys ON, busy_timeout 5000ms, cache_size 64MB, mmap_size 256MB
- **向量存储**: BLOB (raw f32 little-endian bytes)
- **向量搜索**: 全表扫描 → Rust 余弦相似度 → 固定容量小顶堆 top-k (O(k) 内存)
- **FTS5**: BM25 全文搜索（`keyword_search`）
- **审计日志**: `AuditLogger` trait 实现，哈希链
- **SQLCipher**: `new_encrypted(key)` → `PRAGMA key` (AES-256-CBC)
- **崩溃恢复**: `cleanup_zombies()` 将 Processing → Failed
- **嵌入缓存**: `embeddings_cache` 表（content_hash → embedding BLOB，避免重复 ONNX 推理）
- **Proposition**: `propositions` 表（id, chunk_id, content, embedding BLOB, sequence）
- **摘要树**: `summary_nodes` 表（id, doc_id, level, content, children_ids）
- **实体**: `entities` 表（id, chunk_id, entity_text, entity_type）
- **检索记忆**: `retrieval_memory` 表（query_type, method, hit_rate, avg_score, count）

### 8.3 OpenAIProvider 关键设计

- **SSE 流式**: `chat_stream` → `BoxStream<Result<String>>`
- **分段式**: `chat_stream_segmented` → 双 system 消息命中 prompt cache
- **base_url 智能拼接**: `resolve_chat_completions_url` 兼容标准/版本号/完整路径三类格式
- **代理支持**: `proxy_url` 非空时配置 HTTP/HTTPS 代理
- **Token 用量**: `usage_handle()` → 流末尾 usage chunk 解析
- **推理内容**: `reasoning_rx_handle()` → DeepSeek R1 等推理模型的 `reasoning_content`
- **超时**: connect 30s, request 300s

### 8.4 LocalEmbedder 关键设计

- **模型**: all-MiniLM-L6-v2 (384-dim, ~30MB)
- **多源容错**: HF 官方 → hf-mirror.com 回退
- **并行推理池**: N 个独立 ONNX 会话，`embed_batch` 分片并行
- **池大小**: `available_parallelism().min(8)`
- **懒加载**: `OnceCell` 首次调用触发模型下载

### 8.5 LocalLlmEngine (Pro) 关键设计

- **引擎**: mistral.rs v0.9.0 (纯 Rust)
- **模型格式**: GGUF (Q4_K_M / Q5_K_M / Q8_0)
- **内核模式**: `KernelMode::MistralRs` (默认) / `KernelMode::CustomGemv` (Phase 3 自研)
- **GPU 加速**: `best_device()` (Metal/CUDA feature flags)
- **PagedAttention**: `MemoryGpuConfig` + `PagedAttentionMetaBuilder`
- **采样参数**: `LlmSamplingParams` (temperature, top_p, top_k, max_tokens, penalty)
- **模型卸载**: `RwLock<Option<Arc<Model>>>`，支持运行时切换
- **流取消**: `CancellationToken`
- **预热**: `warm_up()` 后台加载
- **KV Cache**: `save_kv_cache` / `restore_kv_cache` 跨会话复用

---

## 9. Tauri 应用层 (crates/tauri-app)

### 9.1 模块结构

| 模块 | 文件 | 用途 |
|---|---|---|
| `lib.rs` | `lib.rs` | `run()` 入口 + 99 IPC 命令注册 + 菜单 + 窗口状态 |
| `commands.rs` | `commands.rs` | 所有 IPC 命令实现 + `*_inner` 函数 + `LlmProvider` 枚举 |
| `state.rs` | `state.rs` | `AppState` (Store 模式) + `DreamEngineState` |
| `stores/` | `stores/mod.rs` | 5 个 Store 模块 |
| `settings_registry.rs` | `settings_registry.rs` | 类型安全 Settings (21 枚举变体) |

### 9.2 AppState (Store 模式)

```rust
pub struct AppState {
    pub storage: SqliteStorage,
    pub data_dir: PathBuf,
    pub security_store: SecurityStore,
    pub config_store: ConfigStore,
    pub model_store: ModelStore,
    pub document_store: DocumentStore,
    pub chat_store: ChatStore,
    pub dream_engine: DreamEngineState,
    pub cache: SqliteCache,
    pub step_cache: Arc<InMemoryStepCache>,
    pub compression_ratio: f32,
    pub speculative_enabled: bool,
    pub retrieval_memory_enabled: bool,
    pub _log_guard: Option<LocalLogger>,
}
```

**Store 职责拆分**:

| Store | 职责 |
|---|---|
| `SecurityStore` | SecurityManager + ClipboardGuard |
| `ConfigStore` | llm_config + is_pro + llm_mode |
| `ModelStore` | embedder + reranker + local_llm + model_manager + page_renderer + ocr_engine |
| `DocumentStore` | 导入取消 + 文件监听 |
| `ChatStore` | 会话中断令牌 + 审计取消 |

### 9.3 LlmProvider 枚举

```rust
enum LlmProvider {
    Remote(OpenAIProvider),
    #[cfg(feature = "pro")]
    Local(LocalLlmEngine),
}
```

实现 `LLMProvider` trait，根据 `llm_mode` 运行时切换。`chat_stream_segmented` 显式转发确保 Prompt Caching 优化。

### 9.4 窗口状态持久化

- `restore_window_state()`: 从 settings 表恢复位置/尺寸/最大化
- `save_window_state()`: 关闭时保存到 settings 表
- 最小尺寸约束: 960×640

### 9.5 系统菜单栏

EchoMind(About/Quit) · File(Import Files…) · Edit(Undo/Redo/Cut/Copy/Paste/Select All) · Window(Minimize/Zoom) · Help(About)

---

## 10. 许可证签发 CLI (crates/license-issuer)

### 10.1 命令

| 子命令 | 用途 |
|---|---|
| `genkey --output <dir>` | 生成 Ed25519 密钥对（公钥 + 私钥 base64，私钥权限 0600） |
| `issue --payload <text> --key <privkey_file>` | 用私钥签发 License Key |

### 10.2 License Key 格式

```
base64(payload)-base64(signature)
```

与客户端 `crates/core/src/license.rs::verify_license` 配套。

### 10.3 安全设计

- 私钥仅存在于签发环境
- 私钥文件权限 0600 (Unix)
- 零网络依赖
- 使用 `OsRng` 密码学安全随机数生成器

---

## 11. 前端 (ui/)

### 11.1 架构

- **源码**: `ui/src/*.js` (54 个 ES Module，Session 1-2 新增 zindex/panel-stack/input-keymap/input-history + focus-trap 改造)
- **构建产物**: `ui/index.html` (单文件内联，由 `scripts/build-ui.mjs` esbuild 打包生成)
- **类型检查**: `tsconfig.json` (checkJs: true) + `types/*.d.ts` (dom-ext / tauri / vendor 全局声明)，`npx tsc --noEmit` 检查
- **样式**: `ui/styles/*.css` (tokens.css + typography.css + components.css)
- **i18n**: `ui/locales/{en,zh-CN}.json`
- **无框架**: 原生 Vanilla JS，无 CDN 依赖
- **第三方库**: Tailwind CSS (本地 JIT), marked.js, DOMPurify, highlight.js, KaTeX, Mermaid — 全部本地 vendored

### 11.2 模块清单

| 模块 | 文件 | 用途 |
|---|---|---|
| `state.js` | Observable 状态容器 | getState/setState/get/subscribe/subscribeAll |
| `i18n.js` | 国际化 | t() 函数 + data-i18n 属性系统 |
| `ipc.js` | Tauri IPC 封装 | invoke/listen/openDialog + 按域分组 API |
| `utils.js` | 工具函数 | $, sanitizeError, copyToClipboard, PRESETS |
| `toast.js` | Toast 通知 | toast/toastError/toastSuccess |
| `chat.js` | 对话流核心 | send/stream/finalize/sources + 看门狗 + 事件监听 (拆分后 ~594 行) |
| `chat-render.js` | 对话渲染 | 消息 DOM 渲染 + Markdown 消费 + 流式追加 (从 chat.js 拆出) |
| `settings-local-llm.js` | 本地 LLM 设置 | 模型选择 + 采样参数 + GPU 配置 UI (从 settings.js 拆出) |
| `markdown.js` | Markdown 渲染 | renderMarkdown + renderRichContent (Mermaid/KaTeX) |
| `sidebar.js` | 侧栏 | 文档列表 + 会话列表 + 搜索 + 折叠 |
| `settings.js` | 设置面板 | LLM 配置 + 缓存 + 压缩 + 嵌入模型 |
| `import.js` | 文档导入 | 拖放 + 文件选择 + 进度 + 取消 |
| `paywall.js` | 付费墙 | Pro 激活 + 许可证输入 |
| `wizard.js` | 启动向导 | 预设卡片 + LLM 配置引导 |
| `security.js` | 安全面板 | 加密/锁定/审计日志/PII |
| `command-palette.js` | 命令面板 | ⌘K 快捷搜索 |
| `keyboard.js` | 键盘快捷键 | 全局快捷键绑定 |
| `thinking-panel.js` | 思维链折叠面板 | DeepSeek 风格 CoT 展示 |
| `input-toggles.js` | 输入区 Toggle | 混合搜索/Agent 模式开关 |
| `focus-trap.js` | 焦点陷阱 | WAI-ARIA Dialog Modal Pattern |
| `confirm-dialog.js` | 确认对话框 | 通用确认弹窗 |
| `guards.js` | 守卫函数 | canSend/requireIdle/requirePro |
| `context-bar.js` | 上下文用量条 | Token 用量指示器 |
| `context-menu.js` | 右键菜单 | 自定义上下文菜单 |
| `caret.js` | 光标动画 | 流式生成光标 |
| `scroll-lock.js` | 滚动锁定 | 自动滚动 + 跳转最新 |
| `empty-state.js` | 空状态 | 占位内容 |
| `error-detail.js` | 错误详情 | 分类错误卡片 |
| `slash-commands.js` | 斜杠命令 | /help /clear 等 |
| `doc-mention.js` | 文档提及 | @doc 过滤检索 |
| `regen-carousel.js` | 重新生成轮播 | 多版本回答切换 |
| `feedback.js` | 反馈按钮 | 点赞/点踩 |
| `privacy-indicator.js` | 隐私指示器 | 安全状态图标 |
| `queue-send.js` | 排队发送 | 流式进行中排队下一问 |
| `followup.js` | 追问建议 | 智能推荐下一问 |
| `message-edit.js` | 消息编辑 | 编辑后重新生成分支 |
| `conv-nav.js` | 会话导航 | 会话切换 |
| `action.js` | Action 前端 | ActionRegistry 类（借鉴 Zed） |
| `main.js` | 入口模块 | ES module 导入 + 初始化 |
| `code-executor.js` | 代码执行沙箱面板 | 语言选择 + 代码编辑区 + stdin + 结果展示（REQ-RAG-032） |
| `graph-viewer.js` | 知识图谱可视化 | D3.js v7 force-directed graph + 实体类型图标 + 子图过滤 + 搜索定位 + SVG/PNG 导出 + 布局切换 + 路径分析 + 社区检测着色（REQ-RAG-027） |
| `download-manager.js` | 下载管理器面板 | 暂停/恢复/取消 + 崩溃恢复 + model_download_progress 事件监听（REQ-LLM-004） |
| `workflow-panel.js` | DAG 工作流可视化面板 | 模板 CRUD + 运行 + 进度（REQ-RAG-030） |
| `memory-panel.js` | 持久化记忆管理面板 | 三列 tier 布局 Wing/Hall/Room + promote/delete/clear/pin（REQ-RAG-033） |
| `dream-panel.js` | AutoDream 知识库整理面板 | 触发/中止/历史建议（REQ-RAG-029） |
| `symbol-search.js` | 代码符号搜索面板 | tree-sitter AST 符号搜索 + 重建索引（REQ-RAG-031） |
| `perf-settings.js` | 性能优化设置面板 | 缓存/压缩比/检索记忆/渐进式/Speculative/质量门控/图谱检索 toggle（REQ-PERF-001~012） |
| `retrieval-feedback.js` | 检索反馈 | 检索结果质量反馈 |
| `turn-tree.js` | 对话轮次树 | 多版本回答分支管理 |
| `zindex.js` | Z-index 统一分层 | 12 层级 Z-index 常量 + zClass() 辅助函数（Session 1） |
| `panel-stack.js` | 全局面板栈管理器 | push/remove/closeTop/peek/isOpen/hasOpen/clear，ESC 键统一关闭栈顶面板（Session 1） |
| `input-keymap.js` | 统一输入 keydown 处理器 | 斜杠命令导航 > Enter 发送 > 历史导航 > Escape 重置（Session 2） |
| `input-history.js` | 输入历史 + 草稿持久化 + Token 估算 | 会话级历史导航（最多 50 条）+ 切换会话保存/恢复草稿 + ~N tokens 估算（Session 2） |

### 11.3 状态管理 (state.js)

Observable 模式，替代全局可变变量：

```javascript
const initialState = {
  history: [],              // 多轮上下文
  currentRawMarkdown: '',    // 流式累积原文
  streaming: false,         // 流式状态
  currentConversationId: null,
  isPro: false,
  securityState: 'unencrypted',
  // ... 25+ 字段
};

// API: getState() / setState(partial) / get(key) / subscribe(key, cb) / subscribeAll(cb)
```

### 11.4 构建脚本 (scripts/build-ui.mjs)

双模式构建，使用 **esbuild** 打包：
- **生产模式** (默认): CSS → 内联 `<style>`, JS → esbuild 从 `src/main.js` 打包为 IIFE → 内联 `<script>`, 输出单文件 `ui/index.html`
- **开发模式** (`--dev`): 外部 `<link>` + `<script type="module">`, 支持浏览器原生 ES module 热加载

esbuild 自动解析 ES module import/export，tree-shake 无用导出，无需手写 MODULE_ORDER 拓扑序。

---

## 12. 构建系统

### 12.1 命令一览

| 操作 | 命令 |
|---|---|
| 构建全部 | `cargo build` |
| 构建 (Pro) | `cargo build --release --features pro` |
| 运行 (热重载) | `cargo tauri dev` |
| 测试全部 | `cargo test` |
| 测试 (Pro) | `cargo test --features pro` |
| 聚焦测试 | `cargo test <name>` 或 `cargo test -p <crate>` |
| Lint | `cargo clippy --all-targets -- -D warnings` |
| Lint (Pro) | `cargo clippy --all-targets --features pro -- -D warnings` |
| 格式检查 | `cargo fmt --check` |
| 前端构建 | `node scripts/build-ui.mjs` (esbuild 打包 IIFE 生产) / `--dev` (外部引用开发) |
| UI 类型检查 | `npx tsc --noEmit` (tsconfig.json + types/*.d.ts) |
| UI 单元测试 | `cd e2e-tests && npm install && npm run test:unit` (Vitest, 389 tests) |
| E2E 桥接 | `cd e2e-tests && npm install && npm run test:bridge` (Playwright) |
| E2E 真实 GUI | `cd e2e-tests && npm run test:e2e` (WebdriverIO, macOS 不支持) |
| Chaos 测试 | `python3 tests/fixtures/generate_corpus.py && cargo test --workspace -- --nocapture --include-ignored` |
| 供应链安全 | `cargo audit` + `cargo deny check` (根目录 `deny.toml`) |
| GitHub 同步 | `./sync-to-github.sh` |

### 12.2 Profile

```toml
[profile.dev.package."*"]
opt-level = 3  # ML 依赖以 opt-level 3 编译
```

### 12.3 tauri.conf.json 关键配置

- 窗口最小尺寸: 960×640
- 数据目录: `app_data_dir` (可通过 `ECHOMIND_DATA_DIR` 覆盖)
- 权限: Unix 下数据目录 `0700`

---

## 13. 测试体系

### 13.1 测试层级

| 层级 | 工具 | 数量 | 覆盖范围 |
|---|---|---|---|
| Rust 单元测试 | `cargo test` | 718 (Free) / 1003 (Pro) | 全部 crate 模块 |
| Rust 集成测试 | `crates/tauri-app/tests/integration.rs` | 125 | IPC 命令端到端 |
| 前端单元测试 | Vitest | 419 (27 files) | `ui/src/*.js` 模块（Session 3 修复 3 失败） |
| E2E 桥接测试 | Playwright + tauri-stub.js | 986 | 前端 + Mock 后端 |
| E2E 真实 GUI | WebdriverIO + tauri-driver | - | 全栈（macOS 不支持） |
| 性能基准 | `#[ignore]` 测试 | 5 | TC-BENCH-001~005 |
| Chaos 测试 | `tests/fixtures/` | - | 边界/异常场景 |

### 13.2 E2E Mock 机制

`e2e-tests/tests/bridge/tauri-stub.js` 通过 `page.addInitScript` 注入 `window.__TAURI__` mock，无需环境变量开关。`E2E_SPEED` 倍率缩放 mock 延迟（CI 用 0.2）。

### 13.3 性能基准测试 (TC-BENCH-001~005)

| ID | 测试内容 | 优化效果 |
|---|---|---|
| TC-BENCH-001 | Token 消耗对比 | 3350-8200 → 800-2000 (↓50-75%) |
| TC-BENCH-002 | 检索 Hit Rate | 0.35 → 0.55 (↑57%) |
| TC-BENCH-003 | 首 Token 延迟 | Speculative RAG 草稿接受率 |
| TC-BENCH-004 | 缓存命中率 | L0/L1/L3 三级统计 |
| TC-BENCH-005 | 总延迟 | Progressive Injection 注入统计 |

---

## 14. 数据库 Schema

### 14.1 表结构（23 张表）

```sql
-- 核心表（6 张）
CREATE TABLE documents (id, file_path, file_hash, status, status_reason, created_at, original_path, domain, summary, tags);
CREATE TABLE chunks (id, doc_id, content, token_count, sequence);  -- ON DELETE CASCADE
CREATE TABLE embeddings (chunk_id, vector BLOB);  -- ON DELETE CASCADE
CREATE TABLE settings (key, value);
CREATE TABLE conversations (id, workspace_id, title, created_at);
CREATE TABLE messages (id, conversation_id, role, content, sources, reasoning, turn_group, version, created_at, security_tainted);  -- ON DELETE CASCADE

-- FTS5 虚拟表（2 张）
CREATE VIRTUAL TABLE chunks_fts USING fts5(content, doc_id UNINDEXED, chunk_id UNINDEXED, tokenize='trigram');
CREATE VIRTUAL TABLE messages_fts USING fts5(content, conversation_id UNINDEXED, message_id UNINDEXED, tokenize='trigram');

-- 性能优化表（6 张）
CREATE TABLE embeddings_cache (content_hash, embedding BLOB);  -- 嵌入去重
CREATE TABLE entities (id, chunk_id, entity_text, entity_type);  -- 实体索引 (REQ-PERF-006)
CREATE TABLE propositions (id, chunk_id, content, embedding BLOB, sequence);  -- 原子命题 (REQ-PERF-007)
CREATE TABLE summary_nodes (id, doc_id, level, content, child_ids, embedding BLOB);  -- RAPTOR 摘要树 (REQ-PERF-009)
CREATE TABLE retrieval_memory (query_type, method, hit_count, miss_count, avg_score);  -- 检索记忆 (REQ-PERF-012)
CREATE TABLE entity_relations (id, subject, relation_type, object, chunk_id, confidence);  -- 知识图谱边 (REQ-RAG-027)

-- 代码符号索引（1 张）
CREATE TABLE code_symbols (id, chunk_id, name, kind, language, start_line, end_line, signature);  -- (REQ-RAG-031, Pro)

-- 记忆与 Wiki（2 张）
CREATE TABLE memory_entries (id, tier, content, source, conversation_id, created_at, last_accessed, access_count, importance);  -- (REQ-RAG-033)
CREATE TABLE wiki_links (id, source_doc_id, target, chunk_id, created_at);  -- (REQ-ING-020)

-- 对话分支与待办（2 张）
CREATE TABLE turn_active_versions (conversation_id, turn_group, active_version);  -- (REQ-RAG-039)
CREATE TABLE pending_inputs (id, conversation_id, content, delivery, created_at, promoted_seq);  -- (B05)

-- QM 借鉴新表（4 张）
CREATE TABLE session_todos (id, conversation_id, content, status, priority, position, created_at);  -- (B08)
CREATE TABLE scratch_logs (id, date, content, created_at);  -- (Q01 Scratch-Promote)
CREATE TABLE idempotency_records (key, timestamp);  -- (Q03 幂等性)
CREATE TABLE budget_records (id AUTOINCREMENT, principal, timestamp, input_tokens, output_tokens, cost_usd, model_name);  -- (Q08 预算追踪)

-- 审计表（1 张）
CREATE TABLE audit_log (id, action, details, pii_count, timestamp, prev_hash, curr_hash);  -- 哈希链 (REQ-SEC-015)

-- 缓存表（3 张，SqliteCache 独立表）
CREATE TABLE exact_cache (query_hash, response, created_at);
CREATE TABLE semantic_cache (query_hash, query_embedding BLOB, response, created_at);
CREATE TABLE retrieval_cache (query_hash, query_embedding BLOB, results_json, created_at);
```

### 14.2 索引

- `idx_chunks_doc_id` — chunks.doc_id
- `idx_messages_conv` — messages.conversation_id
- `idx_documents_domain` — documents.domain
- FTS5 virtual table on chunks content (BM25)

### 14.3 PRAGMA

```sql
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;
PRAGMA cache_size = -65536;      -- 64MB page cache
PRAGMA mmap_size = 268435456;    -- 256MB mmap
PRAGMA wal_autocheckpoint = 10000;
PRAGMA temp_store = MEMORY;
```

---

## 15. IPC 命令清单

### 15.1 命令分类 (187 个)

| 分类 | 命令数 | 代表命令 |
|---|---|---|
| 文档导入 | 3 | import_files, get_file_sizes, get_file_size_limits |
| 对话 | 5 | chat, create_conversation, get_conversations, get_messages, delete_conversation |
| 消息分页 | 2 | get_conversations_paginated, get_messages_paginated |
| 中断 | 2 | abort_chat, abort_import |
| LLM 配置 | 3 | test_llm_connection, update_llm_config, get_settings |
| 检索增强 | 5 | set_hybrid_search, set_rerank_enabled, set_hyde_enabled, set_agent_enabled, set_coordinator_mode |
| 嵌入模型 | 2 | set_embedding_model, set_context_token_limit |
| 本地 LLM (Pro) | 8 | list_local_models, get_recommended_models, download_model, delete_model, set_llm_mode, get_llm_mode, set_local_model, set_kernel_mode, get_kernel_mode |
| KV Cache (Pro) | 5 | save_kv_cache, load_kv_cache, clear_kv_cache, get_kv_cache_status, set_kv_cache_enabled |
| 采样参数 (Pro) | 3 | get_local_llm_device_kind, set_paged_attn, set_sampling_params |
| 文档管理 | 4 | get_documents, delete_document, retry_index, reclassify_document |
| 授权 | 4 | activate_pro, deactivate_pro, get_pro_status, audit_document |
| 模型缓存 | 3 | init_embedder, get_model_cache_info, clear_model_cache |
| 国际化 | 2 | get_locale, set_locale |
| 侧栏 | 2 | get_sidebar_collapsed, set_sidebar_collapsed |
| 导出 | 2 | export_conversation_markdown, save_text_file |
| 文件同步 | 3 | add_watched_folder, remove_watched_folder, get_watched_folders |
| 安全 | 13 | get_security_status, encrypt_database, unlock_database, lock_app, set_auto_lock_config, detect_pii, set_panic_wipe_password, clear_panic_wipe_password, is_panic_wipe_enabled, set_clipboard_config, get_audit_logs, clear_audit_logs, check_password_strength |
| 可观测性 | 4 | get_log_level, set_log_level, export_logs, export_diagnostics |
| 数据目录 | 1 | open_data_dir |
| Token 预算 | 2 | get_conversation_cost, set_token_budget |
| 语义缓存 | 4 | get_cache_stats, clear_cache, set_cache_settings, get_cache_settings |
| Prompt 压缩 | 2 | set_compression_ratio, get_compression_ratio |
| 性能优化 | 4 | rebuild_bm25_index, rebuild_proposition_index, build_summary_tree, set_embedder_model (Pro) |
| Auto Dream | 3 | trigger_dream, get_dream_suggestions, abort_dream |
| 检索记忆 | 3 | set_retrieval_memory_enabled, get_retrieval_memory_stats, reset_retrieval_memory |
| 知识图谱 | 7 | set_graph_retriever_enabled, get_graph_data, get_entity_relations, get_graph_stats, get_entity_types, get_shortest_path, get_communities, get_graph_layout |

### 15.2 Tauri 事件清单

| 事件 | 载荷 | 用途 |
|---|---|---|
| `chat_token` | String | LLM 流式 token |
| `chat_reasoning` | String | 推理内容增量（reasoning_content） |
| `chat_done` | - | 流结束 |
| `chat_error` | String | 流错误 |
| `chat_sources` | Vec<RetrievalResult> | 引用来源 |
| `chat_phase` | ChatPhasePayload | 进度阶段（preparing/retrieving/generating） |
| `agent_step` | AgentStepPayload | Agent 步骤 |
| `coordinator_phase` | CoordinatorPhaseInfo | Coordinator 阶段 |
| `doc_status_changed` | DocStatusPayload | 文档状态变更 |
| `embedding_progress` | EmbeddingProgressPayload | 嵌入进度 |
| `import_progress` | ImportProgressPayload | 导入进度 |
| `model_download_progress` | ModelDownloadProgressPayload | 模型下载进度 |
| `model_load_progress` | ModelLoadStatusPayload | 模型加载状态 |
| `security_state_changed` | String | 安全状态变更 |
| `sync_progress` | SyncProgressPayload | 同步进度 |
| `db-integrity-error` | String | 数据库完整性错误 |

---

## 16. 设置键清单

| 键 | 类型 | 默认值 | 用途 |
|---|---|---|---|
| `llm.api_key` | String | - | LLM API Key |
| `llm.base_url` | String | - | LLM API base URL |
| `llm.model` | String | - | LLM 模型名 |
| `llm.proxy_url` | String | - | HTTP/HTTPS 代理 |
| `llm.mode` | String | "remote" | 推理模式 (remote/local) |
| `llm.local_model` | String | - | 本地模型文件名 |
| `llm.kernel_mode` | String | "mistral" | 推理内核模式 |
| `llm.paged_attn` | String | "false" | PagedAttention 开关 |
| `llm.block_size` | String | "128" | PagedAttention 块大小 |
| `llm.gpu_memory_ctx` | String | - | GPU 显存上下文 |
| `llm.sampling` | JSON | - | 采样参数 |
| `llm.kv_cache_enabled` | String | "false" | KV cache 跨会话复用 |
| `rag.hybrid_search` | String | "true" | 混合检索开关 |
| `rag.rerank_enabled` | String | "false" | 重排序开关 |
| `rag.hyde_enabled` | String | "false" | HyDE 查询改写开关 |
| `rag.agent_enabled` | String | "false" | Agent 模式开关 |
| `rag.coordinator_mode` | String | "false" | Coordinator 模式开关 |
| `rag.progressive_injection` | String | "false" | 渐进式注入开关 |
| `rag.speculative_enabled` | String | "false" | Speculative RAG 开关 |
| `rag.retrieval_memory_enabled` | String | "false" | 检索记忆开关 |
| `rag.graph_retriever_enabled` | String | "false" | 知识图谱图遍历检索开关 |
| `rag.context_token_limit` | String | "8000" | 上下文 token 上限 |
| `rag.embedding_model` | String | default | 嵌入模型名 |
| `mm.vlm_enabled` | String | "false" | VLM 图片理解开关 |
| `compression.ratio` | String | "1.0" | Prompt 压缩比 |
| `cache.enabled` | String | "true" | 缓存开关 |
| `cache.ttl_secs` | String | "3600" | 缓存 TTL |
| `cache.semantic_threshold` | String | "0.92" | 语义相似度阈值 |
| `cache.privacy_mode` | String | "true" | 缓存隐私模式 |
| `ui.locale` | String | "en" | 界面语言 |
| `window.x/y/width/height/maximized` | String | - | 窗口状态 |
| `sidebar.collapsed` | String | "false" | 侧栏折叠 |
| `security.auto_lock_timeout` | String | "180" | 自动锁屏超时 |
| `security.clipboard_clear_timeout` | String | "30" | 剪贴板清除超时 |
| `security.panic_wipe.enabled` | String | "false" | 紧急销毁开关 |
| `security.panic_wipe.password_hash` | String | - | 紧急销毁密码哈希 |
| `security.panic_wipe.salt_hex` | String | - | 紧急销毁 salt |
| `rag.contextual_retrieval` | String | "true" | Contextual Retrieval 开关 (REQ-RAG-041) |
| `rag.quality_gate_enabled` | String | "false" | RAG 质量门控开关 |
| `rag.web_search_enabled` | String | "false" | 网页搜索集成开关 |
| `rag.sub_agent_enabled` | String | "false" | 子代理舰队开关 |
| `memory.enabled` | String | "false" | 持久化记忆系统开关 |
| `security.posture` | String | "auto" | 安全态势分层 (Q05) |
| `security.auto_lock_enabled` | String | "true" | 自动锁定开关 |
| `security.lock_on_sleep` | String | "true" | 休眠时自动锁定 |
| `security.clipboard_clear_enabled` | String | "true" | 剪贴板自动清除开关 |
| `ui.theme` | String | "dark" | 界面主题模式 |
| `stt.api_key` | String | - | 语音转写 API Key |
| `stt.base_url` | String | - | 语音转写 API 端点 |
| `stt.model` | String | "whisper-1" | 语音转写模型名 |
| `log.level` | String | "info" | 日志级别 |
| `license.is_pro` | String | "false" | Pro 授权状态 |

---

## 17. 安全架构

### 17.1 五层纵深防御

| 层 | 措施 | 实现 |
|---|---|---|
| 第一层 | 数据库加密 | SQLCipher AES-256-CBC + Argon2id KDF (64MB/3轮/4并行) |
| 第二层 | 密码安全 | SecureString (zeroize) + PBKDF2 向后兼容 + 密码强度检测 |
| 第三层 | 自动锁屏 + IPC 拦截 | SecurityManager 状态机 + 空闲超时 + 系统睡眠检测 |
| 第四层 | 内存擦零 | zeroize crate 物理擦零敏感数据 |
| 第五层 | 剪贴板清除 | ClipboardGuard 定时清除 (macOS: pbcopy, Linux: xclip) |

### 17.2 安全状态机

```
Unencrypted →(encrypt_database)→ EncryptedUnlocked →(timeout/manual/sleep)→ Locked
Locked →(unlock with correct password)→ EncryptedUnlocked
Locked →(5 failed attempts)→ 30s lockout → exponential backoff
Locked →(10 failed attempts + panic_wipe)→ 紧急销毁
```

### 17.3 PII 检测与脱敏

8 类 PII：Email, Phone (CN), IdCard, BankCard (Luhn), IP, IntlPhone, SSN, Passport。正则匹配 + 脱敏替换 + 审计日志。

### 17.4 审计日志哈希链

```
entry_hash = SHA256(prev_hash || entry_data)
```

删除任意一条日志导致后续验证失败。`verify_audit_chain()` 回放哈希链检测篡改。

### 17.5 许可证验证

- Ed25519 签名验证（离线）
- License Key 格式: `base64(payload)-base64(signature)`
- 内置公钥（32 字节），私钥不在二进制中
- 错误消息不泄露校验内部细节

---

## 18. Pro 功能门控

### 18.1 `#[cfg(feature = "pro")]` 条件编译

| 层 | Pro 模块 |
|---|---|
| Core | `audit` (文档一致性审计), `vlm_prompt` (VLM 分级提示词) |
| Infra | `local_llm`, `hnsw_index`, `ocrs_engine`, `openai_vision`, `pdfium_renderer`, `colbert_embedder`, `onnx_compressor`, `gguf_reader`, `quant_blocks`, `gemv_kernel`, `weight_repack`, `layer_prefetch`, `memory_budget`, `kv_cache` |
| Tauri-app | `get_local_llm_device_kind`, `set_paged_attn`, `set_sampling_params`, `set_kernel_mode`, `get_kernel_mode`, `save_kv_cache`, `load_kv_cache`, `clear_kv_cache`, `get_kv_cache_status`, `set_kv_cache_enabled`, `set_embedder_model` |

### 18.2 GPU Feature Flags

| Feature | 用途 |
|---|---|
| `metal` | Apple Metal GPU 加速 |
| `cuda` | NVIDIA CUDA GPU 加速 |
| `accelerate` | Apple BLAS 加速 |
| `flash-attn` | Flash Attention 2 (需要 cuda) |
| `cudnn` | cuDNN (需要 cuda) |

### 18.3 Free / Pro 差异

| 特性 | Free | Pro |
|---|---|---|
| 文档数 | 50 个上限 | 无限制 |
| PDF 导入 | 禁止 | 允许（含多模态 OCR + VLM） |
| 本地 LLM | 禁止 | mistral.rs GGUF 推理 |
| HNSW 索引 | 禁止 | 允许 |
| ColBERT 多向量 | 禁止 | 允许 |
| ONNX 压缩 | 禁止 | 允许 |
| 文档审计 | 禁止 | 允许 |
| 自研 GEMV 内核 | 禁止 | 允许 |
| KV Cache 跨会话 | 禁止 | 允许 |

---

## 19. 数据流图

### 19.1 文档导入

```
import_files IPC → ImportService.import_one()
  → Loader.load() (Markdown/Text/PDF; PDF 多模态: pdfium 渲染 + ocrs OCR + 可选 VLM)
  → MD5 hash → duplicate check (find_document_by_hash)
  → Splitter.split() (token/semantic/section-aware chunking)
  → Storage.add_document() + add_chunk() (SQLite)
  → Embedder.embed_batch() (fastembed/ONNX, 并行推理池)
  → Storage.add_embedding() (vector BLOB)
  → doc_status_changed event → frontend list update
  → embedding_progress event per micro-batch
```

### 19.2 RAG 查询

```
chat IPC → chat_inner()
  → empty KB check (count_documents == 0 → error)
  → load LLM config (api_key, base_url, model)
  → chat_phase "preparing" event → init embedder (lazy, first-use download)
  → create/update conversation (lazy, first-message creates)
  → ChatEngine.chat()
    → HybridRetriever.retrieve() (if hybrid_search enabled)
      → Embedder.embed(query) → Storage.vector_search(cosine sim, top-k=8)
      → Storage.keyword_search(BM25, top-k=5)
      → rrf_fuse() merge
    → threshold filter (score > 0.1)
    → chunk expansion: expand to adjacent chunks (±1)
    → (可选) PromptCompressor.compress() each chunk
    → (可选) ProgressiveInjector: inject initial subset
    → build_rag_prompt_segmented() → LLMProvider.chat_stream_segmented(SSE)
  → chat_sources event → frontend shows references
  → chat_phase "generating" event
  → chat_token events → frontend typing effect
  → chat_reasoning events → frontend thinking panel (DeepSeek R1)
  → abort_token check (CancellationToken)
  → chat_done event
  → persist_exchange() → Storage.add_message() (user + assistant)
  → auto-extract title on first exchange (first 24 chars + …)
```

### 19.3 Agent RAG (ReAct)

```
AgentEngine.chat() — ReAct loop:
  → LLM generates Thought + Action(s) (or Final Answer)
  → parallel tool execution (search_kb / list_documents) via join_all
  → Observation(s) feed next round; max 5 iterations
  → parse failure → degrade to standard RAG single retrieval
  → each step emits agent_step event
```

### 19.4 Stream Cancellation

```
abort_chat IPC → AppState.abort_chat()
  → CancellationToken.cancel()
  → forward_stream() loop breaks on select! biased branch
  → emit "⏹ 生成已中断" + chat_done
  → persist_exchange() saves partial content
  → clear_abort() cleans up token
```

### 19.5 Security & Encryption

```
encrypt_database IPC → SecurityManager.encrypt_database(password)
  → Argon2id derive_key(password, random_salt) → 32-byte key
  → SqliteStorage new_encrypted(key) → PRAGMA key (SQLCipher AES-256)
  → state: Unencrypted → EncryptedUnlocked
  → SecureString wipe password from memory (zeroize)

unlock_database IPC → SecurityManager.unlock(password)
  → Argon2id verify → PRAGMA key
  → brute-force check (5 attempts → 30s backoff)
  → state: Locked → EncryptedUnlocked

Auto-lock (idle) → SecurityManager.check_auto_lock()
  → elapsed > auto_lock_timeout → state: EncryptedUnlocked → Locked
  → emit security_state_changed event → frontend lock overlay
```

### 19.6 知识图谱（实体关系抽取 → 图遍历检索 → 可视化）

```
导入管线集成:
import_files → ImportService.import_one()
  → Loader.load() → Splitter.split()
  → Storage.add_document() + add_chunk()
  → Embedder.embed_batch() → Storage.add_embedding()
  → EntityExtractor.extract(text) → Storage.add_entities()           ← 实体索引
  → EntityExtractor.extract_relations(text, chunk_id)                 ← 关系抽取（8 种）
  → Storage.add_relations_batch()                                      ← entity_relations 表

检索管线集成:
chat IPC → chat_inner()
  → (if rag.graph_retriever_enabled)
  → GraphRetriever.expand(query, top_k=5)
    → EntityExtractor.extract(query) → query_entities
    → BFS: Storage.get_relations_for_entity(entity)
      → 1-hop: confidence × 1.0, 2-hop: confidence × 0.5 (distance decay)
    → Storage.get_chunk_by_id(chunk_id) → RetrievalResult
  → rrf_fuse_four_way(vector, keyword, entity, graph, top_k)
    → Vector=1.0, BM25=0.8, Entity=0.6, Graph=0.5

可视化与分析:
openGraphViewer → get_graph_data() → D3 forceSimulation
  → 节点拖拽/缩放/高亮 + 8 种关系颜色编码
  → 子图过滤（关系类型复选框）
  → 搜索定位（zoom transform 居中+缩放+高亮）
  → SVG/PNG 导出
  → 布局切换（force/hierarchical/radial）
  → 路径分析（BFS 最短路径 + 高亮）
  → 社区检测（Label Propagation + 10 色着色）
```

### 19.7 Local LLM (Pro)

```
set_llm_mode IPC → AppState.set_llm_mode("local")
  → persist to settings table (llm.mode = "local")
  → Free user: reject with PRO_REQUIRED error
  → tokio::spawn(engine.warm_up()) 后台预热模型

chat IPC (LlmMode::Local) → chat_inner()
  → load selected model filename from settings (llm.local_model)
  → AppState.local_llm() → LocalLlmEngine::new(model_path)
    → emit model_load_progress { status: "loading" }
    → mistral.rs load GGUF (spawn_blocking, first call only)
    → emit model_load_progress { status: "ready" | "error" }
  → LocalLlmEngine.chat_stream() → BoxStream<Result<String>>
    → same forward_stream() loop as OpenAIProvider
```

---

## 20. 依赖清单

### 20.1 核心 Rust 依赖

| 依赖 | 版本 | 用途 |
|---|---|---|
| `tauri` | v2 | 桌面应用框架 |
| `tokio` | 1.x | 异步运行时 |
| `rusqlite` | 0.31+ (bundled) | SQLite 数据库 |
| `r2d2` + `r2d2_sqlite` | - | 连接池 |
| `fastembed` | - | ONNX 嵌入模型 |
| `reqwest` | 0.12+ | HTTP 客户端 (SSE 流式) |
| `pulldown-cmark` | - | Markdown 解析 |
| `tiktoken-rs` | - | BPE tokenizer (cl100k_base) |
| `argon2` | - | Argon2id KDF |
| `zeroize` | - | 敏感数据擦零 |
| `subtle` | - | 恒定时间比较 |
| `ed25519-dalek` | 2.x | Ed25519 签名验证 |
| `base64` | - | Base64 编解码 |
| `serde` / `serde_json` | - | 序列化 |
| `uuid` | - | UUID 生成 |
| `chrono` | - | 时间处理 |
| `regex` | - | PII 正则匹配 |
| `md-5` | - | MD5 哈希 (文档去重) |
| `futures` | - | 异步流 (BoxStream) |
| `tokio-util` | - | CancellationToken |
| `tracing` | - | 结构化日志 |
| `anyhow` | - | 错误处理 |
| `lopdf` | - | PDF 文本提取 |
| `notify` + `notify-debouncer-full` | - | 文件监听 |

### 20.2 Pro 依赖

| 依赖 | 版本 | 用途 |
|---|---|---|
| `mistral.rs` | v0.9.0 | 本地 LLM 推理 |
| `pdfium-render` | - | PDF 页面渲染 |
| `ocrs` + `rten` | - | 本地 OCR |
| `image` | - | 图像处理 |
| `hnsw_rs` | - | HNSW 近似最近邻 |
| `memmap2` | - | mmap GGUF |
| `half` | - | f16 支持 |
| `libc` | - | sysconf / madvise |

### 20.3 前端依赖

| 依赖 | 来源 | 用途 |
|---|---|---|
| Tailwind CSS | 本地 JIT | 样式框架 |
| marked.js | `ui/vendor/` | Markdown 渲染 |
| DOMPurify | `ui/vendor/` | XSS 防护 |
| highlight.js | `ui/vendor/` | 代码高亮 |
| KaTeX | `ui/vendor/katex/` | LaTeX 公式 |
| Mermaid | `ui/vendor/` | 图表渲染 |

### 20.4 开发依赖

| 依赖 | 用途 |
|---|---|
| `tempfile` | 测试临时目录 |
| `clap` | CLI 参数解析 (license-issuer) |

---

## 21. 文档同步规则

### 21.1 代码变更必须同步的文档

| 代码变更类型 | 必须同步的文档 |
|---|---|
| 新增/修改 IPC 命令 | `docs/api/ipc-commands.md` + 本文档 §15 |
| 新增/修改数据库表 | `docs/api/database-schema.md` + 本文档 §14 |
| 新增/修改 trait 方法、数据模型、AppState 字段 | `docs/api/code-contracts.md` + 本文档相关章节 |
| 新增前端 JS 文件 | `docs/api/frontend-modules.md` + 本文档 §11 |
| 新增 settings 键 / Tauri 事件 | `docs/api/settings-reference.md` + 本文档 §16 |
| 功能实现/补齐 | `docs/SRS_v1.0.md` 对应 REQ 状态 + 本文档 |
| 新增架构决策 | `docs/architecture/adr/` 新建 ADR |
| 新增/修改源文件 | 本文档对应章节 |

### 21.2 GitHub 发布同步

双仓库工作流：开发在根目录，`EchoMind/EchoMind/` 是只含打包编译必需文件的 GitHub 发布仓库。修改任何 `.rs` / `.html` / `.css` / `.js` / `.json` / `.toml` 源文件后，必须运行 `./sync-to-github.sh` 同步。

不同步的文件：`docs/`、`tests/fixtures/`、`e2e-tests/`、`.catpaw/`、`.githooks/`、`PROJECT_RULES.md`、`ui/src/` 等开发专属资产。

---

## 22. v1.1 路线图（2026-08-06 规划）

### 22.1 概述

v1.1 是 v1.0.0 发布后的第一个功能迭代版本。通过 ROI 评估（用户需求度 × 技术可行性 / 预计天数）从 12 项 v1.0 范围外功能中选出 Top 5，预计 4.5 天完成全部实现。

### 22.2 新增功能清单

| Session | 功能 | REQ | 依赖 | 预计耗时 |
|---|---|---|---|---|
| S44 | .docx 导入 | REQ-ING-015 | docx-rs v0.4.22 | 0.5 天 |
| S45 | .html 导入 | REQ-ING-016 | html2text v0.17.1 | 0.5 天 |
| S46 | 浅色主题 | REQ-UI-011 | 无（纯 CSS + JS） | 1 天 |
| S47 | 语音输入 + TTS | REQ-RAG-034/035 | 无（Web Speech API） | 1 天 |
| S48 | 网页搜索集成 | REQ-RAG-036 | 无（复用 reqwest） | 1.5 天 |

### 22.3 新增依赖

| 依赖 | 版本 | 用途 | crate | 确认方式 |
|---|---|---|---|---|
| `docx-rs` | `0.4.22` | .docx 文件解析 | crates/core | `cargo search docx-rs`（2026-08-06） |
| `html2text` | `0.17.1` | HTML → 纯文本转换 | crates/core | `cargo search html2text`（2026-08-06） |

### 22.4 新增 IPC 命令

| 命令 | Session | 说明 |
|---|---|---|
| `set_theme` | S46 | 设置主题模式 |
| `get_theme` | S46 | 获取当前主题 |
| `set_web_search_enabled` | S48 | 网页搜索开关 |
| `web_search` | S48 | 手动触发搜索 |

### 22.5 新增设置键

| 设置键 | 类型 | 默认值 | Session |
|---|---|---|---|
| `ui.theme` | String | `"dark"` | S46 |
| `voice.language` | String | 跟随 UI 语言 | S47 |
| `voice.rate` | f32 | `1.0` | S47 |
| `rag.web_search_enabled` | bool | `false` | S48 |
| `search.engine` | String | `"duckduckgo"` | S48 |
| `search.searxng_url` | String | `""` | S48 |

### 22.6 设计原则

1. **本地优先**：所有新功能优先本地实现，不引入云依赖
2. **向后兼容**：v1.0 用户数据无缝升级到 v1.1
3. **渐进式发布**：每个功能可独立开关（settings 键控制）
4. **安全优先**：语音输入需麦克风权限提示，网页搜索需 opt-in
5. **零 Panic**：遵循铁律五，所有新代码严禁 `unwrap()/expect()/panic!()`

### 22.7 原子计划文档

详见 `docs/architecture/V1.1_ATOMIC_PLAN.md`（S44-S48，含变更清单 + TDD 测试用例 + 验收标准 + 衔接提示词）。

> **v1.1.0 发布状态**（2026-08-06）：✅ 全部 5 个功能 Implemented + ✅ 全量回归通过（Free 749 / Pro 1034 / E2E 999）+ ✅ Release build exit_code 0 + ✅ git tag v1.1.0。

---

## 23. v1.2 路线图（2026-08-06 规划）

### 23.1 概述

v1.2 是 v1.1.0 发布后的第二个功能迭代版本。通过 ROI 评估（用户需求度 × 技术可行性 / 预计天数）从 9 项候选功能中选出 Top 4（ROI ≥ 10 的 3 项 + Pro 战略价值 1 项），预计 5 天完成全部实现。

### 23.2 新增功能清单

| Session | 功能 | REQ | 依赖 | Free/Pro | 预计耗时 |
|---|---|---|---|---|---|
| S49 | PDF 导出 | REQ-EXP-005 | 无（`window.print()`） | Free | 1 天 |
| S50 | .pptx 导入 | REQ-ING-017 | pptx-to-md v1.0.0 | Free | 1 天 |
| S51 | .epub 导入 | REQ-ING-018 | rbook v0.7.10 | Free | 1 天 |
| S52 | 自定义 ONNX 嵌入模型上传 | REQ-VEC-014 | 无（复用 fastembed） | Pro | 2 天 |
| S53 | v1.2 发布准备 | — | — | — | 0.5 天 |

### 23.3 新增依赖

| 依赖 | 版本 | 许可证 | 用途 | crate | 确认方式 |
|---|---|---|---|---|---|
| `pptx-to-md` | `1.0.0` | MIT/Apache-2.0 | .pptx → Markdown 转换 | crates/core | `cargo search pptx`（2026-08-06） |
| `rbook` | `0.7.10` | Apache-2.0 | EPUB 2/3 解析 | crates/core | `cargo search epub`（2026-08-06） |

> 排除 `epub` crate（v2.1.5，GPL-3.0 许可证不兼容 MIT 项目）。

### 23.4 新增 IPC 命令

| 命令 | Session | Free/Pro | 说明 |
|---|---|---|---|
| `upload_custom_embedding_model` | S52 | Pro | 上传自定义 ONNX 嵌入模型 |
| `list_custom_models` | S52 | Pro | 列出自定义模型 |
| `delete_custom_model` | S52 | Pro | 删除自定义模型 |

> IPC 命令总数：187（含 12 个编译时 Pro 门控）。

### 23.5 ALLOWED_EXTENSIONS 变更

```
v1.1（11 项）→ v1.2 S50（12 项）→ v1.2 S51（13 项）：新增 "pptx" + "epub"
```

### 23.6 设计原则

1. **零依赖优先**：PDF 导出使用 `window.print()`，零新增 Rust 依赖
2. **Rust 生态优先**：pptx-to-md / rbook 均为纯 Rust crate（铁律九）
3. **许可证兼容**：排除 GPL-3.0 crate，仅引入 MIT/Apache-2.0
4. **Pro 差异化**：自定义嵌入模型上传为 Pro 门控功能
5. **向后兼容**：v1.1 用户数据无缝升级到 v1.2

### 23.7 原子计划文档

详见 `docs/architecture/V1.2_ATOMIC_PLAN.md`（S49-S53，含 ROI 评分表 + 技术调研 + 变更清单 + TDD 测试用例 + 验收标准 + 衔接提示词）。

---

## 附录 A: 关键文件索引

| 文件 | 行数 | 用途 |
|---|---|---|
| `crates/models/src/lib.rs` | ~1095 | 数据模型定义 |
| `crates/prompt/src/lib.rs` | - | Prompt 构建 |
| `crates/compact/src/lib.rs` | - | 上下文压缩 |
| `crates/core/src/lib.rs` | - | 15 个端口 Trait 定义 |
| `crates/core/src/chat.rs` | - | ChatEngine RAG 编排 |
| `crates/core/src/agent.rs` | - | ReAct Agent |
| `crates/core/src/coordinator.rs` | ~420 | 多代理协调 |
| `crates/core/src/import.rs` | - | 导入管线 |
| `crates/core/src/security.rs` | ~696 | 安全状态机 |
| `crates/core/src/encryption.rs` | ~891 | Argon2id + 密码强度 |
| `crates/core/src/privacy.rs` | ~872 | PII + 审计日志 |
| `crates/core/src/license.rs` | ~56 | Ed25519 验证 |
| `crates/core/src/errors.rs` | ~244 | 统一错误前缀 |
| `crates/core/src/stream_parse.rs` | ~139 | SSE 流式解析 |
| `crates/core/src/loader.rs` | ~278 | 文件加载器 |
| `crates/core/src/splitter.rs` | ~179 | Token 窗口分块 |
| `crates/infra/src/sqlite_storage.rs` | ~2830 | SQLite 持久化 |
| `crates/infra/src/openai_provider.rs` | ~458 | OpenAI 兼容 LLM |
| `crates/infra/src/local_embedder.rs` | ~574 | fastembed ONNX 嵌入 |
| `crates/infra/src/local_llm.rs` | ~991 | mistral.rs 本地 LLM (Pro) |
| `crates/infra/src/model_manager.rs` | ~372 | GGUF 模型管理 |
| `crates/tauri-app/src/lib.rs` | ~400 | Tauri 入口 + 99 命令注册 |
| `crates/tauri-app/src/commands.rs` | ~5097 | IPC 命令实现 |
| `crates/tauri-app/src/state.rs` | ~492 | AppState (Store 模式) |
| `crates/license-issuer/src/lib.rs` | ~388 | License 签发库 |
| `crates/license-issuer/src/main.rs` | ~81 | CLI 入口 |
| `ui/src/main.js` | ~706 | 前端入口 |
| `ui/src/state.js` | ~223 | 状态容器 |
| `ui/src/chat.js` | ~594 | 对话流核心（拆分后） |
| `ui/src/chat-render.js` | ~310 | 对话渲染（从 chat.js 拆出） |
| `ui/src/settings-local-llm.js` | ~180 | 本地 LLM 设置（从 settings.js 拆出） |
| `ui/src/ipc.js` | ~175 | IPC 封装 |
| `scripts/build-ui.mjs` | ~223 | 前端构建脚本（esbuild 打包） |
| `tsconfig.json` | ~33 | TypeScript checkJs 配置 |
| `types/dom-ext.d.ts` | ~78 | DOM 类型扩展（Element/HTMLElement/Node/Event） |
| `Cargo.toml` | ~28 | Workspace 配置 |

---

## 附录 B: ADR (架构决策记录)

| ADR | 标题 | 状态 |
|---|---|---|
| ADR-010 | 多模态 PDF 安全审查 | Accepted |
| ADR-011 | 本地 LLM 引擎 (mistral.rs 选型) | Accepted (Phase 1-4 完成) |
| ADR-012 | 极致性能优化架构 | Accepted |

---

> **维护规则**: 每次代码变更（新增文件、修改接口、变更功能）后，必须在同一 PR 内更新本文档对应章节。文档与代码脱节是最大威胁。
