#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

//! 合成 GGUF 测试 fixture 生成器 + V1/V2/V3 解析器集成测试。
//!
//! 本模块提供：
//! 1. 合成 GGUF V1/V2/V3 文件生成器（支持 F32/Q4_0/Q4_K/Q8_0/Q8_K 张量）
//! 2. GGUF 解析器 V1/V2/V3 合成文件测试（替代 `#[ignore]` 真实文件测试）
//! 3. `pub(crate)` 辅助函数供 `gemv_integration_tests` 模块使用
//!
//! 所有测试在 CI 中自动运行（非 `#[ignore]`），合成文件 < 1KB。

use super::gguf_reader::*;
use crate::gguf_reader::GgmlDType;
use half::f16;
use std::io::Write;
use tempfile::NamedTempFile;

// ===========================================================================
// 第一部分：GGUF 版本枚举与二进制写入辅助
// ===========================================================================

/// GGUF 文件版本。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GgufVersion {
    /// V1：计数/字符串长度使用 u32
    V1,
    /// V2：计数/字符串长度使用 u64
    V2,
    /// V3：与 V2 相同，但版本号为 3
    V3,
}

impl GgufVersion {
    /// V2/V3 使用 u64 计数和字符串长度，V1 使用 u32。
    fn use_u64(&self) -> bool {
        matches!(self, Self::V2 | Self::V3)
    }

    /// 版本号。
    fn version_num(&self) -> u32 {
        match self {
            Self::V1 => 1,
            Self::V2 => 2,
            Self::V3 => 3,
        }
    }
}

/// 写入 GGUF 字符串（根据版本选择 u32 或 u64 长度前缀）。
pub(crate) fn write_gguf_str(buf: &mut Vec<u8>, version: GgufVersion, s: &str) {
    if version.use_u64() {
        buf.extend_from_slice(&(s.len() as u64).to_le_bytes());
    } else {
        buf.extend_from_slice(&(s.len() as u32).to_le_bytes());
    }
    buf.extend_from_slice(s.as_bytes());
}

/// 写入 GGUF 计数（u32 或 u64，根据版本）。
pub(crate) fn write_gguf_count(buf: &mut Vec<u8>, version: GgufVersion, val: usize) {
    if version.use_u64() {
        buf.extend_from_slice(&(val as u64).to_le_bytes());
    } else {
        buf.extend_from_slice(&(val as u32).to_le_bytes());
    }
}

/// 将 `GgmlDType` 转为 GGUF 规范的 u32 类型值。
pub(crate) fn dtype_to_u32(dtype: GgmlDType) -> u32 {
    match dtype {
        GgmlDType::F32 => 0,
        GgmlDType::F16 => 1,
        GgmlDType::Q4_0 => 2,
        GgmlDType::Q4_1 => 3,
        GgmlDType::Q5_0 => 6,
        GgmlDType::Q5_1 => 7,
        GgmlDType::Q8_0 => 8,
        GgmlDType::Q8_1 => 9,
        GgmlDType::Q2K => 10,
        GgmlDType::Q3K => 11,
        GgmlDType::Q4K => 12,
        GgmlDType::Q5K => 13,
        GgmlDType::Q6K => 14,
        GgmlDType::Q8K => 15,
        GgmlDType::BF16 => 24,
        GgmlDType::Unknown(v) => v,
    }
}

/// 计算张量数据字节大小。
pub(crate) fn tensor_data_size(dtype: GgmlDType, n_elements: u64) -> usize {
    let (block_size, block_bytes) = dtype.block_layout();
    if block_size == 0 {
        (n_elements * block_bytes) as usize
    } else {
        let n_blocks = n_elements.checked_div(block_size).unwrap_or(0);
        (n_blocks * block_bytes) as usize
    }
}

// ===========================================================================
// 第二部分：张量规格与合成 GGUF 文件生成器
// ===========================================================================

/// 张量规格定义（名称 + 维度 + 量化类型 + 数据）。
pub(crate) struct TensorSpec {
    /// 张量名称（如 `token_embd.weight`）
    pub name: String,
    /// 维度列表（GGUF 倒序，dims[0]=N, dims[1]=K）
    pub dims: Vec<u64>,
    /// 量化数据类型
    pub dtype: GgmlDType,
    /// 张量原始字节数据
    pub data: Vec<u8>,
}

/// 生成合成 GGUF 文件（指定版本 + 张量列表 + 标准元数据）。
///
/// 创建一个包含 `general.architecture` / `general.name` / `general.file_type`
/// 元数据和给定张量的最小 GGUF 文件。张量数据由调用方提供。
///
/// # 参数
///
/// - `version`：GGUF 版本（V1/V2/V3）
/// - `tensors`：张量规格列表
///
/// # 返回
///
/// 完整的 GGUF 文件字节向量（header + alignment padding + tensor data）
pub(crate) fn create_synthetic_gguf(version: GgufVersion, tensors: &[TensorSpec]) -> Vec<u8> {
    let mut buf = Vec::new();

    // ---- 文件头 ----
    buf.extend_from_slice(&0x4655_4747u32.to_le_bytes()); // Magic "GGUF"
    buf.extend_from_slice(&version.version_num().to_le_bytes()); // Version
    write_gguf_count(&mut buf, version, tensors.len()); // tensor_count
    write_gguf_count(&mut buf, version, 3); // metadata_kv_count = 3

    // ---- 元数据 ----
    // 1. general.architecture = "qwen2"
    write_gguf_str(&mut buf, version, "general.architecture");
    buf.extend_from_slice(&8u32.to_le_bytes()); // STRING type = 8
    write_gguf_str(&mut buf, version, "qwen2");

    // 2. general.name = "synthetic_test_model"
    write_gguf_str(&mut buf, version, "general.name");
    buf.extend_from_slice(&8u32.to_le_bytes());
    write_gguf_str(&mut buf, version, "synthetic_test_model");

    // 3. general.file_type = 1 (UINT32 type = 4)
    write_gguf_str(&mut buf, version, "general.file_type");
    buf.extend_from_slice(&4u32.to_le_bytes()); // UINT32 type = 4
    buf.extend_from_slice(&1u32.to_le_bytes());

    // ---- 计算张量偏移量（相对于数据区起始） ----
    let mut offsets = Vec::with_capacity(tensors.len());
    let mut current_offset: u64 = 0;
    for spec in tensors {
        offsets.push(current_offset);
        current_offset += spec.data.len() as u64;
    }

    // ---- 张量信息 ----
    for (i, spec) in tensors.iter().enumerate() {
        write_gguf_str(&mut buf, version, &spec.name);
        buf.extend_from_slice(&(spec.dims.len() as u32).to_le_bytes()); // n_dims
        for &d in &spec.dims {
            buf.extend_from_slice(&d.to_le_bytes()); // dims (always u64)
        }
        buf.extend_from_slice(&dtype_to_u32(spec.dtype).to_le_bytes()); // dtype (always u32)
        buf.extend_from_slice(&offsets[i].to_le_bytes()); // offset (always u64)
    }

    // ---- 对齐填充到 32 字节 ----
    let header_end = buf.len();
    let aligned = header_end.div_ceil(32) * 32;
    buf.resize(aligned, 0);

    // ---- 张量数据 ----
    for spec in tensors {
        buf.extend_from_slice(&spec.data);
    }

    buf
}

/// 创建填充字节的张量数据（用于解析器测试，不需要有效量化值）。
pub(crate) fn fill_tensor_data(dtype: GgmlDType, dims: &[u64], fill_byte: u8) -> Vec<u8> {
    let n_elements: u64 = dims.iter().product();
    let size = tensor_data_size(dtype, n_elements);
    vec![fill_byte; size]
}

/// 将字节写入临时文件并返回文件路径。
pub(crate) fn write_temp_file(data: &[u8]) -> NamedTempFile {
    let mut tmp = tempfile::NamedTempFile::new().expect("创建临时文件失败");
    tmp.write_all(data).expect("写入临时文件失败");
    tmp
}

// ===========================================================================
// 第三部分：量化块字节序列化辅助（供 GEMV 集成测试使用）
// ===========================================================================

/// 从 f32 数据创建 Q8_0 权重字节（行优先布局）。
///
/// # 参数
///
/// - `data`：f32 权重值，长度 = n * k
/// - `n`：输出维度（行数）
/// - `k`：输入维度（列数），必须为 QK8_0 (32) 的倍数
pub(crate) fn make_q8_0_bytes(data: &[f32], n: usize, k: usize) -> Vec<u8> {
    use crate::quant_blocks::{BlockQ8_0, QK8_0};
    let blocks_per_row = k / QK8_0;
    let mut bytes = Vec::with_capacity(n * blocks_per_row * std::mem::size_of::<BlockQ8_0>());
    for row in 0..n {
        for b in 0..blocks_per_row {
            let start = row * k + b * QK8_0;
            let chunk = &data[start..start + QK8_0];
            let mut amax = 0.0f32;
            for &x in chunk {
                amax = amax.max(x.abs());
            }
            let d = if amax > 0.0 { amax / 127.0 } else { 0.0 };
            let id = if d > 0.0 { 1.0 / d } else { 0.0 };
            let mut qs = [0i8; QK8_0];
            for (i, &x) in chunk.iter().enumerate() {
                qs[i] = (x * id).round().clamp(-128.0, 127.0) as i8;
            }
            let block = BlockQ8_0 {
                d: f16::from_f32(d),
                qs,
            };
            // SAFETY: BlockQ8_0 is #[repr(C)] with plain data (f16 + [i8;32]).
            // 我们读取字节视图用于序列化，不违反别名规则。
            unsafe {
                let slice = std::slice::from_raw_parts(
                    &block as *const BlockQ8_0 as *const u8,
                    std::mem::size_of::<BlockQ8_0>(),
                );
                bytes.extend_from_slice(slice);
            }
        }
    }
    bytes
}

/// 从 f32 数据创建 Q4_0 权重字节（行优先布局）。
///
/// Q4_0 量化：nibble 范围 0~15，居中值为 nibble - 8（范围 -8~7）。
/// 缩放因子 d = amax / 7.0，nibble = round(x / d) + 8。
pub(crate) fn make_q4_0_bytes(data: &[f32], n: usize, k: usize) -> Vec<u8> {
    use crate::quant_blocks::{BlockQ4_0, QK4_0};
    let blocks_per_row = k / QK4_0;
    let mut bytes = Vec::with_capacity(n * blocks_per_row * std::mem::size_of::<BlockQ4_0>());
    for row in 0..n {
        for b in 0..blocks_per_row {
            let start = row * k + b * QK4_0;
            let chunk = &data[start..start + QK4_0];
            let mut amax = 0.0f32;
            for &x in chunk {
                amax = amax.max(x.abs());
            }
            let d = if amax > 0.0 { amax / 7.0 } else { 0.0 };
            let id = if d > 0.0 { 1.0 / d } else { 0.0 };
            let mut qs = [0u8; QK4_0 / 2];
            for j in 0..(QK4_0 / 2) {
                let q0 = ((chunk[j] * id).round() + 8.0).clamp(0.0, 15.0) as u8;
                let q1 = ((chunk[j + QK4_0 / 2] * id).round() + 8.0).clamp(0.0, 15.0) as u8;
                qs[j] = q0 | (q1 << 4);
            }
            let block = BlockQ4_0 {
                d: f16::from_f32(d),
                qs,
            };
            unsafe {
                let slice = std::slice::from_raw_parts(
                    &block as *const BlockQ4_0 as *const u8,
                    std::mem::size_of::<BlockQ4_0>(),
                );
                bytes.extend_from_slice(slice);
            }
        }
    }
    bytes
}

/// 创建 Q4_K 权重字节（简化：scales 前 4 子块 sc=1/m=1，后 4 子块 sc=0/m=0）。
///
/// # 参数
///
/// - `d`：超块缩放因子
/// - `dmin`：超块 min 缩放因子
/// - `nibble_val`：所有 nibble 的填充值（0~15）
/// - `n_blocks`：块数
pub(crate) fn make_q4_k_bytes(d: f32, dmin: f32, nibble_val: u8, n_blocks: usize) -> Vec<u8> {
    use crate::quant_blocks::{BlockQ4K, K_SCALE_SIZE, QK_K};
    let mut scales = [0u8; K_SCALE_SIZE];
    // 前 4 子块 sc=1, m=1
    scales[0] = 1;
    scales[1] = 1;
    scales[2] = 1;
    scales[3] = 1;
    scales[4] = 1;
    scales[5] = 1;
    scales[6] = 1;
    scales[7] = 1;
    // 后 4 子块 sc=0, m=0（scales[8..11] = 0）

    let block = BlockQ4K {
        d: f16::from_f32(d),
        dmin: f16::from_f32(dmin),
        scales,
        qs: [nibble_val; QK_K / 2],
    };
    let mut bytes = Vec::with_capacity(n_blocks * std::mem::size_of::<BlockQ4K>());
    for _ in 0..n_blocks {
        unsafe {
            let slice = std::slice::from_raw_parts(
                &block as *const BlockQ4K as *const u8,
                std::mem::size_of::<BlockQ4K>(),
            );
            bytes.extend_from_slice(slice);
        }
    }
    bytes
}

/// 创建 Q8_K 权重字节（简化：d=指定值，qs 全填充）。
///
/// # 参数
///
/// - `d`：f32 缩放因子
/// - `qs_val`：所有 qs 元素的填充值
/// - `n_blocks`：块数
pub(crate) fn make_q8_k_bytes(d: f32, qs_val: i8, n_blocks: usize) -> Vec<u8> {
    use crate::quant_blocks::{BlockQ8K, QK_K};
    let block = BlockQ8K {
        d,
        qs: [qs_val; QK_K],
        bsums: [0; QK_K / 16],
    };
    let mut bytes = Vec::with_capacity(n_blocks * std::mem::size_of::<BlockQ8K>());
    for _ in 0..n_blocks {
        unsafe {
            let slice = std::slice::from_raw_parts(
                &block as *const BlockQ8K as *const u8,
                std::mem::size_of::<BlockQ8K>(),
            );
            bytes.extend_from_slice(slice);
        }
    }
    bytes
}

/// 生成确定性伪随机 f32 向量（LCG 算法，种子固定）。
///
/// 用于 GEMV 测试的输入向量生成，保证结果可重现。
pub(crate) fn generate_input_vector(k: usize, seed: u32) -> Vec<f32> {
    let mut state = seed;
    (0..k)
        .map(|_| {
            // LCG: state = state * 1103515245 + 12345 (glibc)
            state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            // 映射到 [-1.0, 1.0)
            ((state >> 16) as f32 / 32_768.0) - 1.0
        })
        .collect()
}

// ===========================================================================
// 第四部分：V1/V2/V3 解析器集成测试
// ===========================================================================

// ---- TC-SYN-GGUF-001 ~ 003: 各版本文件打开成功 ----

/// TC-SYN-GGUF-001：V1 文件打开成功。
///
/// 创建最小 GGUF V1 文件（F32 张量），验证 `GgufFile::open` 返回 Ok。
#[test]
fn test_syn_gguf_v1_open_succeeds() {
    let tensors = vec![TensorSpec {
        name: "token_embd.weight".to_string(),
        dims: vec![4, 4],
        dtype: GgmlDType::F32,
        data: fill_tensor_data(GgmlDType::F32, &[4, 4], 0xAB),
    }];
    let data = create_synthetic_gguf(GgufVersion::V1, &tensors);
    let tmp = write_temp_file(&data);
    let result = GgufFile::open(tmp.path());
    assert!(result.is_ok(), "V1 文件应打开成功: {:?}", result.err());
}

/// TC-SYN-GGUF-002：V2 文件打开成功。
#[test]
fn test_syn_gguf_v2_open_succeeds() {
    let tensors = vec![TensorSpec {
        name: "token_embd.weight".to_string(),
        dims: vec![4, 4],
        dtype: GgmlDType::F32,
        data: fill_tensor_data(GgmlDType::F32, &[4, 4], 0xCD),
    }];
    let data = create_synthetic_gguf(GgufVersion::V2, &tensors);
    let tmp = write_temp_file(&data);
    let result = GgufFile::open(tmp.path());
    assert!(result.is_ok(), "V2 文件应打开成功: {:?}", result.err());
}

/// TC-SYN-GGUF-003：V3 文件打开成功。
#[test]
fn test_syn_gguf_v3_open_succeeds() {
    let tensors = vec![TensorSpec {
        name: "token_embd.weight".to_string(),
        dims: vec![4, 4],
        dtype: GgmlDType::F32,
        data: fill_tensor_data(GgmlDType::F32, &[4, 4], 0xEF),
    }];
    let data = create_synthetic_gguf(GgufVersion::V3, &tensors);
    let tmp = write_temp_file(&data);
    let result = GgufFile::open(tmp.path());
    assert!(result.is_ok(), "V3 文件应打开成功: {:?}", result.err());
}

// ---- TC-SYN-GGUF-004 ~ 006: 元数据解析 ----

/// TC-SYN-GGUF-004：V1 元数据解析正确（architecture + name + file_type）。
#[test]
fn test_syn_gguf_v1_metadata() {
    let tensors = vec![TensorSpec {
        name: "token_embd.weight".to_string(),
        dims: vec![4, 4],
        dtype: GgmlDType::F32,
        data: fill_tensor_data(GgmlDType::F32, &[4, 4], 0x00),
    }];
    let data = create_synthetic_gguf(GgufVersion::V1, &tensors);
    let tmp = write_temp_file(&data);
    let gguf = GgufFile::open(tmp.path()).expect("V1 打开失败");

    let meta = gguf.metadata();
    // architecture
    match meta.get("general.architecture") {
        Some(GgufValue::String(s)) => assert_eq!(s, "qwen2", "architecture 应为 qwen2"),
        other => panic!("architecture 应为 String，实际: {other:?}"),
    }
    // name
    match meta.get("general.name") {
        Some(GgufValue::String(s)) => assert_eq!(s, "synthetic_test_model", "name 应正确"),
        other => panic!("name 应为 String，实际: {other:?}"),
    }
    // file_type
    match meta.get("general.file_type") {
        Some(GgufValue::Uint32(v)) => assert_eq!(*v, 1, "file_type 应为 1"),
        other => panic!("file_type 应为 Uint32，实际: {other:?}"),
    }
}

/// TC-SYN-GGUF-005：V2 元数据解析正确。
#[test]
fn test_syn_gguf_v2_metadata() {
    let tensors = vec![TensorSpec {
        name: "token_embd.weight".to_string(),
        dims: vec![4, 4],
        dtype: GgmlDType::F32,
        data: fill_tensor_data(GgmlDType::F32, &[4, 4], 0x00),
    }];
    let data = create_synthetic_gguf(GgufVersion::V2, &tensors);
    let tmp = write_temp_file(&data);
    let gguf = GgufFile::open(tmp.path()).expect("V2 打开失败");

    let meta = gguf.metadata();
    assert!(
        meta.contains_key("general.architecture"),
        "应包含 architecture"
    );
    match meta.get("general.architecture") {
        Some(GgufValue::String(s)) => assert_eq!(s, "qwen2"),
        other => panic!("architecture 应为 String，实际: {other:?}"),
    }
}

/// TC-SYN-GGUF-006：V3 architecture() 方法返回正确值。
#[test]
fn test_syn_gguf_v3_architecture_method() {
    let tensors = vec![TensorSpec {
        name: "token_embd.weight".to_string(),
        dims: vec![2, 2],
        dtype: GgmlDType::F32,
        data: fill_tensor_data(GgmlDType::F32, &[2, 2], 0x00),
    }];
    let data = create_synthetic_gguf(GgufVersion::V3, &tensors);
    let tmp = write_temp_file(&data);
    let gguf = GgufFile::open(tmp.path()).expect("V3 打开失败");
    let arch = gguf.architecture().expect("architecture() 应成功");
    assert_eq!(arch, "qwen2", "architecture 应为 qwen2");
}

// ---- TC-SYN-GGUF-007 ~ 009: 张量信息解析 ----

/// TC-SYN-GGUF-007：V1 张量信息解析正确（dims/dtype/offset/size）。
#[test]
fn test_syn_gguf_v1_tensor_info() {
    let tensors = vec![TensorSpec {
        name: "token_embd.weight".to_string(),
        dims: vec![4, 4],
        dtype: GgmlDType::F32,
        data: fill_tensor_data(GgmlDType::F32, &[4, 4], 0x00),
    }];
    let data = create_synthetic_gguf(GgufVersion::V1, &tensors);
    let tmp = write_temp_file(&data);
    let gguf = GgufFile::open(tmp.path()).expect("V1 打开失败");

    assert_eq!(gguf.tensor_count(), 1, "张量数应为 1");
    let info = gguf
        .tensor_info("token_embd.weight")
        .expect("应存在 token_embd.weight");
    assert_eq!(info.name, "token_embd.weight", "张量名应正确");
    assert_eq!(info.dims, vec![4, 4], "dims 应为 [4, 4]");
    assert_eq!(info.dtype, GgmlDType::F32, "dtype 应为 F32");
    assert_eq!(info.offset, 0, "offset 应为 0");
    assert_eq!(info.size, 64, "size 应为 64 字节 (4*4*4)");
}

/// TC-SYN-GGUF-008：V2 张量信息解析正确。
#[test]
fn test_syn_gguf_v2_tensor_info() {
    let tensors = vec![TensorSpec {
        name: "blk.0.attn.weight".to_string(),
        dims: vec![8, 4],
        dtype: GgmlDType::F32,
        data: fill_tensor_data(GgmlDType::F32, &[8, 4], 0x00),
    }];
    let data = create_synthetic_gguf(GgufVersion::V2, &tensors);
    let tmp = write_temp_file(&data);
    let gguf = GgufFile::open(tmp.path()).expect("V2 打开失败");

    let info = gguf.tensor_info("blk.0.attn.weight").expect("应存在张量");
    assert_eq!(info.dims, vec![8, 4], "dims 应为 [8, 4]");
    assert_eq!(info.dtype, GgmlDType::F32, "dtype 应为 F32");
    assert_eq!(info.size, 128, "size 应为 128 字节 (8*4*4)");
}

/// TC-SYN-GGUF-009：V3 不存在的张量名返回 None。
#[test]
fn test_syn_gguf_v3_nonexistent_tensor() {
    let tensors = vec![TensorSpec {
        name: "token_embd.weight".to_string(),
        dims: vec![4, 4],
        dtype: GgmlDType::F32,
        data: fill_tensor_data(GgmlDType::F32, &[4, 4], 0x00),
    }];
    let data = create_synthetic_gguf(GgufVersion::V3, &tensors);
    let tmp = write_temp_file(&data);
    let gguf = GgufFile::open(tmp.path()).expect("V3 打开失败");

    assert!(
        gguf.tensor_info("nonexistent.weight").is_none(),
        "不存在的张量应返回 None"
    );
}

// ---- TC-SYN-GGUF-010 ~ 012: 张量数据零拷贝访问 ----

/// TC-SYN-GGUF-010：V1 张量数据零拷贝访问正确。
#[test]
fn test_syn_gguf_v1_tensor_data() {
    let tensors = vec![TensorSpec {
        name: "token_embd.weight".to_string(),
        dims: vec![4, 4],
        dtype: GgmlDType::F32,
        data: fill_tensor_data(GgmlDType::F32, &[4, 4], 0x42),
    }];
    let data = create_synthetic_gguf(GgufVersion::V1, &tensors);
    let tmp = write_temp_file(&data);
    let gguf = GgufFile::open(tmp.path()).expect("V1 打开失败");

    let tensor_data = gguf
        .tensor_data("token_embd.weight")
        .expect("应返回张量数据");
    assert_eq!(tensor_data.len(), 64, "数据长度应为 64 字节");
    assert!(
        tensor_data.iter().all(|&b| b == 0x42),
        "数据应全部为填充字节 0x42"
    );
}

/// TC-SYN-GGUF-011：V2 张量数据零拷贝访问正确。
#[test]
fn test_syn_gguf_v2_tensor_data() {
    let tensors = vec![TensorSpec {
        name: "test.weight".to_string(),
        dims: vec![2, 2],
        dtype: GgmlDType::F32,
        data: fill_tensor_data(GgmlDType::F32, &[2, 2], 0x99),
    }];
    let data = create_synthetic_gguf(GgufVersion::V2, &tensors);
    let tmp = write_temp_file(&data);
    let gguf = GgufFile::open(tmp.path()).expect("V2 打开失败");

    let tensor_data = gguf.tensor_data("test.weight").expect("应返回张量数据");
    assert_eq!(tensor_data.len(), 16, "数据长度应为 16 字节 (2*2*4)");
    assert!(
        tensor_data.iter().all(|&b| b == 0x99),
        "数据应全部为填充字节 0x99"
    );
}

/// TC-SYN-GGUF-012：张量名列表包含所有张量。
#[test]
fn test_syn_gguf_tensor_names() {
    let tensors = vec![
        TensorSpec {
            name: "tensor_a".to_string(),
            dims: vec![2, 2],
            dtype: GgmlDType::F32,
            data: fill_tensor_data(GgmlDType::F32, &[2, 2], 0x00),
        },
        TensorSpec {
            name: "tensor_b".to_string(),
            dims: vec![4, 2],
            dtype: GgmlDType::F32,
            data: fill_tensor_data(GgmlDType::F32, &[4, 2], 0x00),
        },
    ];
    let data = create_synthetic_gguf(GgufVersion::V3, &tensors);
    let tmp = write_temp_file(&data);
    let gguf = GgufFile::open(tmp.path()).expect("V3 打开失败");

    let names: Vec<&str> = gguf.tensor_names().collect();
    assert_eq!(names.len(), 2, "应有 2 个张量");
    assert!(names.contains(&"tensor_a"), "应包含 tensor_a");
    assert!(names.contains(&"tensor_b"), "应包含 tensor_b");
}

// ---- TC-SYN-GGUF-013 ~ 016: 量化张量类型解析 ----

/// TC-SYN-GGUF-013：Q8_0 张量 dtype 正确解析。
#[test]
fn test_syn_gguf_q8_0_dtype() {
    let tensors = vec![TensorSpec {
        name: "weight.q8_0".to_string(),
        dims: vec![4, 32],
        dtype: GgmlDType::Q8_0,
        data: fill_tensor_data(GgmlDType::Q8_0, &[4, 32], 0x00),
    }];
    let data = create_synthetic_gguf(GgufVersion::V3, &tensors);
    let tmp = write_temp_file(&data);
    let gguf = GgufFile::open(tmp.path()).expect("打开失败");

    let info = gguf.tensor_info("weight.q8_0").expect("应存在张量");
    assert_eq!(info.dtype, GgmlDType::Q8_0, "dtype 应为 Q8_0");
    // 4*32=128 元素，Q8_0 块大小 32 → 4 块 × 34 字节 = 136 字节
    assert_eq!(info.size, 136, "Q8_0 [4,32] size 应为 136");
}

/// TC-SYN-GGUF-014：Q4_0 张量 dtype 正确解析。
#[test]
fn test_syn_gguf_q4_0_dtype() {
    let tensors = vec![TensorSpec {
        name: "weight.q4_0".to_string(),
        dims: vec![4, 32],
        dtype: GgmlDType::Q4_0,
        data: fill_tensor_data(GgmlDType::Q4_0, &[4, 32], 0x00),
    }];
    let data = create_synthetic_gguf(GgufVersion::V3, &tensors);
    let tmp = write_temp_file(&data);
    let gguf = GgufFile::open(tmp.path()).expect("打开失败");

    let info = gguf.tensor_info("weight.q4_0").expect("应存在张量");
    assert_eq!(info.dtype, GgmlDType::Q4_0, "dtype 应为 Q4_0");
    // 4*32=128 元素，Q4_0 块大小 32 → 4 块 × 18 字节 = 72 字节
    assert_eq!(info.size, 72, "Q4_0 [4,32] size 应为 72");
}

/// TC-SYN-GGUF-015：Q4_K 张量 dtype 正确解析。
#[test]
fn test_syn_gguf_q4k_dtype() {
    let tensors = vec![TensorSpec {
        name: "weight.q4k".to_string(),
        dims: vec![2, 256],
        dtype: GgmlDType::Q4K,
        data: fill_tensor_data(GgmlDType::Q4K, &[2, 256], 0x00),
    }];
    let data = create_synthetic_gguf(GgufVersion::V3, &tensors);
    let tmp = write_temp_file(&data);
    let gguf = GgufFile::open(tmp.path()).expect("打开失败");

    let info = gguf.tensor_info("weight.q4k").expect("应存在张量");
    assert_eq!(info.dtype, GgmlDType::Q4K, "dtype 应为 Q4K");
    // 2*256=512 元素，Q4K 块大小 256 → 2 块 × 144 字节 = 288 字节
    assert_eq!(info.size, 288, "Q4K [2,256] size 应为 288");
}

/// TC-SYN-GGUF-016：Q8_K 张量 dtype 正确解析。
#[test]
fn test_syn_gguf_q8k_dtype() {
    let tensors = vec![TensorSpec {
        name: "weight.q8k".to_string(),
        dims: vec![2, 256],
        dtype: GgmlDType::Q8K,
        data: fill_tensor_data(GgmlDType::Q8K, &[2, 256], 0x00),
    }];
    let data = create_synthetic_gguf(GgufVersion::V3, &tensors);
    let tmp = write_temp_file(&data);
    let gguf = GgufFile::open(tmp.path()).expect("打开失败");

    let info = gguf.tensor_info("weight.q8k").expect("应存在张量");
    assert_eq!(info.dtype, GgmlDType::Q8K, "dtype 应为 Q8K");
    // 2*256=512 元素，Q8K 块大小 256 → 2 块 × 292 字节 = 584 字节
    assert_eq!(info.size, 584, "Q8K [2,256] size 应为 584");
}

// ---- TC-SYN-GGUF-017 ~ 018: data_offset 对齐与多张量偏移 ----

/// TC-SYN-GGUF-017：data_offset 32 字节对齐。
#[test]
fn test_syn_gguf_data_offset_aligned() {
    let tensors = vec![TensorSpec {
        name: "test.weight".to_string(),
        dims: vec![2, 2],
        dtype: GgmlDType::F32,
        data: fill_tensor_data(GgmlDType::F32, &[2, 2], 0x00),
    }];
    let data = create_synthetic_gguf(GgufVersion::V3, &tensors);
    let tmp = write_temp_file(&data);
    let gguf = GgufFile::open(tmp.path()).expect("打开失败");

    let offset = gguf.data_offset();
    assert!(
        offset.is_multiple_of(32),
        "data_offset 应为 32 的倍数，实际: {offset}"
    );
}

/// TC-SYN-GGUF-018：多张量 offset 计算正确（张量按顺序排列）。
#[test]
fn test_syn_gguf_multi_tensor_offsets() {
    let tensor_a_data = fill_tensor_data(GgmlDType::F32, &[2, 2], 0xAA); // 16 bytes
    let tensor_b_data = fill_tensor_data(GgmlDType::F32, &[4, 2], 0xBB); // 32 bytes
    let tensors = vec![
        TensorSpec {
            name: "tensor_a".to_string(),
            dims: vec![2, 2],
            dtype: GgmlDType::F32,
            data: tensor_a_data,
        },
        TensorSpec {
            name: "tensor_b".to_string(),
            dims: vec![4, 2],
            dtype: GgmlDType::F32,
            data: tensor_b_data,
        },
    ];
    let data = create_synthetic_gguf(GgufVersion::V3, &tensors);
    let tmp = write_temp_file(&data);
    let gguf = GgufFile::open(tmp.path()).expect("打开失败");

    let info_a = gguf.tensor_info("tensor_a").expect("tensor_a 应存在");
    let info_b = gguf.tensor_info("tensor_b").expect("tensor_b 应存在");

    // tensor_a 在偏移 0
    assert_eq!(info_a.offset, 0, "tensor_a offset 应为 0");
    assert_eq!(info_a.size, 16, "tensor_a size 应为 16");
    // tensor_b 紧跟 tensor_a
    assert_eq!(info_b.offset, 16, "tensor_b offset 应为 16");
    assert_eq!(info_b.size, 32, "tensor_b size 应为 32");

    // 验证数据内容
    let data_a = gguf.tensor_data("tensor_a").expect("tensor_a 数据");
    let data_b = gguf.tensor_data("tensor_b").expect("tensor_b 数据");
    assert!(data_a.iter().all(|&b| b == 0xAA), "tensor_a 数据应为 0xAA");
    assert!(data_b.iter().all(|&b| b == 0xBB), "tensor_b 数据应为 0xBB");
}

// ---- TC-SYN-GGUF-019 ~ 020: mmap 方法 + 文件大小验证 ----

/// TC-SYN-GGUF-019：mmap() 返回有效映射。
#[test]
fn test_syn_gguf_mmap_method() {
    let tensors = vec![TensorSpec {
        name: "test.weight".to_string(),
        dims: vec![4, 4],
        dtype: GgmlDType::F32,
        data: fill_tensor_data(GgmlDType::F32, &[4, 4], 0x00),
    }];
    let data = create_synthetic_gguf(GgufVersion::V3, &tensors);
    let tmp = write_temp_file(&data);
    let gguf = GgufFile::open(tmp.path()).expect("打开失败");

    let mmap = gguf.mmap();
    assert_eq!(mmap.len(), data.len(), "mmap 长度应等于文件大小");
}

/// TC-SYN-GGUF-020：合成文件大小 < 1KB（CI 友好）。
#[test]
fn test_syn_gguf_file_size_small() {
    let tensors = vec![
        TensorSpec {
            name: "tensor_a".to_string(),
            dims: vec![4, 4],
            dtype: GgmlDType::F32,
            data: fill_tensor_data(GgmlDType::F32, &[4, 4], 0x00),
        },
        TensorSpec {
            name: "tensor_b".to_string(),
            dims: vec![2, 2],
            dtype: GgmlDType::F32,
            data: fill_tensor_data(GgmlDType::F32, &[2, 2], 0x00),
        },
    ];
    let data = create_synthetic_gguf(GgufVersion::V3, &tensors);
    assert!(
        data.len() < 1024,
        "合成文件应 < 1KB，实际: {} 字节",
        data.len()
    );
}

// ---- TC-SYN-GGUF-021 ~ 023: V1/V2 差异验证 ----

/// TC-SYN-GGUF-021：V1 与 V2 文件大小不同（u32 vs u64 计数/长度前缀）。
#[test]
fn test_syn_gguf_v1_v2_size_difference() {
    let tensors = vec![TensorSpec {
        name: "test.weight".to_string(),
        dims: vec![2, 2],
        dtype: GgmlDType::F32,
        data: fill_tensor_data(GgmlDType::F32, &[2, 2], 0x00),
    }];
    let data_v1 = create_synthetic_gguf(GgufVersion::V1, &tensors);
    let data_v2 = create_synthetic_gguf(GgufVersion::V2, &tensors);

    // V2 使用 u64 计数和字符串长度前缀，应比 V1 更大
    // 差异来源：
    //   - tensor_count: V1=4B, V2=8B → +4B
    //   - metadata_count: V1=4B, V2=8B → +4B
    //   - 6 个字符串长度前缀 (3 metadata keys + 3 values): V1=4B each, V2=8B each → +36B
    //   - 张量名长度前缀: V1=4B, V2=8B → +4B
    // 总差异 = 4 + 4 + 36 + 4 = 48B（但对齐填充可能吸收部分差异）
    assert_ne!(
        data_v1.len(),
        data_v2.len(),
        "V1 和 V2 文件大小应不同（u32 vs u64 前缀）"
    );
}

/// TC-SYN-GGUF-022：V2 和 V3 文件大小相同（仅版本号不同）。
#[test]
fn test_syn_gguf_v2_v3_same_size() {
    let tensors = vec![TensorSpec {
        name: "test.weight".to_string(),
        dims: vec![2, 2],
        dtype: GgmlDType::F32,
        data: fill_tensor_data(GgmlDType::F32, &[2, 2], 0x00),
    }];
    let data_v2 = create_synthetic_gguf(GgufVersion::V2, &tensors);
    let data_v3 = create_synthetic_gguf(GgufVersion::V3, &tensors);
    assert_eq!(
        data_v2.len(),
        data_v3.len(),
        "V2 和 V3 文件大小应相同（仅版本号不同）"
    );
}

/// TC-SYN-GGUF-023：所有版本都能通过 architecture() 提取架构名。
#[test]
fn test_syn_gguf_all_versions_architecture() {
    let tensors = vec![TensorSpec {
        name: "test.weight".to_string(),
        dims: vec![2, 2],
        dtype: GgmlDType::F32,
        data: fill_tensor_data(GgmlDType::F32, &[2, 2], 0x00),
    }];

    for version in [GgufVersion::V1, GgufVersion::V2, GgufVersion::V3] {
        let data = create_synthetic_gguf(version, &tensors);
        let tmp = write_temp_file(&data);
        let gguf =
            GgufFile::open(tmp.path()).unwrap_or_else(|e| panic!("{version:?} 打开失败: {e}"));
        let arch = gguf
            .architecture()
            .unwrap_or_else(|e| panic!("{version:?} architecture() 失败: {e}"));
        assert_eq!(arch, "qwen2", "{version:?} architecture 应为 qwen2");
    }
}
