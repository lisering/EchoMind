# 前端模块参考

> **状态**：活跃 | **最后审阅**：2026-08-06（v2.4 新增 §8 Agentic AI 前端模块：workflow-panel / memory-panel / dream-panel / symbol-search / perf-settings / input-toggles 扩展；v2.3 新增采样参数函数；v2.2 补充 `handleModelLoadStatus` / `registerModelLoadListener` 函数；v2.1 同步 SRS 原子化重构）
>
> 本文档记录 `ui/index.html` 的模块划分与函数职责。
> 前端为单文件 SPA（vanilla JS + Tailwind CSS 本地构建），**无框架、无 CDN**。
> 所有第三方库本地 vendored 于 `ui/vendor/`。
>
> **与 SRS 的关系**：SRS v2.1.0 §4.4（UI）/ §4.14（DS）/ §4.15（IX）/ §4.21（ERR）已将前端需求原子化为「元素清单 + 状态矩阵 + 交互行为 + 后端契约」格式。本文档记录函数实现层面，SRS 记录规格层面，两者互补。

---

## 1. 第三方库（`ui/vendor/`）

| 文件 | 用途 | 版本 |
|------|------|------|
| `tailwindcss.js` | CSS 框架（本地构建，非 CDN） | Tailwind v4+ |
| `marked.min.js` | Markdown → HTML 解析 | marked.js |
| `purify.min.js` | DOMPurify 白名单消毒（REQ-SEC-002） | DOMPurify |
| `highlight.min.js` | 代码语法高亮 | highlight.js |
| `github-dark.min.css` | highlight.js 暗色主题 | — |
| `mermaid.min.js` | Mermaid 图表渲染（REQ-VIZ-001） | v11.16.0 |
| `katex/katex.min.js` | LaTeX 数学公式渲染（REQ-VIZ-002） | v0.18.1 |
| `katex/katex.min.css` | KaTeX 样式 | v0.18.1 |
| `katex/katex-mhchem.min.js` | KaTeX 化学方程式插件 | — |
| `katex/fonts/*.woff2` | KaTeX 字体（20 个） | — |

> 完整性校验：`ui/vendor/MANIFEST.json` 记录版本、来源 URL 和 SHA-256 hash。

---

## 2. 模块划分（按代码区域）

### 2.1 Tauri API 入口（行 ~261）

封装 `window.__TAURI__.core.invoke()` 调用，提供类型安全的命令封装。

### 2.2 全局状态（行 ~269）

| 状态 | 类型 | 用途 |
|------|------|------|
| `currentRawMarkdown` | String | 流式累积的 Markdown 原文（`chat_token` 事件追加） |
| `currentAssistantEl` | HTMLElement | 当前流式 Block 的 DOM 引用 |
| `currentConversationId` | String | 当前会话 ID |
| `isStreaming` | Boolean | 流式生成状态标记 |

### 2.3 Toast 与错误反馈（行 ~293，REQ-UI-005 / REQ-ERR-001）

| 函数 | 职责 | SRS 引用 |
|------|------|----------|
| `toast(message, kind)` | 全局错误/提示反馈（kind: info/success/error），4.2s 自动消失 | REQ-UI-005 |
| `toastSuccess(message)` | 成功 toast 快捷方法（`toast(message, 'success')`） | REQ-UI-005 |
| `toastError(err)` | 错误 toast 快捷方法（`toast(sanitizeError(err), 'error')`） | REQ-UI-005 / REQ-ERR-001 |
| `sanitizeError(err)` | 错误脱敏：API Key `sk-****` + 用户路径 `/Users/****/` + Windows 路径 `\Users\****\` | REQ-UI-005 / REQ-ERR-001 |
| `resetChatArea(title, desc)` | 重置对话区为空状态引导（◈ 图标 + 标题 + 描述） | REQ-UI-005 |

### 2.3a 统一确认对话框（REQ-IX-005）

| 函数 | 职责 |
|------|------|
| `showConfirmDialog({ title, body, confirmText, cancelText, danger })` | Promise<boolean> 接口的统一确认弹窗；danger=true 时确认按钮红色变体；500ms 防误触 disabled → enabled；Esc=取消, Enter=确认（500ms 后生效）, 点击遮罩=取消 |
| 模块文件 | `ui/src/confirm-dialog.js` |
| 构建顺序 | build-ui.mjs MODULE_ORDER 第 3 位（在 toast.js 之后, markdown.js 之前） |
| 单元测试 | `e2e-tests/tests/unit/confirm-dialog.test.js`（22 tests） |
| E2E 测试 | `e2e-tests/tests/interaction-feedback.spec.ts` E2E-UX-016~018 |
| 调用方 | `settings.js:deleteLocalModel()`, `sidebar.js:showBatchDeleteConfirm()` |
| 替代 | 原生 `confirm()` + 旧 `#kbConfirmDialog` 固定 HTML 弹窗 |

### 2.4 配置向导（行 ~308，REQ-UI-007）

| 函数 | 职责 |
|------|------|
| `renderPresetCards()` | 渲染 DeepSeek/OpenAI/Ollama 预设卡片 |
| `applyPreset()` | 点击预设自动填充 base_url + model |
| `startWizard()` | 验证并开始（调用 `test_llm_connection` → `update_llm_config`） |

### 2.5 会话生命周期（行 ~378，REQ-RAG-006）

| 函数 | 职责 |
|------|------|
| `initConversations()` | 初始化会话列表监听 |
| `newChat()` | 新建会话（调用 `create_conversation`） |
| `loadConversation(id)` | 加载历史会话（调用 `get_messages`） |
| `removeConversation(id)` | 删除会话 |
| `loadConversations()` | 刷新会话列表（调用 `get_conversations`） |

### 2.6 知识库面板（行 ~394，REQ-UI-006 / REQ-ING-005 / REQ-AUDIT-001）

| 函数 | 职责 | SRS 引用 |
|------|------|----------|
| `docStatusOf(d)` | 文档状态徽标渲染（`DOC_STATUS_STYLE` 映射：Pending 灰/Processing 琥珀/Indexed accent/Failed 红） | REQ-UI-006 |
| `displayDocName(filePath)` | 文件名显示（剥离 MD5 前缀，`truncate` 截断 + `title` 完整路径） | REQ-UI-006 |
| `loadDocuments()` | 刷新文档列表（调用 `get_documents`）；空列表渲染导入引导；Pro+Indexed 显示 🔍 审计按钮；Failed 显示 ↻ 重试按钮 | REQ-UI-006 |
| `removeDocument(id, name)` | 删除文档（调用 `delete_document`），toast 反馈 | REQ-UI-006 |
| `deleteDocument(id, name)` | 删除文档（含确认对话框，调用 `delete_document`）— **Draft**：REQ-IX-005 确认对话框模式尚未实现，当前使用 `removeDocument` | REQ-IX-005 |
| `retryDocument(id, name)` | 重试索引（Failed 状态，调用 `retry_index`），toast 反馈 | REQ-VEC-005 |
| `importPaths(paths)` | 导入文件（调用 `import_files`，触发 `doc-status-changed` 事件刷新）；catch 块识别 `LIMIT_REACHED`/`PRO_REQUIRED` 前缀触发 `showPaywall` | REQ-ING-001 / REQ-ERR-001 |
| `auditDocument(docId, docName)` | 文档审计（Pro 门控，调用 `audit_document`，复用 chat_token/chat_done 事件链路渲染报告） | REQ-AUDIT-001 |

### 2.7 主界面渲染（行 ~573，REQ-UI-002 / REQ-VIZ-001/002）

| 函数 | 职责 |
|------|------|
| `showWizard()` / `showApp()` | 向导/主界面切换 |
| `resetChatArea(title, desc)` | 重置对话区 |
| `hideEmptyState()` | 隐藏空状态引导 |
| `appendBlock(role)` | 创建新消息 Block（user/assistant） |
| `appendUserBlock(text)` | 追加用户消息 Block |
| `scheduleRender()` | rAF 节流调度流式渲染（16ms/帧） |
| `renderMarkdown()` | **核心渲染链**：marked.js → mermaid 占位 → KaTeX → DOMPurify → highlight.js |
| `enhanceCallouts(mdEl)` | Markdown callout 块增强 |
| `enhanceCitations(mdEl, sources)` | 引用块增强（[1][2] → 可点击 cite-ref + 双向高亮 REQ-RAG-016） |
| `showMsgActions(blockEl, rawMarkdown)` | 消息操作按钮（复制等） |
| `renderKatexInElement(el)` | KaTeX 实时渲染（行内 `$...$` + 块级 `$$...$$`） |
| `renderRichContent(mdEl)` | **chat_done 后富内容渲染**：mermaid.run() + KaTeX 重试 + 表格图表按钮 |
| `renderSources(sources)` | 引用来源面板渲染（REQ-RAG-016：交互卡片 + 分数 + 预览 + 悬停高亮） |
| `scrollToBottom()` | 平滑滚动到底部 |

### 2.8 发送与流控制（行 ~1010，REQ-UI-003 / REQ-RAG-005 / REQ-AUDIT-005）

| 函数 | 职责 |
|------|------|
| `setInputState(state, hint)` | 输入框三态（idle/generating/error） |
| `finalizeStream(ok)` | 流结束收尾（chat_done 后调用 `renderRichContent` + 持久化；编辑分支更新版本树 assistant + 刷新分页器） |
| `send()` | 发送问题（监听 chat_phase/chat_sources/chat_token/chat_error/chat_done） |
| `stopBtn.onclick` | 停止按钮：审计模式调用 `abort_audit`，对话模式调用 `abort_chat` |

### 2.8.1 消息编辑与版本分页（REQ-RAG-026）

编辑/分页逻辑位于 `ui/src/message-edit.js`（编辑器 + 分页器 DOM）、`ui/src/chat-render.js`（`triggerEdit` 编辑入口）、`ui/src/turn-tree.js`（版本树数据）、`ui/src/chat.js`（`sendFromEdit` 就地重发）。

| 函数 | 职责 |
|------|------|
| `enterEditMode` / `exitEditMode` / `confirmEdit` | 问题气泡就地编辑（Enter 发送 / Shift+Enter 换行 / Esc 取消） |
| `triggerEdit` / `commitEdit` | 编辑入口（点击气泡或编辑按钮共用）：`triggerEdit` 进入编辑模式；`commitEdit` 提交编辑（首次编辑注册原始问答为 v1、调 `edit_user_message` 持久化 v2、`sendFromEdit` 就地重发） |
| `renderBranchPagination` / `navigateBranch` / `applyBranchView` | 分页器（◀ 1/N ▶，DeepSeek 风格渲染在 assistant 答案下方 .msg-actions 之前）切换时问题、答案、来源、思考过程就地替换；`applyBranchView` 切换前先 `clearContent()` 清空旧版本思考内容 |
| `sendFromEdit` | 编辑后就地更新问题文本、重用 assistant 块流式重生成（流式期间不渲染分页器，流完成后由 `finalizeStream` 渲染） |
| `buildTurnTree` / `addVersion` / `updateVersionAssistant` / `setActiveVersion` | 版本树构建与更新（`userMessageId` 记录各版本原始消息行 id） |
| `appendUserBlock(text, msgId)` | 渲染用户消息块并保存消息行 id（首次编辑定位原始行） |

### 2.8.2 对话分支树面板（REQ-RAG-039）

分支树面板位于 `ui/src/turn-tree.js`（树视图渲染 + 分支切换）、`ui/src/main.js`（`#branchTreeBtn` 按钮绑定）、`ui/src/ipc.js`（`convApi.getConversationTree`/`branchFromMessage` 封装）。

| 函数 | 职责 |
|------|------|
| `openConversationTreePanel()` | 打开分支树面板：调 `get_conversation_tree` IPC 加载树数据，渲染为垂直树形列表 |
| `closeConversationTreePanel()` | 关闭分支树面板 |
| `renderTreeNode(node, nodeMap, activePath)` | 递归渲染单个树节点（版本号 + 预览 + Active 徽章 + 子节点缩进） |
| `switchToBranch(node)` | 点击节点切换分支：调 `set_turn_active_version` 持久化 + 更新本地轮次树 + 触发 `echomind-branch-switched` 事件 |
| `isTreePanelOpen()` | 检查分支树面板是否已打开 |
| `convApi.getConversationTree(conversationId)` | IPC 封装：获取会话的分支树结构 |
| `convApi.branchFromMessage(conversationId, messageId, newContent)` | IPC 封装：从指定消息创建新分支 |

### 2.9 侧栏折叠与交互（REQ-NAV-001）

| 函数 | 职责 |
|------|------|
| `toggleSidebar()` | 侧栏折叠/展开切换（图标列模式 48px + `set_sidebar_collapsed` IPC 持久化） |
| `restoreSidebarState()` | 启动时从后端恢复侧栏折叠状态（`get_sidebar_collapsed` IPC） |
| `openFlyout(type)` | 折叠状态下点击图标弹出浮层面板（conversations/kb/settings） |
| `closeFlyout()` | 关闭浮层面板（点击外部/关闭按钮） |
| `setSelectConversationCallback(fn)` | 注入选中会话回调（浮层面板点击列表项 → 展开侧栏 + 加载会话） |
| `initSidebar()` | 绑定侧栏事件（折叠按钮 + 图标列 + 浮层关闭） |

### 2.10 付费墙与 License 管理（行 ~1198，REQ-LIC-003 / REQ-LIC-004）

| 函数 | 职责 | SRS 引用 |
|------|------|----------|
| `showPaywall(reason)` | 显示付费墙 Modal（`LIMIT_REACHED` / `PRO_REQUIRED` 前缀触发） | REQ-LIC-003 / REQ-ERR-001 |
| `hidePaywall()` | 关闭付费墙 | REQ-LIC-003 |
| `updateProStatus()` | 刷新授权状态徽标（调用 `get_pro_status`，更新 `#proStatus` 文案） | REQ-LIC-002 |
| `activatePro()` | 激活 License（调用 `activate_pro`，成功后 `updateProStatus` + `loadDocuments`） | REQ-LIC-003 |
| `deactivatePro()` | 停用 Pro 授权（调用 `deactivate_pro`，回落免费版 + toast + `loadDocuments`） | REQ-LIC-004 |

### 2.11 设置面板（REQ-MM-003 前端接线）

| 函数 | 职责 |
|------|------|
| `openSettings()` | 打开设置面板（调用 `get_settings` 加载配置与 VLM 状态） |
| `closeSettings()` | 关闭设置面板 |
| `updateVlmToggle()` | 更新 VLM Toggle 开关视觉状态（轨道色 + 滑块位置 + 隐私提示可见性） |
| `onVlmToggle()` | VLM Toggle 点击：开启时弹隐私确认，关闭时直接 `set_vlm_enabled` |
| `confirmVlmEnable()` | 隐私确认弹窗「确认开启」→ `set_vlm_enabled(true)` |
| `cancelVlmEnable()` | 隐私确认弹窗「取消」→ 保持关闭 |
| `saveVlmSetting(enabled)` | 持久化 VLM 开关到后端（失败时回滚 UI 状态） |

### 2.12 模型缓存管理（REQ-VEC-008）

| 函数 | 职责 |
|------|------|
| `openSettings()` 中缓存区 | 调用 `get_model_cache_info` 加载缓存大小 → 渲染到设置面板「模型缓存」区 |
| 下载模型按钮 | 调用 `init_embedder` → 监听 `model_download_progress` 事件 → toast 进度 |
| 清理缓存按钮 | 调用 `clear_model_cache` → toast 释放字节数 → 刷新缓存大小 |
| `formatBytes(bytes)` | 字节数格式化工具（B/KB/MB/GB 自适应） |

### 2.12.1 自定义 ONNX 嵌入模型上传（REQ-VEC-014，Pro 门控）

| 函数 | 职责 |
|------|------|
| `loadCustomModels()` | 调用 `list_custom_models` IPC 加载已上传的自定义模型列表，渲染为列表项（名称 + 大小 + 有效性 + 切换/删除按钮）。Free 用户显示 Pro 升级提示。 |
| `onUploadCustomModel()` | 上传流程：选择 ONNX 文件 → 选择 tokenizer 文件（多选）→ 输入模型名称 → 调用 `upload_custom_embedding_model` IPC → toast 结果 → 刷新列表。Pro 门控（Free 返回 `PRO_REQUIRED` 时 toast 升级提示）。 |
| `switchToCustomModel(name)` | 调用 `set_embedding_model('custom:{name}')` IPC → toast 切换成功 → 刷新设置面板（重建索引提示）。 |
| `deleteCustomModel(name)` | 弹出确认对话框 → 调用 `delete_custom_model` IPC → toast 删除结果 → 刷新列表。 |
| `embedModelApi.uploadCustomModel(name, onnxPath, tokenizerFiles)` | IPC 封装：`invoke('upload_custom_embedding_model', { name, onnxPath, tokenizerFiles })` |
| `embedModelApi.listCustomModels()` | IPC 封装：`invoke('list_custom_models')` |
| `embedModelApi.deleteCustomModel(name)` | IPC 封装：`invoke('delete_custom_model', { name })` |

### 2.12 本地 LLM 模型管理（REQ-LLM-003/004）

| 函数 | 职责 |
|------|------|
| `loadLlmModeSetting(settings)` | 从 `get_settings` 返回的 `llm_mode` / `local_model` 字段恢复 UI 状态（radio 选中 + 选中模型标记） |
| `onLlmModeChange(mode)` | 推理模式 radio 切换：Free 用户切 local 被拦截 → `set_llm_mode` → toast |
| `loadLocalModels()` | 调用 `list_local_models` → 渲染已下载模型列表（架构/参数量/量化/大小 + 使用/删除按钮） |
| `loadRecommendedModels()` | 调用 `get_recommended_models` → 渲染推荐模型列表（名称/量化/大小/描述 + 下载按钮） |
| `downloadLocalModel(url, filename, displayName)` | 调用 `download_model` → 监听 `model_download_progress` 事件 → 更新进度条 → 完成后刷新列表 |
| `deleteLocalModel(filename, name, sizeBytes)` | 确认对话框 → 调用 `delete_model` → toast → 刷新列表 |
| `selectLocalModel(filename, name)` | 调用 `set_local_model` → `set_llm_mode("local")` → toast → 刷新列表 |
| `formatModelSize(bytes)` | 模型文件大小格式化（B/KB/MB/GB，GGUF 模型通常 2~5GB） |
| `handleModelLoadStatus(payload)` | 处理 `model_load_progress` 事件：status=loading→info toast，status=ready→success toast，status=error→error toast（REQ-LLM-003 AC-3/AC-4） |
| `registerModelLoadListener()` | 注册 `model_load_progress` 事件全局监听器（防重复注册，在 `initSettings()` 中调用） |
| `loadSamplingParams(settings)` | 从 `get_settings` 返回的 `llm_sampling` 字段恢复采样参数 UI 状态（temperature/top_p/top_k/max_tokens/frequency_penalty/presence_penalty 输入框）（REQ-LLM-006，Phase 2） |
| `collectSamplingParams()` | 从 UI 输入框收集采样参数值，返回 `LlmSamplingParams` 对象（空值转为 `null`，表示使用引擎默认值）（REQ-LLM-006，Phase 2） |
| `saveSamplingParams()` | 调用 `set_sampling_params` IPC 命令保存采样参数 → toast 提示（REQ-LLM-006，Phase 2） |
| `resetSamplingParams()` | 重置采样参数为默认值（全 `null`）→ 调用 `set_sampling_params` 持久化 → toast 提示（REQ-LLM-006，Phase 2） |
| `renderKernelModeSection(settings)` | 渲染内核模式选择 UI（mistral/custom 单选按钮），从 `get_kernel_mode` 返回值恢复当前选择（REQ-LLM-007，Phase 3） |
| `switchKernelMode(mode)` | 调用 `set_kernel_mode` IPC 命令切换内核模式 → toast 提示 → 刷新设置面板（REQ-LLM-007，Phase 3） |
| `onPauseDownload()` | 暂停当前内联进度条对应的下载（委托给 `download-manager.js` 的 `pauseDownload`） |
| `onCancelDownload()` | 取消当前内联进度条对应的下载（委托给 `download-manager.js` 的 `cancelDownload`） |

### 2.13 下载管理器（REQ-LLM-004 v2：断点续传 + 暂停/恢复/取消 + 崩溃恢复）

源文件：`ui/src/download-manager.js`

| 函数 | 职责 |
|------|------|
| `initDownloadManager()` | 初始化下载管理器：注册 `model_download_progress` 事件监听 + 绑定面板 DOM 事件 + 启动时调用 `checkCrashRecovery()` 扫描崩溃恢复 |
| `openDownloadManager()` | 打开下载管理器面板（`#downloadManagerOverlay`），激活 Focus Trap，刷新列表 |
| `closeDownloadManager()` | 关闭面板，停用 Focus Trap |
| `toggleDownloadManager()` | 切换面板可见性 |
| `startDownload(url, filename, displayName)` | 启动模型下载：注册到 `_downloads` Map → 自动打开面板 → 调用 `download_model` IPC（后端自动检测 `.partial` 断点续传） |
| `pauseDownload(filename)` | 暂停下载：调用 `pause_download` IPC → 更新状态为 `paused`（保留 `.partial` + `.meta.json`） |
| `resumeDownload(filename)` | 恢复暂停的下载：从 `_downloads` Map 获取 URL → 重新调用 `download_model`（后端自动续传） |
| `cancelDownload(filename)` | 取消下载：确认对话框 → 调用 `abort_download` IPC → 清理 `.partial` + `.meta.json` → 从列表移除 |
| `checkCrashRecovery()` | 启动时扫描崩溃恢复：调用 `scan_download_recovery` IPC → 检测 `.partial` + `.meta.json` → 显示恢复提示 Modal |
| `resumeAllRecovery()` | 恢复全部崩溃下载：关闭恢复 Modal → 逐个调用 `download_model` 续传 |
| `discardAllRecovery()` | 丢弃全部崩溃下载：调用 `cleanup_partial_downloads` IPC → 清理所有临时文件 → 清空列表 |
| `clearCompletedDownloads()` | 清理已完成/失败的下载条目（仅从 UI 移除，不删文件） |
| `renderDownloadList()` | 渲染下载列表：每个条目含文件名、状态徽章、进度条、百分比、大小/速度、暂停/恢复/取消按钮 |
| `updatePanelBadge()` | 更新面板标题栏的活跃下载数量徽章 |
| `formatSize(bytes)` | 格式化文件大小（B/KB/MB/GB） |
| `formatSpeed(bytesPerSec)` | 格式化下载速度（B/s ~ GB/s） |
| `parseDownloadStatus(status)` | 从后端 `DownloadStatus` 枚举（serde tagged union）提取状态字段 |

### 2.14 启动引导（行 ~1285）

应用启动入口：`get_settings` 判断是否已配置 → 显示向导或主界面。

### 2.15 代码执行沙箱（REQ-RAG-032，Pro feature）

源文件：`ui/src/code-executor.js`

| 函数 | 说明 |
|---|---|
| `openCodeExecutorPanel()` | 打开代码执行模态面板（语言选择 + 代码编辑区 + stdin + 结果展示 + Focus Trap） |
| `closeCodeExecutorPanel()` | 关闭代码执行面板（清理 DOM + 停用 Focus Trap） |
| `initCodeExecutor()` | 初始化代码执行器（当前为空操作，预留命令面板入口） |

- 面板支持 Python / JavaScript 语言选择
- 代码编辑区支持 `Ctrl/Cmd+Enter` 快捷键执行
- 结果展示区显示 stdout / stderr / exit_code / duration / timed_out
- Free 版本调用 `execute_code_snippet` IPC 时后端返回 Pro 错误，前端显示升级提示

---

## 3. 流式渲染时序（REQ-VIZ-001/002）

```
chat_token 事件
    ↓
currentRawMarkdown += token
    ↓
scheduleRender() (rAF 节流)
    ↓
renderMarkdown()
    ├── marked.js 解析 → HTML
    ├── ```mermaid 代码块 → <div class="mermaid-source"> 占位
    ├── $...$ / $$...$$ → KaTeX 实时渲染（半截语法保留 .katex-pending）
    ├── DOMPurify 白名单消毒
    └── highlight.js 语法高亮（跳过 mermaid 语言）
    ↓
chat_done 事件
    ↓
finalizeStream()
    └── renderRichContent()
        ├── mermaid.run() → SVG 图表
        ├── KaTeX 重试渲染（.katex-pending）
        └── 表格数据检测 → 「切换图表」按钮（REQ-VIZ-003 待实施）
```

---

## 4. Tauri 事件监听

| 事件 | 监听处 | 处理逻辑 | SRS 引用 |
|------|--------|----------|----------|
| `doc-status-changed` | `importPaths` / `loadDocuments` | 刷新文档列表徽标 + toast（status=error 时 error kind） | REQ-UI-006 |
| `import-progress` | `importPaths` | 更新导入进度条（completed/total/current_file），取消时隐藏进度条 | REQ-ING-006 |
| `embedding_progress` | `importPaths` | 更新向量化进度条（embedded/total/doc_name），复用导入进度条区域 | REQ-VEC-004 |
| `model_download_progress` | 设置面板 | 模型下载进度 toast（Downloading/Loading/Done/Error）+ 完成后刷新缓存信息 | REQ-VEC-008 |
| `chat_phase` | `send` | 更新 `#inputHint` 文案（preparing/retrieving/generating） | REQ-UI-003 |
| `audit_phase` | `auditDocument` | 审计进度阶段（extracting/comparing/reporting），更新思考指示器文案 | REQ-AUDIT-005 |
| `chat_sources` | `send` | 渲染引用来源面板（`.sources` 芯片区） | REQ-UI-002 |
| `chat_token` | `send` / `auditDocument` | `currentRawMarkdown += token` → `scheduleRender()`（审计报告复用） | REQ-UI-002 |
| `chat_error` | `send` / `auditDocument` | `toastError(err)` + 输入框 error 态（审计中断复用「已中断」语义） | REQ-UI-003 / REQ-ERR-001 |
| `chat_done` | `send` / `auditDocument` | `finalizeStream()` → `renderRichContent()`；审计模式清除 `auditingDocId` | REQ-UI-002 |

---

## 5. SRS v2.1.0 原子化规格交叉引用

> 以下函数/行为在 SRS v2.1.0 中已原子化为「元素清单 + 状态矩阵 + 交互行为 + 后端契约」格式。
> 本节仅列出交叉引用，详细规格见 SRS 对应章节。

### 5.1 UI 域（SRS §4.4）

| SRS REQ | 原子化内容 | 本文档对应章节 |
|---------|------------|----------------|
| REQ-UI-001 | 色板 6 token + 主界面布局图 + 侧栏折叠交互 | §2.1 全局结构 |
| REQ-UI-002 | 消息 Block 元素清单 + 流式渲染状态矩阵（7 阶段）+ 引用芯片规格 + 代码块增强 | §2.7 主界面渲染 |
| REQ-UI-003 | `#inputBar` 元素清单 + idle/streaming/error 三态矩阵 + `send()` 函数行为 8 步 | §2.8 发送与流控制 |
| REQ-UI-004 | `#dragOverlay` 元素清单 + drag-enter/leave/drop 状态矩阵 | §2.6（拖拽事件处理） |
| REQ-UI-005 | Toast 元素清单 + `sanitizeError()` 脱敏规则 + 空状态引导规格 | §2.3 Toast 与错误反馈 |
| REQ-UI-006 | 文档列表项规格 + `DOC_STATUS_STYLE` 徽标映射 + 操作按钮可见性规则 | §2.6 知识库面板 |
| REQ-UI-007 | 向导元素清单 + 预设卡片规格 + `startWizard()` 验证流程 9 步 | §2.4 配置向导 |
| REQ-UI-008 | LLM 配置持久化（AES-256-GCM） | §2.4 + 后端 IPC |

### 5.2 DS 域（SRS §4.14，全部 Draft）

| SRS REQ | 原子化内容 | 实现状态 |
|---------|------------|----------|
| REQ-DS-001 | 色板体系（6 自定义 token + Tailwind 原生色 + `.md` 排版色值） | 部分实现（自定义 token 已定义，语义色待补） |
| REQ-DS-002 | 字体层级（8 档字号 + `.md` em 倍率 + 行高 + 字重） | 已实现（CSS 内联） |
| REQ-DS-003 | 4px 间距网格（10 档 token + 关键容器间距实测） | 已实现（Tailwind spacing scale） |
| REQ-DS-004 | 图标系统（Unicode 字符方案，SVG 系统为 Draft） | 部分实现（Unicode 方案，SVG 待补） |
| REQ-DS-005 | 组件设计规范（按钮 4 变体 + 输入框 + 卡片 + 模态框 + Toast） | 已实现（Tailwind 类组合） |
| REQ-DS-006 | 动效规范（fade-in + thinkingPulse + rAF 节流 + 侧栏过渡） | 部分实现（prefers-reduced-motion 待补） |
| REQ-DS-007 | 圆角阴影体系（7 档圆角 + 不对称圆角 + `.md` 排版圆角） | 已实现 |

### 5.3 IX 域（SRS §4.15，全部 Draft）

| SRS REQ | 原子化内容 | 实现状态 |
|---------|------------|----------|
| REQ-IX-001 | 右键上下文菜单（文档/会话/消息/空白处 4 场景 + 菜单项规格 + 定位逻辑） | Draft（未实现） |
| REQ-IX-002 | 会话拖拽排序（HTML5 Drag API + 插入线 + `sort_order` 持久化） | Draft（未实现） |
| REQ-IX-003 | 统一复制行为（5 场景矩阵 + clipboard API + 错误处理） | 部分实现（代码块复制已有，右键复制 Draft） |
| REQ-IX-004 | 悬停反馈（15 项元素清单 + Tooltip 规格 + 过渡时长） | 已实现（Tailwind hover 变体） |
| REQ-IX-005 | 确认对话框（6 场景 + 元素规格 + 交互行为 + 500ms 防误触） | Draft（`showConfirmDialog` 未实现） |
| REQ-IX-006 | 输入框多行自适应（高度规格 + 键盘行为） | 已实现（`input` 事件 + `max-h-40`） |
| REQ-IX-007 | 骨架屏（文档/会话列表 + 延迟 200ms + animate-pulse） | Draft（`showSkeleton` 未实现） |

### 5.4 ERR 域（SRS §4.21，全部 Draft）

| SRS REQ | 原子化内容 | 实现状态 |
|---------|------------|----------|
| REQ-ERR-001 | 错误分类六元组（9 类前缀 + 消息模板 + Toast 样式 + 恢复建议） | 部分实现（`LIMIT_REACHED`/`PRO_REQUIRED` 已实现，其余 Draft） |
| REQ-ERR-002 | 网络错误自动重试（指数退避 1s→2s→4s + 4xx 不重试 + retrying phase） | Draft（`OpenAIProvider` 无重试逻辑） |
| REQ-ERR-003 | 离线模式降级（离线指示器 + 功能可用性矩阵 + 检测/退出机制） | Draft（`setOfflineMode` 未实现） |
| REQ-ERR-004 | 崩溃恢复（`cleanup_zombies` 已实现 + `PRAGMA integrity_check` Draft） | 部分实现（僵尸清理已有，完整性检查 Draft） |
| REQ-ERR-005 | 输入校验（10 项规则矩阵，路径遍历/扩展名/空值/标题截断已实现） | 部分实现（长度/格式校验 Draft） |

---

## 6. 极致性能优化前端函数（REQ-PERF-001~012）

### 6.1 缓存设置函数

| 函数 | 签名 | 说明 |
|------|------|------|
| `loadCacheSettings()` | `() => Promise<CacheSettingsPayload>` | 读取缓存设置（enabled / ttl_secs / semantic_threshold / privacy_mode） |
| `saveCacheSettings(settings)` | `(settings: CacheSettingsPayload) => Promise<void>` | 保存缓存设置 |
| `getCacheStats()` | `() => Promise<CacheStats>` | 获取缓存运行时统计 |
| `clearCache()` | `() => Promise<void>` | 清空所有缓存 |

### 6.2 压缩设置函数

| 函数 | 签名 | 说明 |
|------|------|------|
| `loadCompressionRatio()` | `() => Promise<number>` | 读取当前压缩比 |
| `saveCompressionRatio(ratio)` | `(ratio: number) => Promise<void>` | 设置压缩比（1.0=禁用 / 2.0=保守 / 3.0=平衡 / 5.0=激进） |

### 6.3 检索增强函数

| 函数 | 签名 | 说明 |
|------|------|------|
| `rebuildBM25Index()` | `() => Promise<void>` | 重建 FTS5 BM25 索引（上下文前缀） |
| `rebuildPropositionIndex()` | `() => Promise<void>` | 重建 Proposition 索引 |
| `buildSummaryTree(docId)` | `(docId: string) => Promise<void>` | 构建文档的 RAPTOR 摘要树 |
| `setEmbedderModel(model)` | `(model: string) => Promise<void>` | 切换嵌入模型 |

### 6.4 检索记忆函数

| 函数 | 签名 | 说明 |
|------|------|------|
| `setRetrievalMemoryEnabled(enabled)` | `(enabled: boolean) => Promise<void>` | 启用/禁用自进化检索记忆 |
| `getRetrievalMemoryStats()` | `() => Promise<MemoryRecord[]>` | 获取检索记忆统计 |
| `resetRetrievalMemory()` | `() => Promise<void>` | 重置检索记忆 |
| `reportFeedback(query, method, feedbackType)` | `(query: string, method: string, feedbackType: string) => Promise<void>` | 上报用户反馈信号到后端（REQ-PERF-012 扩展） |
| `inferRetrievalMethod()` | `() => string` | 推断当前检索方法（'vector_only' / 'hybrid' / 'hybrid_rerank'） |
| `textSimilarity(a, b)` | `(a: string, b: string) => number` | Jaccard 词重叠率相似度（0.0-1.0） |
| `detectAndReportImplicit(newQuery, isEdit)` | `(newQuery: string, isEdit: boolean) => void` | 隐式反馈检测：相似度 >0.6 → retry，<0.3 → accepted，编辑 → edit_and_resend |
| `reportThumbsUp(query)` | `(query: string) => void` | 显式反馈：用户点赞 → thumbs_up |
| `reportThumbsDown(query)` | `(query: string) => void` | 显式反馈：用户点踩 → thumbs_down |
| `resetFeedbackTracking()` | `() => void` | 重置反馈跟踪状态（新对话/切换会话时调用） |

### 6.5 性能优化设置面板

设置面板中新增「极致性能优化」区域，包含以下控件：
- 缓存开关 + TTL 滑块 + 语义阈值输入 + 隐私模式开关
- 压缩比下拉选择（禁用 / 保守 / 平衡 / 激进）
- 渐进式注入开关（通过 `rag.progressive_injection` 设置）
- Speculative RAG 开关（通过 `rag.speculative_enabled` 设置）
- 检索记忆开关 + 统计展示 + 重置按钮

### 7.1 DeepSeek 风格 UI 模块（REQ-UI-010）

| 模块文件 | 函数 | 说明 |
|---|---|---|
| `ui/src/thinking-panel.js` | `createThinkingPanel()` | 创建可折叠思维链面板。返回 `{container, update(text), collapse(), expand(), setComplete(), appendReasoning(text), renderReasoning(text), finalizeReasoning(), clearContent(), getReasoning(), reset()}`。包含 SVG 灯泡图标 + chevron 旋转动画。点击 `.thinking-panel-header` 切换 `.thinking-panel-content` 的 `hidden` 类。思考内容：`appendReasoning` 流式累加纯文本（性能），`finalizeReasoning` 流完成时一次性 markdown 渲染（`renderMarkdown` + DOMPurify，对齐 DeepSeek `ds-think-content`），`renderReasoning` 用于版本切换/历史加载直接渲染完整内容，`clearContent` 清空旧版本内容。 |
| `ui/src/input-toggles.js` | `createInputToggle(config)` | 创建输入区快速 Toggle 按钮。`TOGGLE_CONFIG` 配置表（hybrid/agent/sub_agent/memory/web_search）。`getToggleState(key)` / `setToggleState(key, val)` 读写状态。`doToggle(key)` 异步切换 + IPC 调用（`set_hybrid_search` / `set_agent_enabled` / `set_sub_agent_enabled` / `set_memory_enabled` / `set_web_search_enabled`）+ 失败回滚。`role=switch` + `aria-checked` A11Y 属性。Enter/Space 键盘支持。 |

### 7.2 DeepSeek 风格 CSS 变量（REQ-UI-010）

| 文件 | 变量/规则 | 说明 |
|---|---|---|
| `ui/styles/tokens.css` | `--msg-user-radius: 22px` | 用户消息气泡圆角 |
| `ui/styles/tokens.css` | `--msg-list-max-width: 840px` | 消息列表最大宽度 |
| `ui/styles/tokens.css` | `--msg-assistant-bg: transparent` | AI 消息透明背景 |
| `ui/styles/tokens.css` | `@font-face quote-cjk-patch` | CJK 引号修复（PingFangSC + `unicode-range: U+201C-201D`） |
| `ui/styles/tokens.css` | `--dsw-alias-*` | 语义别名层变量（Static → Alias 映射） |
| `ui/styles/tokens.css` | `--dsl-*` | 组件变量层（Alias → Component 映射） |
| `ui/styles/tokens.css` | `body.change-theme *` | 主题切换过渡禁用规则 |
| `ui/styles/components.css` | `#chatArea { max-width: 840px }` | 聊天区域居中限宽 |
| `ui/styles/components.css` | `.ai-disclaimer` | AI 免责声明样式 |
| `ui/styles/components.css` | `.input-toggle` / `.shortcut-hint` | 输入区 Toggle + 快捷键提示样式 |
| `ui/styles/components.css` | `.sources-toggle` | 搜索来源折叠/展开样式 |

### 7.3 知识图谱可视化模块（REQ-RAG-027）

| 模块文件 | 函数 | 说明 |
|---|---|---|
| `ui/src/graph-viewer.js` | `openGraphViewer()` | 打开知识图谱全屏 overlay 面板。惰性创建 DOM，并行调用 `get_graph_data` + `get_graph_stats` IPC 获取数据，渲染 D3 force-directed graph。激活 Focus Trap（复用 `focus-trap.js`）。 |
| `ui/src/graph-viewer.js` | `closeGraphViewer()` | 关闭图谱面板。移除 `graph-visible` 类，停用 Focus Trap，停止 D3 simulation。支持 ESC 键触发。 |
| `ui/src/graph-viewer.js` | `renderGraph()` | D3 forceSimulation 渲染：节点（圆形 + 标签，大小根据 degree 决定）+ 边（曲线，颜色根据关系类型编码）。支持拖拽、滚轮缩放、画布平移。 |
| `ui/src/graph-viewer.js` | `highlightNode(entityText)` | 高亮指定节点的所有关联边和关联节点，暗淡未关联元素。点击节点触发。 |
| `ui/src/graph-viewer.js` | `zoomIn()` / `zoomOut()` / `resetView()` | 缩放控制（D3 zoom behavior scaleBy 1.3 / 1/1.3 / identity 重置）。 |
| `ui/src/graph-viewer.js` | `getRelationColor(relationType)` | 纯逻辑函数：返回关系类型对应的颜色 hex 值（8 种关系类型 + 默认灰色）。 |
| `ui/src/graph-viewer.js` | `buildGraphData(triples)` | 纯逻辑函数：从 `GraphTriple[]` 构建 D3 图谱数据 `{ nodes, links }`，同名实体合并为一个节点，节点 degree 根据连接数计算。 |
| `ui/src/graph-viewer.js` | `getUniqueRelationTypes(links)` | 纯逻辑函数：从边列表获取去重后的关系类型列表，用于图例渲染。 |
| `ui/src/graph-viewer.js` | `initGraphResize()` | 初始化窗口大小变化监听，图谱打开时自动调整 SVG 尺寸 + 重新启动 simulation。 |
| `ui/src/graph-viewer.js` | `getEntityIcon(entityType)` | 纯逻辑函数：返回实体类型对应的内联 SVG 图标 path（5 种类型 + 默认空）。 |
| `ui/src/graph-viewer.js` | `getEntityTypeName(entityType)` | 纯逻辑函数：返回实体类型的 i18n 本地化名称。 |
| `ui/src/graph-viewer.js` | `isEdgeVisible(relationType, enabledTypes)` | 纯逻辑函数：检查边是否应该可见（关系类型在过滤集合中）。 |
| `ui/src/graph-viewer.js` | `renderFilterPanel()` | 渲染关系类型过滤面板（复选框列表），取消勾选的类型对应的边和节点隐藏。 |
| `ui/src/graph-viewer.js` | `applyFilter()` | 应用过滤：隐藏/显示边和节点（`graph-edge-hidden` / `graph-node-hidden` 类）。 |
| `ui/src/graph-viewer.js` | `searchAndLocate(query)` | 搜索实体并自动定位+高亮：查找匹配节点 → 居中+缩放 → 添加 `graph-node-searched` 类。 |
| `ui/src/graph-viewer.js` | `exportSvg()` | 导出图谱为 SVG 文件（SVG DOM 序列化 → Blob → 下载）。 |
| `ui/src/graph-viewer.js` | `exportPng()` | 导出图谱为 PNG 文件（SVG → Canvas → PNG Blob → 下载）。 |
| `ui/styles/graph-viewer.css` | `.graph-overlay` / `.graph-visible` | 全屏 overlay 面板（visibility + opacity 过渡，z-index: 99998） |
| `ui/styles/graph-viewer.css` | `.graph-node-circle` / `.graph-node-label` | D3 节点样式（圆形 + 标签，高亮/暗淡状态） |
| `ui/styles/graph-viewer.css` | `.graph-edge` / `.graph-edge-highlighted` | D3 边样式（曲线连接，高亮/暗淡状态） |
| `ui/styles/graph-viewer.css` | `.graph-legend` / `.graph-tooltip` / `.graph-detail-panel` | 图例 + 工具提示 + 实体详情面板样式 |
| `ui/styles/graph-viewer.css` | `.graph-empty-state` | 空状态提示样式（无关系数据时显示） |
| `ui/styles/graph-viewer.css` | `.graph-search-bar` / `.graph-search-input` | 搜索栏样式（Session 4 新增） |
| `ui/styles/graph-viewer.css` | `.graph-filter-panel` / `.graph-filter-checkbox` | 关系类型过滤面板样式（Session 4 新增） |
| `ui/styles/graph-viewer.css` | `.graph-node-icon` / `.graph-node-searched` | 实体类型图标 + 搜索高亮节点样式（Session 4 新增） |
| `ui/styles/graph-viewer.css` | `.graph-edge-hidden` / `.graph-node-hidden` | 过滤隐藏状态样式（Session 4 新增） |
| `ui/styles/graph-viewer.css` | `.graph-entity-badge` | 详情面板实体类型徽章样式（Session 4 新增） |
| `ui/src/graph-viewer.js` | `switchLayout(mode)` | 切换图谱布局模式（force/hierarchical/radial）。更新按钮状态，停止旧 simulation，根据模式重新渲染。（Session 5 新增） |
| `ui/src/graph-viewer.js` | `findPath()` | 查找最短路径并高亮显示。调用 `get_shortest_path` IPC 获取路径，在图谱上高亮路径节点和边。（Session 5 新增） |
| `ui/src/graph-viewer.js` | `toggleCommunities()` | 切换社区检测着色。调用 `get_communities` IPC 获取社区映射，按社区 ID 为节点着色。再次点击关闭。（Session 5 新增） |
| `ui/src/graph-viewer.js` | `renderPathSelectors()` | 渲染路径分析实体选择器下拉框，填充所有节点实体。（Session 5 新增） |
| `ui/src/graph-viewer.js` | `applyPathHighlight()` | 应用路径高亮：路径节点和边高亮，其他暗淡。（Session 5 新增） |
| `ui/styles/graph-viewer.css` | `.graph-layout-panel` / `.graph-layout-btn` / `.graph-layout-active` | 布局切换面板 + 按钮样式（Session 5 新增） |
| `ui/styles/graph-viewer.css` | `.graph-path-panel` / `.graph-path-select` / `.graph-path-find-btn` | 路径分析面板 + 选择器 + 查找按钮样式（Session 5 新增） |
| `ui/styles/graph-viewer.css` | `.graph-node-on-path` / `.graph-edge-on-path` | 路径高亮节点和边样式（Session 5 新增） |
| `ui/styles/graph-viewer.css` | `.graph-community-panel` / `.graph-community-btn` | 社区检测面板 + 按钮样式（Session 5 新增） |

---

## 8. Agentic AI 前端模块（REQ-RAG-028~033 + REQ-PERF-001~012）

### 8.1 DAG 工作流可视化面板（REQ-RAG-030）

| 模块文件 | 函数 | 说明 |
|---|---|---|
| `ui/src/workflow-panel.js` | `openWorkflowPanel()` | 打开工作流全屏 overlay 面板。惰性创建 DOM，调用 `list_workflows` IPC 加载模板列表。 |
| `ui/src/workflow-panel.js` | `closeWorkflowPanel()` | 关闭工作流面板。支持 ESC 键触发。 |
| `ui/src/workflow-panel.js` | `loadWorkflowList()` | 调用 `list_workflows` IPC 获取工作流模板列表并渲染为可点击列表项。 |
| `ui/src/workflow-panel.js` | `newWorkflow()` | 创建新工作流模板：清空编辑器，生成默认 JSON 骨架。 |
| `ui/src/workflow-panel.js` | `saveWorkflow()` | 保存当前编辑器中的工作流定义。调用 `save_workflow_template` IPC，成功后刷新列表。 |
| `ui/src/workflow-panel.js` | `runWorkflow()` / `runWorkflowWithId(id)` | 运行工作流：调用 `run_workflow` IPC，监听 `workflow_step` 事件显示执行进度。 |
| `ui/src/workflow-panel.js` | `deleteWorkflow(id)` | 删除指定工作流模板。调用 `delete_workflow` IPC。 |
| `ui/src/workflow-panel.js` | `initWorkflowPanel()` | 初始化工作流面板事件监听（按钮点击 + ESC 关闭）。 |

### 8.2 持久化记忆管理面板（REQ-RAG-033）

| 模块文件 | 函数 | 说明 |
|---|---|---|
| `ui/src/memory-panel.js` | `renderMemorySettings(container)` | 渲染记忆管理设置区块到指定容器。调用 `get_memories` IPC 加载所有记忆条目，按 tier（Wing/Hall/Room）三列布局渲染。包含记忆开关 toggle。 |
| `ui/src/memory-panel.js` | `renderTierColumn(tier, memories)` | 渲染单个 tier 列：标题 + 清空按钮 + 记忆卡片列表。空状态显示占位文本。 |
| `ui/src/memory-panel.js` | `renderMemoryCard(mem, tier)` | 渲染单条记忆卡片：内容（2 行截断）+ 重要性进度条 + 提升按钮 + 删除按钮。 |
| `ui/src/memory-panel.js` | `initMemoryHandlers(memoryEnabled)` | 初始化记忆管理事件处理器：toggle 开关（`set_memory_enabled`）、提升（`promote_memory`）、删除（`delete_memory`）、清空 tier（`clear_memories` + 确认对话框）。 |
| `ui/src/memory-panel.js` | `updateMemToggle(toggle, enabled)` | 更新记忆 toggle 开关的视觉状态（bg-accent / bg-slate-600 + knob 位移）。 |

### 8.3 AutoDream 知识库整理面板（REQ-RAG-029）

| 模块文件 | 函数 | 说明 |
|---|---|---|
| `ui/src/dream-panel.js` | `openDreamPanel()` | 打开 AutoDream overlay 面板。调用 `get_dream_suggestions` IPC 加载历史建议。 |
| `ui/src/dream-panel.js` | `closeDreamPanel()` | 关闭 Dream 面板。支持 ESC 键触发。 |
| `ui/src/dream-panel.js` | `triggerDream()` | 触发 dream 分析。调用 `trigger_dream` IPC，监听 `dream_progress` 事件显示进度条。 |
| `ui/src/dream-panel.js` | `abortDream()` | 中止 dream 分析。调用 `abort_dream` IPC。 |
| `ui/src/dream-panel.js` | `loadDreamSuggestions()` | 加载历史建议列表并按 severity（critical/warning/info）分组渲染。 |
| `ui/src/dream-panel.js` | `initDreamPanel()` | 初始化 Dream 面板事件监听（按钮点击 + ESC 关闭 + dream_progress 事件监听）。 |

### 8.4 代码符号搜索面板（REQ-RAG-031）

| 模块文件 | 函数 | 说明 |
|---|---|---|
| `ui/src/symbol-search.js` | `openSymbolSearch()` | 打开符号搜索 overlay 面板。聚焦搜索输入框。 |
| `ui/src/symbol-search.js` | `closeSymbolSearch()` | 关闭符号搜索面板。支持 ESC 键触发。 |
| `ui/src/symbol-search.js` | `doSearch(query)` | 执行符号搜索。调用 `search_symbols` IPC，按符号类型（function/class/struct/variable）分组渲染结果。 |
| `ui/src/symbol-search.js` | `rebuildSymbolIndex()` | 重建符号索引。调用 `rebuild_symbol_index` IPC，显示进度提示。 |
| `ui/src/symbol-search.js` | `initSymbolSearch()` | 初始化符号搜索面板事件监听（搜索输入 debounce + ESC 关闭 + 重建按钮）。 |

### 8.5 性能优化设置面板（REQ-PERF-001~012）

| 模块文件 | 函数 | 说明 |
|---|---|---|
| `ui/src/perf-settings.js` | `renderPerfSettings(container)` | 渲染性能优化设置区块到指定容器。并行调用 `get_cache_stats` / `get_cache_settings` / `get_compression_ratio` / `get_retrieval_memory_stats` IPC 加载当前状态。包含：语义缓存 toggle + 统计、压缩比滑块、检索记忆 toggle + 统计、索引重建按钮组。 |
| `ui/src/perf-settings.js` | `initPerfHandlers(cacheEnabled)` | 初始化性能优化事件处理器：缓存 toggle（`set_cache_settings`）、清空缓存（`clear_cache`）、压缩比滑块（`set_compression_ratio`）、检索记忆 toggle（`set_retrieval_memory_enabled`）、重置记忆（`reset_retrieval_memory`）、索引重建按钮（`rebuild_bm25_index` / `rebuild_proposition_index` / `build_summary_tree`）。 |
| `ui/src/perf-settings.js` | `updateToggleVisual(toggle, enabled)` | 更新 toggle 开关的视觉状态（通用工具函数）。 |

### 8.6 输入区 Toggle 扩展（REQ-RAG-030/033）

| 模块文件 | 函数 | 说明 |
|---|---|---|
| `ui/src/input-toggles.js` | `createInputToggle(config, state)` | 创建单个 toggle 按钮。支持 `hybrid` / `agent` / `sub_agent` / `memory` / `web_search` 五种 toggle 类型。 |
| `ui/src/input-toggles.js` | `getToggleState(settingKey)` | 查询指定 toggle 的当前状态。 |
| `ui/src/input-toggles.js` | `TOGGLE_CONFIG` | Toggle 配置数组：`hybrid`（`set_hybrid_search` IPC）→ `agent`（`set_agent_enabled` IPC）→ `sub_agent`（`set_sub_agent_enabled` IPC）→ `memory`（`set_memory_enabled` IPC）→ `web_search`（`set_web_search_enabled` IPC）。 |

---

## 9. PDF 导出模块（REQ-EXP-005）

### 9.1 PDF 导出引擎

源文件：`ui/src/export.js`

| 函数 | 说明 |
|---|---|
| `exportConversationToPdf(conversationId, title?)` | 导出当前对话为 PDF：加载会话全部消息（`get_messages` IPC）→ 构建打印 HTML（复用 Markdown 渲染管线 marked → DOMPurify → highlight.js）→ 隐藏 iframe 写入 HTML → `iframe.contentWindow.print()` 触发浏览器打印对话框。空对话显示 toast 错误，不崩溃。 |
| `exportDocumentToPdf(docId)` | 导出知识库文档为 PDF：加载文档全文（`get_documents` + `get_document_chunks` IPC）→ 构建打印 HTML → `print()`。 |
| `buildPrintHtml(title, contentBlocks, options?)` | 构建打印专用 HTML 文档：内联 `@page` CSS（页面大小 + 页边距）+ `@media print` 隐藏规则 + 消息块 HTML（用户/助手角色标签 + Markdown 渲染内容 + 引用来源）+ 页头（标题 + 导出时间）+ 页脚。 |
| `initExportButtons()` | 初始化导出按钮事件监听：`#exportPdfBtn` 绑定对话导出、`[data-action="export-pdf-doc"]` 委托文档导出。在 `main.js` `boot()` 中调用。 |
| `getPdfPageSize()` | 读取 PDF 页面大小设置（localStorage `export.pdf_page_size`，默认 `A4`）。 |
| `setPdfPageSize(size)` | 设置 PDF 页面大小（`A4` / `Letter`），持久化到 localStorage。 |
| `getPdfIncludeSources()` | 读取是否包含引用来源设置（localStorage `export.pdf_include_sources`，默认 `true`）。 |
| `setPdfIncludeSources(include)` | 设置是否包含引用来源，持久化到 localStorage。 |

### 9.2 打印样式

源文件：`ui/styles/print.css`

| 选择器 | 说明 |
|---|---|
| `@media print` | 打印媒体查询，当用户通过 `Ctrl+P` 或 `window.print()` 触发打印时生效 |
| `#sidebar, #inputBar, #settingsPanel, #kbModal, ...` | 隐藏非打印 UI 元素（`display: none !important`） |
| `body` | 打印排版优化：serif 字体（Times New Roman + PingFang SC）、12pt 字号、1.6 行距、黑色文字 |
| `.msg-block` | 对话消息分页控制：`page-break-inside: avoid` 防止跨页截断 |
| `pre, table` | 代码块和表格分页控制：`page-break-inside: avoid` |
| `.sources` | 引用来源样式：顶部边框 + 10pt 字号 |

---

## 10. Wiki 双向链接模块（REQ-ING-020）

### 10.1 Wiki-links 模块

源文件：`ui/src/wiki-links.js`

| 函数 | 说明 |
|---|---|
| `enhanceWikiLinks(mdEl)` | 将 Markdown 渲染后的 DOM 中的 `[[wiki-link]]` 文本替换为可点击链接。遍历文本节点查找 `[[...]]` 模式，跳过代码块（pre/code）和已有的 wiki-link 元素，替换为 `<a class="wiki-link" data-wiki-target="...">` 元素，点击触发 `wiki-link-click` 自定义事件。 |
| `getForwardLinks(docId)` | IPC 封装：调用 `get_forward_links` 命令，返回该文档引用的所有目标文档列表。 |
| `getBacklinks(docName)` | IPC 封装：调用 `get_backlinks` 命令，返回引用该文档的所有来源文档列表。 |
| `rebuildWikiLinks()` | IPC 封装：调用 `rebuild_wiki_links` 命令，重建全量 wiki-link 索引，返回链接总数。 |
| `renderBacklinksPanel(docName, container)` | 渲染反向链接面板：调用 `getBacklinks` 获取数据 → 构建 HTML 列表（backlink-item + 日期）→ 无链接时显示空提示。 |
| `renderForwardLinksPanel(docId, container)` | 渲染正向链接面板：调用 `getForwardLinks` 获取数据 → 构建 HTML 列表（forward-link-item）→ 无链接时显示空提示。 |

### 10.2 Markdown 渲染集成

`ui/src/markdown.js` 中 `renderMarkdown` 函数在渲染流程中调用 `enhanceWikiLinks(mdEl)`，
将 `[[wiki-link]]` 语法渲染为可点击的 `.wiki-link` 链接元素。

### 10.3 i18n 文案

| 键 | 英文 | 中文 |
|---|---|---|
| `wiki.link_tooltip` | Wiki link | Wiki 链接 |
| `wiki.backlinks` | Backlinks | 反向链接 |
| `wiki.forward_links` | Forward Links | 正向链接 |
| `wiki.no_backlinks` | No backlinks found | 暂无反向链接 |
| `wiki.no_forward_links` | No forward links found | 暂无正向链接 |
| `wiki.rebuild` | Rebuild Wiki Links | 重建 Wiki 链接 |
| `wiki.rebuilding` | Rebuilding wiki links... | 正在重建 Wiki 链接... |
| `wiki.rebuild_success` | Wiki links rebuilt ({count} links) | Wiki 链接已重建（{count} 条链接）|
| `wiki.rebuild_failed` | Failed to rebuild wiki links | 重建 Wiki 链接失败 |
