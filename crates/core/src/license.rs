//! License 离线校验（REQ-LIC-001）：Ed25519 签名验证。
//! 纯计算模块，无 I/O 依赖，经架构评审准入 core。
//!
//! License Key 格式：`base64(payload)-base64(signature)`
//! - payload：签名的原始字节（如 `EchoMind-Pro-v1`）
//! - signature：64 字节 Ed25519 签名
//!
//! 安全官审查项：
//! - 错误消息不泄露校验内部细节（如公钥字节、签名比对结果）
//! - 内置公钥仅用于验证，私钥不在二进制中

use anyhow::{Context, Result, anyhow, bail};
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};

/// 内置 Ed25519 公钥（开发环境密钥；正式发布前替换为 License 服务器公钥）。
/// 对应私钥仅存在于 License 签发服务端，不在本仓库中。
const BUILTIN_PUBKEY: [u8; 32] = [
    0xD5, 0x27, 0xD4, 0xFE, 0xB9, 0xA4, 0x9A, 0xCC, 0x98, 0x04, 0xA2, 0x2A, 0x7B, 0xA6, 0x57, 0x29,
    0xBF, 0xD8, 0xE9, 0x75, 0xD4, 0x15, 0x2A, 0x39, 0x29, 0x88, 0xA5, 0xCB, 0x59, 0xC0, 0x2E, 0x5C,
];

/// 使用内置公钥验证 License Key（生产路径）。
///
/// 返回 `Ok(())` 表示签名有效；`Err` 携带可读原因（不含校验内部细节）。
pub fn verify_license(license_key: &str) -> Result<()> {
    let pubkey = VerifyingKey::from_bytes(&BUILTIN_PUBKEY)
        .map_err(|_| anyhow!("内置公钥无效，请联系开发者"))?;
    verify_license_with_key(license_key, &pubkey)
}

/// 使用指定公钥验证 License Key（测试路径：注入测试密钥对）。
pub fn verify_license_with_key(license_key: &str, pubkey: &VerifyingKey) -> Result<()> {
    let (payload, sig_bytes) = parse_license(license_key)?;
    let signature = Signature::from_slice(&sig_bytes).context("签名格式无效")?;
    pubkey
        .verify(&payload, &signature)
        .map_err(|_| anyhow!("License 签名验证失败：签名不匹配或已被篡改"))?;
    Ok(())
}

/// 解析 License Key：`base64(payload)-base64(signature)` → (payload, signature)。
fn parse_license(license_key: &str) -> Result<(Vec<u8>, Vec<u8>)> {
    let trimmed = license_key.trim();
    let parts: Vec<&str> = trimmed.splitn(2, '-').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        bail!("License 格式错误：应为 payload-signature");
    }
    let payload = B64.decode(parts[0]).context("License payload 解码失败")?;
    let signature = B64.decode(parts[1]).context("License 签名解码失败")?;
    if signature.len() != 64 {
        bail!("签名长度非法（期望 64 字节，实际 {}）", signature.len());
    }
    Ok((payload, signature))
}
