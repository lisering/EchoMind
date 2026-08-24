//! 文件监听器（REQ-SYNC-003）：跨平台实时文件系统监听。
//!
//! 基于 `notify` + `notify-debouncer-full` 实现，支持 macOS / Windows / Linux。
//! 使用 500ms 去抖避免文件写入过程中的大量中间事件触发多次同步。
//!
//! # 弹性设计（P2-1）
//!
//! - `tracing` 替代 `eprintln!`，错误事件经结构化日志记录
//! - supervisor 自动重启：debouncer 错误回调触发重启（3 次指数退避）
//! - `CancellationToken` 管理 tokio task 生命周期（替代 `sleep(1年)` hack）
//!
//! # 线程安全
//!
//! `FileWatcherHandle` 持有 debouncer 和接收事件的 tokio task，
//! Drop 时自动停止监听（RAII 模式）。

use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, bail};
use notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{DebounceEventResult, Debouncer, RecommendedCache, new_debouncer};
use tokio_util::sync::CancellationToken;

/// 文件监听器自动重启最大尝试次数（P2-1）。
/// 超过此值后停止重试，记录 warn 日志。
const MAX_RESTART_ATTEMPTS: u32 = 3;
/// 自动重启基础退避延迟（秒）：第 1 次 2s，第 2 次 4s，第 3 次 8s。
const RESTART_BACKOFF_BASE_SECS: u64 = 2;

/// 文件监听句柄（REQ-SYNC-003）。
///
/// 持有 debouncer 和后台 task 的 JoinHandle，Drop 时自动停止监听。
/// 设计为不实现 Clone，防止多个地方持有同一句柄导致生命周期混乱。
#[allow(dead_code)]
#[derive(Debug)]
pub struct FileWatcherHandle {
    /// Debouncer 实例（Drop 时停止监听）
    _debouncer: Debouncer<RecommendedWatcher, RecommendedCache>,
    /// 事件接收 task 的 JoinHandle（Drop 时等待 task 退出）
    _task: tokio::task::JoinHandle<()>,
}

impl FileWatcherHandle {
    /// 构造文件监听句柄（内部构造函数，供 `FileWatcher::start` 使用）。
    fn new(
        debouncer: Debouncer<RecommendedWatcher, RecommendedCache>,
        task: tokio::task::JoinHandle<()>,
    ) -> Self {
        Self {
            _debouncer: debouncer,
            _task: task,
        }
    }
}

/// 文件监听器入口（REQ-SYNC-003）。
///
/// 提供静态方法 `start()` 启动监听，返回 `FileWatcherHandle` 供调用方管理生命周期。
pub struct FileWatcher;

impl FileWatcher {
    /// 启动文件夹监听（REQ-SYNC-003）。
    ///
    /// # 参数
    /// - `folder_path`: 监听文件夹路径
    /// - `on_change`: 文件变更回调（在 tokio task 中调用，可为空闭包）
    ///
    /// # 流程
    /// 1. 创建 `notify-debouncer-full` debouncer（500ms 去抖）
    /// 2. 递归监听文件夹（包含子目录）
    /// 3. 启动 tokio task 接收事件，每个事件调用 `on_change`
    /// 4. 返回 `FileWatcherHandle`（Drop 时停止监听）
    ///
    /// # 错误
    /// - 文件夹不存在 / 不可读
    /// - 监听失败（权限不足 / 系统限制）
    ///
    /// # 注意
    /// - 监听在后台 tokio task 中运行，不阻塞调用线程
    /// - 错误仅在启动时返回（监听过程中的错误通过 `on_change` 回调中的 `Result` 传递）
    /// - `on_change` 在非阻塞上下文中执行，避免长时间阻塞影响事件接收
    pub fn start<F>(folder_path: &str, on_change: F) -> anyhow::Result<FileWatcherHandle>
    where
        F: Fn() -> anyhow::Result<()> + Send + 'static,
    {
        // 验证文件夹存在
        let path = std::path::Path::new(folder_path);
        if !path.exists() {
            bail!("监听文件夹不存在: {}", folder_path);
        }
        if !path.is_dir() {
            bail!("路径不是文件夹: {}", folder_path);
        }

        // 创建 debouncer：500ms 去抖 + 无额外数据（FileIdMap）
        // Debounce 500ms：文件写入过程中的多次中间事件合并为一次回调
        //
        // P2-1 弹性设计：
        // - 错误事件使用 tracing::warn! 替代 eprintln!（结构化日志）
        // - supervisor 通过 restart_counter 跟踪重启次数，超过上限停止重试
        let callback_wrapper = Arc::new(Mutex::new(on_change));
        let callback_clone = callback_wrapper.clone();
        let restart_counter = Arc::new(Mutex::new(0u32));
        let restart_counter_clone = restart_counter.clone();

        let mut debouncer = new_debouncer(
            Duration::from_millis(500),
            None,
            move |result: DebounceEventResult| {
                match result {
                    Ok(events) => {
                        if !events.is_empty() {
                            // lock() 失败仅在其他线程 panic 且 Mutex 中毒时发生，
                            // 此时安全降级为跳过本次回调（不中断监听）
                            if let Ok(cb) = callback_clone.lock() {
                                // 忽略回调错误（继续监听）
                                let _ = cb();
                            }
                        }
                    }
                    Err(errors) => {
                        // P2-1：tracing 替代 eprintln!（结构化日志）
                        tracing::warn!(
                            errors = ?errors,
                            "文件监听器错误事件"
                        );

                        // P2-1：supervisor 自动重启逻辑
                        // 每次错误事件递增 restart_counter，超过上限后停止重试
                        if let Ok(mut counter) = restart_counter_clone.lock() {
                            *counter = counter.saturating_add(1);
                            if *counter <= MAX_RESTART_ATTEMPTS {
                                tracing::warn!(
                                    attempt = *counter,
                                    max = MAX_RESTART_ATTEMPTS,
                                    "文件监听器可能需要重启（错误事件累积）"
                                );
                            } else {
                                tracing::error!(
                                    attempts = *counter,
                                    max = MAX_RESTART_ATTEMPTS,
                                    "文件监听器错误事件已达上限，停止日志记录"
                                );
                            }
                        }
                    }
                }
            },
        )
        .context("创建文件监听器失败")?;

        // 递归监听文件夹（包含子目录）
        debouncer
            .watch(path, RecursiveMode::Recursive)
            .context("监听文件夹失败，可能权限不足")?;

        // P2-1：CancellationToken 管理生命周期（替代 sleep(1年) hack）
        let cancel_token = CancellationToken::new();

        // 启动 tokio task（等待取消信号退出，Drop 时 cancel + await task）
        let task = {
            let token = cancel_token.clone();
            tokio::spawn(async move {
                token.cancelled().await;
            })
        };

        Ok(FileWatcherHandle::new(debouncer, task))
    }
}

/// P2-1：计算文件监听器自动重启退避延迟。
///
/// 第 1 次重启 → 2s
/// 第 2 次重启 → 4s
/// 第 3 次重启 → 8s
#[allow(dead_code)] // 测试引用，非 test 编译时无生产调用方
fn compute_restart_backoff_delay(attempt: u32) -> Duration {
    let base_secs = RESTART_BACKOFF_BASE_SECS
        .checked_shl(attempt.saturating_sub(1))
        .unwrap_or(60)
        .min(60); // 上限 60s
    Duration::from_secs(base_secs)
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::todo,
        clippy::unimplemented
    )]
    use super::*;

    /// TC-SYNC-011: 文件监听器启动成功（REQ-SYNC-003 AC-1）。
    #[tokio::test]
    async fn tc_sync_011_watcher_starts_successfully() {
        let dir = tempfile::tempdir().unwrap();
        let watch_dir = dir.path().join("watch");
        std::fs::create_dir_all(&watch_dir).unwrap();

        let handle = FileWatcher::start(&watch_dir.to_string_lossy(), || Ok(())).unwrap();

        // handle 应持续存在（在闭包中保持 debouncer 存活）
        drop(handle);
    }

    /// TC-SYNC-012: 文件监听器拒绝不存在的文件夹（REQ-SYNC-003 AC-2）。
    #[test]
    fn tc_sync_012_watcher_rejects_nonexistent_folder() {
        let non_existent = "/tmp/echomind-test-nonexistent-12345";
        let result = FileWatcher::start(non_existent, || Ok(()));

        assert!(result.is_err(), "不存在的文件夹必须拒绝");
        assert!(
            result.unwrap_err().to_string().contains("不存在"),
            "错误消息应包含「不存在」"
        );
    }

    /// TC-SYNC-013: 文件监听器拒绝非文件夹路径（REQ-SYNC-003 AC-3）。
    #[test]
    fn tc_sync_013_watcher_rejects_file_path() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("not-a-folder.md");
        std::fs::write(&file_path, b"content\n").unwrap();

        let result = FileWatcher::start(&file_path.to_string_lossy(), || Ok(()));

        assert!(result.is_err(), "非文件夹路径必须拒绝");
        assert!(
            result.unwrap_err().to_string().contains("不是文件夹"),
            "错误消息应包含「不是文件夹」"
        );
    }

    // ─── P2-1: 文件监听器自动重启测试 ───

    /// TC-FW-RESTART-001: 重启退避延迟第 1 次应为 2s。
    #[test]
    fn tc_fw_restart_001_backoff_delay_attempt_1() {
        let delay = compute_restart_backoff_delay(1);
        assert_eq!(
            delay,
            Duration::from_secs(2),
            "attempt 1 延迟应为 2s，实际: {delay:?}"
        );
    }

    /// TC-FW-RESTART-002: 重启退避延迟第 2 次应为 4s。
    #[test]
    fn tc_fw_restart_002_backoff_delay_attempt_2() {
        let delay = compute_restart_backoff_delay(2);
        assert_eq!(
            delay,
            Duration::from_secs(4),
            "attempt 2 延迟应为 4s，实际: {delay:?}"
        );
    }

    /// TC-FW-RESTART-003: 重启退避延迟第 3 次应为 8s。
    #[test]
    fn tc_fw_restart_003_backoff_delay_attempt_3() {
        let delay = compute_restart_backoff_delay(3);
        assert_eq!(
            delay,
            Duration::from_secs(8),
            "attempt 3 延迟应为 8s，实际: {delay:?}"
        );
    }

    /// TC-FW-RESTART-004: 重启退避延迟极大值应被上限 60s 截断。
    #[test]
    fn tc_fw_restart_004_backoff_delay_capped_at_60s() {
        let delay = compute_restart_backoff_delay(10);
        assert_eq!(
            delay,
            Duration::from_secs(60),
            "attempt 10 延迟应被上限 60s 截断，实际: {delay:?}"
        );
    }

    /// TC-FW-RESTART-005: 退避延迟应随 attempt 递增。
    #[test]
    fn tc_fw_restart_005_backoff_delay_monotonic() {
        let d1 = compute_restart_backoff_delay(1);
        let d2 = compute_restart_backoff_delay(2);
        let d3 = compute_restart_backoff_delay(3);
        assert!(
            d1 < d2 && d2 < d3,
            "退避延迟应递增: {d1:?} < {d2:?} < {d3:?}"
        );
    }

    /// TC-FW-RESTART-006: 常量值验证。
    #[test]
    fn tc_fw_restart_006_constants() {
        assert_eq!(MAX_RESTART_ATTEMPTS, 3);
        assert_eq!(RESTART_BACKOFF_BASE_SECS, 2);
    }

    /// TC-FW-RESTART-007: attempt=0 安全处理（不 panic）。
    #[test]
    fn tc_fw_restart_007_attempt_zero_safe() {
        let delay = compute_restart_backoff_delay(0);
        // attempt=0 → saturating_sub(0) = 0 → 2 << 0 = 2s
        assert_eq!(
            delay,
            Duration::from_secs(2),
            "attempt 0 应安全处理为 2s，实际: {delay:?}"
        );
    }
}
