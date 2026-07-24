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

/// TC-DB-003 旧 schema 迁移守卫：模拟旧版数据库（embeddings 列名 embedding 而非 vector，
/// chunks 含 session_id 列），验证 migrate_schema 正确重建表且后续 vector_search 不崩溃。
///
/// 教训：Phase 8.5 混沌测试虽用真实文档，但只测到 import+index（load+split+chunks 入库），
/// **未测 embed+search 链路**。真实用户旧数据库的 embeddings 表列名不匹配导致
/// vector_search SQL 崩溃。本测试直接构造旧 schema 数据库验证迁移。
#[tokio::test]
async fn tc_db_003_old_schema_embeddings_migration() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("echomind.db");

    // 步骤 1：手动构造旧版 schema 数据库（模拟用户从旧版本升级）
    {
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        // 旧版 embeddings 表：列名 embedding，含 session_id / model_name
        conn.execute_batch(
            "CREATE TABLE embeddings (
                chunk_id TEXT PRIMARY KEY,
                embedding BLOB NOT NULL,
                session_id TEXT,
                model_name TEXT NOT NULL DEFAULT 'all-MiniLM-L6-v2'
            );",
        )
        .unwrap();
        // 旧版 chunks 表：含 session_id 列
        conn.execute_batch(
            "CREATE TABLE chunks (
                id TEXT PRIMARY KEY,
                doc_id TEXT NOT NULL,
                content TEXT NOT NULL,
                token_count INTEGER NOT NULL,
                sequence INTEGER NOT NULL,
                session_id TEXT
            );",
        )
        .unwrap();
        // documents 表（与新版一致，不需要迁移）
        conn.execute_batch(
            "CREATE TABLE documents (
                id TEXT PRIMARY KEY,
                file_path TEXT NOT NULL,
                file_hash TEXT NOT NULL,
                status TEXT NOT NULL,
                status_reason TEXT,
                created_at INTEGER NOT NULL
            );",
        )
        .unwrap();
        // 插入旧版数据
        conn.execute(
            "INSERT INTO documents (id, file_path, file_hash, status, status_reason, created_at)
             VALUES ('doc-1', 'old.md', 'hash-old', 'pending', NULL, 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO chunks (id, doc_id, content, token_count, sequence, session_id)
             VALUES ('chunk-1', 'doc-1', 'old content', 2, 0, 'sess-1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO embeddings (chunk_id, embedding, session_id, model_name)
             VALUES ('chunk-1', X'AABBCCDD', 'sess-1', 'old-model')",
            [],
        )
        .unwrap();
    }

    // 步骤 2：用新版 SqliteStorage 打开（触发 migrate_schema）
    let storage = SqliteStorage::new(&db_path).unwrap();

    // 步骤 3：验证旧 embeddings 表已被重建（vector 列存在，旧数据已清除）
    // 写入新向量并检索——若迁移失败，vector_search 会崩溃
    let doc = Document::new("new.md".to_string(), "hash-new".to_string());
    storage.add_document(&doc).await.unwrap();
    let chunk = Chunk::new(doc.id.clone(), "new content".to_string(), 2, 0);
    let chunk_id = chunk.id.clone();
    storage.add_chunk(&chunk).await.unwrap();
    storage
        .add_embedding(&chunk_id, &[0.5, 0.5, 0.5])
        .await
        .unwrap();

    // 步骤 4：vector_search 必须不崩溃且返回结果
    let hits = storage.vector_search(&[0.5, 0.5, 0.5], 5).await.unwrap();
    assert_eq!(hits.len(), 1, "迁移后 vector_search 必须正常返回");
    assert_eq!(hits[0].doc_name, "new.md");
    assert!(
        hits[0].score > 0.99,
        "自身向量余弦相似度应接近 1.0，实际: {}",
        hits[0].score
    );

    // 步骤 5：旧 embeddings 数据已随表删除清除，旧 chunks 也已随表删除清除。
    // documents 表 schema 未变化（不触发迁移），旧文档记录可能残留（无外键依赖），
    // 但其 chunks/embeddings 已不存在，不会影响检索。验证新文档可被检索即可。
    let count = storage.count_documents().await.unwrap();
    assert!(
        count >= 1,
        "至少应有新写入的 1 个文档，实际: {count}"
    );
}
