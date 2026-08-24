# EchoMind 下个 Session 提示词

你是 CatPaw，在 CatPaw IDE 中工作。用中文回答（代码除外）。工作目录：`/Users/john/freesoft/EchoMind`。

## 项目概要

EchoMind — Rust + Tauri v2 本地 RAG 知识库。六边形架构 8 crate：models → prompt → core → infra → tauri-app（+ compact / context / license-issuer）。v2.2.0 已发布。当前 **1566 Free / 1831 Pro** 测试通过。fastembed ONNX 嵌入（5 种 EmbeddingModel），SQLite 向量存储 + HNSW 索引（Free），任意 OpenAI 兼容 LLM。Pro 功能：本地 LLM 推理（mistral.rs）、GPU 加速、ColBERT、tree-sitter、WasmExecutor。

## 上 Session 完成内容（S89 macOS 代码签名与公证）

V2.4 原子计划 S89 macOS 代码签名与公证完成（REQ-SEC-023 Implemented）。

### 实现内容

| 层 | 变更 | 关键文件 |
|---|---|---|
| 规格定义 | REQ-SEC-023 新增（6 AC + 6 TC + 契约），V2.4 原子计划修正 REQ-SEC-021 → REQ-SEC-023（因 REQ-SEC-021 已被 Prompt 注入防护占用） | `docs/SRS_v1.0.md` |
| Tauri 配置 | `macOS.signingIdentity: "-"`（ad-hoc 签名默认值，`APPLE_SIGNING_IDENTITY` 环境变量覆盖为 Developer ID）+ `entitlements: "entitlements.plist"` | `crates/tauri-app/tauri.conf.json` |
| 权限声明 | 8 个 entitlements：network.client/server + files.user-selected.read-write + device.camera + cs.allow-jit + cs.allow-unsigned-executable-memory + cs.allow-dyld-environment-variables + device.audio | `crates/tauri-app/entitlements.plist` |
| 本地签名脚本 | 5 步流程（证书导入临时 keychain → tauri build 自动签名 → notarytool 公证 → stapler staple → 最终验证 codesign + spctl + stapler）+ 3 种模式（完整签名+公证 / --skip-notarize 仅签名 / --verify-only 仅验证）+ 无证书自动跳过 | `scripts/sign-macos.sh` |
| 发布仓库 CI | macOS 签名步骤（base64 P12 证书注入 + APPLE_ID/PASSWORD/TEAM_ID 环境变量 + 签名验证） | `EchoMind/.github/workflows/release.yml` |
| 开发仓库 CI | release-build job 新增 macOS 签名步骤 | `.github/workflows/ci.yml` |

### 关键设计

- **signingIdentity: "-"** 作为默认值（ad-hoc 签名），CI 中通过 `APPLE_SIGNING_IDENTITY` 环境变量覆盖为 Developer ID Application 证书
- **entitlements.plist** 不启用 App Sandbox（EchoMind 需要全盘文件访问和网络访问），而是声明必要的 entitlements
- **无证书自动跳过**：`sign-macos.sh` 检测到无证书时自动跳过签名步骤，仅构建未签名版本
- **CI 条件签名**：GitHub Actions `if: runner.os == 'macOS' && secrets.SIGNING_CERTIFICATE_P12 != ''` 条件控制签名步骤

### 验证结果

| 验证项 | 结果 |
|---|---|
| cargo build | ✅ exit_code 0 |
| Clippy | 零警告 |
| cargo fmt | 通过 |
| tsc | 0 errors |
| esbuild | 1641 行 |

## 测试基线

- Rust Free: 1566 passed, 0 failed, ~32 ignored
- Clippy Free: 零警告
- cargo fmt: 通过
- tsc: 0 errors
- esbuild: 1641 行
- Vitest: 1767 passed (88 files)
- 版本号: 2.3.0（v2.4 原子计划进行中，S86 ✅ S87 ✅ S88 ✅ S89 ✅ S90 ✅ S91 ✅ 已完成）

## V2.4 原子计划进度

| Session | 功能 | REQ | ROI | 预计天数 | 状态 |
|---|---|---|---|---|---|
| S86 | 对话书签/收藏 | REQ-RAG-053 | 11.0 | 0.5 | ✅ 已完成 |
| S87 | 自动更新检查 | REQ-HELP-004 | 10.6 | 0.5 | ✅ 已完成 |
| S88 | 对话模板导入/导出 | REQ-RAG-054 | 9.2 | 0.5 | ✅ 已完成 |
| S89 | macOS 代码签名与公证 | REQ-SEC-023 | 6.0 | 1 | ✅ 已完成 |
| S90 | 全局搜索增强 | REQ-IX-008 | 5.4 | 1 | ✅ 已完成 |
| S91 | 批量操作增强 | REQ-ING-024 | 5.2 | 1 | ✅ 已完成 |
| S92 | v2.4 发布准备 | — | — | 0.5 | ⏳ 待实施 |

详见 `docs/architecture/V2.4_ATOMIC_PLAN.md`。

## 下个 Session 任务

### 优先：S92 V2.4 发布准备（0.5 天）

S86-S91 全部完成后，执行 V2.4 发布：

1. 版本号升级 2.3.0 → 2.4.0（8 crate Cargo.toml + tauri.conf.json）
2. CHANGELOG_v2.4.md 编写（6 功能：书签 + 更新检查 + 模板导入/导出 + macOS 签名 + 全局搜索 + 批量操作）
3. 全量回归测试（Rust Free + Pro + Clippy + fmt + tsc + esbuild + Vitest + E2E）
4. Release build
5. GitHub tag

### macOS 签名使用说明

如需签名+公证，用户需提供以下环境变量/Secrets：
- `SIGNING_CERTIFICATE_P12`：base64 编码的 .p12 证书文件
- `SIGNING_PASSWORD`：.p12 证书密码
- `APPLE_SIGNING_IDENTITY`：签名身份名称（如 "Developer ID Application: Your Name (XXXXXXXXXX)"）
- `APPLE_ID`：Apple ID 邮箱
- `APPLE_PASSWORD`：App-Specific Password
- `APPLE_TEAM_ID`：Apple Team ID

本地签名：`./scripts/sign-macos.sh`
跳过公证：`./scripts/sign-macos.sh --skip-notarize`
仅验证：`./scripts/sign-macos.sh --verify-only`

## 关键约束（铁律）

1. 代理仅限开发调研，软件本身禁止代理（`reqwest` 必须加 `.no_proxy()`）
2. TDD 驱动，先写红灯测试再实现；生产代码严禁 `unwrap()/expect()/panic!()`
3. 代码变更同步更新 `docs/api/` + `docs/SOFTWARE_ASSET.md`
4. 修改源代码后运行 `./sync-to-github.sh --commit`
5. Session 结束更新 `CURRENT_STATE.md` + `LOG.md` + 生成本文件
6. 先写规格再写代码（Spec-driven Development）
7. 前端样式铁律十三：Tailwind utility classes + design tokens，禁止硬编码 hex 颜色
8. `async fn in trait`（Edition 2024），无 `async-trait` 宏

## 验证命令

```bash
# Rust 测试
cargo test 2>&1 | grep -oE "[0-9]+ passed" | awk '{s+=$1} END {print "Free passed:"s}'
cargo test --features pro 2>&1 | grep -E "test result:" | awk '{print $4}' | awk -F, '{s+=$1} END {print "Pro passed:"s}'

# Lint
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --features pro -- -D warnings
cargo fmt --check

# 前端
npx tsc --noEmit
node scripts/build-ui.mjs
cd e2e-tests && npm run test:unit

# macOS 签名
./scripts/sign-macos.sh                    # 签名 + 公证
./scripts/sign-macos.sh --skip-notarize   # 仅签名
./scripts/sign-macos.sh --verify-only     # 仅验证

# 同步
./sync-to-github.sh --commit
```

## 关键文件索引

| 文件 | 说明 |
|---|---|
| `docs/SRS_v1.0.md` | 需求规格（REQ-SEC-023 已 Implemented，V2.4 全部 Draft REQ 已实现） |
| `docs/architecture/V2.4_ATOMIC_PLAN.md` | V2.4 原子计划（S86-S91 ✅ / S92 ⏳） |
| `crates/tauri-app/tauri.conf.json` | Tauri 配置（含 macOS 签名配置） |
| `crates/tauri-app/entitlements.plist` | macOS 权限声明（8 个 entitlements） |
| `scripts/sign-macos.sh` | macOS 本地签名+公证脚本 |
| `EchoMind/.github/workflows/release.yml` | 发布仓库 CI（含 macOS 签名步骤） |
| `.github/workflows/ci.yml` | 开发仓库 CI（含 macOS 签名步骤） |
