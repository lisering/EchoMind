//! License 签发 CLI 工具核心库（REQ-LIC-005）。
//!
//! 与客户端 `crates/core/src/license.rs` 的 `verify_license` 形成闭环：
//! - `generate_keypair()` → 生成 Ed25519 密钥对（32 字节公钥 + 32 字节私钥 seed）
//! - `issue_license(payload, privkey_seed)` → 签发 `base64(payload)-base64(signature)` 格式 License Key
//!
//! 安全设计：
//! - 私钥文件权限 0600（Unix `#[cfg(unix)]`）
//! - 私钥仅存在于签发环境，不编入应用二进制
//! - 零网络依赖

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use ed25519_dalek::Signer;
use ed25519_dalek::{SigningKey, VerifyingKey};

/// 生成 Ed25519 密钥对（REQ-LIC-005-AC-1）。
///
/// 返回 `(pubkey_bytes, privkey_seed_bytes)`：
/// - `pubkey_bytes`：32 字节公钥（用于替换客户端 `BUILTIN_PUBKEY`）
/// - `privkey_seed_bytes`：32 字节私钥 seed（用于签发 License Key）
///
/// 使用 `OsRng` 密码学安全随机数生成器。
pub fn generate_keypair() -> (Vec<u8>, Vec<u8>) {
    let mut rng = rand::rngs::OsRng;
    let signing_key = SigningKey::generate(&mut rng);
    let verifying_key = signing_key.verifying_key();
    (
        verifying_key.to_bytes().to_vec(),
        signing_key.to_bytes().to_vec(),
    )
}

/// 用私钥签发 License Key（REQ-LIC-005-AC-2）。
///
/// 输出格式：`base64(payload)-base64(signature)`，与客户端 `license.rs::parse_license` 兼容。
///
/// # 参数
/// - `payload`：签名的原始文本（如 `EchoMind-Pro-v1`）
/// - `privkey_seed`：32 字节私钥 seed（由 `generate_keypair()` 生成）
///
/// # 错误
/// - 私钥 seed 长度不为 32 字节时返回 `Err`（AC-4）
/// - 私钥 seed 字节无法构造有效 `SigningKey` 时返回 `Err`（AC-4）
pub fn issue_license(payload: &str, privkey_seed: &[u8]) -> Result<String> {
    if privkey_seed.len() != 32 {
        bail!(
            "私钥 seed 长度非法（期望 32 字节，实际 {}）",
            privkey_seed.len()
        );
    }
    let mut seed = [0u8; 32];
    seed.copy_from_slice(privkey_seed);
    let signing_key = SigningKey::from_bytes(&seed);
    let signature = signing_key.sign(payload.as_bytes());
    let payload_b64 = B64.encode(payload.as_bytes());
    let sig_b64 = B64.encode(signature.to_bytes());
    Ok(format!("{payload_b64}-{sig_b64}"))
}

/// 保存密钥对到指定目录（REQ-LIC-005-AC-1）。
///
/// - 公钥写入 `<output_dir>/license_pubkey.b64`（base64 编码）
/// - 私钥写入 `<output_dir>/license_privkey.b64`（base64 编码，权限 0600）
///
/// 返回 `(pubkey_path, privkey_path)`。
pub fn save_keypair_to_dir(
    pubkey: &[u8],
    privkey: &[u8],
    output_dir: &Path,
) -> Result<(PathBuf, PathBuf)> {
    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("创建输出目录失败: {}", output_dir.display()))?;

    let pubkey_path = output_dir.join("license_pubkey.b64");
    let privkey_path = output_dir.join("license_privkey.b64");

    // 写入公钥
    let pubkey_b64 = B64.encode(pubkey);
    std::fs::write(&pubkey_path, &pubkey_b64)
        .with_context(|| format!("写入公钥文件失败: {}", pubkey_path.display()))?;

    // 写入私钥（权限 0600）
    let privkey_b64 = B64.encode(privkey);
    write_secret_file(&privkey_path, &privkey_b64)?;

    Ok((pubkey_path, privkey_path))
}

/// 从文件加载私钥 seed（REQ-LIC-005-AC-4）。
///
/// 读取 base64 编码的私钥文件，解码为 32 字节 seed。
///
/// # 错误
/// - 文件不存在时返回 `Err`（AC-4）
/// - base64 解码失败时返回 `Err`（AC-4）
/// - 解码后长度不为 32 字节时返回 `Err`（AC-4）
pub fn load_privkey(path: &Path) -> Result<Vec<u8>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("读取私钥文件失败: {}", path.display()))?;
    let trimmed = content.trim();
    let seed = B64.decode(trimmed).context("私钥 base64 解码失败")?;
    if seed.len() != 32 {
        bail!("私钥 seed 长度非法（期望 32 字节，实际 {}）", seed.len());
    }
    Ok(seed)
}

/// 从 base64 字符串加载公钥（用于验证签发的 License Key）。
///
/// # 错误
/// - base64 解码失败时返回 `Err`
/// - 解码后长度不为 32 字节时返回 `Err`
pub fn load_pubkey_from_b64(b64_str: &str) -> Result<VerifyingKey> {
    let bytes = B64.decode(b64_str.trim()).context("公钥 base64 解码失败")?;
    if bytes.len() != 32 {
        bail!("公钥长度非法（期望 32 字节，实际 {}）", bytes.len());
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    VerifyingKey::from_bytes(&arr).map_err(|e| anyhow::anyhow!("公钥字节无效: {e}"))
}

/// 写入机密文件并设置权限 0600（Unix）。
fn write_secret_file(path: &Path, content: &str) -> Result<()> {
    std::fs::write(path, content)
        .with_context(|| format!("写入私钥文件失败: {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(path, perms)
            .with_context(|| format!("设置私钥文件权限失败: {}", path.display()))?;
    }
    Ok(())
}

// ================== 单元测试 ==================

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use ed25519_dalek::{Verifier, VerifyingKey};
    use tempfile::TempDir;

    /// TC-LIC-007：`generate_keypair` 输出 32 字节公钥和 32 字节私钥 seed。
    #[test]
    fn tc_lic_007_generate_keypair_output_format() {
        let (pubkey, privkey) = generate_keypair();

        // AC-1：公钥 32 字节
        assert_eq!(pubkey.len(), 32, "公钥应为 32 字节，实际 {}", pubkey.len());
        // AC-1：私钥 seed 32 字节
        assert_eq!(
            privkey.len(),
            32,
            "私钥 seed 应为 32 字节，实际 {}",
            privkey.len()
        );
    }

    /// TC-LIC-007b：保存密钥对到目录，私钥文件权限 0600（AC-1）。
    #[test]
    fn tc_lic_007b_save_keypair_file_permissions() {
        let (pubkey, privkey) = generate_keypair();
        let tmpdir = TempDir::new().expect("创建临时目录失败");

        let (pubkey_path, privkey_path) =
            save_keypair_to_dir(&pubkey, &privkey, tmpdir.path()).expect("保存密钥对失败");

        // 公钥文件存在
        assert!(pubkey_path.exists(), "公钥文件应存在");
        // 私钥文件存在
        assert!(privkey_path.exists(), "私钥文件应存在");

        // AC-1：私钥文件权限 0600（仅 Unix）
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = std::fs::metadata(&privkey_path).expect("读取私钥文件元数据失败");
            let mode = metadata.permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "私钥文件权限应为 0600，实际 {mode:04o}");
        }

        // 公钥文件内容是 base64 编码的 32 字节
        let pubkey_content = std::fs::read_to_string(&pubkey_path).expect("读取公钥文件失败");
        let pubkey_decoded = B64
            .decode(pubkey_content.trim())
            .expect("公钥 base64 解码失败");
        assert_eq!(pubkey_decoded.len(), 32);

        // 私钥文件内容是 base64 编码的 32 字节
        let privkey_content = std::fs::read_to_string(&privkey_path).expect("读取私钥文件失败");
        let privkey_decoded = B64
            .decode(privkey_content.trim())
            .expect("私钥 base64 解码失败");
        assert_eq!(privkey_decoded.len(), 32);
    }

    /// TC-LIC-007c：私钥文件不存在时返回明确错误，不 Panic（AC-4）。
    #[test]
    fn tc_lic_007c_load_privkey_missing_file() {
        let result = load_privkey(Path::new("/nonexistent/path/privkey.b64"));
        assert!(
            result.is_err(),
            "加载不存在的私钥文件应返回 Err，不应 Panic"
        );
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("读取私钥文件失败"),
            "错误消息应包含可读原因: {err_msg}"
        );
    }

    /// TC-LIC-007d：私钥文件格式非法时返回明确错误，不 Panic（AC-4）。
    #[test]
    fn tc_lic_007d_load_privkey_invalid_format() {
        let tmpdir = TempDir::new().expect("创建临时目录失败");
        let bad_file = tmpdir.path().join("bad_privkey.b64");
        // 写入非法 base64
        std::fs::write(&bad_file, "not-valid-base64!!!").expect("写入测试文件失败");

        let result = load_privkey(&bad_file);
        assert!(
            result.is_err(),
            "加载非法格式的私钥文件应返回 Err，不应 Panic"
        );
    }

    /// TC-LIC-007e：私钥 seed 长度不为 32 字节时返回明确错误（AC-4）。
    #[test]
    fn tc_lic_007e_issue_license_wrong_seed_length() {
        let short_seed = vec![0u8; 16]; // 错误长度
        let result = issue_license("EchoMind-Pro-v1", &short_seed);
        assert!(result.is_err(), "错误长度的私钥 seed 应返回 Err");
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("长度非法"),
            "错误消息应包含长度提示: {err_msg}"
        );
    }

    /// TC-LIC-008：`issue_license` 签发 License Key，客户端验证通过（AC-2）。
    #[test]
    fn tc_lic_008_issue_and_verify_roundtrip() {
        let (pubkey, privkey) = generate_keypair();
        let payload = "EchoMind-Pro-v1";

        // 签发 License Key
        let license_key = issue_license(payload, &privkey).expect("签发 License Key 失败");

        // 解析 License Key 格式：base64(payload)-base64(signature)
        let parts: Vec<&str> = license_key.splitn(2, '-').collect();
        assert_eq!(parts.len(), 2, "License Key 应含 payload-signature 两段");

        // 解码 payload，确认与原始 payload 一致
        let decoded_payload = B64.decode(parts[0]).expect("payload base64 解码失败");
        assert_eq!(
            decoded_payload,
            payload.as_bytes(),
            "解码后的 payload 应与原始一致"
        );

        // 用公钥验证签名（模拟客户端验证）
        let mut pubkey_arr = [0u8; 32];
        pubkey_arr.copy_from_slice(&pubkey);
        let verifying_key = VerifyingKey::from_bytes(&pubkey_arr).expect("构造公钥失败");

        let sig_bytes = B64.decode(parts[1]).expect("signature base64 解码失败");
        assert_eq!(sig_bytes.len(), 64, "签名应为 64 字节");

        let signature = ed25519_dalek::Signature::from_slice(&sig_bytes).expect("构造签名失败");
        let verify_result = verifying_key.verify(&decoded_payload, &signature);
        assert!(verify_result.is_ok(), "客户端公钥验证签名应通过（AC-2）");
    }

    /// TC-LIC-008b：篡改 payload 后签名验证失败（AC-3）。
    #[test]
    fn tc_lic_008b_tampered_payload_fails_verification() {
        let (pubkey, privkey) = generate_keypair();
        let payload = "EchoMind-Pro-v1";

        let license_key = issue_license(payload, &privkey).expect("签发 License Key 失败");

        // 篡改 payload：替换为不同的 payload
        let tampered_payload = "EchoMind-Pro-v2";
        let tampered_payload_b64 = B64.encode(tampered_payload.as_bytes());

        let parts: Vec<&str> = license_key.splitn(2, '-').collect();
        let tampered_license = format!("{tampered_payload_b64}-{}", parts[1]);

        // 用公钥验证篡改后的 License Key
        let mut pubkey_arr = [0u8; 32];
        pubkey_arr.copy_from_slice(&pubkey);
        let verifying_key = VerifyingKey::from_bytes(&pubkey_arr).expect("构造公钥失败");

        let tampered_parts: Vec<&str> = tampered_license.splitn(2, '-').collect();
        let tampered_decoded = B64
            .decode(tampered_parts[0])
            .expect("payload base64 解码失败");
        let sig_bytes = B64
            .decode(tampered_parts[1])
            .expect("signature base64 解码失败");
        let signature = ed25519_dalek::Signature::from_slice(&sig_bytes).expect("构造签名失败");

        let verify_result = verifying_key.verify(&tampered_decoded, &signature);
        assert!(
            verify_result.is_err(),
            "篡改 payload 后签名验证应失败（AC-3）"
        );
    }

    /// TC-LIC-008c：不同 payload 生成不同 License Key（AC-5）。
    #[test]
    fn tc_lic_008c_different_payload_different_license() {
        let (_pubkey, privkey) = generate_keypair();

        let license_v1 = issue_license("EchoMind-Pro-v1", &privkey).expect("签发 v1 失败");
        let license_v2 = issue_license("EchoMind-Pro-v2", &privkey).expect("签发 v2 失败");

        assert_ne!(
            license_v1, license_v2,
            "不同 payload 应生成不同 License Key（AC-5）"
        );

        // 两个 License Key 的 payload 部分不同
        let v1_parts: Vec<&str> = license_v1.splitn(2, '-').collect();
        let v2_parts: Vec<&str> = license_v2.splitn(2, '-').collect();
        assert_ne!(v1_parts[0], v2_parts[0], "payload 部分应不同");
    }

    /// TC-LIC-008d：加载私钥文件后签发 License Key，端到端验证（AC-2 + AC-4 闭环）。
    #[test]
    fn tc_lic_008d_load_privkey_and_issue_e2e() {
        let (pubkey, privkey) = generate_keypair();
        let tmpdir = TempDir::new().expect("创建临时目录失败");

        // 保存密钥对
        let (_pubkey_path, privkey_path) =
            save_keypair_to_dir(&pubkey, &privkey, tmpdir.path()).expect("保存密钥对失败");

        // 从文件加载私钥
        let loaded_seed = load_privkey(&privkey_path).expect("加载私钥失败");

        // 用加载的私钥签发 License Key
        let license_key =
            issue_license("EchoMind-Pro-v1", &loaded_seed).expect("签发 License Key 失败");

        // 用原始公钥验证（模拟客户端验证）
        let mut pubkey_arr = [0u8; 32];
        pubkey_arr.copy_from_slice(&pubkey);
        let verifying_key = VerifyingKey::from_bytes(&pubkey_arr).expect("构造公钥失败");

        let parts: Vec<&str> = license_key.splitn(2, '-').collect();
        let payload_bytes = B64.decode(parts[0]).expect("payload base64 解码失败");
        let sig_bytes = B64.decode(parts[1]).expect("signature base64 解码失败");
        let signature = ed25519_dalek::Signature::from_slice(&sig_bytes).expect("构造签名失败");

        assert!(
            verifying_key.verify(&payload_bytes, &signature).is_ok(),
            "从文件加载私钥签发的 License Key 应验证通过"
        );
    }

    /// TC-LIC-008e：`load_pubkey_from_b64` 正确解析 base64 公钥。
    #[test]
    fn tc_lic_008e_load_pubkey_from_b64() {
        let (pubkey, _privkey) = generate_keypair();
        let pubkey_b64 = B64.encode(&pubkey);

        let result = load_pubkey_from_b64(&pubkey_b64);
        assert!(result.is_ok(), "base64 公钥应正确解析");

        // 非 base64 字符串应失败
        let bad_result = load_pubkey_from_b64("not-valid-base64!!!");
        assert!(bad_result.is_err(), "非法 base64 公钥应返回 Err");

        // 长度不正确应失败
        let short_b64 = B64.encode([0u8; 16]);
        let short_result = load_pubkey_from_b64(&short_b64);
        assert!(short_result.is_err(), "长度不正确的公钥应返回 Err");
    }
}
