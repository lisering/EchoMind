//! 本地 OCR 引擎（REQ-MM-002）：ocrs + rten 纯 Rust PaddleOCR PP-OCRv4。
//!
//! 模型在首次使用时惰性下载和加载（~17MB），之后全部在本地 CPU 上运行。
//! 零网络请求（符合「隐私不出域」承诺，ADR-010 安全官要求）。
//!
//! ## 模型
//!
//! 使用 ocrs 项目预转换的 `.rten` 格式模型：
//! - 检测模型（`zh-cn.det-model.rten`，~4.7MB）：定位图片中的文字区域
//! - 识别模型（`zh-cn.rec-model.rten`，~11.8MB）：将文字区域图像识别为文本
//! - 支持中文 + 英文混合识别
//!
//! 来源：rs-pro `crates/core/src/ocr.rs` 的 `LocalOcrEngine`，适配 EchoMind 架构。

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, bail};
use echomind_core::OcrEngine;
use tokio::sync::RwLock;

/// OCR 模型下载源（ocrs GitHub Releases v0.8.0）
const OCRS_MODEL_BASE_URL: &str = "https://github.com/robertknight/ocrs/releases/download/v0.8.0";
/// 中文+英文检测模型文件名
const DET_MODEL_FILENAME: &str = "zh-cn.det-model.rten";
/// 中文+英文识别模型文件名
const REC_MODEL_FILENAME: &str = "zh-cn.rec-model.rten";

/// 本地 OCR 引擎适配器（实现 `OcrEngine` 端口）。
///
/// 基于 `ocrs` crate（PaddleOCR PP-OCRv4 + rten 纯 Rust 推理引擎）。
/// 模型在首次使用时惰性下载和加载（~17MB），之后全部在本地 CPU 上运行。
///
/// # 隐私
///
/// ✅ 完全本地运行，图片数据不离开设备，零网络请求。
///
/// # 惰性加载
///
/// 模型不在构造时加载。需调用 [`OcrsEngine::init`] 下载模型并初始化引擎。
/// 未初始化时 `recognize()` 返回空字符串（优雅降级）。
#[derive(Clone)]
pub struct OcrsEngine {
    /// 模型缓存目录（与 fastembed cache 同级）
    model_dir: PathBuf,
    /// 惰性初始化的 ocrs 引擎（None = 未初始化）
    inner: Arc<RwLock<Option<ocrs::OcrEngine>>>,
}

impl OcrsEngine {
    /// 创建本地 OCR 引擎（未初始化状态）。
    ///
    /// # 参数
    /// - `model_dir`: 模型文件缓存目录（模型将下载到此目录）
    #[must_use]
    pub fn new(model_dir: PathBuf) -> Self {
        Self {
            model_dir,
            inner: Arc::new(RwLock::new(None)),
        }
    }

    /// 初始化本地 OCR 引擎：下载模型（首次）+ 加载模型到内存。
    ///
    /// 幂等操作：如果已初始化则直接返回。
    ///
    /// # 错误
    /// - 模型目录创建失败
    /// - 模型文件下载失败（网络问题）
    /// - 模型加载失败（文件损坏 / rten 初始化失败）
    pub async fn init(&self) -> anyhow::Result<()> {
        // 检查是否已初始化
        {
            let guard = self.inner.read().await;
            if guard.is_some() {
                return Ok(());
            }
        }

        // 确保模型目录存在
        tokio::fs::create_dir_all(&self.model_dir)
            .await
            .context("创建 OCR 模型目录失败")?;

        // 下载模型文件（如果不存在）
        let det_path = self.model_dir.join(DET_MODEL_FILENAME);
        let rec_path = self.model_dir.join(REC_MODEL_FILENAME);
        Self::download_model_if_needed(&det_path, DET_MODEL_FILENAME).await?;
        Self::download_model_if_needed(&rec_path, REC_MODEL_FILENAME).await?;

        // 加载模型（CPU 密集，用 spawn_blocking）
        let det_path_clone = det_path.clone();
        let rec_path_clone = rec_path.clone();
        let engine = tokio::task::spawn_blocking(move || -> anyhow::Result<ocrs::OcrEngine> {
            let detection_model = rten::Model::load_file(&det_path_clone)
                .map_err(|e| anyhow::anyhow!("加载检测模型失败: {e}"))?;
            let recognition_model = rten::Model::load_file(&rec_path_clone)
                .map_err(|e| anyhow::anyhow!("加载识别模型失败: {e}"))?;
            ocrs::OcrEngine::new(ocrs::OcrEngineParams {
                detection_model: Some(detection_model),
                recognition_model: Some(recognition_model),
                ..Default::default()
            })
            .map_err(|e| anyhow::anyhow!("创建 ocrs 引擎失败: {e}"))
        })
        .await
        .context("OCR 模型加载任务失败")??;

        // 存储引擎实例
        let mut guard = self.inner.write().await;
        *guard = Some(engine);
        Ok(())
    }

    /// 下载单个模型文件（如果不存在）。
    ///
    /// 使用 async reqwest 下载（I/O 密集，非 CPU 密集）。
    async fn download_model_if_needed(
        dest: &std::path::Path,
        filename: &str,
    ) -> anyhow::Result<()> {
        if dest.exists() {
            return Ok(());
        }

        let url = format!("{OCRS_MODEL_BASE_URL}/{filename}");
        let client = reqwest::Client::builder()
            .no_proxy() // 🚫 禁止代理，确保 OCR 模型下载直连（铁律一）
            .connect_timeout(Duration::from_secs(30))
            .timeout(Duration::from_secs(300))
            .build()
            .context("创建 OCR 模型下载客户端失败")?;

        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("下载 {filename} 失败: {e}"))?;

        if !resp.status().is_success() {
            bail!("下载 {filename} 失败: HTTP {}", resp.status());
        }

        let bytes = resp
            .bytes()
            .await
            .map_err(|e| anyhow::anyhow!("读取 {filename} 响应体失败: {e}"))?;

        tokio::fs::write(dest, &bytes)
            .await
            .with_context(|| format!("写入 {filename} 失败"))?;

        Ok(())
    }

    /// 使用 ocrs 引擎执行 OCR（内部方法，同步，在 spawn_blocking 内调用）。
    ///
    /// 流程：解码图片 → RGB8 转换 → ocrs 检测 → 分行 → 识别 → 后处理
    fn run_ocr(engine: &ocrs::OcrEngine, image_data: &[u8]) -> anyhow::Result<String> {
        // 解码图片
        let img = image::load_from_memory(image_data)
            .map_err(|e| anyhow::anyhow!("解码图片失败: {e}"))?;

        let rgb_img = img.to_rgb8();
        let (width, height) = rgb_img.dimensions();

        // 构造 ocrs 输入
        let image_source = ocrs::ImageSource::from_bytes(rgb_img.as_raw(), (width, height))
            .map_err(|e| anyhow::anyhow!("OCR 图片源构造失败: {e}"))?;
        let ocr_input = engine
            .prepare_input(image_source)
            .map_err(|e| anyhow::anyhow!("OCR 预处理失败: {e}"))?;

        // 文字检测
        let words = engine
            .detect_words(&ocr_input)
            .map_err(|e| anyhow::anyhow!("OCR 文字检测失败: {e}"))?;

        // 将检测到的文字区域分组为行（阅读顺序）
        let lines = engine.find_text_lines(&ocr_input, &words);

        // 文字识别
        let recognized_lines = engine
            .recognize_text(&ocr_input, &lines)
            .map_err(|e| anyhow::anyhow!("OCR 文字识别失败: {e}"))?;

        // 拼接所有文本行 + 后处理
        let raw_text: Vec<String> = recognized_lines
            .iter()
            .flatten()
            .map(|line| line.to_string())
            .collect();

        Ok(Self::postprocess_text(&raw_text))
    }

    /// OCR 文本后处理：去除行首尾空白 + 合并连续空行（最多保留一个）。
    fn postprocess_text(lines: &[String]) -> String {
        let mut result = String::new();
        let mut prev_empty = false;

        for line in lines {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                if !prev_empty && !result.is_empty() {
                    result.push('\n');
                }
                prev_empty = true;
            } else {
                if !result.is_empty() {
                    result.push('\n');
                }
                result.push_str(trimmed);
                prev_empty = false;
            }
        }

        result.trim_end_matches('\n').to_string()
    }
}

impl OcrEngine for OcrsEngine {
    async fn recognize(&self, image_bytes: &[u8]) -> anyhow::Result<String> {
        let inner = self.inner.clone();
        let image_data = image_bytes.to_vec();

        // OCR 推理是 CPU 密集型，用 spawn_blocking + blocking_read 避免跨 await 持锁
        let result = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
            let guard = inner.blocking_read();
            let Some(ref engine) = *guard else {
                // 未初始化，返回空字符串（优雅降级）
                return Ok(String::new());
            };
            Self::run_ocr(engine, &image_data)
        })
        .await
        .context("OCR 推理任务执行失败")?;

        // OCR 失败时优雅降级为空字符串，不崩溃
        match result {
            Ok(text) => Ok(text),
            Err(e) => {
                eprintln!("[OcrsEngine] OCR 失败: {e}");
                Ok(String::new())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn test_new_creates_uninitialized_engine() {
        let engine = OcrsEngine::new(PathBuf::from("/tmp/test_ocr_models"));
        // 未初始化时 inner 应为 None
        match engine.inner.try_read() {
            Ok(guard) => assert!(guard.is_none(), "新引擎的 inner 应为 None"),
            Err(e) => panic!("try_read 失败: {e}"),
        }
    }

    #[test]
    fn test_postprocess_text_basic() {
        let lines = vec!["  Hello World  ".to_string(), "  Foo Bar  ".to_string()];
        let result = OcrsEngine::postprocess_text(&lines);
        assert_eq!(result, "Hello World\nFoo Bar");
    }

    #[test]
    fn test_postprocess_text_empty_lines_collapse() {
        let lines = vec![
            "Line 1".to_string(),
            "".to_string(),
            "".to_string(),
            "".to_string(),
            "Line 2".to_string(),
        ];
        let result = OcrsEngine::postprocess_text(&lines);
        assert_eq!(result, "Line 1\n\nLine 2");
    }

    #[test]
    fn test_postprocess_text_trailing_empty_lines() {
        let lines = vec!["Line 1".to_string(), "".to_string(), "".to_string()];
        let result = OcrsEngine::postprocess_text(&lines);
        assert_eq!(result, "Line 1");
    }

    #[test]
    fn test_postprocess_text_all_empty() {
        let lines = vec!["".to_string(), "".to_string(), "".to_string()];
        let result = OcrsEngine::postprocess_text(&lines);
        assert_eq!(result, "");
    }

    #[test]
    fn test_postprocess_text_preserves_paragraph_structure() {
        let lines = vec![
            "Title".to_string(),
            "".to_string(),
            "Paragraph 1 line 1".to_string(),
            "Paragraph 1 line 2".to_string(),
            "".to_string(),
            "Paragraph 2".to_string(),
        ];
        let result = OcrsEngine::postprocess_text(&lines);
        assert_eq!(
            result,
            "Title\n\nParagraph 1 line 1\nParagraph 1 line 2\n\nParagraph 2"
        );
    }

    #[tokio::test]
    async fn test_recognize_returns_empty_before_init() {
        let engine = OcrsEngine::new(PathBuf::from("/tmp/test_ocr_models"));
        let result = engine
            .recognize(&[0x89, 0x50, 0x4E, 0x47])
            .await
            .expect("未初始化时不应报错");
        assert!(result.is_empty(), "未初始化时应返回空字符串");
    }
}
