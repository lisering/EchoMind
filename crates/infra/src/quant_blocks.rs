//! 量化块结构与反量化内核（Phase 3：自研量化内核 + 内存层次流式加载）。
//!
//! 定义 GGUF 量化格式的块结构体（BlockQ4_0/BlockQ4K/BlockQ5K/BlockQ6K/BlockQ8_0/BlockQ8K）
//! 并提供反量化（dequantize）方法，从 mmap 数据零拷贝提取量化权重块。
//!
//! # 架构定位
//!
//! 本模块是 Phase 3 的第二层（S17），在 S16 的 GGUF 解析器之上，将原始字节切片
//! 解析为强类型的量化块引用，为 S18 的 GEMV 内核提供操作对象。
//!
//! # 零拷贝设计
//!
//! 所有 Block 结构体使用 `#[repr(C)]` 保证内存布局与 GGUF 文件格式一致。
//! `QuantBlock<'a>` 枚举持有 `&'a [BlockXxx]` 借用引用，直接引用 mmap 区域，
//! 无需将权重数据拷贝到堆内存。
//!
//! # 参考实现
//!
//! - candle-core `candle-core/src/quantized/k_quants.rs` — Block 结构体定义 + `to_float` 反量化
//! - candle-core `candle-core/src/quantized/utils.rs` — `get_scale_min_k4` 6-bit scale/min 提取
//! - llama.cpp `ggml/src/ggml-cpu/quants.c` — `dequantize_row_*` 参考算法
//! - llama.cpp `ggml/src/ggml-common.h` — GGML 量化块格式规范

use anyhow::{Result, bail, ensure};
use half::f16;

use crate::gguf_reader::GgmlDType;

// ---------------------------------------------------------------------------
// 块大小常量（与 llama.cpp / candle-core 对齐）
// ---------------------------------------------------------------------------

/// Q4_0 块大小：每块 32 个元素。
pub const QK4_0: usize = 32;

/// Q8_0 块大小：每块 32 个元素。
pub const QK8_0: usize = 32;

/// K-quants 块大小：每块 256 个元素（Q2K~Q8K 通用）。
pub const QK_K: usize = 256;

/// K-quants 缩放因子数组大小：12 字节（编码 8 个 6-bit scale + 8 个 6-bit min）。
pub const K_SCALE_SIZE: usize = 12;

// ---------------------------------------------------------------------------
// 量化块结构体（#[repr(C)] 保证与 GGUF 二进制布局一致）
// ---------------------------------------------------------------------------

/// Q4_0 量化块：32 个元素，18 字节。
///
/// 4-bit 量化，每块 32 个权重值。d 为缩放因子，qs 存储 32 个 4-bit nibble（打包在 16 字节中）。
/// 反量化公式：`y = d * (q - 8)`，其中 q 为 4-bit 值（0~15）。
///
/// 内存布局与 llama.cpp `block_q4_0` / candle `BlockQ4_0` 完全一致。
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct BlockQ4_0 {
    /// 缩放因子 delta
    pub d: f16,
    /// 32 个 4-bit nibble，打包在 16 字节中
    pub qs: [u8; QK4_0 / 2],
}
const _: () = assert!(std::mem::size_of::<BlockQ4_0>() == 18);

/// Q8_0 量化块：32 个元素，34 字节。
///
/// 8-bit 量化，每块 32 个权重值。d 为缩放因子，qs 存储 32 个 int8 值。
/// 反量化公式：`y = d * q`，其中 q 为 int8 值（-128~127）。
///
/// 内存布局与 llama.cpp `block_q8_0` / candle `BlockQ8_0` 完全一致。
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct BlockQ8_0 {
    /// 缩放因子 delta
    pub d: f16,
    /// 32 个 int8 量化值
    pub qs: [i8; QK8_0],
}
const _: () = assert!(std::mem::size_of::<BlockQ8_0>() == 34);

/// Q4_K 量化块：256 个元素，144 字节。
///
/// 4-bit K-quant，每块 256 个权重值。分为 8 个子块（各 32 元素），每个子块有独立的
/// 6-bit scale 和 6-bit min。d/dmin 为超块缩放因子。
///
/// 反量化公式：`y = d * sc * q - dmin * m`，其中 sc/m 为 6-bit scale/min，q 为 4-bit nibble。
///
/// 内存布局与 llama.cpp `block_q4_K` / candle `BlockQ4K` 完全一致。
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct BlockQ4K {
    /// 超块缩放因子（用于 scale）
    pub d: f16,
    /// 超块缩放因子（用于 min）
    pub dmin: f16,
    /// 6-bit scale/min 编码（12 字节 = 8×6 bit scales + 8×6 bit mins）
    pub scales: [u8; K_SCALE_SIZE],
    /// 256 个 4-bit nibble，打包在 128 字节中
    pub qs: [u8; QK_K / 2],
}
const _: () = assert!(std::mem::size_of::<BlockQ4K>() == 144);

/// Q5_K 量化块：256 个元素，176 字节。
///
/// 5-bit K-quant，每块 256 个权重值。布局与 Q4_K 类似，但额外有 32 字节 qh 存储
/// 每个元素的第 5 位（高 1 bit），使每个值可达 0~31。
///
/// 反量化公式：`y = d * sc * (q + 16*high_bit) - dmin * m`。
///
/// 内存布局与 llama.cpp `block_q5_K` / candle `BlockQ5K` 完全一致。
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct BlockQ5K {
    /// 超块缩放因子（用于 scale）
    pub d: f16,
    /// 超块缩放因子（用于 min）
    pub dmin: f16,
    /// 6-bit scale/min 编码（12 字节）
    pub scales: [u8; K_SCALE_SIZE],
    /// 每个元素的高 1 bit（256 bit = 32 字节）
    pub qh: [u8; QK_K / 8],
    /// 256 个 4-bit nibble（低 4 bit），打包在 128 字节中
    pub qs: [u8; QK_K / 2],
}
const _: () = assert!(std::mem::size_of::<BlockQ5K>() == 176);

/// Q6_K 量化块：256 个元素，210 字节。
///
/// 6-bit K-quant，每块 256 个权重值。ql 存储低 4 bit，qh 存储高 2 bit（共 6 bit）。
/// scales 为 16 个 int8 子块缩放因子（每 16 元素一个 scale）。
///
/// 反量化公式：`y = d * scale * (q - 32)`，其中 q 为 6-bit 值（0~63，减 32 居中到 -32~31）。
///
/// 内存布局与 llama.cpp `block_q6_K` / candle `BlockQ6K` 完全一致。
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct BlockQ6K {
    /// 256 个 4-bit nibble（低 4 bit），打包在 128 字节中
    pub ql: [u8; QK_K / 2],
    /// 每个元素的高 2 bit（512 bit = 64 字节）
    pub qh: [u8; QK_K / 4],
    /// 16 个 int8 子块缩放因子
    pub scales: [i8; QK_K / 16],
    /// 超块缩放因子
    pub d: f16,
}
const _: () = assert!(std::mem::size_of::<BlockQ6K>() == 210);

/// Q8_K 量化块：256 个元素，292 字节。
///
/// 8-bit K-quant，每块 256 个权重值。d 为 f32 缩放因子（注意：Q8_K 用 f32 而非 f16）。
/// bsums 存储每个 16 元素子块的和（用于点积优化），反量化时不需要。
///
/// 反量化公式：`y = d * q`，其中 q 为 int8 值。
///
/// 内存布局与 llama.cpp `block_q8_K` / candle `BlockQ8K` 完全一致。
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct BlockQ8K {
    /// 缩放因子 delta（注意：Q8_K 使用 f32 而非 f16）
    pub d: f32,
    /// 256 个 int8 量化值
    pub qs: [i8; QK_K],
    /// 16 个子块和（每 16 元素一个 i16，用于点积优化，反量化时不使用）
    pub bsums: [i16; QK_K / 16],
}
const _: () = assert!(std::mem::size_of::<BlockQ8K>() == 292);

// ---------------------------------------------------------------------------
// QuantBlock 枚举（借用引用，零拷贝）
// ---------------------------------------------------------------------------

/// 借用引用的量化块枚举（零拷贝，直接引用 mmap 区域）。
///
/// 持有 `&'a [BlockXxx]` 切片引用，指向 mmap 映射的 GGUF 张量数据。
/// 通过 `blocks_from_bytes()` 从原始字节切片创建，无需数据拷贝。
#[derive(Debug, Clone, Copy)]
pub enum QuantBlock<'a> {
    /// Q4_0 量化块引用
    Q4_0(&'a [BlockQ4_0]),
    /// Q4_K 量化块引用
    Q4K(&'a [BlockQ4K]),
    /// Q5_K 量化块引用
    Q5K(&'a [BlockQ5K]),
    /// Q6_K 量化块引用
    Q6K(&'a [BlockQ6K]),
    /// Q8_0 量化块引用
    Q8_0(&'a [BlockQ8_0]),
    /// Q8_K 量化块引用
    Q8K(&'a [BlockQ8K]),
}

impl<'a> QuantBlock<'a> {
    /// 将此量化块反量化到 f32 输出缓冲区。
    ///
    /// # 参数
    ///
    /// - `out`：输出缓冲区，长度必须等于 `self.elem_count()`
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
    ///
    /// 每个块的元素数 = 块数 × 块大小（Q4_0/Q8_0: 32, Q4K/Q5K/Q6K/Q8K: 256）。
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

// ---------------------------------------------------------------------------
// 辅助函数
// ---------------------------------------------------------------------------

/// 从 K-quants scales 数组提取第 j 个 6-bit scale 和 6-bit min。
///
/// scales 数组共 12 字节，编码 8 个 6-bit scale（j=0..7）和 8 个 6-bit min。
///
/// 编码布局（与 llama.cpp/candle `get_scale_min_k4` 一致）：
/// - j < 4: scale = scales[j] & 63, min = scales[j+4] & 63
/// - j >= 4: scale = (scales[j+4] & 0xF) | ((scales[j-4] >> 6) << 4),
///   min = (scales[j+4] >> 4) | ((scales[j] >> 6) << 4)
pub(crate) fn get_scale_min_k4(j: usize, scales: &[u8]) -> (u8, u8) {
    if j < 4 {
        let d = scales[j] & 63;
        let m = scales[j + 4] & 63;
        (d, m)
    } else {
        let d = (scales[j + 4] & 0x0F) | ((scales[j - 4] >> 6) << 4);
        let m = (scales[j + 4] >> 4) | ((scales[j] >> 6) << 4);
        (d, m)
    }
}

/// 将原始字节切片安全转换为 Block 结构体切片引用。
///
/// 执行对齐检查和长度检查，确保 `&[u8]` 可以安全地 `from_raw_parts` 为 `&[T]`。
///
/// # 类型参数
///
/// - `T`：Block 结构体，必须 `#[repr(C)]` 且仅包含 plain data（u8/i8/f16/f32/i16）
///
/// # 错误
///
/// - 数据长度不是 `size_of::<T>()` 的整数倍
/// - 数据指针未对齐到 `align_of::<T>()`
fn cast_blocks<T>(data: &[u8]) -> Result<&[T]>
where
    T: Sized,
{
    let block_size = std::mem::size_of::<T>();
    ensure!(block_size > 0, "zero-sized type in cast_blocks");
    if data.is_empty() {
        return Ok(&[]);
    }
    ensure!(
        data.len().is_multiple_of(block_size),
        "data length {} is not a multiple of block size {}",
        data.len(),
        block_size
    );
    let addr = data.as_ptr() as usize;
    let align = std::mem::align_of::<T>();
    ensure!(
        addr.is_multiple_of(align),
        "data pointer is not aligned to {} (addr = {:#x})",
        align,
        addr
    );
    let len = data.len() / block_size;
    // SAFETY: data 指针已验证对齐，长度已验证为 block_size 整数倍，
    // 且 T 为 #[repr(C)] 仅含 plain data 类型（u8/i8/f16/f32/i16）。
    let blocks = unsafe { std::slice::from_raw_parts(data.as_ptr() as *const T, len) };
    Ok(blocks)
}

// ---------------------------------------------------------------------------
// blocks_from_bytes + block_count
// ---------------------------------------------------------------------------

/// 从原始字节切片解析量化块，返回借用引用的 `QuantBlock`（零拷贝）。
///
/// 根据 `dtype` 将 `data` 字节切片解释为对应类型的 Block 结构体切片。
/// 返回的 `QuantBlock` 持有对原始数据的借用引用，无数据拷贝。
///
/// # 参数
///
/// - `data`：原始字节切片（通常来自 `GgufFile::tensor_data()` 的 mmap 区域）
/// - `dtype`：量化数据类型
///
/// # 错误
///
/// - 数据长度不是对应块大小的整数倍
/// - 数据指针未对齐
/// - 不支持的 dtype（F32/F16/BF16/Q4_1/Q5_0/Q5_1/Q8_1/Q2K/Q3K/Unknown）
pub fn blocks_from_bytes(data: &[u8], dtype: GgmlDType) -> Result<QuantBlock<'_>> {
    match dtype {
        GgmlDType::Q4_0 => {
            let blocks = cast_blocks::<BlockQ4_0>(data)?;
            Ok(QuantBlock::Q4_0(blocks))
        }
        GgmlDType::Q4K => {
            let blocks = cast_blocks::<BlockQ4K>(data)?;
            Ok(QuantBlock::Q4K(blocks))
        }
        GgmlDType::Q5K => {
            let blocks = cast_blocks::<BlockQ5K>(data)?;
            Ok(QuantBlock::Q5K(blocks))
        }
        GgmlDType::Q6K => {
            let blocks = cast_blocks::<BlockQ6K>(data)?;
            Ok(QuantBlock::Q6K(blocks))
        }
        GgmlDType::Q8_0 => {
            let blocks = cast_blocks::<BlockQ8_0>(data)?;
            Ok(QuantBlock::Q8_0(blocks))
        }
        GgmlDType::Q8K => {
            let blocks = cast_blocks::<BlockQ8K>(data)?;
            Ok(QuantBlock::Q8K(blocks))
        }
        _ => bail!("unsupported dtype for blocks_from_bytes: {:?}", dtype),
    }
}

/// 根据元素数计算块数。
///
/// 对于块类型（Q4_0/Q8_0: 块大小 32，K-quants: 块大小 256），
/// 返回 `elem_count / block_size`。对于非块类型返回 `elem_count`。
///
/// # 参数
///
/// - `dtype`：量化数据类型
/// - `elem_count`：元素总数
pub fn block_count(dtype: GgmlDType, elem_count: usize) -> usize {
    let (block_size, _) = dtype.block_layout();
    if block_size == 0 {
        // 非块类型（F32/F16/BF16）：每元素独立，"块数" = 元素数
        elem_count
    } else {
        elem_count / block_size as usize
    }
}

// ---------------------------------------------------------------------------
// 反量化函数
// ---------------------------------------------------------------------------

/// Q4_0 反量化：将 Q4_0 块数组转换为 f32 浮点数组。
///
/// 反量化公式：`y = d * (q - 8)`，其中 q 为 4-bit nibble 值（0~15）。
///
/// Nibble 布局（与 llama.cpp/candle 一致）：
/// - `qs[j] & 0x0F` → 元素 j（0..15）
/// - `qs[j] >> 4` → 元素 j + 16（16..31）
///
/// # 参数
///
/// - `blocks`：Q4_0 块数组
/// - `out`：输出缓冲区，长度必须为 `blocks.len() * QK4_0`
pub fn dequantize_q4_0(blocks: &[BlockQ4_0], out: &mut [f32]) {
    let expected_len = blocks.len() * QK4_0;
    debug_assert_eq!(
        out.len(),
        expected_len,
        "dequantize_q4_0: output length {} != expected {}",
        out.len(),
        expected_len
    );

    for (i, block) in blocks.iter().enumerate() {
        let d = block.d.to_f32();
        for j in 0..(QK4_0 / 2) {
            let x0 = (block.qs[j] & 0x0F) as i16 - 8;
            let x1 = (block.qs[j] >> 4) as i16 - 8;
            out[i * QK4_0 + j] = x0 as f32 * d;
            out[i * QK4_0 + j + QK4_0 / 2] = x1 as f32 * d;
        }
    }
}

/// Q8_0 反量化：将 Q8_0 块数组转换为 f32 浮点数组。
///
/// 反量化公式：`y = d * q`，其中 q 为 int8 值（-128~127）。
///
/// # 参数
///
/// - `blocks`：Q8_0 块数组
/// - `out`：输出缓冲区，长度必须为 `blocks.len() * QK8_0`
pub fn dequantize_q8_0(blocks: &[BlockQ8_0], out: &mut [f32]) {
    let expected_len = blocks.len() * QK8_0;
    debug_assert_eq!(
        out.len(),
        expected_len,
        "dequantize_q8_0: output length {} != expected {}",
        out.len(),
        expected_len
    );

    for (i, block) in blocks.iter().enumerate() {
        let d = block.d.to_f32();
        for j in 0..QK8_0 {
            out[i * QK8_0 + j] = block.qs[j] as f32 * d;
        }
    }
}

/// Q4_K 反量化：将 Q4_K 块数组转换为 f32 浮点数组。
///
/// 反量化公式：`y = d * sc * q - dmin * m`，其中：
/// - d/dmin 为超块缩放因子（f16）
/// - sc/m 为子块 6-bit scale/min（从 scales 数组提取）
/// - q 为 4-bit nibble 值（0~15）
///
/// 块内布局：256 元素分为 4 组（各 64 元素），每组用 32 字节 qs。
/// 每组内：前 32 元素用低 nibble，后 32 元素用高 nibble。
/// 每组有 2 个子块（各 32 元素），各有独立的 scale/min。
///
/// # 参数
///
/// - `blocks`：Q4_K 块数组
/// - `out`：输出缓冲区，长度必须为 `blocks.len() * QK_K`
pub fn dequantize_q4_k(blocks: &[BlockQ4K], out: &mut [f32]) {
    let expected_len = blocks.len() * QK_K;
    debug_assert_eq!(
        out.len(),
        expected_len,
        "dequantize_q4_k: output length {} != expected {}",
        out.len(),
        expected_len
    );

    for (i, block) in blocks.iter().enumerate() {
        let d = block.d.to_f32();
        let min = block.dmin.to_f32();
        let q = &block.qs;
        let mut is = 0usize;
        let mut ys_index = 0usize;
        let base = i * QK_K;

        for j in (0..QK_K).step_by(64) {
            let q_chunk = &q[j / 2..j / 2 + 32];
            let (sc, m) = get_scale_min_k4(is, &block.scales);
            let d1 = d * sc as f32;
            let m1 = min * m as f32;
            let (sc, m) = get_scale_min_k4(is + 1, &block.scales);
            let d2 = d * sc as f32;
            let m2 = min * m as f32;
            for &ql in q_chunk {
                out[base + ys_index] = d1 * (ql & 0x0F) as f32 - m1;
                ys_index += 1;
            }
            for &ql in q_chunk {
                out[base + ys_index] = d2 * (ql >> 4) as f32 - m2;
                ys_index += 1;
            }
            is += 2;
        }
    }
}

/// Q5_K 反量化：将 Q5_K 块数组转换为 f32 浮点数组。
///
/// 反量化公式：`y = d * sc * (q + 16*high_bit) - dmin * m`，其中：
/// - d/dmin 为超块缩放因子（f16）
/// - sc/m 为子块 6-bit scale/min
/// - q 为 4-bit nibble 值（0~15）
/// - high_bit 为 qh 中的第 5 位（0 或 1），使值范围扩展到 0~31
///
/// qh 布局：32 字节，每个字节贡献 8 个 high bit（2 bit/组 × 4 组）。
/// 每组用 u1/u2 掩码选择不同的 bit 位。
///
/// # 参数
///
/// - `blocks`：Q5_K 块数组
/// - `out`：输出缓冲区，长度必须为 `blocks.len() * QK_K`
pub fn dequantize_q5_k(blocks: &[BlockQ5K], out: &mut [f32]) {
    let expected_len = blocks.len() * QK_K;
    debug_assert_eq!(
        out.len(),
        expected_len,
        "dequantize_q5_k: output length {} != expected {}",
        out.len(),
        expected_len
    );

    for (i, block) in blocks.iter().enumerate() {
        let d = block.d.to_f32();
        let min = block.dmin.to_f32();
        let ql_arr = &block.qs;
        let qh = &block.qh;
        let mut is = 0usize;
        let mut u1 = 1u8;
        let mut u2 = 2u8;
        let mut ys_index = 0usize;
        let base = i * QK_K;

        for j in (0..QK_K).step_by(64) {
            let ql_chunk = &ql_arr[j / 2..j / 2 + 32];
            let (sc, m) = get_scale_min_k4(is, &block.scales);
            let d1 = d * sc as f32;
            let m1 = min * m as f32;
            let (sc, m) = get_scale_min_k4(is + 1, &block.scales);
            let d2 = d * sc as f32;
            let m2 = min * m as f32;
            for (k, &ql) in ql_chunk.iter().enumerate() {
                let to_add = if qh[k] & u1 != 0 { 16.0f32 } else { 0.0f32 };
                out[base + ys_index] = d1 * ((ql & 0x0F) as f32 + to_add) - m1;
                ys_index += 1;
            }
            for (k, &ql) in ql_chunk.iter().enumerate() {
                let to_add = if qh[k] & u2 != 0 { 16.0f32 } else { 0.0f32 };
                out[base + ys_index] = d2 * ((ql >> 4) as f32 + to_add) - m2;
                ys_index += 1;
            }
            is += 2;
            u1 <<= 2;
            u2 <<= 2;
        }
    }
}

/// Q6_K 反量化：将 Q6_K 块数组转换为 f32 浮点数组。
///
/// 反量化公式：`y = d * scale * (q - 32)`，其中：
/// - d 为超块缩放因子（f16）
/// - scale 为 int8 子块缩放因子（每 16 元素一个）
/// - q 为 6-bit 值（0~63，减 32 居中到 -32~31），由 ql 低 4 bit + qh 高 2 bit 组成
///
/// 块内布局：256 元素分为 2 组（各 128 元素）。每组用 64 字节 ql + 32 字节 qh + 8 个 scale。
/// 每 32 元素为一个子块（l=0..32），is=l/16 决定 scale 索引。
///
/// # 参数
///
/// - `blocks`：Q6_K 块数组
/// - `out`：输出缓冲区，长度必须为 `blocks.len() * QK_K`
pub fn dequantize_q6_k(blocks: &[BlockQ6K], out: &mut [f32]) {
    let expected_len = blocks.len() * QK_K;
    debug_assert_eq!(
        out.len(),
        expected_len,
        "dequantize_q6_k: output length {} != expected {}",
        out.len(),
        expected_len
    );

    for (i, block) in blocks.iter().enumerate() {
        let d = block.d.to_f32();
        let ql = &block.ql;
        let qh = &block.qh;
        let sc = &block.scales;
        let base = i * QK_K;

        for n in (0..QK_K).step_by(128) {
            let idx = n / 128;
            let sc_chunk = &sc[8 * idx..];
            let ql_chunk = &ql[64 * idx..];
            let qh_chunk = &qh[32 * idx..];
            for l in 0..32 {
                let is = l / 16;
                let q1 = ((ql_chunk[l] & 0x0F) | ((qh_chunk[l] & 0x03) << 4)) as i8 - 32;
                let q2 =
                    ((ql_chunk[l + 32] & 0x0F) | (((qh_chunk[l] >> 2) & 0x03) << 4)) as i8 - 32;
                let q3 = ((ql_chunk[l] >> 4) | (((qh_chunk[l] >> 4) & 0x03) << 4)) as i8 - 32;
                let q4 = ((ql_chunk[l + 32] >> 4) | (((qh_chunk[l] >> 6) & 0x03) << 4)) as i8 - 32;
                out[base + n + l] = d * sc_chunk[is] as f32 * q1 as f32;
                out[base + n + l + 32] = d * sc_chunk[is + 2] as f32 * q2 as f32;
                out[base + n + l + 64] = d * sc_chunk[is + 4] as f32 * q3 as f32;
                out[base + n + l + 96] = d * sc_chunk[is + 6] as f32 * q4 as f32;
            }
        }
    }
}

/// Q8_K 反量化：将 Q8_K 块数组转换为 f32 浮点数组。
///
/// 反量化公式：`y = d * q`，其中 d 为 f32 缩放因子，q 为 int8 值。
/// 注意：Q8_K 使用 f32（而非 f16）存储缩放因子，bsums 字段不参与反量化。
///
/// # 参数
///
/// - `blocks`：Q8_K 块数组
/// - `out`：输出缓冲区，长度必须为 `blocks.len() * QK_K`
pub fn dequantize_q8_k(blocks: &[BlockQ8K], out: &mut [f32]) {
    let expected_len = blocks.len() * QK_K;
    debug_assert_eq!(
        out.len(),
        expected_len,
        "dequantize_q8_k: output length {} != expected {}",
        out.len(),
        expected_len
    );

    for (i, block) in blocks.iter().enumerate() {
        let d = block.d;
        for j in 0..QK_K {
            out[i * QK_K + j] = d * block.qs[j] as f32;
        }
    }
}
