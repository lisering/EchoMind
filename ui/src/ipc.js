/**
 * EchoMind IPC 封装层 — 对 Tauri window.__TAURI__ 的类型安全封装。
 *
 * 设计原则：
 * 1. 单一入口 — 所有 Tauri 调用通过此模块，便于审计和 Mock
 * 2. 契约明确 — 每个命令的参数和返回类型通过 JSDoc 标注
 * 3. 可 Mock — 测试中可替换 window.__TAURI__ 实现隔离
 */

/**
 * 是否为开发模式（Debug 构建）。
 * S10: 开发者工具命令在 Release 构建中不注册，前端调用时静默跳过。
 * E2E 测试环境（window.__TAURI__ 存在 mock）始终视为开发模式。
 */
const __DEV_MODE__ = (() => {
  // E2E bridge 测试注入 mock __TAURI__，始终启用
  if (typeof window !== 'undefined' && window.__TAURI__ && window.__TAURI__.core) {
    return true;
  }
  // Tauri 环境下无法直接检测 debug_assertions，保守策略：
  // 只要 __TAURI__ 存在即允许调用，Release 构建中后端不注册命令时
  // invoke 会 reject，devInvoke 捕获错误并返回空默认值，实现静默降级。
  return true;
})();

/**
 * 开发者工具专用 invoke 封装（S10）。
 * 在 Release 构建中后端不注册对应命令时，invoke 会 reject，
 * 此函数捕获错误并返回空默认值，实现静默降级。
 * @param {string} cmd - 命令名
 * @param {Object} args - 命令参数
 * @param {*} [fallback=null] - 失败时的默认返回值
 * @returns {Promise<any>}
 */
function devInvoke(cmd, args = {}, fallback = null) {
  return invoke(cmd, args).catch(() => fallback);
}

/**
 * 调用 Tauri IPC 命令。
 * @param {string} cmd - 命令名（如 'chat', 'import_files'）
 * @param {Object} [args={}] - 命令参数
 * @returns {Promise<any>} 命令返回值
 */
export function invoke(cmd, args = {}) {
  return window.__TAURI__.core.invoke(cmd, args);
}

/**
 * 监听 Tauri 事件。
 * @param {string} name - 事件名（如 'chat_token', 'chat_done'）
 * @param {(payload: any) => void} cb - 事件回调
 * @returns {Promise<() => void>} 取消监听函数
 */
export function listen(name, cb) {
  return window.__TAURI__.event.listen(name, cb);
}

/**
 * 打开文件选择对话框。
 * @param {Object} [options] - 对话框选项
 * @returns {Promise<string|string[]|null>} 选中文件路径
 */
export function openDialog(options) {
  return window.__TAURI__.dialog.open(options);
}

/**
 * 打开保存文件对话框。
 * @param {Object} [options] - 对话框选项（如 { defaultPath, filters }）
 * @returns {Promise<string|null>} 选中保存路径，用户取消时返回 null
 */
export function saveDialog(options) {
  return window.__TAURI__.dialog.save(options);
}

/**
 * 在系统默认浏览器中打开 URL。
 * @param {string} url - 要打开的 URL
 */
export async function openUrl(url) {
  try {
    await window.__TAURI__.opener.openUrl(url);
  } catch (_) {
    // 静默失败，调用方自行处理
  }
}

// ============================================================
// 高层封装 — 按业务域分组的 IPC 调用
// ============================================================

/**
 * 统一设置 API（S09 IPC 精简）。
 * 替代 ~20 个 set_xxx 命令，通过 `update_setting(key, value)` 统一入口。
 * 布尔值传 `"true"` / `"false"`，数值传字符串形式。
 */
export const settingsApi = {
  /**
   * 更新设置项（S09 统一入口）。
   * @param {string} key - 设置键（如 "rag.hybrid_search"）
   * @param {string} value - 设置值（布尔传 "true"/"false"）
   * @returns {Promise<void>}
   */
  update: (key, value) => invoke('update_setting', { key, value }),

  /**
   * 读取设置项原始值。
   * @param {string} key - 设置键
   * @returns {Promise<string>} 设置值字符串（键不存在时返回空字符串）
   */
  get: (key) => invoke('get_setting', { key }),

  // ── 便捷布尔封装 ──
  /** 设置布尔开关 */
  setBool: (key, enabled) => invoke('update_setting', { key, value: String(enabled) }),
};

/** LLM 配置相关 */
export const llmApi = {
  getSettings: () => invoke('get_settings'),
  updateConfig: (config) => invoke('update_llm_config', { config }),
  testConnection: (apiKey, baseUrl, model) => invoke('test_llm_connection', { apiKey, baseUrl, model }),
};

/** 会话相关 */
export const convApi = {
  create: (workspaceId) => invoke('create_conversation', { workspaceId }),
  list: (workspaceId) => invoke('get_conversations', { workspaceId }),
  messages: (conversationId) => invoke('get_messages', { conversationId }),
  /** 编辑用户消息并创建新版本（DB 持久化，返回新版本号） */
  editUserMessage: (conversationId, turnGroup, newContent, originalMessageId = null) =>
    invoke('edit_user_message', { conversationId, turnGroup, newContent, originalMessageId }),
  /** 设置轮次的活跃版本号（持久化分支切换状态） */
  setTurnActiveVersion: (conversationId, turnGroup, activeVersion) =>
    invoke('set_turn_active_version', { conversationId, turnGroup, activeVersion }),
  /** 获取会话中所有轮次的活跃版本号 */
  getTurnActiveVersions: (conversationId) =>
    invoke('get_turn_active_versions', { conversationId }),
  /** 获取会话的分支树结构（REQ-RAG-039） */
  getConversationTree: (conversationId) =>
    invoke('get_conversation_tree', { conversationId }),
  /** 从指定消息创建新分支（REQ-RAG-039） */
  branchFromMessage: (conversationId, messageId, newContent) =>
    invoke('branch_from_message', { conversationId, messageId, newContent }),
  delete: (id) => invoke('delete_conversation', { id }),
  /** 重命名会话（REQ-IX-001：右键菜单重命名） */
  rename: (id, title) => invoke('rename_conversation', { id, title }),
  /** 分页获取会话列表（大列表性能优化） */
  listPaginated: (offset, limit, workspaceId) =>
    invoke('get_conversations_paginated', { offset, limit, workspaceId }),
  /** 分页获取消息列表（长对话性能优化） */
  messagesPaginated: (conversationId, offset, limit) =>
    invoke('get_messages_paginated', { conversationId, offset, limit }),
  /** 导出会话为 Markdown（REQ-EXP-001）*/
  exportMarkdown: (conversationId) => invoke('export_conversation_markdown', { conversationId }),
  /** 保存文本文件到指定路径（REQ-EXP-001 辅助）*/
  saveTextFile: (path, content) => invoke('save_text_file', { path, content }),
  /** 删除单条消息（REQ-RAG-013：user 消息连带删除下一条 assistant） */
  deleteMessage: (conversationId, messageId) =>
    invoke('delete_message', { conversationId, messageId }),
};

/** 文档相关 */
export const docApi = {
  list: (sortBy, sortOrder) => invoke('get_documents', { sortBy, sortOrder }),
  delete: (id) => invoke('delete_document', { id }),
  retry: (id) => invoke('retry_index', { id }),
  rebuild: (id) => invoke('rebuild_index', { id }),
  import: (paths) => invoke('import_files', { paths }),
  // 文档标签系统（REQ-ING-022）
  addTag: (docId, tag) => invoke('add_document_tag', { docId, tag }),
  removeTag: (docId, tag) => invoke('remove_document_tag', { docId, tag }),
  listAllTags: () => invoke('list_all_tags'),
  filterByTag: (tag) => invoke('filter_documents_by_tag', { tag }),
};

/** 文档内容预览（REQ-ING-010） */
export const docPreviewApi = {
  /** 获取文档预览数据（元数据 + 前 500 字 + chunk 列表） */
  getPreview: (docId) => invoke('get_document_preview', { docId }),
};

/** 文档原文导出（REQ-EXP-004） */
export const docExportApi = {
  /** 导出文档原始文件副本到目标路径 */
  exportOriginal: (docId, destPath) => invoke('export_document_original', { docId, destPath }),
};

/** 对话/审计相关 */
export const chatApi = {
  send: (query, history, conversationId) => invoke('chat', { query, history, conversationId }),
  abort: (conversationId) => invoke('abort_chat', { conversationId }),
  audit: (docId, docName) => invoke('audit_document', { docId, docName }),
  abortAudit: (docId) => invoke('abort_audit', { docId }),
  /** 批量更新会话排序（REQ-IX-002 拖拽排序持久化） */
  reorder: (orderedIds) => invoke('reorder_conversations', { orderedIds }),
  /** 对话书签（REQ-RAG-047） */
  addBookmark: (conversationId, note) => invoke('add_bookmark', { conversationId, note }),
  removeBookmark: (conversationId) => invoke('remove_bookmark', { conversationId }),
  listBookmarks: () => invoke('list_bookmarks'),
  isBookmarked: (conversationId) => invoke('is_bookmarked', { conversationId }),
};

/** 授权相关 */
export const licenseApi = {
  getProStatus: () => invoke('get_pro_status'),
  activate: (licenseKey) => invoke('activate_pro', { licenseKey }),
  deactivate: () => invoke('deactivate_pro'),
};

/** 模型缓存相关 */
export const modelApi = {
  init: () => invoke('init_embedder'),
  cacheInfo: () => invoke('get_model_cache_info'),
  clearCache: (modelName) => invoke('clear_model_cache', { modelName }),
};

/** VLM 相关（S09: set_vlm_enabled → settingsApi.update） */
export const vlmApi = {
  /** 设置 VLM 开关 */
  setEnabled: (enabled) => settingsApi.update('mm.vlm_enabled', String(enabled)),
};

/** 嵌入模型相关（REQ-VEC-012 + REQ-VEC-014 + REQ-VEC-017） */
export const embedModelApi = {
  /** 切换嵌入模型（返回 void，出错抛异常） */
  setModel: (model) => settingsApi.update('vec.embedding_model', model),
  /** 上传自定义 ONNX 嵌入模型（REQ-VEC-014，Pro 门控，返回 CustomModelInfo） */
  uploadCustomModel: (name, onnxPath, tokenizerFiles) =>
    invoke('upload_custom_embedding_model', { name, onnxPath, tokenizerFiles }),
  /** 列出已上传的自定义嵌入模型（REQ-VEC-014，Pro 门控，返回 CustomModelInfo[]） */
  listCustomModels: () => invoke('list_custom_models'),
  /** 删除自定义嵌入模型（REQ-VEC-014，Pro 门控，返回 void） */
  deleteCustomModel: (name) => invoke('delete_custom_model', { name }),
};

/** 嵌入模型下载镜像源配置（REQ-VEC-017） */
export const mirrorSourceApi = {
  /** 设置镜像源（auto / modelscope / hf-mirror / huggingface） */
  set: (source) => invoke('set_mirror_source', { source }),
  /** 获取当前镜像源（返回 string，默认 "auto"） */
  get: () => invoke('get_mirror_source'),
};

/** 分块参数配置（REQ-VEC-011） */
export const chunkParamsApi = {
  /** 获取分块参数（返回 {chunk_size, overlap}） */
  get: () => invoke('get_chunk_params'),
  /** 设置分块参数（参数变更后新导入文档使用新参数） */
  set: (chunkSize, overlap) => invoke('set_chunk_params', { params: { chunk_size: chunkSize, overlap } }),
};

/** 文件监听 + 增量同步相关（REQ-SYNC-001~003） */
export const syncApi = {
  /** 添加监听文件夹（返回 SyncResult） */
  add: (path) => invoke('add_watched_folder', { path }),
  /** 移除监听文件夹 */
  remove: (path) => invoke('remove_watched_folder', { path }),
  /** 获取监听文件夹列表 */
  list: () => invoke('get_watched_folders'),
};

/** 本地 LLM 推理相关（REQ-LLM-003/004） */
export const localLlmApi = {
  /** 列出已下载的本地 GGUF 模型 */
  listModels: () => invoke('list_local_models'),
  /** 获取推荐模型列表 */
  getRecommended: () => invoke('get_recommended_models'),
  /** 下载模型文件（后台执行，通过 model_download_progress 事件推送进度） */
  download: (url, filename) => invoke('download_model', { url, filename }),
  /** 删除本地模型文件 */
  deleteModel: (filename) => invoke('delete_model', { filename }),
  /** 切换推理模式（remote / local） */
  setMode: (mode) => invoke('set_llm_mode', { mode }),
  /** 获取当前推理模式 */
  getMode: () => invoke('get_llm_mode'),
  /** 设置当前选中的本地模型文件名 */
  setLocalModel: (filename) => invoke('set_local_model', { filename }),
  /** 设置采样参数（S11，Pro 功能） */
  setSamplingParams: (params) => invoke('set_sampling_params', { params }),
  /** 获取当前设备类型（CPU/Metal/CUDA，Pro 功能） */
  getDeviceKind: () => invoke('get_local_llm_device_kind'),
  /** 设置嵌入模型（Pro 功能，运行时切换 ONNX 嵌入模型） */
  setEmbedderModel: (model) => invoke('set_embedder_model', { model }),
  /** 设置 PagedAttention 参数（Pro 功能） */
  setPagedAttn: (enabled, blockSize, gpuMemoryCtx) =>
    invoke('set_paged_attn', { enabled, blockSize, gpuMemoryCtx }),
  /** 获取 GEMV 内核模式（Pro 功能） */
  getKernelMode: () => invoke('get_kernel_mode'),
  /** 设置 GEMV 内核模式（Pro 功能） */
  setKernelMode: (mode) => invoke('set_kernel_mode', { mode }),
};

/** 知识图谱布局相关 */
export const graphApi = {
  /** 获取后端布局算法建议列表（用于持久化用户偏好） */
  getLayout: () => invoke('get_graph_layout'),
};

/** KV Cache 管理相关（Pro 功能，跨会话 KV Cache 复用） */
export const kvCacheApi = {
  /** 保存当前会话的 KV Cache 到磁盘 */
  save: (conversationId) => invoke('save_kv_cache', { conversationId }),
  /** 从磁盘恢复指定会话的 KV Cache，返回是否成功 */
  load: (conversationId) => invoke('load_kv_cache', { conversationId }),
  /** 清除指定会话的 KV Cache 文件 */
  clear: (conversationId) => invoke('clear_kv_cache', { conversationId }),
  /** 获取 KV Cache 状态（enabled/cache_dir/file_count/total_size_bytes） */
  getStatus: () => invoke('get_kv_cache_status'),
  /** 启用/禁用 KV Cache 功能 */
  setEnabled: (enabled) => invoke('set_kv_cache_enabled', { enabled }),
};

/** 健壮下载管理相关（REQ-LLM-004 v2：断点续传 + 暂停/恢复/取消 + 崩溃恢复） */
export const downloadApi = {
  /** 暂停指定文件的下载（保留 .partial + .meta.json，可恢复） */
  pause: (filename) => invoke('pause_download', { filename }),
  /** 取消指定文件的下载 + 清理临时文件 */
  abort: (filename) => invoke('abort_download', { filename }),
  /** 获取指定文件的下载状态（从 .meta.json 读取） */
  getStatus: (filename) => invoke('get_download_status', { filename }),
  /** 列出所有未完成下载（扫描 .meta.json 文件） */
  listPending: () => invoke('list_pending_downloads'),
  /** 清理所有 .partial + .meta.json 文件，返回释放的字节数 */
  cleanupPartials: () => invoke('cleanup_partial_downloads'),
  /** 启动时扫描崩溃恢复（检测 .partial + .meta.json 文件） */
  scanRecovery: () => invoke('scan_download_recovery'),
};

/** 导入取消 + 文档替换（REQ-ING-012） */
export const importApi = {
  abort: () => invoke('abort_import'),
  replaceDocument: (filePath, oldDocId) => invoke('replace_document', { filePath, oldDocId }),
};

/** 安全防御相关 */
export const securityApi = {
  getStatus: () => invoke('get_security_status'),
  encrypt: (password) => invoke('encrypt_database', { password }),
  unlock: (password) => invoke('unlock_database', { password }),
  lock: () => invoke('lock_app'),
  /** 设置自动锁屏超时（委托到 set_auto_lock_config） */
  setAutoLockTimeout: (seconds) => invoke('set_auto_lock_config', { enabled: true, timeoutSecs: seconds, lockOnSleep: true }),
  detectPii: (text) => invoke('detect_pii', { text }),
  /** 脱敏文本（委托到 detect_pii，返回结果中含 redacted 字段） */
  redactPii: async (text) => { const r = await invoke('detect_pii', { text }); return r.redacted || text; },
  getAuditLogs: (limit = 100) => invoke('get_audit_logs', { limit }),
  verifyAuditChain: () => invoke('verify_audit_chain'),
  setPiiDetection: (enabled) => invoke('set_pii_detection_enabled', { enabled }),
  /** 清除剪贴板（委托到 set_clipboard_config，临时禁用再恢复） */
  clearClipboard: () => invoke('set_clipboard_config', { enabled: false, clearAfterSecs: 0 }),
  /** 设置剪贴板清除超时（委托到 set_clipboard_config） */
  setClipboardClearTimeout: (seconds) => invoke('set_clipboard_config', { enabled: true, clearAfterSecs: seconds }),
  /** 检查安全状态（委托到 get_security_status） */
  checkStatus: () => invoke('get_security_status'),
  // S2 复盘 — 僵尸命令接线
  unlockApp: () => invoke('unlock_app'),
  recordActivity: () => invoke('record_activity'),
  setAutoLockConfig: (enabled, timeoutSecs, lockOnSleep) =>
    invoke('set_auto_lock_config', { enabled, timeoutSecs, lockOnSleep }),
  checkPasswordStrength: (password) => invoke('check_password_strength', { password }),
  setPanicWipePassword: (password) => invoke('set_panic_wipe_password', { password }),
  clearPanicWipePassword: () => invoke('clear_panic_wipe_password'),
  isPanicWipeEnabled: () => invoke('is_panic_wipe_enabled'),
  clearAuditLogs: () => invoke('clear_audit_logs'),
  /** 导出审计报告（Markdown/JSON） */
  exportAuditReport: (format) => invoke('export_audit_report', { format }),
  setClipboardConfig: (enabled, clearAfterSecs) =>
    invoke('set_clipboard_config', { enabled, clearAfterSecs }),
  setSecurityPosture: (posture) => invoke('set_security_posture', { posture }),
  getSecurityPosture: () => invoke('get_security_posture'),
  getSecurityScreenStats: () => invoke('get_security_screen_stats'),
  resetSecurityScreenStats: () => invoke('reset_security_screen_stats'),
};

/** 自定义快捷指令模板相关（S56） */
export const promptTemplateApi = {
  /** 创建或更新模板（返回模板 ID） */
  save: (name, label, description, icon, promptTemplate) =>
    invoke('save_prompt_template', { name, label, description, icon, promptTemplate }),
  /** 列出所有自定义模板 */
  list: () => invoke('list_prompt_templates'),
  /** 删除指定模板 */
  delete: (templateId) => invoke('delete_prompt_template', { templateId }),
};

/** Trace 链路追踪相关（S2 复盘 — 僵尸命令接线，S10 开发者工具门控） */
export const traceApi = {
  /** 获取最近的 trace 记录列表 */
  getRecent: (limit = 20) => devInvoke('get_recent_traces', { limit }, []),
  /** 获取指定 ID 的 trace 记录详情 */
  getDetail: (id) => devInvoke('get_trace_detail', { id }, null),
  /** 清空所有 trace 记录 */
  clear: () => devInvoke('clear_traces', {}, null),
  /** 获取 trace 记录数量 */
  getCount: () => devInvoke('get_trace_count', {}, 0),
};

/** Token 预算相关（S2 复盘 — 僵尸命令接线，S10 开发者工具门控） */
export const budgetApi = {
  /** 获取预算统计 */
  getStats: () => devInvoke('get_budget_stats', {}, null),
  /** 设置预算限制（日限额美元） */
  setLimit: (dailyLimitUsd) => devInvoke('set_budget_limit', { daily_limit_usd: dailyLimitUsd }),
  /** 获取 Token 预算配置 */
  getConfig: () => invoke('get_token_budget_config'),
  /** 设置 Token 预算配置 */
  setConfig: (config) => invoke('set_token_budget_config', { config }),
};

/** 磁盘空间相关（P1-5 三件套接线：REQ-ERR-004） */
export const diskApi = {
  /** 获取数据目录磁盘空间信息（JSON 字符串，含 free_bytes/total_bytes/free_percent/is_low） */
  getInfo: () => invoke('get_disk_space_info'),
  /** 清理可回收空间（缓存/临时文件），返回释放字节数 */
  cleanup: () => invoke('cleanup_disk_space'),
  /** 检查磁盘空间是否充足（requiredBytes 为预估所需字节） */
  check: (requiredBytes) => invoke('check_disk_space', { requiredBytes }),
};


/** 文档重分类（S2 复盘 — 僵尸命令接线） */
export const docExtApi = {
  /** 重新分类文档领域 */
  reclassify: (docId) => invoke('reclassify_document', { docId }),
};

/** KB 统计仪表盘（REQ-KB-003 v1.5） */
export const kbStatsApi = {
  /** 获取知识库统计仪表盘数据 */
  getStats: () => invoke('get_kb_stats'),
};

/** 窗口管理设置（REQ-WIN-003 v1.6 — close-to-tray） */
export const windowSettingsApi = {
  /** 获取「关闭窗口时最小化到托盘」设置 */
  getCloseToTray: () => invoke('get_close_to_tray'),
  /** 设置「关闭窗口时最小化到托盘」（S09: → settingsApi.update） */
  setCloseToTray: (enabled) => settingsApi.update('window.close_to_tray', String(enabled)),
};

/** 错误日志导出（REQ-ERR-005 v1.6） */
export const errorLogsApi = {
  /** 导出错误日志（返回 JSON Lines 格式字符串） */
  export: () => invoke('export_error_logs'),
};

/** 代码符号查询相关（S4 复盘 — 僵尸命令接线） */
export const symbolApi = {
  /** 搜索代码符号（全局搜索） */
  search: (query, language) => invoke('search_symbols', { query, language }),
  /** 获取指定 chunk 的代码符号列表 */
  getForChunk: (chunkId) => invoke('get_symbols_for_chunk', { chunkId }),
  /** 重建符号索引 */
  rebuildIndex: () => invoke('rebuild_symbol_index'),
};

/** 网页搜索相关（S4 复盘 — 僵尸命令接线） */
export const webSearchApi = {
  /** 执行网页搜索并融合到 RAG 结果中 */
  search: (query) => invoke('web_search', { query }),
};

/** RAG 检索参数（REQ-RAG-014 v1.9 后端，v1.10 前端） */
export const ragParamsApi = {
  /** 获取 RAG 检索参数 */
  get: () => invoke('get_rag_params'),
  /** 设置 RAG 检索参数 */
  set: (params) => invoke('set_rag_params', { params }),
};

/** LLM 生成参数（REQ-RAG-015 v1.9 后端，v1.10 前端） */
export const generationParamsApi = {
  /** 获取 LLM 生成参数 */
  get: () => invoke('get_generation_params'),
  /** 设置 LLM 生成参数 */
  set: (params) => invoke('set_generation_params', { params }),
};

/** 开机自启（REQ-WIN-004 v1.13）（S09: set_autostart → settingsApi.update） */
export const autostartApi = {
  /** 获取开机自启状态 */
  get: () => invoke('get_autostart'),
  /** 设置开机自启 */
  set: (enabled) => settingsApi.update('app.autostart', String(enabled)),
};

/** 应用更新检查（REQ-HELP-004 v1.13）（S09: set_update_check_enabled → settingsApi.update） */
export const updateCheckApi = {
  /** 检查 GitHub Releases 是否有新版本 */
  check: () => invoke('check_for_updates'),
  /** 获取更新检查配置 */
  getConfig: () => invoke('get_update_check_config'),
  /** 设置自动检查开关 */
  setEnabled: (enabled) => settingsApi.update('update.auto_check', String(enabled)),
};

/** 导入历史记录（REQ-ING-011） */
export const importHistoryApi = {
  /** 查询导入历史（可选按结果筛选） */
  list: (resultFilter) => invoke('get_import_history', resultFilter ? { resultFilter } : {}),
  /** 清空导入历史 */
  clear: () => invoke('clear_import_history'),
};

/** 智能模式（S5 审计 P0-1） */
export const smartModeApi = {
  /** 设置智能模式开关 */
  set: (enabled) => invoke('set_smart_mode', { enabled }),
  /** 查询智能模式是否启用 */
  get: () => invoke('get_smart_mode'),
};

/** 演示模式（REQ-RAG-051 无 Key 演示模式） */
export const demoModeApi = {
  /** 检查是否处于演示模式 */
  isDemoMode: () => invoke('is_demo_mode'),
  /** 退出演示模式（清除示例文档 + 设置 rag.demo_mode = false） */
  exit: () => invoke('exit_demo_mode'),
  /** 加载示例文档（3 个预设文档 + 设置 rag.demo_mode = true） */
  loadDemoDocuments: () => invoke('load_demo_documents'),
};

/** RAG 评估指标相关（REQ-RAG-045，S10 开发者工具门控） */
export const ragEvalApi = {
  /** 评估单个 RAG 响应 */
  evaluate: (sample) => devInvoke('evaluate_rag_response', { sample }, []),
  /** 批量评估多个 RAG 响应 */
  evaluateBatch: (samples) => devInvoke('evaluate_rag_batch', { samples }, null),
  /** 获取 RAG 评估设置 */
  getSettings: () => devInvoke('get_rag_eval_settings', {}, null),
  /** 设置 RAG 评估设置 */
  setSettings: (settings) => devInvoke('set_rag_eval_settings', { settings }),
};
