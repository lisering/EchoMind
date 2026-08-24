//! 应用更新检查模块（REQ-HELP-004）。
//!
//! 提供版本比较纯逻辑。GitHub API 调用在 tauri-app 层实现。
//! 纯 Rust 计算，无 I/O 依赖。

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// 更新检查结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCheckResult {
    /// 是否有更新
    pub has_update: bool,
    /// 当前版本号
    pub current_version: String,
    /// 最新版本号
    pub latest_version: String,
    /// 更新日志摘要（如有）
    pub release_notes: Option<String>,
    /// 下载页面 URL
    pub download_url: Option<String>,
}

/// 更新检查配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCheckConfig {
    /// 是否自动检查
    pub auto_check: bool,
    /// 上次检查时间戳（Unix 秒）
    pub last_check: u64,
}

impl Default for UpdateCheckConfig {
    fn default() -> Self {
        Self {
            auto_check: true,
            last_check: 0,
        }
    }
}

/// 24 小时的秒数。
const ONE_DAY_SECS: u64 = 86400;

/// 获取当前 Unix 时间戳（秒）。
pub fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 判断是否需要检查更新（距上次检查超过 24 小时）。
///
/// # 参数
/// - `last_check`: 上次检查的 Unix 时间戳（秒）
///
/// # 返回
/// `true` 表示需要检查
pub fn should_check(last_check: u64) -> bool {
    let now = current_timestamp();
    now.saturating_sub(last_check) >= ONE_DAY_SECS
}

/// 解析 semver 版本号为可比较的元组 (major, minor, patch, pre_release)。
///
/// 支持格式：
/// - `1.0.0`
/// - `v1.0.0`
/// - `1.0.0-alpha`
/// - `1.0.0-beta.1`
///
/// 预发布版本低于同版本号的正式版本（符合 semver 规范）。
///
/// # 返回
/// `Some((major, minor, patch, pre_release_rank))` — pre_release_rank: 0=正式, 1=预发布
/// `None` — 解析失败
pub fn parse_semver(version: &str) -> Option<(u32, u32, u32, u8)> {
    let v = version.trim_start_matches('v').trim();
    let (main_part, pre_part) = if let Some(idx) = v.find('-') {
        (&v[..idx], Some(&v[idx + 1..]))
    } else {
        (v, None)
    };

    let parts: Vec<&str> = main_part.split('.').collect();
    if parts.len() < 3 {
        return None;
    }

    let major = parts[0].parse::<u32>().ok()?;
    let minor = parts[1].parse::<u32>().ok()?;
    let patch = parts[2].parse::<u32>().ok()?;

    // 正式发布 (pre_rank=1) 高于预发布 (pre_rank=0)
    let pre_rank = if pre_part.is_some() { 0 } else { 1 };

    Some((major, minor, patch, pre_rank))
}

/// 比较两个版本号，判断 `latest` 是否比 `current` 新。
///
/// # 参数
/// - `current`: 当前版本号字符串
/// - `latest`: 最新版本号字符串
///
/// # 返回
/// `true` — `latest` 比 `current` 新
/// `false` — `latest` 等于或旧于 `current`，或版本号无法解析
pub fn is_newer_version(current: &str, latest: &str) -> bool {
    let Some(cur) = parse_semver(current) else {
        return false;
    };
    let Some(new) = parse_semver(latest) else {
        return false;
    };
    new > cur
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn test_parse_semver_simple() {
        let (major, minor, patch, pre) = parse_semver("1.0.0").unwrap();
        assert_eq!((major, minor, patch, pre), (1, 0, 0, 1));
    }

    #[test]
    fn test_parse_semver_with_v_prefix() {
        let (major, minor, patch, pre) = parse_semver("v1.2.3").unwrap();
        assert_eq!((major, minor, patch, pre), (1, 2, 3, 1));
    }

    #[test]
    fn test_parse_semver_prerelease() {
        let (major, minor, patch, pre) = parse_semver("1.0.0-alpha").unwrap();
        assert_eq!((major, minor, patch, pre), (1, 0, 0, 0));
    }

    #[test]
    fn test_parse_semver_prerelease_beta() {
        let (major, minor, patch, pre) = parse_semver("1.0.0-beta.1").unwrap();
        assert_eq!((major, minor, patch, pre), (1, 0, 0, 0));
    }

    #[test]
    fn test_parse_semver_invalid() {
        assert!(parse_semver("invalid").is_none());
        assert!(parse_semver("1.2").is_none());
        assert!(parse_semver("").is_none());
    }

    #[test]
    fn test_is_newer_version_true() {
        assert!(is_newer_version("1.0.0", "1.0.1"));
        assert!(is_newer_version("1.0.0", "1.1.0"));
        assert!(is_newer_version("1.0.0", "2.0.0"));
        assert!(is_newer_version("1.12.0", "1.13.0"));
    }

    #[test]
    fn test_is_newer_version_false_same() {
        assert!(!is_newer_version("1.0.0", "1.0.0"));
        assert!(!is_newer_version("1.12.0", "1.12.0"));
    }

    #[test]
    fn test_is_newer_version_false_older() {
        assert!(!is_newer_version("1.1.0", "1.0.0"));
        assert!(!is_newer_version("2.0.0", "1.9.9"));
    }

    #[test]
    fn test_is_newer_version_prerelease() {
        // 预发布版本低于正式版本
        assert!(!is_newer_version("1.0.0", "1.0.0-alpha"));
        assert!(is_newer_version("1.0.0-alpha", "1.0.0"));
    }

    #[test]
    fn test_should_check_first_time() {
        // 从未检查过（last_check = 0），应该检查
        assert!(should_check(0));
    }

    #[test]
    fn test_should_check_recently_checked() {
        let now = current_timestamp();
        // 1 小时前检查过，不应该检查
        assert!(!should_check(now - 3600));
    }

    #[test]
    fn test_should_check_24h_ago() {
        let now = current_timestamp();
        // 25 小时前检查过，应该检查
        assert!(should_check(now - ONE_DAY_SECS - 3600));
    }

    #[test]
    fn test_update_check_result_serde() {
        let result = UpdateCheckResult {
            has_update: true,
            current_version: "1.0.0".to_string(),
            latest_version: "1.1.0".to_string(),
            release_notes: Some("Bug fixes".to_string()),
            download_url: Some("https://github.com".to_string()),
        };
        let json = serde_json::to_string(&result).unwrap();
        let decoded: UpdateCheckResult = serde_json::from_str(&json).unwrap();
        assert!(decoded.has_update);
        assert_eq!(decoded.current_version, "1.0.0");
        assert_eq!(decoded.latest_version, "1.1.0");
    }

    #[test]
    fn test_update_check_config_default() {
        let config = UpdateCheckConfig::default();
        assert!(config.auto_check);
        assert_eq!(config.last_check, 0);
    }
}
