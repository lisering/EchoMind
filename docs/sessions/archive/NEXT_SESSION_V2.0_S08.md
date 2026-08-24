# EchoMind v2.0 深耕 Session S08 提示词

---

你是 CatPaw，在 CatPaw IDE 中工作。用中文回答（代码除外）。

## 项目

EchoMind — Rust + Tauri v2 本地 RAG 知识库，六边形架构 8 crate。v1.22.0 已发布，v2.0 深耕阶段（不新增功能，专注代码质量/测试/CI/文档）。

## 上个 Session（S07）结果

CI 基础流水线统一完成。将开发仓库 2 个 workflow 文件（build.yml + e2e.yml）合并为单一 `.github/workflows/ci.yml`，消除 fmt/clippy/test 职责重叠。6 个 Job：(1) `lint` — fmt + clippy（Free+Pro），ubuntu-22.04，快速失败；(2) `test` — cargo test（Free+Pro）+ chaos tests，依赖 lint；(3) `supply-chain` — cargo-audit + cargo-deny，独立并行；(4) `build-matrix` — 跨平台 release build（4 OS，Free+Pro），依赖 test；(5) `e2e-bridge` — Playwright bridge 分片（4 shard，ubuntu+macos），依赖 test，E2E_SPEED=0.2；(6) `e2e-gui` — 真实 GUI（WebdriverIO+tauri-driver，ubuntu+windows），依赖 build-matrix。新增 concurrency cancel-in-progress + Swatinem/rust-cache@v2 + actions/cache@v4（fastembed 模型缓存）。删除 build.yml + e2e.yml。YAML 语法验证通过。sync-to-github 无变更（.github/ 不同步）。深耕计划 S07 验收标准全部勾选。

## 测试基线

- `cargo test -p echomind-tauri-app --test integration`: 144 passed, 0 failed, 1 ignored
- `cargo build -p echomind-tauri-app`: 通过
- Clippy: 零警告 / cargo fmt: 通过
- YAML 语法验证: 通过

## 本 Session 任务：S08 — CI 增强（跨平台 release build 矩阵 + CI 运行时间优化 ≤ 20min）

**目标**：增强 ci.yml，添加跨平台安装包构建（`cargo tauri build`）+ 优化 CI 运行时间到 ≤ 20 分钟。

### 现状

当前 `ci.yml` 的 `build-matrix` job 只做 `cargo build --release`（编译验证），不产出安装包。原 `e2e.yml` 中的 `build` job 使用 `cargo tauri build --target` 产出安装包（.dmg/.msi/.AppImage/.deb），但该 job 在合并时未被保留（因 S07 专注基础流水线统一）。

### 执行步骤

1. 在 `ci.yml` 中新增 `release-build` job（或在 `build-matrix` 中增加 Tauri 安装包构建步骤）
2. 使用 `cargo tauri build --target ${{ matrix.target }}` 产出安装包
3. 上传安装包为 artifact（.dmg/.msi/.AppImage/.deb）
4. 安装 Tauri CLI（`cargo install tauri-cli --locked`）
5. 设置 Node.js 22（Tauri 构建需要前端构建）
6. 优化 CI 运行时间：
   - `lint` job 与 `supply-chain` 并行（已实现）
   - `test` 依赖 `lint`（已实现，避免已知失败的测试浪费时间）
   - `build-matrix` 依赖 `test`（已实现）
   - 考虑将 `build-matrix` 的 Free+Pro 合并为单次构建（`cargo build --release --features pro` 覆盖 Free）
   - 考虑 cache key 细化（per-job 而非 per-label）
7. 确保 `concurrency` group 正确取消旧 run

### 关键约束

- 保持所有现有测试覆盖不变
- Free + Pro 双矩阵必须覆盖
- E2E bridge 分片（4 shard）保持
- E2E GUI 仅 ubuntu + windows（macOS 不支持 tauri-driver）
- `sync-to-github.sh` 不同步 `.github/` 目录
- CI 总运行时间目标 ≤ 20 分钟（当前无基线，首次 CI push 后测量）

### 验证

```bash
# 本地验证 YAML 语法
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))"

# 确认只有 ci.yml
ls .github/workflows/  # 应只有 ci.yml

# 确认 sync-to-github 不涉及 .github/
./sync-to-github.sh
```

### 铁律

- 严禁 `unwrap()/expect()/panic!()`（本任务仅涉及 YAML，无 Rust 代码）
- 改源码后必须运行 `./sync-to-github.sh`

### 必读文件

- `docs/sessions/CURRENT_STATE.md` — 项目状态快照
- `docs/architecture/V2.0_DEEP_CULTIVATION_ATOMIC_PLAN.md` — 深耕计划 §4 Phase 2 S08
- `.github/workflows/ci.yml` — 当前 CI 配置（S07 产出）
- `AGENTS.md` — 架构概览

### 后续预览

| Session | 任务 |
|---|---|
| S09-S10 | IPC 命令精简（243 → ≤ 180） |
| S11-S13 | 集成测试增强（2.5% → ≥ 15%） |
| S14 | Dead Code 清理 |
| S21 | v2.0 发布 |

---
