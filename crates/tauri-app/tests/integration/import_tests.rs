#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! Import 相关集成测试 — 文档导入/分块/嵌入/多格式/重建索引。

use super::common::*;
use super::*;

// ==================================================================
// REQ-VEC-005 索引失败重试
// ==================================================================

/// TC-VEC-005 索引失败重试（REQ-VEC-005-AC-1）：
/// 调用 retry_index 后文档状态重置为 Pending，旧 chunks 被清理。
/// 重试最终因 embedder 不可用而失败（测试环境无 ONNX 模型），
/// 但状态重置与 chunks 清理逻辑得到验证。
#[tokio::test]
async fn tc_vec_005_retry_resets_and_clears() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 创建测试文件
    let md = dir.path().join("test.md");
    std::fs::write(&md, "# Test\n\nContent here.").unwrap();

    // 直接创建文档记录（跳过导入管线，避免 embedder 依赖）
    let doc = Document::new(
        md.to_string_lossy().into_owned(),
        "fake-hash-001".to_string(),
    );
    state.storage.add_document(&doc).await.unwrap();

    // 添加旧 chunks（模拟之前失败的索引结果）
    let chunk = Chunk::new(doc.id.clone(), "old content".to_string(), 10, 0);
    state.storage.add_chunk(&chunk).await.unwrap();

    // 设置为 Failed 状态
    state
        .storage
        .update_doc_status(&doc.id, DocStatus::Failed("test error".to_string()))
        .await
        .unwrap();

    // 验证前置条件
    let chunks_before = state.storage.list_chunks(&doc.id).await.unwrap();
    assert_eq!(chunks_before.len(), 1, "重试前应有 1 个旧 chunk");

    let app = tauri::test::mock_app();
    let handle = app.handle().clone();

    // 调用 retry_index_inner（会因 embedder 不可用而失败，但状态重置和清理已执行）
    let _ = retry_index_inner(&handle, &doc.id, &state).await;

    // 验证旧 chunks 被清理（delete_chunks_by_doc 被调用）
    // 索引会成功产生新 chunks，但嵌入失败（embedder 不可用）。
    // 旧 chunk（内容为 "old content"）应已被清理，不应出现在新 chunks 中。
    let chunks_after = state.storage.list_chunks(&doc.id).await.unwrap();
    assert!(
        chunks_after.iter().all(|c| c.content != "old content"),
        "旧 chunk 内容应被清理，不应出现在新 chunks 中"
    );

    // 文档记录应保留（delete_chunks_by_doc 只清理 chunks，不删文档）
    let docs = state.storage.list_documents().await.unwrap();
    assert!(docs.iter().any(|d| d.id == doc.id), "文档记录应保留");
}

// ==================================================================
// REQ-ING-005 删除文档与会话隔离
// ==================================================================

/// TC-ING-DEL-001 删除文档不影响会话与消息（REQ-ING-005-AC-3/AC-4/AC-5）：
/// 导入文档 → 创建会话 → 写入消息 → 删除文档 → 验证会话和消息仍然存在。
#[tokio::test]
async fn tc_ing_del_001_delete_document_preserves_conversations() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 1. 导入测试文档
    let md = dir.path().join("test-doc.md");
    std::fs::write(&md, "# 测试文档\n\n这是一段测试内容。").unwrap();
    let app = tauri::test::mock_app();
    let handle = app.handle().clone();
    let imported = import_files_inner(&handle, &[md.to_string_lossy().into_owned()], &state)
        .await
        .unwrap();
    assert_eq!(imported.len(), 1, "应导入 1 个文档");

    let docs = state.storage.list_documents().await.unwrap();
    assert_eq!(docs.len(), 1, "导入后应有 1 个文档");
    let doc_id = docs[0].id.clone();

    // 2. 创建会话并写入消息
    let conv_id = create_conversation_inner("default".to_string(), &state)
        .await
        .unwrap();
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

    let messages_before = get_messages_inner(&conv_id, &state).await.unwrap();
    assert_eq!(messages_before.len(), 2, "删除前应有 2 条消息");

    // 3. 删除文档
    delete_document_inner(&doc_id, &state).await.unwrap();

    // 4. 验证文档已被删除
    let docs_after = state.storage.list_documents().await.unwrap();
    assert!(docs_after.is_empty(), "删除后文档列表应为空");

    // 5. 验证 chunks 已级联删除
    let chunks = state.storage.list_chunks(&doc_id).await.unwrap();
    assert!(chunks.is_empty(), "删除后 chunks 应为空");

    // 6. 核心断言：会话仍然存在
    let convs = get_conversations_inner("default", &state).await.unwrap();
    assert_eq!(
        convs.len(),
        1,
        "删除文档后会话数量不应变化（REQ-ING-005-AC-3）"
    );
    assert_eq!(convs[0].id, conv_id, "会话 ID 应保持不变");

    // 7. 核心断言：消息仍然存在且内容不变
    let messages_after = get_messages_inner(&conv_id, &state).await.unwrap();
    assert_eq!(
        messages_after.len(),
        2,
        "删除文档后消息数量不应变化（REQ-ING-005-AC-3）"
    );
    assert_eq!(messages_after[0].content, "测试问题", "消息内容应保持不变");
    assert_eq!(messages_after[1].content, "测试回答", "消息内容应保持不变");
}

// ==================================================================
// REQ-ING-006 导入进度与取消
// ==================================================================

/// TC-ING-006 导入取消标志机制（REQ-ING-006-AC-1/AC-2）：
/// abort_import 设置取消标志，reset_import_cancel 重置标志，
/// import_cancel_flag 返回共享的 Arc<AtomicBool>。
/// import_files_inner 循环在文件边界检查标志并退出，已完成部分保留。
#[tokio::test]
async fn tc_ing_006_import_cancel_mechanism() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 初始状态：未取消
    let flag = state.import_cancel_flag();
    assert!(
        !flag.load(std::sync::atomic::Ordering::SeqCst),
        "初始状态不应取消"
    );

    // 设置取消
    state.abort_import();
    assert!(
        flag.load(std::sync::atomic::Ordering::SeqCst),
        "abort_import 后应取消"
    );

    // 重置取消
    state.reset_import_cancel();
    assert!(
        !flag.load(std::sync::atomic::Ordering::SeqCst),
        "reset 后不应取消"
    );

    // 验证 flag 与 state 共享同一 AtomicBool（修改 flag 不影响 state）
    flag.store(true, std::sync::atomic::Ordering::SeqCst);
    assert!(
        state
            .import_cancel_flag()
            .load(std::sync::atomic::Ordering::SeqCst),
        "flag 应与 state 共享同一 AtomicBool"
    );
    state.reset_import_cancel();
}
// =============================================================
// 多文档交叉检索集成测试（REQ-RAG-001~008）
// =============================================================
//
// 验证多文档知识库场景下的检索正确性：
// - 不同查询检索到不同文档
// - 删除文档后搜索不再返回该文档的 chunk
// - 重复内容导入去重
// - 跨文档检索结果正确标注来源
// - 大量 chunk 检索性能

use echomind_core::import::{ImportOutcome, ImportService};

/// TC-CROSS-001：多文档导入后——vector_search 返回不同文档的 chunk。
///
/// 导入 3 个不同主题的文档（Rust / Python / 烹饪），
/// 为每个 chunk 写入不同的向量，验证 vector_search 能根据向量相似度
/// 返回正确文档的 chunk，而非混合返回。
#[tokio::test]
async fn tc_cross_001_multi_doc_search_returns_correct_doc() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let service = ImportService::new(state.storage.clone(), state.data_dir.clone());

    // 导入 3 个文档
    let docs_info = [
        (
            "rust-guide.md",
            "Rust 是一门系统级编程语言，注重安全与性能。",
            [1.0, 0.0, 0.0, 0.0],
        ),
        (
            "python-tutorial.md",
            "Python 是一种高级解释型语言，适合快速开发。",
            [0.0, 1.0, 0.0, 0.0],
        ),
        (
            "cooking-recipes.md",
            "番茄炒蛋是一道简单的家常菜，需要鸡蛋和番茄。",
            [0.0, 0.0, 1.0, 0.0],
        ),
    ];

    let mut doc_ids = Vec::new();
    for (name, content, embedding) in &docs_info {
        let path = dir.path().join(name);
        std::fs::write(&path, content).unwrap();
        let canon = path.canonicalize().unwrap().to_string_lossy().into_owned();
        let outcome = service.import_one(&canon, true).await.unwrap();
        if let ImportOutcome::Imported(doc) = outcome {
            service.index_document(&doc).await.unwrap();
            let chunks = state.storage.list_chunks(&doc.id).await.unwrap();
            // 为每个 chunk 写入对应的向量
            for chunk in &chunks {
                state
                    .storage
                    .add_embedding(&chunk.id, embedding)
                    .await
                    .unwrap();
            }
            doc_ids.push(doc.id);
        }
    }

    // 验证文档数
    let doc_count = state.storage.count_documents().await.unwrap();
    assert_eq!(doc_count, 3, "应导入 3 个文档");

    // 搜索 Rust 相关内容（向量 [1,0,0,0]）
    // 注：vector_search 返回全量结果按余弦相似度排序，阈值过滤在 Retriever 层
    let rust_hits = state
        .storage
        .vector_search(&[1.0, 0.0, 0.0, 0.0], 5)
        .await
        .unwrap();
    assert!(!rust_hits.is_empty(), "Rust 搜索应返回结果");
    // 最高分（第一个）应来自 rust-guide.md（余弦相似度 1.0）
    assert!(
        rust_hits[0].doc_name.contains("rust-guide"),
        "最高分命中应来自 rust-guide.md，实际: {} (score={})",
        rust_hits[0].doc_name,
        rust_hits[0].score
    );
    assert!(
        rust_hits[0].score > 0.99,
        "Rust 向量自身余弦相似度应接近 1.0，实际: {}",
        rust_hits[0].score
    );

    // 搜索 Python 相关内容（向量 [0,1,0,0]）
    let python_hits = state
        .storage
        .vector_search(&[0.0, 1.0, 0.0, 0.0], 5)
        .await
        .unwrap();
    assert!(!python_hits.is_empty(), "Python 搜索应返回结果");
    assert!(
        python_hits[0].doc_name.contains("python-tutorial"),
        "最高分命中应来自 python-tutorial.md，实际: {} (score={})",
        python_hits[0].doc_name,
        python_hits[0].score
    );
    assert!(
        python_hits[0].score > 0.99,
        "Python 向量自身余弦相似度应接近 1.0，实际: {}",
        python_hits[0].score
    );

    // 搜索烹饪相关内容（向量 [0,0,1,0]）
    let cooking_hits = state
        .storage
        .vector_search(&[0.0, 0.0, 1.0, 0.0], 5)
        .await
        .unwrap();
    assert!(!cooking_hits.is_empty(), "烹饪搜索应返回结果");
    assert!(
        cooking_hits[0].doc_name.contains("cooking-recipes"),
        "最高分命中应来自 cooking-recipes.md，实际: {} (score={})",
        cooking_hits[0].doc_name,
        cooking_hits[0].score
    );
    assert!(
        cooking_hits[0].score > 0.99,
        "烹饪向量自身余弦相似度应接近 1.0，实际: {}",
        cooking_hits[0].score
    );
}

/// TC-CROSS-002：删除文档后——vector_search 不再返回该文档的 chunk。
///
/// 导入 2 个文档，删除其中一个，验证后续搜索只返回剩余文档的 chunk。
#[tokio::test]
async fn tc_cross_002_delete_doc_then_search_excludes_it() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let service = ImportService::new(state.storage.clone(), state.data_dir.clone());

    // 导入 2 个文档，使用相同向量以便搜索都能命中
    let docs = [("doc-a.md", "文档 A 内容"), ("doc-b.md", "文档 B 内容")];
    let mut doc_ids = Vec::new();
    for (name, content) in &docs {
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
                    .add_embedding(&chunk.id, &[1.0, 0.0])
                    .await
                    .unwrap();
            }
            doc_ids.push(doc.id);
        }
    }

    // 搜索前：两个文档都有命中
    let hits_before = state.storage.vector_search(&[1.0, 0.0], 10).await.unwrap();
    let doc_names_before: std::collections::HashSet<_> =
        hits_before.iter().map(|h| h.doc_name.clone()).collect();
    assert!(
        doc_names_before.iter().any(|n| n.contains("doc-a")),
        "删除前应包含 doc-a"
    );
    assert!(
        doc_names_before.iter().any(|n| n.contains("doc-b")),
        "删除前应包含 doc-b"
    );

    // 删除 doc-b
    delete_document_inner(&doc_ids[1], &state).await.unwrap();

    // 搜索后：只有 doc-a 的命中
    let hits_after = state.storage.vector_search(&[1.0, 0.0], 10).await.unwrap();
    let doc_names_after: std::collections::HashSet<_> =
        hits_after.iter().map(|h| h.doc_name.clone()).collect();
    assert!(
        doc_names_after.iter().any(|n| n.contains("doc-a")),
        "删除后仍应包含 doc-a"
    );
    assert!(
        !doc_names_after.iter().any(|n| n.contains("doc-b")),
        "删除后不应再包含 doc-b"
    );
}

/// TC-CROSS-003：重复导入相同内容——去重不增加文档数和 chunk 数。
#[tokio::test]
async fn tc_cross_003_duplicate_import_dedup() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let service = ImportService::new(state.storage.clone(), state.data_dir.clone());

    let path = dir.path().join("dedup.md");
    std::fs::write(&path, "# 去重测试\n\n这段内容用于测试文件去重功能。").unwrap();
    let canon = path.canonicalize().unwrap().to_string_lossy().into_owned();

    // 第一次导入
    let outcome1 = service.import_one(&canon, true).await.unwrap();
    assert!(
        matches!(outcome1, ImportOutcome::Imported(_)),
        "首次导入应成功"
    );

    // 第二次导入相同文件
    let outcome2 = service.import_one(&canon, true).await.unwrap();
    assert!(
        matches!(outcome2, ImportOutcome::SkippedDuplicate(_)),
        "重复导入应跳过"
    );

    // 文档数仍为 1
    let count = state.storage.count_documents().await.unwrap();
    assert_eq!(count, 1, "重复导入后文档数应仍为 1");
}

/// TC-CROSS-004：多文档多 chunk——检索 top_k 正确截断。
///
/// 导入 3 个文档，每个文档产生多个 chunk，
/// 验证 vector_search 的 top_k 参数正确截断结果数量。
#[tokio::test]
async fn tc_cross_004_multi_doc_top_k_truncation() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let service = ImportService::new(state.storage.clone(), state.data_dir.clone());

    // 导入 3 个文档，每个有多段内容（产生多个 chunk）
    for i in 0..3 {
        let path = dir.path().join(format!("multi-chunk-{i}.md"));
        let content = format!(
            "# 文档{i}\n\n段落一：主题{i}的介绍。\n\n段落二：主题{i}的详细说明。\n\n段落三：主题{i}的总结。"
        );
        std::fs::write(&path, content).unwrap();
        let canon = path.canonicalize().unwrap().to_string_lossy().into_owned();
        let outcome = service.import_one(&canon, true).await.unwrap();
        if let ImportOutcome::Imported(doc) = outcome {
            service.index_document(&doc).await.unwrap();
            let chunks = state.storage.list_chunks(&doc.id).await.unwrap();
            for chunk in &chunks {
                state
                    .storage
                    .add_embedding(&chunk.id, &[1.0, 0.0])
                    .await
                    .unwrap();
            }
        }
    }

    // 验证总 chunk 数 > 3
    let total_chunks = state.storage.count_chunks().await.unwrap();
    assert!(total_chunks >= 3, "总 chunk 数应 ≥ 3，实际: {total_chunks}");

    // top_k=1 应只返回 1 个结果
    let hits_1 = state.storage.vector_search(&[1.0, 0.0], 1).await.unwrap();
    assert_eq!(hits_1.len(), 1, "top_k=1 应返回 1 个结果");

    // top_k=2 应返回 2 个结果
    let hits_2 = state.storage.vector_search(&[1.0, 0.0], 2).await.unwrap();
    assert_eq!(hits_2.len(), 2, "top_k=2 应返回 2 个结果");
}

/// TC-CROSS-005：同名不同内容文件——REQ-ING-012 同名冲突检测 + 替换。
///
/// v1.12 变更：同名不同内容文件不再各自独立入库，而是触发 NameConflict，
/// 用户需通过 replace_and_import 确认替换。
#[tokio::test]
async fn tc_cross_005_same_name_diff_content_independent() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let service = ImportService::new(state.storage.clone(), state.data_dir.clone());

    // 创建两个同名文件（不同目录、不同内容）
    let dir_a = dir.path().join("a");
    let dir_b = dir.path().join("b");
    std::fs::create_dir_all(&dir_a).unwrap();
    std::fs::create_dir_all(&dir_b).unwrap();

    let path_a = dir_a.join("readme.md");
    let path_b = dir_b.join("readme.md");
    std::fs::write(&path_a, "内容 A：关于 Rust 编程。").unwrap();
    std::fs::write(&path_b, "内容 B：关于 Python 数据分析。").unwrap();

    let canon_a = path_a
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let canon_b = path_b
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();

    // 文件 A 首次导入成功
    let outcome_a = service.import_one(&canon_a, true).await.unwrap();
    assert!(
        matches!(outcome_a, ImportOutcome::Imported(_)),
        "文件 A 应导入成功"
    );

    // 文件 B 同名不同内容 → NameConflict（REQ-ING-012）
    let outcome_b = service.import_one(&canon_b, true).await.unwrap();
    match &outcome_b {
        ImportOutcome::NameConflict {
            old_doc_id,
            file_name,
        } => {
            assert!(!old_doc_id.is_empty(), "应返回旧文档 ID");
            assert_eq!(file_name, "readme.md", "应返回冲突文件名");
        }
        _ => panic!("同名不同内容应返回 NameConflict，实际: {outcome_b:?}"),
    }

    // 文档数应为 1（B 未导入）
    let count = state.storage.count_documents().await.unwrap();
    assert_eq!(count, 1, "同名冲突时文档数应为 1");

    // 通过 replace_and_import 替换
    let old_doc_id = match &outcome_b {
        ImportOutcome::NameConflict { old_doc_id, .. } => old_doc_id.clone(),
        _ => unreachable!(),
    };
    let outcome_replace = service
        .replace_and_import(&canon_b, &old_doc_id, true)
        .await
        .unwrap();
    assert!(
        matches!(outcome_replace, ImportOutcome::Imported(_)),
        "替换后应导入成功"
    );

    // 文档数仍为 1（替换，非新增）
    let count = state.storage.count_documents().await.unwrap();
    assert_eq!(count, 1, "替换后文档数仍应为 1");
}

/// TC-CROSS-006：文档全部删除后——count_documents 返回 0，chat 被拦截。
#[tokio::test]
async fn tc_cross_006_all_deleted_chat_blocked() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let service = ImportService::new(state.storage.clone(), state.data_dir.clone());

    // 导入一个文档
    let path = dir.path().join("temp.md");
    std::fs::write(&path, "临时内容").unwrap();
    let canon = path.canonicalize().unwrap().to_string_lossy().into_owned();
    let outcome = service.import_one(&canon, true).await.unwrap();
    let doc = match outcome {
        ImportOutcome::Imported(d) => d,
        _ => panic!("导入应成功"),
    };

    // 删除文档
    delete_document_inner(&doc.id, &state).await.unwrap();

    // 文档数为 0
    let count = state.storage.count_documents().await.unwrap();
    assert_eq!(count, 0, "删除后文档数应为 0");

    // chat 应被空知识库拦截
    update_llm_config_inner(test_config(), &state)
        .await
        .unwrap();
    let app = tauri::test::mock_app();
    let handle = app.handle().clone();
    let result = chat_inner(&handle, "任意问题", &[], "conv-x", None, None, &state).await;
    let err = result.err().unwrap_or_default();
    assert!(
        err.contains("知识库为空"),
        "全部删除后 chat 应被空知识库拦截，实际: {err}"
    );
}

/// TC-CROSS-007：多文档导入——文档列表正确返回所有文档。
#[tokio::test]
async fn tc_cross_007_multi_doc_list_all_returned() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let service = ImportService::new(state.storage.clone(), state.data_dir.clone());

    // 导入 5 个文档
    for i in 0..5 {
        let path = dir.path().join(format!("list-doc-{i}.md"));
        std::fs::write(&path, format!("文档 {i} 的内容")).unwrap();
        let canon = path.canonicalize().unwrap().to_string_lossy().into_owned();
        let outcome = service.import_one(&canon, true).await.unwrap();
        if let ImportOutcome::Imported(doc) = outcome {
            service.index_document(&doc).await.unwrap();
        }
    }

    // list_documents 应返回 5 个文档
    let docs = state.storage.list_documents().await.unwrap();
    assert_eq!(docs.len(), 5, "应返回 5 个文档，实际: {}", docs.len());

    // 每个文档的 status 应为 Indexed
    for doc in &docs {
        assert_eq!(
            doc.status,
            DocStatus::Indexed,
            "文档 {} 的状态应为 Indexed",
            doc.file_path
        );
    }
}

/// TC-CROSS-008：文档分块后——chunk sequence 连续无间隙。
///
/// 验证多文档导入后，每个文档的 chunk sequence 是从 0 开始的连续序列，
/// 没有 gap 或重复（影响 Chunk Expansion 的相邻 chunk 查找）。
#[tokio::test]
async fn tc_cross_008_chunk_sequence_continuous() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let service = ImportService::new(state.storage.clone(), state.data_dir.clone());

    // 导入一个长文档（产生多个 chunk）
    let path = dir.path().join("long-doc.md");
    let mut content = String::from("# 长文档\n\n");
    for i in 0..20 {
        content.push_str(&format!("段落 {i}：这是一段较长的文本内容，用于测试分块器的连续性。需要足够的文本量来触发多个 chunk 的生成。"));
    }
    std::fs::write(&path, &content).unwrap();
    let canon = path.canonicalize().unwrap().to_string_lossy().into_owned();
    let outcome = service.import_one(&canon, true).await.unwrap();
    if let ImportOutcome::Imported(doc) = outcome {
        service.index_document(&doc).await.unwrap();

        let chunks = state.storage.list_chunks(&doc.id).await.unwrap();
        assert!(
            chunks.len() > 1,
            "长文档应产生多个 chunk，实际: {}",
            chunks.len()
        );

        // 验证 sequence 从 0 开始连续
        for (idx, chunk) in chunks.iter().enumerate() {
            assert_eq!(
                chunk.sequence, idx,
                "chunk sequence 应从 0 连续递增，位置 {idx} 的 sequence 实际为 {}",
                chunk.sequence
            );
        }
    }
}

#[tokio::test]
async fn tc_edge_003_free_tier_50_doc_limit() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 确保是 Free 版本
    assert!(!*state.is_pro().read().await, "测试前提：Free 版本");

    // 直接写入 50 个文档到数据库（绕过 import_files 的限制检查）
    for i in 0..50 {
        let doc = Document::new(format!("/tmp/test-{i}.md"), format!("hash-{i}"));
        state.storage.add_document(&doc).await.unwrap();
    }

    // 验证已有 50 个文档
    let docs = state.storage.list_documents().await.unwrap();
    assert_eq!(docs.len(), 50, "应已有 50 个文档");

    // 尝试导入第 51 个文档 → 应被拒绝
    let test_file = dir.path().join("doc-51.md");
    std::fs::write(&test_file, "第 51 个文档内容").unwrap();

    let app = tauri::test::mock_app();
    let handle = app.handle().clone();
    let result =
        import_files_inner(&handle, &[test_file.to_string_lossy().to_string()], &state).await;

    let err = result.err().unwrap_or_default();
    assert!(
        err.contains("50") || err.contains("配额") || err.contains("quota"),
        "Free 版本 50 文档上限应拒绝导入第 51 个，实际: {err}"
    );
}

// ============================================================
// 自定义 ONNX 嵌入模型上传（REQ-VEC-014，Pro 门控）
// TC-VEC-CUSTOM-001~005
// ============================================================

/// 创建测试用的假 ONNX 文件（非空、非 HTML 的二进制数据）。
#[cfg(feature = "pro")]
fn create_fake_onnx_file(dir: &tempfile::TempDir, name: &str) -> std::path::PathBuf {
    let path = dir.path().join(name);
    // ONNX protobuf 文件不需要特定 magic bytes，只需要非空且非 HTML
    std::fs::write(&path, b"\x08\x01\x12\x04test\x1a\x03for").unwrap();
    path
}

/// 创建测试用的假 tokenizer 文件（有效 JSON）。
#[cfg(feature = "pro")]
fn create_fake_tokenizer_files(dir: &tempfile::TempDir) -> Vec<std::path::PathBuf> {
    let files = [
        "tokenizer.json",
        "config.json",
        "special_tokens_map.json",
        "tokenizer_config.json",
    ];
    files
        .iter()
        .map(|name| {
            let path = dir.path().join(name);
            std::fs::write(&path, r#"{"test": true}"#).unwrap();
            path
        })
        .collect()
}

/// TC-VEC-CUSTOM-001: 上传有效 ONNX 模型文件后，文件成功复制到 custom_models 目录。
///
/// 验证：AC-1 — 上传有效 ONNX 模型后文件复制成功，返回 CustomModelInfo。
/// 注：此测试验证文件管理逻辑，不验证实际 ONNX 模型加载（需要真实模型）。
#[cfg(feature = "pro")]
#[tokio::test]
async fn tc_vec_custom_001_upload_valid_model() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 激活 Pro（测试需要 Pro 权限）
    let license_key = make_valid_license();
    activate_pro_inner(&license_key, &state).await.unwrap();

    // 创建测试文件
    let onnx_path = create_fake_onnx_file(&dir, "model.onnx");
    let tokenizer_paths = create_fake_tokenizer_files(&dir);
    let tokenizer_str: Vec<String> = tokenizer_paths
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    // 上传模型
    let result = upload_custom_embedding_model_inner(
        "test-bge-model".to_string(),
        onnx_path.to_string_lossy().to_string(),
        tokenizer_str,
        &state,
    )
    .await;

    assert!(result.is_ok(), "上传有效模型应成功: {:?}", result.err());
    let info = result.unwrap();
    assert_eq!(info.name, "test-bge-model", "模型名称应匹配");
    assert!(info.is_valid, "模型应标记为有效");
    assert!(info.size_bytes > 0, "模型大小应大于 0");

    // 验证文件已复制到 custom_models 目录
    let model_dir = state.custom_model_dir().join("test-bge-model");
    assert!(model_dir.join("model.onnx").exists(), "ONNX 文件应已复制");
    assert!(
        model_dir.join("tokenizer.json").exists(),
        "tokenizer.json 应已复制"
    );
    assert!(
        model_dir.join("config.json").exists(),
        "config.json 应已复制"
    );
}

/// TC-VEC-CUSTOM-002: 切换到自定义模型后，设置持久化且 embedder 缓存清除。
///
/// 验证：AC-2 — 切换到自定义模型后 vec.embedding_model 设置为 "custom:{name}"。
#[cfg(feature = "pro")]
#[tokio::test]
async fn tc_vec_custom_002_switch_to_custom_model() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 切换到自定义模型
    set_embedding_model_inner("custom:my-custom-model", &state)
        .await
        .unwrap();

    // 验证设置已持久化
    let setting = state
        .storage
        .get_setting("vec.embedding_model")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        setting, "custom:my-custom-model",
        "vec.embedding_model 应为 custom:my-custom-model"
    );

    // 验证 embedder 缓存已清除（embedder_initialized 返回 false）
    let initialized = state.embedder_initialized().await;
    assert!(!initialized, "切换模型后 embedder 缓存应已清除");
}

/// TC-VEC-CUSTOM-003: 上传无效文件（HTML 错误页内容）返回 Err。
///
/// 验证：AC-3 — 上传无效文件返回 Err，不崩溃。
#[cfg(feature = "pro")]
#[tokio::test]
async fn tc_vec_custom_003_upload_invalid_file() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 激活 Pro
    let license_key = make_valid_license();
    activate_pro_inner(&license_key, &state).await.unwrap();

    // 创建无效 ONNX 文件（HTML 错误页内容）
    let invalid_path = dir.path().join("invalid.onnx");
    std::fs::write(
        &invalid_path,
        b"<!DOCTYPE html><html><body>404</body></html>",
    )
    .unwrap();

    let tokenizer_paths = create_fake_tokenizer_files(&dir);
    let tokenizer_str: Vec<String> = tokenizer_paths
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    let result = upload_custom_embedding_model_inner(
        "invalid-model".to_string(),
        invalid_path.to_string_lossy().to_string(),
        tokenizer_str,
        &state,
    )
    .await;

    assert!(result.is_err(), "上传无效 ONNX 文件应返回错误");
    let err = result.unwrap_err();
    assert!(
        err.contains("VALIDATION") || err.contains("STORAGE"),
        "错误应包含 VALIDATION 或 STORAGE 前缀: {err}"
    );
}

/// TC-VEC-CUSTOM-004: 删除自定义模型后文件目录被移除。
///
/// 验证：AC-4 — 删除自定义模型后文件删除，无法再切换。
#[cfg(feature = "pro")]
#[tokio::test]
async fn tc_vec_custom_004_delete_custom_model() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 激活 Pro
    let license_key = make_valid_license();
    activate_pro_inner(&license_key, &state).await.unwrap();

    // 先上传模型
    let onnx_path = create_fake_onnx_file(&dir, "model.onnx");
    let tokenizer_paths = create_fake_tokenizer_files(&dir);
    let tokenizer_str: Vec<String> = tokenizer_paths
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    upload_custom_embedding_model_inner(
        "delete-test-model".to_string(),
        onnx_path.to_string_lossy().to_string(),
        tokenizer_str,
        &state,
    )
    .await
    .unwrap();

    // 验证目录存在
    let model_dir = state.custom_model_dir().join("delete-test-model");
    assert!(model_dir.exists(), "上传后目录应存在");

    // 删除模型
    let result = delete_custom_model_inner("delete-test-model".to_string(), &state).await;
    assert!(result.is_ok(), "删除模型应成功: {:?}", result.err());

    // 验证目录已删除
    assert!(!model_dir.exists(), "删除后目录不应存在");

    // 验证再次删除返回错误
    let result2 = delete_custom_model_inner("delete-test-model".to_string(), &state).await;
    assert!(result2.is_err(), "删除不存在的模型应返回错误");
}

/// TC-VEC-CUSTOM-005: Free 用户上传被拒绝，返回 PRO_REQUIRED 错误。
///
/// 验证：AC-5 — Free 用户无法使用自定义模型上传功能。
///
/// v1.20.0 起：Free 用户上传/列出/删除均返回 PRO_REQUIRED 错误。
#[tokio::test]
async fn tc_vec_custom_005_free_user_rejected() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 确保是 Free 模式（不激活 Pro）
    let is_pro = *state.is_pro().read().await;
    assert!(!is_pro, "测试初始状态应为 Free 模式");

    // Free 用户上传应被拒绝
    #[cfg(feature = "pro")]
    {
        // 创建测试文件（仅在 Pro 编译时需要）
        let onnx_path = create_fake_onnx_file(&dir, "model.onnx");
        let tokenizer_paths = create_fake_tokenizer_files(&dir);
        let tokenizer_str: Vec<String> = tokenizer_paths
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();

        let result = upload_custom_embedding_model_inner(
            "test-model".to_string(),
            onnx_path.to_string_lossy().to_string(),
            tokenizer_str.clone(),
            &state,
        )
        .await;
        assert!(result.is_err(), "Free 用户上传应被拒绝");
        let err = result.unwrap_err();
        assert!(
            err.contains("PRO_REQUIRED"),
            "错误应包含 PRO_REQUIRED 前缀: {err}"
        );
    }

    // Free 用户列出模型应被拒绝
    #[cfg(feature = "pro")]
    {
        let result = list_custom_models_inner(&state).await;
        assert!(result.is_err(), "Free 用户列出自定义模型应被拒绝");
        let err = result.unwrap_err();
        assert!(
            err.contains("PRO_REQUIRED"),
            "错误应包含 PRO_REQUIRED 前缀: {err}"
        );
    }

    // Free 用户删除模型应被拒绝
    #[cfg(feature = "pro")]
    {
        let result = delete_custom_model_inner("test-model".to_string(), &state).await;
        assert!(result.is_err(), "Free 用户删除自定义模型应被拒绝");
        let err = result.unwrap_err();
        assert!(
            err.contains("PRO_REQUIRED"),
            "错误应包含 PRO_REQUIRED 前缀: {err}"
        );
    }
}

/// TC-VEC-CUSTOM-006: list_custom_models 返回已上传的模型列表。
///
/// 验证：上传多个模型后，list_custom_models 返回完整列表。
#[cfg(feature = "pro")]
#[tokio::test]
async fn tc_vec_custom_006_list_custom_models() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 激活 Pro
    let license_key = make_valid_license();
    activate_pro_inner(&license_key, &state).await.unwrap();

    // 上传两个模型
    for model_name in &["model-a", "model-b"] {
        let onnx_path = create_fake_onnx_file(&dir, "model.onnx");
        let tokenizer_paths = create_fake_tokenizer_files(&dir);
        let tokenizer_str: Vec<String> = tokenizer_paths
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        upload_custom_embedding_model_inner(
            model_name.to_string(),
            onnx_path.to_string_lossy().to_string(),
            tokenizer_str,
            &state,
        )
        .await
        .unwrap();
    }

    // 列出模型
    let result = list_custom_models_inner(&state).await;
    assert!(result.is_ok(), "列出自定义模型应成功");
    let models = result.unwrap();
    assert_eq!(models.len(), 2, "应有 2 个自定义模型");
    let names: Vec<&str> = models.iter().map(|m| m.name.as_str()).collect();
    assert!(names.contains(&"model-a"), "列表应包含 model-a");
    assert!(names.contains(&"model-b"), "列表应包含 model-b");
}

/// TC-VEC-CUSTOM-007: 路径遍历防护 — 模型名称中的危险字符被替换。
///
/// 验证：模型名称 `../../etc/passwd` 被清理为安全目录名。
#[cfg(feature = "pro")]
#[tokio::test]
async fn tc_vec_custom_007_path_traversal_protection() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 激活 Pro
    let license_key = make_valid_license();
    activate_pro_inner(&license_key, &state).await.unwrap();

    let onnx_path = create_fake_onnx_file(&dir, "model.onnx");
    let tokenizer_paths = create_fake_tokenizer_files(&dir);
    let tokenizer_str: Vec<String> = tokenizer_paths
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    // 使用危险路径作为模型名称
    let result = upload_custom_embedding_model_inner(
        "../../etc/passwd".to_string(),
        onnx_path.to_string_lossy().to_string(),
        tokenizer_str,
        &state,
    )
    .await;

    assert!(
        result.is_ok(),
        "上传应成功（名称被清理）: {:?}",
        result.err()
    );
    let info = result.unwrap();
    // 危险字符应被替换
    assert!(
        !info.name.contains('/') && !info.name.contains('\\') && !info.name.contains(".."),
        "模型名称不应包含路径分隔符或 ..: {}",
        info.name
    );

    // 验证文件存储在 custom_models 目录下（非系统目录）
    let model_dir = state.custom_model_dir().join(&info.name);
    assert!(model_dir.exists(), "模型应存储在 custom_models 目录下");
}

/// TC-VEC-REBUILD-001 rebuild_index 对 Indexed 文档清理旧 chunks 并重新索引（REQ-VEC-009-AC-2/AC-3）。
#[tokio::test]
async fn tc_vec_rebuild_001_rebuild_index_clears_and_reindexes() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 导入文件
    let src_file = dir.path().join("source").join("rebuild_test.md");
    std::fs::create_dir_all(src_file.parent().unwrap()).unwrap();
    std::fs::write(
        &src_file,
        b"# Rebuild Test\n\nSome content for rebuild testing.",
    )
    .unwrap();

    let app = tauri::test::mock_app();
    let handle = app.handle().clone();
    let imported = import_files_inner(&handle, &[src_file.to_string_lossy().to_string()], &state)
        .await
        .unwrap();
    assert_eq!(imported.len(), 1);

    let docs = state.storage.list_documents().await.unwrap();
    let doc = &docs[0];
    assert_eq!(doc.status, DocStatus::Indexed, "导入后应为 Indexed");

    // 记录原始 chunk 数
    let original_chunks = state.storage.list_chunks(&doc.id).await.unwrap();
    assert!(!original_chunks.is_empty(), "应有 chunks");

    // 执行重建索引
    rebuild_index_inner(&handle, &doc.id, &state).await.unwrap();

    // 验证状态恢复为 Indexed（AC-3）
    let docs_after = state.storage.list_documents().await.unwrap();
    let doc_after = docs_after
        .iter()
        .find(|d| d.id == doc.id)
        .expect("文档应存在");
    assert_eq!(
        doc_after.status,
        DocStatus::Indexed,
        "重建后状态应恢复为 Indexed（AC-3）"
    );

    // 验证 chunks 仍然存在（重新索引后应有 chunks）
    let rebuilt_chunks = state.storage.list_chunks(&doc.id).await.unwrap();
    assert!(!rebuilt_chunks.is_empty(), "重建后应有 chunks");
}

/// TC-VEC-REBUILD-002 不存在的文档 ID 返回 Err（REQ-VEC-009-AC-1）。
#[tokio::test]
async fn tc_vec_rebuild_002_rebuild_nonexistent_doc_returns_error() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    let app = tauri::test::mock_app();
    let handle = app.handle().clone();
    let result = rebuild_index_inner(&handle, "nonexistent-id", &state).await;

    assert!(result.is_err(), "不存在的文档 ID 必须返回 Err");
    assert!(
        result.unwrap_err().contains("文档不存在"),
        "错误消息应包含「文档不存在」"
    );
}
