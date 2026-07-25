#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! QA 红灯测试（体系二阶段 2：先于实现编写，初始必然失败）。
//! 覆盖：TC-ING-001 格式白名单 / TC-ING-002 MD5 去重 / TC-ING-003 免费版配额 / TC-ING-004 PDF 付费门。

use std::collections::HashMap;
use std::sync::Arc;

use crate::Storage;
use crate::import::{ImportOutcome, ImportService};
use anyhow::Result;
use echomind_models::{ChatMessage, Chunk, DocStatus, Document, RetrievalResult};
use tempfile::TempDir;

/// QA Mock Storage：内存实现，用于隔离验证导入管线逻辑。
/// 扩展支持 chunks 捕获（TC-VEC-002 端到端分块入库契约验证）。
#[derive(Clone, Default)]
struct MockStorage {
    docs: Arc<tokio::sync::Mutex<HashMap<String, Document>>>,
    /// doc_id -> chunks（按 add_chunk 调用顺序累积，用于验证分块结果）
    chunks: Arc<tokio::sync::Mutex<HashMap<String, Vec<Chunk>>>>,
}

impl MockStorage {
    async fn len(&self) -> usize {
        self.docs.lock().await.len()
    }

    /// 获取指定文档的全部 chunks（按写入顺序，TC-VEC-002 断言用）。
    async fn chunks_for(&self, doc_id: &str) -> Vec<Chunk> {
        self.chunks
            .lock()
            .await
            .get(doc_id)
            .cloned()
            .unwrap_or_default()
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

    async fn cleanup_zombies(&self) -> Result<usize> {
        let mut docs = self.docs.lock().await;
        let mut cleaned = 0;
        for doc in docs.values_mut() {
            if matches!(doc.status, DocStatus::Processing) {
                doc.status = DocStatus::Failed("崩溃恢复：上次会话中断".to_string());
                cleaned += 1;
            }
        }
        Ok(cleaned)
    }

    async fn set_setting(&self, _key: &str, _value: &str) -> Result<()> {
        Ok(())
    }

    async fn get_setting(&self, _key: &str) -> Result<Option<String>> {
        Ok(None)
    }

    async fn create_conversation(
        &self,
        _conversation: &echomind_models::Conversation,
    ) -> Result<()> {
        Ok(())
    }

    async fn list_conversations(
        &self,
        _workspace_id: &str,
    ) -> Result<Vec<echomind_models::Conversation>> {
        Ok(vec![])
    }

    async fn delete_conversation(&self, _id: &str) -> Result<()> {
        Ok(())
    }

    async fn update_conversation_title(&self, _id: &str, _title: &str) -> Result<()> {
        Ok(())
    }

    async fn add_message(&self, _conversation_id: &str, _message: &ChatMessage) -> Result<()> {
        Ok(())
    }

    async fn list_messages(&self, _conversation_id: &str) -> Result<Vec<ChatMessage>> {
        Ok(vec![])
    }

    async fn list_documents(&self) -> Result<Vec<Document>> {
        Ok(vec![])
    }

    async fn delete_document(&self, _doc_id: &str) -> Result<()> {
        Ok(())
    }

    async fn list_chunks(&self, doc_id: &str) -> Result<Vec<Chunk>> {
        Ok(self.chunks_for(doc_id).await)
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

/// TC-VEC-002 SemanticSplitter 端到端分块入库契约（REQ-VEC-001 + 段落感知 + 代码块完整性）。
///
/// 验证 `ImportService.index_with_text` 使用 `SemanticSplitter` 后的完整分块链路：
/// - chunks 非空且 sequence 从 0 连续递增（REQ-VEC-001-AC-3）
/// - 段落感知：标题与其正文保持在同一 chunk（不跨段落硬切分）
/// - 代码块完整性：代码块作为整体段落不被内部分割
/// - token_count 合理（不超过 max_tokens = target * 3 = 768）
///
/// 注：MarkdownLoader 剥离 `#` 和 ` ``` ` 标记，保留正文与代码内容，
/// 段落边界以 `\n\n` 保留（loader.rs 注释明确说明）。
#[tokio::test]
async fn tc_vec_002_semantic_splitter_end_to_end_chunking() {
    let dir = TempDir::new().unwrap();
    // 真实 Markdown 语料：标题 + 正文 + 代码块 + 多段落（~400 tokens，触发多 chunk 切分）
    let md_content = "\
# Lisp 语言简介\n\n\
Lisp 是第二古老的编程语言，诞生于 1958 年。代码和数据都是列表结构，这是 Lisp 的核心特性。\
John McCarthy 在 MIT 发明了这门语言，灵感来自 Lambda 演算。\n\n\
## 基本语法规则\n\n\
Lisp 的核心是 S-表达式。括号表示函数调用。嵌套的括号表示组合操作。\
这是 Lisp 最独特的语法特征，代码即数据的数据即代码。\n\n\
### 代码示例\n\n\
下面是一个简单的程序示例，展示基本语法：\n\n\
```rust\n\
fn main() {\n    println!(\"Hello, Lisp!\");\n    let list = vec![1, 2, 3];\n    println!(\"{:?}\", list);\n}\n\
```\n\n\
## 高阶函数与函数式编程\n\n\
map 和 reduce 是 Lisp 的精髓。函数式编程的基石在于函数是一等公民。\
Lisp 天然支持闭包和连续传递风格。reduce 可以将列表归约为单值。map 对每个元素应用函数变换。\n\n\
## 宏系统与同象性\n\n\
Lisp 的宏比 C 宏强大得多。代码即数据，数据即代码。这是同象性的优势。\
宏允许在编译期生成代码，实现领域特定语言。\n";

    let md_path = write_file(&dir, "lisp-intro.md", md_content.as_bytes());
    let storage = MockStorage::default();
    let service = ImportService::new(storage.clone(), dir.path().join("data"));

    // 导入文件（is_pro=true 跳过配额限制）
    let outcome = service.import_one(&md_path, true).await.unwrap();
    let doc = match outcome {
        ImportOutcome::Imported(d) => d,
        other => panic!("导入应成功，实际: {other:?}"),
    };

    // 执行索引（load → SemanticSplitter.split → add_chunk 入库）
    service.index_document(&doc).await.unwrap();

    // 获取分块结果
    let chunks = storage.chunks_for(&doc.id).await;
    assert!(
        !chunks.is_empty(),
        "SemanticSplitter 必须产生至少 1 个 chunk"
    );

    // 断言 1：sequence 从 0 连续递增（REQ-VEC-001-AC-3）
    for (i, chunk) in chunks.iter().enumerate() {
        assert_eq!(
            chunk.sequence, i,
            "chunk sequence 必须从 0 连续递增，第 {i} 个 chunk 的 sequence = {}",
            chunk.sequence
        );
    }

    // 断言 2：段落感知——标题文本"Lisp 语言简介"与其正文"第二古老的编程语言"在同一 chunk
    // （MarkdownLoader 剥离 # 标记，标题作为独立段落，与正文段落合并到同一 chunk）
    let title_chunk = chunks.iter().find(|c| c.content.contains("Lisp 语言简介"));
    assert!(title_chunk.is_some(), "必须存在包含标题文本的 chunk");
    let title_chunk = title_chunk.unwrap();
    assert!(
        title_chunk.content.contains("第二古老的编程语言"),
        "段落感知：标题与其正文必须在同一 chunk（实际 chunk 不包含正文）"
    );

    // 断言 3：代码块完整性——`fn main` 与 `println!` 在同一 chunk
    // （MarkdownLoader 将代码块作为 Event::Code 整体输出，SemanticSplitter 保持其完整）
    let code_chunk = chunks.iter().find(|c| c.content.contains("fn main"));
    assert!(code_chunk.is_some(), "必须存在包含代码块的 chunk");
    let code_chunk = code_chunk.unwrap();
    assert!(
        code_chunk.content.contains("println!(\"Hello, Lisp!\")"),
        "代码块完整性：fn main 与 println! 必须在同一 chunk（不被内部分割）"
    );
    assert!(
        code_chunk.content.contains("vec![1, 2, 3]"),
        "代码块完整性：代码块全部内容（含 vec!）必须在同一 chunk"
    );

    // 断言 4：每个 chunk 的 token_count 合理（不超过 max_tokens = 256 * 3 = 768）
    for chunk in &chunks {
        assert!(
            chunk.token_count <= 768,
            "chunk token_count {} 超过 max_tokens (768)",
            chunk.token_count
        );
        assert!(!chunk.content.trim().is_empty(), "chunk 内容不得为空");
    }
}
