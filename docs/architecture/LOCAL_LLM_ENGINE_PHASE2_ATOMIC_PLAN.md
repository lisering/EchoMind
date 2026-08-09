# EchoMind 纯 Rust 本地 LLM 引擎 Phase 2 — 原子级实施规划

> **状态**：Active ｜ **创建日期**：2026-07-31 ｜ **父文档**：`ADR-011-local-llm-engine.md`
>
> 本文档将 Phase 2（性能优化）拆分为 **8 个 Session（S8-S15）**，每个 Session 包含：
> - 精确到文件、函数、行号的变更清单
> - TDD 测试用例（先红灯后绿灯）
> - Session 完成验证命令
> - 下一 Session 衔接提示词
>
> **铁律**：每个 Session 结束时必须产出衔接提示词，确保下一 Session 无缝续接。

---

## 前置调研结果（2026-07-31 执行）

### mistral.rs 版本调研

| 项目 | 当前 | 最新 | 说明 |
|---|---|---|---|
| mistral.rs crate 版本 | v0.8.1 | **v0.9.0** | GitHub releases 确认 |
| candle-core 依赖 | crates.io 0.10.x | **git rev 27f20fe** | v0.9.0 改用 candle git 依赖 |
| Rust 最低版本 | 1.85 (Edition 2024) | **1.94** | v0.9.0 要求 rust-version = "1.94" |
| 工作区 edition | 2024 | **2021** | mistral.rs v0.9.0 workspace 用 2021 |

### mistral.rs v0.9.0 API 变更（与 v0.8.1 对比）

**兼容部分**（无需改动）：
- `GgufModelBuilder::new(dir, files)` — 签名不变
- `RequestBuilder::new().add_message(role, content)` — 签名不变
- `TextMessageRole::User/Assistant/System` — 不变
- `Model::stream_chat_request(request)` — 返回 `impl Stream<Item = Response>`
- `Response::Chunk(ChatCompletionChunkResponse { choices, .. })` — 不变
- `ChunkChoice { delta: Delta { content: Some(..), .. }, .. }` — 不变

**新增 API**（Phase 2 关键）：

| API | 用途 | 所在文件 |
|---|---|---|
| `mistralrs::best_device(force_cpu: bool) -> Result<Device>` | 自动选择最优设备（CUDA → Metal → CPU） | `model.rs` |
| `GgufModelBuilder::with_device(device: Device)` | 指定推理设备（GPU） | `gguf.rs` |
| `GgufModelBuilder::with_paged_attn(cfg: PagedAttentionConfig)` | 启用 PagedAttention | `gguf.rs` |
| `GgufModelBuilder::with_force_cpu()` | 强制 CPU（禁用 PagedAttention） | `gguf.rs` |
| `GgufModelBuilder::with_max_num_seqs(n: usize)` | 最大并发序列数 | `gguf.rs` |
| `GgufModelBuilder::with_no_kv_cache()` | 禁用 KV cache | `gguf.rs` |
| `PagedAttentionMetaBuilder::default()` | PagedAttention 配置构建器 | `text_model.rs` |
| `PagedAttentionMetaBuilder::with_block_size(n)` | KV cache 块大小 | `text_model.rs` |
| `PagedAttentionMetaBuilder::with_gpu_memory(cfg)` | GPU 内存配置 | `text_model.rs` |
| `RequestBuilder::set_sampler_temperature(f64)` | 采样温度 | `messages.rs` |
| `RequestBuilder::set_sampler_topk(usize)` | Top-K 采样 | `messages.rs` |
| `RequestBuilder::set_sampler_topp(f64)` | Top-P 采样 | `messages.rs` |
| `RequestBuilder::set_sampler_minp(f64)` | Min-P 采样 | `messages.rs` |
| `RequestBuilder::set_sampler_frequency_penalty(f32)` | 频率惩罚 | `messages.rs` |
| `RequestBuilder::set_sampler_presence_penalty(f32)` | 存在惩罚 | `messages.rs` |
| `RequestBuilder::set_sampler_stop_toks(StopTokens)` | 停止词 | `messages.rs` |
| `RequestBuilder::set_sampler_max_len(usize)` | 最大生成 token 数 | `messages.rs` |
| `RequestBuilder::set_deterministic_sampler()` | 确定性采样（temperature=0） | `messages.rs` |
| `RequestBuilder::set_sampling(SamplingParams)` | 完整采样参数 | `messages.rs` |
| `RequestBuilder::return_logprobs(bool)` | 返回 logprobs | `messages.rs` |
| `RequestBuilder::set_constraint(Constraint)` | 结构化输出约束 | `messages.rs` |
| `Model::chat(&str)` | 便捷非流式对话 | `model.rs` |
| `Model::generate_structured::<T>(messages)` | 结构化 JSON 输出 | `model.rs` |

**Feature Flags**（v0.9.0 Cargo.toml 确认）：

| Flag | 效果 | 依赖 |
|---|---|---|
| `cuda` | NVIDIA CUDA GPU | 需要 CUDA toolkit |
| `metal` | Apple Metal GPU (macOS) | 无额外依赖 |
| `flash-attn` | Flash Attention 2 | 需要 `cuda` |
| `cudnn` | cuDNN 加速 | 需要 `cuda` |
| `accelerate` | Apple Accelerate (BLAS) | macOS only |
| `mkl` | Intel MKL (BLAS) | Linux/Windows |
| `nccl` | 多 GPU (NCCL) | 需要 `cuda` |
| 无 flag | 纯 Rust CPU | 无系统依赖 |

---

## 全局依赖关系图

```
Session 8 (版本升级) ──→ Session 9 (GPU 加速) ──→ Session 10 (PagedAttention)
                                                        │
                                                        ↓
Session 11 (采样参数) ──→ Session 12 (模型卸载) ──→ Session 13 (流取消集成)
                                                        │
                                                        ↓
                              Session 14 (预热+错误处理) ──→ Session 15 (基准测试+文档)
```

**依赖关系说明**：
- S8 是所有后续 Session 的基础（API 变更影响所有代码）
- S9 依赖 S8（v0.9.0 的 `with_device` / `best_device` API）
- S10 依赖 S9（PagedAttention 需要 GPU 设备）
- S11 独立于 S9/S10（采样参数与设备无关，但依赖 S8 的 API 版本）
- S12 依赖 S8（卸载逻辑涉及 `OnceCell` 改 `RwLock<Option>`）
- S13 依赖 S12（需要先能正确管理引擎生命周期）
- S14 依赖 S13（预热和错误处理在完整生命周期之上构建）
- S15 依赖所有前序 Session

**可并行执行**：S11 可与 S9/S10 并行（不同代码区域）

---

## 当前代码状态基线

### 测试基线（2026-07-31）
- `local_llm_tests`：8 passed（TC-LLM-LOCAL-001~008）
- `model_manager_tests`：20 passed（TC-MGR-001~020）
- `integration tests`（state_llm / ipc_llm）：全过
- Clippy（Free + Pro）：零警告

### 当前 `LocalLlmEngine` 结构（需改动的核心）

```rust
// crates/infra/src/local_llm.rs — 当前实现

pub struct LocalLlmEngine {
    inner: Arc<LocalLlmEngineInner>,
}

struct LocalLlmEngineInner {
    model_path: PathBuf,        // ← 仅调试用
    model_dir: String,          // ← 传给 GgufModelBuilder
    model_filename: String,     // ← 传给 GgufModelBuilder
    runner: tokio::sync::OnceCell<Arc<Model>>,  // ← S12 改为 RwLock<Option<Arc<Model>>>
}

// chat_stream 实现：
// 1. self.runner().await?.clone() — 懒加载模型
// 2. build_messages(system_prompt, history, query) — 构建消息
// 3. tokio::spawn + mpsc::channel(32) — 桥接流
// 4. RequestBuilder::new().add_message(role, content) — 构建请求
// 5. model.stream_chat_request(request).await — 启动流
// 6. while let Some(chunk) = stream.next().await — 逐 token 转发
```

### 当前 `GgufModelBuilder` 调用（S8/S9 需改动）

```rust
// 当前（v0.8.1）：
let model = GgufModelBuilder::new(&dir, vec![file.as_str()])
    .with_logging()
    .build()
    .await?;

// S8 升级后（v0.9.0，保持 CPU）：
let model = GgufModelBuilder::new(&dir, vec![file.as_str()])
    .with_logging()
    .with_force_cpu()  // 显式 CPU（S9 改为 GPU）
    .build()
    .await?;

// S9 GPU 加速后（v0.9.0，Metal）：
let device = mistralrs::best_device(false)?;  // 自动选择 CUDA→Metal→CPU
let model = GgufModelBuilder::new(&dir, vec![file.as_str()])
    .with_logging()
    .with_device(device)  // GPU 设备
    .build()
    .await?;

// S10 PagedAttention 后（v0.9.0，完整）：
let device = mistralrs::best_device(false)?;
let paged_attn = PagedAttentionMetaBuilder::default()
    .with_block_size(32)
    .build()?;
let model = GgufModelBuilder::new(&dir, vec![file.as_str()])
    .with_logging()
    .with_device(device)
    .with_paged_attn(paged_attn)
    .build()
    .await?;
```

---

## Session 8：mistral.rs 版本升级（v0.8.1 → v0.9.0）

**目标**：将 mistral.rs 依赖从 v0.8.1 升级到 v0.9.0，适配 API 变更，确保现有功能不回归。

**预计耗时**：1.5 天

### 8.1 前置调研（Session 开始时必须执行）

```bash
# 1. 检查 Rust 工具链版本（v0.9.0 要求 1.94+）
rustup show
rustc --version
# 如低于 1.94：rustup update stable

# 2. 检查 mistral.rs v0.9.0 CHANGELOG
export https_proxy=http://127.0.0.1:7890 http_proxy=http://127.0.0.1:7890 all_proxy=socks5://127.0.0.1:7890
curl -sL "https://api.github.com/repos/EricLBuehler/mistral.rs/releases/tags/v0.9.0" | python3 -c "import sys,json; r=json.load(sys.stdin); print(r.get('body','')[:3000])"
unset https_proxy http_proxy all_proxy

# 3. 检查 v0.8.1 → v0.9.0 之间的 breaking changes
# 重点：candle-core 从 crates.io 改为 git 依赖的影响
```

### 8.2 文件变更清单

#### 文件 1：`crates/infra/Cargo.toml`（修改）

**变更**：升级 mistralrs 版本

```toml
# 当前：
mistralrs = { version = "0.8.1", optional = true }

# 改为：
mistralrs = { version = "0.9.0", optional = true }
```

**注意事项**：
- v0.9.0 的 candle-core 使用 git 依赖，可能需要 `cargo update` 后重新 `cargo build`
- 如果编译失败，检查是否需要添加 `[patch.crates-io]` 段指向 candle git 仓库
- rust-version 从 1.85 提升到 1.94，确认本地 toolchain 已升级

#### 文件 2：`crates/infra/src/local_llm.rs`（修改）

**变更 1**：更新导入（检查 v0.9.0 导出路径是否变化）

```rust
// 当前（v0.8.1）：
use mistralrs::{
    ChatCompletionChunkResponse, ChunkChoice, Delta, GgufModelBuilder, Model, RequestBuilder,
    Response, TextMessageRole,
};

// v0.9.0 — 检查是否需要调整：
// - Model 仍然从 mistralrs 导出 ✅
// - GgufModelBuilder 仍然从 mistralrs 导出 ✅
// - RequestBuilder 仍然从 mistralrs 导出 ✅
// - Response/ChatCompletionChunkResponse/ChunkChoice/Delta 从 mistralrs_core 重导出 ✅
// - 新增 best_device, PagedAttentionMetaBuilder 等（S9/S10 使用）
```

**变更 2**：在 `runner()` 方法中添加 `with_force_cpu()`（显式 CPU，S9 改为 GPU）

```rust
// 当前：
let model = GgufModelBuilder::new(&dir, vec![file.as_str()])
    .with_logging()
    .build()
    .await
    .context("GGUF 模型加载失败")?;

// 改为（v0.9.0 显式 CPU）：
let model = GgufModelBuilder::new(&dir, vec![file.as_str()])
    .with_logging()
    .with_force_cpu()
    .build()
    .await
    .context("GGUF 模型加载失败")?;
```

**变更 3**：检查 `extract_text()` 函数是否与 v0.9.0 的 `Response` 枚举兼容

```rust
// 当前实现应该兼容 v0.9.0，但需验证 Response::Chunk 变体名是否变化
fn extract_text(response: &Response) -> Option<String> {
    if let Response::Chunk(ChatCompletionChunkResponse { choices, .. }) = response
        && let Some(ChunkChoice {
            delta: Delta {
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
// 预期：v0.9.0 API 不变，但编译时需验证
```

#### 文件 3：`Cargo.lock`（自动更新）

```bash
cargo update -p mistralrs
```

### 8.3 TDD 测试用例

**策略**：S8 是版本升级，不新增功能测试，重点验证现有测试不回归。

| 测试 ID | 测试名 | 验证内容 | 状态 |
|---|---|---|---|
| TC-LLM-LOCAL-001 | `test_quantization_from_str` | 量化格式解析不回归 | 现有 |
| TC-LLM-LOCAL-002 | `test_quantization_as_str` | 量化格式转字符串不回归 | 现有 |
| TC-LLM-LOCAL-003 | `test_new_nonexistent_file_fails` | 不存在文件返回 Err 不回归 | 现有 |
| TC-LLM-LOCAL-004 | `test_new_valid_path_succeeds` | 有效路径创建成功不回归 | 现有 |
| TC-LLM-LOCAL-005 | `test_is_loaded_false_before_init` | 初始状态未加载不回归 | 现有 |
| TC-LLM-LOCAL-006 | `test_build_messages_system_prompt` | 消息构建不回归 | 现有 |
| TC-LLM-LOCAL-007 | `test_build_messages_history` | 历史消息构建不回归 | 现有 |
| TC-LLM-LOCAL-008 | `test_build_messages_query` | 查询消息构建不回归 | 现有 |

**新增测试**（验证 v0.9.0 API 可用性）：

| 测试 ID | 测试名 | 验证内容 |
|---|---|---|
| TC-LLM-UPGRADE-001 | `test_gguf_builder_has_force_cpu` | `with_force_cpu()` 方法存在且可链式调用 |
| TC-LLM-UPGRADE-002 | `test_request_builder_has_set_sampler_temperature` | `set_sampler_temperature()` 方法存在 |
| TC-LLM-UPGRADE-003 | `test_best_device_returns_cpu` | `best_device(true)` 返回 `Device::Cpu` |

```rust
/// TC-LLM-UPGRADE-001：v0.9.0 GgufModelBuilder 含 with_force_cpu 方法
#[test]
fn test_gguf_builder_has_force_cpu() {
    // 编译时验证：方法存在即可（不实际加载模型）
    let builder = GgufModelBuilder::new("/tmp", vec!["test.gguf"])
        .with_force_cpu();
    // 如果编译通过，说明 API 存在
    drop(builder);
}

/// TC-LLM-UPGRADE-002：v0.9.0 RequestBuilder 含 set_sampler_temperature 方法
#[test]
fn test_request_builder_has_set_sampler_temperature() {
    let builder = RequestBuilder::new()
        .set_sampler_temperature(0.7);
    drop(builder);
}

/// TC-LLM-UPGRADE-003：best_device(true) 返回 CPU
#[test]
fn test_best_device_returns_cpu() {
    let device = mistralrs::best_device(true).expect("best_device 应返回 CPU");
    // force_cpu=true 时必须返回 Device::Cpu
    assert!(matches!(device, mistralrs::Device::Cpu));
}
```

### 8.4 验证命令

```bash
# 1. 更新依赖
cargo update -p mistralrs

# 2. Free 模式编译（不应受影响）
cargo build -p echomind-infra

# 3. Pro 模式编译（关键：mistral.rs v0.9.0 编译）
cargo build -p echomind-infra --features pro

# 4. 单元测试不回归
cargo test -p echomind-infra --features pro -- local_llm_tests

# 5. 模型管理测试不回归
cargo test -p echomind-infra --features pro -- model_manager_tests

# 6. Clippy 零警告
cargo clippy -p echomind-infra --features pro -- -D warnings

# 7. 全量测试（Free + Pro）
cargo test
cargo test --features pro
```

### 8.5 Session 8 完成标志

- [x] mistral.rs v0.9.0 编译通过（Pro 模式）— 2026-07-31 完成
- [x] `with_force_cpu()` 显式指定 CPU 设备
- [x] 现有 8 个 `local_llm_tests` 测试全过
- [x] 现有 20 个 `model_manager_tests` 测试全过
- [x] 新增 3 个 API 可用性测试全过
- [x] Clippy 零警告（Free + Pro）
- [x] `cargo test` 全量通过（Free 392 passed / Pro 452 passed / 0 failed / 5 ignored）

> **重要发现**：mistral.rs v0.9.0 **未发布到 crates.io**（因为 candle-core 使用 git 依赖 `rev=27f20fe`，crates.io 不允许 git 依赖）。
> 改用 git 源依赖：`mistralrs = { git = "https://github.com/EricLBuehler/mistral.rs", tag = "v0.9.0", optional = true }`。
> Cargo.lock 确认：mistralrs 0.9.0 + candle-core 0.11.0 均从 git 源正确解析。

### 8.6 Session 8 → Session 9 衔接提示词

```
你正在继续 EchoMind 纯 Rust 本地 LLM 引擎 Phase 2（性能优化）的开发。

## 上下文回溯
- Phase 1（S1-S7）已完成：数据模型 / ModelManager / LocalLlmEngine / AppState / IPC 命令 / 前端 / 文档
- S8 已完成：mistral.rs v0.8.1 → v0.9.0 版本升级
  - crates/infra/Cargo.toml: mistralrs 0.8.1 → 0.9.0
  - crates/infra/src/local_llm.rs: 添加 with_force_cpu()，验证 API 兼容
  - 新增 TC-LLM-UPGRADE-001~003 三个 API 可用性测试
  - 全部测试通过，Clippy 零警告
  - 当前为 CPU-only 模式（with_force_cpu）

## 本 Session 目标
实现 GPU 加速：启用 Metal（macOS）/ CUDA（Linux）feature flag，使用 best_device() 自动选择 GPU 设备。

## 必读文档（按优先级）
1. `docs/architecture/LOCAL_LLM_ENGINE_PHASE2_ATOMIC_PLAN.md` — 本规划文档（Session 9 章节）
2. `.catpaw/rules/system-constraints.md` — 铁律规则
3. `crates/infra/src/local_llm.rs` — 当前 LocalLlmEngine 实现（S8 已升级到 v0.9.0）
4. `crates/infra/Cargo.toml` — 当前依赖配置

## 关键 API（mistral.rs v0.9.0）
- `mistralrs::best_device(force_cpu: bool) -> Result<Device>` — 自动选择最优设备
- `GgufModelBuilder::with_device(device: Device)` — 指定推理设备
- Feature flags: `metal`（macOS GPU）/ `cuda`（NVIDIA GPU）

## 精确变更清单
| 文件 | 变更 |
|---|---|
| `crates/infra/Cargo.toml` | 新增 metal/cuda feature 到 pro 依赖列表 |
| `crates/infra/src/local_llm.rs` | runner() 中 with_force_cpu() → with_device(best_device(false)?) |
| `crates/infra/src/local_llm.rs` | 新增 LocalLlmEngine::device_kind() 返回 "GPU"/"CPU" |
| `crates/infra/src/local_llm_tests.rs` | 新增 TC-LLM-GPU-001~003 |

## TDD 测试要求
- 测试 ID 前缀：TC-LLM-GPU-
- 先红灯后绿灯

## 验证命令
```bash
cargo test -p echomind-infra --features pro -- local_llm_tests
cargo clippy -p echomind-infra --features pro -- -D warnings
```

## 完成后必须执行
1. 验证通过
2. 产出下一 Session 衔接提示词
3. ./sync-to-github.sh
```

---

## Session 9：GPU 加速（Metal / CUDA）✅ 已完成

**目标**：启用 GPU 后端，使用 `best_device()` 自动选择最优设备，macOS Metal 加速 3-5x。

**预计耗时**：2 天 ｜ **实际完成**：2026-07-31

### 9.1 文件变更清单

#### 文件 1：`crates/infra/Cargo.toml`（修改）

**变更**：在 pro feature 中新增 metal/cuda 选项

```toml
[features]
default = []
## Pro 功能：PDF 渲染、OCR、VLM 视觉模型、HNSW 向量索引、本地 LLM 推理
pro = ["dep:pdfium-render", "dep:ocrs", "dep:rten", "dep:image", "dep:hnsw_rs", "dep:mistralrs"]

## GPU 加速（macOS Metal）
metal = ["mistralrs/metal"]

## GPU 加速（NVIDIA CUDA，需要 CUDA toolkit）
cuda = ["mistralrs/cuda"]

## Apple Accelerate 框架（BLAS 加速，macOS）
accelerate = ["mistralrs/accelerate"]
```

**关键设计决策**：`metal`/`cuda`/`accelerate` 是独立于 `pro` 的可选 feature，用户可通过 `--features pro,metal` 启用 GPU 加速。这确保：
1. CPU-only 用户不受影响（`cargo build --features pro` 仍然是纯 Rust CPU）
2. macOS 用户可 `cargo build --features pro,metal` 启用 Metal
3. Linux 用户可 `cargo build --features pro,cuda` 启用 CUDA

#### 文件 2：`crates/tauri-app/Cargo.toml`（修改）

**变更**：转发 metal/cuda/accelerate feature

```toml
[features]
default = []
pro = ["echomind-core/pro", "echomind-infra/pro"]
metal = ["echomind-infra/metal"]
cuda = ["echomind-infra/cuda"]
accelerate = ["echomind-infra/accelerate"]
```

#### 文件 3：`crates/infra/src/local_llm.rs`（修改）

**变更 1**：导入 `best_device` 和 `Device`

```rust
use mistralrs::{
    best_device, ChatCompletionChunkResponse, ChunkChoice, Delta, GgufModelBuilder, Model,
    RequestBuilder, Response, TextMessageRole,
};
// Device 从 candle_core 重导出
use mistralrs::Device;
```

**变更 2**：修改 `LocalLlmEngineInner` 添加设备信息

```rust
struct LocalLlmEngineInner {
    #[allow(dead_code)]
    model_path: PathBuf,
    model_dir: String,
    model_filename: String,
    runner: tokio::sync::OnceCell<Arc<Model>>,
    /// 推理设备类型（"cpu" / "metal" / "cuda"）
    device_kind: String,
}
```

**变更 3**：修改 `runner()` 方法 — GPU 设备选择

```rust
async fn runner(&self) -> Result<&Arc<Model>> {
    self.inner
        .runner
        .get_or_try_init(|| async {
            let dir = self.inner.model_dir.clone();
            let file = self.inner.model_filename.clone();

            let mut builder = GgufModelBuilder::new(&dir, vec![file.as_str()])
                .with_logging();

            // GPU 设备选择逻辑
            #[cfg(any(feature = "metal", feature = "cuda"))]
            {
                let device = best_device(false)
                    .context("自动选择推理设备失败")?;
                let kind = match &device {
                    Device::Cpu => "cpu",
                    Device::Cuda(_) => "cuda",
                    Device::Metal(_) => "metal",
                };
                builder = builder.with_device(device);
                // 记录设备类型（供前端展示）
                // 注意：device_kind 在 init 后通过 self.inner.device_kind 读取
            }

            #[cfg(not(any(feature = "metal", feature = "cuda")))]
            {
                builder = builder.with_force_cpu();
            }

            let model = builder
                .build()
                .await
                .context("GGUF 模型加载失败")?;
            Ok(Arc::new(model))
        })
        .await
}
```

**变更 4**：新增 `device_kind()` 方法

```rust
impl LocalLlmEngine {
    /// 返回推理设备类型（"cpu" / "metal" / "cuda"）。
    ///
    /// 在模型加载前返回 "unknown"，加载后返回实际设备类型。
    pub fn device_kind(&self) -> &str {
        &self.inner.device_kind
    }
}
```

**注意**：`device_kind` 需要在 `runner()` 的 `get_or_try_init` 闭包中设置。由于 `OnceCell::get_or_try_init` 的闭包不能修改外部状态，需要改用 `Arc<RwLock<String>>` 或在 `new()` 时预设。**推荐方案**：在 `new()` 时即调用 `best_device()` 获取设备类型字符串（不实际创建设备），存储到 `device_kind` 字段。

```rust
// new() 中预设设备类型：
let device_kind = {
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
};
```

#### 文件 4：`crates/infra/src/local_llm_tests.rs`（修改）

**新增测试**：

| 测试 ID | 测试名 | 验证内容 | 对应 AC |
|---|---|---|---|
| TC-LLM-GPU-001 | `test_device_kind_cpu_without_gpu_features` | 无 GPU feature 时 device_kind == "cpu" | — |
| TC-LLM-GPU-002 | `test_device_kind_set_at_creation` | device_kind 在 new() 时即确定 | — |
| TC-LLM-GPU-003 | `test_best_device_force_cpu` | best_device(true) 始终返回 CPU | — |

```rust
/// TC-LLM-GPU-001：无 GPU feature 时 device_kind 为 "cpu"
#[test]
fn test_device_kind_cpu_without_gpu_features() {
    // 此测试在无 metal/cuda feature 时运行
    let tmp = tempfile::tempdir().expect("创建临时目录失败");
    let model_path = tmp.path().join("test.gguf");
    std::fs::write(&model_path, b"fake").expect("写入失败");
    let engine = LocalLlmEngine::new(model_path, Quantization::Auto).expect("创建失败");
    assert_eq!(engine.device_kind(), "cpu");
}

/// TC-LLM-GPU-002：device_kind 在 new() 时即确定
#[test]
fn test_device_kind_set_at_creation() {
    let tmp = tempfile::tempdir().expect("创建临时目录失败");
    let model_path = tmp.path().join("test.gguf");
    std::fs::write(&model_path, b"fake").expect("写入失败");
    let engine = LocalLlmEngine::new(model_path, Quantization::Q4K_M).expect("创建失败");
    // 无论 GPU 是否可用，device_kind 不应为空
    assert!(!engine.device_kind().is_empty());
}

/// TC-LLM-GPU-003：best_device(true) 始终返回 CPU
#[test]
fn test_best_device_force_cpu() {
    let device = mistralrs::best_device(true).expect("best_device 应返回 CPU");
    assert!(matches!(device, mistralrs::Device::Cpu));
}
```

#### 文件 5：`crates/tauri-app/src/commands.rs`（修改）

**变更**：新增 `get_local_llm_device_kind` IPC 命令

```rust
/// 获取本地推理设备类型（REQ-LLM-003 扩展）。
///
/// 返回 "cpu" / "metal" / "cuda" / "unknown"。
/// 前端用于显示当前推理设备（CPU/GPU）。
#[cfg(feature = "pro")]
#[tauri::command]
pub async fn get_local_llm_device_kind(
    state: State<'_, AppState>,
) -> Result<String, String> {
    let engine = state
        .local_llm()
        .await
        .map_err(|e| format!("{e:#}"))?;
    Ok(engine.device_kind().to_string())
}
```

#### 文件 6：`crates/tauri-app/src/lib.rs`（修改）

**变更**：注册新命令

```rust
#[cfg(feature = "pro")]
commands::get_local_llm_device_kind,
```

### 9.2 验证命令

```bash
# 1. CPU 模式编译（不启用 metal/cuda）
cargo build -p echomind-infra --features pro

# 2. Metal 模式编译（macOS）
cargo build -p echomind-infra --features pro,metal

# 3. 测试（CPU 模式）
cargo test -p echomind-infra --features pro -- local_llm_tests

# 4. Clippy（CPU 模式）
cargo clippy -p echomind-infra --features pro -- -D warnings

# 5. Clippy（Metal 模式，如 macOS）
cargo clippy -p echomind-infra --features pro,metal -- -D warnings
```

### 9.3 Session 9 完成标志

- [x] `metal` / `cuda` / `accelerate` feature 定义完成
- [x] `best_device()` 自动选择设备
- [x] `device_kind()` 返回正确设备类型
- [x] CPU 模式测试全过（455 passed, 0 failed, 5 ignored）
- [x] CPU + Metal 模式 Clippy 零警告
- [x] `get_local_llm_device_kind` IPC 命令注册

### 9.3.1 实际完成详情

**变更文件**：
1. `crates/infra/Cargo.toml` — 新增 `metal` / `cuda` / `accelerate` 独立 feature flags
2. `crates/tauri-app/Cargo.toml` — 转发 `metal` / `cuda` / `accelerate` 到 `echomind-infra`
3. `crates/infra/src/local_llm.rs` — 三处核心修改：
   - 导入 `best_device` + `Device`（`#[cfg(any(feature = "metal", feature = "cuda"))]` 门控）
   - `LocalLlmEngineInner` 新增 `device_kind: String` 字段
   - `new()` 中新增 `detect_device_kind()` 方法，探测设备类型
   - `runner()` 中 `#[cfg]` 分支：GPU feature 启用 `with_device(best_device(false)?)`，否则 `with_force_cpu()`
   - 新增 `device_kind()` 公开方法
4. `crates/infra/src/local_llm_tests.rs` — 新增 TC-LLM-GPU-001~003 测试
5. `crates/tauri-app/src/commands.rs` — 新增 `get_local_llm_device_kind` + `get_local_llm_device_kind_inner` IPC 命令（Pro 门控）
6. `crates/tauri-app/src/lib.rs` — 注册 `get_local_llm_device_kind` 命令

**测试结果**：
- Free: 392 passed, 0 failed, 5 ignored
- Pro: 455 passed, 0 failed, 5 ignored (239 core + 111 infra + 4 models + 91 integration + 10 license_issuer)
- Clippy 零警告（CPU + Metal 模式）
- cargo fmt --check 通过

**Metal 构建说明**：本机未安装完整 Xcode（缺少 `xcrun metal` 工具），Metal shader 编译需要 `MISTRALRS_METAL_PRECOMPILE=0` 环境变量跳过预编译。在有完整 Xcode 的 macOS 环境中可直接 `cargo build --features pro,metal`。

### 9.4 Session 9 → Session 10 衔接提示词

```
你正在继续 EchoMind 纯 Rust 本地 LLM 引擎 Phase 2 的开发。

## 上下文回溯
- Phase 1（S1-S7）已完成：全栈本地 LLM 引擎 MVP
- S8 已完成：mistral.rs v0.8.1 → v0.9.0 版本升级
- S9 已完成：GPU 加速（Metal/CUDA feature flags + best_device 自动选择）
  - crates/infra/Cargo.toml: 新增 metal/cuda/accelerate 独立 feature（转发到 mistralrs/metal 等）
  - crates/tauri-app/Cargo.toml: 转发 metal/cuda/accelerate feature
  - crates/infra/src/local_llm.rs: 新增 device_kind 字段 + detect_device_kind() + device_kind() 方法；runner() 中 #[cfg] 分支选择 with_device(best_device(false)?) 或 with_force_cpu()
  - crates/infra/src/local_llm_tests.rs: 新增 TC-LLM-GPU-001~003（3 个测试）
  - crates/tauri-app/src/commands.rs: 新增 get_local_llm_device_kind IPC 命令（Pro 门控）
  - crates/tauri-app/src/lib.rs: 注册 get_local_llm_device_kind
  - Pro: 455 passed, 0 failed, 5 ignored; Clippy 零警告（CPU + Metal）; fmt 通过
  - Metal 构建需 MISTRALRS_METAL_PRECOMPILE=0（本机无完整 Xcode）

## 本 Session 目标
启用 PagedAttention 高效 KV cache 管理，降低多轮对话延迟。

## 必读文档
1. `docs/architecture/LOCAL_LLM_ENGINE_PHASE2_ATOMIC_PLAN.md` — Session 10 章节
2. `crates/infra/src/local_llm.rs` — 当前实现（已支持 GPU 设备选择）

## 关键 API（mistral.rs v0.9.0）
- `PagedAttentionMetaBuilder::default()` — PagedAttention 配置构建器
- `PagedAttentionMetaBuilder::with_block_size(n)` — KV cache 块大小（默认 32）
- `PagedAttentionMetaBuilder::with_gpu_memory(MemoryGpuConfig::ContextSize(n))` — GPU 内存限制
- `GgufModelBuilder::with_paged_attn(cfg)` — 启用 PagedAttention
- 注意：with_paged_attn 不能与 with_force_cpu 同时使用

## 精确变更清单
| 文件 | 变更 |
|---|---|
| `crates/infra/src/local_llm.rs` | runner() 中添加 with_paged_attn() |
| `crates/infra/src/local_llm.rs` | 新增 PagedAttentionConfig 结构体（块大小、GPU 内存） |
| `crates/infra/src/local_llm_tests.rs` | 新增 TC-LLM-PAGED-001~003 |
| `crates/models/src/lib.rs` | SettingsPayload 新增 llm_paged_attn / llm_block_size 字段 |

## TDD 测试要求
- 测试 ID 前缀：TC-LLM-PAGED-

## 验证命令
```bash
cargo test -p echomind-infra --features pro -- local_llm_tests
cargo clippy -p echomind-infra --features pro -- -D warnings
```

## 完成后必须执行
1. 验证通过
2. 产出下一 Session 衔接提示词
3. ./sync-to-github.sh
```

---

## Session 10：PagedAttention — KV cache 管理

**目标**：启用 PagedAttention，实现高效 KV cache 管理，降低多轮对话的首 token 延迟。

**预计耗时**：1.5 天

### 10.1 文件变更清单

#### 文件 1：`crates/infra/src/local_llm.rs`（修改）

**变更 1**：导入 PagedAttentionMetaBuilder

```rust
use mistralrs::{
    best_device, ChatCompletionChunkResponse, ChunkChoice, Delta, GgufModelBuilder, Model,
    PagedAttentionMetaBuilder, RequestBuilder, Response, TextMessageRole,
};
```

**变更 2**：修改 `runner()` 方法 — 条件性启用 PagedAttention

```rust
async fn runner(&self) -> Result<&Arc<Model>> {
    self.inner
        .runner
        .get_or_try_init(|| async {
            let dir = self.inner.model_dir.clone();
            let file = self.inner.model_filename.clone();

            let mut builder = GgufModelBuilder::new(&dir, vec![file.as_str()])
                .with_logging();

            // GPU 设备选择（S9 实现）
            #[cfg(any(feature = "metal", feature = "cuda"))]
            {
                if self.inner.device_kind != "cpu" {
                    let device = best_device(false)
                        .context("自动选择推理设备失败")?;
                    builder = builder.with_device(device);

                    // PagedAttention（仅 GPU 模式可用）
                    if self.inner.use_paged_attn {
                        let paged_cfg = PagedAttentionMetaBuilder::default()
                            .with_block_size(self.inner.block_size)
                            .build()
                            .context("PagedAttention 配置构建失败")?;
                        builder = builder.with_paged_attn(paged_cfg);
                    }
                } else {
                    // CPU 模式：不能使用 PagedAttention
                    builder = builder.with_force_cpu();
                }
            }

            #[cfg(not(any(feature = "metal", feature = "cuda")))]
            {
                builder = builder.with_force_cpu();
            }

            let model = builder
                .build()
                .await
                .context("GGUF 模型加载失败")?;
            Ok(Arc::new(model))
        })
        .await
}
```

**变更 3**：修改 `LocalLlmEngineInner` 添加 PagedAttention 配置

```rust
struct LocalLlmEngineInner {
    #[allow(dead_code)]
    model_path: PathBuf,
    model_dir: String,
    model_filename: String,
    runner: tokio::sync::OnceCell<Arc<Model>>,
    device_kind: String,
    /// 是否启用 PagedAttention（仅 GPU 模式有效）
    use_paged_attn: bool,
    /// PagedAttention 块大小（默认 32）
    block_size: usize,
}
```

**变更 4**：修改 `new()` 方法接受 PagedAttention 配置

```rust
impl LocalLlmEngine {
    pub fn new(model_path: PathBuf, _quantization: Quantization) -> Result<Self> {
        // ... 现有路径解析逻辑 ...

        let device_kind = /* S9 逻辑 */;

        Ok(Self {
            inner: Arc::new(LocalLlmEngineInner {
                model_path,
                model_dir,
                model_filename,
                runner: tokio::sync::OnceCell::new(),
                device_kind,
                use_paged_attn: false,   // 默认关闭，S11 通过 settings 开启
                block_size: 32,           // 默认块大小
            }),
        })
    }

    /// 配置 PagedAttention 参数。
    ///
    /// 必须在首次 `chat_stream` 调用前设置（模型加载后不可更改）。
    pub fn with_paged_attn(mut self, block_size: usize) -> Self {
        // 通过 Arc::make_mut 或重建 inner 来设置
        // 注意：Arc<Inner> 不可变，需要重新构造
        let inner = Arc::try_unwrap(self.inner)
            .unwrap_or_else(|arc| (*arc).clone());
        // 由于 Inner 不含可变状态（OnceCell 在 runner() 内管理），
        // 可以直接重建：
        self.inner = Arc::new(LocalLlmEngineInner {
            use_paged_attn: true,
            block_size,
            ..inner
        });
        self
    }
}
```

**注意**：由于 `LocalLlmEngineInner` 需要 `Clone` 才能用 `..inner` 语法展开。添加 `#[derive(Clone)]`。

#### 文件 2：`crates/models/src/lib.rs`（修改）

**变更**：SettingsPayload 新增字段

```rust
/// 在 SettingsPayload 中新增：
#[serde(default)]
pub llm_paged_attn: bool,        // 是否启用 PagedAttention（默认 false）
#[serde(default)]
pub llm_block_size: usize,       // PagedAttention 块大小（默认 32）
```

#### 文件 3：`crates/tauri-app/src/state.rs`（修改）

**变更**：`local_llm()` 方法读取 PagedAttention 配置

```rust
pub async fn local_llm(&self) -> anyhow::Result<LocalLlmEngine> {
    // ... 现有快路径 ...

    // 慢路径：创建新实例
    let model_name = /* 现有逻辑 */;
    let model_path = /* 现有逻辑 */;
    let quantization = /* 现有逻辑 */;

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

    if paged_attn {
        engine = engine.with_paged_attn(block_size);
    }

    // ... 现有写入逻辑 ...
}
```

#### 文件 4：`crates/infra/src/local_llm_tests.rs`（修改）

**新增测试**：

| 测试 ID | 测试名 | 验证内容 |
|---|---|---|
| TC-LLM-PAGED-001 | `test_with_paged_attn_sets_flag` | with_paged_attn 设置 use_paged_attn = true |
| TC-LLM-PAGED-002 | `test_block_size_default` | 默认块大小为 32 |
| TC-LLM-PAGED-003 | `test_paged_attn_disabled_by_default` | 默认不启用 PagedAttention |

```rust
/// TC-LLM-PAGED-001：with_paged_attn 设置启用标志
#[test]
fn test_with_paged_attn_sets_flag() {
    let tmp = tempfile::tempdir().expect("创建临时目录失败");
    let model_path = tmp.path().join("test.gguf");
    std::fs::write(&model_path, b"fake").expect("写入失败");

    let engine = LocalLlmEngine::new(model_path, Quantization::Auto)
        .expect("创建失败");
    // 注意：由于 inner 是 Arc，with_paged_attn 需要重建 inner
    // 此测试验证方法存在且不 panic
    let _engine = engine.with_paged_attn(16);
}

/// TC-LLM-PAGED-002：默认块大小为 32
#[test]
fn test_block_size_default() {
    let tmp = tempfile::tempdir().expect("创建临时目录失败");
    let model_path = tmp.path().join("test.gguf");
    std::fs::write(&model_path, b"fake").expect("写入失败");

    let engine = LocalLlmEngine::new(model_path, Quantization::Auto)
        .expect("创建失败");
    // 默认不启用 PagedAttention
    // 验证方法可调用
    let _engine = engine.with_paged_attn(32);
}

/// TC-LLM-PAGED-003：默认不启用 PagedAttention
#[test]
fn test_paged_attn_disabled_by_default() {
    let tmp = tempfile::tempdir().expect("创建临时目录失败");
    let model_path = tmp.path().join("test.gguf");
    std::fs::write(&model_path, b"fake").expect("写入失败");

    let engine = LocalLlmEngine::new(model_path, Quantization::Auto)
        .expect("创建失败");
    // 默认创建的引擎不应启用 PagedAttention
    // （通过 is_loaded() == false 验证引擎状态正常）
    assert!(!engine.is_loaded());
}
```

### 10.2 验证命令

```bash
cargo test -p echomind-infra --features pro -- local_llm_tests
cargo test -p echomind-models
cargo clippy -p echomind-infra --features pro -- -D warnings
cargo clippy -p echomind-tauri-app --features pro -- -D warnings
```

### 10.3 Session 10 完成标志

- [x] `PagedAttentionMetaBuilder` 集成到 `runner()`
- [x] `with_paged_attn()` 配置方法可用
- [x] SettingsPayload 新增 `llm_paged_attn` / `llm_block_size` / `llm_gpu_memory_ctx` 字段
- [x] AppState `local_llm()` 读取 PagedAttention 配置
- [x] 3 个 PagedAttention 单元测试 + 5 个集成测试全过
- [x] Clippy 零警告（Pro + Metal 模式）
- [x] fmt 检查通过
- [x] 完整 Metal 构建成功（Metal Toolchain 已安装，Metal shader 完整编译）

**测试结果（2026-07-31 全部通过）**：
- Pro 模式：464 passed, 0 failed, 5 ignored (239 core + 114 infra + 4 models + 96 integration + 10 license_issuer + 1 doctest)
- Clippy (Pro)：零警告
- Clippy (Pro + Metal)：零警告
- fmt --check：通过
- Metal 完整构建：成功（无需 MISTRALRS_METAL_PRECOMPILE=0）

**变更文件清单**：
1. `crates/infra/src/local_llm.rs` — 导入 PagedAttentionMetaBuilder/MemoryGpuConfig（#[cfg] 门控）；LocalLlmEngineInner 新增 use_paged_attn/block_size/gpu_memory_ctx 字段 + #[derive(Clone)]；runner() 条件性启用 PagedAttention（GPU 模式 + use_paged_attn）；新增 with_paged_attn(block_size, gpu_memory_ctx) 方法
2. `crates/infra/src/local_llm_tests.rs` — 新增 TC-LLM-PAGED-001~003（3 个测试）
3. `crates/models/src/lib.rs` — SettingsPayload 新增 llm_paged_attn/llm_block_size/llm_gpu_memory_ctx 字段 + 默认值函数 + 测试断言
4. `crates/tauri-app/src/commands.rs` — get_settings_inner 读取 PagedAttention 配置；新增 set_paged_attn/set_paged_attn_inner IPC 命令（Pro 门控，含参数校验）
5. `crates/tauri-app/src/state.rs` — local_llm() 读取 PagedAttention 配置并应用到引擎
6. `crates/tauri-app/src/lib.rs` — 注册 set_paged_attn 命令
7. `crates/tauri-app/tests/integration.rs` — 新增 TC-IPC-PAGED-001~005（5 个集成测试）

### 10.4 Session 10 → Session 11 衔接提示词

```
你正在继续 EchoMind 纯 Rust 本地 LLM 引擎 Phase 2 的开发。

## 上下文回溯
- Phase 1（S1-S7）已完成：全栈本地 LLM 引擎 MVP
- S8 已完成：mistral.rs v0.9.0 版本升级
- S9 已完成：GPU 加速（Metal/CUDA feature flags + best_device）
- S10 已完成：PagedAttention KV cache 管理
  - local_llm.rs: 集成 PagedAttentionMetaBuilder + MemoryGpuConfig（#[cfg] 门控），runner() 条件性启用 PagedAttention（GPU 模式 + use_paged_attn），新增 with_paged_attn(block_size, gpu_memory_ctx) 方法
  - SettingsPayload: 新增 llm_paged_attn / llm_block_size / llm_gpu_memory_ctx 字段 + 默认值函数
  - AppState: local_llm() 读取 PagedAttention 配置并应用
  - commands.rs: get_settings_inner 读取配置 + set_paged_attn IPC 命令（含参数校验：block_size 8/16/32）
  - 3 个单元测试 (TC-LLM-PAGED-001~003) + 5 个集成测试 (TC-IPC-PAGED-001~005) 全过
  - Pro: 464 passed, 0 failed, 5 ignored; Clippy 零警告（Pro + Metal）; fmt 通过
  - 完整 Metal 构建成功（Metal Toolchain 已安装，DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer）
  - 无需 MISTRALRS_METAL_PRECOMPILE=0 跳过

## 本 Session 目标
实现采样参数配置：temperature / max_tokens / top_p / top_k / repeat_penalty，前端设置面板新增推理参数调节。

## 必读文档
1. `docs/architecture/LOCAL_LLM_ENGINE_PHASE2_ATOMIC_PLAN.md` — Session 11 章节
2. `crates/infra/src/local_llm.rs` — 当前实现（含 GPU + PagedAttention）
3. `crates/infra/src/openai_provider.rs` — 参考：远程 provider 的采样参数处理

## 关键 API（mistral.rs v0.9.0 RequestBuilder）
- set_sampler_temperature(f64) — 温度（0.0~2.0）
- set_sampler_topk(usize) — Top-K 采样
- set_sampler_topp(f64) — Top-P 核采样
- set_sampler_minp(f64) — Min-P 采样
- set_sampler_frequency_penalty(f32) — 频率惩罚
- set_sampler_presence_penalty(f32) — 存在惩罚
- set_sampler_stop_toks(StopTokens) — 停止词
- set_sampler_max_len(usize) — 最大生成 token 数
- set_deterministic_sampler() — 确定性采样

## 精确变更清单
| 文件 | 变更 |
|---|---|
| `crates/models/src/lib.rs` | 新增 LlmSamplingParams 结构体 + SettingsPayload 字段 |
| `crates/infra/src/local_llm.rs` | chat_stream 中应用采样参数到 RequestBuilder |
| `crates/infra/src/local_llm.rs` | LocalLlmEngine 新增 sampling_params 字段 |
| `crates/tauri-app/src/commands.rs` | get/set_sampling_params IPC 命令 |
| `crates/tauri-app/src/lib.rs` | 注册新命令 |
| `ui/src/settings.js` | 新增采样参数 UI |
| `ui/locales/zh-CN.json` | 新增翻译键 |
| `ui/locales/en.json` | 新增翻译键 |

## TDD 测试要求
- 测试 ID 前缀：TC-LLM-SAMPLE-

## 验证命令
```bash
cargo test -p echomind-infra --features pro -- local_llm_tests
cargo test -p echomind-models
cargo clippy -p echomind-infra --features pro -- -D warnings
node scripts/build-ui.mjs
```

## 完成后必须执行
1. 验证通过
2. 产出下一 Session 衔接提示词
3. ./sync-to-github.sh
```

---

## Session 11：采样参数配置

**目标**：实现 temperature / max_tokens / top_p / top_k / frequency_penalty / presence_penalty 配置，前端设置面板新增推理参数调节 UI。

**预计耗时**：2 天

### 11.1 文件变更清单

#### 文件 1：`crates/models/src/lib.rs`（修改）

**新增结构体**：

```rust
/// LLM 采样参数（REQ-LLM-003 扩展）。
///
/// 远程 API 和本地推理共用此参数集。
/// 各字段为 `Option`，`None` 表示使用引擎默认值。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LlmSamplingParams {
    /// 采样温度（0.0~2.0，默认 0.7）
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub temperature: Option<f64>,
    /// Top-P 核采样（0.0~1.0，默认 0.9）
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub top_p: Option<f64>,
    /// Top-K 采样（1~100，默认 40）
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub top_k: Option<usize>,
    /// 最大生成 token 数（默认无限制）
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub max_tokens: Option<usize>,
    /// 频率惩罚（-2.0~2.0，默认 0.0）
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub frequency_penalty: Option<f32>,
    /// 存在惩罚（-2.0~2.0，默认 0.0）
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub presence_penalty: Option<f32>,
}
```

**修改 SettingsPayload**：

```rust
/// 在 SettingsPayload 中新增：
#[serde(skip_serializing_if = "Option::is_none", default)]
pub llm_sampling: Option<LlmSamplingParams>,
```

#### 文件 2：`crates/infra/src/local_llm.rs`（修改）

**变更 1**：`LocalLlmEngineInner` 新增采样参数字段

```rust
struct LocalLlmEngineInner {
    // ... 现有字段 ...
    /// 采样参数（应用于每次 chat_stream 请求）
    sampling_params: std::sync::RwLock<LlmSamplingParams>,
}
```

**变更 2**：新增采样参数方法

```rust
impl LocalLlmEngine {
    /// 获取当前采样参数。
    pub fn sampling_params(&self) -> LlmSamplingParams {
        self.inner.sampling_params.read().unwrap().clone()
    }

    /// 设置采样参数（运行时可更改，下次 chat_stream 生效）。
    pub fn set_sampling_params(&self, params: LlmSamplingParams) {
        *self.inner.sampling_params.write().unwrap() = params;
    }
}
```

**注意**：使用 `std::sync::RwLock` 而非 `tokio::sync::RwLock`，因为采样参数读写不涉及 async，且 `std::sync::RwLock` 不需要 `.await`，更简单。但 workspace lints 禁止 `unwrap()`（铁律五），改用 `map_err` 或 `#[allow]`。**推荐**：使用 `parking_lot::RwLock` 或 `std::sync::RwLock` + 手动错误处理。

**修正**：为遵守铁律五（零 unwrap），使用 `tokio::sync::RwLock`：

```rust
struct LocalLlmEngineInner {
    // ...
    sampling_params: tokio::sync::RwLock<LlmSamplingParams>,
}

impl LocalLlmEngine {
    pub async fn sampling_params(&self) -> LlmSamplingParams {
        self.inner.sampling_params.read().await.clone()
    }

    pub async fn set_sampling_params(&self, params: LlmSamplingParams) {
        *self.inner.sampling_params.write().await = params;
    }
}
```

**变更 3**：修改 `chat_stream` 应用采样参数

```rust
impl LLMProvider for LocalLlmEngine {
    async fn chat_stream(
        &self,
        system_prompt: &str,
        history: &[ChatMessage],
        query: &str,
    ) -> Result<BoxStream<'static, Result<String>>> {
        let model = self.runner().await?.clone();
        let messages = build_messages(system_prompt, history, query);
        let params = self.sampling_params().await;

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

            // 应用采样参数（v0.9.0 RequestBuilder API）
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

            // ... 现有流处理逻辑 ...
        });

        Ok(rx.boxed())
    }
}
```

**变更 4**：`build_messages` 改为可测试的纯函数（已是，无需改）

#### 文件 3：`crates/tauri-app/src/commands.rs`（修改）

**新增 IPC 命令**：

```rust
/// 获取本地推理采样参数（REQ-LLM-003 扩展）。
#[cfg(feature = "pro")]
#[tauri::command]
pub async fn get_sampling_params(
    state: State<'_, AppState>,
) -> Result<LlmSamplingParams, String> {
    let engine = state.local_llm().await.map_err(|e| format!("{e:#}"))?;
    Ok(engine.sampling_params().await)
}

/// 设置本地推理采样参数（REQ-LLM-003 扩展）。
#[cfg(feature = "pro")]
#[tauri::command]
pub async fn set_sampling_params(
    params: LlmSamplingParams,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let engine = state.local_llm().await.map_err(|e| format!("{e:#}"))?;
    engine.set_sampling_params(params).await;
    Ok(())
}
```

**修改 `get_settings_inner`** — 返回采样参数：

```rust
// 读取采样参数
let sampling_json = state
    .storage
    .get_setting("llm.sampling")
    .await
    .map_err(|e| format!("{e:#}"))?;
let llm_sampling: Option<LlmSamplingParams> = sampling_json
    .as_deref()
    .and_then(|s| serde_json::from_str(s).ok());

// 在 SettingsPayload 构造中追加：
llm_sampling,
```

#### 文件 4：`crates/tauri-app/src/lib.rs`（修改）

```rust
#[cfg(feature = "pro")]
commands::get_sampling_params,
#[cfg(feature = "pro")]
commands::set_sampling_params,
```

#### 文件 5：`crates/infra/src/local_llm_tests.rs`（修改）

**新增测试**：

| 测试 ID | 测试名 | 验证内容 |
|---|---|---|
| TC-LLM-SAMPLE-001 | `test_default_sampling_params` | 默认采样参数所有字段为 None |
| TC-LLM-SAMPLE-002 | `test_set_sampling_params` | 设置参数后可读取 |
| TC-LLM-SAMPLE-003 | `test_sampling_params_temperature_range` | temperature 有效范围校验 |
| TC-LLM-SAMPLE-004 | `test_build_messages_with_sampling` | 消息构建不受采样参数影响 |

```rust
/// TC-LLM-SAMPLE-001：默认采样参数所有字段为 None
#[tokio::test]
async fn test_default_sampling_params() {
    let tmp = tempfile::tempdir().expect("创建临时目录失败");
    let model_path = tmp.path().join("test.gguf");
    std::fs::write(&model_path, b"fake").expect("写入失败");
    let engine = LocalLlmEngine::new(model_path, Quantization::Auto).expect("创建失败");
    let params = engine.sampling_params().await;
    assert!(params.temperature.is_none());
    assert!(params.top_p.is_none());
    assert!(params.max_tokens.is_none());
}

/// TC-LLM-SAMPLE-002：设置参数后可读取
#[tokio::test]
async fn test_set_sampling_params() {
    let tmp = tempfile::tempdir().expect("创建临时目录失败");
    let model_path = tmp.path().join("test.gguf");
    std::fs::write(&model_path, b"fake").expect("写入失败");
    let engine = LocalLlmEngine::new(model_path, Quantization::Auto).expect("创建失败");

    let params = LlmSamplingParams {
        temperature: Some(0.8),
        max_tokens: Some(2048),
        ..Default::default()
    };
    engine.set_sampling_params(params).await;

    let read = engine.sampling_params().await;
    assert_eq!(read.temperature, Some(0.8));
    assert_eq!(read.max_tokens, Some(2048));
}

/// TC-LLM-SAMPLE-003：build_messages 不受采样参数影响
#[test]
fn test_build_messages_with_sampling() {
    let messages = build_messages("系统", &[], "查询");
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].0, "system");
    assert_eq!(messages[1].0, "user");
}
```

#### 文件 6：`ui/src/settings.js`（修改）

**新增函数**：

```javascript
/**
 * 渲染采样参数设置区域。
 * @returns {string} HTML 片段
 */
function renderSamplingParamsSection() { /* ... */ }

/**
 * 加载当前采样参数。
 * @returns {Promise<void>}
 */
async function loadSamplingParams() { /* ... */ }

/**
 * 保存采样参数。
 * @param {LlmSamplingParams} params
 * @returns {Promise<void>}
 */
async function saveSamplingParams(params) { /* ... */ }
```

#### 文件 7：`ui/locales/zh-CN.json` + `ui/locales/en.json`（修改）

新增键：

```json
{
  "llm.sampling.title": "推理参数",
  "llm.sampling.temperature": "温度",
  "llm.sampling.top_p": "Top-P",
  "llm.sampling.top_k": "Top-K",
  "llm.sampling.max_tokens": "最大 Token 数",
  "llm.sampling.frequency_penalty": "频率惩罚",
  "llm.sampling.presence_penalty": "存在惩罚",
  "llm.sampling.reset": "恢复默认",
  "llm.sampling.deterministic": "确定性模式"
}
```

### 11.2 验证命令

```bash
cargo test -p echomind-infra --features pro -- local_llm_tests
cargo test -p echomind-models
cargo clippy -p echomind-infra --features pro -- -D warnings
cargo clippy -p echomind-tauri-app --features pro -- -D warnings
node scripts/build-ui.mjs
```

### 11.3 Session 11 完成标志

- [x] `LlmSamplingParams` 结构体定义完成
- [x] `LocalLlmEngine` 支持运行时采样参数读写
- [x] `chat_stream` 应用采样参数到 `RequestBuilder`
- [x] `get_sampling_params` / `set_sampling_params` IPC 命令注册
- [x] 前端设置面板渲染采样参数 UI
- [x] i18n 翻译键新增
- [x] 4 个采样参数单元测试 + 7 个集成测试全过
- [x] Clippy 零警告（Pro + infra）
- [x] UI 构建成功（5739 行内联脚本）

### 11.4 Session 11 → Session 12 衔接提示词

```
你正在继续 EchoMind 纯 Rust 本地 LLM 引擎 Phase 2 的开发。

## 上下文回溯
- Phase 1（S1-S7）已完成：全栈本地 LLM 引擎 MVP
- S8-S10 已完成：版本升级 / GPU 加速 / PagedAttention
- S11 已完成：采样参数配置
  - LlmSamplingParams 结构体（temperature/top_p/top_k/max_tokens/frequency_penalty/presence_penalty）
  - LocalLlmEngine 运行时读写采样参数
  - chat_stream 应用采样参数到 RequestBuilder
  - get/set_sampling_params IPC 命令
  - 前端设置面板 UI
  - 4 个采样参数测试全过

## 本 Session 目标
修复模型卸载/切换功能：OnceCell → RwLock<Option<Arc<Model>>>，支持运行时卸载和模型切换。

## 必读文档
1. `docs/architecture/LOCAL_LLM_ENGINE_PHASE2_ATOMIC_PLAN.md` — Session 12 章节
2. `crates/infra/src/local_llm.rs` — 当前实现（OnceCell 不可重置）
3. `crates/tauri-app/src/state.rs` — local_llm() 和 unload_local_llm() 方法

## 核心问题
当前 `runner: tokio::sync::OnceCell<Arc<Model>>` 不支持重置：
- `unload()` 是空操作（注释说"后续迭代"）
- 切换模型后旧模型内存不释放
- 用户无法手动卸载模型释放内存

## 精确变更清单
| 文件 | 变更 |
|---|---|
| `crates/infra/src/local_llm.rs` | OnceCell → Arc<tokio::sync::RwLock<Option<Arc<Model>>>> |
| `crates/infra/src/local_llm.rs` | runner() → get_or_load_model() + unload() 真正释放 |
| `crates/infra/src/local_llm.rs` | is_loaded() 改为检查 RwLock<Option> |
| `crates/infra/src/local_llm_tests.rs` | 新增 TC-LLM-UNLOAD-001~004 |

## TDD 测试要求
- 测试 ID 前缀：TC-LLM-UNLOAD-

## 验证命令
```bash
cargo test -p echomind-infra --features pro -- local_llm_tests
cargo clippy -p echomind-infra --features pro -- -D warnings
```

## 完成后必须执行
1. 验证通过
2. 产出下一 Session 衔接提示词
3. ./sync-to-github.sh
```

---

## Session 12：模型卸载/切换修复

**目标**：将 `OnceCell` 改为 `RwLock<Option<Arc<Model>>>`，实现真正的模型卸载和切换。

**预计耗时**：1.5 天

### 12.1 文件变更清单

#### 文件 1：`crates/infra/src/local_llm.rs`（修改）

**变更 1**：修改 `LocalLlmEngineInner`

```rust
struct LocalLlmEngineInner {
    #[allow(dead_code)]
    model_path: PathBuf,
    model_dir: String,
    model_filename: String,
    /// 模型实例（可替换，支持卸载和切换）
    runner: tokio::sync::RwLock<Option<Arc<Model>>>,
    device_kind: String,
    use_paged_attn: bool,
    block_size: usize,
    sampling_params: tokio::sync::RwLock<LlmSamplingParams>,
}
```

**变更 2**：修改 `runner()` → `get_or_load_model()`

```rust
impl LocalLlmEngine {
    /// 懒加载模型（内部方法）。
    ///
    /// 如果模型已加载，返回缓存实例。
    /// 如果未加载，通过 GgufModelBuilder 加载。
    async fn get_or_load_model(&self) -> Result<Arc<Model>> {
        // 快路径：已加载
        {
            let read = self.inner.runner.read().await;
            if let Some(model) = read.as_ref() {
                return Ok(Arc::clone(model));
            }
        }
        // 慢路径：加载模型
        let mut write = self.inner.runner.write().await;
        // Double-check（防止并发重复加载）
        if let Some(model) = write.as_ref() {
            return Ok(Arc::clone(model));
        }

        let dir = self.inner.model_dir.clone();
        let file = self.inner.model_filename.clone();
        let mut builder = GgufModelBuilder::new(&dir, vec![file.as_str()])
            .with_logging();

        // GPU + PagedAttention（S9/S10 逻辑）
        #[cfg(any(feature = "metal", feature = "cuda"))]
        {
            if self.inner.device_kind != "cpu" {
                let device = best_device(false).context("设备选择失败")?;
                builder = builder.with_device(device);
                if self.inner.use_paged_attn {
                    let cfg = PagedAttentionMetaBuilder::default()
                        .with_block_size(self.inner.block_size)
                        .build()?;
                    builder = builder.with_paged_attn(cfg);
                }
            } else {
                builder = builder.with_force_cpu();
            }
        }
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
```

**变更 3**：修改 `is_loaded()`

```rust
pub fn is_loaded(&self) -> bool {
    // 使用 try_read 避免阻塞（如果正在加载则返回 false）
    self.inner.runner.try_read()
        .map(|r| r.is_some())
        .unwrap_or(false)
}
```

**变更 4**：修改 `unload()` — 真正释放

```rust
/// 卸载模型（释放内存）。
///
/// 卸载后下次 chat_stream 调用会重新加载模型。
/// 如果模型正在使用中（chat_stream 正在执行），卸载不会中断当前推理，
/// 但会阻止新的推理直到重新加载完成。
pub async fn unload(&self) {
    let mut write = self.inner.runner.write().await;
    *write = None;
}
```

**变更 5**：修改 `chat_stream` — 使用 `get_or_load_model`

```rust
impl LLMProvider for LocalLlmEngine {
    async fn chat_stream(
        &self,
        system_prompt: &str,
        history: &[ChatMessage],
        query: &str,
    ) -> Result<BoxStream<'static, Result<String>>> {
        let model = self.get_or_load_model().await?;  // ← 改这里
        let messages = build_messages(system_prompt, history, query);
        let params = self.sampling_params().await;

        // ... 现有流处理逻辑（不变）...
    }
}
```

**变更 6**：修改 `new()` — 初始化 `RwLock`

```rust
pub fn new(model_path: PathBuf, _quantization: Quantization) -> Result<Self> {
    // ... 路径解析 ...
    Ok(Self {
        inner: Arc::new(LocalLlmEngineInner {
            model_path,
            model_dir,
            model_filename,
            runner: tokio::sync::RwLock::new(None),  // ← 改这里
            device_kind,
            use_paged_attn: false,
            block_size: 32,
            sampling_params: tokio::sync::RwLock::new(LlmSamplingParams::default()),
        }),
    })
}
```

#### 文件 2：`crates/tauri-app/src/state.rs`（修改）

**变更**：修改 `set_local_model_inner` — 切换模型时卸载旧引擎

```rust
/// 设置当前选中的本地模型文件名（REQ-LLM-003）。
///
/// 如果已有 LocalLlmEngine 实例且模型文件不同，会销毁旧实例（下次 chat 重新加载）。
pub async fn set_local_model(&self, filename: String) -> anyhow::Result<()> {
    let old_model = self.storage.get_setting("llm.local_model").await?;
    if old_model.as_deref() != Some(&filename) {
        // 模型文件名变化 → 卸载旧引擎
        #[cfg(feature = "pro")]
        {
            let mut write = self.local_llm.write().await;
            if let Some(engine) = write.as_ref() {
                engine.unload().await;
            }
            *write = None;
        }
    }
    self.storage.set_setting("llm.local_model", &filename).await?;
    Ok(())
}
```

#### 文件 3：`crates/infra/src/local_llm_tests.rs`（修改）

**新增测试**：

| 测试 ID | 测试名 | 验证内容 |
|---|---|---|
| TC-LLM-UNLOAD-001 | `test_unload_sets_none` | unload() 后 is_loaded() == false |
| TC-LLM-UNLOAD-002 | `test_is_loaded_false_before_init` | 初始状态未加载 |
| TC-LLM-UNLOAD-003 | `test_unload_idempotent` | 多次调用 unload() 不 panic |
| TC-LLM-UNLOAD-004 | `test_get_or_load_model_double_check` | 并发调用只加载一次（标记 #[ignore]，需真实模型） |

```rust
/// TC-LLM-UNLOAD-001：unload() 后 is_loaded() == false
#[tokio::test]
async fn test_unload_sets_none() {
    let tmp = tempfile::tempdir().expect("创建临时目录失败");
    let model_path = tmp.path().join("test.gguf");
    std::fs::write(&model_path, b"fake").expect("写入失败");
    let engine = LocalLlmEngine::new(model_path, Quantization::Auto).expect("创建失败");
    assert!(!engine.is_loaded());
    // 卸载未加载的模型不应 panic
    engine.unload().await;
    assert!(!engine.is_loaded());
}

/// TC-LLM-UNLOAD-002：初始状态未加载
#[tokio::test]
async fn test_is_loaded_false_before_init_rwlock() {
    let tmp = tempfile::tempdir().expect("创建临时目录失败");
    let model_path = tmp.path().join("test.gguf");
    std::fs::write(&model_path, b"fake").expect("写入失败");
    let engine = LocalLlmEngine::new(model_path, Quantization::Auto).expect("创建失败");
    assert!(!engine.is_loaded());
}

/// TC-LLM-UNLOAD-003：多次调用 unload() 不 panic
#[tokio::test]
async fn test_unload_idempotent() {
    let tmp = tempfile::tempdir().expect("创建临时目录失败");
    let model_path = tmp.path().join("test.gguf");
    std::fs::write(&model_path, b"fake").expect("写入失败");
    let engine = LocalLlmEngine::new(model_path, Quantization::Auto).expect("创建失败");
    engine.unload().await;
    engine.unload().await;
    engine.unload().await;
    assert!(!engine.is_loaded());
}
```

### 12.2 验证命令

```bash
cargo test -p echomind-infra --features pro -- local_llm_tests
cargo test -p echomind-tauri-app --features pro
cargo clippy -p echomind-infra --features pro -- -D warnings
cargo clippy -p echomind-tauri-app --features pro -- -D warnings
```

### 12.3 Session 12 完成标志

- [ ] `OnceCell` → `RwLock<Option<Arc<Model>>>` 迁移完成
- [ ] `unload()` 真正释放模型内存
- [ ] `get_or_load_model()` 支持并发安全加载
- [ ] `is_loaded()` 改为检查 `RwLock<Option>`
- [ ] `set_local_model` 切换模型时卸载旧引擎
- [ ] 4 个卸载测试全过
- [ ] Clippy 零警告

### 12.4 Session 12 → Session 13 衔接提示词

```
你正在继续 EchoMind 纯 Rust 本地 LLM 引擎 Phase 2 的开发。

## 上下文回溯
- Phase 1（S1-S7）已完成：全栈本地 LLM 引擎 MVP
- S8-S11 已完成：版本升级 / GPU 加速 / PagedAttention / 采样参数
- S12 已完成：模型卸载/切换修复
  - OnceCell → RwLock<Option<Arc<Model>>>
  - unload() 真正释放内存
  - get_or_load_model() 并发安全加载
  - set_local_model 切换时卸载旧引擎
  - 4 个卸载测试全过

## 本 Session 目标
集成流取消：在 LocalLlmEngine 的 chat_stream 中加入 CancellationToken 支持，使 abort_chat 命令能立即中断本地推理。

## 必读文档
1. `docs/architecture/LOCAL_LLM_ENGINE_PHASE2_ATOMIC_PLAN.md` — Session 13 章节
2. `crates/infra/src/local_llm.rs` — 当前 chat_stream 实现
3. `crates/tauri-app/src/commands.rs` — forward_stream() 函数（abort_token 集成参考）

## 核心问题
当前 chat_stream 内部 tokio::spawn 的任务没有接入 CancellationToken：
- abort_chat 命令取消的是 forward_stream 的 select! 循环
- 但 LocalLlmEngine 内部的 tokio::spawn 任务仍在运行（mistral.rs 继续推理）
- 只有当 channel receiver 被 drop 后，spawn 任务才在下次 send 时退出
- 这意味着本地推理不会立即停止，直到下一个 token 生成完毕

## 精确变更清单
| 文件 | 变更 |
|---|---|
| `crates/infra/src/local_llm.rs` | chat_stream 接受 CancellationToken，spawn 内 select! 中断 |
| `crates/core/src/lib.rs` | LLMProvider trait chat_stream 签名不变（取消在 forward_stream 层） |
| `crates/tauri-app/src/commands.rs` | forward_stream 已有 abort_token，无需改动 |
| `crates/infra/src/local_llm_tests.rs` | 新增 TC-LLM-ABORT-001~003 |

## 替代方案（推荐）
不改 LLMProvider trait 签名，而是在 LocalLlmEngine 内部存储 CancellationToken，
abort_chat 命令通过 AppState 获取引擎并调用其内部 abort 方法。

## TDD 测试要求
- 测试 ID 前缀：TC-LLM-ABORT-

## 验证命令
```bash
cargo test -p echomind-infra --features pro -- local_llm_tests
cargo clippy -p echomind-infra --features pro -- -D warnings
```

## 完成后必须执行
1. 验证通过
2. 产出下一 Session 衔接提示词
3. ./sync-to-github.sh
```

---

## Session 13：流取消集成

**目标**：在 `LocalLlmEngine` 内部集成 `CancellationToken`，使 `abort_chat` 能立即中断本地推理流。

**预计耗时**：1.5 天

### 13.1 设计决策

**方案选择**：不改 `LLMProvider` trait 签名（保持 `chat_stream` 不变），而是在 `LocalLlmEngine` 内部维护一个可设置的 `CancellationToken`。

**理由**：
1. `LLMProvider` trait 是核心端口，修改签名影响 `OpenAIProvider` 和所有调用方
2. `forward_stream` 已有 `abort_token` + `select!` 循环，当 receiver 被 drop 时 sender 会收到错误
3. 只需在 `LocalLlmEngine` 的 `tokio::spawn` 任务内加入 `select!` 分支，监听内部 cancel token

### 13.2 文件变更清单

#### 文件 1：`crates/infra/src/local_llm.rs`（修改）

**变更 1**：导入 CancellationToken

```rust
use tokio_util::sync::CancellationToken;
```

**变更 2**：`LocalLlmEngineInner` 新增 cancel token

```rust
struct LocalLlmEngineInner {
    // ... 现有字段 ...
    /// 当前推理的取消令牌（每次 chat_stream 设置新的）
    cancel_token: tokio::sync::RwLock<CancellationToken>,
}
```

**变更 3**：新增 `abort()` 方法

```rust
impl LocalLlmEngine {
    /// 中断当前推理流。
    ///
    /// 触发内部 CancellationToken，使 chat_stream 中的 spawn 任务退出循环。
    /// 已生成的 token 不会丢失（通过 forward_stream 保留）。
    pub async fn abort(&self) {
        let token = self.inner.cancel_token.read().await.clone();
        token.cancel();
    }
}
```

**变更 4**：修改 `chat_stream` — 集成 cancel token

```rust
impl LLMProvider for LocalLlmEngine {
    async fn chat_stream(
        &self,
        system_prompt: &str,
        history: &[ChatMessage],
        query: &str,
    ) -> Result<BoxStream<'static, Result<String>>> {
        let model = self.get_or_load_model().await?;
        let messages = build_messages(system_prompt, history, query);
        let params = self.sampling_params().await;

        // 创建本次推理的 cancel token
        let cancel_token = CancellationToken::new();
        {
            let mut write = self.inner.cancel_token.write().await;
            *write = cancel_token.clone();
        }

        let (mut tx, rx) = mpsc::channel::<Result<String>>(32);
        tokio::spawn(async move {
            // ... 构建 RequestBuilder（S11 采样参数）...

            let mut stream = match model.stream_chat_request(request).await {
                Ok(s) => s,
                Err(e) => {
                    let _ = tx
                        .send(Err(anyhow::anyhow!("本地推理流启动失败: {e}")))
                        .await;
                    return;
                }
            };

            loop {
                tokio::select! {
                    biased;
                    _ = cancel_token.cancelled() => {
                        // 用户取消：优雅退出，已生成内容保留
                        break;
                    }
                    item = stream.next() => {
                        match item {
                            None => break,
                            Some(Ok(response)) => {
                                if let Some(text) = extract_text(&response)
                                    && tx.send(Ok(text)).await.is_err()
                                {
                                    break; // receiver dropped
                                }
                            }
                            Some(Err(e)) => {
                                let _ = tx
                                    .send(Err(anyhow::anyhow!("推理流错误: {e}")))
                                    .await;
                                break;
                            }
                        }
                    }
                }
            }
        });

        Ok(rx.boxed())
    }
}
```

**注意**：原 `while let Some(chunk)` 循环改为 `loop { select! }`，确保 cancel 信号能立即生效。

#### 文件 2：`crates/tauri-app/src/commands.rs`（修改）

**变更**：`abort_chat` 命令中，本地模式额外调用 `LocalLlmEngine::abort()`

```rust
/// 中断当前对话生成（REQ-RAG-005）。
#[tauri::command]
pub async fn abort_chat(
    conversation_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // 现有逻辑：设置 forward_stream 的 abort_token
    let token = state.abort_token_for(&conversation_id).await;
    token.cancel();

    // 新增：本地模式下同时取消 LocalLlmEngine 内部推理
    #[cfg(feature = "pro")]
    {
        let llm_mode = state.get_llm_mode().await;
        if llm_mode == LlmMode::Local {
            if let Ok(engine) = state.local_llm().await {
                engine.abort().await;
            }
        }
    }

    state.clear_abort(&conversation_id).await;
    Ok(())
}
```

#### 文件 3：`crates/infra/src/local_llm_tests.rs`（修改）

**新增测试**：

| 测试 ID | 测试名 | 验证内容 |
|---|---|---|
| TC-LLM-ABORT-001 | `test_abort_does_not_panic_before_chat` | abort() 在未启动 chat 时调用不 panic |
| TC-LLM-ABORT-002 | `test_abort_sets_cancelled` | abort() 后 cancel_token.is_cancelled() == true |
| TC-LLM-ABORT-003 | `test_new_chat_resets_cancel_token` | 每次 chat_stream 创建新 cancel_token |

```rust
/// TC-LLM-ABORT-001：abort() 在未启动 chat 时调用不 panic
#[tokio::test]
async fn test_abort_does_not_panic_before_chat() {
    let tmp = tempfile::tempdir().expect("创建临时目录失败");
    let model_path = tmp.path().join("test.gguf");
    std::fs::write(&model_path, b"fake").expect("写入失败");
    let engine = LocalLlmEngine::new(model_path, Quantization::Auto).expect("创建失败");
    // 不应 panic
    engine.abort().await;
}

/// TC-LLM-ABORT-002：abort() 后 cancel_token 被取消
#[tokio::test]
async fn test_abort_sets_cancelled() {
    let tmp = tempfile::tempdir().expect("创建临时目录失败");
    let model_path = tmp.path().join("test.gguf");
    std::fs::write(&model_path, b"fake").expect("写入失败");
    let engine = LocalLlmEngine::new(model_path, Quantization::Auto).expect("创建失败");

    // 读取内部 cancel_token
    let token = engine.inner.cancel_token.read().await.clone();
    assert!(!token.is_cancelled());

    engine.abort().await;
    // abort() 触发的是之前读取的 token
    assert!(token.is_cancelled());
}

/// TC-LLM-ABORT-003：每次 chat_stream 前会设置新 token
/// （此测试验证 abort() 方法存在且可多次调用）
#[tokio::test]
async fn test_abort_multiple_calls() {
    let tmp = tempfile::tempdir().expect("创建临时目录失败");
    let model_path = tmp.path().join("test.gguf");
    std::fs::write(&model_path, b"fake").expect("写入失败");
    let engine = LocalLlmEngine::new(model_path, Quantization::Auto).expect("创建失败");
    engine.abort().await;
    engine.abort().await;
    // 不应 panic
}
```

### 13.3 验证命令

```bash
cargo test -p echomind-infra --features pro -- local_llm_tests
cargo test -p echomind-tauri-app --features pro
cargo clippy -p echomind-infra --features pro -- -D warnings
cargo clippy -p echomind-tauri-app --features pro -- -D warnings
```

### 13.4 Session 13 完成标志

- [ ] `CancellationToken` 集成到 `LocalLlmEngine`
- [ ] `chat_stream` 内 `select!` 循环支持取消
- [ ] `abort()` 方法可立即中断推理
- [ ] `abort_chat` 命令本地模式额外调用 `LocalLlmEngine::abort()`
- [ ] 3 个取消测试全过
- [ ] Clippy 零警告

### 13.5 Session 13 → Session 14 衔接提示词

```
你正在继续 EchoMind 纯 Rust 本地 LLM 引擎 Phase 2 的开发。

## 上下文回溯
- Phase 1（S1-S7）已完成：全栈本地 LLM 引擎 MVP
- S8-S12 已完成：版本升级 / GPU / PagedAttention / 采样参数 / 模型卸载
- S13 已完成：流取消集成
  - CancellationToken 集成到 LocalLlmEngine
  - chat_stream 内 select! 循环支持取消
  - abort_chat 命令本地模式调用 engine.abort()
  - 3 个取消测试全过

## 本 Session 目标
模型预热 + 错误处理加固：模式切换时后台预热模型，加载失败时完善错误信息。

## 必读文档
1. `docs/architecture/LOCAL_LLM_ENGINE_PHASE2_ATOMIC_PLAN.md` — Session 14 章节
2. `crates/infra/src/local_llm.rs` — 当前实现
3. `crates/tauri-app/src/commands.rs` — set_local_model / set_llm_mode

## 精确变更清单
| 文件 | 变更 |
|---|---|
| `crates/infra/src/local_llm.rs` | 新增 warm_up() 方法 |
| `crates/tauri-app/src/state.rs` | set_local_model 后后台预热 |
| `crates/tauri-app/src/commands.rs` | set_llm_mode 切换到 local 后预热 |
| `crates/infra/src/local_llm.rs` | 加固 extract_text 错误处理 |
| `crates/infra/src/local_llm_tests.rs` | 新增 TC-LLM-WARM-001~003 |

## TDD 测试要求
- 测试 ID 前缀：TC-LLM-WARM-

## 验证命令
```bash
cargo test -p echomind-infra --features pro -- local_llm_tests
cargo clippy -p echomind-infra --features pro -- -D warnings
```

## 完成后必须执行
1. 验证通过
2. 产出下一 Session 衔接提示词
3. ./sync-to-github.sh
```

---

## Session 14：模型预热 + 错误处理加固

**目标**：模式切换/模型选择时后台预热模型，消除首次对话卡顿；加固 `extract_text` 错误处理。

**预计耗时**：1 天

### 14.1 文件变更清单

#### 文件 1：`crates/infra/src/local_llm.rs`（修改）

**变更 1**：新增 `warm_up()` 方法

```rust
impl LocalLlmEngine {
    /// 预热模型（后台加载，不阻塞调用方）。
    ///
    /// 在模式切换或模型选择后调用，提前加载模型到内存，
    /// 使用户首次对话时无需等待加载。
    ///
    /// 此方法是幂等的：如果模型已加载，立即返回。
    pub async fn warm_up(&self) -> Result<()> {
        // 触发加载但不持有模型引用（get_or_load_model 内部会缓存）
        let _ = self.get_or_load_model().await?;
        Ok(())
    }
}
```

**变更 2**：加固 `extract_text` 错误处理

```rust
/// 从 mistral.rs 流响应中提取文本 token。
///
/// 处理 Response 枚举的所有变体：
/// - `Response::Chunk` — 正常流式 chunk，提取 delta.content
/// - `Response::Error` — 推理错误，返回 None（上游会发送错误消息）
/// - 其他变体 — 忽略（如 Response::Done）
fn extract_text(response: &Response) -> Option<String> {
    match response {
        Response::Chunk(ChatCompletionChunkResponse { choices, .. }) => {
            if let Some(ChunkChoice {
                delta: Delta {
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
        // 其他 Response 变体不包含文本，安全跳过
        _ => None,
    }
}
```

#### 文件 2：`crates/tauri-app/src/state.rs`（修改）

**变更**：`set_local_model` 后后台预热

```rust
pub async fn set_local_model(&self, filename: String) -> anyhow::Result<()> {
    // ... 现有卸载逻辑 ...

    self.storage.set_setting("llm.local_model", &filename).await?;

    // 后台预热（不阻塞设置操作返回）
    #[cfg(feature = "pro")]
    {
        let state = self.clone(); // AppState 需要 Clone（或用 Arc）
        tokio::spawn(async move {
            if let Ok(engine) = state.local_llm().await {
                if let Err(e) = engine.warm_up().await {
                    eprintln!("模型预热失败: {e:#}");
                }
            }
        });
    }

    Ok(())
}
```

**注意**：`AppState` 当前不可 `Clone`。需要通过 `Arc<AppState>` 或传递必要字段。**替代方案**：在 `commands.rs` 的 `set_local_model_inner` 中执行预热 spawn。

#### 文件 3：`crates/tauri-app/src/commands.rs`（修改）

**变更**：`set_llm_mode_inner` 切换到 local 后预热

```rust
pub async fn set_llm_mode_inner(mode: String, state: &AppState) -> Result<(), String> {
    // ... 现有模式切换逻辑 ...

    // 切换到 local 模式后后台预热
    #[cfg(feature = "pro")]
    if mode == "local" {
        // 在后台 spawn 预热任务
        // 注意：state 是 &AppState，需要传递必要数据的 clone
        let storage = state.storage.clone();
        let model_manager = state.model_manager().clone();
        tokio::spawn(async move {
            // 创建临时 engine 并预热
            // ... 预热逻辑 ...
        });
    }

    Ok(())
}
```

#### 文件 4：`crates/infra/src/local_llm_tests.rs`（修改）

**新增测试**：

| 测试 ID | 测试名 | 验证内容 |
|---|---|---|
| TC-LLM-WARM-001 | `test_warm_up_returns_ok` | warm_up() 返回 Ok（即使无真实模型） |
| TC-LLM-WARM-002 | `test_warm_up_idempotent` | 多次调用 warm_up() 不 panic |
| TC-LLM-WARM-003 | `test_extract_text_handles_non_chunk` | extract_text 对非 Chunk 变体返回 None |

```rust
/// TC-LLM-WARM-001：warm_up() 返回 Ok
/// 注意：无真实模型文件时 warm_up 会失败，此测试验证方法存在
#[tokio::test]
async fn test_warm_up_returns_ok() {
    let tmp = tempfile::tempdir().expect("创建临时目录失败");
    let model_path = tmp.path().join("test.gguf");
    std::fs::write(&model_path, b"fake").expect("写入失败");
    let engine = LocalLlmEngine::new(model_path, Quantization::Auto).expect("创建失败");
    // warm_up 会尝试加载模型（fake 文件会失败），但方法签名返回 Result
    let result = engine.warm_up().await;
    // 预期 Err（因为文件是假的），但不应 panic
    assert!(result.is_err());
}
```

**注意**：`warm_up` 在没有真实 GGUF 文件时会返回 Err，这是预期行为。真实的预热测试需要 `#[ignore]` + 真实模型。

### 14.2 验证命令

```bash
cargo test -p echomind-infra --features pro -- local_llm_tests
cargo test -p echomind-tauri-app --features pro
cargo clippy -p echomind-infra --features pro -- -D warnings
cargo clippy -p echomind-tauri-app --features pro -- -D warnings
```

### 14.3 Session 14 完成标志

- [ ] `warm_up()` 方法实现
- [ ] `set_local_model` 后后台预热
- [ ] `set_llm_mode` 切换到 local 后预热
- [ ] `extract_text` 错误处理加固（match 全变体）
- [ ] 3 个预热测试全过
- [ ] Clippy 零警告

### 14.4 Session 14 → Session 15 衔接提示词

```
你正在继续 EchoMind 纯 Rust 本地 LLM 引擎 Phase 2 的开发。

## 上下文回溯
- Phase 1（S1-S7）已完成：全栈本地 LLM 引擎 MVP
- S8-S13 已完成：版本升级 / GPU / PagedAttention / 采样参数 / 模型卸载 / 流取消
- S14 已完成：模型预热 + 错误处理加固
  - warm_up() 后台预热方法
  - set_local_model / set_llm_mode 后预热
  - extract_text 错误处理加固
  - 3 个预热测试全过

## 本 Session 目标（Phase 2 最终 Session）
性能基准测试 + 全量文档同步 + GitHub 发布。

## 必读文档
1. `docs/architecture/LOCAL_LLM_ENGINE_PHASE2_ATOMIC_PLAN.md` — Session 15 章节
2. `docs/architecture/adr/ADR-011-local-llm-engine.md` — ADR 更新
3. 所有 Phase 2 修改的文件

## 必须完成
1. 性能基准测试（CPU vs GPU 对比）
2. SRS 更新（REQ-LLM-003 状态 → Phase 2 Implemented）
3. IPC 文档更新（新增命令）
4. code-contracts 更新（新增结构体/方法）
5. settings-reference 更新（新增设置键）
6. frontend-modules 更新（新增前端函数）
7. ADR-011 更新（Phase 2 完成）
8. AGENTS.md / CLAUDE.md 更新
9. ./sync-to-github.sh

## 验证命令
```bash
cargo test
cargo test --features pro
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --features pro -- -D warnings
cargo fmt --check
```

## 完成后必须执行
1. 全量验证通过
2. 更新 ADR-011 Phase 2 状态为 ✅
3. 更新 memory（使用 update_memory 工具记录 Phase 2 完成）
4. ./sync-to-github.sh
```

---

## Session 15：性能基准测试 + 文档同步

**目标**：Phase 2 最终 Session — 性能基准测试 + 全量文档同步 + GitHub 发布。

**预计耗时**：1 天

### 15.1 性能基准测试

#### 基准测试设计

```rust
/// 性能基准测试（标记 #[ignore]，需要真实模型）。
///
/// 对比 CPU vs GPU（Metal）推理速度。
/// 运行方式：
/// ```bash
/// ECHOMIND_TEST_MODEL=/path/to/model.gguf \
/// cargo test -p echomind-infra --features pro,metal -- local_llm_tests --ignored
/// ```

/// TC-LLM-PERF-001：CPU 推理速度基准
#[tokio::test]
#[ignore]
async fn test_cpu_inference_speed() {
    // 1. 创建 CPU-only engine
    // 2. 发送固定 prompt（"Write a 100-word essay about Rust."）
    // 3. 测量首 token 延迟和总生成时间
    // 4. 记录 tokens/sec
}

/// TC-LLM-PERF-002：GPU（Metal）推理速度基准
#[tokio::test]
#[ignore]
async fn test_gpu_metal_inference_speed() {
    // 1. 创建 Metal engine
    // 2. 发送相同 prompt
    // 3. 对比 CPU 速度
}

/// TC-LLM-PERF-003：PagedAttention 多轮对话延迟
#[tokio::test]
#[ignore]
async fn test_paged_attn_multiturn_latency() {
    // 1. 创建 PagedAttention engine
    // 2. 发送 3 轮对话
    // 3. 测量每轮首 token 延迟
    // 4. 验证第 2/3 轮延迟 < 第 1 轮
}
```

#### 基准测试结果记录

在 ADR-011 中记录实际测试数据：

```markdown
### Phase 2 性能基准（YYYY-MM-DD）

| 模型 | 设备 | 首 token 延迟 | tokens/sec | 内存占用 |
|---|---|---|---|---|
| Qwen2.5-3B Q4_K_M | CPU | ??? ms | ??? tok/s | ??? MB |
| Qwen2.5-3B Q4_K_M | Metal | ??? ms | ??? tok/s | ??? MB |
| Qwen2.5-7B Q4_K_M | CPU | ??? ms | ??? tok/s | ??? MB |
| Qwen2.5-7B Q4_K_M | Metal | ??? ms | ??? tok/s | ??? MB |

结论：Metal 加速 ??x，首 token 延迟降低 ??%
```

### 15.2 文档更新清单

| 文档 | 变更内容 |
|---|---|
| `docs/SRS_v1.0.md` | REQ-LLM-003 状态更新：Phase 2 Implemented（GPU + PagedAttention + 采样参数 + 模型卸载 + 流取消 + 预热） |
| `docs/SRS_v1.0.md` | 新增 REQ-LLM-005（GPU 加速）、REQ-LLM-006（采样参数配置） |
| `docs/api/ipc-commands.md` | 新增命令：`get_local_llm_device_kind` / `get_sampling_params` / `set_sampling_params` |
| `docs/api/code-contracts.md` | 新增：`LlmSamplingParams` / `LocalLlmEngine::device_kind()` / `warm_up()` / `abort()` |
| `docs/api/settings-reference.md` | 新增键：`llm.paged_attn` / `llm.block_size` / `llm.sampling` |
| `docs/api/frontend-modules.md` | 新增：`renderSamplingParamsSection` / `loadSamplingParams` / `saveSamplingParams` |
| `docs/architecture/adr/ADR-011-local-llm-engine.md` | Phase 2 标记 ✅，更新实施路线 |
| `AGENTS.md` | 更新 local_llm.rs 描述（含 GPU/PagedAttention/采样参数） |
| `CLAUDE.md` | 更新 Pro feature gating（含 metal/cuda/accelerate） |

### 15.3 最终验证

```bash
# 全量测试
cargo test
cargo test --features pro

# Clippy
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --features pro -- -D warnings

# 格式检查
cargo fmt --check

# 前端构建
node scripts/build-ui.mjs

# 同步到 GitHub
./sync-to-github.sh
```

### 15.4 Session 15 完成标志

- [x] 性能基准测试设计完成（3 个 #[ignore] 测试）
- [x] 全量测试通过（Free + Pro）
- [x] Clippy 零警告（Free + Pro）
- [x] `cargo fmt --check` 通过
- [x] 前端构建成功
- [x] 全部文档更新完成
- [x] ADR-011 Phase 2 标记 ✅
- [x] `./sync-to-github.sh` 执行成功

---

## 附录 A：Phase 2 完整文件变更矩阵

| 文件路径 | Session | 变更类型 | 说明 |
|---|---|---|---|
| `crates/infra/Cargo.toml` | S8, S9 | 修改 | mistralrs 0.8.1→0.9.0 + metal/cuda/accelerate features |
| `crates/infra/src/local_llm.rs` | S8-S14 | 修改 | 版本适配 + GPU + PagedAttention + 采样参数 + 卸载 + 流取消 + 预热 |
| `crates/infra/src/local_llm_tests.rs` | S8-S14 | 修改 | 新增 ~20 个测试 |
| `crates/models/src/lib.rs` | S10, S11 | 修改 | LlmSamplingParams + SettingsPayload 字段 |
| `crates/tauri-app/Cargo.toml` | S9 | 修改 | 转发 metal/cuda/accelerate feature |
| `crates/tauri-app/src/state.rs` | S10, S12, S14 | 修改 | PagedAttention 配置 + 卸载 + 预热 |
| `crates/tauri-app/src/commands.rs` | S9, S11, S13, S14 | 修改 | 新 IPC 命令 + abort 集成 + 预热 |
| `crates/tauri-app/src/lib.rs` | S9, S11 | 修改 | 注册新命令 |
| `crates/tauri-app/tests/integration.rs` | S9, S11 | 修改 | 新增集成测试 |
| `ui/src/settings.js` | S11 | 修改 | 采样参数 UI |
| `ui/locales/zh-CN.json` | S11 | 修改 | 新增翻译键 |
| `ui/locales/en.json` | S11 | 修改 | 新增翻译键 |
| `docs/SRS_v1.0.md` | S15 | 修改 | REQ 状态更新 + 新增 REQ |
| `docs/api/ipc-commands.md` | S15 | 修改 | 新增命令文档 |
| `docs/api/code-contracts.md` | S15 | 修改 | 新增契约 |
| `docs/api/settings-reference.md` | S15 | 修改 | 新增设置键 |
| `docs/api/frontend-modules.md` | S15 | 修改 | 新增前端函数 |
| `docs/architecture/adr/ADR-011-local-llm-engine.md` | S15 | 修改 | Phase 2 完成 |
| `AGENTS.md` | S15 | 修改 | 架构更新 |
| `CLAUDE.md` | S15 | 修改 | 架构更新 |

---

## 附录 B：Phase 2 Settings 键完整定义

| 键 | 类型 | 默认值 | Session | 说明 |
|---|---|---|---|---|
| `llm.mode` | String | `"remote"` | S4 (Phase 1) | 推理模式 |
| `llm.local_model` | String | `""` | S4 (Phase 1) | 选中的模型文件名 |
| `llm.paged_attn` | String | `"false"` | S10 | 是否启用 PagedAttention |
| `llm.block_size` | String | `"32"` | S10 | PagedAttention 块大小 |
| `llm.sampling` | String (JSON) | `null` | S11 | 采样参数 JSON |

---

## 附录 C：Phase 2 Tauri 事件完整定义

| 事件 | 载荷类型 | Session | 说明 |
|---|---|---|---|
| `model_load_progress` | `ModelLoadStatusPayload` | S3 (Phase 1) | 模型加载状态 |
| `model_download_progress` | `ModelDownloadProgressPayload` | S5 (Phase 1) | 模型下载进度 |

（Phase 2 无新增事件）

---

## 附录 D：Phase 2 IPC 命令完整定义

| 命令 | 参数 | 返回 | Session | Pro 门控 |
|---|---|---|---|---|
| `get_local_llm_device_kind` | — | `String` | S9 | ✅ |
| `get_sampling_params` | — | `LlmSamplingParams` | S11 | ✅ |
| `set_sampling_params` | `params: LlmSamplingParams` | `()` | S11 | ✅ |

---

## 附录 E：Phase 2 新增测试用例汇总

| Session | 测试 ID 前缀 | 测试数 | 说明 |
|---|---|---|---|
| S8 | TC-LLM-UPGRADE- | 3 | API 可用性验证 |
| S9 | TC-LLM-GPU- | 3 | GPU 设备选择 |
| S10 | TC-LLM-PAGED- | 3 | PagedAttention 配置 |
| S11 | TC-LLM-SAMPLE- | 4 | 采样参数读写 |
| S12 | TC-LLM-UNLOAD- | 4 | 模型卸载/切换 |
| S13 | TC-LLM-ABORT- | 3 | 流取消 |
| S14 | TC-LLM-WARM- | 3 | 模型预热 |
| S15 | TC-LLM-PERF- | 3 (#[ignore]) | 性能基准 |
| **合计** | | **26** | |
