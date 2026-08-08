# EchoMind 纯 Rust 本地 LLM 引擎规划

> **状态**：Proposal ｜ **创建日期**：2026-07-30 ｜ **作者**：AI Agent
>
> **调研依据**（铁律八互联网搜索，2026-07-30）：
> - rabbit (ferrumox/rabbit, ⭐87) — MoE 专家磁盘流式加载
> - Kortex — 跨内存层次流式推理
> - smedjan (smedjan/smedjan, ⭐0) — 纯 Rust 训练+推理引擎
> - RAI (Classevelabs/rai, ⭐3) — 纯 CPU AVX2 推理引擎
> - OxiLLaMa (cool-japan/oxillama, ⭐25) — llama.cpp 纯 Rust 替代
> - candle (huggingface/candle, ⭐20,798) — HuggingFace ML 框架
> - mistral.rs (EricLBuehler/mistral.rs, ⭐7,544) — 高速 LLM 推理引擎
> - burn (tracel-ai/burn, ⭐15,681) — 深度学习框架
> - ratchet (huggingface/ratchet, ⭐768) — 浏览器 WebGPU ML 框架

---

## 1. 背景与动机

### 1.1 现状

EchoMind 当前采用 **BYOK（Bring Your Own Key）模式**：所有 LLM 推理通过 `OpenAIProvider` 发送到用户配置的外部 API 端点（OpenAI / DeepSeek / Ollama 等），使用 reqwest SSE 流式接收 token。

```
用户查询 → 向量检索（本地 ONNX）→ 构建 RAG Prompt → [外部 API] → SSE 流式 token
                                                      ↑
                                                   网络出口
```

**痛点**：
1. **隐私顾虑**：高敏感场景（律师、研究人员）即使 BYOK，查询内容仍需离开本机
2. **离线不可用**：无网络时完全无法对话，与「本地优先」定位矛盾
3. **API 成本**：频繁查询的 Pro 用户产生持续费用
4. **延迟依赖**：首 token 延迟受网络 RTT 影响（通常 200ms~2s）

### 1.2 目标

在 EchoMind 现有架构中集成**纯 Rust 本地 LLM 推理引擎**，实现：

| 能力 | 描述 |
|---|---|
| 完全离线推理 | 零网络请求，query → token 全程本地 |
| GGUF 模型加载 | 支持主流量化格式（Q4_K_M / Q5_K_M / Q8_0） |
| 流式输出 | token 级流式，与现有 `LLMProvider` trait 无缝对接 |
| CPU + GPU | CPU（AVX2/FMA）优先，Metal/CUDA 可选加速 |
| 模型管理 | 下载、切换、删除本地模型文件 |

### 1.3 设计约束

| 约束 | 来源 |
|---|---|
| 纯 Rust，零 C/C++ FFI | 铁律九：Rust 生态优先 |
| 六边形架构 | `LLMProvider` trait 不变，新增适配器 |
| Edition 2024 / Rust ≥1.85 | workspace 锁定 |
| clippy 零警告 | 铁律五 |
| Pro 功能门控 | `#[cfg(feature = "pro")]` |
| TDD 驱动 | 铁律二 |

---

## 2. 参考项目深度调研

### 2.1 rabbit — MoE 专家磁盘流式加载

| 属性 | 值 |
|---|---|
| 仓库 | `ferrumox/rabbit` |
| Stars | 87 |
| 语言 | 100% Rust |
| 更新日期 | 2026-07-30（活跃） |
| 核心能力 | 在 128GB 内存消费级机器上运行 Kimi K3 2.8T MoE 模型 |

**技术亮点**：

1. **MoE 专家按需流式加载**：稠密部分（attention、shared experts、embeddings）常驻 RAM；路由专家（routed experts）按需从磁盘流式加载，通过 per-layer LRU 缓存 + 持久化热点专家 pin + OS page cache 三级缓存
2. **io_uring 批量专家加载**：Linux 上使用 `io_uring` 批量 `pread`，带顺序 fallback
3. **AVX2 + AVX-512/VNNI 内核**：运行时选择最优 SIMD 指令集
4. **多原生量化格式**：直接读取 OCP MXFP4 / FP8 (E4M3) / int4/int8/int2，无需转换
5. **MLA attention + weight absorption**：解码时无需 per-token k/v 重建
6. **专家使用缓存**（`.rabbit_usage`）：学习用户路由模式，惰性 pin 热点专家
7. **KV-cache 持久化**（`--session`）：对话跨重启热恢复
8. **OpenAI 兼容 API**：`--serve` 模式

**架构启发**：
- 专家流式加载策略适用于超大 MoE 模型在有限内存下运行
- LRU + page cache 多级缓存设计可复用于 EchoMind 的模型权重管理
- `rayon` 并行化 attention heads 和 matmul 的模式可直接借鉴

### 2.2 Kortex — 跨内存层次流式推理

| 属性 | 值 |
|---|---|
| 核心能力 | NVMe→RAM→VRAM→计算 跨层流式推理 |
| 性能 | 20GB 显卡跑 Llama-3.3-70B 达 ~2 tok/s，比 llama.cpp 快 9 倍 |
| 发布日期 | 2026-07-05 |

**技术亮点**：

1. **跨内存层次管线**：模型权重按需从 NVMe 流入 RAM，再从 RAM 流入 VRAM，最后在 GPU 上计算——突破了「模型必须完全装入显存」的限制
2. **层级预取**：在计算当前层时，后台异步预取下一层的权重到上层缓存
3. **极致显存利用**：20GB 显存即可运行 70B 模型（传统方法需要 ~40GB+）

**架构启发**：
- 跨层流式管线设计适用于 EchoMind 在内存受限设备上运行大模型
- 异步预取 + 计算重叠的流水线模式可大幅降低首 token 延迟
- 内存层次抽象（NVMe/RAM/VRAM）可作为引擎设计的核心抽象

### 2.3 smedjan — 全栈纯 Rust LLM 引擎

| 属性 | 值 |
|---|---|
| 仓库 | `smedjan/smedjan` |
| Stars | 0（新项目） |
| 代码量 | ~45,000 行 Rust |
| 核心能力 | 零 Python/PyTorch/云，可训练、微调、对齐、量化、部署 |
| 发布日期 | 2026-07-01 |

**技术亮点**：

1. **全流程覆盖**：预训练 → 知识蒸馏 → SFT → DPO 对齐 → 量化 → 推理，一条龙
2. **双 GPU 后端**：Metal（macOS/Apple Silicon）+ CUDA（NVIDIA），编译时选择
3. **Tape-based autograd**：自研反向模式自动微分
4. **BPE 分词器训练**：从语料训练自定义分词器
5. **推测解码**：draft-model + self-speculative 两种模式
6. **检查点跨平台**：Mac 训练 → H100 恢复，同一检查点格式
7. **数据管线**：质量过滤 + 去重 + 二进制训练格式

**架构启发**：
- 双后端抽象（Metal/CUDA）的编译时选择模式适用于 EchoMind 跨平台需求
- 全流程覆盖证明了纯 Rust LLM 栈的可行性
- 检查点格式设计可借鉴用于 EchoMind 的模型管理

### 2.4 RAI — 纯 CPU 推理引擎

| 属性 | 值 |
|---|---|
| 仓库 | `Classevelabs/rai` |
| Stars | 3 |
| 核心能力 | 纯 CPU LLM 推理，手写 AVX2 内核，4-bit 量化，推测解码 |
| 发布日期 | 2026-07-03 |

**技术亮点**：

1. **CPU-only by design**：AVX2 + FMA + F16C，2013 年后所有 x86 CPU 均支持
2. **4-bit 权重寄存器内反量化**：权重在内存中保持打包状态，GEMM 内循环中即时解包——零 FP32 权重副本
3. **`.raimodel` 格式**：64 字节头 + 单一二进制 blob，零拷贝加载
4. **GPTQ 量化**：Hessian 引导误差补偿 + 自适应位分配
5. **推测解码**：draft-model + self-speculative（前 N 层），数学精确验证
6. **HTTP/MCP server**：REST + MCP 协议，可作为 Claude Desktop 工具后端
7. **极简依赖树**：整个运行时依赖树放入一个 `Cargo.lock`

**架构启发**：
- 寄存器内反量化策略极大降低内存占用——7B Q4 模型仅需 ~3.5GB RAM
- `.raimodel` 单文件格式设计简洁，可作为 EchoMind 模型管理的参考
- 极简依赖树理念与 EchoMind 的纯 Rust 原则完全一致

### 2.5 OxiLLaMa — llama.cpp 纯 Rust 替代

| 属性 | 值 |
|---|---|
| 仓库 | `cool-japan/oxillama` |
| Stars | 25 |
| 代码量 | ~165,000 行 Rust（11 crates） |
| 核心能力 | 完整 GGUF 加载，25 种量化格式，20 种架构，OpenAI 兼容 API |
| 发布日期 | 2026-04-05 |

**技术亮点**：

1. **全量化格式**：Q4_0~Q8_0, K-quants, I-quants, Q1_0_G128, FP16, BF16, FP32
2. **20 种模型架构**：LLaMA, Qwen3, Mistral, Gemma, Phi, Command-R, StarCoder, Falcon, DeepSeek-V2/V3, DBRX, Grok-1, Mamba-2, Jamba, OLMo2, Yi, Granite, LLaVA, MiniCPM, InternLM3, Mixtral
3. **COOLJAPAN 基础层**：SciRS2（张量）+ OxiBLAS（GEMM）+ OxiFFT（RoPE）
4. **跨平台**：x86-64, ARM64, WASM, RISC-V
5. **OpenAI 兼容 HTTP API**
6. **wgpu GPU 后端**（可选）
7. **2,461 测试通过**

**架构启发**：
- Crate 拆分策略（gguf / quant / arch / runtime / server）非常清晰
- trait-based 架构插件扩展模式适用于支持多模型架构
- COOLJAPAN 基础层（SciRS2/OxiBLAS/OxiFFT）是纯 Rust 数值计算生态的代表

### 2.6 对比总结

| 项目 | Stars | 代码量 | GPU | 量化 | 模型格式 | 架构数 | 核心特色 |
|---|---|---|---|---|---|---|---|
| rabbit | 87 | ~15K | ❌ CPU | int4/8/2, FP8, MXFP4 | 原始 checkpoint | 3 (K3/Linear/GLM) | MoE 专家流式加载 |
| Kortex | ? | ? | ✅ | ? | ? | ? | 跨内存层次流式 |
| smedjan | 0 | ~45K | ✅ Metal/CUDA | ✅ | 自定义 .bin | 自训 | 全流程训练+推理 |
| RAI | 3 | ~20K | ❌ CPU | W4A8/W4A32 | .raimodel | LLaMA/Mistral | 手写 AVX2 + GPTQ |
| OxiLLaMa | 25 | ~165K | ✅ wgpu(可选) | 25 种 | GGUF v3 | 20 种 | 最完整 llama.cpp 替代 |
| **candle** | 20,798 | ~50K | ✅ CUDA/Metal | ✅ | Safetensors | 50+ | HuggingFace 官方 |
| **mistral.rs** | 7,544 | ~100K | ✅ CUDA/Metal | ✅ ISQ/UQFF | GGUF+SafeT | 30+ | 最快 Rust 推理，paged-attn |
| **burn** | 15,681 | ~80K | ✅ CUDA/Metal | ✅ | 多格式 | 自定义 | 训练+推理统一框架 |

---

## 3. 技术选型

### 3.1 方案对比

#### 方案 A：自研引擎（从零实现）

| 维度 | 评价 |
|---|---|
| 可控性 | ⭐⭐⭐⭐⭐ 完全自主 |
| 开发量 | 极大（预估 20K~50K 行） |
| 风险 | 高（量化内核、attention 实现需要深厚 ML 底层经验） |
| 模型支持 | 需逐个实现架构 |
| 适用场景 | 长期差异化竞争 |

**结论**：短期不现实。EchoMind 的核心竞争力在 RAG + 隐私安全，不在推理引擎底层。

#### 方案 B：基于 candle 二次开发

| 维度 | 评价 |
|---|---|
| 成熟度 | ⭐⭐⭐⭐⭐ HuggingFace 官方维护，20K+ stars |
| 后端 | CPU / CUDA / Metal / WASM |
| 模型支持 | 50+ 架构（LLaMA, Qwen, Mistral, Phi, Gemma, ...） |
| API | 底层 tensor API，需自行实现模型前向传播 |
| 量化 | 支持 int4/int8/fp16/bf16 |
| 依赖体积 | 中等（candle-core + candle-nn + candle-transformers） |

**结论**：可行但工作量大。candle 提供 tensor 原语但不含完整推理管线，需自行组装 tokenizer + model + KV cache + sampling。

#### 方案 C：集成 mistral.rs（推荐 ✅）

| 维度 | 评价 |
|---|---|
| 成熟度 | ⭐⭐⭐⭐ 活跃维护，7.5K+ stars |
| 后端 | CPU / CUDA / Metal |
| 模型支持 | 30+ 架构，GGUF + Safetensors |
| 量化 | ISQ（运行时量化）+ UQFF（预量化格式） |
| API | Rust SDK + OpenAI 兼容 HTTP server |
| 性能 | v0.8.2 benchmark 显示比 llama.cpp 快 1.5~2.3x |
| Paged Attention | ✅ 支持 |
| 依赖体积 | 较大（含 candle 全栈） |

**结论**：最佳选择。mistral.rs 提供完整的推理管线（模型加载 + KV cache + sampling + streaming），Rust SDK 可直接嵌入，也可通过 OpenAI 兼容 API 与现有 `OpenAIProvider` 复用。

#### 方案 D：集成 OxiLLaMa

| 维度 | 评价 |
|---|---|
| 成熟度 | ⭐⭐ 新项目（25 stars），2,461 测试 |
| 纯 Rust | ⭐⭐⭐⭐⭐ 零 C/C++/Fortran/FFI |
| 模型支持 | 20 架构，GGUF v3 |
| 量化 | 25 种格式 |
| 后端 | CPU + wgpu（可选） |
| API | OpenAI 兼容 HTTP server |
| 依赖 | SciRS2 + OxiBLAS + OxiFFT |

**结论**：纯度最高但成熟度不足。可作为长期备选方案观察。

#### 方案 E：嵌入 llama.cpp 绑定（rllama / llama-cpp-2）

| 维度 | 评价 |
|---|---|
| 成熟度 | ⭐⭐⭐⭐⭐ llama.cpp 122K+ stars |
| 纯 Rust | ❌ 包含 C++ FFI |
| 性能 | 业界标杆 |
| 模型支持 | 最全（所有 GGUF） |

**结论**：违反铁律九（Rust 生态优先）。不推荐。

### 3.2 推荐方案：mistral.rs 集成 + 渐进式自研

```
Phase 1 (MVP)     → mistral.rs Rust SDK 嵌入，GGUF 模型，CPU 推理
Phase 2 (GPU)     → mistral.rs Metal/CUDA 后端启用
Phase 3 (优化)    → 自研量化内核 + 内存层次流式加载
Phase 4 (差异化)  → MoE 专家缓存 + 推测解码 + KV cache 持久化
```

**选择理由**：
1. **快速可用**：mistral.rs Rust SDK 可直接嵌入，`MistralRSBuilder` → `run()` → token stream
2. **架构兼容**：mistral.rs 的 streaming 输出可适配为 `BoxStream<Result<String>>`，无缝对接 `LLMProvider` trait
3. **性能领先**：v0.8.2 benchmark 证明比 llama.cpp 快 1.5~2.3x
4. **渐进可控**：先集成后自研，降低初始风险，长期可替换底层
5. **社区活跃**：EricLBuehler 持续维护，Discord 活跃，文档完善

---

## 4. 架构设计

### 4.1 六边形架构集成

```
crates/models  →  crates/core  →  crates/infra  →  crates/tauri-app
  (契约层)         (端口+逻辑)     (适配器)         (装配层)
                        ↑                  ↑
                   LLMProvider       LocalLlmEngine
                   (trait 不变)      (新增适配器)
```

**关键设计**：`LLMProvider` trait 完全不变，新增 `LocalLlmEngine` 作为 infra 层适配器，实现 `LLMProvider` trait。上层 `ChatEngine` 无感知。

### 4.2 新增 Crate 结构

```
crates/infra/src/
├── local_llm.rs          ← LocalLlmEngine 适配器（实现 LLMProvider trait）
├── local_llm_tests.rs    ← 单元测试
├── model_manager.rs      ← 模型文件管理（下载/删除/列表）
├── model_manager_tests.rs
└── lib.rs                ← 注册新模块
```

### 4.3 LocalLlmEngine 适配器设计

```rust
//! 本地 LLM 推理引擎适配器（REQ-LLM-003）：基于 mistral.rs 的纯 Rust 本地推理。
//!
//! 实现 `LLMProvider` trait，使本地推理与远程 API 推理对上层完全透明。
//! 用户可在设置中选择「本地模型」或「远程 API」，ChatEngine 无需修改。

use anyhow::Context;
use echomind_core::LLMProvider;
use echomind_models::ChatMessage;
use futures::StreamExt;
use futures::stream::BoxStream;

/// 本地 LLM 推理引擎（基于 mistral.rs）。
///
/// 首次使用时加载模型（可能耗时数秒~数十秒），后续推理直接使用已加载模型。
/// 模型加载在 `tokio::task::spawn_blocking` 中执行，不阻塞 async runtime。
pub struct LocalLlmEngine {
    /// 模型文件路径（GGUF 格式）
    model_path: std::path::PathBuf,
    /// mistral.rs 运行时实例（懒加载）
    runner: tokio::sync::OnceCell<mistralrs::MistralRS>,
    /// 量化格式
    quantization: Quantization,
}

/// 支持的量化格式。
#[derive(Debug, Clone, Copy)]
pub enum Quantization {
    /// 4-bit K-quants（推荐，质量/速度最佳平衡）
    Q4K_M,
    /// 5-bit K-quants（质量更高，速度略慢）
    Q5K_M,
    /// 8-bit（最高质量，内存占用最大）
    Q8_0,
    /// 自动检测（从 GGUF 文件头读取）
    Auto,
}

impl LocalLlmEngine {
    /// 创建本地推理引擎。
    ///
    /// # 参数
    /// - `model_path`: GGUF 模型文件路径
    /// - `quantization`: 量化格式（`Auto` 时从文件头自动检测）
    pub fn new(
        model_path: std::path::PathBuf,
        quantization: Quantization,
    ) -> anyhow::Result<Self> {
        if !model_path.exists() {
            anyhow::bail!("模型文件不存在: {}", model_path.display());
        }
        Ok(Self {
            model_path,
            runner: tokio::sync::OnceCell::new(),
            quantization,
        })
    }

    /// 懒加载模型（首次调用时加载，后续直接返回缓存实例）。
    ///
    /// 模型加载是 CPU 密集操作，在 `spawn_blocking` 中执行。
    /// 大模型（7B Q4）加载约 3~5 秒，70B Q4 约 30~60 秒。
    async fn runner(&self) -> anyhow::Result<&mistralrs::MistralRS> {
        self.runner
            .get_or_try_init(|| async {
                let path = self.model_path.clone();
                let quant = self.quantization;
                tokio::task::spawn_blocking(move || {
                    // mistral.rs SDK 初始化
                    // TODO(req-llm-003): 实现实际 mistral.rs 初始化
                    // let config = ModelConfig::new(...)
                    // MistralRSBuilder::new(config).build()
                    unimplemented!("Phase 1 实现")
                })
                .await
                .context("模型加载任务失败")?
            })
            .await
    }
}

impl LLMProvider for LocalLlmEngine {
    async fn chat_stream(
        &self,
        system_prompt: &str,
        history: &[ChatMessage],
        query: &str,
    ) -> anyhow::Result<BoxStream<'static, anyhow::Result<String>>> {
        let runner = self.runner().await?;
        // 构建 mistral.rs 请求
        // TODO(req-llm-003): 将 system_prompt + history + query 转为 mistral.rs 格式
        // 调用 runner.send_streaming() 获取 token 流
        // 适配为 BoxStream<Result<String>>
        unimplemented!("Phase 1 实现")
    }
}
```

### 4.4 模型管理器

```rust
//! 本地模型文件管理器（REQ-LLM-004）：下载、删除、列表本地 GGUF 模型。
//!
//! 模型文件存储在 `{data_dir}/models/llm/` 目录下。
//! 支持从 HuggingFace 下载 GGUF 模型（通过代理）。

use std::path::PathBuf;

/// 本地模型管理器。
pub struct ModelManager {
    /// 模型存储目录
    models_dir: PathBuf,
}

/// 模型元信息。
#[derive(Debug, Clone, serde::Serialize)]
pub struct ModelInfo {
    /// 文件名（如 `qwen2.5-7b-instruct-q4_k_m.gguf`）
    pub filename: String,
    /// 完整路径
    pub path: String,
    /// 文件大小（字节）
    pub size_bytes: u64,
    /// 推断的模型架构（从文件名解析）
    pub architecture: String,
    /// 推断的参数规模（如 `7B`）
    pub param_size: String,
    /// 量化格式（如 `Q4_K_M`）
    pub quantization: String,
}

impl ModelManager {
    /// 创建模型管理器，确保模型目录存在。
    pub fn new(data_dir: &std::path::Path) -> anyhow::Result<Self> {
        let models_dir = data_dir.join("models").join("llm");
        std::fs::create_dir_all(&models_dir)
            .context("创建模型目录失败")?;
        Ok(Self { models_dir })
    }

    /// 列出已下载的本地模型。
    pub fn list_models(&self) -> anyhow::Result<Vec<ModelInfo>> {
        // 扫描 models_dir 下所有 .gguf 文件
        // TODO(req-llm-004): 实现
        unimplemented!()
    }

    /// 删除指定模型文件。
    pub fn delete_model(&self, filename: &str) -> anyhow::Result<()> {
        // 安全校验：filename 不含路径分隔符（防目录穿越）
        // TODO(req-llm-004): 实现
        unimplemented!()
    }

    /// 获取模型文件路径（用于 LocalLlmEngine::new）。
    pub fn model_path(&self, filename: &str) -> anyhow::Result<PathBuf> {
        // 安全校验 + 路径拼接
        // TODO(req-llm-004): 实现
        unimplemented!()
    }

    /// 获取模型下载进度流（用于前端进度条）。
    pub async fn download_model(
        &self,
        url: &str,
        progress: impl Fn(u64, u64) + Send + 'static,
    ) -> anyhow::Result<()> {
        // 从 HuggingFace 下载 GGUF 文件
        // 支持断点续传
        // TODO(req-llm-004): 实现
        unimplemented!()
    }
}
```

### 4.5 LLM Provider 选择策略

`AppState` 新增 `llm_mode` 字段，决定使用本地引擎还是远程 API：

```rust
/// LLM 推理模式。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LlmMode {
    /// 远程 API（BYOK，现有行为）
    Remote,
    /// 本地推理（mistral.rs，新增）
    Local,
}
```

```rust
// state.rs 新增字段
pub struct AppState {
    // ... 现有字段 ...

    /// LLM 推理模式：Remote（BYOK API）或 Local（mistral.rs 本地推理）
    llm_mode: tokio::sync::RwLock<LlmMode>,
    /// 本地 LLM 引擎（懒加载，仅 llm_mode == Local 时初始化）
    local_llm: tokio::sync::OnceCell<LocalLlmEngine>,
    /// 模型管理器
    model_manager: ModelManager,
}
```

`chat_inner` 中根据 `llm_mode` 选择 provider：

```rust
let provider: Box<dyn LLMProvider> = match state.llm_mode.read().await.clone() {
    LlmMode::Remote => {
        // 现有逻辑：创建 OpenAIProvider
        Box::new(OpenAIProvider::new(api_key, base_url, model)?)
    }
    LlmMode::Local => {
        // 新增逻辑：获取已加载的 LocalLlmEngine
        let engine = state.local_llm().await?;
        Box::new(engine)
    }
};
```

### 4.6 前端集成

设置面板新增「本地模型」区域：

```
┌─────────────────────────────────────────────┐
│ LLM 设置                                     │
│                                             │
│ ○ 远程 API（BYOK）                           │
│   API Key: [________________________]       │
│   Base URL: [________________________]       │
│   Model: [________________________]         │
│                                             │
│ ● 本地模型                                    │
│   ┌─────────────────────────────────────┐   │
│   │ 已下载模型                           │   │
│   │ ✓ Qwen2.5-7B-Instruct Q4_K_M  4.1GB │   │
│   │ ✓ Llama-3.2-3B-Instruct  Q4_K_M  2.0GB│   │
│   │ [删除] [使用]                        │   │
│   └─────────────────────────────────────┘   │
│                                             │
│   [下载模型] → 弹出模型选择器                 │
│   ┌─────────────────────────────────────┐   │
│   │ 推荐模型                             │   │
│   │ Qwen2.5-7B-Instruct  Q4_K_M  4.1GB  │   │
│   │ Llama-3.2-3B-Instruct Q4_K_M  2.0GB │   │
│   │ Phi-3.5-mini-instruct Q4_K_M  2.2GB │   │
│   │ [下载]                               │   │
│   └─────────────────────────────────────┘   │
└─────────────────────────────────────────────┘
```

---

## 5. 实施路线图

### Phase 1：MVP — CPU 本地推理（预估 2~3 周）

**目标**：在 CPU 上运行 GGUF 模型，流式输出 token，与现有 RAG 管线集成。

| 任务 | 描述 | 预估 |
|---|---|---|
| T1-1 | 添加 mistral.rs 依赖到 `crates/infra/Cargo.toml` | 0.5d |
| T1-2 | 实现 `LocalLlmEngine` 适配器（`LLMProvider` trait） | 3d |
| T1-3 | 实现 `ModelManager`（列表/删除/路径管理） | 2d |
| T1-4 | 实现模型下载（HuggingFace GGUF，进度回调） | 2d |
| T1-5 | `AppState` 新增 `llm_mode` / `local_llm` / `model_manager` | 1d |
| T1-6 | `chat_inner` 根据 `llm_mode` 选择 provider | 1d |
| T1-7 | IPC 命令：`list_local_models` / `download_model` / `delete_model` / `set_llm_mode` | 2d |
| T1-8 | 前端设置面板：本地模型区域 | 2d |
| T1-9 | 测试：单元测试 + 集成测试（真实 GGUF 模型） | 2d |
| T1-10 | 文档同步（SRS / IPC 命令 / code-contracts / ADR） | 1d |

**推荐首批支持的模型**（CPU 可跑，质量尚可）：

| 模型 | 参数量 | Q4_K_M 大小 | 推荐场景 |
|---|---|---|---|
| Qwen2.5-3B-Instruct | 3B | ~2.0GB | 低配设备，中文优秀 |
| Llama-3.2-3B-Instruct | 3B | ~2.0GB | 英文场景 |
| Phi-3.5-mini-instruct | 3.8B | ~2.2GB | 推理能力强 |
| Qwen2.5-7B-Instruct | 7B | ~4.1GB | 平衡质量与速度 |
| Llama-3.1-8B-Instruct | 8B | ~4.7GB | 英文标杆 |

### Phase 2：GPU 加速（预估 1~2 周）

**目标**：启用 Metal（macOS）/ CUDA（NVIDIA）后端，提升推理速度 5~20x。

| 任务 | 描述 | 预估 |
|---|---|---|
| T2-1 | mistral.rs GPU 后端 feature flag 配置 | 1d |
| T2-2 | 运行时 GPU 检测与后端选择 | 1d |
| T2-3 | GPU 推理性能基准测试 | 1d |
| T2-4 | 前端 GPU 状态显示（可用/不可用） | 1d |
| T2-5 | 支持更大模型（14B/32B） | 2d |

**GPU 加速预期性能**（参考 mistral.rs v0.8.2 benchmark）：

| 模型 | CPU (tok/s) | Metal/CUDA (tok/s) | 加速比 |
|---|---|---|---|
| 3B Q4 | ~8 | ~40 | 5x |
| 7B Q4 | ~4 | ~25 | 6x |
| 14B Q4 | ~2 | ~12 | 6x |

### Phase 3：高级优化（预估 2~3 周）

**目标**：吸收参考项目的先进技术，提升大模型体验。

| 任务 | 描述 | 来源参考 | 预估 |
|---|---|---|---|
| T3-1 | KV cache 持久化（跨对话热恢复） | rabbit | 3d |
| T3-2 | 推测解码（draft model 加速） | RAI / smedjan | 3d |
| T3-3 | 模型预热（启动时后台加载） | — | 1d |
| T3-4 | 内存层次流式加载（大模型内存不足时） | Kortex / rabbit | 5d |
| T3-5 | 自定义量化内核（AVX2 寄存器内反量化） | RAI | 5d |

### Phase 4：差异化能力（预估 2~4 周）

**目标**：建立技术壁垒，形成与竞品的差异化优势。

| 任务 | 描述 | 来源参考 | 预估 |
|---|---|---|---|
| T4-1 | MoE 专家缓存（大型 MoE 模型支持） | rabbit | 5d |
| T4-2 | OpenAI 兼容本地 server（其他应用可调用） | OxiLLaMa / RAI | 3d |
| T4-3 | 模型微调（LoRA 本地微调） | smedjan | 10d |
| T4-4 | 自定义模型格式（零拷贝加载） | RAI .raimodel | 5d |

---

## 6. SRS 需求定义

### REQ-LLM-003 本地 LLM 推理引擎

- **优先级**：P1 ｜ **状态**：Planned ｜ **依赖**：REQ-LLM-001
- **描述**：集成 mistral.rs 纯 Rust LLM 推理引擎，支持加载本地 GGUF 量化模型，在 CPU/GPU 上执行推理，通过 `LLMProvider` trait 与现有 RAG 管线无缝集成。用户可在设置中切换「远程 API」和「本地模型」两种推理模式。
- **验收标准**：
  - AC-1：设置面板提供「本地模型」选项，可选择已下载的 GGUF 模型。
  - AC-2：选择本地模型后，`chat` 命令使用 `LocalLlmEngine` 进行推理，零网络请求。
  - AC-3：本地推理支持 token 级流式输出，与远程 API 行为一致（`chat_token` 事件）。
  - AC-4：首次加载模型时显示加载进度，不阻塞 UI。
  - AC-5：支持 Q4_K_M / Q5_K_M / Q8_0 三种量化格式。
  - AC-6：macOS 上支持 Metal GPU 加速（可选）。
  - AC-7：模型加载失败时返回可读错误信息，不 Panic。
- **契约**：`LLMProvider` trait 不变；新增 `LocalLlmEngine` 适配器。
- **关联测试**：TC-LLM-002（加载+推理）、TC-LLM-003（流式输出）、TC-LLM-004（错误处理）

### REQ-LLM-004 本地模型管理

- **优先级**：P1 ｜ **状态**：Planned ｜ **依赖**：REQ-LLM-003
- **描述**：提供本地 GGUF 模型文件的管理功能：下载（从 HuggingFace）、列表、删除。模型文件存储在 `{data_dir}/models/llm/` 目录下。下载时显示进度条，支持取消。
- **验收标准**：
  - AC-1：`list_local_models` 命令返回已下载模型列表（文件名、大小、推断架构）。
  - AC-2：`download_model` 命令从指定 URL 下载 GGUF 文件，通过 `model_download_progress` 事件推送进度。
  - AC-3：`delete_model` 命令删除指定模型文件，删除前确认文件在模型目录内（防目录穿越）。
  - AC-4：`set_llm_mode` 命令切换 Remote/Local 模式，持久化到 settings 表。
  - AC-5：下载可取消（`abort_download` 命令），已下载部分保留（支持断点续传）。
  - AC-6：推荐模型列表（Qwen2.5 / Llama / Phi 系列），一键下载。
- **契约**：`list_local_models() -> Vec<ModelInfo>` / `download_model(url: String, filename: String) -> ()` / `delete_model(filename: String) -> ()` / `set_llm_mode(mode: String) -> ()`
- **关联测试**：TC-LLM-005（模型下载）、TC-LLM-006（模型删除）、TC-LLM-007（模式切换）

---

## 7. 技术风险与缓解

| 风险 | 影响 | 概率 | 缓解策略 |
|---|---|---|---|
| mistral.rs 编译时间长 | 首次构建 10+ 分钟 | 高 | Pro feature 门控，Free 版不含本地推理依赖 |
| mistral.rs 依赖体积大 | 安装包膨胀 50~100MB | 高 | 可选 feature，用户按需启用 |
| CPU 推理速度慢 | 7B 模型仅 4 tok/s，体验差 | 中 | Phase 2 GPU 加速；推荐 3B 模型作为默认 |
| mistral.rs API 变更 | 升级时适配工作 | 中 | 锁定版本，关注 release notes |
| macOS Metal 兼容性 | Apple Silicon 差异 | 低 | mistral.rs 已有 Metal 支持，社区验证充分 |
| 内存不足 | 大模型 OOM | 中 | 内存层次流式加载（Phase 3）；模型大小检测 + 警告 |

---

## 8. 依赖变更

### `crates/infra/Cargo.toml` 新增

```toml
[features]
default = []
pro = [
    "dep:pdfium-render", "dep:ocrs", "dep:rten", "dep:image", "dep:hnsw_rs",
    "dep:mistralrs",  # 新增：本地 LLM 推理
]

# 本地 LLM 推理引擎（REQ-LLM-003/004）
mistralrs = { version = "0.8", optional = true }
```

### `crates/tauri-app/Cargo.toml` 转发

```toml
[features]
pro = ["echomind-infra/pro"]
```

### Settings 键

| 键 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `llm.mode` | String | `"remote"` | 推理模式：`remote` / `local` |
| `llm.local_model` | String | `""` | 当前选中的本地模型文件名 |
| `llm.local_quantization` | String | `"q4_k_m"` | 量化格式 |

---

## 9. IPC 命令

| 命令 | 参数 | 返回 | 说明 |
|---|---|---|---|
| `list_local_models` | — | `Vec<ModelInfo>` | 列出已下载的本地模型 |
| `download_model` | `url: String, filename: String` | `()` | 下载 GGUF 模型文件 |
| `delete_model` | `filename: String` | `()` | 删除本地模型文件 |
| `set_llm_mode` | `mode: String` | `()` | 设置推理模式（remote/local） |
| `get_llm_mode` | — | `String` | 获取当前推理模式 |
| `abort_download` | `filename: String` | `()` | 取消模型下载 |

### 新增 Tauri 事件

| 事件 | 载荷 | 说明 |
|---|---|---|
| `model_download_progress` | `{filename, downloaded, total, speed}` | 模型下载进度 |
| `model_load_progress` | `{model, status: "loading"/"ready"/"error"}` | 模型加载状态 |

---

## 10. ADR-011：本地 LLM 引擎技术选型

**日期**：2026-07-30 ｜ **状态**：Proposed

### 背景

EchoMind 当前使用 BYOK 模式，所有 LLM 推理通过外部 API。对于高隐私场景和离线使用，需要本地推理能力。

### 决策

采用 mistral.rs 作为本地 LLM 推理引擎，通过 Pro feature 门控。

### 理由

1. **性能领先**：mistral.rs v0.8.2 benchmark 比 llama.cpp 快 1.5~2.3x
2. **Rust 原生**：纯 Rust 实现，符合铁律九
3. **模型支持广**：30+ 架构，GGUF + Safetensors
4. **Rust SDK**：可直接嵌入，无需启动独立进程
5. **活跃维护**：EricLBuehler 持续开发，Discord 社区活跃
6. **Paged Attention**：支持高效 KV cache 管理
7. **渐进可控**：先集成后自研，Phase 3/4 可逐步替换底层

### 备选方案

- **candle**：底层框架，需自行实现完整推理管线，工作量大
- **OxiLLaMa**：纯度最高但成熟度不足（25 stars），可作为长期观察对象
- **自研**：短期不现实，核心竞争力不在推理引擎底层
- **llama.cpp 绑定**：违反铁律九（C++ FFI）

### 后果

- Pro 版安装包增大 ~50MB（mistral.rs + candle 依赖）
- 首次编译时间增加 ~5 分钟
- 需要维护 mistral.rs 版本升级适配
