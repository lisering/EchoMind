#![allow(clippy::unwrap_used)]
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
