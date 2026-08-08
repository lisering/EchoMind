# 互联网调研发现 → EchoMind 借鉴实施原子计划

> **来源**：IfAI / StoryMoss / KakeiBonByRust / mnemo / Bamboo-agent 等项目源码分析 + 行业最佳实践调研
> **目标**：将调研发现转化为 EchoMind 可落地的改进，分 4 个 Phase / 10 个 Session 实施
> **铁律**：TDD 驱动、真实数据测试、生产级代码质量、最新版本依赖、文档实时同步
> **前置条件**：当前测试基线 Free 617 passed / Pro 888 passed / Clippy 零警告

---

## 调研发现总览

| 来源项目 | 核心发现 | 借鉴价值 |
|---|---|---|
| **mnemo** | 知识图谱（petgraph DiGraph）+ 多阶段检索管线（chunk→entity→graph expansion） | ★★★★★ 高 |
| **StoryMoss** | 质量门控系统（加权评分 code/rule/model → threshold） + 持续学习（观察→蒸馏→技能） | ★★★★☆ 高 |
| **IfAI** | DAG 工作流引擎（WorkflowNode/Edge） + 符号引擎（tree-sitter 代码感知 RAG） + 三层持久记忆 | ★★★★☆ 高 |
| **Bamboo-agent** | 子代理舰队管理（mailbox + transport） + 生命周期 hooks + 上下文压缩增强 | ★★★☆☆ 中 |
| **KakeiBonByRust** | AES-256-GCM 加密最佳实践 + 输入校验模式 | ★★☆☆☆ 低（已有） |

---

## Phase 1: 知识图谱检索增强（高 ROI，1-2 天）

### Session 34: 实体关系图数据模型 + 存储层

**目标**：在现有 `EntityExtractor`（纯规则 NER）基础上，新增实体间关系抽取 + 图数据模型 + SQLite 存储，为图谱检索打基础。

**改动文件**：
- `crates/models/src/lib.rs` — 新增 `EntityRelation` / `GraphTriple` 模型
- `crates/core/src/lib.rs` — `Storage` trait 新增 5 个图查询方法（默认空操作）
- `crates/infra/src/sqlite_storage.rs` — 新增 `entity_relations` 表 + 5 个方法实现
- `crates/core/src/entity_extractor.rs` — 新增 `extract_relations()` 规则方法
- `crates/core/src/entity_extractor_tests.rs` — 关系抽取 TDD 测试

### Session 35: 图谱检索引擎 + 集成到 RAG 管线

**目标**：实现图遍历检索（实体命中 → 邻居扩展 → 关系过滤），集成到 HybridRetriever 作为第四路检索通道。

**改动文件**：
- `crates/core/src/graph_retriever.rs` — 新建 `GraphRetriever` 引擎
- `crates/core/src/graph_retriever_tests.rs` — TDD 测试
- `crates/core/src/hybrid_retriever.rs` — 四路 RRF 融合（Vector + BM25 + Entity + Graph）
- `crates/core/src/lib.rs` — 注册 `graph_retriever` 模块
- `crates/tauri-app/src/commands.rs` — `chat_inner` 集成图检索
- `crates/tauri-app/src/state.rs` — AppState 增加 `graph_retriever_enabled`

---

## Phase 2: 质量门控 + 自适应学习（2-3 天）

### Session 36: RAG 质量门控系统

**目标**：借鉴 StoryMoss 的加权评分门控，为每次 RAG 检索结果计算质量分数（检索覆盖率 / 来源多样性 / 分数方差），低于阈值时触发降级策略（扩大 top-k / 切换检索模式 / 提示用户）。

**改动文件**：
- `crates/core/src/quality_gate.rs` — 新建 `QualityGate` + `GateScore` + `GateConfig`
- `crates/core/src/quality_gate_tests.rs` — TDD 测试
- `crates/core/src/chat.rs` — `ChatEngine::chat()` 检索后调用质量门控
- `crates/core/src/lib.rs` — 注册 `quality_gate` 模块
- `crates/tauri-app/src/commands.rs` — `chat_inner` 读取 `rag.quality_gate_enabled`
- `crates/models/src/lib.rs` — `SettingsPayload` 增加 `quality_gate_enabled`

### Session 37: 检索质量自学习引擎

**目标**：扩展现有 `RetrievalMemoryEngine`（REQ-PERF-012），增加「用户反馈信号」采集（用户重新提问 / 编辑后重发 / 切换检索模式），将反馈作为隐式正/负标签，优化 `select_method()` 权重。

**改动文件**：
- `crates/core/src/retrieval_memory.rs` — 新增 `FeedbackSignal` / `record_feedback()` / `recompute_weights()`
- `crates/core/src/retrieval_memory_tests.rs` — 反馈学习 TDD 测试
- `crates/tauri-app/src/commands.rs` — 新增 `record_retrieval_feedback` IPC
- `crates/tauri-app/src/lib.rs` — 注册新命令
- `ui/src/chat.js` — 检测用户重新提问 / 编辑行为，自动上报反馈

---

## Phase 3: 高级 Agent 编排（3-5 天）

### Session 38: Agent 生命周期 Hooks 系统

**目标**：借鉴 Bamboo-agent 的生命周期 hooks，为 `AgentEngine` 和 `CoordinatorEngine` 增加 `before_retrieval` / `after_retrieval` / `before_generation` / `after_generation` 钩子点，支持插件式扩展（如检索前查询改写、生成后事实核查）。

**改动文件**：
- `crates/core/src/hooks.rs` — 新建 `AgentHook` trait + `HookContext` + `HookRegistry`
- `crates/core/src/hooks_tests.rs` — TDD 测试
- `crates/core/src/agent.rs` — ReAct 循环中插入 hook 调用点
- `crates/core/src/coordinator.rs` — 四阶段管线中插入 hook 调用点
- `crates/core/src/lib.rs` — 注册 `hooks` 模块
- `crates/tauri-app/src/state.rs` — AppState 增加 `hook_registry`

### Session 39: DAG 工作流引擎

**目标**：借鉴 IfAI 的 DAG 工作流，支持用户定义多步骤 Agent 工作流（如「翻译→摘要→审核→输出」），节点间数据通过 DAG 边传递，支持并行节点和条件分支。

**改动文件**：
- `crates/models/src/lib.rs` — 新增 `Workflow` / `WorkflowNode` / `WorkflowEdge` / `NodeStatus`
- `crates/core/src/workflow.rs` — 新建 `WorkflowEngine` + `WorkflowRunner` + 拓扑排序 + 并行执行
- `crates/core/src/workflow_tests.rs` — TDD 测试
- `crates/core/src/lib.rs` — 注册 `workflow` 模块
- `crates/tauri-app/src/commands.rs` — 新增 `create_workflow` / `run_workflow` / `get_workflow_status` IPC
- `crates/tauri-app/src/lib.rs` — 注册新命令
- `ui/src/workflow-editor.js` — 新建工作流可视化编辑器（简易 DAG 画布）

### Session 40: 子代理舰队管理

**目标**：借鉴 Bamboo-agent 的子代理管理，`CoordinatorEngine` 支持动态创建子代理（每个子代理有独立检索上下文），通过 mailbox 消息传递协调，支持「分工→并行→汇总」模式。

**改动文件**：
- `crates/core/src/sub_agent.rs` — 新建 `SubAgent` / `SubAgentFleet` / `Mailbox` / `AgentMessage`
- `crates/core/src/sub_agent_tests.rs` — TDD 测试
- `crates/core/src/coordinator.rs` — 集成子代理舰队
- `crates/core/src/lib.rs` — 注册 `sub_agent` 模块
- `crates/tauri-app/src/commands.rs` — 新增 `set_coordinator_subagents` IPC

---

## Phase 4: 代码感知 RAG + 开发者增强（3-5 天）

### Session 41: 代码符号引擎（tree-sitter）

**目标**：借鉴 IfAI 的 SymbolEngine，用 tree-sitter 对代码类文档（.rs/.ts/.py/.go）抽取符号（函数/类/结构体/接口），建立符号→chunk 映射，使代码查询能精确定位到函数定义。

**改动文件**：
- `crates/infra/Cargo.toml` — 新增 `tree-sitter` + 语言语法依赖
- `crates/infra/src/symbol_engine.rs` — 新建 `SymbolEngine` + `extract_symbols()` + 符号索引
- `crates/infra/src/symbol_engine_tests.rs` — TDD 测试
- `crates/core/src/lib.rs` — `Storage` trait 新增符号查询方法
- `crates/infra/src/sqlite_storage.rs` — 新增 `symbols` 表
- `crates/core/src/import.rs` — 导入代码文件时自动抽取符号
- `crates/core/src/retriever.rs` — 代码查询时优先符号匹配

### Session 42: 代码片段执行沙箱

**目标**：借鉴 CodeForge 的多语言执行器，为代码类查询提供「执行验证」能力——用户提问代码相关问题时，Agent 可选择执行代码片段验证结果。

**改动文件**：
- `crates/core/src/code_executor.rs` — 新建 `CodeExecutor` trait + `DockerExecutor` / `WasmExecutor`
- `crates/core/src/code_executor_tests.rs` — TDD 测试（Mock 执行器）
- `crates/core/src/agent.rs` — Agent 工具列表新增 `execute_code`
- `crates/core/src/lib.rs` — 注册 `code_executor` 模块
- `crates/tauri-app/src/commands.rs` — 新增 `execute_code_snippet` IPC（Pro 门控）

### Session 43: 持久化记忆系统增强

**目标**：借鉴 IfAI 的三层记忆（Wing/Hall/Room）+ Bamboo-agent 的上下文压缩，将 EchoMind 的 `AutoDream` 引擎增强为「跨会话知识沉淀」系统——自动从对话中提取关键事实存入记忆库，后续查询自动注入。

**改动文件**：
- `crates/core/src/memory_store.rs` — 新建 `MemoryStore` + `MemoryEntry` + `MemoryTier`
- `crates/core/src/memory_store_tests.rs` — TDD 测试
- `crates/core/src/auto_dream.rs` — 集成记忆提取（从对话中提取事实）
- `crates/core/src/chat.rs` — 查询时自动注入相关记忆
- `crates/core/src/lib.rs` — 注册 `memory_store` 模块
- `crates/infra/src/sqlite_storage.rs` — 新增 `memory_entries` 表
- `crates/tauri-app/src/commands.rs` — 新增 `get_memories` / `delete_memory` / `clear_memories` IPC

---

## 各 Session 衔接提示词

以下提示词可直接粘贴到新对话中使用，每个提示词自包含完整上下文。

---

### 📌 Session 34 提示词：实体关系图数据模型 + 存储层

```
## 任务：实体关系图数据模型 + SQLite 存储层（借鉴 mnemo 知识图谱架构）

### 背景

对 mnemo 项目（Rust 知识图谱 RAG）源码分析发现，其核心创新是使用 petgraph DiGraph 构建实体-关系图，检索时先匹配实体节点，再沿图边扩展到相关 chunk。多阶段检索管线（chunk search → entity search → graph expansion → relation filtering → scoring）显著提升了关联查询的召回率。

EchoMind 已有 `EntityExtractor`（crates/core/src/entity_extractor.rs，纯规则 NER，5 类实体：person/proper_noun/tech_term/identifier/date），但当前实体仅用于三路 RRF 的实体匹配通道——没有实体间关系，没有图遍历。

### 当前代码状态

- `crates/models/src/lib.rs` 的 `Entity` 结构体：`{ text: String, entity_type: String }` — 只有实体文本和类型，没有关系
- `crates/core/src/entity_extractor.rs`：`EntityExtractor::extract(text) -> Vec<Entity>` — 纯抽取，无关系
- `crates/infra/src/sqlite_storage.rs`：有 `entities` 表（entity_text / entity_type / chunk_id），无关系表
- `crates/core/src/lib.rs` 的 `Storage` trait：有 `add_entities` / `search_by_entity`，无图查询方法

### 需要实现的改动

1. **`crates/models/src/lib.rs`**：新增模型
   ```rust
   /// 实体间关系（知识图谱边）
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct EntityRelation {
       /// 关系唯一标识（UUID v4）
       pub id: String,
       /// 主体实体文本
       pub subject: String,
       /// 关系类型（defined_as / part_of / depends_on / uses / implements / extends / references / related_to）
       pub relation_type: String,
       /// 客体实体文本
       pub object: String,
       /// 来源 chunk ID
       pub chunk_id: String,
       /// 置信度（规则抽取 0.5-1.0）
       pub confidence: f32,
   }
   
   impl EntityRelation {
       pub fn new(subject: String, relation_type: String, object: String, chunk_id: String, confidence: f32) -> Self { ... }
   }
   
   /// 图谱三元组（用于检索结果展示）
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct GraphTriple {
       pub subject: String,
       pub relation: String,
       pub object: String,
   }
   ```

2. **`crates/core/src/entity_extractor.rs`**：新增关系抽取
   ```rust
   impl EntityExtractor {
       /// 从文本中抽取实体间关系（基于规则模式匹配）
       pub fn extract_relations(text: &str, chunk_id: &str) -> Vec<EntityRelation> {
           // 规则模式（中英文）：
           // "X is defined as Y" / "X 定义为 Y" → defined_as
           // "X is part of Y" / "X 是 Y 的一部分" → part_of
           // "X depends on Y" / "X 依赖 Y" → depends_on
           // "X uses Y" / "X 使用 Y" → uses
           // "X implements Y" / "X 实现 Y" → implements
           // "X extends Y" / "X 继承 Y" → extends
           // "X references Y" / "X 引用 Y" → references
           // 先 extract entities，再在实体间搜索关系模式
       }
   }
   ```

3. **`crates/core/src/lib.rs`**：`Storage` trait 新增方法（默认空操作）
   ```rust
   async fn add_relation(&self, relation: &EntityRelation) -> anyhow::Result<()>;
   async fn add_relations_batch(&self, relations: &[EntityRelation]) -> anyhow::Result<()>;
   async fn get_relations_for_entity(&self, entity_text: &str) -> anyhow::Result<Vec<EntityRelation>>;
   async fn get_relations_for_chunk(&self, chunk_id: &str) -> anyhow::Result<Vec<EntityRelation>>;
   async fn search_by_relation(&self, subject: &str, relation_type: &str) -> anyhow::Result<Vec<EntityRelation>>;
   ```

4. **`crates/infra/src/sqlite_storage.rs`**：
   - 新增 `entity_relations` 表：`id TEXT PK, subject TEXT, relation_type TEXT, object TEXT, chunk_id TEXT, confidence REAL, FOREIGN KEY(chunk_id) REFERENCES chunks(id) ON DELETE CASCADE`
   - 索引：`idx_relations_subject` / `idx_relations_object` / `idx_relations_chunk`
   - 实现 5 个 trait 方法

5. **`crates/core/src/import.rs`**：导入时在 `index_entities()` 后调用 `index_relations()`
   ```rust
   async fn index_relations(&self, chunk_id: &str, text: &str, storage: &dyn Storage) -> anyhow::Result<()> {
       let relations = EntityExtractor::extract_relations(text, chunk_id);
       if !relations.is_empty() {
           storage.add_relations_batch(&relations).await?;
       }
       Ok(())
   }
   ```

6. **`crates/core/src/entity_extractor_tests.rs`**：TDD 测试
   - `tc_relation_001`：英文 "X is defined as Y" → defined_as 关系
   - `tc_relation_002`：中文 "X 依赖 Y" → depends_on 关系
   - `tc_relation_003`：无关系的文本返回空 Vec
   - `tc_relation_004`：一段包含多个关系的文本全部识别
   - `tc_relation_005`：置信度计算（精确匹配 1.0，模糊匹配 0.7）
   - `tc_relation_006`：关系去重（同一 chunk 内相同三元组只保留一个）

### 验收标准
- `cargo test -p echomind-core` 全部通过
- `cargo test -p echomind-infra` 全部通过
- `cargo clippy --all-targets -- -D warnings` 零警告
- `cargo fmt --check` 通过
- 现有 `entity_extractor_tests.rs` 测试不受影响（向后兼容）
- 关系抽取是纯 CPU（零 LLM），典型 chunk < 2ms

### 文档同步
- `docs/SRS_v1.0.md`：新增 REQ-RAG-026（知识图谱实体关系检索）
- `docs/api/code-contracts.md`：新增 `EntityRelation` / `GraphTriple` / 5 个 Storage trait 方法
- `docs/api/database-schema.md`：新增 `entity_relations` 表
- `docs/api/settings-reference.md`：新增 `rag.graph_retriever_enabled` 设置键（预留）
```

---

### 📌 Session 35 提示词：图谱检索引擎 + 集成到 RAG 管线

```
## 任务：图谱检索引擎 + 四路 RRF 融合（依赖 Session 34 完成）

### 前置条件

Session 34（实体关系图数据模型 + 存储层）已完成：
- `EntityRelation` 模型已定义
- `entity_relations` 表已建
- `EntityExtractor::extract_relations()` 可用
- `Storage` trait 有 5 个图查询方法
- 导入时自动抽取关系

### 背景

mnemo 项目的多阶段检索管线：
1. Chunk search（向量检索）
2. Entity search（实体匹配）
3. Graph expansion（从命中实体沿图边扩展到邻居实体）
4. Relation filtering（按关系类型过滤）
5. Scoring（综合分数排序）

当前 EchoMind 的 `HybridRetriever` 是三路 RRF（Vector + BM25 + Entity），图检索将作为第四路。

### 当前代码状态

- `crates/core/src/hybrid_retriever.rs`：`HybridRetriever` 有 `retrieve()` → 调用 `vector_search` + `keyword_search` + `search_by_entity` → `rrf_fuse()`
- RRF 融合函数：`rrf_fuse(vector_results, keyword_results, entity_results, k=60)`
- `crates/core/src/retriever.rs`：`expand_neighbors()` 已有（chunk 相邻扩展）

### 需要实现的改动

1. **`crates/core/src/graph_retriever.rs`**（新文件）：
   ```rust
   /// 图谱检索引擎：实体命中 → 图遍历扩展 → 关联 chunk 检索
   pub struct GraphRetriever<E, S> {
       embedder: E,
       storage: S,
   }
   
   impl<E: Embedder, S: Storage> GraphRetriever<E, S> {
       /// 图谱检索：查询 → 抽取实体 → 匹配图中实体 → 沿边扩展 → 获取关联 chunk
       pub async fn retrieve(&self, query: &str, top_k: usize) -> anyhow::Result<Vec<RetrievalResult>> {
           // 1. 从查询中抽取实体（EntityExtractor::extract）
           // 2. 对每个实体调用 storage.get_relations_for_entity()
           // 3. 收集关联 chunk_id（去重）
           // 4. 批量获取 chunk 内容 + 计算分数
           //    分数 = 实体匹配数 / 总实体数 * 0.5 + 关系深度衰减 * 0.5
           // 5. 返回 top_k 结果
       }
       
       /// 图遍历：从种子实体出发，BFS 遍历 N 跳内的所有关联实体
       fn graph_expand(&self, seed_entities: &[String], max_hops: usize) -> Vec<GraphTriple> {
           // BFS：每跳调用 get_relations_for_entity
           // max_hops 默认 2（避免图爆炸）
           // 去重已访问实体
       }
   }
   ```

2. **`crates/core/src/hybrid_retriever.rs`**：四路 RRF
   - 新增 `graph_retriever: Option<GraphRetriever<E, S>>` 字段
   - `retrieve()` 增加 graph 检索分支（当 `graph_enabled` 为 true 时）
   - `rrf_fuse()` 增加第四路输入：`graph_results: Vec<RetrievalResult>`
   - RRF 权重调整：vector 1.0 / keyword 0.8 / entity 0.6 / graph 0.7

3. **`crates/core/src/lib.rs`**：`pub mod graph_retriever;`

4. **`crates/core/src/graph_retriever_tests.rs`**（新文件）：TDD 测试
   - `tc_graph_001`：查询包含已知实体 → 返回关联 chunk
   - `tc_graph_002`：图遍历 2 跳内实体全部覆盖
   - `tc_graph_003`：无匹配实体时返回空 Vec（不报错）
   - `tc_graph_004`：max_hops 限制防止图爆炸
   - `tc_graph_005`：分数计算正确（实体匹配权重 + 深度衰减）
   - `tc_graph_006`：四路 RRF 融合后图检索结果正确排序

5. **`crates/tauri-app/src/commands.rs`**：
   - `chat_inner` 读取 `rag.graph_retriever_enabled` 设置
   - 启用时 `HybridRetriever` 开启图检索通道

6. **`crates/tauri-app/src/state.rs`**：AppState 增加 `graph_retriever_enabled: bool`

### 验收标准
- `cargo test` 全部通过（Free + Pro）
- `cargo clippy --all-targets -- -D warnings` 零警告
- 图检索禁用时行为完全一致（向后兼容）
- 图检索启用后，包含实体关系的查询召回率提升

### 文档同步
- `docs/SRS_v1.0.md`：REQ-RAG-026 补充图检索 AC
- `docs/api/code-contracts.md`：新增 `GraphRetriever` 契约
- `docs/api/settings-reference.md`：新增 `rag.graph_retriever_enabled` 键
- `./sync-to-github.sh --commit`
```

---

### 📌 Session 36 提示词：RAG 质量门控系统

```
## 任务：RAG 质量门控系统（借鉴 StoryMoss 加权评分门控）

### 背景

对 StoryMoss 源码分析发现，其有一个精巧的质量门控系统（gate.rs）：每次 Agent 输出经过三维加权评分（code 0.4 + rule 0.3 + model 0.3），低于阈值（0.75）时触发重试或降级。

EchoMind 当前 RAG 检索后直接将结果送入 LLM 生成，没有质量评估——即使检索结果完全不相关，也会浪费 token 生成低质量回答。需要一个质量门控：检索后评估结果质量，低质量时触发降级策略。

### 当前代码状态

- `crates/core/src/chat.rs`：`ChatEngine::chat()` 检索后直接 `build_rag_prompt` → `chat_stream`
- 检索结果 `Vec<RetrievalResult>` 有 `score: f32` 字段（余弦相似度）
- 没有「检索质量评估」环节
- `HybridRetriever` 有三/四路 RRF 融合，但无质量评估

### 需要实现的改动

1. **`crates/core/src/quality_gate.rs`**（新文件）：
   ```rust
   /// 质量门控配置
   pub struct GateConfig {
       /// 通过阈值（默认 0.6，范围 0.3-0.9）
       pub threshold: f32,
       /// 检索覆盖率权重（默认 0.4）
       pub weight_coverage: f32,
       /// 来源多样性权重（默认 0.3）
       pub weight_diversity: f32,
       /// 分数方差权重（默认 0.3）
       pub weight_score: f32,
       /// 降级策略
       pub degradation: DegradationStrategy,
   }
   
   pub enum DegradationStrategy {
       /// 扩大 top_k 后重试
       ExpandTopK { factor: f32, max_retry: usize },
       /// 切换到混合检索
       SwitchToHybrid,
       /// 提示用户查询不够明确
       WarnUser,
       /// 直接放行（仅记录分数）
       PassThrough,
   }
   
   /// 质量分数
   pub struct GateScore {
       /// 检索覆盖率：top-1 分数 / 阈值（>1.0 表示高分）
       pub coverage: f32,
       /// 来源多样性：不同文档数 / 总结果数（0-1）
       pub diversity: f32,
       /// 分数方差：top-k 分数的标准差（高方差 = 有明显优劣）
       pub score_variance: f32,
       /// 加权总分
       pub weighted: f32,
       /// 是否通过
       pub passed: bool,
   }
   
   pub const GATE_PASS_THRESHOLD: f32 = 0.6;
   
   /// 评估检索结果质量
   pub fn evaluate(results: &[RetrievalResult], config: &GateConfig) -> GateScore {
       // coverage = results.first().score / 0.3（归一化到 0-1+）
       // diversity = unique_doc_count / results.len()
       // score_variance = stddev(results.iter().map(|r| r.score))
       // weighted = coverage * weight_coverage + diversity * weight_diversity + score_variance * weight_score
       // passed = weighted >= threshold
   }
   
   /// 执行降级策略
   pub async fn degrade<D: Embedder, S: Storage>(
       query: &str,
       original_results: &[RetrievalResult],
       config: &GateConfig,
       retriever: &dyn Retriever,
   ) -> anyhow::Result<Vec<RetrievalResult>> { ... }
   ```

2. **`crates/core/src/chat.rs`**：
   - `ChatEngine` 新增 `gate_config: Option<GateConfig>` 字段
   - `with_quality_gate(config)` builder 方法
   - `chat()` 检索后调用 `evaluate()`，未通过时调用 `degrade()`

3. **`crates/core/src/lib.rs`**：`pub mod quality_gate;`

4. **`crates/core/src/quality_gate_tests.rs`**（新文件）：TDD 测试
   - `tc_gate_001`：高分检索结果通过门控
   - `tc_gate_002`：低分检索结果未通过门控
   - `tc_gate_003`：多样性计算正确（单文档 vs 多文档）
   - `tc_gate_004`：分数方差计算正确
   - `tc_gate_005`：ExpandTopK 降级策略扩大 top_k 重试
   - `tc_gate_006`：PassThrough 策略只记录不降级
   - `tc_gate_007`：加权总分计算公式验证

5. **`crates/tauri-app/src/commands.rs`**：
   - `chat_inner` 读取 `rag.quality_gate_enabled` 设置
   - 启用时创建 `GateConfig` 并传给 `ChatEngine`
   - 质量分数通过 `chat_phase` 事件推送给前端

6. **`crates/models/src/lib.rs`**：`SettingsPayload` 增加 `quality_gate_enabled: bool`

### 验收标准
- `cargo test -p echomind-core` 全部通过
- `cargo clippy --all-targets -- -D warnings` 零警告
- 门控禁用时行为完全一致
- 门控启用后低质量检索触发降级

### 文档同步
- `docs/SRS_v1.0.md`：新增 REQ-RAG-027（RAG 质量门控）
- `docs/api/code-contracts.md`：新增 `QualityGate` / `GateScore` / `GateConfig` 契约
- `docs/api/settings-reference.md`：新增 `rag.quality_gate_enabled` / `rag.quality_gate_threshold` 键
- `./sync-to-github.sh --commit`
```

---

### 📌 Session 37 提示词：检索质量自学习引擎

```
## 任务：检索质量自学习引擎（扩展现有 RetrievalMemoryEngine，依赖 Session 36 完成）

### 前置条件

Session 36（RAG 质量门控系统）已完成：`QualityGate` 可评估检索质量分数。

### 背景

对 StoryMoss 的持续学习机制（learning.rs）分析发现，其通过观察用户行为（接受/拒绝/编辑 Agent 输出）自动蒸馏技能，无需显式标注。

EchoMind 已有 `RetrievalMemoryEngine`（crates/core/src/retrieval_memory.rs，REQ-PERF-012），记录每次检索的 `QueryType → RetrievalMethod → hit_rate`，但缺少「用户反馈信号」——不知道用户对结果是否满意。

### 当前代码状态

- `crates/core/src/retrieval_memory.rs`：
  - `RetrievalMemoryEngine` 有 `record_retrieval()` / `select_method()` / `get_stats()`
  - `MemoryRecord` 有 `query_type` / `method` / `hit_rate` / `avg_score` / `count`
  - `select_method()` 选择 hit_rate 最高的方法，冷启动用默认 Hybrid
  - **缺少**：用户反馈信号采集

### 需要实现的改动

1. **`crates/core/src/retrieval_memory.rs`**：新增反馈机制
   ```rust
   /// 用户反馈信号（隐式采集）
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct FeedbackSignal {
       /// 查询文本
       pub query: String,
       /// 使用的检索方法
       pub method: RetrievalMethod,
       /// 反馈类型
       pub feedback: FeedbackType,
       /// 时间戳
       pub timestamp: i64,
   }
   
   pub enum FeedbackType {
       /// 用户重新提问（相同查询不同方法重试）→ 负信号
       RetryWithDifferentMethod,
       /// 用户编辑后重发 → 负信号
       EditAndResend,
       /// 用户切换检索模式 → 中性信号
       SwitchMode,
       /// 用户接受回答（无后续操作 / 继续新话题）→ 正信号
       Accepted,
       /// 用户点赞 → 强正信号
       ThumbsUp,
       /// 用户点踩 → 强负信号
       ThumbsDown,
   }
   
   impl RetrievalMemoryEngine {
       /// 记录用户反馈，更新方法权重
       pub async fn record_feedback(&self, signal: FeedbackSignal) -> anyhow::Result<()> {
           // 正信号：hit_rate += 0.1（上限 1.0）
           // 负信号：hit_rate -= 0.15（下限 0.0）
           // 强正/强负：幅度加倍
           // 更新对应 MemoryRecord
       }
       
       /// 根据反馈历史重新计算所有方法权重
       pub async fn recompute_weights(&self) -> anyhow::Result<()> {
           // 遍历所有 MemoryRecord
           // 基于最近 N 条反馈重新计算 hit_rate
           // 时间衰减：近期反馈权重更高
       }
   }
   ```

2. **`crates/core/src/retrieval_memory_tests.rs`**：新增测试
   - `tc_feedback_001`：正信号提高 hit_rate
   - `tc_feedback_002`：负信号降低 hit_rate
   - `tc_feedback_003`：强信号幅度加倍
   - `tc_feedback_004`：hit_rate 不超过 1.0 / 不低于 0.0
   - `tc_feedback_005`：recompute_weights 时间衰减正确
   - `tc_feedback_006`：多轮反馈后 select_method 偏好变化

3. **`crates/tauri-app/src/commands.rs`**：
   - 新增 `#[tauri::command] pub async fn record_retrieval_feedback(signal: FeedbackSignal, state) -> Result<(), String>`
   - `chat_inner` 在以下场景自动采集隐式反馈：
     - 用户重新提问相同/相似查询 → `RetryWithDifferentMethod`
     - 用户编辑消息后重发 → `EditAndResend`

4. **`crates/tauri-app/src/lib.rs`**：注册 `record_retrieval_feedback` 命令

5. **`ui/src/chat.js`**：
   - 检测用户行为模式：
     - `sendMessage()` 检测与上一条消息相似度 > 0.8 → 上报 `RetryWithDifferentMethod`
     - `editMessage()` 后 `sendMessage()` → 上报 `EditAndResend`
     - 助手消息下方新增 👍/👎 按钮 → 上报 `ThumbsUp` / `ThumbsDown`

### 验收标准
- `cargo test` 全部通过
- `cargo clippy --all-targets -- -D warnings` 零警告
- 反馈信号正确更新 hit_rate 权重
- 禁用检索记忆时反馈采集不执行（向后兼容）

### 文档同步
- `docs/SRS_v1.0.md`：REQ-PERF-012 扩展说明（用户反馈自学习）
- `docs/api/ipc-commands.md`：新增 `record_retrieval_feedback` 命令
- `docs/api/code-contracts.md`：`FeedbackSignal` / `FeedbackType` 契约
- `./sync-to-github.sh --commit`
```

---

### 📌 Session 38 提示词：Agent 生命周期 Hooks 系统

```
## 任务：Agent 生命周期 Hooks 系统（借鉴 Bamboo-agent 生命周期 hooks）

### 背景

对 Bamboo-agent 源码分析发现，其在 Agent 执行的关键节点（before_retrieval / after_retrieval / before_generation / after_generation）插入了可扩展的 hook 点，允许插件式注入逻辑（如检索前自动查询改写、生成后事实核查），无需修改核心引擎代码。

EchoMind 的 `AgentEngine`（crates/core/src/agent.rs）和 `CoordinatorEngine`（crates/core/src/coordinator.rs）的执行流程是硬编码的，无法在不修改源码的情况下扩展行为。

### 当前代码状态

- `crates/core/src/agent.rs`：`AgentEngine::run()` ReAct 循环（Thought → Action → Observation → 重复）
- `crates/core/src/coordinator.rs`：`CoordinatorEngine` 四阶段（Research → Synthesis → Implementation → Verification）
- 没有 hook/插件机制

### 需要实现的改动

1. **`crates/core/src/hooks.rs`**（新文件）：
   ```rust
   use std::sync::Arc;
   
   /// Hook 执行上下文（传递给 hook 的上下文信息）
   #[derive(Debug, Clone)]
   pub struct HookContext {
       pub conversation_id: String,
       pub query: String,
       pub current_phase: HookPhase,
       pub retrieval_results: Vec<RetrievalResult>,
       pub metadata: serde_json::Value,
   }
   
   /// Hook 执行阶段
   #[derive(Debug, Clone, Copy, PartialEq, Eq)]
   pub enum HookPhase {
       BeforeRetrieval,
       AfterRetrieval,
       BeforeGeneration,
       AfterGeneration,
   }
   
   /// Hook trait：插件式扩展点
   #[async_trait::async_trait]  // 注意：EchoMind 用原生 async fn in trait，这里也用原生
   pub trait AgentHook: Send + Sync {
       /// Hook 名称（用于日志和调试）
       fn name(&self) -> &str;
       
       /// Hook 执行阶段
       fn phase(&self) -> HookPhase;
       
       /// 执行 hook，可修改上下文
       async fn execute(&self, ctx: &mut HookContext) -> anyhow::Result<()>;
       
       /// 是否启用
       fn enabled(&self) -> bool { true }
   }
   
   /// Hook 注册表
   pub struct HookRegistry {
       hooks: Vec<Box<dyn AgentHook>>,
   }
   
   impl HookRegistry {
       pub fn new() -> Self { Self { hooks: Vec::new() } }
       
       pub fn register(&mut self, hook: Box<dyn AgentHook>) {
           self.hooks.push(hook);
       }
       
       /// 执行指定阶段的所有 hook（按注册顺序）
       pub async fn run_phase(&self, phase: HookPhase, ctx: &mut HookContext) -> anyhow::Result<()> {
           for hook in &self.hooks {
               if hook.enabled() && hook.phase() == phase {
                   hook.execute(ctx).await?;
               }
           }
           Ok(())
       }
       
       pub fn count(&self) -> usize { self.hooks.len() }
       pub fn is_empty(&self) -> bool { self.hooks.is_empty() }
   }
   ```
   注意：不使用 `#[async_trait]` 宏，用原生 `async fn` in trait（Edition 2024）。

2. **`crates/core/src/agent.rs`**：
   - `AgentEngine` 新增 `hooks: Option<Arc<HookRegistry>>` 字段
   - `with_hooks(registry)` builder 方法
   - `run()` 在关键节点调用 `hooks.run_phase()`
     - 检索前：`BeforeRetrieval`（可修改查询）
     - 检索后：`AfterRetrieval`（可修改/过滤检索结果）
     - 生成前：`BeforeGeneration`（可修改 prompt）
     - 生成后：`AfterGeneration`（可修改输出）

3. **`crates/core/src/coordinator.rs`**：同样集成 hook 调用点

4. **`crates/core/src/lib.rs`**：`pub mod hooks;`

5. **`crates/core/src/hooks_tests.rs`**（新文件）：TDD 测试
   - `tc_hook_001`：注册 hook 后 run_phase 正确调用
   - `tc_hook_002`：多个同阶段 hook 按顺序执行
   - `tc_hook_003`：disabled hook 不执行
   - `tc_hook_004`：hook 可修改 HookContext（如改写查询）
   - `tc_hook_005`：hook 返回 Err 时中断后续 hook
   - `tc_hook_006`：AgentEngine 集成测试（hook 在正确阶段执行）
   - `tc_hook_007`：HookRegistry is_empty / count 正确

### 验收标准
- `cargo test -p echomind-core` 全部通过
- `cargo clippy --all-targets -- -D warnings` 零警告
- 无 hook 注册时行为完全一致（向后兼容）
- 使用原生 async fn in trait（不引入 async-trait 宏）

### 文档同步
- `docs/SRS_v1.0.md`：新增 REQ-RAG-028（Agent 生命周期 Hooks）
- `docs/api/code-contracts.md`：新增 `AgentHook` trait / `HookRegistry` / `HookPhase` 契约
- `./sync-to-github.sh --commit`
```

---

### 📌 Session 39 提示词：DAG 工作流引擎

```
## 任务：DAG 工作流引擎（借鉴 IfAI WorkflowNode/Edge 架构）

### 背景

对 IfAI 源码分析发现，其核心创新是 DAG 工作流引擎：用户定义由节点（AgentType）和边（数据流）组成的有向无环图，引擎负责拓扑排序、并行执行独立节点、串行执行依赖节点。支持条件分支（if-else 节点）和循环（while 节点，限制最大迭代）。

EchoMind 当前的 `AgentEngine` 是单线程 ReAct 循环，`CoordinatorEngine` 是固定四阶段管线——都不支持用户自定义工作流。DAG 工作流可以让用户定义「翻译→摘要→审核→输出」等多步骤 RAG 管线。

### 当前代码状态

- `crates/core/src/agent.rs`：ReAct 循环（Thought → Action → Observation）
- `crates/core/src/coordinator.rs`：固定四阶段（Research → Synthesis → Implementation → Verification）
- 没有用户可自定义的工作流

### 需要实现的改动

1. **`crates/models/src/lib.rs`**：新增工作流模型
   ```rust
   /// 工作流定义
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct Workflow {
       pub id: String,
       pub name: String,
       pub description: String,
       pub nodes: Vec<WorkflowNode>,
       pub edges: Vec<WorkflowEdge>,
       pub created_at: i64,
   }
   
   /// 工作流节点
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct WorkflowNode {
       pub id: String,
       pub label: String,
       pub node_type: NodeType,
       pub config: serde_json::Value,
   }
   
   #[derive(Debug, Clone, Serialize, Deserialize)]
   #[serde(tag = "kind")]
   pub enum NodeType {
       /// RAG 检索节点
       Retrieval { top_k: usize, retrieval_mode: String },
       /// LLM 生成节点
       Generation { system_prompt: String, model: Option<String> },
       /// 条件分支节点
       Condition { expression: String },
       /// 聚合节点（合并多个上游输出）
       Aggregate { strategy: String },
       /// 输出节点
       Output { format: String },
   }
   
   /// 工作流边（数据流）
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct WorkflowEdge {
       pub source: String,
       pub target: String,
       /// 输出字段映射
       pub mapping: Option<String>,
   }
   
   /// 节点执行状态
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub enum NodeStatus {
       Pending,
       Running,
       Completed { output: String },
       Failed { error: String },
       Skipped,
   }
   ```

2. **`crates/core/src/workflow.rs`**（新文件）：
   ```rust
   /// DAG 工作流引擎
   pub struct WorkflowEngine<E, S, L> {
       embedder: E,
       storage: S,
       llm: L,
   }
   
   impl<E: Embedder, S: Storage, L: LLMProvider> WorkflowEngine<E, S, L> {
       /// 执行工作流
       pub async fn run(&self, workflow: &Workflow, input: &str) -> anyhow::Result<WorkflowResult> {
           // 1. 拓扑排序（Kahn's algorithm）
           // 2. 按拓扑序执行节点：
           //    - 无依赖的节点并行执行（tokio::join!）
           //    - 有依赖的节点等待上游完成
           // 3. 每个节点接收上游输出作为输入
           // 4. 节点输出通过 edge mapping 传递给下游
           // 5. Condition 节点根据表达式选择分支
           // 6. Aggregate 节点合并多个上游输出
       }
       
       /// 拓扑排序
       fn topological_sort(nodes: &[WorkflowNode], edges: &[WorkflowEdge]) -> anyhow::Result<Vec<String>> {
           // Kahn's algorithm：计算入度 → 零入度入队 → BFS → 检测环
       }
       
       /// 获取可并行执行的节点层
       fn parallel_layers(nodes: &[WorkflowNode], edges: &[WorkflowEdge]) -> Vec<Vec<String>> { ... }
   }
   
   pub struct WorkflowResult {
       pub node_results: HashMap<String, NodeStatus>,
       pub final_output: String,
       pub duration_ms: u64,
   }
   ```

3. **`crates/core/src/workflow_tests.rs`**（新文件）：TDD 测试
   - `tc_workflow_001`：简单线性工作流（A→B→C）正确执行
   - `tc_workflow_002`：并行节点（A→{B,C}→D）正确并行执行
   - `tc_workflow_003`：拓扑排序正确
   - `tc_workflow_004`：检测到环返回错误
   - `tc_workflow_005`：Condition 节点正确分支
   - `tc_workflow_006`：Aggregate 节点正确合并
   - `tc_workflow_007`：节点失败时下游标记 Skipped
   - `tc_workflow_008`：Retrieval 节点正确检索
   - `tc_workflow_009`：Generation 节点正确调用 LLM

4. **`crates/core/src/lib.rs`**：`pub mod workflow;`

5. **`crates/tauri-app/src/commands.rs`**：
   - `create_workflow(name, nodes, edges) -> Workflow`
   - `run_workflow(workflow_id, input) -> WorkflowResult`
   - `list_workflows() -> Vec<Workflow>`
   - `delete_workflow(workflow_id) -> ()`
   - `save_workflow_template(workflow) -> ()`（持久化到 settings 表 JSON）

6. **`crates/tauri-app/src/lib.rs`**：注册新命令

### 验收标准
- `cargo test` 全部通过
- `cargo clippy --all-targets -- -D warnings` 零警告
- 拓扑排序正确处理并行和串行
- 环检测返回明确错误
- 节点失败不崩溃（下游 Skipped）

### 文档同步
- `docs/SRS_v1.0.md`：新增 REQ-RAG-029（DAG 工作流引擎）
- `docs/api/code-contracts.md`：新增 `WorkflowEngine` / `Workflow` / `WorkflowNode` 契约
- `docs/api/ipc-commands.md`：新增 5 个工作流命令
- `docs/api/database-schema.md`：新增 `workflows` 表（settings 表 JSON 存储）
- `./sync-to-github.sh --commit`
```

---

### 📌 Session 40 提示词：子代理舰队管理

```
## 任务：子代理舰队管理（借鉴 Bamboo-agent 子代理 + mailbox，依赖 Session 38/39 完成）

### 前置条件

- Session 38（Hooks 系统）已完成：`HookRegistry` 可用
- Session 39（DAG 工作流引擎）已完成：`WorkflowEngine` 可用

### 背景

Bamboo-agent 的子代理系统：主代理可动态创建子代理，每个子代理有独立上下文和工具集，通过 mailbox 消息传递协调。支持「分工→并行→汇总」模式：主代理将复杂查询分解为子任务，分配给多个子代理并行处理，最后汇总结果。

EchoMind 的 `CoordinatorEngine` 已有四阶段管线，但每个阶段是串行单代理。子代理舰队可以让 Research 阶段并行派出多个子代理分别检索不同方面。

### 当前代码状态

- `crates/core/src/coordinator.rs`：`CoordinatorEngine` 四阶段串行
- `crates/core/src/agent.rs`：`AgentEngine` 单代理 ReAct 循环
- 无子代理概念

### 需要实现的改动

1. **`crates/core/src/sub_agent.rs`**（新文件）：
   ```rust
   use tokio::sync::mpsc;
   
   /// 子代理消息
   #[derive(Debug, Clone)]
   pub struct AgentMessage {
       pub from: String,
       pub to: String,
       pub kind: MessageKind,
       pub payload: String,
       pub timestamp: i64,
   }
   
   #[derive(Debug, Clone)]
   pub enum MessageKind {
       Task,        // 任务分配
       Result,      // 结果返回
       Query,       // 子代理向主代理查询
       Response,    // 主代理回复
       Terminate,   // 终止指令
   }
   
   /// 子代理（独立检索上下文 + 工具集）
   pub struct SubAgent {
       pub id: String,
       pub name: String,
       pub task: String,
       pub context: Vec<ChatMessage>,
       pub mailbox_tx: mpsc::Sender<AgentMessage>,
       pub mailbox_rx: mpsc::Receiver<AgentMessage>,
   }
   
   impl SubAgent {
       pub fn new(id: String, name: String, task: String) -> Self { ... }
       
       /// 执行子任务（独立 ReAct 循环）
       pub async fn run<E, S, L>(&mut self, embedder: &E, storage: &S, llm: &L) -> anyhow::Result<String> {
           // 独立的 AgentEngine 实例
           // 检索结果存入自身 context
           // 完成后通过 mailbox 发送 Result 消息
       }
   }
   
   /// 子代理舰队（主代理管理多个子代理）
   pub struct SubAgentFleet {
       pub agents: HashMap<String, SubAgent>,
       pub max_agents: usize,  // 默认 4
   }
   
   impl SubAgentFleet {
       /// 创建并派发子代理
       pub async fn dispatch(&mut self, tasks: Vec<(String, String)>) -> anyhow::Result<()> {
           // tasks: Vec<(agent_name, task_description)>
           // 为每个任务创建 SubAgent
           // 超过 max_agents 时排队等待
       }
       
       /// 等待所有子代理完成并收集结果
       pub async fn join_all(&mut self) -> anyhow::Result<Vec<(String, String)>> {
           // 等待所有 Result 消息
           // 返回 Vec<(agent_name, result_text)>
       }
       
       /// 终止所有子代理
       pub async fn terminate_all(&mut self) { ... }
   }
   ```

2. **`crates/core/src/coordinator.rs`**：
   - Research 阶段集成 `SubAgentFleet`
   - 将子查询分解为子代理任务
   - 并行检索后汇总结果

3. **`crates/core/src/lib.rs`**：`pub mod sub_agent;`

4. **`crates/core/src/sub_agent_tests.rs`**（新文件）：TDD 测试
   - `tc_subagent_001`：单个子代理正确执行任务
   - `tc_subagent_002`：多个子代理并行执行
   - `tc_subagent_003`：mailbox 消息正确传递
   - `tc_subagent_004`：max_agents 限制生效
   - `tc_subagent_005`：join_all 收集所有结果
   - `tc_subagent_006`：terminate_all 正确终止
   - `tc_subagent_007`：子代理失败时主代理收到错误消息

### 验收标准
- `cargo test -p echomind-core` 全部通过
- `cargo clippy --all-targets -- -D warnings` 零警告
- 子代理有独立检索上下文（互不干扰）
- mailbox 消息传递可靠
- 并行执行显著降低复杂查询延迟

### 文档同步
- `docs/SRS_v1.0.md`：REQ-RAG-025 扩展说明（子代理舰队）
- `docs/api/code-contracts.md`：新增 `SubAgent` / `SubAgentFleet` / `AgentMessage` 契约
- `./sync-to-github.sh --commit`
```

---

### 📌 Session 41 提示词：代码符号引擎（tree-sitter）

```
## 任务：代码符号引擎（借鉴 IfAI SymbolEngine，tree-sitter 代码感知 RAG）

### 背景

IfAI 的 SymbolEngine 使用 tree-sitter 对代码文件进行符号分析（函数/类/结构体/接口/方法），建立符号→位置映射，使代码查询能精确定位到函数定义而非模糊匹配整个文件。

EchoMind 当前对代码类文档（.rs/.ts/.py/.go）的检索是纯文本分块 + 向量检索——查询 "HashMap::insert 的实现" 无法精确定位到 `impl HashMap` 块。符号引擎可以解决这个精确匹配问题。

### 当前代码状态

- `crates/core/src/loader.rs`：`TextLoader` / `MarkdownLoader` 按文本加载，不识别代码结构
- `crates/core/src/splitter.rs`：`TextSplitter` 按 token 窗口分块，不识别函数边界
- 无代码符号概念

### 需要实现的改动

1. **`crates/infra/Cargo.toml`**：新增依赖
   ```toml
   tree-sitter = "0.22"
   tree-sitter-rust = "0.21"
   tree-sitter-typescript = "0.21"
   tree-sitter-python = "0.21"
   tree-sitter-go = "0.21"
   ```

2. **`crates/models/src/lib.rs`**：新增模型
   ```rust
   /// 代码符号
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct CodeSymbol {
       pub id: String,
       pub chunk_id: String,
       pub name: String,
       pub kind: SymbolKind,
       pub language: String,
       pub start_line: usize,
       pub end_line: usize,
       pub signature: Option<String>,
   }
   
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub enum SymbolKind {
       Function,
       Method,
       Class,
       Struct,
       Interface,
       Enum,
       Constant,
       Module,
   }
   ```

3. **`crates/infra/src/symbol_engine.rs`**（新文件，Pro 门控）：
   ```rust
   pub struct SymbolEngine;
   
   impl SymbolEngine {
       /// 检测文件语言
       pub fn detect_language(file_path: &str) -> Option<&'static str> { ... }
       
       /// 从代码文本中抽取符号
       pub fn extract_symbols(content: &str, language: &str, chunk_id: &str) -> Vec<CodeSymbol> {
           // 根据 language 选择 tree-sitter parser
           // 解析 AST → 遍历节点 → 提取函数/类/结构体等
           // 记录名称、类型、行号范围、签名
       }
       
       /// 代码感知分块：按函数/类边界分块（而非 token 窗口）
       pub fn split_by_symbols(content: &str, language: &str, max_tokens: usize) -> Vec<(String, usize, usize)> {
           // 按函数边界分块
           // 超大函数内部再用 token 窗口分
           // 每块附带符号上下文（如 "在 impl HashMap 中"）
       }
   }
   ```

4. **`crates/core/src/lib.rs`**：`Storage` trait 新增方法
   ```rust
   async fn add_symbols(&self, symbols: &[CodeSymbol]) -> anyhow::Result<()>;
   async fn search_by_symbol(&self, name: &str, kind: Option<&SymbolKind>) -> anyhow::Result<Vec<CodeSymbol>>;
   async fn get_symbols_for_chunk(&self, chunk_id: &str) -> anyhow::Result<Vec<CodeSymbol>>;
   ```

5. **`crates/infra/src/sqlite_storage.rs`**：新增 `code_symbols` 表 + 实现 3 个方法

6. **`crates/core/src/import.rs`**：导入代码文件时使用 `SymbolEngine` 分块 + 索引符号

7. **`crates/core/src/retriever.rs`**：代码查询时优先符号匹配

8. **`crates/infra/src/symbol_engine_tests.rs`**：TDD 测试
   - `tc_symbol_001`：Rust 代码抽取函数符号
   - `tc_symbol_002`：TypeScript 代码抽取类/接口符号
   - `tc_symbol_003`：Python 代码抽取函数符号
   - `tc_symbol_004`：按函数边界分块
   - `tc_symbol_005`：符号名称搜索精确匹配
   - `tc_symbol_006`：语言检测正确

### 验收标准
- `cargo test --features pro` 全部通过
- `cargo clippy --all-targets --features pro -- -D warnings` 零警告
- 代码文件导入时自动抽取符号
- 代码查询能精确定位到函数/类

### 文档同步
- `docs/SRS_v1.0.md`：新增 REQ-RAG-030（代码感知 RAG）
- `docs/api/code-contracts.md`：新增 `SymbolEngine` / `CodeSymbol` / `SymbolKind` 契约
- `docs/api/database-schema.md`：新增 `code_symbols` 表
- `./sync-to-github.sh --commit`
```

---

### 📌 Session 42 提示词：代码片段执行沙箱

```
## 任务：代码片段执行沙箱（借鉴 CodeForge 多语言执行器，Pro feature）

### 背景

CodeForge 是多语言代码运行器，支持在沙箱中执行代码片段并返回结果。EchoMind 的 Agent 在回答代码相关问题时，如果能执行代码片段验证结果，将大幅提升回答可信度。

### 当前代码状态

- `crates/core/src/agent.rs`：Agent 工具列表有 `search_kb` / `list_documents`
- 无代码执行能力

### 需要实现的改动

1. **`crates/core/src/code_executor.rs`**（新文件，Pro 门控）：
   ```rust
   /// 代码执行器 trait
   pub trait CodeExecutor: Send + Sync {
       /// 执行代码片段，返回输出
       async fn execute(&self, code: &str, language: &str, stdin: Option<&str>) -> anyhow::Result<ExecutionResult>;
       
       /// 支持的语言列表
       fn supported_languages(&self) -> Vec<&str>;
   }
   
   pub struct ExecutionResult {
       pub stdout: String,
       pub stderr: String,
       pub exit_code: i32,
       pub duration_ms: u64,
       pub timed_out: bool,
   }
   
   /// WASM 沙箱执行器（本地安全执行）
   #[cfg(feature = "pro")]
   pub struct WasmExecutor {
       timeout_secs: u64,  // 默认 10s
       memory_limit_mb: usize,  // 默认 64MB
   }
   ```

2. **`crates/core/src/agent.rs`**：Agent 工具新增 `execute_code`
   ```rust
   // build_agent_prompt() 工具列表新增：
   // execute_code: 执行代码片段并返回输出
   //   参数: { "language": "python|rust|javascript", "code": "...", "stdin": "..." }
   ```

3. **`crates/core/src/lib.rs`**：注册 `code_executor` 模块 + `CodeExecutor` trait

4. **`crates/tauri-app/src/commands.rs`**：
   - 新增 `execute_code_snippet(code, language) -> ExecutionResult` IPC（Pro 门控）
   - 安全限制：超时 10s / 内存 64MB / 无网络访问

5. **`crates/core/src/code_executor_tests.rs`**：TDD 测试（Mock 执行器）
   - `tc_exec_001`：Mock 执行器正确返回结果
   - `tc_exec_002`：超时处理
   - `tc_exec_003`：不支持的语言返回错误
   - `tc_exec_004`：stderr 正确捕获
   - `tc_exec_005`：Agent 集成测试（Agent 选择执行代码工具）

### 验收标准
- `cargo test --features pro` 全部通过
- `cargo clippy --all-targets --features pro -- -D warnings` 零警告
- 代码执行有超时和内存限制
- Free 版本不可用（Pro 门控）

### 文档同步
- `docs/SRS_v1.0.md`：新增 REQ-RAG-031（代码执行沙箱）
- `docs/api/code-contracts.md`：新增 `CodeExecutor` trait / `ExecutionResult` 契约
- `docs/api/ipc-commands.md`：新增 `execute_code_snippet` 命令
- `./sync-to-github.sh --commit`
```

---

### 📌 Session 43 提示词：持久化记忆系统增强

```
## 任务：持久化记忆系统增强（借鉴 IfAI 三层记忆 + Bamboo-agent 上下文压缩）

### 背景

IfAI 的三层持久记忆系统（Wing/Hall/Room 空间隐喻）将记忆按重要性和时效性分层存储，查询时自动注入相关记忆。Bamboo-agent 的上下文压缩在对话过长时自动提取关键信息。

EchoMind 已有 `AutoDream`（crates/core/src/auto_dream.rs，后台空闲整理引擎），但只做重复检测和矛盾发现——没有从对话中提取关键事实存入记忆库。增强后，系统可自动从对话中学习（如用户偏好、常见问题模式、事实陈述），后续查询自动注入。

### 当前代码状态

- `crates/core/src/auto_dream.rs`：`DreamEngineState` 做重复检测 + 矛盾发现
- `crates/core/src/chat.rs`：`ChatEngine::chat()` 无记忆注入
- 无持久化记忆存储

### 需要实现的改动

1. **`crates/models/src/lib.rs`**：新增模型（已有 `MemoryRecord` 用于检索记忆，新增对话记忆）
   ```rust
   /// 对话记忆条目（跨会话知识沉淀）
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct MemoryEntry {
       pub id: String,
       pub tier: MemoryTier,
       pub content: String,
       pub source: MemorySource,
       pub conversation_id: Option<String>,
       pub created_at: i64,
       pub last_accessed: i64,
       pub access_count: u32,
       pub importance: f32,  // 0.0-1.0
   }
   
   /// 记忆层级（借鉴 IfAI Wing/Hall/Room 空间隐喻）
   #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
   pub enum MemoryTier {
       /// 翼层：临时记忆（当前会话），会话结束后提升或遗忘
       Wing,
       /// 厅层：工作记忆（近期重要），跨会话保留
       Hall,
       /// 室层：长期记忆（核心知识），永久保留
       Room,
   }
   
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub enum MemorySource {
       UserStatement,    // 用户明确陈述的事实
       AssistantAnswer,  // 助手回答中的关键信息
       AutoExtracted,    // AutoDream 自动提取
       UserPinned,       // 用户手动置顶
   }
   ```

2. **`crates/core/src/memory_store.rs`**（新文件）：
   ```rust
   /// 记忆存储引擎
   pub struct MemoryStore<S: Storage> {
       storage: S,
       max_wing: usize,   // 临时记忆上限（默认 50）
       max_hall: usize,   // 工作记忆上限（默认 100）
       max_room: usize,   // 长期记忆上限（默认 500）
   }
   
   impl<S: Storage> MemoryStore<S> {
       /// 从对话中提取记忆（LLM 辅助）
       pub async fn extract_from_conversation<L: LLMProvider>(
           &self, messages: &[ChatMessage], llm: &L,
       ) -> anyhow::Result<Vec<MemoryEntry>> {
           // 将最近 N 轮对话发给 LLM
           // system prompt: "从以下对话中提取值得记住的关键事实（用户偏好、重要决定、事实陈述），每条一行"
           // 解析 LLM 输出 → 创建 Wing 层 MemoryEntry
       }
       
       /// 记忆提升（Wing → Hall → Room）
       pub async fn promote(&self, memory_id: &str) -> anyhow::Result<()> { ... }
       
       /// 记忆遗忘（低重要性 + 低访问频率 → 删除）
       pub async fn forget(&self, tier: MemoryTier) -> anyhow::Result<usize> { ... }
       
       /// 检索相关记忆（用于查询时注入）
       pub async fn retrieve_relevant(&self, query: &str, top_k: usize) -> anyhow::Result<Vec<MemoryEntry>> {
           // 关键词匹配 + 重要性排序
       }
       
       /// 记忆整合（AutoDream 调用）
       pub async fn consolidate(&self) -> anyhow::Result<()> {
           // Wing 层超过 max_wing → 按重要性提升到 Hall 或遗忘
           // Hall 层超过 max_hall → 按重要性提升到 Room 或遗忘
           // Room 层超过 max_room → 按访问频率遗忘最低的
       }
   }
   ```

3. **`crates/core/src/auto_dream.rs`**：集成记忆提取
   - `run_dream()` 新增记忆整合阶段
   - 调用 `MemoryStore::extract_from_conversation()` + `consolidate()`

4. **`crates/core/src/chat.rs`**：
   - `ChatEngine::chat()` 查询前调用 `MemoryStore::retrieve_relevant()`
   - 相关记忆注入 system prompt 前缀

5. **`crates/core/src/lib.rs`**：`pub mod memory_store;`

6. **`crates/infra/src/sqlite_storage.rs`**：新增 `memory_entries` 表 + CRUD 方法

7. **`crates/tauri-app/src/commands.rs`**：
   - `get_memories(tier: Option<MemoryTier>) -> Vec<MemoryEntry>`
   - `delete_memory(id: String) -> ()`
   - `clear_memories(tier: Option<MemoryTier>) -> ()`
   - `pin_memory(conversation_id: String, content: String) -> ()`
   - `trigger_memory_consolidation() -> ()`

8. **`crates/core/src/memory_store_tests.rs`**：TDD 测试
   - `tc_memory_001`：从对话提取记忆（Mock LLM 返回关键事实）
   - `tc_memory_002`：记忆提升 Wing → Hall → Room
   - `tc_memory_003`：记忆遗忘（低重要性被删除）
   - `tc_memory_004`：相关记忆检索（关键词匹配）
   - `tc_memory_005`：记忆整合（超限时自动提升/遗忘）
   - `tc_memory_006`：记忆注入到 chat prompt
   - `tc_memory_007`：访问计数和最后访问时间更新

### 验收标准
- `cargo test` 全部通过
- `cargo clippy --all-targets -- -D warnings` 零警告
- 记忆提取是 LLM 辅助的（非纯规则）
- 记忆注入不破坏现有 RAG 流程（可禁用）
- 记忆整合由 AutoDream 后台触发

### 文档同步
- `docs/SRS_v1.0.md`：新增 REQ-RAG-032（持久化记忆系统）
- `docs/api/code-contracts.md`：新增 `MemoryStore` / `MemoryEntry` / `MemoryTier` 契约
- `docs/api/database-schema.md`：新增 `memory_entries` 表
- `docs/api/ipc-commands.md`：新增 5 个记忆管理命令
- `docs/api/settings-reference.md`：新增 `memory.enabled` / `memory.auto_consolidate` 键
- `./sync-to-github.sh --commit`
```

---

## 实施顺序与依赖关系

```
Phase 1（知识图谱）:
  Session 34（数据模型+存储）→ Session 35（图谱检索+集成）

Phase 2（质量门控+学习）:
  Session 36（质量门控）→ Session 37（自学习引擎）

Phase 3（高级 Agent）:
  Session 38（Hooks 系统）→ Session 39（DAG 工作流）→ Session 40（子代理舰队）
  （Session 40 依赖 38+39）

Phase 4（代码感知）:
  Session 41（符号引擎）→ Session 42（执行沙箱）
  Session 43（记忆系统）可独立实施

推荐并行：
  Phase 1 与 Phase 2 可并行（无依赖）
  Phase 4 Session 43 可与 Phase 3 并行（无依赖）
```

## 验收总门禁

每个 Session 完成后必须通过：
1. `cargo test`（Free）全量通过
2. `cargo test --features pro`（Pro）全量通过
3. `cargo clippy --all-targets -- -D warnings`（Free）零警告
4. `cargo clippy --all-targets --features pro -- -D warnings`（Pro）零警告
5. `cargo fmt --check` 通过
6. `node scripts/build-ui.mjs` 成功（涉及前端改动时）
7. `./sync-to-github.sh --commit` 已执行（涉及源代码改动时）
8. 文档同步完成（SRS / code-contracts / ipc-commands / database-schema / settings-reference）
