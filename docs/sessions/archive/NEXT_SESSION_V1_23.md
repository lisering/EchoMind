# EchoMind 下一 Session 提示词

## 项目概要

EchoMind — Rust + Tauri v2 本地 RAG 知识库。六边形架构 8 crate，版本 v1.20.0。SRS 250 REQ 100% 完成。

## 上 Session 摘要（v1.22 构建体积优化）

**Rust 优化**：`[profile.release]` 新增 `panic = "abort"`（移除 unwinding 机制，预计减小 200-500KB 二进制体积）。已有 `lto = "fat"` + `strip = true` + `codegen-units = 1`。

**前端 vendor 懒加载**：新建 `ui/src/lazy-loader.js` 模块（Promise 缓存 + 动态 `<script>` 加载 + 优雅降级）。将 mermaid.min.js（3.4MB）/ d3.min.js（273KB）/ chart.umd.min.js（204KB）/ katex（265KB）从 HTML eager `<script>` 标签改为按需延迟加载。

**改造 4 个前端模块**：(1) `markdown.js` — `renderMermaid()`/`renderKatexInElement()`/`retryPendingKatex()`/`enhanceTablesForChart()` 改为 async + `await loadXxx()` + 空结果提前返回；(2) `graph-viewer.js` — `openGraphViewer()` 新增 `await loadD3()` + `initGraphResize()` 增加 `typeof d3` 守卫；(3) `main.js` — 移除 eager `mermaid.initialize()`，改用 `setMermaidInitFn()` 回调；(4) `ui/index.html` — 移除 6 个 eager script 标签。

**体积优化效果**：启动时 vendor 加载 5.1MB → 632KB（**-87.6%**，节省 4.4MB）。

## 当前状态

| 指标 | 数值 |
|---|---|
| Rust Free | 1283 passed, 0 failed, 31 ignored |
| Rust Pro | 1553 passed, 0 failed, 39 ignored |
| Clippy Free+Pro | 零警告 |
| fmt / tsc / esbuild | 通过 / 0 errors / 1730 行 |
| Vitest | 430 passed, 0 failed |
| E2E | 1226 passed, 0 failed, 10 skipped |
| 版本号 | 1.20.0 |
| IPC | 220 Free / 230 Pro |
| 前端模块数 | 79 |
| 前端 Vendor（启动） | 632 KB (eager) + 4.4 MB (lazy) |

## 本 Session 可选方向

### 方向 A：版本发布 v1.23.0
- `cargo build --release --features pro` 验证（含 panic=abort 效果）
- 版本号升级 1.20.0→1.23.0（7 crate + tauri.conf.json）
- CHANGELOG_v1.23.md + git tag

### 方向 B：新功能规划
- SRS 250 REQ 全部完成，需新增 REQ
- 候选：多窗口支持 / 插件系统 / Markdown 编辑器增强 / 移动端适配

### 方向 C：进一步体积优化
- Tailwind JIT → 预构建 CSS（441KB → ~50KB，需工具链改造）
- mermaid 按需加载子模块（仅加载使用的图表类型）
- Rust 依赖裁剪（tokio features 精简 / 条件编译去除未用功能）

## 验证命令

```bash
cargo clippy --all-targets -- -D warnings
cargo fmt --check && cargo test && cargo test --features pro
npx tsc --noEmit && node scripts/build-ui.mjs
cd e2e-tests && npm run test:unit
./sync-to-github.sh --commit
```
