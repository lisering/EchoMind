//! AES-256-GCM 加密辅助函数（S01 拆分自 `sqlite_storage.rs`）。
//!
//! 负责：
//! - `load_or_create_cipher`：加载或生成 AES-256-GCM 密钥（文件权限 0600）
//! - `encrypt` / `decrypt`：设置项加密/解密（base64(nonce‖ciphertext) 格式）
//! - `ensure_dir_0700` / `ensure_file_0600`：Unix 文件权限设置（REQ-SEC-004）

use std::path::Path;

use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use anyhow::{Context, anyhow};
use base64::{Engine, engine::general_purpose::STANDARD as B64};

use super::schema::{NONCE_LEN, SECRET_KEY_FILE};

/// 加载或生成 AES-256-GCM 密钥（REQ-UI-008；文件权限 0600）。
///
/// 密钥文件路径为 `data_dir / secret.key`，32 字节随机密钥。
/// 首次调用时生成密钥并写入文件（权限 0600），后续调用时从文件加载。
pub(crate) fn load_or_create_cipher(data_dir: &Path) -> anyhow::Result<Aes256Gcm> {
    let key_path = data_dir.join(SECRET_KEY_FILE);
    let key_bytes: [u8; 32] = if key_path.exists() {
        let raw = std::fs::read(&key_path)
            .with_context(|| format!("读取密钥文件失败: {}", key_path.display()))?;
        raw.try_into()
            .map_err(|_| anyhow!("密钥文件损坏（长度非法）: {}", key_path.display()))?
    } else {
        let generated: [u8; 32] = rand::random();
        std::fs::write(&key_path, generated)
            .with_context(|| format!("写入密钥文件失败: {}", key_path.display()))?;
        // REQ-SEC-004：密钥文件权限 0600（仅所有者可读写）
        ensure_file_0600(&key_path)?;
        generated
    };
    Ok(Aes256Gcm::new(aes_gcm::Key::<Aes256Gcm>::from_slice(
        &key_bytes,
    )))
}

/// AES-256-GCM 加密 → base64(nonce‖ciphertext)。
///
/// 每次加密生成随机 nonce（96 bit），密文格式为 `base64(nonce ‖ ciphertext)`。
/// 相同明文每次加密结果不同（nonce 随机），但解密时能正确分离 nonce 和密文。
pub(crate) fn encrypt(cipher: &Aes256Gcm, plaintext: &str) -> anyhow::Result<String> {
    let nonce_bytes: [u8; NONCE_LEN] = rand::random();
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce_bytes), plaintext.as_bytes())
        .map_err(|e| anyhow!("设置项加密失败: {e}"))?;
    let mut blob = nonce_bytes.to_vec();
    blob.extend_from_slice(&ciphertext);
    Ok(B64.encode(blob))
}

/// base64(nonce‖ciphertext) → 解密明文。
///
/// 先 base64 解码，然后分离前 12 字节作为 nonce，剩余部分为密文。
/// 解密失败意味着密钥不匹配或数据损坏。
pub(crate) fn decrypt(cipher: &Aes256Gcm, encoded: &str) -> anyhow::Result<String> {
    let blob = B64.decode(encoded).context("设置项 base64 解码失败")?;
    if blob.len() < NONCE_LEN {
        anyhow::bail!("设置项密文长度非法");
    }
    let (nonce, ciphertext) = blob.split_at(NONCE_LEN);
    let plaintext = cipher
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|e| anyhow!("设置项解密失败（密钥不匹配或数据损坏）: {e}"))?;
    String::from_utf8(plaintext).context("设置项明文非合法 UTF-8")
}

// ============================================================================
// 文件权限辅助（Unix / Windows 条件编译）
// ============================================================================

/// 设置目录权限为 0700（仅 Unix；Windows 无此概念，自动跳过）。
/// REQ-SEC-004：数据目录仅所有者可读写执行，防止其他用户访问敏感数据。
#[cfg(unix)]
pub(crate) fn ensure_dir_0700(dir: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
        .with_context(|| format!("设置目录权限 0700 失败: {}", dir.display()))
}

/// 设置目录权限为 0700（Windows 无此概念，自动跳过）。
#[cfg(not(unix))]
pub(crate) fn ensure_dir_0700(_dir: &Path) -> anyhow::Result<()> {
    Ok(())
}

/// 设置文件权限为 0600（仅 Unix；Windows 无此概念，自动跳过）。
/// REQ-SEC-004：敏感文件（密钥、数据库）仅所有者可读写。
#[cfg(unix)]
pub(crate) fn ensure_file_0600(file: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o600))
        .with_context(|| format!("设置文件权限 0600 失败: {}", file.display()))
}

/// 设置文件权限为 0600（Windows 无此概念，自动跳过）。
#[cfg(not(unix))]
pub(crate) fn ensure_file_0600(_file: &Path) -> anyhow::Result<()> {
    Ok(())
}
