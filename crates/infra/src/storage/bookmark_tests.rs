//! 消息级书签 TDD 测试（REQ-RAG-053）。
//!
//! 测试 TC-RAG-BMK-001~005：
//! - AC-1：添加消息级书签（message_id + summary 字段正确存储）
//! - AC-2：列出书签包含消息级书签摘要
//! - AC-3：按 message_id 查询书签
//! - AC-4：书签备注设置
//! - AC-5：删除书签后列表更新

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::sqlite_storage::SqliteStorage;
use echomind_core::Storage;
use tempfile::TempDir;

/// 创建临时测试数据库。
async fn make_storage() -> (SqliteStorage, TempDir) {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test_bookmarks.db");
    let storage = SqliteStorage::new(&db_path).unwrap();
    (storage, dir)
}

/// TC-RAG-BMK-001：添加消息级书签（AC-1）。
///
/// 验证 `add_bookmark` 传入 `message_id` + `summary` 后正确存储到数据库。
#[tokio::test]
async fn tc_rag_bmk_001_add_message_bookmark() {
    let (storage, _dir) = make_storage().await;

    // 创建测试会话
    let conv = echomind_models::Conversation {
        id: "conv-001".to_string(),
        workspace_id: "default".to_string(),
        title: "Test Conversation".to_string(),
        created_at: chrono::Utc::now().timestamp(),
        sort_order: 0,
    };
    storage.create_conversation(&conv).await.unwrap();

    // 添加消息级书签
    storage
        .add_bookmark(
            "conv-001",
            Some("重要回答"),
            Some("msg-abc"),
            Some("这是 AI 的回答摘要"),
        )
        .await
        .unwrap();

    // 验证书签已存储
    let bookmarks = storage.list_bookmarks().await.unwrap();
    assert_eq!(bookmarks.len(), 1);
    let bm = &bookmarks[0];
    assert_eq!(bm.conversation_id, "conv-001");
    assert_eq!(bm.message_id.as_deref(), Some("msg-abc"));
    assert_eq!(bm.summary.as_deref(), Some("这是 AI 的回答摘要"));
    assert_eq!(bm.note.as_deref(), Some("重要回答"));
    assert!(bm.is_message_bookmark());
}

/// TC-RAG-BMK-002：列出书签包含消息级书签摘要（AC-2）。
///
/// 验证 `list_bookmarks` 返回的消息级书签包含 summary 字段。
#[tokio::test]
async fn tc_rag_bmk_002_list_with_summary() {
    let (storage, _dir) = make_storage().await;

    // 创建测试会话
    let conv1 = echomind_models::Conversation {
        id: "conv-001".to_string(),
        workspace_id: "default".to_string(),
        title: "Test 1".to_string(),
        created_at: chrono::Utc::now().timestamp(),
        sort_order: 0,
    };
    storage.create_conversation(&conv1).await.unwrap();

    // 添加会话级书签（无 message_id）
    storage
        .add_bookmark("conv-001", Some("会话备注"), None, None)
        .await
        .unwrap();

    // 添加消息级书签
    let conv2 = echomind_models::Conversation {
        id: "conv-002".to_string(),
        workspace_id: "default".to_string(),
        title: "Test 2".to_string(),
        created_at: chrono::Utc::now().timestamp(),
        sort_order: 0,
    };
    storage.create_conversation(&conv2).await.unwrap();
    storage
        .add_bookmark("conv-002", None, Some("msg-xyz"), Some("消息摘要内容"))
        .await
        .unwrap();

    // 验证列表
    let bookmarks = storage.list_bookmarks().await.unwrap();
    assert_eq!(bookmarks.len(), 2);

    // 找到消息级书签
    let msg_bm = bookmarks.iter().find(|b| b.is_message_bookmark()).unwrap();
    assert_eq!(msg_bm.summary.as_deref(), Some("消息摘要内容"));
    assert_eq!(msg_bm.message_id.as_deref(), Some("msg-xyz"));

    // 找到会话级书签
    let conv_bm = bookmarks.iter().find(|b| !b.is_message_bookmark()).unwrap();
    assert!(conv_bm.message_id.is_none());
    assert!(conv_bm.summary.is_none());
}

/// TC-RAG-BMK-003：按 message_id 查询书签（AC-3）。
///
/// 验证 `get_message_bookmark` 正确返回对应消息的书签。
#[tokio::test]
async fn tc_rag_bmk_003_get_by_message_id() {
    let (storage, _dir) = make_storage().await;

    let conv = echomind_models::Conversation {
        id: "conv-001".to_string(),
        workspace_id: "default".to_string(),
        title: "Test".to_string(),
        created_at: chrono::Utc::now().timestamp(),
        sort_order: 0,
    };
    storage.create_conversation(&conv).await.unwrap();

    // 添加消息级书签
    storage
        .add_bookmark("conv-001", None, Some("msg-target"), Some("目标消息摘要"))
        .await
        .unwrap();

    // 按消息 ID 查询
    let result = storage.get_message_bookmark("msg-target").await.unwrap();
    assert!(result.is_some());
    let bm = result.unwrap();
    assert_eq!(bm.conversation_id, "conv-001");
    assert_eq!(bm.message_id.as_deref(), Some("msg-target"));

    // 查询不存在的消息
    let not_found = storage
        .get_message_bookmark("msg-nonexistent")
        .await
        .unwrap();
    assert!(not_found.is_none());
}

/// TC-RAG-BMK-004：书签备注设置（AC-4）。
///
/// 验证书签可添加备注，重复添加时更新备注。
#[tokio::test]
async fn tc_rag_bmk_004_bookmark_note() {
    let (storage, _dir) = make_storage().await;

    let conv = echomind_models::Conversation {
        id: "conv-001".to_string(),
        workspace_id: "default".to_string(),
        title: "Test".to_string(),
        created_at: chrono::Utc::now().timestamp(),
        sort_order: 0,
    };
    storage.create_conversation(&conv).await.unwrap();

    // 初始添加书签带备注
    storage
        .add_bookmark("conv-001", Some("原始备注"), Some("msg-1"), Some("摘要"))
        .await
        .unwrap();

    let bm = storage
        .get_message_bookmark("msg-1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(bm.note.as_deref(), Some("原始备注"));

    // 重复添加（更新备注）
    storage
        .add_bookmark(
            "conv-001",
            Some("更新备注"),
            Some("msg-1"),
            Some("更新摘要"),
        )
        .await
        .unwrap();

    let bm = storage
        .get_message_bookmark("msg-1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(bm.note.as_deref(), Some("更新备注"));
    assert_eq!(bm.summary.as_deref(), Some("更新摘要"));
}

/// TC-RAG-BMK-005：删除书签后列表更新（AC-5）。
///
/// 验证 `remove_bookmark` 后书签从列表中消失。
#[tokio::test]
async fn tc_rag_bmk_005_delete_updates_list() {
    let (storage, _dir) = make_storage().await;

    let conv = echomind_models::Conversation {
        id: "conv-001".to_string(),
        workspace_id: "default".to_string(),
        title: "Test".to_string(),
        created_at: chrono::Utc::now().timestamp(),
        sort_order: 0,
    };
    storage.create_conversation(&conv).await.unwrap();

    // 添加书签
    storage
        .add_bookmark("conv-001", None, Some("msg-1"), Some("摘要"))
        .await
        .unwrap();

    // 验证存在
    assert_eq!(storage.list_bookmarks().await.unwrap().len(), 1);

    // 删除书签
    storage.remove_bookmark("conv-001").await.unwrap();

    // 验证列表为空
    assert_eq!(storage.list_bookmarks().await.unwrap().len(), 0);

    // 验证按消息 ID 查询也返回 None
    assert!(
        storage
            .get_message_bookmark("msg-1")
            .await
            .unwrap()
            .is_none()
    );
}
