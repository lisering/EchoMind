# EchoMind 历史详情归档

> 从 `CURRENT_STATE.md` 拆分出的详细内容，供需要时查阅。
>
> **拆分日期**: 2026-09-06

---

## 1. 已完成功能矩阵

### 1.1 核心功能 (P0)

| 功能 | SRS REQ | 状态 | 关键文件 |
|---|---|---|---|
| 文档导入 (md/txt/pdf/docx/html/pptx/epub/xlsx/csv) | REQ-ING-001~005, REQ-ING-015~018, REQ-ING-021 | ✅ Done | `crates/core/src/import.rs`, `crates/core/src/loader.rs` |
| 文档标签系统 | REQ-ING-022 | ✅ Implemented | `crates/models/src/lib.rs`, `crates/infra/src/sqlite_storage.rs`, `crates/tauri-app/src/commands/document.rs`, `ui/src/main.js` |
| 浅色主题切换 | REQ-UI-011 | ✅ Implemented | `ui/styles/tokens.css`, `ui/src/main.js`, `ui/src/settings.js` |
| 网页搜索集成 | REQ-RAG-036 | ✅ Implemented | `crates/core/src/web_search.rs`, `crates/infra/src/duckduckgo_provider.rs` |
| 语音输入 (SpeechRecognition) | REQ-RAG-034 | ✅ Implemented | `ui/src/voice-input.js`, `ui/index.html` |
| TTS 朗读 (speechSynthesis) | REQ-RAG-035 | ✅ Implemented | `ui/src/voice-input.js`, `ui/src/chat-render.js` |
| Token 窗口分块 | REQ-VEC-001 | ✅ Done | `crates/core/src/splitter.rs` |
| 语义分块 | REQ-VEC-002 | ✅ Done | `crates/core/src/semantic_splitter.rs` |
| 章节感知分块 | REQ-VEC-006 | ✅ Done | `crates/core/src/section_aware_splitter.rs` |
| 本地向量化 (fastembed ONNX) | REQ-VEC-003 | ✅ Done | `crates/infra/src/local_embedder.rs` |
| bge-m3 多语言嵌入模型 (1024-dim) | REQ-VEC-015 | ✅ Implemented | `crates/infra/src/local_embedder.rs`, `crates/infra/src/bge_m3_tests.rs` |
| 全库嵌入重建命令 | REQ-VEC-016 | ✅ Implemented | `crates/tauri-app/src/commands/performance.rs` |
| RAG 评估数据集与端到端检索质量评估 | REQ-RAG-048 | ✅ Implemented | `crates/models/src/lib.rs`, `crates/core/src/rag_eval_dataset.rs` |
| Late Chunking 上下文感知嵌入 | REQ-RAG-049 | ✅ Implemented | `crates/core/src/late_chunking.rs`, `crates/tauri-app/src/commands/import.rs`, `crates/tauri-app/src/commands/performance.rs` |
| 多嵌入模型运行时切换 | REQ-VEC-012 | ✅ Done | `crates/tauri-app/src/stores/model_store.rs` |
| 自定义 ONNX 嵌入模型上传 | REQ-VEC-014 | ✅ Implemented | `crates/infra/src/local_embedder.rs`, `crates/tauri-app/src/commands.rs`, `crates/tauri-app/src/stores/model_store.rs`, `ui/src/settings.js` |
| 领域画像自动分类 | REQ-VEC-013 | ✅ Done | `crates/core/src/domain.rs` |
| SQLite 持久化 (WAL+r2d2) | REQ-DB-001 | ✅ Done | `crates/infra/src/sqlite_storage.rs` |
| 向量检索 (余弦相似度) | REQ-RAG-001~003 | ✅ Done | `crates/core/src/retriever.rs` |
| 混合检索 (向量+BM25 RRF) | REQ-RAG-010 | ✅ Done | `crates/core/src/hybrid_retriever.rs` |
| Cross-Encoder 重排序 | REQ-RAG-020 | ✅ Done | `crates/infra/src/local_reranker.rs` |
| HyDE 查询改写 | REQ-RAG-021 | ✅ Done | `crates/infra/src/hyde_rewriter.rs` |
| RAG 流式对话 (SSE) | REQ-RAG-001~006 | ✅ Done | `crates/core/src/chat.rs` |
| 会话持久化 + 懒创建 | REQ-RAG-006/012 | ✅ Done | `crates/tauri-app/src/commands/conversation.rs` |
| 对话全文搜索 (FTS5) | REQ-RAG-040 | ✅ Implemented | `crates/infra/src/sqlite_storage.rs`, `crates/tauri-app/src/commands/conversation.rs`, `ui/src/sidebar.js` |
| Contextual Retrieval 上下文增强嵌入 | REQ-RAG-041 | ✅ Implemented | `crates/core/src/retriever.rs`, `crates/tauri-app/src/commands/import.rs`, `crates/tauri-app/src/commands/performance.rs`, `ui/src/perf-settings.js` |
| 流中断 (CancellationToken) | REQ-RAG-005 | ✅ Done | `crates/tauri-app/src/stores/chat_store.rs` |
| 消息列表懒加载 | REQ-RAG-019 | ✅ Done | `crates/tauri-app/src/commands.rs` |
| ReAct Agent 多步推理 | REQ-RAG-022 | ✅ Deprecated (v3.1.0) | — |
| 多代理协调引擎 | REQ-RAG-025 | ✅ Deprecated (v3.1.0) | — |
| 后台空闲整理引擎 | - | ✅ Deprecated (v3.1.0) | — |
| 文件监听增量同步 | REQ-SYNC-001~003 | ✅ Done | `crates/core/src/sync.rs`, `crates/infra/src/file_watcher.rs` |
| 对话导出为 Markdown | REQ-EXP-001 | ✅ Done | `crates/core/src/export.rs` |
| PDF 导出 (window.print) | REQ-EXP-005 | ✅ Implemented | `ui/src/export.js`, `ui/styles/print.css` |
| License Ed25519 验证 | REQ-LIC-005 | ✅ Done | `crates/core/src/license.rs`, `crates/license-issuer/` |
| Free/Pro 条件编译 | - | ✅ Done | 全 crate `#[cfg(feature = "pro")]` |

### 1.2 安全功能

| 功能 | SRS REQ | 状态 | 关键文件 |
|---|---|---|---|
| SQLCipher AES-256 加密 | REQ-SEC-001 | ✅ Done | `crates/infra/src/sqlite_storage.rs` |
| Argon2id KDF + PBKDF2 回退 | REQ-SEC-002 | ✅ Done | `crates/core/src/encryption.rs` |
| 安全状态机 + 自动锁屏 | REQ-SEC-003~004 | ✅ Done | `crates/core/src/security.rs` |
| 8 类 PII 检测/脱敏 | REQ-SEC-006 | ✅ Done | `crates/core/src/privacy.rs` |
| PII 脱敏集成 RAG 管线 | REQ-SEC-022 | ✅ Implemented | `crates/prompt/src/sanitize.rs` |
| 审计日志哈希链 | REQ-SEC-007 | ✅ Done | `crates/infra/src/sqlite_storage.rs` |
| 暴力破解防护 | REQ-SEC-008 | ✅ Done | `crates/core/src/encryption.rs` |
| 剪贴板自动清除 | REQ-SEC-009 | ✅ Done | `crates/core/src/security.rs` |
| 紧急销毁密码 | REQ-SEC-010 | ✅ Done | `crates/core/src/security.rs` |

### 1.3 多模态 (Pro)

| 功能 | SRS REQ | 状态 | 关键文件 |
|---|---|---|---|
| PDF 页面渲染 (pdfium) | REQ-MM-001 | ✅ Done | `crates/infra/src/pdfium_renderer.rs` |
| 本地 OCR (ocrs/rten) | REQ-MM-002 | ✅ Done | `crates/infra/src/ocrs_engine.rs` |
| BYOK VLM 图片理解 | REQ-MM-003 | ✅ Done | `crates/infra/src/openai_vision.rs` |
| 多模态 PDF 导入管线 | REQ-MM-004 | ✅ Done | `crates/core/src/loader.rs`, `crates/core/src/import.rs` |

### 1.4 本地 LLM (Pro) — Phase 1 R3 已移除

| 功能 | SRS REQ | 状态 | 关键文件 |
|---|---|---|---|
| mistral.rs 本地 LLM 推理 | REQ-LLM-003 | ✅ Deprecated (v3.1.0) | — |
| GGUF 模型文件管理 | REQ-LLM-004 | ✅ Deprecated (v3.1.0) | SRS 已标注 |
| GPU 加速 (Metal/CUDA) | REQ-LLM-005 | ✅ Deprecated (v3.1.0) | — |
| 采样参数配置 | REQ-LLM-006 | ✅ Deprecated (v3.1.0) | — |
| 自研量化推理内核 | REQ-LLM-007 | ✅ Deprecated (v3.1.0) | — |
| 内存层次流式加载 | REQ-LLM-008 | ✅ Deprecated (v3.1.0) | — |
| KV Cache 跨会话复用 | REQ-LLM-009 | ✅ Deprecated (v3.1.0) | — |

### 1.5 极致性能优化 (S1-S10)

| 功能 | SRS REQ | 状态 | 关键文件 |
|---|---|---|---|
| 语义缓存金字塔 (L0/L1/L3) | REQ-PERF-001 | ✅ Done | `crates/infra/src/sqlite_cache.rs` |
| Prompt 压缩 | REQ-PERF-002 | ✅ Done | `crates/core/src/prompt_compressor.rs` |
| CRAG + Self-RAG | REQ-PERF-003~005 | ✅ Done | `crates/core/src/cache.rs` |
| 实体链接增强 | REQ-PERF-006 | ✅ Done | `crates/core/src/entity_extractor.rs` |
| 知识图谱实体关系检索 | REQ-RAG-026/027 | ✅ Done | `crates/core/src/entity_extractor.rs`, `crates/core/src/graph_retriever.rs`, `crates/core/src/graph_analyzer.rs`, `ui/src/graph-viewer.js` |
| 知识图谱可视化 | REQ-RAG-027 | ✅ Done | `ui/src/graph-viewer.js`, `ui/vendor/d3.min.js` |
| Proposition 级原子分割 | REQ-PERF-007 | ✅ Done | `crates/core/src/proposition_splitter.rs` |
| ColBERT 多向量嵌入 | REQ-PERF-008 | ✅ Done | `crates/infra/src/colbert_embedder.rs` |
| RAPTOR 多级摘要树 | REQ-PERF-009 | ✅ Done | `crates/core/src/summary_tree.rs` |
| 渐进式上下文注入 | REQ-PERF-010 | ✅ Done | `crates/core/src/progressive_injector.rs` |
| Speculative RAG | REQ-PERF-011 | ✅ Done | `crates/core/src/speculative_rag.rs` |
| 自进化检索记忆 | REQ-PERF-012 | ✅ Done | `crates/core/src/retrieval_memory.rs` |

### 1.6 前端 UI

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

### 1.7 Agentic AI 前端 UI (2026-08-06)

| 功能 | SRS REQ | 状态 | 关键文件 |
|---|---|---|---|
| DAG 工作流可视化面板 | REQ-RAG-030 | ✅ Deprecated (v3.1.0) | — |
| 持久化记忆管理面板 | REQ-RAG-033 | ✅ Deprecated (v3.1.0) | — |
| AutoDream 知识库整理面板 | REQ-RAG-029 | ✅ Deprecated (v3.1.0) | — |
| 代码符号搜索面板 | REQ-RAG-031 | ✅ Done | `ui/src/symbol-search.js` |
| 性能优化设置面板 | REQ-PERF-001~012 | ✅ Done | `ui/src/perf-settings.js` |
| 输入区 Toggle 精简 (5→1, 移除 agent toggle) | REQ-UI-014 | ✅ S94+R2 Implemented | `ui/src/input-toggles.js` |
| tauri-stub.js 117 IPC mock 全覆盖 | - | ✅ Done | `e2e-tests/tests/bridge/tauri-stub.js` |
| 5 个新 E2E spec 文件 (26 tests) | - | ✅ Done | `e2e-tests/tests/{workflow-panel,dream-panel,symbol-search,memory-panel,perf-settings}.spec.ts` |

---

## 2. 版本规划详情

### v1.1 (2026-08-06) — 5 功能

| Session | 功能 | REQ | 状态 |
|---|---|---|---|
| S44 | .docx 导入 (docx-rs v0.4.22) | REQ-ING-015 | ✅ Implemented |
| S45 | .html 导入 (html2text v0.17.1) | REQ-ING-016 | ✅ Implemented |
| S46 | 浅色主题 (CSS 变量覆盖) | REQ-UI-011 | ✅ Implemented |
| S47 | 语音输入 + TTS (Web Speech API) | REQ-RAG-034/035 | ✅ Implemented |
| S48 | 网页搜索集成 (DuckDuckGo API) | REQ-RAG-036 | ✅ Implemented |

**验证**: Rust Free 749 passed / Pro 1034 passed / Clippy 零警告 / TypeScript 0 errors / E2E 999 passed / Release build exit_code 0

### v1.2 (2026-08-07) — 4 功能

| Session | 功能 | REQ | 状态 | Free/Pro |
|---|---|---|---|---|
| S49 | PDF 导出 (`window.print()` 零依赖) | REQ-EXP-005 | ✅ Implemented | Free |
| S50 | .pptx 导入 (pptx-to-md v1.0.0) | REQ-ING-017 | ✅ Implemented | Free |
| S51 | .epub 导入 (rbook v0.7.10) | REQ-ING-018 | ✅ Implemented | Free |
| S52 | 自定义 ONNX 嵌入模型上传 | REQ-VEC-014 | ✅ Implemented | Pro |

**新增依赖**: `pptx-to-md = "1.0"` (MIT/Apache-2.0) / `rbook = "0.7"` (Apache-2.0)

### v1.3 (2026-08-07) — 5 功能

| Session | 功能 | REQ | 状态 | Free/Pro |
|---|---|---|---|---|
| S54 | 知识图谱导出 (GraphML/JSON-LD) | REQ-EXP-006 | ✅ Implemented | Free |
| S55 | 文档摘要自动生成 (导入时) | REQ-ING-019 | ✅ Implemented | Free |
| S56 | 对话模板/快捷指令系统 | REQ-RAG-038 | ✅ Implemented | Free |
| S57 | Markdown 笔记双向链接 (Obsidian 风格) | REQ-ING-020 | ✅ Implemented | Free |
| S58 | 对话分支/版本树 (ChatGPT 风格) | REQ-RAG-039 | ✅ Done | Free |

### v1.4 (2026-08-07) — 5 功能

| Session | 功能 | REQ | 状态 | Free/Pro |
|---|---|---|---|---|
| S61 | .xlsx/.csv 文档导入 | REQ-ING-021 | ✅ Implemented | Free |
| S62 | 对话全文搜索 | REQ-RAG-040 | ✅ Implemented | Free |
| S63 | Contextual Retrieval 上下文增强嵌入 | REQ-RAG-041 | ✅ Implemented | Free |
| S64 | 文档标签系统 | REQ-ING-022 | ✅ Implemented | Free |
| S65 | commands.rs 模块化拆分 | REQ-ARCH-008 | ✅ Implemented | Free |

### v1.14 (2026-08-11)

| Session | 功能 | REQ | 状态 | Free/Pro |
|---|---|---|---|---|
| S71 | 右键上下文菜单增强 | REQ-IX-001 | 📋 Planning | Free |
| S72 | 导入历史记录 | REQ-ING-011 | 📋 Planning | Free |
| S73 | 数据目录管理增强 | REQ-ERR-004 | 📋 Planning | Free |
| S74 | 文档列表快捷键 | REQ-KB-004 | 📋 Planning | Free |
| S75 | 安全审计报告导出 | REQ-SEC-011 | 📋 Planning | Free |
| S76 | v1.14 发布准备 | — | 📋 Planning | — |

### v1.15 (2026-08-12) — 4 功能

| Session | 功能 | REQ | 状态 | Free/Pro |
|---|---|---|---|---|
| S77 | 快捷键帮助面板搜索过滤 | REQ-KB-005 | ✅ Implemented | Free |
| S78 | 动效与过渡规范 | REQ-DS-006 | ✅ Implemented | Free |
| S79 | 悬停反馈规范 | REQ-IX-004 | ✅ Implemented | Free |
| S80 | 圆角与阴影体系 | REQ-DS-007 | ✅ Implemented | Free |

### v2.4.0 发布验证 (2026-08-22)

- Rust Free: 1552 passed / Pro: 1857 passed
- Clippy 零警告 / TypeScript 0 errors
- Vitest: 1767 passed, 88 files
- Release build: exit_code 0 (12.3m)

---

## 3. 已完成借鉴计划全览

| 借鉴来源 | 状态 | 内容 |
|---|---|---|
| QM (Quotient) | ✅ 全部完成 (Q01-Q11) | 记忆系统 / 上下文压缩 / 安全层 / 基础设施 / LLMProvider |
| OpenCode | ✅ 全部完成 (B01-B14) | System Context / Epoch / 压缩模板 / Prompt Caching / Admission / Coordinator / Tool Bounding / Skill / 多协议 / Permission |
| 跨 Phase 整合 | ✅ 全部完成 (S67+S68+S71) | Burst Buffer / Sweeper / Shadow 筛查 |
| OpenMontage | ✅ 全部完成 | MMR 多样性重排 / 检索质量门控 / Scored Selector / Pipeline Checkpoint / Event Stream |
| DS4 (DwarfStar) | ✅ DS-01 + DS-02 + DS-03 完成 | Prompt Prefix 磁盘缓存 / 上下文压缩原文尾部保留 / Session Strip 会话条带化 |
| DS4 剩余 | 📋 待定 (DS-04~DS-07) | 功率管理 / Tool Call 续接 / Radix Tree / SSD 流式 |

---

## 4. 验收评估审计期状态

### S1：前端模块全量审计 ✅ 完成（2026-08-13）

**审计范围**: 79 个前端 JS 模块（`ui/src/*.js`），逐个评估。

| 等级 | 数量 | 占比 | 含义 |
|---|---|---|---|
| A | 24 | 30% | 优秀 — 保持 |
| B | 24 | 30% | 良好 — 小幅优化 |
| C | 14 | 18% | 需改进 |
| D | 12 | 15% | 不合格 — 大改 |
| F | 5 | 6% | 需重写/删除 |

**关键发现**: 零僵尸模块 / 设置爆炸 (35 分区) / Toggle 过多 (5 个) / 性能开关过度暴露 / 开发者工具混入用户设置

### S2：后端 IPC 命令全量审计 ✅ 完成（2026-08-13）

**审计范围**: 236 个 IPC 命令（224 Free + 12 Pro-only），逐个评估。

| 等级 | 数量 | 占比 | 含义 |
|---|---|---|---|
| A | 62 | 26% | 优秀 — 核心功能，保持 |
| B | 58 | 25% | 良好 — 有用户价值 |
| C | 42 | 18% | 需改进 — 场景模糊 |
| D | 38 | 16% | 不合格 — 开发者工具 |
| F | 36 | 15% | 需删除/隐藏 |

**关键发现**: 零僵尸命令 / 11 个幻影调用 / ~65 个借鉴命令 / 21 个开发者工具混入用户功能 / 19 个性能优化命令全部默认关闭

### S3：UI 界面验收评估 ✅ 完成（2026-08-13）

**评估标准 10 条**：S1 首次 ≤ 3 步 ✅ / S2 核心 ≤ 2 点击 ✅ / S3 设置 ≤ 8 分区 🔴(35) / S4 toggle ≤ 2 🔴(5) / S5 顶栏 ≤ 5 🟡(8) / S7 设置人类语言 🔴(20 术语) / S8 默认即最佳 🔴(13/20 关闭) / S9 错误可理解 🟡(60% 技术化)

### S4：用户场景验收评估 ✅ 完成（2026-08-13）

- 15/15 场景全部可完成（覆盖率 100%）
- 智能模式 93.3% 场景无需手动配置
- 8 分区方案通过 / 2 toggle 方案通过

### S5：综合行动方案 📋 待执行

S5 P0 行动清单：
1. 智能模式单一开关实现
2. 输入区 5 → 2 toggle 精简
3. 设置面板 35 → 8 分区重整
4. 20 术语通俗化
5. 11 幻影调用修复
6. 4 开发者工具移入隐藏菜单

### S7：P2 行动清单 ✅ 完成（2026-08-13）

- HTML 导出完整性增强
- VLM 设置分区归属调整
- 设置面板 Tab 滚动定位优化
- 斜杠命令面板键盘导航优化

---

## 5. 待办任务详情

### P0: 剩余 3 项待规划

| P0 编号 | 来源 | 问题 | 状态 |
|---|---|---|---|
| P0-1 (R1) | 嵌入模型升级 | bge-m3 1024-dim 已完成 | ✅ 已修复 |
| P0-2 (R1) | Prompt 注入防护 | 检索片段注入无防护 | ✅ 已修复 |
| P0-3 (R2) | Pro 功能 ROI | 14 个 Pro 功能评估完成 | ✅ 已评估 |
| P0-4 (R2) | 持续收入模型 | 无订阅/云服务 | 📋 商业层面 |
| P0-5 (R3) | 数据库回滚 | 无迁移前备份/回滚 | ✅ 已修复 |
| P0-6 (R3) | 插件系统 | MCP 协议 Phase 1 已完成 | ✅ Phase 1 完成 |

### P1: 已全部实现

| REQ ID | 域 | 描述 | SRS 状态 |
|---|---|---|---|
| REQ-ING-003 | ING | 文件选择器导入 | Implemented |
| REQ-NFR-001 | NFR | 冷启动性能 | Implemented |
| REQ-NFR-002 | NFR | 检索性能 | Done |
| REQ-NFR-003 | NFR | 平台覆盖（跨平台 CI） | Implemented |

### P3: 远期

- VIZ Phase 4+: G6/D3/Three.js 专业领域可视化
- MM Phase 4: CLIP 多模态嵌入 / 本地 VLM
- E2E 真实 GUI 测试 macOS 支持

---

## 6. 当前 Git 状态 (v3.1.0 重构前)

- **分支**: `refactor/great-simplification`
- **基线 tag**: `v3.1.0-final`
- **本次变更** (Phase 1 R3): 删除 10 个本地 LLM 推理模块 + 清理 LlmMode::Local + 更新 SRS
- **累计削减** (Phase 1 R1+R2+R3): ~37,862 行 (-28.9%)
