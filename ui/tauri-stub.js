// 契约级 Mock：window.__TAURI__（与 SRS 冻结命令/事件一一对应，仅供 L3-lite 桥接测试）。
// 覆盖命令（70+ 全量，含前端 securityApi 别名）：
//   基础: get_settings / update_llm_config / test_llm_connection / import_files / get_file_sizes /
//         get_file_size_limits / get_documents / delete_document / retry_index /
//         create_conversation / get_conversations / get_conversations_paginated /
//         get_messages / get_messages_paginated / delete_conversation / chat / abort_chat /
//         abort_import / activate_pro / deactivate_pro / get_pro_status / set_vlm_enabled /
//         set_hybrid_search / set_context_token_limit
//   LLM:  list_local_models / get_recommended_models / download_model / delete_model /
//         set_llm_mode / get_llm_mode / set_local_model
//   审计: audit_document / abort_audit
//   嵌入: init_embedder / get_model_cache_info / clear_model_cache
//   RAG:  set_rerank_enabled / set_hyde_enabled / set_agent_enabled / set_embedding_model /
//         reclassify_document
//   导出: export_conversation_markdown / save_text_file
//   同步: add_watched_folder / remove_watched_folder / get_watched_folders
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
// security-state-changed / agent_step / sync_progress / model_load_progress。
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
    configured: false,
    isPro: false,
    docs: [],
    /** 文件路径 → 内容哈希（去重用） */
    hashIndex: {},
    conversations: [],
    messages: {},
    listeners: {},
    aborted: false,
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
    /** 模型缓存信息（REQ-VEC-008） */
    modelCacheInfo: { models: [{ name: 'all-MiniLM-L6-v2', size_bytes: 31457280 }], total_size_bytes: 31457280 },
    /** 混合检索开关（REQ-RAG-010） */
    hybridSearch: false,
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
? { has_llm_config: true, base_url: 'http://mock.local', model: 'mock-llm', api_key_masked: '****-e2e', proxy_url: '', vlm_enabled: state.vlmEnabled, context_token_limit: state.contextTokenLimit, llm_mode: state.llmMode, local_model: state.localModel }
: { has_llm_config: false, base_url: '', model: '', api_key_masked: '', proxy_url: '', vlm_enabled: state.vlmEnabled, context_token_limit: state.contextTokenLimit, llm_mode: state.llmMode, local_model: state.localModel };

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
        const WHITELIST = ['md', 'txt', 'pdf', 'docx', 'html', 'htm'];
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
            throw `不支持的格式：.${ext}，当前支持 .md / .txt / .pdf / .docx / .html`;
          }

          // PDF 付费门
          if (ext === 'pdf' && !state.isPro) {
            throw 'PRO_REQUIRED: PDF 导入为 Pro 版功能，请升级后重试';
          }

          // 配额检查
          if (!state.isPro && state.docs.length >= FREE_LIMIT) {
            throw 'LIMIT_REACHED: 免费版上限 50 个文件，请升级 Pro 版';
          }

          // 内容去重（基于路径确定性哈希模拟）
          const hash = mockHash(p + (state.fileContents[p] || ''));
          if (state.hashIndex[hash]) {
            emit('doc-status-changed', { status: 'done', message: `内容已存在，跳过导入：${name}` });
            completed++;
            emit('import-progress', { completed, total, current_file: name, cancelled: false });
            // 文件间延迟，确保进度条可见（REQ-ING-006 测试需要）
            await new Promise((r) => setTimeout(r, 50));
            continue;
          }
          state.hashIndex[hash] = p;

          const doc = {
            id: 'doc-' + hash,
            file_path: '/mock/data/documents/' + 'a'.repeat(32) + '-' + name,
            file_hash: hash,
            status: 'Pending',
            created_at: Math.floor(Date.now() / 1000),
          };
          state.docs.push(doc);
          emit('doc-status-changed', { status: 'indexing', message: `正在索引：${name}` });
          // 文件处理延迟，确保进度条可见（REQ-ING-006 测试需要）
          await new Promise((r) => setTimeout(r, 80));
          doc.status = 'Indexed';
          emit('doc-status-changed', { status: 'done', message: `索引完成：${name}（3 向量）` });
          names.push(name);
          completed++;
          // 导入进度事件（REQ-ING-006）
          emit('import-progress', { completed, total, current_file: name, cancelled: false });
          // 文件间延迟，确保进度条可见（REQ-ING-006 测试需要）
          await new Promise((r) => setTimeout(r, 50));
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

case 'get_documents':
        return state.docs;

      case 'delete_document':
        state.docs = state.docs.filter((d) => d.id !== args.id);
        return null;

      case 'set_vlm_enabled':
        state.vlmEnabled = args.enabled;
        return null;

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
        });
        state.messages[id] = [];
        return id;
      }

      case 'get_conversations':
        return state.conversations;

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

      case 'abort_chat':
        state.aborted = true;
        return null;

      case 'abort_import':
        state.importCancelled = true;
        return null;

      case 'retry_index': {
        const doc = state.docs.find((d) => d.id === args.id);
        if (!doc) throw '文档不存在';
        doc.status = 'Processing';
        emit('doc-status-changed', { status: 'indexing', message: `正在重试索引：${basename(doc.file_path)}` });
        (async () => {
          await new Promise((r) => setTimeout(r, 300));
          doc.status = 'Indexed';
          emit('doc-status-changed', { status: 'done', message: `索引完成：${basename(doc.file_path)}（3 向量）` });
        })();
        return null;
      }

      case 'deactivate_pro':
        state.isPro = false;
        return null;

      case 'set_hybrid_search':
        state.hybridSearch = args.enabled;
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
            await new Promise((r) => setTimeout(r, 100));
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
          await new Promise((r) => setTimeout(r, 150));
          emit('audit_phase', { phase: 'comparing', message: '正在比对矛盾…' });
          await new Promise((r) => setTimeout(r, 150));
          emit('audit_phase', { phase: 'reporting', message: '正在生成报告…' });
          await new Promise((r) => setTimeout(r, 100));

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
            await new Promise((r) => setTimeout(r, 300));
          }
          emit('chat_done', null);
        })();
        return null;
      }

      case 'abort_audit':
        state.auditAborted = true;
        return null;

      case 'init_embedder': {
        (async () => {
          // 模拟模型下载进度事件（REQ-VEC-008）
          emit('model_download_progress', { downloading: { current: 5242880, total: 31457280, file_name: 'model.onnx' } });
          await new Promise((r) => setTimeout(r, 100));
          emit('model_download_progress', { downloading: { current: 15728640, total: 31457280, file_name: 'model.onnx' } });
          await new Promise((r) => setTimeout(r, 100));
          emit('model_download_progress', { downloading: { current: 31457280, total: 31457280, file_name: 'model.onnx' } });
          await new Promise((r) => setTimeout(r, 100));
          emit('model_download_progress', { loading: true });
          await new Promise((r) => setTimeout(r, 100));
          emit('model_download_progress', { done: true });
        })();
        return null;
      }

      case 'get_model_cache_info':
        return state.modelCacheInfo;

      case 'clear_model_cache': {
        const freed = state.modelCacheInfo.total_size_bytes;
        state.modelCacheInfo = { models: [], total_size_bytes: 0 };
        return freed;
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
            await new Promise((r) => setTimeout(r, 50));
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
          await new Promise((r) => setTimeout(r, 0));

          // 本地 LLM 模式：模拟模型加载进度事件（REQ-LLM-003 AC-4）
          if (state.llmMode === 'local') {
            const modelName = state.localModel || 'mock-model.gguf';
            emit('model_load_progress', { model: modelName, status: 'loading' });
            await new Promise((r) => setTimeout(r, 200));
            emit('model_load_progress', { model: modelName, status: 'ready' });
            await new Promise((r) => setTimeout(r, 50));
          }

          if (isEmpty) {
            // 空上下文模式：不调用 LLM，直接返回固定提示
            emit('chat_phase', { phase: 'generating', message: '正在生成回答…' });
            await new Promise((r) => setTimeout(r, 50));
            assistantContent = '知识库中未找到相关内容。请尝试换个问题，或先导入相关文档。';
            emit('chat_token', assistantContent);
            // 给 requestAnimationFrame 时间渲染
            await new Promise((r) => setTimeout(r, 50));
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
              await new Promise((r) => setTimeout(r, 100));
            }
          } else {
            // 标准 RAG 模式：三阶段 chat_phase 推送（REQ-RAG-001 扩展 / REQ-NFR-006-AC-2）
            // 时序修复：增加各阶段间隔（100ms / 150ms），确保 UI 有足够时间渲染思考指示器文案
            emit('chat_phase', { phase: 'preparing', message: '初始化向量化引擎…' });
            await new Promise((r) => setTimeout(r, 100));
            emit('chat_phase', { phase: 'retrieving', message: '检索知识库…' });
            await new Promise((r) => setTimeout(r, 150));
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
            await new Promise((r) => setTimeout(r, 200));
          }
          // 正常结束持久化助手消息
          if (convId && state.messages[convId]) {
            state.messages[convId].push({ role: 'assistant', content: assistantContent, sources });
          }
          emit('chat_done', null);
        })();
        return null;
      }

      case 'set_context_token_limit':
        const limit = args.limit;
        if (limit >= 2048 && limit <= 32768) {
          state.contextTokenLimit = limit;
          return null;
        }
        throw new Error('上下文 token 限制范围为 2048-32768');

      // ============================================================
      // 高级 RAG 功能（REQ-RAG-020~022, REQ-VEC-012~013）
      // ============================================================
      case 'set_rerank_enabled':
        state.rerankEnabled = args.enabled;
        return null;

      case 'set_hyde_enabled':
        state.hydeEnabled = args.enabled;
        return null;

      case 'set_agent_enabled':
        state.agentEnabled = args.enabled;
        return null;

      case 'set_embedding_model':
        state.embeddingModel = args.model;
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

      // ============================================================
      // 文件监听 + 增量同步（REQ-SYNC-001~003）
      // ============================================================
      case 'add_watched_folder': {
        if (!state.watchedFolders.includes(args.path)) {
          state.watchedFolders.push(args.path);
        }
        // 模拟同步进度事件（REQ-SYNC-002 AC-7）
        (async () => {
          await new Promise((r) => setTimeout(r, 100));
          emit('sync_progress', { phase: 'syncing', message: '正在扫描文件夹…', folder: args.path });
          await new Promise((r) => setTimeout(r, 200));
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

      case 'set_auto_lock_timeout':
        state.autoLockConfig = {
          ...state.autoLockConfig,
          timeout_secs: args.seconds ?? args.timeoutSecs ?? 180,
        };
        return null;

      case 'redact_pii': {
        const text = args.text || '';
        let redactedText = text;
        const patterns = [
          { re: /[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}/g, tag: '[REDACTED-EMAIL]' },
          { re: /1[3-9]\d{9}/g, tag: '[REDACTED-PHONE]' },
          { re: /\b(?:\d{1,3}\.){3}\d{1,3}\b/g, tag: '[REDACTED-IP]' },
          { re: /\d{17}[\dXx]/g, tag: '[REDACTED-IDCARD]' },
          { re: /6\d{15,18}/g, tag: '[REDACTED-BANKCARD]' },
          { re: /\b\d{3}-\d{2}-\d{4}\b/g, tag: '[REDACTED-SSN]' },
          { re: /\b[Gg]\d{8}\b/g, tag: '[REDACTED-PASSPORT]' },
          { re: /\+\d{1,3}-\d{6,14}/g, tag: '[REDACTED-INTL_PHONE]' },
        ];
        for (const { re, tag } of patterns) {
          redactedText = redactedText.replace(re, tag);
        }
        return redactedText;
      }

      case 'verify_audit_chain':
        return { valid: true, count: state.auditLogs.length, broken_at: null };

      case 'set_pii_detection_enabled':
        state.piiDetectionEnabled = args.enabled;
        return null;

      case 'clear_clipboard':
        return null;

      case 'set_clipboard_clear_timeout':
        state.clipboardConfig = {
          ...state.clipboardConfig,
          clear_after_secs: args.seconds ?? args.clearAfterSecs ?? 30,
        };
        return null;

      case 'check_security_status':
        return {
          state: state.securityState,
          color: state.securityState === 'Locked' ? '#ef4444' : state.securityState === 'EncryptedUnlocked' ? '#22c55e' : '#f59e0b',
          is_locked: state.isLocked,
          remaining_attempts: state.remainingAttempts,
          remaining_lock_seconds: state.remainingLockSeconds,
        };

      // ============================================================
      // 国际化（REQ-I18N-001）
      // ============================================================
      case 'get_locale':
        return state.locale;

      case 'set_locale':
        state.locale = args.locale;
        return null;

      default:
        throw new Error('未实现的 mock 命令: ' + cmd);
    }
  }

  window.__TAURI__ = {
    core: { invoke },
    event: {
      listen: (name, cb) => {
        (state.listeners[name] ||= []).push(cb);
        return Promise.resolve(() => {});
      },
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
    },
  };
})();
