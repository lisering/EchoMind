# Session 归档：前端知识图谱可视化（REQ-RAG-027）

> **日期**: 2026-08-05
> **Session 类型**: 前端功能开发
> **依赖**: Session 1（知识图谱实体关系数据层）、Session 2（知识图谱检索管线接入）
> **状态**: ✅ 完成

---

## 1. 目标

将知识图谱数据（EntityRelation 三元组）以交互式可视化图谱呈现在前端，用户可以在知识库面板中查看实体节点和关系边，点击节点高亮关联边，支持缩放/拖拽。

## 2. 变更清单

| 文件 | 变更类型 | 内容 |
|---|---|---|
| `crates/core/src/lib.rs` | 修改 | Storage trait 新增 `list_all_relations(limit, offset)` + `count_relations()` 方法（默认空操作） |
| `crates/infra/src/sqlite_storage.rs` | 修改 | 实现 `list_all_relations`（SQL LIMIT/OFFSET）+ `count_relations`（SQL COUNT） |
| `crates/models/src/lib.rs` | 修改 | 新增 `GraphStats` 结构体（total_entities / total_relations / relation_type_counts） |
| `crates/tauri-app/src/commands.rs` | 修改 | 新增 3 个 IPC 命令 + `*_inner` 函数：`get_graph_data` / `get_entity_relations` / `get_graph_stats` |
| `crates/tauri-app/src/lib.rs` | 修改 | 注册 3 个新 IPC 命令（命令数 89→92） |
| `ui/vendor/d3.min.js` | 新增 | D3.js v7 本地 vendor（279KB UMD bundle，force-graph 布局用） |
| `ui/vendor/MANIFEST.json` | 修改 | 添加 d3.min.js 条目 |
| `ui/src/graph-viewer.js` | 新建 | 知识图谱可视化模块（D3 forceSimulation + 节点拖拽 + 缩放 + 关系类型颜色编码 + Focus Trap + 空状态 + 实体详情面板） |
| `ui/styles/graph-viewer.css` | 新建 | 图谱面板样式（overlay / 节点 / 边 / 工具提示 / 图例 / 工具栏 / 详情面板 / 空状态） |
| `ui/src/main.js` | 修改 | 导入 graph-viewer 模块 + 注册 #graphBtn 按钮 + ESC 关闭回调 |
| `ui/index.html` | 修改 | 添加 D3 `<script>` 标签 + 侧栏 #graphBtn 按钮 + graph-viewer.css 到 CSS 列表 |
| `ui/locales/en.json` | 修改 | 新增 `graph.*` i18n 键（13 条） |
| `ui/locales/zh-CN.json` | 修改 | 新增 `graph.*` i18n 键（13 条） |
| `scripts/build-ui.mjs` | 修改 | CSS_FILES 添加 `graph-viewer.css` |
| `types/vendor.d.ts` | 修改 | 添加 D3 全局类型声明 `declare const d3: any` |
| `e2e-tests/tests/bridge/tauri-stub.js` | 修改 | 新增 `get_graph_data` / `get_entity_relations` / `get_graph_stats` / `set_graph_retriever_enabled` mock + 8 条测试用关系数据 |
| `e2e-tests/tests/graph-viewer.spec.ts` | 新建 | 10 个 E2E 测试（TC-GRAPH-VIZ-001~010） |
| `docs/SRS_v1.0.md` | 修改 | REQ-RAG-027 新增 AC-15~AC-23（图谱可视化验收标准）+ 契约更新 |
| `docs/api/ipc-commands.md` | 修改 | 命令数 89→92 + 新增 3 个命令详情（§2.47.15~17） |
| `docs/api/code-contracts.md` | 修改 | Storage trait 新增 `list_all_relations` / `count_relations` 方法签名 |
| `docs/api/frontend-modules.md` | 修改 | 新增 §7.3 知识图谱可视化模块函数清单（14 条） |

## 3. 新增测试

| 测试 ID | 验证内容 |
|---|---|
| TC-GRAPH-VIZ-001 | 侧栏「知识图谱」按钮存在且可点击，有 aria-label |
| TC-GRAPH-VIZ-002 | 点击按钮后弹出图谱面板 overlay，role=dialog + aria-modal=true |
| TC-GRAPH-VIZ-003 | 面板内渲染 SVG 包含节点（圆形+标签）和边 |
| TC-GRAPH-VIZ-004 | 节点数量与 get_graph_data 返回的去重实体数一致 |
| TC-GRAPH-VIZ-005 | 不同关系类型显示不同颜色边，图例展示关系类型 |
| TC-GRAPH-VIZ-006 | 点击节点高亮关联边和关联节点，未关联元素暗淡 |
| TC-GRAPH-VIZ-007 | 拖拽节点后节点位置更新，节点仍可见 |
| TC-GRAPH-VIZ-008 | 空知识库时面板显示「暂无图谱数据」提示，SVG 隐藏 |
| TC-GRAPH-VIZ-009 | ESC 键关闭图谱面板 |
| TC-GRAPH-VIZ-010 | 统计栏显示实体数和关系数 |

## 4. 关键设计决策

### 4.1 D3.js v7 本地 vendor

D3.js v7 UMD bundle（279KB）下载到 `ui/vendor/d3.min.js`，通过 `<script>` 标签全局加载为 `window.d3`。铁律禁止 CDN，所有第三方库必须本地 vendor。在 `graph-viewer.js` 中直接使用全局 `d3` 变量。

### 4.2 D3 forceSimulation 力导向布局

- `forceSimulation` 配置：charge(-300) / link(80) / center(0.05) / collision(20)
- 节点大小 = `max(8, sqrt(degree + 1) * 6)`，连接数越多节点越大
- 节点拖拽：`d3.drag()` + `fx/fy` 固定位置，拖拽结束后释放
- 滚轮缩放：`d3.zoom().scaleExtent([0.1, 4])`
- 画布平移：zoom behavior 自带 drag-to-pan

### 4.3 关系类型颜色编码

8 种关系类型各有独立颜色：
- `defined_as`: #38bdf8（天蓝）
- `part_of`: #a78bfa（紫色）
- `depends_on`: #fb923c（橙色）
- `uses`: #4ade80（绿色）
- `implements`: #f472b6（粉色）
- `extends`: #facc15（黄色）
- `references`: #22d3ee（青色）
- `related_to`: #94a3b8（灰色）

### 4.4 节点高亮交互

点击节点时：
1. 高亮该节点（`graph-node-highlighted` 类，drop-shadow 发光效果）
2. 高亮所有关联边（`graph-edge-highlighted` 类，加粗 + 不透明）
3. 暗淡未关联节点和边（`graph-node-dimmed` / `graph-edge-dimmed` 类，opacity 0.1-0.3）
4. 点击空白区域取消高亮

### 4.5 全屏 Overlay + Focus Trap

- 面板使用全屏 overlay（`position: fixed; inset: 0; z-index: 99998`）
- `visibility: hidden` + `opacity: 0` 隐藏（Playwright `toBeVisible` 兼容）
- 打开时激活 Focus Trap（复用 `ui/src/focus-trap.js`）
- ESC 键关闭面板
- 右上角关闭按钮 + 工具栏（放大/缩小/重置）

### 4.6 数据量控制

默认加载前 200 条关系（`get_graph_data(limit=200)`），大量数据时统计栏显示「显示前 N 条，共 M 条关系」提示。

### 4.7 空状态处理

无关系数据时显示空状态提示（i18n: `graph.empty_hint`），SVG 隐藏（`display: none`）。

### 4.8 实体详情面板（双击节点）

双击节点时调用 `get_entity_relations` IPC 获取该实体的所有关系，在面板左侧显示详情列表（关系方向 + 关系类型 + 目标实体 + 置信度）。

## 5. 验证结果

| 检查项 | 结果 |
|---|---|
| `cargo build` | ✅ 编译通过 |
| `cargo test -p echomind-core` | ✅ 全部通过 |
| `cargo clippy --all-targets -- -D warnings` (Free) | ✅ 零警告 |
| `cargo clippy --all-targets --features pro -- -D warnings` (Pro) | ✅ 零警告 |
| `cargo fmt --check` | ✅ 通过 |
| `npx tsc --noEmit` | ✅ 42 pre-existing errors（无新增） |
| `node scripts/build-ui.mjs` | ✅ 8985 行打包脚本 |
| E2E (graph-viewer.spec.ts) | ✅ 10 passed, 0 failed |
| `./sync-to-github.sh --commit` | ✅ 11 files changed |

## 6. 后续可扩展方向

1. **实体类型图标**：根据实体类型（person/proper_noun/tech_term/identifier/date）显示不同图标
2. **图算法可视化**：PageRank 节点大小 / 社区发现节点颜色聚类
3. **关系编辑**：在图谱面板中手动添加/删除/修改关系
4. **图谱导出**：导出为 PNG/SVG 图片
5. **子图过滤**：按关系类型筛选显示子图
6. **搜索定位**：在图谱中搜索实体并自动定位+高亮
