# EchoMind 下一步衔接提示词

> **复制下方 `~~~` 代码块中的全部内容作为下个 session 的第一条消息即可。只需复制一次。**

~~~
你正在继续 EchoMind（灵犀）项目的开发。上一个 session 完成了 CatPaw IDE 配置治理（System Prompt 改为 EchoMind 专属、三套 commands 目录 33 文件、索引 & Docs 官方文档清单 50 条）。

## 项目背景

EchoMind 是 Rust + Tauri v2 桌面 RAG 知识库应用。面向有隐私需求的个人用户，隐私优先、本地运行、BYOK（Bring Your Own Key）。

- **架构**: 六边形（端口与适配器），8 crate workspace
  - `crates/models` — 领域契约（Document, Chunk, ChatMessage, MemoryRecord 等）
  - `crates/prompt` — 提示词构建（SegmentedPrompt, RAG/Agent prompt, Cache policy）
  - `crates/compact` — 上下文压缩引擎（LLM 历史摘要替代截断, TokenEstimator, heal_dangling_calls）
  - `crates/context` — 系统上下文注册表（纪元管理, 持久化基线, ContextEpochManager）
  - `crates/core` — 端口 Trait + 业务逻辑（ChatEngine, ImportService, AgentEngine, SecurityManager, MemoryStore, LlmRouter 等）
  - `crates/infra` — 适配器（SqliteStorage, LocalEmbedder, OpenAIProvider, HNSW, LocalLlmEngine mistral.rs, OCR, VLM, SymbolEngine, WasmExecutor）
  - `crates/tauri-app` — Tauri 外壳，162 个 IPC 命令（模块化拆分到 commands/ 目录 14 个子模块，8889 行）
  - `crates/license-issuer` — Ed25519 License 签发 CLI（私有，不公开到 GitHub）
- **前端**: 单文件 SPA（`ui/index.html`，126K 行），vanilla JS ES modules（50 个源文件），esbuild 打包 IIFE，Tailwind CSS JIT，无框架无 CDN
- **许可证**: BUSL-1.1（商业源码许可证，允许个人非商业使用，2030-01-01 转 Apache 2.0）
- **版本**: v1.4.0（全部 8 crate + tauri.conf.json）
- **Rust Edition**: 2024（≥1.85，原生 async fn in trait，无 #[async_trait] 宏）

## 当前测试基线

- Rust Free: 822+ passed, 0 failed, 24 ignored (`cargo test`)
- Rust Pro: 1094 passed, 0 failed, 44 ignored (`cargo test --features pro`)
- Clippy Free+Pro: 零警告 (`cargo clippy --all-targets -- -D warnings`)
- cargo fmt: 通过
- TypeScript: 0 errors (`npx tsc --noEmit`)
- esbuild: 12451 行 (`node scripts/build-ui.mjs`)
- Vitest: 407 passed, 27 files (`cd e2e-tests && npm run test:unit`)
- E2E 桥接: 955 passed, 9 skipped (`cd e2e-tests && npm run test:bridge`)
- 测试文件: 68 个 `*_tests.rs` 文件（core 44 + infra 22 + compact 1 + context 1）
- IPC 命令数: 162（14 个功能域子模块）
- ADR 数: 13

## 铁律要点（详见 .catpaw/rules/system-constraints.md v3.3）

1. **TDD 驱动**（铁律二）: Red → Green → Refactor，禁空断言
2. **零 unwrap/expect/panic**（铁律五）: workspace clippy deny lint 全覆盖（unwrap_used/expect_used/panic/unreachable/todo/unimplemented 全 deny）
3. **Edition 2024**（铁律三）: 原生 async fn in trait，禁用 async-trait 宏，`#![allow(async_fn_in_trait)]` 是有意的豁免
4. **文档同步**（铁律七）: 代码变更必须同步更新 docs/api/ 对应文档 + docs/SOFTWARE_ASSET.md
5. **最新版本依赖**（铁律六）: 开发前 cargo update --dry-run + npm outdated
6. **GitHub 同步**（铁律十一）: 修改源代码后必须运行 ./sync-to-github.sh（双仓库工作流，开发仓库 → 发布仓库 EchoMind/EchoMind/）
7. **Session 管理**（铁律十二）: 会话开始读 CURRENT_STATE.md，会话结束更新 CURRENT_STATE.md + LOG.md + 生成衔接提示词
8. **Spec-driven**（铁律十五）: 先写规格再写代码（SRS REQ + AC + TC），测试从 AC 派生
9. **Vibe Coding 测试门禁**（铁律二十）: L0 fmt/clippy → L1 nextest → L2 llvm-cov → L3 mutants/miri → L4 E2E
10. **网络代理**（铁律一）: 开发调研可设代理 `export https_proxy=http://127.0.0.1:7890`；软件运行时禁止代理（所有 reqwest::Client::builder() 必须加 .no_proxy()）

## 已完成版本

- v1.0.0: 核心功能（RAG 检索 + 文档导入 + 流式对话 + 安全加密 + 本地 LLM Phase 1-3 + Agent RAG + 极致性能优化 S1-S10）
- v1.1.0: .docx 导入 / .html 导入 / 浅色主题 / 语音输入+TTS / 网页搜索
- v1.2.0: PDF 导出 / .pptx 导入 / .epub 导入 / 自定义 ONNX 嵌入模型（Pro）
- v1.3.0: 知识图谱导出 / 文档摘要 / 快捷指令模板 / Wiki 双向链接 / 对话分支树
- v1.4.0: Excel/CSV 导入 / 对话全文搜索 / Contextual Retrieval / 文档标签 / commands.rs 模块化

## 已完成的借鉴计划

### QM 借鉴（Q01-Q11，全部 ✅）
- Q01: Scratch-Promote 两层记忆 + Consolidation 引擎
- Q02: 记忆摘要注入上下文
- Q03-Q04: Token 级预算 + Entry Token 缓存 + Dangling Call 修复
- Q05-Q06: 安全态势分层 + Shadow 安全筛查
- Q07-Q09: 后台压缩 Sweeper + 幂等性存储 + KeyedQueue 并发
- Q10-Q11: LLMProvider Trait 扩展（generate_title/one_shot/judge）+ LlmRouter 路由

### OpenCode 借鉴（B01-B14，全部 ✅）
- B01-B02: System Context Registry + Context Epoch + Durable Baseline
- B03-B04: 结构化压缩模板 + Prompt Caching 自动放置
- B05-B06: Durable Prompt Admission + Session Run Coordinator
- B07-B08: Tool Output Bounding + Ask SDK
- B09: Skill 系统发现
- B10-B14: 多协议 LLM 适配 + Permission 细粒度控制 + Snapshot/Plugin/Effect 评估

### 跨 Phase 整合（S67+S68+S71，全部 ✅）
- S67: Burst Buffer 用 Q10 one_shot 替代非流式 LLM 调用
- S68: 后台压缩用 Q07 Sweeper 统一管理
- S71: Shadow 筛查用 Q10 judge 方法做安全判断

## 本 session 完成的工作

1. **system-constraints.md v3.2 → v3.3**: 标题改为「EchoMind（灵犀）系统约束」，新增项目概要/技术栈/Crate 结构/依赖方向铁律
2. **三套 commands 目录**: `.catpaw/commands/` + `.claude/commands/` + `.cursor/commands/`，各 11 个命令文件（build/test/check/session-start/session-end/feature/e2e/doc-sync/github-sync/new-ipc/new-req）
3. **system-prompt.md**: CatPaw Settings → Prompt & Rules → System Prompt 的 EchoMind 专属内容（项目核心/Crate 结构/关键约束/验证命令/必读文档优先级）
4. **docs-index.md**: CatPaw Settings → 索引 & Docs → Docs 的 50 条官方文档清单（P0 必加 8 条 + P1 Rust 生态 14 条 + P1 Tauri 2 条 + P1 数据库&加密 8 条 + P1 文档解析&ML 8 条 + P1 其他 2 条 + P2 前端工具链 4 条 + P2 前端库 6 条 + P2 Pro 6 条），全部为官方域名

## Git 状态

- **分支**: main
- **未提交变更**: 101 个文件（80 modified + 21 untracked）
- **关键 untracked**: `.catpaw/commands/`（33 命令文件）、`.claude/`、`.cursor/`、`crates/core/src/{budget,concurrency,idempotency,llm_router,security}_tests.rs`、`ui/locales/ja.json`
- **发布仓库**: `EchoMind/EchoMind/`（通过 `./sync-to-github.sh` 同步，GitHub: lisering/EchoMind）

## 验证命令速查

```bash
cargo build                           # Free 编译
cargo build --release --features pro  # Pro Release 编译（~4min）
cargo test                            # Free 测试 (822+ passed)
cargo test --features pro             # Pro 测试 (1094 passed)
cargo test -p <crate>                 # 单 crate 测试
cargo test <name>                     # 聚焦测试
cargo clippy --all-targets -- -D warnings          # Free Clippy
cargo clippy --all-targets --features pro -- -D warnings  # Pro Clippy（~6m39s）
cargo fmt --check                     # 格式检查
npx tsc --noEmit                      # TypeScript 类型检查
node scripts/build-ui.mjs             # 前端 esbuild 打包
cd e2e-tests && npm run test:bridge   # E2E 桥接测试 (955 passed)
cd e2e-tests && npm run test:unit     # Vitest 前端测试 (407 passed)
./sync-to-github.sh                   # GitHub 发布同步
```

## 关键文件索引

| 文件 | 用途 |
|---|---|
| `docs/sessions/CURRENT_STATE.md` | 项目状态快照（新 session 第一必读） |
| `.catpaw/rules/system-constraints.md` | 铁律规则 v3.3（21 条） |
| `.catpaw/rules/system-prompt.md` | CatPaw System Prompt 内容 |
| `.catpaw/rules/docs-index.md` | CatPaw 索引 & Docs 官方文档清单（50 条） |
| `docs/SRS_v1.0.md` | 需求规格唯一依据（REQ-<域>-<序号> 格式） |
| `docs/api/ipc-commands.md` | IPC 命令签名（162 条） |
| `docs/api/code-contracts.md` | Trait 签名 + 数据模型 |
| `docs/api/database-schema.md` | 数据库表结构 |
| `docs/api/settings-reference.md` | 设置键 + Tauri 事件 |
| `docs/api/frontend-modules.md` | 前端模块函数清单（50 个模块） |
| `docs/SOFTWARE_ASSET.md` | 软件资产全景文档（1100+ 行） |
| `docs/architecture/QM_BORROW_ATOMIC_PLAN.md` | QM 借鉴计划（Q01-Q11 全部 ✅） |
| `docs/architecture/OPENCODE_BORROW_ATOMIC_PLAN.md` | OpenCode 借鉴计划（B01-B14 全部 ✅） |
| `AGENTS.md` | AI Agent 指南（架构 + 数据流 + Key files） |
| `CLAUDE.md` | Claude Code 指南 |
| `PROJECT_RULES.md` | 项目宪法（三体系规则） |
| `docs/architecture/adr/` | 13 个架构决策记录 |
| `sync-to-github.sh` | GitHub 发布同步脚本 |

## 下一步可选方向（请选择其一开始，并说明理由）

### 方向 1: v1.5 新功能规划与开发
- MCP 协议支持（待 Rust SDK 成熟，需互联网搜索调研）
- Late Chunking（需长上下文嵌入模型）
- RAG 评估指标（RAGAS 风格，faithfulness/relevance/recall）
- 多窗口支持（多知识库并行）
- 对话分支管理增强
- 多模态文档处理增强（图片/图表 OCR 提取优化）
流程：ROI 评估 → 原子计划文档 → SRS REQ 定义 → TDD 开发

### 方向 2: Git 状态清理与提交
- 当前 101 个未提交文件（80 modified + 21 untracked）
- 需要 git add + commit 将 QM 借鉴 Q01-Q11 + OpenCode 借鉴 B01-B14 + 跨 Phase 整合 + CatPaw 配置治理 的全部成果提交
- 运行 ./sync-to-github.sh 同步到发布仓库

### 方向 3: E2E 测试质量提升
- 修复 pre-existing E2E 失败用例（navigation-advanced 2 failed + error-handling 2 failed）
- 补充 QM 借鉴功能的 E2E 测试覆盖（qm-borrow-features.spec.ts 已存在但可能不完整）
- 混沌测试覆盖率提升
- CI 集成 E2E 测试到 GitHub Actions

### 方向 4: 架构优化与技术债清理
- 编译速度优化（crate 分割、增量编译优化）
- Pro 功能审计（确保所有 Pro 门控正确）
- 文档全面同步（SRS/PROJECT_RULES 更新到最新状态）
- 依赖版本更新（cargo update + npm outdated）
- 新增日文 i18n（ui/locales/ja.json 已存在但可能不完整）

### 方向 5: 生产就绪加固
- 日志系统增强（结构化日志 + OpenTelemetry）
- 错误恢复机制完善
- 数据迁移工具（版本升级时 SQLite schema 迁移脚本）
- 性能基准自动化（CI 集成 benchmark 回归检测）
- 安全审计（渗透测试、PII 脱敏验证、SQLCipher 验证）

### 方向 6: 商业化准备
- License 购买流程（支付集成、License Key 发放自动化）
- 用户文档/帮助系统
- 官网建设
- App Store / Microsoft Store 上架准备

请根据项目优先级选择下一步工作方向，并说明理由。先读取 `docs/sessions/CURRENT_STATE.md` 获取完整项目状态。
~~~
