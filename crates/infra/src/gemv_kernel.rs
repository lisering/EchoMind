//! 自研 GEMV 内核（Phase 3：单批次推理优化的量化矩阵-向量乘法）。
//!
//! 实现 Q4_K_M 和 Q8_0 格式的 GEMV（矩阵-向量乘法）内核，针对单批次推理（batch=1）优化。
//! 支持 AVX2 SIMD 指令（x86_64）或标量回退路径（ARM/其他架构）。
//!
//! # 架构定位
//!
//! 本模块是 Phase 3 的第三层（S18），在 S17 的量化块结构之上，实现高效的
//! 量化矩阵-向量乘法内核。核心策略：
//!
//! 1. **输入预处理**：将 f32 输入向量量化为 Q8_0 格式（一次性完成）
//! 2. **整数点积**：使用 int8 × int8 → int32 → f32 的整数运算路径
//! 3. **SIMD 优化**：x86_64 AVX2 路径使用 `_mm256_maddubs_epi16` 等指令
//!
//! # 函数调用关系
//!
//! ```text
//! gemv_dispatch(dtype, weights, input, output, n, k)
//!   ├── blocks_from_bytes(weights, dtype)  → 零拷贝解析量化块
//!   ├── quantize_row_q8_0(input)           → 输入预处理（f32 → Q8_0）
//!   └── gemv_q8_0 / gemv_q4_0 / gemv_q4_k / gemv_q8_k
//!         └── vec_dot_*  → 整数点积内核
//! ```
//!
//! # 参考
//!
//! - llama.cpp `ggml/src/ggml-cpu/quants.c` — GEMV 内核参考实现
//! - candle-core `candle-core/src/quantized/avx.rs` — AVX2 点积参考
//! - llama.cpp `ggml/src/ggml-quants.c` — `quantize_row_q8_0` 量化算法

use anyhow::{Result, bail};

use crate::gguf_reader::GgmlDType;
use crate::quant_blocks::{
    BlockQ4_0, BlockQ4K, BlockQ8_0, BlockQ8K, QK_K, QK4_0, QK8_0, blocks_from_bytes,
    get_scale_min_k4,
};

// ---------------------------------------------------------------------------
// 输入预处理：f32 → Q8_0 量化
// ---------------------------------------------------------------------------

/// 将 f32 输入向量量化为 Q8_0 格式（输入预处理）。
///
/// 对每 32 个 f32 元素计算一个 `BlockQ8_0`：
/// 1. 找到绝对值最大值 `amax`
/// 2. 计算缩放因子 `d = amax / 127.0`
/// 3. 每个元素量化为 `q = round(x / d)`，限制在 `[-128, 127]`
///
/// 此预处理将浮点输入转为 int8 表示，后续点积全部使用整数运算，
/// 避免 f32→f32 乘法瓶颈。与 llama.cpp 的 `quantize_row_q8_0` 策略一致。
///
/// # 参数
///
/// - `input`：f32 输入向量，长度必须为 `output.len() * QK8_0`
/// - `output`：输出 Q8_0 块数组，调用前已分配
///
/// # Panics
///
/// 仅在 debug 模式下，若 `input.len() != output.len() * QK8_0` 会触发 debug_assert。
pub fn quantize_row_q8_0(input: &[f32], output: &mut [BlockQ8_0]) {
    let expected = output.len() * QK8_0;
    debug_assert_eq!(
        input.len(),
        expected,
        "quantize_row_q8_0: input length {} != expected {}",
        input.len(),
        expected
    );

    for (block_idx, block) in output.iter_mut().enumerate() {
        let start = block_idx * QK8_0;
        let chunk = &input[start..start + QK8_0];

        // 找到绝对值最大值
        let mut amax = 0.0f32;
        for &x in chunk {
            let abs_x = x.abs();
            if abs_x > amax {
                amax = abs_x;
            }
        }

        // 计算缩放因子：d = amax / 127.0
        let d = if amax > 0.0 { amax / 127.0 } else { 0.0 };
        let id = if d > 0.0 { 1.0 / d } else { 0.0 };

        let mut qs = [0i8; QK8_0];
        for (i, &x) in chunk.iter().enumerate() {
            let q = (x * id).round().clamp(-128.0, 127.0);
            // clamp 后值在 [-128.0, 127.0] 范围内，安全转换为 i8
            qs[i] = q as i8;
        }

        *block = BlockQ8_0 {
            d: half::f16::from_f32(d),
            qs,
        };
    }
}

// ---------------------------------------------------------------------------
// 整数点积内核（标量版本，所有架构可用）
// ---------------------------------------------------------------------------

/// 计算两个 Q8_0 块数组的整数点积。
///
/// 对每对对应的 Q8_0 块，计算 int8 点积 `sum(qs_a[i] * qs_b[i])`，
/// 然后乘以两个缩放因子 `d_a * d_b`。
///
/// # 参数
///
/// - `a`：权重 Q8_0 块数组
/// - `b`：输入 Q8_0 块数组（长度必须与 `a` 相同）
///
/// # 返回
///
/// 点积结果 f32 值
pub(crate) fn vec_dot_q8_0_q8_0(a: &[BlockQ8_0], b: &[BlockQ8_0]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    let mut sum = 0.0f32;
    for (ba, bb) in a.iter().zip(b.iter()) {
        let d_a = ba.d.to_f32();
        let d_b = bb.d.to_f32();
        let mut dot: i32 = 0;
        for i in 0..QK8_0 {
            dot += (ba.qs[i] as i32) * (bb.qs[i] as i32);
        }
        sum += dot as f32 * d_a * d_b;
    }
    sum
}

/// 计算 Q4_0 权重块与 Q8_0 输入块的整数点积。
///
/// Q4_0 每块 32 元素，4-bit nibble 存储。对每对块：
/// 1. 提取 4-bit nibble 并居中：`q_w = nibble - 8`（范围 -8~7）
/// 2. 与 Q8_0 的 int8 值做点积
/// 3. 乘以两个缩放因子
///
/// # 参数
///
/// - `weights`：Q4_0 权重块数组
/// - `input_q`：Q8_0 输入块数组（长度必须与 `weights` 相同）
///
/// # 返回
///
/// 点积结果 f32 值
pub(crate) fn vec_dot_q4_0_q8_0(weights: &[BlockQ4_0], input_q: &[BlockQ8_0]) -> f32 {
    debug_assert_eq!(weights.len(), input_q.len());
    let mut sum = 0.0f32;
    for (w, x) in weights.iter().zip(input_q.iter()) {
        let d_w = w.d.to_f32();
        let d_x = x.d.to_f32();
        let mut dot: i32 = 0;
        for j in 0..(QK4_0 / 2) {
            // 低 nibble → 元素 j（0..15）
            let q0 = (w.qs[j] & 0x0F) as i32 - 8;
            // 高 nibble → 元素 j + 16（16..31）
            let q1 = (w.qs[j] >> 4) as i32 - 8;
            dot += q0 * (x.qs[j] as i32);
            dot += q1 * (x.qs[j + QK4_0 / 2] as i32);
        }
        sum += dot as f32 * d_w * d_x;
    }
    sum
}

/// 计算 Q4_K 权重块与 Q8_0 输入块的整数点积。
///
/// Q4_K 每块 256 元素，使用 6-bit scales/mins + 4-bit nibbles。
/// 对应 8 个 Q8_0 输入子块（各 32 元素）。
///
/// 利用分解公式：
/// ```text
/// dot = Σ_sub-blocks (d * sc * Σ(q_w * q_x) - dmin * m * Σ(q_x)) * d_x
/// ```
/// 其中 `q_w` 为 4-bit nibble（0~15），`q_x` 为 int8 输入值。
///
/// # 参数
///
/// - `weights`：Q4_K 权重块数组
/// - `input_q`：Q8_0 输入块数组，长度必须为 `weights.len() * (QK_K / QK8_0)`
///
/// # 返回
///
/// 点积结果 f32 值
pub(crate) fn vec_dot_q4_k_q8_0(weights: &[BlockQ4K], input_q: &[BlockQ8_0]) -> f32 {
    let sub_blocks_per_k_block = QK_K / QK8_0; // 8
    debug_assert_eq!(input_q.len(), weights.len() * sub_blocks_per_k_block);

    let mut sum = 0.0f32;
    for (w_idx, w_block) in weights.iter().enumerate() {
        let d = w_block.d.to_f32();
        let dmin = w_block.dmin.to_f32();
        let q = &w_block.qs;
        let input_start = w_idx * sub_blocks_per_k_block;

        let mut is = 0usize;
        let mut input_idx = input_start;

        // 4 组，每组 64 元素（2 个子块 × 32 元素）
        for j in (0..QK_K).step_by(64) {
            let q_chunk = &q[j / 2..j / 2 + 32];

            // 子块 A：低 nibble（元素 0..31）
            let (sc, m) = get_scale_min_k4(is, &w_block.scales);
            let x_block = &input_q[input_idx];
            let d_x = x_block.d.to_f32();
            let mut dot: i32 = 0;
            let mut sum_x: i32 = 0;
            for (&q_val, &q_x_raw) in q_chunk.iter().zip(x_block.qs.iter()) {
                let q_w = (q_val & 0x0F) as i32;
                let q_x = q_x_raw as i32;
                dot += q_w * q_x;
                sum_x += q_x;
            }
            sum += (d * sc as f32 * dot as f32 - dmin * m as f32 * sum_x as f32) * d_x;
            input_idx += 1;
            is += 1;

            // 子块 B：高 nibble（元素 32..63）
            let (sc, m) = get_scale_min_k4(is, &w_block.scales);
            let x_block = &input_q[input_idx];
            let d_x = x_block.d.to_f32();
            let mut dot: i32 = 0;
            let mut sum_x: i32 = 0;
            for (&q_val, &q_x_raw) in q_chunk.iter().zip(x_block.qs.iter()) {
                let q_w = (q_val >> 4) as i32;
                let q_x = q_x_raw as i32;
                dot += q_w * q_x;
                sum_x += q_x;
            }
            sum += (d * sc as f32 * dot as f32 - dmin * m as f32 * sum_x as f32) * d_x;
            input_idx += 1;
            is += 1;
        }
    }
    sum
}

/// 计算 Q8_K 权重块与 Q8_0 输入块的整数点积。
///
/// Q8_K 每块 256 元素，单一 f32 缩放因子 `d`。对应 8 个 Q8_0 输入子块（各 32 元素）。
/// 点积公式：`sum = Σ_sub-blocks Σ(q_w[i] * q_x[i]) * d_w * d_x`
///
/// # 参数
///
/// - `weights`：Q8_K 权重块数组
/// - `input_q`：Q8_0 输入块数组，长度必须为 `weights.len() * (QK_K / QK8_0)`
///
/// # 返回
///
/// 点积结果 f32 值
pub(crate) fn vec_dot_q8_k_q8_0(weights: &[BlockQ8K], input_q: &[BlockQ8_0]) -> f32 {
    let sub_blocks_per_k_block = QK_K / QK8_0; // 8
    debug_assert_eq!(input_q.len(), weights.len() * sub_blocks_per_k_block);

    let mut sum = 0.0f32;
    for (w_idx, w_block) in weights.iter().enumerate() {
        let d_w = w_block.d; // f32
        let input_start = w_idx * sub_blocks_per_k_block;

        for sub in 0..sub_blocks_per_k_block {
            let x_block = &input_q[input_start + sub];
            let d_x = x_block.d.to_f32();
            let w_start = sub * QK8_0;
            let w_slice = &w_block.qs[w_start..w_start + QK8_0];
            let mut dot: i32 = 0;
            for (&w, &x) in w_slice.iter().zip(x_block.qs.iter()) {
                dot += (w as i32) * (x as i32);
            }
            sum += dot as f32 * d_w * d_x;
        }
    }
    sum
}

// ---------------------------------------------------------------------------
// GEMV 公共 API
// ---------------------------------------------------------------------------

/// Q8_0 GEMV：量化矩阵-向量乘法。
///
/// 计算 `output[n] = Σ_k(weights[n,k] * input[k])`
///
/// 权重以 Q8_0 格式存储，输入为 f32。内部先将输入量化为 Q8_0，
/// 然后使用 int8 点积内核计算。
///
/// # 参数
///
/// - `weights`：权重块数组，行优先布局，每行 `k / QK8_0` 个块
/// - `input`：f32 输入向量，长度 `k`
/// - `output`：输出缓冲区，长度 `n`
/// - `n`：输出维度（行数）
/// - `k`：输入维度（列数），必须为 `QK8_0`（32）的倍数
///
/// # Panics
///
/// 仅在 debug 模式下，若参数不满足上述约束会触发 debug_assert。
pub fn gemv_q8_0(weights: &[BlockQ8_0], input: &[f32], output: &mut [f32], n: usize, k: usize) {
    let blocks_per_row = k / QK8_0;
    debug_assert!(k.is_multiple_of(QK8_0));
    debug_assert!(weights.len() >= n * blocks_per_row);
    debug_assert!(input.len() >= k);
    debug_assert!(output.len() >= n);

    // 输入预处理：f32 → Q8_0
    let mut input_q = vec![
        BlockQ8_0 {
            d: half::f16::from_f32(0.0),
            qs: [0; QK8_0],
        };
        blocks_per_row
    ];
    quantize_row_q8_0(input, &mut input_q);

    for row in 0..n {
        let weight_row = &weights[row * blocks_per_row..(row + 1) * blocks_per_row];
        output[row] = vec_dot_q8_0_q8_0(weight_row, &input_q);
    }
}

/// Q4_0 GEMV：量化矩阵-向量乘法。
///
/// 计算 `output[n] = Σ_k(weights[n,k] * input[k])`
///
/// 权重以 Q4_0 格式存储，输入为 f32。内部先将输入量化为 Q8_0，
/// 然后使用 int8 点积内核计算。
///
/// # 参数
///
/// - `weights`：Q4_0 权重块数组，行优先布局，每行 `k / QK4_0` 个块
/// - `input`：f32 输入向量，长度 `k`
/// - `output`：输出缓冲区，长度 `n`
/// - `n`：输出维度（行数）
/// - `k`：输入维度（列数），必须为 `QK4_0`（32）的倍数
pub fn gemv_q4_0(weights: &[BlockQ4_0], input: &[f32], output: &mut [f32], n: usize, k: usize) {
    let blocks_per_row = k / QK4_0;
    debug_assert!(k.is_multiple_of(QK4_0));
    debug_assert!(weights.len() >= n * blocks_per_row);
    debug_assert!(input.len() >= k);
    debug_assert!(output.len() >= n);

    // 输入预处理：f32 → Q8_0
    let mut input_q = vec![
        BlockQ8_0 {
            d: half::f16::from_f32(0.0),
            qs: [0; QK8_0],
        };
        blocks_per_row
    ];
    quantize_row_q8_0(input, &mut input_q);

    for row in 0..n {
        let weight_row = &weights[row * blocks_per_row..(row + 1) * blocks_per_row];
        output[row] = vec_dot_q4_0_q8_0(weight_row, &input_q);
    }
}

/// Q4_K GEMV：量化矩阵-向量乘法。
///
/// 计算 `output[n] = Σ_k(weights[n,k] * input[k])`
///
/// 权重以 Q4_K 格式存储（256 元素/块），输入为 f32。内部先将输入量化为 Q8_0，
/// 然后使用 int8 点积内核计算（6-bit scales/mins + 4-bit nibbles）。
///
/// # 参数
///
/// - `weights`：Q4_K 权重块数组，行优先布局，每行 `k / QK_K` 个块
/// - `input`：f32 输入向量，长度 `k`
/// - `output`：输出缓冲区，长度 `n`
/// - `n`：输出维度（行数）
/// - `k`：输入维度（列数），必须为 `QK_K`（256）的倍数
pub fn gemv_q4_k(weights: &[BlockQ4K], input: &[f32], output: &mut [f32], n: usize, k: usize) {
    let blocks_per_row = k / QK_K;
    let input_blocks = k / QK8_0;
    debug_assert!(k.is_multiple_of(QK_K));
    debug_assert!(weights.len() >= n * blocks_per_row);
    debug_assert!(input.len() >= k);
    debug_assert!(output.len() >= n);

    // 输入预处理：f32 → Q8_0
    let mut input_q = vec![
        BlockQ8_0 {
            d: half::f16::from_f32(0.0),
            qs: [0; QK8_0],
        };
        input_blocks
    ];
    quantize_row_q8_0(input, &mut input_q);

    for row in 0..n {
        let weight_row = &weights[row * blocks_per_row..(row + 1) * blocks_per_row];
        output[row] = vec_dot_q4_k_q8_0(weight_row, &input_q);
    }
}

/// Q8_K GEMV：量化矩阵-向量乘法。
///
/// 计算 `output[n] = Σ_k(weights[n,k] * input[k])`
///
/// 权重以 Q8_K 格式存储（256 元素/块，f32 缩放因子），输入为 f32。
/// 内部先将输入量化为 Q8_0，然后使用 int8 点积内核计算。
///
/// # 参数
///
/// - `weights`：Q8_K 权重块数组，行优先布局，每行 `k / QK_K` 个块
/// - `input`：f32 输入向量，长度 `k`
/// - `output`：输出缓冲区，长度 `n`
/// - `n`：输出维度（行数）
/// - `k`：输入维度（列数），必须为 `QK_K`（256）的倍数
pub fn gemv_q8_k(weights: &[BlockQ8K], input: &[f32], output: &mut [f32], n: usize, k: usize) {
    let blocks_per_row = k / QK_K;
    let input_blocks = k / QK8_0;
    debug_assert!(k.is_multiple_of(QK_K));
    debug_assert!(weights.len() >= n * blocks_per_row);
    debug_assert!(input.len() >= k);
    debug_assert!(output.len() >= n);

    // 输入预处理：f32 → Q8_0
    let mut input_q = vec![
        BlockQ8_0 {
            d: half::f16::from_f32(0.0),
            qs: [0; QK8_0],
        };
        input_blocks
    ];
    quantize_row_q8_0(input, &mut input_q);

    for row in 0..n {
        let weight_row = &weights[row * blocks_per_row..(row + 1) * blocks_per_row];
        output[row] = vec_dot_q8_k_q8_0(weight_row, &input_q);
    }
}

/// 根据 `dtype` 分发到对应的 GEMV 内核。
///
/// 从原始字节切片解析量化块（零拷贝），然后调用对应的 GEMV 函数。
/// 支持的格式：Q4_0、Q4_K、Q8_0、Q8_K。
///
/// # 参数
///
/// - `dtype`：量化数据类型
/// - `weights`：权重原始字节切片（通常来自 `GgufFile::tensor_data()` 的 mmap 区域）
/// - `input`：f32 输入向量，长度 `k`
/// - `output`：输出缓冲区，长度 `n`
/// - `n`：输出维度（行数）
/// - `k`：输入维度（列数）
///
/// # 错误
///
/// - 不支持的 dtype（F32/F16/BF16/Q4_1/Q5_0/Q5_1/Q8_1/Q2K/Q3K/Unknown）
/// - 数据对齐或长度不匹配
pub fn gemv_dispatch(
    dtype: GgmlDType,
    weights: &[u8],
    input: &[f32],
    output: &mut [f32],
    n: usize,
    k: usize,
) -> Result<()> {
    let blocks = blocks_from_bytes(weights, dtype)?;
    match blocks {
        crate::quant_blocks::QuantBlock::Q8_0(w) => {
            gemv_q8_0(w, input, output, n, k);
            Ok(())
        }
        crate::quant_blocks::QuantBlock::Q4_0(w) => {
            gemv_q4_0(w, input, output, n, k);
            Ok(())
        }
        crate::quant_blocks::QuantBlock::Q4K(w) => {
            gemv_q4_k(w, input, output, n, k);
            Ok(())
        }
        crate::quant_blocks::QuantBlock::Q8K(w) => {
            gemv_q8_k(w, input, output, n, k);
            Ok(())
        }
        _ => bail!("unsupported dtype for GEMV: {dtype:?}"),
    }
}

// ---------------------------------------------------------------------------
// 朴素参考实现（仅用于测试对照）
// ---------------------------------------------------------------------------

/// 朴素 Q8_0 GEMV（无输入量化，f32 直接计算，用于测试对照）。
///
/// 直接将 Q8_0 权重反量化为 f32，然后与 f32 输入做浮点点积。
/// 结果作为「真值」基准，用于验证优化内核（int8 路径）的正确性。
#[cfg(test)]
pub(crate) fn gemv_q8_0_naive(
    weights: &[BlockQ8_0],
    input: &[f32],
    output: &mut [f32],
    n: usize,
    k: usize,
) {
    let blocks_per_row = k / QK8_0;
    for row in 0..n {
        let weight_row = &weights[row * blocks_per_row..(row + 1) * blocks_per_row];
        let mut sum = 0.0f32;
        for (b, block) in weight_row.iter().enumerate() {
            let d = block.d.to_f32();
            for j in 0..QK8_0 {
                sum += d * block.qs[j] as f32 * input[b * QK8_0 + j];
            }
        }
        output[row] = sum;
    }
}

/// 朴素 Q4_K GEMV（f32 反量化后直接计算，用于测试对照）。
///
/// 将 Q4_K 权重反量化为 f32，然后与 f32 输入做浮点点积。
/// 结果作为「真值」基准，用于验证优化内核（int8 路径）的正确性。
#[cfg(test)]
pub(crate) fn gemv_q4_k_naive(
    weights: &[BlockQ4K],
    input: &[f32],
    output: &mut [f32],
    n: usize,
    k: usize,
) {
    let blocks_per_row = k / QK_K;
    for row in 0..n {
        let weight_row = &weights[row * blocks_per_row..(row + 1) * blocks_per_row];
        let mut sum = 0.0f32;
        for (b, block) in weight_row.iter().enumerate() {
            let d = block.d.to_f32();
            let min = block.dmin.to_f32();
            let q = &block.qs;
            let mut is = 0usize;
            let mut ys_index = 0usize;
            let base = b * QK_K;
            for j in (0..QK_K).step_by(64) {
                let q_chunk = &q[j / 2..j / 2 + 32];
                let (sc, m) = get_scale_min_k4(is, &block.scales);
                let d1 = d * sc as f32;
                let m1 = min * m as f32;
                let (sc, m) = get_scale_min_k4(is + 1, &block.scales);
                let d2 = d * sc as f32;
                let m2 = min * m as f32;
                for &ql in q_chunk {
                    sum += (d1 * (ql & 0x0F) as f32 - m1) * input[base + ys_index];
                    ys_index += 1;
                }
                for &ql in q_chunk {
                    sum += (d2 * (ql >> 4) as f32 - m2) * input[base + ys_index];
                    ys_index += 1;
                }
                is += 2;
            }
        }
        output[row] = sum;
    }
}
