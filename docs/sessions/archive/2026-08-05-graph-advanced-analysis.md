# Session 5: 知识图谱高级分析（2026-08-05）

## 目标

在 Session 1-4 的知识图谱基础上，新增三大高级分析功能：
1. 图布局算法切换（force / hierarchical / radial）
2. 路径分析（BFS 最短路径 + 高亮）
3. 社区检测（Label Propagation 算法 + 着色）

## 实现内容

### Rust 后端

| 文件 | 变更类型 | 内容 |
|---|---|---|
| `crates/core/src/graph_analyzer.rs` | **新建** | GraphAnalyzer 图分析引擎：shortest_path() BFS + all_paths() DFS + detect_communities() Label Propagation + degree_centrality() |
| `crates/core/src/graph_analyzer_tests.rs` | **新建** | 10 个 TDD 单元测试（TC-GRAPH-ANALYSIS-001~010） |
| `crates/core/src/lib.rs` | 修改 | 注册 graph_analyzer 模块 + Storage trait 新增 get_entity_graph() 方法 |
| `crates/infra/src/sqlite_storage.rs` | 修改 | 实现 get_entity_graph()（全量邻接表 HashMap 查询，无向图双向添加） |
| `crates/models/src/lib.rs` | 修改 | 新增 GraphPath（path + hops）/ GraphCommunity（communities + community_count）数据模型 |
| `crates/tauri-app/src/commands.rs` | 修改 | 新增 3 个 IPC 命令 + *_inner 函数 |
| `crates/tauri-app/src/lib.rs` | 修改 | 注册 3 个新 IPC 命令 |

### 新增 IPC 命令（3 个）

- `get_shortest_path(from: String, to: String) -> GraphPath` — BFS 最短路径
- `get_communities() -> GraphCommunity` — Label Propagation 社区检测
- `get_graph_layout(mode: Option<String>) -> Vec<String>` — 布局模式验证

### 前端

| 文件 | 变更类型 | 内容 |
|---|---|---|
| `ui/src/graph-viewer.js` | 修改 | 布局切换器（force/hierarchical/radial）+ 路径分析面板（节点选择器 + 路径高亮）+ 社区着色 + renderNodeVisuals/bindNodeInteractions 重构 |
| `ui/styles/graph-viewer.css` | 修改 | 新增布局切换面板 + 路径分析面板 + 社区检测面板 + 路径高亮样式 |
| `ui/locales/en.json` | 修改 | 新增 graph.layout_*/graph.path_*/graph.community_* i18n 键 |
| `ui/locales/zh-CN.json` | 修改 | 新增 graph.layout_*/graph.path_*/graph.community_* i18n 键 |

### E2E 测试

| 文件 | 变更类型 | 内容 |
|---|---|---|
| `e2e-tests/tests/graph-viewer.spec.ts` | 修改 | 新增 8 个 E2E 测试（TC-GRAPH-VIZ-019~026） |
| `e2e-tests/tests/bridge/tauri-stub.js` | 修改 | 新增 get_shortest_path / get_communities / get_graph_layout mock |

### 文档

| 文件 | 变更类型 | 内容 |
|---|---|---|
| `docs/SRS_v1.0.md` | 修改 | REQ-RAG-027 新增 AC-32~AC-40 + 契约更新 |
| `docs/api/ipc-commands.md` | 修改 | 命令数 93→96 + 新增 3 个命令详情 |
| `docs/api/code-contracts.md` | 修改 | Storage trait 新增 get_entity_graph + GraphAnalyzer 契约 + GraphPath/GraphCommunity 模型 |
| `docs/api/frontend-modules.md` | 修改 | §7.3 新增布局/路径/社区函数 |

## 关键设计

1. **图分析算法**：在 Rust 后端实现（crates/core/src/graph_analyzer.rs），纯 Rust 零外部依赖。算法接收邻接表 HashMap<String, Vec<String>> 作为输入，不依赖具体 Storage 实现。
2. **布局切换**：使用 D3 内置布局——force（d3.forceSimulation）/ hierarchical（d3.tree）/ radial（d3.tree + 极坐标），不引入额外布局库。
3. **路径分析**：BFS 最短路径 + 限制深度 DFS 所有路径（最大深度 5 防止指数爆炸）。
4. **社区检测**：Label Propagation 算法（纯 Rust 实现），迭代直到收敛或最大 10 轮。
5. **数据量控制**：get_entity_graph() 一次查询返回全量邻接表，分析在后端内存中完成，避免多次 IPC 往返。
6. **面板模式**：复用 Session 3-4 的全屏 overlay + Focus Trap + 工具栏。
7. **i18n**：所有新增 UI 文案使用 t('graph.*') 调用，中英双语。
8. **Tailwind 样式铁律十三**：使用设计令牌，不硬编码 hex。
9. **esbuild 打包**：graph-viewer.js 作为 ES module 编写。
10. **铁律五**：零 unwrap/panic/unimplemented。

## 测试结果

### Rust 单元测试（10 个新增）
- TC-GRAPH-ANALYSIS-001~010：全部通过 ✅
  - 001: 最短路径——直接相邻两实体返回 1 跳路径
  - 002: 最短路径——间接关联两实体返回 2 跳路径
  - 003: 最短路径——无路径返回空 Vec
  - 004: 最短路径——自环返回单节点路径
  - 005: 所有路径——限制最大深度 3，返回所有 ≤3 跳路径
  - 006: 社区检测——完全连通图返回单一社区
  - 007: 社区检测——两个不连通子图返回两个社区
  - 008: 社区检测——Label Propagation 收敛后稳定
  - 009: 度中心性——hub 节点 centrality 最高
  - 010: 空图降级——所有分析返回空结果不报错

### E2E 测试（8 个新增）
- TC-GRAPH-VIZ-019~026：全部通过 ✅（26/26 总计通过）
  - 019: 布局切换器存在且包含 3 个选项
  - 020: 切换到 hierarchical 布局后图谱重新排列
  - 021: 切换到 radial 布局后图谱重新排列
  - 022: 路径分析面板存在且包含起点/终点选择器
  - 023: 选择起点终点后显示最短路径
  - 024: 路径上的节点高亮显示
  - 025: 社区检测按钮存在且可点击
  - 026: 点击社区检测后节点按社区着色

### 验证结果
- `cargo build`：✅ 通过
- `cargo test -p echomind-core -- graph_analyzer`：10 passed, 0 failed
- `cargo clippy --all-targets -- -D warnings`：零警告
- `cargo fmt`：已格式化
- `node scripts/build-ui.mjs`：9643 行 esbuild IIFE，成功
- E2E `graph-viewer.spec.ts`：26 passed, 0 failed（18 原有 + 8 新增）

## 新增 IPC 命令

| 命令 | 参数 | 返回值 | 说明 |
|---|---|---|---|
| `get_shortest_path` | `from: String, to: String` | `GraphPath` | BFS 最短路径 |
| `get_communities` | 无 | `GraphCommunity` | Label Propagation 社区检测 |
| `get_graph_layout` | `mode: Option<String>` | `Vec<String>` | 布局模式验证 |

## 新增数据模型

```rust
pub struct GraphPath {
    pub path: Vec<String>,  // 路径节点列表
    pub hops: usize,         // 跳数
}

pub struct GraphCommunity {
    pub communities: HashMap<String, usize>,  // 实体→社区 ID
    pub community_count: usize,                 // 社区总数
}
```

## 新增 Storage trait 方法

```rust
async fn get_entity_graph(&self) -> anyhow::Result<HashMap<String, Vec<String>>>;
```

## 前端新增函数

| 函数 | 说明 |
|---|---|
| `switchLayout(mode)` | 切换布局模式（force/hierarchical/radial） |
| `findPath()` | 查找最短路径并高亮 |
| `toggleCommunities()` | 切换社区检测着色 |
| `renderPathSelectors()` | 渲染路径分析选择器 |
| `applyPathHighlight()` | 应用路径高亮 |
| `renderForceLayout()` | 渲染力导向布局 |
| `renderHierarchicalLayout(radial)` | 渲染层次/辐射布局 |
| `renderNodeVisuals()` | 渲染节点视觉元素 |
| `bindNodeInteractions()` | 绑定节点交互事件 |
