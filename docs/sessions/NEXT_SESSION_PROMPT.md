# EchoMind 下一步衔接提示词

> **复制下方 `~~~` 代码块中的全部内容作为下个 session 的第一条消息即可。只需复制一次。**

~~~
你正在继续 EchoMind（灵犀）项目的开发。上一个 session 完成了架构优化与技术债清理。

## 项目背景

EchoMind 是 Rust + Tauri v2 桌面 RAG 知识库应用。面向有隐私需求的个人用户，隐私优先、本地运行、BYOK（Bring Your Own Key）。

- **架构**: 六边形（端口与适配器），8 crate workspace
  - `crates/models` — 领域契约（Document, Chunk, ChatMessage, MemoryRecord 等）
  - `crates/prompt` — 提示词构建（SegmentedPrompt, RAG/Agent prompt, Cache policy）
  - `crates/compact` — 上下文压缩引擎（LLM 历史摘要替代截断, TokenEstimator, heal_dangling_calls）
  - `crates/context` — 系统上下文注册表（纪元管理, 持久化基线, ContextEpochManager）
  - `crates/core` — 端口 Trait + 业务逻辑（ChatEngine, ImportService, AgentEngine, SecurityManager, MemoryStore, LlmRouter 等）
  - `crates/infra` — 适配器（SqliteStorage, LocalEmbedder, OpenAIProvider, HNSW, LocalLlmEngine mistral.rs, OCR, VLM, SymbolEngine, WasmExecutor）
  - `crates/tauri-app` — Tauri 外壳，~175 个 IPC 命令 Free / ~187 Pro（模块化拆分到 commands/ 目录 14 个子模块）
  - `crates/license-issuer` — Ed25519 License 签发 CLI
- **前端**: 单文件 SPA（`ui/index.html`），vanilla JS ES modules（54 个源文件），esbuild 打包 IIFE（13582 行），Tailwind CSS JIT，无框架无 CDN
- **许可证**: BUSL-1.1（商业源码许可证，允许个人非商业使用，2030-01-01 转 Apache 2.0）
- **版本**: v1.4.0（全部 8 crate + tauri.conf.json）
- **Rust Edition**: 2024（≥1.85，原生 async fn in trait，无 #[async_trait] 宏）

## 当前测试基线（全量通过，零失败）

- Rust Free: 1059 passed, 0 failed, 28 ignored (`cargo test`)
- Rust Pro: 1094 passed, 0 failed, 44 ignored (`cargo test --features pro`)
- Clippy Free: 零警告 (`cargo clippy --all-targets -- -D warnings`)
- Clippy Pro: 零警告 (`cargo clippy --all-targets --features pro -- -D warnings`)
- cargo fmt: 通过
- TypeScript: 0 errors (`npx tsc --noEmit`)
- esbuild: 13582 行 (`node scripts/build-ui.mjs`)
- Vitest: 419 passed, 27 files (`cd e2e-tests && npm run test:unit`)
- E2E (Playwright): **1019 passed, 10 skipped, 0 failed** ✅ 全量通过
- IPC 命令数: ~175 Free / ~187 Pro（文档漂移待同步）
- ADR 数: 13

## 上一个 Session 完成内容

架构优化与技术债清理（5 项工作）：

1. **依赖更新** — `cargo update` 更新 69 个 Rust 包（ort rc.12→rc.13, fastembed 5.17.3→5.17.4, http 1.4.2→1.5.0, rustls 0.23.42→0.23.43, ureq 3.3.0→3.4.0 等）+ npm 更新 Playwright 1.61.1→1.62.1 + @wdio/* minor。编译通过 5m37s，全部测试通过。
2. **Pro 门控审计** — 12 编译时门控命令（`#[cfg(feature = "pro")]`：local_llm/KV cache/ColBERT/symbol）+ 4+ 运行时门控命令（is_pro 检查：audit/code_executor/custom_embedding）全部正确。发现 1 个 UX 改进点：`set_llm_mode("local")` 允许 Free 用户切换但 chat 时才报 PRO_REQUIRED（非安全漏洞，LlmRouter 在 chat 时拦截）。
3. **日文 i18n 补全** — `ja.json` 从 204 行扩展到完整翻译（约 600+ 键，覆盖全部 zh-CN.json 结构），包括 wizard/sidebar/chat/knowledge_base/import/settings/paywall/command_palette/markdown/doc_phases/presets/menu/errors/backend/time/ctx/security/db_error/download/graph/code_executor/workflow/memory/dream/symbol/perf/voice/web_search/export_pdf/wiki/tree 全部 section。
4. **编译速度分析** — 8 crate workspace 分割已优化（prompt/compact/context 从 core 拆出），dev profile opt-level=3 已设置，主要瓶颈 ML 依赖（fastembed/ort/mistral.rs）不可避免。无需重大改变。
5. **文档核查** — 发现 IPC 命令数文档漂移（文档 162 vs 实际 ~175 Free/~187 Pro），Q01-Q11 + B01-B14 新增命令未同步到 ipc-commands.md/code-contracts.md 等文档。

## Git 状态

- **分支**: main
- **未提交变更**: 4 文件（Cargo.lock + package-lock.json + ui/locales/ja.json + ui/index.html）
- **发布仓库**: `EchoMind/EchoMind/`（通过 `sync-to-github.sh` 同步，无 remote 配置，需手动 `git remote add` 后 `git push`）

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
cargo test                            # Free 测试 (1059 passed)
cargo test --features pro             # Pro 测试 (1094 passed)
cargo clippy --all-targets -- -D warnings          # Free Clippy
cargo clippy --all-targets --features pro -- -D warnings  # Pro Clippy
cargo fmt --check                     # 格式检查
npx tsc --noEmit                      # TypeScript 类型检查
node scripts/build-ui.mjs             # 前端 esbuild 打包
cd e2e-tests && npm run test:bridge   # E2E 桥接测试 (1019 passed)
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

### 方向 2: 文档全面同步（高优先级）
- IPC 命令文档同步：`docs/api/ipc-commands.md` 更新到实际命令列表（162→~175/~187）
- `docs/api/code-contracts.md` 更新 QM/OpenCode 借鉴新增 Trait
- `docs/api/database-schema.md` 更新 pending_inputs 等新表
- `docs/api/frontend-modules.md` 更新 zindex/panel-stack/focus-trap/input-keymap/input-history 模块
- `docs/api/settings-reference.md` 更新 Q01-Q11/B01-B14 新增设置键
- SRS 更新所有 QM/OpenCode REQ 状态为 Implemented
- `docs/SOFTWARE_ASSET.md` 更新功能矩阵

### 方向 3: E2E 测试覆盖增强
- 补充 QM/OpenCode 借鉴功能的 E2E 测试覆盖（记忆系统 / 上下文压缩 / 安全态势 / LLM Router 等新功能）
- 混沌测试覆盖率提升
- CI 集成 E2E 测试到 GitHub Actions
- 补充 dream-panel.spec.ts 中 4 个 pre-existing 失败修复（DOM ID 重复 strict mode violation）

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
