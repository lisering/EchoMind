# EchoMind 大简化重构计划 (The Great Simplification)

> **文档版本**: 1.0 ｜ **生成日期**: 2026-08-27 ｜ **作者**: AI Agent 全量复盘
>
> **核心主张**: 停止功能开发，对项目进行全面复盘和深度重构。目标是在保持核心 RAG 价值主张不变的前提下，将代码量削减 50-60%，模块数削减 50%，IPC 命令削减 65%，依赖数削减 40%，文档文件削减 90%。

---

## 一、现状全景评估

### 1.1 量化指标快照

| 维度 | 当前值 | 目标值 | 削减率 |
|---|---|---|---|
| Rust 源码总行数 | ~121,000 行 (272 文件) | ~50,000 行 (~120 文件) | -59% |
| `crates/core` 行数 | 56,577 行 (124 模块) | 拆分为 6-8 个小 crate | 单 crate ≤ 10K 行 |
| `crates/infra` 行数 | 36,679 行 (59 模块) | ~15,000 行 (~25 模块) | -59% |
| 前端 JS 行数 | ~31,000 行 (61 文件) | ~12,000 行 (~25 文件) | -61% |
| E2E 测试行数 | ~33,000 行 (117 文件) | ~12,000 行 (~40 文件) | -64% |
| Rust 测试数 (Free+Pro) | 3,691 (1,700+1,991) | ~1,500 | -59% |
| IPC 命令数 | 228 | ~80 | -65% |
| 依赖总数 | 1,744 (cargo tree) | ~800 | -54% |
| 文档文件数 | 255 个 .md 文件 | ~20 个 | -92% |
| 版本号 | v3.1.0 | v1.0.0-rc.1 | 校准到实际成熟度 |
| CI 构建矩阵 | 4 OS × Free+Pro | 3 OS × Free-only (Pro 独立) | 简化 |

### 1.2 项目规模明细

#### Rust Crate 行数分布

| Crate | 行数 | 模块数 | 问题 |
|---|---|---|---|
| `core` | 56,577 | 124 | 🔴 严重违反 SRP，单 crate 承载了检索/聊天/导入/安全/记忆/评估/图谱/基础设施等 8+ 个领域 |
| `infra` | 36,679 | 59 | 🔴 包含大量过度工程模块 (GEMV 内核/权重重排/层预取等) |
| `tauri-app` | 16,845 | 19 | 🟡 IPC 命令过多 (228)，含大量 get/set 对和无用户场景的借鉴命令 |
| `models` | 4,189 | 1 | 🟢 基本健康，但数据模型过多 (Entity/WikiLink/PendingInput/MemoryRecord 等) |
| `compact` | 3,200 | 1 | 🟢 可接受 |
| `context` | 1,776 | 1 | 🟡 OpenCode 借鉴产物，实际使用场景不明确 |
| `prompt` | 1,667 | 1 | 🟢 基本健康 |
| `license-issuer` | 467 | 1 | 🟢 健康 |
| **合计** | **~121,000** | **272** | |

#### 前端模块行数分布 (Top 10)

| 文件 | 行数 | 问题 |
|---|---|---|
| `settings-advanced.js` | 1,562 | 🔴 设置爆炸的根源 — 13 个子区域 |
| `chat-render.js` | 1,485 | 🟡 大但可接受，渲染逻辑集中 |
| `graph-renderer.js` | 1,095 | 🔴 知识图谱可视化 — 超出本地 RAG 核心范围 |
| `sidebar.js` | 1,070 | 🟡 功能过多导致臃肿 |
| `security.js` | 1,061 | 🟡 安全功能可在后端处理 |
| `chat.js` | 1,057 | 🟢 核心功能 |
| `main.js` | 991 | 🟡 入口文件偏大 |
| `export.js` | 965 | 🟡 导出功能可简化 |
| `context-menu.js` | 850 | 🟡 过度设计 |
| `markdown.js` | 785 | 🟢 核心功能 |

### 1.3 功能矩阵盘点

项目当前包含以下功能域，按"是否属于本地 RAG 知识库核心"分类：

#### 核心 (必保) — 本地 RAG 知识库的最小可用产品

| 功能 | 状态 | 保留理由 |
|---|---|---|
| 文档导入 (md/txt/pdf/docx) | ✅ | 核心入口 |
| 文本分块 (token/semantic) | ✅ | 核心管线 |
| 本地向量化 (fastembed ONNX) | ✅ | 核心管线 |
| SQLite 持久化 + 向量检索 | ✅ | 核心存储 |
| 混合检索 (向量+BM25 RRF) | ✅ | 核心检索 |
| RAG 流式对话 (SSE) | ✅ | 核心交互 |
| 会话管理 + 持久化 | ✅ | 核心体验 |
| BYOK LLM 配置 | ✅ | 核心模式 |
| 文档搜索/筛选/删除 | ✅ | 核心管理 |
| 对话导出 (Markdown) | ✅ | 基本需求 |
| SQLCipher 加密 | ✅ | 隐私承诺 |
| PII 检测/脱敏 | ✅ | 隐私承诺 |
| 浅色/暗色主题 | ✅ | 基本体验 |
| 中英双语 i18n | ✅ | 基本体验 |

#### 有价值 (保留但简化) — 提升体验但非必需

| 功能 | 状态 | 行动 |
|---|---|---|
| HNSW 近似最近邻索引 | ✅ | 保留，核心性能 |
| Cross-Encoder 重排序 | ✅ | 保留，但默认关闭 |
| HyDE 查询改写 | ✅ | 保留，但默认关闭 |
| 流中断 (CancellationToken) | ✅ | 保留 |
| 文件夹同步 | ✅ | 保留 |
| 多知识库 (Workspace) | ✅ | 保留 |
| 标签系统 | ✅ | 保留 |
| Markdown 双向链接 | 🟡 | 简化为纯引用 |
| 深色/浅色主题 | ✅ | 保留 |

#### 过度工程 (删除或归档) — 超出本地 RAG 桌面应用范围

| 功能 | 来源 | 行数估计 | 删除理由 |
|---|---|---|---|
| **Speculative RAG** | 极致性能 S8 | ~1,500 | 学术概念，本地桌面应用无需投机推理 |
| **RAPTOR 多级摘要树** | 极致性能 S6 | ~1,000 | 学术概念，知识库不需要递归摘要 |
| **Proposition 原子分割** | 极致性能 S4 | ~800 | 学术概念，语义分块已足够 |
| **ColBERT 多向量嵌入** | 极致性能 S5 | ~2,000 | Pro 功能，需额外 ONNX 模型，ROI 极低 |
| **渐进式上下文注入** | 极致性能 S7 | ~1,000 | 学术概念，复杂度高收益低 |
| **自进化检索记忆** | 极致性能 S9 | ~1,200 | 学术概念，本地 KB 不需要自进化 |
| **语义缓存金字塔** | 极致性能 S1 | ~2,000 | 过度缓存层，SQLite 查询已足够快 |
| **Prompt 压缩器** | 极致性能 S2 | ~1,000 | RuleBasedCompressor 效果有限 |
| **自研 GEMV 量化内核** | LLM Phase 3 | ~3,000 | 重复 mistral.rs 已有能力 |
| **权重重排** | LLM Phase 3 | ~500 | 同上 |
| **层预取** | LLM Phase 3 | ~500 | 同上 |
| **RAM 预算管理** | LLM Phase 3 | ~500 | 同上 |
| **KV Cache 持久化** | LLM Phase 4 | ~1,000 | 同上 |
| **GGUF 解析器** | LLM Phase 3 | ~1,000 | 同上 |
| **量化块结构** | LLM Phase 3 | ~500 | 同上 |
| **AgentEngine (ReAct)** | REQ-RAG-022 | ~2,000 | 超出核心 RAG，复杂度高 |
| **CoordinatorEngine** | REQ-RAG-025 | ~1,500 | 多 Agent 协调，本地应用不需要 |
| **AutoDream** | REQ-RAG-029 | ~1,500 | 后台整理，无用户需求 |
| **DAG 工作流** | REQ-RAG-030 | ~2,000 | 工作流引擎超出 RAG 范围 |
| **持久化记忆系统** | REQ-RAG-033 | ~2,500 | 三层记忆过度设计 |
| **代码符号搜索** | REQ-RAG-031 | ~1,500 | tree-sitter 代码搜索超出 RAG 范围 |
| **代码执行沙箱** | REQ-RAG-032 | ~1,000 | WasmExecutor 超出 RAG 范围 |
| **RAG 评估系统** | REQ-RAG-045 | ~2,000 | 内部工具非用户功能 |
| **网页搜索融合** | REQ-RAG-036 | ~1,000 | DuckDuckGo 集成超出核心范围 |
| **领域分类器** | REQ-VEC-013 | ~1,000 | 16 域分类过度设计 |
| **知识图谱 (完整)** | REQ-RAG-026/027 | ~5,000 | NER+关系+可视化+分析过于庞大 |
| **对话分支树** | REQ-RAG-039 | ~1,000 | ChatGPT 风格分支超出需求 |
| **会话条带化** | DS4 借鉴 | ~500 | 无用户场景 |
| **Durable Admission** | OpenCode 借鉴 | ~500 | 无用户场景 |
| **Session Coordinator** | OpenCode 借鉴 | ~500 | 无用户场景 |
| **Event Stream** | OpenMontage 借鉴 | ~500 | 无用户场景 |
| **Pipeline Checkpoint** | OpenMontage 借鉴 | ~500 | 无用户场景 |
| **MMR 多样性重排** | OpenMontage 借鉴 | ~500 | 可合并到 retriever |
| **Scored Selector** | OpenMontage 借鉴 | ~500 | LLM Provider 选择无需评分 |
| **LLM Router** | QM 借鉴 | ~500 | 本地+远程路由无需复杂 router |
| **Shadow 安全筛查** | QM 借鉴 | ~500 | 本地应用无需影子筛查 |
| **Security Posture** | QM 借鉴 | ~500 | 三态安全态势过度设计 |
| **LLM Protocol** | OpenCode 借鉴 | ~500 | 多协议抽象层无实际多 Provider |
| **Permission** | OpenCode 借鉴 | ~500 | RBAC 权限系统无多用户场景 |
| **Skill 系统** | OpenCode 借鉴 | ~500 | YAML 技能发现无用户场景 |
| **Prompt Prefix 缓存** | DS4 借鉴 | ~1,000 | KPC 磁盘缓存过度优化 |
| **上下文压缩 VerbatimTail** | DS4 借鉴 | ~500 | 压缩尾部保留过度设计 |
| **TokenEstimator** | QM 借鉴 | ~500 | BPE LRU 缓存过度设计 |
| **heal_dangling_calls** | QM 借鉴 | ~300 | ReAct 悬空修复无 ReAct 则无用 |
| **Context Epoch** | OpenCode 借鉴 | ~1,000 | 纪元管理过度设计 |
| **System Context Registry** | OpenCode 借鉴 | ~1,000 | 5 Source 注册表无用户场景 |
| **MCP 协议适配器** | v3.0 | ~2,000 | Phase 1 已完成但无用户使用 |
| **结构化压缩模板** | OpenCode 借鉴 | ~300 | 6 章节模板无用户感知 |
| **Prompt Caching** | OpenCode 借鉴 | ~500 | 缓存断点标注无实际 LLM 支持 |
| **Late Chunking** | REQ-RAG-049 | ~500 | 上下文感知嵌入无显著收益 |
| **Contextual Retrieval** | REQ-RAG-041 | ~500 | 同上 |
| **语音输入/TTS** | REQ-RAG-034/035 | ~500 | Web Speech API 可选保留 |
| **PDF 导出** | REQ-EXP-005 | ~300 | window.print() 可保留 |
| **快捷指令模板** | REQ-RAG-038 | ~1,000 | 可大幅简化 |
| **.xlsx/.csv 导入** | REQ-ING-021 | ~500 | 可保留但低优先级 |
| **.pptx/.epub 导入** | v1.2 | ~500 | 可保留但低优先级 |
| **自定义 ONNX 模型上传** | REQ-VEC-014 | ~500 | Pro 功能可保留 |
| **嵌入评估** | v3.0 | ~500 | 可合并到测试 |

**过度工程合计**: ~55,000 行 (约占总代码的 45%！)

---

## 二、核心问题诊断

### 问题 1: `crates/core` 是巨型上帝 crate

**现状**: 56,577 行，124 个模块，承载了 8+ 个完全不同的领域逻辑。

**根因**: 项目从 v1.0 开始将所有业务逻辑放在 core crate 中，随着功能增长从未拆分。后续虽然拆出了 `prompt` 和 `compact`，但远远不够。

**影响**:
- 编译时间：修改任何模块触发整个 crate 重编译
- 依赖混乱：import.rs 依赖 retriever.rs 依赖 chat.rs，形成隐式循环
- 测试隔离困难：124 个模块的测试全在一个 crate
- 新开发者无法快速定位代码

### 问题 2: AI Agent 开发反馈循环导致功能膨胀

**现状**: 项目经历了 50+ 个 AI Agent session，每个 session 都添加了新功能或借鉴了其他项目的设计。从未有过"删除"或"简化"的 session。

**根因**: AI Agent 的激励结构偏向"添加"而非"简化"——每个 session 都要产出"成果"，而添加功能是最容易量化的成果。

**影响**:
- 功能数从 v1.0 的 ~15 个核心功能增长到 v3.1.0 的 50+ 个功能
- 其中 ~35 个功能（55,000 行代码）属于过度工程，无真实用户场景
- 维护成本随功能数平方级增长

### 问题 3: 版本号膨胀与现实脱节

**现状**: 版本号 v3.1.0，但 README 写 "v0.1.0-alpha"，无真实用户，无正式发布。

**根因**: 每次 session 都升级版本号，但产品从未真正"发布"过。

**影响**:
- 外部观察者无法判断项目成熟度
- v3.1.0 暗示产品已成熟，但实际是 pre-alpha 质量
- 与 GitHub 发布仓库的版本管理混乱

### 问题 4: IPC 命令爆炸

**现状**: 228 个 IPC 命令，其中：
- ~65 个是借鉴项目的命令，无用户场景
- ~30 对 get/set 命令可合并
- 21 个开发者工具命令
- 19 个性能优化命令全默认关闭

**根因**: 每个"借鉴"或"新功能"session 都新增 IPC 命令，从不删除。

**影响**:
- IPC 契约维护成本极高
- 前端 `ipc.js` 需要维护 228 个方法封装
- E2E mock 需要模拟 228 个命令
- 真正的核心命令被淹没在噪声中

### 问题 5: 前端复杂度超出 Vanilla JS 管理能力

**现状**: 61 个 JS 文件，31,000 行，无框架，esbuild 打包。

**根因**: 拒绝引入任何前端框架，导致每个交互（面板、拖拽、快捷键、主题切换）都从零实现。`settings-advanced.js` 1,562 行就是在无框架下手写复杂表单的结果。

**影响**:
- 前端开发效率极低
- 复杂交互的代码难以维护
- 新功能的前端实现成本高
- Vitest 测试需要大量 mock

### 问题 6: 文档负担过重

**现状**: 255 个 Markdown 文件，4.5MB 文档，每次代码变更需同步更新 5+ 个文档。

**根因**: "铁律七"要求文档实时同步，加上 AI Agent 每个 session 都产出新的规划文档、归档文件、复盘报告。

**影响**:
- 文档维护消耗大量时间
- 文档与代码脱节严重（尽管规则要求同步）
- 新 session 的上下文加载时间极长
- 255 个文件中大部分是历史记录，无实际参考价值

### 问题 7: 测试数量膨胀但质量存疑

**现状**: 3,691 个 Rust 测试 (Free 1,700 + Pro 1,991)，1821 个 Vitest 测试，1355 个 E2E 测试 = ~6,900 个测试。

**根因**: 每个"借鉴"或"学术"功能都附带 TDD 测试。删除功能后测试变成无意义的存在。

**影响**:
- `cargo test` 运行时间长
- 测试维护成本高
- 大量测试测试的是内部实现细节而非用户行为
- 给人虚假的安全感

### 问题 8: 依赖树过于庞大

**现状**: 1,744 个依赖 (cargo tree 节点数)。

**根因**: 引入了 mistral.rs (及其全部 GPU 后端)、tree-sitter、wasm、colbert、pdfium、ocrs 等。

**影响**:
- 首次编译 5-10 分钟
- CI 构建时间长
- 供应链风险面大
- 二进制体积大

### 问题 9: Pro/Free 边界模糊

**现状**: 多个功能在 Pro 和 Free 之间反复迁移 (HNSW 从 Pro 下沉到 Free)，ROI 评估报告存在但边界依然不清晰。

**根因**: 缺乏明确的产品定位——Free 版应该能做什么？Pro 版的价值在哪里？

**影响**:
- 用户困惑
- 代码中 `#[cfg(feature = "pro")]` 分布混乱
- 许可证验证逻辑复杂

### 问题 10: 安全过度设计

**现状**: 5 层安全纵深 (加密→密码→锁屏→擦零→剪贴板) + Shadow 安全筛查 + Security Posture 三态 + PII 7类检测 + 审计哈希链 + 暴力破解防护。

**根因**: 安全功能是 AI Agent 最容易"正确"实现的领域（有明确的测试用例），导致过度堆叠。

**影响**:
- 安全代码占比过高
- 用户体验受影响（过多安全弹窗）
- 安全功能之间有重叠

---

## 三、重构总纲

### 3.1 指导原则

1. **核心优先 (Core First)**: 只保留本地 RAG 知识库的最小可用功能集，其他功能必须有明确的用户场景才能保留
2. **删除优于注释 (Delete > Comment)**: 不确定是否需要的功能直接删除（Git 历史保留），而非注释保留
3. **单 crate ≤ 10K 行**: 任何 crate 超过 10,000 行必须拆分
4. **IPC 最小化**: 每个 IPC 命令必须有明确的用户操作对应
5. **文档精简**: 只保留 SRS、架构文档、API 文档、README——删除所有 session 记录、规划文档、复盘报告
6. **测试对齐用户行为**: 测试应验证用户场景而非内部实现细节
7. **版本校准**: 版本号反映实际成熟度

### 3.2 不变项 (Locked Decisions)

以下决策在重构过程中不变：

| 决策 | 锁定理由 |
|---|---|
| Rust + Tauri v2 | 技术栈基线已锁定 |
| 六边形架构 | 依赖方向铁律 |
| Edition 2024 + async fn in trait | 已验证可行 |
| fastembed ONNX 嵌入 | 本地推理核心 |
| SQLite + rusqlite | 本地存储核心 |
| BYOK 模式 | 隐私承诺 |
| BUSL-1.1 → Apache 2.0 许可证 | 已定 |
| workspace deny lint | 代码质量基线 |

### 3.3 可变项 (Open for Change)

| 决策 | 讨论方向 |
|---|---|
| Vanilla JS vs 轻量框架 | 评估引入 Preact/Svelte 的 ROI |
| Pro 功能范围 | 大幅缩小 Pro 范围，只保留本地 LLM + PDF 多模态 |
| 前端构建方式 | esbuild 已足够，但打包策略可调整 |
| 测试策略 | 从"全覆盖"转向"核心路径覆盖" |
| 文档体系 | 从"全量同步"转向"关键文档同步" |

---

## 四、分阶段行动计划

### Phase 0: 冻结与基线 (1 Session)

**目标**: 建立重构基线，冻结当前状态

**任务**:
1. 创建 `release/v3.1.0-final` git tag 作为重构前最后一个快照
2. 运行全量测试并记录基线结果
3. 备份当前所有代码到 `archive/pre-refactor/` 分支
4. 生成当前代码的依赖分析报告
5. 创建重构专用分支 `refactor/great-simplification`

**产出**:
- `docs/refactoring/BASELINE.md` — 基线指标快照
- `docs/refactoring/DEPENDENCY_AUDIT.md` — 依赖审计报告
- git tag `v3.1.0-final`

---

### Phase 1: 功能修剪 (3-4 Sessions)

**目标**: 删除所有过度工程功能，削减 ~55,000 行代码

#### 1.1 删除学术 RAG 优化 (Session R1)

删除以下模块及其测试：

| 模块 | 文件 | 估计行数 |
|---|---|---|
| SpeculativeRagEngine | `crates/core/src/speculative_rag.rs` + tests | ~1,500 |
| SummaryTreeBuilder | `crates/core/src/summary_tree.rs` + tests | ~1,000 |
| PropositionSplitter | `crates/core/src/proposition_splitter.rs` + tests | ~800 |
| ProgressiveInjector | `crates/core/src/progressive_injector.rs` + tests | ~1,000 |
| RetrievalMemoryEngine | `crates/core/src/retrieval_memory.rs` + tests | ~1,200 |
| RuleBasedCompressor | `crates/core/src/prompt_compressor.rs` + tests | ~1,000 |
| ResponseCache (L0/L1/L3) | `crates/core/src/cache.rs` + `crates/infra/src/sqlite_cache.rs` + tests | ~2,000 |
| DomainClassifier | `crates/core/src/domain.rs` + tests | ~1,000 |
| LateChunking | `crates/core/src/late_chunking.rs` + tests | ~500 |
| ContextualRetrieval | (整合在 retriever 中) | ~500 |
| EmbedComparison | `crates/core/src/embed_comparison.rs` + tests | ~500 |

**预计削减**: ~10,000 行

**清理范围**:
- 删除 `crates/infra/src/sqlite_cache.rs` + tests
- 删除 `crates/infra/src/onnx_compressor.rs` (Pro)
- 删除 `crates/infra/src/colbert_embedder.rs` + tests (Pro)
- 清理 `crates/tauri-app/src/commands/performance.rs` 中相关 IPC 命令 (~15 个)
- 清理 `ui/src/perf-settings.js` 前端模块
- 清理相关设置键 (~20 个)
- 清理相关 E2E 测试

#### 1.2 删除过度工程 Agent 功能 (Session R2)

| 模块 | 文件 | 估计行数 |
|---|---|---|
| AgentEngine (ReAct) | `crates/core/src/agent.rs` + `agent_run.rs` + tests | ~3,000 |
| CoordinatorEngine | `crates/core/src/coordinator.rs` + `coordinator_strategy.rs` + tests | ~2,500 |
| AutoDream | `crates/core/src/auto_dream.rs` + tests | ~1,500 |
| WorkflowEngine | `crates/core/src/workflow.rs` + tests | ~2,000 |
| MemoryStore (三层记忆) | `crates/core/src/memory_store.rs` + tests | ~2,500 |
| SessionStrip | `crates/core/src/session_strip.rs` + tests | ~500 |
| SubAgent | `crates/core/src/sub_agent.rs` + tests | ~500 |
| Hooks / TypedHook | `crates/core/src/hooks.rs` + `typed_hook.rs` + tests | ~500 |

**预计削减**: ~13,000 行

#### 1.3 删除过度工程 LLM 功能 (Session R3)

| 模块 | 文件 | 估计行数 |
|---|---|---|
| GEMV 内核 | `crates/infra/src/gemv_kernel.rs` + tests | ~1,500 |
| 量化块结构 | `crates/infra/src/quant_blocks.rs` + tests | ~500 |
| GGUF 解析器 | `crates/infra/src/gguf_reader.rs` + tests | ~1,000 |
| 权重重排 | `crates/infra/src/weight_repack.rs` + tests | ~500 |
| 层预取 | `crates/infra/src/layer_prefetch.rs` + tests | ~500 |
| RAM 预算 | `crates/infra/src/memory_budget.rs` + tests | ~500 |
| KV Cache | `crates/infra/src/kv_cache.rs` + tests | ~1,000 |
| Prompt Prefix 缓存 | `crates/infra/src/prompt_cache.rs` + tests | ~1,000 |
| Robust Downloader | `crates/infra/src/robust_downloader.rs` | ~1,500 |
| Mistral.rs 适配层 | `crates/infra/src/local_llm.rs` + tests | ~3,000 |
| ModelManager | `crates/infra/src/model_manager.rs` + tests | ~1,000 |

**预计削减**: ~12,000 行

**注意**: 本地 LLM 推理保留基础能力 (通过 mistral.rs 直接调用)，删除自研量化内核和优化层。

#### 1.4 删除过度工程工具与借鉴 (Session R4)

| 模块 | 文件 | 估计行数 |
|---|---|---|
| SymbolEngine (tree-sitter) | `crates/infra/src/symbol_engine.rs` + tests | ~1,500 |
| WasmExecutor | `crates/infra/src/wasm_executor.rs` + tests | ~1,000 |
| RagEvaluator | `crates/core/src/rag_eval.rs` + `rag_eval_dataset.rs` + tests | ~2,500 |
| GraphExport | `crates/core/src/graph_export.rs` + tests | ~500 |
| GraphAnalyzer | `crates/core/src/graph_analyzer.rs` + tests | ~1,000 |
| GraphRetriever | `crates/core/src/graph_retriever.rs` + tests | ~1,000 |
| EntityExtractor (完整 NER) | `crates/core/src/entity_extractor.rs` + tests | ~2,000 |
| DuckDuckGoProvider | `crates/infra/src/duckduckgo_provider.rs` | ~500 |
| WebSearch | `crates/core/src/web_search.rs` + tests | ~500 |
| MCP Transport | `crates/infra/src/mcp_transport.rs` | ~500 |
| MCP Commands | `crates/tauri-app/src/commands/mcp.rs` | ~500 |
| Skill 系统 | `crates/core/src/skill.rs` | ~300 |
| LLM Protocol | `crates/core/src/llm_protocol.rs` | ~500 |
| Permission | `crates/core/src/permission.rs` | ~500 |
| LLM Router | `crates/core/src/llm_router.rs` + tests | ~500 |
| Event Stream | `crates/core/src/event_stream.rs` | ~500 |
| Pipeline Checkpoint | `crates/core/src/pipeline_checkpoint.rs` | ~500 |
| MMR Diversifier | `crates/core/src/mmr_diversifier.rs` | ~500 |
| Quality Gate | `crates/core/src/quality_gate.rs` + `retrieval_quality_gate.rs` | ~1,000 |
| Scored Selector | `crates/core/src/scored_selector.rs` | ~500 |
| Tool Output | `crates/core/src/tool_output.rs` | ~300 |
| Portable Tool | `crates/core/src/portable_tool.rs` | ~300 |
| Budget Tracker | `crates/core/src/budget.rs` + tests | ~500 |
| Trace | `crates/core/src/trace.rs` | ~300 |
| Active Gate | `crates/core/src/active_gate_tests.rs` | ~300 |
| Memory Policy | `crates/core/src/memory_policy.rs` | ~300 |
| Finish Reason | `crates/core/src/finish_reason.rs` + tests | ~500 |
| Update Check | `crates/core/src/update_check.rs` | ~300 |
| Step Cache | `crates/core/src/step_cache.rs` + tests | ~500 |
| Idempotency | `crates/core/src/idempotency.rs` + tests | ~500 |
| Concurrency (KeyedQueue) | `crates/core/src/concurrency.rs` + tests | ~500 |
| Session Coordinator | `crates/core/src/session_coordinator.rs` | ~500 |
| Audit Pair | `crates/core/src/audit_pair.rs` + tests | ~500 |
| Audit Engine | `crates/core/src/audit.rs` + tests | ~1,000 |
| Security (Shadow/Posture) | `crates/core/src/security.rs` 中的 Shadow/Posture 部分 | ~1,000 |
| Context crate (全部) | `crates/context/` | ~1,800 |
| Compact crate (部分) | `crates/compact/` 中的 TokenEstimator + heal_dangling + VerbatimTail | ~1,000 |
| Prompt crate (部分) | `crates/prompt/` 中的 CachePolicy | ~500 |

**预计削减**: ~25,000 行

#### 1.5 删除前端过度工程模块

| 模块 | 文件 | 估计行数 |
|---|---|---|
| perf-settings.js | `ui/src/perf-settings.js` | ~500 |
| graph-viewer.js + graph-renderer.js | `ui/src/graph-*.js` | ~1,500 |
| workflow-panel.js | (已删除) | 0 |
| code-executor.js | (已删除) | 0 |
| dream-panel.js | `ui/src/dream-panel.js` (如果仍存在) | ~500 |
| memory-panel.js | `ui/src/memory-panel.js` (如果仍存在) | ~500 |
| symbol-search.js | `ui/src/symbol-search.js` (如果仍存在) | ~500 |
| rag-eval.js | `ui/src/rag-eval.js` (如果仍存在) | ~500 |
| mcp 面板 | `ui/src/mcp-*.js` | ~500 |

**预计削减**: ~3,500 行

**Phase 1 合计削减**: ~63,500 行 (Rust ~55,000 + 前端 ~3,500 + 测试 ~5,000)

---

### Phase 2: Core Crate 拆分 (2-3 Sessions)

**目标**: 将 56K 行的 `crates/core` 拆分为 6-8 个聚焦的小 crate

#### 拆分方案

```
crates/
  models/          (不变)  ~3,000 行
  prompt/          (精简)  ~1,000 行 — 仅保留 SegmentedPrompt + RAG prompt
  compact/         (精简)  ~1,000 行 — 仅保留 CompactionEngine 核心
  core/            (残余)  ~3,000 行 — 仅保留 port traits + errors + stream_parse
  retrieval/       (新)    ~5,000 行 — VectorRetriever + HybridRetriever + HNSW 接口
  chat/            (新)    ~3,000 行 — ChatEngine + 流式管道
  import/          (新)    ~4,000 行 — Loader + Splitter + ImportService
  security/        (新)    ~3,000 行 — SecurityManager + Encryption + Privacy + AuditLogger
  sync/            (新)    ~1,000 行 — SyncEngine + FileWatcher 接口
  export/          (新)    ~500 行 — ExportService
  infra/           (精简)  ~10,000 行 — SqliteStorage + LocalEmbedder + OpenAIProvider + HNSW + 基础设施
  tauri-app/       (精简)  ~8,000 行 — ~80 IPC 命令
  license-issuer/  (不变)  ~500 行
```

#### 依赖方向 (重构后)

```
models → prompt → core (ports only)
                  ↑
  retrieval, chat, import, security, sync, export → core
                          ↑
                    infra → core + (retrieval/chat/import/security/sync/export)
                          ↑
                    tauri-app → 全部
```

**关键设计**: `core` crate 退化为纯端口定义 crate (~3,000 行)，不含任何业务逻辑。

---

### Phase 3: IPC 命令精简 (1-2 Sessions)

**目标**: 228 个 IPC 命令精简到 ~80 个

#### 精简策略

| 类别 | 当前 | 目标 | 策略 |
|---|---|---|---|
| 聊天相关 | 17 | 8 | 删除 agent/coordinator/dream 相关命令 |
| 文档导入 | 11 | 6 | 合并 wiki_link 命令 |
| 文档管理 | 19 | 8 | 删除标签/预览/摘要等非核心 |
| 会话管理 | 25 | 8 | 删除 strip/分支树 |
| 设置 | 42 | 5 | 合并为统一 get_settings/set_settings |
| 安全 | 22 | 8 | 删除 posture/shadow/screen |
| 本地 LLM | 23 | 5 | 删除自研内核相关 |
| 性能优化 | 23 | 0 | 全部删除 |
| 图谱 | 9 | 0 | 全部删除 |
| Agent | 3 | 0 | 全部删除 |
| 审计 | 3 | 2 | 保留基本审计 |
| 许可证 | 3 | 3 | 不变 |
| 同步 | 4 | 2 | 简化 |
| MCP | 6 | 0 | 全部删除 |
| RAG Eval | 4 | 0 | 全部删除 |
| Trace | 4 | 0 | 全部删除 |
| Workspace | 9 | 4 | 简化 |
| 其他 | 21 | ~15 (dev tools debug-only) | 简化 |
| **合计** | **228** | **~80** | -65% |

#### 统一设置 API 设计

将 ~30 对 get/set 命令合并为 2 个统一命令：

```rust
// 统一设置读取
#[tauri::command]
async fn get_settings(keys: Vec<String>, state: State<'_, AppState>) -> Result<HashMap<String, Value>, String>

// 统一设置写入
#[tauri::command]
async fn set_settings(updates: HashMap<String, Value>, state: State<'_, AppState>) -> Result<(), String>
```

前端调用变为：
```js
// 之前：10 个不同 IPC 调用
await invoke('set_hybrid_search', { enabled: true });
await invoke('set_rerank_enabled', { enabled: true });
await invoke('set_hyde_enabled', { enabled: false });

// 之后：1 个调用
await invoke('set_settings', { updates: { 'rag.hybrid_search': true, 'rag.rerank': true, 'rag.hyde': false } });
```

---

### Phase 4: 前端简化 (2-3 Sessions)

**目标**: 61 个 JS 文件 → ~25 个文件，31K 行 → ~12K 行

#### 4.1 删除过度工程前端模块 (Session R6)

删除以下前端模块（对应后端功能删除）：
- `perf-settings.js` — 性能设置面板
- `graph-viewer.js` + `graph-renderer.js` — 知识图谱可视化
- `dream-panel.js` — AutoDream 面板
- `memory-panel.js` — 记忆管理面板
- `symbol-search.js` — 代码符号搜索
- `rag-eval.js` — RAG 评估面板
- `mcp-*.js` — MCP 面板
- `workflow-panel.js` — 工作流面板（如仍存在）

#### 4.2 设置面板重整 (Session R7)

当前：35 个分区 → 目标：5 个分区

| 分区 | 内容 | 模块 |
|---|---|---|
| **模型配置** | LLM Provider / API Key / Base URL / Model / 连接测试 | `settings-llm.js` |
| **外观与语言** | 主题切换 / 语言切换 | `settings-appearance.js` |
| **知识库管理** | 嵌入模型选择 / 分块参数 / 文件夹同步 | `settings-knowledge.js` |
| **安全与隐私** | 数据库加密 / 自动锁屏 / PII 检测 / 剪贴板清除 | `settings-security.js` |
| **高级** | 重排序开关 / HyDE 开关 / 混合搜索开关 / 导入导出 | `settings-advanced.js` |

`settings-advanced.js` 从 1,562 行精简到 ~300 行（5 个开关 + 2 个导入导出按钮）。

#### 4.3 前端架构评估 (Session R8)

评估是否引入轻量框架：

| 方案 | 优点 | 缺点 | 推荐 |
|---|---|---|---|
| 保持 Vanilla JS | 零依赖 | 复杂交互难维护 | ❌ |
| Preact (3KB) | 轻量、API 友好 | 需要迁移 | ✅ 可选 |
| Svelte (编译时) | 零运行时、响应式 | 需要学习 | 🟡 备选 |
| 保持但引入状态管理库 | 最小改动 | 增加复杂度 | ❌ |

**推荐**: 保持 Vanilla JS + esbuild 打包，但引入以下改进：
- 使用 ES2022+ 特性简化代码（可选链、空值合并等）
- 提取通用 UI 组件到 `ui/src/components/` 目录
- 简化 `main.js` 的入口逻辑

---

### Phase 5: 依赖削减 (1-2 Sessions)

**目标**: 1,744 个依赖 → ~800 个

#### 5.1 删除的依赖

| 依赖 | 来源 | 替代方案 |
|---|---|---|
| `mistral.rs` (及 GPU 后端) | 本地 LLM Phase 1-4 | 保留基础推理，删除 GPU 后端 |
| `tree-sitter` 系列 | SymbolEngine | 删除 |
| `colbert` 相关 | ColBERT 嵌入 | 删除 |
| `pdfium-render` | PDF 渲染 | 保留 (PDF 导入需要) |
| `ocrs` + `rten` | OCR | 评估是否保留 (Pro) |
| `d3` (前端) | 知识图谱可视化 | 删除 |
| `mermaid` (前端) | 图表渲染 | 评估是否保留 |
| `katex` (前端) | 数学公式 | 保留 |
| `chart.js` (前端) | 数据可视化 | 删除 |
| `sha1` | Prompt Prefix 缓存 | 删除 |
| `notify` + `notify-debouncer-full` | 文件夹同步 | 保留 |
| `pptx-to-md` | .pptx 导入 | 保留 |
| `rbook` | .epub 导入 | 保留 |
| `calamine` | .xlsx 导入 | 保留 |
| `csv` | .csv 导入 | 保留 |
| `html2text` | .html 导入 | 保留 |
| `docx-rs` | .docx 导入 | 保留 |
| `lopdf` | PDF 文本提取 | 保留 |
| `half` | f16 量化 | 删除 (随量化内核删除) |
| `memmap2` | GGUF mmap | 删除 (随 GGUF 解析器删除) |
| `libc` | sysconf/madvise | 删除 (随层预取删除) |

#### 5.2 Pro 功能重新定义

重构后 Pro 版只包含：

| Pro 功能 | 理由 | 代码量 |
|---|---|---|
| 本地 LLM 推理 (mistral.rs 基础) | 核心差异化 | ~3,000 行 |
| PDF 多模态导入 (pdfium + OCR + VLM) | 高价值功能 | ~3,000 行 |
| 自定义 ONNX 嵌入模型上传 | 高级用户需求 | ~500 行 |
| 无文件数量限制 | 商业模式 | ~0 行 (仅配置) |

**删除的 Pro 功能**: ColBERT、GEMV、量化内核、SymbolEngine、WasmExecutor、ONNX Compressor 等全部删除。

---

### Phase 6: 测试重整 (1-2 Sessions)

**目标**: 6,900 个测试 → ~2,500 个，聚焦用户行为

#### 6.1 测试策略调整

| 层级 | 当前 | 目标 | 原则 |
|---|---|---|---|
| Rust 单元测试 | ~3,700 | ~1,200 | 只测试核心逻辑：导入/检索/聊天/安全 |
| Vitest | ~1,800 | ~600 | 只测试前端交互：聊天/导入/设置/导出 |
| E2E Bridge | ~1,400 | ~500 | 只测试核心用户旅程：5 条主线 |
| E2E GUI | (废弃) | 0 | 删除 wdio |

#### 6.2 测试分层

**核心路径测试** (必须 100% 覆盖):
1. 导入文档 → 分块 → 向量化 → 列表显示
2. 输入问题 → 检索 → RAG 回答 → 引用显示
3. 流式中断 → 部分保存
4. 数据库加密 → 锁屏 → 解锁
5. 多知识库切换

**非核心路径** (有测试即可，不强求覆盖):
- 设置面板交互
- 主题切换
- 文件夹同步
- 导出

---

### Phase 7: 文档重置 (1 Session)

**目标**: 255 个 Markdown 文件 → ~20 个

#### 7.1 保留文档

| 文档 | 理由 |
|---|---|
| `docs/SRS_v1.0.md` (精简版) | 需求规格 |
| `docs/architecture/GREAT_SIMPLIFICATION_PLAN.md` (本文档) | 重构计划 |
| `docs/api/ipc-commands.md` (精简版) | IPC 契约 |
| `docs/api/code-contracts.md` (精简版) | Trait 契约 |
| `docs/api/database-schema.md` (精简版) | DB Schema |
| `docs/api/settings-reference.md` (精简版) | 设置键 |
| `README.md` + `README.zh-CN.md` | 项目入口 |
| `AGENTS.md` (精简版) | Agent 指南 |
| `.catpaw/rules/system-constraints.md` | 铁律规则 |
| `PROJECT_RULES.md` (精简版) | 项目宪法 |
| `CHANGELOG.md` (合并版) | 变更日志 |

#### 7.2 归档文档

以下文档移入 `docs/archive/`：

- `docs/sessions/` 全部内容（CURRENT_STATE.md / LOG.md / archive/ / NEXT_SESSION_*.md）
- `docs/architecture/V1.*_ATOMIC_PLAN.md`
- `docs/architecture/V2.*_ATOMIC_PLAN.md`
- `docs/architecture/V3.*_ATOMIC_PLAN.md`
- `docs/architecture/OPENCODE_BORROW_ATOMIC_PLAN.md`
- `docs/architecture/QM_BORROW_ATOMIC_PLAN.md`
- `docs/architecture/EXTREME_PERFORMANCE_ATOMIC_PLAN.md`
- `docs/architecture/DEEPSEEK_UI_ATOMIC_PLAN.md`
- `docs/architecture/PRO_ROI_EVALUATION.md`
- `docs/architecture/CHERRY_STUDIO_ANALYSIS.md`
- `docs/architecture/OPENCODE_BORROW_ATOMIC_PLAN.md`
- `docs/architecture/CLAUDE_CODE_REVERSE_BORROW_ATOMIC_PLAN.md`
- 所有 `CHANGELOG_v*.md`
- 所有 ADR (15 个) → 保留 ADR-001 到 ADR-005，其余归档

#### 7.3 删除文档

- 所有 session 归档文件（已归档到 `docs/archive/` 的不再保留活跃路径）
- 所有复盘报告
- 所有借鉴分析报告
- `docs/SOFTWARE_ASSET.md` (1100+ 行，重构后重新编写简洁版)

---

### Phase 8: 版本校准与发布准备 (1-2 Sessions)

**目标**: 版本号从 v3.1.0 校准到 v1.0.0-rc.1

#### 8.1 版本号变更

| 文件 | 当前 | 目标 |
|---|---|---|
| `tauri.conf.json` | 3.1.0 | 1.0.0-rc.1 |
| 8 个 `Cargo.toml` | 3.1.0 | 1.0.0-rc.1 |
| README.md | "v0.1.0-alpha" | "v1.0.0-rc.1" |
| README.zh-CN.md | "v0.1.0-alpha" | "v1.0.0-rc.1" |

#### 8.2 发布前验证清单

- [ ] `cargo build` 成功 (Free)
- [ ] `cargo build --features pro` 成功 (Pro)
- [ ] `cargo clippy --all-targets -- -D warnings` 零警告
- [ ] `cargo fmt --check` 通过
- [ ] `npx tsc --noEmit` 零错误
- [ ] `node scripts/build-ui.mjs` 成功
- [ ] `cargo test` 全量通过 (目标 ~1,200 passed)
- [ ] `cd e2e-tests && npm run test:unit` 通过 (目标 ~600 passed)
- [ ] `cd e2e-tests && npm run test:bridge` 通过 (目标 ~500 passed)
- [ ] Release build 成功
- [ ] `cargo audit` + `cargo deny check` 通过
- [ ] 二进制体积: macOS dmg ≤ 30MB (不含 ONNX 模型)
- [ ] `./sync-to-github.sh --push` 成功
- [ ] Git tag `v1.0.0-rc.1` 创建

---

## 五、重构后目标架构

### 5.1 Crate 结构 (重构后)

```
crates/
  models/           ~3,000 行   数据模型契约
  prompt/           ~1,000 行   提示词构建
  compact/          ~1,000 行   上下文压缩
  core/             ~3,000 行   端口 Trait 定义 (纯接口)
  retrieval/        ~5,000 行   检索引擎 (向量+混合+HNSW)
  chat/             ~3,000 行   聊天引擎 (流式+会话)
  import/           ~4,000 行   导入管线 (Loader+Splitter+ImportService)
  security/         ~3,000 行   安全层 (加密+PII+审计)
  sync/             ~1,000 行   文件夹同步
  export/           ~500 行     对话导出
  infra/            ~10,000 行  适配器 (Sqlite+Embedder+OpenAI+HNSW+LocalLLM)
  tauri-app/        ~8,000 行   Tauri 外壳 (~80 IPC 命令)
  license-issuer/   ~500 行     License CLI
```

**总计**: ~43,000 行 Rust 代码 (从 121K 削减 64%)

### 5.2 前端结构 (重构后)

```
ui/src/
  main.js           ~500 行    入口
  state.js          ~300 行    全局状态
  ipc.js            ~300 行    IPC 封装 (统一 settings API)
  chat.js           ~600 行    聊天逻辑
  chat-render.js    ~800 行    消息渲染
  sidebar.js        ~500 行    侧栏 (知识库列表 + 会话列表)
  markdown.js       ~400 行    Markdown 渲染
  settings-*.js     ~1,200 行  5 个设置分区 (每个 ~240 行)
  wizard.js         ~400 行    首启向导
  i18n.js           ~200 行    国际化
  export.js         ~300 行    导出
  security.js       ~500 行    安全 UI
  import.js         ~300 行    导入 UI
  utils.js          ~300 行    工具函数
  toast.js          ~100 行    通知
  lazy-loader.js    ~200 行    懒加载
  help-panel.js     ~300 行    帮助面板
  keyboard.js       ~200 行    快捷键
```

**总计**: ~7,200 行 JS (从 31K 削减 77%)

### 5.3 IPC 命令 (重构后 ~80 个)

```
聊天 (8):
  chat, abort_chat, get_messages, delete_message, 
  edit_message, regenerate_message, get_conversations, delete_conversation

文档 (14):
  import_files, abort_import, get_documents, delete_document,
  get_document_detail, rebuild_embeddings, get_document_chunks,
  search_documents, list_documents_paginated, 
  create_workspace, list_workspaces, switch_workspace, 
  rename_workspace, delete_workspace

设置 (5):
  get_settings, set_settings, get_api_key_masked, 
  test_llm_connection, get_llm_models

安全 (8):
  get_security_status, encrypt_database, unlock_database, 
  lock_app, set_auto_lock_timeout, detect_pii, 
  get_audit_logs, verify_audit_chain

本地 LLM (5):
  set_llm_mode, get_llm_mode, download_model, 
  list_models, delete_model

同步 (2):
  watch_folder, sync_folder

导出 (2):
  export_conversation, get_export_formats

许可证 (3):
  activate_pro, deactivate_pro, check_license

其他 (3):
  get_app_info, open_external, check_for_update

合计: 50 核心 + ~30 辅助 = ~80
```

### 5.4 依赖 (重构后)

| 类别 | 保留 | 删除 |
|---|---|---|
| Tauri v2 | ✅ | |
| rusqlite + r2d2 | ✅ | |
| fastembed | ✅ | |
| mistral.rs (基础) | ✅ (Pro) | GPU 后端可选 |
| reqwest | ✅ | |
| serde / serde_json | ✅ | |
| tokio | ✅ | |
| regex | ✅ | |
| tiktoken-rs | ✅ | |
| lopdf | ✅ | |
| docx-rs | ✅ | |
| html2text | ✅ | |
| pptx-to-md | ✅ | |
| rbook | ✅ | |
| calamine | ✅ | |
| csv | ✅ | |
| argon2 | ✅ | |
| notify | ✅ | |
| ed25519-dalek | ✅ | |
| sha2 | ✅ | |
| hnsw_rs | ✅ | |
| tracing | ✅ | |
| pdfium-render (Pro) | ✅ | |
| ocrs (Pro) | ✅ | |
| tree-sitter | | ❌ 删除 |
| colbert | | ❌ 删除 |
| half | | ❌ 删除 |
| memmap2 | | ❌ 删除 |
| libc | | ❌ 删除 |
| sha1 | | ❌ 删除 |
| mermaid (前端) | | ❌ 评估后决定 |
| chart.js (前端) | | ❌ 删除 |
| d3 (前端) | | ❌ 删除 |

**预计依赖数**: ~600-800 (从 1,744 削减 ~55%)

---

## 六、风险评估

### 6.1 高风险

| 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|
| 删除功能后发现需要 | 中 | 中 | Git 历史保留，可恢复；分阶段删除便于回滚 |
| Core crate 拆分导致循环依赖 | 高 | 高 | 先定义清晰的依赖方向，逐 crate 迁移 |
| 前端删除模块导致 import 断裂 | 高 | 中 | esbuild 会报错，逐文件删除修复 |
| 测试删除过多导致回归 | 中 | 高 | 保留核心路径测试，删除前确认无其他测试依赖 |
| IPC 删除导致前端崩溃 | 高 | 中 | 同步删除前端 IPC 调用，esbuild 会报错 |

### 6.2 中风险

| 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|
| 版本号回退导致用户困惑 | 低 | 低 | 无真实用户，影响可忽略 |
| 文档删除后丢失历史决策 | 中 | 低 | Git 历史保留全部，archive 分支保留快照 |
| Pro 功能删除导致 Pro 价值降低 | 中 | 中 | 只保留核心 Pro (本地 LLM + PDF 多模态) |
| CI 需要重新配置 | 中 | 中 | CI 配置随代码精简而简化 |

### 6.3 低风险

| 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|
| 依赖删除导致编译失败 | 低 | 低 | cargo 会报错，逐个删除修复 |
| 数据库 schema 变更 | 低 | 中 | 不涉及 schema 删除，只删除相关查询代码 |

---

## 七、执行时间表

| Phase | 内容 | Sessions | 预计天数 |
|---|---|---|---|
| Phase 0 | 冻结与基线 | 1 | 1 天 |
| Phase 1 | 功能修剪 (R1-R5) | 5 | 5-7 天 |
| Phase 2 | Core crate 拆分 | 3 | 3-4 天 |
| Phase 3 | IPC 命令精简 | 2 | 2 天 |
| Phase 4 | 前端简化 | 3 | 3-4 天 |
| Phase 5 | 依赖削减 | 2 | 2 天 |
| Phase 6 | 测试重整 | 2 | 2 天 |
| Phase 7 | 文档重置 | 1 | 1 天 |
| Phase 8 | 版本校准与发布 | 2 | 2 天 |
| **合计** | | **21** | **23-27 天** |

---

## 八、验收标准

重构完成的标志：

### 8.1 量化指标

| 指标 | 目标 | 验证方法 |
|---|---|---|
| Rust 代码总行数 | ≤ 50,000 | `find crates -name '*.rs' -exec cat {} + \| wc -l` |
| 最大单 crate 行数 | ≤ 10,000 | 逐 crate 统计 |
| 前端 JS 总行数 | ≤ 12,000 | `find ui/src -name '*.js' -exec cat {} + \| wc -l` |
| IPC 命令数 | ≤ 80 | `grep -c '#\[tauri::command\]' crates/tauri-app/src/commands/*.rs` |
| 依赖总数 | ≤ 800 | `cargo tree \| wc -l` |
| Rust 测试数 | ~1,500 | `cargo test 2>&1 \| grep "test result:"` |
| Vitest 测试数 | ~600 | `cd e2e-tests && npm run test:unit` |
| E2E 测试数 | ~500 | `cd e2e-tests && npx playwright test` |
| 文档文件数 | ≤ 20 | `find docs -name '*.md' \| wc -l` |
| CI 全绿 | ✅ | GitHub Actions |

### 8.2 质量指标

| 指标 | 目标 | 验证方法 |
|---|---|---|
| Clippy 零警告 | ✅ | `cargo clippy --all-targets -- -D warnings` |
| cargo fmt 通过 | ✅ | `cargo fmt --check` |
| TypeScript 零错误 | ✅ | `npx tsc --noEmit` |
| esbuild 构建成功 | ✅ | `node scripts/build-ui.mjs` |
| Release build 成功 | ✅ | `cargo build --release --features pro` |
| 无 unwrap/expect/panic | ✅ | workspace deny lint |
| 核心路径测试 100% | ✅ | 5 条核心路径全通过 |

### 8.3 功能完整性

重构后必须能完成以下核心用户旅程：

1. ✅ 首次启动 → 配置向导 → LLM 配置 → 连接测试 → 进入主界面
2. ✅ 拖拽文件 → 解析 → 分块 → 向量化 → 知识库列表显示
3. ✅ 输入问题 → 检索 → RAG 回答 → 引用显示 → 流式中断
4. ✅ 加密数据库 → 锁屏 → 解锁 → PII 脱敏
5. ✅ 切换知识库 → 导出对话 → 主题切换 → 语言切换

---

## 九、关键决策记录

### 决策 1: 不引入前端框架

**理由**: 虽然前端复杂度高，但引入框架的迁移成本 > 收益。通过删除过度工程模块和精简设置面板，Vanilla JS 可以管理 ~12K 行代码。esbuild 打包 IIFE 已足够。

**替代方案**: 如果未来需要，考虑 Preact (3KB) 作为渐进迁移。

### 决策 2: 保留 mistral.rs 本地 LLM (基础版)

**理由**: 本地 LLM 是 Pro 版的核心差异化。但删除自研量化内核 (GEMV/权重重排/层预取/KV Cache) — 这些是过度工程，mistral.rs 自身的推理能力已足够。

### 决策 3: 知识图谱功能大幅简化

**理由**: 完整的 NER + 关系抽取 + 可视化 + 分析 (BFS/社区检测) 超出了本地 RAG 的核心范围。保留简单的实体引用（在检索结果中高亮实体），删除完整的图谱功能。

### 决策 4: 删除所有"借鉴"功能

**理由**: QM/OpenCode/OpenMontage/DS4/Cherry Studio 的借鉴功能都是其他项目的设计模式，在 EchoMind 中没有对应的用户场景。这些模块存在但从未被真正使用。

### 决策 5: 版本号回退到 v1.0.0-rc.1

**理由**: v3.1.0 暗示了一个成熟的产品，但实际上项目无真实用户、无正式发布。回退到 rc.1 更准确地反映项目处于"发布候选"阶段。

---

## 十、总结

EchoMind 在 50+ 个 AI Agent session 中积累了大量功能和技术债。核心问题不在于功能不够，而在于**功能太多**——121K 行代码中约 55K 行（45%）属于过度工程。

这次"大简化"重构的目标是回归本质：**一个本地优先的 RAG 知识库桌面应用**。

通过 8 个 Phase、~21 个 Session 的重构，将代码量削减 60%，模块数削减 50%，IPC 命令削减 65%，使项目回到可维护、可理解、可发布的状态。

重构后的 EchoMind 将是一个精炼的、专注于核心价值的产品：

> **Rust 极速 · 隐私不出域 · BYOK 买断制**

---

> **本计划文档路径**: `docs/architecture/GREAT_SIMPLIFICATION_PLAN.md`
>
> **生成人**: AI Agent (全量复盘 session, 2026-08-27)
>
> **下一步**: Phase 0 — 冻结基线，创建重构分支