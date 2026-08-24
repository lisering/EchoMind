#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! 共享辅助函数 — 供所有 integration 子模块使用。

use base64::{Engine, engine::general_purpose::STANDARD as B64};
use echomind_models::LlmConfig;
use ed25519_dalek::Signer;

/// 测试用 LLM 配置（DeepSeek API 模拟值）。
pub fn test_config() -> LlmConfig {
    LlmConfig {
        api_key: "sk-test-1234567890abcdef".to_string(),
        base_url: "https://api.deepseek.com".to_string(),
        model: "deepseek-chat".to_string(),
    }
}

/// 开发测试种子（与 BUILTIN_PUBKEY 对应；仅存在于测试代码）。
pub const DEV_SEED: [u8; 32] = [
    0xF1, 0xF7, 0x2D, 0x13, 0x51, 0xE4, 0x8F, 0x18, 0xB7, 0xE1, 0xAA, 0x07, 0x95, 0x27, 0x12, 0x12,
    0x04, 0x72, 0xCF, 0xC5, 0xC3, 0x65, 0xAB, 0x70, 0xED, 0xA3, 0xC6, 0x5A, 0xC3, 0x29, 0xBC, 0x5C,
];

/// 生成合法 License Key（使用开发种子签名）。
pub fn make_valid_license() -> String {
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&DEV_SEED);
    let payload = b"EchoMind-Pro-v1";
    let signature = signing_key.sign(payload);
    format!(
        "{}-{}",
        B64.encode(payload),
        B64.encode(signature.to_bytes())
    )
}

/// 创建测试用的假 ONNX 文件（非空、非 HTML 的二进制数据）。
#[cfg(feature = "pro")]
pub fn create_fake_onnx_file(dir: &tempfile::TempDir, name: &str) -> std::path::PathBuf {
    let path = dir.path().join(name);
    std::fs::write(&path, b"\x08\x01\x12\x04test\x1a\x03for").unwrap();
    path
}

/// 创建测试用的假 tokenizer 文件（有效 JSON）。
#[cfg(feature = "pro")]
pub fn create_fake_tokenizer_files(dir: &tempfile::TempDir) -> Vec<std::path::PathBuf> {
    let files = [
        "tokenizer.json",
        "config.json",
        "special_tokens_map.json",
        "tokenizer_config.json",
    ];
    files
        .iter()
        .map(|name| {
            let path = dir.path().join(name);
            std::fs::write(&path, r#"{"test": true}"#).unwrap();
            path
        })
        .collect()
}
