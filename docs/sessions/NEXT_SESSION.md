# NEXT_SESSION — EchoMind v3.0.0 已发布，下一步规划

> **生成日期**：2026-08-24 ｜ **生成人**：AI Agent (S98)
>
> **当前版本**：v3.0.0 ｜ **测试基线**：Free 1700 / Pro 1991 passed
>
> **关联文档**：`docs/sessions/CURRENT_STATE.md` ｜ `docs/SRS_v1.0.md` ｜ `docs/architecture/V3.0_ATOMIC_PLAN.md`

---

## 项目概要

EchoMind — Rust + Tauri v2 本地 RAG 知识库。六边形架构 8 crate：models → prompt → core → infra → tauri-app（+ compact / context / license-issuer）。**v3.0.0 已发布**，当前 Rust Free 1700 passed / Pro 1991 passed。fastembed ONNX 嵌入（5 种 EmbeddingModel），SQLite 向量存储 + HNSW 索引，任意 OpenAI 兼容 LLM。Pro 功能：本地 LLM 推理（mistral.rs）、GPU 加速、ColBERT、tree-sitter、WasmExecutor。前端 57 个 ES module（vanilla JS，esbuild 打包 IIFE 1816 行）。MCP 协议适配器 Phase 1 已完成（REQ-ARCH-016 Implemented，6 IPC 命令 + stdio/SSE 双 transport + 工具注册表 + 安全确认）。

## v3.0 发布内容（S94-S98 全部完成 ✅）

| Session | 功能 | 状态 |
|---|---|---|
| S94 | 设置面板重构（35→8 分区 + 智能模式 + 术语通俗化） | ✅ |
| S95 | 前端模块精简（72→56）+ 开发者工具隔离 | ✅ |
| S96 | 嵌入模型对比评估面板 | ✅ |
| S97 | MCP 协议适配器 Phase 1 | ✅ |
| S98 | Pro ROI 评估 + v3.0 发布 | ✅ |

### S98 验证明细

- Rust Free: 1700 passed, 0 failed, 31 ignored ✅
- Rust Pro: 1991 passed, 0 failed, 49 ignored ✅
- Clippy Free+Pro: 零警告 ✅
- cargo fmt: 通过 ✅
- tsc: 0 errors ✅
- esbuild: 1816 行 (prod) / 1471 行 (release) ✅
- Vitest: 1821 passed (92 files) ✅
- Release build: exit_code 0 (23.3m) ✅
- sync-to-github: 9 files committed ✅
- git tag v3.0.0: 创建 ✅

### Pro ROI 评估结果

14 个 Pro 功能评估完成：10 保留 Pro / 4 考虑后续下沉 / 0 Deprecated。

核心发现：本地 LLM 推理 + PDF 多模态 ROI 4.1 并列第一。

4 个下沉候选（v3.1+）：GEMV 内核 / ColBERT / ONNX 压缩器 / WasmExecutor。

### P0 状态

| P0 | 状态 |
|---|---|
| P0-1 嵌入模型升级 | ✅ 已修复 |
| P0-2 Prompt 注入防护 | ✅ 已修复 |
| P0-3 Pro 功能 ROI | ✅ 已评估 |
| P0-4 持续收入模型 | 📋 商业层面 |
| P0-5 数据库回滚 | ✅ 已修复 |
| P0-6 插件系统 | ✅ Phase 1 完成 |

**已修复 5/6 P0**。剩余 P0-4 为商业层面。

---

## 下个 Session 任务

### v3.1 规划（建议方向）

v3.0 已发布，SRS Draft REQ 再次归零。下个 Session 建议进行 v3.1 版本规划：

1. **Pro 功能下沉评估**：基于 S98 Pro ROI 评估，考虑将 4 个性能优化型功能逐步下沉 Free
   - ColBERT 多向量嵌入 → Free（提升 Free 版检索质量）
   - ONNX 嵌入压缩器 → Free（提升 Free 版 Prompt 压缩）
   - WasmExecutor 代码沙箱 → Free（扩大 Free 版 Agent 能力）
   - 自研 GEMV 内核 → 评估简化或废弃

2. **MCP 协议 Phase 2**：
   - MCP Resource 支持（文件系统/数据库读取）
   - MCP Prompt 模板支持
   - MCP 服务器自动发现（zero-config）
   - 更多 transport（WebSocket）

3. **用户体验持续改善**：
   - 设置面板进一步优化（基于 S94 重构后的用户反馈）
   - 错误信息统一分类（S3 P1-2 待执行）
   - 顶栏精简 8→5 按钮（S3 P1-1 待执行）

4. **新功能候选**：
   - 自定义主题/皮肤（v3.0 ROI 4.3，已推迟）
   - 文档智能推荐（需阅读历史追踪）
   - 文档版本管理/Diff（v4.0 候选）

### 技术债清理（如时间允许）

1. `bench_real_import_phases` 离线环境失败 — 需 mock 或标记 `#[ignore]`
2. pre-existing Clippy 错误已在 v3.0 清零 ✅（21 个 pre-existing 错误已不存在）
3. E2E Playwright 全量回归（v3.0 未运行 E2E，仅运行 Vitest）

---

## 关键约束（铁律）

1. 代理仅限开发调研，软件本身禁止代理（reqwest 必须加 `.no_proxy()`）
2. TDD 驱动，先写红灯测试再实现；生产代码严禁 `unwrap()/expect()/panic!()`
3. 代码变更同步更新 `docs/api/` + `docs/SOFTWARE_ASSET.md`
4. 修改源代码后运行 `./sync-to-github.sh --commit`
5. Session 结束更新 `CURRENT_STATE.md` + `LOG.md` + 生成本文件
6. 先写规格再写代码（Spec-driven Development）
7. 前端样式铁律十三：Tailwind utility classes + design tokens，禁止硬编码 hex 颜色
8. async fn in trait（Edition 2024），无 async-trait 宏
9. 前端构建：`node scripts/build-ui.mjs`（esbuild 打包 IIFE，不要手改 `ui/index.html`）
10. Workspace lint deny：unwrap_used / expect_used / panic / unreachable / todo / unimplemented 全部 deny

---

## 验证命令

```bash
# Rust 测试
cargo test 2>&1 | grep "test result:" | awk '{s+=$4; f+=$6} END {print "Free passed:"s" failed:"f}'
cargo test --features pro 2>&1 | grep "test result:" | awk '{s+=$4; f+=$6} END {print "Pro passed:"s" failed:"f}'

# Lint
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --features pro -- -D warnings
cargo fmt --check

# 前端
npx tsc --noEmit
node scripts/build-ui.mjs
cd e2e-tests && npm run test:unit

# Release 构建（排除开发者工具）
node scripts/build-ui.mjs --release

# 同步
./sync-to-github.sh --commit
```

---

## 关键文件索引

| 文件 | 说明 |
|---|---|
| `docs/SRS_v1.0.md` | 需求规格唯一依据（v17.0.0，含 §4.28 v3.0 规划需求） |
| `docs/architecture/V3.0_ATOMIC_PLAN.md` | v3.0 原子计划（S94-S98 全部 ✅） |
| `docs/architecture/PRO_ROI_EVALUATION.md` | Pro 功能 ROI 评估报告（S98 产出） |
| `docs/sessions/CURRENT_STATE.md` | 当前项目状态快照 |
| `CHANGELOG_v3.0.md` | v3.0.0 发布说明 |
| `docs/api/ipc-commands.md` | IPC 命令签名 |
| `docs/api/code-contracts.md` | Trait 签名 + 数据模型 |
| `.catpaw/rules/system-constraints.md` | 铁律规则（21 条） |

---

## 注意事项

- v3.0.0 版本号已升级（8 crate Cargo.toml + tauri.conf.json）
- MCP 传输层 `crates/infra/src/mcp_transport.rs` 遵守铁律一 `.no_proxy()` 直连
- esbuild 打包：修改 `ui/src/*.js` 后必须运行 `node scripts/build-ui.mjs` 重新打包
- i18n 三语：所有新增文案需同时更新 `ui/locales/en.json`、`zh-CN.json`、`ja.json`
- MCP IPC 命令无 `#[cfg(debug_assertions)]` 门控，Release 构建中注册
- `mcp-settings.js` 已打包进 esbuild IIFE（1816 行），Release 构建中保留
- Pro ROI 评估报告产出：`docs/architecture/PRO_ROI_EVALUATION.md`（14 功能逐一分析）
- v3.0 P0 修复 5/6，剩余 P0-4 持续收入模型为商业层面需产品决策
