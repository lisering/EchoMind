# EchoMind v1.16 Session 提示词

> 复制下方 --- 之间的全部内容作为新会话的第一条消息。

---

## 项目

EchoMind — Rust + Tauri v2 本地 RAG 知识库（律师/研究者高隐私场景）。六边形架构 8 crate，版本 v1.15.0，1279 Rust passed / 0 failed / 24 ignored。210 Free IPC / 218 Pro。esbuild 18164 行。

上一 Session（v1.15）已完成：REQ-KB-005 快捷键帮助面板搜索过滤（filterShortcutGroups + #keyboardHelpSearch + 编辑分组 + i18n）+ REQ-DS-006 动效与过渡规范（--duration-*/--ease-*/--anim-* + @keyframes modalIn/toastIn）+ REQ-IX-004 悬停反馈规范（--hover-* + .msg-actions/.copy-btn/.cite-ref design token 统一）+ REQ-DS-007 圆角与阴影体系（--radius-*/--shadow-* + :focus-visible）。无新增 IPC、无新增依赖。11 E2E 测试全量通过。GitHub 已提交。

## 本 Session 任务：v1.16 实施剩余高 ROI Draft REQ

剩余 17 个 Draft REQ，筛选纯前端高 ROI 候选实施。

### 候选功能（按 ROI 排序）

1. **REQ-IX-002 会话列表拖拽排序**（P2，前端拖拽交互，需 reorder_conversations IPC 或纯前端排序）
2. **REQ-UI-006 窗口标题动态显示**（P2，document.title = 当前会话标题，1 文件改动）
3. **REQ-VEC-010 知识库索引统计仪表盘**（P2，已有 get_kb_stats IPC + kb-stats.js，可增强索引统计展示）
4. **REQ-A11Y-001 ARIA 语义标注**（P2，role/aria-label 属性补全）
5. **REQ-A11Y-004 屏幕阅读器支持**（P2，aria-live 区域 + 状态播报）

### 执行步骤

1. 评估候选 ROI，选定 3-5 个实施
2. TDD 实现（先红灯测试再代码）
3. 验证全链路：clippy + fmt + test + tsc + esbuild + E2E
4. 更新 SRS 状态 + CURRENT_STATE.md
5. `./sync-to-github.sh --commit`

## 开发规范

- TDD：先写红灯测试再实现
- 零 unwrap/expect/panic（workspace deny lint）
- forbid unsafe
- 文档同步：代码变更同步 docs
- GitHub 同步：会话结束前 `./sync-to-github.sh`
- Tailwind design tokens，禁止硬编码 hex
- Spec-driven：先写规格再写代码

## 验证命令

```bash
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo test
npx tsc --noEmit
node scripts/build-ui.mjs
cd e2e-tests && npm run test:bridge
./sync-to-github.sh --commit
```

## 当前测试基线

| 指标 | 数值 |
|---|---|
| Rust Free | 1279 passed, 0 failed, 24 ignored |
| Clippy | 零警告 |
| cargo fmt | 通过 |
| TypeScript | 0 errors |
| esbuild | 18164 行 |
| E2E | 1095 passed, 10 skipped |
| 版本号 | 1.15.0 |
| IPC 命令 | 210 Free / 218 Pro |

## v1.15 已完成功能摘要

| 功能 | REQ | 关键文件 |
|---|---|---|
| 快捷键帮助面板搜索过滤 | REQ-KB-005 | `ui/src/keyboard-help.js`（filterShortcutGroups + 搜索框 + 编辑分组）|
| 动效与过渡规范 | REQ-DS-006 | `ui/styles/tokens.css`（--duration-*/--ease-*/--anim-* + @keyframes modalIn/toastIn）|
| 悬停反馈规范 | REQ-IX-004 | `ui/styles/tokens.css`（--hover-*）+ `ui/styles/components.css`（design token 过渡统一）|
| 圆角与阴影体系 | REQ-DS-007 | `ui/styles/tokens.css`（--radius-*/--shadow-* + :focus-visible）|

## 关键文件

| 文件 | 用途 |
|---|---|
| `docs/sessions/CURRENT_STATE.md` | 项目状态快照（必读） |
| `docs/SRS_v1.0.md` | 需求规格（17 个 Draft REQ） |
| `.catpaw/rules/system-constraints.md` | 铁律 21 条 |

---
