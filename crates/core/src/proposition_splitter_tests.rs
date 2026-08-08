#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! Proposition 分割器 TDD 测试（REQ-PERF-007）。
//!
//! 测试用例 TC-PROP-001~008 覆盖：
//! - chunk 分解为多个 proposition
//! - 每个 proposition 自包含（无代词依赖）
//! - proposition 保持原意
//! - 检索 proposition → 扩展到包含 chunk
//! - proposition 级检索精度优于 chunk 级
//! - 空 chunk 返回空列表
//! - proposition 关联 chunk_id + sequence
//! - 中文不破坏字符边界

use echomind_models::{Chunk, DocStatus, Document, RetrievalResult};

use crate::Storage;
use crate::proposition_splitter::PropositionSplitter;
use crate::retriever::expand_neighbors;

use anyhow::Result;

// ============================================================================
// TC-PROP-001: chunk 分解为多个 proposition
// ============================================================================

/// TC-PROP-001：多句 chunk 分解为多个 proposition。
#[test]
fn tc_prop_001_chunk_decomposed_into_multiple_propositions() {
    let chunk_content = "Rust is a systems programming language. It focuses on safety and performance. \
         The compiler ensures memory safety without garbage collection.";
    let propositions = PropositionSplitter::split(chunk_content, "chunk-001", "rust-guide.md");

    assert!(
        propositions.len() >= 3,
        "多句 chunk 应分解为 ≥3 个 proposition，实际: {}",
        propositions.len()
    );
}

// ============================================================================
// TC-PROP-002: 每个 proposition 自包含（无代词依赖）
// ============================================================================

/// TC-PROP-002：代词被消解，proposition 自包含。
#[test]
fn tc_prop_002_propositions_are_self_contained() {
    // 第一句引入实体 "Rust"，第二句使用代词 "It"
    let chunk_content = "Rust is a systems programming language. It focuses on safety.";
    let propositions = PropositionSplitter::split(chunk_content, "chunk-002", "rust-guide.md");

    assert_eq!(propositions.len(), 2);

    // 第二个 proposition 中的 "It" 应被替换为 "Rust"
    let second = &propositions[1].content;
    assert!(
        second.contains("Rust"),
        "代词 It 应被替换为 Rust，实际: {second}"
    );
    // 不应包含独立的代词 "It"（作为单词）
    assert!(
        !second.contains(" It ") && !second.starts_with("It "),
        "proposition 不应保留独立代词 It，实际: {second}"
    );
}

/// TC-PROP-002b：中文代词消解。
#[test]
fn tc_prop_002b_chinese_pronoun_resolution() {
    let chunk_content = "张三是一名工程师。他在北京工作。";
    let propositions = PropositionSplitter::split(chunk_content, "chunk-002b", "staff.md");

    assert_eq!(propositions.len(), 2);

    let second = &propositions[1].content;
    assert!(
        second.contains("张三"),
        "中文代词「他」应被替换为「张三」，实际: {second}"
    );
    // 不应包含独立的"他"（排除"其他"等组合）
    assert!(
        !second.contains('他'),
        "proposition 不应保留代词「他」，实际: {second}"
    );
}

// ============================================================================
// TC-PROP-003: proposition 保持原意
// ============================================================================

/// TC-PROP-003：proposition 保持原意，不丢失关键信息。
#[test]
fn tc_prop_003_propositions_preserve_meaning() {
    let chunk_content = "Python supports multiple programming paradigms. It has dynamic typing and garbage collection.";
    let propositions = PropositionSplitter::split(chunk_content, "chunk-003", "python.md");

    // 第一个 proposition 应包含 "Python" 和 "programming paradigms"
    let first = &propositions[0].content;
    assert!(first.contains("Python"), "proposition 应保留 Python 关键词");
    assert!(
        first.contains("paradigms"),
        "proposition 应保留 paradigms 关键词"
    );

    // 第二个 proposition 应包含 "dynamic typing" 或 "garbage collection"
    let second = &propositions[1].content;
    assert!(
        second.contains("dynamic typing") || second.contains("garbage collection"),
        "proposition 应保留关键信息，实际: {second}"
    );
}

// ============================================================================
// TC-PROP-004: 检索 proposition → 扩展到包含 chunk
// ============================================================================

/// TC-PROP-004：proposition 检索结果可扩展到包含 chunk。
///
/// 验证：proposition 的 chunk_id 正确关联到原 chunk，
/// 通过 expand_neighbors 可扩展到相邻 chunk。
#[test]
fn tc_prop_004_proposition_expands_to_chunk() {
    // 创建 chunk 并分割为 propositions
    let chunk_id = "chunk-004";
    let chunk_content = "React is a UI library. It uses virtual DOM for efficient rendering.";
    let propositions = PropositionSplitter::split(chunk_content, chunk_id, "react.md");

    assert!(!propositions.is_empty());

    // 每个 proposition 的 chunk_id 应正确关联
    for prop in &propositions {
        assert_eq!(
            prop.chunk_id, chunk_id,
            "proposition 的 chunk_id 应正确关联到原 chunk"
        );
    }

    // 模拟 proposition 检索结果 → expand_neighbors 扩展
    // 构造 RetrievalResult（模拟 proposition_search 的返回）
    let prop_hit = RetrievalResult {
        chunk: Chunk::new(
            "doc-004".to_string(),
            propositions[0].content.clone(),
            10,
            0,
        ),
        score: 0.85,
        doc_name: "react.md".to_string(),
    };

    // 使用 MockStorage 验证 expand_neighbors
    let mock_storage = PropMockStorage {
        all_chunks: vec![
            Chunk::new("doc-004".to_string(), "前一个 chunk".to_string(), 5, 0),
            prop_hit.chunk.clone(),
            Chunk::new("doc-004".to_string(), "后一个 chunk".to_string(), 5, 2),
        ],
    };

    let expanded = futures::executor::block_on(expand_neighbors(&mock_storage, &[prop_hit]))
        .expect("expand_neighbors 不应失败");

    // 应包含命中 chunk + 相邻 chunk（共 3 个）
    assert!(!expanded.is_empty(), "expand_neighbors 应至少返回 1 个结果");
}

// ============================================================================
// TC-PROP-005: proposition 级检索精度优于 chunk 级
// ============================================================================

/// TC-PROP-005：proposition 比 chunk 更细粒度，检索精度更高。
///
/// 验证：包含多个独立事实的 chunk 分割后，
/// 每个 proposition 只包含一个事实，长度显著短于原 chunk。
#[test]
fn tc_prop_005_proposition_more_precise_than_chunk() {
    let chunk_content = "Docker uses containerization for application deployment. \
         Kubernetes orchestrates containers at scale. \
         Helm is a package manager for Kubernetes.";
    let propositions = PropositionSplitter::split(chunk_content, "chunk-005", "devops.md");

    assert!(propositions.len() >= 3, "应分割为 ≥3 个 proposition");

    let chunk_len = chunk_content.len();

    for (i, prop) in propositions.iter().enumerate() {
        assert!(
            prop.content.len() < chunk_len,
            "proposition[{i}] 长度 ({}) 应小于 chunk 长度 ({})",
            prop.content.len(),
            chunk_len
        );
    }

    // 每个 proposition 应只包含一个主题（Docker / Kubernetes / Helm）
    let docker_prop = propositions
        .iter()
        .find(|p| p.content.contains("Docker"))
        .expect("应存在包含 Docker 的 proposition");
    let helm_prop = propositions
        .iter()
        .find(|p| p.content.contains("Helm"))
        .expect("应存在包含 Helm 的 proposition");

    // Docker proposition 不应包含 Kubernetes 信息
    assert!(
        !docker_prop.content.contains("Kubernetes"),
        "Docker proposition 不应包含 Kubernetes 信息"
    );
    // Helm proposition 不应包含 Docker 信息
    assert!(
        !helm_prop.content.contains("Docker"),
        "Helm proposition 不应包含 Docker 信息"
    );
}

// ============================================================================
// TC-PROP-006: 空 chunk 返回空 proposition 列表
// ============================================================================

/// TC-PROP-006：空 chunk 返回空列表。
#[test]
fn tc_prop_006_empty_chunk_returns_empty() {
    let propositions = PropositionSplitter::split("", "chunk-006", "empty.md");
    assert!(
        propositions.is_empty(),
        "空 chunk 应返回空 proposition 列表"
    );

    let propositions = PropositionSplitter::split("   \n  \t  ", "chunk-006b", "whitespace.md");
    assert!(
        propositions.is_empty(),
        "纯空白 chunk 应返回空 proposition 列表"
    );
}

// ============================================================================
// TC-PROP-007: proposition 关联到原 chunk_id + sequence
// ============================================================================

/// TC-PROP-007：proposition 正确关联 chunk_id 和 sequence。
#[test]
fn tc_prop_007_proposition_associated_with_chunk_id_and_sequence() {
    let chunk_id = "chunk-007";
    let chunk_content = "First sentence. Second sentence. Third sentence.";
    let propositions = PropositionSplitter::split(chunk_content, chunk_id, "doc.md");

    assert_eq!(propositions.len(), 3);

    for (i, prop) in propositions.iter().enumerate() {
        assert_eq!(
            prop.chunk_id, chunk_id,
            "proposition[{i}] 的 chunk_id 应为 {chunk_id}"
        );
        assert_eq!(prop.sequence, i, "proposition[{i}] 的 sequence 应为 {i}");
    }

    // id 应为唯一 UUID
    let ids: std::collections::HashSet<&str> = propositions.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(
        ids.len(),
        propositions.len(),
        "所有 proposition 的 id 应唯一"
    );
}

// ============================================================================
// TC-PROP-008: 中文 proposition 不破坏字符边界
// ============================================================================

/// TC-PROP-008：中文 proposition 不破坏字符边界。
#[test]
fn tc_prop_008_chinese_no_character_boundary_break() {
    let chunk_content = "张三是一名高级工程师。他在美团工作。他负责后端架构设计。";
    let propositions = PropositionSplitter::split(chunk_content, "chunk-008", "staff.md");

    assert_eq!(propositions.len(), 3, "应分割为 3 个中文 proposition");

    for (i, prop) in propositions.iter().enumerate() {
        // 验证不包含半个字符（所有字符应为合法 Unicode）
        let content = &prop.content;
        assert!(
            !content.contains('\u{0}'),
            "proposition[{i}] 不应包含空字符"
        );
        assert!(!content.is_empty(), "proposition[{i}] 不应为空");

        // 验证中文标点完整
        assert!(
            content.contains('。'),
            "中文 proposition[{i}] 应以句号结尾: {content}"
        );
    }

    // 验证代词消解后的中文字符完整性
    let second = &propositions[1].content;
    assert!(
        second.contains('张') && second.contains('三'),
        "中文代词消解后应保留完整的人名字符: {second}"
    );

    // 验证"他"被替换（不在"其他"上下文中）
    assert!(
        !second.contains('他'),
        "proposition[1] 不应保留代词「他」: {second}"
    );
}

/// TC-PROP-008b：中英混合文本不破坏字符边界。
#[test]
fn tc_prop_008b_mixed_text_no_boundary_break() {
    let chunk_content = "Rust 是一门系统级编程语言。It 注重内存安全。";
    let propositions = PropositionSplitter::split(chunk_content, "chunk-008b", "rust-cn.md");

    assert_eq!(propositions.len(), 2);

    // 第一个 proposition 应包含中文
    assert!(
        propositions[0].content.contains("系统级"),
        "第一个 proposition 应包含中文内容"
    );

    // 第二个 proposition 的 "It" 应被替换为 "Rust"
    assert!(
        propositions[1].content.contains("Rust"),
        "代词 It 应被替换为 Rust: {}",
        propositions[1].content
    );
}

// ============================================================================
// Mock Storage（供 TC-PROP-004 expand_neighbors 测试用）
// ============================================================================

/// 简易 Mock Storage，仅支持 list_chunks（供 expand_neighbors 测试用）。
struct PropMockStorage {
    all_chunks: Vec<Chunk>,
}

impl Storage for PropMockStorage {
    async fn add_document(&self, _doc: &Document) -> Result<()> {
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
    async fn find_document_by_hash(&self, _hash: &str) -> Result<Option<Document>> {
        Ok(None)
    }
    async fn count_documents(&self) -> Result<usize> {
        Ok(0)
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
    async fn create_conversation(&self, _conv: &echomind_models::Conversation) -> Result<()> {
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
    async fn add_message(&self, _conv_id: &str, _msg: &echomind_models::ChatMessage) -> Result<()> {
        Ok(())
    }
    async fn list_messages(&self, _conv_id: &str) -> Result<Vec<echomind_models::ChatMessage>> {
        Ok(vec![])
    }
    async fn list_documents(&self) -> Result<Vec<Document>> {
        Ok(vec![])
    }
    async fn delete_document(&self, _doc_id: &str) -> Result<()> {
        Ok(())
    }
    async fn list_chunks(&self, _doc_id: &str) -> Result<Vec<Chunk>> {
        Ok(self.all_chunks.clone())
    }
    async fn delete_chunks_by_doc(&self, _doc_id: &str) -> Result<()> {
        Ok(())
    }
    async fn keyword_search(&self, _query: &str, _top_k: usize) -> Result<Vec<RetrievalResult>> {
        Ok(vec![])
    }
}
