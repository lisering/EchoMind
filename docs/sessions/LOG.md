# EchoMind 会话时间线日志

> **本文件按时间线记录所有 AI 开发会话的摘要。**
>
> 每次 session 结束时在末尾追加记录，最新记录在最下方。
>
> **格式**: `| 日期 | Session ID | 功能 | 测试数 | 变更文件数 | 关键产出 |`

---

## 2026-07-25 之前（早期开发）

| 日期 | Session | 功能 | 测试 | 文件 | 关键产出 |
|---|---|---|---|---|---|
| 早期 | - | 项目初始化 + P0 核心功能 | - | - | 文档导入/分块/向量检索/SSE 流式对话/会话持久化/License 验证 |
| 早期 | - | VIZ 可视化 Phase 1-3 | 11 | - | Mermaid/KaTeX/Chart.js 富内容渲染 |
| 早期 | - | MM 多模态 Phase 1-3 | 6 | - | PDF 渲染/OCR/VLM 多模态管线 |
| 2026-07-25 | - | session-handoff.md 初版 | - | 1 | 开发会话交接文档 |

## 2026-07-28

| 日期 | Session | 功能 | 测试 | 文件 | 关键产出 |
|---|---|---|---|---|---|
| 07-28 | Free/Pro 拆分 | `#[cfg(feature = "pro")]` 条件编译 | Free 144 + Pro 193 | 16 | Free/Pro 双版本编译通过 |

## 2026-07-29

| 日期 | Session | 功能 | 测试 | 文件 | 关键产出 |
|---|---|---|---|---|---|
| 07-29 | Logo + i18n + 菜单 | 品牌 + 国际化 + Tauri 菜单 | 161 | - | Crystal Echo Logo/双语 i18n/系统菜单栏 |
| 07-29 | 列表性能优化 | 懒加载/节流/搜索 | 159 (Free) + 208 (Pro) | - | get_messages_paginated/PaginatedResult/debounce |
| 07-29 | UI 布局修复 | 侧栏/输入框/折叠 | - | - | shrink-0/overflow-hidden/同屏显示 |

## 2026-07-30

| 日期 | Session | 功能 | 测试 | 文件 | 关键产出 |
|---|---|---|---|---|---|
| 07-30 | P1 功能批次 2 | 6 个 P1 功能 | 312 | - | Cross-Encoder/HyDE/导出/多嵌入模型/文件同步/领域分类/Agentic RAG |

## 2026-07-29 ~ 07-31 (Phase 1: 本地 LLM 引擎, 7 个 Session)

| 日期 | Session | 功能 | 测试 | 文件 | 关键产出 |
|---|---|---|---|---|---|
| 07-29 | Phase1 S1 | 数据模型层 (models) | 4 | 1 | LlmMode/ModelInfo/DownloadProgress/LoadStatus |
| 07-29 | Phase1 S2 | ModelManager (infra) | 20 | 3 | list/delete/download + RECOMMENDED_MODELS |
| 07-29 | Phase1 S3 | LocalLlmEngine (infra) | 8 | 2 | mistral.rs GGUF + LLMProvider impl |
| 07-29 | Phase1 S4 | AppState 集成 | - | 2 | llm_mode 切换 + chat_inner provider 分支 |
| 07-29 | Phase1 S5 | IPC 命令 | - | 2 | 7 个命令注册 |
| 07-29 | Phase1 S6 | 前端集成 | - | 2 | 设置面板 + 下载进度 + 模式切换 |
| 07-31 | Phase1 S7 | 文档同步 | - | 8 | SRS/IPC/Contracts/Settings/Frontend/ADR-011/AGENTS/CLAUDE |

## 2026-07-31 (Phase 2: GPU + PagedAttention + 采样, 8 个 Session)

| 日期 | Session | 功能 | 测试 | 文件 | 关键产出 |
|---|---|---|---|---|---|
| 07-31 | Phase2 S8 | GPU 设备选择 | - | - | best_device() Metal/CUDA |
| 07-31 | Phase2 S9 | PagedAttention | - | - | PagedAttentionMetaBuilder |
| 07-31 | Phase2 S10 | 采样参数 | - | - | LlmSamplingParams |
| 07-31 | Phase2 S11 | 模型卸载/切换 | - | - | RwLock<Option<Arc<Model>>> |
| 07-31 | Phase2 S12 | 流取消 | - | - | CancellationToken |
| 07-31 | Phase2 S13 | 预热 | - | - | warm_up() 后台加载 |
| 07-31 | Phase2 S14 | 前端采样参数 UI | - | - | 设置面板采样参数 |
| 07-31 | Phase2 S15 | 性能基准 + 文档 | 103 (Pro) | 9 | TC-LLM-PERF-001~003 + REQ-LLM-003/005/006 Implemented |

## 2026-07-31 ~ 08-01 (Phase 3: 自研量化内核, 8 个 Session)

| 日期 | Session | 功能 | 测试 | 文件 | 关键产出 |
|---|---|---|---|---|---|
| 07-31 | Phase3 S16 | Zed 四模式借鉴 | - | - | Store 模式/Settings Registry/#[perf]/Action 前端 |
| 07-31 | Phase3 S17 | 量化块结构 | - | 2 | BlockQ4_0/Q4K/Q8_0/Q8K + 反量化 |
| 07-31 | Phase3 S18 | GEMV 内核 | 14 | 2 | gemv_dispatch + 4 种量化格式标量实现 |
| 08-01 | Phase3 S19 | 权重重排 | 6 | 2 | Row-Major → Tile-Major cache 友好 |
| 08-01 | Phase3 S20 | 内核模式切换 | - | - | KernelMode::CustomGemv + 2 IPC 命令 |
| 08-01 | Phase3 S21 | Layer 级预取 | 6 | 3 | madvise(MADV_WILLNEED) + SendPtr |
| 08-01 | Phase3 S22 | RAM 预算管理 | 13 | 2 | LRU 驱逐 + libc::sysconf |
| 08-01 | Phase3 S23 | 性能基准 + 文档 | 4 ignored | 8 | TC-PHASE3-PERF-001~004 + REQ-LLM-007/008 |

## 2026-08-03

| 日期 | Session | 功能 | 测试 | 文件 | 关键产出 |
|---|---|---|---|---|---|
| 08-03 | Phase4 S25 | KV Cache 跨会话复用 | 7 | 6 | save/load/clear_kv_cache + 5 IPC 命令 |

## 2026-08-04 (极致性能优化, 10 个 Session)

| 日期 | Session | 功能 | 测试 | 文件 | 关键产出 |
|---|---|---|---|---|---|
| 08-04 | Perf S1 | 语义缓存金字塔 | 12 + 7 | 26 | SqliteCache L0/L1/L3 + 4 IPC + 4 设置键 |
| 08-04 | Perf S2 | Prompt 压缩 | 11 | 9 | RuleBasedCompressor + OnnxCompressor + 2 IPC |
| 08-04 | Perf S3 | CRAG + Self-RAG | - | - | 上下文充分性检测 |
| 08-04 | Perf S4 | Proposition 分割 | 10 | 9 | PropositionSplitter + propositions 表 + 1 IPC |
| 08-04 | Perf S5 | ColBERT 多向量 | - | - | ColbertEmbedder (Pro) |
| 08-04 | Perf S6 | RAPTOR 摘要树 | - | - | SummaryTreeBuilder + summary_nodes 表 |
| 08-04 | Perf S7 | 渐进式上下文注入 | 14 | 5 | ProgressiveInjector + 1 设置键 |
| 08-04 | Perf S8 | Speculative RAG | 12 | 5 | SpeculativeRagEngine<D,L> + 1 设置键 |
| 08-04 | Perf S9 | 检索记忆自进化 | 10 | 7 | RetrievalMemoryEngine + 3 IPC + 1 设置键 |
| 08-04 | Perf S10 | 文档同步 + 基准 | 5 | 2 | TC-BENCH-001~005 + PERF 域 12 REQ Implemented |

## 2026-08-04 (聊天韧性修复)

| 日期 | Session | 功能 | 测试 | 文件 | 关键产出 |
|---|---|---|---|---|---|
| 08-04 | 聊天韧性 | embedder 超时 + 看门狗 | 4 E2E + 2 集成 | - | 180s 后端超时 + 300s 前端看门狗 |

## 2026-08-04 (v1.0.1 P1 Draft 收尾, 4 个 Session)

| 日期 | Session | 功能 | 测试 | 文件 | 关键产出 |
|---|---|---|---|---|---|
| 08-04 | P1 S1 | 键盘无障碍 Phase 1 | 12 E2E | - | Tab 焦点序 + 键盘快捷键 |
| 08-04 | P1 S2 | Focus Trap | 6 E2E | 7 | WAI-ARIA Dialog Modal |
| 08-04 | P1 S3 | 对比度修复 | 4 E2E | 6 | text-quaternary #64748B→#8995A8 |
| 08-04 | P1 S4 | 高 DPI 适配 | 4 E2E | 3 | deviceScaleFactor 2x/1.5x |

## 2026-08-04 (DeepSeek 风格 UI, 5 个 Session)

| 日期 | Session | 功能 | 测试 | 文件 | 关键产出 |
|---|---|---|---|---|---|
| 08-04 | DS S1 | 视觉基础对齐 | - | - | 气泡圆角 22px + AI 透明 + 免责声明 + CJK 引号 |
| 08-04 | DS S2 | 思维链折叠 | - | - | thinking-panel.js + sources 可折叠 |
| 08-04 | DS S3 | 输入区 Toggle | - | - | input-toggles.js + ⌘J |
| 08-04 | DS S4 | 设计系统增强 | - | - | CSS 变量三层架构 + 主题切换过渡 |
| 08-04 | DS S5 | E2E + 文档 | 8 E2E | - | TC-DS-013~020 + 文档同步 |

## 2026-08-04 (E2E 测试质量修复)

| 日期 | Session | 功能 | 测试 | 文件 | 关键产出 |
|---|---|---|---|---|---|
| 08-04 | E2E 质量修复 | 弱断言/恒真断言修复 | 114 passed | 11 | 30+ 处弱断言修复 + tauri-stub 会话幂等 |

## 2026-08-04 (测试体系全面评估)

| 日期 | Session | 功能 | 测试 | 文件 | 关键产出 |
|---|---|---|---|---|---|
| 08-04 | 测试评估 | page.evaluate 语法修复 + 新增测试域 | 20 passed | 13 | 8 文件语法修复 + 5 新 spec 文件 |

## 2026-08-04 (软件资产 + Session 管理)

| 日期 | Session | 功能 | 测试 | 文件 | 关键产出 |
|---|---|---|---|---|---|
| 08-04 | 资产+Session | 全量源码阅读 + 资产文档 + Session 体系 | - | 4 | `docs/SOFTWARE_ASSET.md` (1100+ 行) + `docs/sessions/` 目录 |

---

## 2026-08-04 (前端架构升级)

| 日期 | Session | 功能 | 测试 | 文件 | 关键产出 |
|---|---|---|---|---|---|
| 08-04 | 前端架构 | TypeScript checkJs + esbuild + 模块拆分 + Vitest 加强 | Vitest 389 (26 files) | 12+ | tsconfig.json / types/*.d.ts / esbuild 替代 MODULE_ORDER / chat.js 拆分为 chat-render.js / settings.js 拆出 settings-local-llm.js / 新增 action.test.js + guards.test.js |

## 2026-08-05 (编辑分页 DeepSeek 对齐修复)

| 日期 | Session | 功能 | 测试 | 文件 | 关键产出 |
|---|---|---|---|---|---|
| 08-05 | 编辑分页修复 | 编辑后分页器位置/时机对齐 chat.deepseek.com + 版本切换丢答案/错思考过程修复 | Vitest 404 (27 files) 全绿；tsc 无新增错误 | 8 | 根因：`renderMarkdown` 未写 `dataset.rawMarkdown` → v1 答案永不注册（丢答案）；`applyBranchView` 切换不清思考面板（残留新版本推理）；分页器流式期间挂用户气泡。修复：markdown.js 写入 rawMarkdown、thinking-panel 新增 `clearContent()`、分页器改挂 assistant 答案下方（.msg-actions 之前）、流完成后才渲染、版本切换后编辑/复制读取当前版本内容、turn-tree 清理死代码 |

## 2026-08-05 (DeepSeek 问答区对齐改造)

| 日期 | Session | 功能 | 测试 | 文件 | 关键产出 |
|---|---|---|---|---|---|
| 08-05 | DeepSeek 问答区对齐 | CDP 抓取 chat.deepseek.com 真实 DOM → 用户消息去气泡纯文本右对齐、版本切换器胶囊样式（◀ 1 / N ▶ 带空格）、思考面板 markdown 渲染（finalizeReasoning/renderReasoning）、操作按钮胶囊化 | Vitest 407 (27 files)；deepseek-style 8/8 通过；tsc 无新增 | 8 | 分析：ds-message / ds-think-content / 版本切换器 `2 / 2` 结构；改造：chat-render.js（去气泡）、message-edit.js（counter 带空格）、thinking-panel.js（3 个新方法）、typography.css/components.css（胶囊）、TC-DS-013 更新为纯文本断言 |
| 08-05 | S1 知识图谱实体关系 | EntityRelation/GraphTriple 模型 + extract_relations() 纯规则 8 种关系 + entity_relations 表 + 5 个 Storage trait 方法 + 4 路径导入管线集成 | 6 (TC-RELATION-001~006) | 9 | EntityRelation/GraphTriple 数据模型、纯规则关系抽取（零 LLM）、SQLite 持久化、导入管线集成 |
| 08-05 | S2 知识图谱检索管线 | GraphRetriever 图遍历检索器 + rrf_fuse_four_way() 四路 RRF 融合 + chat_inner 图扩展接入 + set_graph_retriever_enabled IPC + get_chunk_by_id Storage trait | 6 (TC-GRAPH-001~006) | 9 | BFS 图遍历（最大 2 跳 + 距离衰减）、四路 RRF 融合、优雅降级 |
| 08-05 | S3 前端知识图谱可视化 | D3.js v7 force-directed graph + 全屏 overlay 面板 + 节点拖拽/缩放/高亮 + 关系类型颜色编码 + Focus Trap + 空状态 + 实体详情面板 + 3 个 IPC 命令 | 10 (TC-GRAPH-VIZ-001~010) | 21 | D3 forceSimulation 可视化、8 种关系颜色编码、节点高亮交互、双击实体详情、i18n 双语 |

## 统计摘要

| 维度 | 数值 |
|---|---|
| 总 Session 数 | ~50+ |
| Rust 测试 (Free) | 623 passed |
| Rust 测试 (Pro) | 888 passed |
| Rust 集成测试 | 115+ |
| 前端单元测试 | 389+ (26 files) |
| E2E 桥接测试 | 710+ |
| IPC 命令数 | 93 |
| 数据库表数 | 16+ |
| 设置键数 | 39+ |
| Crate 数 | 7 |
| 前端模块数 | 39 |
| SRS 需求数 | 100+ (大部分 Implemented) |
| ADR 数 | 12 |

## 2026-08-05 (知识图谱实体关系检索)

| 日期 | Session | 功能 | 测试 | 文件 | 关键产出 |
|---|---|---|---|---|---|
| 08-05 | 知识图谱 | 实体关系图数据模型 + SQLite 存储层 + 关系抽取引擎（借鉴 mnemo） | 6 新增（TC-RELATION-001~006）全绿 | 10 | `EntityRelation`/`GraphTriple` 模型 + `extract_relations()` 纯规则中英文 8 种关系（零 LLM）+ `entity_relations` 表 + 5 个 Storage trait 方法 + 导入管线 4 路径集成 + SRS REQ-RAG-027 + code-contracts/database-schema/settings-reference 文档同步 |

---

## 2026-08-05 (知识图谱可视化增强)

| 日期 | Session | 功能 | 测试 | 文件 | 关键产出 |
|---|---|---|---|---|---|
| 08-05 | 知识图谱可视化增强 | 实体类型图标 + 子图过滤 + 搜索定位 + SVG/PNG 导出 + 详情面板增强 | 8 新增（TC-GRAPH-VIZ-011~018）全绿 | 15 | `get_entity_types` IPC 命令 + Storage trait 批量查询方法 + 5 种内联 SVG 实体类型图标 + 关系类型过滤面板 + 搜索定位（D3 zoom transform）+ SVG 序列化导出 + Canvas PNG 转换 + 详情面板实体类型徽章 + 来源 chunk 预览 + 中英双语 i18n + SRS AC-24~AC-31 + 4 个文档同步 |

## 2026-08-05 (知识图谱高级分析)

| 日期 | Session | 功能 | 测试 | 文件 | 关键产出 |
|---|---|---|---|---|---|
| 08-05 | 知识图谱高级分析 | 布局切换（force/hierarchical/radial）+ 路径分析（BFS 最短路径 + 高亮）+ 社区检测（Label Propagation + 着色）+ 度中心性 | 10 Rust 单元（TC-GRAPH-ANALYSIS-001~010）+ 8 E2E（TC-GRAPH-VIZ-019~026）全绿 | 16 | `GraphAnalyzer` 图分析引擎（shortest_path BFS + all_paths DFS + detect_communities Label Propagation + degree_centrality）+ `GraphPath`/`GraphCommunity` 数据模型 + `get_entity_graph` Storage trait 方法 + 3 个新 IPC 命令（get_shortest_path/get_communities/get_graph_layout）+ D3 布局切换器 + 路径分析面板（选择器+高亮）+ 社区着色（10 色调色板）+ 中英双语 i18n + SRS AC-32~AC-40 + 4 个文档同步 |

## 2026-08-05 (知识图谱功能收尾)

| 日期 | Session | 功能 | 测试 | 文件 | 关键产出 |
|---|---|---|---|---|---|
| 08-05 | 知识图谱收尾 | 全量回归测试 + 文档同步 + ADR 创建 | Free 653 + Pro 888 passed, 0 failed; clippy 零警告; E2E 26/26; Vitest 407/407 | 6 | `ADR-013-knowledge-graph.md` 架构决策记录 + `SOFTWARE_ASSET.md` 知识图谱章节（模块/数据模型/数据库表/IPC 命令/数据流图/设置键）+ `AGENTS.md` key files + 数据流图 + `CLAUDE.md` key files + 数据流图 + `CURRENT_STATE.md` 更新 + session 归档 |

## 2026-08-06 (Agentic AI 前端 UI 模块 + E2E 测试修复)

| 日期 | Session | 功能 | 测试 | 文件 | 关键产出 |
|---|---|---|---|---|---|
| 08-06 | Agentic AI 前端 UI | 5 个新前端模块 + tauri-stub.js 117 IPC mock 全覆盖 + 5 个 E2E spec 文件 + memory-panel 测试修复 + 文档同步 | E2E 26/26 passed; Vitest 407/407 passed; tsc 42 residual（预存）; UI build 10657 行 | 10+ | `ui/src/perf-settings.js`（性能优化设置：缓存/压缩比/检索记忆/索引重建）+ `ui/src/memory-panel.js`（记忆管理：三列 tier 布局/promote/delete/clear/pin）+ `ui/src/dream-panel.js`（AutoDream：触发/中止/历史建议）+ `ui/src/symbol-search.js`（代码符号搜索：tree-sitter AST 符号）+ `ui/src/workflow-panel.js`（DAG 工作流：模板 CRUD/运行/进度）+ `ui/src/input-toggles.js` 扩展（sub_agent/memory toggle）+ `tauri-stub.js` 117 IPC mock + `memory-panel.spec.ts` 修复（strict mode violation: div vs button data-tier 选择器）+ `frontend-modules.md` §8 新增（6 个子节）+ `CURRENT_STATE.md` 更新 + `sync-to-github.sh` 3 files committed |

## 2026-08-06 (全量回归验证 + 文档同步 + Session 归档)

| 日期 | Session | 功能 | 测试 | 文件 | 关键产出 |
|---|---|---|---|---|---|
| 08-06 | 回归验证+文档同步 | 全量回归测试 + P1 需求确认 + 文档同步 + Session 归档 | Free 718/0/27; Pro 1003/0/44; E2E 986/0/9skip; Vitest 407; Clippy 零警告; fmt 通过; esbuild 成功; tsc 42 预存 | 6 文档 | `CURRENT_STATE.md` 更新（测试数 653→718 / 888→1003 / 752→986; P1 Draft→已全部实现; 验证命令速查）+ `SOFTWARE_ASSET.md` 更新（前端 39→49 模块; 新增 11 个模块清单; 测试矩阵更新）+ `AGENTS.md` 更新（41→49 files; 5 个新 key files）+ `CLAUDE.md` 更新（41→49 ES module; 5 个新 key files）+ `ipc-commands.md` 新增 `set_sub_agent_enabled` 命令文档 + Session 归档 `2026-08-06-regression-verification-doc-sync.md` |
| 08-06 | v1.0 发布准备 | TypeScript 清零 + E2E 弱断言审计 + 集成测试补强 + 版本号 1.0.0 + Changelog + 全量验证 + GitHub 同步 | Free 732/0/27; Pro 1003/0/44; E2E 985/0/9skip; Vitest 407; tsc 0 errors; Clippy 零警告; fmt 通过; esbuild 10821 行; release 编译通过 | 12+ 文件 | TS 42→0; E2E 26 处弱断言修复; 集成测试 14 新增+6 修复; 版本号 0.1.0→1.0.0; `CHANGELOG_v1.0.md`; `v1.0-release-checklist-final.md`; Session 归档 `2026-08-06-v1.0-release-prep.md`; `sync-to-github.sh` 8 files |
| 08-06 | v1.1 版本规划 | ROI 评估（12 项功能）+ Top 5 技术调研 + 原子计划（S44-S48）+ SRS 扩展（6 条新 REQ）+ README Roadmap + 文档同步 + 归档 | 无源代码变更（纯规划文档） | 8 文档 | `V1.1_ATOMIC_PLAN.md`（ROI 评分表 + 5 功能调研报告 + 5 Session 原子计划）+ SRS §6 更新（.docx/.html/浅色主题/语音/TTS/网页搜索移入 v1.1）+ SRS §7 v4.0.0 记录 + 6 条新 Draft REQ（REQ-ING-015/016, REQ-UI-011, REQ-RAG-034/035/036）+ README v1.1 Roadmap（中英文）+ SOFTWARE_ASSET §22 + CURRENT_STATE P2 + Session 归档 + `sync-to-github.sh` |
| 08-06 | v1.1 S44 | .docx 文档导入实现（REQ-ING-015 Draft → Implemented）| Free 736 / Pro 1007（+4 TC-ING-DOCX）| 12 文件 | `DocxLoader`（docx-rs v0.4.22, read_docx → 段落/表格/图片提取）+ `ALLOWED_EXTENSIONS` 8→9 + `index_inner`/`index_multimodal_inner` docx 分支 + 4 TDD 测试 + 前端对话框/i18n/tauri-stub 同步 + 集成测试 tc_err_006 修复（.docx → .xlsx）+ SRS v4.1.0 changelog + V1.1 S44 ✅ + AGENTS/CLAUDE 更新 |
| 08-06 | v1.1 S46 | 浅色主题切换实现（REQ-UI-011 Draft → Implemented）| E2E 5/5 passed; tsc 0 errors; clippy 零警告; fmt 通过; esbuild 10901 行 | 12 文件 | `tokens.css` 新增 `[data-theme="light"]` ~40 变量 + `[data-theme="system"]` @media + `--accent-text` 对比度修复 + 浅色滚动条；`main.js` 新增 `applyTheme`/`setTheme`/`initTheme`（双 rAF 防闪烁 + Mermaid 同步 + prefers-color-scheme 监听）；`settings.js` 新增 `initThemeSwitcher` + 外观 UI（三选一分段控件 + SVG 图标）；`commands.rs` 新增 `set_theme`/`get_theme` IPC + `*_inner`；`lib.rs` 注册 2 命令；i18n 4 键（中英文）；tauri-stub mock；`theme.spec.ts` 5 E2E 测试；SRS v4.3.0 changelog + V1.1 S46 ✅ + settings-reference `ui.theme` |
| 08-06 | v1.1 S47 | 语音输入 + TTS 朗读实现（REQ-RAG-034/035 Draft → Implemented）| E2E 6/6 passed; tsc 0 errors; clippy 零警告; fmt 通过; esbuild 11134 行 | 11 文件 | 新增 `ui/src/voice-input.js`（SpeechRecognition 麦克风转写 + speechSynthesis TTS 朗读 + 优雅降级）+ `index.html` 新增 `#micBtn` + 设置面板语音区域（语速滑块）+ `chat-render.js` 插入 `.tts-btn` + `settings.js` `initVoiceSettings()` + `main.js` `initVoiceInput()`/`stopAllTTS()` + `components.css` `@keyframes micPulse` + `dom-ext.d.ts` 类型声明 + i18n voice 文案 + tauri-stub Web Speech API mock + `voice.spec.ts` 6 E2E + SRS v4.4.0 + V1.1 S47 ✅ + settings-reference `voice.rate` |
| 08-06 | v1.1 S48 | 网页搜索集成实现（REQ-RAG-036 Draft → Implemented）| Free 749/0/27; Pro 1020/0/44; E2E 5/5; tsc 0 errors; clippy 零警告; fmt 通过; esbuild 11142 行 | 15 文件 | `models` 新增 `SearchResult` struct + `core/lib.rs` 新增 `WebSearchProvider` trait + `core/web_search.rs` 融合引擎（should_search/convert_to_retrieval_results/rrf_fuse_with_web/search_and_fuse）+ `core/web_search_tests.rs` 6 TDD 测试 + `chat.rs` `ChatEngine` 可选 web_search_provider + `with_web_search()` builder + `infra/duckduckgo_provider.rs` DuckDuckGo Instant Answer API（连接超时 5s/请求超时 10s/AbstractText+RelatedTopics 解析/优雅降级）+ 3 测试 + `tauri-app` `AppState.web_search_enabled` + `chat_inner` 集成 + 2 IPC 命令（set_web_search_enabled/web_search）+ 前端 input-toggles.js 新增 web_search toggle + i18n 中英文 + tauri-stub mock + `web-search.spec.ts` 5 E2E + SRS v4.5.0 + V1.1 S48 ✅ + AGENTS/CLAUDE 命令数 134→138 + IPC 命令数 136→138 + settings-reference `rag.web_search_enabled` + code-contracts WebSearchProvider trait + SOFTWARE_ASSET duckduckgo_provider |
| 08-06 | v1.1.0 发布验证 | 全量回归测试 + Release build 验证 + GitHub 同步 + git tag v1.1.0 | Free 749/0/27; Pro 1034/0/44; E2E 999/0/9skip; Clippy 零警告; fmt 通过; tsc 0; esbuild 11142; release OK | 4 文档 | `CHANGELOG_v1.1.md` 测试数修正（Pro 1020→1034, E2E 1001→999）+ `SRS_v1.0.md` v5.0.0 changelog E2E 修正 + `CURRENT_STATE.md` 发布验证明细 + `LOG.md` 追加 + `sync-to-github.sh --commit` 6 files + git tag v1.1.0 |
| 08-06 | v1.2 版本规划 | ROI 评估（9 项功能）+ Top 4 技术调研 + 原子计划（S49-S53）+ SRS 扩展（4 条新 REQ）+ README Roadmap + 文档同步 + 归档 | 无源代码变更（纯规划文档） | 10 文档 | `V1.2_ATOMIC_PLAN.md`（ROI 评分表 + 4 功能调研报告 + 5 Session 原子计划）+ SRS §6 更新（.pptx/.epub/自定义模型/PDF 导出移入 v1.2）+ SRS §7 v5.1.0 记录 + 4 条新 Draft REQ（REQ-EXP-005/REQ-ING-017/REQ-ING-018/REQ-VEC-014）+ README v1.2 Roadmap（中英文）+ CURRENT_STATE P2 + LOG 追加 + v1.1 发布归档 + `cargo search` 确认 pptx-to-md 1.0.0 + rbook 0.7.10（排除 epub crate GPL-3.0）+ SOFTWARE_ASSET/AGENTS/CLAUDE 更新 |
| 08-06 | v1.2 S49 | PDF 导出实现（REQ-EXP-005 Draft → Implemented）| E2E 5/5 passed; tsc 0 errors; clippy 零警告; fmt 通过; esbuild 11566 行 | 11 文件 | 新增 `ui/src/export.js`（exportConversationToPdf/exportDocumentToPdf/buildPrintHtml/initExportButtons + getPdfPageSize/setPdfPageSize/getPdfIncludeSources/setPdfIncludeSources——复用 Markdown 渲染管线 marked+DOMPurify+highlight.js → 隐藏 iframe → window.print()）+ `ui/styles/print.css`（@media print 规则：隐藏 UI 元素 + serif 12pt + page-break-inside:avoid）+ `ui/index.html` 新增 #exportPdfBtn + 设置面板 PDF 区域（A4/Letter 选择 + 引用来源 toggle）+ `ui/src/settings.js` initPdfExportSettings() + `ui/src/main.js` initExportButtons() + i18n export_pdf 中英文 19 键 + tauri-stub.js window.print mock + get_document_chunks mock + dom-ext.d.ts Window 属性声明 + export-pdf.spec.ts 5 E2E（TC-EXP-PDF-001~005）+ SRS v5.2.0 + V1.2 S49 ✅ + frontend-modules §9 + settings-reference export.pdf_page_size/export.pdf_include_sources + AGENTS/CLAUDE 49→50 模块。零新增 Rust 依赖，零新增 IPC 命令。 |
| 08-06 | v1.2 S50 | .pptx 演示文稿导入实现（REQ-ING-017 Draft → Implemented）| Free 753/0/27; clippy 零警告; fmt 通过; esbuild 11566 行; tsc 0 errors | 12 文件 | `Cargo.toml` 新增 `pptx-to-md = "1.0"`（MIT/Apache-2.0）+ `loader.rs` 新增 `PptxLoader`（PptxContainer::open → convert_to_md → 后处理压缩换行；config: extract_images(false)/include_speaker_notes(true)/include_slide_number_as_comment(false)/include_presentation_metadata(false)）+ `loader_tests.rs` TC-ING-PPTX-001~004（fixture 文件 + env!("CARGO_MANIFEST_DIR") 路径定位）+ `import.rs` ALLOWED_EXTENSIONS 11→12 + index_inner/index_multimodal_inner pptx 分支 + i18n en/zh-CN 3 处格式列表加 .pptx + tauri-stub WHITELIST + 错误消息 + wizard.js/chat-render.js/import.js extensions 数组 + tests/fixtures/gen_pptx.py + 4 个 .pptx fixture 文件 + SRS v5.3.0 + V1.2 S50 ✅ + code-contracts §5.5b + ipc-commands 格式列表 + SOFTWARE_ASSET ALLOWED_EXTENSIONS + AGENTS/CLAUDE loader.rs 描述 + sync 数据流 |
| 08-07 | v1.2 S51 | .epub 电子书导入实现（REQ-ING-018 Draft → Implemented）| Free 757/0/27; Pro 1042/0/44; clippy Free+Pro 零警告; fmt 通过; esbuild 11566 行; tsc 0 errors | 16 文件 | `Cargo.toml` 新增 `rbook = { version = "0.7", default-features = false, features = ["threadsafe", "prelude"] }`（Apache-2.0，EPUB 2/3）+ `loader.rs` 新增 `EpubLoader`（Epub::open → spine().iter() → manifest_entry.read_str() → html2text::from_read() → 章节间 `---` 分隔 + 压缩换行）+ `loader_tests.rs` TC-ING-EPUB-001~004（fixture 文件 + env!("CARGO_MANIFEST_DIR")）+ `import.rs` ALLOWED_EXTENSIONS 12→13 + index_inner/index_multimodal_inner epub 分支 + i18n en/zh-CN 3 处格式列表加 .epub + tauri-stub WHITELIST + 错误消息 + wizard.js/chat-render.js/import.js extensions 数组 + tests/fixtures/gen_epub.py + 4 个 .epub fixture 文件 + SRS v5.4.0 + V1.2 S51 ✅ + code-contracts §5.5c + ipc-commands 格式列表 + SOFTWARE_ASSET ALLOWED_EXTENSIONS + AGENTS/CLAUDE loader.rs 描述 + sync 数据流 |
| 08-07 | v1.2 S52 | 自定义 ONNX 嵌入模型上传实现（REQ-VEC-014 Draft → Implemented，Pro 门控）| Free 765/0/27; Pro 1057/0/44; clippy Free+Pro 零警告; fmt 通过; esbuild 11670 行; tsc 0 errors | 12 文件 | `local_embedder.rs` 新增 `EmbeddingModel::Custom(String)` + `new_with_custom_model()`/`init_custom()`（fastembed UserDefinedEmbeddingModel，首次推理检测维度）+ `find_onnx_file()`/`validate_onnx_file()`/`validate_tokenizer_files()`/`list_custom_models()`/`delete_custom_model()`/`copy_custom_model_files()` + `CustomModelInfo` struct + `is_custom()`/`dim()` 返回 0 for Custom + `REQUIRED_TOKENIZER_FILES`/`ONNX_FILE_CANDIDATES` 常量；`model_store.rs` `parse_embedding_model()` 解析 `"custom:{name}"` + `load_embedding_model()`/`init_embedder_with_progress()` 走 `new_with_custom_model()` 路径；`state.rs` `custom_model_dir()`；`commands.rs` 3 Pro 门控 IPC + `*_inner`（upload/list/delete + Free stub PRO_REQUIRED + 路径遍历防护）；`lib.rs` 注册 3 命令（138→141）；前端 settings.js `loadCustomModels()`/`onUploadCustomModel()`/`switchToCustomModel()`/`deleteCustomModel()` + ipc.js `embedModelApi.uploadCustomModel/listCustomModels/deleteCustomModel` + index.html 自定义模型设置区域 + i18n 17 键（中英文）+ tauri-stub.js mock 3 命令 + types/tauri.d.ts 3 window 声明；测试 7 集成 TC-VEC-CUSTOM-001~007 + 8 单元 TC-VEC-CUSTOM-UNIT-001~008；SRS v5.5.0 + V1.2 S52 ✅ + IPC §2.53 + settings-reference `vec.embedding_model` custom: 前缀 + code-contracts `new_with_custom_model`/`list_custom_models`/`CustomModelInfo` + frontend-modules §2.12.1。零新增 Rust 依赖（复用 fastembed）。 |
| 08-07 | v1.2.0 发布 | 全量回归测试 + 版本号升级 1.1.0→1.2.0 + Release build + CHANGELOG + GitHub 同步 + git tag v1.2.0 | Free 766/0/27; Pro 1057/0/44; E2E 950/0/9skip; Vitest 407; Clippy 零警告; fmt 通过; tsc 0; esbuild 11670; release OK | 15+ 文件 | 版本号升级 7 crate Cargo.toml + tauri.conf.json（1.1.0→1.2.0）+ `CHANGELOG_v1.2.md`（4 功能 + 技术规格 + 向后兼容性）+ SRS v5.6.0 changelog + `V1.2_ATOMIC_PLAN.md` S53 ✅ + `CURRENT_STATE.md` 更新（版本号/测试数/发布状态）+ Session 归档 + `sync-to-github.sh --commit` + git tag v1.2.0 |

| 08-07 | v1.3 版本规划 | ROI 评估（15 项功能）+ Top 5 技术调研 + 原子计划（S54-S59）+ SRS 扩展（5 条新 REQ）+ README Roadmap + 文档同步 + 归档 | 无源代码变更（纯规划文档） | 5 文档 | `V1.3_ATOMIC_PLAN.md`（ROI 评分表 + 5 功能调研报告 + 6 Session 原子计划）+ SRS §6 更新（知识图谱导出/文档摘要/对话模板/双向链接/对话分支树移入 v1.3）+ SRS §7 v5.7.0 记录 + 5 条新 Draft REQ（REQ-EXP-006/REQ-ING-019/REQ-RAG-038/REQ-ING-020/REQ-RAG-039）+ README v1.3 Roadmap（中英文）+ CURRENT_STATE P2 + LOG 追加 + `sync-to-github.sh --commit`（2 files: README.md + README.zh-CN.md）。零新增依赖（全部使用已有 Rust crate + D3.js v7）。IPC 命令 141→152（+11 Free）。预计 8.5 天。 |
| 08-07 | v1.3 S54 | 知识图谱导出实现（REQ-EXP-006 Draft → Implemented）| 3 单元 + 2 E2E = 5 tests; clippy Free 零警告; fmt 通过; tsc 0 errors; esbuild 11700 行 | 12 文件 | `graph_export.rs` 新增模块（`export_graphml()` 手动拼接 XML + XML 特殊字符转义 + `export_jsonld()` serde_json 构建 @context + @graph）+ `graph_export_tests.rs` 3 TDD 测试（TC-EXP-GRAPH-001~003）+ `lib.rs` 注册模块 + `commands.rs` 新增 `export_graph` IPC + `*_inner`（format 参数 + list_all_relations + get_entity_types 构建实体列表）+ `lib.rs` 注册 1 命令（141→142）+ `graph-viewer.js` 新增 GraphML/JSON-LD 导出按钮 + `exportGraphData()` 函数 + i18n 2 键（中英文）+ tauri-stub.js mock + `graph-export.spec.ts` 2 E2E（TC-EXP-GRAPH-004~005）+ SRS v5.7.1 + IPC 文档 + V1.3 S54 ✅。零新增依赖。 |

---

> **维护规则**: 每次 session 结束时在末尾追加一行记录。大型功能开发（多 Session）的详细记录归档到 `archive/`。

---

| 日期 | Session ID | 功能 | 测试数 | 变更文件数 | 关键产出 |
|---|---|---|---|---|---|
| 2026-08-07 | S57 | Markdown 笔记双向链接（REQ-ING-020） | models 24 (含 3 新) + core wiki 16 + E2E 8 | 16 | WikiLink 模型 + wiki_link_parser.rs 解析器 + wiki_links 表 + 3 IPC 命令（get_forward_links/get_backlinks/rebuild_wiki_links）+ 导入管线集成 + 前端 wiki-links.js 模块 + markdown.js [[link]] 渲染 + i18n 8 键 + tauri-stub mock。IPC 145→148。零新增依赖。验证：clippy Free+Pro 零警告；cargo fmt 通过；tsc 0 errors；esbuild 12097 行；E2E 8 passed。 |
| 2026-08-07 | S58 | 对话分支/版本树（REQ-RAG-039） | models 28 (含 4 新) + E2E 5 (TC-RAG-BRANCH-001~005) | 11 | ConversationTreeNode/ConversationTree 模型 + 2 IPC 命令（get_conversation_tree/branch_from_message）复用现有 messages.turn_group+version+turn_active_versions（零新增数据库表）+ turn-tree.js 树视图面板 + #branchTreeBtn 按钮 + i18n 8 键 + components.css 样式 + tauri-stub mock。IPC 148→150。零新增 Rust 依赖。验证：clippy Free+Pro 零警告；cargo fmt 通过；tsc 0 errors；esbuild 12228 行；E2E 5 passed。 |
| 2026-08-07 | S59 | v1.3.0 发布准备（最终 Session） | Free 803/0/24; Clippy Free+Pro 零警告; fmt 通过; tsc 0 errors; esbuild 12228 行; Vitest 407 passed | 12+ 文件 | Bug 修复（DOC_COLS 缺少 summary 列导致 tc_db_001/tc_db_002 失败 + graph_analyzer flaky 测试修复）+ 版本号升级 1.2.0→1.3.0（7 crate Cargo.toml + tauri.conf.json）+ CHANGELOG_v1.3.md（5 功能 + Bug 修复 + 技术规格 + 向后兼容性）+ SRS v5.8.0 changelog + README v1.3 Released（中英文）+ CURRENT_STATE.md 更新 + LOG 追加 + V1.3_ATOMIC_PLAN S59 ✅。v1.3 全部 5 个功能（S54-S58）实现完毕：知识图谱导出 / 文档摘要自动生成 / 自定义快捷指令模板 / Markdown 笔记双向链接 / 对话分支/版本树。IPC 141→152（+11 Free）。零新增依赖。 |
| 2026-08-07 | S60 | v1.4 版本规划（ROI 评估 + 技术调研 + 原子计划 + SRS 扩展） | 无源代码变更（纯规划文档） | 6 文档 | `V1.4_ATOMIC_PLAN.md`（ROI 评分表 12 项候选 + 5 功能调研报告 + 6 Session S61-S66 原子计划 + 衔接提示词）+ SRS §6 v5.9.0 规划（5 条新 Draft REQ: REQ-ING-021/REQ-RAG-040/REQ-RAG-041/REQ-ING-022/REQ-ARCH-001）+ SRS §7 v5.9.0 changelog + README v1.4 Planning Roadmap（中英文）+ CURRENT_STATE P2 + LOG 追加。Top 5: .xlsx/.csv 导入 ROI 13.3 / 对话搜索 ROI 13.3 / Contextual Retrieval ROI 8.0（铁律十 Anthropic 论文）/ 标签系统 ROI 7.5 / commands.rs 拆分 ROI 5.0（铁律十四 SRP）。新增依赖 calamine v0.26 + csv v1.3。IPC 152→159（+7 Free）。预计 9 天。铁律八：互联网搜索 calamine/csv/Anthropic Contextual Retrieval/SQLite FTS5/MCP 协议。 |
| 2026-08-07 | S62 | 对话全文搜索（REQ-RAG-040） | 存储层 4 TDD (TC-RAG-SEARCH-001~004) + E2E 5 (TC-RAG-SEARCH-005a~e) | 10 文件 | `MessageSearchResult` 模型（已存在）+ `Storage::search_messages()` trait 方法（默认空操作，已存在）+ `SqliteStorage` FTS5 实现（messages_fts 虚拟表 + 3 触发器 + 回填迁移 + BM25 搜索 + 短查询 LIKE 回退，已存在）+ 修复 `MessageSearchResult` import 缺失 + `search_conversations` IPC 命令（conversation.rs）+ lib.rs 注册（152→153）+ 前端 sidebar.js 搜索弹框「会话/对话」模式切换 + `_renderMessageSearchPage()` 对话搜索结果渲染 + index.html 模式切换 toggle 按钮 + i18n 4 键（中英文）+ tauri-stub.js mock。零新增依赖（复用 rusqlite FTS5）。验证：infra 19 passed；clippy 零警告；cargo fmt 通过；tsc 0 errors；esbuild 12339 行；Vitest 407 passed；E2E 5 passed。 |
| 2026-08-07 | S63 | Contextual Retrieval 上下文增强嵌入（REQ-RAG-041） | 存储层 3 TDD (TC-RAG-CTX-001~003) | 3 文件（状态确认 + 文档更新） | **状态确认**：S63 已在 v6.3.0 changelog 中记录为 Implemented。代码审查确认全部组件已实现：`state.rs` `contextual_retrieval_enabled` 字段（默认 true）+ `commands/import.rs` `embed_document_chunks()` 条件化上下文前缀（`build_contextual_text()` 开关控制）+ `commands/performance.rs` `set_contextual_retrieval` + `rebuild_contextual_embeddings` 2 IPC 命令 + `models/lib.rs` `SettingsPayload.contextual_retrieval` 字段 + `lib.rs` 注册 2 命令（153→155）+ `perf-settings.js` 开关 toggle + 重建按钮 + i18n 3 键（中英文）+ tauri-stub.js 2 mock + `contextualRetrieval` state。验证：cargo test exit_code 0；clippy Free 零警告；cargo fmt 通过；tsc 0 errors；esbuild 12369 行。零新增依赖（复用已有 `build_contextual_text()` 函数，零 LLM 调用成本）。CURRENT_STATE.md S63 状态 📋 Planning → ✅ Implemented；IPC 命令数 153→155；NEXT_SESSION_PROMPT.md 更新为 S64 文档标签系统。 |
| 2026-08-08 | B05 | Durable Prompt Admission（持久化提示接纳，REQ-RAG-042） | models 4 (TC-MODEL-PENDING-001~004) + storage 5 (TC-ADMIT-001~005) = 9 新增 | 8 文件 | `PendingInput` 数据模型（id/conversation_id/content/delivery/created_at/promoted_seq）+ Storage trait 3 方法默认空操作（admit_input/promote_input/get_pending_inputs）+ SqliteStorage `pending_inputs` 表（6 列 + ON DELETE CASCADE + 2 索引）+ KNOWN_TABLES 13→14 + 3 方法实现（admit INSERT + promote UPDATE 用当前时间戳 + get_pending SELECT steer 优先 FIFO 排序）+ 3 IPC 命令（chat.rs）+ lib.rs 注册（159→162）+ commands/mod.rs PendingInput import + tauri-stub.js 3 mock + pendingInputs state + reset()。SRS REQ-RAG-042 + changelog v6.6.0 + database-schema pending_inputs 表文档。零新增依赖（复用 rusqlite/serde/uuid/chrono）。借鉴 OpenCode Prompt Admission / Promotion 机制。验证：models 32 passed；infra 172 passed；clippy Free+Pro 零警告；fmt 通过；tsc 0 errors；esbuild 12451 行；sync-to-github 8 files。 |
| 2026-08-08 | B06 | Session Run Coordinator（会话运行协调器，REQ-ARCH-009） | core 5 (TC-COORD-001~005) = 5 新增 | 4 文件 | `crates/core/src/session_coordinator.rs` 新建模块：`SessionCoordinator` 结构体（`Arc<Mutex<HashMap>>` 管理 `CoordEntry`）+ `run()` 泛型方法（`Fn() -> Fut + Sync + Send`）+ `run_inner()` 内部方法（`Box::pin` 递归支持 wake 重执行）+ `wake()` 合并唤醒 + `interrupt()` 中断标记 + `is_stopping()` 状态查询 + `active_sessions()` 活跃列表 + 5 TDD 测试（串行/并发/wake合并/interrupt/移除）。`core/src/lib.rs` 注册模块。`state.rs` AppState 新增 `session_coordinator: Arc<SessionCoordinator>` 字段 + 初始化。SRS REQ-ARCH-009 + changelog v6.7.0。零新增依赖（复用 tokio Mutex+Notify）。借鉴 OpenCode `run-coordinator.ts`。设计关键点：Fn（非 FnOnce）允许 wake 重执行、Box::pin 解决 async 递归、Notify 等待通知、Send+Sync 跨线程安全。验证：core 503 passed；clippy Free 零警告；fmt 通过；tsc 0 errors；esbuild 12451 行；sync-to-github 4 files。 |
| 2026-08-09 | S1-S2 | 前端架构升级（Z-index 分层 + 面板栈 + Focus Trap 栈 + State 补全 + 会话切换关闭面板 + 输入区统一 keydown） | Vitest 419 passed (含修复 3 失败) | 新建 4 文件 + 改造 16+ 模块 | **Session 1**：`zindex.js`（12 层级 Z-index 常量）+ `panel-stack.js`（全局面板栈管理器）+ 16 模块改造（security/graph-viewer/dream-panel/code-executor/symbol-search/workflow-panel/confirm-dialog/turn-tree/paywall/settings/command-palette/sidebar/download-manager/keyboard + CSS）。**Session 2**：`focus-trap.js` 改造（全局 `_trapStack` LIFO 栈 + pause/resume）+ `state.js` 新增 6 字段（hybridEnabled/agentEnabled/subAgentEnabled/memoryEnabled/webSearchEnabled/docList）+ `input-toggles.js` 改为 state.js 管理状态（删除局部变量）+ `main.js` `closeAllPanels()` 函数 + `input-keymap.js`（统一 keydown 处理器）+ `input-history.js`（历史导航 + 草稿持久化 + Token 估算）。验证：tsc 0 errors；esbuild 13580 行；Vitest 419 passed。 |
| 2026-08-09 | S3 | 验证收尾 + E2E + 文档同步 | Vitest 419 passed; Rust Free 1028 passed (9 pre-existing failed); Clippy 零警告; fmt 通过; tsc 0 errors; esbuild 13580 行 | 6 文档 | **验证结果**：Vitest 修复 input-toggles.js 3 失败（`if (initialActive)` → 始终 `setState` 同步状态）。Rust 9 集成测试失败全为 pre-existing（许可证/Pro 门控/配额，与前端改造无关）。10 个手动测试场景代码级验证通过（Focus Trap 栈 / Toggle 同步 / closeAllPanels / input-keymap / input-history）。文档更新：frontend-modules.md §11 + AGENTS.md + CLAUDE.md + CURRENT_STATE.md + LOG.md。 |
