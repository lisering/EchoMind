//! 本地 Cross-Encoder 重排序引擎（REQ-RAG-020）：fastembed + ONNX。
//!
//! 使用 BAAI/bge-reranker-base 模型（BERT-based Cross-Encoder），
//! 对 query-document 对逐对打分，按精确相关性重排检索候选结果。
//!
//! ## 模型信息
//!
//! - 模型：BAAI/bge-reranker-base（中英文双语 Cross-Encoder）
//! - 大小：ONNX ~280MB（量化后更小）
//! - 输入：query + document 文本对
//! - 输出：相关性分数（float32，越高越相关）
//! - 推理：CPU 密集，经 `spawn_blocking` 执行
//!
//! ## 下载策略
//!
//! 复用 `LocalEmbedder` 的多源容错下载模式：
//! - 逐文件先 `huggingface.co`，失败回退 `hf-mirror.com`
//! - 全量 GET（不发 Range 头）
//! - ONNX + tokenizer 文件共 5 个
//!
//! ## 与 Embedder 的区别
//!
//! - Embedder 输入单条文本，输出向量（用于检索）
//! - Reranker 输入 query-document 对，输出分数（用于重排序）
//! - Reranker 推理更重（每对 query-document 需一次前向传播）
//! - 单会话即可（reranking 频率低，仅在 chat 时调用，不需要并行池）

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, bail};
use echomind_core::Reranker;
use echomind_models::RetrievalResult;
use fastembed::{
    OnnxSource, RerankInitOptionsUserDefined, TextRerank, TokenizerFiles, UserDefinedRerankingModel,
};

/// 下载源定义（与 LocalEmbedder 一致，3 源容错）
struct DownloadSource {
    name: &'static str,
    base: &'static str,
    branch: &'static str,
}

const DOWNLOAD_SOURCES: &[DownloadSource] = &[
    DownloadSource {
        name: "HuggingFace",
        base: "https://huggingface.co",
        branch: "main",
    },
    DownloadSource {
        name: "ModelScope",
        base: "https://modelscope.cn/models",
        branch: "master",
    },
    DownloadSource {
        name: "hf-mirror",
        base: "https://hf-mirror.com",
        branch: "main",
    },
];

/// 校验已下载文件是否完好（与 LocalEmbedder 一致的完整性校验）。
fn is_reranker_file_valid(path: &Path, local_name: &str) -> bool {
    if !path.exists() {
        return false;
    }
    let content = match std::fs::read(path) {
        Ok(c) => c,
        Err(_) => return false,
    };
    if content.is_empty() {
        return false;
    }
    if local_name.ends_with(".json") {
        serde_json::from_slice::<serde_json::Value>(&content).is_ok()
    } else if local_name.ends_with(".onnx") {
        let preview = &content[..content.len().min(20)];
        let preview_str = String::from_utf8_lossy(preview);
        !(preview_str.starts_with("<!")
            || preview_str.starts_with("<html")
            || preview_str.starts_with("<HTML"))
    } else {
        true
    }
}

/// HuggingFace 模型仓库
const RERANKER_REPO: &str = "BAAI/bge-reranker-base";

/// 模型文件清单：(仓库内路径, 本地文件名)
const RERANKER_FILES: [(&str, &str); 5] = [
    ("onnx/model.onnx", "model.onnx"),
    ("tokenizer.json", "tokenizer.json"),
    ("config.json", "config.json"),
    ("special_tokens_map.json", "special_tokens_map.json"),
    ("tokenizer_config.json", "tokenizer_config.json"),
];

/// 本地模型目录名
const RERANKER_DIR_NAME: &str = "bge-reranker-base";

/// 默认 batch size（Cross-Encoder 批量推理）
const DEFAULT_BATCH_SIZE: usize = 32;

/// 本地 Cross-Encoder 重排序适配器。
///
/// 使用 `fastembed::TextRerank`（ONNX Runtime）进行本地推理。
/// 克隆廉价（Arc 共享模型实例），可在多线程间安全共享。
///
/// # 性能特征
///
/// - bge-reranker-base 量化模型：~280MB 内存
/// - 单次 rerank（25 对 query-document）：CPU ~200ms
/// - 首次使用需下载模型文件（~280MB）
#[derive(Clone)]
pub struct LocalReranker {
    /// ONNX 推理会话（`TextRerank::rerank` 需要 `&mut self`，用 Mutex 保护）
    session: Arc<Mutex<TextRerank>>,
}

impl LocalReranker {
    /// 初始化 Cross-Engineer 重排序引擎。
    ///
    /// 首次使用时下载模型文件（~280MB），后续从本地缓存加载。
    /// 推理为 CPU 密集任务，一律经 `spawn_blocking` 执行。
    ///
    /// # 参数
    /// - `cache_dir`: 模型缓存目录（与 `LocalEmbedder` 共享，模型存于子目录）
    ///
    /// # 错误
    /// - 模型文件下载失败（双侧源均不可达）
    /// - ONNX 会话构建失败
    pub async fn new(cache_dir: PathBuf) -> anyhow::Result<Self> {
        tokio::task::spawn_blocking(move || Self::init(&cache_dir))
            .await
            .context("重排序引擎初始化任务失败")?
    }

    /// 初始化逻辑（阻塞线程执行）。
    fn init(cache_dir: &Path) -> anyhow::Result<Self> {
        let model_dir = cache_dir.join(RERANKER_DIR_NAME);
        Self::ensure_model_files(&model_dir)?;
        let session = Self::load_from_files(&model_dir).context("重排序模型会话构建失败")?;
        Ok(Self {
            session: Arc::new(Mutex::new(session)),
        })
    }

    /// 确保模型文件齐备：全量 GET + 多源容错 + 完整性校验。
    fn ensure_model_files(model_dir: &Path) -> anyhow::Result<()> {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(30))
            .build()
            .context("创建重排序模型下载客户端失败")?;

        let mut prefer_source: Option<usize> = None;
        for (repo_path, local_name) in RERANKER_FILES {
            let dest = model_dir.join(local_name);

            // 已存在文件：校验完整性，损坏则删除重下
            if dest.exists() {
                if is_reranker_file_valid(&dest, local_name) {
                    continue;
                }
                eprintln!("[LocalReranker] 缓存文件 {local_name} 损坏，重新下载");
                let _ = std::fs::remove_file(&dest);
            }

            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("创建重排序模型目录失败: {}", parent.display()))?;
            }

            let mut order: Vec<usize> = (0..DOWNLOAD_SOURCES.len()).collect();
            if let Some(pref) = prefer_source {
                order.sort_by_key(|&i| if i == pref { 0 } else { 1 + i });
            }

            let mut failures = Vec::new();
            let mut downloaded = false;

            for &idx in &order {
                let src = &DOWNLOAD_SOURCES[idx];
                let url = format!(
                    "{}/{RERANKER_REPO}/resolve/{}/{}",
                    src.base, src.branch, repo_path
                );

                match Self::download_file_full(&client, &url, &dest, local_name) {
                    Ok(()) => {
                        prefer_source = Some(idx);
                        downloaded = true;
                        break;
                    }
                    Err(err) => {
                        eprintln!("[LocalReranker] {} 源下载失败: {url} → {err}", src.name);
                        failures.push(format!("{}: {err}", src.name));
                        let _ = std::fs::remove_file(&dest);
                    }
                }
            }

            if !downloaded {
                bail!(
                    "重排序模型文件 {repo_path} 全部源均下载失败：{}",
                    failures.join("；")
                );
            }
        }
        Ok(())
    }

    /// 下载单个文件（全量 GET + 完整性校验）。
    fn download_file_full(
        client: &reqwest::blocking::Client,
        url: &str,
        dest: &Path,
        file_name: &str,
    ) -> anyhow::Result<()> {
        let resp = client
            .get(url)
            .send()
            .with_context(|| format!("HTTP 请求失败: {url}"))?;

        if !resp.status().is_success() {
            bail!("HTTP {} - {}", resp.status(), url);
        }

        let expected_len: Option<u64> = resp
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok());

        let bytes = resp
            .bytes()
            .with_context(|| format!("读取重排序模型响应体失败: {url}"))?;

        // Content-Length 匹配校验
        if let Some(expected) = expected_len
            && bytes.len() as u64 != expected
        {
            bail!(
                "下载不完整: 期望 {expected} 字节，实际 {} 字节",
                bytes.len()
            );
        }

        // JSON 文件格式验证
        if file_name.ends_with(".json")
            && serde_json::from_slice::<serde_json::Value>(&bytes).is_err()
        {
            let preview = String::from_utf8_lossy(&bytes[..bytes.len().min(100)]);
            bail!("下载的 JSON 文件格式无效: {file_name}, 前100字节: {preview}");
        }

        // ONNX 文件头部检查
        if file_name.ends_with(".onnx") {
            if bytes.is_empty() {
                bail!("下载的 ONNX 文件为空: {file_name}");
            }
            let preview = &bytes[..bytes.len().min(20)];
            let preview_str = String::from_utf8_lossy(preview);
            if preview_str.starts_with("<!")
                || preview_str.starts_with("<html")
                || preview_str.starts_with("<HTML")
            {
                bail!("下载的 ONNX 文件疑似 HTML 错误页: {file_name}");
            }
        }

        std::fs::write(dest, &bytes)
            .with_context(|| format!("写入重排序模型文件失败: {}", dest.display()))?;
        Ok(())
    }

    /// 从本地文件构建 ONNX 会话（fastembed 用户自定义通道）。
    fn load_from_files(model_dir: &Path) -> anyhow::Result<TextRerank> {
        let read = |name: &str| {
            std::fs::read(model_dir.join(name))
                .with_context(|| format!("读取重排序模型文件失败: {name}"))
        };
        let onnx = read("model.onnx")?;
        let tokenizer_files = TokenizerFiles {
            tokenizer_file: read("tokenizer.json")?,
            config_file: read("config.json")?,
            special_tokens_map_file: read("special_tokens_map.json")?,
            tokenizer_config_file: read("tokenizer_config.json")?,
        };
        let user_model = UserDefinedRerankingModel::new(OnnxSource::Memory(onnx), tokenizer_files);
        TextRerank::try_new_from_user_defined(user_model, RerankInitOptionsUserDefined::new())
            .context("fastembed 重排序模型会话构建失败")
    }
}

impl Reranker for LocalReranker {
    fn rerank<'a>(
        &'a self,
        query: &'a str,
        candidates: &'a [RetrievalResult],
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = anyhow::Result<Vec<RetrievalResult>>> + Send + 'a>,
    > {
        Box::pin(async move {
            if candidates.is_empty() {
                return Ok(Vec::new());
            }

            // 提取候选文档内容
            let documents: Vec<String> =
                candidates.iter().map(|c| c.chunk.content.clone()).collect();
            let query_owned = query.to_string();
            let session = Arc::clone(&self.session);

            // CPU 密集任务 → spawn_blocking
            let rerank_results = tokio::task::spawn_blocking(move || {
                let mut model = session
                    .lock()
                    .map_err(|e| anyhow::anyhow!("重排序模型锁获取失败: {e}"))?;
                model.rerank(query_owned, documents, false, Some(DEFAULT_BATCH_SIZE))
            })
            .await
            .context("重排序推理任务执行失败")?
            .context("重排序推理失败")?;

            // 将 rerank 结果映射回 RetrievalResult
            // rerank_results 按 score 降序排列，每个包含原始 index
            let mut reranked: Vec<RetrievalResult> = rerank_results
                .into_iter()
                .map(|rr| {
                    let original = &candidates[rr.index];
                    RetrievalResult {
                        chunk: original.chunk.clone(),
                        score: rr.score,
                        doc_name: original.doc_name.clone(),
                    }
                })
                .collect();

            // 确保按 Cross-Encoder 分数降序
            reranked.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            Ok(reranked)
        })
    }
}
