# EchoMind v1.12 Session 提示词

> 复制本文件 --- 之间的全部内容作为新会话的第一条消息。

---

## 项目

EchoMind — Rust + Tauri v2 本地 RAG 知识库（律师/研究者高隐私场景）。六边形架构 8 crate，~100K 行 Rust + ~27K 行前端。版本 v1.11.0，1255 passed / 0 failed / 31 ignored。

上一 Session（v1.11）已完成：REQ-ING-010 文档内容预览 + REQ-IX-007 骨架屏 + REQ-RAG-013 单条消息删除。9 E2E 测试通过，GitHub 已同步。

## 本 Session 任务：v1.12 规划 + 可实施项评估

v1.11 完成了文档管理增强 + UX 改善。v1.12 需识别新的可实施项。

### 任务 1：评估 v1.12 候选功能

从以下来源识别 v1.12 可实施项：

1. **SRS Draft REQ**：扫描 `docs/SRS_v1.0.md` 中状态非 Done/Implemented 的 REQ
2. **技术债**：`docs/sessions/CURRENT_STATE.md` §3 待办任务中是否有可实施项
3. **用户反馈**：如有新需求可纳入
4. **v2.0 候选评估**：REQ-VEC-009 索引重建 / REQ-KB-002 Command Palette / REQ-ING-011 导入历史 / REQ-ING-012 文档更新 / REQ-VEC-011 分块参数配置 — 是否有可降级为 v1.12 的

评估维度：用户价值 40% + 实现易度 30% + 架构收益 20% + 风险控制 10%。ROI ≥ 5.0 入选。

### 任务 2：创建 `docs/architecture/V1.12_ATOMIC_PLAN.md`

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
| esbuild | 17196 行 |
| E2E | 1051+9=1060 passed |
| IPC 命令 | 204 Free / 214 Pro |
| 版本号 | 1.11.0 |

## v1.11 已完成功能摘要

| 功能 | 关键文件 | E2E 测试 |
|---|---|---|
| REQ-ING-010 文档内容预览 | `ui/src/doc-preview.js` + `crates/tauri-app/src/commands/document.rs`（get_document_preview） | TC-V11-PREVIEW-001~003 |
| REQ-IX-007 骨架屏 | `ui/src/skeleton.js` + `ui/src/main.js`（loadDocuments/loadConversations 集成） | TC-V11-SKELETON-001~003 |
| REQ-RAG-013 单条消息删除 | `crates/tauri-app/src/commands/conversation.rs`（delete_message）+ `ui/src/chat-render.js`（删除按钮） | TC-V11-DEL-001~003 |

## v2.0 候选参考

| REQ | 功能 | ROI | 备注 |
|---|---|---|---|
| REQ-VEC-009 | 索引重建 | 5.9 | 需进度追踪 UI |
| REQ-KB-002 | Command Palette | 5.5 | UI 组件开发量大 |
| REQ-ING-011 | 导入历史记录 | 5.7 | 调试用途 |
| REQ-ING-012 | 文档内容更新 | 6.0 | 需修改导入管线 |
| REQ-VEC-011 | 分块参数配置 | 5.9 | 跨层改动 |
| REQ-IX-002 | 会话拖拽排序 | 5.3 | 需 sort_order 字段 |

## 关键文件

| 文件 | 用途 |
|---|---|
| `docs/architecture/V1.11_ATOMIC_PLAN.md` | v1.11 规划（已完成） |
| `docs/CHANGELOG_v1.11.md` | v1.11 变更日志 |
| `docs/sessions/CURRENT_STATE.md` | 项目状态快照（必读） |
| `docs/SRS_v1.0.md` | 需求规格 |
| `.catpaw/rules/system-constraints.md` | 铁律 21 条 |
