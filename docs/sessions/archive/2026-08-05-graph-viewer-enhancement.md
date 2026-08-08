# Session 4 归档：知识图谱可视化增强（REQ-RAG-027）

> **日期**: 2026-08-05
> **Session 编号**: Session 4
> **REQ**: REQ-RAG-027（知识图谱实体关系检索）
> **状态**: ✅ Done

---

## 1. 目标

在 Session 3 完成的 D3 force-directed graph 基础上，增强交互体验：

1. 实体类型图标——根据实体类型（person/proper_noun/tech_term/identifier/date）显示不同 SVG 图标
2. 子图过滤——按关系类型筛选显示子图（复选框/Toggle 开关）
3. 搜索定位——在图谱中搜索实体并自动定位+高亮
4. 图谱导出——导出为 SVG/PNG 图片
5. 节点详情面板增强——显示实体类型徽章 + 来源 chunk 预览

---

## 2. 变更清单

### 2.1 后端（Rust）

| 文件 | 变更 |
|---|---|
| `crates/core/src/lib.rs` | Storage trait 新增 `get_entity_types(entities: &[String]) -> HashMap<String, String>` 方法（批量查询实体类型，避免 N+1） |
| `crates/infra/src/sqlite_storage.rs` | 实现 `get_entity_types`——SQL `WHERE entity_text IN (...)` 单次批量查询 `entities` 表 |
| `crates/tauri-app/src/commands.rs` | 新增 `get_entity_types` IPC 命令（接收实体文本列表 → 返回类型映射） |
| `crates/tauri-app/src/lib.rs` | 注册 `get_entity_types` 命令到 Tauri handler |

### 2.2 前端（JavaScript / CSS / i18n）

| 文件 | 变更 |
|---|---|
| `ui/src/graph-viewer.js` | 增强：实体类型图标渲染（5 种内联 SVG）+ 子图过滤面板 + 搜索定位 + SVG/PNG 导出 + 详情面板增强（实体类型徽章 + 来源 chunk 预览） |
| `ui/styles/graph-viewer.css` | 新增：搜索栏 / 过滤面板 / 实体类型图标 / 导出按钮 / 过滤隐藏状态 / 搜索高亮 / 实体类型徽章样式 |
| `ui/locales/en.json` | 新增 graph.* i18n 键（filter_title / search_placeholder / export_svg / export_png / entity_type_* / detail_type / detail_source） |
| `ui/locales/zh-CN.json` | 新增 graph.* i18n 键（中英双语） |
| `e2e-tests/tests/bridge/tauri-stub.js` | 新增 `get_entity_types` mock（返回 mock 实体→类型映射） |

### 2.3 文档

| 文件 | 变更 |
|---|---|
| `docs/SRS_v1.0.md` | REQ-RAG-027 新增 AC-24~AC-31（8 条验收标准）+ 契约更新 |
| `docs/api/ipc-commands.md` | 命令数 92→93 + 新增 §2.47.18 get_entity_types 命令详情 |
| `docs/api/code-contracts.md` | Storage trait 新增 `get_entity_types` 方法签名 |
| `docs/api/frontend-modules.md` | §7.3 新增 7 个函数（getEntityIcon / getEntityTypeName / isEdgeVisible / renderFilterPanel / applyFilter / searchAndLocate / exportSvg / exportPng）+ 6 个 CSS 类 |

---

## 3. TDD 测试

### 3.1 E2E 测试（8 个新增）

| 测试 ID | 描述 | 状态 |
|---|---|---|
| TC-GRAPH-VIZ-011 | 节点显示实体类型图标（不同类型不同图标） | ✅ Pass |
| TC-GRAPH-VIZ-012 | 子图过滤面板存在且可操作 | ✅ Pass |
| TC-GRAPH-VIZ-013 | 取消勾选关系类型后图谱边减少 | ✅ Pass |
| TC-GRAPH-VIZ-014 | 搜索框存在且可输入 | ✅ Pass |
| TC-GRAPH-VIZ-015 | 搜索实体后自动定位+高亮该节点 | ✅ Pass |
| TC-GRAPH-VIZ-016 | 导出按钮存在且可点击 | ✅ Pass |
| TC-GRAPH-VIZ-017 | 点击导出 SVG 后触发下载 | ✅ Pass |
| TC-GRAPH-VIZ-018 | 节点详情面板显示实体类型徽章 | ✅ Pass |

E2E 测试总数：18（TC-GRAPH-VIZ-001~018），全部通过。

---

## 4. 验证结果

| 验证项 | 结果 |
|---|---|
| `node scripts/build-ui.mjs` | ✅ 构建成功 |
| `npx tsc --noEmit` | ✅ 无新增错误（42 个残留为预先存在） |
| `cargo build` | ✅ 编译通过 |
| `cargo test -p echomind-core` | ✅ 全部通过 |
| `cargo test -p echomind-infra` | ✅ 全部通过 |
| `cargo test -p echomind-tauri-app` | ✅ 全部通过 |
| `cargo clippy --all-targets -- -D warnings` | ✅ 零警告 |
| `cargo fmt --check` | ✅ 通过 |
| E2E 桥接测试 | ✅ 18/18 通过 |

---

## 5. 设计决策

1. **实体类型图标**：使用内联 SVG（5 种类型 5 个图标），不引入额外图标库，保持零额外依赖
2. **子图过滤**：使用复选框 + CSS 类（`graph-edge-hidden` / `graph-node-hidden`）控制显隐，无需重建 simulation
3. **搜索定位**：使用 D3 zoom transform 实现自动居中+缩放，添加 `graph-node-searched` CSS 类高亮
4. **图谱导出**：SVG 使用 XMLSerializer 序列化 → Blob 下载；PNG 通过 Canvas 2x 分辨率转换
5. **批量查询**：`get_entity_types` 使用 SQL `WHERE entity_text IN (...)` 单次查询，避免 N+1 问题
6. **面板模式**：复用 Session 3 的全屏 overlay + Focus Trap，新增 UI 元素均在 overlay 内
7. **i18n**：所有新增 UI 文案使用 `t('graph.*')` 调用，中英双语
8. **Tailwind 样式铁律十三**：使用设计令牌，不硬编码 hex（图例/徽章颜色除外，使用与关系类型一致的语义色）

---

## 6. 关键文件索引

| 文件 | 用途 |
|---|---|
| `crates/core/src/lib.rs` | Storage trait `get_entity_types` 方法定义 |
| `crates/infra/src/sqlite_storage.rs` | SqliteStorage `get_entity_types` 实现 |
| `crates/tauri-app/src/commands.rs` | `get_entity_types` IPC 命令 |
| `ui/src/graph-viewer.js` | 增强后的图谱可视化模块 |
| `ui/styles/graph-viewer.css` | 新增样式 |
| `ui/locales/en.json` / `zh-CN.json` | i18n 键 |
| `e2e-tests/tests/graph-viewer.spec.ts` | 18 个 E2E 测试 |
| `e2e-tests/tests/bridge/tauri-stub.js` | `get_entity_types` mock |
