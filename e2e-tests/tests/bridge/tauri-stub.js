// 契约级 Mock：window.__TAURI__（与 SRS 冻结命令/事件一一对应，仅供 L3-lite 桥接测试）。
// 覆盖命令：get_settings / update_llm_config / test_llm_connection / import_files /
// get_documents / delete_document / create_conversation / get_conversations /
// get_messages / delete_conversation / chat / abort_chat / activate_pro / get_pro_status。
// 覆盖事件：chat_token / chat_sources / chat_error / chat_done / doc-status-changed。
(() => {
  const state = {
    configured: false,
    isPro: false,
    docs: [],
    conversations: [],
    messages: {},
    listeners: {},
    aborted: false,
  };

  const emit = (name, payload) => {
    (state.listeners[name] || []).forEach((cb) => cb({ payload }));
  };

  const TOKENS = [
    '好的，', '这是', '流式', '回答', '：', '\n\n```rust\n', 'fn main() {\n',
    '    println!("hi");\n', '}\n', '```\n', '正在', '继续', '输出', '更多', '内容', '……',
  ];

  async function invoke(cmd, args = {}) {
    switch (cmd) {
      case 'get_settings':
        return state.configured
          ? { has_llm_config: true, base_url: 'http://mock.local', model: 'mock-llm', api_key_masked: '****-e2e' }
          : { has_llm_config: false, base_url: '', model: '', api_key_masked: '' };
      case 'update_llm_config':
        state.configured = true;
        return null;
      case 'test_llm_connection':
        return '连接成功：mock-llm 响应正常 (HTTP 200)';
      case 'import_files': {
        const names = [];
        // 模拟 REQ-LIC-002 门控：免费版导入 PDF 返回 PRO_REQUIRED
        for (const p of args.paths) {
          const name = p.split('/').pop();
          if (name.endsWith('.pdf') && !state.isPro) {
            throw 'PRO_REQUIRED: PDF 导入为 Pro 版功能，请升级后重试';
          }
          const doc = {
            id: 'doc-' + name,
            // 与真实后端命名规则一致：{32 位 md5}-{原始文件名}
            file_path: '/mock/data/documents/' + 'a'.repeat(32) + '-' + name,
            file_hash: 'mockhash',
            status: 'Pending',
            created_at: Math.floor(Date.now() / 1000),
          };
          state.docs.push(doc);
          emit('doc-status-changed', { status: 'indexing', message: `正在索引：${name}` });
          doc.status = 'Indexed';
          emit('doc-status-changed', { status: 'done', message: `索引完成：${name}（3 向量）` });
          names.push(name);
        }
        return names;
      }
      case 'get_documents':
        return state.docs;
      case 'delete_document':
        state.docs = state.docs.filter((d) => d.id !== args.id);
        return null;
      case 'get_pro_status':
        return state.isPro;
      case 'activate_pro':
        // Mock：任何非空 Key 均视为有效（真实后端使用 Ed25519 校验）
        if (args.licenseKey && args.licenseKey.trim()) {
          state.isPro = true;
          return true;
        }
        throw 'License 激活失败：License 格式错误：应为 payload-signature';
      case 'create_conversation': {
        const id = 'conv-' + state.conversations.length;
        state.conversations.unshift({
          id,
          workspace_id: 'default',
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
      case 'delete_conversation':
        state.conversations = state.conversations.filter((c) => c.id !== args.id);
        return null;
      case 'abort_chat':
        state.aborted = true;
        return null;
      case 'chat': {
        state.aborted = false;
        (async () => {
          emit('chat_sources', [
            {
              chunk: { id: 'c1', doc_id: 'doc-e2e', content: 'EchoMind 支持 md、txt、pdf 格式。', token_count: 9, sequence: 0 },
              score: 0.91,
              doc_name: 'echomind-e2e.md',
            },
          ]);
          for (const t of TOKENS) {
            if (state.aborted) {
              emit('chat_error', '⏹ 生成已中断');
              emit('chat_done', null);
              return;
            }
            emit('chat_token', t);
            await new Promise((r) => setTimeout(r, 200));
          }
          emit('chat_done', null);
        })();
        return null;
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
        return Promise.resolve(() => {});
      },
    },
    dialog: { open: async () => ['/mock/echomind-e2e.md'] },
    opener: { openUrl: async () => {} },
  };
  window.__state = state;
})();
