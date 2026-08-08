# ADR-012: 极致性能优化（RAG Token 消耗与延迟终极优化）

> **状态**：Accepted ｜ **日期**：2026-08-04 ｜ **决策者**：PM / TPM / DEV
>
> **关联文档**：
> - `docs/architecture/EXTREME_PERFORMANCE_OPTIMIZATION_PLAN.md`（调研方案）
> - `docs/architecture/EXTREME_PERFORMANCE_ATOMIC_PLAN.md`（原子级实施规划 S1-S10）
> - `docs/SRS_v1.0.md` §4.26 PERF（REQ-PERF-001~012）

---

## 1. 背景

EchoMind 是面向律师、研究员等高隐私场景的本地 RAG 知识库桌面应用。在极致性能优化前，一次典型 RAG 查询消耗 3350-8200 tokens，检索命中率 65-75%，首 token 延迟 1-2.5s，总延迟 3.5-5s。这些指标在频繁查询场景下影响用户体验和 API 成本。

### 1.1 现状瓶颈

| 维度 | 优化前 | 严重度 |
|---|---|---|
| Token/次（平均） | 3350-8200 | 🔴 严重 |
| Token/次（重复查询） | 3350-8200 | 🔴 严重（无缓存） |
| 检索命中率 | 65-75% | ⚠️ 中等 |
| 首 token 延迟 | 1-2.5s | ⚠️ 中等 |
| 总延迟（平均） | 3.5-5s | 🔴 严重 |

### 1.2 调研来源

系统调研了 18 个论文/开源项目：
- **Anthropic Contextual Retrieval** (2024.09) — 检索失败率 ↓49%/67%
- **Anthropic Prompt Caching** (2024.08) — 成本 ↓90%，延迟 ↓85%
- **LLMLingua-2** (arXiv:2403.12968) — Prompt 压缩 2-5x
- **Self-RAG** (arXiv:2310.11511) — 自主检索决策
- **CRAG** (arXiv:2401.15884) — 检索评估器 + decompose-then-recompose
- **Dense X Retrieval** (arXiv:2312.06648) — Proposition 级原子检索
- **ColBERT** (arXiv:2004.12832) — Late Interaction
- **RAPTOR** — 递归摘要树
- **vLLM** (arXiv:2309.06180) — PagedAttention + Speculative Decoding
- **AutoRAG 2.0** — 自进化记忆系统

---

## 2. 决策

采用 **4 Sprint / 10 Session** 渐进式优化方案，分三个维度同时推进：

### 2.1 Sprint A: Token 消耗极致压缩

| Session | 技术 | 需求 | 核心实现 |
|---|---|---|---|
| S1 | 语义缓存金字塔 | REQ-PERF-001 | L0 精确 + L1 语义 + L3 检索结果三级缓存 |
| S2 | LLMLingua-2 式 Prompt 压缩 | REQ-PERF-002 | RuleBasedCompressor（Free）+ OnnxCompressor（Pro） |
| S3 | CRAG 检索精炼 + Self-RAG 决策 | REQ-PERF-003/004 | 三级置信度评估 + 查询分类器（零 LLM） |

**设计决策**：
- 缓存使用 SQLite（共享 SqliteStorage 连接池），向量存储为 BLOB（f32 little-endian bytes）
- 缓存默认开启，高隐私场景可通过 `privacy_mode` 关闭
- Prompt 压缩仅作用于检索片段，不压缩系统提示和用户查询
- CRAG 精炼使用规则匹配（零 LLM），避免额外推理开销
- Self-RAG 查询分类器使用规则匹配（Simple/Analytical/Comparison/Factual），零 LLM 调用

### 2.2 Sprint B: 检索命中率极致提升

| Session | 技术 | 需求 | 核心实现 |
|---|---|---|---|
| S4 | Contextual BM25 + 实体链接 | REQ-PERF-005/006 | FTS5 上下文前缀 + NER 三路 RRF 融合 |
| S5 | Proposition 级原子分割 | REQ-PERF-007 | 纯规则代词消解 + 上下文补全（零 LLM） |
| S6 | ColBERT + RAPTOR | REQ-PERF-008/009 | Late Interaction 多向量嵌入 + 递归摘要树 |

**设计决策**：
- Contextual BM25 使用 `build_contextual_text()` 拼接文档名前缀（低成本方案）
- 实体抽取使用纯规则+正则（5 类实体），零 LLM/模型依赖
- Proposition 分割使用纯规则方案（句子分割 + 代词消解 + 上下文补全），零 LLM 调用
- ColBERT 多向量嵌入作为 Pro feature（复用 LocalEmbedder ONNX 引擎）
- RAPTOR 摘要树支持 LLM 摘要生成回调 + 增量构建

### 2.3 Sprint C: 推理性能极致优化

| Session | 技术 | 需求 | 核心实现 |
|---|---|---|---|
| S7 | 渐进式上下文注入 | REQ-PERF-010 | ProgressiveInjector 状态机（initial_count=2） |
| S8 | Speculative RAG | REQ-PERF-011 | 草稿模型快速生成 → 大模型验证/修正 |
| S9 | 自进化检索记忆 | REQ-PERF-012 | QueryType × RetrievalMethod 自适应选择 |

**设计决策**：
- 渐进式注入：初始注入 2 个 chunk（可配置），检测到 "信息不足" 关键词时追加，max_rounds=top_k 防无限追加
- Speculative RAG：草稿用简化 prompt（仅 top-1 chunk），验证用完整 prompt + 草稿作为 prefix；基于归一化 Levenshtein 编辑距离计算相似度
- 自进化检索记忆：规则匹配零 LLM 的 4 分类（Factual/Analytical/Code/Conversational），冷启动默认 Hybrid，随使用自动优化
- 所有优化均默认关闭，通过设置键逐个开启，保证向后兼容

### 2.4 Sprint D: 文档同步 + 基准测试

| Session | 内容 |
|---|---|
| S10 | 全量文档同步 + before/after 性能基准对比 |

---

## 3. 架构影响

### 3.1 新增模块

| 模块 | Crate | 说明 |
|---|---|---|
| `cache.rs` | core | 缓存逻辑：归一化 + 哈希 + 语义匹配 |
| `prompt_compressor.rs` | core | PromptCompressor trait + RuleBasedCompressor |
| `progressive_injector.rs` | core | 渐进式注入状态机 |
| `speculative_rag.rs` | core | Speculative RAG 泛型引擎 |
| `retrieval_memory.rs` | core | 自进化检索记忆引擎 |
| `proposition_splitter.rs` | core | Proposition 级原子分割 |
| `summary_tree.rs` | core | RAPTOR 多级摘要树 |
| `entity_extractor.rs` | core | NER 实体抽取器（纯规则） |
| `sqlite_cache.rs` | infra | SQLite 三级缓存实现 |
| `onnx_compressor.rs` | infra | ONNX 嵌入余弦相似度压缩器（Pro） |
| `colbert_embedder.rs` | infra | ColBERT 多向量嵌入适配器（Pro） |

### 3.2 新增 IPC 命令（13 个）

| 命令 | Session | 说明 |
|---|---|---|
| `get_cache_stats` | S1 | 获取缓存统计 |
| `clear_cache` | S1 | 清空所有缓存 |
| `set_cache_settings` | S1 | 设置缓存参数 |
| `get_cache_settings` | S1 | 获取缓存参数 |
| `set_compression_ratio` | S2 | 设置压缩比 |
| `get_compression_ratio` | S2 | 获取压缩比 |
| `rebuild_bm25_index` | S4 | 重建 BM25 索引 |
| `rebuild_proposition_index` | S5 | 重建 Proposition 索引 |
| `build_summary_tree` | S6 | 构建 RAPTOR 摘要树 |
| `set_embedder_model` | S6 | 设置嵌入模型 |
| `set_retrieval_memory_enabled` | S9 | 启用/禁用检索记忆 |
| `get_retrieval_memory_stats` | S9 | 获取检索记忆统计 |
| `reset_retrieval_memory` | S9 | 重置检索记忆 |

### 3.3 新增设置键（9 个）

| 键 | Session | 默认值 | 说明 |
|---|---|---|---|
| `cache.enabled` | S1 | true | 缓存总开关 |
| `cache.ttl_secs` | S1 | 86400 | 缓存 TTL（秒） |
| `cache.semantic_threshold` | S1 | 0.92 | 语义匹配阈值 |
| `cache.privacy_mode` | S1 | false | 隐私模式（关闭缓存） |
| `compression.ratio` | S2 | 1.0 | 压缩比（1.0=禁用） |
| `rag.progressive_injection` | S7 | false | 渐进式注入开关 |
| `rag.speculative_enabled` | S8 | false | Speculative RAG 开关 |
| `rag.retrieval_memory_enabled` | S9 | false | 检索记忆开关 |

### 3.4 新增数据表（3 张）

| 表 | Session | 说明 |
|---|---|---|
| `exact_cache` | S1 | L0 精确缓存 |
| `semantic_cache` | S1 | L1 语义缓存 |
| `retrieval_cache` | S1 | L3 检索结果缓存 |
| `propositions` | S5 | Proposition 级原子事实 |
| `summary_nodes` | S6 | RAPTOR 摘要树节点 |
| `retrieval_memory` | S9 | 检索记忆记录 |

---

## 4. 性能基准对比

### 4.1 预期效果汇总

| 指标 | 优化前 | 优化后 | 提升幅度 |
|---|---|---|---|
| Token/次（平均） | 3350-8200 | 800-2000 | ↓50-75% |
| Token/次（缓存命中） | 3350-8200 | 0 | ↓100% |
| 检索命中率 | 65-75% | 85-95% | +20-30pp |
| 首 token 延迟（平均） | 1-2.5s | 400-1000ms | ↓50-60% |
| 总延迟（平均） | 3.5-5s | 1-1.6s | ↓50-70% |
| 总延迟（缓存命中） | 3.5-5s | ~30ms | ↓99% |

### 4.2 基准测试（TC-BENCH-001~005）

5 个 before/after 对比基准测试，位于 `crates/core/src/benchmark_tests.rs`：

| 测试 | 对比维度 | 验证内容 |
|---|---|---|
| TC-BENCH-001 | Token 消耗 | 压缩 + 渐进注入 → token ↓30%+ |
| TC-BENCH-002 | 检索命中率 | 自进化记忆选择最佳方法 → +20pp |
| TC-BENCH-003 | 首 token 延迟 | Speculative RAG 接受率 → 延迟 ↓30%+ |
| TC-BENCH-004 | 缓存命中率 | 三级缓存 → LLM 调用 ↓30%+ |
| TC-BENCH-005 | 总延迟 | 全部优化 → 延迟 ↓50%+ |

---

## 5. 测试覆盖

### 5.1 测试统计

| Sprint | Session | 测试数 | 累计测试（Free） | 累计测试（Pro） |
|---|---|---|---|---|
| A | S1 | 19（12 单元 + 7 集成） | 536 | — |
| A | S2 | 11 | 546 | 808 |
| A/B | S3-S4 | 20 | 558 | — |
| B | S5 | 10 | 558 | — |
| B | S6 | 14（6+8） | — | — |
| C | S7 | 13 | 587 | — |
| C | S8 | 12 | 599 | 863 |
| C | S9 | 10 | 612 | 881 |
| D | S10 | 5（基准） | 617 | 878 |

### 5.2 质量保证

- 全部 Session 通过 `cargo clippy --all-targets -- -D warnings`（Free + Pro 零警告）
- 全部 Session 通过 `cargo fmt --check`
- 全部 Session 通过 `./sync-to-github.sh --commit` 同步

---

## 6. 风险与缓解

| 风险 | 严重度 | 缓解措施 |
|---|---|---|
| 缓存泄漏敏感信息 | 🔴 高 | `privacy_mode` 可关闭缓存；文档导入触发缓存清空 |
| Prompt 压缩丢失关键信息 | ⚠️ 中 | 句子级评分保留高信息密度句子；sqrt 比例保留避免过度压缩 |
| Speculative RAG 草稿质量不足 | ⚠️ 中 | `min_draft_length` 阈值 + `FallbackDirect` 回退路径 |
| 自进化记忆冷启动效果差 | ⚠️ 低 | 冷启动默认 Hybrid（已验证有效）；随使用自动优化 |
| 渐进式注入误判 "信息不足" | ⚠️ 低 | 关键词列表精心筛选（15 中 + 12 英）；max_rounds 防无限追加 |

---

## 7. 向后兼容性

所有优化功能均**默认关闭**，通过设置键逐个开启：
- 禁用时 `chat_inner` 走标准 ChatEngine 路径
- 禁用时缓存不查询不写入
- 禁用时压缩器返回原始文本（`NoCompressor` 占位实现）
- 禁用时渐进注入注入全部 chunks
- 禁用时 Speculative RAG 走标准 RAG 路径
- 禁用时检索记忆使用默认方法（Hybrid）

---

## 8. 完成状态

| Session | 状态 | 完成日期 |
|---|---|---|
| S1 | ✅ | 2026-08-02 |
| S2 | ✅ | 2026-08-02 |
| S3 | ✅ | 2026-08-03 |
| S4 | ✅ | 2026-08-03 |
| S5 | ✅ | 2026-08-03 |
| S6 | ✅ | 2026-08-03 |
| S7 | ✅ | 2026-08-04 |
| S8 | ✅ | 2026-08-04 |
| S9 | ✅ | 2026-08-04 |
| S10 | ✅ | 2026-08-04 |

**极致性能优化全部完成。**
