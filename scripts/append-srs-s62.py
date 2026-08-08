#!/usr/bin/env python3
"""Append S62 SRS revision entry."""

entry = '| v6.2.0 | 2026-08-07 | **v1.4 S62 — 对话全文搜索实现（REQ-RAG-040 Implemented）**：(1) **models/lib.rs** `MessageSearchResult` 结构体（message_id/conversation_id/conversation_title/role/content/score/created_at）；(2) **core/lib.rs** `Storage` trait 新增 `search_messages()` 默认空操作方法；(3) **sqlite_storage.rs** 新增 `SCHEMA_MESSAGES_FTS` FTS5 虚拟表（trigram 分词器，与 chunks_fts 一致）+ 3 触发器（INSERT/DELETE/UPDATE 自动同步 messages 表）+ `backfill_messages_fts_if_needed()` 迁移函数 + `search_messages()` 实现（短查询 <3 字符回退 LIKE、FTS5 BM25 排序、JOIN conversations 获取标题）+ 修复 `MessageSearchResult` import 缺失；(4) **conversation.rs** 新增 `search_conversations` IPC 命令 + `*_inner` 函数；(5) **mod.rs** 新增 `MessageSearchResult` 到 echomind_models import；(6) **lib.rs** 注册 1 命令（152→153）；(7) **前端 sidebar.js** 搜索弹框新增「会话/对话」模式切换按钮（`_searchMode` 变量、`_setSearchMode()` 切换、`_renderMessageSearchPage()` 对话搜索结果渲染含角色标签 Q/A + 会话标题 + 内容摘要）；(8) **index.html** 搜索弹框 header 新增模式切换 toggle 按钮；(9) **i18n** 新增 4 键 search_mode_title/search_mode_content/search_messages/search_messages_hint（中英文）；(10) **tauri-stub.js** 新增 search_conversations mock；(11) **测试** 4 TDD 存储层测试 TC-RAG-SEARCH-001~004（基本搜索/BM25排序/空查询安全/中文搜索）+ 5 E2E 测试 TC-RAG-SEARCH-005a~e。零新增依赖（复用 rusqlite FTS5）。 | Dev |\n'

with open('docs/SRS_v1.0.md', 'a', encoding='utf-8') as f:
    f.write(entry)
print('OK')
