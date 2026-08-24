# EchoMind v1.15 Session 提示词

> 复制本文件 --- 之间的全部内容作为新会话的第一条消息。

---

## 项目

EchoMind — Rust + Tauri v2 本地 RAG 知识库（律师/研究者高隐私场景）。六边形架构 8 crate，版本 v1.14.0，1279 Rust passed / 0 failed / 24 ignored。210 Free IPC / 218 Pro。esbuild 18118 行。

上一 Session（v1.14）已完成：REQ-IX-001 右键上下文菜单增强（会话/消息/文档3区域右键）+ REQ-ING-011 导入历史记录（import_logs表+IPC+管线集成）+ REQ-KB-004 文档列表快捷键（Up/Down/Enter/Delete/Esc）+ 审计日志报告导出（Markdown/JSON）。新增4 IPC：rename_conversation / get_import_history / clear_import_history / export_audit_report。9 E2E 测试已写。GitHub 已提交（推送失败需检查远程仓库配置）。

## 本 Session 任务：v1.15 实施剩余高 ROI Draft REQ

剩余 21 个 Draft REQ，筛选高 ROI 候选实施（纯前端优先，单 Session 可完成）。

### 候选功能（按 ROI 排序）

1. **REQ-KB-005 快捷键帮助面板**（P2，已有 keyboard-help.js + keyboard.js 基础，纯前端补全搜索过滤）
2. **REQ-UI-006 窗口标题动态显示**（P2，1 文件改动，显示当前会话标题）
3. **REQ-DS-006 动效与过渡规范**（P2，CSS 变量定义，纯样式）
4. **REQ-IX-004 悬停反馈规范**（P2，依赖 DS-006，CSS hover 状态统一）
5. **REQ-VEC-010 知识库索引统计仪表盘**（P2，已有 get_kb_stats IPC，缺前端图表）
6. **REQ-DS-007 圆角与阴影体系**（P2，CSS design tokens 完善）
7. **REQ-IX-002 会话列表拖拽排序**（P2，前端拖拽交互）

### 执行步骤

1. 评估候选 ROI，选定 3-5 个实施
2. TDD 实现（先红灯测试再代码）
3. 验证全链路：clippy + fmt + test + tsc + esbuild
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
| esbuild | 18118 行 |
| E2E | 1084 passed, 10 skipped |
| 版本号 | 1.14.0 |
| IPC 命令 | 210 Free / 218 Pro |

## v1.14 已完成功能摘要

| 功能 | REQ | 关键文件 |
|---|---|---|
| 右键上下文菜单增强 | REQ-IX-001 | `ui/src/context-menu.js`（会话/消息/文档3区域右键）+ `crates/tauri-app/src/commands/conversation.rs`（rename_conversation） |
| 导入历史记录 | REQ-ING-011 | `crates/infra/src/sqlite_storage.rs`（import_logs表）+ `crates/core/src/import.rs`（管线集成）+ `ui/src/ipc.js`（importHistoryApi） |
| 文档列表快捷键 | REQ-KB-004 | `ui/src/doc-keyboard.js`（新建）+ `ui/styles/components.css`（选中样式） |
| 审计日志报告导出 | 新增 | `crates/tauri-app/src/commands/security.rs`（export_audit_report，Markdown/JSON） |

## 关键文件

| 文件 | 用途 |
|---|---|
| `docs/sessions/CURRENT_STATE.md` | 项目状态快照（必读） |
| `docs/SRS_v1.0.md` | 需求规格（21 个 Draft REQ） |
| `docs/architecture/V1.14_ATOMIC_PLAN.md` | v1.14 规划（已完成） |
| `.catpaw/rules/system-constraints.md` | 铁律 21 条 |

---
