//! Document 归一化注入（B-06 借鉴 Rig `CompletionRequest::documents`）。
//!
//! ## 核心设计
//!
//! 将 RAG 检索结果标准化为 `Document` 结构体，通过 `normalized_documents()` 方法
//! 统一注入到 prompt 中，替代手动字符串拼接。
//!
//! ## 与现有 `build_rag_prompt_segmented` 的对比
//!
//! | 维度 | 现有 | Document 归一化 |
//! |---|---|---|
//! | 检索结果传递 | 手动拼接到 `dynamic_context` 字符串 | `Vec<Document>` 结构化 |
//! | 格式化 | 散落在多个函数中 | 统一在 `normalized_documents()` |
//! | 扩展性 | 仅支持文本拼接 | 未来可支持 provider 原生 document 消息 |

use echomind_models::RetrievalResult;
use serde::{Deserialize, Serialize};

/// 归一化的文档片段（B-06 借鉴 Rig Document）。
///
/// 将检索结果标准化为统一格式，支持通过 `normalized_documents()` 方法
/// 注入到 prompt 或 provider 原生 document 消息中。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    /// 文档 ID
    pub id: String,
    /// 文档内容文本
    pub text: String,
    /// 来源文档名（用于引用标注）
    pub source: String,
    /// 相似度得分
    pub score: f32,
    /// 元数据（自由 JSON 值）
    pub metadata: serde_json::Value,
}

impl Document {
    /// 从 `RetrievalResult` 转换。
    pub fn from_retrieval(result: &RetrievalResult) -> Self {
        Self {
            id: result.chunk.id.clone(),
            text: result.chunk.content.clone(),
            source: result.doc_name.clone(),
            score: result.score,
            metadata: serde_json::json!({
                "doc_id": result.chunk.doc_id,
                "token_count": result.chunk.token_count,
                "sequence": result.chunk.sequence,
            }),
        }
    }

    /// 从 `RetrievalResult` 列表批量转换。
    pub fn from_retrieval_list(results: &[RetrievalResult]) -> Vec<Self> {
        results.iter().map(Self::from_retrieval).collect()
    }
}

/// 将 `Vec<Document>` 归一化为编号文本格式（用于 prompt 注入）。
///
/// 格式：
/// ```text
/// [1] 《doc_name》(score=0.95):
/// document text...
///
/// [2] 《doc_name》(score=0.88):
/// document text...
/// ```
///
/// # 参数
/// - `documents`: 文档列表
///
/// # 返回
/// 格式化后的文本，可直接拼接到 `dynamic_context` 中。
pub fn normalized_documents(documents: &[Document]) -> String {
    let mut result = String::new();
    for (i, doc) in documents.iter().enumerate() {
        result.push_str(&format!(
            "[{}] 《{}》(score={:.2}):\n{}\n\n",
            i + 1,
            doc.source,
            doc.score,
            doc.text,
        ));
    }
    result
}

/// 将 `Vec<RetrievalResult>` 转换为 `Vec<Document>` 并归一化为编号文本。
///
/// 便捷方法：一步完成 `RetrievalResult → Document → 文本` 转换。
pub fn documents_from_retrieval(results: &[RetrievalResult]) -> String {
    let docs = Document::from_retrieval_list(results);
    normalized_documents(&docs)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use echomind_models::{Chunk, RetrievalResult};

    fn make_result(doc_name: &str, content: &str, score: f32) -> RetrievalResult {
        RetrievalResult {
            chunk: Chunk {
                id: "chunk-1".into(),
                doc_id: "doc-1".into(),
                content: content.into(),
                token_count: 100,
                sequence: 0,
            },
            doc_name: doc_name.into(),
            score,
        }
    }

    // ─── Document ───

    #[test]
    fn tc_doc_001_from_retrieval() {
        let result = make_result("guide.pdf", "Rust is safe", 0.95);
        let doc = Document::from_retrieval(&result);
        assert_eq!(doc.id, "chunk-1");
        assert_eq!(doc.text, "Rust is safe");
        assert_eq!(doc.source, "guide.pdf");
        assert_eq!(doc.score, 0.95);
        assert_eq!(doc.metadata["doc_id"], "doc-1");
    }

    #[test]
    fn tc_doc_002_from_retrieval_list() {
        let results = vec![
            make_result("a.pdf", "content a", 0.9),
            make_result("b.pdf", "content b", 0.8),
        ];
        let docs = Document::from_retrieval_list(&results);
        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].source, "a.pdf");
        assert_eq!(docs[1].source, "b.pdf");
    }

    // ─── normalized_documents ───

    #[test]
    fn tc_doc_003_normalized_single_document() {
        let docs = vec![Document {
            id: "d1".into(),
            text: "Rust is a systems language".into(),
            source: "guide.pdf".into(),
            score: 0.95,
            metadata: serde_json::Value::Null,
        }];
        let text = normalized_documents(&docs);
        assert!(text.contains("[1]"));
        assert!(text.contains("guide.pdf"));
        assert!(text.contains("0.95"));
        assert!(text.contains("Rust is a systems language"));
    }

    #[test]
    fn tc_doc_004_normalized_multiple_documents() {
        let docs = vec![
            Document {
                id: "d1".into(),
                text: "First".into(),
                source: "a.pdf".into(),
                score: 0.9,
                metadata: serde_json::Value::Null,
            },
            Document {
                id: "d2".into(),
                text: "Second".into(),
                source: "b.pdf".into(),
                score: 0.8,
                metadata: serde_json::Value::Null,
            },
        ];
        let text = normalized_documents(&docs);
        assert!(text.contains("[1]"));
        assert!(text.contains("[2]"));
        assert!(text.contains("a.pdf"));
        assert!(text.contains("b.pdf"));
        assert!(text.contains("First"));
        assert!(text.contains("Second"));
    }

    #[test]
    fn tc_doc_005_normalized_empty_documents() {
        let docs: Vec<Document> = vec![];
        let text = normalized_documents(&docs);
        assert!(text.is_empty());
    }

    // ─── documents_from_retrieval ───

    #[test]
    fn tc_doc_006_documents_from_retrieval() {
        let results = vec![
            make_result("a.pdf", "content a", 0.9),
            make_result("b.pdf", "content b", 0.8),
        ];
        let text = documents_from_retrieval(&results);
        assert!(text.contains("[1]"));
        assert!(text.contains("[2]"));
        assert!(text.contains("a.pdf"));
        assert!(text.contains("content a"));
    }

    #[test]
    fn tc_doc_007_documents_from_empty_retrieval() {
        let results: Vec<RetrievalResult> = vec![];
        let text = documents_from_retrieval(&results);
        assert!(text.is_empty());
    }

    // ─── Document 序列化 ───

    #[test]
    fn tc_doc_008_document_serde_roundtrip() {
        let doc = Document {
            id: "d1".into(),
            text: "hello".into(),
            source: "test.pdf".into(),
            score: 0.5,
            metadata: serde_json::json!({"key": "value"}),
        };
        let json = serde_json::to_string(&doc).unwrap();
        let back: Document = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, "d1");
        assert_eq!(back.text, "hello");
        assert_eq!(back.source, "test.pdf");
        assert_eq!(back.score, 0.5);
        assert_eq!(back.metadata["key"], "value");
    }
}
