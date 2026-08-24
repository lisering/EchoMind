你正在继续 EchoMind 项目的开发。上个 session 完成了 v1.9（检索/LLM 参数可配置 + 文档排序 + StripResult 扩展，10 TDD 测试通过）。

## 项目

EchoMind — Rust + Tauri v2 本地 RAG 知识库（律师/研究者高隐私场景）。六边形架构 8 crate，~100K 行 Rust + ~27K 行前端。

**当前基线**：v1.9.0 | 1255 passed / 0 failed / 31 ignored | clippy 零警告 | fmt 通过 | tsc 0 errors | 202 Free IPC

## 本 Session：v1.10 规划 + 可实施项评估 + TDD 实现 + 发布

v1.9 后端已完成但**前端 UI 未同步**（RAG/LLM 参数设置面板 + 文档排序下拉）。v1.10 需识别新可实施项并执行。

### 任务 1：评估候选功能

来源：(1) v1.9 后端对应前端 UI 完善 (2) SRS Draft REQ 扫描 (3) 技术债 (4) 用户反馈

维度：用户价值 40% + 实现易度 30% + 架构收益 20% + 风险控制 10%。ROI ≥ 5.0 入选。

### 任务 2：创建 `docs/architecture/V1.10_ATOMIC_PLAN.md`（ROI 矩阵 + TDD 计划）

### 任务 3：TDD 实现（先红灯再绿灯）

### 任务 4：文档同步 + `sync-to-github.sh --commit`

## v1.9 已完成（后端 only，前端待补）

| 功能 | 后端关键文件 | 前端待补 |
|---|---|---|
| REQ-RAG-014 检索参数可配置 | `settings.rs`（get/set_rag_params IPC）+ `chat.rs`（settings 驱动 top_k） | settings 面板 RAG 参数滑块 |
| REQ-RAG-015 LLM 生成参数可配置 | `openai_provider.rs`（with_generation_params）+ `settings.rs`（get/set_generation_params IPC） | settings 面板 LLM 参数滑块 |
| REQ-ING-008 文档列表排序 | `document.rs`（sort_documents + get_documents 排序参数） | 文档列表排序下拉 |

## v1.9 ROI 矩阵（v2.0 候选参考）

REQ-VEC-009 索引重建 ROI 6.6 / REQ-KB-002 Command Palette ROI 5.8 / REQ-RAG-013 单条消息删除 ROI 5.8 / REQ-ING-011 导入历史 ROI 5.7 — 均推迟 v2.0

## 规范

TDD（铁律二）| 零 unwrap/expect/panic | forbid unsafe | 文档同步（铁律七）| GitHub 同步（铁律十一）

## 验证

```bash
cargo clippy --all-targets -- -D warnings && cargo fmt --check && cargo test && npx tsc --noEmit && ./sync-to-github.sh --commit
```

## 必读文件

先读 `docs/sessions/CURRENT_STATE.md` 获取完整状态，再读 `docs/SRS_v1.0.md` 扫描 Draft REQ。读取后开始工作，无需等待确认。
