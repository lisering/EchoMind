//! CPU cache 友好权重重排测试（Phase 3 Session 19）。
//!
//! 6 个测试用例覆盖：
//! - TC-REPACK-001：Q8_0 重排后数据可正确反量化为原始值
//! - TC-REPACK-002：Q4_K 重排后数据完整
//! - TC-REPACK-003：重排后布局为 GemvOptimized
//! - TC-REPACK-004：重排后 GEMV 结果与未重排一致
//! - TC-REPACK-005：重排前后字节大小不变
//! - TC-REPACK-006：未对齐数据返回 Err

#![allow(clippy::unwrap_used, clippy::needless_range_loop)]

use super::gemv_kernel::*;
use super::quant_blocks::*;
use super::weight_repack::*;
use crate::gguf_reader::GgmlDType;
use half::f16;

// ---------------------------------------------------------------------------
// 辅助函数
// ---------------------------------------------------------------------------

/// 从 f32 数据创建 Q8_0 权重字节（行优先布局）。
fn make_q8_0_bytes(data: &[f32], n: usize, k: usize) -> Vec<u8> {
    let blocks_per_row = k / QK8_0;
    let mut weights = Vec::with_capacity(n * blocks_per_row);
    for row in 0..n {
        for b in 0..blocks_per_row {
            let start = row * k + b * QK8_0;
            let chunk = &data[start..start + QK8_0];
            let mut amax = 0.0f32;
            for &x in chunk {
                let abs_x = x.abs();
                if abs_x > amax {
                    amax = abs_x;
                }
            }
            let d = if amax > 0.0 { amax / 127.0 } else { 0.0 };
            let id = if d > 0.0 { 1.0 / d } else { 0.0 };
            let mut qs = [0i8; QK8_0];
            for (i, &x) in chunk.iter().enumerate() {
                qs[i] = (x * id).round().clamp(-128.0, 127.0) as i8;
            }
            weights.push(BlockQ8_0 {
                d: f16::from_f32(d),
                qs,
            });
        }
    }
    // 序列化为字节
    let mut bytes = Vec::with_capacity(std::mem::size_of_val(&weights));
    for block in &weights {
        unsafe {
            let slice = std::slice::from_raw_parts(
                block as *const BlockQ8_0 as *const u8,
                std::mem::size_of::<BlockQ8_0>(),
            );
            bytes.extend_from_slice(slice);
        }
    }
    bytes
}

/// 创建 Q4_K 权重块（scales 前 4 子块 sc=1/m=1，后 4 子块 sc=0/m=0）。
fn make_q4k_block(d: f32, dmin: f32, nibble_val: u8) -> BlockQ4K {
    let mut scales = [0u8; K_SCALE_SIZE];
    scales[0] = 1;
    scales[1] = 1;
    scales[2] = 1;
    scales[3] = 1;
    scales[4] = 1;
    scales[5] = 1;
    scales[6] = 1;
    scales[7] = 1;
    BlockQ4K {
        d: f16::from_f32(d),
        dmin: f16::from_f32(dmin),
        scales,
        qs: [nibble_val; QK_K / 2],
    }
}

/// 将 Q4_K 权重块数组序列化为字节（行优先布局）。
fn q4k_blocks_to_bytes(blocks: &[BlockQ4K]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(std::mem::size_of_val(blocks));
    for block in blocks {
        unsafe {
            let slice = std::slice::from_raw_parts(
                block as *const BlockQ4K as *const u8,
                std::mem::size_of::<BlockQ4K>(),
            );
            bytes.extend_from_slice(slice);
        }
    }
    bytes
}

// ---------------------------------------------------------------------------
// TC-REPACK-001：Q8_0 重排后数据可正确反量化为原始值
// ---------------------------------------------------------------------------

/// TC-REPACK-001：Q8_0 重排后数据保留原始值。
///
/// 创建 4×32 的 Q8_0 权重矩阵，重排后还原为行优先，
/// 反量化并与原始 f32 值比较，误差应 < 0.5（量化误差）。
#[test]
fn test_repack_q8_0_preserves_data() {
    let n = 4;
    let k = QK8_0;
    let data: Vec<f32> = (0..n * k).map(|i| ((i * 7) % 17) as f32 - 8.0).collect();

    let original_bytes = make_q8_0_bytes(&data, n, k);
    let repacked = repack_for_gemv(&original_bytes, GgmlDType::Q8_0, n, k).unwrap();

    // 还原为行优先
    let restored_bytes = repacked.to_row_major();

    // 反量化原始数据和还原数据
    let original_blocks: &[BlockQ8_0] = unsafe {
        std::slice::from_raw_parts(
            original_bytes.as_ptr() as *const BlockQ8_0,
            original_bytes.len() / std::mem::size_of::<BlockQ8_0>(),
        )
    };
    let restored_blocks: &[BlockQ8_0] = unsafe {
        std::slice::from_raw_parts(
            restored_bytes.as_ptr() as *const BlockQ8_0,
            restored_bytes.len() / std::mem::size_of::<BlockQ8_0>(),
        )
    };

    let mut orig_deq = vec![0.0f32; n * k];
    let mut rest_deq = vec![0.0f32; n * k];
    dequantize_q8_0(original_blocks, &mut orig_deq);
    dequantize_q8_0(restored_blocks, &mut rest_deq);

    for i in 0..n * k {
        let diff = (orig_deq[i] - rest_deq[i]).abs();
        assert!(
            diff < 1e-6,
            "element {i}: original={}, restored={}, diff={diff}",
            orig_deq[i],
            rest_deq[i]
        );
    }
}

// ---------------------------------------------------------------------------
// TC-REPACK-002：Q4_K 重排后数据完整
// ---------------------------------------------------------------------------

/// TC-REPACK-002：Q4_K 重排后数据保留原始值。
///
/// 创建 4×256 的 Q4_K 权重矩阵，重排后还原为行优先，
/// 反量化并与原始反量化值比较，误差应 < 1e-6（无损重排）。
#[test]
fn test_repack_q4_k_preserves_data() {
    let n = 4;
    let k = QK_K;
    let blocks: Vec<BlockQ4K> = (0..n)
        .map(|row| make_q4k_block(1.0 + row as f32 * 0.1, 0.5, (row as u8) % 15 + 1))
        .collect();
    let original_bytes = q4k_blocks_to_bytes(&blocks);

    let repacked = repack_for_gemv(&original_bytes, GgmlDType::Q4K, n, k).unwrap();
    let restored_bytes = repacked.to_row_major();

    // 反量化原始和还原数据
    let original_blocks: &[BlockQ4K] = unsafe {
        std::slice::from_raw_parts(
            original_bytes.as_ptr() as *const BlockQ4K,
            original_bytes.len() / std::mem::size_of::<BlockQ4K>(),
        )
    };
    let restored_blocks: &[BlockQ4K] = unsafe {
        std::slice::from_raw_parts(
            restored_bytes.as_ptr() as *const BlockQ4K,
            restored_bytes.len() / std::mem::size_of::<BlockQ4K>(),
        )
    };

    let mut orig_deq = vec![0.0f32; n * k];
    let mut rest_deq = vec![0.0f32; n * k];
    dequantize_q4_k(original_blocks, &mut orig_deq);
    dequantize_q4_k(restored_blocks, &mut rest_deq);

    for i in 0..n * k {
        let diff = (orig_deq[i] - rest_deq[i]).abs();
        assert!(
            diff < 1e-6,
            "element {i}: original={}, restored={}, diff={diff}",
            orig_deq[i],
            rest_deq[i]
        );
    }
}

// ---------------------------------------------------------------------------
// TC-REPACK-003：重排后布局为 GemvOptimized
// ---------------------------------------------------------------------------

/// TC-REPACK-003：重排后布局为 GemvOptimized。
#[test]
fn test_repack_layout_is_gemv_optimized() {
    let n = 2;
    let k = QK8_0;
    let data = vec![1.0f32; n * k];
    let bytes = make_q8_0_bytes(&data, n, k);

    let repacked = repack_for_gemv(&bytes, GgmlDType::Q8_0, n, k).unwrap();

    assert_eq!(
        repacked.layout(),
        WeightLayout::GemvOptimized,
        "layout should be GemvOptimized after repack"
    );
}

// ---------------------------------------------------------------------------
// TC-REPACK-004：重排后 GEMV 结果与未重排一致
// ---------------------------------------------------------------------------

/// TC-REPACK-004：重排后 GEMV 结果与未重排一致。
///
/// 创建 64×128 的 Q8_0 权重矩阵，分别用原始布局和重排布局执行 GEMV，
/// 比较输出结果，误差应 < 2.0（输入量化误差累积）。
#[test]
fn test_repack_gemv_result_matches_unrepacked() {
    let n = 64;
    let k = 128;
    let mut data = vec![0.0f32; n * k];
    for i in 0..n {
        for j in 0..k {
            data[i * k + j] = ((i * 11 + j * 7) % 19) as f32 - 9.0;
        }
    }
    let bytes = make_q8_0_bytes(&data, n, k);
    let input: Vec<f32> = (0..k).map(|j| ((j * 13) % 23) as f32 - 11.0).collect();

    // 未重排 GEMV
    let mut output_orig = vec![0.0f32; n];
    gemv_dispatch(GgmlDType::Q8_0, &bytes, &input, &mut output_orig, n, k).unwrap();

    // 重排后 GEMV
    let repacked = repack_for_gemv(&bytes, GgmlDType::Q8_0, n, k).unwrap();
    let mut output_repacked = vec![0.0f32; n];
    gemv_repacked_dispatch(&repacked, &input, &mut output_repacked).unwrap();

    for i in 0..n {
        let diff = (output_orig[i] - output_repacked[i]).abs();
        assert!(
            diff < 2.0,
            "row {i}: original={}, repacked={}, diff={diff}",
            output_orig[i],
            output_repacked[i]
        );
    }
}

// ---------------------------------------------------------------------------
// TC-REPACK-005：重排前后字节大小不变
// ---------------------------------------------------------------------------

/// TC-REPACK-005：重排前后字节大小不变。
#[test]
fn test_repack_data_size_unchanged() {
    let n = 8;
    let k = QK8_0 * 4; // 4 blocks per row
    let data = vec![1.0f32; n * k];
    let bytes = make_q8_0_bytes(&data, n, k);
    let original_len = bytes.len();

    let repacked = repack_for_gemv(&bytes, GgmlDType::Q8_0, n, k).unwrap();

    assert_eq!(
        repacked.data().len(),
        original_len,
        "repacked data size should equal original"
    );
}

// ---------------------------------------------------------------------------
// TC-REPACK-006：未对齐数据返回 Err
// ---------------------------------------------------------------------------

/// TC-REPACK-006：数据长度不是块字节数整数倍时返回 Err。
#[test]
fn test_repack_invalid_alignment_fails() {
    // Q8_0 块大小 34 字节，提供 33 字节（不是 34 的倍数）
    let bad_data = vec![0u8; 33];
    let result = repack_for_gemv(&bad_data, GgmlDType::Q8_0, 1, QK8_0);
    assert!(result.is_err(), "should fail for misaligned data");
}
