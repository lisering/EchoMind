# IPC 命令契约

> **状态**：活跃 | **最后审阅**：2026-08-10（v10 全量同步 187 个命令：补充 Q01-Q11/B05-B08/B09-B10/S69-S70 新增 37 个命令 + 安全态势/Shadow 筛查/Scratch 记忆/Burst Buffer/语音转写/网页搜索/文档标签/会话待办/预算追踪/Token 预算/Trace 系统/主题切换/下载管理；v9 补充 KV Cache 跨会话复用 5 个命令；v8 补充 model_load_progress 事件）
>
> 本文档定义所有 Tauri IPC 命令的签名、参数、返回值与事件负载。
> 源码：`crates/tauri-app/src/commands/` + `crates/tauri-app/src/lib.rs`。

---

## 1. 命令总览（187 个，含 12 个编译时 Pro 门控）

| 分类 | 命令 | 参数 | 返回值 |
|---|---|---|---|
| 导入 | `import_files` | `paths: Vec<String>` | `Vec<String>` (成功导入的文件名) |
| 导入 | `abort_import` | — | `()` |
| 文件 | `get_file_sizes` | `paths: Vec<String>` | `Vec<(String, u64)>` (路径→大小) |
| 文件 | `get_file_size_limits` | — | `(u64, u64)` (警告阈值, 硬上限) |
| 文档 | `get_documents` | — | `Vec<Document>` |
| 文档 | `delete_document` | `id: String` | `()` |
| 文档 | `retry_index` | `id: String` | `()` |
| 文档 | `reclassify_document` | `doc_id: String` | `String` (分类领域) |
| 对话 | `chat` | `query: String, history: Vec<ChatMessage>, conversation_id: String` | `()` |
| 对话 | `abort_chat` | `conversation_id: String` | `()` |
| 会话 | `create_conversation` | `workspace_id: String` | `String` (会话 ID) |
| 会话 | `get_conversations` | `workspace_id: String` | `Vec<Conversation>` |
| 会话 | `get_conversations_paginated` | `workspace_id: String, limit: Option<usize>, offset: Option<usize>` | `PaginatedResult<Conversation>` |
| 会话 | `get_messages` | `conversation_id: String` | `Vec<ChatMessage>` |
| 会话 | `get_messages_paginated` | `conversation_id: String, limit: Option<usize>, offset: Option<usize>` | `PaginatedResult<ChatMessage>` |
| 会话 | `edit_user_message` | `conversation_id: String, turn_group: String, new_content: String, original_message_id: Option<String>` | `i32` (新版本号) |
| 会话 | `delete_conversation` | `id: String` | `()` |
| 导出 | `export_conversation_markdown` | `conversation_id: String` | `(String, String)` (Markdown 内容, 默认文件名) |
| 导出 | `save_text_file` | `path: String, content: String` | `()` |
| 配置 | `test_llm_connection` | `api_key: String, base_url: String, model: String` | `String` |
| 配置 | `update_llm_config` | `config: LlmConfig` | `()` |
| 配置 | `get_settings` | — | `SettingsPayload` |
| 配置 | `set_vlm_enabled` | `enabled: bool` | `()` |
| 配置 | `set_hybrid_search` | `enabled: bool` | `()` |
| 配置 | `set_hyde_enabled` | `enabled: bool` | `()` |
| 配置 | `set_rerank_enabled` | `enabled: bool` | `()` |
| 配置 | `set_agent_enabled` | `enabled: bool` | `()` |
| 配置 | `set_coordinator_mode` | `enabled: bool` | `()` |
| 配置 | `set_sub_agent_enabled` | `enabled: bool` | `()` |
| 配置 | `set_embedding_model` | `model: String` | `()` |
| 嵌入模型 | `upload_custom_embedding_model` | `name: String, onnx_path: String, tokenizer_files: Vec<String>` | `CustomModelInfo` (Pro) |
| 嵌入模型 | `list_custom_models` | — | `Vec<CustomModelInfo>` (Pro) |
| 嵌入模型 | `delete_custom_model` | `name: String` | `()` (Pro) |
| 配置 | `set_context_token_limit` | `limit: usize` | `()` |
| i18n | `get_locale` | — | `String` (如 "zh-CN") |
| i18n | `set_locale` | `locale: String` | `()` |
| 导航 | `get_sidebar_collapsed` | — | `bool` |
| 导航 | `set_sidebar_collapsed` | `collapsed: bool` | `()` |
| 授权 | `activate_pro` | `license_key: String` | `bool` |
| 授权 | `deactivate_pro` | — | `()` |
| 授权 | `get_pro_status` | — | `bool` |
| 审计 | `audit_document` | `doc_id: String, doc_name: String` | `()` |
| 审计 | `abort_audit` | `doc_id: String` | `()` |
| 模型管理 | `init_embedder` | — | `()` |
| 模型管理 | `get_model_cache_info` | — | `ModelCacheInfo` |
| 模型管理 | `clear_model_cache` | `model_name: Option<String>` | `u64` (释放字节数) |
| 文件监听 | `add_watched_folder` | `path: String` | `SyncResult` |
| 文件监听 | `remove_watched_folder` | `path: String` | `()` |
| 文件监听 | `get_watched_folders` | — | `Vec<WatchedFolderInfo>` |
| 安全 | `get_security_status` | — | `SecurityStatus` |
| 安全 | `set_auto_lock_config` | `enabled: bool, timeout_secs: u64, lock_on_sleep: bool` | `()` |
| 安全 | `lock_app` | — | `()` |
| 安全 | `unlock_app` | — | `()` |
| 安全 | `record_activity` | — | `()` |
| 安全 | `detect_pii` | `text: String` | `{ detections, redacted }` |
| 安全 | `set_panic_wipe_password` | `password: String` | `()` |
| 安全 | `clear_panic_wipe_password` | — | `()` |
| 安全 | `is_panic_wipe_enabled` | — | `bool` |
| 安全 | `set_clipboard_config` | `enabled: bool, clear_after_secs: u64` | `()` |
| 安全 | `get_audit_logs` | `limit: Option<usize>` | `Vec<AuditEntry>` |
| 安全 | `clear_audit_logs` | — | `()` |
| 安全 | `check_password_strength` | `password: String` | `{ level, percentage, suggestions }` |
| 本地LLM | `list_local_models` | — | `Vec<ModelInfo>` |
| 本地LLM | `get_recommended_models` | — | `Vec<RecommendedModel>` |
| 本地LLM | `download_model` | `url: String, filename: String` | `()` |
| 本地LLM | `delete_model` | `filename: String` | `()` |
| 本地LLM | `set_llm_mode` | `mode: String` | `()` |
| 本地LLM | `get_llm_mode` | — | `String` |
| 本地LLM | `set_local_model` | `filename: String` | `()` |
| 本地LLM | `get_local_llm_device_kind` | — | `String` (cpu/metal/cuda) |
| 本地LLM | `set_paged_attn` | `enabled: bool, block_size: usize, gpu_memory_ctx: usize` | `()` |
| 本地LLM | `set_sampling_params` | `params: LlmSamplingParams` | `()` |
| 本地LLM | `set_kernel_mode` | `mode: String` | `()` |
| 本地LLM | `get_kernel_mode` | — | `String` |
| 本地LLM (Pro) | `save_kv_cache` | `conversation_id: String` | `()` |
| 本地LLM (Pro) | `load_kv_cache` | `conversation_id: String` | `bool` |
| 本地LLM (Pro) | `clear_kv_cache` | `conversation_id: String` | `()` |
| 本地LLM (Pro) | `get_kv_cache_status` | — | `KvCacheStatus` |
| 本地LLM (Pro) | `set_kv_cache_enabled` | `enabled: bool` | `()` |
| 可观测性 | `get_log_level` | — | `String` (debug/info/warn/error) |
| 可观测性 | `set_log_level` | `level: String` | `()` |
| 可观测性 | `export_logs` | `tailLines: Option<usize>` | `String` |
| 可观测性 | `export_diagnostics` | — | `String` (JSON) |
| 数据目录 | `open_data_dir` | — | `String` (路径) |
| 缓存 | `get_cache_stats` | — | `CacheStats` |
| 缓存 | `clear_cache` | — | `()` |
| 缓存 | `set_cache_settings` | `settings: CacheSettingsPayload` | `()` |
| 缓存 | `get_cache_settings` | — | `CacheSettingsPayload` |
| 压缩 | `set_compression_ratio` | `ratio: f32` | `()` |
| 压缩 | `get_compression_ratio` | — | `f32` |
| 检索增强 | `rebuild_bm25_index` | — | `()` |
| 检索增强 | `rebuild_proposition_index` | — | `()` |
| 检索增强 | `build_summary_tree` | `doc_id: String` | `()` |
| 检索增强 | `set_embedder_model` | `model: String` | `()` |
| 检索记忆 | `set_retrieval_memory_enabled` | `enabled: bool` | `()` |
| 检索记忆 | `get_retrieval_memory_stats` | — | `Vec<MemoryRecord>` |
| 检索记忆 | `reset_retrieval_memory` | — | `()` |
| 检索记忆 | `record_retrieval_feedback` | `signal: FeedbackSignal` | `()` |
| 图检索 | `set_graph_retriever_enabled` | `enabled: bool` | `()` |
| 性能优化 | `set_progressive_injection` | `enabled: bool` | `()` |
| 性能优化 | `set_speculative_enabled` | `enabled: bool` | `()` |
| 性能优化 | `set_quality_gate_enabled` | `enabled: bool` | `()` |
| 图谱可视化 | `get_graph_data` | `limit?: number` | `Vec<GraphTriple>` |
| 图谱可视化 | `get_entity_relations` | `entityText: string` | `Vec<EntityRelation>` |
| 图谱可视化 | `get_graph_stats` | — | `GraphStats` |
| 图谱可视化增强 | `get_entity_types` | `entities: string[]` | `HashMap<String, String>` |
| 记忆系统 | `set_memory_enabled` | `enabled: bool` | `()` |
| 记忆系统 | `get_memories` | `tier?: string` | `Vec<MemoryEntry>` |
| 记忆系统 | `pin_memory` | `conversation_id: string, content: string` | `MemoryEntry` |
| 记忆系统 | `promote_memory` | `memory_id: string` | `()` |
| 记忆系统 | `delete_memory` | `memory_id: string` | `()` |
| 记忆系统 | `clear_memories` | `tier?: string` | `number` |
| 对话模板 | `save_prompt_template` | `template: PromptTemplate` | `String` (template ID) |
| 对话模板 | `list_prompt_templates` | — | `Vec<PromptTemplate>` |
| 对话模板 | `delete_prompt_template` | `templateId: String` | `()` |
| Wiki链接 | `get_forward_links` | `docId: String` | `Vec<WikiLink>` |
| Wiki链接 | `get_backlinks` | `docName: String` | `Vec<WikiLink>` |
| Wiki链接 | `rebuild_wiki_links` | — | `usize` (链接总数) |
| 对话分支树 | `get_conversation_tree` | `conversationId: String` | `ConversationTree` |
| 对话分支树 | `branch_from_message` | `conversationId: String, messageId: String, newContent: String` | `BranchResult { new_version, turn_group }` |
| 对话搜索 | `search_conversations` | `query: String, limit?: usize` | `Vec<MessageSearchResult>` |
| 对话分支 | `set_turn_active_version` | `conversation_id: String, turn_group: String, active_version: i32` | `()` |
| 对话分支 | `get_turn_active_versions` | `conversation_id: String` | `Vec<TurnActiveVersion>` |
| Contextual Retrieval | `set_contextual_retrieval` | `enabled: bool` | `()` |
| Contextual Retrieval | `rebuild_contextual_embeddings` | — | `()` |
| 文档标签 | `add_document_tag` | `doc_id: String, tag: String` | `()` |
| 文档标签 | `remove_document_tag` | `doc_id: String, tag: String` | `()` |
| 文档标签 | `list_all_tags` | — | `Vec<String>` |
| 文档标签 | `filter_documents_by_tag` | `tag: String` | `Vec<Document>` |
| 文档摘要 | `get_document_summary` | `docId: String` | `String \| null` |
| 文档摘要 | `regenerate_summary` | `docId: String` | `String` |
| 主题 | `get_theme` | — | `String` |
| 主题 | `set_theme` | `theme: String` | `()` |
| 嵌入器 | `check_embedder_status` | — | `bool` |
| 下载管理 | `pause_download` | `filename: String` | `()` |
| 下载管理 | `abort_download` | `filename: String` | `()` |
| 下载管理 | `get_download_status` | `filename: String` | `DownloadStatus` |
| 下载管理 | `list_pending_downloads` | — | `Vec<DownloadEntry>` |
| 下载管理 | `cleanup_partial_downloads` | — | `u64` |
| 下载管理 | `scan_download_recovery` | — | `Vec<DownloadEntry>` |
| 安全态势 | `set_security_posture` | `posture: String` | `()` |
| 安全态势 | `get_security_posture` | — | `String` |
| Shadow 筛查 | `get_security_screen_stats` | — | `ShadowScreenStats` |
| Shadow 筛查 | `reset_security_screen_stats` | — | `()` |
| 记忆整合 | `trigger_memory_consolidation` | — | `JSON` |
| 记忆整合 | `get_scratch_logs` | `limit?: usize` | `Vec<ScratchLogEntry>` |
| Burst Buffer | `push_burst_turn` | `conversation_id: String, user_msg: String, assistant_msg: String` | `()` |
| Burst Buffer | `flush_memory_burst_buffer` | — | `JSON` |
| Burst Buffer | `get_burst_buffer_status` | — | `JSON` |
| 语音转写 | `transcribe_audio` | `audio_path: String` | `String` |
| 语音转写 | `get_stt_config` | — | `JSON` |
| 语音转写 | `set_stt_config` | `config: JSON` | `()` |
| 网页搜索 | `set_web_search_enabled` | `enabled: bool` | `()` |
| 网页搜索 | `web_search` | `query: String` | `Vec<SearchResult>` |
| AutoDream | `trigger_dream` | — | `()` |
| AutoDream | `get_dream_suggestions` | — | `Vec<DreamSuggestion>` |
| AutoDream | `abort_dream` | — | `()` |
| 图谱分析 | `get_shortest_path` | `from: String, to: String` | `GraphPath` |
| 图谱分析 | `get_communities` | — | `GraphCommunity` |
| 图谱分析 | `get_graph_layout` | `mode?: String` | `Vec<String>` |
| 图谱导出 | `export_graph` | `format: String` | `String` |
| 工作流 | `save_workflow_template` | `name: String, description: String, nodes_json: String, edges_json: String` | `String` |
| 工作流 | `run_workflow` | `workflow_id: String, input: String` | `WorkflowResult` |
| 工作流 | `list_workflows` | — | `Vec<Workflow>` |
| 工作流 | `delete_workflow` | `workflow_id: String` | `()` |
| 代码符号 | `search_symbols` | `query: String, limit?: usize` | `Vec<CodeSymbol>` |
| 代码符号 | `get_symbols_for_chunk` | `chunk_id: String` | `Vec<CodeSymbol>` |
| 代码符号 (Pro) | `rebuild_symbol_index` | — | `number` |
| 代码执行 (Pro) | `execute_code_snippet` | `code: String, language: String, stdin?: String` | `ExecutionResult` |
| Prompt 接纳 | `admit_input` | `conversation_id: String, content: String, delivery: String` | `String` |
| Prompt 接纳 | `promote_input` | `id: String` | `()` |
| Prompt 接纳 | `get_pending_inputs` | `conversation_id: String` | `Vec<PendingInput>` |
| 会话待办 | `add_session_todo` | `conversation_id: String, text: String` | `String` |
| 会话待办 | `update_todo_status` | `id: String, done: bool` | `()` |
| 会话待办 | `get_session_todos` | `conversation_id: String` | `Vec<SessionTodo>` |
| 会话待办 | `delete_session_todo` | `id: String` | `()` |
| 会话待办 | `delete_session_todos` | `conversation_id: String` | `()` |
| 预算追踪 | `get_budget_stats` | — | `BudgetStats` |
| 预算追踪 | `set_budget_limit` | `limit: u64` | `()` |
| Token 预算 | `get_token_budget_config` | — | `TokenBudgetConfig` |
| Token 预算 | `set_token_budget_config` | `config: TokenBudgetConfig` | `()` |
| Trace 系统 | `get_recent_traces` | `limit?: usize` | `Vec<TraceEntry>` |
| Trace 系统 | `get_trace_detail` | `trace_id: String` | `TraceDetail` |
| Trace 系统 | `clear_traces` | — | `()` |
| Trace 系统 | `get_trace_count` | — | `u64` |
| 用量追踪 | `get_conversation_cost` | `conversation_id: String` | `ConversationCost` |
| 用量追踪 | `set_token_budget` | `budget: u64` | `()` |

---

## 2. 命令详情

### 2.1 import_files

```typescript
invoke('import_files', { paths: string[] }): Promise<string[]>
```

- **行为**：逐个导入文件，执行解析→去重→分块→嵌入→存储链路
- **支持格式**：`.md` / `.txt` / `.pdf` / `.rs` / `.ts` / `.tsx` / `.py` / `.go` / `.docx` / `.html` / `.htm` / `.pptx` / `.epub`（REQ-ING-002/015/016/017/018）
- **错误**：格式非法 / 配额触顶 / 未授权 PDF / 路径遍历 → `Err(可读消息)`
- **事件**：导入过程中发射 `doc-status-changed` 事件
- **配额**：免费版上限 50 文件、禁用 PDF（`is_pro` 判定）
- **领域分类**：嵌入完成后异步执行领域画像分类（REQ-VEC-013），不阻塞导入完成

### 2.2 chat

```typescript
invoke('chat', {
  query: string,
  history: ChatMessage[],
  conversation_id: string
}): Promise<void>
```

- **事件序列**：`chat_phase?` → `chat_context_truncated?` → `agent_step*`（Agentic 模式） → `chat_sources?` → `chat_token*` → `chat_done`
- **失败**：先推 `chat_error` 再返回 `Err`
- **中断**：`abort_chat` 触发 `CancellationToken`，已生成内容保留并落库
- **空库拦截**：`count_documents == 0` 时直接返回错误
- **上下文截断**：历史消息超过 `rag.context_token_limit`（默认 4096）时截断，发射 `chat_context_truncated` 事件
- **Agentic 模式**：`rag.agent_enabled = true` 时走 `AgentEngine.run()` ReAct 循环，发射 `agent_step` 事件展示思考过程

### 2.3 abort_chat

```typescript
invoke('abort_chat', { conversation_id: string }): Promise<void>
```

- **行为**：触发指定会话的中断令牌，流循环立即 break

### 2.4 create_conversation

```typescript
invoke('create_conversation', { workspace_id: string }): Promise<string>
```

- **返回**：新会话的 UUID v4
- **占位标题**：「新会话」

### 2.5 get_conversations

```typescript
invoke('get_conversations', { workspace_id: string }): Promise<Conversation[]>
```

- **排序**：按创建时间倒序

### 2.6 get_conversations_paginated

```typescript
invoke('get_conversations_paginated', {
  workspace_id: string,
  limit?: number,   // 默认 50
  offset?: number   // 默认 0
}): Promise<PaginatedResult<Conversation>>
```

- **返回**：`{ items: Conversation[], total: number }`
- **排序**：按创建时间倒序
- **关联需求**：REQ-NAV-007 会话列表分页与时间分组

### 2.7 get_messages

```typescript
invoke('get_messages', { conversation_id: string }): Promise<ChatMessage[]>
```

- **排序**：按写入顺序正序

### 2.8 get_messages_paginated

```typescript
invoke('get_messages_paginated', {
  conversation_id: string,
  limit?: number,   // 默认 30
  offset?: number   // 默认 0
}): Promise<PaginatedResult<ChatMessage>>
```

- **返回**：`{ items: ChatMessage[], total: number }`
- **排序**：从最新消息向前偏移（`ORDER BY rowid DESC LIMIT OFFSET` + 反转）
- **关联需求**：REQ-RAG-019 消息列表懒加载与分页

### 2.9 test_llm_connection

```typescript
invoke('test_llm_connection', {
  api_key: string,
  base_url: string,
  model: string
}): Promise<string>
```

- **行为**：发送极简请求验证 API Key 与 URL 有效性

### 2.10 update_llm_config

```typescript
invoke('update_llm_config', { config: LlmConfig }): Promise<void>
```

- **行为**：写入 settings 表并刷新运行态（无需重启）

### 2.11 get_settings

```typescript
invoke('get_settings'): Promise<SettingsPayload>
```

- **安全**：API Key 仅脱敏返回（`****` + 末四位）

### 2.12 set_vlm_enabled

```typescript
invoke('set_vlm_enabled', { enabled: boolean }): Promise<void>
```

- **行为**：写入 settings 表 `mm.vlm_enabled`，`vision_provider()` 下次调用时即时生效（无需重启）
- **隐私**：前端开启时须弹出隐私确认弹窗（REQ-MM-003 隐私边界）

### 2.13 set_hybrid_search

```typescript
invoke('set_hybrid_search', { enabled: boolean }): Promise<void>
```

- **行为**：写入 settings 表 `rag.hybrid_search`，下次 `chat` 命令调用时即时生效（无需重启）
- **默认值**：`true`（混合检索默认启用）
- **关闭效果**：退化为纯向量检索（等价于 `VectorRetriever`），跳过关键词检索与 RRF 融合
- **关联需求**：REQ-RAG-010 混合检索（向量 + 关键词 RRF 融合）

### 2.14 set_hyde_enabled

```typescript
invoke('set_hyde_enabled', { enabled: boolean }): Promise<void>
```

- **行为**：写入 settings 表 `rag.hyde_enabled`，下次 `chat` 命令调用时即时生效（无需重启）
- **默认值**：`false`（HyDE 默认关闭，用户主动开启）
- **启用效果**：检索管线在向量检索前插入 HyDE 查询改写阶段——使用用户 LLM 配置生成假设性答案文档，用该文档的嵌入替代原始查询嵌入进行向量检索。关键词检索仍使用原始查询。
- **降级策略**：LLM 调用失败（网络错误、超时 15s、空响应）时优雅降级为原始查询，不中断检索管线
- **无额外模型下载**：复用用户已配置的 LLM 端点（BYOK），与 chat 命令使用相同的 `api_key` / `base_url` / `model`
- **关联需求**：REQ-RAG-021 HyDE 查询改写

### 2.15 set_rerank_enabled

```typescript
invoke('set_rerank_enabled', { enabled: boolean }): Promise<void>
```

- **行为**：写入 settings 表 `rag.rerank_enabled`，下次 `chat` 命令调用时即时生效
- **默认值**：`false`（重排序默认关闭，用户主动开启）
- **启用效果**：RRF 融合后、Chunk Expansion 前插入 Cross-Encoder 精排阶段，对 top-N（N = top_k × 5）候选使用 bge-reranker-base 模型逐对 query-document 打分
- **首次启用**：下载 bge-reranker-base ONNX 模型（~280MB），后续从本地缓存加载
- **关联需求**：REQ-RAG-020 Cross-Encoder 重排序

### 2.16 set_agent_enabled

```typescript
invoke('set_agent_enabled', { enabled: boolean }): Promise<void>
```

- **行为**：写入 settings 表 `rag.agent_enabled`，下次 `chat` 命令调用时即时生效
- **默认值**：`false`（Agentic 模式默认关闭）
- **启用效果**：`chat_inner` 检测到 `agent_enabled = true` 时走 `AgentEngine.run()` ReAct 循环（Thought → Action → Observation），最大 5 次迭代
- **事件**：中间推理步骤通过 `agent_step` 事件推送前端
- **关联需求**：REQ-RAG-022 Agentic RAG 多步推理

### 2.16b set_coordinator_mode

```typescript
invoke('set_coordinator_mode', { enabled: boolean }): Promise<void>
```

- **行为**：写入 settings 表 `rag.coordinator_enabled`，下次 `chat` 命令调用时即时生效
- **默认值**：`false`（协调模式默认关闭）
- **启用效果**：`chat_inner` 检测到 `coordinator_enabled = true` 时走 `CoordinatorEngine.run()` 四阶段流水线（Research → Synthesis → Implementation → Verification）
- **事件**：中间阶段进度通过 `coordinator_phase` 事件推送前端（phase: researching/synthesizing/generating, message, sub_query_count）
- **关联需求**：REQ-RAG-025 多代理协调模式

### 2.16c set_sub_agent_enabled

```typescript
invoke('set_sub_agent_enabled', { enabled: boolean }): Promise<void>
```

- **行为**：写入 settings 表 `rag.sub_agent_enabled`，下次 `chat` 命令调用时即时生效
- **默认值**：`false`（子代理舰队模式默认关闭）
- **启用效果**：`chat_inner` 检测到 `sub_agent_enabled = true` 时，CoordinatorEngine Research 阶段每个子查询由独立 SubAgent 处理，通过 mailbox 消息传递协调。**仅在 `rag.coordinator_enabled` 为 true 时生效**。
- **关联需求**：REQ-RAG-025 扩展（子代理舰队模式）

### 2.17 set_embedding_model

```typescript
invoke('set_embedding_model', { model: string }): Promise<void>
```

- **行为**：将嵌入模型标识持久化到 settings 表 `vec.embedding_model`，并销毁当前 embedder 实例（下次调用 `embedder()` 时用新模型重新初始化）
- **支持的模型标识**：`all-MiniLM-L6-v2`（384d EN）、`bge-small-zh-v1.5`（512d ZH）、`e5-small-v2`（384d Multi）
- **注意**：切换模型后已索引文档的向量维度可能与新模型不匹配，建议用户重建索引
- **错误**：不支持的模型标识 → `Err("不支持的嵌入模型: {model}")`
- **关联需求**：REQ-VEC-012 嵌入模型运行时切换

### 2.18 set_context_token_limit

```typescript
invoke('set_context_token_limit', { limit: number }): Promise<void>
```

- **行为**：写入 settings 表 `rag.context_token_limit`，控制对话历史 token 截断阈值
- **范围**：2048-32768（默认 4096）
- **效果**：多轮对话历史 token 数超过阈值时，采用「保留首轮 + 最近 N 轮」策略截断，发射 `chat_context_truncated` 事件通知前端
- **错误**：超出范围 → `Err("上下文 token 限制范围为 2048-32768")`

### 2.19 get_file_sizes

```typescript
invoke('get_file_sizes', { paths: string[] }): Promise<[string, number][]>
```

- **行为**：批量获取文件大小（字节），用于导入前大小检查
- **关联需求**：REQ-ING-013 文件大小限制与警告

### 2.20 get_file_size_limits

```typescript
invoke('get_file_size_limits'): Promise<[number, number]>
```

- **返回**：`(警告阈值, 硬上限)`，默认 `(100MB, 500MB)`
- **关联需求**：REQ-ING-013 文件大小限制与警告

### 2.21 reclassify_document

```typescript
invoke('reclassify_document', { doc_id: string }): Promise<string>
```

- **行为**：重新对指定文档执行领域画像分类，返回分类结果（领域标识）
- **关联需求**：REQ-VEC-013 领域画像自动分类

### 2.22 get_locale / set_locale

```typescript
invoke('get_locale'): Promise<string>
invoke('set_locale', { locale: string }): Promise<void>
```

- **行为**：读写 settings 表 `ui.locale`，持久化语言偏好
- **支持值**：`zh-CN`（默认）、`en`
- **关联需求**：REQ-I18N-001 界面文案外部化与语言切换

### 2.23 get_sidebar_collapsed / set_sidebar_collapsed

```typescript
invoke('get_sidebar_collapsed'): Promise<boolean>
invoke('set_sidebar_collapsed', { collapsed: boolean }): Promise<void>
```

- **行为**：读写 settings 表 `ui.sidebar_collapsed`，持久化侧栏折叠状态
- **默认值**：`false`（展开）
- **关联需求**：REQ-NAV-001 侧栏折叠与展开（AC-4 状态持久化）

### 2.24 activate_pro

```typescript
invoke('activate_pro', { license_key: string }): Promise<boolean>
```

- **行为**：验证 License 签名，有效则持久化并刷新运行态

### 2.24 deactivate_pro

```typescript
invoke('deactivate_pro'): Promise<void>
```

- **行为**：将 settings 表中 `license.is_pro` 置为 `"false"`，运行态 `is_pro` 置为 `false`
- **关联需求**：REQ-LIC-004 License 停用与状态展示

### 2.25 get_pro_status

```typescript
invoke('get_pro_status'): Promise<boolean>
```

### 2.26 audit_document

```typescript
invoke('audit_document', {
  doc_id: string,
  doc_name: string
}): Promise<void>
```

- **行为**：全量扫描文档 → 声明提取 → 矛盾检测 → 报告生成（REQ-AUDIT-001~005）
- **Pro 限制**：免费版调用返回 `Err("文档审计是 Pro 版功能")`
- **事件序列**：`audit_phase(extracting)` → `audit_phase(comparing)` → `audit_phase(reporting)` → `chat_token*`（Markdown 报告）→ `chat_done`
- **取消**：`abort_audit` 触发 `Arc<AtomicBool>`，审计循环 break，输出部分报告 + `chat_error("⏹ 审计已中断")` + `chat_done`
- **空文档**：`list_chunks` 返回空时推送提示文案 + `chat_done`
- **报告格式**：Markdown（摘要段 + 矛盾清单 + 免责声明），复用 `chat_token` 渲染链路

### 2.27 abort_audit

```typescript
invoke('abort_audit', { doc_id: string }): Promise<void>
```

- **行为**：触发指定文档的审计取消标志（`Arc<AtomicBool>`），审计循环检测后 break
- **与 `abort_chat` 的区别**：`abort_chat` 使用 `CancellationToken`（tokio_util），`abort_audit` 使用 `Arc<AtomicBool>`（零依赖 core 层）

### 2.28 export_conversation_markdown

```typescript
invoke('export_conversation_markdown', {
  conversation_id: string
}): Promise<[string, string]>
```

- **返回**：`(Markdown 内容, 默认文件名)`
- **导出内容**：会话标题、创建时间、全部消息（user/assistant 按序）、引用来源列表
- **关联需求**：REQ-EXP-001 对话导出为 Markdown

### 2.29 save_text_file

```typescript
invoke('save_text_file', {
  path: string,
  content: string
}): Promise<void>
```

- **行为**：将文本内容写入指定路径（用于导出功能）
- **错误**：路径不可写 → `Err("文件保存失败: ...")`

### 2.30 add_watched_folder

```typescript
invoke('add_watched_folder', { path: string }): Promise<SyncResult>
```

- **行为**：验证路径 → 首次全量同步 → 启动文件监听器 → 持久化到 settings 表（REQ-SYNC-001~003）
- **SyncResult**：`{ added: number, updated: number, deleted: number, skipped: number, errors: string[] }`
- **同步进度**：通过 `sync_progress` 事件推送前端
- **关联需求**：REQ-SYNC-001 / REQ-SYNC-002 / REQ-SYNC-003

### 2.31 remove_watched_folder

```typescript
invoke('remove_watched_folder', { path: string }): Promise<void>
```

- **行为**：停止文件监听器并从 settings 表移除（REQ-SYNC-001 AC-4）
- **注意**：仅停止监听，不删除已导入的文档
- **关联需求**：REQ-SYNC-001

### 2.32 get_watched_folders

```typescript
invoke('get_watched_folders'): Promise<WatchedFolderInfo[]>
```

- **行为**：返回监听文件夹列表（REQ-SYNC-001 AC-3）
- **WatchedFolderInfo**：`{ path: string, name: string, sync_status: string, last_synced_at: number | null }`
- **sync_status**：`idle`（监听中）/ `stopped`（已停止）
- **关联需求**：REQ-SYNC-001

### 2.33 get_security_status

```typescript
invoke('get_security_status'): Promise<SecurityStatus>
```

- **返回**：
  ```typescript
  {
    state: string,        // "unencrypted" | "encrypted_unlocked" | "locked"
    color: string,        // "#ef4444" | "#22c55e" | "#f59e0b"
    is_locked: boolean,
    remaining_attempts: number,    // 暴力破解防护剩余尝试次数
    remaining_lock_seconds: number  // 频率限制剩余锁定秒数
  }
  ```
- **关联需求**：REQ-SEC-013 / REQ-SEC-016 / REQ-SEC-017

### 2.34 set_auto_lock_config

```typescript
invoke('set_auto_lock_config', {
  enabled: boolean,
  timeout_secs: number,
  lock_on_sleep: boolean
}): Promise<void>
```

- **行为**：配置自动锁定参数，持久化到 settings 表
- **默认值**：`enabled=true, timeout_secs=300, lock_on_sleep=true`
- **关联需求**：REQ-SEC-016 自动锁定

### 2.35 lock_app

```typescript
invoke('lock_app'): Promise<void>
```

- **行为**：手动锁定应用，状态 `EncryptedUnlocked → Locked`
- **关联需求**：REQ-SEC-016

### 2.36 unlock_app

```typescript
invoke('unlock_app'): Promise<void>
```

- **行为**：解锁应用（需已通过 `encrypt_database` 设置密码）
- **暴力破解防护**：连续 5 次错误后指数退避锁定
- **紧急销毁**：若输入紧急销毁密码，触发数据销毁而非解锁
- **关联需求**：REQ-SEC-013 / REQ-SEC-017 / REQ-SEC-019

### 2.37 record_activity

```typescript
invoke('record_activity'): Promise<void>
```

- **行为**：记录用户活动，重置自动锁屏空闲计时器
- **关联需求**：REQ-SEC-016

### 2.38 detect_pii

```typescript
invoke('detect_pii', { text: string }): Promise<{
  detections: Array<{ type: string, value: string, start: number, end: number }>,
  redacted: string
}>
```

- **行为**：检测文本中的 PII（8 种类型：email/phone/idcard/bankcard/ip/ssn/passport/intl_phone）并返回脱敏结果
- **关联需求**：REQ-SEC-014 PII 检测与脱敏

### 2.39 set_panic_wipe_password

```typescript
invoke('set_panic_wipe_password', { password: string }): Promise<void>
```

- **行为**：设置紧急销毁密码，使用该密码解锁时销毁全部本地数据
- **关联需求**：REQ-SEC-019 紧急销毁

### 2.40 clear_panic_wipe_password

```typescript
invoke('clear_panic_wipe_password'): Promise<void>
```

- **行为**：清除紧急销毁密码
- **关联需求**：REQ-SEC-019

### 2.41 is_panic_wipe_enabled

```typescript
invoke('is_panic_wipe_enabled'): Promise<boolean>
```

- **行为**：查询是否已设置紧急销毁密码
- **关联需求**：REQ-SEC-019

### 2.42 set_clipboard_config

```typescript
invoke('set_clipboard_config', {
  enabled: boolean,
  clear_after_secs: number
}): Promise<void>
```

- **行为**：配置剪贴板自动清除（默认 30 秒后清除）
- **关联需求**：REQ-SEC-018 剪贴板自动清除

### 2.43 get_audit_logs

```typescript
invoke('get_audit_logs', { limit?: number }): Promise<AuditEntry[]>
```

- **行为**：查询审计日志（哈希链），默认返回最近 100 条
- **AuditEntry**：`{ action, target, timestamp, prev_hash, entry_hash }`
- **关联需求**：REQ-SEC-015 审计日志哈希链

### 2.44 clear_audit_logs

```typescript
invoke('clear_audit_logs'): Promise<void>
```

- **行为**：清空全部审计日志
- **关联需求**：REQ-SEC-015

### 2.45 check_password_strength

```typescript
invoke('check_password_strength', { password: string }): Promise<{
  level: string,         // "weak" | "fair" | "good" | "strong"
  percentage: number,
  suggestions: string[]
}>
```

- **行为**：评估密码强度并给出改进建议
- **关联需求**：REQ-SEC-020 密码强度评估

### 本地 LLM 模型管理（REQ-LLM-003/004）

```typescript
// 列出已下载的本地 GGUF 模型
invoke('list_local_models'): Promise<ModelInfo[]>

// 获取推荐模型列表
invoke('get_recommended_models'): Promise<RecommendedModel[]>

// 下载模型文件（后台执行，通过 model_download_progress 事件推送进度）
invoke('download_model', { url: string, filename: string }): Promise<void>

// 删除本地模型文件
invoke('delete_model', { filename: string }): Promise<void>

// 切换推理模式（remote / local）
invoke('set_llm_mode', { mode: string }): Promise<void>

// 获取当前推理模式
invoke('get_llm_mode'): Promise<string>

// 设置当前选中的本地模型文件名
invoke('set_local_model', { filename: string }): Promise<void>

// 获取当前推理设备类型（cpu/metal/cuda，Phase 2）
invoke('get_local_llm_device_kind'): Promise<string>

// 配置 PagedAttention KV cache 管理（Phase 2）
invoke('set_paged_attn', { enabled: boolean, block_size: number, gpu_memory_ctx: number }): Promise<void>

// 设置采样参数（Phase 2）
invoke('set_sampling_params', { params: LlmSamplingParams }): Promise<void>

// 设置推理内核模式（Phase 3）
invoke('set_kernel_mode', { mode: string }): Promise<void>

// 获取当前推理内核模式（Phase 3）
invoke('get_kernel_mode'): Promise<string>
```

- **关联需求**：REQ-LLM-003 本地 LLM 推理引擎、REQ-LLM-004 本地模型管理、REQ-LLM-005 GPU 加速推理、REQ-LLM-006 采样参数配置

#### Phase 2 新增命令详情

**`get_local_llm_device_kind`**：返回当前推理设备类型字符串（`"cpu"` / `"metal"` / `"cuda"` / `"unknown"`）。引擎创建时通过 `best_device()` 探测，编译时由 metal/cuda feature flags 决定。前端用于显示当前推理设备。

**`set_paged_attn`**：配置 PagedAttention 高效 KV cache 管理。写入 settings 表 `llm.paged_attn` / `llm.block_size` / `llm.gpu_memory_ctx` 三个键。参数校验：block_size 仅允许 8/16/32，gpu_memory_ctx 必须 >0。配置变更后销毁已有引擎实例（下次 chat 重新加载时应用新配置）。PagedAttention 仅在 GPU 模式下生效，CPU 模式忽略此配置。

**`set_sampling_params`**：设置本地推理采样参数（temperature/top_p/top_k/max_tokens/frequency_penalty/presence_penalty）。JSON 序列化后持久化到 settings 表 `llm.sampling` 键。如果引擎已加载，同时即时更新运行时采样参数（无需重新加载模型）。参数范围校验：temperature 0.0~2.0、top_p 0.0~1.0、top_k 1~100、max_tokens >0、frequency_penalty -2.0~2.0、presence_penalty -2.0~2.0。

**`set_kernel_mode`**（Phase 3）：设置推理内核模式，支持 `"mistral"`（使用 mistral.rs 内核）和 `"custom"`（使用自研 GEMV 内核）。持久化到 settings 表 `llm.kernel_mode` 键。切换到 `custom` 模式后，下次 `chat_stream` 调用触发 `load_custom_weights()` 加载 GGUF 文件并重排权重。Pro 门控功能。

**`get_kernel_mode`**（Phase 3）：返回当前推理内核模式字符串（`"mistral"` / `"custom"`）。默认返回 `"mistral"`。

#### KV Cache 跨会话复用（Phase 4，Pro 门控，REQ-LLM-009）

```typescript
// 保存当前会话的 KV cache 元数据快照
invoke('save_kv_cache', { conversation_id: string }): Promise<void>

// 恢复指定会话的 KV cache（返回是否成功恢复）
invoke('load_kv_cache', { conversation_id: string }): Promise<boolean>

// 清除指定会话的 KV cache 文件（幂等，不检查启用状态）
invoke('clear_kv_cache', { conversation_id: string }): Promise<void>

// 获取 KV cache 状态信息
invoke('get_kv_cache_status'): Promise<KvCacheStatus>

// 启用/禁用 KV cache 跨会话复用
invoke('set_kv_cache_enabled', { enabled: boolean }): Promise<void>
```

- **关联需求**：REQ-LLM-009 KV Cache 跨会话复用
- **存储路径**：`{data_dir}/kv_cache/{conversation_id}.emkv`
- **设置键**：`llm.kv_cache_enabled`（bool，默认 `false`）
- **注意**：当前 mistral.rs v0.9.0 API 不暴露 KV cache 张量提取/注入，保存的是元数据快照（模型名 + 上下文长度）。禁用时 save 返回 `Ok(())`（noop），load 返回 `Ok(false)`。`clear_kv_cache` 不检查启用状态（允许清理旧数据），幂等。

---

## 3. Tauri Events

| 事件 | 负载 | 触发时机 |
|---|---|---|
| `doc-status-changed` | `DocStatusPayload { status, message, sub_phase? }` | 文档导入/索引状态更新 |
| `chat_phase` | `ChatPhasePayload { phase, message }` | 对话进度阶段（首个 token 前） |
| `chat_context_truncated` | `{ original_count, truncated_count, token_limit }` | 对话历史超过 token 限制被截断时 |
| `chat_token` | `String` | 流式 token 推送 |
| `chat_sources` | `Vec<RetrievalResult>` | 检索引用来源 |
| `chat_error` | `String` | 错误消息 |
| `chat_done` | `()` | 对话/审计结束 |
| `agent_step` | `AgentStepPayload { step_type, content, tool?, input? }` | Agentic RAG 推理步骤（REQ-RAG-022） |
| `audit_phase` | `ChatPhasePayload { phase, message }` | 审计进度阶段（extracting / comparing / reporting） |
| `import-progress` | `ImportProgressPayload { completed, total, current_file, cancelled }` | 批量导入逐文件进度（REQ-ING-006） |
| `embedding_progress` | `EmbeddingProgressPayload { doc_id, doc_name, embedded, total }` | 向量化微批次进度（GB 级文档加速） |
| `model_download_progress` | `DownloadEvent` | 模型下载/加载进度（REQ-VEC-008） |
| `model_load_progress` | `ModelLoadStatusPayload { model, status, error? }` | 本地 LLM 模型加载状态（REQ-LLM-003 AC-3/AC-4；status: loading/ready/error） |
| `sync_progress` | `SyncProgressPayload { phase, folder_path, added, updated, deleted, skipped, message }` | 文件监听同步进度（REQ-SYNC-002 AC-7） |

### 3.1 chat_phase 阶段

| phase | message | 含义 |
|---|---|---|
| `preparing` | "初始化向量化引擎…" | 首次使用需下载模型 |
| `retrieving` | "检索知识库…" | 向量检索 top-k |
| `generating` | "正在生成回答…" | LLM 流式生成 |

### 3.2 agent_step 步骤类型

| step_type | 含义 | 内容 |
|---|---|---|
| `thought` | 推理轨迹 | LLM 生成的思考过程文本 |
| `action` | 工具调用 | `tool`（工具名）+ `input`（工具输入） |
| `observation` | 观察结果 | 工具执行返回的摘要 |
| `answer` | 最终答案开始 | 标记后续 `chat_token` 为最终答案 |

### 3.3 audit_phase 阶段

| phase | message | 含义 |
|---|---|---|
| `extracting` | "初始化向量化引擎…" | 审计启动，初始化 embedder |
| `comparing` | "正在比对声明…" | Decompose → Verify 执行中 |
| `reporting` | "正在生成报告…" | 聚合报告并流式推送 |

### 3.4 sync_progress 阶段

| phase | 含义 |
|---|---|
| `scanning` | 扫描文件夹中 |
| `importing` | 导入文件中 |
| `deleting` | 清理已删除文件对应的文档 |
| `complete` | 同步完成 |
| `error` | 同步出错 |

### 3.5 doc-status-changed 状态

| status | 含义 |
|---|---|
| `indexing` | 正在解析/分块/嵌入 |
| `done` | 索引完成 |
| `error` | 索引失败 |

### 3.6 db-integrity-error 数据库完整性错误

| payload | 含义 |
|---|---|
| `String` | `PRAGMA integrity_check` 返回的错误消息 |

启动时数据库完整性检查失败时触发，前端显示 `#dbError` Modal。

---

## 4. `*_inner` 测试复用

每个命令有对应的 `*_inner` 函数，接收 `&AppState` 而非 `State<'_, AppState>`：

| 命令 | inner 函数 |
|---|---|
| `import_files` | `import_files_inner` |
| `chat` | `chat_inner` |
| `create_conversation` | `create_conversation_inner` |
| `get_conversations` | `get_conversations_inner` |
| `get_conversations_paginated` | `get_conversations_paginated_inner` |
| `get_messages` | `get_messages_inner` |
| `get_messages_paginated` | `get_messages_paginated_inner` |
| `delete_conversation` | `delete_conversation_inner` |
| `export_conversation_markdown` | `export_conversation_markdown_inner` |
| `save_text_file` | `save_text_file_inner` |
| `activate_pro` | `activate_pro_inner` |
| `deactivate_pro` | `deactivate_pro_inner` |
| `get_pro_status` | `get_pro_status_inner` |
| `update_llm_config` | `update_llm_config_inner` |
| `get_settings` | `get_settings_inner` |
| `set_vlm_enabled` | `set_vlm_enabled_inner` |
| `set_hybrid_search` | `set_hybrid_search_inner` |
| `set_hyde_enabled` | `set_hyde_enabled_inner` |
| `set_rerank_enabled` | `set_rerank_enabled_inner` |
| `set_agent_enabled` | `set_agent_enabled_inner` |
| `set_coordinator_mode` | `set_coordinator_mode_inner` |
| `set_sub_agent_enabled` | `set_sub_agent_enabled_inner` |
| `set_embedding_model` | `set_embedding_model_inner` |
| `set_context_token_limit` | `set_context_token_limit_inner` |
| `get_locale` | `get_locale_inner` |
| `set_locale` | `set_locale_inner` |
| `get_sidebar_collapsed` | `get_sidebar_collapsed_inner` |
| `set_sidebar_collapsed` | `set_sidebar_collapsed_inner` |
| `reclassify_document` | `reclassify_document_inner` |
| `audit_document` | `audit_document_inner` |
| `retry_index` | `retry_index_inner` |
| `add_watched_folder` | `add_watched_folder_inner` |
| `remove_watched_folder` | `remove_watched_folder_inner` |
| `get_watched_folders` | `get_watched_folders_inner` |
| `get_log_level` | `get_log_level_inner` |
| `set_log_level` | `set_log_level_inner` |
| `export_logs` | `export_logs_inner` |
| `export_diagnostics` | `export_diagnostics_inner` |
| `open_data_dir` | `open_data_dir_inner` |
| `save_kv_cache` | `save_kv_cache_inner` |
| `load_kv_cache` | `load_kv_cache_inner` |
| `clear_kv_cache` | `clear_kv_cache_inner` |
| `get_kv_cache_status` | `get_kv_cache_status_inner` |
| `set_kv_cache_enabled` | `set_kv_cache_enabled_inner` |

> **注**：`get_file_sizes`、`get_file_size_limits`、`check_password_strength`、`detect_pii` 为纯计算命令（不依赖 `AppState`），无 `*_inner` 函数。安全命令（`get_security_status`、`set_auto_lock_config`、`lock_app`、`unlock_app`、`record_activity`、`set_panic_wipe_password`、`clear_panic_wipe_password`、`is_panic_wipe_enabled`、`set_clipboard_config`、`get_audit_logs`、`clear_audit_logs`）直接操作 `state.security`，集成测试通过构造 `AppState` 直接调用。可观测性命令（`get_log_level`、`set_log_level`、`export_logs`、`export_diagnostics`）通过 `LocalLogger` 全局句柄 + `state.storage` settings 表实现。

集成测试通过构造 `AppState` 并调用 `*_inner` 函数复用命令逻辑。

---

## 2.47 极致性能优化命令（13 个）

### 2.47.1 get_cache_stats

```typescript
invoke('get_cache_stats'): Promise<CacheStats>
```

- **行为**：返回三级缓存（L0 精确 + L1 语义 + L3 检索结果）的运行时统计，包括各级别命中次数、未命中次数、总条目数。
- **关联**：REQ-PERF-001 ｜ Session S1

### 2.47.2 clear_cache

```typescript
invoke('clear_cache'): Promise<void>
```

- **行为**：清空所有三级缓存表（exact_cache / semantic_cache / retrieval_cache）。文档导入时自动触发。
- **关联**：REQ-PERF-001 ｜ Session S1

### 2.47.3 set_cache_settings

```typescript
invoke('set_cache_settings', { settings: CacheSettingsPayload }): Promise<void>
```

- **行为**：设置缓存参数（enabled / ttl_secs / semantic_threshold / privacy_mode），持久化到 settings 表。
- **关联**：REQ-PERF-001 ｜ Session S1

### 2.47.4 get_cache_settings

```typescript
invoke('get_cache_settings'): Promise<CacheSettingsPayload>
```

- **行为**：读取当前缓存设置。
- **关联**：REQ-PERF-001 ｜ Session S1

### 2.47.5 set_compression_ratio

```typescript
invoke('set_compression_ratio', { ratio: number }): Promise<void>
```

- **行为**：设置 Prompt 压缩比（1.0=禁用 / 2.0=保守 / 3.0=平衡 / 5.0=激进），持久化到 settings 表 `compression.ratio`。
- **关联**：REQ-PERF-002 ｜ Session S2

### 2.47.6 get_compression_ratio

```typescript
invoke('get_compression_ratio'): Promise<number>
```

- **行为**：读取当前压缩比。
- **关联**：REQ-PERF-002 ｜ Session S2

### 2.47.7 rebuild_bm25_index

```typescript
invoke('rebuild_bm25_index'): Promise<void>
```

- **行为**：重建 FTS5 BM25 索引，使用 `build_contextual_text()` 上下文前缀。供旧库升级或索引损坏时使用。
- **关联**：REQ-PERF-005 ｜ Session S4

### 2.47.8 rebuild_proposition_index

```typescript
invoke('rebuild_proposition_index'): Promise<void>
```

- **行为**：重建 Proposition 索引（对已有 chunks 执行 PropositionSplitter + 嵌入）。供旧库升级时使用。
- **关联**：REQ-PERF-007 ｜ Session S5

### 2.47.9 build_summary_tree

```typescript
invoke('build_summary_tree', { doc_id: string }): Promise<void>
```

- **行为**：为指定文档构建 RAPTOR 多级摘要树（Level 0 组摘要 → Level 1 主题摘要）。
- **关联**：REQ-PERF-009 ｜ Session S6

### 2.47.10 set_embedder_model

```typescript
invoke('set_embedder_model', { model: string }): Promise<void>
```

- **行为**：设置嵌入模型（如 "all-MiniLM-L6-v2" / "bge-small-zh-v1.5" / "e5-small-v2"），持久化到 settings 表。
- **关联**：REQ-PERF-009 ｜ Session S6

### 2.47.11 set_retrieval_memory_enabled

```typescript
invoke('set_retrieval_memory_enabled', { enabled: boolean }): Promise<void>
```

- **行为**：启用/禁用自进化检索记忆，持久化到 settings 表 `rag.retrieval_memory_enabled`。启用后系统自动记录查询类型 × 检索方法的效果统计，自适应选择最佳检索策略。
- **关联**：REQ-PERF-012 ｜ Session S9

### 2.47.12 get_retrieval_memory_stats

```typescript
invoke('get_retrieval_memory_stats'): Promise<MemoryRecord[]>
```

- **行为**：返回所有查询类型 × 检索方法的累计效果统计（hit_count / miss_count / avg_score / hit_rate）。
- **关联**：REQ-PERF-012 ｜ Session S9

### 2.47.13 reset_retrieval_memory

```typescript
invoke('reset_retrieval_memory'): Promise<void>
```

- **行为**：清空所有检索记忆记录，重置为冷启动状态。
- **关联**：REQ-PERF-012 ｜ Session S9

### 2.47.14 record_retrieval_feedback

```typescript
invoke('record_retrieval_feedback', {
  signal: {
    query: string,           // 用户查询文本
    method: string,          // 检索方法: 'vector_only' | 'hybrid' | 'hybrid_rerank' | 'proposition' | 'colbert'
    feedback: string,        // 反馈类型: 'retry_with_different_method' | 'edit_and_resend' | 'accepted' | 'thumbs_up' | 'thumbs_down'
    timestamp: number         // Unix 秒
  }
}): Promise<void>
```

- **行为**：记录用户反馈信号，调整检索方法的 hit_rate 权重（REQ-PERF-012 扩展：用户反馈自学习）。前端在检测到用户行为（重新提问 / 编辑重发 / 点赞 / 点踩 / 继续新话题）时调用。禁用检索记忆时静默跳过（向后兼容）。
- **反馈类型与 delta 权重**：
  - `retry_with_different_method` (−0.10)：用户重新提问（相似查询）→ 负信号
  - `edit_and_resend` (−0.15)：用户编辑后重发 → 负信号
  - `accepted` (+0.05)：用户接受回答（继续新话题）→ 正信号
  - `thumbs_up` (+0.20)：用户点赞 → 强正信号
  - `thumbs_down` (−0.25)：用户点踩 → 强负信号
- **关联**：REQ-PERF-012 扩展 ｜ v3.4.0

### 2.47.15 set_graph_retriever_enabled

```typescript
invoke('set_graph_retriever_enabled', { enabled: boolean }): Promise<void>
```

- **行为**：启用/禁用知识图谱图遍历检索，持久化到 settings 表 `rag.graph_retriever_enabled`。启用后，检索时沿实体关系图边扩展到关联 chunk，作为 RRF 融合的第四路检索通道（Vector+BM25+Entity+Graph 四路融合）。图遍历最大深度 2 跳，置信度×距离衰减加权排序。
- **关联**：REQ-RAG-027 ｜ Session S2（知识图谱检索管线接入）

### 2.47.16 set_progressive_injection

```typescript
invoke('set_progressive_injection', { enabled: boolean }): Promise<void>
```

- **行为**：启用/禁用渐进式上下文注入，持久化到 settings 表 `rag.progressive_injection`。启用后，ChatEngine 仅注入初始 top-2 检索片段，检测到"不确定"关键词时逐步追加更多片段，减少首 token 延迟。
- **关联**：REQ-PERF-010 ｜ v2.8.0

### 2.47.17 set_speculative_enabled

```typescript
invoke('set_speculative_enabled', { enabled: boolean }): Promise<void>
```

- **行为**：启用/禁用 Speculative RAG（小模型草稿 → 大模型验证），持久化到 settings 表 `rag.speculative_enabled`。启用后，chat_inner 使用简化 prompt 快速生成草稿，再由完整 prompt 验证/修正，提升首 token 速度。
- **关联**：REQ-PERF-011 ｜ v2.9.0

### 2.47.18 set_quality_gate_enabled

```typescript
invoke('set_quality_gate_enabled', { enabled: boolean }): Promise<void>
```

- **行为**：启用/禁用 RAG 质量门控，持久化到 settings 表 `rag.quality_gate_enabled`。启用后，检索后评估结果质量（分数低于阈值 0.6 时记录告警），不影响检索流程但提供质量可观测性。
- **关联**：REQ-RAG-028 ｜ v3.1.0

### 2.47.19 get_graph_data

```typescript
invoke('get_graph_data', { limit?: number }): Promise<GraphTriple[]>
```

- **行为**：获取知识图谱三元组数据，用于前端 D3 force-directed graph 渲染。默认返回前 200 条关系，避免大量数据导致前端渲染卡顿。
- **返回**：`GraphTriple[]`（subject / relation / object 三元组列表）
- **关联**：REQ-RAG-027 ｜ Session S3（前端图谱可视化）

### 2.47.16 get_entity_relations

```typescript
invoke('get_entity_relations', { entityText: string }): Promise<EntityRelation[]>
```

- **行为**：查询指定实体参与的所有关系（subject 或 object 等于 `entityText`），用于双击节点时展开该实体的详细关系列表。
- **返回**：`EntityRelation[]`（含 id / subject / relation_type / object / chunk_id / confidence）
- **关联**：REQ-RAG-027 ｜ Session S3（前端图谱可视化）

### 2.47.17 get_graph_stats

```typescript
invoke('get_graph_stats'): Promise<GraphStats>
```

- **行为**：获取知识图谱统计信息（去重实体总数 / 关系总数 / 关系类型分布），用于前端图谱面板头部概览。
- **返回**：`GraphStats`（total_entities / total_relations / relation_type_counts）
- **关联**：REQ-RAG-027 ｜ Session S3（前端图谱可视化）

### 2.47.18 get_entity_types

```typescript
invoke('get_entity_types', { entities: string[] }): Promise<Record<string, string>>
```

- **行为**：批量查询实体类型映射，接收实体文本列表，返回 `entity_text → entity_type` 映射。后端使用 SQL `WHERE entity_text IN (...)` 单次批量查询，避免 N+1 问题。用于前端图谱面板为每个节点渲染对应的实体类型图标。
- **返回**：`Record<string, string>` — 实体文本 → 类型映射（person / proper_noun / tech_term / identifier / date）
- **关联**：REQ-RAG-027 AC-24 ｜ Session 4（图谱可视化增强）

### 2.47.19 get_shortest_path

```typescript
invoke('get_shortest_path', { from: string, to: string }): Promise<GraphPath>
```

- **行为**：获取两个实体间的最短路径。后端获取全量邻接表后，使用 BFS 计算最短路径。返回路径节点列表和跳数。
- **返回**：`GraphPath` — `{ path: string[], hops: number }`（路径节点列表 + 跳数）
- **关联**：REQ-RAG-027 AC-32~AC-33 ｜ Session 5（图谱高级分析）

### 2.47.20 get_communities

```typescript
invoke('get_communities'): Promise<GraphCommunity>
```

- **行为**：社区检测。后端获取全量邻接表后，使用 Label Propagation 算法发现社区。迭代直到收敛或最大 10 轮。返回实体→社区 ID 映射和社区总数。
- **返回**：`GraphCommunity` — `{ communities: Record<string, number>, community_count: number }`
- **关联**：REQ-RAG-027 AC-38~AC-39 ｜ Session 5（图谱高级分析）

### 2.47.21 get_graph_layout

```typescript
invoke('get_graph_layout', { mode?: string }): Promise<string[]>
```

- **行为**：获取支持的布局模式列表（force/hierarchical/radial）。如果传入 mode 参数，验证是否在支持列表中。布局计算在前端 D3.js 完成。
- **返回**：`string[]` — 支持的布局模式列表
- **关联**：REQ-RAG-027 AC-34~AC-36 ｜ Session 5（图谱高级分析）

### 2.47.22 export_graph

```typescript
invoke('export_graph', { format: string }): Promise<string>
```

- **行为**：将知识图谱（实体 + 关系）导出为标准格式字符串。从 `entity_relations` 表读取全量关系，从关系中提取去重实体文本并通过 `get_entity_types` 查询类型，构建 `Vec<Entity>` 后调用 `export_graphml` 或 `export_jsonld` 生成格式字符串。前端通过 Blob 下载文件。
- **参数**：`format` — 导出格式，`"graphml"`（GraphML XML）或 `"jsonld"`（JSON-LD）
- **返回**：`string` — 导出格式的字符串内容
- **错误**：不支持的格式返回错误消息
- **关联**：REQ-EXP-006 ｜ v1.3 S54（知识图谱导出）

### 2.47.23 get_document_summary

```typescript
invoke('get_document_summary', { docId: string }): Promise<string | null>
```

- **行为**：获取指定文档的自动生成摘要。从 `documents.summary` 列读取，若摘要尚未生成（导入时 LLM 未配置或生成失败）返回 `null`。
- **参数**：`docId` — 文档 ID
- **返回**：`string | null` — 摘要文本或 null
- **错误**：文档不存在时返回错误消息
- **关联**：REQ-ING-019 ｜ v1.3 S55（文档摘要自动生成）

### 2.47.24 regenerate_summary

```typescript
invoke('regenerate_summary', { docId: string }): Promise<string>
```

- **行为**：重新生成指定文档的摘要。获取文档 chunks → 初始化 LLM Provider → 调用 `ImportService::generate_summary()` → 持久化到 `documents.summary` 列 → 返回新摘要。适用于用户切换 LLM 后重新生成摘要的场景。
- **参数**：`docId` — 文档 ID
- **返回**：`string` — 新生成的摘要文本
- **错误**：文档无分块内容 / LLM 未配置 / LLM 调用失败时返回错误消息
- **关联**：REQ-ING-019 ｜ v1.3 S55（文档摘要自动生成）

### 2.48 DAG 工作流引擎（REQ-RAG-030）

#### 2.48.1 save_workflow_template

```typescript
invoke('save_workflow_template', {
  name: string,
  description: string,
  nodes_json: string,
  edges_json: string
}): Promise<string>
```

- **行为**：创建并保存工作流模板。解析 nodes/edges JSON → 构建 `Workflow` → 序列化为 JSON → 存入 settings 表 `workflow.{id}` 键，并更新 `workflow.index` 索引。
- **参数**：
  - `name` — 工作流名称
  - `description` — 工作流描述
  - `nodes_json` — 节点列表 JSON 字符串（`WorkflowNode[]`）
  - `edges_json` — 边列表 JSON 字符串（`WorkflowEdge[]`）
- **返回**：`string` — 工作流 ID（UUID v4）
- **错误**：`PARSE:` JSON 解析失败 ｜ `VALIDATION:` 节点为空 ｜ `STORAGE:` 写入失败
- **关联**：REQ-RAG-030

#### 2.48.2 run_workflow

```typescript
invoke('run_workflow', {
  workflow_id: string,
  input: string
}): Promise<WorkflowResult>
```

- **行为**：执行工作流。从 settings 表读取工作流定义 → 构建 `WorkflowEngine`（HybridRetriever + OpenAIProvider）→ 拓扑排序 → 分层并行执行 → 返回各节点状态和最终输出。
- **参数**：
  - `workflow_id` — 工作流 ID
  - `input` — 初始输入文本（传递给入度为 0 的节点）
- **返回**：`WorkflowResult` — `{ node_results: Record<string, NodeStatus>, final_output: string, duration_ms: number }`
- **错误**：`VALIDATION:` 工作流不存在 / LLM 未配置 ｜ `EMBED:` 向量化引擎不可用 ｜ `LLM:` LLM 初始化失败 ｜ `UNKNOWN:` 执行错误（如环检测）
- **关联**：REQ-RAG-030

#### 2.48.3 list_workflows

```typescript
invoke('list_workflows'): Promise<Workflow[]>
```

- **行为**：列出所有已保存的工作流模板。读取 `workflow.index` 索引，逐个反序列化工作流定义。
- **返回**：`Workflow[]` — 所有已保存的工作流模板列表
- **关联**：REQ-RAG-030

#### 2.48.4 delete_workflow

```typescript
invoke('delete_workflow', { workflow_id: string }): Promise<void>
```

- **行为**：删除工作流模板。从 settings 表删除 `workflow.{id}` 键，并从 `workflow.index` 索引中移除。
- **参数**：`workflow_id` — 要删除的工作流 ID
- **关联**：REQ-RAG-030

### 2.49 代码符号引擎（3 个，1 个 Pro 门控）

#### 2.49.1 search_symbols

```typescript
invoke('search_symbols', {
  query: string,
  limit?: number
}): Promise<CodeSymbol[]>
```

- **行为**：搜索代码符号。精确匹配优先（`WHERE name = ?`），不足时追加模糊匹配（`WHERE name LIKE '%query%'`），去重后截断到 limit。
- **参数**：`query` — 符号名或关键词；`limit` — 返回上限（默认 20）
- **返回**：`CodeSymbol[]` — 匹配的符号列表（id/chunk_id/name/kind/language/start_line/end_line/signature）
- **关联**：REQ-RAG-031

#### 2.49.2 get_symbols_for_chunk

```typescript
invoke('get_symbols_for_chunk', {
  chunk_id: string
}): Promise<CodeSymbol[]>
```

- **行为**：获取指定 chunk 的所有代码符号。按 `start_line` 正序返回。
- **参数**：`chunk_id` — chunk ID
- **返回**：`CodeSymbol[]` — 该 chunk 包含的所有符号
- **关联**：REQ-RAG-031

#### 2.49.3 rebuild_symbol_index *(Pro)*

```typescript
invoke('rebuild_symbol_index'): Promise<number>
```

- **行为**：重建代码符号索引。遍历所有已索引的代码文件（.rs/.ts/.tsx/.py/.go），重新通过 tree-sitter AST 抽取符号并写入 `code_symbols` 表。Pro 专用功能。
- **返回**：`number` — 抽取的符号总数
- **Pro 门控**：是
- **关联**：REQ-RAG-031

### 2.50 代码执行沙箱（1 个，Pro 门控）

#### 2.50.1 execute_code_snippet *(Pro)*

```typescript
invoke('execute_code_snippet', {
  code: string,
  language: string,
  stdin?: string
}): Promise<ExecutionResult>
```

- **行为**：在安全沙箱中执行代码片段（Python / JavaScript）。安全限制：超时 10s、内存 64MB、无网络访问、临时目录隔离、环境变量清空。超时后进程被 kill，返回 `timed_out=true`。Pro 专用功能。
- **参数**：`code` — 代码文本；`language` — 编程语言（`"python"` / `"javascript"`）；`stdin` — 标准输入（可选）
- **返回**：`ExecutionResult` — `{ stdout: string, stderr: string, exit_code: number, duration_ms: number, timed_out: boolean }`
- **Pro 门控**：是
- **关联**：REQ-RAG-032

### 2.51 持久化记忆系统（6 个，REQ-RAG-033）

#### 2.51.1 set_memory_enabled

```typescript
invoke('set_memory_enabled', { enabled: boolean }): Promise<void>
```

- **行为**：启用/禁用持久化记忆系统，持久化到 settings 表 `memory.enabled`。启用后 ChatEngine 检索相关跨会话记忆注入 system prompt；AutoDream 后台自动整合记忆。
- **参数**：`enabled` — 是否启用
- **Pro 门控**：否
- **关联**：REQ-RAG-033

#### 2.51.2 get_memories

```typescript
invoke('get_memories', { tier?: string }): Promise<MemoryEntry[]>
```

- **行为**：获取记忆条目列表，可按层级过滤。返回结果按 importance DESC 排序。
- **参数**：`tier` — 层级过滤（`"wing"` / `"hall"` / `"room"`，省略则返回全部）
- **返回**：`MemoryEntry[]` — 记忆条目数组
- **Pro 门控**：否
- **关联**：REQ-RAG-033

#### 2.51.3 pin_memory

```typescript
invoke('pin_memory', {
  conversation_id: string,
  content: string
}): Promise<MemoryEntry>
```

- **行为**：用户手动置顶记忆，直接创建 Room 层条目（importance 1.0，永久保留）。
- **参数**：`conversation_id` — 关联会话 ID；`content` — 记忆内容文本
- **返回**：`MemoryEntry` — 创建的记忆条目
- **Pro 门控**：否
- **关联**：REQ-RAG-033

#### 2.51.4 promote_memory

```typescript
invoke('promote_memory', { memory_id: string }): Promise<void>
```

- **行为**：手动提升记忆层级（Wing → Hall → Room），每次提升 importance += 0.1（上限 1.0）。Room 层无法再提升。
- **参数**：`memory_id` — 记忆条目 ID
- **Pro 门控**：否
- **关联**：REQ-RAG-033

#### 2.51.5 delete_memory

```typescript
invoke('delete_memory', { memory_id: string }): Promise<void>
```

- **行为**：删除指定记忆条目。
- **参数**：`memory_id` — 记忆条目 ID
- **Pro 门控**：否
- **关联**：REQ-RAG-033

#### 2.51.6 clear_memories

```typescript
invoke('clear_memories', { tier?: string }): Promise<number>
```

- **行为**：清空记忆条目，可按层级过滤。返回删除的行数。
- **参数**：`tier` — 层级过滤（`"wing"` / `"hall"` / `"room"`，省略则清空全部）
- **返回**：`number` — 删除的行数
- **Pro 门控**：否
- **关联**：REQ-RAG-033

---

### 2.52 网页搜索集成（REQ-RAG-036）

当本地知识库检索结果不足时（top-1 score < 0.3），自动搜索互联网（DuckDuckGo Instant Answer API）补充 context，通过 RRF 融合到本地检索结果中。

#### 2.52.1 set_web_search_enabled

```typescript
invoke('set_web_search_enabled', { enabled: boolean }): Promise<void>
```

- **行为**：切换网页搜索开关。持久化到 settings 表 `rag.web_search_enabled` 键。启用后，chat 命令在本地检索 top-1 score < 阈值（0.3）时自动调用 DuckDuckGo API，将搜索结果通过 RRF 融合到本地检索结果中。搜索失败时优雅降级为仅使用本地结果。默认关闭（opt-in）。
- **参数**：`enabled` — 是否启用网页搜索
- **Pro 门控**：否
- **关联**：REQ-RAG-036

#### 2.52.2 web_search

```typescript
invoke('web_search', { query: string }): Promise<SearchResult[]>
```

- **行为**：直接执行网页搜索（DuckDuckGo Instant Answer API），返回搜索结果列表。用于前端独立搜索功能（非 chat 管线内的自动触发）。搜索失败返回空列表（不报错）。
- **参数**：`query` — 搜索查询文本
- **返回**：`SearchResult[]` — 搜索结果数组（`{ title, url, snippet, source }`）
- **Pro 门控**：否
- **关联**：REQ-RAG-036

---

### 2.53 自定义 ONNX 嵌入模型上传（3 个，Pro 门控，REQ-VEC-014）

> Pro 用户可上传自己的 ONNX 嵌入模型（如 bge-large、e5-large），替代 3 个预设模型。
> 模型文件保存到 `{data_dir}/custom_models/{model_name}/`。
> Free 用户调用任何命令均返回 `PRO_REQUIRED` 错误。

#### 2.53.1 upload_custom_embedding_model *(Pro)*

```typescript
invoke('upload_custom_embedding_model', {
  name: string,           // 模型名称（路径遍历字符 /\.. 替换为 _）
  onnx_path: string,       // ONNX 模型文件路径
  tokenizer_files: string[] // tokenizer 文件路径数组（tokenizer.json / vocab.txt 等）
}): Promise<CustomModelInfo>
```

- **行为**：将用户选择的 ONNX 模型文件和 tokenizer 文件复制到 `{data_dir}/custom_models/{sanitized_name}/`。验证 ONNX 文件头（magic number + 文件大小 ≥ 1KB）和 tokenizer 文件存在性。返回 `CustomModelInfo`（名称、维度、大小、有效性）。
- **参数**：`name` — 模型名称；`onnx_path` — ONNX 文件路径；`tokenizer_files` — tokenizer 文件路径数组
- **返回**：`CustomModelInfo` — `{ name: string, dim: number, size_bytes: number, is_valid: boolean }`
- **Pro 门控**：是（Free 返回 `PRO_REQUIRED` 错误）
- **关联**：REQ-VEC-014 AC-1

#### 2.53.2 list_custom_models *(Pro)*

```typescript
invoke('list_custom_models'): Promise<CustomModelInfo[]>
```

- **行为**：扫描 `{data_dir}/custom_models/` 目录，返回所有已上传的自定义模型列表。每个模型包含名称、大小、文件完整性状态（ONNX 文件存在 + tokenizer 文件齐全 → `is_valid: true`）。
- **返回**：`CustomModelInfo[]` — 自定义模型信息数组
- **Pro 门控**：是（Free 返回 `PRO_REQUIRED` 错误）
- **关联**：REQ-VEC-014 AC-2/AC-6

#### 2.53.3 delete_custom_model *(Pro)*

```typescript
invoke('delete_custom_model', { name: string }): Promise<void>
```

- **行为**：删除指定自定义模型目录及其所有文件。模型名称中的路径遍历字符（`/`、`\`、`..`）被替换为 `_` 以防止目录遍历攻击。如果当前正在使用该模型，embedder 缓存会被清除。
- **参数**：`name` — 要删除的模型名称
- **Pro 门控**：是（Free 返回 `PRO_REQUIRED` 错误）
- **关联**：REQ-VEC-014 AC-4/AC-7