/**
 * EchoMind IPC 封装层 — 对 Tauri window.__TAURI__ 的类型安全封装。
 *
 * 设计原则：
 * 1. 单一入口 — 所有 Tauri 调用通过此模块，便于审计和 Mock
 * 2. 契约明确 — 每个命令的参数和返回类型通过 JSDoc 标注
 * 3. 可 Mock — 测试中可替换 window.__TAURI__ 实现隔离
 */

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
  /** 导出会话为 Markdown（REQ-EXP-001）*/
  exportMarkdown: (conversationId) => invoke('export_conversation_markdown', { conversationId }),
  /** 保存文本文件到指定路径（REQ-EXP-001 辅助）*/
  saveTextFile: (path, content) => invoke('save_text_file', { path, content }),
  /** Session Strip: strip messages by index range (REQ-RAG-046) */
  stripMessages: (conversationId, fromIndex, toIndex, replaceWithSummary, summaryText) =>
    invoke('strip_messages', { conversationId, fromIndex, toIndex, replaceWithSummary, summaryText }),
  /** Session Strip: keep last N messages (REQ-RAG-046) */
  stripKeepingRecent: (conversationId, keepLastN, replaceWithSummary, summaryText) =>
    invoke('strip_keeping_recent', { conversationId, keepLastN, replaceWithSummary, summaryText }),
  /** Session Strip: preview messages to be stripped (REQ-RAG-046) */
  previewStrip: (conversationId, fromIndex, toIndex) =>
    invoke('preview_strip', { conversationId, fromIndex, toIndex }),
};

/** 文档相关 */
export const docApi = {
  list: () => invoke('get_documents'),
  delete: (id) => invoke('delete_document', { id }),
  retry: (id) => invoke('retry_index', { id }),
  import: (paths) => invoke('import_files', { paths }),
  // 文档标签系统（REQ-ING-022）
  addTag: (docId, tag) => invoke('add_document_tag', { docId, tag }),
  removeTag: (docId, tag) => invoke('remove_document_tag', { docId, tag }),
  listAllTags: () => invoke('list_all_tags'),
  filterByTag: (tag) => invoke('filter_documents_by_tag', { tag }),
};

/** 对话/审计相关 */
export const chatApi = {
  send: (query, history, conversationId) => invoke('chat', { query, history, conversationId }),
  abort: (conversationId) => invoke('abort_chat', { conversationId }),
  audit: (docId, docName) => invoke('audit_document', { docId, docName }),
  abortAudit: (docId) => invoke('abort_audit', { docId }),
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

/** VLM 相关 */
export const vlmApi = {
  setEnabled: (enabled) => invoke('set_vlm_enabled', { enabled }),
};

/** 嵌入模型相关（REQ-VEC-012 + REQ-VEC-014） */
export const embedModelApi = {
  /** 切换嵌入模型（返回 void，出错抛异常） */
  setModel: (model) => invoke('set_embedding_model', { model }),
  /** 上传自定义 ONNX 嵌入模型（REQ-VEC-014，Pro 门控，返回 CustomModelInfo） */
  uploadCustomModel: (name, onnxPath, tokenizerFiles) =>
    invoke('upload_custom_embedding_model', { name, onnxPath, tokenizerFiles }),
  /** 列出已上传的自定义嵌入模型（REQ-VEC-014，Pro 门控，返回 CustomModelInfo[]） */
  listCustomModels: () => invoke('list_custom_models'),
  /** 删除自定义嵌入模型（REQ-VEC-014，Pro 门控，返回 void） */
  deleteCustomModel: (name) => invoke('delete_custom_model', { name }),
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

/** 导入取消 */
export const importApi = {
  abort: () => invoke('abort_import'),
};

/** 安全防御相关 */
export const securityApi = {
  getStatus: () => invoke('get_security_status'),
  encrypt: (password) => invoke('encrypt_database', { password }),
  unlock: (password) => invoke('unlock_database', { password }),
  lock: () => invoke('lock_app'),
  setAutoLockTimeout: (seconds) => invoke('set_auto_lock_timeout', { seconds }),
  detectPii: (text) => invoke('detect_pii', { text }),
  redactPii: (text) => invoke('redact_pii', { text }),
  getAuditLogs: (limit = 100) => invoke('get_audit_logs', { limit }),
  verifyAuditChain: () => invoke('verify_audit_chain'),
  setPiiDetection: (enabled) => invoke('set_pii_detection_enabled', { enabled }),
  clearClipboard: () => invoke('clear_clipboard'),
  setClipboardClearTimeout: (seconds) => invoke('set_clipboard_clear_timeout', { seconds }),
  checkStatus: () => invoke('check_security_status'),
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
