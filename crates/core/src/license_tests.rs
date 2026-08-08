#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! QA 红灯测试（体系二阶段 2：先于实现编写，初始必然失败）。
//! 覆盖：TC-LIC-001 合法签名验证 / TC-LIC-002 篡改签名拒绝 / TC-LIC-002b 格式异常拒绝。
//! REQ-LIC-001：Ed25519 License 离线校验。

use base64::{Engine, engine::general_purpose::STANDARD as B64};
use ed25519_dalek::{Signer, SigningKey};

use crate::license::{verify_license, verify_license_with_key};

/// 开发测试种子（与 BUILTIN_PUBKEY 对应；仅存在于测试代码，不在生产二进制中）。
const DEV_SEED: [u8; 32] = [
    0xF1, 0xF7, 0x2D, 0x13, 0x51, 0xE4, 0x8F, 0x18, 0xB7, 0xE1, 0xAA, 0x07, 0x95, 0x27, 0x12, 0x12,
    0x04, 0x72, 0xCF, 0xC5, 0xC3, 0x65, 0xAB, 0x70, 0xED, 0xA3, 0xC6, 0x5A, 0xC3, 0x29, 0xBC, 0x5C,
];

/// License payload（签名内容）。
const PAYLOAD: &[u8] = b"EchoMind-Pro-v1";

/// 生成测试用签名密钥与对应验证密钥。
fn test_keypair() -> (SigningKey, ed25519_dalek::VerifyingKey) {
    let signing = SigningKey::from_bytes(&DEV_SEED);
    let verifying = signing.verifying_key();
    (signing, verifying)
}

/// 构造合法 License Key 字符串：`base64(payload)-base64(signature)`。
fn make_license(signing_key: &SigningKey, payload: &[u8]) -> String {
    let signature = signing_key.sign(payload);
    format!(
        "{}-{}",
        B64.encode(payload),
        B64.encode(signature.to_bytes())
    )
}

/// TC-LIC-001：合法签名 License 验证通过（REQ-LIC-001-AC-1）。
#[test]
fn tc_lic_001_valid_license_verified() {
    let (signing, verifying) = test_keypair();
    let license = make_license(&signing, PAYLOAD);

    // 使用测试公钥验证
    let result = verify_license_with_key(&license, &verifying);
    assert!(
        result.is_ok(),
        "合法 License 必须验证通过: {:?}",
        result.err()
    );

    // 使用内置公钥验证（DEV_SEED 对应 BUILTIN_PUBKEY）
    let result_builtin = verify_license(&license);
    assert!(
        result_builtin.is_ok(),
        "内置公钥必须能验证开发密钥签发的 License: {:?}",
        result_builtin.err()
    );
}

/// TC-LIC-002：篡改签名后验证拒绝（REQ-LIC-001-AC-2）。
#[test]
fn tc_lic_002_tampered_signature_rejected() {
    let (signing, verifying) = test_keypair();
    let signature = signing.sign(PAYLOAD);
    let mut sig_bytes = signature.to_bytes();
    // 翻转首字节，篡改签名
    sig_bytes[0] ^= 0xFF;
    let license = format!("{}-{}", B64.encode(PAYLOAD), B64.encode(sig_bytes));

    let result = verify_license_with_key(&license, &verifying);
    assert!(result.is_err(), "篡改签名后必须验证失败");

    // 错误消息不得泄露校验细节（安全官要求）
    let err_msg = result.unwrap_err().to_string();
    assert!(
        !err_msg.contains("pubkey") && !err_msg.contains("verify"),
        "错误消息不得泄露校验内部细节: {err_msg}"
    );
}

/// TC-LIC-002b：格式异常的 License 被拒绝（无分隔符 / 空段 / 签名长度非法）。
#[test]
fn tc_lic_002b_malformed_license_rejected() {
    let (_, verifying) = test_keypair();

    // 无分隔符
    let result = verify_license_with_key("justastring", &verifying);
    assert!(result.is_err(), "无分隔符的 License 必须被拒绝");

    // 空签名段
    let result = verify_license_with_key("dGVzdA==-", &verifying);
    assert!(result.is_err(), "空签名段必须被拒绝");

    // 签名长度非法（非 64 字节）
    let short_sig = B64.encode(b"too_short");
    let result =
        verify_license_with_key(&format!("{}-{short_sig}", B64.encode(PAYLOAD)), &verifying);
    assert!(result.is_err(), "签名长度非法必须被拒绝");
}

/// TC-LIC-002c：篡改 payload（签名不匹配）被拒绝。
#[test]
fn tc_lic_002c_tampered_payload_rejected() {
    let (signing, verifying) = test_keypair();
    // 对 PAYLOAD 签名，但 License 中放入不同的 payload
    let signature = signing.sign(PAYLOAD);
    let license = format!(
        "{}-{}",
        B64.encode(b"different-payload"),
        B64.encode(signature.to_bytes())
    );

    let result = verify_license_with_key(&license, &verifying);
    assert!(result.is_err(), "payload 与签名不匹配必须被拒绝");
}
