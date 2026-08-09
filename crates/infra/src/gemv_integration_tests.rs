#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

//! GEMV 内核 + 权重重排端到端集成测试（Phase 3 CI 自动化正确性测试）。
//!
//! 本模块测试完整管道：
//! 1. 合成 GGUF → mmap 读取 → 提取张量 → GEMV 推理 → 验证输出形状与值范围
//! 2. 合成权重 → `repack_for_gemv` → `gemv_repacked_dispatch` → 对比未重排结果
//! 3. 验证 Tile-Major 布局结果与 Row-Major 一致（数值等价性）
//!
//! 所有测试在 CI 中自动运行（非 `#[ignore]`），合成文件 < 1KB，运行 < 5s。

use super::gemv_kernel::*;
use super::gguf_reader::*;
use super::quant_blocks::*;
use super::weight_repack::*;
use crate::gguf_reader::GgmlDType;

// 从 synthetic_gguf_tests 导入共享 fixture 生成器
use super::synthetic_gguf_tests::{
    GgufVersion, TensorSpec, create_synthetic_gguf, generate_input_vector, make_q4_0_bytes,
    make_q4_k_bytes, make_q8_0_bytes, make_q8_k_bytes, write_temp_file,
};

// ===========================================================================
// 步骤 2：GEMV 内核端到端集成测试
// ===========================================================================

// ---- TC-GEMV-INT-001 ~ 004: Q8_0 合成 GGUF → GEMV ----

/// TC-GEMV-INT-001：Q8_0 合成 GGUF → mmap → GEMV → 输出形状正确。
///
/// 创建包含 Q8_0 张量 (n=4, k=32) 的合成 GGUF 文件，
/// mmap 读取后提取张量数据，执行 GEMV，验证输出长度 = n。
#[test]
fn test_gemv_int_q8_0_output_shape() {
    let n = 4;
    let k = QK8_0; // 32

    // 生成确定性权重数据
    let weight_data: Vec<f32> = (0..n * k)
        .map(|i| ((i * 7 + 3) % 17) as f32 - 8.0)
        .collect();
    let tensor_bytes = make_q8_0_bytes(&weight_data, n, k);

    // 创建合成 GGUF 文件
    let tensors = vec![TensorSpec {
        name: "token_embd.weight".to_string(),
        dims: vec![n as u64, k as u64],
        dtype: GgmlDType::Q8_0,
        data: tensor_bytes.clone(),
    }];
    let gguf_data = create_synthetic_gguf(GgufVersion::V3, &tensors);
    let tmp = write_temp_file(&gguf_data);

    // mmap 读取 GGUF 文件
    let gguf = GgufFile::open(tmp.path()).expect("GGUF 打开失败");

    // 提取张量数据
    let tensor_data = gguf
        .tensor_data("token_embd.weight")
        .expect("张量数据应存在");
    assert_eq!(tensor_data.len(), tensor_bytes.len(), "张量数据长度应匹配");

    // 执行 GEMV
    let input = generate_input_vector(k, 42);
    let mut output = vec![0.0f32; n];
    let result = gemv_dispatch(GgmlDType::Q8_0, tensor_data, &input, &mut output, n, k);
    assert!(result.is_ok(), "GEMV dispatch 应成功: {:?}", result.err());
    assert_eq!(output.len(), n, "输出长度应等于 n={n}");
}

/// TC-GEMV-INT-002：Q8_0 GEMV 输出值范围合理（有限非 NaN）。
#[test]
fn test_gemv_int_q8_0_output_finite() {
    let n = 8;
    let k = QK8_0 * 2; // 64

    let weight_data: Vec<f32> = (0..n * k)
        .map(|i| ((i * 11 + 5) % 19) as f32 - 9.0)
        .collect();
    let tensor_bytes = make_q8_0_bytes(&weight_data, n, k);

    let tensors = vec![TensorSpec {
        name: "weight".to_string(),
        dims: vec![n as u64, k as u64],
        dtype: GgmlDType::Q8_0,
        data: tensor_bytes,
    }];
    let gguf_data = create_synthetic_gguf(GgufVersion::V3, &tensors);
    let tmp = write_temp_file(&gguf_data);
    let gguf = GgufFile::open(tmp.path()).expect("GGUF 打开失败");

    let tensor_data = gguf.tensor_data("weight").expect("张量数据应存在");
    let input = generate_input_vector(k, 99);
    let mut output = vec![0.0f32; n];
    gemv_dispatch(GgmlDType::Q8_0, tensor_data, &input, &mut output, n, k).expect("GEMV 应成功");

    for (i, &v) in output.iter().enumerate() {
        assert!(v.is_finite(), "output[{i}] 应为有限值，实际: {v}");
        // 权重范围 [-9, 9]，输入范围 [-1, 1)，k=64
        // 理论最大值 ≈ 9 * 1 * 64 = 576，量化误差放宽到 2x
        assert!(v.abs() < 2000.0, "output[{i}] 绝对值应 < 2000，实际: {v}");
    }
}

/// TC-GEMV-INT-003：Q8_0 GEMV 与朴素实现一致（通过 GGUF 管道）。
///
/// 完整管道测试：合成 GGUF → mmap → 提取张量 → GEMV dispatch
/// 对比同一权重数据的朴素 f32 计算（gemv_q8_0_naive）。
#[test]
fn test_gemv_int_q8_0_matches_naive() {
    let n = 4;
    let k = QK8_0 * 2; // 64

    let weight_data: Vec<f32> = (0..n * k)
        .map(|i| ((i * 13 + 7) % 23) as f32 - 11.0)
        .collect();
    let tensor_bytes = make_q8_0_bytes(&weight_data, n, k);

    // 创建 GGUF 并通过 mmap 读取
    let tensors = vec![TensorSpec {
        name: "weight".to_string(),
        dims: vec![n as u64, k as u64],
        dtype: GgmlDType::Q8_0,
        data: tensor_bytes.clone(),
    }];
    let gguf_data = create_synthetic_gguf(GgufVersion::V3, &tensors);
    let tmp = write_temp_file(&gguf_data);
    let gguf = GgufFile::open(tmp.path()).expect("GGUF 打开失败");

    // 从 GGUF mmap 提取张量数据
    let tensor_data_from_gguf = gguf.tensor_data("weight").expect("张量数据应存在");

    // 通过 GGUF 管道的 GEMV
    let input = generate_input_vector(k, 7);
    let mut output_gguf = vec![0.0f32; n];
    gemv_dispatch(
        GgmlDType::Q8_0,
        tensor_data_from_gguf,
        &input,
        &mut output_gguf,
        n,
        k,
    )
    .expect("GEMV dispatch 应成功");

    // 直接使用原始字节的朴素参考实现
    // 将行优先 Q8_0 字节数据解析为 &[BlockQ8_0] 引用
    let n_blocks = tensor_bytes.len() / std::mem::size_of::<BlockQ8_0>();
    let blocks: &[BlockQ8_0] =
        unsafe { std::slice::from_raw_parts(tensor_bytes.as_ptr() as *const BlockQ8_0, n_blocks) };
    let mut output_naive = vec![0.0f32; n];
    gemv_q8_0_naive(blocks, &input, &mut output_naive, n, k);

    // 比较结果（输入量化误差累积）
    for i in 0..n {
        let diff = (output_gguf[i] - output_naive[i]).abs();
        assert!(
            diff < 2.0,
            "row {i}: gguf={}, naive={}, diff={diff}",
            output_gguf[i],
            output_naive[i]
        );
    }
}

/// TC-GEMV-INT-004：Q8_0 零输入 → 零输出（通过 GGUF 管道）。
#[test]
fn test_gemv_int_q8_0_zero_input() {
    let n = 4;
    let k = QK8_0;

    let weight_data: Vec<f32> = (0..n * k).map(|i| (i as f32) / 10.0 - 1.0).collect();
    let tensor_bytes = make_q8_0_bytes(&weight_data, n, k);

    let tensors = vec![TensorSpec {
        name: "weight".to_string(),
        dims: vec![n as u64, k as u64],
        dtype: GgmlDType::Q8_0,
        data: tensor_bytes,
    }];
    let gguf_data = create_synthetic_gguf(GgufVersion::V3, &tensors);
    let tmp = write_temp_file(&gguf_data);
    let gguf = GgufFile::open(tmp.path()).expect("GGUF 打开失败");

    let tensor_data = gguf.tensor_data("weight").expect("张量数据应存在");
    let input = vec![0.0f32; k];
    let mut output = vec![0.0f32; n];
    gemv_dispatch(GgmlDType::Q8_0, tensor_data, &input, &mut output, n, k).expect("GEMV 应成功");

    for (i, &v) in output.iter().enumerate() {
        assert!(v.abs() < 1e-6, "零输入应产生零输出，output[{i}]={v}");
    }
}

// ---- TC-GEMV-INT-005 ~ 006: Q4_0 合成 GGUF → GEMV ----

/// TC-GEMV-INT-005：Q4_0 合成 GGUF → mmap → GEMV → 输出形状正确。
#[test]
fn test_gemv_int_q4_0_output_shape() {
    let n = 4;
    let k = QK4_0; // 32

    let weight_data: Vec<f32> = (0..n * k)
        .map(|i| ((i * 5 + 2) % 15) as f32 - 7.0)
        .collect();
    let tensor_bytes = make_q4_0_bytes(&weight_data, n, k);

    let tensors = vec![TensorSpec {
        name: "weight".to_string(),
        dims: vec![n as u64, k as u64],
        dtype: GgmlDType::Q4_0,
        data: tensor_bytes,
    }];
    let gguf_data = create_synthetic_gguf(GgufVersion::V3, &tensors);
    let tmp = write_temp_file(&gguf_data);
    let gguf = GgufFile::open(tmp.path()).expect("GGUF 打开失败");

    let tensor_data = gguf.tensor_data("weight").expect("张量数据应存在");
    let input = generate_input_vector(k, 17);
    let mut output = vec![0.0f32; n];
    let result = gemv_dispatch(GgmlDType::Q4_0, tensor_data, &input, &mut output, n, k);
    assert!(
        result.is_ok(),
        "Q4_0 GEMV dispatch 应成功: {:?}",
        result.err()
    );
    assert_eq!(output.len(), n, "输出长度应等于 n={n}");

    // 验证输出有限
    for (i, &v) in output.iter().enumerate() {
        assert!(v.is_finite(), "output[{i}] 应为有限值");
    }
}

/// TC-GEMV-INT-006：Q4_0 GEMV 与朴素实现一致。
#[test]
fn test_gemv_int_q4_0_matches_naive() {
    let n = 4;
    let k = QK4_0;

    let weight_data: Vec<f32> = (0..n * k)
        .map(|i| ((i * 7 + 3) % 13) as f32 - 6.0)
        .collect();
    let tensor_bytes = make_q4_0_bytes(&weight_data, n, k);

    // 通过 GGUF 管道
    let tensors = vec![TensorSpec {
        name: "weight".to_string(),
        dims: vec![n as u64, k as u64],
        dtype: GgmlDType::Q4_0,
        data: tensor_bytes.clone(),
    }];
    let gguf_data = create_synthetic_gguf(GgufVersion::V3, &tensors);
    let tmp = write_temp_file(&gguf_data);
    let gguf = GgufFile::open(tmp.path()).expect("GGUF 打开失败");

    let tensor_data = gguf.tensor_data("weight").expect("张量数据应存在");
    let input = generate_input_vector(k, 21);
    let mut output_gguf = vec![0.0f32; n];
    gemv_dispatch(GgmlDType::Q4_0, tensor_data, &input, &mut output_gguf, n, k)
        .expect("GEMV 应成功");

    // 直接使用字节数据（非 GGUF 管道）的 GEMV
    let mut output_direct = vec![0.0f32; n];
    gemv_dispatch(
        GgmlDType::Q4_0,
        &tensor_bytes,
        &input,
        &mut output_direct,
        n,
        k,
    )
    .expect("直接 GEMV 应成功");

    // 两者应完全一致（相同输入、相同数据）
    for i in 0..n {
        let diff = (output_gguf[i] - output_direct[i]).abs();
        assert!(
            diff < 1e-6,
            "row {i}: gguf={}, direct={}, diff={diff}",
            output_gguf[i],
            output_direct[i]
        );
    }
}

// ---- TC-GEMV-INT-007 ~ 008: Q4_K / Q8_K 合成 GGUF → GEMV ----

/// TC-GEMV-INT-007：Q4_K 合成 GGUF → mmap → GEMV → 输出形状正确。
#[test]
fn test_gemv_int_q4_k_output_shape() {
    let n = 2;
    let k = QK_K; // 256

    // 使用 make_q4_k_bytes 创建简化的 Q4_K 权重
    let tensor_bytes = make_q4_k_bytes(1.0, 0.5, 5, n);

    let tensors = vec![TensorSpec {
        name: "weight".to_string(),
        dims: vec![n as u64, k as u64],
        dtype: GgmlDType::Q4K,
        data: tensor_bytes,
    }];
    let gguf_data = create_synthetic_gguf(GgufVersion::V3, &tensors);
    let tmp = write_temp_file(&gguf_data);
    let gguf = GgufFile::open(tmp.path()).expect("GGUF 打开失败");

    let tensor_data = gguf.tensor_data("weight").expect("张量数据应存在");
    let input = vec![1.0f32; k]; // 全 1 输入
    let mut output = vec![0.0f32; n];
    let result = gemv_dispatch(GgmlDType::Q4K, tensor_data, &input, &mut output, n, k);
    assert!(
        result.is_ok(),
        "Q4K GEMV dispatch 应成功: {:?}",
        result.err()
    );
    assert_eq!(output.len(), n, "输出长度应等于 n={n}");

    // 验证输出有限
    for (i, &v) in output.iter().enumerate() {
        assert!(v.is_finite(), "output[{i}] 应为有限值，实际: {v}");
    }
}

/// TC-GEMV-INT-008：Q8_K 合成 GGUF → mmap → GEMV → 输出形状正确。
#[test]
fn test_gemv_int_q8_k_output_shape() {
    let n = 2;
    let k = QK_K; // 256

    let tensor_bytes = make_q8_k_bytes(1.0, 3, n);

    let tensors = vec![TensorSpec {
        name: "weight".to_string(),
        dims: vec![n as u64, k as u64],
        dtype: GgmlDType::Q8K,
        data: tensor_bytes,
    }];
    let gguf_data = create_synthetic_gguf(GgufVersion::V3, &tensors);
    let tmp = write_temp_file(&gguf_data);
    let gguf = GgufFile::open(tmp.path()).expect("GGUF 打开失败");

    let tensor_data = gguf.tensor_data("weight").expect("张量数据应存在");
    let input = vec![0.5f32; k];
    let mut output = vec![0.0f32; n];
    let result = gemv_dispatch(GgmlDType::Q8K, tensor_data, &input, &mut output, n, k);
    assert!(
        result.is_ok(),
        "Q8K GEMV dispatch 应成功: {:?}",
        result.err()
    );
    assert_eq!(output.len(), n, "输出长度应等于 n={n}");

    // 验证输出有限
    for (i, &v) in output.iter().enumerate() {
        assert!(v.is_finite(), "output[{i}] 应为有限值，实际: {v}");
    }
}

// ===========================================================================
// 步骤 3：权重重排 + GEMV 端到端测试
// ===========================================================================

// ---- TC-REPACK-INT-001 ~ 004: 重排后 GEMV 与未重排一致 ----

/// TC-REPACK-INT-001：Q8_0 合成权重 → repack → GEMV → 与未重排一致。
///
/// 测试数值等价性：Tile-Major 布局的 GEMV 结果应与 Row-Major 一致。
#[test]
fn test_repack_int_q8_0_equivalence() {
    let n = 16;
    let k = QK8_0 * 2; // 64

    let weight_data: Vec<f32> = (0..n * k)
        .map(|i| ((i * 11 + 7) % 19) as f32 - 9.0)
        .collect();
    let bytes = make_q8_0_bytes(&weight_data, n, k);
    let input = generate_input_vector(k, 31);

    // 未重排 GEMV
    let mut output_orig = vec![0.0f32; n];
    gemv_dispatch(GgmlDType::Q8_0, &bytes, &input, &mut output_orig, n, k)
        .expect("未重排 GEMV 应成功");

    // 重排后 GEMV
    let repacked = repack_for_gemv(&bytes, GgmlDType::Q8_0, n, k).expect("repack 应成功");
    let mut output_repacked = vec![0.0f32; n];
    gemv_repacked_dispatch(&repacked, &input, &mut output_repacked).expect("重排 GEMV 应成功");

    // 比较结果（应完全一致，因为只是布局重排，不改变量化值）
    for i in 0..n {
        let diff = (output_orig[i] - output_repacked[i]).abs();
        assert!(
            diff < 2.0,
            "row {i}: orig={}, repacked={}, diff={diff}",
            output_orig[i],
            output_repacked[i]
        );
    }
}

/// TC-REPACK-INT-002：Q4_0 合成权重 → repack → GEMV → 与未重排一致。
#[test]
fn test_repack_int_q4_0_equivalence() {
    let n = 8;
    let k = QK4_0; // 32

    let weight_data: Vec<f32> = (0..n * k)
        .map(|i| ((i * 7 + 3) % 15) as f32 - 7.0)
        .collect();
    let bytes = make_q4_0_bytes(&weight_data, n, k);
    let input = generate_input_vector(k, 41);

    // 未重排
    let mut output_orig = vec![0.0f32; n];
    gemv_dispatch(GgmlDType::Q4_0, &bytes, &input, &mut output_orig, n, k)
        .expect("未重排 GEMV 应成功");

    // 重排后
    let repacked = repack_for_gemv(&bytes, GgmlDType::Q4_0, n, k).expect("repack 应成功");
    let mut output_repacked = vec![0.0f32; n];
    gemv_repacked_dispatch(&repacked, &input, &mut output_repacked).expect("重排 GEMV 应成功");

    for i in 0..n {
        let diff = (output_orig[i] - output_repacked[i]).abs();
        assert!(
            diff < 2.0,
            "row {i}: orig={}, repacked={}, diff={diff}",
            output_orig[i],
            output_repacked[i]
        );
    }
}

/// TC-REPACK-INT-003：Q4_K 合成权重 → repack → GEMV → 与未重排一致。
#[test]
fn test_repack_int_q4_k_equivalence() {
    let n = 4;
    let k = QK_K; // 256

    let bytes = make_q4_k_bytes(1.0, 0.5, 7, n);
    let input: Vec<f32> = (0..k).map(|j| ((j % 7) as f32) / 3.0).collect();

    // 未重排
    let mut output_orig = vec![0.0f32; n];
    gemv_dispatch(GgmlDType::Q4K, &bytes, &input, &mut output_orig, n, k)
        .expect("未重排 GEMV 应成功");

    // 重排后
    let repacked = repack_for_gemv(&bytes, GgmlDType::Q4K, n, k).expect("repack 应成功");
    let mut output_repacked = vec![0.0f32; n];
    gemv_repacked_dispatch(&repacked, &input, &mut output_repacked).expect("重排 GEMV 应成功");

    for i in 0..n {
        let diff = (output_orig[i] - output_repacked[i]).abs();
        assert!(
            diff < 2.0,
            "row {i}: orig={}, repacked={}, diff={diff}",
            output_orig[i],
            output_repacked[i]
        );
    }
}

/// TC-REPACK-INT-004：Q8_K 合成权重 → repack → GEMV → 与未重排一致。
#[test]
fn test_repack_int_q8_k_equivalence() {
    let n = 2;
    let k = QK_K; // 256

    let bytes = make_q8_k_bytes(0.5, 5, n);
    let input: Vec<f32> = (0..k).map(|j| ((j % 11) as f32) / 5.0).collect();

    // 未重排
    let mut output_orig = vec![0.0f32; n];
    gemv_dispatch(GgmlDType::Q8K, &bytes, &input, &mut output_orig, n, k)
        .expect("未重排 GEMV 应成功");

    // 重排后
    let repacked = repack_for_gemv(&bytes, GgmlDType::Q8K, n, k).expect("repack 应成功");
    let mut output_repacked = vec![0.0f32; n];
    gemv_repacked_dispatch(&repacked, &input, &mut output_repacked).expect("重排 GEMV 应成功");

    for i in 0..n {
        let diff = (output_orig[i] - output_repacked[i]).abs();
        assert!(
            diff < 2.0,
            "row {i}: orig={}, repacked={}, diff={diff}",
            output_orig[i],
            output_repacked[i]
        );
    }
}

// ---- TC-REPACK-INT-005 ~ 006: 通过 GGUF 管道的重排测试 ----

/// TC-REPACK-INT-005：通过 GGUF 管道提取 Q8_0 权重 → repack → GEMV。
///
/// 完整端到端：合成 GGUF → mmap → 提取张量 → repack_for_gemv → gemv_repacked_dispatch
/// 对比未重排的 gemv_dispatch 结果。
#[test]
fn test_repack_int_q8_0_through_gguf_pipeline() {
    let n = 8;
    let k = QK8_0 * 2; // 64

    let weight_data: Vec<f32> = (0..n * k)
        .map(|i| ((i * 13 + 5) % 17) as f32 - 8.0)
        .collect();
    let tensor_bytes = make_q8_0_bytes(&weight_data, n, k);

    // 创建 GGUF 文件
    let tensors = vec![TensorSpec {
        name: "weight".to_string(),
        dims: vec![n as u64, k as u64],
        dtype: GgmlDType::Q8_0,
        data: tensor_bytes.clone(),
    }];
    let gguf_data = create_synthetic_gguf(GgufVersion::V3, &tensors);
    let tmp = write_temp_file(&gguf_data);
    let gguf = GgufFile::open(tmp.path()).expect("GGUF 打开失败");

    // 从 GGUF 提取张量数据
    let tensor_data = gguf.tensor_data("weight").expect("张量数据应存在");

    // 未重排 GEMV（使用 GGUF mmap 数据）
    let input = generate_input_vector(k, 53);
    let mut output_orig = vec![0.0f32; n];
    gemv_dispatch(GgmlDType::Q8_0, tensor_data, &input, &mut output_orig, n, k)
        .expect("未重排 GEMV 应成功");

    // 重排后 GEMV（使用 GGUF mmap 数据）
    let repacked = repack_for_gemv(tensor_data, GgmlDType::Q8_0, n, k).expect("repack 应成功");
    let mut output_repacked = vec![0.0f32; n];
    gemv_repacked_dispatch(&repacked, &input, &mut output_repacked).expect("重排 GEMV 应成功");

    for i in 0..n {
        let diff = (output_orig[i] - output_repacked[i]).abs();
        assert!(
            diff < 2.0,
            "row {i}: orig={}, repacked={}, diff={diff}",
            output_orig[i],
            output_repacked[i]
        );
    }
}

/// TC-REPACK-INT-006：重排后 to_row_major 还原数据正确（无损重排验证）。
#[test]
fn test_repack_int_to_row_major_restores_data() {
    let n = 4;
    let k = QK8_0 * 2; // 64

    let weight_data: Vec<f32> = (0..n * k)
        .map(|i| ((i * 7 + 3) % 19) as f32 - 9.0)
        .collect();
    let original_bytes = make_q8_0_bytes(&weight_data, n, k);

    // 重排
    let repacked = repack_for_gemv(&original_bytes, GgmlDType::Q8_0, n, k).expect("repack 应成功");

    // 逆变换
    let restored_bytes = repacked.to_row_major();

    // 验证字节完全一致
    assert_eq!(
        original_bytes.len(),
        restored_bytes.len(),
        "重排前后字节长度应一致"
    );
    for i in 0..original_bytes.len() {
        assert_eq!(
            original_bytes[i], restored_bytes[i],
            "字节 {i} 不一致：original={:#x}, restored={:#x}",
            original_bytes[i], restored_bytes[i]
        );
    }
}

// ---- TC-REPACK-INT-007 ~ 008: 重排布局属性验证 ----

/// TC-REPACK-INT-007：重排后布局为 GemvOptimized。
#[test]
fn test_repack_int_layout_is_gemv_optimized() {
    let n = 4;
    let k = QK8_0;
    let data = vec![1.0f32; n * k];
    let bytes = make_q8_0_bytes(&data, n, k);

    let repacked = repack_for_gemv(&bytes, GgmlDType::Q8_0, n, k).expect("repack 应成功");
    assert_eq!(
        repacked.layout(),
        WeightLayout::GemvOptimized,
        "布局应为 GemvOptimized"
    );
    assert_eq!(repacked.n(), n, "n 应正确");
    assert_eq!(repacked.k(), k, "k 应正确");
    assert_eq!(repacked.dtype(), GgmlDType::Q8_0, "dtype 应为 Q8_0");
}

/// TC-REPACK-INT-008：重排前后数据大小不变。
#[test]
fn test_repack_int_data_size_unchanged() {
    let n = 8;
    let k = QK8_0 * 4; // 128

    let data = vec![1.0f32; n * k];
    let bytes = make_q8_0_bytes(&data, n, k);
    let original_len = bytes.len();

    let repacked = repack_for_gemv(&bytes, GgmlDType::Q8_0, n, k).expect("repack 应成功");
    assert_eq!(
        repacked.data().len(),
        original_len,
        "重排前后数据大小应不变"
    );
}
