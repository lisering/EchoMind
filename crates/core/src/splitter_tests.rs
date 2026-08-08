#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! QA 红灯测试：TC-ING-007 token 窗口分块器（REQ-VEC-001-AC-2/AC-3 前置验证）。

use crate::Splitter;
use crate::splitter::TextSplitter;

/// TC-ING-007 分块器：长文本按 token 窗口切分，短文本保持单块，空文本返回空。
#[tokio::test]
async fn tc_ing_007_splitter_respects_token_window() {
    let splitter = TextSplitter::with_window(32, 8).unwrap();

    // 短文本：不超过窗口 → 单块且内容不变
    let short = "灵犀，本地知识库。";
    let short_chunks = splitter.split(short).await.unwrap();
    assert_eq!(short_chunks.len(), 1, "短文本必须保持单块");
    assert_eq!(short_chunks[0], short);

    // 长文本：远超窗口 → 多块，且每块 token 数不超上限（含解码容差）
    let long = "灵犀知识库 ".repeat(200);
    let chunks = splitter.split(&long).await.unwrap();
    assert!(chunks.len() > 1, "长文本必须被切分为多个 chunk");
    for (i, chunk) in chunks.iter().enumerate() {
        assert!(!chunk.is_empty(), "chunk {i} 不得为空");
        let count = splitter.count_tokens(chunk).unwrap();
        assert!(count <= 40, "chunk {i} token 数 {count} 超出窗口容差");
    }

    // 空文本 → 空结果（不产生空 chunk）
    let empty = splitter.split("").await.unwrap();
    assert!(empty.is_empty(), "空文本必须返回空 Vec");
}

/// TC-ING-007b 段落感知分块：标题与其正文保持在同一 chunk，不在段落中间切分。
#[tokio::test]
async fn tc_ing_007b_paragraph_aware_chunking() {
    let splitter = TextSplitter::with_window(48, 12).unwrap();

    // 模拟 MarkdownLoader 输出：段落间以 \n\n 分隔
    let md_text = "什么是 Lisp？\n\n\
Lisp 是第二古老的编程语言，诞生于 1958 年。代码和数据都是列表。\n\n\
规则一：括号表示调用\n\n\
规则二：可以嵌套，像套娃一样\n\n\
这是第五段，内容较长用于触发切分。\
这里继续填充更多文本以确保超过 token 窗口。\
再加一些内容让 token 数增长。";

    let chunks = splitter.split(md_text).await.unwrap();
    assert!(chunks.len() > 1, "多段文本必须被切分");

    // 关键断言：标题"什么是 Lisp？"与其正文必须在同一 chunk
    let title_chunk = chunks.iter().find(|c| c.contains("什么是 Lisp"));
    assert!(title_chunk.is_some(), "必须存在包含标题的 chunk");
    let title_chunk = title_chunk.unwrap();
    assert!(
        title_chunk.contains("第二古老的编程语言"),
        "标题与其正文必须在同一 chunk（段落感知）"
    );

    // 不应在段落中间切分（每个 chunk 的边界应是段落边界）
    for chunk in &chunks {
        assert!(!chunk.is_empty(), "chunk 不得为空");
        let count = splitter.count_tokens(chunk).unwrap();
        // 段落感知切分：token 数不超过窗口（+8 容差用于 \n\n 分隔符）
        assert!(count <= 56, "chunk token 数 {count} 超出窗口容差（48+8）");
    }
}

/// TC-ING-007c 无段落分隔的纯文本回退到 token 窗口切分（含重叠）。
#[tokio::test]
async fn tc_ing_007c_no_paragraphs_falls_back_to_token_window() {
    let splitter = TextSplitter::with_window(32, 8).unwrap();

    // 无 \n\n 的连续文本（模拟代码或纯文本）
    let code_text = "let x = 1 + 2 + 3 + 4 + 5; ".repeat(20);
    let chunks = splitter.split(&code_text).await.unwrap();
    assert!(chunks.len() > 1, "连续文本必须被切分");

    for (i, chunk) in chunks.iter().enumerate() {
        assert!(!chunk.is_empty(), "chunk {i} 不得为空");
        let count = splitter.count_tokens(chunk).unwrap();
        // token 窗口切分含重叠，容差为窗口 + 8
        assert!(count <= 40, "chunk {i} token 数 {count} 超出窗口容差");
    }
}
