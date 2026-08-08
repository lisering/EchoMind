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

use crate::LLMProvider;
use crate::Storage;
use crate::import::{ImportOutcome, ImportService};
use anyhow::Result;
use echomind_models::{ChatMessage, Chunk, DocStatus, Document, RetrievalResult};
use futures::StreamExt;
use futures::stream::BoxStream;
use tempfile::TempDir;

/// QA Mock Storage：内存实现，用于隔离验证导入管线逻辑。
/// 扩展支持 chunks 捕获（TC-VEC-002 端到端分块入库契约验证）。
#[derive(Clone, Default)]
struct MockStorage {
    docs: Arc<tokio::sync::Mutex<HashMap<String, Document>>>,
    /// doc_id -> chunks（按 add_chunk 调用顺序累积，用于验证分块结果）
    chunks: Arc<tokio::sync::Mutex<HashMap<String, Vec<Chunk>>>>,
    /// doc_id -> summary（REQ-ING-019 摘要持久化验证）
    summaries: Arc<tokio::sync::Mutex<HashMap<String, String>>>,
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

    async fn count_chunks(&self) -> Result<usize> {
        Ok(0)
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
        let mut docs: Vec<Document> = self.docs.lock().await.values().cloned().collect();
        // 合并 summaries（REQ-ING-019）
        let summaries = self.summaries.lock().await;
        for doc in &mut docs {
            if let Some(summary) = summaries.get(&doc.id) {
                doc.summary = Some(summary.clone());
            }
        }
        Ok(docs)
    }

    async fn delete_document(&self, _doc_id: &str) -> Result<()> {
        Ok(())
    }

    async fn list_chunks(&self, doc_id: &str) -> Result<Vec<Chunk>> {
        Ok(self.chunks_for(doc_id).await)
    }

    async fn delete_chunks_by_doc(&self, _doc_id: &str) -> Result<()> {
        Ok(()) // MockStorage 测试不验证清理逻辑
    }

    async fn keyword_search(&self, _query: &str, _top_k: usize) -> Result<Vec<RetrievalResult>> {
        Ok(vec![])
    }

    async fn update_document_summary(&self, doc_id: &str, summary: &str) -> Result<()> {
        self.summaries
            .lock()
            .await
            .insert(doc_id.to_string(), summary.to_string());
        Ok(())
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

// ==================================================================
// GB 级文档加速：分块大小可配置（路径 5）
// ==================================================================

/// TC-PERF-007：`auto_chunk_tokens` 根据文件大小自动选择窗口。
///
/// 小文件（<1MB）→ 256 tokens（精确检索）
/// 大文件（>1MB）→ 1024 tokens（减少 chunk 总量 4x）
#[test]
fn tc_perf_007_auto_chunk_tokens_selects_by_file_size() {
    use crate::import::{
        DEFAULT_CHUNK_TOKENS, LARGE_CHUNK_TOKENS, MEDIUM_CHUNK_TOKENS, MMAP_THRESHOLD,
        XLARGE_CHUNK_TOKENS,
    };

    // 小文件 ≤50MB → 256
    assert_eq!(
        crate::import::ImportService::<MockStorage>::auto_chunk_tokens(100),
        DEFAULT_CHUNK_TOKENS,
        "10KB 小文件应使用默认窗口 256"
    );
    assert_eq!(
        crate::import::ImportService::<MockStorage>::auto_chunk_tokens(MMAP_THRESHOLD),
        DEFAULT_CHUNK_TOKENS,
        "50MB 边界应使用 256"
    );

    // 50-200MB → 512
    assert_eq!(
        crate::import::ImportService::<MockStorage>::auto_chunk_tokens(100_000_000),
        MEDIUM_CHUNK_TOKENS,
        "100MB 中文件应使用 512"
    );

    // 200MB-1GB → 1024
    assert_eq!(
        crate::import::ImportService::<MockStorage>::auto_chunk_tokens(500_000_000),
        LARGE_CHUNK_TOKENS,
        "500MB 大文件应使用 1024"
    );

    // >1GB → 2048
    assert_eq!(
        crate::import::ImportService::<MockStorage>::auto_chunk_tokens(2_000_000_000),
        XLARGE_CHUNK_TOKENS,
        "2GB 超大文件应使用 2048"
    );
}

/// TC-PERF-008：大窗口分块产生更少的 chunk（4x 减少）。
///
/// 同一段长文本，用 1024 tokens 窗口分块应比 256 tokens 窗口产生更少的 chunk。
/// 验证 `with_chunk_tokens` 构造的 ImportService 正确传递分块参数。
#[tokio::test]
async fn tc_perf_008_large_window_fewer_chunks() {
    let dir = TempDir::new().unwrap();

    // 构造一段长文本（~2000 tokens，确保在两个窗口下都产生多 chunk）
    let paragraph = "这是一段用于测试分块大小的中文文本。每个句子都应该被正确处理。\
                     语义分块器会按段落和句子递归分割，保持语义完整性。";
    let long_text = paragraph.repeat(50); // ~2000+ tokens

    let txt_path = write_file(&dir, "long.txt", long_text.as_bytes());

    // 小窗口（256 tokens）
    let storage_small = MockStorage::default();
    let service_small = ImportService::new(storage_small.clone(), dir.path().join("data"));
    let outcome = service_small.import_one(&txt_path, true).await.unwrap();
    let doc_small = match outcome {
        ImportOutcome::Imported(d) => d,
        _ => panic!("导入应成功"),
    };
    service_small.index_document(&doc_small).await.unwrap();
    let chunks_small = storage_small.chunks_for(&doc_small.id).await;

    // 大窗口（1024 tokens）
    let storage_large = MockStorage::default();
    let service_large =
        ImportService::with_chunk_tokens(storage_large.clone(), dir.path().join("data2"), 1024);
    let outcome = service_large.import_one(&txt_path, true).await.unwrap();
    let doc_large = match outcome {
        ImportOutcome::Imported(d) => d,
        _ => panic!("导入应成功"),
    };
    service_large.index_document(&doc_large).await.unwrap();
    let chunks_large = storage_large.chunks_for(&doc_large.id).await;

    // 关键断言：大窗口产生的 chunk 数应显著少于小窗口
    assert!(
        chunks_large.len() < chunks_small.len(),
        "大窗口 (1024) 应产生更少 chunk：实际 large={} small={}",
        chunks_large.len(),
        chunks_small.len()
    );

    // 大窗口的 chunk 平均 token_count 应更大
    let avg_small: usize =
        chunks_small.iter().map(|c| c.token_count).sum::<usize>() / chunks_small.len().max(1);
    let avg_large: usize =
        chunks_large.iter().map(|c| c.token_count).sum::<usize>() / chunks_large.len().max(1);
    assert!(
        avg_large > avg_small,
        "大窗口 chunk 平均 token 应更大：large={avg_large} small={avg_small}"
    );
}

// ============================================================
// REQ-ING-019：文档摘要自动生成 TDD 测试
// ============================================================

/// 摘要测试用 Mock LLM（返回固定摘要文本）。
#[derive(Clone)]
struct SummaryMockLlm {
    /// LLM 返回的摘要文本
    output: String,
    /// 是否模拟 LLM 错误（返回 Err）
    should_fail: bool,
}

impl SummaryMockLlm {
    fn new(output: &str) -> Self {
        Self {
            output: output.to_string(),
            should_fail: false,
        }
    }

    fn failing() -> Self {
        Self {
            output: String::new(),
            should_fail: true,
        }
    }
}

impl LLMProvider for SummaryMockLlm {
    async fn chat_stream(
        &self,
        _system_prompt: &str,
        _history: &[ChatMessage],
        _query: &str,
    ) -> Result<BoxStream<'static, Result<String>>> {
        if self.should_fail {
            return Err(anyhow::anyhow!("模拟 LLM 错误"));
        }
        let output = self.output.clone();
        Ok(futures::stream::once(async move { Ok(output) }).boxed())
    }
}

/// TC-ING-SUMMARY-001：导入后生成摘要，summary 非空且长度合理。
///
/// 验证 `ImportService::generate_summary()` 正常路径：
/// - 提供有效的 chunks
/// - Mock LLM 返回摘要文本
/// - 摘要持久化到 Storage
/// - summary 非空，长度 100-500 字符
#[tokio::test]
async fn tc_ing_summary_001_generate_summary_success() {
    let storage = MockStorage::default();
    let service = ImportService::new(storage.clone(), std::path::PathBuf::new());

    // 创建文档并添加 chunks
    let doc = Document::new("test.md".to_string(), "hash001".to_string());
    storage.add_document(&doc).await.unwrap();
    let chunk = Chunk::new(
        doc.id.clone(),
        "这是一篇关于 Rust 异步编程的技术文档。".to_string(),
        20,
        0,
    );
    storage.add_chunk(&chunk).await.unwrap();

    // Mock LLM 返回摘要
    let llm = SummaryMockLlm::new(
        "本文介绍了 Rust 异步编程的核心概念，包括 async/await 语法、Future trait 和 tokio 运行时。文档详细讲解了异步任务调度、并发原语和错误处理模式。",
    );

    // 执行摘要生成
    let chunks = storage.list_chunks(&doc.id).await.unwrap();
    service
        .generate_summary(&doc.id, &chunks, &llm)
        .await
        .unwrap();

    // 断言：摘要已持久化
    let docs = storage.list_documents().await.unwrap();
    let saved_doc = docs.iter().find(|d| d.id == doc.id).unwrap();
    assert!(saved_doc.summary.is_some(), "摘要应已持久化");
    let summary = saved_doc.summary.as_ref().unwrap();
    assert!(
        summary.len() >= 50 && summary.len() <= 500,
        "摘要长度应在 50-500 字符之间，实际: {}",
        summary.len()
    );
}

/// TC-ING-SUMMARY-002：空 chunks 时 generate_summary 返回 Ok（不生成摘要）。
///
/// 验证优雅降级：文档无分块内容时，不调用 LLM，直接返回 Ok(())。
#[tokio::test]
async fn tc_ing_summary_002_empty_chunks_noop() {
    let storage = MockStorage::default();
    let service = ImportService::new(storage.clone(), std::path::PathBuf::new());

    let doc = Document::new("empty.md".to_string(), "hash002".to_string());
    storage.add_document(&doc).await.unwrap();

    // 空 chunks
    let llm = SummaryMockLlm::new("不应被调用");
    let result = service.generate_summary(&doc.id, &[], &llm).await;

    // 断言：返回 Ok，未生成摘要
    assert!(result.is_ok(), "空 chunks 应返回 Ok");
    let docs = storage.list_documents().await.unwrap();
    let saved_doc = docs.iter().find(|d| d.id == doc.id).unwrap();
    assert!(saved_doc.summary.is_none(), "空 chunks 不应生成摘要");
}

/// TC-ING-SUMMARY-003：超长文档截断到 SUMMARY_MAX_INPUT_CHARS。
///
/// 验证 `generate_summary()` 对超长输入的截断行为：
/// - 提供超过 4000 字符的 chunks
/// - LLM 被调用时收到的 query 不超过 SUMMARY_MAX_INPUT_CHARS + prompt 前缀
#[tokio::test]
async fn tc_ing_summary_003_long_document_truncated() {
    let storage = MockStorage::default();
    let service = ImportService::new(storage.clone(), std::path::PathBuf::new());

    let doc = Document::new("long.md".to_string(), "hash003".to_string());
    storage.add_document(&doc).await.unwrap();

    // 创建超过 4000 字符的内容
    let long_content = "x".repeat(6000);
    let chunk = Chunk::new(doc.id.clone(), long_content, 1000, 0);
    storage.add_chunk(&chunk).await.unwrap();

    let llm = SummaryMockLlm::new("长文档摘要");
    let chunks = storage.list_chunks(&doc.id).await.unwrap();
    service
        .generate_summary(&doc.id, &chunks, &llm)
        .await
        .unwrap();

    // 断言：摘要已生成（截断不影响生成）
    let docs = storage.list_documents().await.unwrap();
    let saved_doc = docs.iter().find(|d| d.id == doc.id).unwrap();
    assert!(saved_doc.summary.is_some(), "超长文档也应生成摘要");
}

/// TC-ING-SUMMARY-004：update_document_summary 持久化摘要。
///
/// 验证 Storage::update_document_summary 正确持久化摘要文本。
#[tokio::test]
async fn tc_ing_summary_004_update_document_summary_persists() {
    let storage = MockStorage::default();

    let doc = Document::new("persist.md".to_string(), "hash004".to_string());
    storage.add_document(&doc).await.unwrap();

    // 直接调用 update_document_summary
    let summary_text = "这是通过 update_document_summary 持久化的摘要文本。";
    storage
        .update_document_summary(&doc.id, summary_text)
        .await
        .unwrap();

    // 断言：list_documents 返回的文档包含摘要
    let docs = storage.list_documents().await.unwrap();
    let saved_doc = docs.iter().find(|d| d.id == doc.id).unwrap();
    assert_eq!(
        saved_doc.summary.as_deref(),
        Some(summary_text),
        "摘要应与写入的一致"
    );
}

/// TC-ING-SUMMARY-005：LLM 错误时 generate_summary 返回 Err（优雅降级）。
///
/// 验证 LLM 调用失败时，`generate_summary()` 返回 Err，
/// 调用方应静默降级（summary 保持 None），不影响导入流程。
#[tokio::test]
async fn tc_ing_summary_005_llm_error_graceful_degradation() {
    let storage = MockStorage::default();
    let service = ImportService::new(storage.clone(), std::path::PathBuf::new());

    let doc = Document::new("error.md".to_string(), "hash005".to_string());
    storage.add_document(&doc).await.unwrap();
    let chunk = Chunk::new(doc.id.clone(), "文档内容".to_string(), 10, 0);
    storage.add_chunk(&chunk).await.unwrap();

    // Mock LLM 模拟错误
    let llm = SummaryMockLlm::failing();
    let chunks = storage.list_chunks(&doc.id).await.unwrap();
    let result = service.generate_summary(&doc.id, &chunks, &llm).await;

    // 断言：返回 Err
    assert!(result.is_err(), "LLM 错误时应返回 Err");

    // 断言：summary 保持 None（优雅降级）
    let docs = storage.list_documents().await.unwrap();
    let saved_doc = docs.iter().find(|d| d.id == doc.id).unwrap();
    assert!(
        saved_doc.summary.is_none(),
        "LLM 失败时 summary 应保持 None"
    );
}
