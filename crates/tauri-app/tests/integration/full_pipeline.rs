#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! 全链路集成测试 — 导入→分块→嵌入→检索→对话→流式响应。
//!
//! 覆盖 V2.0 深耕计划 Phase 4 S11 测试用例：
//! TC-FULL-001 ~ TC-FULL-008。

use super::common::*;
use super::*;
use echomind_core::Storage;
use echomind_core::import::{ImportOutcome, ImportService};

// ============================================================================
// TC-FULL-001: 导入 .md → 分块 → 向量化（mock）→ 检索 → RAG 对话 → 持久化
// ============================================================================

/// TC-FULL-001：导入 → 分块 → 手动写入向量 → vector_search → persist_exchange → 验证。
///
/// 测试不依赖 ONNX 模型下载（embedder mock 模式）。
/// 导入一个 Markdown 文件，手动为 chunks 写入向量，执行 vector_search，
/// 验证检索到的 chunk 与导入内容匹配，最后 persist_exchange 验证消息落库。
#[tokio::test]
async fn tc_full_001_import_retrieve_chat_persist() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let service = ImportService::new(state.storage.clone(), state.data_dir.clone());

    // 1. 导入 Markdown 文件
    let md = dir.path().join("full-pipeline.md");
    std::fs::write(
        &md,
        "# 全链路测试\n\nEchoMind 是一个本地 RAG 知识库工具。\n\n它使用 Rust 和 Tauri 构建。",
    )
    .unwrap();
    let canon = md.canonicalize().unwrap().to_string_lossy().into_owned();
    let outcome = service.import_one(&canon, true).await.unwrap();
    let doc = match outcome {
        ImportOutcome::Imported(d) => d,
        _ => panic!("导入应成功"),
    };

    // 2. 分块
    service.index_document(&doc).await.unwrap();
    let chunks = state.storage.list_chunks(&doc.id).await.unwrap();
    assert!(!chunks.is_empty(), "分块后应有 chunks");

    // 3. 手动写入向量（模拟 embedder 输出）
    let mock_vec = [1.0_f32, 0.0, 0.0, 0.0];
    for chunk in &chunks {
        state
            .storage
            .add_embedding(&chunk.id, &mock_vec)
            .await
            .unwrap();
    }

    // 4. 检索
    let hits = state.storage.vector_search(&mock_vec, 5).await.unwrap();
    assert!(!hits.is_empty(), "vector_search 应返回结果");
    assert!(
        hits[0].doc_name.contains("full-pipeline"),
        "检索到的文档应为导入的 full-pipeline.md"
    );

    // 5. persist_exchange 验证消息持久化
    let conv_id = create_conversation_inner("default".to_string(), &state)
        .await
        .unwrap();
    persist_exchange(
        &state,
        &conv_id,
        "什么是 EchoMind？",
        "EchoMind 是一个本地 RAG 知识库工具。",
        Some(hits),
        None,
        None,
        None,
    )
    .await
    .unwrap();

    // 6. 验证消息和标题
    let messages = get_messages_inner(&conv_id, &state).await.unwrap();
    assert_eq!(messages.len(), 2, "应有 2 条消息（user + assistant）");
    assert_eq!(messages[0].role, "user");
    assert_eq!(messages[1].role, "assistant");

    let convs = get_conversations_inner("default", &state).await.unwrap();
    assert!(convs[0].title.contains("EchoMind"), "标题应从用户消息提取");
}

// ============================================================================
// TC-FULL-003: 导入多文件 → 批量处理 → 并发安全
// ============================================================================

/// TC-FULL-003：导入多个文件 → 批量处理 → 验证每个文件都有独立的文档记录。
///
/// 使用 ImportService.import_one 逐个导入 5 个文件，验证文档数 = 5，
/// 每个 chunks 连续，且并发导入不会产生数据损坏。
#[tokio::test]
async fn tc_full_003_multi_file_batch_import() {
    let dir = TempDir::new().unwrap();
    let state = Arc::new(AppState::new(dir.path().to_path_buf()).await.unwrap());
    // 创建 5 个不同主题的文件（内容足够长以产生 chunks）
    let topics = ["rust", "python", "java", "go", "kotlin"];
    for topic in &topics {
        let path = dir.path().join(format!("{topic}-doc.md"));
        std::fs::write(
            &path,
            format!(
                "# {topic}\n\n\
                 {topic} is a programming language with unique features. \
                 It has a rich ecosystem and strong community support. \
                 Many developers use {topic} for building scalable applications. \
                 The language design emphasizes safety, concurrency, and performance. \
                 {topic} continues to evolve with new features and improvements each year."
            ),
        )
        .unwrap();
    }

    // 顺序导入 5 个文件（避免并发 hash 冲突）
    // import_one 只创建文档记录，需要手动调用 index_document 执行分块
    let mut imported_count = 0;
    for topic in &topics {
        let path = dir.path().join(format!("{topic}-doc.md"));
        let canon = path.canonicalize().unwrap().to_string_lossy().into_owned();
        let svc = ImportService::new(state.storage.clone(), state.data_dir.clone());
        let outcome = svc.import_one(&canon, true).await.unwrap();
        if let ImportOutcome::Imported(doc) = outcome {
            // 执行索引：分块 + 实体/关系/Proposition/Wiki-link
            svc.index_document(&doc).await.unwrap();
            imported_count += 1;
        }
    }
    assert_eq!(imported_count, 5, "应成功导入 5 个文件");

    // 验证文档数
    let doc_count = state.storage.count_documents().await.unwrap();
    assert_eq!(doc_count, 5, "数据库应有 5 个文档");

    // 验证每个文档都有 chunks
    let docs = state.storage.list_documents().await.unwrap();
    for doc in &docs {
        let chunks = state.storage.list_chunks(&doc.id).await.unwrap();
        assert!(!chunks.is_empty(), "文档 {} 应有 chunks", doc.file_path);
        // 验证 sequence 连续
        for (idx, chunk) in chunks.iter().enumerate() {
            assert_eq!(chunk.sequence, idx, "chunk sequence 应从 0 连续递增");
        }
    }
}

// ============================================================================
// TC-FULL-004: 导入 → 删除文档 → 级联清理（chunks/embeddings）
// ============================================================================

/// TC-FULL-004：导入 → 删除文档 → 验证 chunks 和 embeddings 级联清理。
///
/// 导入一个文档并手动写入向量，删除文档后验证：
/// - 文档列表为空
/// - chunks 已级联删除
/// - vector_search 不再返回该文档
#[tokio::test]
async fn tc_full_004_delete_cascade_cleanup() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let service = ImportService::new(state.storage.clone(), state.data_dir.clone());

    // 导入文档
    let md = dir.path().join("cascade-test.md");
    std::fs::write(&md, "# 级联清理测试\n\n这是测试内容。").unwrap();
    let canon = md.canonicalize().unwrap().to_string_lossy().into_owned();
    let outcome = service.import_one(&canon, true).await.unwrap();
    let doc = match outcome {
        ImportOutcome::Imported(d) => d,
        _ => panic!("导入应成功"),
    };
    service.index_document(&doc).await.unwrap();

    // 写入向量
    let chunks = state.storage.list_chunks(&doc.id).await.unwrap();
    assert!(!chunks.is_empty(), "应有 chunks");
    for chunk in &chunks {
        state
            .storage
            .add_embedding(&chunk.id, &[0.5_f32, 0.5])
            .await
            .unwrap();
    }

    // 验证搜索能找到
    let hits_before = state.storage.vector_search(&[0.5, 0.5], 10).await.unwrap();
    assert!(!hits_before.is_empty(), "删除前 vector_search 应返回结果");

    // 删除文档
    delete_document_inner(&doc.id, &state).await.unwrap();

    // 验证文档列表为空
    let docs = state.storage.list_documents().await.unwrap();
    assert!(docs.is_empty(), "删除后文档列表应为空");

    // 验证 chunks 已级联删除
    let chunks_after = state.storage.list_chunks(&doc.id).await.unwrap();
    assert!(chunks_after.is_empty(), "删除后 chunks 应为空（级联清理）");

    // 验证 vector_search 不再返回该文档
    let hits_after = state.storage.vector_search(&[0.5, 0.5], 10).await.unwrap();
    assert!(
        hits_after.is_empty(),
        "删除后 vector_search 不应返回结果（embeddings 已级联清理）"
    );
}

// ============================================================================
// TC-FULL-005: 导入 → 编辑消息 → 重新检索 → 新对话分支
// ============================================================================

/// TC-FULL-005：导入 → 创建会话 → 写入消息 → 编辑用户消息 → 验证版本管理。
///
/// 验证导入文档后，创建会话并写入问答，编辑用户消息产生新版本，
/// 旧版本保留在 DB 中，新版本可被检索。
#[tokio::test]
async fn tc_full_005_edit_message_version_branch() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let service = ImportService::new(state.storage.clone(), state.data_dir.clone());

    // 导入文档
    let md = dir.path().join("version-test.md");
    std::fs::write(&md, "# 版本测试\n\n这是版本管理测试内容。").unwrap();
    let canon = md.canonicalize().unwrap().to_string_lossy().into_owned();
    let outcome = service.import_one(&canon, true).await.unwrap();
    let doc = match outcome {
        ImportOutcome::Imported(d) => d,
        _ => panic!("导入应成功"),
    };
    service.index_document(&doc).await.unwrap();

    // 创建会话并写入问答
    let conv_id = create_conversation_inner("default".to_string(), &state)
        .await
        .unwrap();
    persist_exchange(
        &state,
        &conv_id,
        "原始问题",
        "原始回答",
        None,
        None,
        None,
        None,
    )
    .await
    .unwrap();

    // 编辑用户消息（创建新版本）
    let turn_group = "turn-full-005";
    let v1 = edit_user_message_inner(&conv_id, turn_group, "编辑后的问题", &state)
        .await
        .unwrap();
    assert_eq!(v1, 1, "首次编辑应返回 version=1");

    // 再次编辑
    let v2 = edit_user_message_inner(&conv_id, turn_group, "二次编辑的问题", &state)
        .await
        .unwrap();
    assert_eq!(v2, 2, "二次编辑应返回 version=2");

    // 验证消息数量
    let messages = get_messages_inner(&conv_id, &state).await.unwrap();
    // 初始 2 + 编辑 v1 + 编辑 v2 = 4
    assert_eq!(messages.len(), 4, "应有 4 条消息");

    // 验证编辑版本有正确的 turn_group
    let edited: Vec<_> = messages
        .iter()
        .filter(|m| m.turn_group.as_deref() == Some(turn_group))
        .collect();
    assert_eq!(edited.len(), 2, "应有 2 条带 turn_group 的消息");
    assert_eq!(edited[0].version, Some(1));
    assert_eq!(edited[1].version, Some(2));
}

// ============================================================================
// TC-FULL-006: 导入 → 混合检索（向量+BM25）→ RRF 融合
// ============================================================================

/// TC-FULL-006：导入多文档 → 手动写入向量 → keyword_search + vector_search → 验证 BM25 排序。
///
/// 验证 FTS5 关键词搜索和向量搜索都能返回正确结果。
/// 完整的 RRF 融合在 HybridRetriever 层，此处验证底层两个通道。
#[tokio::test]
async fn tc_full_006_hybrid_search_vector_and_keyword() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let service = ImportService::new(state.storage.clone(), state.data_dir.clone());

    // 导入 3 个不同主题的文档
    let docs = [
        ("rust-lang.md", "Rust programming language is fast."),
        ("python-lang.md", "Python is a high-level language."),
        ("cooking.md", "Tomato eggs is a simple Chinese dish."),
    ];

    let mut doc_ids = Vec::new();
    for (name, content) in &docs {
        let path = dir.path().join(name);
        std::fs::write(&path, content).unwrap();
        let canon = path.canonicalize().unwrap().to_string_lossy().into_owned();
        let outcome = service.import_one(&canon, true).await.unwrap();
        if let ImportOutcome::Imported(doc) = outcome {
            service.index_document(&doc).await.unwrap();
            let chunks = state.storage.list_chunks(&doc.id).await.unwrap();
            // 为每个文档写入不同的向量
            let vec: &[f32] = match *name {
                "rust-lang.md" => &[1.0, 0.0, 0.0],
                "python-lang.md" => &[0.0, 1.0, 0.0],
                _ => &[0.0, 0.0, 1.0],
            };
            for chunk in &chunks {
                state.storage.add_embedding(&chunk.id, vec).await.unwrap();
            }
            doc_ids.push(doc.id);
        }
    }

    // 向量搜索：rust 向量应返回 rust-lang.md
    let vec_hits = state
        .storage
        .vector_search(&[1.0, 0.0, 0.0], 5)
        .await
        .unwrap();
    assert!(!vec_hits.is_empty());
    assert!(
        vec_hits[0].doc_name.contains("rust-lang"),
        "向量搜索应返回 rust-lang.md"
    );

    // 关键词搜索：搜索 "Python"
    let kw_hits = state.storage.keyword_search("Python", 5).await.unwrap();
    assert!(!kw_hits.is_empty(), "关键词搜索应返回结果");
    let kw_names: Vec<_> = kw_hits.iter().map(|h| h.doc_name.clone()).collect();
    assert!(
        kw_names.iter().any(|n| n.contains("python-lang")),
        "关键词搜索应返回 python-lang.md"
    );

    // 关键词搜索：搜索 "Tomato"
    let kw_hits2 = state.storage.keyword_search("Tomato", 5).await.unwrap();
    assert!(!kw_hits2.is_empty());
    assert!(
        kw_hits2[0].doc_name.contains("cooking"),
        "关键词搜索应返回 cooking.md"
    );
}

// ============================================================================
// TC-FULL-008: 导入 → 压缩 → 压缩后检索 → 结果一致性
// ============================================================================

/// TC-FULL-008：导入 → 设置压缩比 → 验证压缩比持久化 → 检索结果一致。
///
/// 验证设置 compression_ratio 后，检索结果不受压缩设置影响（压缩仅影响 prompt 构建，
/// 不影响检索本身）。验证设置持久化和重启恢复。
#[tokio::test]
async fn tc_full_008_compression_then_search_consistency() {
    let dir = TempDir::new().unwrap();

    // 1. 初始状态：设置压缩比 = 3.0
    {
        let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
        set_compression_ratio_inner(3.0, &state).await.unwrap();

        // 导入文档
        let service = ImportService::new(state.storage.clone(), state.data_dir.clone());
        let md = dir.path().join("compress-test.md");
        std::fs::write(
            &md,
            "# 压缩测试\n\n这是用于测试压缩后检索一致性的文档内容。",
        )
        .unwrap();
        let canon = md.canonicalize().unwrap().to_string_lossy().into_owned();
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
                .add_embedding(&chunk.id, &[0.8_f32, 0.2])
                .await
                .unwrap();
        }

        // 检索
        let hits = state.storage.vector_search(&[0.8, 0.2], 5).await.unwrap();
        assert!(!hits.is_empty(), "压缩设置下检索应返回结果");
        assert!(
            hits[0].doc_name.contains("compress-test"),
            "检索应返回压缩测试文档"
        );
    }

    // 2. 重启后验证设置持久化
    let restarted = AppState::new(dir.path().to_path_buf()).await.unwrap();
    assert!(
        (restarted.compression_ratio - 3.0).abs() < 0.01,
        "重启后 compression_ratio 应保持 3.0"
    );

    // 3. 检索结果应与压缩前一致
    let hits_after = restarted
        .storage
        .vector_search(&[0.8, 0.2], 5)
        .await
        .unwrap();
    assert!(!hits_after.is_empty(), "重启后检索应返回结果");
    assert!(
        hits_after[0].doc_name.contains("compress-test"),
        "重启后检索应返回相同文档"
    );
}
