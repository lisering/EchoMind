//! # 安全管理模块（军工级纵深防御）
//!
//! 实现安全防御体系的第三~五层：
//!
//! - **第三层**：自动锁屏 + IPC 拦截 + 系统睡眠检测
//! - **第四层**：内存擦零（通过 `zeroize` crate，在 encryption.rs 中实现）
//! - **第五层**：剪贴板自动清除
//!
//! ## 安全状态机
//!
//! ```text
//! 未加密 → 加密就绪 →(超时/手动)→ 已锁定 →(正确密码)→ 加密就绪
//!                                  →(紧急密码)→ 紧急销毁 → 首次启动
//! ```

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;

use crate::encryption::{BruteForceProtection, KeyDerivation};

/// 锁定原因
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LockReason {
    /// 用户手动锁定（Cmd+L）
    Manual,
    /// 无操作超时自动锁定
    AutoTimeout,
    /// 系统睡眠/休眠唤醒后锁定
    SystemSleep,
}

/// 安全状态
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SecurityState {
    /// 未加密模式（数据库明文存储）
    Unencrypted,
    /// 加密就绪（已加密，已解锁）
    EncryptedUnlocked,
    /// 已锁定（蒙版，IPC 拦截）
    Locked(LockReason),
}

impl SecurityState {
    /// 返回状态图标标识（用于前端安全指示器）
    #[must_use]
    pub fn icon_id(&self) -> &'static str {
        match self {
            SecurityState::Unencrypted => "unlocked",
            SecurityState::EncryptedUnlocked => "encrypted_unlocked",
            SecurityState::Locked(_) => "locked",
        }
    }

    /// 返回状态颜色（用于前端安全指示器）
    #[must_use]
    pub fn color(&self) -> &'static str {
        match self {
            SecurityState::Unencrypted => "#ef4444",
            SecurityState::EncryptedUnlocked => "#22c55e",
            SecurityState::Locked(_) => "#f59e0b",
        }
    }
}

/// 自动锁屏配置
#[derive(Clone, Debug)]
pub struct AutoLockConfig {
    /// 是否启用自动锁屏
    pub enabled: bool,
    /// 无操作超时时间（秒）
    pub timeout_secs: u64,
    /// 是否在系统睡眠唤醒后自动锁定
    pub lock_on_sleep: bool,
}

impl Default for AutoLockConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            timeout_secs: 180, // 3 分钟
            lock_on_sleep: true,
        }
    }
}

/// 剪贴板清除配置
#[derive(Clone, Debug)]
pub struct ClipboardConfig {
    /// 是否启用剪贴板自动清除
    pub enabled: bool,
    /// 清除延迟（秒）
    pub clear_after_secs: u64,
}

impl Default for ClipboardConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            clear_after_secs: 30, // 30 秒
        }
    }
}

/// 紧急销毁配置
///
/// 用户可设置"紧急销毁密码"（与主密码不同）。
/// 密码以 Argon2id 派生密钥的哈希形式存储，不存明文。
#[derive(Clone, Debug, Default)]
pub struct PanicWipeConfig {
    /// 是否已设置紧急销毁密码
    pub enabled: bool,
    /// 密码哈希（Argon2id 派生密钥的十六进制）
    pub password_hash: Option<String>,
    /// salt（十六进制）
    pub salt_hex: Option<String>,
}

/// 安全管理器
///
/// 管理应用的安全状态、自动锁屏定时器、暴力破解防护。
pub struct SecurityManager {
    /// 当前安全状态
    state: Arc<RwLock<SecurityState>>,
    /// 自动锁屏配置
    auto_lock_config: Arc<RwLock<AutoLockConfig>>,
    /// 剪贴板配置
    clipboard_config: Arc<RwLock<ClipboardConfig>>,
    /// 暴力破解防护
    brute_force: Arc<RwLock<BruteForceProtection>>,
    /// 上次用户活动时间
    last_activity: Arc<RwLock<Instant>>,
    /// 锁定时间戳（用于审计日志）
    locked_at: Arc<RwLock<Option<chrono::DateTime<chrono::Utc>>>>,
    /// 紧急销毁配置
    panic_wipe_config: Arc<RwLock<PanicWipeConfig>>,
}

impl SecurityManager {
    /// 创建新的安全管理器（初始状态为未加密）
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(SecurityState::Unencrypted)),
            auto_lock_config: Arc::new(RwLock::new(AutoLockConfig::default())),
            clipboard_config: Arc::new(RwLock::new(ClipboardConfig::default())),
            brute_force: Arc::new(RwLock::new(BruteForceProtection::new())),
            last_activity: Arc::new(RwLock::new(Instant::now())),
            locked_at: Arc::new(RwLock::new(None)),
            panic_wipe_config: Arc::new(RwLock::new(PanicWipeConfig::default())),
        }
    }

    /// 获取当前安全状态
    pub async fn state(&self) -> SecurityState {
        self.state.read().await.clone()
    }

    /// 设置加密状态（启用加密后调用）
    pub async fn set_encrypted(&self) {
        let mut state = self.state.write().await;
        *state = SecurityState::EncryptedUnlocked;
    }

    /// 设置未加密状态
    pub async fn set_unencrypted(&self) {
        let mut state = self.state.write().await;
        *state = SecurityState::Unencrypted;
    }

    /// 手动锁定应用
    pub async fn lock(&self, reason: LockReason) {
        let mut state = self.state.write().await;
        *state = SecurityState::Locked(reason);
        let mut locked_at = self.locked_at.write().await;
        *locked_at = Some(chrono::Utc::now());
    }

    /// 解锁应用（密码验证通过后调用）
    pub async fn unlock(&self) {
        let mut state = self.state.write().await;
        if matches!(*state, SecurityState::Locked(_)) {
            *state = SecurityState::EncryptedUnlocked;
            let mut locked_at = self.locked_at.write().await;
            *locked_at = None;
        }
        let mut bf = self.brute_force.write().await;
        bf.record_success();
    }

    /// 检查应用是否已锁定
    pub async fn is_locked(&self) -> bool {
        matches!(*self.state.read().await, SecurityState::Locked(_))
    }

    /// 记录用户活动（重置自动锁屏计时器）
    pub async fn record_activity(&self) {
        let mut last = self.last_activity.write().await;
        *last = Instant::now();
    }

    /// 检查是否应自动锁定（由定时器定期调用）
    pub async fn check_auto_lock(&self) -> bool {
        let config = self.auto_lock_config.read().await;
        if !config.enabled {
            return false;
        }

        let state = self.state.read().await;
        if !matches!(*state, SecurityState::EncryptedUnlocked) {
            return false;
        }
        drop(state);

        let last = self.last_activity.read().await;
        let elapsed = last.elapsed();
        if elapsed >= Duration::from_secs(config.timeout_secs) {
            drop(last);
            drop(config);
            self.lock(LockReason::AutoTimeout).await;
            return true;
        }
        false
    }

    /// 系统睡眠唤醒后调用
    pub async fn on_system_wake(&self) {
        let config = self.auto_lock_config.read().await;
        if config.lock_on_sleep {
            drop(config);
            self.lock(LockReason::SystemSleep).await;
        }
    }

    /// 更新自动锁屏配置
    pub async fn set_auto_lock_config(&self, config: AutoLockConfig) {
        let mut current = self.auto_lock_config.write().await;
        *current = config;
    }

    /// 获取自动锁屏配置
    pub async fn get_auto_lock_config(&self) -> AutoLockConfig {
        self.auto_lock_config.read().await.clone()
    }

    /// 更新剪贴板配置
    pub async fn set_clipboard_config(&self, config: ClipboardConfig) {
        let mut current = self.clipboard_config.write().await;
        *current = config;
    }

    /// 获取剪贴板配置
    pub async fn get_clipboard_config(&self) -> ClipboardConfig {
        self.clipboard_config.read().await.clone()
    }

    /// 获取暴力破解防护状态
    pub async fn brute_force(&self) -> BruteForceProtection {
        self.brute_force.read().await.clone()
    }

    /// 记录密码验证失败
    pub async fn record_auth_failure(&self) -> bool {
        let mut bf = self.brute_force.write().await;
        bf.record_failure()
    }

    /// 检查是否应触发紧急销毁
    pub async fn should_panic_wipe(&self) -> bool {
        let bf = self.brute_force.read().await;
        bf.should_panic_wipe()
    }

    /// 获取上次锁定时间
    pub async fn locked_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        *self.locked_at.read().await
    }

    /// 获取剩余尝试次数
    pub async fn remaining_attempts(&self) -> u32 {
        self.brute_force.read().await.remaining_attempts()
    }

    /// 获取剩余锁定秒数
    pub async fn remaining_lock_seconds(&self) -> u32 {
        self.brute_force.read().await.remaining_lock_seconds()
    }

    // ========================================================================
    // 紧急销毁（Panic Wipe）
    // ========================================================================

    /// 设置紧急销毁密码
    ///
    /// 密码使用 Argon2id 派生后存储哈希，不存明文。
    pub async fn set_panic_wipe_password(&self, password: &str) {
        let salt = KeyDerivation::generate_salt();
        let key = KeyDerivation::derive_key(password, &salt).unwrap_or([0u8; 32]);
        let salt_hex: String = salt.iter().map(|b| format!("{:02x}", b)).collect();
        let key_hex: String = key.iter().map(|b| format!("{:02x}", b)).collect();

        let mut config = self.panic_wipe_config.write().await;
        config.enabled = true;
        config.password_hash = Some(key_hex);
        config.salt_hex = Some(salt_hex);
    }

    /// 清除紧急销毁密码
    pub async fn clear_panic_wipe_password(&self) {
        let mut config = self.panic_wipe_config.write().await;
        config.enabled = false;
        config.password_hash = None;
        config.salt_hex = None;
    }

    /// 检查输入的密码是否匹配紧急销毁密码
    ///
    /// 使用恒定时间比较防止时序攻击。
    pub async fn check_panic_wipe_password(&self, password: &str) -> bool {
        let config = self.panic_wipe_config.read().await;
        if !config.enabled {
            return false;
        }
        if let (Some(stored_hash), Some(salt_hex)) = (&config.password_hash, &config.salt_hex) {
            let salt: Vec<u8> = (0..salt_hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&salt_hex[i..i + 2], 16).unwrap_or(0))
                .collect();
            let key = KeyDerivation::derive_key(password, &salt).unwrap_or([0u8; 32]);
            let key_hex: String = key.iter().map(|b| format!("{:02x}", b)).collect();
            use subtle::ConstantTimeEq;
            key_hex.as_bytes().ct_eq(stored_hash.as_bytes()).into()
        } else {
            false
        }
    }

    /// 获取紧急销毁配置（用于持久化到 settings 表）
    pub async fn get_panic_wipe_config(&self) -> PanicWipeConfig {
        self.panic_wipe_config.read().await.clone()
    }

    /// 从持久化的 settings 加载紧急销毁配置
    pub async fn load_panic_wipe_config(
        &self,
        enabled: bool,
        password_hash: Option<String>,
        salt_hex: Option<String>,
    ) {
        let mut config = self.panic_wipe_config.write().await;
        config.enabled = enabled;
        config.password_hash = password_hash;
        config.salt_hex = salt_hex;
    }

    /// 紧急销毁是否已启用
    pub async fn is_panic_wipe_enabled(&self) -> bool {
        self.panic_wipe_config.read().await.enabled
    }
}

impl Default for SecurityManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 剪贴板清除管理器
///
/// 实现复制后定时自动清空系统剪贴板。
pub struct ClipboardGuard {
    config: Arc<RwLock<ClipboardConfig>>,
    /// 清除任务的取消句柄
    cancel_handle: Arc<tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl ClipboardGuard {
    /// 创建剪贴板清除管理器
    #[must_use]
    pub fn new(config: ClipboardConfig) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            cancel_handle: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }

    /// 更新配置
    pub async fn set_config(&self, config: ClipboardConfig) {
        let mut current = self.config.write().await;
        *current = config;
    }

    /// 获取配置
    pub async fn get_config(&self) -> ClipboardConfig {
        self.config.read().await.clone()
    }

    /// 启动剪贴板自动清除定时器
    ///
    /// 在用户复制操作后调用。延迟指定秒数后清空剪贴板。
    /// 如果已有定时器在运行，取消旧定时器并启动新的。
    pub async fn schedule_clear(&self) {
        let config = self.config.read().await;
        if !config.enabled {
            return;
        }
        let delay = config.clear_after_secs;
        drop(config);

        // 取消之前的定时器
        {
            let mut handle = self.cancel_handle.lock().await;
            if let Some(old) = handle.take() {
                old.abort();
            }
        }

        // 启动新定时器
        let cancel_handle = self.cancel_handle.clone();
        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(delay)).await;

            // 清空剪贴板
            clear_clipboard();

            // 清除取消句柄
            let mut handle = cancel_handle.lock().await;
            *handle = None;
        });

        let mut h = self.cancel_handle.lock().await;
        *h = Some(handle);
    }

    /// 立即清除剪贴板
    pub async fn clear_now(&self) {
        let mut handle = self.cancel_handle.lock().await;
        if let Some(old) = handle.take() {
            old.abort();
        }
        clear_clipboard();
    }
}

/// 清空系统剪贴板（平台特定实现）
fn clear_clipboard() {
    #[cfg(target_os = "macos")]
    {
        use std::io::Write;
        use std::process::{Command, Stdio};
        if let Ok(mut child) = Command::new("pbcopy").stdin(Stdio::piped()).spawn() {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(b"");
                let _ = stdin.flush();
            }
            let _ = child.wait();
        }
    }

    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xclip")
            .args(["-selection", "clipboard", "/dev/null"])
            .spawn();
    }

    #[cfg(target_os = "windows")]
    {
        // Windows 剪贴板清除通过 Tauri 前端 API 实现
    }
}

/// 启动自动锁屏检查循环
///
/// 每秒检查一次是否应自动锁定。
/// 锁定时通过回调通知调用者。
///
/// 同时检测系统睡眠/唤醒：如果两次 tick 间隔远超 1 秒，
/// 说明系统刚从睡眠中恢复，自动触发锁定。
pub async fn start_auto_lock_loop<F>(security: Arc<SecurityManager>, on_lock: F)
where
    F: Fn() + Send + 'static,
{
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    let mut last_tick = Instant::now();
    loop {
        interval.tick().await;
        let now = Instant::now();
        let elapsed = now.duration_since(last_tick);
        last_tick = now;

        // 系统睡眠检测：如果间隔超过 5 秒，说明系统刚从睡眠中恢复
        if elapsed >= Duration::from_secs(5) {
            security.on_system_wake().await;
            on_lock();
            continue;
        }

        // 自动锁定检查
        if security.check_auto_lock().await {
            on_lock();
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, unused_must_use)]
    use super::*;

    #[tokio::test]
    async fn test_security_state_transitions() {
        let mgr = SecurityManager::new();

        assert_eq!(mgr.state().await, SecurityState::Unencrypted);

        mgr.set_encrypted().await;
        assert_eq!(mgr.state().await, SecurityState::EncryptedUnlocked);

        mgr.lock(LockReason::Manual).await;
        assert!(mgr.is_locked().await);

        mgr.unlock().await;
        assert_eq!(mgr.state().await, SecurityState::EncryptedUnlocked);
    }

    #[tokio::test]
    async fn test_auto_lock_timeout() {
        let mgr = SecurityManager::new();
        mgr.set_encrypted().await;

        mgr.set_auto_lock_config(AutoLockConfig {
            enabled: true,
            timeout_secs: 1,
            lock_on_sleep: true,
        })
        .await;

        mgr.record_activity().await;
        assert!(!mgr.check_auto_lock().await);

        tokio::time::sleep(Duration::from_millis(1500)).await;
        assert!(mgr.check_auto_lock().await);
        assert!(mgr.is_locked().await);
    }

    #[tokio::test]
    async fn test_auto_lock_disabled() {
        let mgr = SecurityManager::new();
        mgr.set_encrypted().await;

        mgr.set_auto_lock_config(AutoLockConfig {
            enabled: false,
            timeout_secs: 1,
            lock_on_sleep: true,
        })
        .await;

        mgr.record_activity().await;
        tokio::time::sleep(Duration::from_millis(1500)).await;

        assert!(!mgr.check_auto_lock().await);
        assert!(!mgr.is_locked().await);
    }

    #[tokio::test]
    async fn test_system_wake_lock() {
        let mgr = SecurityManager::new();
        mgr.set_encrypted().await;

        mgr.on_system_wake().await;
        assert!(mgr.is_locked().await);
        assert!(matches!(
            mgr.state().await,
            SecurityState::Locked(LockReason::SystemSleep)
        ));
    }

    #[tokio::test]
    async fn test_brute_force_integration() {
        let mgr = SecurityManager::new();
        mgr.set_encrypted().await;

        for i in 0..4 {
            let locked = mgr.record_auth_failure().await;
            assert!(!locked, "Attempt {} should not trigger lock", i + 1);
        }

        let locked = mgr.record_auth_failure().await;
        assert!(locked);

        let bf = mgr.brute_force().await;
        assert!(bf.is_locked());
    }

    #[tokio::test]
    async fn test_clipboard_guard_config() {
        let guard = ClipboardGuard::new(ClipboardConfig {
            enabled: true,
            clear_after_secs: 5,
        });

        let config = guard.get_config().await;
        assert!(config.enabled);
        assert_eq!(config.clear_after_secs, 5);
    }

    #[test]
    fn test_security_state_icon_id() {
        assert_eq!(SecurityState::Unencrypted.icon_id(), "unlocked");
        assert_eq!(
            SecurityState::EncryptedUnlocked.icon_id(),
            "encrypted_unlocked"
        );
        assert_eq!(
            SecurityState::Locked(LockReason::Manual).icon_id(),
            "locked"
        );
    }

    #[test]
    fn test_security_state_color() {
        assert_eq!(SecurityState::Unencrypted.color(), "#ef4444");
        assert_eq!(SecurityState::EncryptedUnlocked.color(), "#22c55e");
        assert_eq!(SecurityState::Locked(LockReason::Manual).color(), "#f59e0b");
    }

    #[test]
    fn test_auto_lock_config_default() {
        let config = AutoLockConfig::default();
        assert!(config.enabled);
        assert_eq!(config.timeout_secs, 180);
        assert!(config.lock_on_sleep);
    }

    #[tokio::test]
    async fn test_record_activity_resets_auto_lock_timer() {
        let mgr = SecurityManager::new();
        mgr.set_encrypted().await;
        mgr.set_auto_lock_config(AutoLockConfig {
            enabled: true,
            timeout_secs: 2,
            lock_on_sleep: true,
        })
        .await;

        mgr.record_activity().await;
        tokio::time::sleep(Duration::from_millis(1000)).await;
        assert!(!mgr.check_auto_lock().await);

        mgr.record_activity().await;
        tokio::time::sleep(Duration::from_millis(1000)).await;
        assert!(!mgr.check_auto_lock().await);
    }

    #[tokio::test]
    async fn test_unlock_resets_brute_force() {
        let mgr = SecurityManager::new();
        mgr.set_encrypted().await;

        mgr.record_auth_failure().await;
        mgr.record_auth_failure().await;
        assert_eq!(mgr.remaining_attempts().await, 3);

        mgr.lock(LockReason::Manual).await;
        mgr.unlock().await;

        assert_eq!(mgr.remaining_attempts().await, 5);
    }

    #[tokio::test]
    async fn test_locked_at_timestamp_set_on_lock() {
        let mgr = SecurityManager::new();
        assert!(mgr.locked_at().await.is_none());

        mgr.lock(LockReason::Manual).await;
        assert!(mgr.locked_at().await.is_some());

        mgr.unlock().await;
        assert!(mgr.locked_at().await.is_none());
    }

    #[tokio::test]
    async fn test_panic_wipe_password_set_and_check() {
        let mgr = SecurityManager::new();
        assert!(!mgr.is_panic_wipe_enabled().await);
        mgr.set_panic_wipe_password("panic123").await;
        assert!(mgr.is_panic_wipe_enabled().await);
        assert!(mgr.check_panic_wipe_password("panic123").await);
        assert!(!mgr.check_panic_wipe_password("wrong").await);
        mgr.clear_panic_wipe_password().await;
        assert!(!mgr.is_panic_wipe_enabled().await);
    }
}
