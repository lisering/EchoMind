#![allow(clippy::unwrap_used)]
//! QA 红灯测试（体系二阶段 2：先于实现编写，初始必然失败）。
//! 覆盖：TC-ING-001 格式白名单 / TC-ING-002 MD5 去重 / TC-ING-003 免费版配额 / TC-ING-004 PDF 付费门。

use std::collections::HashMap;
use std::sync::Arc;

use crate::Storage;
use crate::import::{ImportOutcome, ImportService};
use anyhow::Result;
use echomind_models::{Chunk, DocStatus, Document, RetrievalResult};
use tempfile::TempDir;

/// QA Mock Storage：内存实现，用于隔离验证导入管线逻辑。
#[derive(Clone, Default)]
struct MockStorage {
    docs: Arc<tokio::sync::Mutex<HashMap<String, Document>>>,
}

impl MockStorage {
    async fn len(&self) -> usize {
        self.docs.lock().await.len()
    }
}

impl Storage for MockStorage {
    async fn add_document(&self, doc: &Document) -> Result<()> {
        self.docs.lock().await.insert(doc.id.clone(), doc.clone());
        Ok(())
    }

    async fn update_doc_status(&self, _doc_id: &str, _status: DocStatus) -> Result<()> {
        Ok(())
    }

    async fn add_chunk(&self, _chunk: &Chunk) -> Result<()> {
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
}

fn write_file(dir: &TempDir, name: &str, content: &[u8]) -> String {
    let path = dir.path().join(name);
    std::fs::write(&path, content).unwrap();
    path.to_string_lossy().into_owned()
}

/// TC-ING-001 格式校验：传入 .jpg 路径，断言返回 Err（REQ-ING-001-AC-3 / REQ-ING-002-AC-3）。
#[tokio::test]
async fn tc_ing_001_reject_unsupported_format() {
    let dir = TempDir::new().unwrap();
    let jpg = write_file(&dir, "photo.jpg", b"\xff\xd8\xff\xe0");
    let service = ImportService::new(MockStorage::default(), dir.path().join("data"));

    let result = service.import_one(&jpg, true).await;

    assert!(result.is_err(), ".jpg 必须被格式白名单拦截");
}

/// TC-ING-002 哈希去重：同内容文件第二次导入必须被跳过（REQ-ING-004-AC-1）。
#[tokio::test]
async fn tc_ing_002_duplicate_content_skipped() {
    let dir = TempDir::new().unwrap();
    let first_path = write_file(&dir, "a.md", b"same content");
    let second_path = write_file(&dir, "b.md", b"same content");
    let storage = MockStorage::default();
    let service = ImportService::new(storage.clone(), dir.path().join("data"));

    let first = service.import_one(&first_path, true).await.unwrap();
    assert!(
        matches!(first, ImportOutcome::Imported(_)),
        "首次导入必须成功"
    );

    let second = service.import_one(&second_path, true).await.unwrap();
    assert!(
        matches!(second, ImportOutcome::SkippedDuplicate(_)),
        "同内容文件第二次导入必须跳过"
    );
    assert_eq!(storage.len().await, 1, "去重后库内应只有 1 个文档");
}

/// TC-ING-003 配额拦截：免费版已有 50 个文件时，第 51 个必须返回 Err（REQ-LIC-001-AC-1）。
#[tokio::test]
async fn tc_ing_003_free_tier_quota_blocked() {
    let dir = TempDir::new().unwrap();
    let storage = MockStorage::default();
    for i in 0..50 {
        let doc = Document::new(format!("doc-{i}.md"), format!("hash-{i}"));
        storage.add_document(&doc).await.unwrap();
    }
    let service = ImportService::new(storage.clone(), dir.path().join("data"));
    let extra = write_file(&dir, "extra.md", b"extra");

    let result = service.import_one(&extra, false).await;

    assert!(result.is_err(), "免费版第 51 个文件必须被拦截");
    assert_eq!(storage.len().await, 50, "被拦截的文件不得入库");
}

/// TC-ING-004 付费门：免费版导入 PDF 必须返回 Err；Pro 版允许入库（REQ-ING-002-AC-1/AC-2）。
#[tokio::test]
async fn tc_ing_004_pdf_blocked_for_free_tier() {
    let dir = TempDir::new().unwrap();
    let pdf = write_file(&dir, "paper.pdf", b"%PDF-1.7 fake-bytes");
    let service = ImportService::new(MockStorage::default(), dir.path().join("data"));

    let free_result = service.import_one(&pdf, false).await;
    assert!(free_result.is_err(), "免费版导入 PDF 必须被拦截");

    let pro_result = service.import_one(&pdf, true).await.unwrap();
    assert!(
        matches!(pro_result, ImportOutcome::Imported(_)),
        "Pro 版导入 PDF 必须放行入库"
    );
}

/// TC-ING-001 补充：路径遍历攻击必须被拒绝（安全官审查项，防 `../`）。
#[tokio::test]
async fn tc_ing_001b_path_traversal_rejected() {
    let dir = TempDir::new().unwrap();
    let storage = MockStorage::default();
    let service = ImportService::new(storage, dir.path().join("data"));
    let traversal = format!("{}/../etc/passwd", dir.path().display());

    let result = service.import_one(&traversal, true).await;

    assert!(result.is_err(), "含 .. 的路径遍历必须被拒绝");
}
