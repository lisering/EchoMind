//! Fuzz 目标：wiki_link_parser 对任意 UTF-8 输入的鲁棒性。
//!
//! 不变量（铁律二.5）：
//! 1. 任意输入不得 panic
//! 2. 返回的 WikiLink target 非空

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    // 使用固定的 source_doc_id 和 chunk_id
    let links = echomind_core::wiki_link_parser::parse_wiki_links(&text, "fuzz-doc", "fuzz-chunk");
    
    // 不变量检查：每个链接的 target 应该非空
    for link in &links {
        assert!(
            !link.target.is_empty(),
            "wiki_link_parser 返回了空 target"
        );
    }
});