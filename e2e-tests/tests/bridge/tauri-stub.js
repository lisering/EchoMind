// 契约级 Mock：window.__TAURI__（与 SRS 冻结命令/事件一一对应，仅供 L3-lite 桥接测试）。
// 覆盖命令（117+ 全量 IPC，含前端 securityApi 别名 + Pro 门控命令）：
//   基础: get_settings / update_llm_config / test_llm_connection / import_files / get_file_sizes /
//         get_file_size_limits / get_documents / delete_document / retry_index /
//         create_conversation / get_conversations / get_conversations_paginated /
//         get_messages / get_messages_paginated / delete_conversation / chat / abort_chat /
//         abort_import / activate_pro / deactivate_pro / get_pro_status / set_vlm_enabled /
//         set_hybrid_search / set_context_token_limit / edit_user_message / set_turn_active_version /
//         get_turn_active_versions
//   LLM:  list_local_models / get_recommended_models / download_model / delete_model /
//         set_llm_mode / get_llm_mode / set_local_model / pause_download / abort_download /
//         get_download_status / list_pending_downloads / cleanup_partial_downloads /
//         scan_download_recovery
//   LLM Pro: get_local_llm_device_kind / set_paged_attn / set_sampling_params /
//            set_kernel_mode / get_kernel_mode / save_kv_cache / load_kv_cache /
//            clear_kv_cache / get_kv_cache_status / set_kv_cache_enabled / set_embedder_model
//   审计: audit_document / abort_audit
//   嵌入: init_embedder / check_embedder_status / get_model_cache_info / clear_model_cache
//   RAG:  set_rerank_enabled / set_hyde_enabled / set_agent_enabled / set_embedding_model /
//         reclassify_document / set_coordinator_mode / set_sub_agent_enabled
//   知识图谱: get_graph_data / get_entity_relations / get_graph_stats / get_entity_types /
//             get_shortest_path / get_communities / get_graph_layout / set_graph_retriever_enabled / export_graph
//   DAG 工作流: save_workflow_template / run_workflow / list_workflows / delete_workflow
//   记忆系统: set_memory_enabled / get_memories / pin_memory / promote_memory /
//             delete_memory / clear_memories
//   代码符号: search_symbols / get_symbols_for_chunk / rebuild_symbol_index
//   代码执行: execute_code_snippet
//   AutoDream: trigger_dream / get_dream_suggestions / abort_dream
//   性能优化: get_cache_stats / clear_cache / set_cache_settings / get_cache_settings /
//             set_compression_ratio / get_compression_ratio / rebuild_bm25_index /
//             rebuild_proposition_index / build_summary_tree /
//             set_retrieval_memory_enabled / get_retrieval_memory_stats /
//             reset_retrieval_memory / record_retrieval_feedback
//   导出: export_conversation_markdown / save_text_file
//   同步: add_watched_folder / remove_watched_folder / get_watched_folders
//   可观测性: get_log_level / set_log_level / export_logs / export_diagnostics / open_data_dir
//   Token: get_conversation_cost / set_token_budget
//   Budget: get_budget_stats / set_budget_limit
//   安全: get_security_status / set_auto_lock_config / lock_app / unlock_app / record_activity /
//         detect_pii / redact_pii / set_panic_wipe_password / clear_panic_wipe_password /
//         is_panic_wipe_enabled / set_clipboard_config / get_audit_logs / clear_audit_logs /
//         check_password_strength / verify_audit_chain
//   安全别名: encrypt_database / unlock_database / set_auto_lock_timeout /
//             set_pii_detection_enabled / clear_clipboard / set_clipboard_clear_timeout /
//             check_security_status
//   i18n: get_locale / set_locale
// 覆盖事件：chat_phase / chat_token / chat_sources / chat_error / chat_done / doc-status-changed /
// audit_phase / import-progress / embedding_progress / model_download_progress /
// security-state-changed / agent_step / sync_progress / model_load_progress /
// workflow_progress / dream_progress。
//
// 增强能力（E2E_COVERAGE.md §3）：
// - 文件内容哈希去重（REQ-ING-004）
// - 配额计数器（REQ-LIC-002）
// - chat_phase 阶段事件（REQ-RAG-001 扩展，含时序修复）
// - Agentic RAG agent_step 事件（REQ-RAG-022）
// - 空上下文模式（REQ-RAG-003）
// - 消息持久化（REQ-RAG-006）
// - XSS token 注入（REQ-SEC-002）
// - 连接测试失败模拟（REQ-UI-007）
// - 拖拽事件模拟（REQ-UI-004）
// - 8 类 PII 检测与脱敏（REQ-SEC-007/008）
// - 文件夹同步进度事件（REQ-SYNC-002）
// - 本地 LLM 模型加载进度事件（REQ-LLM-003）
(() => {
  // ============================================================
  // 语音输入测试模式标志
  // voice-input.js 使用 getUserMedia + MediaRecorder + IPC 方案，
  // E2E 测试需要 mock 这三个组件。
  // ============================================================
  window.__voiceMockEnabled = true;

  // ============================================================
  // MediaRecorder Mock（REQ-RAG-034 桌面方案）
  // Playwright headless 浏览器无真实麦克风，mock getUserMedia + MediaRecorder
  // 使 E2E 测试可以验证完整录音 → 转写数据流。
  // ============================================================
  if (!navigator.mediaDevices) {
    navigator.mediaDevices = {};
  }
  navigator.mediaDevices.getUserMedia = function () {
    return Promise.resolve({
      getTracks: function () {
        return [{ stop: function () {} }];
      },
    });
  };

  window.MediaRecorder = function MockMediaRecorder(stream, options) {
    this.state = 'inactive';
    this.mimeType = (options && options.mimeType) || 'audio/webm';
    this.ondataavailable = null;
    this.onstop = null;
    this.onerror = null;
    this.start = function () {
      this.state = 'recording';
      // 同步触发 ondataavailable，确保 stop 时数据已就绪
      if (this.ondataavailable) {
        this.ondataavailable({
          data: new Blob(['mock-audio-data-mock-audio-data-mock-audio-data'], { type: this.mimeType }),
        });
      }
    };
    this.stop = function () {
      this.state = 'inactive';
      var self = this;
      setTimeout(function () {
        if (self.onstop) self.onstop();
      }, 10);
    };
  };
  window.MediaRecorder.isTypeSupported = function (type) {
    return type === 'audio/webm' || type === 'audio/ogg';
  };

  // ============================================================
  // AudioContext Mock（静音检测需要）
  // Playwright headless 无真实音频输入，mock AnalyserNode 返回静音数据
  // ============================================================
  window.AudioContext = function MockAudioContext() {
    this.state = 'running';
    this.close = function () { return Promise.resolve(); };
    this.createMediaStreamSource = function () {
      return { connect: function () {} };
    };
    this.createAnalyser = function () {
      return {
        fftSize: 512,
        smoothingTimeConstant: 0.5,
        getByteTimeDomainData: function (buffer) {
          // 返回静音数据（128 = 静默），让静音检测在测试中快速触发
          for (var i = 0; i < buffer.length; i++) {
            buffer[i] = 128;
          }
        },
      };
    };
  };
  window.webkitAudioContext = window.AudioContext;

  // ============================================================
  // Web Speech API Mock（REQ-RAG-034 / REQ-RAG-035）
  // Playwright headless 浏览器不支持真实麦克风/扬声器，
  // 此处注入 mock 以支持 E2E 测试。
  // 注意：Chromium 可能有原生 speechSynthesis，必须始终覆盖（不用 if 守卫）。
  // ============================================================
  window.SpeechRecognition = function MockSpeechRecognition() {
    this.lang = '';
    this.continuous = false;
    this.interimResults = false;
    this.maxAlternatives = 1;
    this._started = false;
    this._stopped = false;
    this.onresult = null;
    this.onerror = null;
    this.onend = null;
    this.onstart = null;
    this.start = function () {
      this._started = true;
      var self = this;
      setTimeout(function () {
        if (self.onstart) self.onstart();
        setTimeout(function () {
          if (self.onresult) {
            // results 结构：results[0] 是一个 result（array-like），result[0] 是 alternative
            var alt = { transcript: '测试语音输入', confidence: 0.9 };
            var result = [alt];
            result.isFinal = true;
            self.onresult({
              resultIndex: 0,
              results: [result],
            });
          }
          if (self.onend) self.onend();
        }, 100);
      }, 50);
    };
    this.stop = function () {
      if (this.onend) this.onend();
    };
    this.abort = function () {};
  };

  // 始终覆盖原生 speechSynthesis（Chromium 原生在 headless 下不触发 onend）
  window.speechSynthesis = {
    speak: function (u) {
      if (u && u.onend) setTimeout(function () { u.onend(); }, 100);
    },
    cancel: function () {},
    getVoices: function () {
      return [{ name: 'Test Voice', lang: 'zh-CN' }];
    },
    pending: false,
    speaking: false,
    paused: false,
    onvoiceschanged: null,
  };

  // 始终覆盖原生 SpeechSynthesisUtterance
  window.SpeechSynthesisUtterance = function MockUtterance(text) {
    this.text = text || '';
    this.lang = '';
    this.rate = 1;
    this.pitch = 1;
    this.volume = 1;
    this.onend = null;
    this.onerror = null;
    this.onstart = null;
  };

  // ============================================================
  // window.print mock（REQ-EXP-005 PDF 导出）
  // Playwright headless 浏览器无法打开真实打印对话框，
  // 此处注入 mock 以记录调用次数和捕获打印 HTML 供 E2E 断言。
  // export.js printViaIframe() 会检查 window.__printMockCalled 存在性来决定是否走 mock 路径。
  // ============================================================
  window.__printMockCalled = 0;
  window.__lastPrintHtml = null;
  window.print = function mockPrint() {
    window.__printMockCalled++;
  };

  /**
   * 延迟函数：受 window.__E2E_SPEED__ 倍率控制（默认 1.0，CI 设 0.2 加速 5 倍）。
   * helpers.mjs setupPage() 会在 stub 注入前设置 window.__E2E_SPEED__。
   */
  const SPEED = typeof window !== 'undefined' && window.__E2E_SPEED__ ? window.__E2E_SPEED__ : 1;
  const delay = (ms) => new Promise((r) => setTimeout(r, Math.round(ms * SPEED)));

  /** 简易哈希（测试用，非安全场景）：将路径映射为确定性伪哈希。 */
  function mockHash(str) {
    let h = 0;
    for (let i = 0; i < str.length; i++) {
      h = ((h << 5) - h + str.charCodeAt(i)) | 0;
    }
    return 'h' + Math.abs(h).toString(16).padStart(8, '0');
  }

  /** 从路径提取文件名。 */
  function basename(p) {
    return p.split('/').pop() || p;
  }

  /** 从文件名提取扩展名（小写）。 */
  function extname(p) {
    const name = basename(p);
    const dot = name.lastIndexOf('.');
    return dot >= 0 ? name.slice(dot + 1).toLowerCase() : '';
  }

  /** 从 file_path 提取显示名（去除 mock 哈希前缀，与 UI displayDocName 逻辑一致）。 */
  function displayName(filePath) {
    const base = basename(filePath);
    return base.length > 33 && base[32] === '-' ? base.slice(33) : base;
  }

const state = {
configured: window.__TEST_OPTS?.configured || false,
isPro: true, // 测试便利：默认 Pro 开启（v1.20.0 Alpha 结束，实际后端默认 false）
docs: [],
/** 文件路径 → 内容哈希（去重用） */
hashIndex: {},
conversations: [],
messages: {},
listeners: {},
aborted: false,
/** 工作空间列表（REQ-WS-001 多知识库 mock） */
workspaces: [
  { id: 'default', name: 'Default', created_at: Date.now() },
],
/** 当前工作空间 ID */
currentWorkspaceId: 'default',
/** 工作空间设置（持久化 mock） */
workspaceSettings: {},
/** 测试控制开关：下次 chat 是否返回空上下文 */
nextChatEmpty: false,
/** 测试控制开关：下次 chat 的自定义 token 序列 */
customTokens: null,
/** 测试控制开关：test_llm_connection 是否失败 */
connectionFail: false,
/** 文档内容模拟（用于去重判定） */
fileContents: {},
/** VLM 图片理解增强开关（REQ-MM-003） */
vlmEnabled: false,
/** 审计取消标志（REQ-AUDIT-005） */
auditAborted: false,
/** 导入取消标志（REQ-ING-006） */
importCancelled: false,
/** 测试控制开关：下次 chat 返回指定错误消息（REQ-ERR-001） */
chatError: null,
/** 测试控制开关：下次 chat 挂起（模拟后端永久阻塞，不发射任何事件，invoke 永不 resolve） */
chatHang: false,
/** 测试控制开关：下次 chat 模拟 embedder 初始化失败（发射 chat_error + chat_done） */
chatEmbedderError: false,
/** 测试控制开关：embedder 下载状态（'ready' | 'needs_download' | 'partial_download'） */
embedderStatus: window.__TEST_OPTS?.embedderStatus || 'ready',
/** 测试控制开关：init_embedder 是否模拟下载失败 */
embedderDownloadFail: window.__TEST_OPTS?.embedderDownloadFail || false,
/** 侧栏折叠状态持久化（REQ-NAV-001） */
sidebarCollapsed: false,
/** 安全态势级别（Q05 借鉴 QM SecurityPosture） */
securityPosture: 'auto',
/** Shadow 安全筛查统计（Q06 借鉴 QM security-screen.ts） */
shadowScreenStats: { total: 0, agree: 0, disagree: 0, unavailable: 0 },
/** 知识图谱关系数据（REQ-RAG-027 前端图谱可视化测试用） */
graphRelations: [
  { id: 'rel-1', subject: 'Rust', relation_type: 'defined_as', object: 'systems programming language', chunk_id: 'chunk-1', confidence: 1.0 },
  { id: 'rel-2', subject: 'Cargo', relation_type: 'part_of', object: 'Rust', chunk_id: 'chunk-2', confidence: 1.0 },
  { id: 'rel-3', subject: 'Cargo', relation_type: 'depends_on', object: 'crates.io', chunk_id: 'chunk-3', confidence: 0.7 },
  { id: 'rel-4', subject: 'tokio', relation_type: 'uses', object: 'Rust', chunk_id: 'chunk-4', confidence: 0.7 },
  { id: 'rel-5', subject: 'serde', relation_type: 'implements', object: 'Serialize', chunk_id: 'chunk-5', confidence: 1.0 },
  { id: 'rel-6', subject: 'HashMap', relation_type: 'extends', object: 'Collection', chunk_id: 'chunk-6', confidence: 0.7 },
  { id: 'rel-7', subject: 'trait', relation_type: 'references', object: 'Rust', chunk_id: 'chunk-7', confidence: 0.5 },
  { id: 'rel-8', subject: 'Rust', relation_type: 'related_to', object: 'WebAssembly', chunk_id: 'chunk-8', confidence: 0.5 },
],
    /** 模型缓存信息（REQ-VEC-008） */
    modelCacheInfo: { models: [{ name: 'all-MiniLM-L6-v2', size_bytes: 31457280 }], total_size_bytes: 31457280 },
/** 混合检索开关（REQ-RAG-010） */
hybridSearch: false,
/** 智能模式开关（S5 审计 P0-1） */
smartModeEnabled: true,
/** REQ-RAG-051: 演示模式 */
demoMode: false,
    /** 上下文 token 限制（REQ-RAG-017） */
    contextTokenLimit: 4096,
    /** LLM 推理模式（REQ-LLM-003）：'remote' / 'local' */
    llmMode: 'remote',
    /** 当前选中的本地模型文件名（REQ-LLM-003） */
    localModel: '',
    /** 已下载的本地模型列表（REQ-LLM-004） */
    localModels: [
      { filename: 'qwen2.5-3b-instruct-q4_k_m.gguf', path: '/mock/models/llm/qwen2.5-3b-instruct-q4_k_m.gguf', size_bytes: 2000000000, architecture: 'qwen2.5', param_size: '3B', quantization: 'Q4_K_M' },
    ],
    // ============================================================
    // 安全防御域状态（REQ-SEC-013~020）
    // ============================================================
    /** 安全状态：'Unencrypted' | 'EncryptedUnlocked' | 'Locked' */
    securityState: 'Unencrypted',
    /** 锁定原因 */
    lockReason: null,
    /** 自动锁屏配置 */
    autoLockConfig: { enabled: true, timeout_secs: 180, lock_on_sleep: true },
    /** 剪贴板清除配置 */
    clipboardConfig: { enabled: true, clear_after_secs: 30 },
    /** 暴力破解失败次数 */
    authFailures: 0,
    /** 剩余尝试次数 */
    remainingAttempts: 5,
    /** 是否被锁定 */
    isLocked: false,
    /** 剩余锁定秒数 */
    remainingLockSeconds: 0,
    /** 紧急销毁密码已设置 */
    panicWipeEnabled: false,
    /** 加密密码（mock 用于 unlock 验证，null 时默认 'test'） */
    encryptionPassword: null,
    /** PII 检测开关 */
    piiDetectionEnabled: false,
    /** 审计日志条目 */
    auditLogs: [],
    /** 上次活动时间戳 */
    lastActivity: Date.now(),
    // ============================================================
    // 导出功能状态（REQ-EXP-001）
    // ============================================================
    /** 最后导出的文件路径 */
    lastExportPath: null,
    /** 最后导出的内容 */
    lastExportContent: null,
    // ============================================================
    // 文件监听状态（REQ-SYNC-001~003）
    // ============================================================
    /** 已监听的文件夹列表 */
    watchedFolders: [],
    // ============================================================
    // 高级 RAG 功能状态
    // ============================================================
    /** Cross-Encoder 重排序开关（REQ-RAG-020） */
    rerankEnabled: false,
    /** HyDE 查询改写开关（REQ-RAG-021） */
    hydeEnabled: false,
    /** Agentic RAG 开关（REQ-RAG-022） */
    agentEnabled: false,
    /** 当前嵌入模型 */
    embeddingModel: 'all-MiniLM-L6-v2',
    // ============================================================
    // 国际化状态（REQ-I18N-001）
    // ============================================================
    /** 当前语言 */
    locale: 'zh-CN',
    // ============================================================
    // 文档领域分类状态（REQ-VEC-013）
    // ============================================================
    /** 文档领域分类 */
    docDomains: {},
    // ============================================================
    // DAG 工作流状态（REQ-RAG-030）
    // ============================================================
    /** 已保存的工作流模板 */
workflowTemplates: [],
// S56：自定义快捷指令模板（S88：预填测试模板）
promptTemplates: [
  { id: 'pt-test-summary', name: 'test_summary', label: 'Test Summary', description: 'Test summary template', icon: '📋', prompt_template: 'Please summarize: {query}', created_at: 1700000000, updated_at: 1700000000 },
  { id: 'pt-test-translate', name: 'test_translate', label: 'Test Translate', description: 'Test translate template', icon: '🌐', prompt_template: 'Please translate: {query}', created_at: 1700000001, updated_at: 1700000001 },
],
// B09 v1.8：Skill 文件（slash: true 的技能）
skills: [],
/** Wiki 双向链接数据（REQ-ING-020） */
wikiLinks: [],
/** Durable Prompt Admission 待处理输入（B05） */
pendingInputs: [],
sessionTodos: [],
/** Trace 记录（S2 复盘接线） */
traces: [],
/** Token 预算配置（S2 复盘接线） */
tokenBudgetMaxTokens: 32768,
tokenBudgetThreshold: 0.8,
tokenBudgetKeepRatio: 0.67,
tokenBudgetMinMsgs: 3,
/** 对话分支树数据（REQ-RAG-039） */
conversationTree: null,
// ============================================================
// 持久化记忆系统状态（REQ-RAG-033）
// ============================================================
    /** 记忆功能开关 */
    memoryEnabled: false,
    /** 记忆条目列表 */
    memories: [],
// Scratch-Promote 记忆整合（Q01）
scratchLogs: [],
// Burst Buffer 延迟批量记忆提取（Q02）
burstBuffer: [],
    // ============================================================
    // AutoDream 状态
    // ============================================================
    /** Dream 取消标志 */
    dreamAborted: false,
    /** Dream 历史建议 */
    dreamSuggestions: [],
    // ============================================================
    // 代码符号引擎状态（REQ-RAG-031）
    // ============================================================
    /** 符号索引是否已构建 */
    symbolIndexBuilt: false,
    // ============================================================
    // 性能优化状态
    // ============================================================
    /** 压缩比 */
    compressionRatio: 1.0,
    /** 缓存设置 */
    cacheSettings: { enabled: true, ttl_secs: 86400, semantic_threshold: 0.92, privacy_mode: false },
    /** 缓存统计 */
    cacheStats: { enabled: true, exact_hits: 5, semantic_hits: 3, retrieval_hits: 8, total_queries: 50, cache_size_entries: 16, estimated_token_saved: 12000 },
    /** 检索记忆开关 */
    retrievalMemoryEnabled: false,
    /** 检索记忆统计 */
    retrievalMemoryStats: [],
    /** 子代理开关 */
    subAgentEnabled: false,
    /** 网页搜索开关（REQ-RAG-036） */
    webSearchEnabled: false,
    /** RAG 检索参数（REQ-RAG-014 v1.10） */
    ragParams: { top_k: 8, score_threshold: 0.0, chunk_expansion_enabled: true, chunk_expansion_window: 1 },
    /** LLM 生成参数（REQ-RAG-015 v1.10） */
    generationParams: { temperature: 0.7, max_tokens: 4096, top_p: 1.0 },
    /** 文档排序状态（REQ-ING-008 v1.10） */
    docSortBy: null,
    docSortOrder: null,
    /** 协调模式开关 */
    coordinatorEnabled: false,
    /** 渐进式注入开关 */
    progressiveInjection: false,
contextualRetrieval: true,
lateChunking: false,
    /** Speculative RAG 开关 */
    speculativeEnabled: false,
    // ============================================================
    // 可观测性状态（REQ-OBS-001）
    // ============================================================
    /** 当前日志级别 */
    logLevel: 'info',
    // ============================================================
    // 下载管理状态（REQ-LLM-004 v2）
    // ============================================================
    /** 下载状态 Map */
    downloadStatuses: {},
    // ============================================================
    // 本地 LLM Pro 状态
    // ============================================================
    /** PagedAttention 开关 */
    pagedAttn: false,
    /** 采样参数 */
    samplingParams: { temperature: null, top_p: null, top_k: null, max_tokens: null, frequency_penalty: null, presence_penalty: null },
    /** 内核模式 */
    kernelMode: 'mistral_rs',
    /** KV cache 开关 */
    kvCacheEnabled: false,
    // ============================================================
    // Token 用量追踪
    // ============================================================
    /** Token 预算 */
    tokenBudget: 0,
    // ============================================================
    // v1.13 功能状态
    // ============================================================
    /** 开机自启开关（REQ-WIN-004） */
    autostart: false,
    /** 应用更新检查信息（REQ-HELP-004） */
    updateInfo: null,
    /** 自动检查更新开关 */
    updateAutoCheck: true,
    /** 上次检查时间戳 */
    updateLastCheck: 0,
  };

  const emit = (name, payload) => {
    (state.listeners[name] || []).forEach((cb) => cb({ payload }));
  };

  /** 默认流式 token 序列（含 Rust 代码块）。 */
  const DEFAULT_TOKENS = [
    '好的，', '这是', '流式', '回答', '：', '\n\n```rust\n', 'fn main() {\n',
    '    println!("hi");\n', '}\n', '```\n', '正在', '继续', '输出', '更多', '内容', '……',
  ];

  /** XSS 注入 token 序列（用于安全测试）。 */
  const XSS_TOKENS = [
    '<script>alert("xss")</script>',
    '<img src=x onerror=alert(1)>',
    '<a href="javascript:alert(1)">click</a>',
    '<iframe src="evil.com"></iframe>',
    '正常文字',
  ];

  /** Mermaid flowchart token 序列（用于 TC-VIZ-001/002/004）。 */
  const MERMAID_TOKENS = [
    '好的，', '这是一个流程图：', '\n\n```mermaid\n',
    'flowchart TD\n', '    A[开始] --> B[处理]\n',
    '    B --> C{判断}\n', '    C -->|是| D[结束]\n',
    '    C -->|否| B\n', '```\n',
  ];

  /** Mermaid 语法错误 token 序列（用于 TC-VIZ-003）。 */
  const MERMAID_INVALID_TOKENS = [
    '好的，', '这是一个有语法错误的图表：', '\n\n```mermaid\n',
    'flowchart TD\n', '    A --> --> B\n',  // 双箭头语法错误
    '    B[未闭合节点\n',  // 未闭合方括号
    '```\n',
  ];

  /** Mermaid XSS 注入 token 序列（用于 TC-VIZ-006 安全测试）。 */
  const MERMAID_XSS_TOKENS = [
    '好的，', '这是一个含 XSS 载荷的图表：', '\n\n```mermaid\n',
    'flowchart TD\n',
    '    A["<script>alert(1)</script>"] --> B[正常]\n',
    '```\n',
  ];

  /** KaTeX 行内公式 token 序列（用于 TC-VIZ-007）。 */
  const KATEX_INLINE_TOKENS = [
    '好的，', '质能方程为 $E = mc^2$，', '其中 $c$ 是光速，', '水的化学式 $\\ce{H2O}$。',
  ];

  /** KaTeX 块级公式 token 序列（用于 TC-VIZ-008）。 */
  const KATEX_BLOCK_TOKENS = [
    '好的，', '积分公式如下：', '\n\n$$\\int_0^1 f(x)\\,dx$$\n\n', '这就是定积分的定义。',
  ];

  /** KaTeX 语法错误 token 序列（用于 TC-VIZ-009）。 */
  const KATEX_INVALID_TOKENS = [
    '好的，', '这个公式有语法错误：', '$\\undefinedcmd$', '。正常文字。',
  ];

  /** Chart.js 表格数据 token 序列（用于 TC-VIZ-007 数据图表）。 */
  const CHART_TABLE_TOKENS = [
    '好的，', '以下是季度销售数据：\n\n',
    '| 季度 | 产品A | 产品B |\n',
    '|------|-------|-------|\n',
    '| Q1 | 120 | 80 |\n',
    '| Q2 | 150 | 95 |\n',
    '| Q3 | 180 | 110 |\n',
    '| Q4 | 200 | 130 |\n',
  ];

  /** KaTeX XSS 注入 token 序列（用于 TC-VIZ-010 安全测试）。 */
  const KATEX_XSS_TOKENS = [
    '好的，', '公式含 XSS 载荷：', '$<script>alert(1)</script>$', '正常文字。',
  ];

  /** Mermaid 甘特图 token 序列（用于 E2E-VIZ-ADV-001）。 */
  const MERMAID_GANTT_TOKENS = [
    '好的，', '这是甘特图：', '\n\n```mermaid\n',
    'gantt\n',
    '    title 项目进度\n',
    '    section 设计阶段\n',
    '    需求分析 :a1, 2024-01-01, 7d\n',
    '    原型设计 :a2, after a1, 5d\n',
    '    section 开发阶段\n',
    '    前端开发 :a3, after a2, 14d\n',
    '    后端开发 :a4, after a2, 14d\n',
    '    section 测试阶段\n',
    '    集成测试 :a5, after a3, 7d\n',
    '```\n',
  ];

  /** Mermaid 序列图 token 序列（用于 E2E-VIZ-ADV-002）。 */
  const MERMAID_SEQUENCE_TOKENS = [
    '好的，', '这是序列图：', '\n\n```mermaid\n',
    'sequenceDiagram\n',
    '    participant U as 用户\n',
    '    participant A as 应用\n',
    '    participant D as 数据库\n',
    '    U->>A: 发送请求\n',
    '    A->>D: 查询数据\n',
    '    D-->>A: 返回结果\n',
    '    A-->>U: 响应数据\n',
    '```\n',
  ];

  /** Mermaid 类图 token 序列（用于 E2E-VIZ-ADV-003）。 */
  const MERMAID_CLASS_TOKENS = [
    '好的，', '这是类图：', '\n\n```mermaid\n',
    'classDiagram\n',
    '    class Animal {\n',
    '    +String name\n',
    '    +int age\n',
    '    +makeSound() void\n',
    '    }\n',
    '    class Dog {\n',
    '    +fetch() void\n',
    '    }\n',
    '    class Cat {\n',
    '    +purr() void\n',
    '    }\n',
    '    Animal <|-- Dog\n',
    '    Animal <|-- Cat\n',
    '```\n',
  ];

  /** Mermaid 饼图 token 序列（用于 E2E-VIZ-ADV-004）。 */
  const MERMAID_PIE_TOKENS = [
    '好的，', '这是饼图：', '\n\n```mermaid\n',
    'pie title 浏览器市场份额\n',
    '    "Chrome" : 65\n',
    '    "Safari" : 18\n',
    '    "Firefox" : 5\n',
    '    "Edge" : 4\n',
    '    "其他" : 8\n',
    '```\n',
  ];

  /** KaTeX 化学方程式 token 序列（用于 E2E-VIZ-ADV-007）。 */
  const KATEX_CHEM_TOKENS = [
    '好的，', '水的化学方程式：$\\ce{H2O}$，', '燃烧反应：$\\ce{2H2 + O2 -> 2H2O}$。',
  ];

  /** 简单表格 token 序列（用于 E2E-VIZ-ADV-009 柱状图测试）。 */
  const TABLE_TOKENS = [
    '好的，', '数据对比如下：\n\n',
    '| 类别 | 值A | 值B |\n',
    '|------|-----|-----|\n',
    '| 一月 | 100 | 80 |\n',
    '| 二月 | 120 | 90 |\n',
    '| 三月 | 110 | 95 |\n',
  ];

  /** 折线图数据 token 序列（用于 E2E-VIZ-ADV-010）。 */
  const LINE_CHART_TOKENS = [
    '好的，', '趋势数据如下：\n\n',
    '| 月份 | 访问量 | 转化率 |\n',
    '|------|--------|--------|\n',
    '| 1月 | 1000 | 15 |\n',
    '| 2月 | 1200 | 18 |\n',
    '| 3月 | 1500 | 22 |\n',
    '| 4月 | 1800 | 25 |\n',
    '| 5月 | 2000 | 28 |\n',
    '| 6月 | 2200 | 30 |\n',
  ];

  /** 饼图数据 token 序列（用于 E2E-VIZ-ADV-011）。 */
  const PIE_CHART_TOKENS = [
    '好的，', '占比数据如下：\n\n',
    '| 类别 | 占比 |\n',
    '|------|------|\n',
    '| 桌面端 | 55 |\n',
    '| 移动端 | 35 |\n',
    '| 平板 | 10 |\n',
  ];

  /** 审计报告 token 序列（用于 E2E-AUDIT-001~005，模拟 Markdown 勘误报告流式输出）。 */
  const AUDIT_TOKENS = [
    '# 文档一致性审计报告\n\n',
    '## 审计摘要\n\n',
    '本次审计共扫描 **12** 个片段，提取 **8** 条声明，发现 **2** 处矛盾。\n\n',
    '## 矛盾清单\n\n',
    '| # | 声明 A | 声明 B | 类型 |\n',
    '|---|--------|--------|------|\n',
    '| 1 | 第一章说「支持 PDF」 | 第三章说「不支持 PDF」 | contradiction |\n',
    '| 2 | 第二章说「上限 50 文件」 | 第四章说「上限 100 文件」 | contradiction |\n\n',
    '## 免责声明\n\n',
    '> 本报告由 AI 自动生成，可能存在误差，请以原文为准。',
  ];

  async function invoke(cmd, args = {}) {
    switch (cmd) {
      case 'get_settings':
        return state.configured
          ? { has_llm_config: true, base_url: 'http://mock.local', model: 'mock-llm', api_key_masked: '****-e2e', vlm_enabled: state.vlmEnabled, context_token_limit: state.contextTokenLimit, llm_mode: state.llmMode, local_model: state.localModel, quality_gate_enabled: state.qualityGateEnabled || false, sub_agent_enabled: state.subAgentEnabled || false, progressive_injection: state.progressiveInjection || false, speculative_enabled: state.speculativeEnabled || false, graph_retriever_enabled: state.graphRetrieverEnabled || false, web_search_enabled: state.webSearchEnabled || false, contextual_retrieval: state.contextualRetrieval ?? true }
          : { has_llm_config: false, base_url: '', model: '', api_key_masked: '', vlm_enabled: state.vlmEnabled, context_token_limit: state.contextTokenLimit, llm_mode: state.llmMode, local_model: state.localModel, quality_gate_enabled: state.qualityGateEnabled || false, sub_agent_enabled: state.subAgentEnabled || false, progressive_injection: state.progressiveInjection || false, speculative_enabled: state.speculativeEnabled || false, graph_retriever_enabled: state.graphRetrieverEnabled || false, web_search_enabled: state.webSearchEnabled || false, contextual_retrieval: state.contextualRetrieval ?? true };

      case 'update_llm_config':
        state.configured = true;
        return null;

      case 'test_llm_connection':
        if (state.connectionFail) {
          state.connectionFail = false; // 一次性触发
          throw 'LLM API 错误 (HTTP 401): {"error":{"message":"Invalid API Key"}}';
        }
        return '连接成功：mock-llm 响应正常 (HTTP 200)';

      case 'import_files': {
        state.importCancelled = false;
        const names = [];
        const FREE_LIMIT = 50;
        const WHITELIST = ['md', 'txt', 'pdf', 'docx', 'html', 'htm', 'pptx', 'epub', 'xlsx', 'csv'];
        const total = args.paths.length;
        let completed = 0;
        for (const p of args.paths) {
          // 导入取消检查（REQ-ING-006）
          if (state.importCancelled) {
            emit('import-progress', { completed, total, current_file: basename(p), cancelled: true });
            return names;
          }
          const name = basename(p);
          const ext = extname(p);

          // 格式白名单校验
          if (!WHITELIST.includes(ext)) {
            throw `不支持的格式：.${ext}，当前支持 .md / .txt / .pdf / .docx / .html / .pptx / .epub / .xlsx / .csv`;
          }

// Pro 门控格式付费门（REQ-LIC-002）
// Alpha 阶段：全功能免费，跳过 Pro 门控
const PRO_GATED = ['pdf', 'docx', 'pptx', 'epub', 'xlsx', 'csv'];
if (!state.isPro && PRO_GATED.includes(ext)) {
throw `PRO_REQUIRED: .${ext} 导入为 Pro 版功能，请升级后重试`;
}

          // 配额检查（REQ-WS-002：按工作空间独立计数）
          const wsDocs = state.docs.filter((d) => (d.workspace_id || 'default') === state.currentWorkspaceId);
          if (!state.isPro && wsDocs.length >= FREE_LIMIT) {
            throw 'LIMIT_REACHED: 免费版每个知识库上限 50 个文件，请升级 Pro 版';
          }

          // 内容去重（基于路径确定性哈希模拟）
          const hash = mockHash(p + (state.fileContents[p] || ''));
          if (state.hashIndex[hash]) {
            emit('doc-status-changed', { status: 'done', message: `内容已存在，跳过导入：${name}` });
            completed++;
            emit('import-progress', { completed, total, current_file: name, cancelled: false });
            // 文件间延迟，确保进度条可见（REQ-ING-006 测试需要）
            await delay(50);
            continue;
          }
          state.hashIndex[hash] = p;

          const doc = {
            id: 'doc-' + hash,
            file_path: '/mock/data/documents/' + 'a'.repeat(32) + '-' + name,
            file_hash: hash,
            status: 'Pending',
            created_at: Math.floor(Date.now() / 1000),
            workspace_id: state.currentWorkspaceId,
          };
          state.docs.push(doc);
          emit('doc-status-changed', { status: 'indexing', message: `正在索引：${name}` });
          // 文件处理延迟，确保进度条可见（REQ-ING-006 测试需要）
          await delay(80);
          doc.status = 'Indexed';
          emit('doc-status-changed', { status: 'done', message: `索引完成：${name}（3 向量）` });
          names.push(name);
          completed++;
          // 导入进度事件（REQ-ING-006）
          emit('import-progress', { completed, total, current_file: name, cancelled: false });
          // 文件间延迟，确保进度条可见（REQ-ING-006 测试需要）
          await delay(50);
        }
        return names;
      }

      case 'get_file_sizes': {
// REQ-ING-013 mock: 文件名含 'large' 返回 150MB，含 'huge' 返回 600MB，其他返回 1MB
return args.paths.map((p) => {
  const name = basename(p).toLowerCase();
  let size = 1024 * 1024; // 1MB default
  if (name.includes('huge')) size = 600 * 1024 * 1024;
  else if (name.includes('large')) size = 150 * 1024 * 1024;
  return [p, size];
});
}

case 'get_file_size_limits':
return [100 * 1024 * 1024, 500 * 1024 * 1024];

case 'get_documents': {
// REQ-ING-008 文档排序支持（v1.10）
let docs = [...state.docs];
const sortBy = args.sortBy || null;
const sortOrder = args.sortOrder || null;
if (sortBy === 'file_name') {
docs.sort((a, b) => {
const na = (a.file_path || '').split('/').pop() || '';
const nb = (b.file_path || '').split('/').pop() || '';
return sortOrder === 'asc' ? na.localeCompare(nb) : nb.localeCompare(na);
});
} else if (sortBy === 'file_size') {
docs.sort((a, b) => sortOrder === 'asc' ? (a.file_size || 0) - (b.file_size || 0) : (b.file_size || 0) - (a.file_size || 0));
} else {
// imported_at (default)
docs.sort((a, b) => sortOrder === 'asc' ? (a.created_at || 0) - (b.created_at || 0) : (b.created_at || 0) - (a.created_at || 0));
}
 return docs;
      }

      // 磁盘空间管理（P1-1 三件套：有后端注册，此前无前端/mock）
      case 'get_disk_space_info': {
        const total = 500 * 1024 * 1024 * 1024; // 500GB
        const free = 100 * 1024 * 1024 * 1024; // 100GB
        return JSON.stringify({
          free_bytes: free,
          total_bytes: total,
          used_bytes: total - free,
          free_percent: (free / total) * 100,
          is_low: false,
          threshold_bytes: 1024 * 1024 * 1024,
        });
      }

      case 'cleanup_disk_space':
        return 0;

      case 'check_disk_space': {
        const required = args.requiredBytes ?? args.required_bytes ?? 0;
        if (required > 50 * 1024 * 1024 * 1024) {
          throw '磁盘空间不足：可用 100GB，需要 ' + Math.round(required / 1024 / 1024 / 1024) + 'GB';
        }
        return null;
      }

      case 'get_document_chunks': {
        // REQ-EXP-005: 返回文档分块（用于 PDF 导出）
        const doc = state.docs.find((d) => d.id === args.docId);
        if (!doc) return [];
        // 返回模拟分块（基于文档名生成内容）
        return [
          { id: 'chunk-0', content: '# ' + (doc.file_path || 'Document') + '\n\nSample content for PDF export.', sequence: 0 },
          { id: 'chunk-1', content: 'Second chunk with more details.', sequence: 1 },
        ];
      }

case 'delete_document':
state.docs = state.docs.filter((d) => d.id !== args.id);
return null;

case 'batch_delete_documents':
state.docs = state.docs.filter((d) => !args.ids.includes(d.id));
return { success_count: args.ids.length, failed_count: 0, failed_ids: [] };

case 'batch_move_documents':
state.docs = state.docs.map((d) =>
  args.ids.includes(d.id) ? { ...d, workspace_id: args.targetWorkspaceId } : d
);
return { success_count: args.ids.length, failed_count: 0, failed_ids: [] };

case 'batch_add_tags':
state.docs = state.docs.map((d) => {
  if (args.ids.includes(d.id)) {
    const existing = d.tags || [];
    const newTags = [...existing];
    for (const tag of args.tags) {
      if (!newTags.includes(tag)) newTags.push(tag);
    }
    return { ...d, tags: newTags };
  }
  return d;
});
return { success_count: args.ids.length, failed_count: 0, failed_ids: [] };


      case 'get_pro_status':
        return state.isPro;

      case 'activate_pro':
        if (args.licenseKey && args.licenseKey.trim()) {
          state.isPro = true;
          return true;
        }
        throw 'License 激活失败：License 格式错误：应为 payload-signature';

      case 'create_conversation': {
        const id = 'conv-' + state.conversations.length;
        state.conversations.unshift({
          id,
          workspace_id: args.workspaceId || 'default',
          title: '新会话',
          created_at: Math.floor(Date.now() / 1000),
          sort_order: 0,
        });
        state.messages[id] = [];
        return id;
      }

      case 'get_conversations':
        return state.conversations.filter(
          (c) => (c.workspace_id || 'default') === state.currentWorkspaceId
        );

      case 'get_messages':
        return state.messages[args.conversationId] || [];

      case 'get_messages_paginated': {
        const all = state.messages[args.conversationId] || [];
        const limit = args.limit || 30;
        const offset = args.offset || 0;
        // offset from end: offset=0 returns most recent `limit` messages
        const start = Math.max(0, all.length - offset - limit);
        const end = Math.max(0, all.length - offset);
        return { items: all.slice(start, end), total: all.length };
      }

      case 'delete_conversation':
        state.conversations = state.conversations.filter((c) => c.id !== args.id);
        delete state.messages[args.id];
        return null;

      case 'rename_conversation': {
        const conv = state.conversations.find((c) => c.id === args.id);
        if (conv) conv.title = args.title;
        return null;
      }

      case 'reorder_conversations': {
        // REQ-IX-002: 按传入的有序 ID 列表更新 sort_order
        const ids = args.orderedIds || [];
        for (let i = 0; i < ids.length; i++) {
          const conv = state.conversations.find((c) => c.id === ids[i]);
          if (conv) conv.sort_order = i + 1;
        }
        // 重新按 sort_order ASC, created_at DESC 排序
        state.conversations.sort((a, b) => {
          const sa = a.sort_order || 0;
          const sb = b.sort_order || 0;
          if (sa !== sb) return sa - sb;
          return (b.created_at || 0) - (a.created_at || 0);
        });
        return null;
      }

case 'add_bookmark': {
// REQ-RAG-047 + REQ-RAG-053: 添加对话书签（支持消息级）
if (!state._bookmarks) state._bookmarks = [];
const existing = state._bookmarks.findIndex((b) => b.conversation_id === args.conversationId);
const bm = {
conversation_id: args.conversationId,
note: args.note || null,
created_at: Date.now(),
message_id: args.messageId || null,
summary: args.summary || null,
};
        if (existing >= 0) {
          state._bookmarks[existing] = bm;
        } else {
          state._bookmarks.push(bm);
        }
        return null;
      }

      case 'remove_bookmark': {
        if (!state._bookmarks) state._bookmarks = [];
        state._bookmarks = state._bookmarks.filter((b) => b.conversation_id !== args.conversationId);
        return null;
      }

      case 'list_bookmarks': {
        if (!state._bookmarks) state._bookmarks = [];
        return state._bookmarks.slice().sort((a, b) => (b.created_at || 0) - (a.created_at || 0));
      }

case 'is_bookmarked': {
if (!state._bookmarks) state._bookmarks = [];
return state._bookmarks.some((b) => b.conversation_id === args.conversationId);
}

case 'get_message_bookmark': {
// REQ-RAG-053: 按消息 ID 查询书签
if (!state._bookmarks) state._bookmarks = [];
return state._bookmarks.find((b) => b.message_id === args.messageId) || null;
}

case 'edit_user_message': {
        // Mock: 返回顺序递增的版本号（v1=原始, v2=首次编辑, v3=第二次编辑, ...）
        const tg = args.turn_group || 'default';
        if (!state._editVersionCounter) state._editVersionCounter = {};
        state._editVersionCounter[tg] = (state._editVersionCounter[tg] || 1) + 1;
        return state._editVersionCounter[tg];
      }

      case 'set_turn_active_version':
        // Mock: 空操作
        return null;

      case 'get_turn_active_versions':
        // Mock: 返回空列表
        return [];

      case 'abort_chat':
        state.aborted = true;
        return null;

      case 'abort_import':
        state.importCancelled = true;
        return null;

      case 'replace_document': {
        // REQ-ING-012：替换文档 mock
        const oldDocIdx = state.docs.findIndex((d) => d.id === args.oldDocId);
        if (oldDocIdx >= 0) state.docs.splice(oldDocIdx, 1);
        const newId = 'replaced-' + Date.now();
        state.docs.push({
          id: newId,
          file_path: args.filePath,
          file_hash: 'replaced-hash-' + Date.now(),
          status: 'Indexed',
          created_at: Math.floor(Date.now() / 1000),
          tags: [],
        });
        emit('doc-status-changed', { status: 'done', message: `替换完成：${basename(args.filePath)}` });
        return newId;
      }

      case 'retry_index': {
        const doc = state.docs.find((d) => d.id === args.id);
        if (!doc) throw '文档不存在';
        doc.status = 'Processing';
        emit('doc-status-changed', { status: 'indexing', message: `正在重试索引：${basename(doc.file_path)}` });
        (async () => {
          await delay(300);
          doc.status = 'Indexed';
          emit('doc-status-changed', { status: 'done', message: `索引完成：${basename(doc.file_path)}（3 向量）` });
        })();
        return null;
      }

      // 索引重建（REQ-VEC-009）
      case 'rebuild_index': {
        const doc = state.docs.find((d) => d.id === args.id);
        if (!doc) throw '文档不存在';
        doc.status = 'Processing';
        emit('doc-status-changed', { status: 'indexing', message: `正在重建索引：${basename(doc.file_path)}` });
        (async () => {
          await delay(400);
          doc.status = 'Indexed';
          emit('doc-status-changed', { status: 'done', message: `重建完成：${basename(doc.file_path)}` });
        })();
        return null;
      }

      // 文档原文导出（REQ-EXP-004）
      case 'export_document_original': {
        const doc = state.docs.find((d) => d.id === args.docId);
        if (!doc) throw '文档不存在';
        // mock：模拟成功导出
        return null;
      }

      case 'deactivate_pro':
        state.isPro = false;
        return null;


      // ============================================================
      // 本地 LLM 推理（REQ-LLM-003/004）
      // ============================================================
      case 'list_local_models':
        return state.localModels;

      case 'get_recommended_models':
        return [
          { name: 'Qwen2.5-3B-Instruct', architecture: 'qwen2.5', param_size: '3B', quantization: 'Q4_K_M', size_gb: 2.0, url: 'https://huggingface.co/Qwen/Qwen2.5-3B-Instruct-GGUF/resolve/main/qwen2.5-3b-instruct-q4_k_m.gguf', description: '低配设备推荐，中文能力优秀' },
          { name: 'Llama-3.2-3B-Instruct', architecture: 'llama3.2', param_size: '3B', quantization: 'Q4_K_M', size_gb: 2.0, url: 'https://huggingface.co/meta-llama/Llama-3.2-3B-Instruct-GGUF/resolve/main/llama-3.2-3b-instruct-q4_k_m.gguf', description: '英文场景推荐' },
          { name: 'Phi-3.5-mini-instruct', architecture: 'phi3.5', param_size: '3.8B', quantization: 'Q4_K_M', size_gb: 2.2, url: 'https://huggingface.co/microsoft/Phi-3.5-mini-instruct-gguf/resolve/main/Phi-3.5-mini-instruct-q4_k_m.gguf', description: '推理能力强' },
          { name: 'Qwen2.5-7B-Instruct', architecture: 'qwen2.5', param_size: '7B', quantization: 'Q4_K_M', size_gb: 4.1, url: 'https://huggingface.co/Qwen/Qwen2.5-7B-Instruct-GGUF/resolve/main/qwen2.5-7b-instruct-q4_k_m.gguf', description: '平衡质量与速度' },
        ];

      case 'download_model': {
        const dlFilename = args.filename;
        const dlTotal = 2000000000; // 2GB
        (async () => {
          // 模拟下载进度事件（REQ-LLM-004 AC-2）
          const steps = [10, 25, 50, 75, 100];
          for (const pct of steps) {
            const downloaded = Math.floor(dlTotal * pct / 100);
            const speed = 5000000; // 5MB/s
            emit('model_download_progress', { filename: dlFilename, downloaded, total: dlTotal, speed });
            await delay(100);
          }
          // 下载完成：将模型添加到列表
          const arch = dlFilename.includes('qwen') ? 'qwen2.5' : dlFilename.includes('llama') ? 'llama3.2' : dlFilename.includes('phi') ? 'phi3.5' : 'unknown';
          const param = dlFilename.match(/(\d+\.?\d*)b/i) ? dlFilename.match(/(\d+\.?\d*)b/i)[1] + 'B' : '?';
          const quant = dlFilename.match(/q(\d[_\d]*\w*)/i) ? ('Q' + dlFilename.match(/q(\d[_\d]*\w*)/i)[1].toUpperCase()) : 'Q4_K_M';
          state.localModels.push({ filename: dlFilename, path: '/mock/models/llm/' + dlFilename, size_bytes: dlTotal, architecture: arch, param_size: param, quantization: quant });
        })();
        return null;
      }

      case 'delete_model': {
        state.localModels = state.localModels.filter((m) => m.filename !== args.filename);
        if (state.localModel === args.filename) {
          state.localModel = '';
        }
        return null;
      }

      case 'set_llm_mode':
        if (args.mode !== 'remote' && args.mode !== 'local') {
          throw '无效的 LLM 模式: ' + args.mode + '（可选: remote / local）';
        }
        state.llmMode = args.mode;
        return null;

      case 'get_llm_mode':
        return state.llmMode;

      case 'set_local_model':
        state.localModel = args.filename;
        return null;

      case 'audit_document': {
        state.auditAborted = false;
        const auditTokens = state.customTokens || AUDIT_TOKENS;
        state.customTokens = null;

        (async () => {
          // 审计三阶段事件（REQ-AUDIT-005）
          emit('audit_phase', { phase: 'extracting', message: '正在提取声明…' });
          await delay(150);
          emit('audit_phase', { phase: 'comparing', message: '正在比对矛盾…' });
          await delay(150);
          emit('audit_phase', { phase: 'reporting', message: '正在生成报告…' });
          await delay(100);

          // 流式输出审计报告（复用 chat_token 事件链路）
          let content = '';
          for (const t of auditTokens) {
            if (state.auditAborted) {
              emit('chat_error', '⏹ 审计已中断');
              emit('chat_done', null);
              return;
            }
            content += t;
            emit('chat_token', t);
            await delay(300);
          }
          emit('chat_done', null);
        })();
        return null;
      }

      case 'abort_audit':
        state.auditAborted = true;
        return null;

      case 'init_embedder': {
        // 测试控制：模拟下载失败
        if (state.embedderDownloadFail) {
          state.embedderDownloadFail = false;
          (async () => {
            await delay(100);
            emit('model_download_progress', { error: { message: '下载失败：网络连接超时' } });
          })();
          return null;
        }
        // 测试控制：模拟下载挂起（无进度事件）
        if (state.embedderDownloadHang) {
          state.embedderDownloadHang = false;
          // 不发送任何事件，模拟后端阻塞
          return null;
        }
        // 测试控制：模拟慢速首事件（延迟 2s 才发送第一个进度事件）
        const initialDelay = state.embedderSlowConnect ? 2000 : 0;
        if (state.embedderSlowConnect) state.embedderSlowConnect = false;
        // 测试控制：模拟多文件下载
        const multiFile = state.embedderMultiFile;
        if (state.embedderMultiFile) state.embedderMultiFile = false;
        (async () => {
          await delay(initialDelay);
          if (multiFile) {
            // 模拟 3 个文件下载
            const files = ['config.json', 'tokenizer.json', 'model_quantized.onnx'];
            const sizes = [1024, 51200, 31457280];
            for (let fi = 0; fi < files.length; fi++) {
              emit('model_download_progress', { downloading: { current: 0, total: sizes[fi], file_name: files[fi], file_index: fi, total_files: 3 } });
              await delay(50);
              emit('model_download_progress', { downloading: { current: Math.floor(sizes[fi] * 0.3), total: sizes[fi], file_name: files[fi], file_index: fi, total_files: 3 } });
              await delay(50);
              emit('model_download_progress', { downloading: { current: Math.floor(sizes[fi] * 0.7), total: sizes[fi], file_name: files[fi], file_index: fi, total_files: 3 } });
              await delay(50);
              emit('model_download_progress', { downloading: { current: sizes[fi], total: sizes[fi], file_name: files[fi], file_index: fi, total_files: 3 } });
              await delay(50);
            }
          } else {
            // 模拟单文件下载进度事件（REQ-VEC-008）
            emit('model_download_progress', { downloading: { current: 5242880, total: 31457280, file_name: 'model_quantized.onnx', file_index: 0, total_files: 1 } });
            await delay(100);
            emit('model_download_progress', { downloading: { current: 15728640, total: 31457280, file_name: 'model_quantized.onnx', file_index: 0, total_files: 1 } });
            await delay(100);
            emit('model_download_progress', { downloading: { current: 31457280, total: 31457280, file_name: 'model_quantized.onnx', file_index: 0, total_files: 1 } });
            await delay(100);
          }
          emit('model_download_progress', { loading: true });
          await delay(100);
          emit('model_download_progress', { done: true });
          // 下载完成后更新状态
          state.embedderStatus = 'ready';
        })();
        return null;
      }

      case 'check_embedder_status':
        return state.embedderStatus;

      case 'get_model_cache_info':
        return state.modelCacheInfo;

      case 'clear_model_cache': {
        const freed = state.modelCacheInfo.total_size_bytes;
        state.modelCacheInfo = { models: [], total_size_bytes: 0 };
        return freed;
      }

      // REQ-VEC-014: 自定义 ONNX 嵌入模型上传（Pro 门控）
      case 'upload_custom_embedding_model': {
        if (!state.isPro) throw 'PRO_REQUIRED: 自定义嵌入模型上传是 Pro 版功能';
        const name = args.name.replace(/[\/\\..~]/g, '_');
        if (!state.customModels) state.customModels = [];
        const existing = state.customModels.findIndex((m) => m.name === name);
        const info = { name, dim: 0, size_bytes: 1024 * 100, is_valid: true };
        if (existing >= 0) {
          state.customModels[existing] = info;
        } else {
          state.customModels.push(info);
        }
        return info;
      }

      case 'list_custom_models': {
        if (!state.isPro) throw 'PRO_REQUIRED: 自定义嵌入模型管理是 Pro 版功能';
        return state.customModels || [];
      }

      case 'delete_custom_model': {
        if (!state.isPro) throw 'PRO_REQUIRED: 自定义嵌入模型管理是 Pro 版功能';
        if (!state.customModels) throw 'VALIDATION: 自定义模型不存在';
        const idx = state.customModels.findIndex((m) => m.name === args.name);
        if (idx < 0) throw 'VALIDATION: 自定义模型 \'' + args.name + '\' 不存在';
        state.customModels.splice(idx, 1);
        return null;
      }

      case 'chat': {
        state.aborted = false;
        // V1 修复测试：模拟后端永久挂起（不发射任何事件，invoke 永不 resolve）
        if (state.chatHang) {
          state.chatHang = false;
          // 不发射任何事件，也不 resolve invoke promise — 模拟后端永久阻塞
          return new Promise(() => {});
        }
        // V1 修复测试：模拟 embedder 初始化失败
        if (state.chatEmbedderError) {
          state.chatEmbedderError = false;
          const embedErr = 'EMBED: 向量化引擎初始化超时（180 秒），请检查网络连接后重试。如在境内，可在设置中手动初始化向量化引擎（镜像源自动回退）';
          (async () => {
            await delay(50);
            emit('chat_phase', { phase: 'preparing', message: '初始化向量化引擎…' });
            // 修复后：不再 emit chat_error，只通过 invoke reject 传递错误
            // 前端 send() catch 负责 toastError + finalizeStream + setInputState
          })();
          // 模拟修复后的后端：invoke promise reject（chat 命令返回 Err，不 emit chat_error）
          throw embedErr;
        }
        // REQ-ERR-001：测试注入的错误消息
        if (state.chatError) {
          const errMsg = state.chatError;
          state.chatError = null;
          // 修复后：后端 chat 命令不再 emit chat_error + return Err，只 return Err
          // 前端 send() catch 负责 toastError + finalizeStream + setInputState
          // 模拟修复后的后端：invoke promise reject
          throw errMsg;
        }
        const tokens = state.customTokens || DEFAULT_TOKENS;
        state.customTokens = null;
        const isEmpty = state.nextChatEmpty;
        state.nextChatEmpty = false;
const convId = args.conversationId;
const userQuery = args.query;

// 空知识库检查（模拟后端 count_documents == 0 → VALIDATION 错误拦截）
if (state.docs.length === 0) {
  // 修复后：后端返回 Err("VALIDATION: 知识库为空...")，不 emit chat_error
  throw 'VALIDATION: 知识库为空，请先通过左下角 + 号导入文档';
}

// 持久化用户消息（REQ-RAG-006）
// 兜底机制：前端 newChat() 使用 crypto.randomUUID() 生成会话 ID，不调用 create_conversation。
// 后端 chat 命令在会话不存在时幂等创建。mock 需模拟相同行为。
if (convId && !state.messages[convId]) {
  // 会话不存在 → 幂等创建
  state.conversations.unshift({
    id: convId,
    workspace_id: 'default',
    title: userQuery.slice(0, 24),
    created_at: Math.floor(Date.now() / 1000),
  });
  state.messages[convId] = [];
}
if (convId && state.messages[convId]) {
state.messages[convId].push({ role: 'user', content: userQuery, sources: null });
// 首轮问答后自动提取标题
const conv = state.conversations.find((c) => c.id === convId);
if (conv && state.messages[convId].length === 1) {
conv.title = userQuery.slice(0, 24);
}
}

        let assistantContent = '';

        (async () => {
          // 时序修复：让出微任务队列，确保 invoke 返回后 DOM 已完成 appendBlock 渲染。
          // 此前 chat_phase 事件在 invoke 调用内同步发射，导致思考指示器尚未挂载就被覆盖。
          await delay(0);

          // 本地 LLM 模式：模拟模型加载进度事件（REQ-LLM-003 AC-4）
          if (state.llmMode === 'local') {
            const modelName = state.localModel || 'mock-model.gguf';
            emit('model_load_progress', { model: modelName, status: 'loading' });
            await delay(200);
            emit('model_load_progress', { model: modelName, status: 'ready' });
            await delay(50);
          }

          if (isEmpty) {
            // 空上下文模式：不调用 LLM，直接返回固定提示
            emit('chat_phase', { phase: 'generating', message: '正在生成回答…' });
            await delay(50);
            assistantContent = '知识库中未找到相关内容。请尝试换个问题，或先导入相关文档。';
            emit('chat_token', assistantContent);
            // 给 requestAnimationFrame 时间渲染
            await delay(50);
            // 持久化助手消息
            if (convId && state.messages[convId]) {
              state.messages[convId].push({ role: 'assistant', content: assistantContent, sources: null });
            }
            emit('chat_done', null);
            return;
          }

          // Agentic RAG 模式（REQ-RAG-022）：先推送 agent_step 事件，再进入标准流式输出
          if (state.agentEnabled) {
            emit('chat_phase', { phase: 'retrieving', message: 'Agent 多步推理…' });

            // 模拟 ReAct 循环：Thought → Action → Observation（2 轮）
            const agentSteps = [
              { step_type: 'thought', content: '用户询问关于知识库内容的问题，需要先检索相关文档。', tool: null, input: null, iteration: 1 },
              { step_type: 'action', content: '检索知识库', tool: 'vector_search', input: userQuery, iteration: 1 },
              { step_type: 'observation', content: '找到 3 条相关片段，相似度 > 0.8。', tool: null, input: null, iteration: 1 },
              { step_type: 'thought', content: '已获取相关上下文，可以生成最终答案。', tool: null, input: null, iteration: 2 },
            ];
            for (const step of agentSteps) {
              if (state.aborted) {
                emit('chat_error', '⏹ 生成已中断');
                emit('chat_done', null);
                return;
              }
              emit('agent_step', step);
              await delay(100);
            }
          } else {
            // 标准 RAG 模式：三阶段 chat_phase 推送（REQ-RAG-001 扩展 / REQ-NFR-006-AC-2）
            // 时序修复：增加各阶段间隔（100ms / 150ms），确保 UI 有足够时间渲染思考指示器文案
            emit('chat_phase', { phase: 'preparing', message: '初始化向量化引擎…' });
            await delay(100);
            emit('chat_phase', { phase: 'retrieving', message: '检索知识库…' });
            await delay(150);
          }

          // 动态生成 sources：基于导入文档 + 简单关键词匹配
          const indexedDocs = state.docs.filter((d) => d.status === 'Indexed');
          const queryTokens = (userQuery || '').toLowerCase().match(/[a-z0-9]+/g) || [];
          const matchedDocs = indexedDocs.filter((d) => {
            const name = displayName(d.file_path).toLowerCase();
            const stem = name.replace(/\.(md|txt|pdf)$/, '');
            const docTokens = stem.match(/[a-z0-9]+/g) || [];
            return docTokens.some((t) => queryTokens.includes(t));
          });
          const sourceDocs = matchedDocs.length > 0 ? matchedDocs.slice(0, 3) : indexedDocs.slice(0, 3);
          const sources = sourceDocs.map((d, i) => ({
            chunk: { id: 'c' + i, doc_id: d.id, content: 'Mock content from ' + displayName(d.file_path), token_count: 10, sequence: i },
            score: 0.91 - i * 0.05,
            doc_name: displayName(d.file_path),
          }));
          emit('chat_sources', sources);

          emit('chat_phase', { phase: 'generating', message: '正在生成回答…' });

          for (const t of tokens) {
            if (state.aborted) {
              emit('chat_error', '⏹ 生成已中断');
              // 中断也持久化已生成部分
              if (convId && state.messages[convId] && assistantContent) {
                state.messages[convId].push({ role: 'assistant', content: assistantContent, sources });
              }
              emit('chat_done', null);
              return;
            }
            assistantContent += t;
            emit('chat_token', t);
            await delay(200);
          }
          // 正常结束持久化助手消息
          if (convId && state.messages[convId]) {
            state.messages[convId].push({ role: 'assistant', content: assistantContent, sources });
          }
          emit('chat_done', null);
        })();
        return null;
      }


      // ============================================================
      // 高级 RAG 功能（REQ-RAG-020~022, REQ-VEC-012~013）
      // ============================================================




      case 'get_chunk_params':
        return { chunk_size: state.chunkSize || 256, overlap: state.chunkOverlap || 32 };

      case 'set_chunk_params':
        state.chunkSize = args.params.chunk_size;
        state.chunkOverlap = args.params.overlap;
        return null;

      case 'reclassify_document': {
        const doc = state.docs.find((d) => d.id === args.docId);
        if (!doc) throw '文档不存在';
        const domain = doc.file_path.includes('rust') ? 'programming' :
                       doc.file_path.includes('medical') ? 'medical' : 'general';
        state.docDomains[args.docId] = domain;
        return domain;
      }

      case 'get_conversations_paginated': {
        const limit = args.limit || 20;
        const offset = args.offset || 0;
        const items = state.conversations.slice(offset, offset + limit);
        return { items, total: state.conversations.length };
      }

      // ============================================================
      // 导出功能（REQ-EXP-001）
      // ============================================================
      case 'export_conversation_markdown': {
        const msgs = state.messages[args.conversationId] || [];
        let md = '# 对话导出\n\n';
        md += `> 导出时间: ${new Date().toISOString()}\n\n`;
        for (const m of msgs) {
          md += `## ${m.role === 'user' ? '👤 用户' : '🤖 助手'}\n\n${m.content}\n\n`;
        }
        state.lastExportContent = md;
        return md;
      }

case 'save_text_file':
state.lastExportPath = args.path;
state.lastExportContent = args.content;
return null;

case 'read_text_file':
// REQ-EXP-003: 返回模拟的备份文件内容
if (state.mockBackupContent) return state.mockBackupContent;
// S88: 返回模拟的模板导出 JSON（路径含 template 关键字时）
if (args.path && String(args.path).includes('template')) {
  return JSON.stringify({
    version: '1.0',
    exported_at: '2026-08-22T12:00:00.000Z',
    templates: [
      { name: 'imported_summary', label: 'Imported Summary', description: 'Imported template', icon: '📋', prompt_template: 'Summarize: {query}' },
      { name: 'test_summary', label: 'Conflict Test', description: 'Name conflict test', icon: '⚡', prompt_template: 'Test: {query}' },
    ],
  });
}
return JSON.stringify({
version: 1,
exported_at: '0',
conversations: [],
messages: {},
documents: [],
settings: {},
});

      // ============================================================
      // 文件监听 + 增量同步（REQ-SYNC-001~003）
      // ============================================================
case 'add_watched_folder': {
if (!args.path || args.path.trim() === '') {
  throw 'VALIDATION: 路径不能为空';
}
if (!state.watchedFolders.includes(args.path)) {
          state.watchedFolders.push(args.path);
        }
        // 模拟同步进度事件（REQ-SYNC-002 AC-7）
        (async () => {
          await delay(100);
          emit('sync_progress', { phase: 'syncing', message: '正在扫描文件夹…', folder: args.path });
          await delay(200);
          emit('sync_progress', { phase: 'complete', message: '文件夹同步完成', folder: args.path });
        })();
        return null;
      }

      case 'remove_watched_folder':
        state.watchedFolders = state.watchedFolders.filter((p) => p !== args.path);
        return null;

      case 'get_watched_folders':
        return state.watchedFolders;

      // ============================================================
      // 安全防御 IPC 命令（REQ-SEC-013~020）
      // ============================================================
      case 'get_security_status':
        return {
          state: state.securityState,
          color: state.securityState === 'Locked' ? '#ef4444' : state.securityState === 'EncryptedUnlocked' ? '#22c55e' : '#f59e0b',
          is_locked: state.isLocked,
          lock_reason: state.lockReason,
          auto_lock_config: state.autoLockConfig,
          auto_lock_timeout: state.autoLockConfig.timeout_secs,
          clipboard_config: state.clipboardConfig,
          remaining_attempts: state.remainingAttempts,
          remaining_lock_seconds: state.remainingLockSeconds,
          panic_wipe_enabled: state.panicWipeEnabled,
          pii_detection_enabled: state.piiDetectionEnabled,
        };

      case 'set_auto_lock_config':
        state.autoLockConfig = {
          enabled: args.enabled ?? true,
          timeout_secs: args.timeout_secs ?? args.timeoutSecs ?? 180,
          lock_on_sleep: args.lock_on_sleep ?? args.lockOnSleep ?? true,
        };
        return null;

      case 'lock_app':
        state.securityState = 'Locked';
        state.isLocked = true;
        state.lockReason = args.reason || 'Manual';
        emit('security-state-changed', { state: 'Locked', reason: state.lockReason });
        return null;

      case 'unlock_app': {
        // 暴力破解锁定检查
        if (state.authFailures >= 5) {
          state.isLocked = true;
          state.remainingLockSeconds = 30;
          throw '错误次数过多，请等待 30 秒后重试';
        }
        // 密码验证：使用加密时设置的密码，未加密时 'test' 为默认密码
        const expectedPwd = state.encryptionPassword || 'test';
        if (args.password !== expectedPwd) {
          state.authFailures++;
          state.remainingAttempts = Math.max(0, 5 - state.authFailures);
          if (state.authFailures >= 5) {
            state.isLocked = true;
            state.remainingLockSeconds = 30;
            throw '错误次数过多，请等待 30 秒后重试';
          }
          throw '密码错误，剩余尝试次数 ' + state.remainingAttempts + ' 次';
        }
        // 正确密码：重置计数器并解锁
        state.securityState = 'EncryptedUnlocked';
        state.isLocked = false;
        state.lockReason = null;
        state.authFailures = 0;
        state.remainingAttempts = 5;
        emit('security-state-changed', { state: 'EncryptedUnlocked' });
        return null;
      }

      case 'record_activity':
        state.lastActivity = Date.now();
        return null;

      case 'detect_pii': {
        const text = args.text || '';
        const detections = [];
        let redactedText = text;
        // 邮箱
        const emailMatches = text.match(/[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}/g) || [];
        for (const m of emailMatches) {
          detections.push({ pii_type: 'email', matched: m, redacted: m.charAt(0) + '***@' + m.split('@')[1] });
          redactedText = redactedText.replace(m, '[REDACTED-EMAIL]');
        }
        // 中国手机号
        const phoneMatches = text.match(/1[3-9]\d{9}/g) || [];
        for (const m of phoneMatches) {
          detections.push({ pii_type: 'phone', matched: m, redacted: m.slice(0, 3) + '****' + m.slice(-4) });
          redactedText = redactedText.replace(m, '[REDACTED-PHONE]');
        }
        // IP 地址
        const ipMatches = text.match(/\b(?:\d{1,3}\.){3}\d{1,3}\b/g) || [];
        for (const m of ipMatches) {
          detections.push({ pii_type: 'ip_address', matched: m, redacted: m.split('.').slice(0, 2).join('.') + '.**.**' });
          redactedText = redactedText.replace(m, '[REDACTED-IP]');
        }
        // 身份证号（18 位）
        const idMatches = text.match(/\d{17}[\dXx]/g) || [];
        for (const m of idMatches) {
          detections.push({ pii_type: 'id_card', matched: m, redacted: m.slice(0, 3) + '**************' + m.slice(-4) });
          redactedText = redactedText.replace(m, '[REDACTED-IDCARD]');
        }
        // 银行卡号（16-19 位以 6 开头的连续数字）
        const bankMatches = text.match(/6\d{15,18}/g) || [];
        for (const m of bankMatches) {
          detections.push({ pii_type: 'bankcard', matched: m, redacted: m.slice(0, 4) + '********' + m.slice(-4) });
          redactedText = redactedText.replace(m, '[REDACTED-BANKCARD]');
        }
        // SSN（美国社会安全号，格式 XXX-XX-XXXX）
        const ssnMatches = text.match(/\b\d{3}-\d{2}-\d{4}\b/g) || [];
        for (const m of ssnMatches) {
          detections.push({ pii_type: 'ssn', matched: m, redacted: '***-**-' + m.slice(-4) });
          redactedText = redactedText.replace(m, '[REDACTED-SSN]');
        }
        // 护照号（G + 8 位数字）
        const passportMatches = text.match(/\b[Gg]\d{8}\b/g) || [];
        for (const m of passportMatches) {
          detections.push({ pii_type: 'passport', matched: m, redacted: '****' + m.slice(-4) });
          redactedText = redactedText.replace(m, '[REDACTED-PASSPORT]');
        }
        // 国际电话号码（+国家代码-号码）
        const intlPhoneMatches = text.match(/\+\d{1,3}-\d{6,14}/g) || [];
        for (const m of intlPhoneMatches) {
          detections.push({ pii_type: 'intl_phone', matched: m, redacted: '+' + m.split('-')[0].slice(1) + '-****' + m.slice(-4) });
          redactedText = redactedText.replace(m, '[REDACTED-INTL_PHONE]');
        }
        return { detections, redacted: redactedText, redacted_text: redactedText, pii_count: detections.length };
      }

      case 'set_panic_wipe_password':
        state.panicWipeEnabled = true;
        return null;

      case 'clear_panic_wipe_password':
        state.panicWipeEnabled = false;
        return null;

      case 'is_panic_wipe_enabled':
        return state.panicWipeEnabled;

      case 'set_clipboard_config':
        state.clipboardConfig = {
          enabled: args.enabled ?? true,
          clear_after_secs: args.clear_after_secs ?? args.clearAfterSecs ?? 30,
        };
        return null;

      case 'get_audit_logs':
        return state.auditLogs.slice(-(args.limit || 100));

      case 'clear_audit_logs':
        state.auditLogs = [];
        return null;

      case 'check_password_strength': {
        const pwd = args.password || '';
        const len = pwd.length;
        const common = ['123456', 'password', '123456789', 'qwerty', 'abc123', 'admin'];
        if (common.includes(pwd.toLowerCase()) || len < 6) {
          return { strength: 'VeryWeak', level: 'weak', percentage: 20, color: '#ef4444', suggestions: ['pwd_tip_length', 'pwd_tip_common'] };
        }
        const hasLower = /[a-z]/.test(pwd);
        const hasUpper = /[A-Z]/.test(pwd);
        const hasDigit = /\d/.test(pwd);
        const hasSpecial = /[^a-zA-Z0-9]/.test(pwd);
        const types = [hasLower, hasUpper, hasDigit, hasSpecial].filter(Boolean).length;
        if (len >= 12 && types >= 4) return { strength: 'VeryStrong', level: 'strong', percentage: 100, color: '#16a34a', suggestions: [] };
        if (len >= 8 && types >= 3) return { strength: 'Strong', level: 'strong', percentage: 80, color: '#22c55e', suggestions: [] };
        if (len >= 6 && types >= 2) return { strength: 'Medium', level: 'medium', percentage: 60, color: '#eab308', suggestions: [] };
        return { strength: 'Weak', level: 'weak', percentage: 40, color: '#f97316', suggestions: ['pwd_tip_uppercase', 'pwd_tip_digit'] };
      }

      // ============================================================
      // 安全态势分层（Q05 借鉴 QM SecurityPosture）
      // ============================================================
      case 'set_security_posture': {
        const posture = args.posture;
        if (!['dangerous', 'auto', 'strict'].includes(posture)) {
          throw new Error(`无效的安全态势值: ${posture}`);
        }
        state.securityPosture = posture;
        return null;
      }

case 'get_security_posture': {
return state.securityPosture || 'auto';
}

// Shadow 安全筛查统计（Q06 借鉴 QM security-screen.ts）
case 'get_security_screen_stats': {
return state.shadowScreenStats || { total: 0, agree: 0, disagree: 0, unavailable: 0 };
}

case 'reset_security_screen_stats': {
state.shadowScreenStats = { total: 0, agree: 0, disagree: 0, unavailable: 0 };
return null;
}

// ============================================================
// 前端 securityApi 兼容命令（ipc.js 中使用的命令名别名）
// ============================================================
      case 'encrypt_database': {
        const pwd = args.password;
        if (!pwd || pwd.length < 8) {
          return { success: false, message: '密码至少 8 个字符' };
        }
        state.encryptionPassword = pwd;
        state.securityState = 'EncryptedUnlocked';
        state.isLocked = false;
        state.lockReason = null;
        state.authFailures = 0;
        state.remainingAttempts = 5;
        emit('security-state-changed', { state: 'EncryptedUnlocked' });
        return { success: true };
      }

      case 'unlock_database': {
        // 前端 securityApi.unlock 调用，返回 { success, message?, wait_seconds? }
        if (state.authFailures >= 5) {
          state.isLocked = true;
          state.remainingLockSeconds = 30;
          return { success: false, message: '错误次数过多', wait_seconds: 30 };
        }
        const expectedPwd = state.encryptionPassword || 'test';
        if (args.password !== expectedPwd) {
          state.authFailures++;
          state.remainingAttempts = Math.max(0, 5 - state.authFailures);
          if (state.authFailures >= 5) {
            state.isLocked = true;
            state.remainingLockSeconds = 30;
            return { success: false, message: '错误次数过多', wait_seconds: 30 };
          }
          return { success: false, message: '密码错误，剩余尝试次数 ' + state.remainingAttempts + ' 次' };
        }
        state.securityState = 'EncryptedUnlocked';
        state.isLocked = false;
        state.lockReason = null;
        state.authFailures = 0;
        state.remainingAttempts = 5;
        emit('security-state-changed', { state: 'EncryptedUnlocked' });
        return { success: true };
      }

      case 'verify_audit_chain':
        return { valid: true, count: state.auditLogs.length, broken_at: null };

      case 'set_pii_detection_enabled':
        state.piiDetectionEnabled = args.enabled;
        return null;




      // ============================================================
      // 国际化（REQ-I18N-001）
      // ============================================================
      case 'get_locale':
        return state.locale;

      case 'set_locale':
        state.locale = args.locale;
        return null;

      case 'get_theme':
        return state.theme || localStorage.getItem('echomind.theme') || 'dark';

      case 'set_theme':
        state.theme = args.theme;
        localStorage.setItem('echomind.theme', args.theme);
        return null;

      case 'get_sidebar_collapsed':
        return state.sidebarCollapsed || false;

      case 'get_graph_data': {
        const limit = args.limit || 200;
        const relations = state.graphRelations || [];
        return relations.slice(0, limit).map((r) => ({
          subject: r.subject,
          relation: r.relation_type,
          object: r.object,
        }));
      }

      case 'get_entity_relations': {
        const entityText = args.entityText || '';
        const relations = state.graphRelations || [];
        return relations.filter(
          (r) => r.subject === entityText || r.object === entityText
        );
      }

      case 'get_graph_stats': {
        const relations = state.graphRelations || [];
        const entities = new Set();
        const typeCounts = {};
        for (const r of relations) {
          entities.add(r.subject);
          entities.add(r.object);
          typeCounts[r.relation_type] = (typeCounts[r.relation_type] || 0) + 1;
        }
        return {
          total_entities: entities.size,
          total_relations: relations.length,
          relation_type_counts: typeCounts,
        };
      }

      case 'get_entity_types': {
        // 返回实体文本 → 类型映射（mock 数据）
        const entityTypeMap = {
          'Rust': 'tech_term',
          'Cargo': 'tech_term',
          'tokio': 'tech_term',
          'serde': 'tech_term',
          'HashMap': 'tech_term',
          'WebAssembly': 'tech_term',
          'Serialize': 'tech_term',
          'Collection': 'tech_term',
          'trait': 'proper_noun',
          'crates.io': 'identifier',
          'systems programming language': 'proper_noun',
        };
        const result = {};
        for (const e of (args.entities || [])) {
          if (entityTypeMap[e]) result[e] = entityTypeMap[e];
        }
        return result;
      }

      case 'get_shortest_path': {
        // 构建邻接表并使用 BFS 查找最短路径
        const relations = state.graphRelations || [];
        const from = args.from || '';
        const to = args.to || '';
        if (!from || !to) return { path: [], hops: 0 };

        // 构建无向邻接表
        const adj = {};
        for (const r of relations) {
          if (!adj[r.subject]) adj[r.subject] = [];
          if (!adj[r.object]) adj[r.object] = [];
          adj[r.subject].push(r.object);
          adj[r.object].push(r.subject);
        }

        // BFS
        if (!adj[from] || !adj[to]) return { path: [], hops: 0 };
        if (from === to) return { path: [from], hops: 0 };

        const visited = new Set([from]);
        const parent = {};
        const queue = [from];
        let found = false;

        while (queue.length > 0) {
          const current = queue.shift();
          if (current === to) { found = true; break; }
          for (const neighbor of (adj[current] || [])) {
            if (!visited.has(neighbor)) {
              visited.add(neighbor);
              parent[neighbor] = current;
              queue.push(neighbor);
            }
          }
        }

        if (!found) return { path: [], hops: 0 };

        const path = [];
        let cur = to;
        path.push(cur);
        while (parent[cur]) {
          path.push(parent[cur]);
          cur = parent[cur];
        }
        path.reverse();
        return { path, hops: path.length - 1 };
      }

      case 'get_communities': {
        // 简单 Label Propagation mock
        const relations = state.graphRelations || [];
        if (relations.length === 0) return { communities: {}, community_count: 0 };

        // 构建无向邻接表
        const adj = {};
        for (const r of relations) {
          if (!adj[r.subject]) adj[r.subject] = [];
          if (!adj[r.object]) adj[r.object] = [];
          adj[r.subject].push(r.object);
          adj[r.object].push(r.subject);
        }

        // 简单社区检测：连通分量
        const visited = new Set();
        const communities = {};
        let communityId = 0;

        for (const node of Object.keys(adj)) {
          if (!visited.has(node)) {
            const queue = [node];
            while (queue.length > 0) {
              const cur = queue.shift();
              if (visited.has(cur)) continue;
              visited.add(cur);
              communities[cur] = communityId;
              for (const neighbor of (adj[cur] || [])) {
                if (!visited.has(neighbor)) queue.push(neighbor);
              }
            }
            communityId++;
          }
        }

        return { communities, community_count: communityId };
      }

      case 'get_graph_layout': {
        return ['force', 'hierarchical', 'radial'];
      }

      // ============================================================
      // 知识图谱导出（REQ-EXP-006 GraphML/JSON-LD）
      // ============================================================
      case 'export_graph': {
        const format = args.format || 'graphml';
        const relations = state.graphRelations || [];
        if (format === 'graphml') {
          let xml = '<?xml version="1.0" encoding="UTF-8"?>\n<graphml xmlns="http://graphml.graphdrawing.org/xmlns">\n  <graph id="G" edgedefault="directed">\n';
          const entities = new Set();
          for (const r of relations) {
            entities.add(r.subject);
            entities.add(r.object);
          }
          for (const e of entities) {
            xml += '    <node id="' + e + '"/>\n';
          }
          for (const r of relations) {
            xml += '    <edge source="' + r.subject + '" target="' + r.object + '"/>\n';
          }
          xml += '  </graph>\n</graphml>\n';
          return xml;
        } else if (format === 'jsonld') {
          const graph = [];
          const entities = new Set();
          for (const r of relations) {
            entities.add(r.subject);
            entities.add(r.object);
          }
          for (const e of entities) {
            graph.push({ '@id': e, '@type': 'Entity' });
          }
          for (const r of relations) {
            graph.push({ '@id': r.id || 'rel-' + Math.random(), '@type': 'Relation', subject: r.subject, relationType: r.relation_type, object: r.object });
          }
          return JSON.stringify({ '@context': { 'Entity': 'https://echomind.local/ontology#Entity', 'Relation': 'https://echomind.local/ontology#Relation' }, '@graph': graph }, null, 2);
        }
        throw '不支持的导出格式: ' + format;
      }

      // ============================================================
      // 文档摘要自动生成（REQ-ING-019）
      // ============================================================
      case 'get_document_summary': {
        const doc = state.docs.find((d) => d.id === args.docId);
        if (!doc) throw '文档不存在';
        return doc.summary || null;
      }

      case 'regenerate_summary': {
        const doc = state.docs.find((d) => d.id === args.docId);
        if (!doc) throw '文档不存在';
        // 模拟 LLM 生成摘要
        const mockSummary = '这是一份关于' + (doc.file_path || '文档') + '的自动摘要。文档涵盖了关键技术概念、实现方法和应用场景。摘要由 LLM 在导入时自动生成，帮助用户快速了解文档核心内容。';
        doc.summary = mockSummary;
        return mockSummary;
      }

      // 文档内容预览（REQ-ING-010）
      case 'get_document_preview': {
        const doc = state.docs.find((d) => d.id === args.docId);
        if (!doc) return null;
        const chunks = (state.chunks || {})[args.docId] || [];
        const contentPreview = chunks.map((c) => c.content).join(' ').substring(0, 500);
        return {
          id: doc.id,
          file_path: doc.file_path,
          status: doc.status || 'Indexed',
          created_at: doc.created_at || Math.floor(Date.now() / 1000),
          domain: doc.domain || null,
          summary: doc.summary || null,
          tags: doc.tags || [],
          file_hash: doc.file_hash || 'abc123',
          content_preview: contentPreview || '(empty)',
          chunks: chunks.map((c) => ({
            id: c.id || 'chunk-1',
            sequence: c.sequence || 0,
            content_preview: (c.content || '').substring(0, 200),
            token_count: c.token_count || 100,
          })),
          chunk_count: chunks.length,
        };
      }

      // 单条消息删除（REQ-RAG-013）
      case 'delete_message': {
        const msgs = state.messages[args.conversationId] || [];
        const idx = msgs.findIndex((m) => m.id === args.messageId);
        if (idx < 0) throw '消息不存在';
        const target = msgs[idx];
        let count = 1;
        // user 消息连带删除下一条 assistant
        if (target.role === 'user' && idx + 1 < msgs.length && msgs[idx + 1].role === 'assistant') {
          msgs.splice(idx, 2);
          count = 2;
        } else {
          msgs.splice(idx, 1);
        }
        state.messages[args.conversationId] = msgs;
        return count;
      }

      // ============================================================
      // DAG 工作流引擎（REQ-RAG-030）
      // ============================================================
      case 'save_workflow_template': {
        const wf = args.workflow || args.template;
        if (wf) {
          const template = { ...wf, id: wf.id || 'wf-' + Date.now(), created_at: Math.floor(Date.now() / 1000) };
          state.workflowTemplates.push(template);
          return template.id;
        }
        throw '工作流定义无效';
      }

      case 'run_workflow': {
        const wfId = args.workflow_id || args.workflowId;
        const wf = state.workflowTemplates.find((w) => w.id === wfId);
        if (!wf) throw '工作流不存在: ' + wfId;
        const nodeResults = {};
        for (const node of (wf.nodes || [])) {
          nodeResults[node.id] = { Completed: { output: 'Mock output for ' + node.label } };
        }
        (async () => {
          await delay(100);
          emit('workflow_progress', { workflow_id: wfId, node_id: wf.nodes?.[0]?.id, status: 'Running' });
          await delay(200);
          for (const node of (wf.nodes || [])) {
            emit('workflow_progress', { workflow_id: wfId, node_id: node.id, status: 'Completed' });
            await delay(100);
          }
        })();
        return { node_results: nodeResults, final_output: 'Mock workflow final output', duration_ms: 500 };
      }

      case 'list_workflows':
        return state.workflowTemplates;

case 'delete_workflow':
state.workflowTemplates = state.workflowTemplates.filter((w) => w.id !== args.workflow_id);
return null;

// ============================================================
// 自定义快捷指令模板（S56）
// ============================================================
case 'save_prompt_template': {
const name = args.name;
const label = args.label;
const description = args.description || '';
const icon = args.icon || '⚡';
const promptTemplate = args.prompt_template || args.promptTemplate;
if (!name || !label || !promptTemplate) throw '名称、标签和模板内容不能为空';
// S56 验证：系统指令名称冲突
const SYSTEM_COMMANDS = ['summary', 'compare', 'extract', 'translate', 'timeline', 'mindmap'];
if (SYSTEM_COMMANDS.includes(name)) throw 'VALIDATION: 指令名称与系统内置指令冲突';
// S56 验证：{query} 占位符
if (!promptTemplate.includes('{query}')) throw 'VALIDATION: 模板内容必须包含 {query} 占位符';
const existing = state.promptTemplates.find((t) => t.name === name);
if (existing) {
// 更新已有模板
existing.label = label;
existing.description = description;
existing.icon = icon;
existing.prompt_template = promptTemplate;
existing.updated_at = Math.floor(Date.now() / 1000);
return existing.id;
}
const tmpl = {
id: 'pt-' + Date.now() + '-' + Math.random().toString(36).slice(2, 8),
name, label, description, icon,
prompt_template: promptTemplate,
created_at: Math.floor(Date.now() / 1000),
updated_at: Math.floor(Date.now() / 1000),
};
state.promptTemplates.push(tmpl);
return tmpl.id;
}

case 'list_prompt_templates':
return state.promptTemplates.slice().sort((a, b) => a.name.localeCompare(b.name));

case 'delete_prompt_template':
state.promptTemplates = state.promptTemplates.filter((t) => t.id !== (args.templateId || args.template_id));
return null;

// ============================================================
// B09 Skill 系统集成（v1.8：斜杠命令面板 Skill 发现）
// ============================================================
case 'discover_skills':
return state.skills || [];

// ============================================================
// Wiki 双向链接（REQ-ING-020 Markdown 笔记双向链接）
// ============================================================
case 'get_forward_links': {
  // Mock: return wiki links where source_doc_id matches
  return (state.wikiLinks || []).filter((l) => l.source_doc_id === args.docId);
}

case 'get_backlinks': {
  // Mock: return wiki links where target matches docName (fuzzy)
  const pattern = args.docName || '';
  return (state.wikiLinks || []).filter((l) => 
    l.target.toLowerCase().includes(pattern.toLowerCase())
  );
}

case 'rebuild_wiki_links': {
  // Mock: return count of existing wiki links
  return (state.wikiLinks || []).length;
}

case 'get_conversation_tree': {
  // Mock: return conversation tree from state, or empty tree
  if (state.conversationTree) {
    return state.conversationTree;
  }
  return {
    conversation_id: args.conversationId || '',
    nodes: [],
    root_ids: [],
    active_path: [],
  };
}

case 'branch_from_message': {
// Mock: return new version number
return {
new_version: 2,
turn_group: 'turn-mock-branch',
};
}

// ============================================================
// 对话全文搜索（REQ-RAG-040）
// ============================================================

case 'search_conversations': {
// Mock: search messages in mock state
const query = (args.query || '').toLowerCase();
const messages = state.messages || [];
const allMessages = Array.isArray(messages) ? messages : Object.values(messages).flat();
if (!query) return [];
return allMessages
.filter(m => (m.content || '').toLowerCase().includes(query))
.slice(0, args.limit || 50)
.map(m => ({
message_id: m.id || 'msg-1',
conversation_id: m.conversation_id || 'conv-1',
conversation_title: m.conversation_title || (state.conversations || []).find(c => c.id === (m.conversation_id || 'conv-1'))?.title || 'Mock Conversation',
role: m.role || 'user',
content: m.content || '',
score: 1.0,
created_at: m.created_at || 0,
}));
}

// ============================================================
// 全局搜索增强（REQ-IX-008, S90）
// ============================================================

case 'global_search': {
const query = (args.query || '').toLowerCase();
const limit = args.limit || 5;
if (!query) return { messages: [], documents: [], entities: [] };

// 搜索消息
const allMessages = Array.isArray(state.messages) ? state.messages : Object.values(state.messages || {}).flat();
const messages = allMessages
  .filter(m => (m.content || '').toLowerCase().includes(query))
  .slice(0, limit)
  .map(m => ({
    message_id: m.id || 'msg-1',
    conversation_id: m.conversation_id || 'conv-1',
    conversation_title: (state.conversations || []).find(c => c.id === (m.conversation_id || 'conv-1'))?.title || 'Mock Conversation',
    role: m.role || 'user',
    content: m.content || '',
    score: 1.0,
    created_at: m.created_at || 0,
  }));

// 搜索文档
const docs = (state.documents || []).filter(d => {
  const name = (d.file_path || '').toLowerCase();
  const summary = (d.summary || '').toLowerCase();
  return name.includes(query) || summary.includes(query);
}).slice(0, limit).map(d => ({
  doc_id: d.id || 'doc-1',
  file_path: d.file_path || '',
  summary: d.summary || null,
  match_type: (d.file_path || '').toLowerCase().includes(query) ? 'title' : 'summary',
}));

// 搜索实体
const entities = (state.graphRelations || [])
  .filter(r => (r.subject || '').toLowerCase().includes(query) || (r.object || '').toLowerCase().includes(query))
  .slice(0, limit)
  .map(r => ({
    entity_text: r.subject || '',
    entity_type: r.entity_type || 'PERSON',
    chunk_id: r.chunk_id || 'chunk-1',
    doc_id: r.doc_id || 'doc-1',
  }));

return { messages, documents, entities };
}

// ============================================================
// Session Strip 会话条带化（REQ-RAG-046）
// ============================================================

case 'strip_messages': {
const convId = args.conversation_id;
const fromIdx = args.from_index || 0;
const toIdx = args.to_index || 0;
const convMessages = (state.messages || []).filter(m => m.conversation_id === convId);
const stripped = convMessages.slice(fromIdx, toIdx + 1);
const strippedIds = stripped.map(m => m.id).filter(Boolean);
state.messages = (state.messages || []).filter(m => !strippedIds.includes(m.id));
let summaryInserted = false;
if (args.replace_with_summary && args.summary_text) {
state.messages.push({
id: 'strip-summary-' + Date.now(),
conversation_id: convId,
role: 'system',
content: '[摘要] ' + args.summary_text,
created_at: Date.now(),
});
summaryInserted = true;
}
return {
stripped_count: stripped.length,
summary_inserted: summaryInserted,
stripped_message_ids: strippedIds,
estimated_tokens_saved: stripped.reduce((sum, m) => sum + Math.floor((m.content || '').length / 4), 0),
};
}

case 'strip_keeping_recent': {
const convId = args.conversation_id;
const keepN = args.keep_last_n || 0;
const convMessages = (state.messages || []).filter(m => m.conversation_id === convId);
const stripped = convMessages.slice(0, convMessages.length - keepN);
const strippedIds = stripped.map(m => m.id).filter(Boolean);
state.messages = (state.messages || []).filter(m => !strippedIds.includes(m.id));
let summaryInserted = false;
if (args.replace_with_summary && args.summary_text) {
state.messages.push({
id: 'strip-summary-' + Date.now(),
conversation_id: convId,
role: 'system',
content: '[摘要] ' + args.summary_text,
created_at: Date.now(),
});
summaryInserted = true;
}
return {
stripped_count: stripped.length,
summary_inserted: summaryInserted,
stripped_message_ids: strippedIds,
estimated_tokens_saved: stripped.reduce((sum, m) => sum + Math.floor((m.content || '').length / 4), 0),
};
}

case 'preview_strip': {
const convId = args.conversation_id;
const fromIdx = args.from_index || 0;
const toIdx = args.to_index || 0;
const convMessages = (state.messages || []).filter(m => m.conversation_id === convId);
const previewMessages = convMessages.slice(fromIdx, toIdx + 1);
return {
messages: previewMessages,
total_messages: convMessages.length,
estimated_tokens_saved: previewMessages.reduce((sum, m) => sum + Math.floor((m.content || '').length / 4), 0),
};
}

// ============================================================
// 持久化记忆系统（REQ-RAG-033）
// ============================================================

      case 'get_memories': {
        let memories = state.memories;
        if (args.tier) {
          memories = memories.filter((m) => m.tier === args.tier);
        }
        return memories;
      }

      case 'pin_memory': {
        const entry = {
          id: 'mem-' + Date.now(),
          tier: 'room',
          content: args.content || '',
          source: 'user_pinned',
          conversation_id: args.conversation_id || null,
          created_at: Math.floor(Date.now() / 1000),
          last_accessed: Math.floor(Date.now() / 1000),
          access_count: 0,
          importance: 1.0,
        };
        state.memories.push(entry);
        return entry;
      }

      case 'promote_memory': {
        const mem = state.memories.find((m) => m.id === args.memory_id || m.id === args.memoryId);
        if (!mem) throw '记忆不存在';
        if (mem.tier === 'wing') mem.tier = 'hall';
        else if (mem.tier === 'hall') mem.tier = 'room';
        return mem;
      }

      case 'delete_memory': {
        const id = args.memory_id || args.memoryId;
        state.memories = state.memories.filter((m) => m.id !== id);
        return null;
      }

      case 'clear_memories': {
        const tier = args.tier;
        if (tier) {
          const before = state.memories.length;
          state.memories = state.memories.filter((m) => m.tier !== tier);
          return before - state.memories.length;
        }
        const count = state.memories.length;
        state.memories = [];
        return count;
      }

      // Scratch-Promote 记忆整合（Q01 借鉴 QM scratch-promote + consolidation）
      case 'trigger_memory_consolidation': {
        if (!state.scratchLogs) state.scratchLogs = [];
        const before = state.scratchLogs.length;
        state.scratchLogs = [];
        return {
          actions_count: 0,
          expired_cleaned: 0,
          remaining_scratch: 0,
        };
      }

      case 'get_scratch_logs': {
        if (!state.scratchLogs) state.scratchLogs = [];
        const limit = args.limit;
        const logs = state.scratchLogs.slice();
        return limit ? logs.slice(0, limit) : logs;
      }

      // Burst Buffer 延迟批量记忆提取（Q02 借鉴 QM createBurstBuffer）
      case 'push_burst_turn': {
        if (!state.burstBuffer) state.burstBuffer = [];
        state.burstBuffer.push({
          user_msg: args.user_msg || '',
          assistant_reply: args.assistant_reply || '',
          conversation_id: args.conversation_id || '',
          message_seq: args.message_seq || 1,
        });
        // Mock: flush when buffer reaches 10 turns (default max_turns)
        const shouldFlush = state.burstBuffer.length >= 10;
        let extracted = 0;
        if (shouldFlush) {
          // Mock: simulate extraction by writing to scratchLogs
          for (const turn of state.burstBuffer) {
            state.scratchLogs.push({
              id: 'scratch-' + Math.random().toString(36).slice(2),
              date: new Date().toISOString().slice(0, 10),
              content: 'extracted fact (said in 对话：' + turn.conversation_id + ')',
              created_at: Date.now(),
            });
            extracted++;
          }
          state.burstBuffer = [];
        }
        return {
          pending: shouldFlush ? 0 : state.burstBuffer.length,
          flushed: shouldFlush,
          extracted: extracted,
        };
      }

      case 'flush_memory_burst_buffer': {
        if (!state.burstBuffer) state.burstBuffer = [];
        const pendingBefore = state.burstBuffer.length;
        let extracted = 0;
        for (const turn of state.burstBuffer) {
          if (!state.scratchLogs) state.scratchLogs = [];
          state.scratchLogs.push({
            id: 'scratch-' + Math.random().toString(36).slice(2),
            date: new Date().toISOString().slice(0, 10),
            content: 'extracted fact (said in 对话：' + turn.conversation_id + ')',
            created_at: Date.now(),
          });
          extracted++;
        }
        state.burstBuffer = [];
        return {
          extracted: extracted,
          pending_before: pendingBefore,
        };
      }

      case 'get_burst_buffer_status': {
        if (!state.burstBuffer) state.burstBuffer = [];
        return {
          pending: state.burstBuffer.length,
          should_flush: state.burstBuffer.length >= 10,
        };
      }

      // ============================================================
      // 语音转写（REQ-RAG-034 桌面方案：getUserMedia + MediaRecorder + Whisper API）
      // ============================================================
case 'transcribe_audio': {
// mock：返回预设的转写文本
return '测试语音输入';
}

case 'get_stt_config': {
return {
  stt_api_key_masked: window.__mock?.sttConfig?.apiKeyMasked || '',
  stt_base_url: window.__mock?.sttConfig?.baseUrl || '',
  stt_model: window.__mock?.sttConfig?.model || 'whisper-1',
  stt_language: window.__mock?.sttConfig?.language || 'zh',
  has_custom_config: !!(window.__mock?.sttConfig?.baseUrl)
};
}

case 'set_stt_config': {
if (!window.__mock) window.__mock = {};
if (!window.__mock.sttConfig) window.__mock.sttConfig = {};
if (args.sttApiKey !== null && args.sttApiKey !== undefined) {
  const key = args.sttApiKey;
  window.__mock.sttConfig.apiKeyMasked = key ? '****' + key.slice(-4) : '';
}
if (args.sttBaseUrl !== null && args.sttBaseUrl !== undefined) {
  window.__mock.sttConfig.baseUrl = args.sttBaseUrl;
}
if (args.sttModel !== null && args.sttModel !== undefined) {
  window.__mock.sttConfig.model = args.sttModel;
}
if (args.sttLanguage !== null && args.sttLanguage !== undefined) {
  window.__mock.sttConfig.language = args.sttLanguage;
}
return null;
}

      // ============================================================
      // 代码符号引擎（REQ-RAG-031）
      // ============================================================
      case 'search_symbols': {
        const query = (args.query || '').toLowerCase();
        const mockSymbols = [
          { id: 'sym-1', chunk_id: 'chunk-1', name: 'main', kind: 'Function', language: 'rust', start_line: 1, end_line: 10, signature: 'fn main()' },
          { id: 'sym-2', chunk_id: 'chunk-2', name: 'ChatEngine', kind: 'Struct', language: 'rust', start_line: 15, end_line: 50, signature: 'struct ChatEngine' },
          { id: 'sym-3', chunk_id: 'chunk-3', name: 'Storage', kind: 'Interface', language: 'rust', start_line: 5, end_line: 30, signature: 'trait Storage' },
          { id: 'sym-4', chunk_id: 'chunk-4', name: 'embed_batch', kind: 'Method', language: 'rust', start_line: 20, end_line: 35, signature: 'async fn embed_batch()' },
        ];
        if (!query) return mockSymbols;
        return mockSymbols.filter((s) => s.name.toLowerCase().includes(query));
      }

      case 'get_symbols_for_chunk': {
        const chunkId = args.chunk_id || args.chunkId;
        return [
          { id: 'sym-' + chunkId + '-1', chunk_id: chunkId, name: 'process_data', kind: 'Function', language: 'rust', start_line: 1, end_line: 20, signature: 'fn process_data(input: &str) -> Result<String>' },
        ];
      }

      case 'rebuild_symbol_index':
        state.symbolIndexBuilt = true;
        return null;

      // ============================================================
      // 代码执行沙箱（REQ-RAG-032）
      // ============================================================
      case 'execute_code_snippet': {
        const code = args.code || '';
        const lang = args.language || 'python';
        // Mock: if code contains "print(" return the content inside print
        const printMatch = code.match(/print\(["'](.+?)["']\)/);
        const output = printMatch ? printMatch[1] + '\n' : 'Mock execution output\n';
        return {
          stdout: output,
          stderr: '',
          exit_code: 0,
          duration_ms: 42,
          timed_out: false,
        };
      }

      // ============================================================
      // AutoDream 引擎
      // ============================================================
      case 'trigger_dream': {
        state.dreamAborted = false;
        (async () => {
          await delay(100);
          emit('dream_progress', { phase: 'scanning', message: '正在扫描文档…', progress: 0.1 });
          await delay(200);
          emit('dream_progress', { phase: 'analyzing', message: '正在分析重复与矛盾…', progress: 0.5 });
          await delay(200);
          if (state.dreamAborted) {
            emit('dream_progress', { phase: 'aborted', message: '已中止', progress: 0 });
            return;
          }
          emit('dream_progress', { phase: 'complete', message: '分析完成', progress: 1.0 });
        })();
        const suggestions = [
          { suggestion_id: 'dream-1', suggestion_type: 'duplicate_documents', title: '发现重复文档', description: 'doc-a.md 和 doc-b.md 内容高度相似（92%）', doc_ids: ['doc-a', 'doc-b'], doc_names: ['doc-a.md', 'doc-b.md'], severity: 'medium', similarity: 0.92 },
          { suggestion_id: 'dream-2', suggestion_type: 'contradiction', title: '检测到矛盾信息', description: '关于 PDF 支持的描述存在矛盾', doc_ids: ['doc-c', 'doc-d'], doc_names: ['guide.md', 'faq.md'], severity: 'high', similarity: null },
          { suggestion_id: 'dream-3', suggestion_type: 'organization', title: '建议添加标签', description: '3 个文档未分类', doc_ids: ['doc-e', 'doc-f', 'doc-g'], doc_names: ['notes.md', 'draft.md', 'misc.md'], severity: 'low', similarity: null },
        ];
        state.dreamSuggestions = suggestions;
        return { suggestions, total_documents: state.docs.length, total_suggestions: suggestions.length, elapsed_ms: 500 };
      }

      case 'get_dream_suggestions':
        return state.dreamSuggestions;

      case 'abort_dream':
        state.dreamAborted = true;
        return null;

      // ============================================================
      // 性能优化：缓存（REQ-PERF-001）
      // ============================================================
      case 'get_cache_stats':
        return state.cacheStats;

      case 'clear_cache':
        state.cacheStats = { ...state.cacheStats, exact_hits: 0, semantic_hits: 0, retrieval_hits: 0, cache_size_entries: 0, estimated_token_saved: 0 };
        return null;

      case 'set_cache_settings':
        state.cacheSettings = { ...state.cacheSettings, ...args.settings };
        return null;

      case 'get_cache_settings':
        return state.cacheSettings;

      // ============================================================
      // 性能优化：Prompt 压缩（REQ-PERF-002）
      // ============================================================


      // ============================================================
      // 性能优化：索引重建
      // ============================================================
      case 'rebuild_bm25_index':
        return null;

      case 'rebuild_proposition_index':
        return null;

      case 'build_summary_tree':
        return null;

      // ============================================================
      // Contextual Retrieval（REQ-RAG-041：上下文增强嵌入）
      // ============================================================

      case 'rebuild_contextual_embeddings':
        return null;

      case 'rebuild_all_embeddings':
        return null;

      // Late Chunking 上下文感知嵌入（REQ-RAG-049）
      case 'set_late_chunking':
        state.lateChunking = args.enabled;
        return null;
      case 'get_late_chunking':
        return state.lateChunking === true;

      // ============================================================
      // 文档标签系统（REQ-ING-022：用户自定义标签管理）
      // ============================================================
      case 'add_document_tag': {
        const doc = state.docs.find(d => d.id === args.docId);
        if (doc) {
          if (!doc.tags) doc.tags = [];
          if (!doc.tags.includes(args.tag)) {
            doc.tags.push(args.tag);
          }
        }
        return null;
      }

      case 'remove_document_tag': {
        const doc = state.docs.find(d => d.id === args.docId);
        if (doc && doc.tags) {
          doc.tags = doc.tags.filter(t => t !== args.tag);
        }
        return null;
      }

      case 'list_all_tags': {
        const tagCounts = {};
        for (const doc of state.docs) {
          if (doc.tags) {
            for (const tag of doc.tags) {
              tagCounts[tag] = (tagCounts[tag] || 0) + 1;
            }
          }
        }
        return Object.entries(tagCounts).sort((a, b) => b[1] - a[1]);
      }

case 'filter_documents_by_tag': {
return state.docs.filter(d => d.tags && d.tags.includes(args.tag));
}

case 'get_kb_stats': {
// REQ-KB-003 v1.5 + REQ-VEC-010 v1.16 KB statistics dashboard mock
const docs = state.docs || [];
const docCount = docs.length;
const chunkCount = docs.reduce((sum, d) => sum + (d.chunk_count || 3), 0);
const vectorCount = chunkCount; // mock: 每个chunk一个向量
const domainMap = {};
const formatMap = {};
const statusMap = { pending: 0, processing: 0, indexed: 0, failed: 0 };
for (const doc of docs) {
  const domain = doc.domain || 'general';
  domainMap[domain] = (domainMap[domain] || 0) + 1;
  const ext = (doc.file_path || '').split('.').pop() || 'unknown';
  formatMap[ext] = (formatMap[ext] || 0) + 1;
  const status = doc.status || 'Indexed';
  if (status === 'Pending' || status === 'pending') statusMap.pending++;
  else if (status === 'Processing' || status === 'processing') statusMap.processing++;
  else if (status === 'Failed' || status === 'failed') statusMap.failed++;
  else statusMap.indexed++;
}
const tagCounts = {};
for (const doc of docs) {
  if (doc.tags) {
    for (const tag of doc.tags) {
      tagCounts[tag] = (tagCounts[tag] || 0) + 1;
    }
  }
}
return {
  doc_count: docCount,
  chunk_count: chunkCount,
  vector_count: vectorCount,
  db_size_bytes: docCount * 10240,
  domain_distribution: Object.entries(domainMap).sort((a, b) => b[1] - a[1]),
  format_distribution: Object.entries(formatMap).sort((a, b) => b[1] - a[1]),
  tags: Object.entries(tagCounts).sort((a, b) => b[1] - a[1]),
  status_distribution: [
    ['pending', statusMap.pending],
    ['processing', statusMap.processing],
    ['indexed', statusMap.indexed],
    ['failed', statusMap.failed],
  ],
};
}

      // ============================================================
      // Durable Prompt Admission（B05 持久化提示接纳）
      // ============================================================
      case 'admit_input': {
        const input = {
          id: 'pi-' + Date.now() + '-' + Math.random().toString(36).slice(2, 8),
          conversation_id: args.conversation_id || '',
          content: args.content || '',
          delivery: args.delivery || 'queue',
          created_at: Math.floor(Date.now() / 1000),
          promoted_seq: null,
        };
        state.pendingInputs.push(input);
        return input.id;
      }

      case 'promote_input': {
        const input = state.pendingInputs.find(p => p.id === args.input_id);
        if (input) {
          input.promoted_seq = Date.now();
        }
        return null;
      }

      case 'get_pending_inputs': {
        const pending = (state.pendingInputs || [])
          .filter(p => p.conversation_id === args.conversation_id && p.promoted_seq === null)
          .sort((a, b) => {
            // steer 优先，然后按 created_at ASC
            const aPriority = a.delivery === 'steer' ? 0 : 1;
            const bPriority = b.delivery === 'steer' ? 0 : 1;
            if (aPriority !== bPriority) return aPriority - bPriority;
            return a.created_at - b.created_at;
          });
        return pending;
      }

      // ============================================================
      // Session Todo 持久化（B08 会话待办持久化）
      // ============================================================
      case 'add_session_todo': {
        const todo = {
          id: 'todo-' + Date.now() + '-' + Math.random().toString(36).slice(2, 8),
          conversation_id: args.conversation_id || '',
          content: args.content || '',
          status: 'pending',
          priority: 'medium',
          position: args.position || 0,
          created_at: Math.floor(Date.now() / 1000),
        };
        state.sessionTodos.push(todo);
        return todo.id;
      }

      case 'update_todo_status': {
        const todo = state.sessionTodos.find(t => t.id === args.todo_id);
        if (todo) {
          todo.status = args.status;
        }
        return null;
      }

      case 'get_session_todos': {
        return (state.sessionTodos || [])
          .filter(t => t.conversation_id === args.conversation_id)
          .sort((a, b) => a.position - b.position);
      }

      case 'delete_session_todo': {
        state.sessionTodos = state.sessionTodos.filter(t => t.id !== args.todo_id);
        return null;
      }

      case 'delete_session_todos': {
        state.sessionTodos = state.sessionTodos.filter(
          t => t.conversation_id !== args.conversation_id,
        );
        return null;
      }

      // ============================================================
      // 性能优化：检索记忆（REQ-PERF-012）
      // ============================================================

      case 'get_retrieval_memory_stats':
        return state.retrievalMemoryStats;

      case 'reset_retrieval_memory':
        state.retrievalMemoryStats = [];
        return null;

      case 'record_retrieval_feedback':
        // Mock: 记录反馈到统计列表
        state.retrievalMemoryStats.push({
          query_type: args.query_type || 'factual',
          retrieval_method: args.retrieval_method || 'hybrid',
          hit_rate: args.hit_rate || 0.5,
          avg_score: args.avg_score || 0.7,
          sample_count: 1,
        });
        return null;

      // ============================================================
      // 子代理 + 协调模式
      // ============================================================





      // ============================================================
      // 网页搜索集成（REQ-RAG-036）
      // ============================================================

      case 'web_search': {
        // 模拟 DuckDuckGo Instant Answer API 返回
        const mockResults = [
          { title: 'Mock Web Result 1', url: 'https://example.com/1', snippet: 'This is a mock web search result for testing.', source: 'duckduckgo' },
          { title: 'Mock Web Result 2', url: 'https://example.com/2', snippet: 'Another mock search result snippet.', source: 'duckduckgo' },
        ];
return mockResults;
}

// ============================================================
// RAG/LLM 参数可配置（REQ-RAG-014/015 v1.10）
// ============================================================
case 'get_rag_params':
return state.ragParams || { top_k: 8, score_threshold: 0.0, chunk_expansion_enabled: true, chunk_expansion_window: 1 };

case 'set_rag_params':
state.ragParams = args.params;
return null;

case 'get_generation_params':
return state.generationParams || { temperature: 0.7, max_tokens: 4096, top_p: 1.0 };

case 'set_generation_params':
state.generationParams = args.params;
return null;

// ============================================================
// 嵌入模型下载镜像源配置（REQ-VEC-017 v2.3）
// ============================================================
case 'set_mirror_source':
state.mirrorSource = args.source;
return null;

case 'get_mirror_source':
return state.mirrorSource || 'auto';

// ============================================================
// 可观测性（REQ-OBS-001/002）
      // ============================================================
      case 'get_log_level':
        return state.logLevel;

      case 'set_log_level':
        state.logLevel = args.level;
        return null;

      case 'export_logs':
        return 'Mock log export\n2026-08-06T03:00:00Z INFO Application started\n2026-08-06T03:01:00Z INFO Document imported';

case 'export_diagnostics':
return { logs: 'Mock diagnostics', app_version: '1.0.0', os: 'mock', rust_version: '1.85.0' };

// REQ-EXP-002/003: 全量数据备份与恢复
case 'export_backup':
return JSON.stringify({
version: 1,
exported_at: '0',
conversations: state.conversations || [],
messages: state.messages || {},
documents: state.documents || [],
settings: {},
});

case 'import_backup':
state.mockBackupContent = args.content;
return JSON.stringify({
conversations: 0,
messages: 0,
documents: 0,
settings: 0,
});

      case 'open_data_dir':
        return null;

      // ============================================================
      // Token 用量追踪
      // ============================================================
      case 'get_conversation_cost': {
        const convId = args.conversation_id || args.conversationId;
        return {
          conversation_id: convId,
          total_prompt_tokens: 1500,
          total_completion_tokens: 800,
          total_tokens: 2300,
          token_budget: state.tokenBudget,
          exchange_count: (state.messages[convId] || []).filter((m) => m.role === 'user').length,
        };
      }

case 'set_token_budget':
    state.tokenBudget = args.budget || 0;
    return null;

// ============================================================
// Q08: 预算追踪（QM 借鉴 — LLM API 费用控制和速率限制）
// ============================================================

case 'get_budget_stats':
    return {
        daily_limit_usd: state.budgetDailyLimit || 0,
        daily_spent_usd: state.budgetSpentToday || 0.0,
        monthly_spent_usd: state.budgetSpentMonthly || 0.0,
        remaining: state.budgetDailyLimit > 0 ? Math.max(0, state.budgetDailyLimit - (state.budgetSpentToday || 0)) : Infinity
    };

case 'set_budget_limit':
    state.budgetDailyLimit = args.daily_limit_usd || 0;
    return null;

// ============================================================
// S69: Token 预算配置（S2 复盘接线）
// ============================================================

case 'get_token_budget_config':
    return {
        max_tokens: state.tokenBudgetMaxTokens || 32768,
        compaction_threshold: state.tokenBudgetThreshold || 0.8,
        recent_keep_ratio: state.tokenBudgetKeepRatio || 0.67,
        min_messages_to_compact: state.tokenBudgetMinMsgs || 3
    };

case 'set_token_budget_config':
    if (args.config) {
        state.tokenBudgetMaxTokens = args.config.max_tokens;
        state.tokenBudgetThreshold = args.config.compaction_threshold;
        state.tokenBudgetKeepRatio = args.config.recent_keep_ratio;
        state.tokenBudgetMinMsgs = args.config.min_messages_to_compact;
    }
    return null;

// ============================================================
// S70: Trace 系统命令（S2 复盘接线）
// ============================================================

case 'get_recent_traces': {
    const limit = args.limit || 20;
    const traces = state.traces || [];
    return traces.slice(-limit).reverse();
}

case 'get_trace_detail': {
    const traces = state.traces || [];
    return traces.find(function(tr) { return tr.id === args.id; }) || null;
}

case 'clear_traces':
    state.traces = [];
    return null;

case 'get_trace_count':
    return (state.traces || []).length;

// ============================================================
// RAG 评估指标（REQ-RAG-045，RAGAS 风格）
// ============================================================

case 'evaluate_rag_response':
    return [
        { metric_type: 'faithfulness', score: 0.85, details: null },
        { metric_type: 'answer_relevance', score: 0.9, details: null },
        { metric_type: 'context_precision', score: 0.75, details: null },
        { metric_type: 'keyword_overlap', score: 0.8, details: null },
    ];

case 'evaluate_rag_batch':
    return {
        sample_count: args.samples ? args.samples.length : 0,
        aggregate_metrics: [
            { metric_type: 'faithfulness', score: 0.82, details: null },
            { metric_type: 'answer_relevance', score: 0.88, details: null },
        ],
        per_sample_metrics: [],
    };

case 'get_rag_eval_settings':
    return state.ragEvalSettings || {
        enable_faithfulness: true,
        enable_answer_relevance: true,
        enable_context_precision: true,
        enable_context_recall: false,
        enable_retrieval_metrics: true,
        enable_embedding_metrics: false,
        enable_keyword_overlap: true,
    };

case 'set_rag_eval_settings':
    state.ragEvalSettings = args.settings;
    return null;

// ============================================================
// 下载管理（REQ-LLM-004 v2）
// ============================================================
      case 'pause_download':
        state.downloadStatuses[args.filename] = { status: 'paused', downloaded: 0, total: 0 };
        return null;

      case 'abort_download':
        state.downloadStatuses[args.filename] = { status: 'cancelled', downloaded: 0, total: 0 };
        return null;

      case 'get_download_status':
        return state.downloadStatuses[args.filename] || null;

      case 'list_pending_downloads':
        return Object.entries(state.downloadStatuses).map(([filename, info]) => ({ filename, ...info }));

      case 'cleanup_partial_downloads':
        state.downloadStatuses = {};
        return 0;

      case 'scan_download_recovery':
        return [];

      // ============================================================
      // 本地 LLM Pro 命令
      // ============================================================
      case 'get_local_llm_device_kind':
        return { kind: 'cpu', name: 'Mock CPU Device', vram_mb: 0 };

      case 'set_paged_attn':
        state.pagedAttn = args.enabled;
        return null;

      case 'set_sampling_params':
        state.samplingParams = { ...state.samplingParams, ...args.params };
        return null;

      case 'set_kernel_mode':
        state.kernelMode = args.mode;
        return null;

      case 'get_kernel_mode':
        return state.kernelMode;

      case 'save_kv_cache':
        return null;

      case 'load_kv_cache':
        return false;

      case 'clear_kv_cache':
        return null;

      case 'get_kv_cache_status':
        return { enabled: state.kvCacheEnabled, cache_dir: '/mock/kv_cache', file_count: 0, total_size_bytes: 0 };

      case 'set_kv_cache_enabled':
        state.kvCacheEnabled = args.enabled;
        return null;

      case 'set_embedder_model':
        state.embeddingModel = args.model;
        return null;

      // S4 v1.6: close-to-tray 设置（REQ-WIN-003）
      case 'get_close_to_tray':
        return state.closeToTray === true;

      case 'export_error_logs':
        return state.errorLogs || '';

      // v1.13: 开机自启（REQ-WIN-004）
      case 'get_autostart':
        return state.autostart === true;

      case 'check_for_updates':
        return state.updateInfo || { has_update: false, current_version: '2.2.0', latest_version: '2.2.0', release_notes: null, download_url: null };

      case 'get_update_check_config':
        return { auto_check: state.updateAutoCheck !== false, last_check: state.updateLastCheck || 0 };

      case 'get_import_history':
        return (state.importLogs || []).filter(
          (l) => !args.resultFilter || l.result === args.resultFilter
        );

      case 'clear_import_history':
        state.importLogs = [];
        return null;

      // 智能模式（S5 审计 P0-1）
      case 'set_smart_mode':
        state.smartModeEnabled = args.enabled;
        if (args.enabled) {
          state.hybridSearch = true;
          state.rerankEnabled = true;
          state.hydeEnabled = true;
        }
        return null;
      case 'get_smart_mode':
        return state.smartModeEnabled !== false;

      // REQ-RAG-051: 无 Key 演示模式
      case 'is_demo_mode':
        return state.demoMode === true;

      case 'exit_demo_mode':
        state.demoMode = false;
        // 清除示例文档
        state.documents = state.documents.filter((d) => !d.title.startsWith('[Demo] '));
        return null;

      case 'load_demo_documents': {
        state.demoMode = true;
        const demoDocs = [
          { title: '[Demo] EchoMind 介绍', content: 'EchoMind 是一款本地优先的智能知识库助手。', file_path: 'demo://intro', file_type: 'txt' },
          { title: '[Demo] RAG 技术概述', content: 'RAG 是检索增强生成技术。', file_path: 'demo://rag', file_type: 'txt' },
          { title: '[Demo] 隐私安全说明', content: 'EchoMind 隐私安全设计。', file_path: 'demo://privacy', file_type: 'txt' },
        ];
        for (const d of demoDocs) {
          state.documents.push({
            id: 'doc-' + Math.random().toString(36).slice(2, 10),
            title: d.title,
            file_path: d.file_path,
            file_type: d.file_type,
            content: d.content,
            created_at: Date.now(),
            chunk_count: 1,
            total_tokens: 10,
            status: 'Indexed',
          });
        }
        return null;
      }

      // 多知识库管理（REQ-WS-001/003）
      case 'create_workspace': {
        const id = 'ws-' + Math.random().toString(36).slice(2, 10);
        const ws = { id, name: args.name, created_at: Date.now() };
        state.workspaces.push(ws);
        return id;
      }

      case 'list_workspaces':
        return state.workspaces;

      case 'switch_workspace':
        state.currentWorkspaceId = args.workspaceId;
        state.workspaceSettings['workspace.current'] = args.workspaceId;
        return null;

      case 'get_current_workspace':
        return state.workspaceSettings['workspace.current'] || 'default';

      case 'rename_workspace': {
        const ws = state.workspaces.find((w) => w.id === args.id);
        if (ws) ws.name = args.name;
        return null;
      }

      case 'delete_workspace': {
        if (state.workspaces.length <= 1) {
          throw new Error('至少保留一个知识库');
        }
        const ws = state.workspaces.find((w) => w.id === args.id);
        if (!ws) throw new Error('工作空间不存在: ' + args.id);
        if (args.id === 'default') throw new Error('默认知识库不可删除');
        // 级联清理 conversations
        state.conversations = state.conversations.filter((c) => c.workspace_id !== args.id);
        // 级联清理 docs（有 workspace_id 字段的文档）
        state.docs = state.docs.filter((d) => (d.workspace_id || 'default') !== args.id);
        // 删除工作空间
        state.workspaces = state.workspaces.filter((w) => w.id !== args.id);
        // 如果删除的是当前工作空间，回退到 default
        if (state.currentWorkspaceId === args.id) {
          state.currentWorkspaceId = 'default';
          state.workspaceSettings['workspace.current'] = 'default';
        }
        return null;
      }

      case 'get_workspace_stats': {
        const docs = state.docs.filter((d) => (d.workspace_id || 'default') === args.id);
        const convs = state.conversations.filter((c) => c.workspace_id === args.id);
        return { document_count: docs.length, conversation_count: convs.length };
      }

      case 'get_workspace_quota': {
        // REQ-WS-002：配额按工作空间独立计算
        const wsId = state.workspaceSettings['workspace.current'] || 'default';
        const wsDocs = state.docs.filter((d) => (d.workspace_id || 'default') === wsId);
        const limit = state.isPro ? 0 : 50;
        return [wsDocs.length, limit];
      }

      case 'migrate_document': {
        // REQ-WS-004：文档跨知识库迁移
        const doc = state.docs.find((d) => d.id === args.docId);
        if (!doc) throw new Error('文档不存在: ' + args.docId);
        const targetWs = state.workspaces.find((w) => w.id === args.targetWorkspaceId);
        if (!targetWs) throw new Error('目标知识库不存在: ' + args.targetWorkspaceId);
        // 配额检查（免费版）
        if (!state.isPro) {
          const targetDocs = state.docs.filter((d) => (d.workspace_id || 'default') === args.targetWorkspaceId);
          if (targetDocs.length >= 50) {
            throw 'LIMIT_REACHED: 目标知识库已达免费版上限（50 个文件）';
          }
        }
        // 执行迁移（仅更新 workspace_id，不重新嵌入）
        doc.workspace_id = args.targetWorkspaceId;
        return null;
      }

      case 'export_audit_report': {
        const logs = state.auditLogs || [];
        if (args.format === 'json') {
          return JSON.stringify(logs, null, 2);
        }
        let md = '# EchoMind 安全审计报告\n\n';
        md += `**总条目数**: ${logs.length}\n\n---\n\n`;
        for (let i = 0; i < logs.length; i++) {
          const e = logs[i];
          md += `## ${i + 1}. ${e.action || 'unknown'}\n\n`;
          md += `- **时间**: ${e.timestamp}\n- **PII 检测数**: ${e.pii_count || 0}\n`;
          if (e.prev_hash) md += `- **前条哈希**: \`${e.prev_hash}\`\n`;
          if (e.curr_hash) md += `- **当前哈希**: \`${e.curr_hash}\`\n`;
          md += '\n';
        }
        return md;
      }

      // ============================================================
      // S09: 统一设置命令 update_setting / get_setting
      // ============================================================
      case 'update_setting': {
        const key = args.key;
        const value = args.value;
        const enabled = value === 'true';

        // 映射设置键到 mock state（与旧 set_xxx 命令一致）
        switch (key) {
          case 'rag.hybrid_search': state.hybridSearch = enabled; break;
          case 'rag.rerank_enabled': state.rerankEnabled = enabled; break;
          case 'rag.hyde_enabled': state.hydeEnabled = enabled; break;
          case 'rag.agent_enabled': state.agentEnabled = enabled; break;
          case 'rag.coordinator_enabled': state.coordinatorEnabled = enabled; break;
          case 'rag.sub_agent_enabled': state.subAgentEnabled = enabled; break;
          case 'vec.embedding_model': state.embeddingModel = value; break;
          case 'compression.ratio': state.compressionRatio = parseFloat(value); break;
          case 'rag.context_token_limit': state.contextTokenLimit = parseInt(value); break;
          case 'mm.vlm_enabled': state.vlmEnabled = enabled; break;
          case 'rag.progressive_injection': state.progressiveInjection = enabled; break;
          case 'rag.speculative_enabled': state.speculativeEnabled = enabled; break;
          case 'rag.quality_gate_enabled': state.qualityGateEnabled = enabled; break;
          case 'rag.graph_retriever_enabled': state.graphRetrieverEnabled = enabled; break;
          case 'memory.enabled': state.memoryEnabled = enabled; break;
          case 'rag.web_search_enabled': state.webSearchEnabled = enabled; break;
          case 'rag.contextual_retrieval': state.contextualRetrieval = enabled; break;
          case 'window.close_to_tray': state.closeToTray = enabled; break;
          case 'ui.sidebar_collapsed': state.sidebarCollapsed = enabled; break;
          case 'app.autostart': state.autostart = enabled; break;
          case 'rag.retrieval_memory_enabled': state.retrievalMemoryEnabled = enabled; break;
          case 'update.auto_check': state.updateAutoCheck = enabled; break;
          case 'vec.mirror_source': state.embeddingMirrorSource = value; break;
          default:
            throw new Error('S09 mock: 不支持的设置键: ' + key);
        }
        return null;
      }

      case 'get_setting': {
        const key = args.key;
        // 返回 mock state 中的值（与旧 get_xxx 命令一致）
        switch (key) {
          case 'rag.hybrid_search': return String(state.hybridSearch);
          case 'rag.rerank_enabled': return String(state.rerankEnabled);
          case 'rag.hyde_enabled': return String(state.hydeEnabled);
          case 'rag.agent_enabled': return String(state.agentEnabled);
          case 'rag.coordinator_enabled': return String(state.coordinatorEnabled);
          case 'rag.sub_agent_enabled': return String(state.subAgentEnabled);
          case 'vec.embedding_model': return state.embeddingModel || 'all-MiniLM-L6-v2';
          case 'compression.ratio': return String(state.compressionRatio || 1.0);
          case 'rag.context_token_limit': return String(state.contextTokenLimit || 4096);
          case 'mm.vlm_enabled': return String(state.vlmEnabled);
          case 'rag.progressive_injection': return String(state.progressiveInjection);
          case 'rag.speculative_enabled': return String(state.speculativeEnabled);
          case 'rag.quality_gate_enabled': return String(state.qualityGateEnabled);
          case 'rag.graph_retriever_enabled': return String(state.graphRetrieverEnabled);
          case 'memory.enabled': return String(state.memoryEnabled);
          case 'rag.web_search_enabled': return String(state.webSearchEnabled);
          case 'rag.contextual_retrieval': return String(state.contextualRetrieval);
          case 'window.close_to_tray': return String(state.closeToTray);
          case 'ui.sidebar_collapsed': return String(state.sidebarCollapsed);
          case 'app.autostart': return String(state.autostart);
          case 'rag.retrieval_memory_enabled': return String(state.retrievalMemoryEnabled);
          case 'update.auto_check': return String(state.updateAutoCheck !== false);
          case 'vec.mirror_source': return state.embeddingMirrorSource || '';
          default: return '';
        }
      }

      // ============================================================
      // S96: 嵌入模型对比评估（REQ-VEC-018，开发者工具门控）
      // ============================================================
      case 'run_embed_comparison': {
        const req = args.request || {};
        const modelNames = req.model_names || ['all-MiniLM-L6-v2'];
        const topK = req.top_k || 5;

        // mock：返回确定性评估结果（不同模型略有差异）
        return modelNames.map((name, i) => {
          // 不同模型返回不同分数（模拟检索质量差异）
          const seed = (i + 1) * 0.15;
          return {
            model_name: name,
            dim: name === 'bge-m3' ? 1024 : name === 'bge-base-en-v1.5' ? 768 : name === 'bge-small-zh-v1.5' ? 512 : 384,
            metrics: {
              hit_rate: Math.min(1.0, 0.6 + seed),
              mrr: Math.min(1.0, 0.5 + seed * 0.8),
              ndcg: Math.min(1.0, 0.55 + seed * 0.7),
            },
            sample_count: 6,
          };
        });
      }

      // ============================================================
      // S97: MCP 协议适配器（REQ-ARCH-016）
      // ============================================================
      case 'add_mcp_server': {
        state.mcpServers = state.mcpServers || [];
        const config = args.config;
        state.mcpServers.push({ ...config, _status: 'disconnected', _tools: [] });
        return null;
      }
      case 'remove_mcp_server': {
        state.mcpServers = (state.mcpServers || []).filter(s => s.id !== args.id);
        return null;
      }
      case 'list_mcp_servers': {
        return (state.mcpServers || []).map(s => ({
          config: { id: s.id, name: s.name, transport: s.transport, enabled: s.enabled, command: s.command, args: s.args || [], env: [], url: s.url, headers: [] },
          status: s._status || 'disconnected',
          error_message: s._error || null,
          tool_count: (s._tools || []).length,
        }));
      }
      case 'toggle_mcp_server': {
        state.mcpServers = (state.mcpServers || []).map(s => s.id === args.id ? { ...s, enabled: args.enabled } : s);
        return null;
      }
      case 'get_mcp_tools': {
        const all = (state.mcpServers || []).filter(s => s.enabled).flatMap(s => (s._tools || []).map(t => ({ ...t, server_id: s.id, server_name: s.name })));
        return all;
      }
      case 'call_mcp_tool': {
        return {
          tool_name: args.toolName,
          server_id: args.serverId,
          success: true,
          content: 'Mock tool result',
          is_error: false,
        };
      }

      default:
        throw new Error('未实现的 mock 命令: ' + cmd);
    }
  }

  window.__TAURI__ = {
    core: { invoke },
    event: {
      listen: (name, cb) => {
        (state.listeners[name] ||= []).push(cb);
        return Promise.resolve(() => {
          const arr = state.listeners[name];
          if (arr) {
            const idx = arr.indexOf(cb);
            if (idx !== -1) arr.splice(idx, 1);
          }
        });
      },
      emit: (name, payload) => { emit(name, payload); },
    },
    dialog: { open: async () => ['/mock/echomind-e2e.md'] },
    opener: { openUrl: async () => {} },
    window: {
      getCurrentWindow: () => ({
        isFullscreen: async () => false,
        onResized: () => {},
      }),
    },
  };

  /** 测试控制接口：允许测试用例在运行时修改 mock 行为。 */
  window.__mock = {
    state,
    /** 设置下次 chat 返回空上下文（REQ-RAG-003）。 */
    setNextChatEmpty: () => { state.nextChatEmpty = true; },
    /** 设置下次 chat 使用自定义 token 序列。 */
    setCustomTokens: (tokens) => { state.customTokens = tokens; },
    /** 设置下次 test_llm_connection 失败。 */
    setConnectionFail: () => { state.connectionFail = true; },
    /** 设置文件内容（用于去重测试）。 */
    setFileContent: (path, content) => { state.fileContents[path] = content; },
    /** 获取 XSS token 序列（用于安全测试）。 */
    xssTokens: () => [...XSS_TOKENS],
    /** 获取 Mermaid flowchart token 序列（用于 TC-VIZ-001/002/004）。 */
    mermaidTokens: () => [...MERMAID_TOKENS],
    /** 获取 Mermaid 语法错误 token 序列（用于 TC-VIZ-003）。 */
    mermaidInvalidTokens: () => [...MERMAID_INVALID_TOKENS],
    /** 获取 Mermaid XSS 注入 token 序列（用于 TC-VIZ-006）。 */
    mermaidXssTokens: () => [...MERMAID_XSS_TOKENS],
    /** 获取 KaTeX 行内公式 token 序列（用于 TC-VIZ-007）。 */
    katexInlineTokens: () => [...KATEX_INLINE_TOKENS],
    /** 获取 KaTeX 块级公式 token 序列（用于 TC-VIZ-008）。 */
    katexBlockTokens: () => [...KATEX_BLOCK_TOKENS],
    /** 获取 KaTeX 语法错误 token 序列（用于 TC-VIZ-009）。 */
    katexInvalidTokens: () => [...KATEX_INVALID_TOKENS],
    /** 获取 KaTeX XSS 注入 token 序列（用于 TC-VIZ-010 安全测试）。 */
    katexXssTokens: () => [...KATEX_XSS_TOKENS],
    /** 获取 Chart.js 表格数据 token 序列（用于 TC-VIZ-007 数据图表）。 */
    chartTableTokens: () => [...CHART_TABLE_TOKENS],
    /** 获取 Mermaid 甘特图 token 序列。 */
    mermaidGanttTokens: () => [...MERMAID_GANTT_TOKENS],
    /** 获取 Mermaid 序列图 token 序列。 */
    mermaidSequenceTokens: () => [...MERMAID_SEQUENCE_TOKENS],
    /** 获取 Mermaid 类图 token 序列。 */
    mermaidClassTokens: () => [...MERMAID_CLASS_TOKENS],
    /** 获取 Mermaid 饼图 token 序列。 */
    mermaidPieTokens: () => [...MERMAID_PIE_TOKENS],
    /** 获取 KaTeX 化学方程式 token 序列。 */
    katexChemTokens: () => [...KATEX_CHEM_TOKENS],
    /** 获取简单表格 token 序列。 */
    tableTokens: () => [...TABLE_TOKENS],
    /** 获取折线图数据 token 序列。 */
    lineChartTokens: () => [...LINE_CHART_TOKENS],
    /** 获取饼图数据 token 序列。 */
    pieChartTokens: () => [...PIE_CHART_TOKENS],
    /** 获取审计报告 token 序列（用于 E2E-AUDIT-001~005）。 */
    auditTokens: () => [...AUDIT_TOKENS],
    /** 获取 Agent RAG 步骤序列（用于 E2E-AGENT-003~009）。 */
    agentSteps: () => [
      { step_type: 'thought', content: '分析用户问题，确定检索策略。', tool: null, input: null, iteration: 1 },
      { step_type: 'action', content: '检索知识库', tool: 'vector_search', input: '', iteration: 1 },
      { step_type: 'observation', content: '找到相关片段，相似度 > 0.8。', tool: null, input: null, iteration: 1 },
      { step_type: 'thought', content: '已获取足够上下文，生成最终答案。', tool: null, input: null, iteration: 2 },
    ],
    /** 设置加密密码（用于安全测试前置条件）。 */
    setEncryptionPassword: (pwd) => { state.encryptionPassword = pwd; },
    /** 设置下次 chat 返回指定错误（REQ-ERR-001）。 */
    setChatError: (msg) => { state.chatError = msg; },
    /** 清除 chat 错误模式。 */
    clearChatError: () => { state.chatError = null; },
    /** 设置下次 chat 挂起（模拟后端永久阻塞，V1 修复测试）。 */
    setChatHang: () => { state.chatHang = true; },
    /** 设置下次 chat 模拟 embedder 初始化失败（V1 修复测试）。 */
    setChatEmbedderError: () => { state.chatEmbedderError = true; },
    /** 设置 embedder 下载状态（'ready' | 'needs_download' | 'partial_download'） */
    setEmbedderStatus: (status) => { state.embedderStatus = status; },
    /** 设置下次 init_embedder 模拟下载失败 */
    setEmbedderDownloadFail: () => { state.embedderDownloadFail = true; },
    /** 设置下次 init_embedder 模拟下载挂起（无进度事件） */
    setEmbedderDownloadHang: () => { state.embedderDownloadHang = true; },
    /** 设置下次 init_embedder 模拟慢速连接（2s 后才发首个进度事件） */
    setEmbedderSlowConnect: () => { state.embedderSlowConnect = true; },
    /** 设置下次 init_embedder 模拟多文件下载 */
    setEmbedderMultiFile: () => { state.embedderMultiFile = true; },
    /** 重置全部状态（每个测试 beforeEach 调用）。 */
    reset: () => {
      state.configured = false;
      state.isPro = false;
      state.docs = [];
      state.hashIndex = {};
      state.conversations = [];
      state.messages = {};
      state.listeners = {};
      state.aborted = false;
      state.nextChatEmpty = false;
      state.customTokens = null;
      state.connectionFail = false;
      state.fileContents = {};
      state.vlmEnabled = false;
      state.auditAborted = false;
      state.importCancelled = false;
      state.chatError = null;
      state.chatHang = false;
      state.chatEmbedderError = false;
      state.embedderStatus = 'ready';
      state.embedderDownloadFail = false;
      state.embedderDownloadHang = false;
      state.embedderSlowConnect = false;
      state.embedderMultiFile = false;
      state.modelCacheInfo = { models: [{ name: 'all-MiniLM-L6-v2', size_bytes: 31457280 }], total_size_bytes: 31457280 };
      state.hybridSearch = false;
      state.llmMode = 'remote';
      state.localModel = '';
      state.localModels = [
        { filename: 'qwen2.5-3b-instruct-q4_k_m.gguf', path: '/mock/models/llm/qwen2.5-3b-instruct-q4_k_m.gguf', size_bytes: 2000000000, architecture: 'qwen2.5', param_size: '3B', quantization: 'Q4_K_M' },
      ];
      // 安全防御状态重置
      state.securityState = 'Unencrypted';
      state.lockReason = null;
      state.autoLockConfig = { enabled: true, timeout_secs: 180, lock_on_sleep: true };
      state.clipboardConfig = { enabled: true, clear_after_secs: 30 };
      state.authFailures = 0;
      state.remainingAttempts = 5;
      state.isLocked = false;
      state.remainingLockSeconds = 0;
      state.panicWipeEnabled = false;
      state.encryptionPassword = null;
      state.piiDetectionEnabled = false;
      state.auditLogs = [];
      state.lastActivity = Date.now();
      // 导出状态重置
      state.lastExportPath = null;
      state.lastExportContent = null;
      // 文件监听状态重置
      state.watchedFolders = [];
      // 高级 RAG 状态重置
      state.rerankEnabled = false;
      state.hydeEnabled = false;
      state.agentEnabled = false;
      state.embeddingModel = 'all-MiniLM-L6-v2';
      // i18n 状态重置
      state.locale = 'zh-CN';
      // 领域分类重置
      state.docDomains = {};
      // 工作流重置
      state.workflowTemplates = [];
      // 记忆系统重置
      state.memoryEnabled = false;
      state.memories = [];
      // AutoDream 重置
      state.dreamAborted = false;
      state.dreamSuggestions = [];
      // 符号引擎重置
      state.symbolIndexBuilt = false;
      // 性能优化重置
      state.compressionRatio = 1.0;
      state.cacheSettings = { enabled: true, ttl_secs: 86400, semantic_threshold: 0.92, privacy_mode: false };
      state.cacheStats = { enabled: true, exact_hits: 5, semantic_hits: 3, retrieval_hits: 8, total_queries: 50, cache_size_entries: 16, estimated_token_saved: 12000 };
      state.retrievalMemoryEnabled = false;
      state.retrievalMemoryStats = [];
state.subAgentEnabled = false;
state.webSearchEnabled = false;
state.ragParams = { top_k: 8, score_threshold: 0.0, chunk_expansion_enabled: true, chunk_expansion_window: 1 };
state.generationParams = { temperature: 0.7, max_tokens: 4096, top_p: 1.0 };
state.docSortBy = null;
state.docSortOrder = null;
state.coordinatorEnabled = false;
      state.progressiveInjection = false;
      state.speculativeEnabled = false;
      // 可观测性重置
      state.logLevel = 'info';
      // 下载管理重置
      state.downloadStatuses = {};
      // 本地 LLM Pro 重置
      state.pagedAttn = false;
      state.samplingParams = { temperature: null, top_p: null, top_k: null, max_tokens: null, frequency_penalty: null, presence_penalty: null };
      state.kernelMode = 'mistral_rs';
      state.kvCacheEnabled = false;
      // 书签重置
      state._bookmarks = [];
      // Token 用量重置
      state.tokenBudget = 0;
    },
    /** 模拟拖拽事件触发。 */
    simulateDragEnter: () => {
      (state.listeners['tauri://drag-enter'] || []).forEach((cb) => cb({ payload: {} }));
    },
    simulateDragLeave: () => {
      (state.listeners['tauri://drag-leave'] || []).forEach((cb) => cb({ payload: {} }));
    },
    simulateDragDrop: (paths) => {
      (state.listeners['tauri://drag-drop'] || []).forEach((cb) => cb({ payload: { paths } }));
    },
  };
  window.__state = state;

  // ============================================================
  // Mock 控制接口（根因 V2 修复：消除 Mock 驱动盲区）
  // 测试通过 window.__ECHOMIND_MOCK_CONTROL__ 控制 mock 状态，
  // 实现"空状态→有文档→聊天"等状态转换测试。
  // ============================================================
  window.__ECHOMIND_MOCK_CONTROL__ = {
    /** 设置文档列表（替换全部） */
    setDocs(docs) {
      state.docs = docs || [];
    },
    /** 清空所有文档（模拟空知识库） */
    clearDocs() {
      state.docs = [];
      state.hashIndex = {};
      state.fileContents = {};
    },
    /** 添加一个文档（模拟导入完成） */
    addDoc(path, status) {
      const name = basename(path);
      const hash = mockHash(path);
      const docId = 'doc-' + hash;
      // 去重：已存在则跳过
      if (state.docs.find((d) => d.id === docId)) return;
      state.docs.push({
        id: docId,
        file_path: 'a'.repeat(32) + '-' + name,
        status: status || 'Indexed',
        content_hash: hash,
        chunk_count: 3,
        created_at: Date.now(),
        workspace_id: state.currentWorkspaceId,
      });
    },
    /** 设置下次 chat 的错误消息 */
    setChatError(msg) {
      state.chatError = msg;
    },
    /** 设置下次 chat 挂起（不返回） */
    setChatHang(hang) {
      state.chatHang = hang;
    },
    /** 设置下次 chat embedder 错误 */
    setChatEmbedderError(err) {
      state.chatEmbedderError = err;
    },
    /** 设置下次 chat 返回空上下文 */
    setChatEmpty(empty) {
      state.nextChatEmpty = empty;
    },
    /** 设置自定义 token 序列 */
    setCustomTokens(tokens) {
      state.customTokens = tokens;
    },
    /** 设置连接测试失败 */
    setConnectionFail(fail) {
      state.connectionFail = fail;
    },
    /** 重置所有 mock 状态到默认值 */
    reset() {
      state.docs = [];
      state.hashIndex = {};
      state.fileContents = {};
      state.conversations = [];
      state.messages = {};
      state.aborted = false;
      state.nextChatEmpty = false;
      state.customTokens = null;
      state.connectionFail = false;
      state.chatError = null;
      state.chatHang = false;
      state.chatEmbedderError = false;
      state.importCancelled = false;
      state.auditAborted = false;
      state.isPro = false;
      state.vlmEnabled = false;
      state.hybridSearch = false;
      state.llmMode = 'remote';
      state.localModel = '';
      // S34-S43 新增状态重置
      state.workflowTemplates = [];
      state.memoryEnabled = false;
      state.memories = [];
      state.dreamAborted = false;
      state.dreamSuggestions = [];
      state.symbolIndexBuilt = false;
      state.compressionRatio = 1.0;
      state.retrievalMemoryEnabled = false;
      state.retrievalMemoryStats = [];
state.subAgentEnabled = false;
state.webSearchEnabled = false;
state.ragParams = { top_k: 8, score_threshold: 0.0, chunk_expansion_enabled: true, chunk_expansion_window: 1 };
state.generationParams = { temperature: 0.7, max_tokens: 4096, top_p: 1.0 };
state.docSortBy = null;
state.docSortOrder = null;
state.coordinatorEnabled = false;
      state.progressiveInjection = false;
      state.speculativeEnabled = false;
      state.logLevel = 'info';
      state.downloadStatuses = {};
      state.tokenBudget = 0;
      state.pendingInputs = [];
  state.sessionTodos = [];
  state.burstBuffer = [];
  state.shadowScreenStats = { total: 0, agree: 0, disagree: 0, unavailable: 0 };
    state.mcpServers = [];
    },
  };
})();
