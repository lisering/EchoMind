# EchoMind v1.21 Session 提示词

## 项目概要

EchoMind — Rust + Tauri v2 本地 RAG 知识库。六边形架构 8 crate，版本 v1.20.0。SRS 250 REQ，完成率 98.8%（100 Done + 147 Implemented + 2 Draft + 1 Deprecated）。剩余 2 Draft：REQ-DS-004（图标系统 P2）+ REQ-NFR-019（构建体积 P2）。

## 本 Session 目标

实施 **REQ-NFR-019 构建产物体积优化**（SRS 第 4095 行）。

### AC 验收标准

1. macOS arm64 .dmg ≤ 50MB（不含 ONNX 模型）
2. Windows x64 .msi ≤ 60MB（不含 ONNX 模型）
3. ONNX 模型 ~30MB 不打包，首次使用从 HuggingFace 下载
4. 前端 vendored 库总体积 ≤ 5MB（`ui/vendor/`）
5. Release profile 启用 LTO（`lto = true`）+ strip（`strip = true`）

### 当前状态（预检结果）

- **`Cargo.toml`：无 `[profile.release]` 段**，需新增 `lto = true` + `strip = true`
- **`ui/vendor/` = 5.1MB**，略超 AC-4 的 5MB 上限，需审计裁剪
- `tauri.conf.json` bundle 配置正常（无 resources 字段打包模型文件）
- ONNX 模型已为懒加载（`tokio::sync::OnceCell`，首次 chat 触发下载），AC-3 已满足

### 实现步骤

1. `Cargo.toml` 新增 `[profile.release]` 段：`lto = "fat"` + `strip = true` + `codegen-units = 1`
2. 审计 `ui/vendor/` 目录：`du -sh ui/vendor/*`，裁剪未使用的库文件或子目录（如 KaTeX fonts 子集化）
3. 验证：`cargo build --release` 成功 + 检查二进制体积
4. 编写体积检查脚本 `scripts/check-bundle-size.sh`（CI 可选集成）
5. 更新 SRS REQ-NFR-019 状态 Draft → Implemented + changelog
6. 更新 `docs/sessions/CURRENT_STATE.md`
7. `./sync-to-github.sh --commit`

### TDD 测试

在 `crates/tauri-app/tests/` 新增 `bundle_size.rs` 集成测试：
- TC-NFR-019-001：`Cargo.toml` 含 `[profile.release]` 且 `lto` 非 `false`
- TC-NFR-019-002：`Cargo.toml` 含 `strip = true`
- TC-NFR-019-003：`ui/vendor/` 总体积 ≤ 5MB（运行时 `fs::read_dir` 递归求和）

## 开发规范（铁律摘要）

- TDD 先红后绿，测试与生产代码同等质量
- 零 unwrap/expect/panic（workspace deny lint）
- 改前端后 `node scripts/build-ui.mjs` 重建
- 文档同步：SRS + CURRENT_STATE + (如有新 IPC) ipc-commands.md

## 验证命令

```bash
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo test -p echomind-tauri-app
npx tsc --noEmit
node scripts/build-ui.mjs
./sync-to-github.sh --commit
```

## 测试基线

Rust Free 1279 passed / 31 ignored | Clippy 零警告 | tsc 0 errors | esbuild 19165 行 | IPC 220 Free / 230 Pro

## 关键文件

| 文件 | 用途 |
|---|---|
| `Cargo.toml` | workspace 配置（新增 `[profile.release]`） |
| `crates/tauri-app/tauri.conf.json` | Tauri bundle 配置（确认无模型打包） |
| `ui/vendor/` | 前端本地库目录（体积审计裁剪） |
| `docs/SRS_v1.0.md` | SRS（REQ-NFR-019 第 4095 行，改状态） |
| `docs/sessions/CURRENT_STATE.md` | 项目状态快照（更新） |

## 上一 Session 摘要

v1.20 实施 REQ-WS-002（配额独立计算）+ REQ-WS-004（文档跨库迁移）。ImportService 新增 workspace_id + per-workspace 配额计数 + migrate_document IPC + 右键迁移菜单。6 E2E 全通过。
