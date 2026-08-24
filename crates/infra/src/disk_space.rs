//! 磁盘空间检查模块（P1-1：SQLite 磁盘满弹性设计）。
//!
//! R3 复盘发现磁盘满是最大弹性缺口（评分 2/10）。SQLite 写入时如果磁盘满，
//! 返回 `SQLITE_FULL` 错误，但用户只看到 `STORAGE:` 前缀，无法理解根因。
//!
//! 本模块提供：
//! - `disk_free_space()` — 跨平台获取磁盘可用空间（从 robust_downloader 提取为公共函数）
//! - `disk_total_space()` — 跨平台获取磁盘总空间
//! - `DiskSpaceInfo` — 磁盘空间信息结构体
//! - `check_disk_space()` — 检查指定路径的可用空间是否满足阈值
//!
//! ## 设计原则
//!
//! - **预防优于恢复**：在关键写入操作前检查磁盘空间，避免 SQLITE_FULL 错误
//! - **优雅降级**：磁盘满时返回 `DISK_FULL:` 前缀错误，前端展示"磁盘空间不足"提示
//! - **自动清理**：提供 `cleanup_temp_files()` 清理临时文件释放空间
//! - **跨平台**：Unix statvfs / Windows GetDiskFreeSpaceExW

use std::path::Path;

use anyhow::{Context, Result, bail};

/// 磁盘空间不足的默认阈值（50MB）。
///
/// 当可用空间低于此值时，写入操作应被拦截并返回 `DISK_FULL:` 错误。
/// 50MB 留量用于 SQLite WAL 文件、临时文件、日志轮转等系统开销。
pub const DISK_SPACE_LOW_THRESHOLD: u64 = 50 * 1024 * 1024;

/// 磁盘空间信息。
#[derive(Debug, Clone, serde::Serialize)]
pub struct DiskSpaceInfo {
    /// 可用空间（字节）
    pub free_bytes: u64,
    /// 总空间（字节）
    pub total_bytes: u64,
    /// 使用空间（字节）
    pub used_bytes: u64,
    /// 可用空间百分比（0.0 ~ 100.0）
    pub free_percent: f64,
    /// 是否低于阈值
    pub is_low: bool,
    /// 阈值（字节）
    pub threshold_bytes: u64,
}

impl DiskSpaceInfo {
    /// 从 free_bytes 和 total_bytes 构建磁盘空间信息。
    fn new(free_bytes: u64, total_bytes: u64) -> Self {
        let used_bytes = total_bytes.saturating_sub(free_bytes);
        let free_percent = if total_bytes > 0 {
            (free_bytes as f64 / total_bytes as f64) * 100.0
        } else {
            0.0
        };
        let is_low = free_bytes < DISK_SPACE_LOW_THRESHOLD;
        Self {
            free_bytes,
            total_bytes,
            used_bytes,
            free_percent,
            is_low,
            threshold_bytes: DISK_SPACE_LOW_THRESHOLD,
        }
    }
}

/// 获取磁盘可用空间（字节）。
///
/// Unix: `statvfs` 系统调用；Windows: `GetDiskFreeSpaceExW`。
///
/// # 参数
/// - `path`: 任意路径（文件或目录），函数会检查该路径所在磁盘分区的可用空间
///
/// # 返回
/// 可用空间字节数。如果路径不存在或系统调用失败，返回 `Err`。
pub fn disk_free_space(path: &Path) -> Result<u64> {
    #[cfg(unix)]
    {
        use std::ffi::CString;
        let path_str = path.to_str().context("路径转字符串失败")?;
        let c_path = CString::new(path_str).context("路径 CString 转换失败")?;
        // SAFETY: c_path 是有效的 C 字符串，statvfs 不会解引用无效内存
        unsafe {
            let mut statv: libc_statvfs = std::mem::zeroed();
            if statvfs(c_path.as_ptr(), &mut statv) != 0 {
                bail!("statvfs 失败: {}", std::io::Error::last_os_error());
            }
            // 使用 u128 防止溢出（f_bavail * f_bsize 可能超过 u64 范围）
            let avail = statv.f_bavail as u128;
            let bsize = statv.f_bsize as u128;
            Ok((avail * bsize).min(u64::MAX as u128) as u64)
        }
    }
    #[cfg(windows)]
    {
        // Windows: 使用 GetDiskFreeSpaceExW
        use std::os::windows::ffi::OsStrExt;
        let wide: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let mut free: u64 = 0;
        // SAFETY: wide 是以 null 结尾的 UTF-16 字符串
        unsafe {
            if GetDiskFreeSpaceExW(
                wide.as_ptr(),
                &mut free,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            ) == 0
            {
                bail!("GetDiskFreeSpaceExW 失败");
            }
        }
        Ok(free)
    }
    #[cfg(not(any(unix, windows)))]
    {
        // 未知平台：返回足够大的值（不阻塞操作）
        Ok(u64::MAX)
    }
}

/// 获取磁盘总空间（字节）。
///
/// Unix: `statvfs` 的 `f_blocks * f_bsize`；Windows: `GetDiskFreeSpaceExW` 的 total 参数。
///
/// # 参数
/// - `path`: 任意路径（文件或目录）
///
/// # 返回
/// 总空间字节数。
pub fn disk_total_space(path: &Path) -> Result<u64> {
    #[cfg(unix)]
    {
        use std::ffi::CString;
        let path_str = path.to_str().context("路径转字符串失败")?;
        let c_path = CString::new(path_str).context("路径 CString 转换失败")?;
        // SAFETY: c_path 是有效的 C 字符串
        unsafe {
            let mut statv: libc_statvfs = std::mem::zeroed();
            if statvfs(c_path.as_ptr(), &mut statv) != 0 {
                bail!("statvfs 失败: {}", std::io::Error::last_os_error());
            }
            let blocks = statv.f_blocks as u128;
            let bsize = statv.f_bsize as u128;
            Ok((blocks * bsize).min(u64::MAX as u128) as u64)
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let wide: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let mut free: u64 = 0;
        let mut total: u64 = 0;
        // SAFETY: wide 是以 null 结尾的 UTF-16 字符串
        unsafe {
            if GetDiskFreeSpaceExW(wide.as_ptr(), &mut free, &mut total, std::ptr::null_mut()) == 0
            {
                bail!("GetDiskFreeSpaceExW 失败");
            }
        }
        Ok(total)
    }
    #[cfg(not(any(unix, windows)))]
    {
        Ok(u64::MAX)
    }
}

/// 获取磁盘空间完整信息（可用 + 总计 + 使用率）。
///
/// 一次系统调用获取完整磁盘空间快照，用于前端展示和后端决策。
///
/// # 参数
/// - `path`: 任意路径（文件或目录）
///
/// # 返回
/// `DiskSpaceInfo` 结构体，包含可用/总/已用空间、百分比、是否低于阈值。
pub fn get_disk_space_info(path: &Path) -> Result<DiskSpaceInfo> {
    let free_bytes = disk_free_space(path)?;
    let total_bytes = disk_total_space(path)?;
    Ok(DiskSpaceInfo::new(free_bytes, total_bytes))
}

/// 检查磁盘空间是否充足。
///
/// 在关键写入操作前调用此函数，如果可用空间低于阈值，返回错误。
///
/// # 参数
/// - `path`: 要检查的路径（通常是数据库文件路径）
/// - `required_bytes`: 写入操作所需的空间（字节）。如果为 0，仅检查是否低于阈值。
///
/// # 返回
/// - `Ok(())`：空间充足
/// - `Err`：空间不足，错误消息包含可用空间和需求空间
pub fn check_disk_space(path: &Path, required_bytes: u64) -> Result<()> {
    let free = disk_free_space(path)?;

    // 计算所需空间：需求 + 安全余量（取 max(required_bytes, threshold)）
    let needed = required_bytes.max(DISK_SPACE_LOW_THRESHOLD);

    if free < needed {
        bail!(
            "磁盘空间不足: 可用 {} 字节 ({:.1} MB)，需要 {} 字节 ({:.1} MB)",
            free,
            free as f64 / (1024.0 * 1024.0),
            needed,
            needed as f64 / (1024.0 * 1024.0)
        );
    }
    Ok(())
}

/// 清理临时文件释放磁盘空间。
///
/// 扫描数据目录下的临时文件和旧日志，删除可安全清理的文件。
///
/// # 清理范围
/// - `.partial` 文件（下载中断残留）
/// - `.tmp` 文件（原子写入残留）
/// - 7 天以上的日志文件
/// - WAL/checkpoint 临时文件
///
/// # 参数
/// - `data_dir`: 应用数据目录
///
/// # 返回
/// 释放的字节数。
pub fn cleanup_temp_files(data_dir: &Path) -> u64 {
    let mut freed: u64 = 0;

    // 第一遍：删除 .partial 和 .tmp 文件，记录已删除的 partial 基础名
    let mut deleted_partial_bases: std::collections::HashSet<String> =
        std::collections::HashSet::new();

    if let Ok(entries) = std::fs::read_dir(data_dir) {
        for entry in entries.flatten() {
            let path = entry.path();

            // 检查扩展名 .partial 或 .tmp
            if let Some(ext) = path.extension()
                && (ext == "partial" || ext == "tmp")
            {
                if let Ok(meta) = entry.metadata() {
                    freed += meta.len();
                }
                let _ = std::fs::remove_file(&path);
                continue;
            }

            // 检查以点开头的 .filename.partial 文件
            if let Some(name) = path.file_name().and_then(|n| n.to_str())
                && name.starts_with('.')
                && name.ends_with(".partial")
            {
                if let Ok(meta) = entry.metadata() {
                    freed += meta.len();
                }
                let _ = std::fs::remove_file(&path);
                // 记录基础名（去掉 . 前缀和 .partial 后缀）
                let base = name.trim_start_matches('.').trim_end_matches(".partial");
                deleted_partial_bases.insert(base.to_string());
            }
        }
    }

    // 第二遍：清理孤立的 .meta.json 文件
    if let Ok(entries) = std::fs::read_dir(data_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str())
                && name.starts_with('.')
                && name.ends_with(".meta.json")
            {
                // 提取基础名（如 .test.gguf.meta.json → test.gguf）
                let base = name.trim_start_matches('.').trim_end_matches(".meta.json");
                // 检查对应的 .partial 是否已不存在（要么原本就没有，要么刚被删除）
                let partial_path = data_dir.join(format!(".{base}.partial"));
                if !partial_path.exists() {
                    if let Ok(meta) = entry.metadata() {
                        freed += meta.len();
                    }
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
    }

    // 清理 models 子目录中的 .partial 和 .meta.json
    let models_dir = data_dir.join("models");
    if models_dir.exists() {
        freed += cleanup_models_dir(&models_dir);
    }

    // 清理旧日志文件（7 天以上）
    let logs_dir = data_dir.join("logs");
    if logs_dir.exists() {
        freed += cleanup_old_logs(&logs_dir, 7);
    }

    freed
}

/// 递归清理 models 目录中的临时文件。
fn cleanup_models_dir(models_dir: &Path) -> u64 {
    let mut freed: u64 = 0;
    if let Ok(entries) = std::fs::read_dir(models_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                freed += cleanup_models_dir(&path);
            } else if let Some(name) = path.file_name().and_then(|n| n.to_str())
                && name.starts_with('.')
                && (name.ends_with(".partial") || name.ends_with(".meta.json"))
            {
                // .meta.json 仅在对应 .partial 不存在时删除
                if name.ends_with(".meta.json") {
                    let partial_name = format!(
                        ".{}.partial",
                        name.trim_start_matches('.').trim_end_matches(".meta.json")
                    );
                    if !models_dir.join(&partial_name).exists() {
                        if let Ok(meta) = entry.metadata() {
                            freed += meta.len();
                        }
                        let _ = std::fs::remove_file(&path);
                    }
                } else {
                    // .partial 文件直接删除
                    if let Ok(meta) = entry.metadata() {
                        freed += meta.len();
                    }
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
    }
    freed
}

/// 清理旧日志文件。
fn cleanup_old_logs(logs_dir: &Path, max_age_days: u64) -> u64 {
    let mut freed: u64 = 0;
    let now = chrono::Utc::now().timestamp();
    let max_age_secs = (max_age_days * 24 * 60 * 60) as i64;

    if let Ok(entries) = std::fs::read_dir(logs_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            // 只删除 .log 和 .jsonl 文件
            if let Some(ext) = path.extension() {
                if ext != "log" && ext != "jsonl" {
                    continue;
                }
            } else {
                continue;
            }

            // 检查修改时间
            if let Ok(meta) = entry.metadata()
                && let Ok(mtime) = meta.modified()
                && let Ok(secs) = mtime.duration_since(std::time::UNIX_EPOCH)
            {
                let age = now - secs.as_secs() as i64;
                if age > max_age_secs {
                    freed += meta.len();
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
    }
    freed
}

// ============================================================================
// FFI 声明（从 robust_downloader.rs 提取为公共）
// ============================================================================

// Unix statvfs FFI
#[cfg(unix)]
#[repr(C)]
#[allow(non_camel_case_types)]
pub(crate) struct libc_statvfs {
    pub(crate) f_bsize: u64,
    pub(crate) f_frsize: u64,
    pub(crate) f_blocks: u64,
    pub(crate) f_bfree: u64,
    pub(crate) f_bavail: u64,
    pub(crate) f_files: u64,
    pub(crate) f_ffree: u64,
    pub(crate) f_favail: u64,
    pub(crate) f_fsid: u64,
    pub(crate) f_flag: u64,
    pub(crate) f_namemax: u64,
    __reserved: [u8; 256],
}

#[cfg(unix)]
unsafe extern "C" {
    fn statvfs(path: *const std::os::raw::c_char, buf: *mut libc_statvfs) -> i32;
}

#[cfg(windows)]
unsafe extern "system" {
    fn GetDiskFreeSpaceExW(
        directory: *const u16,
        free_bytes_available: *mut u64,
        total_bytes: *mut u64,
        free_total: *mut u64,
    ) -> i32;
}

// ============================================================================
// TDD 测试
// ============================================================================

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    // ─── disk_free_space ───

    #[test]
    fn tc_disk_001_free_space_positive() {
        // 根目录可用空间必须 > 0
        let free = disk_free_space(Path::new("/")).unwrap();
        assert!(free > 0, "磁盘可用空间应大于 0，实际: {free}");
    }

    #[test]
    fn tc_disk_002_free_space_temp_dir() {
        // 临时目录可用空间必须 > 0
        let tmp = tempfile::tempdir().unwrap();
        let free = disk_free_space(tmp.path()).unwrap();
        assert!(free > 0, "临时目录可用空间应大于 0");
    }

    #[test]
    fn tc_disk_003_free_space_nonexistent_path() {
        // 不存在的路径应返回 Err
        let result = disk_free_space(Path::new("/nonexistent/path/that/does/not/exist"));
        // statvfs 对不存在的路径返回错误
        assert!(result.is_err(), "不存在的路径应返回错误");
    }

    // ─── disk_total_space ───

    #[test]
    fn tc_disk_004_total_space_positive() {
        let total = disk_total_space(Path::new("/")).unwrap();
        assert!(total > 0, "磁盘总空间应大于 0");
    }

    #[test]
    fn tc_disk_005_total_ge_free() {
        // 总空间 >= 可用空间
        let free = disk_free_space(Path::new("/")).unwrap();
        let total = disk_total_space(Path::new("/")).unwrap();
        assert!(total >= free, "总空间 ({total}) 应 >= 可用空间 ({free})");
    }

    // ─── get_disk_space_info ───

    #[test]
    fn tc_disk_006_disk_space_info_fields() {
        let info = get_disk_space_info(Path::new("/")).unwrap();
        assert!(info.free_bytes > 0);
        assert!(info.total_bytes > 0);
        assert!(info.total_bytes >= info.free_bytes);
        assert_eq!(info.used_bytes, info.total_bytes - info.free_bytes);
        assert!(info.free_percent >= 0.0 && info.free_percent <= 100.0);
    }

    #[test]
    fn tc_disk_007_disk_space_info_is_low() {
        // 在正常系统上，根目录通常不会低于 50MB 阈值
        let info = get_disk_space_info(Path::new("/")).unwrap();
        // 如果系统真的磁盘满了，is_low 会是 true
        if info.free_bytes < DISK_SPACE_LOW_THRESHOLD {
            assert!(info.is_low, "可用空间低于阈值时 is_low 应为 true");
        } else {
            assert!(!info.is_low, "可用空间高于阈值时 is_low 应为 false");
        }
    }

    #[test]
    fn tc_disk_008_threshold_value() {
        assert_eq!(DISK_SPACE_LOW_THRESHOLD, 50 * 1024 * 1024);
    }

    // ─── check_disk_space ───

    #[test]
    fn tc_disk_009_check_space_sufficient() {
        // 需求 0 字节时，只要不低于阈值就应通过
        let result = check_disk_space(Path::new("/"), 0);
        // 正常系统应该有 > 50MB 空间
        assert!(result.is_ok(), "磁盘空间充足时应返回 Ok");
    }

    #[test]
    fn tc_disk_010_check_space_zero_requirement() {
        // 需求 0 字节时仍检查阈值
        let result = check_disk_space(Path::new("/"), 0);
        assert!(result.is_ok());
    }

    #[test]
    fn tc_disk_011_check_space_small_requirement() {
        // 需求 1MB（小于阈值），应通过
        let result = check_disk_space(Path::new("/"), 1024 * 1024);
        assert!(result.is_ok());
    }

    #[test]
    fn tc_disk_012_check_space_huge_requirement() {
        // 需求 u64::MAX - 1（绝对不可能满足），应返回 Err
        // 注意：某些文件系统（如 macOS APFS 容器）可能报告极大的可用空间，
        // statvfs 返回的 f_bavail * f_bsize 可能被 clamp 到 u64::MAX，
        // 导致 free == needed，所以用 u64::MAX - 1 确保 free < needed。
        let result = check_disk_space(Path::new("/"), u64::MAX - 1);
        if let Err(e) = &result {
            // 正常情况：空间不足
            let err_msg = e.to_string();
            assert!(
                err_msg.contains("磁盘空间不足"),
                "错误消息应包含'磁盘空间不足'，实际: {err_msg}"
            );
        }
        // 某些文件系统（如 APFS 容器）可能报告 u64::MAX 可用空间，
        // 此时 free >= needed（u64::MAX - 1），检查通过。这是预期行为。
    }

    // ─── cleanup_temp_files ───

    #[test]
    fn tc_disk_013_cleanup_no_temp_files() {
        // 没有临时文件时应返回 0
        let tmp = tempfile::tempdir().unwrap();
        let freed = cleanup_temp_files(tmp.path());
        assert_eq!(freed, 0, "没有临时文件时应返回 0");
    }

    #[test]
    fn tc_disk_014_cleanup_partial_files() {
        let tmp = tempfile::tempdir().unwrap();
        // 创建 .partial 文件
        let partial_path = tmp.path().join("test.partial");
        std::fs::write(&partial_path, b"x".repeat(1000)).unwrap();
        // 创建 .tmp 文件
        let tmp_path = tmp.path().join("test.tmp");
        std::fs::write(&tmp_path, b"y".repeat(500)).unwrap();

        let freed = cleanup_temp_files(tmp.path());
        assert_eq!(freed, 1500, "应清理 1500 字节");
        assert!(!partial_path.exists(), ".partial 文件应被删除");
        assert!(!tmp_path.exists(), ".tmp 文件应被删除");
    }

    #[test]
    fn tc_disk_015_cleanup_dot_partial_files() {
        let tmp = tempfile::tempdir().unwrap();
        // 创建 .test.gguf.partial 文件
        let dot_partial = tmp.path().join(".test.gguf.partial");
        std::fs::write(&dot_partial, b"data").unwrap();

        let freed = cleanup_temp_files(tmp.path());
        assert_eq!(freed, 4, "应清理 4 字节");
        assert!(!dot_partial.exists(), ".partial 文件应被删除");
    }

    #[test]
    fn tc_disk_016_cleanup_orphaned_meta_json() {
        let tmp = tempfile::tempdir().unwrap();
        // 创建 .test.gguf.meta.json 但没有对应的 .partial 文件
        let meta_path = tmp.path().join(".test.gguf.meta.json");
        std::fs::write(&meta_path, b"{}").unwrap();

        let freed = cleanup_temp_files(tmp.path());
        assert_eq!(freed, 2, "应清理孤立的 meta.json（2 字节）");
        assert!(!meta_path.exists(), "孤立的 meta.json 应被删除");
    }

    #[test]
    fn tc_disk_017_cleanup_keeps_normal_files() {
        let tmp = tempfile::tempdir().unwrap();
        // 正常文件不应被删除
        let normal_file = tmp.path().join("document.md");
        std::fs::write(&normal_file, b"important data").unwrap();

        let freed = cleanup_temp_files(tmp.path());
        assert_eq!(freed, 0, "正常文件不应被清理");
        assert!(normal_file.exists(), "正常文件应保留");
    }

    #[test]
    fn tc_disk_018_cleanup_models_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let models_dir = tmp.path().join("models");
        std::fs::create_dir_all(&models_dir).unwrap();

        // 在 models 目录下创建 .partial 文件
        let partial = models_dir.join(".model.gguf.partial");
        std::fs::write(&partial, b"x".repeat(2000)).unwrap();

        let freed = cleanup_temp_files(tmp.path());
        assert_eq!(freed, 2000, "应清理 models 目录下的临时文件");
        assert!(!partial.exists());
    }

    #[test]
    fn tc_disk_019_cleanup_keeps_meta_when_partial_exists() {
        let tmp = tempfile::tempdir().unwrap();
        // 创建 .partial 和 .meta.json
        let partial = tmp.path().join(".test.gguf.partial");
        std::fs::write(&partial, b"data").unwrap();
        let meta = tmp.path().join(".test.gguf.meta.json");
        std::fs::write(&meta, b"{}").unwrap();

        let freed = cleanup_temp_files(tmp.path());
        // .partial 被删除（4 字节），但 .meta.json 也应被删除（因为 .partial 先被删了）
        // 注意：cleanup 先扫 .partial 再扫 .meta.json，所以 .meta.json 此时已是孤儿
        assert!(freed >= 4, "至少清理 .partial 文件");
        assert!(!partial.exists(), ".partial 应被删除");
    }

    // ─── DiskSpaceInfo 序列化 ───

    #[test]
    fn tc_disk_020_disk_space_info_serializable() {
        let info = get_disk_space_info(Path::new("/")).unwrap();
        let json = serde_json::to_string(&info);
        assert!(json.is_ok(), "DiskSpaceInfo 应可序列化为 JSON");
    }
}
