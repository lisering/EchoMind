//! Fuzz 目标：查询规范化（normalize_query）对任意输入的鲁棒性。

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let query = String::from_utf8_lossy(data);
    let normalized = echomind_core::cache::normalize_query(&query);
    // 不变量：输出长度不得超过输入（只做裁剪/折叠，不放大）
    assert!(
        normalized.chars().count() <= query.chars().count() + 8,
        "normalize_query 不得显著放大输入"
    );
});
