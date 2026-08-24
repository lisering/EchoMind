# EchoMind v1.17 Session 提示词

> 复制下方 --- 之间的全部内容作为新会话的第一条消息。

---

## 项目

EchoMind — Rust + Tauri v2 本地 RAG 知识库。六边形架构 8 crate，版本 v1.16.0，1134 Rust passed / 0 failed / 31 ignored。211 Free IPC / 218 Pro。esbuild 18299 行。

上一 Session（v1.16）已完成：REQ-IX-002 会话拖拽排序（sort_order 列 + reorder_conversations IPC + HTML5 DnD）+ REQ-VEC-010 KB 统计增强（vector_count + status_distribution）+ REQ-A11Y-001 ARIA 语义标注（role/aria-live/aria-label）+ REQ-A11Y-004 屏幕阅读器支持（srStatus/srError + aria-hidden + document.title）。16 E2E 测试全量通过。GitHub 已提交。

## 本 Session 任务

### 第一步：SRS 状态修正（必做，5 分钟）

以下 REQ 已实现但 SRS 仍标 Draft，逐条改为 Implemented：
- REQ-ING-016（.html 导入，v1.1 S45 已实现）
- REQ-RAG-011（会话搜索，sidebar.js 已实现）
- REQ-ERR-003（离线降级，v1.13 已实现）
- REQ-OBS-002（性能指标，v1.13 已实现）
- REQ-WIN-004（开机自启，v1.13 已实现）

### 第二步：实施 3-4 个高 ROI Draft REQ

候选（按 ROI 排序）：

1. **REQ-A11Y-005 高对比度模式适配**（P2，纯 CSS，`[data-theme="high-contrast"]` 变量覆盖，类似浅色主题模式）
2. **REQ-DS-004 图标系统**（P2，统一 SVG 图标组件，纯前端）
3. **REQ-RAG-018 用户提问建议**（P2，空知识库引导 + 首次对话建议词，纯前端）
4. **REQ-EXP-003 全量数据恢复**（P2，导出/导入 JSON 备份，需后端 IPC）

### 执行步骤

1. 先修正 SRS 状态（5 个已实现 REQ）
2. 评估候选 ROI，选定 3-4 个实施
3. TDD 实现（先红灯测试再代码）
4. 验证全链路：clippy + fmt + test + tsc + esbuild + E2E
5. 更新 SRS 状态 + CURRENT_STATE.md
6. `./sync-to-github.sh --commit`

## 开发规范

- TDD：先写红灯测试再实现
- 零 unwrap/expect/panic（workspace deny lint）
- 文档同步：代码变更同步 docs
- Tailwind design tokens，禁止硬编码 hex
- E2E 测试模式：`setupPage(page)` 注入 mock → 操作 UI → 断言

## 验证命令

```bash
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo test
npx tsc --noEmit
node scripts/build-ui.mjs
cd e2e-tests && npx playwright test tests/v1.17-features.spec.ts --reporter=line
./sync-to-github.sh --commit
```

## 当前测试基线

| 指标 | 数值 |
|---|---|
| Rust Free | 1134 passed, 31 ignored |
| Clippy | 零警告 |
| tsc | 0 errors |
| esbuild | 18299 行 |
| E2E (v1.16) | 16 passed |
| 版本号 | 1.16.0 |
| IPC | 211 Free / 218 Pro |

## 关键文件

| 文件 | 用途 |
|---|---|
| `docs/sessions/CURRENT_STATE.md` | 项目状态快照（必读） |
| `docs/SRS_v1.0.md` | 需求规格（~13 个 Draft REQ，含 5 个待修正） |
| `.catpaw/rules/system-constraints.md` | 铁律 21 条 |
| `ui/src/main.js` | 前端入口（loadConversations/newChat/boot） |
| `ui/styles/tokens.css` | CSS 变量（主题/动效/圆角/阴影） |
| `e2e-tests/tests/helpers.mjs` | E2E 测试工具（setupPage/injectStub） |

---
