# EchoMind v1.25 Session 提示词

## 项目概要

EchoMind — Rust + Tauri v2 本地 RAG 知识库。六边形架构 8 crate，版本 v1.20.0。SRS 250 REQ 100% 完成。

## 上 Session 摘要（技术债清理：Alpha 结束 + Pro 0 failed）

### 1. ALPHA_ALL_FEATURES_FREE=true → false

`config_store.rs`：Alpha 阶段正式结束，Pro 门控生效。移除 `debug_assertions+pro` 自动激活。`load_is_pro()` 简化为仅检查 settings 表。

### 2. 9 处测试分支简化

`integration.rs`：9 个 `if ALPHA_ALL_FEATURES_FREE { } else { }` 全部简化为仅非 alpha 路径。`tc_lic_004` 重命名 `alpha_all_free` → `blocked`。移除未使用 import。净减 133 行死代码。

### 3. tc_audit_006 修复

is_pro 初始为 false（不再自动激活），测试断言通过。

### 4. 前端 76 模块审视

76 个 JS 模块 30705 行 → esbuild 7300 行（76% 压缩）。拆分候选：`settings.js`(2423行) + `graph-viewer.js`(1888行)。`main.js`(1594行) 保持不动。21 个 <200 行模块健康无需合并。

## 当前状态

| 指标 | 数值 |
|---|---|
| Rust Free | **1283 passed**, 0 failed, 31 ignored |
| Rust Pro | **1553 passed**, 0 failed, 39 ignored |
| Clippy Free+Pro | 零警告 |
| fmt / tsc / esbuild | 通过 / 0 errors / 1730 行 |
| Vitest | 430 passed, 0 failed |
| E2E 全量 | **1243 passed**, 0 failed, 10 skipped ✅ |
| 版本号 | 1.20.0 |
| IPC | 220 Free / 230 Pro |
| 前端模块数 | 76 |

## 本 Session 可选方向

### 方向 A：版本发布 v1.22.0

Pro 0 failed + E2E 0 failed，可直接发布：
- `cargo build --release --features pro` 验证
- 版本号升级 1.20.0→1.22.0（7 crate + tauri.conf.json）
- `CHANGELOG_v1.22.md` + git tag
- `./sync-to-github.sh --commit`

### 方向 B：前端拆分

- `settings.js`(2423行) → `settings-general.js` + `settings-advanced.js` + `settings.js`
- `graph-viewer.js`(1888行) → `graph-renderer.js` + `graph-interactions.js` + `graph-viewer.js`
- 每个约 0.5 天工作量

### 方向 C：新功能规划

SRS 250 REQ 全部完成，需新增 REQ。候选：多窗口支持 / 插件系统 / Markdown 编辑器增强 / 移动端适配

## 开发规范（铁律摘要）

- TDD 先红后绿 | 零 unwrap/expect/panic
- 改前端后 `node scripts/build-ui.mjs` 重建
- 铁律十一：改源码后必须 `./sync-to-github.sh`
- 样式铁律十三：Tailwind 工具类 + design tokens

## 验证命令

```bash
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --features pro -- -D warnings
cargo fmt --check && cargo test && cargo test --features pro
npx tsc --noEmit && node scripts/build-ui.mjs
cd e2e-tests && npm run test:unit
cd e2e-tests && npx playwright test --reporter=line
./sync-to-github.sh --commit
```

## 关键文件

| 文件 | 用途 |
|---|---|
| `crates/tauri-app/src/stores/config_store.rs` | ALPHA_ALL_FEATURES_FREE=false（本 Session 修改） |
| `crates/tauri-app/tests/integration.rs` | 9 处测试分支简化（本 Session 修改） |
| `docs/sessions/CURRENT_STATE.md` | 项目状态快照 |
| `docs/SRS_v1.0.md` | 需求规格（250 REQ 全部完成） |
