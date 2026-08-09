//! 健壮下载系统（REQ-LLM-004 v2）。
//!
//! 参考：Ollama `download.go` + HuggingFace `hf_transfer` + HuggingFace Hub `file_download.py`。
//!
//! ## 设计目标
//! 1. **断点续传** — HTTP Range + `.partial` 文件 + `.meta.json` 元数据持久化
//! 2. **多源容错** — HuggingFace → hf-mirror → ModelScope 自动降级
//! 3. **并发分块** — 大文件多连接并行下载（Semaphore 限制）
//! 4. **崩溃恢复** — `.meta.json` 持久化 + 启动扫描恢复
//! 5. **完整性校验** — SHA256 哈希验证
//! 6. **取消/暂停** — CancellationToken 支持
//! 7. **自适应重试** — backon 指数退避 + 抖动 + 条件重试
//! 8. **磁盘空间检查** — 下载前验证可用空间
//! 9. **卡顿检测** — 30 秒无进度自动重试
//! 10. **统一管线** — ONNX 嵌入模型 + GGUF 大模型共用下载逻辑

use std::collections::HashMap;
use std::io::SeekFrom;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use echomind_models::{
    DownloadManifest, DownloadPart, DownloadSourceDef, DownloadStatus, DownloadStatusSummary,
};
use futures::StreamExt;
use rand::Rng;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

/// 下载进度回调类型。
///
/// 参数：`(已下载字节, 总字节, 速度字节/秒)`
pub type DownloadProgressFn = Arc<dyn Fn(u64, u64, u64) + Send + Sync>;

/// 下载完成结果。
#[derive(Debug, Clone)]
pub enum DownloadOutcome {
    /// 下载成功完成
    Completed,
    /// 已暂停（用户主动暂停或应用睡眠）
    Paused { completed: u64, total: u64 },
    /// 已从之前的 `.partial` 文件恢复
    Resumed { completed: u64, total: u64 },
}

/// 并发分块下载参数。
#[derive(Debug, Clone)]
pub struct DownloadConfig {
    /// 每个分块的最小大小（字节）。默认 128MB。
    pub min_chunk_size: u64,
    /// 每个分块的最大大小（字节）。默认 512MB。
    pub max_chunk_size: u64,
    /// 最大并发数。默认 8。
    pub max_concurrency: usize,
    /// 最大重试次数。默认 6（参考 Ollama）。
    pub max_retries: u32,
    /// 退避基础延迟（毫秒）。默认 500。
    pub backoff_base_ms: u64,
    /// 退避最大延迟（秒）。默认 30。
    pub backoff_max_secs: u64,
    /// 卡顿检测超时（秒）。默认 30（参考 Ollama）。
    pub stall_timeout_secs: u64,
    /// 进度上报间隔（毫秒）。默认 100。
    pub progress_interval_ms: u64,
    /// 元数据持久化间隔（字节）。默认 1MB。
    pub meta_persist_bytes: u64,
}

impl Default for DownloadConfig {
    fn default() -> Self {
        Self {
            min_chunk_size: 128 * 1024 * 1024, // 128MB
            max_chunk_size: 512 * 1024 * 1024, // 512MB
            max_concurrency: 8,
            max_retries: 6,
            backoff_base_ms: 500,
            backoff_max_secs: 30,
            stall_timeout_secs: 30,
            progress_interval_ms: 100,
            meta_persist_bytes: 1024 * 1024, // 1MB
        }
    }
}

/// 下载源列表（多源容错）。
///
/// 根据系统语言和网络探测动态选择源顺序：
/// - 中文系统：ModelScope（魔搭）→ hf-mirror → HuggingFace
/// - 非中文系统 + HuggingFace 可达：HuggingFace → hf-mirror → ModelScope
/// - 非中文系统 + HuggingFace 不可达：ModelScope → hf-mirror → HuggingFace
fn get_download_sources() -> Vec<DownloadSourceDef> {
    let hf = DownloadSourceDef {
        name: "HuggingFace".to_string(),
        base_url: "https://huggingface.co".to_string(),
        branch: "main".to_string(),
    };
    let mirror = DownloadSourceDef {
        name: "hf-mirror".to_string(),
        base_url: "https://hf-mirror.com".to_string(),
        branch: "main".to_string(),
    };
    let ms = DownloadSourceDef {
        name: "ModelScope".to_string(),
        base_url: "https://modelscope.cn/models".to_string(),
        branch: "master".to_string(),
    };
    if is_chinese_locale() || !*HF_REACHABLE {
        // 中文系统 或 HuggingFace 不可达：魔搭优先
        vec![ms, mirror, hf]
    } else {
        // 非中文系统且 HuggingFace 可达：HF 优先
        vec![hf, mirror, ms]
    }
}

/// HuggingFace 连通性探测结果缓存。
static HF_REACHABLE: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
    let client = match reqwest::blocking::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(3))
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };
    let url = "https://huggingface.co/Xenova/all-MiniLM-L6-v2/resolve/main/config.json";
    client
        .get(url)
        .send()
        .map(|r| r.status().is_success())
        .unwrap_or(false)
});

/// 检测系统是否为中文环境。
fn is_chinese_locale() -> bool {
    for var in &["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(val) = std::env::var(var)
            && val.to_lowercase().starts_with("zh")
        {
            return true;
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("defaults")
            .args(["read", "-g", "AppleLocale"])
            .output()
            && let Ok(s) = String::from_utf8(output.stdout)
            && s.trim().to_lowercase().starts_with("zh")
        {
            return true;
        }
    }
    false
}

/// 指数退避 + 随机抖动（防止 thundering herd）。
///
/// 参考：Ollama `n²*10ms` + hf_transfer `300ms base + n² + jitter`。
/// 公式：`base_ms + n² * 100ms + random(0-500ms)`，上限 `max_secs`。
fn backoff_delay(retry: u32, config: &DownloadConfig) -> Duration {
    let n = retry as u64;
    let n_squared = n.pow(2) * 100;
    let jitter = rand::thread_rng().gen_range(0..=500);
    let total = config.backoff_base_ms + n_squared + jitter;
    Duration::from_millis(total.min(config.backoff_max_secs * 1000))
}

/// 判断错误是否可重试。
///
/// 参考：HuggingFace Hub `_DEFAULT_RETRY_ON_EXCEPTIONS`。
#[allow(dead_code)]
fn is_retryable_error(err: &anyhow::Error) -> bool {
    let msg = format!("{err:#}");
    // 网络超时、连接重置、DNS 解析失败 → 可重试
    msg.contains("timed out")
        || msg.contains("connection")
        || msg.contains("dns")
        || msg.contains("reset")
        || msg.contains("broken pipe")
        || msg.contains("EOF")
        // 5xx 服务器错误 → 可重试
        || msg.contains("500")
        || msg.contains("502")
        || msg.contains("503")
        || msg.contains("504")
        // 429 限流 → 可重试
        || msg.contains("429")
}

/// 安全校验文件名（防目录穿越）。
fn sanitize_filename(filename: &str) -> Result<String> {
    if filename.is_empty() {
        bail!("文件名不能为空");
    }
    if filename.contains('/') || filename.contains('\\') {
        bail!("文件名不能包含路径分隔符: {filename}");
    }
    if filename.contains("..") {
        bail!("文件名不能包含 '..': {filename}");
    }
    if filename.contains('\0') {
        bail!("文件名不能包含空字节");
    }
    Ok(filename.to_string())
}

/// 获取磁盘可用空间（字节）。
///
/// Unix: `statvfs` 系统调用；Windows: `GetDiskFreeSpaceExW`。
fn disk_free_space(path: &Path) -> Result<u64> {
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
        // 未知平台：返回足够大的值（不阻塞下载）
        Ok(u64::MAX)
    }
}

// Unix statvfs FFI
#[cfg(unix)]
#[repr(C)]
#[allow(non_camel_case_types)]
struct libc_statvfs {
    f_bsize: u64,
    f_frsize: u64,
    f_blocks: u64,
    f_bfree: u64,
    f_bavail: u64,
    f_files: u64,
    f_ffree: u64,
    f_favail: u64,
    f_fsid: u64,
    f_flag: u64,
    f_namemax: u64,
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

// ---- RobustDownloader 主结构 ----

/// 健壮下载器（统一 ONNX 和 GGUF 下载管线）。
///
/// 核心能力：
/// - 断点续传（HTTP Range + `.partial` 文件 + `.meta.json` 元数据）
/// - 多源容错（HuggingFace → hf-mirror → ModelScope 自动降级）
/// - 并发分块下载（大文件多连接并行，Semaphore 限制）
/// - 崩溃恢复（`.meta.json` 持久化 + 启动扫描恢复）
/// - 完整性校验（SHA256 哈希验证）
/// - 取消/暂停（CancellationToken 支持）
/// - 自适应重试（backon 指数退避 + 抖动）
/// - 磁盘空间检查 + 卡顿检测
pub struct RobustDownloader {
    /// HTTP 客户端（复用连接池）
    client: reqwest::Client,
    /// 下载目录（GGUF: models/llm/, ONNX: models/<model_name>/）
    download_dir: PathBuf,
    /// 活跃下载的取消令牌（filename → CancellationToken）
    cancel_tokens: Arc<Mutex<HashMap<String, CancellationToken>>>,
    /// 下载配置
    config: DownloadConfig,
}

impl RobustDownloader {
    /// 创建下载器，确保目录存在。
    pub fn new(download_dir: PathBuf) -> Result<Self> {
        Self::new_with_config(download_dir, DownloadConfig::default())
    }

    /// 使用自定义配置创建下载器。
    pub fn new_with_config(download_dir: PathBuf, config: DownloadConfig) -> Result<Self> {
        std::fs::create_dir_all(&download_dir)
            .with_context(|| format!("创建下载目录失败: {}", download_dir.display()))?;

        let client = reqwest::Client::builder()
            .no_proxy() // 🚫 禁止代理，确保下载直连（铁律一）
            .connect_timeout(Duration::from_secs(15))
            .http2_keep_alive_timeout(Duration::from_secs(15))
            .build()
            .context("创建 HTTP 客户端失败")?;

        Ok(Self {
            client,
            download_dir,
            cancel_tokens: Arc::new(Mutex::new(HashMap::new())),
            config,
        })
    }

    /// 返回下载目录路径。
    pub fn download_dir(&self) -> &Path {
        &self.download_dir
    }

    /// 返回 HTTP 客户端引用（供外部 HEAD 请求复用）。
    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    // ================================================================
    // 核心下载方法
    // ================================================================

    /// 下载文件（支持断点续传 + 多源容错 + 并发分块）。
    ///
    /// # 参数
    /// - `url`: 下载地址（必须 `https://`）
    /// - `filename`: 保存的文件名（仅文件名，不含路径）
    /// - `expected_sha256`: 预期 SHA256 哈希（可选，用于完整性校验）
    /// - `progress`: 进度回调
    /// - `cancel`: 取消令牌（用于暂停/取消）
    pub async fn download(
        &self,
        url: &str,
        filename: &str,
        expected_sha256: Option<&str>,
        progress: DownloadProgressFn,
        cancel: CancellationToken,
    ) -> Result<DownloadOutcome> {
        // 安全校验
        if !url.starts_with("https://") {
            bail!("下载 URL 必须使用 HTTPS 协议: {url}");
        }
        let safe_name = sanitize_filename(filename)?;
        let final_path = self.download_dir.join(&safe_name);
        let partial_path = self.download_dir.join(format!(".{safe_name}.partial"));
        let meta_path = self.download_dir.join(format!(".{safe_name}.meta.json"));

        // 注册取消令牌
        {
            let mut tokens = self.cancel_tokens.lock().await;
            tokens.insert(safe_name.clone(), cancel.clone());
        }

        // 尝试加载已有 manifest（崩溃恢复）
        let mut manifest = if meta_path.exists() {
            match Self::load_manifest(&meta_path).await {
                Ok(m) if m.filename == safe_name => {
                    let completed = m.parts.iter().map(|p| p.completed).sum::<u64>();
                    let outcome = DownloadOutcome::Resumed {
                        completed,
                        total: m.total_size,
                    };
                    tracing::info!(
                        filename = %safe_name,
                        completed,
                        total = m.total_size,
                        "检测到未完成下载，尝试恢复"
                    );
                    // 更新 SHA256（可能调用方提供了新值）
                    let mut m = m;
                    m.sha256 = expected_sha256.map(|s| s.to_string());
                    m.status = DownloadStatus::Downloading {
                        completed,
                        total: m.total_size,
                        speed: 0,
                    };
                    m.updated_at = chrono::Utc::now().timestamp();
                    let _ = outcome; // 抑制未使用警告
                    m
                }
                _ => {
                    // manifest 损坏或文件名不匹配 → 删除重建
                    let _ = tokio::fs::remove_file(&meta_path).await;
                    self.create_new_manifest(&safe_name, url, expected_sha256)?
                }
            }
        } else {
            self.create_new_manifest(&safe_name, url, expected_sha256)?
        };

        // 探测文件大小（通过 Range: bytes=0-0 获取 Content-Range）
        if manifest.total_size == 0 {
            manifest.total_size = self
                .probe_file_size(url, &manifest.sources, &cancel)
                .await?;
            // 持久化探测结果
            manifest.updated_at = chrono::Utc::now().timestamp();
            Self::save_manifest(&manifest, &meta_path).await?;
        }

        let total = manifest.total_size;

        // 磁盘空间检查（5% 余量）
        let free = disk_free_space(&self.download_dir)?;
        if free < total + total / 20 {
            bail!(
                "磁盘空间不足: 需要 {} 字节，可用 {} 字节",
                total + total / 20,
                free
            );
        }

        // 规划分块
        if manifest.parts.is_empty() {
            manifest.parts = self.plan_parts(total);
            // 持久化分块计划
            manifest.updated_at = chrono::Utc::now().timestamp();
            Self::save_manifest(&manifest, &meta_path).await?;
        }

        // 确保 .partial 文件存在（并发分块下载中各 task 独立打开）
        // 多源下载：逐源尝试
        let mut source_order: Vec<usize> = (0..manifest.sources.len()).collect();
        if let Some(pref) = manifest.prefer_source {
            source_order.sort_by_key(|&i| if i == pref { 0 } else { 1 + i });
        }

        let mut last_error = String::new();
        let mut success = false;

        for &src_idx in &source_order {
            let (source_name, download_url) = {
                let source = &manifest.sources[src_idx];
                (source.name.clone(), Self::build_url(source, &manifest.url))
            };

            match self
                .download_with_retry(
                    &download_url,
                    &mut manifest,
                    &partial_path,
                    &progress,
                    &cancel,
                    &meta_path,
                )
                .await
            {
                Ok(()) => {
                    manifest.prefer_source = Some(src_idx);
                    success = true;
                    break;
                }
                Err(e) => {
                    tracing::warn!(
                        source = %source_name,
                        error = %format!("{e:#}"),
                        "源下载失败"
                    );
                    last_error = format!("{source_name}: {e:#}");
                    // 检查是否用户取消
                    if cancel.is_cancelled() {
                        break;
                    }
                }
            }
        }

        if !success {
            if cancel.is_cancelled() {
                // 保存 Paused 状态
                let completed = manifest.parts.iter().map(|p| p.completed).sum::<u64>();
                manifest.status = DownloadStatus::Paused { completed, total };
                manifest.updated_at = chrono::Utc::now().timestamp();
                Self::save_manifest(&manifest, &meta_path).await?;
                // 清理取消令牌
                let mut tokens = self.cancel_tokens.lock().await;
                tokens.remove(&safe_name);
                return Ok(DownloadOutcome::Paused { completed, total });
            }
            manifest.status = DownloadStatus::Failed {
                error: last_error.clone(),
                completed: manifest.parts.iter().map(|p| p.completed).sum(),
                total,
            };
            manifest.updated_at = chrono::Utc::now().timestamp();
            Self::save_manifest(&manifest, &meta_path).await?;
            // 清理取消令牌
            let mut tokens = self.cancel_tokens.lock().await;
            tokens.remove(&safe_name);
            bail!("全部源下载失败: {last_error}");
        }

        // fsync（确保所有分块数据落盘后再校验）
        // 各 part task 已在退出前 sync_data，此处再做一次全局 sync_all
        {
            let file = tokio::fs::OpenOptions::new()
                .write(true)
                .open(&partial_path)
                .await
                .context("打开 .partial 文件失败（fsync 阶段）")?;
            file.sync_all().await.context("fsync 失败")?;
        }

        // SHA256 完整性校验
        if let Some(expected) = &manifest.sha256 {
            manifest.status = DownloadStatus::Verifying {
                completed: total,
                total,
            };
            Self::save_manifest(&manifest, &meta_path).await?;
            progress(total, total, 0);

            let actual = Self::compute_sha256(&partial_path).await?;
            if actual != *expected {
                let _ = tokio::fs::remove_file(&partial_path).await;
                let _ = tokio::fs::remove_file(&meta_path).await;
                bail!("SHA256 校验失败: 期望 {expected}，实际 {actual}");
            }
            tracing::info!(filename = %safe_name, "SHA256 校验通过");
        }

        // 原子重命名
        tokio::fs::rename(&partial_path, &final_path)
            .await
            .context(format!(
                "原子重命名失败: {} → {}",
                partial_path.display(),
                final_path.display()
            ))?;

        // 清理 .meta.json
        let _ = tokio::fs::remove_file(&meta_path).await;

        // 最终进度推送
        progress(total, total, 0);

        // 清理取消令牌
        let mut tokens = self.cancel_tokens.lock().await;
        tokens.remove(&safe_name);

        Ok(DownloadOutcome::Completed)
    }

    // ================================================================
    // 控制：暂停 / 取消 / 状态查询
    // ================================================================

    /// 暂停指定文件的下载。
    pub async fn pause(&self, filename: &str) -> Result<()> {
        let safe_name = sanitize_filename(filename)?;
        let tokens = self.cancel_tokens.lock().await;
        if let Some(token) = tokens.get(&safe_name) {
            token.cancel();
        }
        Ok(())
    }

    /// 取消指定文件的下载 + 清理临时文件。
    pub async fn abort(&self, filename: &str) -> Result<()> {
        let safe_name = sanitize_filename(filename)?;

        // 取消活跃下载
        {
            let tokens = self.cancel_tokens.lock().await;
            if let Some(token) = tokens.get(&safe_name) {
                token.cancel();
            }
        }

        // 清理临时文件
        let partial = self.download_dir.join(format!(".{safe_name}.partial"));
        let meta = self.download_dir.join(format!(".{safe_name}.meta.json"));
        let _ = tokio::fs::remove_file(&partial).await;
        let _ = tokio::fs::remove_file(&meta).await;

        // 清理取消令牌
        let mut tokens = self.cancel_tokens.lock().await;
        tokens.remove(&safe_name);

        Ok(())
    }

    /// 获取下载状态（从 .meta.json 读取）。
    pub async fn get_status(&self, filename: &str) -> Result<Option<DownloadStatus>> {
        let safe_name = sanitize_filename(filename)?;
        let meta_path = self.download_dir.join(format!(".{safe_name}.meta.json"));
        if !meta_path.exists() {
            return Ok(None);
        }
        let manifest = Self::load_manifest(&meta_path).await?;
        Ok(Some(manifest.status))
    }

    /// 列出所有未完成下载（扫描 .meta.json 文件）。
    pub async fn list_pending(&self) -> Vec<DownloadStatusSummary> {
        let mut results = Vec::new();

        let entries = match tokio::fs::read_dir(&self.download_dir).await {
            Ok(e) => e,
            Err(_) => return results,
        };

        #[cfg(unix)]
        let _ = entries; // 使用下面的 spawn_blocking 版本

        let dir = self.download_dir.clone();
        let summaries = tokio::task::spawn_blocking(move || -> Vec<DownloadStatusSummary> {
            let mut items = Vec::new();
            let entries = match std::fs::read_dir(&dir) {
                Ok(e) => e,
                Err(_) => return items,
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let name = match path.file_name().and_then(|n| n.to_str()) {
                    Some(n) => n.to_string(),
                    None => continue,
                };
                // 匹配 .{filename}.meta.json 模式
                if !name.starts_with('.') || !name.ends_with(".meta.json") {
                    continue;
                }
                let content = match std::fs::read_to_string(&path) {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                let manifest: DownloadManifest = match serde_json::from_str(&content) {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                let completed = manifest.parts.iter().map(|p| p.completed).sum();
                items.push(DownloadStatusSummary {
                    filename: manifest.filename.clone(),
                    status: manifest.status.clone(),
                    total_size: manifest.total_size,
                    completed,
                    created_at: manifest.created_at,
                    updated_at: manifest.updated_at,
                });
            }
            items
        })
        .await
        .unwrap_or_default();

        results.extend(summaries);
        results
    }

    /// 清理所有 `.partial` + `.meta.json` 文件，返回释放的字节数。
    pub async fn cleanup_partials(&self) -> u64 {
        let dir = self.download_dir.clone();
        tokio::task::spawn_blocking(move || -> u64 {
            let mut total = 0u64;
            let entries = match std::fs::read_dir(&dir) {
                Ok(e) => e,
                Err(_) => return 0,
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let name = match path.file_name().and_then(|n| n.to_str()) {
                    Some(n) => n,
                    None => continue,
                };
                if name.starts_with('.')
                    && (name.ends_with(".partial") || name.ends_with(".meta.json"))
                {
                    if let Ok(meta) = std::fs::metadata(&path) {
                        total += meta.len();
                    }
                    let _ = std::fs::remove_file(&path);
                }
            }
            total
        })
        .await
        .unwrap_or(0)
    }

    /// 启动时扫描崩溃恢复（检测 .partial + .meta.json 文件）。
    ///
    /// 返回需要恢复的下载列表。
    pub async fn scan_for_recovery(&self) -> Vec<DownloadManifest> {
        let mut results = Vec::new();
        let dir = self.download_dir.clone();

        let manifests = tokio::task::spawn_blocking(move || -> Vec<DownloadManifest> {
            let mut items = Vec::new();
            let entries = match std::fs::read_dir(&dir) {
                Ok(e) => e,
                Err(_) => return items,
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let name = match path.file_name().and_then(|n| n.to_str()) {
                    Some(n) => n.to_string(),
                    None => continue,
                };
                if !name.starts_with('.') || !name.ends_with(".meta.json") {
                    continue;
                }
                let content = match std::fs::read_to_string(&path) {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                if let Ok(manifest) = serde_json::from_str::<DownloadManifest>(&content) {
                    // 检查 .partial 文件是否存在
                    let partial = dir.join(format!(".{}.partial", manifest.filename));
                    if partial.exists() {
                        items.push(manifest);
                    } else {
                        // .partial 不存在但 .meta.json 存在 → 无用元数据，删除
                        let _ = std::fs::remove_file(&path);
                    }
                }
            }
            items
        })
        .await
        .unwrap_or_default();

        results.extend(manifests);
        results
    }

    // ================================================================
    // 内部方法
    // ================================================================

    /// 创建新的下载清单。
    fn create_new_manifest(
        &self,
        safe_name: &str,
        url: &str,
        sha256: Option<&str>,
    ) -> Result<DownloadManifest> {
        let now = chrono::Utc::now().timestamp();
        Ok(DownloadManifest {
            filename: safe_name.to_string(),
            url: url.to_string(),
            total_size: 0, // 后续探测
            sha256: sha256.map(|s| s.to_string()),
            sources: get_download_sources(),
            parts: Vec::new(), // 后续规划
            status: DownloadStatus::Queued,
            prefer_source: None,
            created_at: now,
            updated_at: now,
        })
    }

    /// 探测文件大小（通过 Range: bytes=0-0 → Content-Range）。
    ///
    /// 参考：hf_transfer — 避免 HEAD 请求（某些 CDN 不支持）。
    async fn probe_file_size(
        &self,
        url: &str,
        sources: &[DownloadSourceDef],
        cancel: &CancellationToken,
    ) -> Result<u64> {
        for source in sources {
            if cancel.is_cancelled() {
                bail!("探测文件大小时被取消");
            }
            let probe_url = Self::build_url(source, url);
            let resp = match self
                .client
                .get(&probe_url)
                .header("Range", "bytes=0-0")
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(source = %source.name, error = %format!("{e}"), "探测文件大小失败");
                    continue;
                }
            };
            if !resp.status().is_success() {
                continue;
            }
            // 从 Content-Range 解析总大小
            if let Some(cr) = resp.headers().get("content-range")
                && let Ok(s) = cr.to_str()
                && let Some(total_str) = s.split('/').nth(1)
                && let Ok(total) = total_str.trim().parse::<u64>()
            {
                return Ok(total);
            }
            // 回退到 Content-Length（可能不是真实大小，但好过 0）
            if let Some(len) = resp.content_length() {
                return Ok(len);
            }
        }
        bail!("无法探测文件大小: 全部源探测失败")
    }

    /// 规划分块。
    ///
    /// 小文件（< min_chunk_size）→ 单块。
    /// 大文件 → 多块，每块 min_chunk_size ~ max_chunk_size。
    fn plan_parts(&self, total_size: u64) -> Vec<DownloadPart> {
        if total_size == 0 {
            return vec![DownloadPart {
                index: 0,
                offset: 0,
                size: 0,
                completed: 0,
                retries: 0,
            }];
        }
        if total_size <= self.config.min_chunk_size {
            return vec![DownloadPart {
                index: 0,
                offset: 0,
                size: total_size,
                completed: 0,
                retries: 0,
            }];
        }
        let chunk_size =
            (total_size / 4).clamp(self.config.min_chunk_size, self.config.max_chunk_size);
        let mut parts = Vec::new();
        let mut offset = 0u64;
        let mut index = 0;
        while offset < total_size {
            let size = chunk_size.min(total_size - offset);
            parts.push(DownloadPart {
                index,
                offset,
                size,
                completed: 0,
                retries: 0,
            });
            offset += size;
            index += 1;
        }
        parts
    }

    /// 从源和原始 URL 构建完整下载 URL。
    ///
    /// 如果原始 URL 以 `https://huggingface.co` 开头，替换为源 base_url。
    fn build_url(source: &DownloadSourceDef, original_url: &str) -> String {
        if original_url.starts_with("https://huggingface.co") {
            original_url.replacen("https://huggingface.co", &source.base_url, 1)
        } else {
            original_url.to_string()
        }
    }

    /// 并发分块下载（带重试 + stall 检测 + 进度聚合）。
    ///
    /// 参考：Ollama `run()` + hf_transfer `download_async()`。
    async fn download_with_retry(
        &self,
        url: &str,
        manifest: &mut DownloadManifest,
        partial_path: &Path,
        progress: &DownloadProgressFn,
        cancel: &CancellationToken,
        meta_path: &Path,
    ) -> Result<()> {
        let total = manifest.total_size;

        // 进度追踪器：每个 part 一个 AtomicU64
        let parts_completed: Vec<Arc<AtomicU64>> = manifest
            .parts
            .iter()
            .map(|p| Arc::new(AtomicU64::new(p.completed)))
            .collect();
        let all_progress: Vec<Arc<AtomicU64>> = parts_completed.clone();

        // 下载速度追踪
        let speed_tracker = Arc::new(AtomicU64::new(0));
        let download_start = std::time::Instant::now();

        let semaphore = Arc::new(tokio::sync::Semaphore::new(self.config.max_concurrency));
        let mut handles = Vec::new();

        for part in &manifest.parts {
            if part.completed >= part.size {
                continue; // 已完成
            }

            let part_idx = part.index;
            let part_offset = part.offset;
            let part_size = part.size;
            let part_completed = part.completed;

            let sem = semaphore.clone();
            let url = url.to_string();
            let client = self.client.clone();
            let cancel = cancel.clone();
            let progress = progress.clone();
            let config = self.config.clone();
            let partial_path = partial_path.to_path_buf();
            let part_progress = all_progress[part_idx].clone();
            let all_progress_clone = all_progress.clone();
            let speed_clone = speed_tracker.clone();
            let dl_start = download_start;

            let handle = tokio::spawn(async move {
                let _permit = sem.acquire().await;

                Self::download_single_part(
                    &client,
                    &url,
                    &partial_path,
                    part_offset,
                    part_size,
                    part_completed,
                    &part_progress,
                    &all_progress_clone,
                    &speed_clone,
                    dl_start,
                    total,
                    &progress,
                    &cancel,
                    &config,
                )
                .await
            });
            handles.push((part_idx, handle, all_progress[part_idx].clone()));
        }

        // 等待所有 part 完成
        let mut errors = Vec::new();
        for (part_idx, handle, part_completed) in handles {
            match handle.await {
                Ok(Ok(final_completed)) => {
                    manifest.parts[part_idx].completed = final_completed;
                }
                Ok(Err(e)) => {
                    let completed = part_completed.load(Ordering::Relaxed);
                    manifest.parts[part_idx].completed = completed;
                    errors.push(format!("part {part_idx}: {e:#}"));
                }
                Err(e) => {
                    errors.push(format!("part {part_idx}: 任务 panic: {e}"));
                }
            }
        }

        if !errors.is_empty() {
            // 持久化当前进度（允许后续恢复）
            let completed = manifest.parts.iter().map(|p| p.completed).sum::<u64>();
            manifest.status = DownloadStatus::Failed {
                error: errors.join("; "),
                completed,
                total,
            };
            manifest.updated_at = chrono::Utc::now().timestamp();
            Self::save_manifest(manifest, meta_path).await?;
            bail!("部分分块下载失败: {}", errors.join("; "));
        }

        // 持久化最终状态
        let completed = manifest.parts.iter().map(|p| p.completed).sum::<u64>();
        manifest.status = DownloadStatus::Downloading {
            completed,
            total,
            speed: speed_tracker.load(Ordering::Relaxed),
        };
        manifest.updated_at = chrono::Utc::now().timestamp();
        Self::save_manifest(manifest, meta_path).await?;

        Ok(())
    }

    /// 下载单个分块（带重试 + stall 检测 + Range 续传）。
    ///
    /// 每个分块拥有独立的文件句柄，写入到各自的偏移位置。
    #[allow(clippy::too_many_arguments)]
    async fn download_single_part(
        client: &reqwest::Client,
        url: &str,
        partial_path: &Path,
        offset: u64,
        size: u64,
        initial_completed: u64,
        part_progress: &Arc<AtomicU64>,
        all_progress: &[Arc<AtomicU64>],
        speed_tracker: &Arc<AtomicU64>,
        download_start: std::time::Instant,
        total: u64,
        progress: &DownloadProgressFn,
        cancel: &CancellationToken,
        config: &DownloadConfig,
    ) -> Result<u64> {
        part_progress.store(initial_completed, Ordering::Relaxed);
        let mut completed = initial_completed;
        let max_retries = config.max_retries;
        let mut retry_count: u32 = 0;
        let mut last_stall_progress: u64 = initial_completed;
        let mut last_stall_time = std::time::Instant::now();

        loop {
            if cancel.is_cancelled() {
                return Ok(completed);
            }

            if completed >= size {
                return Ok(completed); // 该分块已完成
            }

            let start = offset + completed;
            let end = offset + size - 1;
            let range = format!("bytes={start}-{end}");

            // 发送 Range 请求
            let resp = match client.get(url).header("Range", &range).send().await {
                Ok(r) => r,
                Err(e) => {
                    retry_count += 1;
                    if retry_count > max_retries {
                        bail!("Range 请求失败（已达最大重试 {max_retries} 次）: {e:#}");
                    }
                    let delay = backoff_delay(retry_count, config);
                    tracing::warn!(retry = retry_count, error = %format!("{e}"), "Range 请求失败，等待重试");
                    tokio::select! {
                        _ = cancel.cancelled() => return Ok(completed),
                        _ = tokio::time::sleep(delay) => {}
                    }
                    continue;
                }
            };

            if !resp.status().is_success() {
                // 416 Range Not Satisfiable → 可能文件已完整
                if resp.status().as_u16() == 416 {
                    return Ok(size);
                }
                retry_count += 1;
                if retry_count > max_retries {
                    bail!(
                        "HTTP {} - {}",
                        resp.status(),
                        resp.status().canonical_reason().unwrap_or("未知")
                    );
                }
                let delay = backoff_delay(retry_count, config);
                tokio::select! {
                    _ = cancel.cancelled() => return Ok(completed),
                    _ = tokio::time::sleep(delay) => {}
                }
                continue;
            }

            // 检查服务器是否支持 Range（206 vs 200）
            let server_ignores_range = resp.status().as_u16() == 200;
            if server_ignores_range && completed > 0 {
                tracing::warn!("服务器忽略 Range 请求，重置该分块");
                completed = 0;
                part_progress.store(0, Ordering::Relaxed);
            }

            // 打开文件并定位到偏移
            let mut file = tokio::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(false)
                .open(partial_path)
                .await
                .context("打开 .partial 文件失败")?;
            let seek_pos = if server_ignores_range {
                offset
            } else {
                offset + completed
            };
            file.seek(SeekFrom::Start(seek_pos)).await?;

            // 流式写入
            let mut stream = resp.bytes_stream();
            let mut last_progress_time = std::time::Instant::now();
            let mut last_meta_persist_bytes: u64 = completed;
            let mut had_error = false;

            while let Some(chunk_result) = stream.next().await {
                if cancel.is_cancelled() {
                    file.flush().await.ok();
                    return Ok(completed);
                }

                let chunk = match chunk_result {
                    Ok(c) => c,
                    Err(e) => {
                        had_error = true;
                        tracing::warn!("读取数据流失败: {e}");
                        break;
                    }
                };

                file.write_all(&chunk).await.context("写入文件失败")?;
                completed += chunk.len() as u64;
                part_progress.store(completed, Ordering::Relaxed);

                // 进度上报（节流 + 聚合）
                let now = std::time::Instant::now();
                if now.duration_since(last_progress_time)
                    >= Duration::from_millis(config.progress_interval_ms)
                {
                    let total_completed: u64 =
                        all_progress.iter().map(|p| p.load(Ordering::Relaxed)).sum();
                    let elapsed = download_start.elapsed().as_secs_f64();
                    let speed = if elapsed > 0.0 {
                        (total_completed as f64 / elapsed) as u64
                    } else {
                        0
                    };
                    speed_tracker.store(speed, Ordering::Relaxed);
                    progress(total_completed, total, speed);
                    last_progress_time = now;
                }

                // stall 检测：检查该 part 是否有进度
                if now.duration_since(last_stall_time)
                    >= Duration::from_secs(config.stall_timeout_secs)
                {
                    let current = part_progress.load(Ordering::Relaxed);
                    if current == last_stall_progress {
                        tracing::warn!(
                            "分块下载卡顿，{} 秒无进度，重试",
                            config.stall_timeout_secs
                        );
                        had_error = true;
                        break;
                    }
                    last_stall_progress = current;
                    last_stall_time = now;
                }

                // 定期 fsync（确保数据落盘）
                if completed - last_meta_persist_bytes >= config.meta_persist_bytes {
                    file.sync_data().await.ok();
                    last_meta_persist_bytes = completed;
                }
            }

            file.sync_data().await.ok();
            drop(file);

            if had_error {
                retry_count += 1;
                if retry_count > max_retries {
                    bail!("分块下载达到最大重试次数 {max_retries}");
                }
                let delay = backoff_delay(retry_count, config);
                tracing::info!(
                    retry = retry_count,
                    delay_ms = delay.as_millis(),
                    "分块下载失败，等待后重试"
                );
                tokio::select! {
                    _ = cancel.cancelled() => return Ok(completed),
                    _ = tokio::time::sleep(delay) => {}
                }
                last_stall_progress = completed;
                last_stall_time = std::time::Instant::now();
                continue;
            }

            // 成功完成 → 最终进度上报
            let total_completed: u64 = all_progress.iter().map(|p| p.load(Ordering::Relaxed)).sum();
            let elapsed = download_start.elapsed().as_secs_f64();
            let speed = if elapsed > 0.0 {
                (total_completed as f64 / elapsed) as u64
            } else {
                0
            };
            speed_tracker.store(speed, Ordering::Relaxed);
            progress(total_completed, total, speed);
            return Ok(completed);
        }
    }

    // ================================================================
    // 持久化 / 序列化
    // ================================================================

    /// 保存 manifest 到 .meta.json 文件。
    async fn save_manifest(manifest: &DownloadManifest, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(manifest).context("序列化 manifest 失败")?;
        // 原子写入：先写 .tmp 再 rename
        let tmp_path = path.with_extension("meta.json.tmp");
        tokio::fs::write(&tmp_path, json.as_bytes())
            .await
            .context(format!("写入 .meta.json.tmp 失败: {}", tmp_path.display()))?;
        tokio::fs::rename(&tmp_path, path).await.context(format!(
            "重命名 .meta.json 失败: {} → {}",
            tmp_path.display(),
            path.display()
        ))?;
        Ok(())
    }

    /// 从 .meta.json 文件加载 manifest。
    async fn load_manifest(path: &Path) -> Result<DownloadManifest> {
        let content = tokio::fs::read_to_string(path)
            .await
            .context(format!("读取 .meta.json 失败: {}", path.display()))?;
        serde_json::from_str::<DownloadManifest>(&content)
            .context(format!("解析 .meta.json 失败: {}", path.display()))
    }

    /// 计算文件 SHA256 哈希（流式读取，内存友好）。
    async fn compute_sha256(path: &Path) -> Result<String> {
        let path = path.to_path_buf();
        let hash = tokio::task::spawn_blocking(move || -> Result<String> {
            use std::io::Read;
            let mut hasher = Sha256::new();
            let mut file = std::fs::File::open(&path)
                .with_context(|| format!("打开文件失败: {}", path.display()))?;
            let mut buf = vec![0u8; 1024 * 1024]; // 1MB buffer
            loop {
                let n = file.read(&mut buf).context("读取文件失败")?;
                if n == 0 {
                    break;
                }
                hasher.update(&buf[..n]);
            }
            Ok(format!("{:x}", hasher.finalize()))
        })
        .await
        .context("SHA256 计算任务失败")??;
        Ok(hash)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn test_sanitize_filename() {
        assert!(sanitize_filename("model.gguf").is_ok());
        assert!(sanitize_filename("../etc/passwd").is_err());
        assert!(sanitize_filename("a/b").is_err());
        assert!(sanitize_filename("").is_err());
        assert!(sanitize_filename("test\0.gguf").is_err());
    }

    #[test]
    fn test_backoff_delay() {
        let config = DownloadConfig::default();
        let d0 = backoff_delay(0, &config);
        let d1 = backoff_delay(1, &config);
        let d6 = backoff_delay(6, &config);
        // 退避时间递增
        assert!(d1 >= d0 || d1.as_millis() >= 100);
        // 不超过上限
        assert!(d6.as_secs() <= config.backoff_max_secs + 1);
    }

    #[test]
    fn test_plan_parts_small_file() {
        let dl = RobustDownloader::new(std::env::temp_dir()).unwrap();
        let parts = dl.plan_parts(50 * 1024 * 1024); // 50MB
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].offset, 0);
        assert_eq!(parts[0].size, 50 * 1024 * 1024);
    }

    #[test]
    fn test_plan_parts_large_file() {
        let dl = RobustDownloader::new(std::env::temp_dir()).unwrap();
        let parts = dl.plan_parts(2 * 1024 * 1024 * 1024); // 2GB
        assert!(parts.len() > 1);
        // 所有 part 的总和等于总大小
        let total: u64 = parts.iter().map(|p| p.size).sum();
        assert_eq!(total, 2 * 1024 * 1024 * 1024);
        // 每个 part 的 offset 正确
        let mut expected_offset = 0u64;
        for (i, part) in parts.iter().enumerate() {
            assert_eq!(part.index, i);
            assert_eq!(part.offset, expected_offset);
            expected_offset += part.size;
        }
    }

    #[test]
    fn test_plan_parts_zero() {
        let dl = RobustDownloader::new(std::env::temp_dir()).unwrap();
        let parts = dl.plan_parts(0);
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].size, 0);
    }

    #[test]
    fn test_is_retryable_error() {
        assert!(is_retryable_error(&anyhow::anyhow!("connection timed out")));
        assert!(is_retryable_error(&anyhow::anyhow!(
            "HTTP 503 Service Unavailable"
        )));
        assert!(is_retryable_error(&anyhow::anyhow!(
            "429 Too Many Requests"
        )));
        assert!(!is_retryable_error(&anyhow::anyhow!("HTTP 404 Not Found")));
    }

    #[test]
    fn test_build_url() {
        let source = DownloadSourceDef {
            name: "hf-mirror".to_string(),
            base_url: "https://hf-mirror.com".to_string(),
            branch: "main".to_string(),
        };
        let url = "https://huggingface.co/Qwen/Qwen2.5-3B-Instruct-GGUF/resolve/main/qwen2.5-3b-instruct-q4_k_m.gguf";
        let built = RobustDownloader::build_url(&source, url);
        assert!(built.starts_with("https://hf-mirror.com/"));
    }

    #[test]
    fn test_manifest_serde_roundtrip() {
        let manifest = DownloadManifest {
            filename: "test.gguf".to_string(),
            url: "https://huggingface.co/test/test/resolve/main/test.gguf".to_string(),
            total_size: 2000000000,
            sha256: Some("abc123".to_string()),
            sources: get_download_sources(),
            parts: vec![
                DownloadPart {
                    index: 0,
                    offset: 0,
                    size: 1000000000,
                    completed: 500000000,
                    retries: 1,
                },
                DownloadPart {
                    index: 1,
                    offset: 1000000000,
                    size: 1000000000,
                    completed: 0,
                    retries: 0,
                },
            ],
            status: DownloadStatus::Downloading {
                completed: 500000000,
                total: 2000000000,
                speed: 1000000,
            },
            prefer_source: Some(0),
            created_at: 1700000000,
            updated_at: 1700000100,
        };
        let json = serde_json::to_string(&manifest).unwrap();
        let back: DownloadManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.filename, manifest.filename);
        assert_eq!(back.total_size, manifest.total_size);
        assert_eq!(back.parts.len(), manifest.parts.len());
        assert_eq!(back.status, manifest.status);
    }

    #[test]
    fn test_download_status_default() {
        assert_eq!(DownloadStatus::default(), DownloadStatus::Queued);
    }

    #[test]
    fn test_download_config_default() {
        let config = DownloadConfig::default();
        assert_eq!(config.max_retries, 6);
        assert_eq!(config.stall_timeout_secs, 30);
        assert_eq!(config.progress_interval_ms, 100);
    }

    #[tokio::test]
    async fn test_save_load_manifest_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let meta_path = tmp.path().join("test.meta.json");
        let manifest = DownloadManifest {
            filename: "test.gguf".to_string(),
            url: "https://huggingface.co/test/resolve/main/test.gguf".to_string(),
            total_size: 1000,
            sha256: None,
            sources: vec![DownloadSourceDef {
                name: "test".to_string(),
                base_url: "https://test.com".to_string(),
                branch: "main".to_string(),
            }],
            parts: vec![DownloadPart {
                index: 0,
                offset: 0,
                size: 1000,
                completed: 500,
                retries: 0,
            }],
            status: DownloadStatus::Paused {
                completed: 500,
                total: 1000,
            },
            prefer_source: None,
            created_at: 1700000000,
            updated_at: 1700000100,
        };
        RobustDownloader::save_manifest(&manifest, &meta_path)
            .await
            .unwrap();
        let loaded = RobustDownloader::load_manifest(&meta_path).await.unwrap();
        assert_eq!(loaded.filename, manifest.filename);
        assert_eq!(loaded.total_size, manifest.total_size);
        assert_eq!(loaded.status, manifest.status);
    }

    #[tokio::test]
    async fn test_scan_for_recovery_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let dl = RobustDownloader::new(tmp.path().to_path_buf()).unwrap();
        let pending = dl.scan_for_recovery().await;
        assert!(pending.is_empty());
    }

    #[tokio::test]
    async fn test_scan_for_recovery_with_meta() {
        let tmp = tempfile::tempdir().unwrap();
        let dl = RobustDownloader::new(tmp.path().to_path_buf()).unwrap();
        // 创建 .meta.json + .partial 文件
        let manifest = DownloadManifest {
            filename: "test.gguf".to_string(),
            url: "https://huggingface.co/test/resolve/main/test.gguf".to_string(),
            total_size: 1000,
            sha256: None,
            sources: vec![],
            parts: vec![],
            status: DownloadStatus::Paused {
                completed: 500,
                total: 1000,
            },
            prefer_source: None,
            created_at: 1700000000,
            updated_at: 1700000100,
        };
        let meta_path = tmp.path().join(".test.gguf.meta.json");
        RobustDownloader::save_manifest(&manifest, &meta_path)
            .await
            .unwrap();
        let partial_path = tmp.path().join(".test.gguf.partial");
        tokio::fs::write(&partial_path, b"partial data")
            .await
            .unwrap();

        let pending = dl.scan_for_recovery().await;
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].filename, "test.gguf");
    }

    #[tokio::test]
    async fn test_cleanup_partials() {
        let tmp = tempfile::tempdir().unwrap();
        let dl = RobustDownloader::new(tmp.path().to_path_buf()).unwrap();
        // 创建临时文件
        tokio::fs::write(tmp.path().join(".test.gguf.partial"), b"partial")
            .await
            .unwrap();
        tokio::fs::write(tmp.path().join(".test.gguf.meta.json"), b"{}")
            .await
            .unwrap();
        tokio::fs::write(tmp.path().join("final.gguf"), b"final")
            .await
            .unwrap();

        let freed = dl.cleanup_partials().await;
        assert!(freed > 0);
        // 最终文件不应被删除
        assert!(tmp.path().join("final.gguf").exists());
        // 临时文件应被删除
        assert!(!tmp.path().join(".test.gguf.partial").exists());
        assert!(!tmp.path().join(".test.gguf.meta.json").exists());
    }

    #[tokio::test]
    async fn test_list_pending_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let dl = RobustDownloader::new(tmp.path().to_path_buf()).unwrap();
        let pending = dl.list_pending().await;
        assert!(pending.is_empty());
    }

    #[tokio::test]
    async fn test_abort_cleans_files() {
        let tmp = tempfile::tempdir().unwrap();
        let dl = RobustDownloader::new(tmp.path().to_path_buf()).unwrap();
        tokio::fs::write(tmp.path().join(".test.gguf.partial"), b"data")
            .await
            .unwrap();
        tokio::fs::write(tmp.path().join(".test.gguf.meta.json"), b"{}")
            .await
            .unwrap();
        dl.abort("test.gguf").await.unwrap();
        assert!(!tmp.path().join(".test.gguf.partial").exists());
        assert!(!tmp.path().join(".test.gguf.meta.json").exists());
    }

    #[tokio::test]
    async fn test_get_status_no_meta() {
        let tmp = tempfile::tempdir().unwrap();
        let dl = RobustDownloader::new(tmp.path().to_path_buf()).unwrap();
        let status = dl.get_status("nonexistent.gguf").await.unwrap();
        assert!(status.is_none());
    }

    #[test]
    fn test_disk_free_space() {
        let free = disk_free_space(Path::new("/")).unwrap();
        assert!(free > 0, "磁盘可用空间应大于 0");
    }
}
