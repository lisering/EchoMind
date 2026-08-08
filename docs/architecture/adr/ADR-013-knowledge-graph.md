# ADR-013: 知识图谱实体关系检索与可视化

> **状态**：Accepted ｜ **日期**：2026-08-05 ｜ **决策者**：PM / TPM / DEV
>
> **关联文档**：
> - `docs/SRS_v1.0.md` §4.27 RAG（REQ-RAG-026 / REQ-RAG-027，AC-1~AC-40）
> - `docs/api/ipc-commands.md` §2.47 知识图谱 IPC 命令（18 个）
> - `docs/api/code-contracts.md` §8.8 Storage trait 实体关系方法 + GraphAnalyzer + GraphRetriever
> - `docs/api/database-schema.md` §1.8 entity_relations 表
> - `docs/api/settings-reference.md` `rag.graph_retriever_enabled`

---

## 1. 背景

EchoMind 是面向律师、研究员等高隐私场景的本地 RAG 知识库桌面应用。在知识图谱功能前，检索管线包含三路 RRF 融合（向量 + BM25 + 实体匹配），但缺乏**实体间关系感知**能力——当查询涉及"A 依赖 B"或"A 是 B 的一部分"等结构化关系时，仅靠向量相似度和关键词匹配无法精确定位到通过关系间接关联的 chunk。

### 1.1 现状瓶颈

| 维度 | 优化前 | 严重度 |
|---|---|---|
| 检索通道 | 3 路（Vector + BM25 + Entity） | ⚠️ 缺少关系推理 |
| 实体关系感知 | 无 | 🔴 结构化查询无法命中 |
| 可视化分析 | 无 | 🔴 无法直观探索知识结构 |
| 路径分析 | 无 | 🔴 无法发现间接关联 |

### 1.2 调研来源

- **mnemo** (2026) — 知识图谱 + 实体关系数据层架构，Triple 存储 + 图遍历检索
- **Mem0** (2026.04) — 实体链接 + 多信号检索，LoCoMo 92.5
- **Anthropic Contextual Retrieval** (2024.09) — 检索失败率 ↓49%
- **Raghavan et al.** (2007) — Label Propagation 近线性时间社区检测算法
- **D3.js v7** — Force-directed graph 可视化标准方案

---

## 2. 决策

采用 **5 Session 渐进式实施**方案，分三个维度同时推进：

### 2.1 数据层：实体关系抽取与存储

| Session | 技术 | 需求 | 核心实现 |
|---|---|---|---|
| S1 | 实体关系数据层 | REQ-RAG-026 | EntityRelation / GraphTriple 模型 + extract_relations() 纯规则 8 种关系 |

**设计决策**：
- 关系抽取使用**纯规则模式匹配**（零 LLM），中英文 8 种关系类型：`defined_as` / `part_of` / `depends_on` / `uses` / `implements` / `extends` / `references` / `related_to`
- 精确匹配置信度 1.0，模糊匹配置信度 0.7，同句共现兜底 0.5
- 按句子切分文本，在每句中查找实体对之间的关系模式
- 同一 chunk 内相同三元组去重
- 关系存储在 `entity_relations` 表（subject / relation_type / object / chunk_id / confidence）

### 2.2 检索层：图遍历检索 + 四路 RRF 融合

| Session | 技术 | 需求 | 核心实现 |
|---|---|---|---|
| S2 | 图遍历检索器 | REQ-RAG-027 AC-1~AC-6 | GraphRetriever BFS 遍历 + rrf_fuse_four_way() |

**设计决策**：
- 图扩展作为 RRF 融合的**第四路检索通道**，权重 0.5（低于 Vector=1.0 / BM25=0.8 / Entity=0.6）
- BFS 遍历最大深度 2 跳（entity → related entity → chunk），防止无限扩散
- 距离衰减：1 跳 confidence × 1.0，2 跳 confidence × 0.5
- 已访问实体去重（HashSet 防止环路）
- 空图降级：查询无实体或实体无关系时返回空 Vec（不报错），使管线优雅降级为三路 RRF
- `rag.graph_retriever_enabled` 默认 false，用户主动开启

### 2.3 可视化层：D3.js 力导向图

| Session | 技术 | 需求 | 核心实现 |
|---|---|---|---|
| S3 | D3 可视化基础 | REQ-RAG-027 AC-7~AC-23 | D3 v7 forceSimulation + 全屏 overlay + 节点交互 |
| S4 | 可视化增强 | REQ-RAG-027 AC-24~AC-31 | 实体类型图标 + 子图过滤 + 搜索定位 + SVG/PNG 导出 |

**设计决策**：
- D3.js v7 vendored 本地（`ui/vendor/d3.min.js`），无 CDN 依赖
- 8 种关系类型颜色编码（defined_as=蓝 / part_of=绿 / depends_on=橙 / uses=紫 / implements=青 / extends=粉 / references=黄 / related_to=灰）
- 5 种实体类型内联 SVG 图标（person / proper_noun / tech_term / identifier / date）
- Focus Trap 确保 WAI-ARIA Dialog Modal 无障碍
- 子图过滤面板（关系类型复选框），搜索定位（D3 zoom transform 居中+缩放+高亮）
- SVG 序列化导出 + Canvas PNG 2x 转换

### 2.4 分析层：高级图分析

| Session | 技术 | 需求 | 核心实现 |
|---|---|---|---|
| S5 | 图分析引擎 | REQ-RAG-027 AC-32~AC-40 | GraphAnalyzer: shortest_path BFS + detect_communities Label Propagation + degree_centrality |

**设计决策**：
- 图分析算法在 Rust 后端实现（`crates/core/src/graph_analyzer.rs`），纯 Rust 零外部依赖
- 算法接收邻接表 `HashMap<String, Vec<String>>` 作为输入，不依赖具体 Storage 实现
- **BFS 最短路径**：经典广度优先搜索，保证最短跳数
- **所有路径**：DFS 限制最大深度 5 防止指数爆炸
- **Label Propagation 社区检测**：迭代直到收敛或最大 10 轮，标签重新编号为连续 0, 1, 2, ...
- **度中心性**：节点连接数 / (总节点数 - 1)，识别 hub 节点
- 布局切换使用 D3 内置布局（force / hierarchical / radial），不引入额外布局库
- `get_entity_graph()` 一次查询返回全量邻接表，分析在后端内存中完成，避免多次 IPC 往返

---

## 3. 架构

### 3.1 数据流（实体关系抽取 → 导入管线集成）

```
import_files IPC → ImportService.import_one()
  → Loader.load() → Splitter.split()
  → Storage.add_document() + add_chunk()
  → Embedder.embed_batch() → Storage.add_embedding()
  → EntityExtractor.extract(text) → Storage.add_entities()     ← 实体索引
  → EntityExtractor.extract_relations(text, chunk_id)           ← 关系抽取
  → Storage.add_relation() / add_relations_batch()             ← 关系持久化
  → doc_status_changed event
```

集成路径覆盖 4 条导入管线：常规 `index_inner`、Markdown mmap、Text mmap、Proposition 重建。

### 3.2 数据流（知识图谱检索）

```
chat IPC → chat_inner()
  → (if rag.graph_retriever_enabled)
  → GraphRetriever.expand(query, top_k=5)
    → EntityExtractor.extract(query) → query_entities
    → BFS 遍历: Storage.get_relations_for_entity(entity)
      → 1-hop: confidence × 1.0
      → 2-hop: confidence × 0.5 (distance decay)
    → Storage.get_chunk_by_id(chunk_id) → RetrievalResult
  → rrf_fuse_four_way(vector, keyword, entity, graph, top_k)
    → score = Σ weight_i / (60 + rank_i + 1)
    → weights: Vector=1.0, BM25=0.8, Entity=0.6, Graph=0.5
  → threshold filter → chunk expansion → build RAG prompt → LLM stream
```

### 3.3 数据流（知识图谱可视化与分析）

```
openGraphViewer (前端) → get_graph_data() → D3 forceSimulation 渲染
  → 节点拖拽/缩放/高亮交互
  → 子图过滤（关系类型复选框）
  → 搜索定位（zoom transform 居中+缩放+高亮）
  → SVG/PNG 导出

get_shortest_path(from, to) → GraphAnalyzer.shortest_path(BFS)
  → 前端路径分析面板高亮路径节点

get_communities() → GraphAnalyzer.detect_communities(Label Propagation)
  → 前端社区着色（10 色调色板）

get_graph_layout(mode) → 布局验证 → D3 布局切换（force/hierarchical/radial）
```

---

## 4. 关键类型

### 4.1 数据模型

```rust
// 实体关系（知识图谱边）
pub struct EntityRelation {
    pub subject: String,       // 主体实体
    pub relation_type: String, // 关系类型（8 种）
    pub object: String,        // 客体实体
    pub chunk_id: String,      // 来源 chunk
    pub confidence: f32,       // 置信度（1.0/0.7/0.5）
}

// 三元组（用于批量导入）
pub struct GraphTriple {
    pub subject: String,
    pub relation_type: String,
    pub object: String,
    pub chunk_id: String,
    pub confidence: f32,
}

// 最短路径
pub struct GraphPath {
    pub path: Vec<String>,  // 路径节点列表
    pub hops: usize,         // 跳数
}

// 社区检测结果
pub struct GraphCommunity {
    pub communities: HashMap<String, usize>,  // 实体→社区 ID
    pub community_count: usize,                 // 社区总数
}

// 图统计
pub struct GraphStats {
    pub entity_count: usize,
    pub relation_count: usize,
    pub relation_types: HashMap<String, usize>,
}
```

### 4.2 Storage trait 新增方法

```rust
// 实体关系 CRUD
async fn add_relation(&self, relation: EntityRelation) -> Result<()>;
async fn add_relations_batch(&self, relations: Vec<EntityRelation>) -> Result<()>;
async fn get_relations_for_entity(&self, entity: &str) -> Result<Vec<EntityRelation>>;
async fn list_all_relations(&self) -> Result<Vec<EntityRelation>>;
async fn count_relations(&self) -> Result<usize>;

// 图分析支持
async fn get_entity_graph(&self) -> Result<HashMap<String, Vec<String>>>;  // 全量邻接表
async fn get_chunk_by_id(&self, chunk_id: &str) -> Result<Option<Chunk>>;
async fn get_entity_types(&self) -> Result<Vec<String>>;
```

---

## 5. IPC 命令（18 个）

### 5.1 检索

| 命令 | 参数 | 返回值 |
|---|---|---|
| `set_graph_retriever_enabled` | `enabled: bool` | `()` |

### 5.2 可视化

| 命令 | 参数 | 返回值 |
|---|---|---|
| `get_graph_data` | 无 | `GraphData` |
| `get_entity_relations` | `entity: String` | `Vec<EntityRelation>` |
| `get_graph_stats` | 无 | `GraphStats` |
| `get_entity_types` | 无 | `Vec<String>` |

### 5.3 高级分析

| 命令 | 参数 | 返回值 |
|---|---|---|
| `get_shortest_path` | `from: String, to: String` | `GraphPath` |
| `get_communities` | 无 | `GraphCommunity` |
| `get_graph_layout` | `mode: Option<String>` | `Vec<String>` |

---

## 6. 数据库 Schema

### 6.1 entity_relations 表

```sql
CREATE TABLE entity_relations (
    id TEXT PRIMARY KEY,
    subject TEXT NOT NULL,
    relation_type TEXT NOT NULL,
    object TEXT NOT NULL,
    chunk_id TEXT NOT NULL REFERENCES chunks(id) ON DELETE CASCADE,
    confidence REAL NOT NULL DEFAULT 1.0
);
CREATE INDEX idx_entity_relations_subject ON entity_relations(subject);
CREATE INDEX idx_entity_relations_object ON entity_relations(object);
CREATE INDEX idx_entity_relations_chunk ON entity_relations(chunk_id);
```

### 6.2 entities 表（已有，Session 1 前已存在）

```sql
CREATE TABLE entities (
    id TEXT PRIMARY KEY,
    chunk_id TEXT NOT NULL REFERENCES chunks(id) ON DELETE CASCADE,
    entity_text TEXT NOT NULL,
    entity_type TEXT NOT NULL
);
CREATE INDEX idx_entities_chunk ON entities(chunk_id);
CREATE INDEX idx_entities_text ON entities(entity_text);
```

---

## 7. 测试覆盖

### 7.1 Rust 单元测试

| 测试组 | 数量 | 覆盖范围 |
|---|---|---|
| TC-RELATION-001~006 | 6 | 关系抽取（8 种关系类型 + 去重 + 空文本降级） |
| TC-GRAPH-001~006 | 6 | 图遍历检索（BFS + 距离衰减 + 四路 RRF 融合 + 空图降级） |
| TC-GRAPH-ANALYSIS-001~010 | 10 | 图分析（最短路径 + 所有路径 + 社区检测 + 度中心性 + 空图降级） |

### 7.2 E2E 测试

| 测试组 | 数量 | 覆盖范围 |
|---|---|---|
| TC-GRAPH-VIZ-001~010 | 10 | D3 可视化基础（面板渲染 + 节点交互 + 关系颜色 + Focus Trap + 空状态） |
| TC-GRAPH-VIZ-011~018 | 8 | 可视化增强（实体图标 + 子图过滤 + 搜索定位 + SVG/PNG 导出 + 详情面板） |
| TC-GRAPH-VIZ-019~026 | 8 | 高级分析（布局切换 + 路径分析 + 社区检测 + 着色） |

---

## 8. 关键设计约束

1. **零 LLM 关系抽取**：所有关系通过规则模式匹配（中英文精确 + 模糊 + 同句共现兜底），无 LLM 调用
2. **图遍历最大 2 跳**：防止无限扩散，1 跳 confidence × 1.0，2 跳 × 0.5
3. **四路 RRF 融合权重**：Vector=1.0 > BM25=0.8 > Entity=0.6 > Graph=0.5（图扩展是间接信号）
4. **空图降级**：所有分析和检索方法在空图时返回空结果，不报错
5. **Label Propagation 最大 10 轮**：防止不收敛，标签重新编号为连续 ID
6. **所有路径最大深度 5**：防止 DFS 指数爆炸
7. **D3 vendored 本地**：`ui/vendor/d3.min.js`，无 CDN 依赖
8. **铁律五**：零 unwrap/panic/unimplemented（workspace deny lint 强制）
9. **铁律十三**：使用设计令牌（`bg-surface-2` / `text-text-primary` 等），不硬编码 hex

---

## 9. 实施总结

| Session | 功能 | 测试 | 状态 |
|---|---|---|---|
| S1 | 实体关系数据层 | 6 Rust | ✅ |
| S2 | 图遍历检索管线 | 6 Rust | ✅ |
| S3 | D3 可视化基础 | 10 E2E | ✅ |
| S4 | 可视化增强 | 8 E2E | ✅ |
| S5 | 高级图分析 | 10 Rust + 8 E2E | ✅ |
| **总计** | **5 Session** | **22 Rust + 26 E2E** | **✅ 全部完成** |

---

## 10. 后续方向

- **VIZ Phase 4+**: G6/Three.js 专业领域可视化（大型图谱性能优化）
- **关系抽取增强**: LLM 辅助复杂关系抽取（当前纯规则覆盖率有限）
- **图谱推理**: 基于图谱的推理问答（Knowledge Graph Reasoning）
- **跨文档关系**: 发现不同文档间实体的隐含关系
- **图谱导出**: GraphML / JSON-LD 标准格式导出
