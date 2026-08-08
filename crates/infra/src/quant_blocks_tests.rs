//! 量化块结构与反量化测试（Phase 3 Session 17）。
//!
//! 14 个测试用例覆盖：
//! - 块结构体大小（TC-QUANT-001~003）
//! - 全零块反量化（TC-QUANT-004~005）
//! - 已知值块反量化（TC-QUANT-006~008）
//! - blocks_from_bytes 字节解析（TC-QUANT-009~011）
//! - block_count 计算函数（TC-QUANT-012~013）
//! - 反量化输出长度（TC-QUANT-014）

// 测试代码中 unwrap/panic/range_loop 是常见模式，允许使用
#![allow(clippy::unwrap_used, clippy::panic, clippy::needless_range_loop)]

use super::quant_blocks::*;
use crate::gguf_reader::GgmlDType;
use half::f16;

// ---------------------------------------------------------------------------
// TC-QUANT-001~003：块结构体大小验证
// ---------------------------------------------------------------------------

/// TC-QUANT-001：BlockQ4_0 大小为 18 字节。
#[test]
fn test_block_q4_0_size() {
    assert_eq!(std::mem::size_of::<BlockQ4_0>(), 18);
}

/// TC-QUANT-002：BlockQ8_0 大小为 34 字节。
#[test]
fn test_block_q8_0_size() {
    assert_eq!(std::mem::size_of::<BlockQ8_0>(), 34);
}

/// TC-QUANT-003：BlockQ4K 大小为 144 字节。
#[test]
fn test_block_q4k_size() {
    assert_eq!(std::mem::size_of::<BlockQ4K>(), 144);
}

// ---------------------------------------------------------------------------
// TC-QUANT-004~005：全零块反量化
// ---------------------------------------------------------------------------

/// TC-QUANT-004：全零 BlockQ4_0 反量化为全 0.0。
///
/// d=0.0 → y = 0 * (q - 8) = 0.0
#[test]
fn test_dequantize_q4_0_all_zeros() {
    let block = BlockQ4_0 {
        d: f16::from_f32(0.0),
        qs: [0; 16],
    };
    let mut out = [0.0f32; QK4_0];
    dequantize_q4_0(&[block], &mut out);
    for &v in &out {
        assert_eq!(v, 0.0, "expected all zeros when d=0");
    }
}

/// TC-QUANT-005：全零 BlockQ8_0 反量化为全 0.0。
///
/// d=0.0 → y = 0 * q = 0.0
#[test]
fn test_dequantize_q8_0_all_zeros() {
    let block = BlockQ8_0 {
        d: f16::from_f32(0.0),
        qs: [0; 32],
    };
    let mut out = [0.0f32; QK8_0];
    dequantize_q8_0(&[block], &mut out);
    for &v in &out {
        assert_eq!(v, 0.0, "expected all zeros when d=0");
    }
}

// ---------------------------------------------------------------------------
// TC-QUANT-006~008：已知值块反量化
// ---------------------------------------------------------------------------

/// TC-QUANT-006：Q4_0 已知值块反量化结果正确（手算对照）。
///
/// d=2.0，qs[0]=0x97 (低 nibble=7, 高 nibble=9)：
/// - y[0] = (7-8)*2 = -2.0
/// - y[16] = (9-8)*2 = 2.0
///
/// qs[1]=0xA6 (低 nibble=6, 高 nibble=10)：
/// - y[1] = (6-8)*2 = -4.0
/// - y[17] = (10-8)*2 = 4.0
#[test]
fn test_dequantize_q4_0_known_values() {
    let mut qs = [0u8; 16];
    qs[0] = 0x97; // low=7, high=9
    qs[1] = 0xA6; // low=6, high=10
    // 其余 nibble = 0 → y = (0-8)*2 = -16.0
    let block = BlockQ4_0 {
        d: f16::from_f32(2.0),
        qs,
    };
    let mut out = [0.0f32; QK4_0];
    dequantize_q4_0(&[block], &mut out);

    // 低 nibble → 元素 0..15
    assert_eq!(out[0], -2.0, "y[0] = (7-8)*2 = -2.0");
    assert_eq!(out[1], -4.0, "y[1] = (6-8)*2 = -4.0");
    assert_eq!(out[2], -16.0, "y[2] = (0-8)*2 = -16.0");

    // 高 nibble → 元素 16..31
    assert_eq!(out[16], 2.0, "y[16] = (9-8)*2 = 2.0");
    assert_eq!(out[17], 4.0, "y[17] = (10-8)*2 = 4.0");
    assert_eq!(out[18], -16.0, "y[18] = (0-8)*2 = -16.0");
}

/// TC-QUANT-007：Q8_0 已知值块反量化结果正确。
///
/// d=0.5，qs=[1, -1, 2, -2, ...]：
/// - y[0] = 1 * 0.5 = 0.5
/// - y[1] = -1 * 0.5 = -0.5
/// - y[2] = 2 * 0.5 = 1.0
/// - y[3] = -2 * 0.5 = -1.0
#[test]
fn test_dequantize_q8_0_known_values() {
    let qs: [i8; 32] = [
        1, -1, 2, -2, 3, -3, 4, -4, 5, -5, 6, -6, 7, -7, 8, -8, 9, -9, 10, -10, 11, -11, 12, -12,
        13, -13, 14, -14, 15, -15, 16, -16,
    ];
    let block = BlockQ8_0 {
        d: f16::from_f32(0.5),
        qs,
    };
    let mut out = [0.0f32; QK8_0];
    dequantize_q8_0(&[block], &mut out);

    for j in 0..QK8_0 {
        assert_eq!(out[j], qs[j] as f32 * 0.5, "y[{}] mismatch", j);
    }
}

/// TC-QUANT-008：Q4_K 已知值反量化正确。
///
/// d=1.0, dmin=0.5
/// scales[0..3]=1 (sub-blocks 0-3: sc=1, m=1)
/// scales[4..7]=1 (sub-blocks 0-3: min=1)
/// scales[8..11]=0 (sub-blocks 4-7: sc=0, m=0)
/// qs=all zeros → all nibbles = 0
///
/// sub-blocks 0-3 (elements 0-127): y = 1.0*1*0 - 0.5*1 = -0.5
/// sub-blocks 4-7 (elements 128-255): y = 0*0 - 0*0 = 0.0
#[test]
fn test_dequantize_q4k_known_values() {
    let mut scales = [0u8; K_SCALE_SIZE];
    // sub-blocks 0-3: sc=1, m=1
    scales[0] = 1;
    scales[1] = 1;
    scales[2] = 1;
    scales[3] = 1;
    scales[4] = 1;
    scales[5] = 1;
    scales[6] = 1;
    scales[7] = 1;
    // scales[8..11] = 0 → sub-blocks 4-7 have sc=0, m=0

    let block = BlockQ4K {
        d: f16::from_f32(1.0),
        dmin: f16::from_f32(0.5),
        scales,
        qs: [0; QK_K / 2],
    };
    let mut out = [0.0f32; QK_K];
    dequantize_q4_k(&[block], &mut out);

    // sub-blocks 0-3 (elements 0-127): y = 1.0 * 1 * 0 - 0.5 * 1 = -0.5
    for i in 0..128 {
        assert!(
            (out[i] - (-0.5)).abs() < 1e-6,
            "y[{}] expected -0.5, got {}",
            i,
            out[i]
        );
    }
    // sub-blocks 4-7 (elements 128-255): y = 0 * 0 - 0 * 0 = 0.0
    for i in 128..QK_K {
        assert!(out[i].abs() < 1e-6, "y[{}] expected 0.0, got {}", i, out[i]);
    }
}

// ---------------------------------------------------------------------------
// TC-QUANT-009~011：blocks_from_bytes 字节解析
// ---------------------------------------------------------------------------

/// TC-QUANT-009：字节切片正确解析为 Q4_0 块。
///
/// 2 个 BlockQ4_0 = 36 字节，解析后应得到 2 个块。
#[test]
fn test_blocks_from_bytes_q4_0() {
    // 2 blocks × 18 bytes = 36 bytes
    let data = vec![0u8; 2 * std::mem::size_of::<BlockQ4_0>()];
    let result = blocks_from_bytes(&data, GgmlDType::Q4_0);
    assert!(result.is_ok());
    let block = result.unwrap();
    match block {
        QuantBlock::Q4_0(blocks) => {
            assert_eq!(blocks.len(), 2);
        }
        _ => panic!("expected QuantBlock::Q4_0"),
    }
}

/// TC-QUANT-010：字节切片正确解析为 Q8_0 块。
///
/// 3 个 BlockQ8_0 = 102 字节，解析后应得到 3 个块。
#[test]
fn test_blocks_from_bytes_q8_0() {
    // 3 blocks × 34 bytes = 102 bytes
    let data = vec![0u8; 3 * std::mem::size_of::<BlockQ8_0>()];
    let result = blocks_from_bytes(&data, GgmlDType::Q8_0);
    assert!(result.is_ok());
    let block = result.unwrap();
    match block {
        QuantBlock::Q8_0(blocks) => {
            assert_eq!(blocks.len(), 3);
        }
        _ => panic!("expected QuantBlock::Q8_0"),
    }
}

/// TC-QUANT-011：未对齐字节返回 Err。
///
/// 17 字节不是 18（BlockQ4_0 大小）的整数倍，应返回错误。
#[test]
fn test_blocks_from_bytes_invalid_alignment() {
    // 17 bytes, not a multiple of 18 (BlockQ4_0 size)
    let data = vec![0u8; 17];
    let result = blocks_from_bytes(&data, GgmlDType::Q4_0);
    assert!(result.is_err(), "expected error for unaligned data length");
}

// ---------------------------------------------------------------------------
// TC-QUANT-012~013：block_count 计算函数
// ---------------------------------------------------------------------------

/// TC-QUANT-012：256 元素 → 8 个 Q4_0 块。
#[test]
fn test_block_count_q4_0() {
    assert_eq!(block_count(GgmlDType::Q4_0, 256), 8);
}

/// TC-QUANT-013：256 元素 → 1 个 Q4_K 块。
#[test]
fn test_block_count_q4k() {
    assert_eq!(block_count(GgmlDType::Q4K, 256), 1);
}

// ---------------------------------------------------------------------------
// TC-QUANT-014：反量化输出长度
// ---------------------------------------------------------------------------

/// TC-QUANT-014：反量化输出长度 == elem_count。
///
/// 创建 2 个 Q4_0 块（64 元素），反量化后输出应为 64 个 f32。
#[test]
fn test_dequantize_output_length() {
    let blocks = vec![
        BlockQ4_0 {
            d: f16::from_f32(1.0),
            qs: [0; 16],
        };
        2
    ];
    let mut out = vec![0.0f32; 2 * QK4_0];
    dequantize_q4_0(&blocks, &mut out);
    assert_eq!(out.len(), blocks.len() * QK4_0);

    // 也测试 QuantBlock 的 elem_count 方法
    let qb = QuantBlock::Q4_0(&blocks);
    assert_eq!(qb.elem_count(), 2 * QK4_0);
}
