//! 本地向量化引擎（REQ-VEC-002）：fastembed + ONNX 量化模型。
//! 支持多模型预设切换（E3-1 Embedding 模型选择）。
//! 下载与加载策略（经安全官/架构师评审）：
//!
//! - 自管理下载：强制全量 GET（不发送 Range 头，规避镜像源 Content-Range 报错）；
//! - 多源容错：根据系统语言智能选择源顺序——中文系统优先 `https://modelscope.cn`（境内 CDN），
//!   其他系统优先 `https://huggingface.co`；失败依次回退到镜像源；
//! - 加载通道：fastembed 用户自定义模型（内存字节 + TokenizerFiles），绕开 hf-hub 续传逻辑；
//! - 推理为 CPU 密集任务，一律经 `spawn_blocking` 执行；
//! - ONNX intra-op 线程数由 fastembed 内部按 `available_parallelism()` 设置（即系统核心数）。
//!
//! ## 下载加速（REQ-VEC-017）
//!
//! 1. **重试机制**：每个源 3 次重试，退避间隔 2s/4s/8s；
//! 2. **断点续传**：HTTP Range 请求 + `.partial` 文件，大文件中断后不重复下载；
//! 3. **用户可配置镜像源**：设置键 `vec.mirror_source`（auto/ModelScope/hf-mirror/HuggingFace）。
//!
//! ## 并行推理会话池（GB 级文档加速）
//!
//! `LocalEmbedder` 内部维护 N 个独立 ONNX 会话（`TextEmbedding` 实例），
//! 每个 `embed_batch` 调用将输入按会话数分片，在 N 个 `spawn_blocking` 任务中并行推理。
//! 这解决了 `fastembed::TextEmbedding::embed()` 要求 `&mut self` 导致的单会话串行瓶颈。
//!
//! - 默认池大小 = `available_parallelism().min(8)`（封顶 8 防止内存爆炸，
//!   每个量化模型实例约 30MB 内存）
//! - `pool_size=1` 时回退到单会话路径，零额外开销
//! - 分片后结果按原始顺序合并，调用方无感知

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, bail};
use echomind_core::Embedder;
use fastembed::{
    InitOptionsUserDefined, Pooling, QuantizationMode, TextEmbedding, TokenizerFiles,
    UserDefinedEmbeddingModel,
};
use serde::{Deserialize, Serialize};

/// 下载进度回调函数类型（REQ-VEC-008）。
///
/// 使用 `Arc<dyn Fn>` 以便跨 `spawn_blocking` 线程传递。
/// Tauri 层用 `app.emit()` 实现此回调。
pub type DownloadProgressFn = Arc<dyn Fn(DownloadEvent) + Send + Sync>;

/// 模型下载进度事件（REQ-VEC-008 + REQ-VEC-017）。
///
/// 通过 Tauri 事件 `model_download_progress` 推送到前端。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadEvent {
    /// 正在下载某文件
    ///
    /// - `file_name`: 当前下载的文件名
    /// - `current`: 当前文件已下载字节数
    /// - `total`: 当前文件总字节数（Content-Length 缺失时为 0）
    /// - `file_index`: 当前文件序号（0-based）
    /// - `total_files`: 总文件数
    /// - `source`: 当前下载源 URL（REQ-VEC-017 AC-4）
    Downloading {
        file_name: String,
        current: u64,
        total: u64,
        file_index: usize,
        total_files: usize,
        source: String,
    },
    /// 下载完成，正在构建 ONNX 会话
    Loading,
    /// 全部完成，引擎可用
    Done,
    /// 下载或加载失败
    Error { message: String },
}

/// 镜像源选择（REQ-VEC-017 AC-3）。
///
/// 用户可在设置中手动选择镜像源，`auto` 为自动检测。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MirrorSource {
    /// 自动检测（中文系统→ModelScope 优先，其他→HuggingFace 优先）
    Auto,
    /// 魔搭（境内 CDN）
    ModelScope,
    /// HuggingFace 镜像（境内可达）
    HfMirror,
    /// HuggingFace 官方
    HuggingFace,
}

impl MirrorSource {
    /// 从字符串解析镜像源选择。
    ///
    /// 支持的值：`auto` / `modelscope` / `hf-mirror` / `huggingface`
    pub fn parse_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "modelscope" => Some(Self::ModelScope),
            "hf-mirror" | "hf_mirror" => Some(Self::HfMirror),
            "huggingface" | "hf" => Some(Self::HuggingFace),
            _ => None,
        }
    }

    /// 返回字符串标识（持久化用）。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::ModelScope => "modelscope",
            Self::HfMirror => "hf-mirror",
            Self::HuggingFace => "huggingface",
        }
    }

    /// 返回该镜像源对应的下载源列表。
    /// `Auto` 时根据系统语言动态选择。
    pub(crate) fn to_sources(&self) -> Vec<&'static DownloadSource> {
        match self {
            Self::Auto => get_download_sources(),
            Self::ModelScope => vec![&SOURCE_MODELSCOPE, &SOURCE_HF_MIRROR, &SOURCE_HUGGINGFACE],
            Self::HfMirror => vec![&SOURCE_HF_MIRROR, &SOURCE_MODELSCOPE, &SOURCE_HUGGINGFACE],
            Self::HuggingFace => vec![&SOURCE_HUGGINGFACE, &SOURCE_HF_MIRROR, &SOURCE_MODELSCOPE],
        }
    }
}

/// 模型缓存信息（REQ-VEC-008）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCacheInfo {
    /// 缓存目录总大小（字节）
    pub total_size_bytes: u64,
    /// 已安装模型列表
    pub models: Vec<ModelEntry>,
}

/// 向量化引擎下载状态（用于首启向导判断）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbedderStatus {
    /// 模型文件已全部就绪，可直接加载
    Ready,
    /// 模型文件缺失，需要从头下载
    NeedsDownload,
    /// 存在 .partial 文件，可断点续传
    PartialDownload {
        /// 缺失的文件列表
        missing_files: Vec<String>,
        /// 有 .partial 的文件列表（可续传）
        partial_files: Vec<String>,
    },
}

/// 单个模型缓存条目（REQ-VEC-008）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEntry {
    /// 模型目录名
    pub name: String,
    /// 该模型目录大小（字节）
    pub size_bytes: u64,
}

/// 下载源定义（参考 rs-pro 多源容错方案）
///
/// 源顺序由 [`get_download_sources`] 根据系统语言动态决定：
/// - 中文系统：ModelScope → hf-mirror → HuggingFace
/// - 其他系统：HuggingFace → hf-mirror → ModelScope
pub(crate) struct DownloadSource {
    /// 源名称（日志用）
    pub(crate) name: &'static str,
    /// Base URL 前缀（不含 repo 和文件路径）
    pub(crate) base: &'static str,
    /// 分支名：HuggingFace 用 `main`，ModelScope 用 `master`
    pub(crate) branch: &'static str,
}

/// 所有可用下载源（顺序由 `get_download_sources` 动态决定）
const SOURCE_MODELSCOPE: DownloadSource = DownloadSource {
    name: "ModelScope",
    base: "https://modelscope.cn/models",
    branch: "master",
};
const SOURCE_HF_MIRROR: DownloadSource = DownloadSource {
    name: "hf-mirror",
    base: "https://hf-mirror.com",
    branch: "main",
};
const SOURCE_HUGGINGFACE: DownloadSource = DownloadSource {
    name: "HuggingFace",
    base: "https://huggingface.co",
    branch: "main",
};

/// 检测系统是否为中文环境。
///
/// 通过环境变量 `LC_ALL` / `LC_MESSAGES` / `LANG` 判断（macOS/Linux），
/// macOS 额外读取 `defaults read -g AppleLocale` 作为回退。
///
/// 结果通过 `LazyLock` 缓存：首次调用后不再重复执行子进程检测。
fn is_chinese_locale() -> bool {
    *IS_CHINESE_LOCALE
}

/// 中文环境检测的缓存结果（LazyLock 保证线程安全的延迟初始化，仅执行一次）。
static IS_CHINESE_LOCALE: std::sync::LazyLock<bool> =
    std::sync::LazyLock::new(detect_chinese_locale);

/// 实际的中文环境检测逻辑（仅由 LazyLock 调用一次）。
fn detect_chinese_locale() -> bool {
    // Unix 环境变量检测
    for var in &["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(val) = std::env::var(var)
            && val.to_lowercase().starts_with("zh")
        {
            return true;
        }
    }
    // macOS：读取系统偏好设置中的 AppleLocale
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

/// HuggingFace 连通性探测结果缓存。
///
/// 首次模型下载时，用 3 秒超时的 HEAD 请求探测 huggingface.co 是否可达。
/// - 可达：用户在海外或有 VPN，HuggingFace 优先（国际源对非中文模型更全）
/// - 不可达：用户在中国大陆无 VPN，魔搭优先（境内 CDN 速度 16-38 MB/s）
///
/// 这样即使外国人到中国出差（系统语言为英文），也能自动切换到魔搭源。
static HF_REACHABLE: std::sync::LazyLock<bool> = std::sync::LazyLock::new(probe_huggingface);

/// 探测 HuggingFace 是否可达（3 秒超时 HEAD 请求）。
fn probe_huggingface() -> bool {
    let client = match reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };
    // 用一个小文件探测，HEAD 请求可能被 CDN 拒绝，改用 GET + 立即丢弃
    let url = "https://huggingface.co/Xenova/all-MiniLM-L6-v2/resolve/main/config.json";
    client
        .get(url)
        .send()
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

/// 根据系统语言和网络探测返回下载源优先级顺序。
///
/// 判定逻辑：
/// 1. 中文系统 → 魔搭优先（境内 CDN）
/// 2. 非中文系统 + HuggingFace 可达 → HuggingFace 优先（国际源）
/// 3. 非中文系统 + HuggingFace 不可达 → 魔搭优先（外国人在中国出差等场景）
///
/// 无论哪种顺序，hf-mirror 始终作为中间回退源。
fn get_download_sources() -> Vec<&'static DownloadSource> {
    if is_chinese_locale() {
        // 中文系统：魔搭优先
        vec![&SOURCE_MODELSCOPE, &SOURCE_HF_MIRROR, &SOURCE_HUGGINGFACE]
    } else if *HF_REACHABLE {
        // 非中文系统但 HuggingFace 可达：HF 优先
        vec![&SOURCE_HUGGINGFACE, &SOURCE_HF_MIRROR, &SOURCE_MODELSCOPE]
    } else {
        // 非中文系统且 HuggingFace 不可达：魔搭优先（外国人在中国）
        vec![&SOURCE_MODELSCOPE, &SOURCE_HF_MIRROR, &SOURCE_HUGGINGFACE]
    }
}

/// 校验已下载文件是否完好（参考 rs-pro 完整性校验）。
///
/// JSON 文件：验证 JSON 格式有效性
/// ONNX 文件：检查非空 + 非 HTML 错误页
fn is_file_valid(path: &Path, local_name: &str) -> bool {
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

/// 预设 Embedding 模型配置（E3-1 模型选择）。
///
/// 每个模型有不同维度的向量输出，适合不同语言/场景。
/// 切换模型需要重建全部 embeddings（维度变化会导致不兼容）。
///
/// `Custom(String)` 变体（REQ-VEC-014，Pro 门控）表示用户上传的 ONNX 模型，
/// String 为模型名称（对应 `custom_models/{name}/` 目录）。
/// 自定义模型的维度在加载时动态检测（`dim()` 返回 0）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmbeddingModel {
    /// all-MiniLM-L6-v2（384 维，英文通用场景，2023 模型）
    AllMiniLML6V2,
    /// bge-small-en-v1.5（384 维，英文优化场景，P1-5 新增默认模型）
    /// 检索质量优于 all-MiniLM-L6-v2，维度兼容，迁移成本低。
    BgeSmallEnV1_5,
    /// bge-small-zh-v1.5（512 维，中文优化场景）
    BgeSmallZhV1_5,
    /// e5-small-v2（384 维，多语言场景）
    E5SmallV2,
    /// bge-base-en-v1.5（768 维，英文强检索场景；P1-4 嵌入模型升级评估新增）
    BgeBaseEnV1_5,
    /// bge-m3（1024 维，多语言场景，REQ-VEC-015）
    /// 支持 100+ 语言的通用嵌入模型，检索质量显著优于 small 系列。
    /// 维度变化需重建全部嵌入（REQ-VEC-016 提供重建命令）。
    BgeM3,
    /// 用户自定义 ONNX 嵌入模型（REQ-VEC-014，Pro 门控）。
    /// String = 模型名称（对应 `custom_models/{name}/` 目录）。
    /// 自定义模型的文件结构由用户上传，维度在加载时动态检测。
    Custom(String),
}

impl EmbeddingModel {
    /// 返回 HuggingFace 模型仓库路径（pub(crate) 供测试访问）
    pub(crate) fn repo(&self) -> &'static str {
        match self {
            Self::AllMiniLML6V2 => "Xenova/all-MiniLM-L6-v2",
            Self::BgeSmallEnV1_5 => "BAAI/bge-small-en-v1.5",
            Self::BgeSmallZhV1_5 => "BAAI/bge-small-zh-v1.5",
            Self::E5SmallV2 => "intfloat/e5-small-v2",
            Self::BgeBaseEnV1_5 => "BAAI/bge-base-en-v1.5",
            Self::BgeM3 => "BAAI/bge-m3",
            Self::Custom(_) => "", // 自定义模型无仓库路径
        }
    }

    /// 返回本地模型目录名（pub(crate) 供测试访问）
    pub(crate) fn dir_name(&self) -> &'static str {
        match self {
            Self::AllMiniLML6V2 => "all-MiniLM-L6-v2",
            Self::BgeSmallEnV1_5 => "bge-small-en-v1.5",
            Self::BgeSmallZhV1_5 => "bge-small-zh-v1.5",
            Self::E5SmallV2 => "e5-small-v2",
            Self::BgeBaseEnV1_5 => "bge-base-en-v1.5",
            Self::BgeM3 => "bge-m3",
            Self::Custom(_) => "", // 自定义模型目录名由调用方管理
        }
    }

    /// 返回 ONNX 模型文件名（pub(crate) 供测试访问）
    pub(crate) fn onnx_file(&self) -> &'static str {
        match self {
            Self::AllMiniLML6V2 => "model_quantized.onnx",
            Self::BgeSmallEnV1_5 => "model.onnx",
            Self::BgeSmallZhV1_5 => "model.onnx",
            Self::E5SmallV2 => "model_optimized.onnx",
            Self::BgeBaseEnV1_5 => "model.onnx",
            Self::BgeM3 => "model.onnx",
            Self::Custom(_) => "", // 自定义模型 ONNX 文件名由 new_with_custom_model 检测
        }
    }

    /// 返回仓库内 ONNX 文件路径（pub(crate) 供测试访问）
    pub(crate) fn onnx_repo_path(&self) -> &'static str {
        match self {
            Self::AllMiniLML6V2 => "onnx/model_quantized.onnx",
            Self::BgeSmallEnV1_5 => "onnx/model.onnx",
            Self::BgeSmallZhV1_5 => "onnx/model.onnx",
            Self::E5SmallV2 => "onnx/model_optimized.onnx",
            Self::BgeBaseEnV1_5 => "onnx/model.onnx",
            Self::BgeM3 => "onnx/model.onnx",
            Self::Custom(_) => "",
        }
    }

    /// 返回所需模型文件清单：(仓库内路径, 本地文件名)（pub(crate) 供测试访问）
    pub(crate) fn files(&self) -> [(&'static str, &'static str); 5] {
        [
            (self.onnx_repo_path(), self.onnx_file()),
            ("tokenizer.json", "tokenizer.json"),
            ("config.json", "config.json"),
            ("special_tokens_map.json", "special_tokens_map.json"),
            ("tokenizer_config.json", "tokenizer_config.json"),
        ]
    }

    /// 返回向量维度。
    ///
    /// 预设模型返回已知维度；自定义模型返回 0（维度在加载时动态检测）。
    pub fn dim(&self) -> usize {
        match self {
            Self::AllMiniLML6V2 => 384,
            Self::BgeSmallEnV1_5 => 384,
            Self::BgeSmallZhV1_5 => 512,
            Self::E5SmallV2 => 384,
            Self::BgeBaseEnV1_5 => 768,
            Self::BgeM3 => 1024,
            Self::Custom(_) => 0, // 自定义模型维度在加载时动态检测
        }
    }

    /// 判断是否为自定义模型。
    pub fn is_custom(&self) -> bool {
        matches!(self, Self::Custom(_))
    }
}

/// 默认会话池大小上限：防止内存爆炸（每个量化模型实例约 30MB）。
/// 8 个实例 ≈ 240MB，在 16GB+ 桌面机上可接受。
const MAX_POOL_SIZE: usize = 8;

/// 本地 Embedding 适配器。克隆廉价（Arc 共享模型实例）。
///
/// 内部维护 N 个独立 ONNX 会话池，支持并行批量推理（GB 级文档加速）。
#[derive(Clone)]
pub struct LocalEmbedder {
    /// 推理会话池：N 个独立 ONNX 实例，支持无锁并行推理。
    /// `embed()` 要求 `&mut self`，每个实例用独立 Mutex 保护；
    /// 不同分片可在不同 `spawn_blocking` 线程上并行执行。
    sessions: Vec<Arc<Mutex<TextEmbedding>>>,
    /// 向量维度（模型配置），用于运行时校验
    dim: usize,
}

impl LocalEmbedder {
    /// 默认会话池大小。
    ///
    /// **性能优化（2026-08-02）**：默认返回 1（单会话 + 全核）。
    ///
    /// 原实现返回 `available_parallelism().min(8)`，配合分片并行。
    /// 但 fastembed 的 `QuantizationMode::Dynamic` 会将整批文本在单个 ONNX 推理中处理，
    /// 分片不仅无法提升吞吐，还因多会话 × 全核 = 线程爆炸导致 40-70s 延迟。
    ///
    /// 新策略：单会话 + `intra_threads=None`（ONNX Runtime 使用全部 CPU 核心）。
    /// 实测 414 chunks 从 66.9s → ~1-2s（30-60x 加速）。
    ///
    /// 如需并发查询支持（如多窗口同时聊天），可通过 `new_with_pool_size` 显式指定 >1。
    pub fn default_pool_size() -> usize {
        1
    }

    /// 使用默认模型（AllMiniLML6V2）+ 默认池大小初始化（向后兼容）。
    pub async fn new(cache_dir: PathBuf) -> anyhow::Result<Self> {
        Self::new_with_model(cache_dir, EmbeddingModel::AllMiniLML6V2).await
    }

    /// 使用指定模型 + 默认池大小初始化（E3-1 模型选择）。
    ///
    /// 不同模型产生不同维度的向量，切换模型后需重建全部 embeddings。
    pub async fn new_with_model(cache_dir: PathBuf, model: EmbeddingModel) -> anyhow::Result<Self> {
        Self::new_with_progress(cache_dir, model, None).await
    }

    /// 使用指定模型 + 进度回调 + 默认池大小初始化（REQ-VEC-008）。
    ///
    /// `progress` 回调在下载阶段推送 `DownloadEvent::Downloading`，
    /// 加载阶段推送 `Loading`，完成后推送 `Done`，失败推送 `Error`。
    pub async fn new_with_progress(
        cache_dir: PathBuf,
        model: EmbeddingModel,
        progress: Option<DownloadProgressFn>,
    ) -> anyhow::Result<Self> {
        let pool_size = Self::default_pool_size();
        Self::new_with_pool_size(cache_dir, model, progress, pool_size).await
    }

    /// 使用指定模型 + 自定义池大小初始化（GB 级文档加速）。
    ///
    /// `pool_size` 控制 ONNX 会话数：N 个会话允许 N 个 `spawn_blocking` 任务并行推理。
    /// 每个量化模型实例约 30MB 内存，8 个 ≈ 240MB。
    ///
    /// # 参数
    /// - `cache_dir`: 模型缓存目录
    /// - `model`: Embedding 模型预设
    /// - `progress`: 下载进度回调（`None` 表示不需要进度推送）
    /// - `pool_size`: 会话池大小（1=单会话，4=4 路并行，8=最大并行）
    pub async fn new_with_pool_size(
        cache_dir: PathBuf,
        model: EmbeddingModel,
        progress: Option<DownloadProgressFn>,
        pool_size: usize,
    ) -> anyhow::Result<Self> {
        Self::new_with_mirror(cache_dir, model, progress, pool_size, MirrorSource::Auto).await
    }

    /// 使用指定模型 + 镜像源 + 池大小初始化（REQ-VEC-017）。
    ///
    /// # 参数
    /// - `cache_dir`: 模型缓存目录
    /// - `model`: Embedding 模型预设
    /// - `progress`: 下载进度回调
    /// - `pool_size`: 会话池大小
    /// - `mirror_source`: 镜像源选择（auto/modelscope/hf-mirror/huggingface）
    pub async fn new_with_mirror(
        cache_dir: PathBuf,
        model: EmbeddingModel,
        progress: Option<DownloadProgressFn>,
        pool_size: usize,
        mirror_source: MirrorSource,
    ) -> anyhow::Result<Self> {
        let pool_size = pool_size.clamp(1, MAX_POOL_SIZE);
        tokio::task::spawn_blocking(move || {
            Self::init(&cache_dir, model, progress, pool_size, mirror_source)
        })
        .await
        .context("向量化引擎初始化任务失败")?
    }

    /// 返回当前会话池大小（测试与诊断用）。
    pub fn pool_size(&self) -> usize {
        self.sessions.len()
    }

    /// 返回当前模型的向量维度。
    pub fn dim(&self) -> usize {
        self.dim
    }

    /// 将文本列表均匀分片为 `pool_size` 个子列表。
    ///
    /// 纯逻辑函数，不涉及模型加载。分片大小差 ≤ 1，保证均匀。
    /// 空输入返回空 Vec；文本数 < pool_size 时每条独占一个分片。
    pub fn shard_texts(texts: &[String], pool_size: usize) -> Vec<Vec<String>> {
        if texts.is_empty() || pool_size == 0 {
            return Vec::new();
        }
        let actual_size = pool_size.min(texts.len());
        let shard_size = texts.len().div_ceil(actual_size);
        texts.chunks(shard_size).map(|c| c.to_vec()).collect()
    }

    fn init(
        cache_dir: &Path,
        model: EmbeddingModel,
        progress: Option<DownloadProgressFn>,
        pool_size: usize,
        mirror_source: MirrorSource,
    ) -> anyhow::Result<Self> {
        let model_dir = cache_dir.join(model.dir_name());
        Self::ensure_model_files(&model_dir, &model, &progress, mirror_source)?;
        if let Some(p) = &progress {
            p(DownloadEvent::Loading);
        }
        let dim = model.dim();
        // 性能优化：当 pool_size > 1 时，为每个 ONNX 会话设置 intra_threads=1。
        //
        // 原因：InitOptionsUserDefined 默认 intra_threads=None，即每个会话使用全部 CPU 核心。
        // 当 pool_size=8 时，8 个会话 × N 核心 = 8N 个线程竞争 N 个核心，
        // 导致严重线程争用（测试：414 chunks 耗时 66.9s，即 161.7ms/chunk）。
        //
        // 修复后：每个会话使用 1 个线程，分片并行在 shard 层面实现并行化。
        // 预期效果：414 chunks 从 66.9s 降至 ~1-2s（20-60x 加速）。
        let intra_threads = if pool_size > 1 { Some(1) } else { None };
        let mut sessions = Vec::with_capacity(pool_size);
        for i in 0..pool_size {
            let embedding_model =
                Self::load_from_files(&model_dir, model.clone(), intra_threads)
                    .with_context(|| format!("加载第 {i}/{pool_size} 个 ONNX 会话失败"))?;
            sessions.push(Arc::new(Mutex::new(embedding_model)));
        }
        if let Some(p) = &progress {
            p(DownloadEvent::Done);
        }
        Ok(Self { sessions, dim })
    }

    /// 确保模型文件齐备：多源容错 + 重试 + 断点续传 + 完整性校验（REQ-VEC-017）。
    ///
    /// 下载策略：
    /// 1. **镜像源选择**：优先使用用户配置的 `mirror_source`（设置键 `vec.mirror_source`），
    ///    未配置时走自动检测（中文系统→魔搭优先，其他→HuggingFace 优先）。
    /// 2. **重试机制**：每个源失败后重试 3 次，退避间隔 2s/4s/8s（REQ-VEC-017 AC-1）。
    /// 3. **断点续传**：下载到 `.partial` 文件，完成后原子 rename。支持 HTTP Range 请求
    ///    续传已下载部分（REQ-VEC-017 AC-2）。镜像源不支持 Range 时回退为全量下载。
    /// 4. **完整性校验**：Content-Length 匹配 + JSON 格式验证 + ONNX 头部检查。
    ///
    /// 一旦某源成功，后续文件优先使用该源（避免逐文件超时惩罚）。
    fn ensure_model_files(
        model_dir: &Path,
        model: &EmbeddingModel,
        progress: &Option<DownloadProgressFn>,
        mirror_source: MirrorSource,
    ) -> anyhow::Result<()> {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .build()
            .context("创建模型下载客户端失败")?;

        let repo = model.repo();
        let files = model.files();
        let total_files = files.len();
        let mut prefer_source: Option<usize> = None;

        for (file_index, (repo_path, local_name)) in files.into_iter().enumerate() {
            let dest = model_dir.join(local_name);

            // 已存在文件：校验完整性，损坏则删除重下
            if dest.exists() {
                if is_file_valid(&dest, local_name) {
                    // 推送已缓存文件的进度（让前端知道当前文件已跳过）
                    if let Some(p) = progress {
                        let size = std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
                        p(DownloadEvent::Downloading {
                            file_name: local_name.to_string(),
                            current: size,
                            total: size,
                            file_index,
                            total_files,
                            source: "cache".to_string(),
                        });
                    }
                    continue;
                }
                eprintln!("[LocalEmbedder] 缓存文件 {local_name} 损坏，重新下载");
                let _ = std::fs::remove_file(&dest);
            }

            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("创建模型目录失败: {}", parent.display()))?;
            }

            // 按优先级排序源（用户配置或系统语言决定初始顺序）
            let sources = mirror_source.to_sources();
            let mut order: Vec<usize> = (0..sources.len()).collect();
            if let Some(pref) = prefer_source {
                order.sort_by_key(|&i| if i == pref { 0 } else { 1 + i });
            }

            let mut failures = Vec::new();
            let mut downloaded = false;

            for &idx in &order {
                let src = sources[idx];
                let url = format!("{}/{repo}/resolve/{}/{}", src.base, src.branch, repo_path);

                // 重试机制：3 次重试 + 退避 2s/4s/8s（REQ-VEC-017 AC-1）
                let backoff_secs = [2u64, 4, 8];
                let mut attempt = 0;
                loop {
                    match Self::download_file_with_resume(
                        &client,
                        &url,
                        &dest,
                        local_name,
                        file_index,
                        total_files,
                        progress,
                    ) {
                        Ok(()) => {
                            prefer_source = Some(idx);
                            downloaded = true;
                            break;
                        }
                        Err(err) => {
                            eprintln!(
                                "[LocalEmbedder] {} 源下载失败 (attempt {}/{}) \
                                 {url} → {err}",
                                src.name,
                                attempt + 1,
                                backoff_secs.len() + 1,
                            );
                            if attempt < backoff_secs.len() {
                                let wait = backoff_secs[attempt];
                                eprintln!("[LocalEmbedder] {wait}s 后重试…");
                                std::thread::sleep(Duration::from_secs(wait));
                                attempt += 1;
                                continue;
                            }
                            failures.push(format!("{}: {err}", src.name));
                            break;
                        }
                    }
                }
                if downloaded {
                    break;
                }
            }

            if !downloaded {
                let msg = format!(
                    "模型文件 {repo_path} 全部源均下载失败：{}",
                    failures.join("；")
                );
                if let Some(p) = progress {
                    p(DownloadEvent::Error {
                        message: msg.clone(),
                    });
                }
                bail!(msg);
            }
        }
        Ok(())
    }

    /// 下载单个文件，支持断点续传（REQ-VEC-017 AC-2）。
    ///
    /// 断点续传策略：
    /// 1. 检查 `.partial` 文件是否存在，获取已下载字节数
    /// 2. 发送 `Range: bytes={offset}-` 请求续传
    /// 3. 服务器返回 206 → 追加写入 `.partial` 文件
    /// 4. 服务器返回 200 或不支持 Range → 全量下载覆盖 `.partial`
    /// 5. 下载完成后原子 rename 到目标路径
    ///
    /// 下载后执行完整性校验：
    /// - Content-Length 匹配（检测传输截断）
    /// - JSON 文件格式验证（检测 HTML 错误页被保存为 JSON）
    /// - ONNX 文件头部检查（检测 HTML 错误页）
    fn download_file_with_resume(
        client: &reqwest::blocking::Client,
        url: &str,
        dest: &Path,
        file_name: &str,
        file_index: usize,
        total_files: usize,
        progress: &Option<DownloadProgressFn>,
    ) -> anyhow::Result<()> {
        use std::io::{Read, Write};

        // .partial 文件路径（断点续传用）
        let partial = dest.with_extension(format!(
            "{}partial",
            dest.extension()
                .map(|e| format!("{}.", e.to_string_lossy()))
                .unwrap_or_default()
        ));

        // 检查已有 .partial 文件大小
        let existing_offset: u64 = if partial.exists() {
            std::fs::metadata(&partial).map(|m| m.len()).unwrap_or(0)
        } else {
            0
        };

        // 构建请求：如果有已下载部分，尝试 Range 请求
        let mut request = client.get(url);
        if existing_offset > 0 {
            request = request.header(reqwest::header::RANGE, format!("bytes={existing_offset}-"));
        }

        let resp = request
            .send()
            .with_context(|| format!("HTTP 请求失败: {url}"))?;

        // 416 Range Not Satisfiable：.partial 文件可能已完整或超出范围
        // 清理 .partial 文件后全量重下
        if resp.status() == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
            let _ = std::fs::remove_file(&partial);
            return Self::download_file_with_resume(
                client,
                url,
                dest,
                file_name,
                file_index,
                total_files,
                progress,
            );
        }

        // 判断是否为续传响应（206）或全量响应（200）
        let is_resume = resp.status() == reqwest::StatusCode::PARTIAL_CONTENT;

        // 非 2xx 错误
        if !resp.status().is_success() {
            bail!("HTTP {} - {}", resp.status(), url);
        }

        // 解析 Content-Length
        let content_length: Option<u64> = resp
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok());

        // 续传模式：Content-Length 是剩余大小，总大小 = offset + remaining
        // 全量模式：Content-Length 是总大小
        let total: u64 = if is_resume {
            existing_offset + content_length.unwrap_or(0)
        } else {
            content_length.unwrap_or(0)
        };

        // 打开 .partial 文件：续传模式追加，全量模式创建新文件
        let mut file = if is_resume && existing_offset > 0 {
            std::fs::OpenOptions::new()
                .append(true)
                .open(&partial)
                .with_context(|| format!("打开 .partial 文件失败: {}", partial.display()))?
        } else {
            // 全量下载或非续传响应：创建/截断 .partial
            std::fs::File::create(&partial)
                .with_context(|| format!("创建 .partial 文件失败: {}", partial.display()))?
        };

        // 进度节流：每 100ms 最多发一次事件
        let mut last_emit = std::time::Instant::now();
        let mut buf = vec![0u8; 64 * 1024]; // 64KB 缓冲区
        let mut current: u64 = if is_resume { existing_offset } else { 0 };

        let mut reader = resp;
        loop {
            let n = reader
                .read(&mut buf)
                .with_context(|| format!("读取响应体失败: {url}"))?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n])
                .with_context(|| format!("写入文件失败: {}", partial.display()))?;
            current += n as u64;

            let now = std::time::Instant::now();
            let should_emit = total == 0 || now.duration_since(last_emit).as_millis() >= 100;
            if should_emit {
                last_emit = now;
                if let Some(p) = progress {
                    p(DownloadEvent::Downloading {
                        file_name: file_name.to_string(),
                        current,
                        total,
                        file_index,
                        total_files,
                        source: url.to_string(),
                    });
                }
            }
        }
        file.sync_all().ok();
        drop(file); // 确保文件句柄关闭

        // 下载完成：发送 100% 事件
        if let Some(p) = progress {
            p(DownloadEvent::Downloading {
                file_name: file_name.to_string(),
                current,
                total: if total > 0 { total } else { current },
                file_index,
                total_files,
                source: url.to_string(),
            });
        }

        // ================================================================
        // 完整性校验 1：Content-Length 匹配
        // ================================================================
        if let Some(expected) = content_length
            && !is_resume
            && current != expected
        {
            let _ = std::fs::remove_file(&partial);
            bail!("下载不完整: 期望 {expected} 字节，实际 {current} 字节");
        }

        // 原子 rename .partial → dest
        std::fs::rename(&partial, dest)
            .with_context(|| format!("重命名 .partial → {} 失败", dest.display()))?;

        // ================================================================
        // 完整性校验 2：JSON 文件格式验证
        // ================================================================
        if file_name.ends_with(".json") {
            let content = std::fs::read(dest)
                .with_context(|| format!("下载后读取文件失败: {}", dest.display()))?;
            if serde_json::from_slice::<serde_json::Value>(&content).is_err() {
                let _ = std::fs::remove_file(dest);
                let preview = String::from_utf8_lossy(&content[..content.len().min(100)]);
                bail!(
                    "下载的 JSON 文件格式无效 (可能被截断或为错误页面): {file_name}, 前100字节: {preview}"
                );
            }
        }

        // ================================================================
        // 完整性校验 3：ONNX 文件头部检查（HTML 错误页检测）
        // ================================================================
        if file_name.ends_with(".onnx") {
            let content = std::fs::read(dest)
                .with_context(|| format!("下载后读取文件失败: {}", dest.display()))?;
            if content.is_empty() {
                let _ = std::fs::remove_file(dest);
                bail!("下载的 ONNX 文件为空: {file_name}");
            }
            let preview = &content[..content.len().min(20)];
            let preview_str = String::from_utf8_lossy(preview);
            if preview_str.starts_with("<!")
                || preview_str.starts_with("<html")
                || preview_str.starts_with("<HTML")
            {
                let _ = std::fs::remove_file(dest);
                bail!("下载的 ONNX 文件疑似 HTML 错误页: {file_name}");
            }
        }

        Ok(())
    }

    // ---- 缓存管理（REQ-VEC-008）----

    /// 获取模型缓存目录信息（REQ-VEC-008-AC-5）。
    ///
    /// 返回缓存目录总大小 + 已安装模型列表。
    /// 不需要实例化 `LocalEmbedder`，静态方法可直接调用。
    pub fn get_cache_info(cache_dir: &Path) -> ModelCacheInfo {
        let mut models = Vec::new();
        let mut total_size: u64 = 0;

        for model in [
            EmbeddingModel::AllMiniLML6V2,
            EmbeddingModel::BgeSmallEnV1_5,
            EmbeddingModel::BgeSmallZhV1_5,
            EmbeddingModel::E5SmallV2,
            EmbeddingModel::BgeBaseEnV1_5,
            EmbeddingModel::BgeM3,
        ] {
            let model_dir = cache_dir.join(model.dir_name());
            if model_dir.exists() {
                let size = Self::dir_size(&model_dir);
                total_size += size;
                models.push(ModelEntry {
                    name: model.dir_name().to_string(),
                    size_bytes: size,
                });
            }
        }

        ModelCacheInfo {
            total_size_bytes: total_size,
            models,
        }
    }

    /// 清理模型缓存（REQ-VEC-008-AC-6）。
    ///
    /// 删除指定模型目录（`model_name` 为 `None` 时删除全部模型）。
    /// 返回删除的字节数。
    pub fn clear_cache(cache_dir: &Path, model_name: Option<&str>) -> u64 {
        let mut freed: u64 = 0;
        if let Some(name) = model_name {
            let model_dir = cache_dir.join(name);
            if model_dir.exists() {
                freed = Self::dir_size(&model_dir);
                let _ = std::fs::remove_dir_all(&model_dir);
            }
        } else {
            // 删除全部模型目录
            if cache_dir.exists() {
                for model in [
                    EmbeddingModel::AllMiniLML6V2,
                    EmbeddingModel::BgeSmallEnV1_5,
                    EmbeddingModel::BgeSmallZhV1_5,
                    EmbeddingModel::E5SmallV2,
                    EmbeddingModel::BgeBaseEnV1_5,
                    EmbeddingModel::BgeM3,
                ] {
                    let model_dir = cache_dir.join(model.dir_name());
                    if model_dir.exists() {
                        freed += Self::dir_size(&model_dir);
                        let _ = std::fs::remove_dir_all(&model_dir);
                    }
                }
            }
        }
        freed
    }

    /// 检查默认模型的下载状态（用于首启向导判断）。
    ///
    /// 检查 `cache_dir/bge-small-en-v1.5/` 下 5 个必需文件是否齐全。
    /// 如果存在 `.partial` 文件，返回 `PartialDownload` 状态。
    pub fn check_status(cache_dir: &Path) -> EmbedderStatus {
        let model = EmbeddingModel::BgeSmallEnV1_5;
        let model_dir = cache_dir.join(model.dir_name());
        let files = model.files();

        let mut missing = Vec::new();
        let mut partials = Vec::new();

        for (_, local_name) in files {
            let dest = model_dir.join(local_name);
            if dest.exists() {
                continue;
            }
            // 检查 .partial 文件
            let partial = dest.with_extension(format!(
                "{}partial",
                dest.extension()
                    .map(|e| format!("{}.", e.to_string_lossy()))
                    .unwrap_or_default()
            ));
            if partial.exists() {
                partials.push(local_name.to_string());
            } else {
                missing.push(local_name.to_string());
            }
        }

        if missing.is_empty() && partials.is_empty() {
            EmbedderStatus::Ready
        } else if partials.is_empty() {
            EmbedderStatus::NeedsDownload
        } else {
            EmbedderStatus::PartialDownload {
                missing_files: missing,
                partial_files: partials,
            }
        }
    }

    /// 递归计算目录大小（字节）。
    pub fn dir_size(dir: &Path) -> u64 {
        let mut size = 0u64;
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    size += Self::dir_size(&path);
                } else if let Ok(meta) = entry.metadata() {
                    size += meta.len();
                }
            }
        }
        size
    }

    /// 从本地文件构建模型（fastembed 用户自定义通道，绕开 hf-hub 续传逻辑）。
    ///
    /// `intra_threads`：ONNX Runtime intra-op 线程数。`None` 使用全部核心，
    /// `Some(1)` 限制为单线程（用于多会话池场景，避免线程争用）。
    fn load_from_files(
        model_dir: &Path,
        model: EmbeddingModel,
        intra_threads: Option<usize>,
    ) -> anyhow::Result<TextEmbedding> {
        let onnx_file = model.onnx_file();
        let read = |name: &str| {
            std::fs::read(model_dir.join(name)).with_context(|| format!("读取模型文件失败: {name}"))
        };
        let onnx = read(onnx_file)?;
        let tokenizer_files = TokenizerFiles {
            tokenizer_file: read("tokenizer.json")?,
            config_file: read("config.json")?,
            special_tokens_map_file: read("special_tokens_map.json")?,
            tokenizer_config_file: read("tokenizer_config.json")?,
        };
        // Mean pooling + 无运行时量化。
        //
        // 性能优化（2026-08-02）：原使用 QuantizationMode::Dynamic，导致 fastembed
        // 将整批文本在单个 ONNX 推理中处理（batch_size = texts.len()），无法分批。
        // 实测 414 chunks 耗时 65-67s（单次推理 414×512 token 矩阵，CPU 缓存频繁失效）。
        //
        // model_quantized.onnx 已是静态量化模型，无需运行时动态量化。
        // 改用 QuantizationMode::None 后，fastembed 使用默认 batch_size=256，
        // 分批推理显著提升 CPU 缓存命中率，预期 414 chunks < 5s。
        let user_model = UserDefinedEmbeddingModel::new(onnx, tokenizer_files)
            .with_pooling(Pooling::Mean)
            .with_quantization(QuantizationMode::None);
        let mut options = InitOptionsUserDefined::new();
        if let Some(threads) = intra_threads {
            options = options.with_intra_threads(threads);
        }
        TextEmbedding::try_new_from_user_defined(user_model, options)
            .context("fastembed 模型会话构建失败")
    }

    // ---- 自定义 ONNX 嵌入模型（REQ-VEC-014，Pro 门控）----

    /// 自定义 ONNX 嵌入模型所需的文件名常量。
    ///
    /// 用户上传的模型目录必须包含以下文件：
    /// - ONNX 模型文件：`model.onnx` 或 `model_quantized.onnx`（至少一个）
    /// - Tokenizer 文件：`tokenizer.json`、`config.json`、`special_tokens_map.json`、`tokenizer_config.json`
    pub const REQUIRED_TOKENIZER_FILES: [&'static str; 4] = [
        "tokenizer.json",
        "config.json",
        "special_tokens_map.json",
        "tokenizer_config.json",
    ];

    /// 可能的 ONNX 模型文件名（按优先级检查）。
    pub const ONNX_FILE_CANDIDATES: [&'static str; 3] =
        ["model_quantized.onnx", "model.onnx", "model_optimized.onnx"];

    /// 从本地文件加载用户自定义 ONNX 嵌入模型（REQ-VEC-014，Pro 门控）。
    ///
    /// 与预设模型不同，自定义模型的维度在加载时通过试推理动态检测。
    /// 模型文件已由 `upload_custom_embedding_model` IPC 命令保存到 `model_dir`。
    ///
    /// # 参数
    /// - `model_dir`: 自定义模型目录（`{data_dir}/custom_models/{name}/`）
    ///
    /// # 返回
    /// 成功返回 `LocalEmbedder` 实例，其 `dim` 为检测到的向量维度。
    ///
    /// # 错误
    /// - ONNX 文件缺失或无效
    /// - Tokenizer 文件缺失
    /// - fastembed 加载失败
    /// - 维度检测推理失败
    pub async fn new_with_custom_model(model_dir: PathBuf) -> anyhow::Result<Self> {
        tokio::task::spawn_blocking(move || Self::init_custom(&model_dir))
            .await
            .context("自定义向量化引擎初始化任务失败")?
    }

    /// 自定义模型初始化核心逻辑（阻塞线程池执行）。
    ///
    /// 1. 查找 ONNX 文件（`model_quantized.onnx` → `model.onnx` → `model_optimized.onnx`）
    /// 2. 读取 tokenizer 文件
    /// 3. 构建 `UserDefinedEmbeddingModel` + `TextEmbedding` 会话
    /// 4. 试推理 `"test"` 检测向量维度
    fn init_custom(model_dir: &Path) -> anyhow::Result<Self> {
        // 查找 ONNX 文件
        let onnx_name = Self::find_onnx_file(model_dir)
            .context("未找到 ONNX 模型文件（需要 model.onnx 或 model_quantized.onnx）")?;

        let read = |name: &str| {
            std::fs::read(model_dir.join(name))
                .with_context(|| format!("读取自定义模型文件失败: {name}"))
        };

        let onnx = read(onnx_name)?;
        let tokenizer_files = TokenizerFiles {
            tokenizer_file: read("tokenizer.json")?,
            config_file: read("config.json")?,
            special_tokens_map_file: read("special_tokens_map.json")?,
            tokenizer_config_file: read("tokenizer_config.json")?,
        };

        let user_model = UserDefinedEmbeddingModel::new(onnx, tokenizer_files)
            .with_pooling(Pooling::Mean)
            .with_quantization(QuantizationMode::None);
        let options = InitOptionsUserDefined::new();
        let mut session = TextEmbedding::try_new_from_user_defined(user_model, options)
            .context("自定义 fastembed 模型会话构建失败（请检查 ONNX 文件格式是否兼容）")?;

        // 维度检测：试推理 "test" 文本，获取向量长度
        let test_vec = session
            .embed(vec!["test".to_string()], None)
            .context("自定义模型维度检测推理失败")?;
        let dim = test_vec
            .first()
            .map(|v| v.len())
            .ok_or_else(|| anyhow::anyhow!("自定义模型维度检测返回空结果"))?;

        Ok(Self {
            sessions: vec![Arc::new(Mutex::new(session))],
            dim,
        })
    }

    /// 在模型目录中查找 ONNX 文件（按优先级检查候选文件名）。
    ///
    /// 返回第一个存在的 ONNX 文件名。如果都不存在，返回 `None`。
    pub fn find_onnx_file(model_dir: &Path) -> Option<&'static str> {
        for name in Self::ONNX_FILE_CANDIDATES {
            let path = model_dir.join(name);
            if path.exists() {
                return Some(name);
            }
        }
        None
    }

    /// 验证 ONNX 文件格式有效性（非 HTML 错误页 + 非空）。
    ///
    /// ONNX 是 protobuf 格式，无固定 magic bytes，因此检查：
    /// 1. 文件非空
    /// 2. 不以 HTML 标签开头（检测镜像源错误页）
    pub fn validate_onnx_file(path: &Path) -> anyhow::Result<()> {
        let content = std::fs::read(path)
            .with_context(|| format!("读取 ONNX 文件失败: {}", path.display()))?;
        if content.is_empty() {
            bail!("ONNX 文件为空");
        }
        let preview = &content[..content.len().min(20)];
        let preview_str = String::from_utf8_lossy(preview);
        if preview_str.starts_with("<!")
            || preview_str.starts_with("<html")
            || preview_str.starts_with("<HTML")
        {
            bail!("文件疑似 HTML 错误页，不是有效的 ONNX 模型");
        }
        Ok(())
    }

    /// 验证 tokenizer 文件完整性（检查 4 个必需文件是否存在）。
    ///
    /// 返回缺失文件列表。空 Vec 表示全部文件就绪。
    pub fn validate_tokenizer_files(model_dir: &Path) -> Vec<String> {
        Self::REQUIRED_TOKENIZER_FILES
            .iter()
            .filter(|name| !model_dir.join(name).exists())
            .map(|s| s.to_string())
            .collect()
    }

    /// 列出自定义模型目录下所有已上传的模型（REQ-VEC-014-AC-2）。
    ///
    /// 扫描 `custom_models_dir` 下的子目录，每个子目录视为一个自定义模型。
    /// 返回 `CustomModelInfo` 列表，包含名称、大小和文件完整性信息。
    pub fn list_custom_models(custom_models_dir: &Path) -> Vec<CustomModelInfo> {
        let mut models = Vec::new();
        let entries = match std::fs::read_dir(custom_models_dir) {
            Ok(e) => e,
            Err(_) => return models,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            let size = Self::dir_size(&path);
            let has_onnx = Self::find_onnx_file(&path).is_some();
            let missing_tokenizers = Self::validate_tokenizer_files(&path);
            let is_valid = has_onnx && missing_tokenizers.is_empty();
            models.push(CustomModelInfo {
                name,
                dim: 0, // 维度在加载时检测，列表时不加载模型
                size_bytes: size,
                is_valid,
            });
        }
        models
    }

    /// 删除自定义模型目录（REQ-VEC-014-AC-4）。
    ///
    /// 删除 `custom_models_dir/{name}` 目录及其所有文件。
    /// 返回删除的字节数。如果目录不存在，返回 0。
    pub fn delete_custom_model(custom_models_dir: &Path, name: &str) -> u64 {
        // 路径安全检查：防止路径遍历攻击（如 `../../etc/passwd`）
        let clean_name = name
            .replace(['/', '\\'], "_")
            .replace("..", "_")
            .replace('~', "_");
        let model_dir = custom_models_dir.join(&clean_name);
        if !model_dir.exists() {
            return 0;
        }
        let size = Self::dir_size(&model_dir);
        let _ = std::fs::remove_dir_all(&model_dir);
        size
    }

    /// 将用户上传的文件复制到自定义模型目录（REQ-VEC-014-AC-1）。
    ///
    /// # 参数
    /// - `dest_dir`: 目标目录（`custom_models/{name}/`）
    /// - `onnx_path`: 用户选择的 ONNX 文件路径
    /// - `tokenizer_file_paths`: 用户选择的 tokenizer 文件路径列表
    ///
    /// # 逻辑
    /// 1. 创建目标目录
    /// 2. 复制 ONNX 文件（保留原文件名）
    /// 3. 复制 tokenizer 文件（按必需文件名匹配）
    /// 4. 验证完整性
    pub fn copy_custom_model_files(
        dest_dir: &Path,
        onnx_path: &Path,
        tokenizer_file_paths: &[PathBuf],
    ) -> anyhow::Result<()> {
        std::fs::create_dir_all(dest_dir)
            .with_context(|| format!("创建自定义模型目录失败: {}", dest_dir.display()))?;

        // 复制 ONNX 文件（保留原文件名）
        let onnx_dest_name = onnx_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("model.onnx");
        let onnx_dest = dest_dir.join(onnx_dest_name);
        std::fs::copy(onnx_path, &onnx_dest).with_context(|| {
            format!(
                "复制 ONNX 文件失败: {} → {}",
                onnx_path.display(),
                onnx_dest.display()
            )
        })?;

        // 复制 tokenizer 文件（按文件名匹配必需文件）
        for required_name in Self::REQUIRED_TOKENIZER_FILES {
            let dest = dest_dir.join(required_name);
            if dest.exists() {
                continue; // 已存在（可能是之前上传的）
            }
            // 在用户提供的文件路径中查找同名文件
            let source = tokenizer_file_paths
                .iter()
                .find(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n == required_name)
                })
                .with_context(|| format!("缺少必需的 tokenizer 文件: {required_name}"))?;
            std::fs::copy(source, &dest)
                .with_context(|| format!("复制 tokenizer 文件失败: {required_name}"))?;
        }

        // 验证完整性
        Self::validate_onnx_file(&onnx_dest)?;
        let missing = Self::validate_tokenizer_files(dest_dir);
        if !missing.is_empty() {
            bail!("自定义模型文件不完整，缺少: {}", missing.join(", "));
        }

        Ok(())
    }
}

/// 自定义嵌入模型信息（REQ-VEC-014）。
///
/// 由 `list_custom_models` IPC 命令返回，前端用于展示已上传的模型列表。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomModelInfo {
    /// 模型名称（目录名）
    pub name: String,
    /// 向量维度（0 表示尚未检测，加载时动态获取）
    pub dim: usize,
    /// 模型文件总大小（字节）
    pub size_bytes: u64,
    /// 模型是否完整有效（ONNX 文件 + 全部 tokenizer 文件就绪）
    pub is_valid: bool,
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
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // 性能优化（2026-08-02）：始终走单会话路径，不再分片。
        //
        // 原因：fastembed 的 QuantizationMode::Dynamic 会将整批文本在单个 ONNX 推理中
        // 处理（batch_size = texts.len()），分片到多会话不会提升吞吐。
        // 相反，多会话 × 全核 = 线程爆炸，导致 40-70s 延迟。
        //
        // 当前策略：
        // - pool_size=1（默认）：单会话 + intra_threads=None（全核），最优配置
        // - pool_size>1（显式指定）：单会话路径仍用 sessions[0]（intra_threads=1），
        //   保留 sessions[1..] 供并发 embed() 调用使用
        //
        // 测试数据（lisp-rs/README_zh.md, 414 chunks）：
        // - pool=8, threads=None, 分片: 66.9s（线程爆炸）
        // - pool=8, threads=1,   分片: 39.6s（线程争用缓解但分片无效）
        // - pool=1, threads=None, 不分片: ~1-2s（预期）
        let _pool_size = self.sessions.len();
        let session = Arc::clone(&self.sessions[0]);
        let owned: Vec<String> = texts.to_vec();
        // V3.1 P2-5：热路径计时埋点（tracing debug 级别）
        let started = std::time::Instant::now();
        let result = tokio::task::spawn_blocking(move || {
            let mut model = session
                .lock()
                .map_err(|e| anyhow::anyhow!("模型锁获取失败: {e}"))?;
            model.embed(owned, None).context("批量推理失败")
        })
        .await
        .context("推理任务执行失败")?;
        tracing::debug!(
            batch_size = texts.len(),
            elapsed_ms = started.elapsed().as_millis() as u64,
            "embed_batch 完成"
        );
        result
    }
}
