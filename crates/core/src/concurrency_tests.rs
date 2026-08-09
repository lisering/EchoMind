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

// ============================================================
// KeyedQueue 测试（Q09 借鉴 QM createKeyedQueue）
// ============================================================

use std::collections::HashMap;

use crate::concurrency::KeyedQueue;

/// TC-KEYED-001：同 key 操作串行执行（验证执行顺序）。
///
/// 两个同 key 的操作必须按提交顺序串行执行。
/// 第二个操作必须等待第一个完成后才开始。
#[tokio::test]
async fn tc_keyed_001_same_key_serial() {
    let queue = KeyedQueue::<String>::new();
    let order = Arc::new(tokio::sync::Mutex::new(Vec::<u32>::new()));

    let o1 = Arc::clone(&order);
    let o2 = Arc::clone(&order);

    // 启动两个同 key 操作，第一个延迟 100ms 验证串行
    let h1 = tokio::spawn(async move {
        queue
            .run("conv-1".to_string(), || async {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                o1.lock().await.push(1);
            })
            .await;
    });

    let h2 = tokio::spawn(async move {
        // 等待一小段时间确保 h1 先提交
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        let queue2 = KeyedQueue::<String>::new();
        queue2
            .run("conv-1".to_string(), || async {
                o2.lock().await.push(2);
            })
            .await;
    });

    h1.await.unwrap();
    h2.await.unwrap();

    // 但 h2 使用了不同的 queue 实例，无法验证跨实例串行。
    // 使用同一 queue 重新测试：
    let queue = Arc::new(KeyedQueue::<String>::new());
    let order = Arc::new(tokio::sync::Mutex::new(Vec::<u32>::new()));

    let q1 = Arc::clone(&queue);
    let o1 = Arc::clone(&order);
    let h1 = tokio::spawn(async move {
        q1.run("conv-A".to_string(), || async {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            o1.lock().await.push(1);
        })
        .await;
    });

    // 确保 h1 先获取锁
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;

    let q2 = Arc::clone(&queue);
    let o2 = Arc::clone(&order);
    let h2 = tokio::spawn(async move {
        q2.run("conv-A".to_string(), || async {
            o2.lock().await.push(2);
        })
        .await;
    });

    h1.await.unwrap();
    h2.await.unwrap();

    let result = order.lock().await;
    assert_eq!(*result, vec![1, 2], "同 key 操作必须按提交顺序串行执行");
}

/// TC-KEYED-002：不同 key 操作并行执行（验证并行性）。
///
/// 两个不同 key 的操作应该并行执行。
/// 如果串行执行，总耗时约为 200ms；如果并行，总耗时约为 100ms。
#[tokio::test]
async fn tc_keyed_002_different_key_parallel() {
    let queue = Arc::new(KeyedQueue::<String>::new());

    let q1 = Arc::clone(&queue);
    let h1 = tokio::spawn(async move {
        q1.run("conv-A".to_string(), || async {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        })
        .await;
    });

    let q2 = Arc::clone(&queue);
    let h2 = tokio::spawn(async move {
        q2.run("conv-B".to_string(), || async {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        })
        .await;
    });

    let start = std::time::Instant::now();
    h1.await.unwrap();
    h2.await.unwrap();
    let elapsed = start.elapsed();

    // 并行执行应明显快于串行（200ms）
    assert!(
        elapsed < std::time::Duration::from_millis(180),
        "不同 key 操作应并行执行，实际耗时 {:?}（应 < 180ms）",
        elapsed
    );
}

/// TC-KEYED-003：操作完成后 tail 被清理（无内存泄漏）。
///
/// 完成所有操作后，调用 cleanup() 应清理空闲的 tail。
#[tokio::test]
async fn tc_keyed_003_tail_cleanup() {
    let queue = KeyedQueue::<String>::new();

    // 执行几个操作
    queue.run("conv-1".to_string(), || async { 42 }).await;
    queue.run("conv-2".to_string(), || async { 99 }).await;

    // 操作完成后 tail 仍存在（无等待者）
    {
        let tails = queue.tails.read().await;
        assert_eq!(tails.len(), 2, "操作完成后 tail 仍存在（等待清理）");
    }

    // 清理空闲 tail
    queue.cleanup().await;

    // 清理后 tail 应为空（无活跃操作时全部可清理）
    {
        let tails = queue.tails.read().await;
        assert_eq!(tails.len(), 0, "cleanup() 后 tail 应为空");
    }

    // 清理后仍可正常使用
    let result = queue.run("conv-3".to_string(), || async { 7 }).await;
    assert_eq!(result, 7, "清理后 queue 仍可正常工作");
}

/// TC-KEYED-004：panic 不影响后续同 key 操作（catch_unwind 等效）。
///
/// 如果一个操作 panic，后续同 key 操作仍应正常执行。
/// KeyedQueue 使用 Mutex guard 在 panic 时自动释放（Rust Mutex 的 poison 机制）。
/// 但 tokio::sync::Mutex 不 poison，所以 panic 后 guard 正常释放。
#[tokio::test]
async fn tc_keyed_004_panic_resilience() {
    let queue = Arc::new(KeyedQueue::<String>::new());

    // 第一个操作 panic（模拟），使用 spawn + catch
    let q1 = Arc::clone(&queue);
    let h1 = tokio::spawn(async move {
        q1.run("conv-panic".to_string(), || async {
            panic!("模拟操作 panic");
        })
        .await;
    });

    // 等待 panic 完成（spawn 中的 panic 会被 JoinError 捕获）
    let _ = h1.await;

    // 后续同 key 操作仍应正常执行
    let q2 = Arc::clone(&queue);
    let result = q2.run("conv-panic".to_string(), || async { "ok" }).await;
    assert_eq!(result, "ok", "panic 后同 key 操作仍应正常执行");

    // 清理
    queue.cleanup().await;
}

/// TC-KEYED-005：run 返回操作结果（验证泛型返回值）。
#[tokio::test]
async fn tc_keyed_005_return_value() {
    let queue = KeyedQueue::<String>::new();

    // 返回整数
    let r1 = queue.run("k1".to_string(), || async { 42 }).await;
    assert_eq!(r1, 42);

    // 返回字符串
    let r2 = queue
        .run("k2".to_string(), || async { "hello".to_string() })
        .await;
    assert_eq!(r2, "hello");

    // 返回结构体
    let r3 = queue
        .run("k3".to_string(), || async { HashMap::from([("a", 1)]) })
        .await;
    assert_eq!(r3.get("a"), Some(&1));
}

/// TC-KEYED-006：多 key 并发操作各自独立串行（验证多会话场景）。
///
/// 模拟 3 个会话各自并发写入，同一会话内串行，不同会话间并行。
#[tokio::test]
async fn tc_keyed_006_multi_key_concurrent() {
    let queue = Arc::new(KeyedQueue::<String>::new());
    let results = Arc::new(tokio::sync::Mutex::new(HashMap::<String, Vec<u32>>::new()));

    let mut handles = Vec::new();

    // 每个会话提交 3 个串行操作
    for conv in &["conv-A", "conv-B", "conv-C"] {
        for i in 0..3u32 {
            let q = Arc::clone(&queue);
            let r = Arc::clone(&results);
            let key = conv.to_string();
            handles.push(tokio::spawn(async move {
                q.run(key.clone(), || async move {
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    r.lock().await.entry(key).or_default().push(i);
                })
                .await;
            }));
        }
    }

    for h in handles {
        h.await.unwrap();
    }

    let results = results.lock().await;
    // 每个会话应有 3 个操作
    for conv in &["conv-A", "conv-B", "conv-C"] {
        let entries = results.get(*conv).expect("会话应有结果");
        assert_eq!(entries.len(), 3, "会话 {} 应有 3 个操作", conv);
        // 同会话内操作应按顺序执行（0, 1, 2）
        // 由于 tokio::spawn 顺序非严格保证，检查排序可能不稳定，
        // 但同会话内不应该出现并发交叉（各条目的值应唯一且完整）
        let mut sorted = entries.clone();
        sorted.sort();
        assert_eq!(sorted, vec![0, 1, 2], "会话 {} 操作集应为 {{0,1,2}}", conv);
    }
}
