# EchoMind 纯 Rust 本地 LLM 引擎 Phase 3 — 原子级实施规划

> **状态**：Active ｜ **创建日期**：2026-07-31 ｜ **父文档**：`ADR-011-local-llm-engine.md`
>
> 本文档将 Phase 3（自研量化内核 + 内存层次流式加载）拆分为 **8 个 Session（S16-S23）**，每个 Session 包含：
> - 精确到文件、函数、行号的变更清单
> - TDD 测试用例（先红灯后绿灯）
> - Session 完成验证命令
> - 下一 Session 衔接提示词
>
> **铁律**：每个 Session 结束时必须产出衔接提示词，确保下一 Session 无缝续接。

---

## 前置调研结果（2026-07-31 执行）

### 调研来源

| 来源 | URL | 调研内容 |
|---|---|---|
| mistral.rs v0.9.0 源码 | `github.com/EricLBuehler/mistral.rs` tag v0.9.0 | GGUF 加载、量化 crate 结构、PagedAttention、device_map |
| mistralrs-quant crate | `mistralrs-quant/src/gguf/mod.rs`、`cpu.rs` | GgufMatMul、QuantMethod trait、CPU 量化路径 |
| candle-core 量化模块 | `huggingface/candle` main 分支 `candle-core/src/quantized/` | BlockQ4K/BlockQ8_0、AVX/NEON SIMD 内核、QTensor/QMatMul、Cow<[u8]> |
| llama.cpp ggml CPU | `ggml-org/llama.cpp` `ggml/src/ggml-cpu/` | quants.c/quants.h、repack.cpp、llamafile |
| Rust 量化项目调研 | GitHub API 搜索 "quantization rust llm" | herbert-rs（AVX-512 手写）、pulsar（SSD streaming MoE）、lattice（Apple Silicon 纯 Rust） |
| memmap2 crate | `mistralrs-quant/Cargo.toml` 确认 memmap2 依赖 | mistral.rs 已使用 memmap2 |

### 现有架构分析

#### 量化路径（Phase 1/2 现状）

```
GGUF 文件
  → candle-core::quantized::gguf_file::Content::read()  — 解析文件头 + 元数据
  → QTensor { storage: QStorage::Cpu(Cow<[u8]>), shape, dtype: GgmlDType }
  → QMatMul — 包装 QTensor 提供矩阵乘法
  → mistralrs_quant::GgufMatMul — 实现 QuantMethod trait
  → mistral.rs 模型层（quantized_qwen.rs 等）调用 GgufMatMul.forward()
  → candle-core SIMD 内核（avx.rs/neon.rs/simd128.rs）执行点积
```

**关键发现**：
1. `candle-core` 已有 AVX2/NEON/SIMD128 内核，但仅实现**块级点积**（`vec_dot_q4_0_q8_0` 等），非完整 GEMV/GEMM
2. `mistralrs_quant::GgufMatMul` 的 `forward_raw()` 委托给 `QMatMul::forward()`，后者调用 candle 的通用量化 matmul
3. CPU 路径无自定义 GEMV 内核——candle 使用通用 dequantize+matmul 路径
4. **单批次推理（batch=1，即 chat 场景）** 时 GEMV 远优于 GEMM，但 candle 未优化此路径

#### 内存加载（Phase 1/2 现状）

1. `mistralrs-quant` Cargo.toml 已含 `memmap2` 依赖
2. `candle-core::quantized::QTensor` 使用 `Cow<[u8]>` 存储数据（支持零拷贝 mmap）
3. 但 **无显式内存层次管理**：
   - 无 layer 级预取（prefetch）
   - 无 `madvise()` 提示（MADV_WILLNEED / MADV_SEQUENTIAL）
   - 无 RAM 预算管理
   - 无磁盘溢出（模型 > RAM 时无法运行）

### Phase 3 目标与范围

| 目标 | 价值 | 风险 |
|---|---|---|
| **自研 GEMV 内核**（Q4_K_M / Q8_0 单批次推理优化） | 单批次推理 2-5x 加速 | 中（需深入 SIMD） |
| **CPU cache 友好权重重排** | 减少 cache miss，10-30% 加速 | 低（数据重排，不改变语义） |
| **Layer 级流式预取** | 消除大模型加载等待 | 低（mmap + madvise） |
| **RAM 预算管理** | 支持模型 > RAM 场景 | 中（需要驱逐 + 重载逻辑） |
| **性能基准对比** | 量化改进效果 | 低 |

**不包含**（Phase 4 范围）：
- MoE 专家缓存
- 推测解码（Speculative Decoding）
- KV cache 持久化

---

## 全局依赖关系图

```
S16 (GGUF 解析器) ──→ S17 (量化块结构) ──→ S18 (GEMV 内核)
                                              │
                                              ↓
                    S19 (权重重排) ──→ S20 (集成 LocalLlmEngine)
                                          │
                                          ↓
                    S21 (流式预取) ──→ S22 (RAM 预算) ──→ S23 (基准测试 + 文档)
```

**依赖关系说明**：
- S16 是基础：自研 GGUF 文件解析，不依赖 candle 的解析器
- S17 依赖 S16：量化块结构需要从 GGUF tensor 数据中提取
- S18 依赖 S17：GEMV 内核操作量化块结构
- S19 依赖 S18：权重重排需要理解量化块布局
- S20 依赖 S18/S19：将自研内核集成到 LocalLlmEngine
- S21 依赖 S20：流式预取需要引擎层的 layer 抽象
- S22 依赖 S21：RAM 预算管理基于流式加载机制
- S23 依赖所有前序 Session

---

## Session 16：自研 GGUF 文件解析器

**目标**：实现 `crates/infra/src/gguf_reader.rs`，不依赖 candle-core 独立解析 GGUF 文件格式。

**预计耗时**：2 天

### 16.1 文件变更清单

#### 文件 1：`crates/infra/Cargo.toml`（修改）

**变更**：新增 `memmap2` 依赖（Pro 门控）

```toml
[features]
pro = [..., "dep:memmap2"]

# GGUF 文件 mmap 零拷贝读取（Phase 3）
memmap2 = { version = "0.9", optional = true }
```

#### 文件 2：`crates/infra/src/lib.rs`（修改）

**变更**：新增模块声明（Pro 门控）

```rust
/// GGUF 文件解析器（Phase 3：自研量化内核 + 内存层次流式加载）。
///
/// 独立于 candle-core 的 GGUF 格式解析器，支持 mmap 零拷贝读取。
#[cfg(feature = "pro")]
pub mod gguf_reader;
```

#### 文件 3：`crates/infra/src/gguf_reader.rs`（新建）

**完整函数签名清单**：

| 函数 | 签名 | 职责 |
|---|---|---|
| `GgufFile::open` | `pub fn open(path: &Path) -> Result<GgufFile>` | 打开 GGUF 文件，mmap 映射，解析文件头 |
| `GgufFile::metadata` | `pub fn metadata(&self) -> &HashMap<String, GgufValue>` | 返回所有元数据键值对 |
| `GgufFile::tensor_count` | `pub fn tensor_count(&self) -> usize` | 返回张量数量 |
| `GgufFile::tensor_info` | `pub fn tensor_info(&self, name: &str) -> Option<&TensorInfo>` | 返回指定张量的元信息 |
| `GgufFile::tensor_data` | `pub fn tensor_data(&self, name: &str) -> Option<&[u8]>` | 返回张量原始数据（零拷贝 mmap 切片） |
| `GgufFile::tensor_names` | `pub fn tensor_names(&self) -> impl Iterator<Item = &str>` | 返回所有张量名迭代器 |
| `GgufFile::architecture` | `pub fn architecture(&self) -> Result<&str>` | 从元数据提取模型架构名 |
| `GgufFile::close` | `pub fn close(self)` | 释放 mmap 映射 |

**类型定义**：

```rust
/// GGUF 文件值类型（对应 GGUF 规范的元数据值类型）。
#[derive(Debug, Clone)]
pub enum GgufValue {
    String(String),
    Uint8(u8),
    Uint16(u16),
    Uint32(u32),
    Uint64(u64),
    Int8(i8),
    Int16(i16),
    Int32(i32),
    Int64(i64),
    Float32(f32),
    Float64(f64),
    Bool(bool),
    Array(Vec<GgufValue>),
}

/// 张量元信息。
#[derive(Debug, Clone)]
pub struct TensorInfo {
    /// 张量名称（如 `token_embd.weight`）
    pub name: String,
    /// 维度列表（倒序，GGUF 使用 col-major）
    pub dims: Vec<u64>,
    /// 量化数据类型（GGML 量化格式枚举）
    pub dtype: GgmlDType,
    /// 数据在文件中的偏移量
    pub offset: u64,
    /// 数据长度（字节）
    pub size: u64,
}

/// GGML 量化数据类型枚举（与 llama.cpp/candle 对齐）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GgmlDType {
    F32, F16, BF16,
    Q4_0, Q4_1, Q5_0, Q5_1, Q8_0, Q8_1,
    Q2K, Q3K, Q4K, Q5K, Q6K, Q8K,
    Unknown(u32),
}

/// mmap 映射的 GGUF 文件。
pub struct GgufFile {
    /// mmap 映射区域
    mmap: memmap2::Mmap,
    /// 元数据
    metadata: HashMap<String, GgufValue>,
    /// 张量信息列表
    tensors: Vec<TensorInfo>,
    /// 张量名→索引映射（快速查找）
    tensor_map: HashMap<String, usize>,
    /// 数据区起始偏移（张量数据相对于文件头）
    data_offset: u64,
}
```

### 16.2 TDD 测试用例

#### 文件：`crates/infra/src/gguf_reader_tests.rs`（新建，Pro 门控）

| 测试 ID | 测试名 | 验证内容 | 对应 AC |
|---|---|---|---|
| TC-GGUF-001 | `test_open_valid_gguf` | 打开合法 GGUF 文件返回 Ok | — |
| TC-GGUF-002 | `test_open_nonexistent_fails` | 不存在的文件返回 Err | — |
| TC-GGUF-003 | `test_open_invalid_magic_fails` | 非 GGUF 文件（magic 不匹配）返回 Err | — |
| TC-GGUF-004 | `test_metadata_contains_architecture` | 元数据包含 `general.architecture` 键 | — |
| TC-GGUF-005 | `test_metadata_string_value` | 字符串值正确解析 | — |
| TC-GGUF-006 | `test_metadata_uint32_value` | uint32 值正确解析 | — |
| TC-GGUF-007 | `test_metadata_array_value` | 数组值正确解析 | — |
| TC-GGUF-008 | `test_tensor_count_positive` | 张量数量 > 0 | — |
| TC-GGUF-009 | `test_tensor_info_returns_correct_data` | tensor_info 返回正确的 dims/dtype/offset | — |
| TC-GGUF-010 | `test_tensor_info_nonexistent_returns_none` | 不存在的张量名返回 None | — |
| TC-GGUF-011 | `test_tensor_data_zero_copy` | tensor_data 返回的切片长度与 TensorInfo.size 一致 | — |
| TC-GGUF-012 | `test_tensor_names_contains_token_embd` | 张量名列表包含 `token_embd.weight` | — |
| TC-GGUF-013 | `test_architecture_returns_qwen` | Qwen2.5 模型返回 `"qwen2"` | — |
| TC-GGUF-014 | `test_ggml_dtype_from_value` | GgmlDType 枚举值与 GGUF 规范一致 | — |

**注册测试模块**（`crates/infra/src/lib.rs`）：

```rust
#[cfg(all(test, feature = "pro"))]
mod gguf_reader_tests;
```

### 16.3 关键实现细节

1. **mmap 零拷贝**：
   - 使用 `memmap2::Mmap` 映射整个 GGUF 文件
   - `tensor_data()` 返回 `&[u8]` 切片，直接指向 mmap 区域，无数据拷贝
   - OS 按需分页（page fault 时从磁盘加载）

2. **GGUF 格式解析**：
   - Magic: `0x46554747`（"GGUF"）
   - Version: 1/2/3（影响长度前缀大小，V1=4字节，V2/V3=8字节）
   - 元数据区：key-value 对列表（key=字符串，value=类型+值）
   - 张量信息区：TensorInfo 数组
   - 对齐填充：32 字节对齐
   - 数据区：张量权重数据

3. **安全考量**：
   - 所有偏移量使用 `checked_add` 防溢出（铁律五）
   - 文件大小校验：所有 offset+size 不超过文件总大小
   - 字符串长度上限：防止恶意文件分配超大内存

### 16.4 验证命令

```bash
# 需要真实 GGUF 文件（标记 #[ignore] 的测试使用 ECHOMIND_TEST_MODEL 环境变量）
ECHOMIND_TEST_MODEL=/path/to/model.gguf \
cargo test -p echomind-infra --features pro -- gguf_reader_tests --ignored --nocapture

# 不需要真实文件的单元测试
cargo test -p echomind-infra --features pro -- gguf_reader_tests
cargo clippy -p echomind-infra --features pro -- -D warnings
```

### 16.5 Session 16 完成标志

- [x] `GgufFile` / `GgufValue` / `TensorInfo` / `GgmlDType` 定义完成
- [x] mmap 零拷贝读取实现完成
- [x] 14 个测试全部通过（需真实文件的标记 `#[ignore]`）
- [x] clippy 零警告

### 16.6 Session 16 → Session 17 衔接提示词

```
你正在继续 EchoMind 本地 LLM 引擎 Phase 3 的开发。

## 上下文
- Phase 1 完成：mistral.rs 集成，基础 CPU 推理
- Phase 2 完成：GPU 加速、PagedAttention、采样参数、模型卸载、流取消、预热
- Phase 3 Session 16 完成：自研 GGUF 文件解析器
  - crates/infra/src/gguf_reader.rs 已实现 GgufFile（mmap 零拷贝）
  - 支持 GGUF V1/V2/V3 格式
  - GgufValue/TensorInfo/GgmlDType 类型定义
  - 14 个测试通过，clippy 零警告

## 本 Session 目标
实现量化块结构（BlockQ4K/BlockQ8_0 等），从 GGUF tensor 数据中提取量化权重块。

## 必读文档
1. docs/architecture/LOCAL_LLM_ENGINE_PHASE3_ATOMIC_PLAN.md — Session 17 章节
2. crates/infra/src/gguf_reader.rs — 已完成的 GGUF 解析器
3. candle-core/src/quantized/k_quants.rs — 参考实现（Block 结构体定义）
4. llama.cpp ggml-common.h — GGML 量化块格式规范

## TDD 纪律
先写测试（红灯），再写实现（绿灯）。

## 完成后
1. 运行验证命令确认通过
2. 产出下一 Session 衔接提示词
3. 运行 ./sync-to-github.sh 同步
```

---

## Session 17：量化块结构与反量化

**目标**：实现 `crates/infra/src/quant_blocks.rs`，定义 GGUF 量化块结构体并提供反量化（dequantize）方法。

**预计耗时**：2 天

### 17.1 文件变更清单

#### 文件 1：`crates/infra/src/lib.rs`（修改）

**变更**：新增模块声明（Pro 门控）

```rust
/// 量化块结构与反量化内核（Phase 3）。
#[cfg(feature = "pro")]
pub mod quant_blocks;
```

#### 文件 2：`crates/infra/src/quant_blocks.rs`（新建）

**完整函数/结构体签名清单**：

| 项 | 签名 | 职责 |
|---|---|---|
| `BlockQ4_0` | `pub struct BlockQ4_0 { d: f16, qs: [u8; 16] }` | Q4_0 块（32 元素，18 字节） |
| `BlockQ4K` | `pub struct BlockQ4K { d: f16, dmin: f16, scales: [u8; 12], qs: [u8; 128] }` | Q4_K 块（256 元素，144 字节） |
| `BlockQ8_0` | `pub struct BlockQ8_0 { d: f16, qs: [i8; 32] }` | Q8_0 块（32 元素，34 字节） |
| `BlockQ8K` | `pub struct BlockQ8K { d: f16, qs: [i8; 256] }` | Q8_K 块（256 元素，258 字节） |
| `BlockQ5K` | `pub struct BlockQ5K { d: f16, dmin: f16, scales: [u8; 12], qh: [u8; 32], qs: [u8; 64] }` | Q5_K 块（256 元素） |
| `BlockQ6K` | `pub struct BlockQ6K { ql: [u8; 128], qh: [u8; 64], scales: [i8; 16], d: f16 }` | Q6_K 块（256 元素） |
| `QuantBlock` | `pub enum QuantBlock<'a>` | 借用引用量化块的枚举 |
| `dequantize_q4_0` | `pub fn dequantize_q4_0(blocks: &[BlockQ4_0], out: &mut [f32])` | Q4_0 → f32 反量化 |
| `dequantize_q4_k` | `pub fn dequantize_q4_k(blocks: &[BlockQ4K], out: &mut [f32])` | Q4_K → f32 反量化 |
| `dequantize_q8_0` | `pub fn dequantize_q8_0(blocks: &[BlockQ8_0], out: &mut [f32])` | Q8_0 → f32 反量化 |
| `dequantize_q8_k` | `pub fn dequantize_q8_k(blocks: &[BlockQ8K], out: &mut [f32])` | Q8_K → f32 反量化 |
| `dequantize_q5_k` | `pub fn dequantize_q5_k(blocks: &[BlockQ5K], out: &mut [f32])` | Q5_K → f32 反量化 |
| `dequantize_q6_k` | `pub fn dequantize_q6_k(blocks: &[BlockQ6K], out: &mut [f32])` | Q6_K → f32 反量化 |
| `blocks_from_bytes` | `pub fn blocks_from_bytes(data: &[u8], dtype: GgmlDType) -> Result<QuantBlock<'_>>` | 从原始字节切片解析量化块 |
| `block_count` | `pub fn block_count(dtype: GgmlDType, elem_count: usize) -> usize` | 根据元素数计算块数 |

**关键设计**：

```rust
/// 借用引用的量化块枚举（零拷贝，直接引用 mmap 区域）。
#[derive(Debug, Clone, Copy)]
pub enum QuantBlock<'a> {
    Q4_0(&'a [BlockQ4_0]),
    Q4K(&'a [BlockQ4K]),
    Q5K(&'a [BlockQ5K]),
    Q6K(&'a [BlockQ6K]),
    Q8_0(&'a [BlockQ8_0]),
    Q8K(&'a [BlockQ8K]),
}

impl<'a> QuantBlock<'a> {
    /// 将此量化块反量化到 f32 输出缓冲区。
    pub fn dequantize(&self, out: &mut [f32]) {
        match self {
            QuantBlock::Q4_0(blocks) => dequantize_q4_0(blocks, out),
            QuantBlock::Q4K(blocks) => dequantize_q4_k(blocks, out),
            QuantBlock::Q5K(blocks) => dequantize_q5_k(blocks, out),
            QuantBlock::Q6K(blocks) => dequantize_q6_k(blocks, out),
            QuantBlock::Q8_0(blocks) => dequantize_q8_0(blocks, out),
            QuantBlock::Q8K(blocks) => dequantize_q8_k(blocks, out),
        }
    }

    /// 返回此量化块包含的原始元素数。
    pub fn elem_count(&self) -> usize {
        match self {
            QuantBlock::Q4_0(b) => b.len() * QK4_0,
            QuantBlock::Q4K(b) => b.len() * QK_K,
            QuantBlock::Q5K(b) => b.len() * QK_K,
            QuantBlock::Q6K(b) => b.len() * QK_K,
            QuantBlock::Q8_0(b) => b.len() * QK8_0,
            QuantBlock::Q8K(b) => b.len() * QK_K,
        }
    }
}
```

### 17.2 TDD 测试用例

#### 文件：`crates/infra/src/quant_blocks_tests.rs`（新建，Pro 门控）

| 测试 ID | 测试名 | 验证内容 |
|---|---|---|
| TC-QUANT-001 | `test_block_q4_0_size` | BlockQ4_0 大小为 18 字节 |
| TC-QUANT-002 | `test_block_q8_0_size` | BlockQ8_0 大小为 34 字节 |
| TC-QUANT-003 | `test_block_q4k_size` | BlockQ4K 大小为 144 字节 |
| TC-QUANT-004 | `test_dequantize_q4_0_all_zeros` | 全零块反量化为全 0.0 |
| TC-QUANT-005 | `test_dequantize_q8_0_all_zeros` | 全零块反量化为全 0.0 |
| TC-QUANT-006 | `test_dequantize_q4_0_known_values` | 已知值块反量化结果正确（手算对照） |
| TC-QUANT-007 | `test_dequantize_q8_0_known_values` | 已知值块反量化结果正确 |
| TC-QUANT-008 | `test_dequantize_q4k_known_values` | Q4_K 已知值反量化正确 |
| TC-QUANT-009 | `test_blocks_from_bytes_q4_0` | 字节切片正确解析为 Q4_0 块 |
| TC-QUANT-010 | `test_blocks_from_bytes_q8_0` | 字节切片正确解析为 Q8_0 块 |
| TC-QUANT-011 | `test_blocks_from_bytes_invalid_alignment` | 未对齐字节返回 Err |
| TC-QUANT-012 | `test_block_count_q4_0` | 256 元素 → 8 个 Q4_0 块 |
| TC-QUANT-013 | `test_block_count_q4k` | 256 元素 → 1 个 Q4_K 块 |
| TC-QUANT-014 | `test_dequantize_output_length` | 反量化输出长度 == elem_count |

### 17.3 关键实现细节

1. **`#[repr(C)]` 保证内存布局**：所有 Block 结构体使用 `#[repr(C)]` 确保 mmap 数据可直接 `from_raw_parts` 转换
2. **块大小常量**：`QK4_0 = 32`, `QK8_0 = 32`, `QK_K = 256`（与 llama.cpp/candle 一致）
3. **反量化算法**：
   - Q4_0: `y = d * (q - 8)` （q 为 4-bit nibble）
   - Q8_0: `y = d * q` （q 为 8-bit int8）
   - Q4_K: 6-bit scales + 4-bit values，super-block 级缩放
   - Q8_K: 简单缩放（`y = d * q`），但块大小 256
4. **安全**：`blocks_from_bytes` 使用 `bytemuck::cast_slice` 或手写对齐检查

### 17.4 验证命令

```bash
cargo test -p echomind-infra --features pro -- quant_blocks_tests
cargo clippy -p echomind-infra --features pro -- -D warnings
```

### 17.5 Session 17 完成标志

- [x] `BlockQ4_0` / `BlockQ4K` / `BlockQ5K` / `BlockQ6K` / `BlockQ8_0` / `BlockQ8K` 结构体定义完成（`#[repr(C)]` + `const` 大小断言）
- [x] `QuantBlock<'a>` 借用引用枚举 + `dequantize()` / `elem_count()` 方法完成
- [x] `get_scale_min_k4` 6-bit scale/min 提取辅助函数完成
- [x] `blocks_from_bytes` 零拷贝字节→块解析函数完成（手写对齐检查）
- [x] `block_count` 块数计算函数完成
- [x] 6 个反量化函数全部实现（Q4_0/Q4_K/Q5_K/Q6_K/Q8_0/Q8_K）
- [x] 14 个测试全部通过
- [x] clippy 零警告

**实现详情**（2026-07-31 完成）：
- 新增 `half` crate 作为 Pro 依赖（f16 半精度浮点类型）
- `cast_blocks<T>` 泛型辅助函数：手写对齐检查 + `unsafe from_raw_parts`（不依赖 bytemuck）
- Block 结构体字段与 candle-core `k_quants.rs` 完全对齐（含 Q8K 的 `d: f32` + `bsums: [i16; 16]`）
- 反量化算法从 candle-core `to_float()` 直接移植，含 `get_scale_min_k4` 6-bit scale/min 编码解析
- 测试覆盖：大小断言、全零块、已知值手算对照、字节解析、对齐错误、块数计算、输出长度

### 17.6 Session 17 → Session 18 衔接提示词

```
你正在继续 EchoMind 本地 LLM 引擎 Phase 3 的开发。

## 上下文
- Phase 3 Session 16 完成：GGUF 文件解析器（mmap 零拷贝）
- Phase 3 Session 17 完成：量化块结构与反量化
  - BlockQ4_0/BlockQ4K/BlockQ5K/BlockQ6K/BlockQ8_0/BlockQ8K 结构体
  - QuantBlock 枚举（借用引用，零拷贝）
  - dequantize_q4_0/q4_k/q5_k/q6_k/q8_0/q8_k 函数
  - blocks_from_bytes 从 mmap 数据解析量化块
  - 14 个测试通过，clippy 零警告

## 本 Session 目标
实现自研 GEMV（矩阵-向量乘法）内核，针对单批次推理（batch=1）优化 Q4_K_M 和 Q8_0 格式。

## 必读文档
1. docs/architecture/LOCAL_LLM_ENGINE_PHASE3_ATOMIC_PLAN.md — Session 18 章节
2. crates/infra/src/quant_blocks.rs — 已完成的量化块结构
3. candle-core/src/quantized/avx.rs — AVX2 参考实现
4. llama.cpp ggml-cpu/quants.c — GEMV 内核参考

## 关键约束
- 优先支持 Q4_K_M（最常用）和 Q8_0（最高质量）
- 单批次推理（batch=1）是主要场景
- 使用 std::arch::x86_64 AVX2 intrinsics（ARM NEON 后续支持）

## TDD 纪律
先写测试（红灯），再写实现（绿灯）。

## 完成后
1. 运行验证命令确认通过
2. 产出下一 Session 衔接提示词
3. 运行 ./sync-to-github.sh 同步
```

---

## Session 18：自研 GEMV 内核

**目标**：实现 `crates/infra/src/gemv_kernel.rs`，提供 Q4_K_M 和 Q8_0 格式的 GEMV（矩阵-向量乘法）内核，使用 AVX2 SIMD 指令优化单批次推理。

**预计耗时**：3 天

### 18.1 文件变更清单

#### 文件 1：`crates/infra/src/lib.rs`（修改）

**变更**：新增模块声明（Pro 门控）

```rust
/// 自研 GEMV 内核（Phase 3）：单批次推理优化的量化矩阵-向量乘法。
#[cfg(feature = "pro")]
pub mod gemv_kernel;
```

#### 文件 2：`crates/infra/src/gemv_kernel.rs`（新建）

**完整函数签名清单**：

| 函数 | 签名 | 职责 |
|---|---|---|
| `gemv_q4_k` | `pub fn gemv_q4_k(weights: &[BlockQ4K], input: &[f32], output: &mut [f32], n: usize, k: usize)` | Q4_K GEMV: output[m,n] = weights[n,k] @ input[k] |
| `gemv_q8_0` | `pub fn gemv_q8_0(weights: &[BlockQ8_0], input: &[f32], output: &mut [f32], n: usize, k: usize)` | Q8_0 GEMV |
| `gemv_q4_0` | `pub fn gemv_q4_0(weights: &[BlockQ4_0], input: &[f32], output: &mut [f32], n: usize, k: usize)` | Q4_0 GEMV |
| `gemv_q8_k` | `pub fn gemv_q8_k(weights: &[BlockQ8K], input: &[f32], output: &mut [f32], n: usize, k: usize)` | Q8_K GEMV |
| `quantize_row_q8_0` | `pub fn quantize_row_q8_0(input: &[f32], output: &mut [BlockQ8_0])` | f32 → Q8_0 量化（预处理输入向量） |
| `gemv_dispatch` | `pub fn gemv_dispatch(dtype: GgmlDType, weights: &[u8], input: &[f32], output: &mut [f32], n: usize, k: usize) -> Result<()>` | 根据 dtype 分发到对应 GEMV 内核 |

**AVX2 内核设计**（以 Q8_0 为例）：

```rust
/// Q8_0 GEMV 内核（AVX2 优化）。
///
/// 计算 output[n] = sum_k(weights[n,k] * input[k])
///
/// - `weights`: [n, k/32] 个 BlockQ8_0（行优先，每行 k/32 个块）
/// - `input`: [k] 个 f32
/// - `output`: [n] 个 f32（输出缓冲区）
/// - `n`: 输出维度（行数）
/// - `k`: 输入维度（列数）
///
/// # AVX2 策略
///
/// 每个 BlockQ8_0 包含 32 个 int8 值和一个 f16 scale。
/// 利用 `_mm256_maddubs_epi16` + `_mm256_madd_epi16` 计算 8 路 int8 点积，
/// 再乘以 scale 转为 f32 累加。
///
/// # Safety
///
/// 调用方需确保 `weights` 切片长度 >= n * (k/32)，`input` >= k，`output` >= n。
/// 仅在 x86_64 with target_feature = "avx2" 时使用 AVX2 路径。
pub fn gemv_q8_0(weights: &[BlockQ8_0], input: &[f32], output: &mut [f32], n: usize, k: usize) {
    let blocks_per_row = k / QK8_0;

    // 先将 input 量化为 Q8_0 格式（预处理，一次性完成）
    let mut input_q = vec![BlockQ8_0::zeros(); blocks_per_row];
    quantize_row_q8_0(input, &mut input_q);

    for row in 0..n {
        let weight_row = &weights[row * blocks_per_row..(row + 1) * blocks_per_row];
        output[row] = vec_dot_q8_0_q8_0(weight_row, &input_q);
    }
}
```

### 18.2 TDD 测试用例

#### 文件：`crates/infra/src/gemv_kernel_tests.rs`（新建，Pro 门控）

| 测试 ID | 测试名 | 验证内容 |
|---|---|---|
| TC-GEMV-001 | `test_gemv_q8_0_identity` | 单位矩阵 GEMV 结果 == 输入向量 |
| TC-GEMV-002 | `test_gemv_q8_0_zero_input` | 零输入 → 零输出 |
| TC-GEMV-003 | `test_gemv_q8_0_known_values` | 小矩阵（2x4）手算结果对照 |
| TC-GEMV-004 | `test_gemv_q8_0_large_matrix` | 256x256 矩阵与 f32 参考实现误差 < 0.5 |
| TC-GEMV-005 | `test_gemv_q4_0_identity` | Q4_0 单位矩阵 GEMV |
| TC-GEMV-006 | `test_gemv_q4_0_known_values` | 小矩阵手算对照 |
| TC-GEMV-007 | `test_gemv_q4k_known_values` | Q4_K 小矩阵手算对照 |
| TC-GEMV-008 | `test_gemv_q4k_large_matrix` | 256x256 与参考实现误差 < 1.0 |
| TC-GEMV-009 | `test_quantize_row_q8_0_roundtrip` | 量化→反量化往返误差 < 0.5 |
| TC-GEMV-010 | `test_gemv_dispatch_q8_0` | dispatch 正确路由到 Q8_0 内核 |
| TC-GEMV-011 | `test_gemv_dispatch_q4_k` | dispatch 正确路由到 Q4_K 内核 |
| TC-GEMV-012 | `test_gemv_dispatch_unknown_dtype` | 未知 dtype 返回 Err |
| TC-GEMV-013 | `test_gemv_q8_0_vs_naive` | AVX2 内核结果与朴素循环一致 |
| TC-GEMV-014 | `test_gemv_q4_k_vs_naive` | Q4_K AVX2 与朴素循环一致 |

**朴素参考实现**（测试对照）：

```rust
/// 朴素 Q8_0 GEMV（无 SIMD，用于测试对照）。
fn gemv_q8_0_naive(weights: &[BlockQ8_0], input: &[f32], output: &mut [f32], n: usize, k: usize) {
    let blocks_per_row = k / QK8_0;
    for row in 0..n {
        let mut sum = 0f32;
        for b in 0..blocks_per_row {
            let block = &weights[row * blocks_per_row + b];
            let d = f16::to_f32(block.d);
            for i in 0..QK8_0 {
                sum += d * block.qs[i] as f32 * input[b * QK8_0 + i];
            }
        }
        output[row] = sum;
    }
}
```

### 18.3 关键实现细节

1. **AVX2 点积内核**（Q8_0 @ Q8_0）：
   - `_mm256_maddubs_epi16`：8 路 uint8×int8 → int16
   - `_mm256_madd_epi16`：int16×int16 → int32
   - `_mm256_cvtepi32_ps` + `_mm256_fmadd_ps`：int32 → f32 累加
   - 最后 `_mm256_set1_ps(d)` 乘以 scale

2. **Q4_K GEMV**：
   - Q4_K 使用 6-bit scales + 4-bit values，更复杂
   - 先从 `scales` 数组提取 6-bit 缩放因子
   - 然后对 4-bit nibbles 进行移位和掩码
   - 利用 `bytes_from_nibbles_32` 辅助函数（参考 candle avx.rs）

3. **输入预处理**：
   - 将 f32 输入向量先量化为 Q8_0 格式
   - 后续点积全部使用 int8 运算（避免 f32→f32 乘法瓶颈）
   - 这与 llama.cpp 的 `quantize_row_q8_0` 策略一致

4. **多架构支持**：
   - `#[cfg(target_feature = "avx2")]` — AVX2 路径
   - `#[cfg(target_feature = "neon")]` — NEON 路径（ARM，后续 Session）
   - 回退路径：朴素循环（正确但不快速）

### 18.4 验证命令

```bash
cargo test -p echomind-infra --features pro -- gemv_kernel_tests
cargo clippy -p echomind-infra --features pro -- -D warnings
```

### 18.5 Session 18 完成标志

- [x] `gemv_q8_0` / `gemv_q4_0` / `gemv_q4_k` / `gemv_q8_k` 标量 GEMV 内核实现完成
- [x] `quantize_row_q8_0` 输入预处理（f32 → Q8_0 量化）完成
- [x] `gemv_dispatch` dtype 路由函数完成（支持 Q4_0/Q4_K/Q8_0/Q8_K）
- [x] `vec_dot_q8_0_q8_0` / `vec_dot_q4_0_q8_0` / `vec_dot_q4_k_q8_0` / `vec_dot_q8_k_q8_0` 整数点积内核完成
- [x] 朴素参考实现 `gemv_q8_0_naive` / `gemv_q4_k_naive` 完成（测试对照）
- [x] 14 个测试全部通过（含朴素实现对照），clippy 零警告

**实现详情**（2026-07-31 完成）：
- 标量实现（所有架构可用）：输入预处理 → int8 点积 → f32 累加
- AVX2 SIMD 路径预留 `#[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]`（后续优化）
- Q4_K GEMV 利用分解公式：`dot = Σ(d * sc * Σ(q_w * q_x) - dmin * m * Σ(q_x)) * d_x`
- 输入量化误差容忍度：256 元素矩阵 < 1.5，Q4_K 块 < 2.0
- `get_scale_min_k4` 改为 `pub(crate)` 供 gemv_kernel 使用
- `quant_blocks_tests.rs` 添加 `#![allow(clippy::unwrap_used, clippy::panic, clippy::needless_range_loop)]`

### 18.6 Session 18 → Session 19 衔接提示词

```
你正在继续 EchoMind 本地 LLM 引擎 Phase 3 的开发。

## 上下文
- Phase 3 Session 16 完成：GGUF 文件解析器（mmap 零拷贝）
- Phase 3 Session 17 完成：量化块结构与反量化
  - BlockQ4_0/BlockQ4K/BlockQ5K/BlockQ6K/BlockQ8_0/BlockQ8K 结构体
  - QuantBlock 枚举（借用引用，零拷贝）
  - dequantize_q4_0/q4_k/q5_k/q6_k/q8_0/q8_k 函数
  - blocks_from_bytes 从 mmap 数据解析量化块
  - 14 个测试通过，clippy 零警告
- Phase 3 Session 18 完成：自研 GEMV 内核
  - gemv_q8_0/gemv_q4_0/gemv_q4_k/gemv_q8_k 标量 GEMV 内核（所有架构可用）
  - quantize_row_q8_0 输入预处理（f32 → Q8_0 量化）
  - gemv_dispatch dtype 路由（Q4_0/Q4_K/Q8_0/Q8_K）
  - vec_dot_* 整数点积内核（int8 × int8 → i32 → f32）
  - 朴素参考实现 gemv_q8_0_naive / gemv_q4_k_naive（测试对照）
  - 14 个测试通过（含朴素实现对照），clippy 零警告

## 本 Session 目标
实现 CPU cache 友好的权重重排，将行优先布局转为 Tile-Major 布局，减少 GEMV 推理时的 L1/L2 cache miss。

## 必读文档
1. docs/architecture/LOCAL_LLM_ENGINE_PHASE3_ATOMIC_PLAN.md — Session 19 章节
2. crates/infra/src/gemv_kernel.rs — 已完成的 GEMV 内核
3. crates/infra/src/quant_blocks.rs — 量化块结构定义
4. llama.cpp ggml-cpu/repack.cpp — 参考实现
5. https://preshing.com/20120207/optimizing-the-cache-layout/ — cache 友好布局原理

## 精确变更清单
| 文件 | 变更类型 | 内容 |
|---|---|---|
| crates/infra/src/lib.rs | 修改 | 新增 weight_repack 模块声明（Pro 门控） |
| crates/infra/src/weight_repack.rs | 新建 | RepackedWeights + repack_for_gemv + WeightLayout |
| crates/infra/src/weight_repack_tests.rs | 新建 | 6 个测试（TC-REPACK-001~006） |

## TDD 纪律
先写测试（红灯），再写实现（绿灯）。

## 完成后
1. 运行验证命令确认通过
2. 产出下一 Session 衔接提示词
3. 运行 ./sync-to-github.sh 同步
```

---

## Session 19：CPU cache 友好权重重排

**目标**：实现 `crates/infra/src/weight_repack.rs`，将 GGUF 量化权重重排为 CPU cache 友好的布局，减少推理时的 L1/L2 cache miss。

**预计耗时**：2 天

### 19.1 文件变更清单

#### 文件 1：`crates/infra/src/lib.rs`（修改）

```rust
/// 权重重排（Phase 3）：CPU cache 友好的权重布局优化。
#[cfg(feature = "pro")]
pub mod weight_repack;
```

#### 文件 2：`crates/infra/src/weight_repack.rs`（新建）

**完整函数签名清单**：

| 函数 | 签名 | 职责 |
|---|---|---|
| `RepackedWeights` | `pub struct RepackedWeights` | 重排后的权重容器 |
| `repack_for_gemv` | `pub fn repack_for_gemv(data: &[u8], dtype: GgmlDType, n: usize, k: usize) -> Result<RepackedWeights>` | 将行优先权重重排为 GEMV 友好布局 |
| `RepackedWeights::data` | `pub fn data(&self) -> &[u8]` | 返回重排后的字节切片 |
| `RepackedWeights::dtype` | `pub fn dtype(&self) -> GgmlDType` | 返回量化格式 |
| `RepackedWeights::layout` | `pub fn layout(&self) -> &WeightLayout` | 返回布局类型 |
| `WeightLayout` | `pub enum WeightLayout` | 布局枚举（RowMajor/TileMajor/GemvOptimized） |

**设计原理**：

```
原始布局（Row-Major）:
  ┌──────────────────────────────────────┐
  │ Row 0: [Block0][Block1][Block2]...   │  ← 连续 144B (Q4_K)
  │ Row 1: [Block0][Block1][Block2]...   │
  │ Row 2: [Block0][Block1][Block2]...   │
  └──────────────────────────────────────┘

GEMV 时逐行访问，但输入向量 Block_i 被重复读取：
  Row 0 × input → 需要读 Row0 的所有 Block
  Row 1 × input → 需要读 Row1 的所有 Block
  ...
  问题：Row N 的 Block_i 与 Row 0 的 Block_i 在内存中相距很远
  → L1 cache 无法复用输入向量的 Block_i 读取

GEMV 优化布局（Tile-Major）:
  ┌──────────────────────────────────────┐
  │ Col 0: [Row0_B0][Row1_B0][Row2_B0]..│  ← 所有行的 Block0 连续
  │ Col 1: [Row0_B1][Row1_B1][Row2_B1]..│  ← 所有行的 Block1 连续
  │ Col 2: [Row0_B2][Row1_B2][Row2_B2]..│
  └──────────────────────────────────────┘

GEMV 时按列遍历：
  遍历 Col 0: 读 input_B0，然后 Row0..N 的 B0 × input_B0 → 全在 L1
  遍历 Col 1: 读 input_B1，然后 Row0..N 的 B1 × input_B1 → 全在 L1
  → 每个输入 Block 只从 L2 读一次，在 L1 中被 N 行复用
```

### 19.2 TDD 测试用例

| 测试 ID | 测试名 | 验证内容 |
|---|---|---|
| TC-REPACK-001 | `test_repack_q8_0_preserves_data` | 重排后数据可正确反量化为原始值 |
| TC-REPACK-002 | `test_repack_q4_k_preserves_data` | Q4_K 重排后数据完整 |
| TC-REPACK-003 | `test_repack_layout_is_gemv_optimized` | 重排后布局为 GemvOptimized |
| TC-REPACK-004 | `test_repack_gemv_result_matches_unrepacked` | 重排后 GEMV 结果与未重排一致 |
| TC-REPACK-005 | `test_repack_data_size_unchanged` | 重排前后字节大小不变 |
| TC-REPACK-006 | `test_repack_invalid_alignment_fails` | 未对齐数据返回 Err |

### 19.3 验证命令

```bash
cargo test -p echomind-infra --features pro -- weight_repack_tests
cargo clippy -p echomind-infra --features pro -- -D warnings
```

### 19.4 Session 19 完成标志

- [x] `RepackedWeights` / `WeightLayout` 定义完成
- [x] `repack_for_gemv` 行优先 → Tile-Major 块级转置函数完成
- [x] `gemv_repacked_dispatch` 使用重排权重的 cache 友好 GEMV 完成（支持 Q4_0/Q4_K/Q8_0/Q8_K）
- [x] `RepackedWeights::to_row_major` 逆变换方法完成（测试用）
- [x] `gemv_kernel.rs` 中 `vec_dot_*` 函数改为 `pub(crate)` 供 weight_repack 模块调用
- [x] 6 个测试全部通过，clippy 零警告

**实现详情**（2026-08-01 完成）：
- 块级转置：`[row][col]` → `[col][row]`，重排前后字节大小不变
- `gemv_repacked_dispatch` 利用 `std::ptr::read_unaligned` 从非对齐的 `Vec<u8>` 安全读取 Block 结构体
- 按列遍历 GEMV：外层遍历 col（输入 Block），内层遍历所有行，使每个输入 Block 在 L1 中被 N 行复用
- `RepackedWeights::block_bytes_at(row, col)` 提供 Tile-Major 布局中指定位置的块字节切片
- `RepackedWeights::to_row_major()` 提供逆变换，用于测试验证数据完整性
- 全量测试：175 passed, 0 failed, 8 ignored

### 19.5 Session 19 → Session 20 衔接提示词

```
你正在继续 EchoMind 本地 LLM 引擎 Phase 3 的开发。

## 上下文
- Phase 3 Session 16 完成：GGUF 文件解析器（mmap 零拷贝）
- Phase 3 Session 17 完成：量化块结构与反量化
  - BlockQ4_0/BlockQ4K/BlockQ5K/BlockQ6K/BlockQ8_0/BlockQ8K 结构体
  - QuantBlock 枚举（借用引用，零拷贝）
  - dequantize_q4_0/q4_k/q5_k/q6_k/q8_0/q8_k 函数
  - blocks_from_bytes 从 mmap 数据解析量化块
  - 14 个测试通过，clippy 零警告
- Phase 3 Session 18 完成：自研 GEMV 内核
  - gemv_q8_0/gemv_q4_0/gemv_q4_k/gemv_q8_k 标量 GEMV 内核（所有架构可用）
  - quantize_row_q8_0 输入预处理（f32 → Q8_0 量化）
  - gemv_dispatch dtype 路由（Q4_0/Q4_K/Q8_0/Q8_K）
  - vec_dot_* 整数点积内核（int8 × int8 → i32 → f32），已改为 pub(crate)
  - 朴素参考实现 gemv_q8_0_naive / gemv_q4_k_naive（测试对照）
  - 14 个测试通过（含朴素实现对照），clippy 零警告
- Phase 3 Session 19 完成：CPU cache 友好权重重排
  - RepackedWeights / WeightLayout（RowMajor/TileMajor/GemvOptimized）
  - repack_for_gemv：行优先 → Tile-Major 块级转置
  - gemv_repacked_dispatch：使用重排权重的 cache 友好 GEMV（按列遍历）
  - RepackedWeights::to_row_major / block_bytes_at 辅助方法
  - 6 个测试通过，clippy 零警告

## 本 Session 目标
将自研 GEMV 内核 + 权重重排集成到 LocalLlmEngine，提供「自研内核模式」作为 mistral.rs 之外的推理路径。

## 必读文档
1. docs/architecture/LOCAL_LLM_ENGINE_PHASE3_ATOMIC_PLAN.md — Session 20 章节
2. crates/infra/src/local_llm.rs — 已有的 mistral.rs 引擎集成
3. crates/infra/src/weight_repack.rs — 已完成的重排模块
4. crates/infra/src/gemv_kernel.rs — 已完成的 GEMV 内核
5. crates/infra/src/gguf_reader.rs — GGUF 文件解析器

## 关键约束
- 新增 KernelMode 枚举（MistralRs / CustomGemv），默认 MistralRs
- CustomGemv 模式需先加载 GGUF + repack 权重，再调用 gemv_repacked_dispatch
- IPC 命令：set_kernel_mode / get_kernel_mode
- 设置持久化到 settings 表（llm.kernel_mode = "mistral" / "custom"）

## TDD 纪律
先写测试（红灯），再写实现（绿灯）。

## 完成后
1. 运行验证命令确认通过
2. 产出下一 Session 衔接提示词
3. 运行 ./sync-to-github.sh 同步
```

---

## Session 20：集成 LocalLlmEngine

**目标**：将自研 GEMV 内核集成到 `LocalLlmEngine`，提供「自研内核模式」作为 mistral.rs 之外的推理路径。

**预计耗时**：2 天

### 20.1 文件变更清单

#### 文件 1：`crates/infra/src/local_llm.rs`（修改）

**新增字段**：

```rust
pub struct LocalLlmEngineInner {
    // ... 现有字段 ...

    /// 推理内核模式（Phase 3）。
    ///
    /// - `MistralRs`：使用 mistral.rs 引擎（默认，Phase 1/2）
    /// - `CustomGemv`：使用自研 GEMV 内核（Phase 3，单批次优化）
    kernel_mode: Arc<tokio::sync::RwLock<KernelMode>>,
    /// 自研 GGUF 文件解析器（Phase 3，CustomGemv 模式时使用）。
    gguf_file: Arc<tokio::sync::RwLock<Option<GgufFile>>>,
    /// 重排后的权重缓存（Phase 3）。
    repacked_weights: Arc<tokio::sync::RwLock<Option<RepackedWeights>>>,
}

/// 推理内核模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KernelMode {
    /// mistral.rs 引擎（默认）
    #[default]
    MistralRs,
    /// 自研 GEMV 内核（Phase 3）
    CustomGemv,
}
```

**新增方法**：

| 方法 | 签名 | 职责 |
|---|---|---|
| `LocalLlmEngine::with_kernel_mode` | `pub fn with_kernel_mode(self, mode: KernelMode) -> Self` | 设置内核模式 |
| `LocalLlmEngine::kernel_mode` | `pub async fn kernel_mode(&self) -> KernelMode` | 获取当前模式 |
| `LocalLlmEngine::set_kernel_mode` | `pub async fn set_kernel_mode(&self, mode: KernelMode)` | 运行时切换模式 |
| `LocalLlmEngine::load_custom_weights` | `pub async fn load_custom_weights(&self) -> Result<()>` | 加载 GGUF 并重排权重 |
| `LocalLlmEngine::chat_stream_custom` | `async fn chat_stream_custom(...) -> Result<BoxStream<...>>` | 自研内核推理路径 |

#### 文件 2：`crates/tauri-app/src/commands.rs`（修改）

**新增 IPC 命令**：

| 命令 | 参数 | 返回 | 说明 |
|---|---|---|---|
| `set_kernel_mode` | `mode: String` | `()` | 切换推理内核（"mistral" / "custom"） |
| `get_kernel_mode` | — | `String` | 获取当前内核模式 |

#### 文件 3：`crates/tauri-app/src/lib.rs`（修改）

```rust
commands::set_kernel_mode,
commands::get_kernel_mode,
```

### 20.2 TDD 测试用例

| 测试 ID | 测试名 | 验证内容 |
|---|---|---|
| TC-INTEG-001 | `test_kernel_mode_default_mistral` | 默认为 MistralRs |
| TC-INTEG-002 | `test_set_kernel_mode_custom` | 切换到 CustomGemv |
| TC-INTEG-003 | `test_set_kernel_mode_invalid` | 无效模式返回 Err |
| TC-INTEG-004 | `test_load_custom_weights` | 加载 GGUF + 重排权重成功 |
| TC-INTEG-005 | `test_chat_stream_custom_q8_0` | 自研内核 Q8_0 推理产生输出 |
| TC-INTEG-006 | `test_chat_stream_custom_q4_k` | 自研内核 Q4_K 推理产生输出 |
| TC-INTEG-007 | `test_kernel_mode_persisted` | 模式持久化到 settings 表 |

### 20.3 验证命令

```bash
cargo test -p echomind-infra --features pro -- local_llm_tests -- kernel_mode
cargo test -p tauri-app --features pro -- kernel_mode
cargo clippy -p echomind-infra --features pro -- -D warnings
cargo clippy -p tauri-app --features pro -- -D warnings
```

### 20.4 完成记录

**实现内容**：
- `crates/infra/src/local_llm.rs` — KernelMode 枚举 + 3 个新字段 + 5 个新方法
- `crates/infra/src/local_llm_tests.rs` — 12 个新测试（含 2 个 #[ignore]）
- `crates/tauri-app/src/commands.rs` — 2 个新 IPC 命令 + `*_inner` 函数
- `crates/tauri-app/src/lib.rs` — 注册 2 个新命令
- `crates/tauri-app/src/stores/model_store.rs` — 引擎创建时加载 kernel_mode 设置
- `crates/tauri-app/tests/integration.rs` — 5 个集成测试

**新增类型/方法**：
- `KernelMode` 枚举（MistralRs / CustomGemv，Default = MistralRs）
- `KernelMode::from_str(s) -> Result<Self>` — 字符串解析（"mistral"/"custom"）
- `KernelMode::as_str() -> &'static str` — 字符串表示
- `LocalLlmEngine::kernel_mode() -> KernelMode` — 获取当前模式
- `LocalLlmEngine::set_kernel_mode(mode)` — 运行时切换模式
- `LocalLlmEngine::with_kernel_mode(mode) -> Self` — builder 模式
- `LocalLlmEngine::load_custom_weights() -> Result<()>` — GGUF 加载 + 权重重排
- `LocalLlmEngine::chat_stream_custom()` — 自研内核推理路径（当前返回错误，完整 transformer 未实现）
- `set_kernel_mode` / `get_kernel_mode` IPC 命令

**关键设计**：
- `LocalLlmEngineInner` 新增 3 个字段：`kernel_mode` / `gguf_file` / `repacked_weights`
- `LLMProvider::chat_stream` 在方法入口检查 `kernel_mode`，CustomGemv 模式路由到 `chat_stream_custom`
- `load_custom_weights` 使用 double-check locking + `spawn_blocking`（CPU 密集型）
- `warm_up()` 在 CustomGemv 模式下触发 `load_custom_weights` 而非 `get_or_load_model`
- `ModelStore::local_llm()` 创建引擎时从 settings 读取 `llm.kernel_mode` 并应用
- `chat_stream_custom` 当前返回错误（完整 transformer 前向传播未实现，S21+ 实现）
- TC-INTEG-005/006 标记 `#[ignore]`（需真实 GGUF 模型文件）
- TC-INTEG-004 使用合成 GGUF 文件（Q8_0 token_embd.weight, dims=[4,32]）验证权重加载管道

**验证结果**：
- cargo test -p echomind-infra --features pro: 182 passed, 0 failed, 10 ignored
- cargo test -p echomind-infra --features pro -- local_llm_tests -- kernel_mode: 37 passed, 0 failed, 6 ignored
- cargo test -p echomind-tauri-app --features pro -- kernel_mode: 5 passed, 0 failed
- cargo clippy -p echomind-infra --features pro --all-targets -- -D warnings: 零警告
- cargo clippy -p echomind-tauri-app --features pro --all-targets -- -D warnings: 零警告
- cargo fmt --check: 通过

### 20.5 Session 20 → Session 21 衔接提示词

```
你正在继续 EchoMind 本地 LLM 引擎 Phase 3 的开发。

## 上下文
- Phase 3 Session 16 完成：GGUF 文件解析器（mmap 零拷贝）
  - GgufFile / TensorInfo / GgmlDType / GgufValue
  - GGUF v1/v2/v3 格式解析，元数据 + 张量信息 + 零拷贝张量数据
  - 14 个测试通过，clippy 零警告
- Phase 3 Session 17 完成：量化块结构与反量化
  - BlockQ4_0/BlockQ4K/BlockQ5K/BlockQ6K/BlockQ8_0/BlockQ8K 结构体
  - QuantBlock 枚举（借用引用，零拷贝）
  - dequantize_q4_0/q4_k/q5_k/q6_k/q8_0/q8_k 函数
  - blocks_from_bytes 从 mmap 数据解析量化块
  - 14 个测试通过，clippy 零警告
- Phase 3 Session 18 完成：自研 GEMV 内核
  - gemv_q8_0/gemv_q4_0/gemv_q4_k/gemv_q8_k 标量 GEMV 内核（所有架构可用）
  - quantize_row_q8_0 输入预处理（f32 → Q8_0 量化）
  - gemv_dispatch dtype 路由（Q4_0/Q4_K/Q8_0/Q8_K）
  - vec_dot_* 整数点积内核（int8 × int8 → i32 → f32），已改为 pub(crate)
  - 朴素参考实现 gemv_q8_0_naive / gemv_q4_k_naive（测试对照）
  - 14 个测试通过，clippy 零警告
- Phase 3 Session 19 完成：CPU cache 友好权重重排
  - RepackedWeights / WeightLayout（RowMajor/TileMajor/GemvOptimized）
  - repack_for_gemv：行优先 → Tile-Major 块级转置
  - gemv_repacked_dispatch：使用重排权重的 cache 友好 GEMV（按列遍历）
  - RepackedWeights::to_row_major / block_bytes_at 辅助方法
  - 6 个测试通过，clippy 零警告
- Phase 3 Session 20 完成：集成 LocalLlmEngine
  - KernelMode 枚举（MistralRs / CustomGemv，默认 MistralRs）
  - LocalLlmEngine 新增 kernel_mode/gguf_file/repacked_weights 字段
  - load_custom_weights：GGUF 加载 + repack_for_gemv 权重重排（double-check locking）
  - chat_stream_custom：自研内核推理路径（当前返回错误，完整 transformer 未实现）
  - LLMProvider::chat_stream 根据 kernel_mode 路由
  - warm_up 在 CustomGemv 模式下触发 load_custom_weights
  - set_kernel_mode / get_kernel_mode IPC 命令 + 持久化到 settings 表
  - ModelStore::local_llm 创建引擎时从 settings 读取 kernel_mode
  - 12 个单元测试 + 5 个集成测试通过，clippy 零警告

## 本 Session 目标
实现 Layer 级流式预取（`crates/infra/src/layer_prefetch.rs`），在推理进行时后台预取下一层权重到 RAM（利用 `madvise(MADV_WILLNEED)`）。

## 必读文档
1. docs/architecture/LOCAL_LLM_ENGINE_PHASE3_ATOMIC_PLAN.md — Session 21 章节
2. crates/infra/src/gguf_reader.rs — GGUF 文件解析器（mmap 区域访问）
3. crates/infra/src/local_llm.rs — 已集成的 LocalLlmEngine（含 gguf_file 字段）
4. crates/infra/src/weight_repack.rs — 权重重排模块

## 关键约束
- LayerPrefetcher 管理 mmap 区域的 layer 级偏移量
- 使用 libc::madvise(MADV_WILLNEED) 告知 OS 预取（Unix-only，Windows 回退）
- 预取线程异步执行，不阻塞推理主线程
- layer 偏移量从 GgufFile::tensor_info() 获取

## TDD 纪律
先写测试（红灯），再写实现（绿灯）。

## 完成后
1. 运行验证命令确认通过
2. 产出下一 Session 衔接提示词
3. 运行 ./sync-to-github.sh 同步
```

---

## Session 21：Layer 级流式预取

**目标**：实现 `crates/infra/src/layer_prefetch.rs`，在推理进行时后台预取下一层权重到 RAM（利用 `madvise(MADV_WILLNEED)`）。

**预计耗时**：2 天

### 21.1 文件变更清单

#### 文件 1：`crates/infra/src/lib.rs`（修改）

```rust
/// Layer 级流式预取（Phase 3）。
#[cfg(feature = "pro")]
pub mod layer_prefetch;
```

#### 文件 2：`crates/infra/src/layer_prefetch.rs`（新建）

**完整函数签名清单**：

| 函数/结构体 | 签名 | 职责 |
|---|---|---|
| `LayerPrefetcher` | `pub struct LayerPrefetcher` | 预取器，管理后台预取线程 |
| `LayerPrefetcher::new` | `pub fn new(mmap: &Mmap, layer_offsets: Vec<(String, u64, u64)>) -> Self` | 创建预取器 |
| `LayerPrefetcher::prefetch_next` | `pub fn prefetch_next(&self, layer_idx: usize)` | 预取指定层（madvise + 触发 page fault） |
| `LayerPrefetcher::prefetch_range` | `pub fn prefetch_range(&self, start: usize, count: usize)` | 预取连续 N 层 |
| `LayerPrefetcher::stop` | `pub fn stop(&self)` | 停止预取线程 |
| `LayerPrefetcher::is_active` | `pub fn is_active(&self) -> bool` | 预取线程是否运行中 |

**设计原理**：

```rust
impl LayerPrefetcher {
    /// 预取指定层权重到 RAM。
    ///
    /// 使用 `madvise(MADV_WILLNEED)` 告知 OS 这些页面即将被访问，
    /// OS 会在后台异步从磁盘加载到 page cache。
    ///
    /// # Safety
    ///
    /// 此方法调用 `libc::madvise()`，仅在 Unix 系统可用。
    /// Windows 上回退为 `PrefetchVirtualMemory`（后续支持）。
    pub fn prefetch_next(&self, layer_idx: usize) {
        let (_, offset, size) = &self.layer_offsets[layer_idx];
        let ptr = unsafe { self.mmap.as_ptr().add(*offset as usize) };
        let ret = unsafe {
            libc::madvise(
                ptr as *mut libc::c_void,
                *size as libc::size_t,
                libc::MADV_WILLNEED,
            )
        };
        if ret != 0 {
            // madvise 失败不影响正确性，仅日志记录
            tracing::debug!("madvise failed for layer {}: errno={}", layer_idx, ret);
        }
    }
}
```

### 21.2 TDD 测试用例

| 测试 ID | 测试名 | 验证内容 |
|---|---|---|
| TC-PREFETCH-001 | `test_prefetcher_creation` | 预取器创建成功 |
| TC-PREFETCH-002 | `test_prefetch_next_no_panic` | prefetch_next 不 panic |
| TC-PREFETCH-003 | `test_prefetch_range_no_panic` | prefetch_range 不 panic |
| TC-PREFETCH-004 | `test_prefetch_out_of_bounds_safe` | 越界预取安全返回（不 panic） |
| TC-PREFETCH-005 | `test_prefetch_stop` | stop 后 is_active 返回 false |
| TC-PREFETCH-006 | `test_prefetch_pages_loaded` | 预取后页面在 RAM 中（mincore 检查，#[ignore]） |

### 21.3 Session 21 完成记录（2026-08-01）

**实现内容**：
- `crates/infra/src/layer_prefetch.rs` — LayerPrefetcher + SendPtr + layer_offsets_from_gguf
- `crates/infra/src/layer_prefetch_tests.rs` — 6 个测试（含 1 个 #[ignore]）
- `crates/infra/src/lib.rs` — 注册 layer_prefetch 模块 + layer_prefetch_tests 测试模块
- `crates/infra/Cargo.toml` — 新增 libc Pro 依赖

**新增类型/函数**：
- `SendPtr(*const u8)` — 可跨线程传递的裸指针包装（Clone + Copy + Send + Sync）
- `LayerPrefetcher` — 预取器结构体（mmap_ptr/mmap_len/layer_offsets/active/stop_flag/thread_handle）
- `LayerPrefetcher::new(mmap, layer_offsets)` — 创建预取器（初始非活跃）
- `LayerPrefetcher::prefetch_next(layer_idx)` — 同步 madvise 单层（非阻塞）
- `LayerPrefetcher::prefetch_range(start, count)` — 后台线程批量 madvise（1ms 间隔）
- `LayerPrefetcher::stop()` — 停止预取线程（join 等待退出）
- `LayerPrefetcher::is_active()` — 查询活跃状态
- `LayerPrefetcher::layer_offsets()` — 获取层偏移量引用
- `layer_offsets_from_gguf(gguf)` — 从 GgufFile 提取所有张量的 mmap 绝对偏移量
- `do_madvise_with_sendptr(ptr, len, off, sz)` — SendPtr 包装的 madvise 调用（后台线程用）
- `do_madvise_raw(ptr, len, off, sz)` — 原始 madvise 调用（边界检查 + Unix/Windows 条件编译）
- `Drop for LayerPrefetcher` — 析构时确保后台线程退出

**关键设计**：
- `SendPtr` newtype 解决 RFC 2229 disjoint closure capture 问题（裸指针不满足 Send）
- `prefetch_next` 同步调用 madvise（微秒级，OS 异步处理 I/O）
- `prefetch_range` 启动后台 std::thread 逐层 madvise（1ms 间隔避免 I/O 风暴）
- `stop_flag` AtomicBool 控制后台线程退出，`stop()` 中 join 等待
- `Drop` 实现确保后台线程在预取器销毁前退出
- `do_madvise_raw` 使用 `saturating_add` 边界检查，`#[cfg(unix)]` / `#[cfg(not(unix))]` 条件编译
- madvise 失败仅 `eprintln!` 日志，不影响正确性
- TC-PREFETCH-006 使用 `libc::mincore` 检查页面驻留状态，标记 `#[ignore]`

**验证结果**：
- cargo test -p echomind-infra --features pro: 198 passed, 0 failed, 11 ignored
- cargo test -p echomind-infra --features pro -- layer_prefetch_tests: 5 passed, 0 failed, 1 ignored
- cargo clippy -p echomind-infra --features pro --all-targets -- -D warnings: 零警告
- cargo clippy -p echomind-infra --all-targets -- -D warnings（Free）: 零警告
- cargo fmt --check: 通过

### 21.4 Session 21 → Session 22 衔接提示词

```
你正在继续 EchoMind 本地 LLM 引擎 Phase 3 的开发。

## 上下文
- Phase 3 Session 16 完成：GGUF 文件解析器（mmap 零拷贝）
  - GgufFile / TensorInfo / GgmlDType / GgufValue
  - GGUF v1/v2/v3 格式解析，元数据 + 张量信息 + 零拷贝张量数据
  - 14 个测试通过，clippy 零警告
- Phase 3 Session 17 完成：量化块结构与反量化
  - BlockQ4_0/BlockQ4K/BlockQ5K/BlockQ6K/BlockQ8_0/BlockQ8K 结构体
  - QuantBlock 枚举（借用引用，零拷贝）
  - dequantize_q4_0/q4_k/q5_k/q6_k/q8_0/q8_k 函数
  - blocks_from_bytes 从 mmap 数据解析量化块
  - 14 个测试通过，clippy 零警告
- Phase 3 Session 18 完成：自研 GEMV 内核
  - gemv_q8_0/gemv_q4_0/gemv_q4_k/gemv_q8_k 标量 GEMV 内核（所有架构可用）
  - quantize_row_q8_0 输入预处理（f32 → Q8_0 量化）
  - gemv_dispatch dtype 路由（Q4_0/Q4_K/Q8_0/Q8_K）
  - vec_dot_* 整数点积内核（int8 × int8 → i32 → f32），已改为 pub(crate)
  - 朴素参考实现 gemv_q8_0_naive / gemv_q4_k_naive（测试对照）
  - 14 个测试通过，clippy 零警告
- Phase 3 Session 19 完成：CPU cache 友好权重重排
  - RepackedWeights / WeightLayout（RowMajor/TileMajor/GemvOptimized）
  - repack_for_gemv：行优先 → Tile-Major 块级转置
  - gemv_repacked_dispatch：使用重排权重的 cache 友好 GEMV（按列遍历）
  - RepackedWeights::to_row_major / block_bytes_at 辅助方法
  - 6 个测试通过，clippy 零警告
- Phase 3 Session 20 完成：集成 LocalLlmEngine
  - KernelMode 枚举（MistralRs / CustomGemv，默认 MistralRs）
  - LocalLlmEngine 新增 kernel_mode/gguf_file/repacked_weights 字段
  - load_custom_weights：GGUF 加载 + repack_for_gemv 权重重排（double-check locking）
  - chat_stream_custom：自研内核推理路径（当前返回错误，完整 transformer 未实现）
  - LLMProvider::chat_stream 根据 kernel_mode 路由
  - warm_up 在 CustomGemv 模式下触发 load_custom_weights
  - set_kernel_mode / get_kernel_mode IPC 命令 + 持久化到 settings 表
  - ModelStore::local_llm 创建引擎时从 settings 读取 kernel_mode
  - 12 个单元测试 + 5 个集成测试通过，clippy 零警告
- Phase 3 Session 21 完成：Layer 级流式预取
  - LayerPrefetcher 结构体（mmap_ptr/mmap_len/layer_offsets/active/stop_flag/thread_handle）
  - SendPtr newtype（Clone+Copy+Send+Sync）解决 RFC 2229 闭包捕获问题
  - prefetch_next：同步 madvise(MADV_WILLNEED) 单层（非阻塞）
  - prefetch_range：后台 std::thread 批量 madvise（1ms 间隔避免 I/O 风暴）
  - stop：设置 stop_flag + join 后台线程
  - Drop 实现：析构时确保后台线程退出
  - layer_offsets_from_gguf：从 GgufFile 提取张量 mmap 绝对偏移量
  - do_madvise_raw：边界检查 + Unix/Windows 条件编译
  - 6 个测试通过（含 1 个 #[ignore] mincore 检查），clippy 零警告

## 本 Session 目标
实现 RAM 预算管理（`crates/infra/src/memory_budget.rs`），在模型 > 可用 RAM 时自动管理内存，驱逐已使用的层、预取即将使用的层。

## 必读文档
1. docs/architecture/LOCAL_LLM_ENGINE_PHASE3_ATOMIC_PLAN.md — Session 22 章节
2. crates/infra/src/layer_prefetch.rs — Layer 级流式预取（S21 完成）
3. crates/infra/src/gguf_reader.rs — GGUF 文件解析器（mmap 区域访问）
4. crates/infra/src/local_llm.rs — LocalLlmEngine（集成目标）

## 关键约束
- MemoryBudget 管理模型内存使用量，支持 LRU 驱逐
- 使用 sysinfo crate 获取系统总 RAM
- 与 LayerPrefetcher 协作：预取前检查预算，驱逐 LRU 层释放空间
- allocate/release/can_fit/evict_lru 方法

## TDD 纪律
先写测试（红灯），再写实现（绿灯）。

## 完成后
1. 运行验证命令确认通过
2. 产出下一 Session 衔接提示词
3. 运行 ./sync-to-github.sh 同步
```

### 21.3 Session 21 完成标志

- [x] `layer_prefetch.rs` 创建完成（LayerPrefetcher + 6 个方法）
- [x] `layer_prefetch_tests` 6 个测试通过（5 passed + 1 `#[ignore]`）
- [x] `cargo clippy -p echomind-infra --features pro --all-targets -- -D warnings`：零警告
- [x] `cargo fmt --check`：通过
- [x] Pro 模式全量测试：203 passed, 0 failed, 13 ignored

### 21.4 Session 21 实现详情

**新增类型/方法**：
- `LayerInfo` type alias（`(String, u64, u64)` — 层名/偏移/大小）
- `LayerPrefetcher` 结构体（ptr + len + layer_offsets + active）
- `LayerPrefetcher::new(mmap, layer_offsets)` — 创建预取器
- `LayerPrefetcher::prefetch_next(layer_idx)` — 预取指定层（madvise + 边界检查）
- `LayerPrefetcher::prefetch_range(start, count)` — 预取连续 N 层
- `LayerPrefetcher::stop()` — 停止预取
- `LayerPrefetcher::is_active()` — 预取器是否活跃
- `LayerPrefetcher::layer_offsets()` — 获取层信息引用
- `do_madvise(offset, size)` — Unix: libc::madvise(MADV_WILLNEED)；非 Unix: 无操作

**关键设计**：
- `Send + Sync` 手动实现（mmap 原始指针需 unsafe impl）
- 越界索引安全返回（不 panic）
- madvise 失败仅日志记录（不影响正确性）
- `stop()` 后所有预取操作变为无操作（原子标志）
- 测试使用 `tempfile::NamedTempFile` + `memmap2::MmapOptions` 创建临时 mmap

### 21.5 Session 21 → Session 22 衔接提示词

```
你正在继续 EchoMind 本地 LLM 引擎 Phase 3 的开发。

## 上下文
- Phase 3 Session 16-20 已完成：GGUF解析/量化块/GEMV内核/权重重排/引擎集成
- Phase 3 Session 21 完成：Layer 级流式预取
  - LayerPrefetcher 结构体（mmap + madvise(MADV_WILLNEED)）
  - prefetch_next/prefetch_range/stop/is_active 方法
  - 越界安全返回，madvise 失败仅日志
  - 6 个测试通过（5 passed + 1 #[ignore] mincore 检查）
  - clippy 零警告，fmt 通过

## 下一步
- 实现 Session 22：RAM 预算管理
  - crates/infra/src/memory_budget.rs
  - MemoryBudget 结构体（max_bytes/current_usage/can_fit/allocate/release/evict_lru）
  - MemoryStats 结构体
  - get_system_memory() 函数
  - 8 个 TDD 测试用例

## 验证
- cargo test -p echomind-infra --features pro
- cargo clippy -p echomind-infra --features pro --all-targets -- -D warnings
```

---

## Session 22：RAM 预算管理

**目标**：实现 `crates/infra/src/memory_budget.rs`，在模型 > 可用 RAM 时自动管理内存，驱逐已使用的层、预取即将使用的层。

**预计耗时**：2 天

### 22.1 文件变更清单

#### 文件 1：`crates/infra/src/lib.rs`（修改）

```rust
/// RAM 预算管理（Phase 3）。
#[cfg(feature = "pro")]
pub mod memory_budget;
```

#### 文件 2：`crates/infra/src/memory_budget.rs`（新建）

| 函数/结构体 | 签名 | 职责 |
|---|---|---|
| `MemoryBudget` | `pub struct MemoryBudget` | 内存预算管理器 |
| `MemoryBudget::new` | `pub fn new(max_bytes: usize) -> Self` | 创建预算管理器 |
| `MemoryBudget::current_usage` | `pub fn current_usage(&self) -> usize` | 当前 RAM 使用量 |
| `MemoryBudget::can_fit` | `pub fn can_fit(&self, bytes: usize) -> bool` | 检查是否能容纳指定字节 |
| `MemoryBudget::allocate` | `pub fn allocate(&mut self, bytes: usize) -> Result<()>` | 分配内存预算 |
| `MemoryBudget::release` | `pub fn release(&mut self, bytes: usize)` | 释放内存预算 |
| `MemoryBudget::evict_lru` | `pub fn evict_lru(&mut self, need_bytes: usize) -> Result<()>` | LRU 驱逐直到释放足够空间 |
| `MemoryStats` | `pub struct MemoryStats` | 内存统计快照 |
| `get_system_memory` | `pub fn get_system_memory() -> usize` | 获取系统总 RAM（使用 sysinfo crate） |

### 22.2 TDD 测试用例

| 测试 ID | 测试名 | 验证内容 |
|---|---|---|
| TC-MEM-001 | `test_budget_creation` | 创建预算管理器 |
| TC-MEM-002 | `test_can_fit_within_budget` | 预算内 can_fit 返回 true |
| TC-MEM-003 | `test_can_fit_exceeds_budget` | 超预算 can_fit 返回 false |
| TC-MEM-004 | `test_allocate_increases_usage` | allocate 后 current_usage 增加 |
| TC-MEM-005 | `test_release_decreases_usage` | release 后 current_usage 减少 |
| TC-MEM-006 | `test_evict_lru_frees_space` | LRU 驱逐释放足够空间 |
| TC-MEM-007 | `test_get_system_memory_positive` | 系统内存 > 0 |
| TC-MEM-008 | `test_evict_lru_no_evict_needed` | 空间足够时不驱逐 |

### 22.3 Session 22 完成标志

- [x] `memory_budget.rs` 创建完成（MemoryBudget + MemoryStats + get_system_memory + 13 个测试）
- [x] `lib.rs` 已注册 `memory_budget` 模块（`#[cfg(feature = "pro")]`）
- [x] 13 个测试全部通过（超出计划的 8 个，新增 touch/stats/exceeds/reallocate/clear）
- [x] `cargo clippy -p echomind-infra --features pro --all-targets -- -D warnings`：零警告
- [x] `cargo fmt --check`：通过
- [x] Pro 模式全量测试：211 passed, 0 failed, 11 ignored

### 22.4 Session 22 实现详情

**新增类型/函数**：
- `MemoryStats` 结构体（total_system/max_budget/current_usage/available + summary()）
- `LruEntry` 结构体（layer_id + bytes，私有）
- `MemoryBudget` 结构体（max_bytes/current_usage/lru_queue/layer_map）
- `MemoryBudget::new(max_bytes)` — 创建预算管理器
- `MemoryBudget::from_system(percentage)` — 基于系统 RAM 百分比创建
- `MemoryBudget::current_usage()` — 获取当前使用量
- `MemoryBudget::max_bytes()` — 获取预算上限
- `MemoryBudget::available()` — 获取可用预算
- `MemoryBudget::can_fit(bytes)` — 检查是否能容纳
- `MemoryBudget::allocate(layer_id, bytes)` — 分配预算（自动驱逐 + 重复分配释放旧分配）
- `MemoryBudget::release(layer_id)` — 释放指定层
- `MemoryBudget::touch(layer_id)` — 标记最近使用（移到 LRU 队尾）
- `MemoryBudget::evict_lru(need_bytes)` — LRU 驱逐（空间足够时提前返回）
- `MemoryBudget::stats()` — 获取统计快照
- `MemoryBudget::clear()` — 清空所有分配
- `MemoryBudget::layer_count()` — 获取管理的层数
- `MemoryBudget::rebuild_index()` — 重建索引映射（私有）
- `get_system_memory()` — 获取系统总 RAM（Unix: libc::sysconf，非 Unix: 默认 8GB）

**关键设计**：
- 不依赖 sysinfo crate，使用 `libc::sysconf(_SC_PHYS_PAGES * _SC_PAGESIZE)` 获取系统 RAM
- LRU 驱逐策略：空间足够时直接返回（O(1)），不足时从队首驱逐直到满足需求
- `allocate` 自动处理重复分配（先释放旧分配再分配新的）
- `layer_map` HashMap + `lru_queue` VecDeque 双索引结构，`rebuild_index` 处理 remove 后索引失效
- `evict_lru` 的 bug 修复：原实现不检查可用空间直接驱逐，修复后先检查 `available() >= need_bytes`
- 测试使用 `#![allow(clippy::expect_used, clippy::panic)]` 豁免（与其他 Pro 测试模块一致）

**验证结果**：
- cargo test -p echomind-infra --features pro: 211 passed, 0 failed, 11 ignored
- cargo clippy -p echomind-infra --features pro --all-targets -- -D warnings: 零警告
- cargo clippy -p echomind-infra --all-targets -- -D warnings（Free）: 零警告
- cargo fmt --check: 通过

### 22.5 Session 22 → Session 23 衔接提示词

```
你正在继续 EchoMind 本地 LLM 引擎 Phase 3 的开发。

## 上下文
- Phase 3 Session 16-20 已完成：GGUF解析/量化块/GEMV内核/权重重排/引擎集成
- Phase 3 Session 21 完成：Layer 级流式预取
  - LayerPrefetcher 结构体（mmap + madvise(MADV_WILLNEED)）
  - prefetch_next/prefetch_range/stop/is_active 方法
  - 越界安全返回，madvise 失败仅日志
  - 6 个测试通过（5 passed + 1 #[ignore] mincore 检查）
  - clippy 零警告，fmt 通过
- Phase 3 Session 22 完成：RAM 预算管理
  - MemoryBudget 结构体（max_bytes/current_usage/lru_queue/layer_map）
  - allocate/release/touch/evict_lru/clear/stats 方法
  - from_system + get_system_memory（libc::sysconf，无需 sysinfo 依赖）
  - LRU 驱逐策略：空间足够时提前返回，不足时从队首驱逐
  - 13 个测试通过（超出计划的 8 个），clippy 零警告，fmt 通过

## 下一步
- 实现 Session 23：性能基准测试 + 文档同步
  - crates/infra/src/local_llm_tests.rs — 新增 4 个 #[ignore] 性能基准测试
  - 文档更新：SRS/IPC/Code-contracts/Settings/Frontend/ADR-011/AGENTS.md/CLAUDE.md

## 必读文档
1. docs/architecture/LOCAL_LLM_ENGINE_PHASE3_ATOMIC_PLAN.md — Session 23 章节
2. crates/infra/src/local_llm_tests.rs — 现有测试（含 Phase 2 性能基准）
3. crates/infra/src/gemv_kernel.rs — 自研 GEMV 内核（基准对比目标）
4. crates/infra/src/layer_prefetch.rs — 流式预取（基准对比目标）
5. crates/infra/src/memory_budget.rs — RAM 预算管理（S22 完成）

## 验证
- cargo test（Free）
- cargo test --features pro
- cargo clippy --all-targets -- -D warnings（Free + Pro）
- cargo fmt --check
- node scripts/build-ui.mjs
- ./sync-to-github.sh
```

---

## Session 23：性能基准测试 + 文档同步

**目标**：编写 Phase 3 性能基准测试，对比自研 GEMV 内核与 mistral.rs 的推理速度；同步全部文档。

**预计耗时**：1 天

### 23.1 性能基准测试

#### 文件：`crates/infra/src/local_llm_tests.rs`（修改，新增 `#[ignore]` 测试）

| 测试 ID | 测试名 | 验证内容 |
|---|---|---|
| TC-PHASE3-PERF-001 | `test_gemv_vs_mistralrs_q8_0` | 自研 GEMV Q8_0 vs mistral.rs Q8_0 速度对比 |
| TC-PHASE3-PERF-002 | `test_gemv_vs_mistralrs_q4_k` | 自研 GEMV Q4_K vs mistral.rs Q4_K 速度对比 |
| TC-PHASE3-PERF-003 | `test_prefetch_effectiveness` | 有/无预取的首层加载延迟对比 |
| TC-PHASE3-PERF-004 | `test_repack_cache_miss_reduction` | 重排前后的 cache miss 对比（perf stat，#[ignore]） |

运行方式：
```bash
ECHOMIND_TEST_MODEL=/path/to/model.gguf \
cargo test -p echomind-infra --features pro -- phase3_perf --ignored --nocapture
```

### 23.2 文档更新清单

| 文档 | 变更内容 |
|---|---|
| `docs/SRS_v1.0.md` | 新增 REQ-LLM-007（自研量化内核）、REQ-LLM-008（内存层次流式加载） |
| `docs/api/ipc-commands.md` | 新增命令：`set_kernel_mode` / `get_kernel_mode` |
| `docs/api/code-contracts.md` | 新增：`GgufFile` / `QuantBlock` / `gemv_dispatch` / `LayerPrefetcher` / `MemoryBudget` |
| `docs/api/settings-reference.md` | 新增键：`llm.kernel_mode`（"mistral" / "custom"） |
| `docs/api/frontend-modules.md` | 新增：`renderKernelModeSection` / `switchKernelMode` |
| `docs/architecture/adr/ADR-011-local-llm-engine.md` | Phase 3 标记，更新实施路线 |
| `AGENTS.md` | 更新 local_llm.rs 描述（含自研内核/流式加载） |
| `CLAUDE.md` | 更新 Pro feature gating |

### 23.3 最终验证

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

### 23.4 Session 23 完成标志

- [x] 性能基准测试设计完成（4 个 `#[ignore]` 测试）
- [x] 全量测试通过（Free: 328 passed / Pro: 480 passed, 0 failed）
- [x] Clippy 零警告（Free + Pro）
- [x] `cargo fmt --check` 通过
- [x] 全部文档更新完成（SRS/IPC/Code-contracts/Settings/Frontend/ADR-011/AGENTS.md/CLAUDE.md）
- [x] ADR-011 Phase 3 标记 ✅
- [x] `./sync-to-github.sh` 执行成功

### 23.5 Phase 3 完成总结

**Phase 3 全部 8 个 Session（S16-S23）完成**，实现内容：

| Session | 文件 | 测试数 | 状态 |
|---|---|---|---|
| S16 | `gguf_reader.rs` | 14 | ✅ |
| S17 | `quant_blocks.rs` | 14 | ✅ |
| S18 | `gemv_kernel.rs` | 14 | ✅ |
| S19 | `weight_repack.rs` | 6 | ✅ |
| S20 | `local_llm.rs` 修改 | 7 | ✅ |
| S21 | `layer_prefetch.rs` | 6 | ✅ |
| S22 | `memory_budget.rs` | 13 | ✅ |
| S23 | `local_llm_tests.rs` 修改 | 4 (#[ignore]) | ✅ |

**总计**：78 个测试（73 单元 + 4 #[ignore] 性能基准 + 1 #[ignore] mincore）

**新增模块**：6 个 Pro 门控文件（`gguf_reader` / `quant_blocks` / `gemv_kernel` / `weight_repack` / `layer_prefetch` / `memory_budget`）

**新增 IPC 命令**：`set_kernel_mode` / `get_kernel_mode`（Pro 门控）

**新增设置键**：`llm.kernel_mode`（默认 `mistral`）

**新增 SRS 需求**：REQ-LLM-007（自研量化推理内核）、REQ-LLM-008（内存层次流式加载）

**验证结果**：
- cargo test（Free）：328 passed, 0 failed, 4 ignored
- cargo test --features pro：480 passed, 0 failed, 15 ignored
- cargo clippy --all-targets -- -D warnings（Free）：零警告
- cargo clippy --all-targets --features pro -- -D warnings（Pro）：零警告
- cargo fmt --check：通过
- node scripts/build-ui.mjs：6012 行内联脚本，成功
- ./sync-to-github.sh --commit：已提交到发布仓库

**Phase 3 完成，ADR-011 状态更新为 Accepted（Phase 3 完成）。**

---

## 附录 A：Phase 3 完整文件变更矩阵

| 文件路径 | Session | 变更类型 | 说明 |
|---|---|---|---|
| `crates/infra/Cargo.toml` | S16 | 修改 | 新增 memmap2 Pro 依赖 |
| `crates/infra/src/lib.rs` | S16-S22 | 修改 | 注册 6 个新模块 |
| `crates/infra/src/gguf_reader.rs` | S16 | 新建 | GGUF 文件解析器（mmap） |
| `crates/infra/src/gguf_reader_tests.rs` | S16 | 新建 | 14 个测试 |
| `crates/infra/src/quant_blocks.rs` | S17 | 新建 | 量化块结构 + 反量化 |
| `crates/infra/src/quant_blocks_tests.rs` | S17 | 新建 | 14 个测试 |
| `crates/infra/src/gemv_kernel.rs` | S18 | 新建 | 自研 GEMV 内核（AVX2） |
| `crates/infra/src/gemv_kernel_tests.rs` | S18 | 新建 | 14 个测试 |
| `crates/infra/src/weight_repack.rs` | S19 | 新建 | CPU cache 友好权重重排 |
| `crates/infra/src/weight_repack_tests.rs` | S19 | 新建 | 6 个测试 |
| `crates/infra/src/local_llm.rs` | S20 | 修改 | 集成自研内核模式 |
| `crates/infra/src/local_llm_tests.rs` | S20, S23 | 修改 | 新增内核模式测试 + 性能基准 |
| `crates/infra/src/layer_prefetch.rs` | S21 | 新建 | Layer 级流式预取 |
| `crates/infra/src/layer_prefetch_tests.rs` | S21 | 新建 | 6 个测试 |
| `crates/infra/src/memory_budget.rs` | S22 | 新建 | RAM 预算管理 |
| `crates/infra/src/memory_budget_tests.rs` | S22 | 新建 | 8 个测试 |
| `crates/tauri-app/src/commands.rs` | S20 | 修改 | 新增 2 个 IPC 命令 |
| `crates/tauri-app/src/lib.rs` | S20 | 修改 | 注册新命令 |
| `crates/tauri-app/tests/integration.rs` | S20 | 修改 | 新增内核模式集成测试 |
| `docs/SRS_v1.0.md` | S23 | 修改 | 新增 REQ-LLM-007/008 |
| `docs/api/ipc-commands.md` | S23 | 修改 | 新增 2 个命令 |
| `docs/api/code-contracts.md` | S23 | 修改 | 新增契约 |
| `docs/api/settings-reference.md` | S23 | 修改 | 新增 llm.kernel_mode 键 |
| `docs/architecture/adr/ADR-011-local-llm-engine.md` | S23 | 修改 | Phase 3 完成 |
| `AGENTS.md` | S23 | 修改 | 架构更新 |
| `CLAUDE.md` | S23 | 修改 | 架构更新 |

---

## 附录 B：Phase 3 Settings 键完整定义

| 键 | 类型 | 默认值 | Session | 说明 |
|---|---|---|---|---|
| `llm.kernel_mode` | String | `"mistral"` | S20 | 推理内核：`mistral`（mistral.rs）/ `custom`（自研 GEMV） |

---

## 附录 C：Phase 3 IPC 命令完整定义

| 命令 | 参数 | 返回 | Session | Pro 门控 |
|---|---|---|---|---|
| `set_kernel_mode` | `mode: String` | `()` | S20 | ✅ |
| `get_kernel_mode` | — | `String` | S20 | ✅ |

---

## 附录 D：Phase 3 新增测试用例汇总

| Session | 测试 ID 前缀 | 测试数 | 说明 |
|---|---|---|---|
| S16 | TC-GGUF- | 14 | GGUF 文件解析 |
| S17 | TC-QUANT- | 14 | 量化块结构 + 反量化 |
| S18 | TC-GEMV- | 14 | GEMV 内核 |
| S19 | TC-REPACK- | 6 | 权重重排 |
| S20 | TC-INTEG- | 7 | 集成测试 |
| S21 | TC-PREFETCH- | 6 | 流式预取 |
| S22 | TC-MEM- | 8 | RAM 预算管理 |
| S23 | TC-PHASE3-PERF- | 4 (#[ignore]) | 性能基准 |
| **合计** | | **73** | |

---

## 附录 E：调研来源详细记录

| # | 来源 | URL | 调研日期 | 关键发现 |
|---|---|---|---|---|
| 1 | mistral.rs v0.9.0 源码 | `github.com/EricLBuehler/mistral.rs` tag v0.9.0 | 2026-07-31 | mistralrs-quant crate 已含 memmap2 依赖；GgufMatMul 委托 candle QMatMul |
| 2 | candle-core 量化模块 | `github.com/huggingface/candle` main `candle-core/src/quantized/` | 2026-07-31 | AVX2/NEON/SIMD128 内核已有但仅块级点积；Cow<[u8]> 支持零拷贝 |
| 3 | llama.cpp ggml CPU | `github.com/ggml-org/llama.cpp` `ggml/src/ggml-cpu/` | 2026-07-31 | quants.c/quants.h 有完整 GEMV 内核参考；repack.cpp 有 cache 友好重排 |
| 4 | herbert-rs | `github.com/xigh/herbert-rs` (33 stars) | 2026-07-31 | "hand-written AVX-512 and quantization" — 纯 Rust 手写 SIMD 量化 |
| 5 | pulsar | `github.com/giannisanni/pulsar` (72 stars) | 2026-07-31 | "SSD-streaming inference engine for giant MoE models" — 流式加载参考 |
| 6 | lattice | `github.com/ohdearquant/lattice` (38 stars) | 2026-07-31 | "Pure Rust, no Python, no CUDA" — Apple Silicon 量化 |
| 7 | Camelid | `github.com/timtoole02/Camelid` (161 stars) | 2026-07-31 | Rust-native local inference backend |
| 8 | mistral.rs GgufMatMul 源码 | `mistralrs-quant/src/gguf/mod.rs` v0.9.0 | 2026-07-31 | from_raw_uqff / from_qtensor / forward_raw 方法签名 |
| 9 | mistral.rs Content 源码 | `mistralrs-core/src/gguf/content.rs` v0.9.0 | 2026-07-31 | GGUF 多文件加载，from_readers API |
| 10 | candle GGUF 文件解析 | `candle-core/src/quantized/gguf_file.rs` main | 2026-07-31 | V1/V2/V3 格式，DEFAULT_ALIGNMENT=32 |

---

## 附录 F：后续工作衔接提示词（2026-08-01）

以下三个衔接提示词用于在新的 AI Session 中依次完成剩余工作。
每个提示词可独立粘贴使用，包含完整的上下文摘要和任务目标。

### F.1 Session A：Phase 3 S23 — 性能基准测试 + 文档同步

```
你正在继续 EchoMind 本地 LLM 引擎 Phase 3 的开发，执行最后一个 Session（S23）。

## 上下文
- EchoMind 是 Rust + Tauri v2 桌面本地 RAG 知识库应用（六边形架构，4 crate）
- Phase 3（自研量化内核 + 内存层次流式加载）Session 16-22 全部完成
- Pro 功能通过 `--features pro` 编译门控

### Phase 3 已完成的 8 个 Session
- S16: GGUF 文件解析器（mmap 零拷贝，V1/V2/V3）→ 14 测试
- S17: 量化块结构 + 反量化（Q4_0/Q4_K/Q5_K/Q6_K/Q8_0/Q8_K）→ 14 测试
- S18: 自研 GEMV 内核（4 种量化格式标量实现 + AVX2 预留）→ 14 测试
- S19: CPU cache 友好权重重排（Row-Major → Tile-Major）→ 6 测试
- S20: 集成 LocalLlmEngine（KernelMode 枚举 + set/get_kernel_mode IPC）→ 7 测试
- S21: Layer 级流式预取（madvise + 后台线程 + SendPtr）→ 6 测试（5+1 #[ignore]）
- S22: RAM 预算管理（LRU 驱逐 + libc::sysconf 系统内存检测）→ 13 测试

### 当前测试状态
- cargo test -p echomind-infra --features pro: 229 passed, 0 failed, 17 ignored
- cargo clippy -p echomind-infra --features pro --all-targets -- -D warnings: 零警告
- cargo fmt --check: 通过

### 同时完成的 Zed 架构借鉴（4 个模式）
1. Store 模式: crates/tauri-app/src/stores/（5 Store，state.rs 重构）
2. 类型安全 Settings: settings_registry.rs（21 枚举变体 + SettingsRegistry trait）
3. #[perf] 框架: crates/infra/src/perf.rs（PerfReport + PerfRunner）
4. Action 前端: ui/src/action.js（ActionRegistry 类 + keyboard.js 重构）

## 本 Session 目标
### 1. 性能基准测试（4 个 #[ignore] 测试）
在 `crates/infra/src/local_llm_tests.rs` 新增：
- TC-PHASE3-PERF-001: `test_gemv_vs_mistralrs_q8_0` — 自研 GEMV Q8_0 vs mistral.rs Q8_0 速度对比
- TC-PHASE3-PERF-002: `test_gemv_vs_mistralrs_q4_k` — 自研 GEMV Q4_K vs mistral.rs Q4_K 速度对比
- TC-PHASE3-PERF-003: `test_prefetch_effectiveness` — 有/无预取的首层加载延迟对比
- TC-PHASE3-PERF-004: `test_repack_cache_miss_reduction` — 重排前后 cache miss 对比

运行方式: `ECHOMIND_TEST_MODEL=/path/to/model.gguf cargo test -p echomind-infra --features pro -- phase3_perf --ignored --nocapture`

可利用已实现的 `perf::PerfRunner` 和 `perf::PerfReport` 框架（crates/infra/src/perf.rs）。

### 2. 文档全量同步
更新以下文档（Phase 3 完整变更 + Zed 模式变更）：
- docs/SRS_v1.0.md — 新增 REQ-LLM-007（自研量化内核）+ REQ-LLM-008（内存层次流式加载）
- docs/api/ipc-commands.md — 新增 set_kernel_mode / get_kernel_mode + set_local_model 等命令
- docs/api/code-contracts.md — 新增 GgufFile / QuantBlock / gemv_dispatch / LayerPrefetcher / MemoryBudget / PerfReport / PerfRunner / SettingKey / SettingsRegistry / ActionRegistry 契约
- docs/api/settings-reference.md — 新增 llm.kernel_mode 键 + SettingsRegistry schema_json
- docs/api/frontend-modules.md — 新增 ActionRegistry / createDefaultRegistry / setGlobalRegistry
- docs/architecture/adr/ADR-011-local-llm-engine.md — Phase 3 标记 ✅ + 完整实施详情
- AGENTS.md — 更新 local_llm.rs 描述（自研内核/流式加载）+ stores/ 目录 + settings_registry.rs + perf.rs
- CLAUDE.md — 同步 AGENTS.md 更新 + Pro feature gating 更新

## 必读文档
1. docs/architecture/LOCAL_LLM_ENGINE_PHASE3_ATOMIC_PLAN.md — Session 23 章节（§23.1-23.4）
2. crates/infra/src/local_llm_tests.rs — 现有测试（含 Phase 2 性能基准 TC-LLM-PERF-001~003）
3. crates/infra/src/perf.rs — PerfRunner + PerfReport 框架（可直接复用）
4. crates/infra/src/gemv_kernel.rs — 自研 GEMV 内核（基准对比目标）
5. crates/infra/src/layer_prefetch.rs — 流式预取（基准对比目标）
6. crates/tauri-app/src/stores/mod.rs — Store 模式（文档需同步）
7. crates/tauri-app/src/settings_registry.rs — 类型安全 Settings（文档需同步）

## 验证命令
```bash
cargo test                              # Free 全量
cargo test --features pro               # Pro 全量
cargo clippy --all-targets -- -D warnings                    # Free clippy
cargo clippy --all-targets --features pro -- -D warnings     # Pro clippy
cargo fmt --check
node scripts/build-ui.mjs
./sync-to-github.sh --commit
```

## 完成标志
- [ ] 4 个 #[ignore] 性能基准测试编写完成
- [ ] 全量测试通过（Free + Pro）
- [ ] Clippy 零警告（Free + Pro）
- [ ] cargo fmt --check 通过
- [ ] 8 个文档全部更新完成
- [ ] ADR-011 Phase 3 标记 ✅
- [ ] ./sync-to-github.sh --commit 执行成功
```

### F.2 Session B：GitHub 同步 — Zed 模式 + S21/S22 代码

```
你正在为 EchoMind 项目执行 GitHub 发布仓库同步。

## 上下文
- EchoMind 开发目录: /Users/john/freesoft/EchoMind/
- GitHub 发布仓库: /Users/john/freesoft/EchoMind/EchoMind/（git 子目录）
- 同步脚本: ./sync-to-github.sh（位于开发根目录）

### 本轮新增的源代码文件（需同步到发布仓库）
**Zed 架构借鉴（4 个模式）：**
1. crates/tauri-app/src/stores/mod.rs — Store 模式模块入口
2. crates/tauri-app/src/stores/document_store.rs — DocumentStore
3. crates/tauri-app/src/stores/chat_store.rs — ChatStore
4. crates/tauri-app/src/stores/security_store.rs — SecurityStore
5. crates/tauri-app/src/stores/model_store.rs — ModelStore
6. crates/tauri-app/src/stores/config_store.rs — ConfigStore
7. crates/tauri-app/src/settings_registry.rs — 类型安全 Settings 系统
8. crates/infra/src/perf.rs — #[perf] 基准测试框架
9. ui/src/action.js — Action 前端模式（ActionRegistry 类）

**Phase 3 S21/S22（如果 S23 尚未同步）：**
10. crates/infra/src/layer_prefetch.rs — Layer 级流式预取
11. crates/infra/src/layer_prefetch_tests.rs — 预取测试
12. crates/infra/src/memory_budget.rs — RAM 预算管理
13. crates/infra/src/memory_budget_tests.rs — 预算管理测试

**修改的源代码文件（sync-to-github.sh 自动处理）：**
- crates/tauri-app/src/state.rs — 重构为 Store 委托模式
- crates/tauri-app/src/lib.rs — 注册 stores + settings_registry 模块
- crates/tauri-app/src/commands.rs — 字段访问改为方法调用
- crates/tauri-app/tests/integration.rs — 同步字段访问修改
- crates/infra/src/lib.rs — 注册 perf + layer_prefetch + memory_budget 模块
- crates/infra/Cargo.toml — libc 依赖
- ui/src/keyboard.js — 重构为 Action 系统委托
- scripts/build-ui.mjs — 添加 action.js 到模块列表
- ui/index.html — 前端构建产物

## 任务
1. 执行 `./sync-to-github.sh` 检查变更
2. 如果 S23 已完成文档同步，确保文档变更也同步
3. 执行 `./sync-to-github.sh --commit` 自动提交
4. 进入 `cd EchoMind && git push` 推送到 GitHub

## 验证
- 发布仓库 git log 确认提交
- 发布仓库 git status 确认无残留变更
```

### F.3 Session C：自研 GEMV Transformer 前向传播 — chat_stream_custom() 完整实现

```
你正在为 EchoMind 实现自研 GEMV Transformer 前向传播，这是 Phase 3 的延伸工作。

## 上下文
- EchoMind 是 Rust + Tauri v2 桌面本地 RAG 知识库应用
- Phase 3 S16-S22 已完成自研量化基础设施（GGUF解析/量化块/GEMV内核/权重重排/流式预取/RAM预算）
- `chat_stream_custom()` 当前在 `crates/infra/src/local_llm.rs` 中返回错误（bail!）
- 需要实现完整的 Transformer 前向传播管道

### 现有基础设施（S16-S22 产出，可直接复用）
1. **GGUF 解析器** (`crates/infra/src/gguf_reader.rs`):
   - `GgufFile` — mmap 零拷贝解析，V1/V2/V3 格式
   - `GgufFile::tensor_data(name) -> Option<(&[u8], GgmlDType, Vec<usize>)>`
   - 张量名约定: `token_embd.weight`, `blk.0.attn_q.weight`, `blk.0.ffn_gate.weight` 等

2. **量化块** (`crates/infra/src/quant_blocks.rs`):
   - `QuantBlock` 枚举（借用引用，零拷贝）: Q4_0/Q4_K/Q5_K/Q8_0/Q8_K
   - `blocks_from_bytes(data, dtype, n, k) -> Vec<QuantBlock>`
   - `dequantize_q4_0/q4_k/q8_0/q8_k` 反量化到 f32

3. **GEMV 内核** (`crates/infra/src/gemv_kernel.rs`):
   - `gemv_dispatch(dtype, weights, input, output, n, k)` — dtype 路由 GEMV
   - `gemv_q4_0/q4_k/q8_0/q8_k` — 4 种量化格式标量 GEMV
   - `quantize_row_q8_0(input, output)` — f32 → Q8_0 输入预处理
   - 输入: f32 向量 → Q8_0 量化 → int8 点积 → f32 累加 → 输出 f32 向量

4. **权重重排** (`crates/infra/src/weight_repack.rs`):
   - `repack_for_gemv(data, dtype, n, k) -> RepackedWeights` — Row-Major → Tile-Major
   - `gemv_repacked_dispatch(repacked, input, output)` — cache 友好 GEMV

5. **流式预取** (`crates/infra/src/layer_prefetch.rs`):
   - `LayerPrefetcher` — madvise(MADV_WILLNEED) 后台预取
   - `layer_offsets_from_gguf(gguf) -> Vec<(String, u64, u64)>` 从 GGUF 提取偏移

6. **RAM 预算** (`crates/infra/src/memory_budget.rs`):
   - `MemoryBudget` — allocate/release/touch/evict_lru
   - `MemoryBudget::from_system(0.8)` — 基于系统 RAM 80% 创建

### 当前 chat_stream_custom() 状态
```rust
// crates/infra/src/local_llm.rs:452-468
async fn chat_stream_custom(
    &self,
    _system_prompt: &str,
    _history: &[ChatMessage],
    _query: &str,
) -> Result<BoxStream<'static, Result<String>>> {
    self.load_custom_weights().await?;
    bail!(
        "自研内核推理模式（CustomGemv）尚在开发中：权重加载 + 重排管道已就绪，\
         完整 transformer 前向传播尚未实现。请使用 mistral.rs 模式。"
    );
}
```

### LocalLlmEngine 结构体（已含 Phase 3 字段）
```rust
pub struct LocalLlmEngine {
    inner: Arc<Inner>,  // mistral.rs 引擎（MistralRs 模式使用）
    model_path: PathBuf,
    quantization: Quantization,
    // Phase 3 字段:
    kernel_mode: KernelMode,  // MistralRs / CustomGemv
    gguf_file: tokio::sync::RwLock<Option<Arc<GgufFile>>>,
    repacked_weights: tokio::sync::RwLock<Option<HashMap<String, RepackedWeights>>>,
}
```

## 目标：实现完整 Transformer 前向传播

### 需要实现的组件
1. **Tokenizer**: 从 GGUF 元数据提取 tokenizer（tokenizer.json 或 tokenizer.model）
   - 或复用 mistral.rs 的 tokenizer（如果可访问）
   - 至少支持 BPE/SentencePiece 编码 + decode

2. **Transformer 层结构**（参考 llama.cpp / mistral.rs 架构）:
   - `token_embd.weight` → token embedding 查表
   - `blk.{i}.attn_norm.weight` → RMSNorm（attention 前）
   - `blk.{i}.attn_q.weight` / `blk.{i}.attn_k.weight` / `blk.{i}.attn_v.weight` → QKV 投影
   - `blk.{i}.attn_output.weight` → 输出投影
   - `blk.{i}.ffn_norm.weight` → RMSNorm（FFN 前）
   - `blk.{i}.ffn_gate.weight` / `blk.{i}.ffn_up.weight` → SwiGLU gate/up
   - `blk.{i}.ffn_down.weight` → SwiGLU down
   - `output_norm.weight` → 最终 RMSNorm
   - `output.weight` → lm_head（logits 投影）

3. **前向传播管道**:
   ```
   token_ids → token_embd → [for each layer: RMSNorm → RoPE → QKV → Attention → Output → RMSNorm → SwiGLU] → final RMSNorm → output.weight → logits → argmax → next_token
   ```

4. **推理循环**:
   - 支持温度采样（复用 LlmSamplingParams）
   - 支持 KV cache（简单实现：每层缓存 K/V 向量）
   - 支持流式输出（通过 mpsc channel 逐 token 发送）
   - 支持中断（CancellationToken）

### 文件变更
- `crates/infra/src/local_llm.rs` — 替换 `chat_stream_custom()` 实现
- 可能新增 `crates/infra/src/custom_transformer.rs` — Transformer 组件实现
- 可能新增 `crates/infra/src/custom_transformer_tests.rs` — 组件单元测试

### 关键设计约束
- 不得引入新的外部 crate（使用已有的 memmap2/half/libc/tokenizers）
- 所有量化权重通过 GEMV 内核计算，不反量化全量权重
- 必须支持 Q4_K_M 和 Q8_0 两种量化格式
- 首版可以只支持 Llama/Qwen 架构（RoPE + RMSNorm + SwiGLU）
- 首版可以不做 PagedAttention（简单 KV cache 即可）
- 必须 `#[cfg(feature = "pro")]` 门控

### 性能目标
- 首 token 延迟: < 5s（3B Q4_K_M 模型，M1 Pro 16GB）
- 后续 token: > 5 tokens/sec
- 可通过 PerfRunner 框架测量

## 必读文档
1. crates/infra/src/local_llm.rs — 引擎主体（chat_stream_custom 在 L452）
2. crates/infra/src/gguf_reader.rs — GGUF 解析器（tensor_data 方法）
3. crates/infra/src/gemv_kernel.rs — GEMV 内核（gemv_dispatch）
4. crates/infra/src/quant_blocks.rs — 量化块结构
5. crates/infra/src/weight_repack.rs — 权重重排
6. crates/infra/src/layer_prefetch.rs — 流式预取
7. crates/infra/src/memory_budget.rs — RAM 预算
8. docs/architecture/LOCAL_LLM_ENGINE_PHASE3_ATOMIC_PLAN.md — Phase 3 完整规划

## 验证
```bash
# 单元测试
cargo test -p echomind-infra --features pro -- custom_transformer

# 集成测试（需要真实模型）
ECHOMIND_TEST_MODEL=/path/to/model.gguf \
cargo test -p echomind-infra --features pro -- chat_stream_custom --ignored --nocapture

# Clippy
cargo clippy -p echomind-infra --features pro --all-targets -- -D warnings

# 格式
cargo fmt --check
```
```

