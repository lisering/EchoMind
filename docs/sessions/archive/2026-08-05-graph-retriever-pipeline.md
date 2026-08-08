# Session 归档：知识图谱检索管线接入（REQ-RAG-027）

> **日期**: 2026-08-05
> **Session 类型**: 后端功能开发
> **依赖**: Session 1（知识图谱实体关系数据层）、REQ-PERF-006（EntityExtractor）、REQ-VEC-003（Storage）
> **状态**: ✅ 完成

---

## 1. 目标

将知识图谱检索接入 `chat_inner` 检索管线（启用 `rag.graph_retriever_enabled` 后，检索时沿实体关系图边扩展到关联 chunk），实现多阶段检索：chunk search → entity search → graph expansion → relation filtering → scoring

## 2. 变更清单

| 文件 | 变更类型 | 内容 |
|---|---|---|
| `crates/core/src/graph_retriever.rs` | 新建 | GraphRetriever 图遍历检索器 + `rrf_fuse_four_way()` 四路 RRF 融合函数 |
| `crates/core/src/graph_retriever_tests.rs` | 新建 | 6 个 TDD 测试（TC-GRAPH-001~006） |
| `crates/core/src/lib.rs` | 修改 | 注册 `graph_retriever` 模块 + 测试模块 + Storage trait 新增 `get_chunk_by_id` 方法 |
| `crates/infra/src/sqlite_storage.rs` | 修改 | 实现 `get_chunk_by_id` SQL 查询 |
| `crates/tauri-app/src/commands.rs` | 修改 | `set_graph_retriever_enabled` IPC 命令 + `chat_inner` 图扩展接入 |
| `crates/tauri-app/src/state.rs` | 修改 | AppState 新增 `graph_retriever_enabled` 字段 |
| `crates/tauri-app/src/lib.rs` | 修改 | 注册 `set_graph_retriever_enabled` IPC 命令 |
| `docs/SRS_v1.0.md` | 修改 | REQ-RAG-027 新增 AC-9~AC-14 + 契约更新 |
| `docs/api/ipc-commands.md` | 修改 | 命令数 88→89 + 新增 `set_graph_retriever_enabled` 详情 |
| `docs/api/code-contracts.md` | 修改 | 新增 §8.8.3 GraphRetriever 契约 + `get_chunk_by_id` 方法 |
| `docs/api/settings-reference.md` | 修改 | `rag.graph_retriever_enabled` 从「预留」更新为「已接入」 |

## 3. 新增测试

| 测试 ID | 验证内容 |
|---|---|
| TC-GRAPH-001 | 单跳图扩展（entity → related chunk） |
| TC-GRAPH-002 | 多跳图扩展（entity → entity → chunk），距离衰减验证 |
| TC-GRAPH-003 | 关系类型过滤（只沿 depends_on 扩展） |
| TC-GRAPH-004 | 置信度加权排序（高置信度优先） |
| TC-GRAPH-005 | 空图降级（无关系/无 chunk 时返回空 Vec） |
| TC-GRAPH-006 | 四路 RRF 融合（Vector+BM25+Entity+Graph 排序验证） |

## 4. 关键设计决策

### 4.1 GraphRetriever 作为独立模块

GraphRetriever 是一个泛型结构体 `GraphRetriever<S: Storage>`，不实现 `Retriever` trait。它作为 RRF 融合的第四路检索通道，由 `chat_inner` 在标准检索后调用 `expand()` 方法获取图扩展结果。

### 4.2 BFS 图遍历 + 距离衰减

- BFS 队列存储 `(entity_text, current_depth)`
- 最大深度 2 跳（防止无限扩散）
- 已访问实体去重（HashSet，防止环路）
- 距离衰减：1 跳 confidence × 1.0，2 跳 confidence × 0.5

### 4.3 四路 RRF 融合

`rrf_fuse_four_way()` 函数在现有三路 RRF（Vector 1.0 + BM25 0.8 + Entity 0.6）基础上增加第四路 Graph（权重 0.5）。图扩展命中的 chunk 获得额外 RRF 分数加成。

### 4.4 chat_inner 集成方式

在标准 RAG 路径（非 coordinator/agent/speculative）中，标准检索完成后：
1. 创建 `GraphRetriever::new(state.storage.clone())`
2. 调用 `expand(query, DEFAULT_TOP_K)` 获取图扩展结果
3. 合并：已存在 chunk 取最高分，新 chunk 追加
4. 重新排序并截取

### 4.5 get_chunk_by_id Storage trait 方法

新增 `get_chunk_by_id` 方法到 Storage trait（默认返回 None），避免图遍历后通过 `list_documents` + `list_chunks` 遍历查找 chunk。SqliteStorage 覆盖为 SQL `WHERE id = ?` 查询。

### 4.6 优雅降级

- 空查询/无实体 → 返回空 Vec
- 有实体但无关系 → 返回空 Vec
- 关系指向不存在的 chunk → 跳过该 chunk
- 图扩展失败 → 降级为标准三路 RRF

## 5. 验证结果

| 检查项 | 结果 |
|---|---|
| `cargo test -p echomind-core -- graph_retriever_tests` | ✅ 6 passed, 0 failed |
| `cargo clippy --all-targets -- -D warnings` (Free) | ✅ 零警告 |
| `cargo clippy --all-targets --features pro -- -D warnings` (Pro) | ✅ 零警告 |
| `cargo fmt --check` | ✅ 通过 |

## 6. 后续可扩展方向

1. **前端图谱可视化**：使用 G6/D3 渲染 `GraphTriple` 三元组为交互式知识图谱
2. **LLM 增强关系抽取**：对规则无法覆盖的关系，可选调用 LLM 补充抽取（BYOK 模式）
3. **图算法**：PageRank / 社区发现等图算法应用于检索排序
4. **图扩展缓存**：缓存图遍历结果，避免重复 BFS 计算
