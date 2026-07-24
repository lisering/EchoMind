#![allow(clippy::unwrap_used)]
//! QA 红灯测试：TC-DB-001 跨实例持久化 / TC-DB-002 僵尸崩溃恢复（REQ-DB-001）。

use echomind_core::Storage;
use echomind_models::{Chunk, DocStatus, Document};
use tempfile::TempDir;

use crate::sqlite_storage::SqliteStorage;

/// TC-DB-001 持久化：写入 → 销毁实例 → 重新实例化 → 数据仍在（REQ-DB-001-AC-1）。
#[tokio::test]
async fn tc_db_001_persistence_across_instances() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("echomind.db");

    let doc = Document::new("note.md".to_string(), "hash-note".to_string());
    let chunk = Chunk::new(doc.id.clone(), "持久化内容".to_string(), 4, 0);
    let chunk_id = chunk.id.clone();

    {
        let storage = SqliteStorage::new(&db_path).unwrap();
        storage.add_document(&doc).await.unwrap();
        storage.add_chunk(&chunk).await.unwrap();
        storage
            .add_embedding(&chunk_id, &[0.1, 0.2, 0.3])
            .await
            .unwrap();
    } // 实例与连接池在此销毁

    let reopened = SqliteStorage::new(&db_path).unwrap();
    let found = reopened.find_document_by_hash("hash-note").await.unwrap();
    assert!(found.is_some(), "重新实例化后文档必须仍在");
    let found = found.unwrap();
    assert_eq!(found.file_path, "note.md");
    assert_eq!(found.status, DocStatus::Pending);
    assert_eq!(reopened.count_documents().await.unwrap(), 1);

    // 旁证：向量已随库持久化且可检索
    let hits = reopened.vector_search(&[0.1, 0.2, 0.3], 1).await.unwrap();
    assert_eq!(hits.len(), 1, "持久化的向量必须可检索");
    assert_eq!(hits[0].doc_name, "note.md");
}

/// TC-DB-002 崩溃恢复：Processing 僵尸经清理后变 Failed（REQ-DB-001-AC-2）。
#[tokio::test]
async fn tc_db_002_zombie_cleanup_marks_failed() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("echomind.db");
    let storage = SqliteStorage::new(&db_path).unwrap();

    let mut zombie = Document::new("zombie.md".to_string(), "hash-zombie".to_string());
    zombie.status = DocStatus::Processing;
    storage.add_document(&zombie).await.unwrap();

    let cleaned = storage.cleanup_zombies().await.unwrap();
    assert_eq!(cleaned, 1, "必须清理恰好 1 条僵尸记录");

    let after = storage
        .find_document_by_hash("hash-zombie")
        .await
        .unwrap()
        .unwrap();
    assert!(
        matches!(after.status, DocStatus::Failed(_)),
        "僵尸状态必须变为 Failed"
    );
}
