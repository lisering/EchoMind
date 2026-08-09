# EchoMind 当前项目状态快照

> **本文件是任何新 AI 会话开始时的第一份必读文件。**
>
> 每次 session 结束时**必须更新**此文件，确保反映最新状态。
>
> **最后更新**: 2026-08-09 | **更新人**: AI Agent (S3 验证收尾 + 前端架构升级)

---

## 1. 项目健康度

| 指标 | 数值 | 备注 |
|---|---|---|
| Rust 单元+集成测试 (Free) | 1028 passed, 9 failed (pre-existing), 20 ignored | `cargo test`（9 失败全为许可证/Pro 门控/配额 pre-existing）|
| Rust 单元+集成测试 (Pro) | 1094 passed, 0 failed, 44 ignored | `cargo test --features pro`（v1.3.0 发布回归验证）|
| Clippy (Free) | 零警告 | `cargo clippy --all-targets -- -D warnings` |
| Clippy (Pro) | 零警告 | `cargo clippy --all-targets --features pro -- -D warnings`（6m39s）|
| cargo fmt | 通过 | `cargo fmt --check` |
| 前端单元测试 (Vitest) | 419 passed, 27 files | `npm run test:unit`（修复 input-toggles 3 失败）|
| UI 类型检查 | 0 errors | `npx tsc --noEmit` |
| E2E 桥接测试 (Playwright) | 运行中 | `npm run test:bridge`（Session 1-2 改造后回归验证）|
| 版本号 | 1.4.0 | 全部 7 crate + tauri.conf.json（v1.4.0 发布升级）|
| IPC 命令数 | 162 | B05 新增 3 命令（admit_input + promote_input + get_pending_inputs）|
| ADR 数 | 13 | 新增 ADR-013-knowledge-graph |
| 供应链安全 | cargo audit + cargo deny | `deny.toml` 配置 |
| 前端构建行数 | 13580 | esbuild 打包 IIFE 单文件（Session 1-2 改造后）|
| 前端模块数 | 54 | `ui/src/*.js`（新增 zindex/panel-stack/input-keymap/input-history + focus-trap）|
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
| ReAct Agent 多步推理 | REQ-RAG-022 | ✅ Done | `crates/core/src/agent.rs` |
| 多代理协调引擎 | REQ-RAG-025 | ✅ Done | `crates/core/src/coordinator.rs` |
| 后台空闲整理引擎 | - | ✅ Done | `crates/core/src/auto_dream.rs` |
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

### 2.4 本地 LLM (Pro)

| 功能 | SRS REQ | 状态 | 关键文件 |
|---|---|---|---|
| mistral.rs 本地 LLM 推理 | REQ-LLM-003 | ✅ Done (Phase 1-4) | `crates/infra/src/local_llm.rs` |
| GGUF 模型文件管理 | REQ-LLM-004 | ✅ Done | `crates/infra/src/model_manager.rs` |
| GPU 加速 (Metal/CUDA) | REQ-LLM-005 | ✅ Done (Phase 2) | `crates/infra/src/local_llm.rs` |
| 采样参数配置 | REQ-LLM-006 | ✅ Done (Phase 2) | `crates/infra/src/local_llm.rs` |
| 自研量化推理内核 | REQ-LLM-007 | ✅ Done (Phase 3) | `crates/infra/src/gemv_kernel.rs` 等 6 模块 |
| 内存层次流式加载 | REQ-LLM-008 | ✅ Done (Phase 3) | `crates/infra/src/layer_prefetch.rs` 等 |
| KV Cache 跨会话复用 | REQ-LLM-009 | ✅ Done (Phase 4) | `crates/infra/src/kv_cache.rs` |

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
| DAG 工作流可视化面板 | REQ-RAG-030 | ✅ Done | `ui/src/workflow-panel.js` |
| 持久化记忆管理面板 | REQ-RAG-033 | ✅ Done | `ui/src/memory-panel.js` |
| AutoDream 知识库整理面板 | REQ-RAG-029 | ✅ Done | `ui/src/dream-panel.js` |
| 代码符号搜索面板 | REQ-RAG-031 | ✅ Done | `ui/src/symbol-search.js` |
| 性能优化设置面板 | REQ-PERF-001~012 | ✅ Done | `ui/src/perf-settings.js` |
| 输入区 Toggle 扩展 (sub_agent/memory) | REQ-RAG-030/033 | ✅ Done | `ui/src/input-toggles.js` |
| tauri-stub.js 117 IPC mock 全覆盖 | - | ✅ Done | `e2e-tests/tests/bridge/tauri-stub.js` |
| 5 个新 E2E spec 文件 (26 tests) | - | ✅ Done | `e2e-tests/tests/{workflow-panel,dream-panel,symbol-search,memory-panel,perf-settings}.spec.ts` |

---

## 3. 待办任务

### P0: 无

当前无 P0 级阻塞任务。

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
cd e2e-tests && npm run test:unit   # Vitest (407 tests)
cd e2e-tests && npm run test:bridge # Playwright E2E

# 同步
./sync-to-github.sh                 # GitHub 发布仓库
```

---

## 7. 当前 Git 状态

- **分支**: `main`
- **未提交变更**: 大量 untracked 文件（历史功能开发累积）
- **发布仓库**: `EchoMind/EchoMind/`（通过 `sync-to-github.sh` 同步）

---

> **新会话指引**: 读完本文件后，如需了解特定功能的历史详情，查阅 `docs/sessions/archive/`。如需开始新功能开发，先读取对应 `docs/architecture/` 原子计划或 `docs/SRS_v1.0.md` 对应 REQ。
