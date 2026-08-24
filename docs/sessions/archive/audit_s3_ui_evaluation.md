# EchoMind UI 界面验收评估报告（S3）

> **审计日期**: 2026-08-13
> **审计范围**: 全量 UI 界面元素 — 设置面板、输入区 toggle、侧栏/顶栏按钮、对话区、右键菜单、首启向导、错误提示
> **审计方法**: 从用户视角逐界面审查交互简洁度和用户旅程，只审计不写代码
> **评估标准**: 10 条（S1-S10）
> **核心理念**: 使用越简单，功能越强大

---

## 0. 审计摘要

### 0.1 评估总览

| 编号 | 标准 | 当前 | 目标 | 状态 |
|---|---|---|---|---|
| S1 | 首次使用 ≤ 3 步开始对话 | 3 步 | ≤ 3 | ✅ 达标 |
| S2 | 核心操作 ≤ 2 次点击 | ✅ | ≤ 2 | ✅ 达标 |
| S3 | 设置面板 ≤ 8 个分区 | **35** 🔴🔴 | ≤ 8 | P0 重设计 |
| S4 | 输入区 toggle ≤ 2 个 | **5** 🔴 | ≤ 2 | P0 精简 |
| S5 | 侧栏/顶栏功能按钮 ≤ 5 个 | **8** 🟡 | ≤ 5 | P1 精简 |
| S6 | 同时只显示 1 个 overlay | ✅ | 1 | ✅ 达标 |
| S7 | 设置项使用人类语言 | **20 术语** 🔴 | 通俗化 | P0 通俗化 |
| S8 | 默认配置即最佳配置 | **13/20 关闭** 🔴 | 智能模式 | P0 智能模式 |
| S9 | 错误信息可理解 | **60% 技术化** 🟡 | 人类化 | P1 统一分类 |
| S10 | 快捷键覆盖核心操作 | ✅ | ✅ | ✅ 达标 |

**达标**: 4/10（S1/S2/S6/S10）| **不达标**: 4/10（S3/S4/S7/S8）| **需改进**: 2/10（S5/S9）

---

## 1. 设置面板分区审计（S3 标准）

### 1.1 静态 HTML 分区（`index.html` 中 `data-i18n="settings.*_section"`）

| # | 分区名称 | i18n key | 来源文件 | 用户可见性 |
|---|---|---|---|---|
| 1 | LLM 连接信息 | settingsLlmInfo | index.html | ✅ 核心 |
| 2 | LLM 模式（远程/本地） | settings.llm_mode_section | index.html | ✅ 核心 |
| 3 | 语言 | settings.language_section | index.html | ✅ 核心 |
| 4 | 外观主题 | settings.theme_section | index.html | ✅ 核心 |
| 5 | VLM 图片理解 | settings.vlm_section | settings-general.js | 🟡 高级 |
| 6 | Cross-Encoder 重排序 | settings.rerank_section | settings-general.js | 🔴 术语 |
| 7 | HyDE 查询改写 | settings.hyde_section | settings-general.js | 🔴 术语 |
| 8 | 多代理协调 | settings.coordinator_section | settings-advanced.js | 🔴 术语 |
| 9 | 子代理 | settings.sub_agent_section | settings-advanced.js | 🔴 术语 |
| 10 | 上下文长度 | settings.context_token_section | settings-advanced.js | 🟡 高级 |
| 11 | Token 用量 | settings.cost_section | settings-advanced.js | 🟡 高级 |
| 12 | 许可证 | settings.license_section | index.html | ✅ 核心 |
| 13 | 嵌入模型 | settings.embedding_model_section | settings-advanced.js | 🟡 高级 |
| 14 | 分块参数 | settings.chunk_params_section | settings-advanced.js | 🟡 高级 |
| 15 | 自定义模型 | settings.custom_model_section | settings-advanced.js | 🟡 Pro |
| 16 | 模型缓存 | settings.model_cache_section | settings-general.js | 🟡 高级 |
| 17 | 文件同步 | settings.sync_section | settings-advanced.js | 🟡 高级 |
| 18 | PDF 导出 | export_pdf.settings_section | settings-general.js | ✅ 核心 |
| 19 | 语音/TTS | （无 _section 标题） | settings-general.js | 🟡 高级 |
| 20 | 日志/可观测性 | （无 _section 标题） | settings-general.js | 🔴 开发者 |
| 21 | 备份/恢复 | （无 _section 标题） | settings-general.js | ✅ 核心 |

### 1.2 JS 动态注入分区（`settings.js openSettings()` 中 `renderXxx()` 调用）

| # | 分区名称 | 渲染函数 | 来源文件 | 用户可见性 |
|---|---|---|---|---|
| 22 | RAG/LLM 参数 | renderRagLlmParams() | settings-advanced.js | 🟡 高级 |
| 23 | 安全防御 | renderSecuritySettings() | security.js | ✅ 核心 |
| 24 | 记忆管理 | renderMemorySettings() | memory-panel.js | 🔴 术语 |
| 25 | 性能优化 | renderPerfSettings() | perf-settings.js | 🔴 术语 |
| 26 | RAG 评估指标 | renderRagEvalSettings() | rag-eval.js | 🔴 开发者 |
| 27 | Trace + Budget | renderTraceBudgetSettings() | trace-panel.js | 🔴 开发者 |
| 28 | Durable + Todo | renderDurableTodoSettings() | durable-panel.js | 🔴 开发者 |
| 29 | 快捷指令模板 | renderPromptTemplateSettings() | settings-advanced.js | ✅ 核心 |
| 30 | KV Cache | renderKvCacheSettings() | settings-local-llm.js | 🔴 术语 |
| 31 | 硬件加速 | renderDeviceKindSettings() | settings-local-llm.js | 🟡 Pro |
| 32 | 嵌入模型切换 | renderEmbedderModelSettings() | settings-local-llm.js | 🟡 Pro |
| 33 | PagedAttention | renderPagedAttnSettings() | settings-local-llm.js | 🔴 术语 |
| 34 | GEMV 内核模式 | renderKernelModeSettings() | settings-local-llm.js | 🔴 术语 |
| 35 | 窗口管理 | renderWindowSettings() | settings-advanced.js | 🟡 高级 |
| 36 | 错误日志导出 | renderErrorLogsSettings() | settings-advanced.js | 🔴 开发者 |
| 37 | 开机自启 + 更新 | renderStartupSettings() | settings-advanced.js | ✅ 核心 |

### 1.3 分区总数：**35**（去重后，部分无标题分区合并）

### 1.4 建议合并方案（35 → 8）

| 新分区 # | 名称 | 合并来源 | 用户语言 |
|---|---|---|---|
| 1 | **模型配置** | LLM 信息 + LLM 模式 + 嵌入模型 + 嵌入模型切换 + 模型缓存 + 自定义模型 + KV Cache + 硬件加速 + PagedAttention + GEMV 内核 + 采样参数 | "AI 模型设置" |
| 2 | **外观与语言** | 语言 + 主题 + 窗口管理 + 开机自启 | "外观和语言" |
| 3 | **知识库** | 分块参数 + 文件同步 + PDF 导出 + 嵌入模型选择 | "知识库设置" |
| 4 | **安全与隐私** | 安全防御 + 许可证 + 备份/恢复 | "安全和隐私" |
| 5 | **智能检索** | 混合搜索 + Rerank + HyDE + RAG/LLM 参数 + 性能优化（智能模式单一开关） + Contextual Retrieval + 知识图谱检索 | "检索设置" |
| 6 | **高级调优** | 上下文长度 + Token 用量 + 压缩比 + 质量门控 + 渐进式注入 + Speculative RAG | "高级调优"（默认折叠） |
| 7 | **应用设置** | 日志 + 错误日志 + 诊断导出 + 开机自启 + 更新检查 | "应用设置" |
| 8 | **快捷指令** | 快捷指令模板 | "快捷指令" |

**移入隐藏开发者菜单**（不计入 8 分区）：
- RAG 评估指标（rag-eval.js）
- Trace + Budget（trace-panel.js）
- Durable + Todo（durable-panel.js）
- 记忆管理（memory-panel.js）
- VLM 图片理解（保留但移入「模型配置」子项）

---

## 2. 输入区 Toggle 审计（S4 标准）

### 2.1 当前 Toggle 清单

来源：`main.js` 第 1281-1292 行 + `input-toggles.js` TOGGLE_CONFIG

| # | Toggle | 默认值 | IPC 命令 | i18n key | 用户理解度 | 评估 |
|---|---|---|---|---|---|---|
| 1 | 混合搜索 (hybrid) | **true** | set_hybrid_search | chat.toggle_hybrid | ✅ 「混合搜索」通俗易懂 | **保留** |
| 2 | Agent 模式 (agent) | false | set_agent_enabled | chat.toggle_agent | ✅ 「Agent 模式」可理解 | **保留** |
| 3 | 子代理 (sub_agent) | false | set_sub_agent_enabled | chat.toggle_sub_agent | 🔴 「子代理」术语化 | **移入设置** |
| 4 | 记忆 (memory) | false | set_memory_enabled | chat.toggle_memory | 🟡 「记忆」概念模糊 | **移入设置** |
| 5 | 网页搜索 (web_search) | false | set_web_search_enabled | chat.toggle_web_search | ✅ 「网页搜索」通俗易懂 | 🟡 可选保留 |

### 2.2 Toggle 总数：**5**（目标 ≤ 2）

### 2.3 建议方案：5 → 2

| 保留 Toggle | 理由 |
|---|---|
| **混合搜索** | 核心检索增强，Deep(Seek) 风格 toggle，用户一秒理解 |
| **Agent 模式** | 核心推理增强，Deep(Seek) 风格 toggle，用户一秒理解 |

**移除的 3 个 Toggle**：
- `sub_agent`：移入设置面板「高级调优」分区，默认关闭
- `memory`：移入设置面板「高级调优」分区，默认关闭
- `web_search`：移入设置面板「智能检索」分区（或保留为第 3 个 toggle 如果用户反馈需要）

**替代方案**：用斜杠命令 `/agent`、`/memory`、`/web` 替代 toggle，减少视觉噪音

5 个 toggle 占输入区约 200px �'2 个 toggle 仅占 ~80px）

---

## 3. 顶栏/侧栏功能按钮审计（S5 标准）

### 3.1 顶栏按钮清单（`index.html` topBar，第 4514-4557 行）

| # | 按钮 ID | 功能 | 用户可见性 | 评估 |
|---|---|---|---|---|
| 1 | kbBtn | 知识库 | ✅ 核心 | **保留** |
| 2 | convSearchBtn | 搜索会话 | ✅ 核心 | **保留** |
| 3 | graphBtn | 知识图谱 | 🟡 高级 | **保留**（收入「工具」下拉） |
| 4 | dreamBtn | AutoDream 整理 | 🔴 开发者工具 | **移入工具菜单** |
| 5 | symbolBtn | 符号搜索 | 🔴 开发者工具 | **移入工具菜单** |
| 6 | workflowBtn | 工作流 | 🔴 开发者工具 | **移入工具菜单** |
| 7 | branchTreeBtn | 对话分支树 | 🟡 高级 | **保留**（收入「工具」下拉） |
| 8 | exportPdfBtn | 导出 PDF | ✅ 核心 | **保留** |
| 9 | settingsBtn | 设置 | ✅ 核心 | **保留** |
| — | collapseBtn/expandBtn | 侧栏折叠 | 布局控制 | 不计入 |

### 3.2 侧栏按钮清单（`index.html` sidebar）

| # | 按钮 ID | 功能 | 评估 |
|---|---|---|---|
| 10 | workspaceToggle | 工作空间选择 | ✅ 保留 |
| 11 | newChatBtn | 新对话 | ✅ 保留 |
| 12 | plusBtn | 导入文件 | ✅ 保留（在输入区旁） |

### 3.3 功能按钮总数：**8**（顶栏 8 个功能按钮，目标 ≤ 5）

### 3.4 建议方案：8 → 5

| 保留按钮 | 位置 | 理由 |
|---|---|---|
| 知识库 | 顶栏 | 核心功能，高频使用 |
| 搜索会话 | 顶栏 | 核心功能，高频使用 |
| 导出 PDF | 顶栏 | 核心功能，中频使用 |
| 设置 | 顶栏 | 核心功能，低频使用 |
| 工具菜单（下拉） | 顶栏 | 收纳：知识图谱 + AutoDream + 符号搜索 + 工作流 + 对话分支树 |

**工具菜单设计**：单一「工具」按钮 → 点击展开下拉菜单 → 5 个工具项。从 8 按钮减至 5 按钮（含工具菜单按钮本身）。

---

## 4. 用户旅程走查（S1/S2/S6 标准）

### 4.1 旅程 A：新用户首次打开 → 开始对话

| 步骤 | 操作 | 耗时估计 | 痛点 |
|---|---|---|---|
| 1 | 启动应用 → 自动进入向导 Step 1 | 0s | ✅ 无 |
| 2 | 向导 Step 1：向量模型下载 | 10-30s | 🟡 HuggingFace 不可达时 180s 超时 |
| 3 | 向导 Step 2：选择 LLM 预设 → 输入 API Key → 验证 | 30s | ✅ 预设卡片降低门槛 |
| 4 | 向导 Step 3：拖拽/选择文档导入 | 10s | ✅ 拖拽支持 |
| 5 | 向导完成 → 主界面 → 输入问题 → Enter | 5s | ✅ 即问即答 |
| **总计** | **3 步配置 + 1 步提问** | **~60s** | **✅ ≤ 3 步达标** |

**痛点**：
- Step 2 后无「跳过 LLM 配置」的明显提示（有 skipBtn 但 UI 不显眼）
- 向导完成后主界面为空，无引导性提示（如「试试输入你的第一个问题」）
- 首次输入时 toggle 区域显示 5 个 toggle，新用户困惑

### 4.2 旅程 B：配置 LLM → 导入文档 → 提问 → 查看回答

| 步骤 | 操作 | 点击数 | 痛点 |
|---|---|---|---|
| 1 | 点击设置按钮 | 1 | ✅ |
| 2 | 在 35 个分区中找到「模型配置」 | 1（滚动） | 🔴 设置面板过长，用户需滚动寻找 |
| 3 | 修改 LLM 配置 → 保存 | 2 | ✅ |
| 4 | 关闭设置 → 点击知识库按钮 → 导入 | 2 | ✅ |
| 5 | 回到对话 → 输入问题 → Enter | 1 | ✅ |
| 6 | 查看流式回答 + 引用来源 | 0 | ✅ 思维链面板 + 来源可折叠 |
| **总计** | **7 次点击** | | **✅ 核心操作 ≤ 2 点击（导入 + 提问）** |

**痛点**：
- 设置面板 35 个分区，用户无法快速定位目标设置
- 5 个 toggle 在输入区，用户不确定是否需要开启

### 4.3 旅程 C：导出对话

| 步骤 | 操作 | 点击数 | 痛点 |
|---|---|---|---|
| 1 | 点击顶栏导出 PDF 按钮 | 1 | ✅ |
| 2 | 浏览器打印预览 → 保存 | 1 | ✅ |
| **总计** | **2 次点击** | | **✅ 达标** |

**替代路径**：右键会话列表项 → 导出 Markdown（1+1=2 点击）✅

### 4.4 旅程 D：开启安全加密

| 步骤 | 操作 | 点击数 | 痛点 |
|---|---|---|---|
| 1 | 点击设置按钮 | 1 | ✅ |
| 2 | 在 35 个分区中找到「安全防御」 | 1（大量滚动） | 🔴 分区太深 |
| 3 | 点击「加密数据库」→ 输入密码 → 确认 | 2 | ✅ |
| **总计** | **4 次点击** | | **🟡 可接受但分区过深** |

### 4.5 旅程 E：切换知识图谱检索

| 步骤 | 操作 | 点击数 | 痛点 |
|---|---|---|---|
| 1 | 点击设置 | 1 | ✅ |
| 2 | 找到「性能优化」分区 | 1（大量滚动） | 🔴 术语化分区名 |
| 3 | 找到「知识图谱检索」toggle → 开启 | 1 | 🔴 用户不知道这是什么 |
| **总计** | **3 次点击** | | **🟡 操作可完成但术语障碍** |

### 4.6 面板栈管理（S6 标准）

✅ **达标**。`panel-stack.js` 统一管理 overlay 栈：
- ESC 键统一关闭栈顶面板
- 同时只显示 1 个 overlay（新面板 push 时旧面板不关闭，但视觉上只有栈顶可见）
- `closeAllPanels()` 会话切换时关闭所有动态面板

---

## 5. 设置项术语化审计（S7 标准）

### 5.1 术语化设置项清单

| # | 当前文案 | 来源 | 用户理解度 | 建议通俗化 |
|---|---|---|---|---|
| 1 | "Cross-Encoder 重排序" | settings.rerank_toggle | 🔴 学术术语 | "检索结果精排" |
| 2 | "bge-reranker-base" | settings.rerank_desc | 🔴 模型名暴露 | 删除模型名 |
| 3 | "HyDE 查询改写" | settings.hyde_toggle | 🔴 缩写术语 | "智能问题改写" |
| 4 | "Speculative RAG" | perf.speculative_toggle | 🔴 学术术语 | "快速预生成" |
| 5 | "渐进式注入" | perf.progressive_toggle | 🟡 半术语 | "分阶段提供上下文" |
| 6 | "质量门控" | perf.quality_gate_toggle | 🟡 半术语 | "检索质量检查" |
| 7 | "知识图谱检索" | perf.graph_retriever_toggle | 🟡 可理解 | 保持 |
| 8 | "Contextual2 Retrieval" / "上下文增强嵌入" | perf.contextual_retrieval_toggle | 🔴 英文术语 | "文档名前缀增强" |
| 9 | "Anthropic Contextual Retrieval" | perf.contextual_retrieval_desc | 🔴 公司名 | 删除公司名 |
| 10 | "Proposition 索引" | perf.rebuild_proposition | 🔴 学术术语 | "原子命题索引" |
| 11 | "BM25" | perf.rebuild_bm25 | 🔴 学术术语 | "关键词索引" |
| 12 | "Summary Tree" / "RAPTOR" | perf.build_summary_tree | 🔴 学术术语 | "多级摘要索引" |
| 13 | "PagedAttention" | settings-local-llm.js | 🔴 技术术语 | "内存分页注意力" |
| 14 | "GEMV 内核" | settings-local-llm.js | 🔴 技术术语 | "推理加速内核" |
| 15 | "KV Cache" | settings-local-llm.js | 🔴 技术术语 | "对话缓存" |
| 16 | "频率惩罚" (frequency_penalty) | settings-advanced.js | 🔴 技术术语 | "重复词惩罚" |
| 17 | "存在惩罚" (presence_penalty) | settings-advanced.js | 🔴 技术术语 | "话题引导惩罚" |
| 18 | "Token 预算" | settings-advanced.js | 🟡 半术语 | "用量上限" |
| 19 | "分块参数" | settings-advanced.js | 🟡 半术语 | "文档切分设置" |
| 20 | "多代理协调" | settings-advanced.js | 🔴 术语 | "多 AI 协作" |

### 5.2 术语化总数：**20**（其中 🔴 严重 15 个，🟡 中等 5 个）

### 5.3 建议方案

**P0 立即通俗化**（15 个 🔴）：将所有学术术语替换为用户可理解的描述，技术细节移入 tooltip 或「了解更多」链接。

**P1 保留技术名但增加解释**（5 个 🟡）：在设置项旁边增加 `?` 图标，hover 显示通俗解释。

---

## 6. 默认配置审计（S8 标准）

### 6.1 功能开关默认值清单

来源：`crates/tauri-app/src/commands/settings.rs` `get_settings_inner()`

| # | 开关 | 设置键 | 默认值 | 理想默认 | 评估 |
|---|---|---|---|---|---|
| 1 | VLM 图片理解 | mm.vlm_enabled | false | false | ✅ 正确（需 Pro + 隐私） |
| 2 | 混合搜索 | rag.hybrid_search | **true** | true | ✅ 正确 |
| 3 | Cross-Encoder 重排序 | rag.rerank_enabled | **true** | true | ✅ 正确（已从 false 修正） |
| 4 | HyDE 查询改写 | rag.hyde_enabled | false | **true** | 🔴 应默认开启 |
| 5 | Agent 模式 | rag.agent_enabled | false | false | ✅ 正确（按需启用） |
| 6 | 子代理 | rag.sub_agent_enabled | false | false | ✅ 正确 |
| 7 | 上下文长度 | rag.context_token_limit | 4096 | 4096 | ✅ 合理 |
| 8 | 缓存 | cache.enabled | **true** | true |4 | ✅ 正确 |
| 9 | 缓存 TTL | cache.ttl_secs | 86400 | 86400 | ✅ 合理 |
| 10 | 缓存语义阈值 | cache.semantic_threshold | 0.92 | 0.92 | ✅ 合理 |
| 11 | 缓存隐私模式 | cache.privacy_mode | false | false | ✅ 正确 |
| 12 | 质量门控 | rag.quality_gate_enabled | false | **true** | 🔴 应默认开启 |
| 13 | 渐进式注入 | rag.progressive_injection | false | false | ✅ 正确（实验性） |
| 14 | Speculative RAG | state.speculative_enabled | false | false | ✅ 正确（实验性） |
| 15 | 知识图谱检索 | state.graph_retriever_enabled | false | **true** | 🔴 应默认开启 |
| 16 | Contextual Retrieval | state.contextual_retrieval_enabled | **true** | true | ✅ 正确 |
| 17 | 压缩比 | compression.ratio | 1.0 (off) | **2.0** | 🔴 应默认 2x |
| 18 | PagedAttention | llm.paged_attn | false | false | ✅ 正确（Pro 专用） |
| 19 | Coordinator | coordinator_mode | false | false | ✅ 正确 |
| 20 | 记忆 | memory_enabled | false | false | ✅ 正确 |

### 6.2 默认关闭统计：**13/20 关闭（65%）**

### 6.3 应该默认开启但当前关闭的开关

| 开关 | 当前默认 | 建议默认 | 理由 |
|---|---|---|---|
| HyDE 查询改写 | false | **true** | 提升复杂问题召回率，失败时优雅降级 |
| 质量门控 | false | **true** | 检索后自动评估质量，低质量时告警 |
| �B25 知识图谱检索 | false | **true** | 实体关系图遍历增强检索，零 LLM 成本 |
| 压缩比 | 1.0 (off) | **2.0** | Prompt 压缩减少 token 消耗，规则压缩零依赖 |

### 6.4 建议方案：「智能模式」单一开关

**设计**：设置面板「智能检索」分区顶部增加「智能模式」总开关（默认开启）。

**智能模式开启时**：
- 混合搜索 = true
-3. Cross-Encoder 重排序 = true
- HyDE 查询改写 = true
- 知识图谱检索 = true
- 质量门控 = true
- 压缩比 = 2x
- Contextual Retrieval = true

**智能模式关闭时**：
- 以上全部关闭/设为 1x
- 用户手动逐项配置

**效果**：用户开箱即用「最佳配置」，无需理解 12 个术语开关。`perf-settings.js` 12 个 toggle → 1 个智能模式开关 + 折叠的「高级调优」子面板。

---

## 7. 错误信息可理解性审计（S9 标准）

### 7.1 错误类型分类

| 类型 | 示例 | 当前展示 | 用户理解度 | 建议 |
|---|---|---|---|---|
| Embedder 超时 | "EMBED: embedder �9init timed out" | 原始技术信息 | 🔴 不理解 | "向量化引擎初始化超时，请检查网络连接" |
| Pro 门控 | "PRO_REQUIRED: this feature needs Pro" | 原始错误码 | 🔴 不理解 | "此功能需要 Pro 版本，点击了解详情" |
| 数据库错误 | "Invalid column index: 8" | 原始3SQL 错误 | 🔴 不理解 | "数据读取异常，请重启应用" |
| LLM 连接 | "HTTP 401 Unauthorized" | 原始 HTTP 状态 | 🟡 部分理解 | "A2 "'API Key 无效或已过期" |
| 空知识库 | "知识库为空，请先导入文档" | i18n 友好文案 | ✅ �#理解 | 保持 |
| 流中断 | "⏹ 生成已中断" | i18n 友好文案 | ✅ 理解 | 保持 |
| 文件格式 | "Unsupported file extension: .bin" | 原始扩展名 | 🟡 部分理解 | "不支持的文件格式 .bin，支持的格式：md/txt/pdf/..." |

### 7.2 技术化错误比例：**~60%**（7/12 类错误为技术化展示）

### 7.3 建议方案

**P1 统一错误分类**：已有 `error-detail.js` 的 `classifyError()` 函数，应扩展覆盖所有错误类型：

1. `EMBED:` 前缀 → 网络/模型下载类错误 → 通俗化 +%描述 + 重试按钮
2. `PRO_REQUIRED` → Pro 门控错误 → 通俗化 + 升级引导
3. `Invalid column` /%SQL → 数据库错误 → 通俗化 + 重启建议
4. `HTTP 401/403` → 认证错误 → 通俗化 + 配置引导
5. `HTTP 5xx` → 服务端错误 → 通俗化 + 重试按钮
6. `Unsupported` → 格式错误 → 通俗化 + 支持格式列表
7. `Network`/`Timeout` → 网络错误 → 通俗化 + 检查网络提示

---

## 8. 快捷键覆盖审计（S10 标准）

### 8.1 快捷键清单

来源) 快捷键 | 操作 | 来源文件 |
|---|---|---|
| ⌘ J | 新对话 | keyboard.js |
| ⌘ N | 新对话（别名） | keyboard.js |
| ⌘ O | 导入文件 | keyboard.js |
| ⌘ , | 设置 | keyboard.js |
| ⌘ L | 聚焦输入框 | keyboard.js |
| ⌘ K | 命令面板 | keyboard.js |
| ⌘ / | 快捷键帮助 | keyboard.js |
| ⌘ Shift ⌫ | 清空对话 | keyboard.js |
| Esc | 停止生成 / 关闭面板 | chat.js / panel-stack.js |
| Enter | 发送 | input-keymap.js |
| Shift+Enter | 换行 | input-keymap.js |
| ↑/↓ | 历史导航 | input-history.js |

### 8.2 评估：✅ **达标**

核心操作快捷键覆盖完善：
- 新对话 ✅ | 导入 ✅ | 设置 ✅ | 发送 ✅ | 停止 ✅ | 搜索 ✅（⌘K 命令面板）

**建议补充**：
- ⌘ E：导出当前对话（当前需点击顶栏按钮）
- ⌘ G：切换知识图谱检索（当前需进入设置）

---

## 9. 首启向导审计（S1 标准）

### 9.1 向导流程

来源：`wizard.js`

| 步骤 | 内容 | 用户操作 | 耗时 | 痛点 |
|---|---|---|---|---|
| Step 1 | 向量模型下载 | 等待 | 10-30s | 🟡 HuggingFace 不可达时超时 |
| Step 2 | 配置 LLM | 选预设 → 输入 Key → 验证 | 30s | ✅ 预设卡片降低门槛 |
| Step 3 | 导入文档 | 拖拽/选择 | 10s | ✅ 拖拽支持 |

### 9.2 评估：✅ **3 步达标**

**亮点**：
- 预设卡片（DeepSeek/OpenAI/Ollama）一键填充 Base URL + Model
- 下载支持断点续传 + 自动重试（3 次指数退避）
- Step 2 可跳过（无 LLM 也能导入文档）
- Step 3 可跳过（直接进入主界面）

**痛点**：
- Step 1 下载 HuggingFace 模型时，境内用户可能 180s 超时（已有重试但无镜像选择）
- 向导完成后主界面为空，无引导性提示
- 向导中无「为什么需要这些配置」的解释

---

## 10. 右键菜单审计

来源：`context-menu.js`

### 10.1 菜单项清单

| 上下文 | 菜单项 | 用户价值 |
|---|---|---|
| 输入框 | 剪切/复制/粘贴/全选 | ✅ 基础编辑 |
| 会话列表项 | 重命名/删除/导出 Markdown | ✅ 核心管理 |
| 消息 Block | 复制全文/复制纯文本/重新生成/删除 | ✅ 核心操作 |
| 文档列表项 | 删除/导出 PDF/导出 HTML | ✅ 核心管理 |

### 10.2 评估：✅ **良好**

右键菜单设计合理，分组清晰（分隔线），快捷键提示显示，边界检测防溢出。

---

## 11. 综合发现与行动方案

### 11.1 P0 行动（立即执行，影响用户体验）

| # | 行动 | 当前 | 目标 | 预计工作量 |
|---|---|---|---|---|
| P0-1 | 设置面板分区合并 | 35 分区 | 8 分区 | 2 Sessions |
| P0-2 | 输入区 toggle 精简 | 5 toggle | 2 toggle | 0.5 Session |
| P0-3 | 设置项术语通俗化 | 20 术语 | 0 术语 | 1 Session |
| P0-4 | 智能模式单一开关 | 13 关闭 | 智能模式默认 | 1 Session |
| P0-5 | 开发者工具移入隐藏菜单 | 4 分区混入 | 隐藏开发者菜单 | 0.5 Session |

### 11.2 P1 行动（后续优化）

| # | 行动 | 当前 | 目标 | 预计工作量 |
|---|---|---|---|---|
| P1-1 | 顶栏按钮精简 | 8 按钮 | 5 按钮（工具菜单） | 1 Session |
| P1-2 | 错误信息统一分类 | 60% 技术化 | 100% 人类化 | 1 Session |
| P1-3 | 快捷键补充 | 缺 ⌘E/⌘G | 补充导出/图谱 | 0.5 Session |
| P1-4 | 向导完成后引导提示 | 空白 | 引导提示 | 0.5 Session |

### 11.3 与 S1/S2 审计的交叉验证

| S3 发现 | S1 来源 | S2 来源 | 一致性 |
|---|---|---|---|
| 设置面板 35 分区 | S1: settings-advanced 13 子区域 🔴 | — | ✅ 一致 |
| 5 toggle 过多 | S1: input-toggles 3 个用户无法理解 🔴 | — | ✅ 一致 |
| perf-settings 12 术语 | S1: perf-settings F 级 🔴 | S2: 19 性能命令默认关闭 🔴 | ✅ 一致 |
| 开发者工具混入 | S) rag-eval/trace/durable D-F 级 🔴 | S2: 21 开发者工具命令 🔴 | ✅ 一致 |
| 11 幻影调用 | — | S2: 11 幻影调用 🔴 P0 | ✅ 一致 |

---

## 12. 建议的 8 分区设置面板设计

```
┌─────────────────────────────────────────┐
│ 设置                                    │
├─────────────────────────────────────────┤
│                                         │
│  📦 模型配置                            │
│  ├─ AI 模型（远程/本地切换）            │
│  ├─ 嵌入模型                           │
│  └─ 模型缓存                            │
│                                         │
│  🎨 外观与语言                          │
│  ├─ 主题（深色/浅色/跟随系统）           │
│  ├─ 语言（中文/English）                │
│  └─ 窗口管理（最小化到托盘）             │
│                                         │
│  📚 知识库设置                          │
│  ├─ 文档分块参数                        │
│  ├─ 文件夹同步                          │
│  └─ PDF 导出设置                        │
│                                         │
│  🔒 安全和隐私                          │
│  ├─ 数据库加密                          │
│  ├─ PII 检测/脱敏                       │
│  ├─ 自动锁屏                            │
│  ├─ 许可证                              │
│  └─ 备份与恢复                          │
│                                         │
│  🔍 检索设置                            │
│  ├─ ⭐ 智能模式（单一开关）              │
│  ├─ 混合搜索                            │
│  ├─ 检索结果精排                        │
│  ├─ 智能问题改写                        │
│  └─ 知识图谱检索                        │
│                                         │
│  ⚙️ 高级调优（默认折叠）                │
│  ├─ 上下文长度                          │
│  ├─ 用量上限                            │
│  ├─ Prompt 压缩比                       │
│  ├─ 采样参数                            │
│  └─ 实验性功能                          │
│                                         │
│  🚀 应用设置                            │
│  ├─ 开机自启                            │
│  ├─ 自动更新                            │
│  └─ 日志与诊断                          │
│                                         │
│  ⌨ 快捷指令                             │
│  └─ 自定义模板管理                      │
│                                         │
└─────────────────────────────────────────┘

隐藏的开发者菜单（⌘Shift+D 或设置底部「开发者选项」链接）：
  - RAG 评估指标
  - Trace + Budget
  - Durable + Todo
  - 记忆管理
  - GEMV 内核模式
  - PagedAttention
  - KV Cache
  - 错误日志导出
```

---

## 13. 审计结论

### 13.1 核心问题

EchoMind 的 UI **功能完备但暴露过度**。250 REQ 全部实现，但所有功能开关直接堆砌到用户面前，违反「使用越简单，功能越强大」的核心理念。

**三个核心矛盾**：
1. **功能丰富 vs 认知负荷**：35 个设置分区 + 5 个 toggle + 8 个顶栏按钮 = 48 个决策点，远超用户认知容量（7±2）
2. **技术正确 vs 用户理解**：20 个学术术语（Speculative RAG / HyDE / Cross-Encoder）直接展示，用户无法理解
3. **灵活配置 vs 默认最佳**：13/20 功能默认关闭，用户开箱即用-「最差配置」

### 13.2 解决方向

**智能模式 + 分层暴露**：
- 智能模式单一开关 → 默认最佳配置，90% 用户无需手动调整
- 8 分区设置面板 → 核心设置 1 次点击可达，高级设置折叠隐藏
- 2 toggle → 输入区只留最高频的「混合搜索」+「Agent 模式」
- 5 顶栏按钮 → 4 核心 + 1 工具菜单收纳开发者工具
- 0 术语 → 所有设置项使用人类语言，技术细节移入 tooltip

### 13.3 预期效果

| 指标 | 当前 | 目标 | 改善 |
|---|---|---|---|
| 设置分区数 | 35 | 8 | -77% |
| 输入区 toggle | 5 | 2 | -60% |
| 顶栏按钮 | 8 | 5 | -37% |
| 术语化设置项 | 20 | 0 | -100% |
| 默认关闭功能 | 13/20 (65%) | 0/20 (智能模式) | -100% |
| 用户决策点 | 48 | 15 | -69% |

---

## 附录 A：审计文件清单

| 文件 | 用途 | 审计内容 |
|---|---|---|
| `ui/src/settings.js` | 设置面板主协调器 | openSettings() 中 16 个 renderXxx() 调用E |8 |
| `ui/src/settings-general.js` | 通用设置 | VLM/Rerank/HyDE/主题/语音/PDF/日志/备份 9 个子功能 |
| `ui/src/settings-advanced.js` | 高级设置 | 1320 行，13 个子区域 |
| `ui/src/perf-settings.js` | 性能设置 | 12 个学术术语 toggle |
| `ui/src/input-toggles.js` | 输入区 toggle | TOGGLE_CONFIG 5 个 toggle |
| `ui/src/sidebar.js` | 侧栏 | 折叠/KB Modal/搜索/多选/批量删除 |
| `ui/src/wizard.js` | 首启向导 | 3 步流程 |
| `ui/src/main.js` | 入口编排 | toggle 创建（第 1281-1292 行） |
|C `ui/src/context-menu.js` | 右键菜单 | 4 种上下文菜单 |
| `ui/index.html` | HTML 结构 | topBar 9 按钮 + sidebar 3 按钮 |
| `ui/locales/zh-CN.json` | 中文 i18n | 术语化文案 |
| `crates/tauri=app/src/commands/settings.rs` | 后端设置 | 默认值 |

## 附录 B：评估标准定义

| 标准 | 定义 | 测量方法 | 达标阈值 |
|---|---|---|---|
| S1 | 首次使用 ≤ 3 步开始对话 | 向导步骤数 | ≤ 3 |
| S2 | 核心操作 ≤ 2 次点击 | 导入+提问点击数 | ≤ 2 |
| S3 | 设置面板 ≤ 8 个分区 | HTML 静态 + JS 动态注入分区数 | ≤ 8 |
| S4 | 输入区 toggle ≤ 2 个 | TOGGLE_CONFIG 条目数 | ≤ 2 |
| S5 | 侧栏功能按钮 ≤ 5 个 | topBar 功能按钮数（排除布局控制） | ≤ 5 |
| S6 | 同时只显示 1 个 overlay | panel-stack.js 栈管理 | 1 |
| S7 | 设置项使用人类语言 | 术语化设置项数 | 0 |
| S8 | 默认配置即最佳配置 | 默认关闭功能数 / 总功能数 | ≤ 20% |
| S9 | 错误信息可理解 | 技术化错误比例 | ≤ 10% |
| S10 | 快捷键覆盖核心操作 | 核心操作快捷键覆盖率 | 100% |
