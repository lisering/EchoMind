# EchoMind v2.0 深耕 Session S07 提示词

---

你是 CatPaw，在 CatPaw IDE 中工作。用中文回答（代码除外）。

## 项目

EchoMind — Rust + Tauri v2 本地 RAG 知识库，六边形架构 8 crate。v1.22.0 已发布，v2.0 深耕阶段（不新增功能，专注代码质量/测试/CI/文档）。

## 上个 Session（S06）结果

`features.rs` 拆分完成。原 1,559 行单文件 → 薄入口（30 行）+ `features/` 子目录 12 个功能域子模块（code_exec / custom_model / dream / memory / skill / stt / symbol / template / trace / web_search / wiki / workflow）。`features.rs` 仅含 `mod` 声明 + `pub use` 重导出，保持 `lib.rs` `generate_handler!` 不变。修复了 Rust 2024 edition 下 `features.rs` 与 `features/mod.rs` 冲突（删除 `mod.rs`）、`format!` 宏语法错误、`cargo fmt` 排序。验证：build 通过 / Clippy 零警告 / fmt 通过 / integration 144 passed。sync-to-github 已提交。

## 测试基线

- `cargo test -p echomind-tauri-app --test integration`: 144 passed, 0 failed, 1 ignored
- `cargo build -p echomind-tauri-app`: 通过
- Clippy: 零警告 / cargo fmt: 通过

## 本 Session 任务：S07 — CI 基础流水线统一

**目标**：统一开发仓库 `.github/workflows/` 下的 CI 配置，消除 `build.yml` 与 `e2e.yml` 之间的职责重叠，合并为单一 `ci.yml`。

### 现状

开发仓库有 2 个 workflow 文件：
- `build.yml` — 跨平台构建矩阵（4 OS）+ fmt/clippy/test（Free + Pro）+ 嵌入模型缓存
- `e2e.yml` — Rust 检查 + 供应链安全 + E2E GUI + E2E Bridge（分片）+ 跨平台安装包产出

**问题**：`build.yml` 和 `e2e.yml` 都做 fmt/clippy/test，职责重叠。`e2e.yml` 的 `rust-check` job 在 ubuntu-latest 上跑，`build.yml` 在 4 OS 矩阵上跑同样的检查。

发布仓库 `EchoMind/EchoMind/.github/workflows/` 已有 3 个文件：
- `ci.yml` — 跨平台构建（4 OS）+ fmt/clippy/test（Free+Pro）+ 供应链安全（cargo-audit + cargo-deny）
- `ai-review.yml` — AI 代码审查
- `release.yml` — 发布

### 执行步骤

1. 分析 `build.yml` + `e2e.yml` 的所有 job，确定哪些可合并、哪些保持独立
2. 创建统一 `.github/workflows/ci.yml`，包含以下 job：
   - `lint` — fmt + clippy（Free + Pro），ubuntu-latest，快速失败
   - `test` — cargo test（Free + Pro），ubuntu-latest
   - `supply-chain` — cargo-audit + cargo-deny，ubuntu-latest
   - `build-matrix` — 跨平台 release build（4 OS），仅 push to main + PR
   - `e2e-bridge` — Playwright bridge 测试（分片），ubuntu + macos
   - `e2e-gui` — 真实 GUI 测试（WebdriverIO），ubuntu + windows
3. 删除 `build.yml` 和 `e2e.yml`（被 `ci.yml` 取代）
4. 确保 `ci.yml` 中的 job 依赖关系合理（`lint` → `test` → `build-matrix`；`supply-chain` 独立并行；`e2e-bridge` 依赖 `build` 成功）
5. 使用 `Swatinem/rust-cache@v2` 缓存 Rust 编译产物
6. 使用 `actions/cache@v4` 缓存 fastembed 嵌入模型
7. 设置 `E2E_SPEED: '0.2'` 加速 mock 测试

### 关键约束

- 保持所有现有测试覆盖不变（不减少任何 job 的测试范围）
- Free + Pro 双矩阵必须覆盖
- E2E bridge 分片（4 shard）保持
- E2E GUI 仅 ubuntu + windows（macOS 不支持 tauri-driver）
- `sync-to-github.sh` 不同步 `.github/` 目录（发布仓库已有独立 CI），所以仅在开发仓库操作

### 验证

```bash
# 本地验证 YAML 语法
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))"

# 确认旧文件已删除
ls .github/workflows/  # 应只有 ci.yml

# 确认 sync-to-github 不涉及 .github/
./sync-to-github.sh --commit
```

### 铁律

- 严禁 `unwrap()/expect()/panic!()`（本任务仅涉及 YAML，无 Rust 代码）
- 改源码后必须运行 `./sync-to-github.sh`

### 必读文件

- `docs/sessions/CURRENT_STATE.md` — 项目状态快照
- `docs/architecture/V2.0_DEEP_CULTIVATION_ATOMIC_PLAN.md` — 深耕计划 §4 Phase 2
- `AGENTS.md` — 架构概览

### 后续预览

| Session | 任务 |
|---|---|
| S08 | CI 增强（跨平台 release build 矩阵 + CI 运行时间优化 ≤ 20min） |
| S09-S10 | IPC 命令精简（243 → ≤ 180） |
| S14 | Dead Code 清理 |
| S21 | v2.0 发布 |

---
