# Settings 键与 Tauri 事件参考

> **状态**：活跃 | **最后审阅**：2026-08-10（v9 新增 Q01-Q11/B05-B08/B09-B10/S69-S70 设置键：security.posture / rag.contextual_retrieval 等；v8 新增 `llm.kv_cache_enabled` 设置键；v7 新增 `llm.paged_attn` / `llm.block_size` / `llm.gpu_memory_ctx` / `llm.sampling` 设置键）
>
> 本文档记录 SQLite `settings` 表的所有键名、用途与值格式，以及所有 Tauri 事件。
> 源码：`crates/tauri-app/src/state.rs`、`crates/tauri-app/src/commands.rs`、`crates/infra/src/sqlite_storage.rs`。

---

## 1. Settings 表键名

`settings` 表结构：`(key TEXT PRIMARY KEY, value TEXT)`，经 `Storage::set_setting()` / `get_setting()` 读写。

| 键名 | 值格式 | 用途 | 写入处 | 读取处 |
|------|--------|------|--------|--------|
| `llm.api_key` | 明文字符串（存入前经 AES-256-GCM 加密为密文 base64） | BYOK API Key，允许空串（兼容本地 Ollama） | `update_llm_config` 命令 | `AppState::new()` 启动恢复 |
| `llm.base_url` | URL 字符串（如 `https://api.openai.com/v1`） | OpenAI 兼容端点 | `update_llm_config` 命令 | `AppState::new()` 启动恢复 |
| `llm.model` | 模型名字符串（如 `gpt-4o`） | LLM 模型名 | `update_llm_config` 命令 | `AppState::new()` 启动恢复 |
| `license.is_pro` | `"true"` 或不存在 | Pro 授权状态 | `activate_pro` 命令验证通过后写入 | `AppState::new()` 启动恢复 `is_pro` |
| `mm.vlm_enabled` | `"true"` / `"false"` / 不存在 | VLM 图片理解增强开关（REQ-MM-003；默认关闭，用户主动开启后图片发送到 BYOK LLM 端点） | `set_vlm_enabled` 命令（前端设置面板开关） | `AppState::vision_provider()` 每次调用时检查，支持运行时切换 |
| `rag.hybrid_search` | `"true"` / `"false"` / 不存在 | 混合检索开关（REQ-RAG-010；默认启用，向量+关键词 RRF 融合检索；关闭后退化为纯向量检索） | `set_hybrid_search` 命令（前端设置面板开关） | `chat_inner` 每次调用时读取，构建 `HybridRetriever` 时传入 `set_hybrid_enabled()` |
| `rag.hyde_enabled` | `"true"` / `"false"` / 不存在 | HyDE 查询改写开关（REQ-RAG-021；默认关闭，启用后向量检索使用 LLM 生成的假设性答案文档嵌入；关键词检索仍用原始查询） | `set_hyde_enabled` 命令（前端设置面板开关） | `chat_inner` 每次调用时读取，构建 `HybridRetriever` 时注入 `HydeRewriter` |
| `vec.embedding_model` | `"all-MiniLM-L6-v2"` / `"bge-small-zh-v1.5"` / `"e5-small-v2"` / `"custom:{name}"` / 不存在 | 嵌入模型标识（REQ-VEC-012 + REQ-VEC-014；默认 `all-MiniLM-L6-v2`，切换后销毁 embedder 实例，下次 `embedder()` 用新模型初始化；`custom:{name}` 前缀表示使用用户上传的自定义 ONNX 模型，Pro 门控） | `set_embedding_model` 命令（前端设置面板下拉选择 / `switchToCustomModel` 函数） | `AppState::embedder()` / `init_embedder_with_progress()` 初始化时读取；`ModelStore::parse_embedding_model()` 解析 `custom:` 前缀 |
| `sync.watched_folders` | JSON 数组字符串 | 监听文件夹列表（REQ-SYNC-001；`[{path, last_synced_at}, ...]`） | `add_watched_folder` / `remove_watched_folder` 命令 | `get_watched_folders` 命令读取 |
| `rag.rerank_enabled` | `"true"` / `"false"` / 不存在 | Cross-Encoder 重排序开关（REQ-RAG-020；默认关闭，启用后 RRF 融合后插入 bge-reranker-base 精排阶段） | `set_rerank_enabled` 命令（前端设置面板开关） | `chat_inner` 每次调用时读取 |
| `rag.agent_enabled` | `"true"` / `"false"` / 不存在 | Agentic RAG 多步推理开关（REQ-RAG-022；默认关闭，启用后走 ReAct 循环 Thought→Action→Observation） | `set_agent_enabled` 命令（前端设置面板开关） | `chat_inner` 每次调用时读取 |
| `rag.coordinator_enabled` | `"true"` / `"false"` / 不存在 | 多代理协调模式开关（REQ-RAG-025；默认关闭，启用后走四阶段流水线 Research→Synthesis→Implementation→Verification） | `set_coordinator_mode` 命令（前端设置面板开关） | `chat_inner` 每次调用时读取 |
| `rag.context_token_limit` | 数字字符串（如 `"4096"`）/ 不存在 | 对话历史 token 截断阈值（范围 2048-32768，默认 4096） | `set_context_token_limit` 命令 | `chat_inner` 每次调用时读取 |
| `ui.locale` | `"zh-CN"` / `"en"` / 不存在 | 界面语言偏好（REQ-I18N-001；默认 `zh-CN`） | `set_locale` 命令（前端设置面板下拉） | `get_locale` 命令 + 前端 i18n 模块启动时读取 |
| `ui.sidebar_collapsed` | `"true"` / `"false"` / 不存在 | 侧栏折叠状态（REQ-NAV-001 AC-4；默认 `false` 展开） | `set_sidebar_collapsed` 命令（前端 `toggleSidebar()` 调用） | `get_sidebar_collapsed` 命令 + 前端 `restoreSidebarState()` 启动时读取 |
| `ui.theme` | `"dark"` / `"light"` / `"system"` / 不存在 | 界面主题模式（REQ-UI-011；默认 `dark` 暗色主题。`light` 为浅色主题，`system` 跟随 `prefers-color-scheme` 自动切换） | `set_theme` 命令（前端设置面板「外观」区域三选一） | `get_theme` 命令 + 前端 `initTheme()` 启动时读取 |
| `voice.rate` | 数字字符串（如 `"1.0"`）/ 不存在 | TTS 朗读语速（REQ-RAG-035；范围 0.5~2.0，默认 1.0。存储在 localStorage，不经过后端 IPC） | 前端 `initVoiceSettings()` 设置面板滑块 | 前端 `getVoiceRate()` 朗读时读取 |
| `export.pdf_page_size` | `"A4"` / `"Letter"` / 不存在 | PDF 导出页面大小（REQ-EXP-005；默认 `A4`。存储在 localStorage，不经过后端 IPC） | 前端 `initPdfExportSettings()` 设置面板下拉 | 前端 `getPdfPageSize()` 导出时读取 |
| `export.pdf_include_sources` | `"true"` / `"false"` / 不存在 | PDF 导出是否包含引用来源（REQ-EXP-005；默认 `true`。存储在 localStorage，不经过后端 IPC） | 前端 `initPdfExportSettings()` 设置面板 toggle | 前端 `getPdfIncludeSources()` 导出时读取 |
| `security.auto_lock_enabled` | `"true"` / `"false"` / 不存在 | 自动锁定开关（REQ-SEC-016；默认 `true`） | `set_auto_lock_config` 命令 | `SecurityManager` 启动时读取 |
| `security.auto_lock_timeout` | 数字字符串（秒，如 `"300"`）/ 不存在 | 自动锁定空闲超时秒数（默认 300） | `set_auto_lock_config` 命令 | `SecurityManager` 启动时读取 |
| `security.lock_on_sleep` | `"true"` / `"false"` / 不存在 | 系统休眠时自动锁定（默认 `true`） | `set_auto_lock_config` 命令 | `SecurityManager` 启动时读取 |
| `security.clipboard_clear_enabled` | `"true"` / `"false"` / 不存在 | 剪贴板自动清除开关（REQ-SEC-018；默认 `true`） | `set_clipboard_config` 命令 | `SecurityManager` 启动时读取 |
| `security.clipboard_clear_timeout` | 数字字符串（秒，如 `"30"`）/ 不存在 | 剪贴板清除超时秒数（默认 30） | `set_clipboard_config` 命令 | `SecurityManager` 启动时读取 |
| `security.panic_wipe_password` | 加密字符串 / 不存在 | 紧急销毁密码（REQ-SEC-019；设置后用此密码解锁触发数据销毁） | `set_panic_wipe_password` 命令 | `SecurityManager::unlock()` 解锁时校验 |
| `llm.mode` | `"remote"` / `"local"` / 不存在 | LLM 推理模式（REQ-LLM-003；默认 `remote`，BYOK 远程 API；`local` 使用 mistral.rs 本地推理，Pro 功能） | `set_llm_mode` 命令（前端设置面板模式切换） | `AppState::get_llm_mode()` / `chat_inner` 每次调用时读取 |
| `llm.local_model` | 文件名字符串 / 不存在 | 当前选中的本地 GGUF 模型文件名（REQ-LLM-003；空串表示未选择） | `set_local_model` 命令（前端设置面板模型选择） | `AppState::local_llm()` 加载模型时读取 |
| `llm.paged_attn` | `"true"` / `"false"` / 不存在 | PagedAttention KV cache 管理开关（REQ-LLM-005；默认 `false`，启用后降低多轮对话首 token 延迟，仅 GPU 模式生效） | `set_paged_attn` 命令（Pro 门控） | `AppState::local_llm()` 创建引擎时读取 |
| `llm.block_size` | 数字字符串（如 `"32"`）/ 不存在 | PagedAttention KV cache 块大小（REQ-LLM-005；默认 `32`，支持 8/16/32，仅 `llm.paged_attn=true` 时生效） | `set_paged_attn` 命令（Pro 门控） | `AppState::local_llm()` 创建引擎时读取 |
| `llm.gpu_memory_ctx` | 数字字符串（如 `"4096"`）/ 不存在 | PagedAttention GPU 上下文 token 数（REQ-LLM-005；默认 `4096`，控制 KV cache 可使用的 GPU 显存量） | `set_paged_attn` 命令（Pro 门控） | `AppState::local_llm()` 创建引擎时读取 |
| `llm.sampling` | JSON 字符串 / 不存在 | 本地推理采样参数 JSON（REQ-LLM-006；`{temperature, top_p, top_k, max_tokens, frequency_penalty, presence_penalty}`，各字段为 Optional） | `set_sampling_params` 命令（Pro 门控） | `AppState::local_llm()` 创建引擎时读取 + `chat_stream` 使用 |
| `llm.kernel_mode` | `"mistral"` / `"custom"` / 不存在 | 推理内核模式（REQ-LLM-007；默认 `mistral`，使用 mistral.rs 内核；`custom` 使用自研 GEMV 内核，Phase 3） | `set_kernel_mode` 命令（Pro 门控） | `LocalLlmEngine::kernel_mode()` 引擎创建时读取 + `chat_stream` 路由 |
| `llm.kv_cache_enabled` | `"true"` / `"false"` / 不存在 | KV Cache 跨会话复用开关（REQ-LLM-009；默认 `false`，启用后保存/加载会话 KV cache 元数据快照，减少多轮对话首 token 延迟） | `set_kv_cache_enabled` 命令（Pro 门控） | `AppState` KV cache 操作时读取 |
| `cache.enabled` | `"true"` / `"false"` / 不存在 | 缓存总开关（REQ-PERF-001；默认 `true`，三级缓存 L0 精确 + L1 语义 + L3 检索结果） | `set_cache_settings` 命令 | `chat_inner` 每次调用时读取 |
| `cache.ttl_secs` | 数字字符串（如 `"86400"`）/ 不存在 | 缓存 TTL 秒数（REQ-PERF-001；默认 86400 = 24h） | `set_cache_settings` 命令 | `SqliteCache` 查询时检查过期 |
| `cache.semantic_threshold` | 浮点数字符串（如 `"0.92"`）/ 不存在 | 语义匹配余弦相似度阈值（REQ-PERF-001；默认 0.92） | `set_cache_settings` 命令 | `SqliteCache` L1 语义缓存匹配 |
| `cache.privacy_mode` | `"true"` / `"false"` / 不存在 | 隐私模式（REQ-PERF-001；默认 `false`，`true` 时禁用缓存） | `set_cache_settings` 命令 | `chat_inner` 检查是否跳过缓存 |
| `compression.ratio` | 浮点数字符串（如 `"3.0"`）/ 不存在 | Prompt 压缩比（REQ-PERF-002；1.0=禁用 / 2.0=保守 / 3.0=平衡 / 5.0=激进，默认 1.0） | `set_compression_ratio` 命令 | `chat_inner` 构建 ChatEngine 时读取 |
| `rag.progressive_injection` | `"true"` / `"false"` / 不存在 | 渐进式上下文注入开关（REQ-PERF-010；默认 `false`，启用后初始注入 2 chunks，检测"信息不足"时追加） | `set_progressive_injection` 命令 | `ChatEngine.with_progressive()` builder |
| `rag.speculative_enabled` | `"true"` / `"false"` / 不存在 | Speculative RAG 开关（REQ-PERF-011；默认 `false`，启用后草稿模型快速生成 → 大模型验证/修正） | `set_speculative_enabled` 命令 | `SpeculativeRagEngine` 分支 |
| `rag.retrieval_memory_enabled` | `"true"` / `"false"` / 不存在 | 自进化检索记忆开关（REQ-PERF-012；默认 `false`，启用后自动记录查询类型 × 检索方法效果，自适应选择最佳策略） | `set_retrieval_memory_enabled` 命令 | `chat_inner` 检索前选择方法 |
| `rag.eval_settings` | JSON 字符串 / 不存在 | RAG 评估指标配置（REQ-RAG-045；默认不存在，使用 `RagEvalSettings::default()`。JSON 包含 7 个布尔开关：`enable_faithfulness` / `enable_answer_relevance` / `enable_context_precision` / `enable_context_recall` / `enable_retrieval_metrics` / `enable_embedding_metrics` / `enable_keyword_overlap`） | `set_rag_eval_settings` 命令 | `load_eval_settings()` 读取 → `RagEvaluator::with_settings()` |
| `rag.graph_retriever_enabled` | `"true"` / `"false"` / 不存在 | 知识图谱图遍历检索开关（REQ-RAG-027；默认 `false`，启用后检索时沿实体关系图边扩展到关联 chunk，作为 RRF 融合第四路通道，BFS 最大 2 跳 + 置信度×距离衰减加权） | `set_graph_retriever_enabled` 命令 | `chat_inner` 图扩展分支 + `GraphRetriever::expand()` |
| `rag.quality_gate_enabled` | `"true"` / `"false"` / 不存在 | RAG 质量门控开关（REQ-RAG-028；默认 `false`，启用后检索结果质量低于阈值时记录告警日志，三维加权评分：覆盖率 0.4 + 多样性 0.3 + 方差 0.3，阈值 0.6） | `set_quality_gate_enabled` 命令 | `ChatEngine::with_quality_gate()` 注入 + `quality_gate::evaluate()` 评估 |
| `memory.enabled` | `"true"` / `"false"` / 不存在 | 持久化记忆系统开关（REQ-RAG-033；默认 `false`，启用后 ChatEngine 检索相关跨会话记忆注入 system prompt，AutoDream 后台自动整合记忆 Wing→Hall→Room 提升 + 低分遗忘） | `set_memory_enabled` 命令 | `chat_inner` 从 `get_settings_batch` 读取 → `ChatEngine::with_memory()` 注入 + `DreamEngine::consolidate_memories()` |
| `rag.sub_agent_enabled` | `"true"` / `"false"` / 不存在 | 子代理舰队开关（REQ-RAG-025 扩展；默认 `false`，启用后 Coordinator Research 阶段每个子查询由独立 SubAgent 处理，通过 mailbox 消息传递协调。**仅在 `rag.coordinator_enabled` 为 true 时生效**） | `set_sub_agent_enabled` 命令 | `chat_inner` 从 `get_settings_batch` 读取 → `CoordinatorEngine::with_sub_agent_timeout(120)` |
| `rag.web_search_enabled` | `"true"` / `"false"` / 不存在 | 网页搜索集成开关（REQ-RAG-036；默认 `false`，启用后当本地检索 top-1 score < 0.3 阈值时自动调用 DuckDuckGo Instant Answer API 搜索互联网，结果通过 RRF 融合到本地检索（Web 权重 0.4），搜索失败优雅降级为仅本地结果） | `set_web_search_enabled` 命令（前端输入区 toggle 开关） | `chat_inner` 从 `get_settings_batch` 读取 → `DuckDuckGoProvider::new()` → `web_search::search_and_fuse()` |
| `rag.contextual_retrieval` | `"true"` / `"false"` / 不存在 | Contextual Retrieval 开关（REQ-RAG-041；默认 `true`，嵌入时拼接文档名上下文前缀 `build_contextual_text(doc_name, chunk_content)`，使向量包含文档上下文信息，检索失败率 ↓49%（Anthropic Contextual Retrieval 论文）。关闭后嵌入使用纯 chunk content。变更后需调用 `rebuild_contextual_embeddings` 重建已有向量） | `set_contextual_retrieval` 命令（性能优化设置面板 toggle 开关） | `embed_document_chunks()` 条件化嵌入文本 + `rebuild_contextual_embeddings` IPC 重建 |
| `log.level` | `"debug"` / `"info"` / `"warn"` / `"error"` / 不存在 | 日志级别（REQ-OBS-001；默认 `info`，可在设置面板切换为 `debug`） | `set_log_level` 命令 | `LocalLogger::init()` 启动时读取 + `set_level()` 运行时切换 |
| `window.x` | 整数字符串（如 `"100"`）/ 不存在 | 窗口左上角 X 坐标（REQ-WIN-001-AC-3；关闭时保存，启动恢复） | `lib.rs save_window_state()` | `lib.rs restore_window_state()` |
| `window.y` | 整数字符串（如 `"200"`）/ 不存在 | 窗口左上角 Y 坐标（REQ-WIN-001-AC-3） | `lib.rs save_window_state()` | `lib.rs restore_window_state()` |
| `window.width` | 整数字符串（如 `"1280"`）/ 不存在 | 窗口宽度（REQ-WIN-001-AC-3；≥ 960） | `lib.rs save_window_state()` | `lib.rs restore_window_state()` |
| `window.height` | 整数字符串（如 `"800"`）/ 不存在 | 窗口高度（REQ-WIN-001-AC-3；≥ 640） | `lib.rs save_window_state()` | `lib.rs restore_window_state()` |
| `window.maximized` | `"true"` / `"false"` / 不存在 | 窗口最大化状态（REQ-WIN-001-AC-5；最大化时优先恢复，跳过 x/y/width/height） | `lib.rs save_window_state()` | `lib.rs restore_window_state()` |
| `security.posture` | `"dangerous"` / `"auto"` / `"strict"` / 不存在 | 安全态势分层（Q05 借鉴 QM SecurityPosture；默认 `auto`。`dangerous` 无筛查无审批 / `auto` 有筛查无审批 / `strict` 有筛查有审批。子 scope 只能收紧不能放松父 scope） | `set_security_posture` 命令 | `AppState::get_security_posture()` / `resolve_security_policy()` |
| `stt.api_key` | 明文字符串 / 不存在 | 语音转写 API Key（REQ-RAG-034；OpenAI Whisper API 兼容端点） | `set_stt_config` 命令 | `transcribe_audio` 命令读取 |
| `stt.base_url` | URL 字符串 / 不存在 | 语音转写 API 端点（REQ-RAG-034） | `set_stt_config` 命令 | `transcribe_audio` 命令读取 |
| `stt.model` | 模型名字符串 / 不存在 | 语音转写模型名（REQ-RAG-034；默认 `whisper-1`） | `set_stt_config` 命令 | `transcribe_audio` 命令读取 |
| `stt.language` | 语言代码 / 不存在 | 语音转写语言提示（REQ-RAG-034；如 `zh` / `en`，可选） | `set_stt_config` 命令 | `transcribe_audio` 命令读取 |

### 1.1 API Key 加密存储（REQ-SEC-001 / REQ-UI-008）

`llm.api_key` 在写入 settings 表前经 AES-256-GCM 加密：
- 随机生成 32 字节密钥，存于数据目录 `secret.key` 文件（权限 0600）
- 加密格式：`base64(nonce ‖ ciphertext)`（12 字节 nonce + 密文 + 16 字节 tag）
- `get_settings` 命令返回 `api_key_masked`（`****` + 末四位），**严禁回传明文**

---

## 2. Tauri 事件

所有事件经 `app.emit()` 发射，前端经 `listen()` 监听。

### 2.1 文档导入事件

| 事件 | 负载 | 触发时机 |
|------|------|----------|
| `doc-status-changed` | `DocStatusPayload { status: String, message: String, sub_phase?: String }` | 文档导入/索引状态变更 |
| `import-progress` | `ImportProgressPayload { completed: usize, total: usize, current_file: String, cancelled: bool }` | 批量导入逐文件进度（REQ-ING-006） |
| `embedding_progress` | `EmbeddingProgressPayload { doc_id: String, doc_name: String, embedded: usize, total: usize }` | 向量化微批次进度（GB 级文档加速） |
| `model_download_progress` | `DownloadEvent` | 模型下载/加载进度（REQ-VEC-008） |
| `model_load_progress` | `ModelLoadStatusPayload { model: String, status: String, error?: String }` | 本地 LLM 模型加载状态（REQ-LLM-003 AC-3/AC-4；status: loading/ready/error；后端在 `LocalLlmEngine` 加载模型时发射） |
| `sync_progress` | `SyncProgressPayload { phase, folder_path, added, updated, deleted, skipped, message }` | 文件监听同步进度（REQ-SYNC-002 AC-7） |

**status 值**：
- `indexing` — 正在解析/分块/嵌入
- `done` — 索引完成
- `error` — 索引失败

### 2.2 对话事件

| 事件 | 负载 | 触发时机 |
|------|------|----------|
| `chat_phase` | `ChatPhasePayload { phase: String, message: String }` | 对话进度阶段（首个 token 前） |
| `chat_context_truncated` | `{ original_count, truncated_count, token_limit }` | 对话历史超过 token 限制被截断时 |
| `chat_sources` | `Vec<RetrievalResult>` | 检索引用来源 |
| `chat_token` | `String` | 流式 token 推送 |
| `chat_error` | `String` | 错误消息（含中断 `⏹ 生成已中断`） |
| `chat_done` | `()` | 对话结束信号 |
| `agent_step` | `AgentStepPayload { step_type, content, tool?, input? }` | Agentic RAG 推理步骤（REQ-RAG-022；thought/action/observation/answer） |
| `audit_phase` | `ChatPhasePayload { phase, message }` | 审计进度阶段（extracting / comparing / reporting） |

**chat_phase 阶段**：
- `preparing` — "初始化向量化引擎…"（首次使用需下载模型）
- `retrieving` — "检索知识库…"
- `generating` — "正在生成回答…"

---

## 3. 事件流时序

### 3.1 文档导入流

```
import_files 调用
  → import-progress { completed: 0, total: N, current_file: "..." }
  → doc-status-changed { status: "indexing", message: "正在导入 {文件名}" }
  → [解析 → 分块 → chunks 入库]
  → embedding_progress { doc_id, doc_name, embedded: 64, total: M }  // 微批次进度
  → embedding_progress { doc_id, doc_name, embedded: 128, total: M }  // 每 64 条推送一次
  → doc-status-changed { status: "done" | "error", message: ... }
```

### 3.2 RAG 对话流

```
chat 调用
  → chat_phase { phase: "preparing", message: "初始化向量化引擎…" }
  → chat_context_truncated? { original_count, truncated_count, token_limit }  // 仅超限时
  → [Agentic 模式：agent_step { thought } → agent_step { action } → agent_step { observation }] × N
  → chat_phase { phase: "retrieving", message: "检索知识库…" }
  → chat_sources [{ chunk, score, doc_name }, ...]
  → chat_phase { phase: "generating", message: "正在生成回答…" }
  → chat_token("...") × N
  → chat_done()
```

> **注**：Agentic 模式（`rag.agent_enabled = true`）下，`agent_step` 事件在 `chat_phase(retrieving)` 之前推送推理步骤；标准 RAG 模式跳过此阶段。

### 3.3 中断流

```
abort_chat 调用
  → CancellationToken.cancel()
  → chat_error("⏹ 生成已中断")
  → chat_done()
  → persist_exchange() 保存已生成内容
```
