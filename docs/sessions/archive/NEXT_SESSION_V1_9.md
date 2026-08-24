# EchoMind v1.9 Session 提示词

> 复制本文件 --- 之间的全部内容作为新会话的第一条消息。

---

## 项目

EchoMind — Rust + Tauri v2 本地 RAG 知识库（律师/研究者高隐私场景）。六边形架构 8 crate，~100K 行 Rust + ~27K 行前端。版本 v1.8.0，1245 passed / 0 failed / 31 ignored。

上一 Session（v1.8）已完成：F5-F8 远期项目评估（全部推迟到 v2.0）+ B09 Skill 系统集成（`discover_skills` IPC + 前端斜杠命令面板 `loadSkills()` 合并）。3 TDD 测试通过，clippy 零警告，GitHub 已同步。

## 本 Session 任务：v1.9 规划 + 可实施项评估

v1.8 评估结论：F5-F8 全部推迟到 v2.0（基础设施已就绪但需架构级变更）。v1.9 需识别新的可实施项。

### 任务 1：评估 v1.9 候选功能

从以下来源识别 v1.9 可实施项：

1. **DS4 剩余**（`docs/sessions/SESSION_DS4_HANDOFF.md`）：DS-04 功率管理 / DS-05 Tool Call 续接 / DS-06 Radix Tree / DS-07 SSD 流式
2. **SRS 未实现 REQ**：扫描 `docs/SRS_v1.0.md` 中状态非 Done/Implemented 的 REQ
3. **技术债**：`docs/sessions/CURRENT_STATE.md` §3 待办任务中是否有可实施项
4. **用户反馈**：如有新需求可纳入

评估维度：用户价值 40% + 实现复杂度 30% + 架构收益 20% + 风险控制 10%。ROI ≥ 5.0 入选。

### 任务 2：创建 `docs/architecture/V1.9_ATOMIC_PLAN.md`

含 ROI 矩阵 + 选定功能 + 实施步骤 + TDD 测试计划。

### 任务 3：TDD 实现

按新规划执行，TDD 先写红灯测试再实现。

### 任务 4：文档同步 + 发布验证

同步 `docs/` + CHANGELOG + `sync-to-github.sh --commit`。

## 开发规范

- TDD（铁律二）：先写红灯测试再实现
- 零 unwrap/expect/panic（workspace deny lint）
- forbid unsafe
- 文档同步（铁律七）：代码变更同步 docs
- GitHub 同步（铁律十一）：会话结束前 `./sync-to-github.sh`

## 验证命令

```bash
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo test
npx tsc --noEmit
./sync-to-github.sh --commit
```

## 当前测试基线

| 指标 | 数值 |
|---|---|
| Rust Free | 1245 passed, 0 failed, 31 ignored |
| Clippy | 零警告 |
| cargo fmt | 通过 |
| TypeScript | 0 errors |
| 版本号 | 1.8.0 |

## v1.8 已完成功能摘要

| 功能 | 关键文件 | TDD 测试 |
|---|---|---|
| B09 Skill 系统集成 | `crates/tauri-app/src/commands/features.rs`（`discover_skills`）+ `ui/src/slash-commands.js`（`loadSkills`） | TC-SKILL-INTEGRATE-001~003 |
| F5-F8 评估文档 | `docs/architecture/V1.8_ATOMIC_PLAN.md` | — |

## F5-F8 评估结论（v2.0 候选）

| 项 | 实现状态 | 阻碍原因 |
|---|---|---|
| F5 Scored Selector | ✅ 12 测试 | 需多 provider 架构 |
| F6 SystemContext+Epoch | ✅ 16 测试 | 需 chat flow 重构 |
| F7 PromptCache | ✅ 27 测试 | 需 mistral.rs API 变更 |
| F8 CachePolicy | ✅ 6 测试 | 需 Anthropic API 支持 |

## 关键文件

| 文件 | 用途 |
|---|---|
| `docs/architecture/V1.8_ATOMIC_PLAN.md` | v1.8 规划（含 F5-F8 评估详情） |
| `docs/CHANGELOG_v1.8.md` | v1.8 变更日志 |
| `docs/sessions/CURRENT_STATE.md` | 项目状态快照（必读） |
| `docs/SRS_v1.0.md` | 需求规格 |
| `.catpaw/rules/system-constraints.md` | 铁律 21 条 |
