# NEXT_SESSION — EchoMind v3.1.0 CI 全绿，v3.1 规划

> **生成日期**：2026-08-27 ｜ **生成人**：AI Agent (CI 容错加固会话)
>
> **当前版本**：v3.1.0 ｜ **测试基线**：Free 1700 / Pro 1991 passed
>
> **关联文档**：`docs/sessions/CURRENT_STATE.md` ｜ `docs/SRS_v1.0.md` ｜ `docs/architecture/V3.0_ATOMIC_PLAN.md`

---

## 项目概要

EchoMind — Rust + Tauri v2 本地 RAG 知识库。六边形架构 8 crate：models → prompt → core → infra → tauri-app（+ compact / context / license-issuer）。**v3.1.0**（tauri.conf.json 版本号已升级）。fastembed ONNX 嵌入（5 种 EmbeddingModel），SQLite 向量存储 + HNSW 索引，任意 OpenAI 兼容 LLM。Pro 功能：本地 LLM 推理（mistral.rs）、GPU 加速、ColBERT、tree-sitter、WasmExecutor。前端 56 个 ES module（vanilla JS，esbuild 打包 IIFE 1816 行）。MCP 协议适配器 Phase 1 已完成。

## CI 全绿确认 ✅

### CI run 33024377225（上一会话推送）— 全绿

| Job | 状态 |
|---|---|
| Lint (fmt + clippy) | ✅ success |
| Frontend Checks (tsc + esbuild + vitest) | ✅ success |
| Supply Chain Security | ✅ success |
| Test (Free + Pro + chaos) | ✅ success |
| Build (macOS-arm64) | ✅ success |
| Build (Linux-x64) | ✅ success |
| E2E Bridge 8/8 shards | ✅ 全部 success |
| Release Build (macOS-arm64) | ✅ success |
| Release Build (Linux-x64) | ✅ success |
| E2E Bridge Report | ✅ success |
| E2E GUI (ubuntu + windows) | ❌ 但 continue-on-error: true |

### 本次加固修复

**问题**：CI run 32994016879 中 E2E Bridge Report "Merge reports" 步骤失败，`npx playwright merge-reports` 因 `download-artifact@v4` blob 数据截断而崩溃（Node.js 未处理 'error' 事件），原有 `|| { exit 0 }` 未能捕获。

**修复**：
- `.github/workflows/ci.yml` — "Merge reports" 步骤：`|| { exit 0 }` → `|| true`（更简单可靠的 shell 错误捕获）
- `.github/workflows/ci.yml` — "Upload HTML report" 步骤添加 `if: always()` + `if-no-files-found: ignore`

---

## 下个 Session 任务

### 1. 验证最新 CI run 33027743323 全绿（优先）

```bash
cd EchoMind && gh run view 33027743323 --json status,conclusion
# 确认 conclusion 为 success
```

### 2. v3.1 版本规划（建议方向）

v3.0 已发布，CI 全绿，SRS Draft REQ 再次归零。建议进行 v3.1 版本规划：

#### 方向 A：Pro 功能下沉评估

基于 S98 Pro ROI 评估（`docs/architecture/PRO_ROI_EVALUATION.md`），考虑将 4 个性能优化型功能逐步下沉 Free：
- ColBERT 多向量嵌入 → Free（提升 Free 版检索质量）
- ONNX 嵌入压缩器 → Free（提升 Free 版 Prompt 压缩）
- WasmExecutor 代码沙箱 → Free（扩大 Free 版 Agent 能力）
- 自研 GEMV 内核 → 评估简化或废弃

#### 方向 B：MCP 协议 Phase 2

- MCP Resource 支持（文件系统/数据库读取）
- MCP Prompt 模板支持
- MCP 服务器自动发现（zero-config）
- 更多 transport（WebSocket）

#### 方向 C：用户体验持续改善

- 设置面板进一步优化（基于 S94 重构后的用户反馈）
- 错误信息统一分类（S3 P1-2 待执行）
- 顶栏精简 8→5 按钮（S3 P1-1 待执行）

#### 方向 D：新功能候选

- 自定义主题/皮肤（v3.0 ROI 4.3，已推迟）
- 文档智能推荐（需阅读历史追踪）
- 文档版本管理/Diff（v4.0 候选）

### 3. 技术债清理（如时间允许）

1. `bench_real_import_phases` 离线环境失败 — 需 mock 或标记 `#[ignore]`
2. E2E GUI (wdio) 现代化 — 当前 `continue-on-error: true`，应迁移到 Playwright 或 tauri-driver 新方案
3. E2E Bridge Report blob 下载不完整 — `download-artifact@v4` 已知问题，`|| true` 为容错方案

### 4. CI 基础设施改善（可选）

- Windows-x64 build matrix 恢复：当前注释掉（bundled-sqlcipher 需 Windows OpenSSL 工具链）
- macOS-x64 build matrix 恢复：当前只有 arm64
- E2E GUI 现代化或废弃

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
11. `sync-to-github.sh` 现已同步 `Info.plist`，后续新增 tauri-app 根目录文件需手动添加同步行
12. `tauri.conf.json` 不再设置 `signingIdentity`，签名仅通过 CI 环境变量 `APPLE_SIGNING_IDENTITY` 控制

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

# E2E Bridge（本地）
cd e2e-tests && npx playwright test tests/v2.4-template-import-export.spec.ts tests/v2.4-batch-ops.spec.ts tests/v1.19-features.spec.ts --reporter=line

# Release 构建（排除开发者工具）
node scripts/build-ui.mjs --release

# 同步 + 推送
./sync-to-github.sh --push

# CI 状态检查
cd EchoMind && gh run list --limit 5 --json databaseId,event,status,conclusion,createdAt
cd EchoMind && gh run view <run_id> --json jobs
```

---

## 关键文件索引

| 文件 | 说明 |
|---|---|
| `docs/SRS_v1.0.md` | 需求规格唯一依据（v17.0.0，含 §4.28 v3.0 规划需求） |
| `docs/architecture/V3.0_ATOMIC_PLAN.md` | v3.0 原子计划（S94-S98 全部 ✅） |
| `docs/architecture/PRO_ROI_EVALUATION.md` | Pro 功能 ROI 评估报告（S98 产出） |
| `docs/sessions/CURRENT_STATE.md` | 当前项目状态快照（2026-08-27 更新） |
| `docs/sessions/LOG.md` | 会话时间线日志 |
| `CHANGELOG_v3.0.md` | v3.0.0 发布说明 |
| `docs/api/ipc-commands.md` | IPC 命令签名 |
| `docs/api/code-contracts.md` | Trait 签名 + 数据模型 |
| `.github/workflows/ci.yml` | CI 工作流 |
| `sync-to-github.sh` | 发布仓库同步脚本 |
| `crates/tauri-app/tauri.conf.json` | Tauri 配置 |
| `.catpaw/rules/system-constraints.md` | 铁律规则（21 条） |

---

## 注意事项

- **版本号**：v3.1.0（tauri.conf.json + 8 crate Cargo.toml），v3.0.0 git tag 已创建
- **CI 状态**：CI run 33024377225 全绿 ✅（所有核心 job success），CI run 33027743323 验证中
- **E2E Bridge Report 容错**：`|| true` 确保即使 `download-artifact@v4` blob 下载不完整也不会失败
- **E2E GUI**：`continue-on-error: true`，历史遗留 wdio 冒烟测试，非阻塞
- **MCP 传输层** `crates/infra/src/mcp_transport.rs` 遵守铁律一 `.no_proxy()` 直连
- **esbuild 打包**：修改 `ui/src/*.js` 后必须运行 `node scripts/build-ui.mjs` 重新打包
- **i18n 三语**：所有新增文案需同时更新 `ui/locales/en.json`、`zh-CN.json`、`ja.json`
- **P0 状态**：5/6 已修复，剩余 P0-4 持续收入模型为商业层面需产品决策
