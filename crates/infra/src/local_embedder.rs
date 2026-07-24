//! 本地向量化引擎（REQ-VEC-002）：fastembed + all-MiniLM-L6-v2 量化模型（384 维）。
//! 下载与加载策略（经安全官/架构师评审）：
//!
//! - 自管理下载：强制全量 GET（不发送 Range 头，规避镜像源 Content-Range 报错）；
//! - 多源容错：逐文件先 `https://huggingface.co`，失败回退 `https://hf-mirror.com`；
//!   官方源一旦对某文件失败，后续文件优先镜像源（避免逐文件超时惩罚）；
//! - 加载通道：fastembed 用户自定义模型（内存字节 + TokenizerFiles），绕开 hf-hub 续传逻辑；
//! - 推理为 CPU 密集任务，一律经 `spawn_blocking` 执行；
//! - ONNX intra-op 线程数由 fastembed 内部按 `available_parallelism()` 设置（即系统核心数）。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, bail};
use echomind_core::Embedder;
use fastembed::{
    InitOptionsUserDefined, Pooling, QuantizationMode, TextEmbedding, TokenizerFiles,
    UserDefinedEmbeddingModel,
};

/// 官方模型源
const HF_OFFICIAL: &str = "https://huggingface.co";
/// 镜像模型源（官方源不可达时回退）
const HF_MIRROR: &str = "https://hf-mirror.com";
/// 模型仓库与本地目录
const MODEL_REPO: &str = "Xenova/all-MiniLM-L6-v2";
const MODEL_DIR_NAME: &str = "all-MiniLM-L6-v2";
const ONNX_FILE: &str = "model_quantized.onnx";

/// 所需模型文件清单：(仓库内路径, 本地文件名)
const MODEL_FILES: [(&str, &str); 5] = [
    ("onnx/model_quantized.onnx", ONNX_FILE),
    ("tokenizer.json", "tokenizer.json"),
    ("config.json", "config.json"),
    ("special_tokens_map.json", "special_tokens_map.json"),
    ("tokenizer_config.json", "tokenizer_config.json"),
];

/// 本地 Embedding 适配器。克隆廉价（Arc 共享模型实例）。
#[derive(Clone)]
pub struct LocalEmbedder {
    model: Arc<TextEmbedding>,
}

impl LocalEmbedder {
    /// 初始化引擎：确保模型文件齐备（缺失则全量下载），随后构建 ONNX 会话。
    /// 下载与构建为阻塞 I/O + CPU 密集，整体放入 `spawn_blocking`。
    pub async fn new(cache_dir: PathBuf) -> anyhow::Result<Self> {
        tokio::task::spawn_blocking(move || Self::init(&cache_dir))
            .await
            .context("向量化引擎初始化任务失败")?
    }

    fn init(cache_dir: &Path) -> anyhow::Result<Self> {
        let model_dir = cache_dir.join(MODEL_DIR_NAME);
        Self::ensure_model_files(&model_dir)?;
        let model = Self::load_from_files(&model_dir)?;
        Ok(Self {
            model: Arc::new(model),
        })
    }

    /// 确保模型文件齐备：仅下载缺失文件（全量 GET，多源容错）。
    fn ensure_model_files(model_dir: &Path) -> anyhow::Result<()> {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(30))
            .timeout(Duration::from_secs(600))
            .build()
            .context("创建模型下载客户端失败")?;

        let mut prefer_mirror = false;
        for (repo_path, local_name) in MODEL_FILES {
            let dest = model_dir.join(local_name);
            if dest.exists() {
                continue;
            }
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("创建模型目录失败: {}", parent.display()))?;
            }
            let mut endpoints = [HF_OFFICIAL, HF_MIRROR];
            if prefer_mirror {
                endpoints.swap(0, 1);
            }
            let used = Self::download_file(&client, repo_path, &endpoints, &dest)?;
            if used == HF_MIRROR {
                prefer_mirror = true;
            }
        }
        Ok(())
    }

    /// 下载单个文件（全量 GET，不发送 Range 头），返回实际成功的端点。
    fn download_file(
        client: &reqwest::blocking::Client,
        repo_path: &str,
        endpoints: &[&'static str; 2],
        dest: &Path,
    ) -> anyhow::Result<&'static str> {
        let mut failures = Vec::new();
        for endpoint in endpoints {
            let url = format!("{endpoint}/{MODEL_REPO}/resolve/main/{repo_path}");
            match client.get(&url).send().and_then(|r| r.error_for_status()) {
                Ok(resp) => {
                    let bytes = resp.bytes().context("读取模型响应体失败")?;
                    std::fs::write(dest, &bytes)
                        .with_context(|| format!("写入模型文件失败: {}", dest.display()))?;
                    return Ok(*endpoint);
                }
                Err(err) => {
                    eprintln!("[LocalEmbedder] {url} 下载失败，准备回退: {err}");
                    failures.push(format!("{endpoint}: {err}"));
                }
            }
        }
        bail!(
            "模型文件 {repo_path} 双侧源均下载失败：{}",
            failures.join("；")
        )
    }

    /// 从本地文件构建模型（fastembed 用户自定义通道，绕开 hf-hub 续传逻辑）。
    fn load_from_files(model_dir: &Path) -> anyhow::Result<TextEmbedding> {
        let read = |name: &str| {
            std::fs::read(model_dir.join(name)).with_context(|| format!("读取模型文件失败: {name}"))
        };
        let onnx = read(ONNX_FILE)?;
        let tokenizer_files = TokenizerFiles {
            tokenizer_file: read("tokenizer.json")?,
            config_file: read("config.json")?,
            special_tokens_map_file: read("special_tokens_map.json")?,
            tokenizer_config_file: read("tokenizer_config.json")?,
        };
        // 与 fastembed 内建 AllMiniLML6V2Q 配置对齐：Mean pooling + 动态量化
        let user_model = UserDefinedEmbeddingModel::new(onnx, tokenizer_files)
            .with_pooling(Pooling::Mean)
            .with_quantization(QuantizationMode::Dynamic);
        TextEmbedding::try_new_from_user_defined(user_model, InitOptionsUserDefined::new())
            .context("fastembed 模型会话构建失败")
    }
}

impl Embedder for LocalEmbedder {
    async fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        let mut batch = self.embed_batch(&[text.to_string()]).await?;
        match batch.pop() {
            Some(vector) => Ok(vector),
            None => anyhow::bail!("推理结果为空"),
        }
    }

    async fn embed_batch(&self, texts: &[String]) -> anyhow::Result<Vec<Vec<f32>>> {
        let model = self.model.clone();
        let owned: Vec<String> = texts.to_vec();
        tokio::task::spawn_blocking(move || model.embed(owned, None).context("批量推理失败"))
            .await
            .context("推理任务执行失败")?
    }
}
