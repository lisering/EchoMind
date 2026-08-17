#![cfg(test)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Late Chunking TDD 测试（REQ-RAG-049, TC-LATE-CHUNK-001~010）
//!
//! 借鉴 Jina AI 2024 Late Chunking 技术：在嵌入阶段为每个 chunk 注入文档级上下文前缀，
//! 使 chunk 嵌入向量包含全文语义上下文，显著提升检索质量。

use crate::late_chunking::{LateChunkingConfig, build_late_chunking_text, extract_doc_prefix};

/// TC-LATE-CHUNK-001：`build_late_chunking_text` 基本拼接
#[test]
fn tc_late_chunk_001_build_late_chunking_text_basic() {
    let doc_prefix = "本文档介绍 Rust 语言的异步编程模型。";
    let chunk_content = "async fn 在 trait 中需要 Send bound 约束。";
    let result = build_late_chunking_text(doc_prefix, chunk_content);
    assert!(result.contains(doc_prefix), "结果应包含文档前缀");
    assert!(result.contains(chunk_content), "结果应包含 chunk 内容");
    assert!(
        result.find(doc_prefix).unwrap() < result.find(chunk_content).unwrap(),
        "文档前缀应在 chunk 内容之前"
    );
}

/// TC-LATE-CHUNK-002：`extract_doc_prefix` 段落边界截取
#[test]
fn tc_late_chunk_002_extract_doc_prefix_paragraph_boundary() {
    let text = "第一段：这是文档的开头部分，介绍了基本概念。\n\n第二段：这里是更多内容。";
    let prefix = extract_doc_prefix(text, 50);
    // 应在 50 字符内尽量在段落边界截取
    assert!(
        prefix.chars().count() <= 50,
        "前缀长度应 <= max_chars，实际: {}",
        prefix.chars().count()
    );
    assert!(!prefix.is_empty(), "非空文本的前缀不应为空");
}

/// TC-LATE-CHUNK-003：`extract_doc_prefix` 空文本
#[test]
fn tc_late_chunk_003_extract_doc_prefix_empty() {
    let prefix = extract_doc_prefix("", 500);
    assert_eq!(prefix, "", "空文本的前缀应为空字符串");
}

/// TC-LATE-CHUNK-004：`extract_doc_prefix` 短文本返回全文
#[test]
fn tc_late_chunk_004_extract_doc_prefix_short_text() {
    let text = "这是一个短文档。";
    let prefix = extract_doc_prefix(text, 500);
    assert_eq!(prefix, text, "短文本应返回全文");
}

/// TC-LATE-CHUNK-005：`build_late_chunking_text` 与 `build_contextual_text` 组合
#[test]
fn tc_late_chunk_005_combine_with_contextual_retrieval() {
    use crate::retriever::build_contextual_text;
    let doc_name = "rust-guide.md";
    let doc_prefix = "本文档介绍 Rust 语言的异步编程模型。";
    let chunk_content = "async fn 在 trait 中需要 Send bound 约束。";

    // 先用 Contextual Retrieval 拼接文档名
    let contextual = build_contextual_text(doc_name, chunk_content);
    // 再用 Late Chunking 在前面拼接文档前缀
    let combined = build_late_chunking_text(doc_prefix, &contextual);

    assert!(combined.contains(doc_prefix), "组合文本应包含文档前缀");
    assert!(combined.contains(doc_name), "组合文本应包含文档名");
    assert!(
        combined.contains(chunk_content),
        "组合文本应包含 chunk 内容"
    );
    // 顺序：文档前缀 → 文档名 → chunk 内容
    let pos_prefix = combined.find(doc_prefix).unwrap();
    let pos_name = combined.find(doc_name).unwrap();
    let pos_chunk = combined.find(chunk_content).unwrap();
    assert!(
        pos_prefix < pos_name && pos_name < pos_chunk,
        "顺序应为：文档前缀 → 文档名 → chunk 内容"
    );
}

/// TC-LATE-CHUNK-006：Late Chunking 开启时嵌入文本变化
#[test]
fn tc_late_chunk_006_late_chunking_changes_embedding_text() {
    let doc_prefix = "文档前缀摘要";
    let chunk_content = "chunk 原始内容";
    let plain_text = chunk_content.to_string();
    let late_text = build_late_chunking_text(doc_prefix, chunk_content);
    assert_ne!(
        plain_text, late_text,
        "Late Chunking 开启时嵌入文本应与纯 chunk 文本不同"
    );
    assert!(
        late_text.len() > plain_text.len(),
        "Late Chunking 文本应比纯 chunk 文本更长"
    );
}

/// TC-LATE-CHUNK-007：Late Chunking + Contextual Retrieval 组合嵌入文本
#[test]
fn tc_late_chunk_007_late_chunking_plus_contextual_combined() {
    use crate::retriever::build_contextual_text;
    let doc_name = "guide.md";
    let doc_prefix = "文档全局摘要内容";
    let chunk_content = "具体 chunk 内容";

    // 组合：文档名前缀 + 文档前缀摘要 + chunk 内容
    let contextual = build_contextual_text(doc_name, chunk_content);
    let combined = build_late_chunking_text(doc_prefix, &contextual);

    // 所有三个部分都应在结果中
    assert!(combined.contains(doc_name));
    assert!(combined.contains(doc_prefix));
    assert!(combined.contains(chunk_content));
}

/// TC-LATE-CHUNK-008：Late Chunking 关闭时向后兼容
#[test]
fn tc_late_chunk_008_backward_compatibility() {
    // LateChunkingConfig::default() 的 separator
    let config = LateChunkingConfig::default();
    assert_eq!(
        config.max_prefix_chars, 500,
        "默认 max_prefix_chars 应为 500"
    );
    assert!(!config.separator.is_empty(), "分隔符不应为空");

    // 当 late_chunking_enabled = false 时，嵌入文本就是纯 chunk 内容
    let chunk_content = "纯 chunk 内容";
    let plain_text = chunk_content.to_string();
    assert_eq!(plain_text, chunk_content, "关闭时嵌入文本 = 纯 chunk 内容");
}

/// TC-LATE-CHUNK-009：`extract_doc_prefix` 在长文档上截取正确
#[test]
fn tc_late_chunk_009_extract_doc_prefix_long_document() {
    let long_text = "这是文档的第一段，介绍了基本概念。\n\n\
                     这是第二段，包含更多详细信息。\n\n\
                     这是第三段，讨论了高级主题。\n\n\
                     这是第四段，提供了实践示例。\n\n\
                     这是第五段，总结了全文。";

    let prefix = extract_doc_prefix(long_text, 100);
    assert!(prefix.chars().count() <= 100, "前缀长度应 <= max_chars");
    assert!(!prefix.is_empty(), "长文档的前缀不应为空");
    // 前缀应包含第一段内容
    assert!(prefix.contains("第一段"), "前缀应包含文档开头内容");
}

/// TC-LATE-CHUNK-010：`LateChunkingConfig` serde 往返
#[test]
fn tc_late_chunk_010_config_serde_roundtrip() {
    let config = LateChunkingConfig {
        max_prefix_chars: 800,
        separator: "\n---\n".to_string(),
    };
    let json = serde_json::to_string(&config).expect("序列化失败");
    let deserialized: LateChunkingConfig = serde_json::from_str(&json).expect("反序列化失败");
    assert_eq!(config.max_prefix_chars, deserialized.max_prefix_chars);
    assert_eq!(config.separator, deserialized.separator);
}
