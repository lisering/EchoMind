# EchoMind v2.0 深耕 S12 提示词

你是 CatPaw，在 CatPaw IDE 中工作。用中文回答（代码除外）。

## 项目

EchoMind — Rust + Tauri v2 本地 RAG 知识库，六边形架构 8 crate。v1.22.0 已发布，v2.0 深耕阶段（不新增功能，专注代码质量/测试/CI/文档）。深耕计划见 `docs/architecture/V2.0_DEEP_CULTIVATION_ATOMIC_PLAN.md`。

## 上个 Session（S11）

集成测试增强完成。新增 3 个测试模块 18 个测试（full_pipeline 8 + security_chain 10 + local_llm_chain 6），原计划 S11-S13 三步合并为一步完成。关键修复：`import_one` 不执行分块需手动调 `index_document`；`chrono` 用 `std::time::SystemTime` 替代；枚举变体名修正（PiiType::IpAddress/InternationalPhone, PasswordStrength::Medium/Strong/VeryStrong）。

## 测试基线

- Rust 集成测试: 162 passed（含 18 新增）, 0 failed, 1 ignored
- Clippy 零警告 / cargo fmt 通过
- sync-to-github: 4 files committed, 1394 insertions

## 本 Session 任务：S12 — Dead Code 清理（Phase 5）

**目标**：删除 4 个 F 级模块文件 + 清理未使用导入，Dead Code 归零。

### 步骤

1. 确认 `ui/src/workflow-panel.js` / `code-executor.js` / `durable-panel.js` / `session-strip.js` 在 `main.js` / `settings.js` / `index.html` 中无导入引用（S6 已删除引用）
2. 确认 E2E 测试中无直接引用
3. 删除 4 个文件
4. 运行 `node scripts/build-ui.mjs` 确认打包成功
5. 运行 `npx tsc --noEmit` 确认无类型错误
6. 运行 Vitest 确认无回归
7. 运行 E2E bridge 测试确认无回归
8. 全局搜索清理任何残留引用（`grep -r "workflow-panel\|code-executor\|durable-panel\|session-strip"` 排除已删文件）
9. 运行 `./sync-to-github.sh --commit`

### 约束

- 严禁 `unwrap()/expect()/panic!()`
- 改源码后必须运行 `./sync-to-github.sh`
- E2E + Vitest + Rust 测试全量通过（零回归）
- 前端构建: `node scripts/build-ui.mjs`（esbuild 打包 IIFE）

### 必读文件

- `docs/sessions/CURRENT_STATE.md`
- `docs/architecture/V2.0_DEEP_CULTIVATION_ATOMIC_PLAN.md` §7 Phase 5

### 后续

S13 前端测试增强（Phase 6，Vitest 440→≥800）→ S14-S15 继续前端测试 → S16 Draft REQ 分类 → S17 文档完善 → S18 v2.0 发布
