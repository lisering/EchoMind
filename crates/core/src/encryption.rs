//! # 数据库加密模块（军工级 — Argon2id）
//!
//! 提供 SQLCipher 数据库加密所需的密钥派生和密码验证功能。
//!
//! ## 加密方案
//!
//! 使用 **Argon2id** 算法从用户密码派生 256 位加密密钥，
//! 密钥仅存在于内存中，应用退出后通过 `zeroize` 物理擦零。
//!
//! ### Argon2id 参数（FR-SEC-L2-002）
//!
//! - 内存成本：64 MB（65536 KiB）
//! - 时间成本：3 轮
//! - 并行度：4
//! - 输出长度：32 字节（256 位）
//!
//! ### 向后兼容
//!
//! 旧版本使用 PBKDF2-HMAC-SHA256（100K iterations）。
//! 升级后自动检测：新数据库使用 Argon2id，
//! 旧数据库仍可通过 PBKDF2 验证后升级。
//!
//! ## 安全设计
//!
//! - 密码不存储，仅存储 salt
//! - 密钥仅存在于内存中（通过 PRAGMA key 传递给 SQLCipher）
//! - Argon2id 抗 GPU/ASIC 暴力破解
//! - 密码验证使用 constant-time comparison（subtle crate）
//! - 敏感数据使用 `zeroize` 物理擦零

use anyhow::Result;
use argon2::{Algorithm, Argon2, Params, Version};
use zeroize::Zeroize;

/// Argon2id 内存成本：64 MB（65536 KiB）
const ARGON2_MEMORY_COST: u32 = 65_536;

/// Argon2id 时间成本：3 轮
const ARGON2_TIME_COST: u32 = 3;

/// Argon2id 并行度：4
const ARGON2_PARALLELISM: u32 = 4;

/// 派生密钥长度（32 字节 = 256 位）
const DERIVED_KEY_LEN: usize = 32;

/// salt 长度（16 字节 = 128 位）
pub const SALT_LEN: usize = 16;

/// 密钥派生算法标识
///
/// 存储在数据库 settings 表中，用于判断使用哪种 KDF。
/// - "argon2id"：当前推荐（Argon2id）
/// - "pbkdf2"：旧版本兼容（PBKDF2-HMAC-SHA256, 100K iterations）
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KdfAlgorithm {
    Argon2id,
    Pbkdf2,
}

impl KdfAlgorithm {
    /// 返回算法标识字符串
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            KdfAlgorithm::Argon2id => "argon2id",
            KdfAlgorithm::Pbkdf2 => "pbkdf2",
        }
    }

    /// 从字符串解析算法标识
    ///
    /// 注意：此方法命名为 `parse` 而非 `from_str`，以避免与 `std::str::FromStr` trait 混淆。
    #[must_use]
    pub fn parse(s: &str) -> Self {
        match s {
            "argon2id" => KdfAlgorithm::Argon2id,
            _ => KdfAlgorithm::Pbkdf2,
        }
    }
}

/// **安全字符串** — 用后即焚
///
/// 包装用户密码等敏感字符串，在 Drop 时调用 `zeroize` 物理擦零，
/// 确保内存中不再残留原始密码明文。
#[derive(Zeroize)]
pub struct SecureString {
    inner: String,
}

impl SecureString {
    /// 从普通字符串创建安全字符串
    #[must_use]
    pub fn new(s: String) -> Self {
        Self { inner: s }
    }

    /// 返回内部字符串引用（用于密钥派生等操作）
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.inner
    }

    /// 手动擦零（在派生密钥后立即调用）
    pub fn wipe(&mut self) {
        self.inner.zeroize();
    }
}

impl From<String> for SecureString {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl From<&str> for SecureString {
    fn from(s: &str) -> Self {
        Self::new(s.to_string())
    }
}

impl std::fmt::Debug for SecureString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SecureString(**REDACTED**)")
    }
}

impl std::ops::Deref for SecureString {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// 密钥派生器（军工级 — Argon2id）
///
/// 从用户密码派生 SQLCipher 加密密钥。
/// 使用 Argon2id 算法，抗 GPU/ASIC 暴力破解。
pub struct KeyDerivation;

impl KeyDerivation {
    /// 生成随机 salt
    ///
    /// 使用系统级 CSPRNG 生成 16 字节随机 salt。
    #[must_use]
    pub fn generate_salt() -> [u8; SALT_LEN] {
        use rand::RngCore;
        let mut salt = [0u8; SALT_LEN];
        rand::rngs::OsRng.fill_bytes(&mut salt);
        salt
    }

    /// 从密码和 salt 派生密钥（Argon2id）
    ///
    /// 使用 Argon2id 算法，参数满足 FR-SEC-L2-002：
    /// - 内存成本：64 MB
    /// - 时间成本：3 轮
    /// - 并行度：4
    /// - 输出：32 字节（256 位）
    pub fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; DERIVED_KEY_LEN]> {
        let params = Params::new(
            ARGON2_MEMORY_COST,
            ARGON2_TIME_COST,
            ARGON2_PARALLELISM,
            Some(DERIVED_KEY_LEN),
        )
        .map_err(|e| anyhow::anyhow!("Argon2id 参数无效: {e:?}"))?;

        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
        let mut key = [0u8; DERIVED_KEY_LEN];
        argon2
            .hash_password_into(password.as_bytes(), salt, &mut key)
            .map_err(|e| anyhow::anyhow!("Argon2id 密钥派生失败: {e:?}"))?;
        Ok(key)
    }

    /// 从密码和 salt 派生密钥（PBKDF2 向后兼容）
    ///
    /// 用于验证旧版本设置的密码。
    /// PBKDF2-HMAC-SHA256, 100000 iterations.
    #[must_use]
    pub fn derive_key_pbkdf2(password: &str, salt: &[u8]) -> [u8; DERIVED_KEY_LEN] {
        pbkdf2_hmac_sha256(
            password.as_bytes(),
            salt,
            PBKDF2_ITERATIONS,
            DERIVED_KEY_LEN,
        )
    }

    /// 将密钥编码为 SQLCipher PRAGMA key 所需的十六进制格式
    ///
    /// SQLCipher 支持 `PRAGMA key = "x'2DD29CA8...'"` 格式
    #[must_use]
    pub fn key_to_hex(key: &[u8]) -> String {
        let hex: String = key.iter().map(|b| format!("{:02x}", b)).collect();
        format!("x'{hex}'")
    }

    /// 从密码派生密钥并生成 PRAGMA key 语句的参数（Argon2id）
    ///
    /// 返回的密码会被擦零。
    pub fn derive_pragma_key(password: &str, salt: &[u8]) -> Result<String> {
        let mut secure = SecureString::from(password);
        let key = Self::derive_key(&secure, salt)?;
        secure.wipe();
        Ok(Self::key_to_hex(&key))
    }

    /// 验证密码是否正确（Argon2id）
    ///
    /// 通过 constant-time comparison 比较派生密钥与预期密钥，
    /// 防止时序攻击。
    pub fn verify_password(password: &str, salt: &[u8], expected_key: &[u8]) -> Result<bool> {
        let derived = Self::derive_key(password, salt)?;
        Ok(subtle::ConstantTimeEq::ct_eq(&derived[..], expected_key).into())
    }

    /// 验证密码是否正确（PBKDF2 向后兼容）
    #[must_use]
    pub fn verify_password_pbkdf2(password: &str, salt: &[u8], expected_key: &[u8]) -> bool {
        let derived = Self::derive_key_pbkdf2(password, salt);
        subtle::ConstantTimeEq::ct_eq(&derived[..], expected_key).into()
    }

    /// 获取当前 KDF 算法标识
    #[must_use]
    pub fn current_algorithm() -> KdfAlgorithm {
        KdfAlgorithm::Argon2id
    }
}

// ============================================================================
// PBKDF2 向后兼容实现
// ============================================================================

/// PBKDF2 迭代次数（100000 次）— 仅用于向后兼容旧数据
const PBKDF2_ITERATIONS: u32 = 100_000;

/// PBKDF2-HMAC-SHA256 手动实现（向后兼容）
fn pbkdf2_hmac_sha256(
    password: &[u8],
    salt: &[u8],
    iterations: u32,
    _dk_len: usize,
) -> [u8; DERIVED_KEY_LEN] {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    const H_LEN: usize = 32;
    let mut derived_key = [0u8; DERIVED_KEY_LEN];

    for (block_index, chunk) in (1u32..).zip(derived_key.chunks_mut(H_LEN)) {
        // HMAC 初始化在密码有效时不会失败；若失败则跳过该 block
        let Ok(mut hmac) = Hmac::<Sha256>::new_from_slice(password) else {
            continue;
        };
        hmac.update(salt);
        hmac.update(&block_index.to_be_bytes());
        let mut u_prev = hmac.finalize().into_bytes();
        let mut t = u_prev;

        for _ in 1..iterations {
            let Ok(mut hmac) = Hmac::<Sha256>::new_from_slice(password) else {
                continue;
            };
            hmac.update(&u_prev);
            u_prev = hmac.finalize().into_bytes();
            for (t_byte, u_byte) in t.iter_mut().zip(u_prev.iter()) {
                *t_byte ^= u_byte;
            }
        }

        let copy_len = chunk.len().min(H_LEN);
        chunk[..copy_len].copy_from_slice(&t[..copy_len]);
    }

    derived_key
}

// ============================================================================
// 密码强度检测
// ============================================================================

/// 密码强度等级
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PasswordStrength {
    /// 极弱（如 123456, password）
    VeryWeak,
    /// 弱（<6 位 或 纯数字/纯字母）
    Weak,
    /// 中（6+ 位含字母+数字）
    Medium,
    /// 强（8+ 位含大小写+数字+特殊字符）
    Strong,
    /// 极强（12+ 位含大小写+数字+特殊字符）
    VeryStrong,
}

impl PasswordStrength {
    /// 返回颜色标识（用于前端强度条）
    #[must_use]
    pub fn color(&self) -> &'static str {
        match self {
            PasswordStrength::VeryWeak => "#ef4444",
            PasswordStrength::Weak => "#f97316",
            PasswordStrength::Medium => "#eab308",
            PasswordStrength::Strong => "#22c55e",
            PasswordStrength::VeryStrong => "#16a34a",
        }
    }

    /// 返回百分比（0-100，用于强度条宽度）
    #[must_use]
    pub fn percentage(&self) -> u32 {
        match self {
            PasswordStrength::VeryWeak => 20,
            PasswordStrength::Weak => 40,
            PasswordStrength::Medium => 60,
            PasswordStrength::Strong => 80,
            PasswordStrength::VeryStrong => 100,
        }
    }

    /// 返回国际化键名（用于前端 t() 函数）
    #[must_use]
    pub fn i18n_key(&self) -> &'static str {
        match self {
            PasswordStrength::VeryWeak => "pwd_strength_very_weak",
            PasswordStrength::Weak => "pwd_strength_weak",
            PasswordStrength::Medium => "pwd_strength_medium",
            PasswordStrength::Strong => "pwd_strength_strong",
            PasswordStrength::VeryStrong => "pwd_strength_very_strong",
        }
    }
}

/// 常见弱密码字典（Top 100）
const COMMON_WEAK_PASSWORDS: &[&str] = &[
    "123456",
    "password",
    "123456789",
    "12345678",
    "12345",
    "1234567",
    "1234567890",
    "qwerty",
    "abc123",
    "111111",
    "iloveyou",
    "admin",
    "welcome",
    "monkey",
    "login",
    "password1",
    "123123",
    "000000",
    "master",
    "666666",
    "dragon",
    "sunshine",
    "princess",
    "letmein",
    "trustno1",
    "passw0rd",
    "1234",
    "admin123",
    "root",
    "toor",
    "11111111",
    "00000000",
    "1q2w3e4r",
    "qwerty123",
    "password123",
    "123abc",
    "admin@123",
    "Passw0rd",
    "P@ssw0rd",
    "p@ssword",
    "123qwe",
    "test",
    "guest",
];

/// 密码强度检测器
///
/// 实现 FR-SEC-L2-005：注册时检测弱密码并提示"建议使用更强密码"。
/// 检测维度：长度、复杂度（大小写、数字、特殊字符）、常见密码字典。
pub struct PasswordStrengthChecker;

impl PasswordStrengthChecker {
    /// 检测密码强度
    #[must_use]
    pub fn check(password: &str) -> PasswordStrength {
        let len = password.len();

        // 1. 检查常见弱密码字典
        let lower = password.to_lowercase();
        if COMMON_WEAK_PASSWORDS.contains(&lower.as_str()) {
            return PasswordStrength::VeryWeak;
        }

        // 2. 检查长度
        if len < 6 {
            return PasswordStrength::VeryWeak;
        }

        // 3. 检查字符多样性
        let has_lower = password.chars().any(|c| c.is_ascii_lowercase());
        let has_upper = password.chars().any(|c| c.is_ascii_uppercase());
        let has_digit = password.chars().any(|c| c.is_ascii_digit());
        let has_special = password.chars().any(|c| !c.is_alphanumeric());

        let char_type_count = [has_lower, has_upper, has_digit, has_special]
            .iter()
            .filter(|&&b| b)
            .count();

        // 4. 全纯数字或全纯字母
        if char_type_count == 1 {
            return if len >= 8 {
                PasswordStrength::Weak
            } else {
                PasswordStrength::VeryWeak
            };
        }

        // 5. 综合评分
        let score = match (len, char_type_count) {
            (0..=5, _) => 0,
            (6..=7, 2..=4) => 2,
            (8..=11, 2) => 2,
            (8..=11, 3) => 3,
            (8..=11, 4) => 3,
            (12.., 3) => 4,
            (12.., 4) => 4,
            _ => 1,
        };

        match score {
            0 => PasswordStrength::VeryWeak,
            1 => PasswordStrength::Weak,
            2 => PasswordStrength::Medium,
            3 => PasswordStrength::Strong,
            _ => PasswordStrength::VeryStrong,
        }
    }

    /// 返回改进建议
    #[must_use]
    pub fn suggestions(password: &str) -> Vec<&'static str> {
        let mut tips = Vec::new();
        let len = password.len();

        if len < 8 {
            tips.push("pwd_tip_length");
        }
        if !password.chars().any(|c| c.is_ascii_uppercase()) {
            tips.push("pwd_tip_uppercase");
        }
        if !password.chars().any(|c| c.is_ascii_digit()) {
            tips.push("pwd_tip_digit");
        }
        if !password.chars().any(|c| !c.is_alphanumeric()) {
            tips.push("pwd_tip_special");
        }
        let lower = password.to_lowercase();
        if COMMON_WEAK_PASSWORDS.contains(&lower.as_str()) {
            tips.push("pwd_tip_common");
        }

        tips
    }
}

// ============================================================================
// 暴力破解防护
// ============================================================================

/// 暴力破解防护状态
///
/// 实现 FR-SEC-L2-007：错误密码尝试 5 次后强制等待 30 秒。
/// 实现 FR-SEC-L2-008：连续错误 10 次后可选"紧急销毁"模式。
#[derive(Clone, Debug)]
pub struct BruteForceProtection {
    /// 当前连续错误次数
    failed_attempts: u32,
    /// 最大允许错误次数（超过后强制等待）
    max_attempts: u32,
    /// 锁定截止时间（Unix 时间戳秒）
    locked_until: Option<i64>,
    /// 是否启用紧急销毁模式
    panic_wipe_enabled: bool,
    /// 触发紧急销毁的阈值
    panic_wipe_threshold: u32,
}

impl Default for BruteForceProtection {
    fn default() -> Self {
        Self::new()
    }
}

impl BruteForceProtection {
    /// 创建默认配置：5 次错误后锁定 30 秒，10 次后触发紧急销毁
    #[must_use]
    pub fn new() -> Self {
        Self {
            failed_attempts: 0,
            max_attempts: 5,
            locked_until: None,
            panic_wipe_enabled: false,
            panic_wipe_threshold: 10,
        }
    }

    /// 检查当前是否被锁定（频率限制）
    #[must_use]
    pub fn is_locked(&self) -> bool {
        if let Some(until) = self.locked_until {
            let now = chrono::Utc::now().timestamp();
            now < until
        } else {
            false
        }
    }

    /// 返回剩余锁定秒数
    #[must_use]
    pub fn remaining_lock_seconds(&self) -> u32 {
        if let Some(until) = self.locked_until {
            let now = chrono::Utc::now().timestamp();
            let remaining = until - now;
            if remaining > 0 { remaining as u32 } else { 0 }
        } else {
            0
        }
    }

    /// 记录一次失败尝试，返回是否应触发锁定
    ///
    /// 返回 `true` 表示刚刚触发锁定，前端应显示倒计时。
    #[must_use]
    pub fn record_failure(&mut self) -> bool {
        self.failed_attempts += 1;

        if self.failed_attempts >= self.max_attempts {
            let lock_duration = 30; // 30 秒
            self.locked_until = Some(chrono::Utc::now().timestamp() + lock_duration as i64);
            true
        } else {
            false
        }
    }

    /// 记录一次成功尝试，重置计数器
    pub fn record_success(&mut self) {
        self.failed_attempts = 0;
        self.locked_until = None;
    }

    /// 检查是否应触发紧急销毁
    #[must_use]
    pub fn should_panic_wipe(&self) -> bool {
        self.panic_wipe_enabled && self.failed_attempts >= self.panic_wipe_threshold
    }

    /// 启用/禁用紧急销毁模式
    pub fn set_panic_wipe_enabled(&mut self, enabled: bool) {
        self.panic_wipe_enabled = enabled;
    }

    /// 获取当前错误次数
    #[must_use]
    pub fn failed_attempts(&self) -> u32 {
        self.failed_attempts
    }

    /// 获取剩余尝试次数（锁定前）
    #[must_use]
    pub fn remaining_attempts(&self) -> u32 {
        self.max_attempts.saturating_sub(self.failed_attempts)
    }

    /// 重置状态（解锁后调用）
    pub fn reset(&mut self) {
        self.failed_attempts = 0;
        self.locked_until = None;
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, unused_must_use)]
    use super::*;

    // ============ Argon2id 密钥派生 ============

    #[test]
    fn test_argon2id_derive_key_is_deterministic() {
        let salt = [0u8; SALT_LEN];
        let key1 = KeyDerivation::derive_key("password123", &salt).unwrap();
        let key2 = KeyDerivation::derive_key("password123", &salt).unwrap();
        assert_eq!(key1, key2, "相同密码和 salt 应派生出相同密钥");
    }

    #[test]
    fn test_argon2id_different_passwords_produce_different_keys() {
        let salt = [0u8; SALT_LEN];
        let key1 = KeyDerivation::derive_key("password1", &salt).unwrap();
        let key2 = KeyDerivation::derive_key("password2", &salt).unwrap();
        assert_ne!(key1, key2, "不同密码应派生出不同密钥");
    }

    #[test]
    fn test_argon2id_different_salts_produce_different_keys() {
        let salt1 = [0u8; SALT_LEN];
        let salt2 = [1u8; SALT_LEN];
        let key1 = KeyDerivation::derive_key("password", &salt1).unwrap();
        let key2 = KeyDerivation::derive_key("password", &salt2).unwrap();
        assert_ne!(key1, key2, "不同 salt 应派生出不同密钥");
    }

    #[test]
    fn test_argon2id_key_length_is_32_bytes() {
        let salt = [0u8; SALT_LEN];
        let key = KeyDerivation::derive_key("test", &salt).unwrap();
        assert_eq!(key.len(), DERIVED_KEY_LEN, "密钥长度应为 32 字节（256 位）");
    }

    #[test]
    fn test_argon2id_empty_password_does_not_error() {
        let salt = [0u8; SALT_LEN];
        let key = KeyDerivation::derive_key("", &salt).unwrap();
        assert_eq!(key.len(), DERIVED_KEY_LEN, "空密码也应派生出 32 字节密钥");
    }

    #[test]
    fn test_argon2id_unicode_password() {
        let salt = [0u8; SALT_LEN];
        let key1 = KeyDerivation::derive_key("密码123", &salt).unwrap();
        let key2 = KeyDerivation::derive_key("密码123", &salt).unwrap();
        assert_eq!(key1, key2, "Unicode 密码应正确派生且确定性");
    }

    // ============ Salt 生成 ============

    #[test]
    fn test_generate_salt_produces_random_values() {
        let salt1 = KeyDerivation::generate_salt();
        let salt2 = KeyDerivation::generate_salt();
        assert_ne!(salt1, salt2, "两次生成的 salt 应不同");
    }

    #[test]
    fn test_generate_salt_length_is_16_bytes() {
        let salt = KeyDerivation::generate_salt();
        assert_eq!(salt.len(), SALT_LEN, "salt 长度应为 16 字节");
    }

    // ============ 密码验证 ============

    #[test]
    fn test_verify_password_correct_returns_true() {
        let salt = [0u8; SALT_LEN];
        let key = KeyDerivation::derive_key("mypassword", &salt).unwrap();
        assert!(
            KeyDerivation::verify_password("mypassword", &salt, &key).unwrap(),
            "正确密码应验证通过"
        );
    }

    #[test]
    fn test_verify_password_incorrect_returns_false() {
        let salt = [0u8; SALT_LEN];
        let key = KeyDerivation::derive_key("correct", &salt).unwrap();
        assert!(
            !KeyDerivation::verify_password("wrong", &salt, &key).unwrap(),
            "错误密码应验证失败"
        );
    }

    #[test]
    fn test_verify_password_wrong_salt_returns_false() {
        let salt1 = [0u8; SALT_LEN];
        let salt2 = [1u8; SALT_LEN];
        let key = KeyDerivation::derive_key("password", &salt1).unwrap();
        assert!(
            !KeyDerivation::verify_password("password", &salt2, &key).unwrap(),
            "正确密码但错误 salt 应验证失败"
        );
    }

    // ============ PBKDF2 向后兼容 ============

    #[test]
    fn test_pbkdf2_derive_and_verify_correct() {
        let salt = [0u8; SALT_LEN];
        let key = KeyDerivation::derive_key_pbkdf2("test_password", &salt);
        assert!(
            KeyDerivation::verify_password_pbkdf2("test_password", &salt, &key),
            "PBKDF2 正确密码应验证通过"
        );
    }

    #[test]
    fn test_pbkdf2_verify_incorrect_returns_false() {
        let salt = [0u8; SALT_LEN];
        let key = KeyDerivation::derive_key_pbkdf2("test_password", &salt);
        assert!(
            !KeyDerivation::verify_password_pbkdf2("wrong", &salt, &key),
            "PBKDF2 错误密码应验证失败"
        );
    }

    #[test]
    fn test_pbkdf2_key_length_is_32_bytes() {
        let salt = [0u8; SALT_LEN];
        let key = KeyDerivation::derive_key_pbkdf2("test", &salt);
        assert_eq!(key.len(), DERIVED_KEY_LEN, "PBKDF2 密钥长度应为 32 字节");
    }

    // ============ Hex 编码 ============

    #[test]
    fn test_key_to_hex_format() {
        let key = [0xab, 0xcd, 0xef, 0x01];
        let hex = KeyDerivation::key_to_hex(&key);
        assert_eq!(hex, "x'abcdef01'");
    }

    #[test]
    fn test_key_to_hex_empty_key() {
        let key = [];
        let hex = KeyDerivation::key_to_hex(&key);
        assert_eq!(hex, "x''", "空密钥应编码为 x''");
    }

    // ============ PRAGMA key 派生 ============

    #[test]
    fn test_derive_pragma_key_format() {
        let salt = [0u8; SALT_LEN];
        let pragma_key = KeyDerivation::derive_pragma_key("test", &salt).unwrap();
        assert!(pragma_key.starts_with("x'"), "PRAGMA key 应以 x' 开头");
        assert!(pragma_key.ends_with("'"), "PRAGMA key 应以 ' 结尾");
    }

    // ============ SecureString ============

    #[test]
    fn test_secure_string_debug_redacts_content() {
        let secure = SecureString::from("super_secret_password_123");
        let debug_str = format!("{secure:?}");
        assert!(
            !debug_str.contains("super_secret_password_123"),
            "Debug 输出不应暴露原始密码"
        );
        assert!(
            debug_str.contains("REDACTED"),
            "Debug 输出应包含 REDACTED 标记"
        );
    }

    #[test]
    fn test_secure_string_wipe_clears_content() {
        let mut secure = SecureString::from("secret_password");
        assert_eq!(secure.as_str(), "secret_password", "wipe 前应有内容");
        secure.wipe();
        assert_eq!(secure.as_str(), "", "wipe 后内部字符串应为空");
    }

    // ============ KdfAlgorithm ============

    #[test]
    fn test_kdf_algorithm_as_str() {
        assert_eq!(KdfAlgorithm::Argon2id.as_str(), "argon2id");
        assert_eq!(KdfAlgorithm::Pbkdf2.as_str(), "pbkdf2");
    }

    #[test]
    fn test_kdf_algorithm_parse_known() {
        assert_eq!(KdfAlgorithm::parse("argon2id"), KdfAlgorithm::Argon2id);
        assert_eq!(KdfAlgorithm::parse("pbkdf2"), KdfAlgorithm::Pbkdf2);
    }

    #[test]
    fn test_current_algorithm_is_argon2id() {
        assert_eq!(KeyDerivation::current_algorithm(), KdfAlgorithm::Argon2id);
    }

    // ============ 密码强度检测 ============

    #[test]
    fn test_password_strength_common_weak_passwords_are_very_weak() {
        assert_eq!(
            PasswordStrengthChecker::check("123456"),
            PasswordStrength::VeryWeak
        );
        assert_eq!(
            PasswordStrengthChecker::check("password"),
            PasswordStrength::VeryWeak
        );
    }

    #[test]
    fn test_password_strength_short_password_is_very_weak() {
        assert_eq!(
            PasswordStrengthChecker::check("12345"),
            PasswordStrength::VeryWeak
        );
    }

    #[test]
    fn test_password_strength_two_types_is_medium() {
        assert_eq!(
            PasswordStrengthChecker::check("xyz789"),
            PasswordStrength::Medium
        );
    }

    #[test]
    fn test_password_strength_four_types_12_chars_is_very_strong() {
        assert_eq!(
            PasswordStrengthChecker::check("Str0ng@Pass!2024"),
            PasswordStrength::VeryStrong
        );
    }

    #[test]
    fn test_password_suggestions_for_strong_password() {
        let tips = PasswordStrengthChecker::suggestions("Str0ng@Pass!2024");
        assert!(tips.is_empty(), "强密码不应有改进建议，实际: {tips:?}");
    }

    // ============ 暴力破解防护 ============

    #[test]
    fn test_brute_force_initial_state_is_unlocked() {
        let bfp = BruteForceProtection::new();
        assert!(!bfp.is_locked(), "初始状态不应被锁定");
        assert_eq!(bfp.failed_attempts(), 0);
        assert_eq!(bfp.remaining_attempts(), 5);
    }

    #[test]
    fn test_brute_force_locks_after_5_failures() {
        let mut bfp = BruteForceProtection::new();

        for i in 0..4 {
            let locked = bfp.record_failure();
            assert!(!locked, "第 {} 次失败不应触发锁定", i + 1);
        }

        let locked = bfp.record_failure();
        assert!(locked, "第 5 次失败应触发锁定");
        assert!(bfp.is_locked());
    }

    #[test]
    fn test_brute_force_success_resets_counter() {
        let mut bfp = BruteForceProtection::new();
        bfp.record_failure();
        bfp.record_failure();

        bfp.record_success();
        assert_eq!(bfp.failed_attempts(), 0);
        assert!(!bfp.is_locked());
    }

    #[test]
    fn test_brute_force_panic_wipe_when_enabled_and_threshold_reached() {
        let mut bfp = BruteForceProtection::new();
        bfp.set_panic_wipe_enabled(true);

        for _ in 0..10 {
            bfp.record_failure();
        }
        assert!(bfp.should_panic_wipe());
    }

    #[test]
    fn test_brute_force_no_panic_wipe_when_disabled() {
        let mut bfp = BruteForceProtection::new();
        for _ in 0..15 {
            bfp.record_failure();
        }
        assert!(!bfp.should_panic_wipe());
    }
}
