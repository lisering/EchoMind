# EchoMind v1.24 Session 提示词

## 项目概要

EchoMind — Rust + Tauri v2 本地 RAG 知识库。六边形架构 8 crate，版本 v1.20.0。SRS 250 REQ 100% 完成。

## 上 Session 摘要（E2E 全量修复 + 中文 IME 防护）

### 1. E2E 测试全量修复（1177 passed 187 failed → 1243 passed 0 failed）

- `comprehensive-ux.spec.ts` 2.12 向导完成：向导 step 3 需点击 `#wizFinish` 而非 `#wizStart3`
- `edit-scroll-comprehensive.spec.ts` EDIT-STREAM-001：快速 mock 下用 `Promise.race` 检测禁用态/完成态
- `visual-regression.spec.ts`：快照更新，21/21 通过

### 2. 中文输入法（IME）组合事件防护

新建 `ui/src/ime-guard.js` 模块，提供 `isComposingEvent(e)` + `createImeGuard()` 工具。修复 12 个文件的 Enter 键处理：

| 文件 | 修复内容 |
|---|---|
| `message-edit.js` | `createImeGuard()` 阻止组合期间按钮启用 + `isComposingEvent()` 阻止 Enter 发送 |
| `input-keymap.js` | 顶部统一 `isComposingEvent()` 拦截，组合中不处理任何按键 |
| `code-executor.js` / `graph-viewer.js` / `global-search.js` / `workspace.js` / `sidebar.js` / `durable-panel.js` / `command-palette.js` / `context-menu.js` / `paywall.js` / `security.js` | 各自 Enter handler 添加 `isComposingEvent()` 检查 |

## 当前状态

| 指标 | 数值 |
|---|---|
| Rust Free | 1282 passed, 0 failed, 31 ignored |
| Rust Pro | 1542 passed, 1 failed, 36 ignored（预存 `tc_audit_006`） |
| Clippy Free+Pro | 零警告 |
| fmt / tsc / esbuild | 通过 / 0 errors / 1730 行 |
| Vitest | 430 passed, 0 failed |
| **E2E 全量** | **1243 passed, 0 failed, 10 skipped** ✅ |
| 版本号 | 1.20.0 |
| IPC | 220 Free / 230 Pro |
| 前端模块数 | 76（新增 `ime-guard.js`） |

## 本 Session 可选方向

### 方向 A：版本发布 v1.22.0

E2E 已全量通过，可直接发布：
- `cargo build --release --features pro` 验证
- 版本号升级 1.20.0→1.22.0（7 crate + tauri.conf.json）
- `CHANGELOG_v1.22.md` + git tag
- `./sync-to-github.sh --commit`

### 方向 B：新功能规划

SRS 250 REQ 全部完成，需新增 REQ。候选：
- 多窗口支持 / 插件系统 / Markdown 编辑器增强 / 移动端适配

### 方向 C：技术债

- 修复 Pro 1 failed（`tc_audit_006`：设 `ALPHA_ALL_FEATURES_FREE=false`）
- 前端模块 76 个，考虑进一步拆分或合并

## 开发规范（铁律摘要）

- TDD 先红后绿 | 零 unwrap/expect/panic
- 改前端后 `node scripts/build-ui.mjs` 重建
- 铁律十一：改源码后必须 `./sync-to-github.sh`
- 样式铁律十三：Tailwind 工具类 + design tokens

## 验证命令

```bash
cargo clippy --all-targets -- -D warnings
cargo fmt --check && cargo test
npx tsc --noEmit && node scripts/build-ui.mjs
cd e2e-tests && npm run test:unit
cd e2e-tests && npx playwright test --reporter=line
./sync-to-github.sh --commit
```

## 关键文件

| 文件 | 用途 |
|---|---|
| `ui/src/ime-guard.js` | **新增** IME 组合防护工具 |
| `ui/src/message-edit.js` | 编辑模式（本 Session 修复 IME + compositionend 补检） |
| `ui/src/input-keymap.js` | 主输入框 keydown（本 Session 顶部统一 IME 拦截） |
| `docs/sessions/CURRENT_STATE.md` | 项目状态快照 |
| `docs/SRS_v1.0.md` | 需求规格（250 REQ 全部完成） |
