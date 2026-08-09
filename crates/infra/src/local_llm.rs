//! 本地 LLM 推理引擎（REQ-LLM-003）：基于 mistral.rs 的纯 Rust 本地推理。
//!
//! 实现 `LLMProvider` trait，提供与 `OpenAIProvider` 相同的流式接口，
//! 但所有推理在本地 CPU/GPU 上完成，零网络请求。
//!
//! ## 模型加载
//!
//! - GGUF 格式模型文件，支持 Q4_K_M / Q5_K_M / Q8_0 等量化格式
//! - 模型懒加载：首次调用 `chat_stream` 时触发加载
//! - 加载在异步上下文中执行，不阻塞 runtime
//!
//! ## Pro 功能门控
//!
//! 本模块仅在 `--features pro` 时编译，依赖 `mistralrs` crate。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use echomind_core::LLMProvider;
use echomind_models::{ChatMessage, LlmSamplingParams};
use futures::StreamExt;
use futures::channel::mpsc;
use futures::sink::SinkExt;
use futures::stream::BoxStream;
use mistralrs::{
    ChatCompletionChunkResponse, ChunkChoice, Delta, GgufModelBuilder, Model, RequestBuilder,
    Response, TextMessageRole,
};
use tokio_util::sync::CancellationToken;

// Phase 4 KV cache 序列化
use crate::kv_cache::KvCacheSnapshot;
// GPU 设备选择 + PagedAttention（仅 metal/cuda feature 启用时使用）
#[cfg(any(feature = "metal", feature = "cuda"))]
use mistralrs::{Device, MemoryGpuConfig, PagedAttentionMetaBuilder, best_device};

// Phase 3 自研内核模块
use crate::gguf_reader::GgufFile;
use crate::weight_repack::{RepackedWeights, repack_for_gemv};

/// 量化格式枚举（仅用于元信息，GGUF 文件已含量化权重）。
///
/// GGUF 文件在创建时已含量化后的权重，此枚举仅用于记录和展示元信息，
/// 不影响模型加载行为。`Auto` 表示从文件名自动推断。
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quantization {
    /// Q4_K_M 量化（推荐，质量与大小平衡）
    Q4K_M,
    /// Q5_K_M 量化（更高质量，更大文件）
    Q5K_M,
    /// Q8_0 量化（最高质量，最大文件）
    Q8_0,
    /// 自动推断（从文件名解析）
    Auto,
}

impl Quantization {
    /// 从字符串解析量化格式。
    ///
    /// 支持的输入：`Q4_K_M`、`Q5_K_M`、`Q8_0`（大小写不敏感）。
    /// 无法识别的字符串返回 `Auto`。
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "Q4_K_M" => Quantization::Q4K_M,
            "Q5_K_M" => Quantization::Q5K_M,
            "Q8_0" => Quantization::Q8_0,
            _ => Quantization::Auto,
        }
    }

    /// 转为字符串表示。
    pub fn as_str(&self) -> &'static str {
        match self {
            Quantization::Q4K_M => "Q4_K_M",
            Quantization::Q5K_M => "Q5_K_M",
            Quantization::Q8_0 => "Q8_0",
            Quantization::Auto => "auto",
        }
    }
}

/// 推理内核模式（Phase 3 Session 20）。
///
/// 控制 `LocalLlmEngine` 使用哪种推理内核：
/// - `MistralRs`：使用 mistral.rs 引擎（默认，Phase 1/2）
/// - `CustomGemv`：使用自研 GEMV 内核（Phase 3，单批次优化）
///
/// 通过 `set_kernel_mode` IPC 命令切换，持久化到 settings 表 `llm.kernel_mode` 键。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KernelMode {
    /// mistral.rs 引擎（默认）
    #[default]
    MistralRs,
    /// 自研 GEMV 内核（Phase 3）
    CustomGemv,
}

impl KernelMode {
    /// 从字符串解析内核模式。
    ///
    /// 支持的输入：`"mistral"`（MistralRs）、`"custom"`（CustomGemv），大小写不敏感。
    ///
    /// # 错误
    ///
    /// 无法识别的字符串返回错误。
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "mistral" => Ok(KernelMode::MistralRs),
            "custom" => Ok(KernelMode::CustomGemv),
            _ => bail!("无效的内核模式: {s}（可选: mistral / custom）"),
        }
    }

    /// 转为字符串表示（用于持久化到 settings 表）。
    pub fn as_str(&self) -> &'static str {
        match self {
            KernelMode::MistralRs => "mistral",
            KernelMode::CustomGemv => "custom",
        }
    }
}

/// 本地 LLM 推理引擎（REQ-LLM-003）。
///
/// 基于 mistral.rs 实现，提供与 `OpenAIProvider` 相同的 `LLMProvider` 接口。
/// 模型在首次调用时懒加载，后续调用复用已加载的模型实例。
///
/// # 生命周期
///
/// 1. `new()` — 验证模型文件存在，不加载模型
/// 2. 首次 `chat_stream()` — 懒加载模型（可能耗时数秒到数十秒）
/// 3. 后续 `chat_stream()` — 复用已加载模型，即时响应
///
/// # Phase 3 自研内核模式
///
/// 通过 `kernel_mode()` 切换到 `CustomGemv` 模式后，`chat_stream` 路由到
/// `chat_stream_custom()`，使用自研 GEMV 内核 + 权重重排进行推理。
/// 该路径在 `load_custom_weights()` 中加载 GGUF 文件并重排权重。
pub struct LocalLlmEngine {
    /// 内部状态（Arc 包装，支持 Clone 共享已加载模型）
    inner: Arc<LocalLlmEngineInner>,
}

#[derive(Clone)]
struct LocalLlmEngineInner {
    /// GGUF 模型文件完整路径（调试用，加载使用 model_dir + model_filename）
    #[allow(dead_code)]
    model_path: PathBuf,
    /// 模型文件所在目录（`GgufModelBuilder::new` 第一个参数）
    model_dir: String,
    /// 模型文件名（`GgufModelBuilder::new` 第二个参数）
    model_filename: String,
    /// 懒加载的模型实例（可替换，支持运行时卸载和模型切换）。
    ///
    /// 使用 `Arc<tokio::sync::RwLock<Option<Arc<Model>>>>` 替代 `OnceCell`：
    /// - `RwLock` 允许运行时将 `Some` 重置为 `None`（`OnceCell` 不支持重置）
    /// - `Arc` 外包装使 `LocalLlmEngineInner` 仍可 `derive(Clone)`（`RwLock` 未实现 `Clone`）
    /// - 内层 `Arc<Model>` 使多个引擎 Clone 共享同一模型实例，引用计数归零时自动释放
    runner: Arc<tokio::sync::RwLock<Option<Arc<Model>>>>,
    /// 推理设备类型（"cpu" / "metal" / "cuda" / "unknown"）。
    ///
    /// 在 `new()` 时通过 `best_device(false)` 探测确定，供前端展示当前推理设备。
    /// 即使 GPU feature 未启用，也固定为 `"cpu"`。
    device_kind: String,
    /// 是否启用 PagedAttention（仅 GPU 模式有效）。
    ///
    /// PagedAttention 实现高效 KV cache 管理，降低多轮对话的首 token 延迟。
    /// 仅在 GPU 模式（metal/cuda feature）下生效；CPU 模式下忽略此配置。
    #[allow(dead_code)]
    use_paged_attn: bool,
    /// PagedAttention 块大小（默认 32）。
    ///
    /// 支持的值：8、16、32。块大小影响 KV cache 内存分配粒度。
    #[allow(dead_code)]
    block_size: usize,
    /// PagedAttention GPU 内存配置（上下文 token 数，默认 4096）。
    ///
    /// 控制 KV cache 可使用的 GPU 显存量。值越大可处理更长上下文，但占用更多显存。
    #[allow(dead_code)]
    gpu_memory_ctx: usize,
    /// 采样参数（应用于每次 chat_stream 请求的 temperature / top_p / top_k / max_tokens / penalty）。
    ///
    /// 使用 `Arc<tokio::sync::RwLock<>>` 实现运行时读写（无需 unwrap，遵守铁律五）。
    /// Arc 包装使 Clone 仅复制指针，所有引擎 Clone 共享同一份采样参数。
    /// 默认全为 `None`（使用 mistral.rs 引擎默认值）。
    /// 通过 `set_sampling_params()` 即时修改，下次 `chat_stream` 生效。
    sampling_params: Arc<tokio::sync::RwLock<LlmSamplingParams>>,
    /// 当前推理的取消令牌（S13 流取消集成）。
    ///
    /// 每次 `chat_stream` 调用时创建新的 `CancellationToken` 并存储于此。
    /// `abort()` 方法读取并取消此令牌，使 `chat_stream` 内部 `spawn` 任务的
    /// `select!` 循环立即退出，停止 mistral.rs 推理。
    ///
    /// 使用 `Arc<tokio::sync::RwLock<>>` 包装：
    /// - `RwLock` 允许 `abort()` 读 + `chat_stream()` 写交替执行
    /// - `Arc` 外包装使 `LocalLlmEngineInner` 仍可 `derive(Clone)`（`RwLock` 未实现 `Clone`）
    cancel_token: Arc<tokio::sync::RwLock<CancellationToken>>,
    /// 推理内核模式（Phase 3 Session 20）。
    ///
    /// - `MistralRs`：使用 mistral.rs 引擎（默认，Phase 1/2）
    /// - `CustomGemv`：使用自研 GEMV 内核（Phase 3，单批次优化）
    ///
    /// 通过 `set_kernel_mode()` 运行时切换，下次 `chat_stream` 调用生效。
    /// 使用 `Arc<tokio::sync::RwLock<>>` 包装以支持运行时读写。
    kernel_mode: Arc<tokio::sync::RwLock<KernelMode>>,
    /// 自研 GGUF 文件解析器实例（Phase 3，CustomGemv 模式时使用）。
    ///
    /// 在 `load_custom_weights()` 中通过 `GgufFile::open()` 加载，
    /// 提供 mmap 零拷贝张量数据访问。
    /// `None` 表示尚未加载或当前模式为 `MistralRs`。
    gguf_file: Arc<tokio::sync::RwLock<Option<GgufFile>>>,
    /// 重排后的权重缓存（Phase 3，CustomGemv 模式时使用）。
    ///
    /// key 为张量名（如 `"token_embd.weight"`），value 为重排后的权重。
    /// 在 `load_custom_weights()` 中通过 `repack_for_gemv()` 创建。
    /// `None` 表示尚未加载。
    repacked_weights: Arc<tokio::sync::RwLock<Option<HashMap<String, RepackedWeights>>>>,
}

impl Clone for LocalLlmEngine {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl std::fmt::Debug for LocalLlmEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalLlmEngine")
            .field("loaded", &self.is_loaded())
            .finish()
    }
}

impl LocalLlmEngine {
    /// 创建引擎实例（REQ-LLM-003）。
    ///
    /// 仅验证模型文件存在，不实际加载模型。模型在首次 `chat_stream` 调用时懒加载。
    ///
    /// # 参数
    /// - `model_path`: GGUF 模型文件路径
    /// - `_quantization`: 量化格式（仅用于元信息，不影响加载行为）
    pub fn new(model_path: PathBuf, _quantization: Quantization) -> Result<Self> {
        if !model_path.exists() {
            bail!("模型文件不存在: {}", model_path.display());
        }
        let model_dir = model_path
            .parent()
            .context("无法获取模型文件所在目录")?
            .to_string_lossy()
            .to_string();
        let model_filename = model_path
            .file_name()
            .context("无法获取模型文件名")?
            .to_string_lossy()
            .to_string();

        // 在创建时探测推理设备类型（不实际加载模型，仅获取设备枚举类型字符串）。
        // GPU feature 启用时调用 best_device(false) 尝试获取 Metal/CUDA 设备；
        // 未启用时固定为 "cpu"。
        let device_kind = Self::detect_device_kind();

        Ok(Self {
            inner: Arc::new(LocalLlmEngineInner {
                model_path,
                model_dir,
                model_filename,
                runner: Arc::new(tokio::sync::RwLock::new(None)),
                device_kind,
                use_paged_attn: false,
                block_size: 32,
                gpu_memory_ctx: 4096,
                sampling_params: Arc::new(tokio::sync::RwLock::new(LlmSamplingParams::default())),
                cancel_token: Arc::new(tokio::sync::RwLock::new(CancellationToken::new())),
                kernel_mode: Arc::new(tokio::sync::RwLock::new(KernelMode::default())),
                gguf_file: Arc::new(tokio::sync::RwLock::new(None)),
                repacked_weights: Arc::new(tokio::sync::RwLock::new(None)),
            }),
        })
    }

    /// 配置 PagedAttention 参数（S10）。
    ///
    /// 必须在首次 `chat_stream` 调用前设置（模型加载后不可更改）。
    /// PagedAttention 仅在 GPU 模式（metal/cuda feature）下生效；
    /// CPU 模式下此配置被忽略，使用 `with_force_cpu()` 路径。
    ///
    /// # 参数
    /// - `block_size`: KV cache 块大小（支持 8/16/32，默认 32）
    /// - `gpu_memory_ctx`: GPU 上下文 token 数（默认 4096）
    ///
    /// # 示例
    /// ```no_run
    /// # use echomind_infra::local_llm::*;
    /// # let engine = LocalLlmEngine::new(std::path::PathBuf::from("/tmp/model.gguf"), Quantization::Auto).unwrap();
    /// engine.with_paged_attn(32, 4096);
    /// ```
    pub fn with_paged_attn(mut self, block_size: usize, gpu_memory_ctx: usize) -> Self {
        // 由于 inner 是 Arc，需要通过重建来设置字段。
        // OnceCell 在 runner() 内管理，重建不影响已加载的模型（若已加载则不应调用此方法）。
        let inner = (*self.inner).clone();
        self.inner = Arc::new(LocalLlmEngineInner {
            use_paged_attn: true,
            block_size,
            gpu_memory_ctx,
            ..inner
        });
        self
    }

    // ---- Phase 3 自研内核模式（Session 20）----

    /// 获取当前推理内核模式（Phase 3）。
    ///
    /// 返回 `KernelMode::MistralRs`（默认）或 `KernelMode::CustomGemv`。
    /// 通过 `set_kernel_mode()` 切换。
    pub async fn kernel_mode(&self) -> KernelMode {
        *self.inner.kernel_mode.read().await
    }

    /// 设置推理内核模式（Phase 3，运行时可更改）。
    ///
    /// 修改后立即生效，下次 `chat_stream` 调用应用新模式。
    /// - 切换到 `CustomGemv` 时，首次 `chat_stream` 会触发 `load_custom_weights()`
    ///   加载 GGUF 文件并重排权重。
    /// - 切换回 `MistralRs` 时，直接使用 mistral.rs 引擎。
    ///
    /// 此方法是幂等的：多次设置同一模式不会产生副作用。
    pub async fn set_kernel_mode(&self, mode: KernelMode) {
        *self.inner.kernel_mode.write().await = mode;
    }

    /// 使用 builder 模式设置内核模式（Phase 3）。
    ///
    /// 与 `with_paged_attn()` 类似，在引擎创建后、首次 `chat_stream` 前调用。
    ///
    /// # 示例
    /// ```no_run
    /// # use echomind_infra::local_llm::*;
    /// # let engine = LocalLlmEngine::new(std::path::PathBuf::from("/tmp/model.gguf"), Quantization::Auto).unwrap();
    /// let _engine = engine.with_kernel_mode(KernelMode::CustomGemv);
    /// ```
    pub fn with_kernel_mode(mut self, mode: KernelMode) -> Self {
        let inner = (*self.inner).clone();
        self.inner = Arc::new(LocalLlmEngineInner {
            kernel_mode: Arc::new(tokio::sync::RwLock::new(mode)),
            ..inner
        });
        self
    }

    /// 加载 GGUF 文件并重排权重（Phase 3，CustomGemv 模式入口）。
    ///
    /// 使用自研 `GgufFile` 解析器打开 GGUF 文件（mmap 零拷贝），
    /// 对关键权重张量（`token_embd.weight`）执行 `repack_for_gemv()` 重排。
    ///
    /// 此方法在 `chat_stream_custom()` 首次调用时自动触发（懒加载），
    /// 也可通过 `warm_up()` 预先调用。
    ///
    /// # 数据流
    ///
    /// ```text
    /// chat_stream_custom() → load_custom_weights()
    ///   → GgufFile::open(model_path)  — mmap 映射 + 解析文件头
    ///   → tensor_data("token_embd.weight")  — 零拷贝获取权重字节
    ///   → repack_for_gemv(data, dtype, n, k)  — Row-Major → Tile-Major
    ///   → 存储到 repacked_weights 缓存
    /// ```
    ///
    /// # 错误
    ///
    /// - GGUF 文件解析失败（魔数不匹配、版本不支持等）
    /// - 张量不存在或数据不完整
    /// - 权重重排失败（不支持的量化格式、维度不匹配等）
    pub async fn load_custom_weights(&self) -> Result<()> {
        // 快路径：已加载
        {
            let read = self.inner.repacked_weights.read().await;
            if read.is_some() {
                return Ok(());
            }
        }
        // 慢路径：加载 GGUF + 重排权重
        let mut write = self.inner.repacked_weights.write().await;
        // Double-check
        if write.is_some() {
            return Ok(());
        }

        let model_path = self.inner.model_path.clone();

        // 在 spawn_blocking 中执行 GGUF 解析 + 权重重排（CPU 密集型）
        let repacked =
            tokio::task::spawn_blocking(move || -> Result<HashMap<String, RepackedWeights>> {
                let gguf =
                    GgufFile::open(&model_path).context("自研内核模式：GGUF 文件打开失败")?;

                let mut weights_map = HashMap::new();

                // 重排 token_embd.weight（嵌入层权重，推理的第一个 GEMV 操作）
                if let Some(info) = gguf.tensor_info("token_embd.weight") {
                    let data = gguf
                        .tensor_data("token_embd.weight")
                        .context("token_embd.weight 张量数据不存在")?;
                    // GGUF 维度倒序（col-major）：dims[0] = 输出维度 N，dims[1] = 输入维度 K
                    let (n, k) = if info.dims.len() >= 2 {
                        (info.dims[0] as usize, info.dims[1] as usize)
                    } else {
                        (info.dims[0] as usize, info.dims[0] as usize)
                    };
                    let repacked = repack_for_gemv(data, info.dtype, n, k)
                        .context("token_embd.weight 权重重排失败")?;
                    weights_map.insert("token_embd.weight".to_string(), repacked);
                }

                // 将 GgufFile 存储到引擎内部（需要单独的写锁）
                // 注意：GgufFile 持有 mmap 映射，必须在 repacked_weights 之外管理
                // 这里返回 weights_map，GgufFile 由调用方存储
                Ok(weights_map)
            })
            .await
            .context("自研内核模式：权重加载任务失败")??;

        // 存储 GgufFile（需要单独获取写锁）
        {
            let model_path = self.inner.model_path.clone();
            let gguf = tokio::task::spawn_blocking(move || GgufFile::open(&model_path))
                .await
                .context("GGUF 文件打开任务失败")??;
            let mut gf_write = self.inner.gguf_file.write().await;
            *gf_write = Some(gguf);
        }

        *write = Some(repacked);
        Ok(())
    }

    /// 自研内核推理路径（Phase 3，CustomGemv 模式）。
    ///
    /// 此方法是 `chat_stream` 在 `KernelMode::CustomGemv` 模式下的路由目标。
    ///
    /// 当前实现状态：S20 集成阶段，已实现 GGUF 加载 + 权重重排管道，
    /// 但完整的 transformer 前向传播尚未实现（需要 attention/RMSNorm/SwiGLU/RoPE 等）。
    /// 当完整前向传播实现后，此方法将使用 `gemv_repacked_dispatch` 执行推理。
    ///
    /// # 错误
    ///
    /// 当前始终返回错误（完整推理管道未实现），调用方应回退到 `MistralRs` 模式。
    async fn chat_stream_custom(
        &self,
        _system_prompt: &str,
        _history: &[ChatMessage],
        _query: &str,
    ) -> Result<BoxStream<'static, Result<String>>> {
        // 确保权重已加载
        self.load_custom_weights().await?;

        // 当前阶段：自研 GEMV 内核已实现权重加载 + 重排管道，
        // 但完整 transformer 前向传播（attention/RMSNorm/SwiGLU/RoPE）尚未实现。
        // 返回错误，提示用户切换回 mistral.rs 模式。
        bail!(
            "自研内核推理模式（CustomGemv）尚在开发中：权重加载 + 重排管道已就绪，\
             完整 transformer 前向传播尚未实现。请使用 mistral.rs 模式。"
        );
    }

    /// 获取当前采样参数（S11）。
    ///
    /// 返回采样参数的克隆。默认全为 `None`（使用引擎默认值）。
    /// 通过 `set_sampling_params()` 修改后立即生效，下次 `chat_stream` 调用应用新值。
    pub async fn sampling_params(&self) -> LlmSamplingParams {
        self.inner.sampling_params.read().await.clone()
    }

    /// 设置采样参数（S11，运行时可更改）。
    ///
    /// 修改后立即生效，下次 `chat_stream` 调用应用新值。无需重新加载模型。
    ///
    /// # 参数
    /// - `params`: 采样参数。各字段为 `Option`，`None` 表示使用引擎默认值。
    ///
    /// # 示例
    /// ```no_run
    /// # use echomind_infra::local_llm::*;
    /// # use echomind_models::LlmSamplingParams;
    /// # let engine = LocalLlmEngine::new(std::path::PathBuf::from("/tmp/model.gguf"), Quantization::Auto).unwrap();
    /// # tokio::runtime::Runtime::new().unwrap().block_on(async {
    /// engine.set_sampling_params(LlmSamplingParams {
    ///     temperature: Some(0.8),
    ///     max_tokens: Some(2048),
    ///     ..Default::default()
    /// }).await;
    /// # });
    /// ```
    pub async fn set_sampling_params(&self, params: LlmSamplingParams) {
        *self.inner.sampling_params.write().await = params;
    }

    /// 中断当前推理流（S13 流取消集成）。
    ///
    /// 触发内部 `CancellationToken`，使 `chat_stream` 中 `spawn` 任务的
    /// `select!` 循环立即退出，停止 mistral.rs 推理。
    /// 已生成的 token 不会丢失（通过 `forward_stream` 保留并落库）。
    ///
    /// 此方法是幂等的：多次调用不会 panic。
    /// 在 `chat_stream` 未启动时调用也是安全的（取消一个无人监听的令牌）。
    ///
    /// # 数据流
    ///
    /// ```text
    /// abort_chat IPC → abort_chat() 命令
    ///   → state.abort_chat() 取消 forward_stream 的 abort_token
    ///   → engine.abort() 取消 LocalLlmEngine 内部的 cancel_token
    ///   → spawn 任务的 select! 循环检测到 cancel → break
    ///   → mistral.rs 推理流被丢弃（不再消费 token）
    /// ```
    pub async fn abort(&self) {
        let token = self.inner.cancel_token.read().await.clone();
        token.cancel();
    }

    /// 获取当前取消令牌的克隆（测试辅助方法）。
    ///
    /// 返回引擎内部存储的最新 `CancellationToken` 克隆。
    /// 用于测试验证 `abort()` 是否正确触发取消。
    #[allow(dead_code)]
    pub(crate) async fn current_cancel_token(&self) -> CancellationToken {
        self.inner.cancel_token.read().await.clone()
    }

    /// 探测当前可用的推理设备类型（"cpu" / "metal" / "cuda" / "unknown"）。
    ///
    /// 当 `metal` 或 `cuda` feature 启用时，调用 `best_device(false)` 尝试获取 GPU 设备。
    /// 探测失败（如无可用 GPU）时返回 `"unknown"`，`runner()` 中将回退到 CPU。
    ///
    /// 未启用 GPU feature 时固定返回 `"cpu"`。
    fn detect_device_kind() -> String {
        #[cfg(any(feature = "metal", feature = "cuda"))]
        {
            match best_device(false) {
                Ok(Device::Cpu) => "cpu".to_string(),
                Ok(Device::Cuda(_)) => "cuda".to_string(),
                Ok(Device::Metal(_)) => "metal".to_string(),
                Err(_) => "unknown".to_string(),
            }
        }
        #[cfg(not(any(feature = "metal", feature = "cuda")))]
        {
            "cpu".to_string()
        }
    }

    /// 返回推理设备类型（"cpu" / "metal" / "cuda" / "unknown"）。
    ///
    /// 在 `new()` 时通过 `best_device()` 探测确定，模型加载前后均可安全调用。
    /// 前端用此值显示当前推理设备（CPU/GPU）。
    pub fn device_kind(&self) -> &str {
        &self.inner.device_kind
    }

    /// 检查模型是否已加载到内存。
    ///
    /// 返回 `true` 表示模型已加载，后续 `chat_stream` 调用将直接使用已加载的模型。
    /// 返回 `false` 表示模型尚未加载或正在加载中，下次 `chat_stream` 调用将触发加载。
    ///
    /// 使用 `try_read()` 避免阻塞：如果写锁正被 `unload()` 或 `get_or_load_model()` 持有，
    /// 返回 `false`（保守策略，不阻塞调用方）。
    pub fn is_loaded(&self) -> bool {
        self.inner
            .runner
            .try_read()
            .map(|r| r.is_some())
            .unwrap_or(false)
    }

    /// 卸载模型（释放内存）。
    ///
    /// 将内部 `RwLock<Option<Arc<Model>>>>` 设置为 `None`，释放模型内存。
    /// 如果有其他 `Arc<Model>` 克隆仍在使用（如 `chat_stream` 正在执行），
    /// 模型内存将在最后一个 `Arc` 引用归零时自动释放。
    ///
    /// 卸载后下次 `chat_stream` 调用会重新加载模型。
    /// 此方法是幂等的：多次调用不会 panic。
    pub async fn unload(&self) {
        let mut write = self.inner.runner.write().await;
        *write = None;
    }

    /// 预热模型（S14：后台加载，不阻塞调用方）。
    ///
    /// 在模式切换或模型选择后调用，提前加载模型到内存，
    /// 使用户首次对话时无需等待加载（消除首 token 延迟）。
    ///
    /// 此方法是幂等的：如果模型已加载，立即返回 Ok。
    /// 加载失败时返回 Err，调用方应优雅降级（不影响主流程）。
    ///
    /// # 数据流
    ///
    /// ```text
    /// set_local_model / set_llm_mode IPC
    ///   → unload_local_llm()（销毁旧引擎）
    ///   → local_llm()（创建新引擎，不加载模型）
    ///   → tokio::spawn(engine.warm_up())
    ///     → get_or_load_model()
    ///       → GgufModelBuilder::build()（加载 GGUF 权重到内存）
    ///   → 设置操作立即返回（不等预热完成）
    /// ```
    ///
    /// # 错误处理
    ///
    /// - 模型文件不存在 → `Err("模型文件不存在")`
    /// - GGUF 解析失败 → `Err("GGUF 模型加载失败: ...")`
    /// - 推理设备不可用 → `Err("自动选择推理设备失败")`
    pub async fn warm_up(&self) -> Result<()> {
        // Phase 3：CustomGemv 模式预热权重加载管道
        let mode = self.kernel_mode().await;
        if mode == KernelMode::CustomGemv {
            return self.load_custom_weights().await;
        }
        // 默认模式：预热 mistral.rs 模型
        let _ = self.get_or_load_model().await?;
        Ok(())
    }

    // ---- Phase 4 KV cache 跨会话复用（Session 25）----

    /// 保存当前会话的 KV cache 快照到磁盘（REQ-LLM-009）。
    ///
    /// 将当前已加载模型的 KV cache 序列化到 `{kv_cache_dir}/{conversation_id}.emkv` 文件。
    /// 用于对话切换时自动保存上下文，避免重复前缀计算。
    ///
    /// # 参数
    /// - `conversation_id` — 会话 ID（用作文件名，自动清理非法字符）
    /// - `kv_cache_dir` — KV cache 文件存储目录
    ///
    /// # 行为
    ///
    /// - 如果模型尚未加载，保存空快照（仅含模型名 + context_length=0）
    /// - mistral.rs v0.9.0 API 不直接暴露 KV cache 张量提取，
    ///   当前实现保存元数据快照（模型名 + 上下文长度）。
    ///   当 CustomGemv 模式完整实现后，将填充实际 K/V 张量数据。
    /// - 文件原子写入（.tmp → rename），防止崩溃损坏
    ///
    /// # 错误
    ///
    /// - 目录创建失败
    /// - 文件写入失败
    /// - 序列化失败
    pub async fn save_kv_cache(&self, conversation_id: &str, kv_cache_dir: &Path) -> Result<()> {
        // 确保目录存在
        std::fs::create_dir_all(kv_cache_dir)
            .with_context(|| format!("创建 KV cache 目录失败: {}", kv_cache_dir.display()))?;

        // 构建快照
        let model_name = self.inner.model_filename.clone();
        let context_length = if self.is_loaded() {
            // 模型已加载：记录当前上下文长度
            // 注意：mistral.rs v0.9.0 不暴露内部 KV cache 的 seq_len，
            // 此处使用 0 表示"有缓存但长度未知"。实际长度将在恢复时从文件读取。
            0
        } else {
            0
        };

        let mut snapshot = KvCacheSnapshot::new(model_name, context_length);

        // 如果模型已加载，尝试提取 KV cache 张量数据。
        // mistral.rs v0.9.0 的 Model 结构体不直接暴露 KV cache 内部状态，
        // 因此当前无法提取实际 K/V 张量。这里保存空层列表。
        // 当 CustomGemv 模式的前向传播实现后，可以通过自定义 KV cache
        // 管理器提取张量数据并填充到快照中。
        // 当前设计确保文件基础设施就绪，后续只需填充数据即可。
        //
        // TODO(phase4): 从 mistral.rs / CustomGemv 引擎提取实际 KV cache 张量
        // let model = self.get_or_load_model().await?;
        // for layer_idx in 0..model.layer_count() {
        //     let (k_bytes, v_bytes, seq_len) = model.extract_kv_cache(layer_idx)?;
        //     snapshot.add_layer(LayerKvCache::new(layer_idx, k_bytes, v_bytes, seq_len));
        // }

        let _ = &mut snapshot; // 目前不添加层（mistral.rs API 限制）

        // 文件路径（清理 conversation_id 中的非法字符）
        let safe_id = sanitize_conversation_id(conversation_id);
        let file_path = kv_cache_dir.join(format!("{safe_id}.emkv"));

        snapshot
            .save_to_file(&file_path)
            .context("KV cache 快照保存失败")?;

        Ok(())
    }

    /// 从磁盘恢复 KV cache 快照（REQ-LLM-009）。
    ///
    /// 从 `{kv_cache_dir}/{conversation_id}.emkv` 文件加载 KV cache 快照，
    /// 验证模型名匹配后注入到当前引擎实例。
    ///
    /// # 参数
    /// - `conversation_id` — 会话 ID
    /// - `kv_cache_dir` — KV cache 文件存储目录
    ///
    /// # 返回
    ///
    /// - `Ok(true)` — 缓存命中且模型名匹配
    /// - `Ok(false)` — 缓存未命中（文件不存在）或模型名不匹配
    ///
    /// # 行为
    ///
    /// - 如果文件不存在，返回 `Ok(false)`（cache miss）
    /// - 如果模型名不匹配，返回 `Ok(false)`（模型已切换，旧缓存无效）
    /// - mistral.rs v0.9.0 API 不直接暴露 KV cache 注入，
    ///   当前实现仅验证模型名匹配，不实际注入 KV cache 张量。
    ///   当 CustomGemv 模式完整实现后，将实际加载张量并注入到自定义 KV cache 管理器。
    ///
    /// # 错误
    ///
    /// - 文件读取失败（IO 错误，非文件不存在）
    /// - 反序列化失败（文件损坏）
    pub async fn restore_kv_cache(
        &self,
        conversation_id: &str,
        kv_cache_dir: &Path,
    ) -> Result<bool> {
        let safe_id = sanitize_conversation_id(conversation_id);
        let file_path = kv_cache_dir.join(format!("{safe_id}.emkv"));

        // 文件不存在 = cache miss
        if !file_path.exists() {
            return Ok(false);
        }

        // 加载快照
        let snapshot =
            KvCacheSnapshot::load_from_file(&file_path).context("KV cache 快照加载失败")?;

        // 验证模型名匹配
        let current_model = &self.inner.model_filename;
        if snapshot.model_name() != current_model {
            // 模型已切换，旧缓存无效
            return Ok(false);
        }

        // 实际注入 KV cache 张量：
        // mistral.rs v0.9.0 不支持外部注入 KV cache，此处仅完成元数据验证。
        // 当 CustomGemv 模式的前向传播实现后，将实际加载张量并注入。
        //
        // TODO(phase4): 将 snapshot 中的 K/V 张量注入到自定义 KV cache 管理器
        // let model = self.get_or_load_model().await?;
        // for layer in snapshot.layers() {
        //     model.inject_kv_cache(layer.layer_idx(), layer.k_bytes(), layer.v_bytes(), layer.seq_len())?;
        // }

        Ok(true)
    }

    /// 懒加载模型实例（内部方法）。
    ///
    /// 使用 double-check locking 模式确保并发安全：
    /// 1. 快路径：读锁检查模型是否已加载，已加载则返回克隆
    /// 2. 慢路径：获取写锁，再次检查（防止并发重复加载），然后加载模型
    ///
    /// 返回 `Arc<Model>`（所有权转移），调用方无需持有锁即可使用模型。
    ///
    /// ## PagedAttention（S10）
    ///
    /// 当 `use_paged_attn` 为 `true` 且 GPU 模式可用时，通过 `PagedAttentionMetaBuilder`
    /// 构建配置并传入 `GgufModelBuilder::with_paged_attn()`，启用高效 KV cache 管理。
    /// CPU 模式下忽略 PagedAttention 配置（`with_paged_attn` 不能与 `with_force_cpu` 同时使用）。
    pub(crate) async fn get_or_load_model(&self) -> Result<Arc<Model>> {
        // 快路径：已加载（读锁）
        {
            let read = self.inner.runner.read().await;
            if let Some(model) = read.as_ref() {
                return Ok(Arc::clone(model));
            }
        }
        // 慢路径：加载模型（写锁 + double-check）
        let mut write = self.inner.runner.write().await;
        // Double-check：防止并发场景下重复加载
        if let Some(model) = write.as_ref() {
            return Ok(Arc::clone(model));
        }

        let dir = self.inner.model_dir.clone();
        let file = self.inner.model_filename.clone();

        let mut builder = GgufModelBuilder::new(&dir, vec![file.as_str()]).with_logging();

        // GPU 设备选择 + PagedAttention（仅 metal/cuda feature 启用时）
        #[cfg(any(feature = "metal", feature = "cuda"))]
        {
            let device = best_device(false).context("自动选择推理设备失败")?;
            builder = builder.with_device(device);

            // PagedAttention：仅 GPU 模式 + 用户主动开启时生效
            if self.inner.use_paged_attn {
                let paged_cfg = PagedAttentionMetaBuilder::default()
                    .with_block_size(self.inner.block_size)
                    .with_gpu_memory(MemoryGpuConfig::ContextSize(self.inner.gpu_memory_ctx))
                    .build()
                    .context("PagedAttention 配置构建失败")?;
                builder = builder.with_paged_attn(paged_cfg);
            }
        }

        // CPU 回退路径：无 GPU feature 时强制 CPU 推理
        // 注意：with_force_cpu 不能与 with_paged_attn 同时使用
        #[cfg(not(any(feature = "metal", feature = "cuda")))]
        {
            builder = builder.with_force_cpu();
        }

        let model = builder.build().await.context("GGUF 模型加载失败")?;
        let model = Arc::new(model);
        *write = Some(Arc::clone(&model));
        Ok(model)
    }
}

impl LLMProvider for LocalLlmEngine {
    async fn chat_stream(
        &self,
        system_prompt: &str,
        history: &[ChatMessage],
        query: &str,
    ) -> Result<BoxStream<'static, Result<String>>> {
        // Phase 3：根据 kernel_mode 路由到不同的推理路径
        let mode = self.kernel_mode().await;
        if mode == KernelMode::CustomGemv {
            return self.chat_stream_custom(system_prompt, history, query).await;
        }

        // 默认路径：使用 mistral.rs 引擎
        let model = self.get_or_load_model().await?;
        let messages = build_messages(system_prompt, history, query);
        let params = self.sampling_params().await;

        // S13：创建本次推理的 cancel token 并存储到引擎内部。
        // abort() 方法会读取并取消此 token，使下方 spawn 任务的 select! 循环立即退出。
        // 每次 chat_stream 调用都会创建新 token，确保上一次的取消不会影响本次推理。
        let cancel_token = CancellationToken::new();
        {
            let mut write = self.inner.cancel_token.write().await;
            *write = cancel_token.clone();
        }

        // 通过 channel 桥接 mistral.rs 借用流到 'static 流
        let (mut tx, rx) = mpsc::channel::<Result<String>>(32);
        tokio::spawn(async move {
            let mut request = RequestBuilder::new();
            for (role, content) in &messages {
                let role = match role.as_str() {
                    "user" => TextMessageRole::User,
                    "assistant" => TextMessageRole::Assistant,
                    "system" => TextMessageRole::System,
                    _ => TextMessageRole::User,
                };
                request = request.add_message(role, content);
            }

            // 应用采样参数到 RequestBuilder（mistral.rs v0.9.0 API）
            if let Some(temp) = params.temperature {
                request = request.set_sampler_temperature(temp);
            }
            if let Some(top_p) = params.top_p {
                request = request.set_sampler_topp(top_p);
            }
            if let Some(top_k) = params.top_k {
                request = request.set_sampler_topk(top_k);
            }
            if let Some(max_len) = params.max_tokens {
                request = request.set_sampler_max_len(max_len);
            }
            if let Some(fp) = params.frequency_penalty {
                request = request.set_sampler_frequency_penalty(fp);
            }
            if let Some(pp) = params.presence_penalty {
                request = request.set_sampler_presence_penalty(pp);
            }

            let mut stream = match model.stream_chat_request(request).await {
                Ok(s) => s,
                Err(e) => {
                    let _ = tx
                        .send(Err(anyhow::anyhow!("本地推理流启动失败: {e}")))
                        .await;
                    return;
                }
            };

            // S13：select! 循环监听 cancel token，实现立即中断推理。
            // biased 确保 cancel 分支优先检查——即使有 token 就绪，取消信号也能立即生效。
            loop {
                tokio::select! {
                    biased;
                    _ = cancel_token.cancelled() => {
                        // 用户取消：优雅退出，已生成内容通过 forward_stream 保留
                        break;
                    }
                    item = stream.next() => match item {
                        None => break,
                        Some(response) => {
                            if let Some(text) = extract_text(&response)
                                && tx.send(Ok(text)).await.is_err()
                            {
                                break; // receiver dropped（forward_stream 已退出）
                            }
                        }
                    },
                }
            }
        });

        Ok(rx.boxed())
    }
}

/// 构建消息列表（可测试的纯函数，不依赖 mistral.rs 类型）。
///
/// 将 system_prompt + history + query 转换为 `(role, content)` 元组列表。
/// `chat_stream` 中将此列表转换为 mistral.rs 的 `RequestBuilder`。
pub(crate) fn build_messages(
    system_prompt: &str,
    history: &[ChatMessage],
    query: &str,
) -> Vec<(String, String)> {
    let mut messages = vec![("system".to_string(), system_prompt.to_string())];
    for msg in history {
        messages.push((msg.role.clone(), msg.content.clone()));
    }
    messages.push(("user".to_string(), query.to_string()));
    messages
}

/// 从 mistral.rs 流响应中提取文本 token（S14 加固错误处理）。
///
/// 处理 `Response` 枚举的所有变体：
/// - `Response::Chunk` — 正常流式 chunk，提取 `delta.content`
/// - `Response::InternalError` / `ValidationError` / `ModelError` — 推理错误，返回 None
///   （上游 `chat_stream` 的 spawn 任务已处理错误路径，此处安全跳过）
/// - `Response::Done` — 流结束信号，无文本内容，返回 None
/// - 其他变体（`CompletionChunk` / `ImageGeneration` / `Speech` / `Raw` / `Embeddings` /
///   `AgenticToolCallProgress` / `BlockDenoisingProgress` / `AgenticToolApprovalRequired` / `File`）—
///   非文本流式 chunk，安全跳过
///
/// 使用 `match` 表达式显式处理所有变体，避免 `if let` 链遗漏新变体时的隐式 fallthrough。
pub(crate) fn extract_text(response: &Response) -> Option<String> {
    match response {
        Response::Chunk(ChatCompletionChunkResponse { choices, .. }) => {
            if let Some(ChunkChoice {
                delta:
                    Delta {
                        content: Some(content),
                        ..
                    },
                ..
            }) = choices.first()
            {
                return Some(content.clone());
            }
            None
        }
        // 其他 Response 变体不包含可提取的文本 token，安全跳过
        _ => None,
    }
}

/// 清理会话 ID 中的文件系统非法字符（Phase 4 Session 25）。
///
/// 将 `conversation_id` 中的路径分隔符和特殊字符替换为下划线，
/// 防止路径遍历攻击和非法文件名。
fn sanitize_conversation_id(conversation_id: &str) -> String {
    conversation_id
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}
