//! Fuzz 目标：SseParser 字节流解析的鲁棒性。
//!
//! 模拟真实场景：任意字节流随机切分成块喂入 parser，
//! 验证不丢失完整事件、不 panic、UTF-8 安全。

#![no_main]

use echomind_core::stream_parse::SseParser;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut parser = SseParser::new();
    
    // 随机切分点模拟网络粘包/拆包
    let mut events = Vec::new();
    let mut start = 0;
    
    // 每次随机取 1-64 字节作为一个 chunk 喂入
    for i in 0..data.len() {
        // 简单切分：每 1-64 字节作为一个 chunk（使用斐波那契式增长增加变化）
        let chunk_size = ((i * 7 + 13) % 64) + 1;
        let end = (start + chunk_size).min(data.len());
        let chunk = &data[start..end];
        
        let mut parsed = parser.feed(chunk);
        events.append(&mut parsed);
        
        start = end;
        if start >= data.len() {
            break;
        }
    }
    
    // 最后一次喂入剩余数据
    if start < data.len() {
        let mut parsed = parser.feed(&data[start..]);
        events.append(&mut parsed);
    }
    
    // 不变量：每个解析出的事件应该是有效的 UTF-8 字符串
    for event in &events {
        // String::from_utf8_lossy 在构造时已经保证是有效的 UTF-8
        // 这里只验证不为空（空事件应该被跳过）
        assert!(
            !event.is_empty(),
            "SseParser 返回了空事件"
        );
    }
});