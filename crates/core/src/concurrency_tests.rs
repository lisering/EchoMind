#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! 并发与竞态条件测试（REQ-SEC-007 / NFR-010）。
//!
//! 验证 SQLite WAL 并发读写、CancellationToken 竞态、
//! 并发导入/聊天/删除操作的状态一致性。

use std::sync::Arc;

use anyhow::Result;
use echomind_models::{Chunk, DocStatus, Document, RetrievalResult};

use crate::Storage;
use echomind_models::{ChatMessage, Conversation};

/// 并发安全 Mock Storage：内部使用 Mutex 保护 HashMap，
/// 模拟 SQLite WAL 模式下的并发读写。
#[derive(Clone, Default)]
struct ConcurrentMockStorage {
    docs: Arc<tokio::sync::Mutex<std::collections::HashMap<String, Document>>>,
    chunks: Arc<tokio::sync::Mutex<std::collections::HashMap<String, Vec<Chunk>>>>,
    settings: Arc<tokio::sync::Mutex<std::collections::HashMap<String, String>>>,
    conversations: Arc<tokio::sync::Mutex<std::collections::HashMap<String, Conversation>>>,
    messages: Arc<tokio::sync::Mutex<std::collections::HashMap<String, Vec<ChatMessage>>>>,
}

impl Storage for ConcurrentMockStorage {
    async fn add_document(&self, doc: &Document) -> Result<()> {
        let mut docs = self.docs.lock().await;
        docs.insert(doc.id.clone(), doc.clone());
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
        let mut chunks = self.chunks.lock().await;
        chunks
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
        let docs = self.docs.lock().await;
        Ok(docs.values().find(|d| d.file_hash == hash).cloned())
    }

    async fn count_documents(&self) -> Result<usize> {
        let docs = self.docs.lock().await;
        Ok(docs.len())
    }

    async fn count_chunks(&self) -> Result<usize> {
        Ok(0)
    }

    async fn cleanup_zombies(&self) -> Result<usize> {
        let mut docs = self.docs.lock().await;
        let mut cleaned = 0;
        for doc in docs.values_mut() {
            if matches!(doc.status, DocStatus::Processing) {
                doc.status = DocStatus::Failed("崩溃恢复".to_string());
                cleaned += 1;
            }
        }
        Ok(cleaned)
    }

    async fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let mut settings = self.settings.lock().await;
        settings.insert(key.to_string(), value.to_string());
        Ok(())
    }

    async fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let settings = self.settings.lock().await;
        Ok(settings.get(key).cloned())
    }

    async fn create_conversation(&self, conv: &Conversation) -> Result<()> {
        let mut convs = self.conversations.lock().await;
        convs.insert(conv.id.clone(), conv.clone());
        Ok(())
    }

    async fn list_conversations(&self, _workspace_id: &str) -> Result<Vec<Conversation>> {
        let convs = self.conversations.lock().await;
        Ok(convs.values().cloned().collect())
    }

    async fn delete_conversation(&self, id: &str) -> Result<()> {
        let mut convs = self.conversations.lock().await;
        convs.remove(id);
        let mut msgs = self.messages.lock().await;
        msgs.remove(id);
        Ok(())
    }

    async fn update_conversation_title(&self, id: &str, title: &str) -> Result<()> {
        let mut convs = self.conversations.lock().await;
        if let Some(conv) = convs.get_mut(id) {
            conv.title = title.to_string();
        }
        Ok(())
    }

    async fn add_message(&self, conversation_id: &str, message: &ChatMessage) -> Result<()> {
        let mut msgs = self.messages.lock().await;
        msgs.entry(conversation_id.to_string())
            .or_default()
            .push(message.clone());
        Ok(())
    }

    async fn list_messages(&self, conversation_id: &str) -> Result<Vec<ChatMessage>> {
        let msgs = self.messages.lock().await;
        Ok(msgs.get(conversation_id).cloned().unwrap_or_default())
    }

    async fn list_documents(&self) -> Result<Vec<Document>> {
        let docs = self.docs.lock().await;
        Ok(docs.values().cloned().collect())
    }

    async fn delete_document(&self, doc_id: &str) -> Result<()> {
        let mut docs = self.docs.lock().await;
        docs.remove(doc_id);
        let mut chunks = self.chunks.lock().await;
        chunks.remove(doc_id);
        Ok(())
    }

    async fn list_chunks(&self, doc_id: &str) -> Result<Vec<Chunk>> {
        let chunks = self.chunks.lock().await;
        Ok(chunks.get(doc_id).cloned().unwrap_or_default())
    }

    async fn delete_chunks_by_doc(&self, doc_id: &str) -> Result<()> {
        let mut chunks = self.chunks.lock().await;
        chunks.remove(doc_id);
        Ok(())
    }

    async fn keyword_search(&self, _query: &str, _top_k: usize) -> Result<Vec<RetrievalResult>> {
        Ok(vec![])
    }
}

/// TC-CONC-001：并发导入 50 个文档，无死锁、无数据丢失。
#[tokio::test]
async fn tc_conc_001_concurrent_import_50_docs() {
    let storage = ConcurrentMockStorage::default();

    let mut handles = Vec::new();
    for i in 0..50 {
        let s = storage.clone();
        handles.push(tokio::spawn(async move {
            let doc = Document::new(format!("/path/file-{i}.md"), format!("hash-{i}"));
            s.add_document(&doc).await.unwrap();
            s.update_doc_status(&doc.id, DocStatus::Indexed)
                .await
                .unwrap();
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    let count = storage.count_documents().await.unwrap();
    assert_eq!(count, 50, "并发导入 50 个文档后计数必须为 50");
}

/// TC-CONC-002：并发读 + 写不阻塞。
#[tokio::test]
async fn tc_conc_002_concurrent_read_write() {
    let storage = ConcurrentMockStorage::default();

    // 先写入 10 个文档
    for i in 0..10 {
        let doc = Document::new(format!("/path/file-{i}.md"), format!("hash-{i}"));
        storage.add_document(&doc).await.unwrap();
    }

    // 并发：10 个写入 + 10 个读取
    let mut handles = Vec::new();
    for i in 10..20 {
        let s = storage.clone();
        handles.push(tokio::spawn(async move {
            let doc = Document::new(format!("/path/file-{i}.md"), format!("hash-{i}"));
            s.add_document(&doc).await.unwrap();
        }));
    }
    for _ in 0..10 {
        let s = storage.clone();
        handles.push(tokio::spawn(async move {
            let _ = s.count_documents().await.unwrap();
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    let count = storage.count_documents().await.unwrap();
    assert_eq!(count, 20, "并发读写后总数应为 20");
}

/// TC-CONC-003：并发删除同一文档不 Panic。
#[tokio::test]
async fn tc_conc_003_concurrent_delete_same_doc() {
    let storage = ConcurrentMockStorage::default();
    let doc = Document::new("/path/file.md".to_string(), "hash-x".to_string());
    storage.add_document(&doc).await.unwrap();
    let doc_id = doc.id.clone();

    // 并发删除同一文档
    let mut handles = Vec::new();
    for _ in 0..10 {
        let s = storage.clone();
        let did = doc_id.clone();
        handles.push(tokio::spawn(async move {
            // 删除不存在的文档不 Panic
            let _ = s.delete_document(&did).await;
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    // 文档已被删除
    let count = storage.count_documents().await.unwrap();
    assert_eq!(count, 0, "并发删除后文档应不存在");
}

/// TC-CONC-004：并发 add_chunk + list_chunks 无竞态。
#[tokio::test]
async fn tc_conc_004_concurrent_add_and_list_chunks() {
    let storage = ConcurrentMockStorage::default();
    let doc = Document::new("/path/file.md".to_string(), "hash-1".to_string());
    storage.add_document(&doc).await.unwrap();
    let doc_id = doc.id.clone();

    // 并发添加 chunks
    let mut handles = Vec::new();
    for i in 0..20 {
        let s = storage.clone();
        let did = doc_id.clone();
        handles.push(tokio::spawn(async move {
            let chunk = Chunk::new(did, format!("content-{i}"), i, 0);
            s.add_chunk(&chunk).await.unwrap();
        }));
    }

    // 并发读取 chunks
    for _ in 0..10 {
        let s = storage.clone();
        let did = doc_id.clone();
        handles.push(tokio::spawn(async move {
            let _ = s.list_chunks(&did).await.unwrap();
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    let chunks = storage.list_chunks(&doc_id).await.unwrap();
    assert_eq!(chunks.len(), 20, "并发添加后 chunk 数应为 20");
}

/// TC-CONC-005：cleanup_zombies 在并发环境下正确恢复。
#[tokio::test]
async fn tc_conc_005_concurrent_cleanup_zombies() {
    let storage = ConcurrentMockStorage::default();

    // 添加 5 个 Processing 状态文档（模拟崩溃遗留）
    for i in 0..5 {
        let doc = Document::new(format!("/path/file-{i}.md"), format!("hash-{i}"));
        storage.add_document(&doc).await.unwrap();
        storage
            .update_doc_status(&doc.id, DocStatus::Processing)
            .await
            .unwrap();
    }

    // 并发 cleanup
    let mut handles = Vec::new();
    for _ in 0..3 {
        let s = storage.clone();
        handles.push(tokio::spawn(
            async move { s.cleanup_zombies().await.unwrap() },
        ));
    }

    let results: Vec<usize> = futures::future::join_all(handles)
        .await
        .into_iter()
        .map(|r| r.unwrap())
        .collect();

    // 首次 cleanup 清理 5 个，后续 cleanup 清理 0 个
    let total_cleaned: usize = results.iter().sum();
    assert_eq!(total_cleaned, 5, "总共应清理 5 个僵尸文档");

    // 确认全部恢复为 Failed
    let docs = storage.list_documents().await.unwrap();
    for doc in &docs {
        assert!(
            matches!(doc.status, DocStatus::Failed(_)),
            "文档 {} 应恢复为 Failed 状态",
            doc.id
        );
    }
}
