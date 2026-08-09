# EchoMind 下一步衔接提示词

> **复制下方 `~~~` 代码块中的全部内容作为下个 session 的第一条消息即可。只需复制一次。**

~~~
你正在继续 EchoMind（灵犀）项目的开发。上一个 session 完成了 Git 状态清理（49 个未提交文件分 4 组提交）。

## 项目背景

EchoMind 是 Rust + Tauri v2 桌面 RAG 知识库应用。面向有隐私需求的个人用户，隐私优先、本地运行、BYOK（Bring Your Own Key）。

- **架构**: 六边形（端口与适配器），8 crate workspace
  - `crates/models` — 领域契约（Document, Chunk, ChatMessage, MemoryRecord 等）
  - `crates/prompt` — 提示词构建（SegmentedPrompt, RAG/Agent prompt, Cache policy）
  - `crates/compact` — 上下文压缩引擎（LLM 历史摘要替代截断, TokenEstimator, heal_dangling_calls）
  - `crates/context` — 系统上下文注册表（纪元管理, 持久化基线, ContextEpochManager）
  - `crates/core` — 端口 Trait + 业务逻辑（ChatEngine, ImportService, AgentEngine, SecurityManager, MemoryStore, LlmRouter 等）
  - `crates/infra` — 适配器（SqliteStorage, LocalEmbedder, OpenAIProvider, HNSW, LocalLlmEngine mistral.rs, OCR, VLM, SymbolEngine, WasmExecutor）
  - `crates/tauri-app` — Tauri 外壳，162 个 IPC 命令（模块化拆分到 commands/ 目录 14 个子模块）
  - `crates/license-issuer` — Ed25519 License 签发 CLI
- **前端**: 单文件 SPA（`ui/index.html`），vanilla JS ES modules（54 个源文件），esbuild 打包 IIFE（13580 行），Tailwind CSS JIT，无框架无 CDN
- **许可证**: BUSL-1.1（商业源码许可证，允许个人非商业使用，2030-01-01 转 Apache 2.0）
- **版本**: v1.4.0（全部 8 crate + tauri.conf.json）
- **Rust Edition**: 2024（≥1.85，原生 async fn in trait，无 #[async_trait] 宏）

## 当前测试基线

- Rust Free: 1028 passed, 9 failed (pre-existing), 20 ignored (`cargo test`)
- Rust Pro: 1094 passed, 0 failed, 44 ignored (`cargo test --features pro`)
- Clippy Free: 零警告 (`cargo clippy --all-targets -- -D warnings`)
- cargo fmt: 通过
- TypeScript: 0 errors (`npx tsc --noEmit`)
- esbuild: 13580 行 (`node scripts/build-ui.mjs`)
- Vitest: 419 passed, 27 files (`cd e2e-tests && npm run test:unit`)
- E2E 桥接: pre-existing 失败在 error-handling.spec.ts（Alpha 阶段付费墙不触发）
- IPC 命令数: 162
- ADR 数: 13

## Pre-existing 失败说明

9 个 Rust 集成测试失败全在 `echomind-tauri-app` 集成测试中：
- `tc_edge_003_free_tier_50_doc_limit` / `tc_lic_003_free_tier_quota_blocked` — 配额限制
- `tc_lic_005*` / `tc_lic_006*` — 许可证验证
- `tc_pro_gate_*` — Pro 门控
- `tc_vec_custom_005_free_user_rejected` — 自定义模型 Pro 门控
原因是 Alpha 阶段 `ALPHA_ALL_FEATURES_FREE = true` 配置，与前端改造无关。

## 前端架构升级（Session 1-3 已完成）

- **Z-index 分层**: `zindex.js` — 12 层级常量 + zClass() 辅助函数
- **面板栈**: `panel-stack.js` — 全局面板栈（push/remove/closeTop/peek/isOpen/hasOpen/clear）
- **Focus Trap 栈**: `focus-trap.js` — LIFO 栈管理（pause/resume），只激活栈顶 Trap
- **State 补全**: `state.js` 新增 6 字段（hybridEnabled/agentEnabled/subAgentEnabled/memoryEnabled/webSearchEnabled/docList）
- **会话切换关闭面板**: `main.js` `closeAllPanels()` 函数
- **统一输入 keydown**: `input-keymap.js` — 斜杠命令 > Enter > 历史 > Escape
- **输入历史/草稿**: `input-history.js` — 会话级历史导航 + 草稿持久化 + Token 估算

## Git 状态（Session 4 清理完成）

- **分支**: main
- **未提交变更**: 0
- **提交历史**: 4 个分组提交（前端架构 / Rust 后端 / 文档 / submodule）
- **发布仓库**: `EchoMind/EchoMind/`（无 remote，需手动 `git remote add` 后 `git push`）

## 已完成版本

- v1.0.0: 核心功能（RAG + 文档导入 + 流式对话 + 安全加密 + 本地 LLM + Agent RAG + 极致性能优化）
- v1.1.0: .docx 导入 / .html 导入 / 浅色主题 / 语音输入+TTS / 网页搜索
- v1.2.0: PDF 导出 / .pptx 导入 / .epub 导入 / 自定义 ONNX 嵌入模型（Pro）
- v1.3.0: 知识图谱导出 / 文档摘要 / 快捷指令模板 / Wiki 双向链接 / 对话分支树
- v1.4.0: Excel/CSV 导入 / 对话全文搜索 / Contextual Retrieval / 文档标签 / commands.rs 模块化

## 已完成的借鉴计划

- QM 借鉴 Q01-Q11（全部 ✅）: 记忆系统 / 上下文压缩 / 安全层 / 基础设施 / LLMProvider
- OpenCode 借鉴 B01-B14（全部 ✅）: System Context / Epoch / 压缩模板 / Prompt Caching / Admission / Coordinator / Tool Bounding / Skill / 多协议 / Permission
- 跨 Phase 整合 S67+S68+S71（全部 ✅）: Burst Buffer / Sweeper / Shadow 筛查

## 验证命令速查

```bash
cargo build                           # Free 编译
cargo build --release --features pro  # Pro Release 编译
cargo test                            # Free 测试 (1028 passed, 9 pre-existing failed)
cargo test --features pro             # Pro 测试 (1094 passed)
cargo clippy --all-targets -- -D warnings          # Free Clippy
cargo clippy --all-targets --features pro -- -D warnings  # Pro Clippy
cargo fmt --check                     # 格式检查
npx tsc --noEmit                      # TypeScript 类型检查
node scripts/build-ui.mjs             # 前端 esbuild 打包
cd e2e-tests && npm run test:bridge   # E2E 桥接测试
cd e2e-tests && npm run test:unit     # Vitest 前端测试 (419 passed)
./sync-to-github.sh --commit          # 同步到发布仓库
```

## 下一步可选方向（请选择其一开始，并说明理由）

### 方向 1: v1.5 新功能规划与开发
- MCP 协议支持（待 Rust SDK 成熟，需互联网搜索调研）
- Late Chunking（需长上下文嵌入模型）
- RAG 评估指标（RAGAS 风格，faithfulness/relevance/recall）
- 多窗口支持（多知识库并行）
- 对话分支管理增强
- 多模态文档处理增强（图片/图表 OCR 提取优化）
流程：ROI 评估 → 原子计划文档 → SRS REQ 定义 → TDD 开发

### 方向 2: E2E 测试质量提升
- 修复 9 个 pre-existing Rust 集成测试失败（Alpha 阶段配置问题）
- 修复 pre-existing E2E 失败用例（error-handling.spec.ts 付费墙 + 超长标题）
- 补充 QM/OpenCode 借鉴功能的 E2E 测试覆盖
- 混沌测试覆盖率提升
- CI 集成 E2E 测试到 GitHub Actions

### 方向 3: 架构优化与技术债清理
- 编译速度优化（crate 分割、增量编译优化）
- Pro 功能审计（确保所有 Pro 门控正确）
- 文档全面同步（SRS/PROJECT_RULES 更新到最新状态）
- 依赖版本更新（cargo update + npm outdated）
- 新增日文 i18n（ui/locales/ja.json 已存在但可能不完整）

### 方向 4: 生产就绪加固
- 日志系统增强（结构化日志 + OpenTelemetry）
- 错误恢复机制完善
- 数据迁移工具（版本升级时 SQLite schema 迁移脚本）
- 性能基准自动化（CI 集成 benchmark 回归检测）
- 安全审计（渗透测试、PII 脱敏验证、SQLCipher 验证）

### 方向 5: 商业化准备
- License 购买流程（支付集成、License Key 发放自动化）
- 用户文档/帮助系统
- 官网建设
- App Store / Microsoft Store 上架准备

请根据项目优先级选择下一步工作方向，并说明理由。先读取 `docs/sessions/CURRENT_STATE.md` 获取完整项目状态。
~~~
