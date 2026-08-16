//! 模型管理 Store — 嵌入/重排序/本地LLM/模型文件管理。
//!
//! 借鉴 Zed `buffer_store` + `agent` 的 Store 模式：
//! ML 模型实例和懒加载逻辑封装在独立 Store 中。

use std::path::PathBuf;

use echomind_core::Storage;
use echomind_infra::local_embedder::{EmbeddingModel, LocalEmbedder};
use echomind_infra::local_reranker::LocalReranker;
use echomind_infra::model_manager::ModelManager;
use echomind_infra::robust_downloader::RobustDownloader;
use echomind_infra::sqlite_storage::SqliteStorage;

// Pro 模块导入
#[cfg(feature = "pro")]
use echomind_infra::local_llm::LocalLlmEngine;
#[cfg(feature = "pro")]
use echomind_infra::ocrs_engine::OcrsEngine;
#[cfg(feature = "pro")]
use echomind_infra::pdfium_renderer::PdfiumRenderer;

/// 模型管理 Store（嵌入/重排序/本地LLM/PDF渲染/OCR/模型文件管理）。
///
/// # 职责
/// - 向量化引擎（懒加载，首次使用时初始化）
/// - Cross-Encoder 重排序引擎（懒加载）
/// - PDF 页面渲染引擎（Pro，懒加载）
/// - OCR 引擎（Pro，懒加载）
/// - 本地 LLM 引擎实例（Pro，懒加载）
/// - 本地 GGUF 模型文件管理
///
/// # 线程安全
/// 使用 `tokio::sync::RwLock`/`OnceCell` 实现懒加载和双重检查。
pub struct ModelStore {
    /// 共享存储引用（用于读取 settings 中的模型配置）
    storage: SqliteStorage,
    /// 应用数据目录（模型缓存根目录）
    data_dir: PathBuf,
    /// 向量化引擎（首次使用时初始化：模型下载可能耗时）
    embedder: tokio::sync::RwLock<Option<LocalEmbedder>>,
    /// Cross-Encoder 重排序引擎（首次使用时初始化）
    reranker: tokio::sync::OnceCell<LocalReranker>,
    /// PDF 页面渲染引擎（首次使用时初始化，REQ-MM-001）
    #[cfg(feature = "pro")]
    page_renderer: tokio::sync::OnceCell<PdfiumRenderer>,
    /// OCR 引擎（首次使用时初始化，REQ-MM-002）
    #[cfg(feature = "pro")]
    ocr_engine: tokio::sync::OnceCell<OcrsEngine>,
    /// 本地 LLM 模型文件管理器（REQ-LLM-004）
    model_manager: ModelManager,
    /// 健壮下载器（REQ-LLM-004 v2：断点续传 + 多源容错 + 并发分块 + 崩溃恢复）
    robust_downloader: RobustDownloader,
    /// 本地 LLM 引擎实例（Pro 门控，懒加载）
    #[cfg(feature = "pro")]
    local_llm: tokio::sync::RwLock<Option<LocalLlmEngine>>,
}

impl ModelStore {
    /// 创建新的模型管理 Store。
    pub fn new(storage: SqliteStorage, data_dir: PathBuf) -> anyhow::Result<Self> {
        let model_manager = ModelManager::new(&data_dir)?;
        let robust_downloader = RobustDownloader::new(data_dir.join("models").join("llm"))?;
        Ok(Self {
            storage,
            data_dir,
            embedder: tokio::sync::RwLock::new(None),
            reranker: tokio::sync::OnceCell::new(),
            #[cfg(feature = "pro")]
            page_renderer: tokio::sync::OnceCell::new(),
            #[cfg(feature = "pro")]
            ocr_engine: tokio::sync::OnceCell::new(),
            model_manager,
            robust_downloader,
            #[cfg(feature = "pro")]
            local_llm: tokio::sync::RwLock::new(None),
        })
    }

    /// 获取存储引用。
    pub fn storage(&self) -> &SqliteStorage {
        &self.storage
    }

    /// 获取数据目录引用。
    pub fn data_dir(&self) -> &PathBuf {
        &self.data_dir
    }

    /// 获取模型文件管理器引用（REQ-LLM-004）。
    pub fn model_manager(&self) -> &ModelManager {
        &self.model_manager
    }

    /// 获取健壮下载器引用（REQ-LLM-004 v2）。
    pub fn robust_downloader(&self) -> &RobustDownloader {
        &self.robust_downloader
    }

    // ---- Embedder 懒加载 ----

    /// 懒加载向量化引擎（首次调用时初始化，模型下载在阻塞线程池执行）。
    ///
    /// 支持预设模型和自定义 ONNX 模型（REQ-VEC-014，Pro 门控）。
    /// 自定义模型通过 `new_with_custom_model()` 加载，维度动态检测。
    pub async fn embedder(&self) -> anyhow::Result<LocalEmbedder> {
        // 快路径：已初始化
        {
            let read = self.embedder.read().await;
            if let Some(e) = read.as_ref() {
                return Ok(e.clone());
            }
        }
        // 慢路径：初始化
        let model = self.load_embedding_model().await;
        let new_embedder = match &model {
            EmbeddingModel::Custom(name) => {
                // 自定义模型路径（REQ-VEC-014，Pro 门控）
                let model_dir = self.data_dir.join("custom_models").join(name);
                LocalEmbedder::new_with_custom_model(model_dir).await?
            }
            _ => {
                // 预设模型路径
                let cache_dir = self.data_dir.join("models");
                LocalEmbedder::new_with_model(cache_dir, model.clone()).await?
            }
        };
        {
            let mut write = self.embedder.write().await;
            if write.is_none() {
                *write = Some(new_embedder.clone());
            }
        }
        let read = self.embedder.read().await;
        match read.as_ref() {
            Some(e) => Ok(e.clone()),
            None => anyhow::bail!("向量化引擎初始化后仍为空（不可达）"),
        }
    }

    /// 检查向量化引擎是否已初始化（REQ-VEC-008-AC-4）。
    pub async fn embedder_initialized(&self) -> bool {
        self.embedder.read().await.is_some()
    }

    /// 从 settings 表读取嵌入模型标识（REQ-VEC-012 + REQ-VEC-014）。
    ///
    /// 支持预设模型名称和 `custom:{name}` 前缀的自定义模型。
    async fn load_embedding_model(&self) -> EmbeddingModel {
        match self.storage.get_setting("vec.embedding_model").await {
            Ok(Some(s)) => parse_embedding_model(&s),
            _ => EmbeddingModel::BgeSmallEnV1_5, // P1-5: 默认升级为 bge-small-en-v1.5
        }
    }

    /// 切换嵌入模型（REQ-VEC-012）。
    pub async fn set_embedding_model(&self, model_str: &str) -> anyhow::Result<()> {
        self.storage
            .set_setting("vec.embedding_model", model_str)
            .await?;
        let mut write = self.embedder.write().await;
        *write = None;
        Ok(())
    }

    /// 带进度回调初始化向量化引擎（REQ-VEC-008）。
    ///
    /// 自定义模型（REQ-VEC-014）不支持下载进度回调（文件已在本地），
    /// 直接走 `new_with_custom_model()` 路径。
    pub async fn init_embedder_with_progress(
        &self,
        progress: echomind_infra::local_embedder::DownloadProgressFn,
    ) -> anyhow::Result<()> {
        {
            let read = self.embedder.read().await;
            if read.is_some() {
                return Ok(());
            }
        }
        let model = self.load_embedding_model().await;
        let new_embedder = match &model {
            EmbeddingModel::Custom(name) => {
                // 自定义模型路径（文件已在本地，不需要下载进度）
                let model_dir = self.data_dir.join("custom_models").join(name);
                echomind_infra::local_embedder::LocalEmbedder::new_with_custom_model(model_dir)
                    .await?
            }
            _ => {
                let cache_dir = self.data_dir.join("models");
                echomind_infra::local_embedder::LocalEmbedder::new_with_progress(
                    cache_dir,
                    model.clone(),
                    Some(progress),
                )
                .await?
            }
        };
        let mut write = self.embedder.write().await;
        if write.is_none() {
            *write = Some(new_embedder);
        }
        Ok(())
    }

    // ---- Reranker 懒加载 ----

    /// 懒加载 Cross-Encoder 重排序引擎（REQ-RAG-020）。
    pub async fn reranker(&self) -> anyhow::Result<&LocalReranker> {
        self.reranker
            .get_or_try_init(|| async { LocalReranker::new(self.data_dir.join("models")).await })
            .await
    }

    /// 检查重排序引擎是否已初始化。
    pub fn reranker_initialized(&self) -> bool {
        self.reranker.initialized()
    }

    // ---- Pro: 本地 LLM 引擎 ----

    /// 懒加载本地 LLM 引擎（Pro 门控，REQ-LLM-003）。
    #[cfg(feature = "pro")]
    pub async fn local_llm(&self) -> anyhow::Result<LocalLlmEngine> {
        // 快路径
        {
            let read = self.local_llm.read().await;
            if let Some(engine) = read.as_ref() {
                return Ok(engine.clone());
            }
        }
        // 慢路径
        let model_name = self
            .storage
            .get_setting("llm.local_model")
            .await?
            .unwrap_or_default();
        if model_name.is_empty() {
            anyhow::bail!("未选择本地模型，请先在设置中选择一个模型");
        }
        let model_path = self.model_manager.model_path(&model_name)?;
        let quantization = echomind_infra::local_llm::Quantization::from_str(
            &echomind_infra::model_manager::ModelManager::parse_quantization(&model_name),
        );
        let mut engine = LocalLlmEngine::new(model_path, quantization)?;

        // 读取 PagedAttention 配置
        let paged_attn = self
            .storage
            .get_setting("llm.paged_attn")
            .await?
            .is_some_and(|v| v == "true");
        let block_size = self
            .storage
            .get_setting("llm.block_size")
            .await?
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(32);
        let gpu_memory_ctx = self
            .storage
            .get_setting("llm.gpu_memory_ctx")
            .await?
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(4096);

        if paged_attn {
            engine = engine.with_paged_attn(block_size, gpu_memory_ctx);
        }

        // 读取采样参数
        let sampling_json = self.storage.get_setting("llm.sampling").await?;
        if let Some(json_str) = sampling_json
            && let Ok(params) =
                serde_json::from_str::<echomind_models::LlmSamplingParams>(&json_str)
        {
            engine.set_sampling_params(params).await;
        }

        // Phase 3：读取内核模式（mistral / custom）
        let kernel_mode_str = self.storage.get_setting("llm.kernel_mode").await?;
        if let Some(mode_str) = kernel_mode_str
            && let Ok(mode) = echomind_infra::local_llm::KernelMode::from_str(&mode_str)
        {
            engine.set_kernel_mode(mode).await;
        }

        let mut write = self.local_llm.write().await;
        if write.is_none() {
            *write = Some(engine.clone());
        }
        match write.as_ref() {
            Some(e) => Ok(e.clone()),
            None => anyhow::bail!("本地 LLM 引擎初始化后仍为空（不可达）"),
        }
    }

    /// 卸载本地 LLM 引擎（释放模型内存 + 销毁引擎实例）。
    #[cfg(feature = "pro")]
    pub async fn unload_local_llm(&self) {
        let mut write = self.local_llm.write().await;
        if let Some(engine) = write.as_ref() {
            engine.unload().await;
        }
        *write = None;
    }

    // ---- Pro: PDF 渲染 ----

    /// 懒加载 PDF 页面渲染引擎（REQ-MM-001）。
    #[cfg(feature = "pro")]
    pub async fn page_renderer(&self) -> anyhow::Result<&PdfiumRenderer> {
        self.page_renderer
            .get_or_try_init(|| async { Ok(PdfiumRenderer::new()) })
            .await
    }

    // ---- Pro: OCR 引擎 ----

    /// 懒加载 OCR 引擎（REQ-MM-002）。
    #[cfg(feature = "pro")]
    pub async fn ocr_engine(&self) -> anyhow::Result<&OcrsEngine> {
        self.ocr_engine
            .get_or_try_init(|| async {
                let engine = OcrsEngine::new(self.data_dir.join("models").join("ocrs"));
                engine.init().await?;
                Ok(engine)
            })
            .await
    }
}

/// 解析嵌入模型标识字符串为 `EmbeddingModel` 枚举（REQ-VEC-012 + REQ-VEC-014）。
///
/// 支持预设模型名称和 `custom:{name}` 前缀的自定义模型。
/// 例如：
/// - `"bge-small-en-v1.5"` → `EmbeddingModel::BgeSmallEnV1_5`（P1-5 新增默认模型）
/// - `"all-MiniLM-L6-v2"` → `EmbeddingModel::AllMiniLML6V2`
/// - `"custom:my-bge-model"` → `EmbeddingModel::Custom("my-bge-model")`
pub fn parse_embedding_model(s: &str) -> EmbeddingModel {
    if let Some(name) = s.strip_prefix("custom:") {
        return EmbeddingModel::Custom(name.to_string());
    }
    match s {
        "bge-small-en-v1.5" => EmbeddingModel::BgeSmallEnV1_5,
        "all-MiniLM-L6-v2" => EmbeddingModel::AllMiniLML6V2,
        "bge-small-zh-v1.5" => EmbeddingModel::BgeSmallZhV1_5,
        "e5-small-v2" => EmbeddingModel::E5SmallV2,
        "bge-base-en-v1.5" => EmbeddingModel::BgeBaseEnV1_5,
        _ => EmbeddingModel::BgeSmallEnV1_5, // P1-5: 默认升级为 bge-small-en-v1.5
    }
}
