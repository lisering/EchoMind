//! REQ-NFR-019 构建产物体积优化 — TDD 集成测试
//!
//! 验收标准：
//! - AC-4: 前端 vendored 库总体积 ≤ 5MB（`ui/vendor/` 目录）
//! - AC-5: Release profile 启用 LTO + strip

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::Path;

/// 递归计算目录总字节数
fn dir_size_bytes(path: &Path) -> u64 {
    let mut total: u64 = 0;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                total += dir_size_bytes(&entry_path);
            } else if let Ok(metadata) = entry.metadata() {
                total += metadata.len();
            }
        }
    }
    total
}

/// 读取 Cargo.toml 内容
fn read_cargo_toml() -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let cargo_toml_path = Path::new(manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("Cargo.toml"))
        .unwrap_or_else(|| Path::new("../../../Cargo.toml").to_path_buf());
    fs::read_to_string(&cargo_toml_path)
        .unwrap_or_else(|_| panic!("Failed to read Cargo.toml at {:?}", cargo_toml_path))
}

/// 获取 ui/vendor/ 目录路径
fn ui_vendor_path() -> std::path::PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    Path::new(manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("ui").join("vendor"))
        .unwrap_or_else(|| Path::new("../../../ui/vendor").to_path_buf())
}

/// TC-NFR-019-001: Cargo.toml 含 [profile.release] 段且 lto 非 false
///
/// AC-5: Release profile 启用 LTO（`lto = true`）
#[test]
fn tc_nfr_019_001_release_profile_lto_enabled() {
    let cargo_toml = read_cargo_toml();

    // 检查 [profile.release] 段存在
    assert!(
        cargo_toml.contains("[profile.release]"),
        "Cargo.toml must contain [profile.release] section"
    );

    // 查找 [profile.release] 段下的 lto 设置
    let release_section = cargo_toml
        .split("[profile.release]")
        .nth(1)
        .and_then(|s| s.split('[').next())
        .unwrap_or("");

    assert!(
        release_section.contains("lto") && !release_section.contains("lto = false"),
        "[profile.release] must have lto enabled (lto = true or lto = \"fat\"), found: {}",
        release_section.trim()
    );
}

/// TC-NFR-019-002: Cargo.toml 含 strip = true
///
/// AC-5: Release profile 启用 strip（`strip = true`）
#[test]
fn tc_nfr_019_002_release_profile_strip_enabled() {
    let cargo_toml = read_cargo_toml();

    let release_section = cargo_toml
        .split("[profile.release]")
        .nth(1)
        .and_then(|s| s.split('[').next())
        .unwrap_or("");

    assert!(
        release_section.contains("strip = true") || release_section.contains("strip = \"symbols\""),
        "[profile.release] must have strip = true, found: {}",
        release_section.trim()
    );
}

/// TC-NFR-019-003: ui/vendor/ 总体积 ≤ 5MB (5,242,880 bytes)
///
/// AC-4: 前端 vendored 库总体积 ≤ 5MB
#[test]
fn tc_nfr_019_003_ui_vendor_size_within_limit() {
    let vendor_path = ui_vendor_path();
    assert!(
        vendor_path.exists(),
        "ui/vendor/ directory must exist at {:?}",
        vendor_path
    );

    let size = dir_size_bytes(&vendor_path);
    let limit: u64 = 5 * 1024 * 1024; // 5MB = 5,242,880 bytes

    assert!(
        size <= limit,
        "ui/vendor/ size must be ≤ 5MB ({} bytes), actual: {} bytes ({:.2} MB)",
        limit,
        size,
        size as f64 / (1024.0 * 1024.0)
    );
}
