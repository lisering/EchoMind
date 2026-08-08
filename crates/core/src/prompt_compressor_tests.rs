//! Prompt 压缩模块测试（TC-COMP-001~010）。
//!
//! 测试规则压缩器（`RuleBasedCompressor`）的各类压缩行为：
//! - 停用词去除、冗余空白去除、代码注释去除
//! - 不同压缩比下的信息保留率
//! - 中文文本字符边界安全
//! - token 数量化对比
//! - 语义保持
//! - 禁用时向后兼容

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use crate::PromptCompressor;
use crate::prompt_compressor::RuleBasedCompressor;

/// TC-COMP-001：规则压缩去除停用词后保持关键信息。
///
/// 输入含英文停用词的文本，压缩后停用词被去除但关键名词保留。
#[test]
fn tc_comp_001_stopwords_removed_key_info_preserved() {
    let compressor = RuleBasedCompressor::new();
    let text = "The quick brown fox jumps over the lazy dog. \
                The dog is very lazy and does not move.";
    let query = "fox dog";
    let result = compressor.compress_sync(text, 2.0, query);

    // 关键信息保留
    assert!(result.contains("fox"), "fox 应被保留");
    assert!(result.contains("dog"), "dog 应被保留");
    // 停用词被去除
    let lower = result.to_lowercase();
    assert!(
        !lower.split_whitespace().any(|w| w == "the"),
        "the 应被去除"
    );
}

/// TC-COMP-002：规则压缩去除冗余空白。
///
/// 输入含多空格、多空行的文本，压缩后空白被规整。
#[test]
fn tc_comp_002_redundant_whitespace_removed() {
    let compressor = RuleBasedCompressor::new();
    let text = "Hello    world\n\n\n\nThis  is   a  test\n\n  End";
    let query = "hello world test";
    let result = compressor.compress_sync(text, 2.0, query);

    // 不应包含连续 2+ 个空格
    assert!(
        !result.contains("  "),
        "压缩后不应有连续空格, got: {result:?}"
    );
    // 不应包含连续 2+ 个换行
    assert!(!result.contains("\n\n\n"), "压缩后不应有连续 3+ 换行");
}

/// TC-COMP-003：规则压缩去除代码注释（保留代码块语法）。
///
/// 输入含 // 和 /* */ 注释的代码块，压缩后注释被去除但代码保留。
#[test]
fn tc_comp_003_code_comments_removed_structure_preserved() {
    let compressor = RuleBasedCompressor::new();
    let text = "Here is some code:\n\n\
                ```rust\n\
                // This is a comment\n\
                fn main() {\n\
                    /* block comment */\n\
                    println!(\"hello\");\n\
                }\n\
                ```";
    let query = "main function";
    let result = compressor.compress_sync(text, 2.0, query);

    // 代码块围栏保留
    assert!(result.contains("```"), "代码块围栏应保留");
    // 代码结构保留
    assert!(result.contains("fn main"), "fn main 应保留");
    assert!(result.contains("println"), "println 应保留");
    // 注释被去除
    assert!(!result.contains("// This is a comment"), "行注释应被去除");
    assert!(!result.contains("/* block comment */"), "块注释应被去除");
}

/// TC-COMP-004：压缩比 2x 时关键信息保留率 ≥ 90%。
///
/// 输入一段含关键术语的文本，2x 压缩后 ≥ 90% 的关键术语保留。
#[test]
fn tc_comp_004_ratio_2x_key_info_preservation_90_percent() {
    let compressor = RuleBasedCompressor::new();
    let text = "RAG (Retrieval-Augmented Generation) combines retrieval with generation. \
                The system retrieves relevant documents from a knowledge base. \
                Then it generates answers using the retrieved context. \
                Vector search uses cosine similarity for matching. \
                The embedding model is all-MiniLM-L6-v2 with 384 dimensions. \
                SQLite stores vectors as BLOB in WAL mode. \
                The chat engine orchestrates the RAG pipeline. \
                Prompt caching reduces API costs by 90 percent. \
                FastEmbed provides ONNX runtime for local inference.";
    let query = "RAG retrieval generation embedding vector SQLite";

    // 关键术语列表
    let key_terms = vec![
        "RAG",
        "retriev",
        "generation",
        "embed",
        "vector",
        "SQLite",
        "MiniLM",
        "384",
        "BLOB",
        "WAL",
        "chat",
        "caching",
        "FastEmbed",
        "ONNX",
    ];

    let result = compressor.compress_sync(text, 2.0, query);
    let preserved = key_terms
        .iter()
        .filter(|term| result.contains(*term))
        .count();
    let retention = preserved as f32 / key_terms.len() as f32;
    assert!(
        retention >= 0.90,
        "2x 压缩应保留 ≥ 90% 关键术语，实际 {:.0}% ({}/{})",
        retention * 100.0,
        preserved,
        key_terms.len()
    );
}

/// TC-COMP-005：压缩比 3x 时关键信息保留率 ≥ 80%。
#[test]
fn tc_comp_005_ratio_3x_key_info_preservation_80_percent() {
    let compressor = RuleBasedCompressor::new();
    let text = "RAG (Retrieval-Augmented Generation) combines retrieval with generation. \
                The system retrieves relevant documents from a knowledge base. \
                Then it generates answers using the retrieved context. \
                Vector search uses cosine similarity for matching. \
                The embedding model is all-MiniLM-L6-v2 with 384 dimensions. \
                SQLite stores vectors as BLOB in WAL mode. \
                The chat engine orchestrates the RAG pipeline. \
                Prompt caching reduces API costs by 90 percent. \
                FastEmbed provides ONNX runtime for local inference.";
    let query = "RAG retrieval generation embedding vector SQLite";

    let key_terms = vec![
        "RAG",
        "retriev",
        "generation",
        "embed",
        "vector",
        "SQLite",
        "MiniLM",
        "384",
        "BLOB",
        "WAL",
        "chat",
        "caching",
        "FastEmbed",
        "ONNX",
    ];

    let result = compressor.compress_sync(text, 3.0, query);
    let preserved = key_terms
        .iter()
        .filter(|term| result.contains(*term))
        .count();
    let retention = preserved as f32 / key_terms.len() as f32;
    assert!(
        retention >= 0.80,
        "3x 压缩应保留 ≥ 80% 关键术语，实际 {:.0}% ({}/{})",
        retention * 100.0,
        preserved,
        key_terms.len()
    );
}

/// TC-COMP-006：压缩比 5x 时关键信息保留率 ≥ 60%。
#[test]
fn tc_comp_006_ratio_5x_key_info_preservation_60_percent() {
    let compressor = RuleBasedCompressor::new();
    let text = "RAG (Retrieval-Augmented Generation) combines retrieval with generation. \
                The system retrieves relevant documents from a knowledge base. \
                Then it generates answers using the retrieved context. \
                Vector search uses cosine similarity for matching. \
                The embedding model is all-MiniLM-L6-v2 with 384 dimensions. \
                SQLite stores vectors as BLOB in WAL mode. \
                The chat engine orchestrates the RAG pipeline. \
                Prompt caching reduces API costs by 90 percent. \
                FastEmbed provides ONNX runtime for local inference.";
    let query = "RAG retrieval generation embedding vector SQLite";

    let key_terms = vec![
        "RAG",
        "retriev",
        "generation",
        "embed",
        "vector",
        "SQLite",
        "MiniLM",
        "384",
        "BLOB",
        "WAL",
        "chat",
        "caching",
        "FastEmbed",
        "ONNX",
    ];

    let result = compressor.compress_sync(text, 5.0, query);
    let preserved = key_terms
        .iter()
        .filter(|term| result.contains(*term))
        .count();
    let retention = preserved as f32 / key_terms.len() as f32;
    assert!(
        retention >= 0.60,
        "5x 压缩应保留 ≥ 60% 关键术语，实际 {:.0}% ({}/{})",
        retention * 100.0,
        preserved,
        key_terms.len()
    );
}

/// TC-COMP-007：中文文本压缩不破坏字符边界。
///
/// 输入含中文字符的文本，压缩后不应出现乱码或截断的多字节字符。
#[test]
fn tc_comp_007_chinese_text_no_char_boundary_break() {
    let compressor = RuleBasedCompressor::new();
    let text = "检索增强生成（RAG）是一种结合检索和生成的技术。\
                系统从知识库中检索相关文档。\
                然后使用检索到的上下文生成答案。\
                向量搜索使用余弦相似度进行匹配。\
                嵌入模型是 all-MiniLM-L6-v2，维度为 384。";
    let query = "RAG 检索 生成 向量";
    let result = compressor.compress_sync(text, 3.0, query);

    // 验证压缩结果是有效的 UTF-8（Rust String 保证这一点）
    // 验证不包含乱码（每个中文字符都是有效的 Unicode）
    for c in result.chars() {
        // 确保没有无效的 Unicode 替换字符
        assert_ne!(c, '\u{FFFD}', "不应出现 Unicode 替换字符");
    }
    // 验证关键中文信息保留
    assert!(result.contains("RAG"), "RAG 应保留");
    // 验证不包含半个中文字符（通过 chars() 遍历无 panic 即可验证）
    let _ = result.chars().count();
}

/// TC-COMP-008：压缩后 token 数可量化（before/after 对比）。
///
/// 压缩后文本长度（字符数）应明显小于原始文本。
#[test]
fn tc_comp_008_token_count_reduction_quantifiable() {
    let compressor = RuleBasedCompressor::new();
    let text = "The retrieval augmented generation system is a very powerful tool. \
                It combines the retrieval of relevant documents with the generation \
                of natural language responses. The system uses vector search and \
                cosine similarity for matching. The embedding model transforms text \
                into high dimensional vectors for similarity comparison.";
    let query = "retrieval generation vector";
    let result = compressor.compress_sync(text, 3.0, query);

    let original_len = text.chars().count();
    let compressed_len = result.chars().count();

    assert!(
        compressed_len < original_len,
        "压缩后长度 ({compressed_len}) 应小于原始长度 ({original_len})"
    );
    // 压缩比至少 1.5x（规则压缩不如模型压缩激进，但应明显减少）
    let actual_ratio = original_len as f32 / compressed_len.max(1) as f32;
    assert!(
        actual_ratio >= 1.5,
        "实际压缩比应 ≥ 1.5x，实际 {actual_ratio:.2}x"
    );
}

/// TC-COMP-009：压缩结果可被 LLM 正确理解（语义保持）。
///
/// 压缩后文本仍包含回答 query 所需的关键信息。
#[test]
fn tc_comp_009_semantic_preservation() {
    let compressor = RuleBasedCompressor::new();
    let text = "EchoMind is a local RAG knowledge base application built with Rust and Tauri. \
                It uses fastembed for ONNX embedding with all-MiniLM-L6-v2 model. \
                SQLite stores vectors as BLOB with WAL mode. \
                The app supports SQLCipher AES-256 encryption for security. \
                PII detection covers 8 types including email, phone, and ID card.";
    let query = "EchoMind RAG Rust Tauri SQLite encryption";
    let result = compressor.compress_sync(text, 2.0, query);

    // 验证压缩后仍包含回答 query 所需的关键事实
    let facts = vec!["EchoMind", "RAG", "Rust", "Tauri", "SQLite", "encryption"];
    for fact in &facts {
        assert!(result.contains(fact), "压缩后应保留关键事实: {fact}");
    }
}

/// TC-COMP-010：压缩禁用时行为与当前完全一致（向后兼容）。
///
/// ratio = 1.0（或 ≤ 1.0）时，压缩结果应与原始文本完全一致。
#[test]
fn tc_comp_010_disabled_compression_backward_compatible() {
    let compressor = RuleBasedCompressor::new();
    let text = "This text should not be modified at all when compression is disabled.";
    let query = "anything";
    let result = compressor.compress_sync(text, 1.0, query);

    assert_eq!(result, text, "ratio=1.0 时压缩结果应与原文完全一致");

    // 测试 NoCompressor 也返回原文
    use crate::NoCompressor;
    let no_compressor = NoCompressor;
    let result2 = futures::executor::block_on(no_compressor.compress(text, 1.0, query))
        .expect("NoCompressor should succeed");
    assert_eq!(result2, text, "NoCompressor 应返回原文");
}

/// 额外测试：RuleBasedCompressor 实现了 PromptCompressor trait。
#[test]
fn tc_comp_trait_implementation() {
    let compressor = RuleBasedCompressor::new();
    let text = "This is a test sentence for the compressor trait implementation.";
    let query = "test compressor";

    // 通过 trait 调用
    let result = futures::executor::block_on(compressor.compress(text, 2.0, query))
        .expect("compress should succeed");
    assert!(!result.is_empty(), "压缩结果不应为空");
}
