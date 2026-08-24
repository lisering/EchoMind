# EchoMind 下一步衔接提示词

> **复制下方 `~~~` 代码块中的全部内容作为下个 session 的第一条消息即可。只需复制一次。**

~~~
你正在继续 EchoMind（灵犀）项目的开发。上一个 session 完成了 DS4 (DwarfStar) 借鉴实施（DS-01 Prompt Prefix 磁盘缓存 + DS-02 上下文压缩增强 — 原文尾部保留）。

## 项目背景

EchoMind 是 Rust + Tauri v2 桌面 RAG 知识库应用。面向有隐私需求的个人用户，隐私优先、本地运行、BYOK（Bring Your Own Key）。

- **架构**: 六边形（端口与适配器），8 crate workspace
  - `crates/models` — 领域契约（Document, Chunk, ChatMessage, MemoryRecord 等）
  - `crates/prompt` — 提示词构建（SegmentedPrompt, RAG/Agent prompt, Cache policy）
  - `crates/compact` — 上下文压缩引擎（LLM 历史摘要替代截断, TokenEstimator, heal_dangling_calls, VerbatimTailConfig 原文尾部保留）
  - `crates/context` — 系统上下文注册表（纪元管理, 持久化基线, ContextEpochManager）
  - `crates/core` — 端口 Trait + 业务逻辑（ChatEngine, ImportService, AgentEngine, SecurityManager, MemoryStore, LlmRouter 等）
  - `crates/infra` — 适配器（SqliteStorage, LocalEmbedder, OpenAIProvider, HNSW, LocalLlmEngine mistral.rs, OCR, VLM, SymbolEngine, WasmExecutor, PromptCache 磁盘缓存）
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
- E2E (Playwright): 1019 passed, 10 skipped, 0 failed ✅ 全量通过
- IPC 命令数: ~175 Free / ~187 Pro（文档漂移待同步）
- ADR 数: 13

**注**: DS-01 + DS-02 新增的 41 个测试（infra 27 + compact 14）已通过 `cargo test -p echomind-infra prompt_cache` 和 `cargo test -p echomind-compact` 验证，但尚未纳入全量 `cargo test` 基线统计（预计 Free 全量将达 ~1100 passed）。下个 session 应运行全量 `cargo test` 更新基线。

## 上一个 Session 完成内容（DS4 借鉴实施）

### 调研对象
**ds4 (DwarfStar)** — antirez (Redis 作者) 开发的 DeepSeek V4 Flash 专用纯 C 推理引擎（`https://github.com/antirez/ds4`，本地源码 `/Users/john/code/ds4-main`）。支持 Metal/CUDA/ROCm 三后端 + SSD 流式加载 + 分布式推理 + 内置 Agent + OpenAI/Anthropic/Responses API 三协议兼容。

### DS-01: Prompt Prefix 磁盘缓存 + LRU 驱逐 ✅

**借鉴来源**: `ds4_kvstore.c` — ds4 的磁盘 KV 缓存系统

**实现文件**:
- `crates/infra/src/prompt_cache.rs` — KPC 文件格式(32字节头+JSON payload) + SHA1 匹配 + LRU 驱逐评分公式 `(effective_hits+1)*tokens/file_size*anchor_factor*exp2(-elapsed/6h)` + 边界裁剪(trim 32, align 2048) + 原子写入(.tmp→rename) + 预算管理(1% headroom)
- `crates/infra/src/prompt_cache_tests.rs` — 27 个 TDD 测试
- `crates/infra/src/lib.rs` — 注册 `prompt_cache` 模块 + `prompt_cache_tests` 测试模块
- `crates/infra/Cargo.toml` — 新增 `sha1 = "0.10"` 依赖

**EchoMind 适配**: mistral.rs v0.9.0 不暴露 KV 张量，缓存 token IDs + 元数据（非 KV payload）；`find_text_prefix`/`find_exact` 使用完整 SHA1 匹配（非前缀 SHA1）

### DS-02: 上下文压缩增强 — 原文尾部保留 ✅

**借鉴来源**: `misc/COMPACT.md` — ds4 的上下文压缩策略

**实现文件**:
- `crates/compact/src/lib.rs` — 新增 `VerbatimTailConfig`(tail_percentage/max_tail_tokens/soft_trigger_threshold/hard_trigger_remaining_tokens) + `needs_verbatim_tail_compaction()`(双触发: 软85%+硬8192) + `compact_with_verbatim_tail()`(摘要+原文尾部) + `align_to_user_boundary()`(对齐到user message)
- `crates/compact/src/compact_tests.rs` — 14 个 TDD 测试 TC-DS02-001~014

**关键设计**: 压缩后历史 = [摘要 system 消息] + [原文尾部消息]；LLM 失败降级为截断提示（不返回 Err）

### 验证结果
- infra prompt_cache: 27 passed, 0 failed
- compact: 65 passed (51 existing + 14 new), 0 failed
- clippy Free 零警告（infra + compact）
- cargo fmt 通过
- core crate 编译正常（pipeline_checkpoint.rs 修复 `Self::all()` → `PipelineStage::all()`）
- sync-to-github: 5 files, 443 insertions, 123 deletions

### 关键修复
- `is_none_or` → `is_none() || is_some_and()` (兼容性)
- `try_into().unwrap()` → 显式数组索引 (clippy deny)
- `live_tokens % step != 0` → `!live_tokens.is_multiple_of(step)` (clippy new lint)
- `.min_by(...).unwrap()` → `let Some(...) else { break; }` (clippy deny)
- 测试文件添加 `#![allow(clippy::unwrap_used, ...)]` 豁免
- `needless_update` clippy lint 修复（移除全字段结构体的 `..Default::default()`）

### DS4 剩余借鉴项（未实施，可后续考虑）

| ID | 借鉴项 | ds4 源码位置 | EchoMind 目标位置 | 优先级 | ROI |
|---|---|---|---|---|---|
| DS-03 | Session Strip 操作 | `ds4_agent.c` `/strip` | `crates/core/src/session.rs` | 高 | 8.0 |
| DS-04 | 本地 LLM 功率管理 | `ds4.c --power N` | `crates/infra/src/local_llm.rs` | 中 | 6.0 |
| DS-05 | Live Tool Call 续接 | `misc/RESPONSE_API.md` | `crates/core/src/agent.rs` | 中 | 4.0 |
| DS-06 | Radix Tree 精确重放 | `rax.c` | EchoMind 用 HashMap 即可 | 低 | — |
| DS-07 | SSD 流式自动缓存计划 | `ds4_ssd.c` | 已有 memory_budget.rs | 低 | — |

## Git 状态

- **分支**: main
- **发布仓库**: `EchoMind/EchoMind/` 已提交（commit `a5cfe1c`，5 files, 443 insertions, 123 deletions）
- **发布仓库无 remote 配置**：需手动 `cd EchoMind && git remote add origin <url> && git push`

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
- OpenMontage 借鉴（全部 ✅）: MMR 多样性重排 / 检索质量门控 / Scored Selector / Pipeline Checkpoint / Event Stream
- DS4 借鉴 DS-01 + DS-02（✅ 已完成）: Prompt Prefix 磁盘缓存 / 上下文压缩原文尾部保留
- DS4 借鉴 DS-03~DS-07（📋 待定）: Session Strip / 功率管理 / Tool Call 续接 / Radix Tree / SSD 流式

## 验证命令速查

```bash
cargo build                           # Free 编译
cargo build --release --features pro  # Pro Release 编译
cargo test                            # Free 测试（预计 ~1100 passed）
cargo test --features pro             # Pro 测试
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
- DS-03 Session Strip 操作（ds4 借鉴，ROI 8.0）
流程：ROI 评估 → 原子计划文档 → SRS REQ 定义 → TDD 开发

### 方向 2: 文档全面同步（高优先级）
- IPC 命令文档同步：`docs/api/ipc-commands.md` 更新到实际命令列表（162→~175/~187）
- `docs/api/code-contracts.md` 更新 QM/OpenCode/DS4 借鉴新增 Trait + PromptCache + VerbatimTailConfig
- `docs/api/database-schema.md` 更新 pending_inputs 等新表
- `docs/api/frontend-modules.md` 更新 zindex/panel-stack/focus-trap/input-keymap/input-history 模块
- `docs/api/settings-reference.md` 更新 Q01-Q11/B01-B14 新增设置键
- SRS 更新所有 QM/OpenCode/DS4 REQ 状态为 Implemented
- `docs/SOFTWARE_ASSET.md` 更新功能矩阵
- 全量 `cargo test` 更新测试基线（含 DS-01 27 + DS-02 14 新测试）

### 方向 3: E2E 测试覆盖增强
- 补充 QM/OpenCode/DS4 借鉴功能的 E2E 测试覆盖（记忆系统 / 上下文压缩 / 安全态势 / LLM Router / Prompt Cache 等新功能）
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

### 方向 6: DS4 剩余借鉴项实施
- DS-03: Session Strip 操作（`/strip` 命令，移除指定范围消息，ROI 8.0）
- DS-04: 本地 LLM 功率管理（`--power N` 限制 GPU 功耗，ROI 6.0）
- DS-05: Live Tool Call 续接（Agent 执行中工具调用状态保存/恢复，ROI 4.0）

请根据项目优先级选择下一步工作方向，并说明理由。先读取 `docs/sessions/CURRENT_STATE.md` 获取完整项目状态。读取后开始工作，无需等待确认。
~~~
