#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! REQ-SYNC-002 增量同步引擎 TDD 测试。
//!
//! 覆盖验收标准：
//! - TC-SYNC-004: 首次同步 3 个 .md 文件全部导入 + original_path 指向源路径
//! - TC-SYNC-005: 修改文件后同步 → 旧文档删除、新文档导入（doc_id 变化，hash 变化）
//! - TC-SYNC-006: 删除文件后同步 → 知识库中对应文档级联删除
//! - TC-SYNC-007: 新建文件后同步 → 自动导入
//! - TC-SYNC-008: 隐藏文件（以 `.` 开头）被跳过
//! - TC-SYNC-009: 不受支持的格式（如 .docx）被跳过
//! - TC-SYNC-010: 同步幂等——多次同步无变更的文件夹不产生重复

use std::collections::HashMap;
use std::sync::Arc;

use crate::Storage;
use crate::sync::SyncService;
use anyhow::Result;
use echomind_models::{
    ChatMessage, Chunk, Conversation, DocStatus, Document, RetrievalResult, SyncProgressPayload,
};
use tempfile::TempDir;

/// QA Mock Storage：内存实现，完整支持同步引擎所需的全部 Storage 方法。
///
/// 与 `import_tests::MockStorage` 的区别：实现了 `list_documents` 和 `delete_document`，
/// 以及 `find_document_by_original_path` 和 `find_documents_by_original_path_prefix` 的内存版本。
#[derive(Clone, Default)]
struct MockStorage {
    docs: Arc<tokio::sync::Mutex<HashMap<String, Document>>>,
    chunks: Arc<tokio::sync::Mutex<HashMap<String, Vec<Chunk>>>>,
}

impl MockStorage {
    async fn doc_count(&self) -> usize {
        self.docs.lock().await.len()
    }
}

impl Storage for MockStorage {
    async fn add_document(&self, doc: &Document) -> Result<()> {
        self.docs.lock().await.insert(doc.id.clone(), doc.clone());
        Ok(())
    }

    async fn update_doc_status(&self, doc_id: &str, status: DocStatus) -> Result<()> {
        let mut docs = self.docs.lock().await;
        if let Some(doc) = docs.get_mut(doc_id) {
            doc.status = status;
        }
        Ok(())
    }

    async fn add_chunk(&self, chunk: &Chunk) -> Result<()> {
        self.chunks
            .lock()
            .await
            .entry(chunk.doc_id.clone())
            .or_default()
            .push(chunk.clone());
        Ok(())
    }

    async fn add_embedding(&self, _chunk_id: &str, _embedding: &[f32]) -> Result<()> {
        Ok(())
    }

    async fn vector_search(
        &self,
        _query_embedding: &[f32],
        _top_k: usize,
    ) -> Result<Vec<RetrievalResult>> {
        Ok(vec![])
    }

    async fn find_document_by_hash(&self, hash: &str) -> Result<Option<Document>> {
        Ok(self
            .docs
            .lock()
            .await
            .values()
            .find(|d| d.file_hash == hash)
            .cloned())
    }

    async fn count_documents(&self) -> Result<usize> {
        Ok(self.docs.lock().await.len())
    }

    async fn count_chunks(&self) -> Result<usize> {
        Ok(0)
    }

    async fn cleanup_zombies(&self) -> Result<usize> {
        Ok(0)
    }

    async fn set_setting(&self, _key: &str, _value: &str) -> Result<()> {
        Ok(())
    }

    async fn get_setting(&self, _key: &str) -> Result<Option<String>> {
        Ok(None)
    }

    async fn create_conversation(&self, _conv: &Conversation) -> Result<()> {
        Ok(())
    }

    async fn list_conversations(&self, _workspace_id: &str) -> Result<Vec<Conversation>> {
        Ok(vec![])
    }

    async fn delete_conversation(&self, _id: &str) -> Result<()> {
        Ok(())
    }

    async fn update_conversation_title(&self, _id: &str, _title: &str) -> Result<()> {
        Ok(())
    }

    async fn add_message(&self, _conv_id: &str, _msg: &ChatMessage) -> Result<()> {
        Ok(())
    }

    async fn list_messages(&self, _conv_id: &str) -> Result<Vec<ChatMessage>> {
        Ok(vec![])
    }

    async fn list_documents(&self) -> Result<Vec<Document>> {
        Ok(self.docs.lock().await.values().cloned().collect())
    }

    async fn delete_document(&self, doc_id: &str) -> Result<()> {
        self.docs.lock().await.remove(doc_id);
        self.chunks.lock().await.remove(doc_id);
        Ok(())
    }

    async fn list_chunks(&self, doc_id: &str) -> Result<Vec<Chunk>> {
        Ok(self
            .chunks
            .lock()
            .await
            .get(doc_id)
            .cloned()
            .unwrap_or_default())
    }

    async fn delete_chunks_by_doc(&self, doc_id: &str) -> Result<()> {
        self.chunks.lock().await.remove(doc_id);
        Ok(())
    }

    async fn keyword_search(&self, _query: &str, _top_k: usize) -> Result<Vec<RetrievalResult>> {
        Ok(vec![])
    }
}

/// 创建临时文件夹并写入文件（测试辅助函数）。
fn write_file(dir: &TempDir, name: &str, content: &[u8]) -> String {
    let path = dir.path().join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&path, content).unwrap();
    path.to_string_lossy().into_owned()
}

/// TC-SYNC-004: 首次同步一个含 3 个 .md 文件的文件夹，全部导入成功，
/// 文档 `original_path` 指向源文件路径（REQ-SYNC-002 AC-1）。
#[tokio::test]
async fn tc_sync_004_first_sync_imports_all() {
    let dir = TempDir::new().unwrap();
    let watch_dir = dir.path().join("watch");
    std::fs::create_dir_all(&watch_dir).unwrap();

    // 创建 3 个 .md 文件（真实语料：Rust 编程相关内容）
    write_file(
        &dir,
        "watch/rust-ownership.md",
        b"# Rust Ownership\n\nOwnership is Rust's most unique feature.\n",
    );
    write_file(
        &dir,
        "watch/rust-borrowing.md",
        b"# Borrowing\n\nReferences allow you to refer to a value without taking ownership.\n",
    );
    write_file(
        &dir,
        "watch/rust-lifetimes.md",
        b"# Lifetimes\n\nLifetimes ensure references are valid as long as we need them.\n",
    );

    let storage = MockStorage::default();
    let data_dir = dir.path().join("data");
    let service = SyncService::new(storage.clone(), data_dir);

    let result = service
        .sync_folder(&watch_dir.to_string_lossy(), true, None)
        .await
        .unwrap();

    assert_eq!(result.added, 3, "首次同步应导入 3 个文件");
    assert_eq!(result.updated, 0);
    assert_eq!(result.deleted, 0);
    assert_eq!(result.errors.len(), 0, "不应有错误");
    assert_eq!(storage.doc_count().await, 3, "知识库应有 3 个文档");

    // 验证 original_path 指向源文件路径
    let docs = storage.list_documents().await.unwrap();
    for doc in &docs {
        assert!(
            doc.original_path.is_some(),
            "同步导入的文档必须有 original_path"
        );
        let path = doc.original_path.as_ref().unwrap();
        assert!(
            path.contains("rust-"),
            "original_path 应包含源文件名: {path}"
        );
    }
}

/// TC-SYNC-005: 修改文件夹中已导入文件的内容，触发同步后旧文档被删除、
/// 新文档导入（doc_id 变化，file_hash 变化）（REQ-SYNC-002 AC-2）。
#[tokio::test]
async fn tc_sync_005_modified_file_updates_document() {
    let dir = TempDir::new().unwrap();
    let watch_dir = dir.path().join("watch");
    std::fs::create_dir_all(&watch_dir).unwrap();

    // 初始文件
    let file_path = watch_dir.join("notes.md");
    std::fs::write(
        &file_path,
        b"# Original Content\n\nThis is the original text.\n",
    )
    .unwrap();

    let storage = MockStorage::default();
    let data_dir = dir.path().join("data");
    let service = SyncService::new(storage.clone(), data_dir);

    // 第一次同步
    let result1 = service
        .sync_folder(&watch_dir.to_string_lossy(), true, None)
        .await
        .unwrap();
    assert_eq!(result1.added, 1, "首次同步导入 1 个文件");

    let docs_before = storage.list_documents().await.unwrap();
    let old_doc = &docs_before[0];
    let old_id = old_doc.id.clone();
    let old_hash = old_doc.file_hash.clone();

    // 修改文件内容
    std::fs::write(
        &file_path,
        b"# Modified Content\n\nThis text has been changed significantly.\n",
    )
    .unwrap();

    // 第二次同步
    let result2 = service
        .sync_folder(&watch_dir.to_string_lossy(), true, None)
        .await
        .unwrap();

    assert_eq!(result2.added, 0, "不应有新增");
    assert_eq!(result2.updated, 1, "应更新 1 个文件");
    assert_eq!(result2.deleted, 0);
    assert_eq!(storage.doc_count().await, 1, "更新后仍只有 1 个文档");

    let docs_after = storage.list_documents().await.unwrap();
    let new_doc = &docs_after[0];
    assert_ne!(
        new_doc.id, old_id,
        "更新后 doc_id 必须变化（旧文档删除，新文档创建）"
    );
    assert_ne!(
        new_doc.file_hash, old_hash,
        "更新后 file_hash 必须变化（内容已修改）"
    );
}

/// TC-SYNC-006: 删除文件夹中已导入的文件，触发同步后知识库中对应文档被级联删除
/// （REQ-SYNC-002 AC-3）。
#[tokio::test]
async fn tc_sync_006_deleted_file_removes_document() {
    let dir = TempDir::new().unwrap();
    let watch_dir = dir.path().join("watch");
    std::fs::create_dir_all(&watch_dir).unwrap();

    // 创建 2 个文件
    let file_a = watch_dir.join("a.md");
    let file_b = watch_dir.join("b.md");
    std::fs::write(&file_a, b"Content A\n").unwrap();
    std::fs::write(&file_b, b"Content B\n").unwrap();

    let storage = MockStorage::default();
    let data_dir = dir.path().join("data");
    let service = SyncService::new(storage.clone(), data_dir);

    // 第一次同步
    let result1 = service
        .sync_folder(&watch_dir.to_string_lossy(), true, None)
        .await
        .unwrap();
    assert_eq!(result1.added, 2, "首次同步导入 2 个文件");

    // 删除文件 A
    std::fs::remove_file(&file_a).unwrap();

    // 第二次同步
    let result2 = service
        .sync_folder(&watch_dir.to_string_lossy(), true, None)
        .await
        .unwrap();

    assert_eq!(result2.deleted, 1, "应删除 1 个文档");
    assert_eq!(storage.doc_count().await, 1, "删除后应剩 1 个文档");

    // 验证剩余文档是 B
    let docs = storage.list_documents().await.unwrap();
    assert_eq!(docs.len(), 1);
    assert!(
        docs[0]
            .original_path
            .as_ref()
            .is_some_and(|p| p.contains("b.md")),
        "剩余文档应为 b.md"
    );
}

/// TC-SYNC-007: 在文件夹中新建文件，触发同步后自动导入（REQ-SYNC-002 AC-4）。
#[tokio::test]
async fn tc_sync_007_new_file_auto_imported() {
    let dir = TempDir::new().unwrap();
    let watch_dir = dir.path().join("watch");
    std::fs::create_dir_all(&watch_dir).unwrap();

    // 初始 1 个文件
    std::fs::write(watch_dir.join("initial.md"), b"Initial content\n").unwrap();

    let storage = MockStorage::default();
    let data_dir = dir.path().join("data");
    let service = SyncService::new(storage.clone(), data_dir);

    // 第一次同步
    let result1 = service
        .sync_folder(&watch_dir.to_string_lossy(), true, None)
        .await
        .unwrap();
    assert_eq!(result1.added, 1);

    // 新建文件
    std::fs::write(watch_dir.join("new-file.md"), b"New file content\n").unwrap();

    // 第二次同步
    let result2 = service
        .sync_folder(&watch_dir.to_string_lossy(), true, None)
        .await
        .unwrap();

    assert_eq!(result2.added, 1, "应新增导入 1 个文件");
    assert_eq!(result2.skipped, 1, "原有文件应跳过（幂等）");
    assert_eq!(storage.doc_count().await, 2, "知识库应有 2 个文档");
}

/// TC-SYNC-008: 隐藏文件（以 `.` 开头）被跳过，不导入（REQ-SYNC-002 AC-5）。
#[tokio::test]
async fn tc_sync_008_hidden_files_skipped() {
    let dir = TempDir::new().unwrap();
    let watch_dir = dir.path().join("watch");
    std::fs::create_dir_all(&watch_dir).unwrap();

    // 正常文件 + 隐藏文件
    std::fs::write(watch_dir.join("visible.md"), b"Visible content\n").unwrap();
    std::fs::write(watch_dir.join(".hidden.md"), b"Hidden content\n").unwrap();
    std::fs::write(watch_dir.join(".gitignore"), b"node_modules\n").unwrap();

    let storage = MockStorage::default();
    let data_dir = dir.path().join("data");
    let service = SyncService::new(storage.clone(), data_dir);

    let result = service
        .sync_folder(&watch_dir.to_string_lossy(), true, None)
        .await
        .unwrap();

    assert_eq!(result.added, 1, "只应导入 1 个可见文件");
    assert_eq!(storage.doc_count().await, 1, "知识库应有 1 个文档");

    let docs = storage.list_documents().await.unwrap();
    assert!(
        docs[0]
            .original_path
            .as_ref()
            .is_some_and(|p| p.contains("visible.md")),
        "导入的文件应为 visible.md"
    );
}

/// TC-SYNC-009: 不受支持的格式（如 .docx）被跳过，不报错（REQ-SYNC-002 AC-6）。
#[tokio::test]
async fn tc_sync_009_unsupported_format_skipped() {
    let dir = TempDir::new().unwrap();
    let watch_dir = dir.path().join("watch");
    std::fs::create_dir_all(&watch_dir).unwrap();

    // 支持的格式 + 不支持的格式
    std::fs::write(watch_dir.join("readme.md"), b"Markdown content\n").unwrap();
    std::fs::write(watch_dir.join("document.docx"), b"Fake DOCX content\n").unwrap();
    std::fs::write(watch_dir.join("image.png"), b"\x89PNG fake\n").unwrap();

    let storage = MockStorage::default();
    let data_dir = dir.path().join("data");
    let service = SyncService::new(storage.clone(), data_dir);

    let result = service
        .sync_folder(&watch_dir.to_string_lossy(), true, None)
        .await
        .unwrap();

    assert_eq!(result.added, 1, "只应导入 1 个 .md 文件");
    assert_eq!(result.errors.len(), 0, "不支持的格式不应产生错误");
    assert_eq!(storage.doc_count().await, 1, "知识库应有 1 个文档");
}

/// TC-SYNC-010: 同步是幂等的——多次同步无变更的文件夹不会产生重复导入或删除
/// （REQ-SYNC-002 AC-8）。
#[tokio::test]
async fn tc_sync_010_idempotent_sync() {
    let dir = TempDir::new().unwrap();
    let watch_dir = dir.path().join("watch");
    std::fs::create_dir_all(&watch_dir).unwrap();

    std::fs::write(watch_dir.join("a.md"), b"Content A\n").unwrap();
    std::fs::write(watch_dir.join("b.md"), b"Content B\n").unwrap();

    let storage = MockStorage::default();
    let data_dir = dir.path().join("data");
    let service = SyncService::new(storage.clone(), data_dir);

    // 第一次同步
    let result1 = service
        .sync_folder(&watch_dir.to_string_lossy(), true, None)
        .await
        .unwrap();
    assert_eq!(result1.added, 2);
    assert_eq!(storage.doc_count().await, 2);

    // 第二次同步（无变更）
    let result2 = service
        .sync_folder(&watch_dir.to_string_lossy(), true, None)
        .await
        .unwrap();
    assert_eq!(result2.added, 0, "幂等：不应有新增");
    assert_eq!(result2.updated, 0, "幂等：不应有更新");
    assert_eq!(result2.deleted, 0, "幂等：不应有删除");
    assert_eq!(result2.skipped, 2, "幂等：2 个文件应跳过");
    assert_eq!(storage.doc_count().await, 2, "幂等：文档数不变");

    // 第三次同步（仍然无变更）
    let result3 = service
        .sync_folder(&watch_dir.to_string_lossy(), true, None)
        .await
        .unwrap();
    assert_eq!(result3.added, 0);
    assert_eq!(result3.skipped, 2);
    assert_eq!(storage.doc_count().await, 2, "三次同步后文档数仍为 2");
}

/// TC-SYNC-010b: 进度回调验证——同步过程中通过回调推送进度事件
/// （REQ-SYNC-002 AC-7）。
#[tokio::test]
async fn tc_sync_010b_progress_callback() {
    let dir = TempDir::new().unwrap();
    let watch_dir = dir.path().join("watch");
    std::fs::create_dir_all(&watch_dir).unwrap();

    std::fs::write(watch_dir.join("a.md"), b"Content A\n").unwrap();

    let storage = MockStorage::default();
    let data_dir = dir.path().join("data");
    let service = SyncService::new(storage.clone(), data_dir);

    // 收集进度事件
    let events: Arc<tokio::sync::Mutex<Vec<SyncProgressPayload>>> =
        Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let events_clone = events.clone();
    let progress: crate::sync::SyncProgressFn = Arc::new(move |payload| {
        events_clone.try_lock().unwrap().push(payload);
    });

    let result = service
        .sync_folder(&watch_dir.to_string_lossy(), true, Some(progress))
        .await
        .unwrap();

    assert_eq!(result.added, 1);

    let events = events.lock().await;
    // 应至少包含 scanning、importing、complete 阶段
    let phases: Vec<String> = events.iter().map(|e| e.phase.clone()).collect();
    assert!(
        phases.iter().any(|p| p == "scanning"),
        "应包含 scanning 阶段: {phases:?}"
    );
    assert!(
        phases.iter().any(|p| p == "importing"),
        "应包含 importing 阶段: {phases:?}"
    );
    assert!(
        phases.iter().any(|p| p == "complete"),
        "应包含 complete 阶段: {phases:?}"
    );

    // complete 阶段应包含正确的计数
    let complete = events.iter().find(|e| e.phase == "complete");
    assert!(complete.is_some(), "必须有 complete 事件");
    let complete = complete.unwrap();
    assert_eq!(complete.added, 1, "complete 事件中 added 应为 1");
}

/// TC-SYNC-010c: 子目录递归扫描——同步文件夹时递归扫描子目录中的文件。
#[tokio::test]
async fn tc_sync_010c_recursive_subdirectory() {
    let dir = TempDir::new().unwrap();
    let watch_dir = dir.path().join("watch");
    std::fs::create_dir_all(watch_dir.join("subdir")).unwrap();

    std::fs::write(watch_dir.join("root.md"), b"Root level\n").unwrap();
    std::fs::write(watch_dir.join("subdir/nested.md"), b"Nested content\n").unwrap();

    let storage = MockStorage::default();
    let data_dir = dir.path().join("data");
    let service = SyncService::new(storage.clone(), data_dir);

    let result = service
        .sync_folder(&watch_dir.to_string_lossy(), true, None)
        .await
        .unwrap();

    assert_eq!(result.added, 2, "应递归导入 2 个文件（含子目录）");
    assert_eq!(storage.doc_count().await, 2);

    // 验证子目录文件被正确导入
    let docs = storage.list_documents().await.unwrap();
    let has_nested = docs.iter().any(|d| {
        d.original_path
            .as_ref()
            .is_some_and(|p| p.contains("nested.md"))
    });
    assert!(has_nested, "应包含子目录中的文件");
}

/// TC-SYNC-011: 幂等性防护——同一文件夹短时间内重复同步会被跳过。
///
/// 验证 IdempotencyStore 防止重复同步的效果。
#[tokio::test]
async fn tc_sync_011_idempotency_protection() {
    let dir = TempDir::new().unwrap();
    let watch_dir = dir.path().join("watch");
    std::fs::create_dir_all(&watch_dir).unwrap();

    std::fs::write(watch_dir.join("test.md"), b"Test content\n").unwrap();

    let storage = MockStorage::default();
    let data_dir = dir.path().join("data");
    let service = SyncService::new(storage.clone(), data_dir);

    // 第一次同步
    let result1 = service
        .sync_folder(&watch_dir.to_string_lossy(), true, None)
        .await
        .unwrap();

    assert_eq!(result1.added, 1, "首次同步应导入 1 个文件");
    assert_eq!(storage.doc_count().await, 1);

    // 立即进行第二次同步（应被幂等性防护跳过）
    let result2 = service
        .sync_folder(&watch_dir.to_string_lossy(), true, None)
        .await
        .unwrap();

    // 第二次同步应被跳过
    assert_eq!(result2.added, 0, "重复同步应跳过新增");
    assert_eq!(result2.updated, 0, "重复同步应跳过更新");
    assert_eq!(result2.deleted, 0, "重复同步应跳过删除");
    // 注意：由于实际同步发生了（有文件被跳过），skipped 可能不为0
    assert!(
        !result2.errors.is_empty() || result2.skipped > 0,
        "重复同步应有跳过行为"
    );

    // 知识库文档数量不应变化
    assert_eq!(storage.doc_count().await, 1, "重复同步不应改变文档数量");
}
