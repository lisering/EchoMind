#![allow(clippy::unwrap_used)]
//! QA 红灯测试：TC-LLM-001 SSE 粘包/拆包解析（REQ-LLM-001-AC-1）。

use crate::stream_parse::{SseParser, StreamItem, parse_openai_payload};

fn feed_and_collect(parser: &mut SseParser, chunks: &[&[u8]]) -> Vec<StreamItem> {
    chunks
        .iter()
        .flat_map(|c| parser.feed(c))
        .filter_map(|p| parse_openai_payload(&p))
        .collect()
}

/// TC-LLM-001：完整事件被 `\n\n` 切断 + 下一事件残头混入时，必须缓冲并正确拼接，不崩溃不丢字。
#[test]
fn tc_llm_001_sse_sticky_packet_buffering() {
    let mut parser = SseParser::new();

    // 第一次投喂：一个完整事件 + 第二个事件的残头 "da"
    let part1 = b"data: {\"choices\":[{\"delta\":{\"content\":\"hel\"}}]}\n\nda";
    let items1 = feed_and_collect(&mut parser, &[part1]);
    assert_eq!(items1.len(), 1, "第一个完整事件必须立即产出");
    assert!(matches!(&items1[0], StreamItem::Token(t) if t == "hel"));

    // 第二次投喂：补全第二事件
    let part2 = b"ta: {\"choices\":[{\"delta\":{\"content\":\"lo\"}}]}\n\n";
    let items2 = feed_and_collect(&mut parser, &[part2]);
    assert_eq!(items2.len(), 1, "缓冲残片拼接后必须产出第二事件");
    assert!(matches!(&items2[0], StreamItem::Token(t) if t == "lo"));

    // 结束标记
    let part3 = b"data: [DONE]\n\n";
    let items3 = feed_and_collect(&mut parser, &[part3]);
    assert!(
        items3.iter().any(|i| matches!(i, StreamItem::Done)),
        "[DONE] 必须被识别"
    );
}

/// TC-LLM-001 补充：事件在多字节 UTF-8 字符边界被切断，解码不得损坏字符。
#[test]
fn tc_llm_001_sse_split_inside_utf8_char() {
    let mut parser = SseParser::new();
    let full = "data: {\"choices\":[{\"delta\":{\"content\":\"你好\"}}]}\n\n";
    let bytes = full.as_bytes();
    // 在「你」的 3 字节内部切断（第 1 字节之后）
    let cut = bytes.windows(3).position(|w| w == "你".as_bytes()).unwrap() + 1;

    let items = feed_and_collect(&mut parser, &[&bytes[..cut], &bytes[cut..]]);

    assert_eq!(items.len(), 1, "切断后拼接必须恰好产出一个事件");
    assert!(
        matches!(&items[0], StreamItem::Token(t) if t == "你好"),
        "多字节字符不得丢失或损坏"
    );
}

/// TC-LLM-001 补充：注释行与无 content 的 delta 不得产出 token。
#[test]
fn tc_llm_001_ignores_non_token_payloads() {
    let mut parser = SseParser::new();
    let chunk = b": comment\n\ndata: {\"choices\":[{\"delta\":{\"role\":\"assistant\"}}]}\n\n";
    let items = feed_and_collect(&mut parser, &[chunk]);
    assert!(items.is_empty(), "注释与无 content 的 delta 必须被忽略");
}
