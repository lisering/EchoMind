# EchoMind v1.11 Session 提示词

> 复制本文件 --- 之间的全部内容作为新会话的第一条消息。

---

## 项目

EchoMind — Rust + Tauri v2 本地 RAG 知识库（律师/研究者高隐私场景）。六边形架构 8 crate，~100K 行 Rust + ~27K 行前端。版本 v1.10.0，1255 passed / 0 failed / 31 ignored。

上一 Session（v1.10）已完成：REQ-RAG-014/015 检索&LLM 参数前端 UI + REQ-ING-008 文档排序下拉 UI。5 E2E 测试通过，GitHub 已同步。

## 本 Session 任务：v1.11 规划 + 可实施项评估

v1.10 完成了 v1.9 后端对应的前端 UI 同步。v1.11 需识别新的可实施项。

### 任务 1：评估 v1.11 候选功能

从以下来源识别 v1.11 可实施项：

1. **SRS Draft REQ**：扫描 `docs/SRS_v1.0.md` 中状态非 Done/Implemented 的 REQ
2. **技术债**：`docs/sessions/CURRENT_STATE.md` §3 待办任务中是否有可实施项
3. **用户反馈**：如有新需求可纳入
4. **v2.0 候选评估**：REQ-VEC-009 索引重建 / REQ-KB-002 Command Palette / REQ-RAG-013 单条消息删除 / REQ-ING-011 导入历史 — 是否有可降级为 v1.11 的

评估维度：用户价值 40% + 实现易度 30% + 架构收益 20% + 风险控制 10%。ROI ≥ 5.0 入选。

### 任务 2：创建 `docs/architecture/V1.11_ATOMIC_PLAN.md`

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
| Rust Free | 1255 passed, 0 failed, 31 ignored |
| Clippy | 零警告 |
| cargo fmt | 通过 |
| TypeScript | 0 errors |
| esbuild | 16901 行 |
| E2E | 1046+5=1051 passed |
| 版本号 | 1.10.0 |

## v1.10 已完成功能摘要

| 功能 | 关键文件 | E2E 测试 |
|---|---|---|
| REQ-RAG-014 检索参数前端 | `ui/src/settings.js`（renderRagLlmParams）+ `ui/src/ipc.js`（ragParamsApi） | TC-V10-RAG-001 |
| REQ-RAG-015 LLM 参数前端 | 同上（renderRagLlmParams LLM 区） | TC-V10-LLM-001 |
| REQ-ING-008 文档排序前端 | `ui/src/main.js`（_kbSortBy/Order）+ `ui/index.html`（排序下拉） | TC-V10-SORT-001~002 |

## v2.0 候选参考

| REQ | 功能 | ROI | 备注 |
|---|---|---|---|
| REQ-VEC-009 | 索引重建 | 6.6 | 需进度追踪 UI |
| REQ-KB-002 | Command Palette | 5.8 | UI 组件开发量大 |
| REQ-RAG-013 | 单条消息删除 | 5.8 | 与对话分支树重叠 |
| REQ-ING-011 | 导入历史记录 | 5.7 | 调试用途 |
| REQ-VEC-011 | 分块参数配置 | 6.0 | 跨层改动 |

## 关键文件

| 文件 | 用途 |
|---|---|
| `docs/architecture/V1.10_ATOMIC_PLAN.md` | v1.10 规划（已完成） |
| `docs/CHANGELOG_v1.10.md` | v1.10 变更日志 |
| `docs/sessions/CURRENT_STATE.md` | 项目状态快照（必读） |
| `docs/SRS_v1.0.md` | 需求规格 |
| `.catpaw/rules/system-constraints.md` | 铁律 21 条 |
