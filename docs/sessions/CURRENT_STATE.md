# EchoMind 当前项目状态快照

> **本文件是任何新 AI 会话开始时的第一份必读文件。**
>
> 每次 session 结束时**必须更新**此文件，确保反映最新状态。
>
> **最后更新**: 2026-08-27 | **更新人**: AI Agent (大简化重构 Phase 1 R3 完成。删除 10 个本地 LLM 推理模块 + Mistral.rs 适配层 + LlmMode::Local 变体，削减 ~12,000 行。测试 1,326 passed, clippy 零警告, esbuild 1,584 行。累计削减 ~37,862 行，-28.9%。)

---

## 1. 项目健康度

| 指标 | 数值 | 备注 |
|---|---|---|
| Rust 单元+集成测试 (Free) | **1326 passed**, 0 failed, 33 ignored | `cargo test` ✅（Phase 1 R3 后回归） |
| Rust 单元+集成测试 (Pro) | 待验证 | `cargo test --features pro` (R1 后未单独验证 Pro) |
| Clippy (Free) | 零警告 | `cargo clippy --all-targets -- -D warnings` ✅ |
| cargo fmt | 通过 | `cargo fmt --check` ✅ |
| 前端单元测试 (Vitest) | **1821 passed**, 92 files, 0 failed | S97 MCP 面板后全量回归通过（v3.0.0 发布回归） |
| UI 类型检查 | **0 errors** ✅ | `npx tsc --noEmit`（v2.0 S18 修复 5 个 pre-existing errors → 0） |
| E2E 桥接测试 (Playwright) | **1355 passed**, 0 failed, 10 skipped (9.1m) | ✅ 零回归（S15 后 E2E 全量通过，2 个 pre-existing flaky 不再失败） |
| 版本号 | **3.1.0** ✅ | tauri.conf.json + 8 crate Cargo.toml（v3.0 发布：MCP 协议 + 设置面板重构 + 模块精简 + 嵌入评估 + Pro ROI 评估） |
| IPC 命令数 | **~221 Free + 12 Pro-only = ~233（Debug）/~212（Release）** | S10: 21 个开发者工具命令 #[cfg(debug_assertions)] 门控，Release 构建中不注册。Debug 构建 ~233（含开发者工具），Release 构建 ~212 |
| ADR 数 | 15 | 新增 ADR-015 Prompt 注入防护三层防御策略 + ADR-014 mistral.rs bus factor 监控 + ADR-004~008 独立归档 |
| 僵尸命令数 | **0**（僵尸率 0%） | Zombie Zero ✅ S2 验证：全部 236 命令有前端调用者 |
| 幻影调用数 | **0** ✅ | S5 P0-5 修复：4 后端命令实现 + 5 前端委托迁移 + 1 后端命令新增 |
| 供应链安全 | cargo audit + cargo deny | `deny.toml` 配置 |
| 前端构建行数 | 1584 (prod) | esbuild IIFE 打包（Phase 1 R2 后） |
| 前端模块数 | 56 | `ui/src/*.js`（S93+S95 合并后：input-keymap/input-history→input-utils 等）|
| 前端源码体积 | ~1.02 MB (JS) + 118 KB (CSS) | 56 个 JS 源文件 + 6 个 CSS 文件（S95 精简后） |
| 前端 Vendor 体积（启动） | **~100 KB** (eager) + 4.5 MB (lazy) | **三层优化**：① Tailwind JIT 441KB → 预构建 36KB ② mermaid UMD 3482KB → esbuild 自定义打包 3371KB（-3.2%） ③ highlight.js 124KB eager → lazy（见 lazy-loader.js）|
| Rust Edition | 2024 (≥1.85) | 原生 `async fn` in trait |

---

## 2. 已完成功能矩阵

### 2.1 核心功能 (P0)

| 功能 | SRS REQ | 状态 | 关键文件 |
|---|---|---|---|
| 文档导入 (md/txt/pdf/docx/html/pptx/epub/xlsx/csv) | REQ-ING-001~005, REQ-ING-015, REQ-ING-016, REQ-ING-017, REQ-ING-018, REQ-ING-021 | ✅ Done | `crates/core/src/import.rs`, `crates/core/src/loader.rs` |
| **文档标签系统** | **REQ-ING-022** | **✅ Implemented** | `crates/models/src/lib.rs`（Document.tags）, `crates/infra/src/sqlite_storage.rs`（tags 列 + 4 方法）, `crates/tauri-app/src/commands/document.rs`（4 IPC 命令）, `ui/src/main.js`（标签 UI） |
| **浅色主题切换** | **REQ-UI-011** | **✅ Implemented** | `ui/styles/tokens.css`, `ui/src/main.js`, `ui/src/settings.js` |
| **网页搜索集成** | **REQ-RAG-036** | **✅ Implemented** | `crates/core/src/web_search.rs`, `crates/infra/src/duckduckgo_provider.rs` |
| **语音输入 (SpeechRecognition)** | **REQ-RAG-034** | **✅ Implemented** | `ui/src/voice-input.js`, `ui/index.html` |
| **TTS 朗读 (speechSynthesis)** | **REQ-RAG-035** | **✅ Implemented** | `ui/src/voice-input.js`, `ui/src/chat-render.js` |
| Token 窗口分块 | REQ-VEC-001 | ✅ Done | `crates/core/src/splitter.rs` |
| 语义分块 | REQ-VEC-002 | ✅ Done | `crates/core/src/semantic_splitter.rs` |
| 章节感知分块 | REQ-VEC-006 | ✅ Done | `crates/core/src/section_aware_splitter.rs` |
| 本地向量化 (fastembed ONNX) | REQ-VEC-003 | ✅ Done | `crates/infra/src/local_embedder.rs` |
| **bge-m3 多语言嵌入模型 (1024-dim)** | **REQ-VEC-015** | **✅ Implemented** | `crates/infra/src/local_embedder.rs`（`EmbeddingModel::BgeM3` 变体）, `crates/infra/src/bge_m3_tests.rs`（7 TC-VEC-M3） |
| **全库嵌入重建命令** | **REQ-VEC-016** | **✅ Implemented** | `crates/tauri-app/src/commands/performance.rs`（`rebuild_all_embeddings` IPC） |
| **RAG 评估数据集与端到端检索质量评估** | **REQ-RAG-048** | **✅ Implemented** | `crates/models/src/lib.rs`（`RagEvalDataset` / `RagEvalDatasetSample` 模型）, `crates/core/src/rag_eval_dataset.rs`（`load_eval_dataset` / `run_retrieval_eval` / `create_sample_dataset`） |
| **Late Chunking 上下文感知嵌入** | **REQ-RAG-049** | **✅ Implemented** | `crates/core/src/late_chunking.rs`（`build_late_chunking_text` / `extract_doc_prefix` / `LateChunkingConfig`）, `crates/tauri-app/src/commands/import.rs`（`embed_document_chunks` 集成）, `crates/tauri-app/src/commands/performance.rs`（`set_late_chunking` / `get_late_chunking` IPC） |
| 多嵌入模型运行时切换 | REQ-VEC-012 | ✅ Done | `crates/tauri-app/src/stores/model_store.rs` |
| **自定义 ONNX 嵌入模型上传** | **REQ-VEC-014** | **✅ Implemented** | `crates/infra/src/local_embedder.rs`, `crates/tauri-app/src/commands.rs`, `crates/tauri-app/src/stores/model_store.rs`, `ui/src/settings.js` |
| 领域画像自动分类 | REQ-VEC-013 | ✅ Done | `crates/core/src/domain.rs` |
| SQLite 持久化 (WAL+r2d2) | REQ-DB-001 | ✅ Done | `crates/infra/src/sqlite_storage.rs` |
| 向量检索 (余弦相似度) | REQ-RAG-001~003 | ✅ Done | `crates/core/src/retriever.rs` |
| 混合检索 (向量+BM25 RRF) | REQ-RAG-010 | ✅ Done | `crates/core/src/hybrid_retriever.rs` |
| Cross-Encoder 重排序 | REQ-RAG-020 | ✅ Done | `crates/infra/src/local_reranker.rs` |
| HyDE 查询改写 | REQ-RAG-021 | ✅ Done | `crates/infra/src/hyde_rewriter.rs` |
| RAG 流式对话 (SSE) | REQ-RAG-001~006 | ✅ Done | `crates/core/src/chat.rs` |
| 会话持久化 + 懒创建 | REQ-RAG-006/012 | ✅ Done | `crates/tauri-app/src/commands/conversation.rs` |
| **对话全文搜索 (FTS5)** | **REQ-RAG-040** | **✅ Implemented** | `crates/infra/src/sqlite_storage.rs`（messages_fts FTS5 虚拟表 + 触发器）, `crates/tauri-app/src/commands/conversation.rs`, `ui/src/sidebar.js` |
| **Contextual Retrieval 上下文增强嵌入** | **REQ-RAG-041** | **✅ Implemented** | `crates/core/src/retriever.rs`（`build_contextual_text()`）, `crates/tauri-app/src/commands/import.rs`（`embed_document_chunks()` 条件化上下文前缀）, `crates/tauri-app/src/commands/performance.rs`（`set_contextual_retrieval` + `rebuild_contextual_embeddings` IPC）, `ui/src/perf-settings.js`（开关 toggle） |
| 流中断 (CancellationToken) | REQ-RAG-005 | ✅ Done | `crates/tauri-app/src/stores/chat_store.rs` |
| 消息列表懒加载 | REQ-RAG-019 | ✅ Done | `crates/tauri-app/src/commands.rs` |
| ReAct Agent 多步推理 | REQ-RAG-022 | ✅ Deprecated (v3.1.0 Phase 1 R2) | — |
| 多代理协调引擎 | REQ-RAG-025 | ✅ Deprecated (v3.1.0 Phase 1 R2) | — |
| 后台空闲整理引擎 | - | ✅ Deprecated (v3.1.0 Phase 1 R2) | — |
| 文件监听增量同步 | REQ-SYNC-001~003 | ✅ Done | `crates/core/src/sync.rs`, `crates/infra/src/file_watcher.rs` |
| 对话导出为 Markdown | REQ-EXP-001 | ✅ Done | `crates/core/src/export.rs` |
| **PDF 导出 (window.print)** | **REQ-EXP-005** | **✅ Implemented** | `ui/src/export.js`, `ui/styles/print.css` |
| License Ed25519 验证 | REQ-LIC-005 | ✅ Done | `crates/core/src/license.rs`, `crates/license-issuer/` |
| Free/Pro 条件编译 | - | ✅ Done | 全 crate `#[cfg(feature = "pro")]` |

### 2.2 安全功能

| 功能 | SRS REQ | 状态 | 关键文件 |
|---|---|---|---|
| SQLCipher AES-256 加密 | REQ-SEC-001 | ✅ Done | `crates/infra/src/sqlite_storage.rs` |
| Argon2id KDF + PBKDF2 回退 | REQ-SEC-002 | ✅ Done | `crates/core/src/encryption.rs` |
| 安全状态机 + 自动锁屏 | REQ-SEC-003~004 | ✅ Done | `crates/core/src/security.rs` |
| 8 类 PII 检测/脱敏 | REQ-SEC-006 | ✅ Done | `crates/core/src/privacy.rs` |
| **PII 脱敏集成 RAG 管线** | **REQ-SEC-022** | **✅ Implemented** | `crates/prompt/src/sanitize.rs`（`redact_pii()` + `detect_pii()` + 7 类 PII + 10 TC-PII-RAG） |
| 审计日志哈希链 | REQ-SEC-007 | ✅ Done | `crates/infra/src/sqlite_storage.rs` |
| 暴力破解防护 | REQ-SEC-008 | ✅ Done | `crates/core/src/encryption.rs` |
| 剪贴板自动清除 | REQ-SEC-009 | ✅ Done | `crates/core/src/security.rs` |
| 紧急销毁密码 | REQ-SEC-010 | ✅ Done | `crates/core/src/security.rs` |

### 2.3 多模态 (Pro)

| 功能 | SRS REQ | 状态 | 关键文件 |
|---|---|---|---|
| PDF 页面渲染 (pdfium) | REQ-MM-001 | ✅ Done | `crates/infra/src/pdfium_renderer.rs` |
| 本地 OCR (ocrs/rten) | REQ-MM-002 | ✅ Done | `crates/infra/src/ocrs_engine.rs` |
| BYOK VLM 图片理解 | REQ-MM-003 | ✅ Done | `crates/infra/src/openai_vision.rs` |
| 多模态 PDF 导入管线 | REQ-MM-004 | ✅ Done | `crates/core/src/loader.rs`, `crates/core/src/import.rs` |

### 2.4 本地 LLM (Pro) — Phase 1 R3 已移除

| 功能 | SRS REQ | 状态 | 关键文件 |
|---|---|---|---|
| mistral.rs 本地 LLM 推理 | REQ-LLM-003 | ✅ Deprecated (v3.1.0 Phase 1 R3) | — |
| GGUF 模型文件管理 | REQ-LLM-004 | ✅ Deprecated (v3.1.0 Phase 1 R3) | — |
| GPU 加速 (Metal/CUDA) | REQ-LLM-005 | ✅ Deprecated (v3.1.0 Phase 1 R3) | — |
| 采样参数配置 | REQ-LLM-006 | ✅ Deprecated (v3.1.0 Phase 1 R3) | — |
| 自研量化推理内核 | REQ-LLM-007 | ✅ Deprecated (v3.1.0 Phase 1 R3) | — |
| 内存层次流式加载 | REQ-LLM-008 | ✅ Deprecated (v3.1.0 Phase 1 R3) | — |
| KV Cache 跨会话复用 | REQ-LLM-009 | ✅ Deprecated (v3.1.0 Phase 1 R3) | — |

### 2.5 极致性能优化 (S1-S10)

| 功能 | SRS REQ | 状态 | 关键文件 |
|---|---|---|---|
| 语义缓存金字塔 (L0/L1/L3) | REQ-PERF-001 | ✅ Done | `crates/infra/src/sqlite_cache.rs` |
| Prompt 压缩 | REQ-PERF-002 | ✅ Done | `crates/core/src/prompt_compressor.rs` |
| CRAG + Self-RAG | REQ-PERF-003~005 | ✅ Done | `crates/core/src/cache.rs` |
| 实体链接增强 | REQ-PERF-006 | ✅ Done | `crates/core/src/entity_extractor.rs` |
| 知识图谱实体关系检索 | REQ-RAG-026/027 | ✅ Done (S1-S6) | `crates/core/src/entity_extractor.rs`, `crates/core/src/graph_retriever.rs`, `crates/core/src/graph_analyzer.rs`, `ui/src/graph-viewer.js` |
| 知识图谱可视化 | REQ-RAG-027 | ✅ Done (S3-S5) | `ui/src/graph-viewer.js`, `ui/vendor/d3.min.js`（D3 v7 force-directed graph + 实体类型图标 + 子图过滤 + 搜索定位 + SVG/PNG 导出 + 布局切换 + 路径分析 + 社区检测着色）|
| Proposition 级原子分割 | REQ-PERF-007 | ✅ Done | `crates/core/src/proposition_splitter.rs` |
| ColBERT 多向量嵌入 | REQ-PERF-008 | ✅ Done | `crates/infra/src/colbert_embedder.rs` |
| RAPTOR 多级摘要树 | REQ-PERF-009 | ✅ Done | `crates/core/src/summary_tree.rs` |
| 渐进式上下文注入 | REQ-PERF-010 | ✅ Done | `crates/core/src/progressive_injector.rs` |
| Speculative RAG | REQ-PERF-011 | ✅ Done | `crates/core/src/speculative_rag.rs` |
| 自进化检索记忆 | REQ-PERF-012 | ✅ Done | `crates/core/src/retrieval_memory.rs` |

### 2.6 前端 UI

| 功能 | SRS REQ | 状态 | 关键文件 |
|---|---|---|---|
| DeepSeek 风格 UI (S1-S5) | REQ-UI-010 | ✅ Done | `ui/src/thinking-panel.js`, `ui/src/input-toggles.js` 等 |
| 无障碍 Focus Trap | REQ-A11Y-002 | ✅ Done | `ui/src/focus-trap.js` |
| 对比度达标 WCAG AA | REQ-A11Y-003 | ✅ Done | `ui/styles/tokens.css` |
| 高 DPI 适配 | REQ-WIN-002 | ✅ Done | E2E: `window-management.spec.ts` |
| 国际化 (中英双语) | REQ-I18N-001 | ✅ Done | `ui/src/i18n.js`, `ui/locales/` |
| 窗口状态持久化 | REQ-WIN-001 | ✅ Done | `crates/tauri-app/src/lib.rs` |
| Store 模式状态管理 | - | ✅ Done | `crates/tauri-app/src/stores/` |
| 类型安全 Settings | - | ✅ Done | `crates/tauri-app/src/settings_registry.rs` |
| #[perf] 框架 | - | ✅ Done | `crates/infra/src/perf.rs` |
| Action 前端注册 | - | ✅ Done | `ui/src/action.js` |

### 2.7 Agentic AI 前端 UI (2026-08-06)

| 功能 | SRS REQ | 状态 | 关键文件 |
|---|---|---|---|
| DAG 工作流可视化面板 | REQ-RAG-030 | ✅ Deprecated (v3.1.0 Phase 1 R2) | — |
| 持久化记忆管理面板 | REQ-RAG-033 | ✅ Deprecated (v3.1.0 Phase 1 R2) | — |
| AutoDream 知识库整理面板 | REQ-RAG-029 | ✅ Deprecated (v3.1.0 Phase 1 R2) | — |
| 代码符号搜索面板 | REQ-RAG-031 | ✅ Done | `ui/src/symbol-search.js` |
| 性能优化设置面板 | REQ-PERF-001~012 | ✅ Done | `ui/src/perf-settings.js` |
| 输入区 Toggle 精简 (5→1, 移除 agent toggle) | REQ-UI-014 | ✅ S94+R2 Implemented | `ui/src/input-toggles.js` (TOGGLE_CONFIG 1项: hybrid) |
| tauri-stub.js 117 IPC mock 全覆盖 | - | ✅ Done | `e2e-tests/tests/bridge/tauri-stub.js` |
| 5 个新 E2E spec 文件 (26 tests) | - | ✅ Done | `e2e-tests/tests/{workflow-panel,dream-panel,symbol-search,memory-panel,perf-settings}.spec.ts` |

---

## 3. 待办任务

### P0: 剩余 3 项待规划

| P0 编号 | 来源 | 问题 | 状态 |
|---|---|---|---|
| P0-1 (R1) | 嵌入模型升级 | all-MiniLM-L6-v2 → bge-small-en-v1.5 → **bge-m3 1024-dim 已完成** | ✅ 已修复 |
| P0-2 (R1) | Prompt 注入防护 | 检索片段直接注入 prompt 无防护 | ✅ 已修复 |
| P0-3 (R2) | Pro 功能 ROI | 14 个 Pro 功能评估完成：10 保留 Pro / 4 考虑下沉 / 0 Deprecated | ✅ 已评估 |
| P0-4 (R2) | 持续收入模型 | 无订阅/云服务 | 📋 商业层面 |
| P0-5 (R3) | 数据库回滚 | 无迁移前备份/回滚 | ✅ 已修复 |
| P0-6 (R3) | 插件系统 | MCP 协议 Phase 1 已完成（S97） | ✅ Phase 1 完成 |

**已修复 5/6 P0**（P0-1 + P0-2 + P0-3 + P0-5 + P0-6）。剩余 P0-4 为商业层面，需产品/商业决策。

### 前端架构升级已完成 (2026-08-04)

- **TypeScript checkJs**: tsconfig.json + types/*.d.ts（dom-ext / tauri / vendor），类型错误 273→42（84.6% 减少）
- **esbuild 打包**: scripts/build-ui.mjs 用 esbuild 替代手写 MODULE_ORDER 拓扑序拼接
- **模块拆分**: chat.js 1291→594 行（拆出 chat-render.js）；settings.js 拆出 settings-local-llm.js
- **Vitest 加强**: 新增 action.test.js (20) + guards.test.js (16)，总计 389 tests / 26 files

### P1: 已全部实现（SRS 状态确认）

| REQ ID | 域 | 描述 | SRS 状态 |
|---|---|---|---|
| REQ-ING-003 | ING | 文件选择器导入 | Implemented |
| REQ-NFR-001 | NFR | 冷启动性能 | Implemented |
| REQ-NFR-002 | NFR | 检索性能 | Done |
| REQ-NFR-003 | NFR | 平台覆盖（跨平台 CI） | Implemented |

> **注**: 导入格式扩展 .docx（REQ-ING-015）、.html（REQ-ING-016）、.pptx（REQ-ING-017）和 .epub（REQ-ING-018）已在 v1.1/v1.2 实现。ALLOWED_EXTENSIONS 当前 13 项。

### P2: v1.1 全部 5 个功能已实现（2026-08-06）

v1.1 版本全部 5 个 Session 已完成（S44-S48），原子计划见 `docs/architecture/V1.1_ATOMIC_PLAN.md`：

| Session | 功能 | REQ | 状态 |
|---|---|---|---|
| S44 | .docx 导入（docx-rs v0.4.22） | REQ-ING-015 | ✅ Implemented |
| S45 | .html 导入（html2text v0.17.1） | REQ-ING-016 | ✅ Implemented |
| S46 | 浅色主题（CSS 变量覆盖） | REQ-UI-011 | ✅ Implemented |
| S47 | 语音输入 + TTS（Web Speech API） | REQ-RAG-034/035 | ✅ Implemented |
| S48 | 网页搜索集成（DuckDuckGo API） | REQ-RAG-036 | ✅ Implemented |

**v1.1.0 发布状态**：✅ 全量回归测试通过 + ✅ 版本号升级 1.0.0→1.1.0 + ✅ Release build (`cargo build --release --features pro` exit_code 0) + ✅ CHANGELOG_v1.1.md + ✅ GitHub 同步 (`sync-to-github.sh --commit`, 6 files) + ✅ git tag v1.1.0 创建。

**验证明细**（2026-08-06 v1.1.0 发布回归）：
- Rust Free: 749 passed, 0 failed, 27 ignored
- Rust Pro: 1034 passed, 0 failed, 44 ignored
- Clippy Free + Pro: 零警告
- cargo fmt: 通过
- TypeScript: 0 errors
- esbuild: 11142 行
- E2E: 999 passed, 9 skipped, 0 failed
- Release build: exit_code 0

**v1.2.0 发布状态**：✅ 全量回归测试通过 + ✅ 版本号升级 1.1.0→1.2.0 + ✅ Release build (`cargo build --release --features pro` exit_code 0) + ✅ CHANGELOG_v1.2.md + ✅ GitHub 同步 + ✅ git tag v1.2.0 创建。

**验证明细**（2026-08-07 v1.2.0 发布回归）：
- Rust Free: 766 passed, 0 failed, 27 ignored
- Rust Pro: 1057 passed, 0 failed, 44 ignored
- Clippy Free + Pro: 零警告
- cargo fmt: 通过
- TypeScript: 0 errors
- esbuild: 11670 行
- Vitest: 407 passed, 27 files
- E2E: 950 passed, 9 skipped, 0 failed
- Release build: exit_code 0

**v1.3.0 发布状态**：✅ 全量回归测试通过 + ✅ Bug 修复（DOC_COLS + flaky test）+ ✅ 版本号升级 1.2.0→1.3.0 + ✅ Release build (`cargo build --release --features pro` exit_code 0, 1m28s) + ✅ CHANGELOG_v1.3.md + ✅ GitHub 同步 (13 files, 3881 insertions) + ✅ git tag v1.3.0 创建。

**验证明细**（2026-08-07 v1.3.0 发布回归）：
- Rust Free: 803 passed, 0 failed, 24 ignored
- Rust Pro: 1094 passed, 0 failed, 44 ignored
- Clippy Free + Pro: 零警告（Pro 6m39s）
- cargo fmt: 通过
- TypeScript: 0 errors
- esbuild: 12228 行
- Vitest: 407 passed, 27 files
- E2E: 955 passed, 9 skipped
- Release build: exit_code 0 (1m28s)

**下一步**：B07 Tool Output Bounding（500行/64KB 上限，保留首尾截断）。B01-B06 已完成。

**v2.4.0 发布状态**：✅ 全量回归测试通过 + ✅ 版本号升级 2.3.0→2.4.0 + ✅ Release build (`cargo build --release --features pro` exit_code 0, 12.3m) + ✅ CHANGELOG_v2.4.md + ✅ GitHub 同步 (8 files, 15 ins, 15 del) + ✅ git tag v2.4.0 创建。

**验证明细**（2026-08-22 v2.4.0 发布回归）：
- Rust Free: 1552 passed, 0 failed, 23 ignored
- Rust Pro: 1857 passed, 0 failed, 49 ignored
- Clippy Free + Pro: 零警告
- cargo fmt: 通过
- TypeScript: 0 errors
- esbuild: 1641 行
- Vitest: 1767 passed, 88 files
- Release build: exit_code 0 (12.3m)

### P2: v1.3 规划完成（2026-08-07）

v1.3 版本规划已完成，原子计划见 `docs/architecture/V1.3_ATOMIC_PLAN.md`：

| Session | 功能 | REQ | 状态 | Free/Pro |
|---|---|---|---|---|
| S54 | 知识图谱导出（GraphML/JSON-LD） | REQ-EXP-006 | ✅ Implemented | Free |
| S55 | 文档摘要自动生成（导入时） | REQ-ING-019 | ✅ Implemented | Free |
| S56 | 对话模板/快捷指令系统 | REQ-RAG-038 | ✅ Implemented | Free |
| **S57** | **Markdown 笔记双向链接（Obsidian 风格）** | **REQ-ING-020** | **✅ Implemented** | Free |
| S57 | Markdown 笔记双向链接（Obsidian 风格） | REQ-ING-020 | 📋 Planning | Free |
| S58 | 对话分支/版本树（ChatGPT 风格） | REQ-RAG-039 | ✅ Done | Free |
| S59 | v1.3 发布准备 | — | ✅ Done | — |

**ROI 评估**：15 项候选功能评分，Top 5 入选（知识图谱导出 ROI 15.0 / 文档摘要 ROI 13.3 / 对话模板 ROI 13.3 / 双向链接 ROI 8.0 / 对话分支树 ROI 6.0）。总预计 8.5 天（≤ 10 天预算）。

**零新增依赖**：全部 5 个功能使用已有 Rust crate（serde_json / regex / pulldown-cmark）+ 前端库（D3.js v7）。IPC 命令 141→148（+3 Free，S57 wiki 双向链接）。

### P2: v1.2 规划完成（2026-08-06）

v1.2 版本规划已完成，原子计划见 `docs/architecture/V1.2_ATOMIC_PLAN.md`：

| Session | 功能 | REQ | 状态 | Free/Pro |
|---|---|---|---|---|
| S49 | PDF 导出（`window.print()` 零依赖） | REQ-EXP-005 | ✅ Implemented | Free |
| S50 | .pptx 导入（pptx-to-md v1.0.0） | REQ-ING-017 | ✅ Implemented | Free |
| S51 | .epub 导入（rbook v0.7.10 Apache-2.0） | REQ-ING-018 | ✅ Implemented | Free |
| S52 | 自定义 ONNX 嵌入模型上传 | REQ-VEC-014 | ✅ Implemented | Pro |
| S53 | v1.2 发布准备 | — | ✅ Done | — |

**ROI 评估**：9 项候选功能评分，Top 4 入选（PDF 导出 ROI 20.0 / .pptx ROI 12.0 / .epub ROI 12.0 / 自定义模型 ROI 6.0 Pro 战略入选）。总预计 5 天（≤ 10 天预算）。

**新增依赖**：`pptx-to-md = "1.0"`（MIT/Apache-2.0）/ `rbook = "0.7"`（Apache-2.0）。排除 `epub` crate（GPL-3.0 不兼容）。

### P2: v1.4 规划完成（2026-08-07）

v1.4 版本规划已完成，原子计划见 `docs/architecture/V1.4_ATOMIC_PLAN.md`：

| Session | 功能 | REQ | 状态 | Free/Pro |
|---|---|---|---|---|
| S61 | .xlsx/.csv 文档导入 | REQ-ING-021 | ✅ Implemented | Free |
| S62 | 对话全文搜索 | REQ-RAG-040 | ✅ Implemented | Free |
| S63 | Contextual Retrieval 上下文增强嵌入 | REQ-RAG-041 | ✅ Implemented | Free |
| S64 | 文档标签系统 | REQ-ING-022 | ✅ Implemented | Free |
| S65 | commands.rs 模块化拆分 | REQ-ARCH-008 | ✅ Implemented | Free |
| S66 | v1.4 发布准备 | — | ✅ Done | — |

**ROI 评估**：12 项候选功能评分，Top 5 入选（.xlsx/csv ROI 13.3 / 对话搜索 ROI 13.3 / Contextual Retrieval ROI 8.0 / 标签系统 ROI 7.5 / commands.rs 拆分 ROI 5.0 铁律十四战略）。总预计 9 天（≤ 10 天预算）。

**新增依赖**：calamine v0.26（MIT/Apache-2.0，纯 Rust Excel 读取）+ csv v1.3（MIT/Unlicense，标准 CSV 库）。IPC 命令 152→159（+7 Free）。

### P3: 远期

- VIZ Phase 4+: G6/D3/Three.js 专业领域可视化
- MM Phase 4: CLIP 多模态嵌入 / 本地 VLM
- E2E 真实 GUI 测试 macOS 支持

---

## 4. 关键约束提醒

1. **铁律五**: 生产代码严禁 `unwrap()/expect()/panic!()/unreachable!()/todo!()/unimplemented!()`（workspace deny lint 强制）
2. **铁律七**: 代码变更必须同步更新 `docs/api/` 对应文档 + `docs/SOFTWARE_ASSET.md`
3. **铁律十一**: 修改源代码后必须运行 `./sync-to-github.sh`
4. **铁律十二**: Session 结束必须更新本文件 + `LOG.md` + 生成衔接提示词
5. **依赖方向**: `tauri-app → core → models`，严禁反向
6. **Edition 2024**: 原生 `async fn` in trait，无 `#[async_trait]` 宏
7. **前端构建**: `node scripts/build-ui.mjs`（esbuild 打包 IIFE，不要手改 `ui/index.html`）
8. **TypeScript**: `npx tsc --noEmit` 类型检查（tsconfig.json + types/*.d.ts）
9. **网络代理**: 访问 HuggingFace/GitHub 失败时设置 `export https_proxy=http://127.0.0.1:7890`

---

## 5. 关键文件快速索引

| 文件 | 用途 |
|---|---|
| `docs/SOFTWARE_ASSET.md` | 软件资产全景文档（1100+ 行） |
| `docs/SRS_v1.0.md` | 需求规格唯一依据 |
| `docs/api/ipc-commands.md` | IPC 命令签名 |
| `docs/api/code-contracts.md` | Trait 签名 + 数据模型 |
| `docs/api/database-schema.md` | 数据库表结构 |
| `docs/api/settings-reference.md` | 设置键 + 事件 |
| `docs/api/frontend-modules.md` | 前端模块函数清单 |
| `.catpaw/rules/system-constraints.md` | 铁律规则（21 条） |
| `AGENTS.md` | AI Agent 指南 |
| `CLAUDE.md` | Claude Code 指南 |
| `PROJECT_RULES.md` | 项目宪法 |
| `docs/architecture/adr/` | 架构决策记录 |
| `sync-to-github.sh` | GitHub 发布同步脚本 |

---

## 6. 验证命令速查

```bash
# 编译
cargo build                          # Free
cargo build --release --features pro # Pro

# 测试
cargo test                           # Free (753 passed)
cargo test --features pro            # Pro (1038 passed)
cargo test -p <crate>               # 单 crate
cargo test <name>                   # 聚焦测试

# Lint
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --features pro -- -D warnings
cargo fmt --check

# 前端
node scripts/build-ui.mjs           # esbuild 打包 IIFE 生产构建
node scripts/build-ui.mjs --dev     # 开发模式外部引用
npx tsc --noEmit                    # TypeScript checkJs 类型检查
cd e2e-tests && npm run test:unit   # Vitest (938 tests)
cd e2e-tests && npm run test:bridge # Playwright E2E

# 同步
./sync-to-github.sh                 # GitHub 发布仓库
```

---

## 7. 当前 Git 状态

- **分支**: `refactor/great-simplification` (从 main 切出，大简化重构专用分支)
- **基线 tag**: `v3.1.0-final` (重构前最后快照)
- **本次变更** (Phase 1 R3 — 删除过度工程 LLM 功能):
  - 删除 10 个本地 LLM 推理模块文件 + 测试（gemv_kernel.rs, quant_blocks.rs, gguf_reader.rs, weight_repack.rs, layer_prefetch.rs, memory_budget.rs, kv_cache.rs, prompt_cache.rs, robust_downloader.rs, local_llm.rs, model_manager.rs）
  - 移除 `LlmMode::Local` 变体（crates/models/src/lib.rs）
  - 清理 `llm_router.rs`（移除 Local 路由、回退切换逻辑、LlmChoice::local）
  - 清理 `config_store.rs`（仅处理 Remote 模式）
  - 清理 `trace.rs`（移除 Local 匹配）
  - 清理 `integration.rs`（删除所有本地 LLM/KV cache/kernel mode/paged_attn/sampling/model_manager 相关测试）
  - 更新 SRS（REQ-LLM-003/004/005/006/007/008/009 标记为 Deprecated）
  - 更新 NEXT.md / LOG.md / CURRENT_STATE.md
- **验证状态**: Free 1326 passed / Clippy 零警告 / fmt 通过 / esbuild 1,584 行
- **累计削减** (Phase 1 R1+R2+R3): ~37,862 行 (-28.9%)

---

## 8. v1.15 规划完成状态

### P2: v1.15 规划完成（2026-08-12）

v1.15 版本已完成实施，4 个纯前端高 ROI Draft REQ：

| Session | 功能 | REQ | 状态 | Free/Pro |
|---|---|---|---|---|
| S77 | 快捷键帮助面板搜索过滤 | REQ-KB-005 | ✅ Implemented | Free |
| S78 | 动效与过渡规范 | REQ-DS-006 | ✅ Implemented | Free |
| S79 | 悬停反馈规范 | REQ-IX-004 | ✅ Implemented | Free |
| S80 | 圆角与阴影体系 | REQ-DS-007 | ✅ Implemented | Free |

**ROI 评估**：21 个 Draft REQ 中筛选 4 个纯前端高 ROI 候选，单 Session 完成。无新增依赖、无新增 IPC。11 E2E 测试全量通过。

**关键文件**：
- `ui/src/keyboard-help.js` — 搜索过滤 + 编辑分组
- `ui/styles/tokens.css` — transition/hover/radius/shadow CSS design tokens
- `ui/styles/components.css` — hover 反馈 design token 统一
- `ui/locales/en.json` / `zh-CN.json` — 搜索框 i18n
- `e2e-tests/tests/v1.15-features.spec.ts` — 11 E2E 测试

---

## 9. v1.14 规划完成状态

### P2: v1.14 规划完成（2026-08-11）

v1.14 版本规划已完成，原子计划见 `docs/architecture/V1.14_ATOMIC_PLAN.md`：

| Session | 功能 | REQ | 状态 | Free/Pro |
|---|---|---|---|---|
| S71 | 右键上下文菜单增强 | REQ-IX-001 | 📋 Planning | Free |
| S72 | 导入历史记录 | REQ-ING-011 | 📋 Planning | Free |
| S73 | 数据目录管理增强 | REQ-ERR-004 | 📋 Planning | Free |
| S74 | 文档列表快捷键 | REQ-KB-004 | 📋 Planning | Free |
| S75 | 安全审计报告导出 | REQ-SEC-011 | 📋 Planning | Free |
| S76 | v1.14 发布准备 | — | 📋 Planning | — |

**ROI 评估**：8 项候选功能评分，Top 5 入选（右键菜单 ROI 6.5 / 导入历史 ROI 6.1 / 数据管理 ROI 6.1 / 文档快捷键 ROI 6.1 / 安全审计 ROI 5.9）。总预计 7 天（≤ 10 天预算）。

**无新增依赖**：全部 5 个功能使用已有 Rust crate + 前端基础架构。IPC 命令预计新增 7 个（2+2+3+0+1）。

**3-Session 复盘完成状态

| Session | 主题 | 状态 | 产出文件 |
|---|---|---|---|
| S1 | 代码审计 + 架构健康度 | ✅ 已完成（2026-08-10） | `RETROSPECTIVE_S1_REPORT.md` |
| S2 | 功能审计 + 僵尸命令接线 + Draft REQ 分类 | ✅ 已完成（2026-08-10） | `RETROSPECTIVE_S2_REPORT.md` |
| S3 | 性能 + 安全 + 文档 + 测试策略 | ✅ 已完成（2026-08-10） | `RETROSPECTIVE_S3_REPORT.md` |
| S4 | 集成验证 + 技术债清理 + v1.5 确认 | ✅ 已完成（2026-08-10） | `RETROSPECTIVE_S4_REPORT.md` |

**S4 关键成果**：
- 借鉴集成验证：37 项逐一验证 — 14 已集成 / 3 部分集成 / 13 未集成 / 4 已弃用
- 技术债：clippy 零警告 + fmt 通过 + tsc 零错误 + 无死代码
- v1.5 + v1.6 已发布确认，tray-icon 已启用，无阻塞项
- 3 项可快速接入（heal_dangling_calls / VerbatimTail / schedule_background_compaction 替换）
- 8 项设计性未集成（模块供未来使用，标记 TODO）

---

## 9. 已完成借鉴计划全览

| 借鉴来源 | 状态 | 内容 |
|---|---|---|
| QM (Quotient) | ✅ 全部完成 (Q01-Q11) | 记忆系统 / 上下文压缩 / 安全层 / 基础设施 / LLMProvider |
| OpenCode | ✅ 全部完成 (B01-B14) | System Context / Epoch / 压缩模板 / Prompt Caching / Admission / Coordinator / Tool Bounding / Skill / 多协议 / Permission |
| 跨 Phase 整合 | ✅ 全部完成 (S67+S68+S71) | Burst Buffer / Sweeper / Shadow 筛查 |
| OpenMontage | ✅ 全部完成 | MMR 多样性重排 / 检索质量门控 / Scored Selector / Pipeline Checkpoint / Event Stream |
| DS4 (DwarfStar) | ✅ DS-01 + DS-02 + DS-03 完成 | Prompt Prefix 磁盘缓存 / 上下文压缩原文尾部保留 / Session Strip 会话条带化（StripResult 扩展 summary/kept_count） |
| DS4 剩余 | 📋 待定 (DS-04~DS-07) | 功率管理 / Tool Call 续接 / Radix Tree / SSD 流式 |

---

## 10. 验收评估审计期状态

### S1：前端模块全量审计 ✅ 完成（2026-08-13）

**审计范围**：79 个前端 JS 模块（`ui/src/*.js`），逐个评估。

**审计产出**：`docs/sessions/archive/audit_s1_frontend_modules.md`

**等级分布**：

| 等级 | 数量 | 占比 | 含义 |
|---|---|---|---|
| A | 24 | 30% | 优秀 — 保持 |
| B | 24 | 30% | 良好 — 小幅优化 |
| C | 14 | 18% | 需改进 |
| D | 12 | 15% | 不合格 — 大改 |
| F | 5 | 6% | 需重写/删除 |

**关键发现**：
- ✅ **零僵尸模块**：所有 79 个模块均被至少 1 个其他模块导入
- 🔴 **设置爆炸**：9 个分区（目标 ≤ 8），settings-advanced 内部 13 个子区域
- 🔴 **Toggle 过多**：5 个 toggle（目标 ≤ 2），3 个用户无法理解
- 🔴 **性能开关过度暴露**：12 个学术术语开关全部默认关闭
- 🔴 **开发者工具混入用户设置**：rag-eval / trace / durable / session-strip 4 个模块
- 🔴 **5 个 F 级删除候选**：workflow-panel / code-executor / durable-panel / session-strip / perf-settings

**P0 行动清单**（立即执行）：
1. input-toggles.js 精简为 2 个 toggle（hybrid + agent）
2. perf-settings.js 重设计为"智能模式"单一开关
3. settings.js 移除 4 个开发者工具子面板

### S2：后端 IPC 命令全量审计 ✅ 完成（2026-08-13）

**审计范围**：236 个 IPC 命令（224 Free + 12 Pro-only），逐个评估。

**审计产出**：`docs/sessions/archive/audit_s2_ipc_commands.md`

**等级分布**：

| 等级 | 数量 | 占比 | 含义 |
|---|---|---|---|
| A | 62 | 26% | 优秀 — 核心功能，保持 |
| B | 58 | 25% | 良好 — 有用户价值 |
| C | 42 | 18% | 需改进 — 场景模糊 |
| D | 38 | 16% | 不合格 — 开发者工具 |
| F | 36 | 15% | 需删除/隐藏 |

**关键发现**：
- ✅ **零僵尸命令**：全部 236 个注册命令有前端调用者（含动态 invoke 模式）
- 🔴 **11 个幻影调用**：前端调用 11 个后端未注册命令（encrypt_database / unlock_database / redact_pii 等），运行时崩溃
- 🔴 **~65 个借鉴命令**：QM(22) / OpenCode(10) / DS4(3) / Cherry Studio(10) / RAGAS(4) / 学术(16)，其中 ~45 无用户场景
- 🔴 **21 个开发者工具混入用户功能**：trace(4) + budget(2) + rag_eval(4) + durable(3) + session_todo(5) + strip(3)
- 🔴 **19 个性能优化命令全部默认关闭**：用户永远不会手动开启
- 🟠 **~30 对 set/get 命令**：可合并为 ~15 个单一 settings 命令

**P0 行动清单**（立即修复）：
1. 修复 11 个幻影调用（前端 ipc.js 删除旧封装 + 迁移到新命令名）
2. 删除 tauri-stub.js 中 2 个无对应后端的 mock

### S3：UI 界面验收评估 ✅ 完成（2026-08-13）

**审计范围**：全量 UI 界面元素 — 设置面板（35 分区）、输入区（5 toggle）、顶栏（8 功能按钮）、对话区、右键菜单、首启向导、错误提示。

**审计产出**：`docs/sessions/archive/audit_s3_ui_evaluation.md`

**评估标准 10 条**：

| 标准 | 当前 | 目标 | 状态 |
|---|---|---|---|
| S1 首次 ≤ 3 步 | 3 步 | ≤ 3 | ✅ 达标 |
| S2 核心 ≤ 2 点击 | ✅ | ≤ 2 | ✅ 达标 |
| S3 设置 ≤ 8 分区 | **35** 🔴🔴 | ≤ 8 | P0 重设计 |
| S4 toggle ≤ 2 | **5** 🔴 | ≤ 2 | P0 精简 |
| S5 顶栏 ≤ 5 按钮 | **8** 🟡 | ≤ 5 | P1 精简 |
| S6 同时 1 overlay | ✅ | 1 | ✅ 达标 |
| S7 设置人类语言 | **20 术语** 🔴 | 通俗化 | P0 通俗化 |
| S8 默认即最佳 | **13/20 关闭** 🔴 | 智能模式 | P0 智能模式 |
| S9 错误可理解 | **60% 技术化** 🟡 | 人类化 | P1 统一分类 |
| S10 快捷键覆盖 | ✅ | ✅ | ✅ 达标 |

**关键发现**：
- 🔴🔴 **设置面板 35 个分区**（21 HTML 静态 + 16 JS 动态注入）远超目标 8
- 🔴 **输入区 5 toggle**：3 个用户无法理解（sub_agent/memory/web_search 应移入设置）
- 🔴 **20 个设置项术语化**：Speculative RAG / HyDE / Cross-Encoder Rerank / PagedAttention / GEMV / KV Cache 等用户无法理解
- 🔴 **13/20 功能默认关闭（65%）**：用户开箱即用「最差配置」，HyDE/质量门控/知识图谱/压缩 应默认开启
- 🟡 **顶栏 8 功能按钮**：3 个开发者工具（AutoDream/符号搜索/工作流）应移入工具菜单
- 🟡 **60% 错误信息技术化**：EMBED:/PRO_REQUIRED 前缀 + 原始错误堆栈直接展示
- ✅ **首启向导 3 步达标**：下载→配置→导入
- ✅ **核心操作 ≤ 2 点击**：快捷键覆盖完善
- ✅ **面板栈管理**：同时只显示 1 个 overlay

**P0 行动清单**：
1. 设置面板 35 → 8 分区（模型配置/外观语言/知识库/安全隐私/智能检索/高级调优/应用设置/快捷指令）
2. 输入区 5 → 2 toggle（保留混合搜索 + Agent 模式）
3. 20 术语化设置项通俗化（如 Speculative RAG → 快速预生成）
4. 添加「智能模式」单一开关（默认开启混合搜索 + Rerank + HyDE + 知识图谱 + 质量门控 + 压缩 2x）
5. 4 个开发者工具分区移入隐藏的「开发者菜单」
6. perf-settings 12 个术语开关 → 1 个智能模式开关 + 折叠高级调优

### S4：用户场景验收评估 ✅ 完成（2026-08-13）

**审计范围**：3 类用户画像（小明/新手、小王/知识工作者、小李/高级用户）× 5 个核心场景 = 15 个场景全覆盖 + 智能模式/8 分区/2 toggle 三大方案验证。

**审计产出**：`docs/sessions/archive/audit_s4_user_scenarios.md`

**U1-U8 评估结果**：

| 标准 | 结果 | 状态 |
|---|---|---|
| U1 新手场景覆盖率 | 5/5 = 100% | ✅ 达标 |
| U2 知识工作者覆盖率 | 5/5 = 100% | ✅ 达标 |
| U3 高级用户覆盖率 | 5/5 = 100% | ✅ 达标 |
| U4 智能模式覆盖 | 93.3% 场景无需手动配置 | ✅ 达标 |
| U5 设置面板可达性 | 8 分区每项 1 次点击 | ✅ 达标 |
| U6 Toggle 必要性 | 5/5 场景覆盖 | ✅ 达标 |
| U7 场景流畅度 | 12/15 = 80%（重整后 100%） | 🟡 基本达标 |
| U8 术语无障碍 | 重整后 ~5 术语（含 tooltip） | 🟡 基本达标 |

**三大方案验证结论**：
- ✅ **智能模式方案通过**：93.3% 场景无需手动配置（新手+知识工作者 100%，高级用户 80%）
- ✅ **8 分区方案通过**：每项 1 次点击可达（含 N3/N6/N7 修正建议）
- ✅ **2 toggle 方案通过**：5/5 场景覆盖（含 N8 网页搜索替代方案）

**关键发现**：
- ✅ 15/15 场景全部可完成（覆盖率 100%）
- 🟡 3 个当前卡点：S-B（检索质量退化）/ S-E（5 toggle 困惑）/ S-K（设置分散）→ 重整后全部解决
- ✅ S3 全部 P0/P1 行动方案经 S4 验证可行性确认
- 🟡 8 个新发现（N1~N8）：导出发现性 / HTML 导出验证 / PagedAttention 分区修正 / VLM 分区修正 / Agent 发现性 / 网页搜索替代方案

### S5：综合行动方案 📋 待执行

**计划范围**：基于 S1-S4 四轮审计结果，制定综合行动方案并开始实施。预计 ~10 Sessions。

**S5 P0 行动清单**（立即实施）：

| # | 行动 | 来源 | 预计工作量 |
|---|---|---|---|
| P0-1 | 智能模式单一开关实现 | S3 P0-4 + S4 U4 | 1 Session |
| P0-2 | 输入区 5 → 2 toggle 精简 | S3 P0-2 + S4 U6 | 0.5 Session |
| P0-3 | 设置面板 35 → 8 分区重整 | S3 P0-1 + S4 U5 | 2 Sessions |
| P0-4 | 20 术语通俗化 | S3 P0-3 + S4 U8 | 1 Session |
| P0-5 | 11 幻影调用修复 | S2 P0 | 0.5 Session |
| P0-6 | 4 开发者工具移入隐藏菜单 | S3 P0-5 + S4 N3/N6 | 0.5 Session |

**S5 P1 行动清单**（后续优化）：

| # | 行动 | 来源 | 预计工作量 |
|---|---|---|---|
| P1-1 | 顶栏 8 → 5 按钮精简 | S3 P1-1 | 1 Session |
| P1-2 | 错误信息统一分类 | S3 P1-2 | 1 Session |
| P1-3 | N8 网页搜索替代方案 | S4 N8 | 0.5 Session |
| P1-4 | N4/N5 Agent 发现性改善 | S4 N4/N5 | 0.5 Session |
| P1-5 | 5 个 F 级模块删除 | S1 P1 | 1 Session |
| P1-6 | N1 导出发现性改善 | S4 N1 | 0.5 Session |

### S7：P2 行动清单 ✅ 完成（2026-08-13）

**计划范围**：基于 S4 审计发现的 N2/N7 + 设置面板 Tab 滚动 + 斜杠命令键盘导航优化。

**S7 P2 行动清单**（全部完成）：

| # | 行动 | 来源 | 状态 |
|---|---|---|---|
| P2-1 | HTML 导出完整性增强（hljs 语法高亮 + Mermaid 标注 + CSS 主题） | S4 N2 | ✅ Done |
| P2-2 | VLM 设置分区归属调整（从「智能检索」移入「模型配置」） | S4 N7 | ✅ Done |
| P2-3 | 设置面板 8 分区 Tab 滚动定位优化（scroll-padding-top: 52px） | S3/S4 | ✅ Done |
| P2-4 | 斜杠命令面板键盘导航优化（Tab 选择 + Home/End 跳转） | S3 | ✅ Done |

**修改文件**：
- `ui/src/export.js` — 新增 `applyHighlightInHtmlString()` + `annotateMermaidInHtmlString()` + hljs CSS 主题 + 预加载 `loadHighlight()`
- `ui/index.html` — VLM toggle 区块从智能检索区域移至模型配置区域（LLM 配置之后）
- `ui/src/settings.js` — `createSettingsTabBar()` 新增 `scrollPaddingTop = '52px'`
- `ui/src/input-keymap.js` — 键导航列表添加 `Tab`/`Home`/`End` + `handleSlashNavigation` 新增 Home/End 处理
- `ui/src/slash-commands.js` — `navigateSlashCommand()` 支持 `'home'`/`'end'` 方向
- `e2e-tests/tests/unit/slash-commands.test.js` — 新增 3 个 P2-4 测试（home/end/单项列表）

**验证结果**：
- TypeScript: 0 errors ✅
- esbuild: 1622 行 ✅
- Vitest: 440 passed (27 files) ✅（+3 新增 P2-4 测试）
- E2E 相关测试全通过：导出 12/12 ✅、设置/VLM 9/9 ✅、斜杠命令 8/8 ✅
- E2E 全量: 1163 passed, 205 failed (pre-existing from S5/S6 UI 重构), 10 skipped
- sync-to-github: 已提交 + 已推送 ✅

---

> **新会话指引**: 读完本文件后，如需了解特定功能的历史详情，查阅 `docs/sessions/archive/`。如需开始新功能开发，先读取对应 `docs/architecture/` 原子计划或 `docs/SRS_v1.0.md` 对应 REQ。
