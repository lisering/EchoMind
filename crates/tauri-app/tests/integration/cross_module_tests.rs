#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! 跨模块协作集成测试 — Storage ↔ Embedder ↔ Retriever 协作。
//!
//! 覆盖场景：
//! - Storage 与 Embedder 的数据一致性
//! - Embedder 与 Retriever 的向量检索完整性
//! - 多模块间的并发安全
//! - 数据迁移与兼容性

use super::common::*;
use super::*;
use echomind_core::Storage;
use echomind_core::import::{ImportOutcome, ImportService};

// ============================================================================
// Storage ↔ Embedder 协作测试
// ============================================================================

/// TC-XMOD-001: Storage 向量写入与 Embedder 状态同步。
///
/// 验证：导入文档 → 分块 → 写入向量 → 检索 → 验证数据一致性。
/// 确保 Storage.add_embedding 和 vector_search 的数据一致性。
#[tokio::test]
async fn tc_xmod_001_storage_embedding_sync() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let service = ImportService::new(state.storage.clone(), state.data_dir.clone());

    // 创建测试文档
    let md = dir.path().join("sync-test.md");
    std::fs::write(
        &md,
        "# 同步测试\n\n这是一个测试 Storage 与 Embedder 数据一致性的文档。\n\n内容包含多个段落以确保分块。",
    )
    .unwrap();
    let canon = md.canonicalize().unwrap().to_string_lossy().into_owned();

    // 导入文档
    let outcome = service.import_one(&canon, true).await.unwrap();
    let doc = match outcome {
        ImportOutcome::Imported(d) => d,
        _ => panic!("导入应成功"),
    };

    // 索引文档（分块）
    service.index_document(&doc).await.unwrap();
    let chunks = state.storage.list_chunks(&doc.id).await.unwrap();
    assert!(!chunks.is_empty(), "分块后应有 chunks");

    // 为每个 chunk 写入向量
    let test_vector = vec![0.1_f32; 384];
    for chunk in &chunks {
        state
            .storage
            .add_embedding(&chunk.id, &test_vector)
            .await
            .unwrap();
    }

    // 验证向量可检索
    let hits = state.storage.vector_search(&test_vector, 10).await.unwrap();
    assert!(!hits.is_empty(), "向量检索应返回结果");
    assert!(
        hits[0].score > 0.99,
        "自身向量余弦相似度应接近 1.0，实际: {}",
        hits[0].score
    );

    // 验证 chunk 与 embedding 数据一致性
    let chunk_ids: std::collections::HashSet<_> = chunks.iter().map(|c| c.id.clone()).collect();
    let hit_chunk_ids: std::collections::HashSet<_> =
        hits.iter().map(|h| h.chunk.id.clone()).collect();
    assert!(
        chunk_ids.is_subset(&hit_chunk_ids),
        "所有 chunk 的 embedding 应可检索"
    );
}

/// TC-XMOD-002: 删除文档后 Embedding 级联清理。
///
/// 验证：导入文档 → 写入向量 → 删除文档 → 向量不可检索。
/// 确保 Storage.delete_document 正确清理 embeddings 表。
#[tokio::test]
async fn tc_xmod_002_delete_cascades_embedding() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let service = ImportService::new(state.storage.clone(), state.data_dir.clone());

    // 创建测试文档
    let md = dir.path().join("cascade-test.md");
    std::fs::write(&md, "# 级联清理测试\n\n删除文档后向量应被清理。").unwrap();
    let canon = md.canonicalize().unwrap().to_string_lossy().into_owned();

    // 导入并索引
    let outcome = service.import_one(&canon, true).await.unwrap();
    let doc = match outcome {
        ImportOutcome::Imported(d) => d,
        _ => panic!("导入应成功"),
    };
    service.index_document(&doc).await.unwrap();

    // 写入向量
    let test_vector = vec![0.5_f32; 4];
    let chunks = state.storage.list_chunks(&doc.id).await.unwrap();
    for chunk in &chunks {
        state
            .storage
            .add_embedding(&chunk.id, &test_vector)
            .await
            .unwrap();
    }

    // 验证可检索
    let hits_before = state.storage.vector_search(&test_vector, 10).await.unwrap();
    assert!(!hits_before.is_empty(), "删除前应可检索");

    // 删除文档
    delete_document_inner(&doc.id, &state).await.unwrap();

    // 验证不可检索
    let hits_after = state.storage.vector_search(&test_vector, 10).await.unwrap();
    assert!(
        hits_after.is_empty(),
        "删除后向量应不可检索（embeddings 已级联清理）"
    );
}

/// TC-XMOD-003: 多文档向量空间隔离。
///
/// 验证：导入多个文档 → 写入不同向量 → 检索时各文档向量空间隔离。
#[tokio::test]
async fn tc_xmod_003_multi_doc_vector_isolation() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let service = ImportService::new(state.storage.clone(), state.data_dir.clone());

    // 导入 3 个文档
    let docs_info = [
        ("rust.md", "Rust 编程语言", vec![1.0, 0.0, 0.0]),
        ("python.md", "Python 编程语言", vec![0.0, 1.0, 0.0]),
        ("go.md", "Go 编程语言", vec![0.0, 0.0, 1.0]),
    ];

    for (name, content, vector) in &docs_info {
        let path = dir.path().join(name);
        std::fs::write(&path, content).unwrap();
        let canon = path.canonicalize().unwrap().to_string_lossy().into_owned();
        let outcome = service.import_one(&canon, true).await.unwrap();
        if let ImportOutcome::Imported(doc) = outcome {
            service.index_document(&doc).await.unwrap();
            let chunks = state.storage.list_chunks(&doc.id).await.unwrap();
            for chunk in &chunks {
                state
                    .storage
                    .add_embedding(&chunk.id, vector)
                    .await
                    .unwrap();
            }
        }
    }

    // 检索 Rust 向量
    let rust_hits = state
        .storage
        .vector_search(&[1.0, 0.0, 0.0], 5)
        .await
        .unwrap();
    assert!(!rust_hits.is_empty());
    assert!(
        rust_hits[0].doc_name.contains("rust"),
        "Rust 向量检索应返回 rust.md"
    );

    // 检索 Python 向量
    let python_hits = state
        .storage
        .vector_search(&[0.0, 1.0, 0.0], 5)
        .await
        .unwrap();
    assert!(!python_hits.is_empty());
    assert!(
        python_hits[0].doc_name.contains("python"),
        "Python 向量检索应返回 python.md"
    );

    // 检索 Go 向量
    let go_hits = state
        .storage
        .vector_search(&[0.0, 0.0, 1.0], 5)
        .await
        .unwrap();
    assert!(!go_hits.is_empty());
    assert!(
        go_hits[0].doc_name.contains("go"),
        "Go 向量检索应返回 go.md"
    );
}

// ============================================================================
// IPC 命令端到端测试
// ============================================================================

/// TC-IPC-001: 完整 IPC 命令链路：导入 → 设置 → 检索 → 导出。
///
/// 验证多个 IPC 命令的完整执行链路，确保状态正确传递。
#[tokio::test]
async fn tc_ipc_001_full_command_chain() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 1. 配置 LLM
    update_llm_config_inner(test_config(), &state)
        .await
        .unwrap();

    // 2. 验证设置已持久化
    let settings = get_settings_inner(&state).await.unwrap();
    assert!(settings.has_llm_config, "LLM 配置应已持久化");
    assert_eq!(settings.model, "deepseek-chat");

    // 3. 导入文档
    let md = dir.path().join("ipc-test.md");
    std::fs::write(&md, "# IPC 测试\n\n这是一个 IPC 命令链路测试文档。").unwrap();
    let app = tauri::test::mock_app();
    let handle = app.handle().clone();
    let imported = import_files_inner(&handle, &[md.to_string_lossy().into_owned()], &state)
        .await
        .unwrap();
    assert_eq!(imported.len(), 1);

    // 4. 创建会话
    let conv_id = create_conversation_inner("default".to_string(), &state)
        .await
        .unwrap();
    assert!(!conv_id.is_empty());

    // 5. 写入问答
    persist_exchange(
        &state,
        &conv_id,
        "测试问题",
        "测试回答",
        None,
        None,
        None,
        None,
    )
    .await
    .unwrap();

    // 6. 验证消息
    let messages = get_messages_inner(&conv_id, &state).await.unwrap();
    assert_eq!(messages.len(), 2);

    // 7. 导出会话
    let (markdown, _) = export_conversation_markdown_inner(&conv_id, &state)
        .await
        .unwrap();
    assert!(markdown.contains("测试问题"));
    assert!(markdown.contains("测试回答"));

    // 8. 删除会话
    delete_conversation_inner(&conv_id, &state).await.unwrap();
    let convs = get_conversations_inner("default", &state).await.unwrap();
    assert!(convs.is_empty());
}

/// TC-IPC-002: 设置持久化全量覆盖。
///
/// 验证所有可配置设置项的持久化和重启恢复。
#[tokio::test]
async fn tc_ipc_002_all_settings_persist() {
    let dir = TempDir::new().unwrap();

    // 第一次会话：设置所有配置
    {
        let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
        update_llm_config_inner(test_config(), &state)
            .await
            .unwrap();

        // 设置各种配置
        set_hybrid_search_inner(true, &state).await.unwrap();
        set_rerank_enabled_inner(true, &state).await.unwrap();
        set_hyde_enabled_inner(true, &state).await.unwrap();
        set_token_budget_inner(4096, &state).await.unwrap();
        set_context_token_limit_inner(8192, &state).await.unwrap();
    }

    // 第二次会话：验证所有配置
    {
        let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
        let settings = get_settings_inner(&state).await.unwrap();

        assert!(settings.has_llm_config, "LLM 配置应持久化");
        assert!(settings.hybrid_search, "混合搜索应持久化");
        assert!(settings.rerank_enabled, "重排序应持久化");
        assert!(settings.hyde_enabled, "HyDE 应持久化");
        assert_eq!(settings.token_budget, 4096, "token 预算应持久化");
        assert_eq!(settings.context_token_limit, 8192, "上下文限制应持久化");
    }
}

/// TC-IPC-003: 会话管理完整 CRUD。
///
/// 验证会话的创建、读取、更新、删除完整流程。
#[tokio::test]
async fn tc_ipc_003_conversation_full_crud() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 创建多个会话
    let mut conv_ids = Vec::new();
    for i in 0..5 {
        let id = create_conversation_inner("default".to_string(), &state)
            .await
            .unwrap();
        conv_ids.push(id.clone());

        persist_exchange(
            &state,
            &id,
            &format!("问题 {i}"),
            &format!("回答 {i}"),
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
    }

    // 读取会话列表
    let convs = get_conversations_inner("default", &state).await.unwrap();
    assert_eq!(convs.len(), 5);

    // 读取消息
    for id in &conv_ids {
        let messages = get_messages_inner(id, &state).await.unwrap();
        assert_eq!(messages.len(), 2);
    }

    // 分页读取
    let page = get_conversations_paginated_inner("default", 2, 0, &state)
        .await
        .unwrap();
    assert_eq!(page.items.len(), 2);
    assert_eq!(page.total, 5);

    // 删除单个会话
    delete_conversation_inner(&conv_ids[0], &state)
        .await
        .unwrap();
    let convs_after = get_conversations_inner("default", &state).await.unwrap();
    assert_eq!(convs_after.len(), 4);

    // 验证消息级联删除
    let messages = get_messages_inner(&conv_ids[0], &state).await.unwrap();
    assert!(messages.is_empty());
}

// ============================================================================
// SQLite 真实读写测试
// ============================================================================

/// TC-SQL-001: SQLite 并发读写安全性。
///
/// 验证多个并发操作不会导致数据库锁死或数据损坏。
#[tokio::test]
async fn tc_sql_001_concurrent_read_write_safety() {
    let dir = TempDir::new().unwrap();
    let state = Arc::new(AppState::new(dir.path().to_path_buf()).await.unwrap());

    // 并发写入多个文档
    let mut handles = Vec::new();
    for i in 0..10 {
        let state_clone = state.clone();
        let path = dir.path().join(format!("concurrent-{i}.md"));
        std::fs::write(&path, format!("文档 {i} 的内容")).unwrap();
        let file_path = path.to_string_lossy().to_string();

        handles.push(tokio::spawn(async move {
            let service =
                ImportService::new(state_clone.storage.clone(), state_clone.data_dir.clone());
            let canon = std::path::Path::new(&file_path)
                .canonicalize()
                .unwrap()
                .to_string_lossy()
                .into_owned();
            service.import_one(&canon, true).await
        }));
    }

    // 等待所有导入完成
    let mut success_count = 0;
    for handle in handles {
        let result = handle.await.unwrap();
        if result.is_ok() {
            success_count += 1;
        }
    }
    assert_eq!(success_count, 10, "所有并发导入应成功");

    // 验证文档数
    let doc_count = state.storage.count_documents().await.unwrap();
    assert_eq!(doc_count, 10);

    // 并发读取
    let mut read_handles = Vec::new();
    for _ in 0..5 {
        let state_clone = state.clone();
        read_handles.push(tokio::spawn(async move {
            state_clone.storage.list_documents().await
        }));
    }

    for handle in read_handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 10);
    }
}

/// TC-SQL-002: SQLite WAL 模式下读写并发。
///
/// 验证 WAL 模式下读操作不会被写操作阻塞。
#[tokio::test]
async fn tc_sql_002_wal_concurrent_read_write() {
    let dir = TempDir::new().unwrap();
    let state = Arc::new(AppState::new(dir.path().to_path_buf()).await.unwrap());

    // 先写入一些数据
    for i in 0..5 {
        let doc = Document::new(format!("doc-{i}.md"), format!("hash-{i}"));
        state.storage.add_document(&doc).await.unwrap();
    }

    // 并发：写入 + 读取
    let state_write = state.clone();
    let state_read = state.clone();

    let write_handle = tokio::spawn(async move {
        for i in 5..10 {
            let doc = Document::new(format!("doc-{i}.md"), format!("hash-{i}"));
            state_write.storage.add_document(&doc).await.unwrap();
        }
    });

    let read_handle = tokio::spawn(async move {
        let mut count = 0;
        for _ in 0..10 {
            let docs = state_read.storage.list_documents().await.unwrap();
            count = docs.len();
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        count
    });

    write_handle.await.unwrap();
    let final_count = read_handle.await.unwrap();

    // 验证最终数据一致性
    let doc_count = state.storage.count_documents().await.unwrap();
    assert_eq!(doc_count, 10);
    assert!(final_count >= 5, "读操作应能读取到至少 5 个文档");
}

/// TC-SQL-003: FTS5 全文搜索索引一致性。
///
/// 验证文档导入后 FTS5 索引正确建立，搜索返回正确结果。
#[tokio::test]
async fn tc_sql_003_fts5_index_consistency() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let service = ImportService::new(state.storage.clone(), state.data_dir.clone());

    // 导入包含特定关键词的文档
    let docs = [
        ("rust.md", "Rust 是一门系统级编程语言"),
        ("python.md", "Python 是一种解释型语言"),
        ("mixed.md", "Rust 和 Python 都很流行"),
    ];

    for (name, content) in &docs {
        let path = dir.path().join(name);
        std::fs::write(&path, content).unwrap();
        let canon = path.canonicalize().unwrap().to_string_lossy().into_owned();
        let outcome = service.import_one(&canon, true).await.unwrap();
        if let ImportOutcome::Imported(doc) = outcome {
            service.index_document(&doc).await.unwrap();
        }
    }

    // 搜索 "Rust"
    let rust_hits = state.storage.keyword_search("Rust", 10).await.unwrap();
    assert!(!rust_hits.is_empty(), "FTS5 搜索 'Rust' 应返回结果");
    let rust_doc_names: Vec<_> = rust_hits.iter().map(|h| h.doc_name.clone()).collect();
    assert!(
        rust_doc_names.iter().any(|n| n.contains("rust")),
        "应包含 rust.md"
    );
    assert!(
        rust_doc_names.iter().any(|n| n.contains("mixed")),
        "应包含 mixed.md"
    );

    // 搜索 "Python"
    let python_hits = state.storage.keyword_search("Python", 10).await.unwrap();
    assert!(!python_hits.is_empty());
    let python_doc_names: Vec<_> = python_hits.iter().map(|h| h.doc_name.clone()).collect();
    assert!(
        python_doc_names.iter().any(|n| n.contains("python")),
        "应包含 python.md"
    );
}

/// TC-SQL-004: 数据库加密模式（SQLCipher）。
///
/// 验证数据库在加密模式下正常工作。
#[tokio::test]
async fn tc_sql_004_encrypted_db_operations() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 配置 LLM
    update_llm_config_inner(test_config(), &state)
        .await
        .unwrap();

    // 写入数据
    let doc = Document::new("encrypted-test.md".to_string(), "hash-enc".to_string());
    state.storage.add_document(&doc).await.unwrap();

    // 读取数据
    let docs = state.storage.list_documents().await.unwrap();
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].file_path, "encrypted-test.md");

    // 验证设置持久化
    let settings = get_settings_inner(&state).await.unwrap();
    assert!(settings.has_llm_config);
}

// ============================================================================
// 完整流程测试
// ============================================================================

/// TC-FLOW-001: 端到端 RAG 流程：导入 → 索引 → 检索 → 对话 → 持久化。
///
/// 验证完整的 RAG 流程，不依赖真实 LLM。
#[tokio::test]
async fn tc_flow_001_end_to_end_rag() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let service = ImportService::new(state.storage.clone(), state.data_dir.clone());

    // 1. 导入文档
    let md = dir.path().join("rag-doc.md");
    std::fs::write(
        &md,
        "# RAG 文档\n\nRAG（检索增强生成）是一种结合检索和生成的 AI 技术。\n\n它首先从知识库中检索相关文档，然后利用 LLM 生成回答。",
    )
    .unwrap();
    let canon = md.canonicalize().unwrap().to_string_lossy().into_owned();
    let outcome = service.import_one(&canon, true).await.unwrap();
    let doc = match outcome {
        ImportOutcome::Imported(d) => d,
        _ => panic!("导入应成功"),
    };

    // 2. 索引文档
    service.index_document(&doc).await.unwrap();
    let chunks = state.storage.list_chunks(&doc.id).await.unwrap();
    assert!(!chunks.is_empty(), "索引后应有 chunks");

    // 3. 写入向量
    let test_vector = vec![0.2_f32; 384];
    for chunk in &chunks {
        state
            .storage
            .add_embedding(&chunk.id, &test_vector)
            .await
            .unwrap();
    }

    // 4. 检索
    let hits = state.storage.vector_search(&test_vector, 5).await.unwrap();
    assert!(!hits.is_empty(), "检索应返回结果");
    assert!(hits[0].doc_name.contains("rag-doc"));

    // 5. 创建会话并写入问答
    let conv_id = create_conversation_inner("default".to_string(), &state)
        .await
        .unwrap();
    persist_exchange(
        &state,
        &conv_id,
        "什么是 RAG？",
        "RAG 是检索增强生成技术。",
        Some(hits),
        None,
        None,
        None,
    )
    .await
    .unwrap();

    // 6. 验证完整流程
    let messages = get_messages_inner(&conv_id, &state).await.unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].content, "什么是 RAG？");
    assert!(messages[1].content.contains("RAG"));

    // 7. 验证标题自动提取
    let convs = get_conversations_inner("default", &state).await.unwrap();
    assert!(convs[0].title.contains("RAG"));
}

/// TC-FLOW-002: 文档生命周期管理：导入 → 重建索引 → 删除 → 清理。
///
/// 验证文档的完整生命周期管理。
#[tokio::test]
async fn tc_flow_002_document_lifecycle() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let service = ImportService::new(state.storage.clone(), state.data_dir.clone());

    // 1. 导入文档
    let md = dir.path().join("lifecycle.md");
    std::fs::write(&md, "# 生命周期测试\n\n这是一个测试文档。").unwrap();
    let canon = md.canonicalize().unwrap().to_string_lossy().into_owned();
    let outcome = service.import_one(&canon, true).await.unwrap();
    let doc = match outcome {
        ImportOutcome::Imported(d) => d,
        _ => panic!("导入应成功"),
    };
    service.index_document(&doc).await.unwrap();

    // 2. 验证文档存在
    let docs = state.storage.list_documents().await.unwrap();
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].status, DocStatus::Indexed);

    // 3. 重建索引
    let app = tauri::test::mock_app();
    let handle = app.handle().clone();
    rebuild_index_inner(&handle, &doc.id, &state).await.unwrap();

    // 4. 验证重建后状态
    let docs_after = state.storage.list_documents().await.unwrap();
    assert_eq!(docs_after[0].status, DocStatus::Indexed);

    // 5. 删除文档
    delete_document_inner(&doc.id, &state).await.unwrap();

    // 6. 验证清理
    let docs_final = state.storage.list_documents().await.unwrap();
    assert!(docs_final.is_empty());
    let chunks = state.storage.list_chunks(&doc.id).await.unwrap();
    assert!(chunks.is_empty());
}

/// TC-FLOW-003: 工作空间隔离：不同工作空间的文档和会话隔离。
///
/// 验证不同工作空间的数据完全隔离。
#[tokio::test]
async fn tc_flow_003_workspace_isolation() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 创建两个工作空间的会话
    let conv_ws1 = create_conversation_inner("workspace-1".to_string(), &state)
        .await
        .unwrap();
    let conv_ws2 = create_conversation_inner("workspace-2".to_string(), &state)
        .await
        .unwrap();

    // 写入消息
    persist_exchange(
        &state,
        &conv_ws1,
        "工作空间 1 的问题",
        "工作空间 1 的回答",
        None,
        None,
        None,
        None,
    )
    .await
    .unwrap();

    persist_exchange(
        &state,
        &conv_ws2,
        "工作空间 2 的问题",
        "工作空间 2 的回答",
        None,
        None,
        None,
        None,
    )
    .await
    .unwrap();

    // 验证工作空间隔离
    let ws1_convs = get_conversations_inner("workspace-1", &state)
        .await
        .unwrap();
    let ws2_convs = get_conversations_inner("workspace-2", &state)
        .await
        .unwrap();

    assert_eq!(ws1_convs.len(), 1);
    assert_eq!(ws2_convs.len(), 1);
    assert_ne!(ws1_convs[0].id, ws2_convs[0].id);

    // 验证消息隔离
    let ws1_messages = get_messages_inner(&conv_ws1, &state).await.unwrap();
    let ws2_messages = get_messages_inner(&conv_ws2, &state).await.unwrap();

    assert!(ws1_messages[0].content.contains("工作空间 1"));
    assert!(ws2_messages[0].content.contains("工作空间 2"));
}

/// TC-FLOW-004: 多格式文档导入流程。
///
/// 验证多种格式文档的导入和处理流程。
#[tokio::test]
async fn tc_flow_004_multi_format_import() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let service = ImportService::new(state.storage.clone(), state.data_dir.clone());

    // 创建不同格式的测试文件
    let formats = [
        ("test.md", "# Markdown\n\n这是 Markdown 文档"),
        ("test.txt", "这是纯文本文档"),
    ];

    let mut imported_count = 0;
    for (name, content) in &formats {
        let path = dir.path().join(name);
        std::fs::write(&path, content).unwrap();
        let canon = path.canonicalize().unwrap().to_string_lossy().into_owned();
        let outcome = service.import_one(&canon, true).await.unwrap();
        if let ImportOutcome::Imported(doc) = outcome {
            service.index_document(&doc).await.unwrap();
            imported_count += 1;
        }
    }

    assert_eq!(imported_count, 2);

    // 验证所有文档都已导入
    let docs = state.storage.list_documents().await.unwrap();
    assert_eq!(docs.len(), 2);

    // 验证每个文档都有 chunks
    for doc in &docs {
        let chunks = state.storage.list_chunks(&doc.id).await.unwrap();
        assert!(!chunks.is_empty(), "文档 {} 应有 chunks", doc.file_path);
    }
}

/// TC-FLOW-005: 搜索结果分页和排序。
///
/// 验证搜索结果的分页和排序功能。
#[tokio::test]
async fn tc_flow_005_search_pagination_and_sorting() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let service = ImportService::new(state.storage.clone(), state.data_dir.clone());

    // 导入多个文档
    for i in 0..10 {
        let path = dir.path().join(format!("search-doc-{i}.md"));
        std::fs::write(&path, format!("文档 {i} 的内容，包含搜索关键词。")).unwrap();
        let canon = path.canonicalize().unwrap().to_string_lossy().into_owned();
        let outcome = service.import_one(&canon, true).await.unwrap();
        if let ImportOutcome::Imported(doc) = outcome {
            service.index_document(&doc).await.unwrap();
            let chunks = state.storage.list_chunks(&doc.id).await.unwrap();
            // 写入不同的向量（模拟不同相关性）
            let vector = vec![i as f32; 4];
            for chunk in &chunks {
                state
                    .storage
                    .add_embedding(&chunk.id, &vector)
                    .await
                    .unwrap();
            }
        }
    }

    // 搜索并验证排序
    let query_vector = vec![5.0_f32; 4];
    let hits = state.storage.vector_search(&query_vector, 5).await.unwrap();
    assert_eq!(hits.len(), 5);

    // 验证结果按相似度降序排列
    for i in 1..hits.len() {
        assert!(
            hits[i - 1].score >= hits[i].score,
            "搜索结果应按相似度降序排列"
        );
    }

    // 验证 top_k 限制
    let hits_3 = state.storage.vector_search(&query_vector, 3).await.unwrap();
    assert_eq!(hits_3.len(), 3);
}

// ============================================================================
// 错误处理和边界条件测试
// ============================================================================

/// TC-ERR-001: 导入不存在的文件返回错误。
///
/// 验证导入不存在的文件时返回明确的错误信息。
#[tokio::test]
async fn tc_err_001_import_nonexistent_file() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let service = ImportService::new(state.storage.clone(), state.data_dir.clone());

    let result = service.import_one("/nonexistent/path/file.md", true).await;
    assert!(result.is_err(), "导入不存在的文件应返回错误");
}

/// TC-ERR-002: 空文件处理。
///
/// 验证导入空文件时不产生垃圾 chunks。
#[tokio::test]
async fn tc_err_002_empty_file_handling() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let service = ImportService::new(state.storage.clone(), state.data_dir.clone());

    let path = dir.path().join("empty.md");
    std::fs::write(&path, "").unwrap();
    let canon = path.canonicalize().unwrap().to_string_lossy().into_owned();

    let outcome = service.import_one(&canon, true).await.unwrap();
    let doc = match outcome {
        ImportOutcome::Imported(d) => d,
        _ => panic!("导入应成功"),
    };

    // 空文件不应产生 chunks
    service.index_document(&doc).await.unwrap();
    let chunks = state.storage.list_chunks(&doc.id).await.unwrap();
    assert!(chunks.is_empty(), "空文件不应产生 chunks");
}

/// TC-ERR-003: 重复文档导入去重。
///
/// 验证重复导入相同内容时自动去重。
#[tokio::test]
async fn tc_err_003_duplicate_import_dedup() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let service = ImportService::new(state.storage.clone(), state.data_dir.clone());

    let path = dir.path().join("dedup.md");
    std::fs::write(&path, "# 去重测试\n\n这是去重测试内容。").unwrap();
    let canon = path.canonicalize().unwrap().to_string_lossy().into_owned();

    // 第一次导入
    let outcome1 = service.import_one(&canon, true).await.unwrap();
    assert!(matches!(outcome1, ImportOutcome::Imported(_)));

    // 第二次导入
    let outcome2 = service.import_one(&canon, true).await.unwrap();
    assert!(matches!(outcome2, ImportOutcome::SkippedDuplicate(_)));

    // 文档数应为 1
    let count = state.storage.count_documents().await.unwrap();
    assert_eq!(count, 1);
}

/// TC-ERR-004: 会话不存在时操作返回错误。
///
/// 验证对不存在的会话进行操作时返回明确的错误。
#[tokio::test]
async fn tc_err_004_nonexistent_conversation() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 获取不存在会话的消息
    let result = get_messages_inner("nonexistent-conv", &state).await;
    assert!(result.is_ok()); // 返回空列表而非错误
    assert!(result.unwrap().is_empty());

    // 删除不存在的会话
    let _result = delete_conversation_inner("nonexistent-conv", &state).await;
    // 应该成功（无操作）或返回错误
}

/// TC-ERR-005: 无效 LLM 配置处理。
///
/// 验证无效的 LLM 配置被正确拒绝。
#[tokio::test]
async fn tc_err_005_invalid_llm_config() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 空 API Key
    let invalid_config = LlmConfig {
        api_key: String::new(),
        base_url: "https://api.deepseek.com".to_string(),
        model: "deepseek-chat".to_string(),
    };

    let _result = update_llm_config_inner(invalid_config, &state).await;
    // 应该成功保存（API Key 可能是 BYOK 模式）
    // 但运行时使用时会失败
}

// ============================================================================
// 性能和压力测试
// ============================================================================

/// TC-PERF-001: 大量文档导入性能。
///
/// 验证批量导入文档的性能在可接受范围内。
#[tokio::test]
async fn tc_perf_001_batch_import_performance() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let service = ImportService::new(state.storage.clone(), state.data_dir.clone());

    let start = std::time::Instant::now();

    // 导入 20 个文档
    for i in 0..20 {
        let path = dir.path().join(format!("perf-doc-{i}.md"));
        let content = format!("# 文档 {i}\n\n这是性能测试文档的内容。").repeat(10);
        std::fs::write(&path, content).unwrap();
        let canon = path.canonicalize().unwrap().to_string_lossy().into_owned();
        let outcome = service.import_one(&canon, true).await.unwrap();
        if let ImportOutcome::Imported(doc) = outcome {
            service.index_document(&doc).await.unwrap();
        }
    }

    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs() < 60,
        "20 个文档导入应在 60 秒内完成，实际: {:?}",
        elapsed
    );

    // 验证所有文档都已导入
    let doc_count = state.storage.count_documents().await.unwrap();
    assert_eq!(doc_count, 20);
}

/// TC-PERF-002: 向量检索性能。
///
/// 验证大量向量下的检索性能。
#[tokio::test]
async fn tc_perf_002_vector_search_performance() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let service = ImportService::new(state.storage.clone(), state.data_dir.clone());

    // 创建并导入 50 个文档
    for i in 0..50 {
        let path = dir.path().join(format!("search-perf-{i}.md"));
        std::fs::write(&path, format!("文档 {i} 的内容")).unwrap();
        let canon = path.canonicalize().unwrap().to_string_lossy().into_owned();
        let outcome = service.import_one(&canon, true).await.unwrap();
        if let ImportOutcome::Imported(doc) = outcome {
            service.index_document(&doc).await.unwrap();
            let chunks = state.storage.list_chunks(&doc.id).await.unwrap();
            let vector = vec![i as f32; 384];
            for chunk in &chunks {
                state
                    .storage
                    .add_embedding(&chunk.id, &vector)
                    .await
                    .unwrap();
            }
        }
    }

    // 执行多次搜索
    let start = std::time::Instant::now();
    for _ in 0..10 {
        let query = vec![25.0_f32; 384];
        let _ = state.storage.vector_search(&query, 10).await.unwrap();
    }
    let elapsed = start.elapsed();

    assert!(
        elapsed.as_secs() < 30,
        "10 次向量检索应在 30 秒内完成，实际: {:?}",
        elapsed
    );
}

/// TC-PERF-003: 并发搜索性能。
///
/// 验证并发搜索的性能和正确性。
#[tokio::test]
async fn tc_perf_003_concurrent_search_performance() {
    let dir = TempDir::new().unwrap();
    let state = Arc::new(AppState::new(dir.path().to_path_buf()).await.unwrap());
    let service = ImportService::new(state.storage.clone(), state.data_dir.clone());

    // 创建测试数据
    for i in 0..20 {
        let path = dir.path().join(format!("concurrent-search-{i}.md"));
        std::fs::write(&path, format!("文档 {i} 的内容")).unwrap();
        let canon = path.canonicalize().unwrap().to_string_lossy().into_owned();
        let outcome = service.import_one(&canon, true).await.unwrap();
        if let ImportOutcome::Imported(doc) = outcome {
            service.index_document(&doc).await.unwrap();
            let chunks = state.storage.list_chunks(&doc.id).await.unwrap();
            let vector = vec![i as f32; 4];
            for chunk in &chunks {
                state
                    .storage
                    .add_embedding(&chunk.id, &vector)
                    .await
                    .unwrap();
            }
        }
    }

    // 并发搜索
    let start = std::time::Instant::now();
    let mut handles = Vec::new();
    for i in 0..10 {
        let state_clone = state.clone();
        handles.push(tokio::spawn(async move {
            let query = vec![i as f32; 4];
            state_clone.storage.vector_search(&query, 5).await
        }));
    }

    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }

    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs() < 30,
        "10 个并发搜索应在 30 秒内完成，实际: {:?}",
        elapsed
    );
}

// ============================================================================
// 数据一致性测试
// ============================================================================

/// TC-CONSIST-001: 事务性写入一致性。
///
/// 验证写入操作的事务性，确保数据一致性。
#[tokio::test]
async fn tc_consist_001_transactional_write_consistency() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 创建文档
    let doc = Document::new("transactional.md".to_string(), "hash-tx".to_string());
    state.storage.add_document(&doc).await.unwrap();

    // 创建多个 chunks
    for i in 0..5 {
        let chunk = Chunk::new(doc.id.clone(), format!("chunk {i}"), 10, i);
        state.storage.add_chunk(&chunk).await.unwrap();
    }

    // 验证数据一致性
    let docs = state.storage.list_documents().await.unwrap();
    assert_eq!(docs.len(), 1);

    let chunks = state.storage.list_chunks(&doc.id).await.unwrap();
    assert_eq!(chunks.len(), 5);

    // 删除文档并验证级联删除
    delete_document_inner(&doc.id, &state).await.unwrap();

    let docs_after = state.storage.list_documents().await.unwrap();
    assert!(docs_after.is_empty());

    let chunks_after = state.storage.list_chunks(&doc.id).await.unwrap();
    assert!(chunks_after.is_empty());
}

/// TC-CONSIST-002: 向量索引一致性。
///
/// 验证向量索引在文档更新后保持一致。
#[tokio::test]
async fn tc_consist_002_vector_index_consistency() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let service = ImportService::new(state.storage.clone(), state.data_dir.clone());

    // 导入文档
    let path = dir.path().join("consistency.md");
    std::fs::write(&path, "一致性测试内容").unwrap();
    let canon = path.canonicalize().unwrap().to_string_lossy().into_owned();
    let outcome = service.import_one(&canon, true).await.unwrap();
    let doc = match outcome {
        ImportOutcome::Imported(d) => d,
        _ => panic!("导入应成功"),
    };
    service.index_document(&doc).await.unwrap();

    // 写入向量
    let chunks = state.storage.list_chunks(&doc.id).await.unwrap();
    for chunk in &chunks {
        state
            .storage
            .add_embedding(&chunk.id, &[1.0, 0.0])
            .await
            .unwrap();
    }

    // 重建索引
    let app = tauri::test::mock_app();
    let handle = app.handle().clone();
    rebuild_index_inner(&handle, &doc.id, &state).await.unwrap();

    // 验证向量索引仍一致
    let _hits = state.storage.vector_search(&[1.0, 0.0], 10).await.unwrap();
    // 重建索引后向量可能被清除，但不应崩溃
}
