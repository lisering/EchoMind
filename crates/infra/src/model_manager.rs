//! 本地 LLM 模型文件管理器（REQ-LLM-004）。
//!
//! 提供本地 GGUF 模型的列表、删除、路径获取、下载功能。
//! 模型存储于 `<data_dir>/models/llm/` 目录下。
//!
//! # 安全要点
//! - 所有接受 `filename` 参数的方法均通过 `sanitize_filename()` 校验，防止路径穿越攻击
//! - 下载仅允许 `https://` 前缀的 URL
//! - 下载时先写入 `.tmp` 临时文件，完成后原子重命名

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use echomind_models::ModelInfo;
use futures::StreamExt;
use serde::Serialize;
use tokio::io::AsyncWriteExt;

/// 下载进度回调类型。
///
/// 参数：(已下载字节, 总字节, 速度字节/秒)
pub type DownloadProgressFn = Box<dyn Fn(u64, u64, u64) + Send + Sync>;

/// 推荐模型信息（REQ-LLM-004 AC-6）。
///
/// 用户可从设置面板一键下载这些模型。
/// 来源：HuggingFace GGUF 仓库。
#[derive(Debug, Clone, Serialize)]
pub struct RecommendedModel {
    /// 模型显示名
    pub name: &'static str,
    /// 模型架构（如 `qwen2.5` / `llama3.2` / `phi3.5`）
    pub architecture: &'static str,
    /// 参数规模（如 `3B` / `7B` / `3.8B`）
    pub param_size: &'static str,
    /// 量化格式（如 `Q4_K_M`）
    pub quantization: &'static str,
    /// 解压后文件大小（GB，近似值，用于前端展示）
    pub size_gb: f64,
    /// HuggingFace 下载 URL
    pub url: &'static str,
    /// 模型简介
    pub description: &'static str,
}

/// 推荐模型列表（REQ-LLM-004 AC-6）。
///
/// 用户可从设置面板一键下载这些模型。
/// 来源：HuggingFace GGUF 仓库（通过代理下载）。
pub const RECOMMENDED_MODELS: &[RecommendedModel] = &[
    RecommendedModel {
        name: "Qwen2.5-3B-Instruct",
        architecture: "qwen2.5",
        param_size: "3B",
        quantization: "Q4_K_M",
        size_gb: 2.0,
        url: "https://huggingface.co/Qwen/Qwen2.5-3B-Instruct-GGUF/resolve/main/qwen2.5-3b-instruct-q4_k_m.gguf",
        description: "低配设备推荐，中文能力优秀",
    },
    RecommendedModel {
        name: "Llama-3.2-3B-Instruct",
        architecture: "llama3.2",
        param_size: "3B",
        quantization: "Q4_K_M",
        size_gb: 2.0,
        url: "https://huggingface.co/meta-llama/Llama-3.2-3B-Instruct-GGUF/resolve/main/llama-3.2-3b-instruct-q4_k_m.gguf",
        description: "英文场景推荐",
    },
    RecommendedModel {
        name: "Phi-3.5-mini-instruct",
        architecture: "phi3.5",
        param_size: "3.8B",
        quantization: "Q4_K_M",
        size_gb: 2.2,
        url: "https://huggingface.co/microsoft/Phi-3.5-mini-instruct-gguf/resolve/main/Phi-3.5-mini-instruct-q4_k_m.gguf",
        description: "推理能力强",
    },
    RecommendedModel {
        name: "Qwen2.5-7B-Instruct",
        architecture: "qwen2.5",
        param_size: "7B",
        quantization: "Q4_K_M",
        size_gb: 4.1,
        url: "https://huggingface.co/Qwen/Qwen2.5-7B-Instruct-GGUF/resolve/main/qwen2.5-7b-instruct-q4_k_m.gguf",
        description: "平衡质量与速度",
    },
];

/// 本地 LLM 模型文件管理器（REQ-LLM-004）。
///
/// 管理 `<data_dir>/models/llm/` 目录下的 GGUF 模型文件。
/// 提供列表、删除、路径获取、下载功能。
pub struct ModelManager {
    /// 模型存储目录（`<data_dir>/models/llm/`）
    models_dir: PathBuf,
}

impl ModelManager {
    /// 创建管理器，确保 `models/llm/` 目录存在。
    ///
    /// # 参数
    /// - `data_dir`: 应用数据目录路径
    pub fn new(data_dir: &Path) -> Result<Self> {
        let models_dir = data_dir.join("models").join("llm");
        std::fs::create_dir_all(&models_dir)
            .with_context(|| format!("创建模型目录失败: {}", models_dir.display()))?;
        Ok(Self { models_dir })
    }

    /// 返回模型存储目录路径。
    pub fn models_dir(&self) -> &Path {
        &self.models_dir
    }

    /// 扫描目录，列出所有 `.gguf` 文件（REQ-LLM-004 AC-1）。
    ///
    /// 返回的列表按文件名升序排列。
    pub fn list_models(&self) -> Result<Vec<ModelInfo>> {
        let mut models = Vec::new();
        let entries = std::fs::read_dir(&self.models_dir)
            .with_context(|| format!("读取模型目录失败: {}", self.models_dir.display()))?;
        for entry in entries {
            let entry = entry.context("读取目录条目失败")?;
            let path = entry.path();
            if path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("gguf"))
                && let Some(info) = Self::parse_model_info(&path)
            {
                models.push(info);
            }
        }
        models.sort_by(|a, b| a.filename.cmp(&b.filename));
        Ok(models)
    }

    /// 删除指定模型文件（REQ-LLM-004 AC-3）。
    ///
    /// # 安全
    /// 文件名通过 `sanitize_filename()` 校验，防止路径穿越。
    pub fn delete_model(&self, filename: &str) -> Result<()> {
        let path = self.model_path(filename)?;
        std::fs::remove_file(&path)
            .with_context(|| format!("删除模型文件失败: {}", path.display()))?;
        Ok(())
    }

    /// 获取模型文件完整路径（含路径穿越防护）。
    pub fn model_path(&self, filename: &str) -> Result<PathBuf> {
        let safe_name = Self::sanitize_filename(filename)?;
        Ok(self.models_dir.join(safe_name))
    }

    /// 从 URL 下载 GGUF 文件，支持进度回调（REQ-LLM-004 AC-2）。
    ///
    /// # 安全
    /// - 仅允许 `https://` 前缀的 URL
    /// - 下载先写入 `.tmp` 临时文件，完成后原子重命名
    /// - 文件名通过 `sanitize_filename()` 校验
    ///
    /// # 参数
    /// - `url`: 下载地址（必须 `https://`）
    /// - `filename`: 保存的文件名（仅文件名，不含路径）
    /// - `progress`: 进度回调（已下载字节, 总字节, 速度字节/秒）
    pub async fn download_model(
        &self,
        url: &str,
        filename: &str,
        progress: DownloadProgressFn,
    ) -> Result<()> {
        // 安全校验：仅允许 HTTPS
        if !url.starts_with("https://") {
            bail!("下载 URL 必须使用 HTTPS 协议: {url}");
        }

        let safe_name = Self::sanitize_filename(filename)?;
        let final_path = self.models_dir.join(&safe_name);
        let tmp_path = self.models_dir.join(format!("{safe_name}.tmp"));

        // 执行下载
        let client = reqwest::Client::builder()
            .no_proxy() // 🚫 禁止代理，确保模型下载直连（铁律一）
            .timeout(std::time::Duration::from_secs(600))
            .build()
            .context("创建 HTTP 客户端失败")?;

        let response = client
            .get(url)
            .send()
            .await
            .context(format!("请求下载失败: {url}"))?;

        if !response.status().is_success() {
            bail!(
                "下载失败: HTTP {} — {}",
                response.status(),
                response.status().canonical_reason().unwrap_or("未知错误")
            );
        }

        let total = response.content_length().unwrap_or(0);

        // 流式写入临时文件
        let mut file = tokio::fs::File::create(&tmp_path)
            .await
            .context(format!("创建临时文件失败: {}", tmp_path.display()))?;

        let mut stream = response.bytes_stream();
        let mut downloaded: u64 = 0;
        let start_time = std::time::Instant::now();
        let mut last_progress_time = std::time::Instant::now();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.context("读取下载数据流失败")?;
            file.write_all(&chunk).await.context("写入下载文件失败")?;
            downloaded += chunk.len() as u64;

            // 每 100ms 推送一次进度，确保前端进度条平滑推进
            if last_progress_time.elapsed() >= std::time::Duration::from_millis(100) {
                let elapsed = start_time.elapsed().as_secs_f64();
                let speed = if elapsed > 0.0 {
                    (downloaded as f64 / elapsed) as u64
                } else {
                    0
                };
                progress(downloaded, total, speed);
                last_progress_time = std::time::Instant::now();
            }
        }

        file.flush().await.context("刷新下载文件失败")?;
        drop(file);

        // 最终进度推送
        progress(downloaded, total, downloaded);

        // 原子重命名
        tokio::fs::rename(&tmp_path, &final_path)
            .await
            .context(format!(
                "重命名临时文件失败: {} → {}",
                tmp_path.display(),
                final_path.display()
            ))?;

        Ok(())
    }

    /// 从文件路径解析模型元信息。
    ///
    /// 提取文件名中的架构、参数规模、量化格式等信息。
    fn parse_model_info(path: &Path) -> Option<ModelInfo> {
        let filename = path.file_name()?.to_str()?.to_string();
        let metadata = std::fs::metadata(path).ok()?;
        let size_bytes = metadata.len();

        Some(ModelInfo {
            filename: filename.clone(),
            path: path.to_str()?.to_string(),
            size_bytes,
            architecture: Self::parse_architecture(&filename),
            param_size: Self::parse_param_size(&filename),
            quantization: Self::parse_quantization(&filename),
        })
    }

    /// 安全校验文件名（防目录穿越）。
    ///
    /// 拒绝包含 `/`、`\`、`..` 的文件名。
    pub(crate) fn sanitize_filename(filename: &str) -> Result<String> {
        if filename.is_empty() {
            bail!("文件名不能为空");
        }
        if filename.contains('/') || filename.contains('\\') {
            bail!("文件名不能包含路径分隔符: {filename}");
        }
        if filename.contains("..") {
            bail!("文件名不能包含 '..': {filename}");
        }
        // 拒绝空字节
        if filename.contains('\0') {
            bail!("文件名不能包含空字节");
        }
        Ok(filename.to_string())
    }

    /// 从文件名推断模型架构。
    ///
    /// 支持的格式：
    /// - `qwen2.5-...` → `qwen2.5`
    /// - `llama-3.2-...` / `llama3.2-...` → `llama3.2`
    /// - `phi-3.5-...` / `phi3.5-...` → `phi3.5`
    /// - `mistral-7b-...` → `mistral`
    /// - 其他 → `unknown`
    pub(crate) fn parse_architecture(filename: &str) -> String {
        let lower = filename.to_lowercase();
        if lower.starts_with("qwen") {
            "qwen2.5".to_string()
        } else if lower.starts_with("llama") {
            "llama3.2".to_string()
        } else if lower.starts_with("phi") {
            "phi3.5".to_string()
        } else if lower.starts_with("mistral") {
            "mistral".to_string()
        } else if lower.starts_with("gemma") {
            "gemma2".to_string()
        } else {
            "unknown".to_string()
        }
    }

    /// 从文件名推断参数规模。
    ///
    /// 支持的格式：
    /// - `...-7b-...` / `...-7B-...` → `7B`
    /// - `...-3b-...` → `3B`
    /// - `...-3.8b-...` → `3.8B`
    /// - `...-13b-...` → `13B`
    /// - `...-70b-...` → `70B`
    /// - 其他 → `unknown`
    pub(crate) fn parse_param_size(filename: &str) -> String {
        let lower = filename.to_lowercase();
        // 仅按 '-' 和 '_' 分割（不按 '.' 分割，因为参数规模可能含小数点如 3.8b）
        for part in lower.split(['-', '_']) {
            // 去除可能的 .gguf 后缀
            let part = part.strip_suffix(".gguf").unwrap_or(part);
            if let Some(num_str) = part.strip_suffix('b')
                && num_str.parse::<f64>().is_ok()
            {
                return format!("{}B", num_str);
            }
        }
        "unknown".to_string()
    }

    /// 从文件名推断量化格式。
    ///
    /// 支持的格式：
    /// - `...q4_k_m...` → `Q4_K_M`
    /// - `...q5_k_m...` → `Q5_K_M`
    /// - `...q8_0...` → `Q8_0`
    /// - `...q4_0...` → `Q4_0`
    /// - `...q4_k_s...` → `Q4_K_S`
    /// - `...f16...` → `F16`
    /// - 其他 → `unknown`
    pub fn parse_quantization(filename: &str) -> String {
        let lower = filename.to_lowercase();
        if lower.contains("q4_k_m") {
            "Q4_K_M".to_string()
        } else if lower.contains("q5_k_m") {
            "Q5_K_M".to_string()
        } else if lower.contains("q6_k") {
            "Q6_K".to_string()
        } else if lower.contains("q4_k_s") {
            "Q4_K_S".to_string()
        } else if lower.contains("q5_k_s") {
            "Q5_K_S".to_string()
        } else if lower.contains("q8_0") {
            "Q8_0".to_string()
        } else if lower.contains("q4_0") {
            "Q4_0".to_string()
        } else if lower.contains("q5_0") {
            "Q5_0".to_string()
        } else if lower.contains("f16") {
            "F16".to_string()
        } else if lower.contains("f32") {
            "F32".to_string()
        } else {
            "unknown".to_string()
        }
    }
}
