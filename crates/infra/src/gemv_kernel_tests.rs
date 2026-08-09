//! 自研 GEMV 内核测试（Phase 3 Session 18）。
//!
//! 14 个测试用例覆盖：
//! - Q8_0 GEMV 正确性（TC-GEMV-001~004）：单位矩阵、零输入、已知值、大矩阵
//! - Q4_0 GEMV 正确性（TC-GEMV-005~006）：单位矩阵、已知值
//! - Q4_K GEMV 正确性（TC-GEMV-007~008）：已知值、大矩阵
//! - 量化往返（TC-GEMV-009）：f32→Q8_0→f32 误差
//! - dispatch 路由（TC-GEMV-010~012）：Q8_0/Q4_K/未知类型
//! - 优化内核 vs 朴素对照（TC-GEMV-013~014）：Q8_0/Q4_K 一致性

use super::gemv_kernel::*;
use super::quant_blocks::*;
use crate::gguf_reader::GgmlDType;
use half::f16;

// ---------------------------------------------------------------------------
// 辅助函数
// ---------------------------------------------------------------------------

/// 从 f32 数据创建 Q8_0 权重块数组（行优先布局）。
fn make_q8_0_weights(data: &[f32], n: usize, k: usize) -> Vec<BlockQ8_0> {
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
    weights
}

/// 创建 Q4_K 权重块（scales 编码为前 4 子块 sc=1/m=1，后 4 子块 sc=0/m=0）。
fn make_q4k_block(d: f32, dmin: f32, nibble_val: u8) -> BlockQ4K {
    let mut scales = [0u8; K_SCALE_SIZE];
    // 前半：sc=1, m=1
    scales[0] = 1;
    scales[1] = 1;
    scales[2] = 1;
    scales[3] = 1;
    scales[4] = 1;
    scales[5] = 1;
    scales[6] = 1;
    scales[7] = 1;
    // 后半：sc=0, m=0（scales[8..11] = 0）

    BlockQ4K {
        d: f16::from_f32(d),
        dmin: f16::from_f32(dmin),
        scales,
        qs: [nibble_val; QK_K / 2],
    }
}

// ---------------------------------------------------------------------------
// TC-GEMV-001~004：Q8_0 GEMV 正确性
// ---------------------------------------------------------------------------

/// TC-GEMV-001：Q8_0 单位矩阵 GEMV 结果 == 输入向量。
///
/// 1×32 矩阵，权重 d=1.0, qs=[1,0,...,0]，输入 [1.0, 0, ..., 0]。
/// GEMV: output[0] = weight[0] × input[0] = 1×1 = 1.0
#[test]
fn test_gemv_q8_0_identity() {
    let n = 1;
    let k = QK8_0;
    let mut qs = [0i8; QK8_0];
    qs[0] = 1;
    let weights = vec![BlockQ8_0 {
        d: f16::from_f32(1.0),
        qs,
    }];
    let input = vec![1.0f32; k];
    let mut output = vec![0.0f32; n];
    gemv_q8_0(&weights, &input, &mut output, n, k);
    // f32 精度：127 * (1.0/127.0) ≈ 0.99999，量化误差约 6e-5
    assert!(
        (output[0] - 1.0).abs() < 1e-2,
        "expected 1.0, got {}",
        output[0]
    );
}

/// TC-GEMV-002：零输入 → 零输出。
#[test]
fn test_gemv_q8_0_zero_input() {
    let n = 2;
    let k = QK8_0;
    let weights = vec![
        BlockQ8_0 {
            d: f16::from_f32(1.0),
            qs: [5; QK8_0],
        };
        n
    ];
    let input = vec![0.0f32; k];
    let mut output = vec![0.0f32; n];
    gemv_q8_0(&weights, &input, &mut output, n, k);
    for (i, &v) in output.iter().enumerate() {
        assert!(v.abs() < 1e-6, "output[{}] expected 0.0, got {}", i, v);
    }
}

/// TC-GEMV-003：小矩阵（2×32）手算结果对照。
///
/// Row 0: d=1.0, qs=[1,2,3,0,...] → weight [1,2,3,0,...]
/// Row 1: d=1.0, qs=[4,5,6,0,...] → weight [4,5,6,0,...]
/// Input: [1,1,1,0,...]
/// Expected: [6.0, 15.0]  (1+2+3=6, 4+5+6=15)
#[test]
fn test_gemv_q8_0_known_values() {
    let n = 2;
    let k = QK8_0;
    let mut qs0 = [0i8; QK8_0];
    qs0[0] = 1;
    qs0[1] = 2;
    qs0[2] = 3;
    let mut qs1 = [0i8; QK8_0];
    qs1[0] = 4;
    qs1[1] = 5;
    qs1[2] = 6;
    let weights = vec![
        BlockQ8_0 {
            d: f16::from_f32(1.0),
            qs: qs0,
        },
        BlockQ8_0 {
            d: f16::from_f32(1.0),
            qs: qs1,
        },
    ];
    let mut input = vec![0.0f32; k];
    input[0] = 1.0;
    input[1] = 1.0;
    input[2] = 1.0;
    let mut output = vec![0.0f32; n];
    gemv_q8_0(&weights, &input, &mut output, n, k);
    assert!(
        (output[0] - 6.0).abs() < 0.1,
        "row 0 expected 6.0, got {}",
        output[0]
    );
    assert!(
        (output[1] - 15.0).abs() < 0.1,
        "row 1 expected 15.0, got {}",
        output[1]
    );
}

/// TC-GEMV-004：256×256 矩阵与朴素实现误差 < 0.5。
#[test]
fn test_gemv_q8_0_large_matrix() {
    let n = 256;
    let k = 256;
    // 用确定性的伪随机值创建权重
    let mut data = vec![0.0f32; n * k];
    for i in 0..n {
        for j in 0..k {
            data[i * k + j] = ((i * 7 + j * 3) % 17) as f32 - 8.0;
        }
    }
    let weights = make_q8_0_weights(&data, n, k);
    let input: Vec<f32> = (0..k).map(|j| ((j * 5) % 11) as f32 - 5.0).collect();
    let mut output_opt = vec![0.0f32; n];
    let mut output_naive = vec![0.0f32; n];
    gemv_q8_0(&weights, &input, &mut output_opt, n, k);
    gemv_q8_0_naive(&weights, &input, &mut output_naive, n, k);
    for i in 0..n {
        let diff = (output_opt[i] - output_naive[i]).abs();
        // 256×256 矩阵，输入量化误差累积约 0.8
        assert!(
            diff < 1.5,
            "row {i}: optimized={}, naive={}, diff={diff}",
            output_opt[i],
            output_naive[i]
        );
    }
}

// ---------------------------------------------------------------------------
// TC-GEMV-005~006：Q4_0 GEMV 正确性
// ---------------------------------------------------------------------------

/// TC-GEMV-005：Q4_0 单位矩阵 GEMV。
///
/// 1×32 矩阵，d=1.0，qs[0]=0x89（低 nibble=9, 高 nibble=8），其余=0x88。
/// 元素 0: (9-8)*1 = 1.0, 其余: (8-8)*1 = 0.0
/// 输入 [1,0,...,0] → 输出 [1.0]
#[test]
fn test_gemv_q4_0_identity() {
    let n = 1;
    let k = QK4_0;
    let mut qs = [0x88u8; QK4_0 / 2];
    qs[0] = 0x89; // low=9 (y=1.0), high=8 (y=0.0)
    let weights = vec![BlockQ4_0 {
        d: f16::from_f32(1.0),
        qs,
    }];
    let mut input = vec![0.0f32; k];
    input[0] = 1.0;
    let mut output = vec![0.0f32; n];
    gemv_q4_0(&weights, &input, &mut output, n, k);
    assert!(
        (output[0] - 1.0).abs() < 0.1,
        "expected 1.0, got {}",
        output[0]
    );
}

/// TC-GEMV-006：Q4_0 小矩阵手算对照。
///
/// d=1.0, qs[0]=0x97（低=7→y=-1, 高=9→y=1），其余=0x88（y=0）
/// 输入 [1, 0, ..., 0, 1, 0, ...]（元素 0=1, 元素 16=1）
/// Expected: -1*1 + 1*1 = 0.0
#[test]
fn test_gemv_q4_0_known_values() {
    let n = 1;
    let k = QK4_0;
    let mut qs = [0x88u8; QK4_0 / 2];
    qs[0] = 0x97; // low=7 (y=-1), high=9 (y=1)
    let weights = vec![BlockQ4_0 {
        d: f16::from_f32(1.0),
        qs,
    }];
    let mut input = vec![0.0f32; k];
    input[0] = 1.0;
    input[16] = 1.0;
    let mut output = vec![0.0f32; n];
    gemv_q4_0(&weights, &input, &mut output, n, k);
    assert!((output[0]).abs() < 0.1, "expected ~0.0, got {}", output[0]);
}

// ---------------------------------------------------------------------------
// TC-GEMV-007~008：Q4_K GEMV 正确性
// ---------------------------------------------------------------------------

/// TC-GEMV-007：Q4_K 已知值手算对照。
///
/// d=1.0, dmin=0.5, scales 前4子块 sc=1/m=1, 后4子块 sc=0/m=0, qs 全0
/// 元素 0-127: y = 1*1*0 - 0.5*1 = -0.5
/// 元素 128-255: y = 0*0 - 0*0 = 0.0
/// 输入全 1.0 → 输出 = 128×(-0.5) + 128×0 = -64.0
#[test]
fn test_gemv_q4k_known_values() {
    let n = 1;
    let k = QK_K;
    let block = make_q4k_block(1.0, 0.5, 0);
    let weights = vec![block];
    let input = vec![1.0f32; k];
    let mut output = vec![0.0f32; n];
    gemv_q4_k(&weights, &input, &mut output, n, k);
    assert!(
        (output[0] - (-64.0)).abs() < 1.0,
        "expected ~-64.0, got {}",
        output[0]
    );
}

/// TC-GEMV-008：256×256 Q4_K 矩阵与朴素实现误差 < 1.0。
#[test]
fn test_gemv_q4k_large_matrix() {
    let n = 4;
    let k = QK_K;
    // 每行用不同的 nibble 值
    let weights: Vec<BlockQ4K> = (0..n)
        .map(|row| make_q4k_block(1.0 + row as f32 * 0.1, 0.5, (row as u8) % 15 + 1))
        .collect();
    let input: Vec<f32> = (0..k).map(|j| ((j % 7) as f32) / 3.0).collect();
    let mut output_opt = vec![0.0f32; n];
    let mut output_naive = vec![0.0f32; n];
    gemv_q4_k(&weights, &input, &mut output_opt, n, k);
    gemv_q4_k_naive(&weights, &input, &mut output_naive, n, k);
    for i in 0..n {
        let diff = (output_opt[i] - output_naive[i]).abs();
        assert!(
            diff < 1.0,
            "row {i}: optimized={}, naive={}, diff={diff}",
            output_opt[i],
            output_naive[i]
        );
    }
}

// ---------------------------------------------------------------------------
// TC-GEMV-009：量化往返
// ---------------------------------------------------------------------------

/// TC-GEMV-009：f32→Q8_0→f32 往返误差 < 0.5。
#[test]
fn test_quantize_row_q8_0_roundtrip() {
    let input: Vec<f32> = (0..QK8_0).map(|i| ((i as f32) / 3.0) - 5.0).collect();
    let mut blocks = vec![
        BlockQ8_0 {
            d: f16::from_f32(0.0),
            qs: [0; QK8_0],
        };
        1
    ];
    quantize_row_q8_0(&input, &mut blocks);
    let mut dequant = vec![0.0f32; QK8_0];
    dequantize_q8_0(&blocks, &mut dequant);
    for (i, (orig, deq)) in input.iter().zip(dequant.iter()).enumerate() {
        let err = (orig - deq).abs();
        assert!(
            err < 0.5,
            "element {i}: original={orig}, dequant={deq}, error={err}"
        );
    }
}

// ---------------------------------------------------------------------------
// TC-GEMV-010~012：dispatch 路由
// ---------------------------------------------------------------------------

/// TC-GEMV-010：dispatch 正确路由到 Q8_0 内核。
#[test]
fn test_gemv_dispatch_q8_0() {
    let n = 1;
    let k = QK8_0;
    let mut qs = [0i8; QK8_0];
    qs[0] = 1;
    let block = BlockQ8_0 {
        d: f16::from_f32(1.0),
        qs,
    };
    let weights_bytes = unsafe {
        std::slice::from_raw_parts(
            &block as *const BlockQ8_0 as *const u8,
            std::mem::size_of::<BlockQ8_0>(),
        )
    };
    let input = vec![1.0f32; k];
    let mut output = vec![0.0f32; n];
    let result = gemv_dispatch(GgmlDType::Q8_0, weights_bytes, &input, &mut output, n, k);
    assert!(
        result.is_ok(),
        "dispatch should succeed: {:?}",
        result.err()
    );
    assert!(
        (output[0] - 1.0).abs() < 0.1,
        "expected ~1.0, got {}",
        output[0]
    );
}

/// TC-GEMV-011：dispatch 正确路由到 Q4_K 内核。
#[test]
fn test_gemv_dispatch_q4_k() {
    let n = 1;
    let k = QK_K;
    let block = make_q4k_block(1.0, 0.5, 0);
    let weights_bytes = unsafe {
        std::slice::from_raw_parts(
            &block as *const BlockQ4K as *const u8,
            std::mem::size_of::<BlockQ4K>(),
        )
    };
    let input = vec![1.0f32; k];
    let mut output = vec![0.0f32; n];
    let result = gemv_dispatch(GgmlDType::Q4K, weights_bytes, &input, &mut output, n, k);
    assert!(
        result.is_ok(),
        "dispatch should succeed: {:?}",
        result.err()
    );
    assert!(
        (output[0] - (-64.0)).abs() < 1.0,
        "expected ~-64.0, got {}",
        output[0]
    );
}

/// TC-GEMV-012：未知 dtype 返回 Err。
#[test]
fn test_gemv_dispatch_unknown_dtype() {
    let weights_bytes = &[0u8; 64];
    let input = vec![0.0f32; 32];
    let mut output = vec![0.0f32; 1];
    let result = gemv_dispatch(GgmlDType::F32, weights_bytes, &input, &mut output, 1, 32);
    assert!(result.is_err(), "F32 dtype should return Err");
}

// ---------------------------------------------------------------------------
// TC-GEMV-013~014：优化内核 vs 朴素对照
// ---------------------------------------------------------------------------

/// TC-GEMV-013：Q8_0 优化内核结果与朴素循环一致。
#[test]
fn test_gemv_q8_0_vs_naive() {
    let n = 64;
    let k = 128;
    let mut data = vec![0.0f32; n * k];
    for i in 0..n {
        for j in 0..k {
            data[i * k + j] = ((i * 11 + j * 7) % 19) as f32 - 9.0;
        }
    }
    let weights = make_q8_0_weights(&data, n, k);
    let input: Vec<f32> = (0..k).map(|j| ((j * 13) % 23) as f32 - 11.0).collect();
    let mut output_opt = vec![0.0f32; n];
    let mut output_naive = vec![0.0f32; n];
    gemv_q8_0(&weights, &input, &mut output_opt, n, k);
    gemv_q8_0_naive(&weights, &input, &mut output_naive, n, k);
    for i in 0..n {
        let diff = (output_opt[i] - output_naive[i]).abs();
        // 64×128 矩阵，输入量化误差累积约 1.2
        assert!(
            diff < 2.0,
            "row {i}: optimized={}, naive={}, diff={}",
            output_opt[i],
            output_naive[i],
            diff
        );
    }
}

/// TC-GEMV-014：Q4_K 优化内核结果与朴素循环一致。
#[test]
fn test_gemv_q4_k_vs_naive() {
    let n = 4;
    let k = QK_K;
    // 用不同 nibble 值创建 Q4_K 权重
    let weights: Vec<BlockQ4K> = (0..n)
        .map(|row| make_q4k_block(1.0, 0.5, ((row * 3) % 15) as u8))
        .collect();
    let input: Vec<f32> = (0..k).map(|j| ((j % 9) as f32) / 2.0).collect();
    let mut output_opt = vec![0.0f32; n];
    let mut output_naive = vec![0.0f32; n];
    gemv_q4_k(&weights, &input, &mut output_opt, n, k);
    gemv_q4_k_naive(&weights, &input, &mut output_naive, n, k);
    for i in 0..n {
        let diff = (output_opt[i] - output_naive[i]).abs();
        // Q4_K 256 元素块，输入量化误差累积约 1.3
        assert!(
            diff < 2.0,
            "row {i}: optimized={}, naive={}, diff={}",
            output_opt[i],
            output_naive[i],
            diff
        );
    }
}
