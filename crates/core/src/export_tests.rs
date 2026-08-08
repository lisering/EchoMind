#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! 对话导出模块测试（REQ-EXP-001）。
//!
//! 测试覆盖 AC-1 ~ AC-6：
//! - AC-2: 导出的 Markdown 文件包含会话标题、创建时间、全部消息
//! - AC-3: 消息按 user / assistant 交替排列，角色标记清晰
//! - AC-4: Assistant 消息末尾附带引用来源列表（文档名 + chunk 序号）
//! - AC-5: 导出文件名默认为会话标题
//! - AC-6: 代码块、Mermaid 语法、LaTeX 公式以原始 Markdown 语法保留

use echomind_models::{ChatMessage, Chunk, Conversation, RetrievalResult};

use crate::export::{export_conversation_to_markdown, sanitize_filename};

/// 创建测试用会话。
fn make_conversation(title: &str) -> Conversation {
    Conversation {
        id: "conv-001".to_string(),
        workspace_id: "default".to_string(),
        title: title.to_string(),
        created_at: 1753872000, // 2025-07-30 12:00:00 UTC
    }
}

/// 创建测试用引用来源。
fn make_source(doc_name: &str, sequence: usize, score: f32) -> RetrievalResult {
    RetrievalResult {
        chunk: Chunk {
            id: "chunk-001".to_string(),
            doc_id: "doc-001".to_string(),
            content: "source content".to_string(),
            token_count: 10,
            sequence,
        },
        score,
        doc_name: doc_name.to_string(),
    }
}

/// TC-EXP-001：导出包含会话标题和创建时间（REQ-EXP-001-AC-2 标题和时间）。
#[test]
fn tc_exp_001_export_contains_title_and_time() {
    let conv = make_conversation("RAG 原理解析");
    let md = export_conversation_to_markdown(&conv, &[]);

    assert!(md.contains("# RAG 原理解析"), "Markdown 必须包含 H1 标题");
    assert!(md.contains("Created at"), "Markdown 必须包含创建时间标签");
    assert!(md.contains("2025"), "Markdown 必须包含年份");
}

/// TC-EXP-002：导出包含全部消息且角色标记清晰（REQ-EXP-001-AC-3）。
#[test]
fn tc_exp_002_export_contains_messages_with_roles() {
    let conv = make_conversation("测试会话");
    let messages = vec![
        ChatMessage {
            id: None,
            role: "user".to_string(),
            content: "什么是向量检索？".to_string(),
            sources: None,
            reasoning: None,
            ..Default::default()
        },
        ChatMessage {
            id: None,
            role: "assistant".to_string(),
            content: "向量检索是一种基于语义相似度的搜索方式。".to_string(),
            sources: None,
            reasoning: None,
            ..Default::default()
        },
        ChatMessage {
            id: None,
            role: "user".to_string(),
            content: "和关键词检索有什么区别？".to_string(),
            sources: None,
            reasoning: None,
            ..Default::default()
        },
        ChatMessage {
            id: None,
            role: "assistant".to_string(),
            content: "关键词检索基于精确匹配，向量检索基于语义相似度。".to_string(),
            sources: None,
            reasoning: None,
            ..Default::default()
        },
    ];

    let md = export_conversation_to_markdown(&conv, &messages);

    // AC-3: 消息按 user / assistant 交替排列，角色标记清晰
    assert!(md.contains("🧑 User"), "必须包含 User 角色标记");
    assert!(md.contains("🤖 Assistant"), "必须包含 Assistant 角色标记");
    assert!(
        md.contains("什么是向量检索？"),
        "必须包含第一条 user 消息内容"
    );
    assert!(
        md.contains("向量检索是一种基于语义相似度的搜索方式"),
        "必须包含第一条 assistant 消息内容"
    );
    assert!(
        md.contains("和关键词检索有什么区别？"),
        "必须包含第二条 user 消息内容"
    );
    assert!(
        md.contains("关键词检索基于精确匹配"),
        "必须包含第二条 assistant 消息内容"
    );

    // 验证消息顺序（user 在 assistant 之前）
    let user1_pos = md.find("什么是向量检索").unwrap();
    let asst1_pos = md.find("向量检索是一种").unwrap();
    let user2_pos = md.find("和关键词检索").unwrap();
    let asst2_pos = md.find("关键词检索基于").unwrap();
    assert!(
        user1_pos < asst1_pos,
        "第一条 user 必须在第一条 assistant 之前"
    );
    assert!(
        asst1_pos < user2_pos,
        "第一条 assistant 必须在第二条 user 之前"
    );
    assert!(
        user2_pos < asst2_pos,
        "第二条 user 必须在第二条 assistant 之前"
    );
}

/// TC-EXP-003：Assistant 消息末尾附带引用来源列表（REQ-EXP-001-AC-4）。
#[test]
fn tc_exp_003_export_contains_citation_sources() {
    let conv = make_conversation("来源测试");
    let sources = vec![
        make_source("论文.md", 3, 0.85),
        make_source("笔记.txt", 7, 0.72),
    ];
    let messages = vec![ChatMessage {
        id: None,
        role: "assistant".to_string(),
        content: "根据知识库文档回答。".to_string(),
        sources: Some(sources),
        reasoning: None,
        ..Default::default()
    }];

    let md = export_conversation_to_markdown(&conv, &messages);

    // AC-4: 引用来源列表包含文档名和 chunk 序号
    assert!(md.contains("Citation Sources"), "必须包含引用来源标题");
    assert!(md.contains("论文.md"), "必须包含来源文档名");
    assert!(md.contains("chunk #3"), "必须包含 chunk 序号");
    assert!(md.contains("笔记.txt"), "必须包含第二个来源文档名");
    assert!(md.contains("chunk #7"), "必须包含第二个 chunk 序号");
}

/// TC-EXP-004：代码块、Mermaid 语法、LaTeX 公式以原始 Markdown 语法保留（REQ-EXP-001-AC-6）。
#[test]
fn tc_exp_004_export_preserves_raw_markdown() {
    let conv = make_conversation("富内容测试");
    let content = r#"这是回答内容。

```python
def hello():
    print("Hello, World!")
```

$$E = mc^2$$

```mermaid
graph TD
    A[Start] --> B[End]
```

> 引用文本
"#;
    let messages = vec![ChatMessage {
        id: None,
        role: "assistant".to_string(),
        content: content.to_string(),
        sources: None,
        reasoning: None,
        ..Default::default()
    }];

    let md = export_conversation_to_markdown(&conv, &messages);

    // AC-6: 代码块、Mermaid 语法、LaTeX 公式以原始 Markdown 语法保留
    assert!(md.contains("```python"), "必须保留 Python 代码块语法");
    assert!(md.contains(r#"def hello():"#), "必须保留代码内容");
    assert!(md.contains("$$E = mc^2$$"), "必须保留 LaTeX 公式语法");
    assert!(md.contains("```mermaid"), "必须保留 Mermaid 语法");
    assert!(md.contains("graph TD"), "必须保留 Mermaid 图表内容");
    assert!(md.contains("> 引用文本"), "必须保留引用块语法");
}

/// TC-EXP-005：空消息列表产生仅含标题和时间的 Markdown（健壮性）。
#[test]
fn tc_exp_005_empty_messages_produces_valid_markdown() {
    let conv = make_conversation("空会话");
    let md = export_conversation_to_markdown(&conv, &[]);

    assert!(md.contains("# 空会话"), "空消息时仍需包含标题");
    assert!(md.contains("Created at"), "空消息时仍需包含创建时间");
    assert!(!md.contains("🧑 User"), "空消息不应包含 User 标记");
    assert!(
        !md.contains("🤖 Assistant"),
        "空消息不应包含 Assistant 标记"
    );
}

/// TC-EXP-006：User 消息不附带引用来源（仅 assistant 有来源）。
#[test]
fn tc_exp_006_user_messages_have_no_sources() {
    let conv = make_conversation("来源区分测试");
    let messages = vec![
        ChatMessage {
            id: None,
            role: "user".to_string(),
            content: "用户问题".to_string(),
            sources: Some(vec![make_source("不该出现.md", 1, 0.9)]),
            reasoning: None,
            ..Default::default()
        },
        ChatMessage {
            id: None,
            role: "assistant".to_string(),
            content: "助手回答".to_string(),
            sources: Some(vec![make_source("应该出现.md", 2, 0.8)]),
            reasoning: None,
            ..Default::default()
        },
    ];

    let md = export_conversation_to_markdown(&conv, &messages);

    // User 消息的 sources 不应出现在导出中
    assert!(!md.contains("不该出现.md"), "User 消息不应附带引用来源");
    // Assistant 消息的 sources 应出现
    assert!(md.contains("应该出现.md"), "Assistant 消息应附带引用来源");
}

/// TC-EXP-007：sanitize_filename 正确处理不安全字符（REQ-EXP-001-AC-5 文件名安全）。
#[test]
fn tc_exp_007_sanitize_filename_replaces_unsafe_chars() {
    assert_eq!(sanitize_filename("正常标题"), "正常标题");
    assert_eq!(sanitize_filename("a/b\\c:d*e?f"), "a_b_c_d_e_f");
    assert_eq!(sanitize_filename("标题|<>\"?"), "标题_____");
    assert_eq!(sanitize_filename(""), "conversation");
    assert_eq!(sanitize_filename("   "), "conversation");
    assert_eq!(sanitize_filename("..."), "conversation");
    assert_eq!(sanitize_filename("  标题  "), "标题");
}

/// TC-EXP-008：无 sources 的 assistant 消息不输出引用来源区域。
#[test]
fn tc_exp_008_assistant_without_sources_has_no_citation_section() {
    let conv = make_conversation("无来源测试");
    let messages = vec![ChatMessage {
        id: None,
        role: "assistant".to_string(),
        content: "这个回答没有引用来源。".to_string(),
        sources: None,
        reasoning: None,
        ..Default::default()
    }];

    let md = export_conversation_to_markdown(&conv, &messages);

    assert!(
        !md.contains("Citation Sources"),
        "无 sources 时不应输出引用来源区域"
    );
    assert!(md.contains("这个回答没有引用来源"), "消息内容仍需保留");
}

/// TC-EXP-009：system 消息不出现在导出中（内部使用，不导出）。
#[test]
fn tc_exp_009_system_messages_excluded() {
    let conv = make_conversation("系统消息测试");
    let messages = vec![
        ChatMessage {
            id: None,
            role: "system".to_string(),
            content: "你是一个知识库助手。".to_string(),
            sources: None,
            reasoning: None,
            ..Default::default()
        },
        ChatMessage {
            id: None,
            role: "user".to_string(),
            content: "用户问题".to_string(),
            sources: None,
            reasoning: None,
            ..Default::default()
        },
    ];

    let md = export_conversation_to_markdown(&conv, &messages);

    assert!(
        !md.contains("你是一个知识库助手"),
        "System 消息不应出现在导出中"
    );
    assert!(md.contains("用户问题"), "User 消息应正常出现");
}

/// TC-EXP-010：导出文件包含 EchoMind 签名（来源标识）。
#[test]
fn tc_exp_010_export_contains_signature() {
    let conv = make_conversation("签名测试");
    let md = export_conversation_to_markdown(&conv, &[]);

    assert!(
        md.contains("Exported by EchoMind"),
        "导出文件应包含 EchoMind 签名"
    );
}
