//! Fuzz 目标：SemanticSplitter 对任意 UTF-8 文本的分块鲁棒性。
//!
//! 不变量（铁律二.5）：
//! 1. 任意输入不得 panic
//! 2. 返回的块非空、无空内容
//!
//! 注：`Splitter::split` 是 async + spawn_blocking，fuzz harness 用
//! 最小单线程 tokio runtime 驱动。

#![no_main]

use echomind_core::Splitter;
use libfuzzer_sys::fuzz_target;
use std::sync::OnceLock;

static SPLITTER: OnceLock<echomind_core::semantic_splitter::SemanticSplitter> = OnceLock::new();

fn splitter() -> &'static echomind_core::semantic_splitter::SemanticSplitter {
    SPLITTER.get_or_init(|| {
        // 小目标块（64/8）以最大化边界路径覆盖
        echomind_core::semantic_splitter::SemanticSplitter::new(64, 8)
            .expect("splitter init")
    })
}

fn rt() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap())
}

fuzz_target!(|data: &[u8]| {
    // 接受任意字节：合法 UTF-8 直接测；非法 UTF-8 经 lossy 转换后仍应安全
    // （loader 层对二进制文件正是 from_utf8_lossy 处理——铁律五.5）
    let text = String::from_utf8_lossy(data);
    let splitter = splitter();
    let chunks = match rt().block_on(splitter.split(&text)) {
        Ok(c) => c,
        Err(_) => return, // 可恢复错误属合法行为
    };

    for chunk in &chunks {
        assert!(
            !chunk.trim().is_empty(),
            "分出的块不得为纯空白：{:?}",
            chunk.chars().take(32).collect::<String>()
        );
    }
});
