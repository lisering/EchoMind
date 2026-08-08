# EchoMind 极致性能优化方案 — RAG 性能与 Token 消耗终极优化

> 调研日期：2026-08-02
> 调研来源：Anthropic Contextual Retrieval / Prompt Caching, Microsoft GraphRAG, ColBERT, Self-RAG, CRAG, LLMLingua-2, vLLM, AutoRAG, Mem0, LlamaIndex, NousResearch Hermes, LoRAX, Mistral 7B, Dense X Retrieval, RAG Survey 等 20+ 论文与开源项目

---

## 一、现状诊断：EchoMind 当前性能瓶颈

### 1.1 当前架构评估

| 维度 | 当前实现 | 瓶颈严重度 | 备注 |
|---|---|---|---|
| **嵌入模型** | all-MiniLM-L6-v2 (384-dim) | ⚠️ 中等 | 中文性能不足；384 维容量有限 |
| **向量检索** | SQLite 全表扫描 + Rust 余弦相似度 | 🔴 严重 | O(n) 线性扫描，10K+ chunks 后 P95 显著退化 |
| **检索策略** | 混合检索 (Vector+BM25+RRF) + HyDE + Reranker | ✅ 已较先进 | 已有 Cross-Encoder 重排序 + HyDE 改写 |
| **Prompt 结构** | 分段式 (静态前缀+动态上下文) | ✅ 已有缓存优化 | 但仅 OpenAIProvider 覆盖 segmented API |
| **历史压缩** | LLM 摘要压缩 (CompactionEngine) | ⚠️ 中等 | 每次压缩需一次 LLM 调用，增加延迟与 token |
| **Chunk 粒度** | 固定 token 窗口 + 语义分割 | ⚠️ 中等 | 缺少 proposition 级原子检索 |
| **上下文注入** | 全 chunk 内容注入 prompt | 🔴 严重 | 无 token 压缩/过滤，低相关信息也注入 |
| **缓存层** | 无语义缓存 | 🔴 严重 | 相似问题重复调用 LLM，token 全额消耗 |
| **本地推理** | mistral.rs v0.9.0 | ✅ 已有 Phase 3 | GEMV 内核已自研 |
| **HNSW 索引** | Pro feature, 可选 | ✅ 已有 | 但 Free 版仍全表扫描 |

### 1.2 Token 消耗分析

当前一次典型 RAG 查询的 token 构成：

```
System Prompt (静态前缀)     ~300 tokens   (可缓存)
  └─ 角色描述 + 可视化引导 + 语言引导
检索片段 (动态上下文)         ~2000-4000 tokens  (不可缓存，每次不同)
  └─ top_k=5 chunks × ~400-800 tokens/chunk
  └─ Chunk Expansion ±1 → 可能 15 chunks
对话历史                      ~500-2000 tokens  (随对话轮数增长)
  └─ 截断/压缩后仍可能很大
用户查询                      ~50-200 tokens
────────────────────────────────────
总计                          ~2850-6500 tokens / 次
```

**问题**：检索片段中大量低相关信息被注入，浪费 token；无语义缓存导致重复问题全额计费。

---

## 二、互联网调研成果汇总

### 2.1 Anthropic Contextual Retrieval (2024.09)

**核心发现**：
- **Contextual Embeddings**：为每个 chunk 添加文档级上下文前缀后嵌入，检索失败率 ↓49%
- **Contextual BM25**：同样的上下文前缀应用于 BM25 索引，效果叠加
- **+ Reranking**：Contextual Retrieval + Cross-Encoder 重排序 → 检索失败率 ↓67%
- **Prompt Caching**：缓存静态前缀 → 成本 ↓90%，延迟 ↓85%（首 token 延迟 11.5s → 2.4s）

**EchoMind 现状对比**：
- ✅ 已有 `build_contextual_text()` (低成本的文档名前缀方案)
- ✅ 已有 Reranker (bge-reranker-base Cross-Encoder)
- ✅ 已有 SegmentedPrompt + Prompt Caching 支持
- ❌ 未使用 LLM 生成 chunk 级上下文摘要（Anthropic 高成本方案，每 chunk 调用 LLM）
- ❌ Contextual BM25 未实现（BM25 索引仍使用原始 chunk 文本）

### 2.2 GraphRAG (Microsoft, 2024.04, arXiv:2404.16130)

**核心发现**：
- 将文档构建为**实体知识图谱** + **社区摘要层次**
- 全局问题（"主要主题是什么？"）传统 RAG 无法回答
- 两阶段：LLM 抽取实体→构建图→社区聚类→预生成社区摘要→查询时多社区摘要合并
- **适用场景**：全局分析型查询，不适合精确事实查询
- **成本警告**：索引构建 LLM 调用量大，"GraphRAG indexing can be an expensive operation"

**对 EchoMind 的启发**：
- EchoMind 面向本地知识库（律师/研究员），有全局分析需求
- 但图索引构建成本高，适合作为 Pro feature 按需触发

### 2.3 ColBERT (2020, arXiv:2004.12832)

**核心发现**：
- **Late Interaction** 模型：query 和 document 独立编码，延迟交互
- 每个 token 级向量独立存储，检索时 MaxSim 操作
- 比传统 BERT 双塔模型快 2 个数量级，FLOPs 少 4 个数量级
- 精度与 BERT-based 模型竞争，远超非 BERT 基线
- **关键优势**：文档表示可预计算离线存储，查询时只需向量相似度索引

**对 EchoMind 的启发**：
- 当前 all-MiniLM-L6-v2 是单向量嵌入，丢失了 token 级细粒度
- ColBERT 的 late interaction 可以显著提升精确匹配场景的命中率
- 可考虑支持 ColBERT-style 多向量嵌入作为可选嵌入模型

### 2.4 Self-RAG (2023.10, arXiv:2310.11511)

**核心发现**：
- LLM 自主决定**是否需要检索**（避免不必要的检索）
- 生成**反思 token**：[Retrieve], [NoRetrieve], [Relevant], [Irrelevant], [NoSupport], [PartiallySupport], [FullySupport]
- 检索结果按相关性过滤后再注入
- 7B/13B 参数模型超越 ChatGPT + Llama2-chat 在 QA/推理/事实验证任务

**对 EchoMind 的启发**：
- 当前 EchoMind 每次查询无条件检索（空库才拦截）
- 可引入轻量级检索必要性判断：简单问候/闲聊不需要检索
- 检索结果相关性过滤已部分实现（score_threshold），但可更智能

### 2.5 CRAG - Corrective RAG (2024.01, arXiv:2401.15884)

**核心发现**：
- 轻量级**检索评估器**评估检索结果质量，返回置信度
- 三级触发：Correct（高置信→直接用）、Incorrect（低置信→Web 搜索补充）、Ambiguous（中置信→混合）
- **decompose-then-recompose**：将检索文档分解为句子级，过滤无关信息
- Plug-and-play，可与任何 RAG 方法组合

**对 EchoMind 的启发**：
- 当前 score_threshold 是硬阈值，CRAG 的置信度分级更灵活
- decompose-then-recompose 可用于 chunk 级信息过滤，减少注入 token

### 2.6 Dense X Retrieval / Proposition (2023.12, arXiv:2312.06648)

**核心发现**：
- **检索粒度**对性能影响巨大：document > passage > sentence > **proposition**
- Proposition = 原子性事实表达，自包含、简洁
- 索引按 proposition 粒度 → 检索精度显著优于 passage 级
- 在相同计算预算下，细粒度检索单元提升下游 QA 性能

**对 EchoMind 的启发**：
- 当前 chunk 粒度（256 token）仍较粗
- 可引入 proposition 分割：用 LLM/规则将 chunk 分解为原子事实
- 检索命中 proposition → 扩展到包含 passage → 注入 prompt

### 2.7 LLMLingua-2 (2024.03, arXiv:2403.12968)

**核心发现**：
- **Prompt 压缩**：将 prompt 中的冗余 token 压缩，保留关键信息
- 使用 XLM-RoBERTa-large/mBERT 做 token 分类（保留/删除）
- 比原 LLMLingua 快 3-6 倍，端到端延迟加速 1.6-2.9 倍
- 压缩比 2x-5x，性能损失极小
- **任务无关**：无需针对特定任务训练

**对 EchoMind 的启发**：
- 当前检索片段全量注入 prompt，无压缩
- 可用 LLMLingua-2 对检索片段做 token 级压缩，2-5x 减少 token 消耗
- 特别适合长 chunk（>400 tokens）的压缩

### 2.8 vLLM (2023.09, arXiv:2309.06180)

**核心发现**：
- **PagedAttention**：高效的 KV cache 内存管理
- **Continuous Batching**：动态批处理
- **Prefix Caching**：前缀缓存（与 EchoMind 的 SegmentedPrompt 对应）
- **Speculative Decoding**：小模型先猜测 + 大模型验证，2-3x 加速
- 支持 FP8/INT8/INT4/GPTQ/AWQ/GGUF 量化

**对 EchoMind 的启发**：
- EchoMind 本地推理已有 mistral.rs v0.9.0 + PagedAttention (Phase 2)
- Speculative Decoding 可作为后续优化方向
- 前缀缓存概念已应用于 SegmentedPrompt

### 2.9 AutoRAG 2.0 (Marker-Inc-Korea)

**核心发现**：
- **自进化记忆系统**：记录哪些检索方法对哪类查询有效
- 多检索方法可插拔（BM25 / Vector / Hybrid）
- **父编排器 + 探索子代理**架构
- 检索结果 curated 为知识单元，非原始 grep 输出

**对 EchoMind 的启发**：
- 可引入检索方法记忆：记录某类查询的最佳检索策略
- 当前 EchoMind 的领域画像分类（DomainClassifier）是此方向的雏形

### 2.10 Mem0 (2026.04 新算法)

**核心发现**：
- **Single-pass ADD-only extraction**：一次 LLM 调用提取记忆，无 UPDATE/DELETE
- 实体链接 + 多信号检索（semantic + BM25 + entity matching）
- **LoCoMo 基准 92.5**（+21 points），LongMemEval 94.4（+27 points）
- token 效率极高：6.8K tokens，延迟 0.88-1.09s

**对 EchoMind 的启发**：
- 对话历史压缩可借鉴 Mem0 的单次提取策略
- 实体链接可增强检索（跨 chunk 实体关联）
- 多信号检索融合已部分实现（RRF），可增加实体匹配信号

### 2.11 NousResearch Hermes 2 Pro

**核心发现**：
- ChatML 格式 + Function Calling 优化
- 结构化输出（JSON mode）减少后处理 token
- 系统提示词 steerability 强

**对 EchoMind 的启发**：
- Prompt 格式优化（ChatML 结构化）可减少 token
- 结构化工具调用减少 LLM 输出 token

### 2.12 LoRAX (Predibase)

**核心发现**：
- 多 LoRA 推理：单 GPU 服务数千个 fine-tuned 模型
- 不同领域/用户使用不同 LoRA adapter

**对 EchoMind 的启发**：
- 本地 LLM 可支持多 LoRA：不同知识库领域使用不同 adapter
- 但 EchoMind 是桌面应用，GPU 资源有限，此方向优先级较低

---

## 三、头脑风暴：极致优化方案

### 🧠 发散思考：如果没有任何限制...

**如果 token 是无限免费的**：只需关注命中率和延迟
**如果延迟不是问题**：只需关注命中率和 token 消耗
**现实约束**：两者都要极致优化

#### 思路 1：多级缓存金字塔

```
用户查询
  │
  ▼
L0: 精确匹配缓存 (query hash → answer)     ← 命中率 5-10%, 0 token
  │ miss
  ▼
L1: 语义缓存 (embedding similarity → answer) ← 命中率 10-20%, 0 token
  │ miss
  ▼
L2: Prompt Caching (静态前缀命中)            ← 节省 30-90% input token
  │
  ▼
L3: 检索结果缓存 (query embedding → chunks)  ← 节省嵌入计算
  │
  ▼
L4: 完整 RAG pipeline
```

**预期效果**：L0+L1 覆盖 15-30% 重复查询，0 token 消耗；L2+L3 减少剩余 70-85% 查询的 token。

#### 思路 2：Proposition 级原子检索 + 动态扩展

```
当前: chunk (256 tokens) → embed → search → inject chunk
优化: chunk → decompose into propositions → embed each → search propositions
      → hit propositions → expand to containing chunk → inject only relevant sentences
```

**预期效果**：检索精度提升 30-50%（Dense X Retrieval 论文数据），注入 token 减少 40-60%。

#### 思路 3：Self-RAG 式智能检索决策

```
当前: 每次查询 → 无条件检索
优化: 查询分类器 → 判断是否需要检索
      ├── 闲聊/问候/确认 → 不检索，直接 LLM 回答 (0 检索 token)
      ├── 事实型查询 → 标准检索
      ├── 分析型查询 → GraphRAG 式全局检索
      └── 多步推理 → Agentic RAG (已有)
```

**预期效果**：20-30% 查询无需检索，节省嵌入+检索+注入 token。

#### 思路 4：LLMLingua 式 Prompt 压缩

```
当前: 检索 chunk 原文注入 prompt (~400 tokens/chunk × 5 = 2000 tokens)
优化: 检索 chunk → LLMLingua-2 压缩 (2-5x) → 注入压缩后文本 (~400-1000 tokens)
```

**预期效果**：注入 token 减少 50-80%，端到端延迟也降低（LLM 处理更短输入）。

#### 思路 5：CRAG 式检索结果精炼

```
当前: 检索 top_k chunks → 全部注入
优化: 检索 top_k chunks → 置信度评估
      ├── 高置信 → decompose-then-recompose (句子级过滤)
      ├── 低置信 → 降低 chunk 数量或返回 NoContext
      └── 中置信 → 部分注入 + 标注不确定性
```

**预期效果**：低质量检索结果不再污染 prompt，节省 30-50% 无效 token。

#### 思路 6：ColBERT 式 Late Interaction 嵌入

```
当前: all-MiniLM-L6-v2 → 单向量 (384-dim) → 余弦相似度
优化: ColBERT-style → token 级多向量 → MaxSim 交互
```

**预期效果**：精确匹配场景命中率显著提升，但存储和计算成本增加。适合作为 Pro feature。

#### 思路 7：Mem0 式实体链接增强

```
当前: 纯向量 + BM25 → RRF 融合
优化: 增加 entity matching 信号
      chunk → 抽取实体 → 实体嵌入 → 查询实体匹配 → 第三路检索信号
```

**预期效果**：包含相同实体的 chunk 获得额外加分，命中率提升 10-20%。

#### 思路 8：Speculative RAG (推测式 RAG)

```
当前: 检索 → 构建 prompt → LLM 生成 (串行)
优化: 检索 → 小模型快速生成草稿 → 大模型验证/修正 (并行流水线)
```

**预期效果**：首 token 延迟降低 40-60%，总生成时间降低 30-50%。

#### 思路 9：渐进式上下文注入

```
当前: 一次性注入所有 top_k chunks → LLM 生成
优化: 先注入 top_1 chunk → LLM 开始生成
      → 如果不够，流式追加 top_2, top_3...
      → LLM 在已有上下文足够时停止追加
```

**预期效果**：平均注入 chunk 数从 5 降至 2-3，token 节省 40-60%。

#### 思路 10：预计算摘要索引 (RAPTOR 式)

```
当前: chunk → embed → 平面索引
优化: chunk → 多级摘要树
      Level 0: 原始 chunks (256 tokens)
      Level 1: chunk 组摘要 (每个摘要 ~100 tokens)
      Level 2: 主题级摘要 (~50 tokens)
      查询时: 先搜摘要级 → 定位到 chunk 级 → 精确检索
```

**预期效果**：全局问题命中率大幅提升，大知识库检索速度提升（摘要索引远小于原文索引）。

---

## 四、优化方案优先级排序

### Phase A：Token 消耗极致压缩 (ROI 最高，优先实施)

| 优先级 | 优化项 | 预期 token 节省 | 实现复杂度 | 依赖 |
|---|---|---|---|---|
| P0 | 语义缓存 (L1) | 15-30% 重复查询 0 token | 中 | 嵌入缓存 + 相似度阈值 |
| P0 | LLMLingua-2 式 Prompt 压缩 | 50-80% 注入 token | 高 | 需集成压缩模型 |
| P1 | CRAG 式检索结果精炼 | 30-50% 无效 token | 中 | 检索置信度评估 |
| P1 | 渐进式上下文注入 | 40-60% 注入 token | 中 | 流式 prompt 构建 |
| P2 | Self-RAG 式检索决策 | 20-30% 查询 0 检索 | 低 | 查询分类器 |
| P2 | Proposition 级分割 | 30-50% 注入 token | 中 | LLM/规则分解 |

### Phase B：检索命中率极致提升

| 优先级 | 优化项 | 预期命中率提升 | 实现复杂度 | 依赖 |
|---|---|---|---|---|
| P0 | Contextual BM25 | +10-15% | 低 | BM25 索引使用上下文前缀文本 |
| P0 | 实体链接增强 | +10-20% | 中 | NER 抽取 + 实体嵌入 |
| P1 | ColBERT 式多向量 | +20-30% | 高 | 新嵌入模型 + MaxSim 检索 |
| P1 | 预计算摘要索引 (RAPTOR) | +15-25% (全局) | 中 | 多级摘要树构建 |
| P2 | GraphRAG 式图索引 | +20-30% (全局) | 高 | 实体图 + 社区摘要 |
| P2 | 自进化检索记忆 | +5-15% | 中 | 检索方法效果记录 |

### Phase C：推理性能极致优化

| 优先级 | 优化项 | 预期延迟降低 | 实现复杂度 | 依赖 |
|---|---|---|---|---|
| P0 | 精确匹配缓存 (L0) | 100% (命中时 0 延迟) | 低 | query hash → answer map |
| P0 | 检索结果缓存 (L3) | 50-70% (嵌入计算) | 低 | query embedding → chunks |
| P1 | Speculative RAG | 40-60% 首 token | 高 | 小模型 + 大模型流水线 |
| P1 | Speculative Decoding | 2-3x 生成加速 | 高 | vLLM/llama.cpp 支持 |
| P2 | 渐进式上下文注入 | 30-50% 首 token | 中 | 流式 prompt |
| P2 | HNSW 索引 Free 版 | 70-90% 检索 | 中 | 已有 Pro feature，需降级 |

---

## 五、详细实施计划

### Phase A-1: 语义缓存 (Semantic Cache) — P0

**目标**：相似查询直接返回缓存答案，0 LLM token 消耗。

**架构**：
```
用户查询
  │
  ├─ L0: 精确匹配 (SHA256(query) → answer)
  │      命中率: 5-10% (重复问题/FAQ 场景)
  │
  └─ L1: 语义匹配 (embed(query) → cosine sim ≥ 0.95 → cached answer)
         命中率: 10-20% (改述/同义问题)
         成本: 1 次嵌入 (远低于完整 RAG pipeline)
```

**实现要点**：
- 缓存存储：SQLite 表 `semantic_cache (query_text, query_embedding BLOB, answer_text, conversation_id, timestamp, hit_count)`
- TTL 过期：默认 24h，可配置（知识库更新时失效相关缓存）
- 失效策略：文档导入/删除时清空相关缓存（按 domain 或全局）
- 相似度阈值：0.92-0.95（可配置），太高命中率低，太低误命中
- 安全考虑：高隐私场景（律师/研究员）默认关闭，可 settings 开启

**代码位置**：
- `crates/core/src/semantic_cache.rs` — 新建 SemanticCache trait
- `crates/infra/src/sqlite_cache.rs` — SQLite 实现
- `crates/tauri-app/src/commands.rs` — chat_inner 集成缓存检查

**预期效果**：
- 重复查询场景 token 消耗 ↓ 15-30%
- 命中时延迟从 2-5s 降至 <50ms

### Phase A-2: LLMLingua-2 式 Prompt 压缩 — P0

**目标**：检索片段注入前做 token 级压缩，2-5x 减少注入 token。

**架构**：
```
检索 top_k chunks
  │
  ▼
chunk 内容 → LLMLingua-2 压缩模型
  │  (XLM-RoBERTa-large, ~1.1GB ONNX)
  │  压缩比 2x-5x (可配置)
  ▼
压缩后文本注入 prompt
```

**实现要点**：
- 压缩模型：microsoft/llmlingua-2-xlm-roberta-large-meetingbank (ONNX 格式)
- 使用 fastembed 或 ort 直接加载（与现有嵌入基础设施一致）
- 压缩策略：token 分类（保留/删除），双向上下文
- 延迟开销：~50-100ms/chunk（CPU 推理），可批量处理
- 可配置压缩比：2x (保守) / 3x (平衡) / 5x (激进)
- Pro feature：压缩模型下载 + ONNX 推理资源

**备选方案**（如果 ONNX 模型不可用）：
- 规则压缩：去除停用词、冗余空白、代码注释
- LLM 摘要压缩：用小模型（如 Haiku）摘要每个 chunk
- 抽取式压缩：句子级评分 + top-N 选择

**预期效果**：
- 注入 token 从 ~2000 降至 ~400-1000
- 总 token 消耗 ↓ 30-60%
- LLM 响应速度也提升（更短输入）

### Phase A-3: CRAG 式检索结果精炼 — P1

**目标**：检索结果按质量分级处理，低质量结果不注入。

**架构**：
```
检索 top_k chunks
  │
  ▼
检索评估器 (轻量级)
  │ 评估维度: 平均 score / score 方差 / top_1 score
  │
  ├── Correct (置信度 > 0.7)
  │   └─ decompose-then-recompose: chunk → sentences → 保留相关句子
  │      └─ 注入精炼后文本 (token ↓ 40-60%)
  │
  ├── Ambiguous (0.4 < 置信度 < 0.7)
  │   └─ 部分注入 + 标注 "以下信息可能不完全相关"
  │
  └── Incorrect (置信度 < 0.4)
      └─ 返回 NoContext (不浪费 token)
```

**实现要点**：
- 评估器无需额外模型，基于检索 score 统计
- decompose-then-recompose：句子分割 + 与 query 的相似度评分
- 阈值可配置，默认值基于 all-MiniLM-L6-v2 的 score 分布

### Phase A-4: 渐进式上下文注入 — P1

**目标**：按需注入 chunk，LLM 上下文足够时停止追加。

**架构**：
```
检索 top_k=5 chunks (按 score 降序)
  │
  ▼
注入 chunk[0] → LLM 开始生成
  │
  ├── LLM 生成中需要更多信息？
  │   └── 是: 追加 chunk[1], chunk[2]... (流式追加)
  │   └── 否: 停止追加，继续生成
  │
  ▼
最终生成 (平均注入 2-3 chunks 而非 5)
```

**实现要点**：
- 需要修改 `chat_stream_segmented` 支持动态上下文追加
- LLM 判断"是否需要更多信息"可通过特殊 token 或 prompt 引导
- 简化版：先注入 top-2，如果 LLM 回答中包含"不确定"则追加

### Phase B-1: Contextual BM25 — P0

**目标**：BM25 索引使用上下文前缀文本，而非原始 chunk 文本。

**当前**：
- `build_contextual_text(doc_name, chunk_content)` 已用于向量嵌入
- 但 FTS5 索引仍使用原始 `chunk.content`

**修改**：
- `add_chunk()` 时 FTS5 索引也使用 `build_contextual_text()` 的输出
- 需要重建索引（迁移脚本）

**预期效果**：BM25 精确匹配 +10-15%（Anthropic 数据：Contextual BM25 独立贡献 ~17.6% 失败率降低）

### Phase B-2: 实体链接增强 — P0

**目标**：增加实体匹配作为第三路检索信号。

**架构**：
```
当前: Vector search + BM25 → RRF fusion
优化: Vector search + BM25 + Entity match → 三路 RRF fusion

Entity match:
  chunk → NER 抽取实体 → 实体索引
  query → NER 抽取实体 → 实体匹配 → 命中 chunk 加分
```

**实现要点**：
- NER 模型：可用规则（正则）+ 轻量级模型
- 实体索引：SQLite 表 `entities (chunk_id, entity_text, entity_type)`
- 融合权重：entity_weight = 0.6（低于 vector=1.0, keyword=0.8）

**预期效果**：+10-20% 命中率（Mem0 数据显示实体匹配贡献显著）

### Phase C-1: 精确匹配缓存 (L0) — P0

**目标**：完全相同查询直接返回缓存答案。

**实现**：
```sql
CREATE TABLE exact_cache (
    query_hash TEXT PRIMARY KEY,  -- SHA256(normalized_query)
    query_text TEXT,
    answer_text TEXT,
    sources_json TEXT,  -- 序列化的 RetrievalResult
    conversation_id TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    hit_count INTEGER DEFAULT 0
);
```

- 查询归一化：小写化 + 去除标点 + 去除多余空白
- TTL: 默认 24h
- 失效：文档变更时清空

### Phase C-2: 检索结果缓存 (L3) — P0

**目标**：相似查询复用检索结果，跳过嵌入+检索计算。

**实现**：
```sql
CREATE TABLE retrieval_cache (
    query_hash TEXT PRIMARY KEY,
    query_embedding BLOB,  -- 用于语义匹配
    results_json TEXT,    -- 序列化的 Vec<RetrievalResult>
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
```

- 匹配方式：精确 hash + 语义相似度 ≥ 0.90
- TTL: 默认 1h（知识库不变时有效）
- 失效：文档导入/删除时清空

---

## 六、量化预期效果汇总

### Token 消耗优化

| 优化组合 | 预期 token 节省 | 备注 |
|---|---|---|
| 语义缓存 (L0+L1) | 15-30% 查询 → 0 token | 重复/相似查询 |
| Prompt 压缩 (LLMLingua) | 30-60% 注入 token | 检索片段压缩 2-5x |
| CRAG 精炼 | 30-50% 无效 token | 低质量结果过滤 |
| 渐进式注入 | 40-60% 注入 token | 按需追加 |
| Self-RAG 决策 | 20-30% 查询 → 0 检索 | 闲聊不检索 |
| **全部组合** | **60-85% 总 token 节省** | 叠加效果（有边际递减） |

### 检索命中率优化

| 优化组合 | 预期命中率提升 | 备注 |
|---|---|---|
| Contextual BM25 | +10-15% | BM25 索引上下文 |
| 实体链接 | +10-20% | 第三路检索信号 |
| ColBERT 多向量 | +20-30% | token 级匹配 |
| RAPTOR 摘要树 | +15-25% (全局) | 全局问题 |
| Proposition 粒度 | +30-50% | 原子事实检索 |
| **全部组合** | **40-70% 命中率提升** | 叠加效果 |

### 延迟优化

| 优化组合 | 预期延迟降低 | 备注 |
|---|---|---|
| 精确缓存 (L0) | 100% (命中时) | 0 延迟 |
| 检索缓存 (L3) | 50-70% 嵌入+检索 | 跳过计算 |
| Prompt 压缩 | 20-40% LLM 延迟 | 更短输入 |
| Speculative RAG | 40-60% 首 token | 草稿+验证 |
| HNSW Free 版 | 70-90% 检索 | O(log n) vs O(n) |

---

## 七、实施路线图

### Sprint 1 (2 周): 缓存金字塔 — 最高 ROI

- [ ] L0 精确匹配缓存 (SQLite 表 + chat_inner 集成)
- [ ] L1 语义缓存 (嵌入相似度 + TTL + 失效策略)
- [ ] L3 检索结果缓存 (嵌入+检索结果缓存)
- [ ] 缓存统计 IPC 命令 (`get_cache_stats`, `clear_cache`)
- [ ] 缓存设置 UI (开关 + TTL 配置 + 隐私模式)

### Sprint 2 (2 周): Prompt 压缩 + 检索精炼

- [ ] LLMLingua-2 ONNX 模型集成 (或规则压缩备选)
- [ ] CRAG 式检索评估器 (置信度分级)
- [ ] decompose-then-recompose (句子级过滤)
- [ ] 压缩比可配置设置
- [ ] 压缩效果统计 (token before/after)

### Sprint 3 (2 周): 检索命中率提升

- [ ] Contextual BM25 (FTS5 索引重建)
- [ ] 实体链接增强 (NER + 实体索引 + 三路 RRF)
- [ ] Self-RAG 式检索决策 (查询分类器)
- [ ] Proposition 级分割 (LLM 分解 chunk → 原子事实)
- [ ] 命中率基准测试 (before/after 对比)

### Sprint 4 (3 周): 高级优化 (Pro features)

- [ ] 渐进式上下文注入 (流式 prompt 构建)
- [ ] RAPTOR 式摘要树索引
- [ ] ColBERT 式多向量嵌入 (可选嵌入模型)
- [ ] Speculative RAG (小模型草稿 + 大模型验证)
- [ ] 自进化检索记忆 (方法效果记录)

---

## 八、参考来源

| 编号 | 来源 | 关键贡献 |
|---|---|---|
| R1 | Anthropic Contextual Retrieval (2024.09) | Contextual Embeddings + BM25, 检索失败率 ↓49%/67% |
| R2 | Anthropic Prompt Caching (2024.08) | 静态前缀缓存, 成本 ↓90%, 延迟 ↓85% |
| R3 | Microsoft GraphRAG (arXiv:2404.16130) | 实体知识图谱 + 社区摘要, 全局问题 |
| R4 | ColBERT (arXiv:2004.12832) | Late Interaction, token 级多向量, 2 个数量级加速 |
| R5 | Self-RAG (arXiv:2310.11511) | 自主检索决策 + 反思 token |
| R6 | CRAG (arXiv:2401.15884) | 检索评估器 + decompose-then-recompose |
| R7 | Dense X Retrieval (arXiv:2312.06648) | Proposition 级原子检索粒度 |
| R8 | LLMLingua-2 (arXiv:2403.12968) | Prompt 压缩 2-5x, 3-6x 加速 |
| R9 | vLLM (arXiv:2309.06180) | PagedAttention, Continuous Batching, Prefix Caching |
| R10 | RAG Survey (arXiv:2312.10997) | Naive/Advanced/Modular RAG 范式综述 |
| R11 | CRAG Benchmark (arXiv:2406.04744) | RAG 基准, SOTA 63% 无幻觉 |
| R12 | AutoRAG 2.0 (GitHub) | 自进化记忆系统 + 多检索方法 |
| R13 | Mem0 (GitHub, 2026.04) | 单次提取 + 实体链接 + 多信号检索, LoCoMo 92.5 |
| R14 | NousResearch Hermes 2 Pro | ChatML + Function Calling 优化 |
| R15 | LoRAX (Predibase) | 多 LoRA 推理, 千模型单 GPU |
| R16 | LlamaIndex | RAG 框架, 数据连接器 + 索引 + 检索 |
| R17 | Mistral 7B (arXiv:2310.06825) | GQA + SWA 高效推理 |
| R18 | Instructor (567-labs) | 结构化输出 + Pydantic 验证 |

---

> **本方案是基于 18 个论文/项目的系统调研成果，所有数据引用均来自原始论文摘要。**
> **实施时需根据 EchoMind 实际架构约束（桌面应用、本地优先、隐私安全）做适配调整。**
