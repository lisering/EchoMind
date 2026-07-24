//! 应用状态与开发期内存存储适配器（Phase 2 临时方案）。
//! TODO(phase-3): 以 crates/infra 的 sqlite-vec 适配器替换（REQ-VEC-003），
//! 届时本文件仅保留 AppState 组装，存储实现迁出。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use echomind_core::Storage;
use echomind_models::{Chunk, DocStatus, Document, RetrievalResult};

/// Tauri 全局状态。
pub struct AppState {
    /// 存储端口实现（开发期为内存适配器）
    pub storage: InMemoryStorage,
    /// 应用数据目录（文档副本根目录）
    pub data_dir: PathBuf,
}

impl AppState {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            storage: InMemoryStorage::default(),
            data_dir,
        }
    }
}

/// 开发期内存存储：进程退出即丢失，仅用于打通 Phase 2 导入流水线。
/// 内部以 Arc 共享，克隆廉价；全部锁为 tokio 异步锁且临界区不含 await（体系三）。
#[derive(Clone, Default)]
pub struct InMemoryStorage {
    docs: Arc<tokio::sync::Mutex<HashMap<String, Document>>>,
    chunks: Arc<tokio::sync::Mutex<HashMap<String, Chunk>>>,
    embeddings: Arc<tokio::sync::Mutex<HashMap<String, Vec<f32>>>>,
}

impl Storage for InMemoryStorage {
    async fn add_document(&self, doc: &Document) -> anyhow::Result<()> {
        self.docs.lock().await.insert(doc.id.clone(), doc.clone());
        Ok(())
    }

    async fn update_doc_status(&self, doc_id: &str, status: DocStatus) -> anyhow::Result<()> {
        let mut docs = self.docs.lock().await;
        if let Some(doc) = docs.get_mut(doc_id) {
            doc.status = status;
        }
        Ok(())
    }

    async fn add_chunk(&self, chunk: &Chunk) -> anyhow::Result<()> {
        self.chunks
            .lock()
            .await
            .insert(chunk.id.clone(), chunk.clone());
        Ok(())
    }

    async fn add_embedding(&self, chunk_id: &str, embedding: &[f32]) -> anyhow::Result<()> {
        self.embeddings
            .lock()
            .await
            .insert(chunk_id.to_string(), embedding.to_vec());
        Ok(())
    }

    async fn vector_search(
        &self,
        _query_embedding: &[f32],
        _top_k: usize,
    ) -> anyhow::Result<Vec<RetrievalResult>> {
        Err(anyhow::anyhow!(
            "向量检索适配器将于 Phase 3（sqlite-vec）交付"
        ))
    }

    async fn find_document_by_hash(&self, hash: &str) -> anyhow::Result<Option<Document>> {
        Ok(self
            .docs
            .lock()
            .await
            .values()
            .find(|d| d.file_hash == hash)
            .cloned())
    }

    async fn count_documents(&self) -> anyhow::Result<usize> {
        Ok(self.docs.lock().await.len())
    }
}
