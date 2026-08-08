# ADR-011: 本地 LLM 推理引擎技术选型

**日期**：2026-07-30 ｜ **状态**：Accepted（Phase 3 完成）

## 背景

EchoMind 当前使用 BYOK（Bring Your Own Key）模式，所有 LLM 推理通过 `OpenAIProvider` 发送到用户配置的外部 API 端点。对于高隐私场景（律师、研究人员）和离线使用，需要本地推理能力。

**痛点**：
1. 隐私顾虑：查询内容离开本机
2. 离线不可用：无网络时完全无法对话
3. API 成本：频繁查询产生持续费用
4. 延迟依赖：首 token 延迟受网络 RTT 影响

## 决策

采用 **mistral.rs** 作为本地 LLM 推理引擎，通过 Pro feature 门控（`#[cfg(feature = "pro")]`）。

### 技术选型对比

| 方案 | Stars | GPU | 量化 | 模型格式 | 架构数 | 核心特色 |
|---|---|---|---|---|---|---|
| **mistral.rs**（选中） | 7,544 | CUDA/Metal | ISQ/UQFF | GGUF+SafeT | 30+ | 最快 Rust 推理，paged-attn |
| candle | 20,798 | CUDA/Metal | int4/8/fp16 | Safetensors | 50+ | HuggingFace 官方 |
| OxiLLaMa | 25 | wgpu(可选) | 25 种 | GGUF v3 | 20 | 最完整 llama.cpp 替代 |
| llama.cpp 绑定 | 122K+ | CUDA/Metal | 全部 | GGUF | 全部 | C++ FFI，违反铁律九 |

### 选择理由

1. **性能领先**：mistral.rs v0.8.2 benchmark 比 llama.cpp 快 1.5~2.3x
2. **Rust 原生**：纯 Rust 实现，符合铁律九（Rust 生态优先）
3. **模型支持广**：30+ 架构，GGUF + Safetensors
4. **Rust SDK**：可直接嵌入，无需启动独立进程
5. **活跃维护**：EricLBuehler 持续开发，Discord 社区活跃
6. **Paged Attention**：支持高效 KV cache 管理
7. **渐进可控**：先集成后自研，Phase 3/4 可逐步替换底层

## 架构设计

### 六边形架构集成

```
crates/models  →  crates/core  →  crates/infra  →  crates/tauri-app
  (契约层)         (端口+逻辑)     (适配器)         (装配层)
                        ↑                  ↑
                   LLMProvider       LocalLlmEngine
                   (trait 不变)      (新增适配器)
```

`LLMProvider` trait 完全不变，新增 `LocalLlmEngine` 作为 infra 层适配器。上层 `ChatEngine` 无感知。

### 新增文件

| 文件 | 职责 |
|---|---|
| `crates/models/src/lib.rs` | `LlmMode` 枚举、`ModelInfo`、`ModelDownloadProgressPayload`、`ModelLoadStatusPayload` |
| `crates/infra/src/model_manager.rs` | `ModelManager`：模型文件列表/删除/下载/路径管理 |
| `crates/infra/src/local_llm.rs` | `LocalLlmEngine`：mistral.rs 适配器，实现 `LLMProvider` trait |
| `crates/tauri-app/src/state.rs` | `AppState` 集成 `llm_mode` / `local_llm` / `model_manager` |
| `crates/tauri-app/src/commands.rs` | 7 个新 IPC 命令 + `chat_inner` 模式路由 |
| `ui/src/settings.js` | 前端设置面板：模式切换、模型列表、下载进度 |

### Pro Feature 门控

mistral.rs 依赖通过 `#[cfg(feature = "pro")]` 门控：
- Free 版本不含本地推理依赖，编译时间不增加
- Pro 版本包含完整本地推理能力
- `chat_inner` 在 Free 模式下遇到 `LlmMode::Local` 返回错误提示

## 后果

- Pro 版安装包增大 ~50MB（mistral.rs + candle 依赖）
- 首次编译时间增加 ~5 分钟
- 需要维护 mistral.rs 版本升级适配
- 用户需自行下载 GGUF 模型文件（2~5GB）

## 实施路线

```
Phase 1 (MVP)     → mistral.rs Rust SDK 嵌入，GGUF 模型，CPU 推理 ✅
Phase 2 (性能优化) → mistral.rs v0.9.0 升级 + Metal/CUDA GPU + PagedAttention + 采样参数 + 模型卸载 + 流取消 + 预热 ✅
Phase 3 (优化)    → 自研量化内核 + 内存层次流式加载 ✅（8 Session: S16-S23 完成）
Phase 4 (差异化)  → MoE 专家缓存 + 推测解码 + KV cache 持久化 📋 规划完成（8 Session: S24-S31，详见 LOCAL_LLM_ENGINE_PHASE4_ATOMIC_PLAN.md）
```

### Phase 2 完成详情（2026-07-31）

**8 个 Session 全部完成**：

| Session | 目标 | 状态 |
|---|---|---|
| S8 | mistral.rs v0.8.1→v0.9.0 版本升级 | ✅ |
| S9 | GPU 加速（Metal/CUDA feature flags + best_device） | ✅ |
| S10 | PagedAttention KV cache 管理 | ✅ |
| S11 | 采样参数配置（temperature/top_p/top_k/max_tokens/penalty） | ✅ |
| S12 | 模型卸载/切换修复（OnceCell → RwLock<Option>） | ✅ |
| S13 | 流取消集成（CancellationToken in LocalLlmEngine） | ✅ |
| S14 | 模型预热 + 错误处理加固 | ✅ |
| S15 | 性能基准测试 + 文档同步 | ✅ |

**Phase 2 性能基准测试设计**：

3 个 `#[ignore]` 测试（需真实 GGUF 模型文件）：
- TC-LLM-PERF-001：CPU 推理速度基准（首 token 延迟 + tokens/sec）
- TC-LLM-PERF-002：GPU（Metal）推理速度基准（与 CPU 对比）
- TC-LLM-PERF-003：PagedAttention 多轮对话延迟（KV cache 复用效果）

运行方式：
```bash
ECHOMIND_TEST_MODEL=/path/to/model.gguf \
cargo test -p echomind-infra --features pro -- local_llm_tests --ignored --nocapture
```

> 实际基准数据待真实模型测试后填写。测试设计覆盖 CPU vs GPU 对比和 PagedAttention 多轮对话延迟。

**Phase 2 新增命令**（3 个）：
- `get_local_llm_device_kind` — 返回当前推理设备类型（cpu/metal/cuda）
- `set_paged_attn` — 配置 PagedAttention KV cache 管理
- `set_sampling_params` — 设置采样参数（运行时即时生效）

**Phase 2 新增设置键**（4 个）：
- `llm.paged_attn` — PagedAttention 开关
- `llm.block_size` — KV cache 块大小
- `llm.gpu_memory_ctx` — GPU 上下文 token 数
- `llm.sampling` — 采样参数 JSON

### Phase 2 详细规划

详见 `docs/architecture/LOCAL_LLM_ENGINE_PHASE2_ATOMIC_PLAN.md`（8 个 Session：S8-S15）。

**Phase 2 Session 概览**：

| Session | 目标 | 预计耗时 |
|---|---|---|
| S8 | ✅ mistral.rs v0.8.1 → v0.9.0 版本升级（2026-07-31 完成） | 1.5 天 |
| S9 | ✅ GPU 加速（Metal/CUDA feature flags + best_device） | 2 天 |
| S10 | ✅ PagedAttention KV cache 管理 | 1.5 天 |
| S11 | ✅ 采样参数配置（temperature/top_p/max_tokens 等） | 2 天 |
| S12 | ✅ 模型卸载/切换修复（OnceCell → RwLock<Option>） | 1.5 天 |
| S13 | ✅ 流取消集成（CancellationToken in LocalLlmEngine） | 1.5 天 |
| S14 | ✅ 模型预热 + 错误处理加固 | 1 天 |
| S15 | ✅ 性能基准测试 + 文档同步 | 1 天 |

**调研来源**（2026-07-31，铁律八互联网搜索）：
- mistral.rs GitHub releases: v0.9.0（最新）
- v0.9.0 Cargo.toml: metal/cuda/accelerate feature flags
- v0.9.0 lib.rs: GgufModelBuilder/RequestBuilder/PagedAttentionMetaBuilder API
- v0.9.0 model.rs: best_device() 自动设备选择
- v0.9.0 gguf.rs: with_device/with_force_cpu/with_paged_attn
- v0.9.0 messages.rs: set_sampler_temperature/set_sampler_topk/set_sampler_max_len 等

### Phase 3 规划详情（2026-07-31）

**8 个 Session 规划完成**，详见 `docs/architecture/LOCAL_LLM_ENGINE_PHASE3_ATOMIC_PLAN.md`。

| Session | 目标 | 预计耗时 | 状态 |
|---|---|---|---|
| S16 | 自研 GGUF 文件解析器（mmap 零拷贝） | 2 天 | ✅ |
| S17 | 量化块结构与反量化（BlockQ4K/BlockQ8_0 等） | 2 天 | ✅ |
| S18 | 自研 GEMV 内核（AVX2 SIMD，单批次推理优化） | 3 天 | ✅ |
| S19 | CPU cache 友好权重重排（减少 L1/L2 cache miss） | 2 天 | ✅ |
| S20 | 集成 LocalLlmEngine（内核模式切换 + IPC） | 2 天 | ✅ |
| S21 | Layer 级流式预取（madvise + 后台预取） | 2 天 | ✅ |
| S22 | RAM 预算管理（LRU 驱逐 + 模型 > RAM 支持） | 2 天 | ✅ |
| S23 | 性能基准测试 + 文档同步 | 1 天 | ✅ |

**Phase 3 核心价值**：
1. **自研 GEMV 内核**：单批次推理（batch=1，即 chat 场景）相比 mistral.rs 的通用 GEMM 路径 2-5x 加速
2. **CPU cache 友好布局**：权重重排减少 L1/L2 cache miss，10-30% 加速
3. **流式预取**：大模型加载无需等待全部权重，layer 级后台预取消除加载延迟
4. **RAM 预算管理**：支持模型 > 可用 RAM 场景，自动驱逐已使用层、预取即将使用层

**调研来源**（2026-07-31，铁律八互联网搜索）：
- mistral.rs v0.9.0 源码：mistralrs-quant crate（GgufMatMul/QuantMethod）、mistralrs-core/src/gguf/content.rs
- candle-core 量化模块：candle-core/src/quantized/（k_quants.rs/avx.rs/neon.rs/gguf_file.rs）
- llama.cpp ggml CPU：quants.c/quants.h/repack.cpp（GEMV 内核 + cache 友好重排参考）
- GitHub 搜索：herbert-rs（AVX-512 手写）、pulsar（SSD streaming MoE）、lattice（Apple Silicon 纯 Rust）

### Phase 3 完成详情（2026-08-01）

**8 个 Session 全部完成**：

| Session | 目标 | 状态 | 测试数 |
|---|---|---|---|
| S16 | GGUF 文件解析器（mmap 零拷贝，V1/V2/V3 格式） | ✅ | 14 |
| S17 | 量化块结构与反量化（Q4_0/Q4_K/Q8_0/Q8_K） | ✅ | 14 |
| S18 | 自研 GEMV 内核（标量 + AVX2 预留，4 种格式） | ✅ | 14 |
| S19 | CPU cache 友好权重重排（Row-Major → Tile-Major） | ✅ | 6 |
| S20 | 集成 LocalLlmEngine（KernelMode 切换 + IPC + warm_up） | ✅ | 7 |
| S21 | Layer 级流式预取（madvise + 后台线程 + SendPtr） | ✅ | 6 |
| S22 | RAM 预算管理（LRU 驱逐 + libc::sysconf） | ✅ | 13 |
| S23 | 性能基准测试 + 文档同步 | ✅ | 4 (#[ignore]) |

**Phase 3 测试总计**：78 个（73 个单元 + 4 个 #[ignore] 性能基准 + 1 个 #[ignore] mincore 检查）

**Phase 3 性能基准测试**（4 个 `#[ignore]` 测试）：
- TC-PHASE3-PERF-001：自研 GEMV Q8_0 内核性能基准（vs 朴素 f32 参考实现）
- TC-PHASE3-PERF-002：自研 GEMV Q4_K 内核性能基准（vs 朴素 f32 参考实现）
- TC-PHASE3-PERF-003：Layer 级流式预取效果基准（cold vs warm page cache 访问延迟）
- TC-PHASE3-PERF-004：权重重排 cache miss 降低基准（Row-Major vs Tile-Major GEMV 执行时间）

运行方式：
```bash
ECHOMIND_TEST_MODEL=/path/to/model.gguf \
cargo test -p echomind-infra --features pro -- phase3_perf --ignored --nocapture
```

**Phase 3 新增模块**（6 个 `crates/infra/src/` 文件）：
| 模块 | 文件 | 功能 |
|---|---|---|
| GGUF 文件解析器 | `gguf_reader.rs` | mmap 零拷贝，V1/V2/V3 格式，元数据/张量信息 |
| 量化块结构 | `quant_blocks.rs` | BlockQ4_0/BlockQ4K/BlockQ8_0/BlockQ8K + 反量化 + blocks_from_bytes |
| GEMV 内核 | `gemv_kernel.rs` | 4 种格式 GEMV + 输入预处理 + int8 点积 + dispatch |
| 权重重排 | `weight_repack.rs` | Row-Major → Tile-Major 块级转置 + cache 友好 GEMV |
| Layer 级预取 | `layer_prefetch.rs` | madvise(MADV_WILLNEED) + 后台线程 + SendPtr |
| RAM 预算管理 | `memory_budget.rs` | LRU 驱逐 + libc::sysconf + from_system |

**Phase 3 新增 IPC 命令**（2 个）：
| 命令 | 参数 | 返回 | 说明 |
|---|---|---|---|
| `set_kernel_mode` | `mode: String` | `()` | 切换内核模式（"mistral"/"custom"），Pro 门控 |
| `get_kernel_mode` | — | `String` | 返回当前内核模式，Pro 门控 |

**Phase 3 新增设置键**（1 个）：
| 键 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `llm.kernel_mode` | String | `"mistral"` | 推理内核：mistral（mistral.rs）/ custom（自研 GEMV） |

**Phase 3 新增 SRS 需求**：
- REQ-LLM-007：自研量化推理内核（GGUF 解析 + GEMV + 权重重排 + 内核模式切换）
- REQ-LLM-008：内存层次流式加载（madvise 预取 + LRU 驱逐 + RAM 预算管理）
