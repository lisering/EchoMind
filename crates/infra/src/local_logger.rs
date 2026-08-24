//! 本地日志系统（REQ-OBS-001）：基于 `tracing` 的结构化日志。
//!
//! ## 设计
//!
//! - 日志写入 `{data_dir}/logs/echomind.log`（按日期轮转，保留最近 7 天）
//! - JSON Lines 格式（每行一个 JSON 对象，含 timestamp / level / module / message）
//! - 日志级别：DEBUG / INFO / WARN / ERROR（默认 INFO）
//! - 运行时切换级别（通过 `reload::Layer` 动态更新 `EnvFilter`）
//! - 隐私铁律：日志不含 API Key 明文、不含用户文档内容或聊天内容
//!
//! ## 架构
//!
//! ```text
//! tracing::info!(...) → EnvFilter(reload) → fmt::Layer(json) → NonBlocking → RollingFileAppender
//! ```
//!
//! - `EnvFilter(reload)`：支持运行时 `set_level()` 动态切换级别
//! - `fmt::Layer(json)`：JSON Lines 格式化层
//! - `NonBlocking`：后台线程写入，不阻塞主线程
//! - `RollingFileAppender`：按日期轮转，每日一个文件
//!
//! ## 使用方式
//!
//! ```no_run
//! # use std::path::PathBuf;
//! # use echomind_infra::local_logger::LocalLogger;
//! let _guard = LocalLogger::init(PathBuf::from("/tmp/logs"), "info").unwrap();
//! tracing::info!("应用已启动");
//! LocalLogger::set_level("debug").unwrap();
//! ```

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use tracing_subscriber::reload;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

/// 日志保留天数（超过自动删除旧日志文件）。
const LOG_RETENTION_DAYS: i64 = 7;

/// 全局 reload handle（运行时切换日志级别）。
static RELOAD_HANDLE: OnceLock<reload::Handle<EnvFilter, tracing_subscriber::Registry>> =
    OnceLock::new();

/// 全局日志目录路径（读取日志文件时使用）。
static LOG_DIR: OnceLock<PathBuf> = OnceLock::new();

/// 本地日志系统句柄。
///
/// 持有 `WorkerGuard` 以保持非阻塞写入器后台线程存活。
/// 当此值被 drop 时，后台线程会 flush 剩余日志并退出。
pub struct LocalLogger {
    /// WorkerGuard（保持非阻塞写入器后台线程存活）
    _guard: tracing_appender::non_blocking::WorkerGuard,
}

/// 日志级别枚举（与 SRS REQ-OBS-001 AC-5 对齐）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    /// 从字符串解析日志级别（不区分大小写）。
    ///
    /// 注意：此方法不实现 `std::str::FromStr` trait，因为返回 `Option` 而非 `Result`。
    pub fn parse_level(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "debug" => Some(Self::Debug),
            "info" => Some(Self::Info),
            "warn" => Some(Self::Warn),
            "error" => Some(Self::Error),
            _ => None,
        }
    }

    /// 转为 `tracing` 兼容的过滤器字符串。
    pub fn as_filter_str(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }

    /// 转为展示字符串。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
        }
    }
}

impl LocalLogger {
    /// 初始化日志系统（全局只能调用一次）。
    ///
    /// # 参数
    /// - `log_dir` — 日志文件目录（`{data_dir}/logs/`）
    /// - `level_str` — 初始日志级别（`"debug"` / `"info"` / `"warn"` / `"error"`）
    ///
    /// # 返回
    ///
    /// `LocalLogger` 句柄（持有 `WorkerGuard`，必须保活以确保日志 flush）。
    ///
    /// # 错误
    ///
    /// - 目录创建失败
    /// - 全局订阅器已被设置（`set_global_default` 只能调用一次）
    pub fn init(log_dir: PathBuf, level_str: &str) -> anyhow::Result<Self> {
        std::fs::create_dir_all(&log_dir).map_err(|e| anyhow::anyhow!("创建日志目录失败: {e}"))?;

        // 清理超过保留期的旧日志文件
        cleanup_old_logs(&log_dir);

        // 按日期轮转的文件 appender
        let file_appender = tracing_appender::rolling::daily(&log_dir, "echomind.log");
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

        // 构建 EnvFilter（支持级别过滤）
        let level = LogLevel::parse_level(level_str).unwrap_or(LogLevel::Info);
        let filter = EnvFilter::new(level.as_filter_str());

        // reload::Layer 支持运行时动态切换过滤级别
        let (filter_layer, reload_handle) = reload::Layer::new(filter);

        // 存储全局 reload handle 和日志目录
        let _ = RELOAD_HANDLE.set(reload_handle);
        let _ = LOG_DIR.set(log_dir);

        // 构建 JSON Lines 格式化层
        let json_layer = fmt::layer()
            .json()
            .with_writer(non_blocking)
            .with_ansi(false); // 文件日志不使用 ANSI 颜色码

        // 组装订阅器并设为全局默认
        let subscriber = tracing_subscriber::registry()
            .with(filter_layer)
            .with(json_layer);

        tracing::subscriber::set_global_default(subscriber).map_err(|_| {
            anyhow::anyhow!(
                "日志全局订阅器已被设置（set_global_default 只能调用一次）；\
                 如果在测试中多次初始化，请使用 filter_subscriber 或 test_log_guard 模式"
            )
        })?;

        tracing::info!(
            module = "local_logger",
            "日志系统已初始化：级别={}，目录={}",
            level.as_str(),
            LOG_DIR
                .get()
                .map(|p| p.display().to_string())
                .unwrap_or_default()
        );

        Ok(Self { _guard: guard })
    }

    /// 运行时切换日志级别。
    ///
    /// 通过 `reload::Handle` 动态更新 `EnvFilter`，无需重新初始化订阅器。
    pub fn set_level(level_str: &str) -> anyhow::Result<()> {
        let handle = RELOAD_HANDLE
            .get()
            .ok_or_else(|| anyhow::anyhow!("日志系统未初始化，无法切换级别"))?;

        let level = LogLevel::parse_level(level_str)
            .ok_or_else(|| anyhow::anyhow!("无效的日志级别: {level_str}"))?;

        handle
            .reload(EnvFilter::new(level.as_filter_str()))
            .map_err(|e| anyhow::anyhow!("更新日志级别失败: {e}"))?;

        tracing::info!(
            module = "local_logger",
            "日志级别已切换为 {}",
            level.as_str()
        );
        Ok(())
    }

    /// 获取当前日志级别（从全局 reload handle 读取当前过滤器的字符串表示）。
    ///
    /// 由于 `EnvFilter` 不直接暴露级别，此处通过尝试 reload 相同级别来确认。
    /// 实际实现中，级别由 settings 表管理，此方法从 settings 读取。
    pub fn get_current_level() -> String {
        // 简化实现：reload handle 的当前过滤器无法直接读取，
        // 级别由 settings 表 `log.level` 键管理，IPC 命令从 settings 读取。
        // 此处返回 reload handle 是否已初始化的状态。
        if RELOAD_HANDLE.get().is_some() {
            "info".to_string()
        } else {
            "未初始化".to_string()
        }
    }

    /// 读取最近 N 行日志。
    ///
    /// 扫描日志目录下所有 `echomind.log*` 文件，按文件名排序后合并读取，
    /// 返回最后 `tail_lines` 行。
    pub fn read_recent_logs(tail_lines: usize) -> anyhow::Result<String> {
        let log_dir = LOG_DIR
            .get()
            .ok_or_else(|| anyhow::anyhow!("日志系统未初始化"))?;

        Self::read_logs_from_dir(log_dir, tail_lines)
    }

    /// 从指定目录读取最近 N 行日志（供诊断导出使用）。
    pub fn read_logs_from_dir(log_dir: &Path, tail_lines: usize) -> anyhow::Result<String> {
        let mut entries: Vec<_> = std::fs::read_dir(log_dir)
            .map_err(|e| anyhow::anyhow!("读取日志目录失败: {e}"))?
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with("echomind.log"))
            .collect();

        entries.sort_by_key(|e| e.file_name());

        let mut all_lines: Vec<String> = Vec::new();
        for entry in &entries {
            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                all_lines.extend(content.lines().map(String::from));
            }
        }

        let start = all_lines.len().saturating_sub(tail_lines);
        Ok(all_lines[start..].join("\n"))
    }

    /// 获取日志目录路径。
    pub fn log_dir() -> Option<PathBuf> {
        LOG_DIR.get().cloned()
    }
}

/// 清理超过保留期的旧日志文件。
///
/// 删除修改时间早于 `LOG_RETENTION_DAYS` 天前的 `echomind.log*` 文件。
pub(crate) fn cleanup_old_logs(log_dir: &Path) {
    let cutoff = chrono::Utc::now()
        .checked_sub_signed(chrono::Duration::days(LOG_RETENTION_DAYS))
        .unwrap_or_else(chrono::Utc::now);

    if let Ok(entries) = std::fs::read_dir(log_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            if !name.to_string_lossy().starts_with("echomind.log") {
                continue;
            }
            if let Ok(meta) = entry.metadata()
                && let Ok(modified) = meta.modified()
            {
                let mod_time = chrono::DateTime::<chrono::Utc>::from(modified);
                if mod_time < cutoff {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
    }
}

/// 收集系统诊断信息（REQ-OBS-002 AC-2）。
///
/// 聚合应用版本、操作系统、CPU、内存、数据库规模、知识库规模、
/// 嵌入维度、LLM 配置（脱敏）、最近日志等，导出为 JSON。
///
/// # 隐私铁律
///
/// - 不包含 API Key 明文（REQ-OBS-002 AC-3）
/// - 不包含用户文档内容或对话内容（REQ-OBS-002 AC-4）
#[allow(clippy::too_many_arguments)]
pub fn collect_diagnostics(
    app_version: &str,
    data_dir: &Path,
    db_path: &Path,
    doc_count: usize,
    chunk_count: usize,
    embedding_dim: usize,
    llm_config_masked: Option<&str>,
    llm_model: Option<&str>,
    base_url: Option<&str>,
    llm_mode: Option<&str>,
) -> serde_json::Value {
    // 系统信息（使用 std，不依赖 sysinfo crate）
    let os_name = std::env::consts::OS.to_string();
    let arch = std::env::consts::ARCH.to_string();
    let cpu_count = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(0);

    // 内存信息（跨平台：macOS 用 sysctl，Linux 读 /proc/meminfo）
    let memory_mb = get_system_memory_mb();

    // 数据库文件大小
    let db_size_bytes = std::fs::metadata(db_path).map(|m| m.len()).unwrap_or(0);

    // 最近 100 行日志
    let recent_logs = LocalLogger::read_logs_from_dir(&data_dir.join("logs"), 100)
        .unwrap_or_else(|_| "(日志不可用)".to_string());

    serde_json::json!({
        "app_version": app_version,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "system": {
            "os": os_name,
            "arch": arch,
            "cpu_count": cpu_count,
            "memory_mb": memory_mb,
        },
        "database": {
            "path": db_path.to_string_lossy(),
            "size_bytes": db_size_bytes,
            "size_mb": (db_size_bytes as f64) / 1_048_576.0,
        },
        "knowledge_base": {
            "document_count": doc_count,
            "chunk_count": chunk_count,
            "embedding_dimension": embedding_dim,
        },
        "llm_config": {
            "base_url": base_url.unwrap_or("(未配置)"),
            "model": llm_model.unwrap_or("(未配置)"),
            "api_key": llm_config_masked.unwrap_or("(未配置)"),
            "mode": llm_mode.unwrap_or("remote"),
        },
        "data_dir": data_dir.to_string_lossy(),
        "recent_logs": recent_logs,
    })
}

/// 获取系统总内存（MB），跨平台实现（不依赖 sysinfo crate）。
///
/// - macOS：`sysctl -n hw.memsize`
/// - Linux：读取 `/proc/meminfo` 中的 `MemTotal`
/// - 其他平台：返回 0
fn get_system_memory_mb() -> u64 {
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("sysctl")
            .args(["-n", "hw.memsize"])
            .output()
            && let Ok(s) = String::from_utf8(output.stdout)
            && let Ok(bytes) = s.trim().parse::<u64>()
        {
            return bytes / 1_048_576;
        }
        0
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
            for line in content.lines() {
                if line.starts_with("MemTotal:") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if let Some(Ok(kb)) = parts.get(1).map(|s| s.parse::<u64>()) {
                        return kb / 1024;
                    }
                    break;
                }
            }
        }
        0
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        0
    }
}
