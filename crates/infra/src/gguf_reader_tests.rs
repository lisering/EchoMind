#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented,
    clippy::manual_checked_ops
)]

//! GGUF 文件解析器单元测试（Phase 3 Session 16）。
//!
//! 测试覆盖：文件打开、魔数校验、元数据解析（字符串/u32/数组）、
//! 张量信息查询、零拷贝数据访问、架构提取、量化类型枚举。
//!
//! 大部分测试使用合成的最小 GGUF 文件，无需真实模型文件。
//! 需要真实模型的测试标记为 `#[ignore]`。

use super::gguf_reader::*;
use std::io::Write;
use tempfile::NamedTempFile;

// ---------------------------------------------------------------------------
// 辅助函数：合成 GGUF 文件生成
// ---------------------------------------------------------------------------

/// 创建最小化的 GGUF v3 测试文件，包含标准元数据和 1 个 F32 张量。
///
/// 文件结构：
/// - Magic + Version 3
/// - 3 个元数据：general.architecture="qwen2", general.name="test_model", general.file_type=1
/// - 1 个张量：token_embd.weight, dims=[4,4], dtype=F32
/// - 32 字节对齐填充
/// - 64 字节张量数据（4×4×4=64，填充 0xAB）
fn create_minimal_gguf_v3() -> Vec<u8> {
    create_gguf_v3_with_tensor("token_embd.weight", &[4, 4], 0, 0xAB)
}

/// 创建 GGUF v3 文件，包含标准元数据和 1 个 F32 张量。
///
/// # 参数
///
/// - `tensor_name`：张量名称
/// - `dims`：张量维度
/// - `dtype_val`：GGML dtype u32 值（0=F32, 8=Q8_0, 等）
/// - `fill_byte`：张量数据填充字节
fn create_gguf_v3_with_tensor(
    tensor_name: &str,
    dims: &[u64],
    dtype_val: u32,
    fill_byte: u8,
) -> Vec<u8> {
    let mut buf = Vec::new();

    // ---- 文件头 ----
    // Magic
    buf.extend_from_slice(&0x4655_4747u32.to_le_bytes());
    // Version: 3
    buf.extend_from_slice(&3u32.to_le_bytes());
    // tensor_count: 1
    buf.extend_from_slice(&1u64.to_le_bytes());
    // metadata_kv_count: 3
    buf.extend_from_slice(&3u64.to_le_bytes());

    // ---- 元数据 ----
    // 1. general.architecture = "qwen2"
    write_gguf_str_v3(&mut buf, "general.architecture");
    buf.extend_from_slice(&8u32.to_le_bytes()); // STRING type
    write_gguf_str_v3(&mut buf, "qwen2");

    // 2. general.name = "test_model"
    write_gguf_str_v3(&mut buf, "general.name");
    buf.extend_from_slice(&8u32.to_le_bytes());
    write_gguf_str_v3(&mut buf, "test_model");

    // 3. general.file_type = 1 (uint32)
    write_gguf_str_v3(&mut buf, "general.file_type");
    buf.extend_from_slice(&4u32.to_le_bytes()); // UINT32 type
    buf.extend_from_slice(&1u32.to_le_bytes());

    // ---- 张量信息 ----
    write_gguf_str_v3(&mut buf, tensor_name);
    buf.extend_from_slice(&(dims.len() as u32).to_le_bytes()); // n_dims
    for &d in dims {
        buf.extend_from_slice(&d.to_le_bytes());
    }
    buf.extend_from_slice(&dtype_val.to_le_bytes()); // dtype
    buf.extend_from_slice(&0u64.to_le_bytes()); // offset = 0

    // ---- 对齐填充到 32 字节 ----
    let header_end = buf.len();
    let aligned = header_end.div_ceil(32) * 32;
    buf.resize(aligned, 0);

    // ---- 张量数据 ----
    // 计算数据大小
    let n_elements: u64 = dims.iter().product();
    let data_size = if dtype_val == 0 {
        // F32: 4 bytes per element
        n_elements as usize * 4
    } else {
        // 其他类型：使用 block_layout 计算
        let dtype = GgmlDType::from_value(dtype_val);
        let (block_size, block_bytes) = dtype.block_layout();
        if block_size == 0 {
            n_elements as usize * block_bytes as usize
        } else {
            let n_blocks = n_elements / block_size;
            (n_blocks * block_bytes) as usize
        }
    };
    buf.resize(buf.len() + data_size, fill_byte);

    buf
}

/// 写入 GGUF v3 字符串（u64 长度前缀 + UTF-8 数据）。
fn write_gguf_str_v3(buf: &mut Vec<u8>, s: &str) {
    buf.extend_from_slice(&(s.len() as u64).to_le_bytes());
    buf.extend_from_slice(s.as_bytes());
}

/// 创建包含 u32 数组元数据的 GGUF v3 文件。
fn create_gguf_v3_with_array_meta(key: &str, vals: &[u32]) -> Vec<u8> {
    let mut buf = Vec::new();

    // 文件头
    buf.extend_from_slice(&0x4655_4747u32.to_le_bytes());
    buf.extend_from_slice(&3u32.to_le_bytes());
    buf.extend_from_slice(&1u64.to_le_bytes()); // tensor_count
    buf.extend_from_slice(&1u64.to_le_bytes()); // metadata_kv_count = 1

    // 元数据: 数组类型
    write_gguf_str_v3(&mut buf, key);
    buf.extend_from_slice(&9u32.to_le_bytes()); // ARRAY type
    buf.extend_from_slice(&4u32.to_le_bytes()); // 元素类型: UINT32
    buf.extend_from_slice(&(vals.len() as u64).to_le_bytes());
    for &v in vals {
        buf.extend_from_slice(&v.to_le_bytes());
    }

    // 张量信息: dummy tensor
    write_gguf_str_v3(&mut buf, "token_embd.weight");
    buf.extend_from_slice(&1u32.to_le_bytes()); // n_dims = 1
    buf.extend_from_slice(&4u64.to_le_bytes()); // dim = 4
    buf.extend_from_slice(&0u32.to_le_bytes()); // dtype = F32
    buf.extend_from_slice(&0u64.to_le_bytes()); // offset = 0

    // 对齐 + 数据
    let header_end = buf.len();
    let aligned = header_end.div_ceil(32) * 32;
    buf.resize(aligned, 0);
    buf.resize(buf.len() + 16, 0xCD); // 4 elements * 4 bytes = 16 bytes

    buf
}

/// 将字节写入临时文件并返回文件路径。
fn write_temp_file(data: &[u8]) -> NamedTempFile {
    let mut tmp = tempfile::NamedTempFile::new().expect("创建临时文件失败");
    tmp.write_all(data).expect("写入临时文件失败");
    tmp
}

// ---------------------------------------------------------------------------
// TC-GGUF-001 ~ TC-GGUF-014
// ---------------------------------------------------------------------------

/// TC-GGUF-001：打开合法 GGUF 文件返回 Ok。
#[test]
fn test_open_valid_gguf() {
    let data = create_minimal_gguf_v3();
    let tmp = write_temp_file(&data);
    let result = GgufFile::open(tmp.path());
    assert!(
        result.is_ok(),
        "打开合法 GGUF 文件应成功: {:?}",
        result.err()
    );
}

/// TC-GGUF-002：不存在的文件返回 Err。
#[test]
fn test_open_nonexistent_fails() {
    let path = std::path::PathBuf::from("/nonexistent/path/model.gguf");
    let result = GgufFile::open(&path);
    assert!(result.is_err(), "不存在的文件应返回错误");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("failed to open") || err.contains("No such file"),
        "错误信息应包含打开失败原因: {}",
        err
    );
}

/// TC-GGUF-003：非 GGUF 文件（magic 不匹配）返回 Err。
#[test]
fn test_open_invalid_magic_fails() {
    let tmp = write_temp_file(b"NOTGGUF12345678901234567890");
    let result = GgufFile::open(tmp.path());
    assert!(result.is_err(), "魔数不匹配应返回错误");
    let err = result.unwrap_err().to_string();
    assert!(err.contains("magic"), "错误信息应提及 magic: {}", err);
}

/// TC-GGUF-004：元数据包含 `general.architecture` 键。
#[test]
fn test_metadata_contains_architecture() {
    let data = create_minimal_gguf_v3();
    let tmp = write_temp_file(&data);
    let gguf = GgufFile::open(tmp.path()).expect("打开 GGUF 文件失败");

    let meta = gguf.metadata();
    assert!(
        meta.contains_key("general.architecture"),
        "元数据应包含 general.architecture 键"
    );
}

/// TC-GGUF-005：字符串值正确解析。
#[test]
fn test_metadata_string_value() {
    let data = create_minimal_gguf_v3();
    let tmp = write_temp_file(&data);
    let gguf = GgufFile::open(tmp.path()).expect("打开 GGUF 文件失败");

    let meta = gguf.metadata();
    let name = meta.get("general.name").expect("应存在 general.name");
    match name {
        GgufValue::String(s) => assert_eq!(s, "test_model", "字符串值应正确解析"),
        other => panic!("general.name 应为 String 类型，实际: {:?}", other),
    }
}

/// TC-GGUF-006：uint32 值正确解析。
#[test]
fn test_metadata_uint32_value() {
    let data = create_minimal_gguf_v3();
    let tmp = write_temp_file(&data);
    let gguf = GgufFile::open(tmp.path()).expect("打开 GGUF 文件失败");

    let meta = gguf.metadata();
    let file_type = meta
        .get("general.file_type")
        .expect("应存在 general.file_type");
    match file_type {
        GgufValue::Uint32(v) => assert_eq!(*v, 1, "uint32 值应正确解析"),
        other => panic!("general.file_type 应为 Uint32 类型，实际: {:?}", other),
    }
}

/// TC-GGUF-007：数组值正确解析。
#[test]
fn test_metadata_array_value() {
    let vals = vec![10u32, 20, 30, 40];
    let data = create_gguf_v3_with_array_meta("tokenizer.ggml.tokens.count_list", &vals);
    let tmp = write_temp_file(&data);
    let gguf = GgufFile::open(tmp.path()).expect("打开 GGUF 文件失败");

    let meta = gguf.metadata();
    let arr = meta
        .get("tokenizer.ggml.tokens.count_list")
        .expect("应存在数组元数据");
    match arr {
        GgufValue::Array(items) => {
            assert_eq!(items.len(), 4, "数组应有 4 个元素");
            match &items[0] {
                GgufValue::Uint32(v) => assert_eq!(*v, 10, "第一个元素应为 10"),
                other => panic!("数组元素应为 Uint32，实际: {:?}", other),
            }
            match &items[3] {
                GgufValue::Uint32(v) => assert_eq!(*v, 40, "最后一个元素应为 40"),
                other => panic!("数组元素应为 Uint32，实际: {:?}", other),
            }
        }
        other => panic!("应为 Array 类型，实际: {:?}", other),
    }
}

/// TC-GGUF-008：张量数量 > 0。
#[test]
fn test_tensor_count_positive() {
    let data = create_minimal_gguf_v3();
    let tmp = write_temp_file(&data);
    let gguf = GgufFile::open(tmp.path()).expect("打开 GGUF 文件失败");

    assert!(gguf.tensor_count() > 0, "张量数量应大于 0");
    assert_eq!(gguf.tensor_count(), 1, "测试文件应有 1 个张量");
}

/// TC-GGUF-009：tensor_info 返回正确的 dims/dtype/offset。
#[test]
fn test_tensor_info_returns_correct_data() {
    let data = create_minimal_gguf_v3();
    let tmp = write_temp_file(&data);
    let gguf = GgufFile::open(tmp.path()).expect("打开 GGUF 文件失败");

    let info = gguf
        .tensor_info("token_embd.weight")
        .expect("应存在 token_embd.weight 张量");

    assert_eq!(info.name, "token_embd.weight", "张量名应正确");
    assert_eq!(info.dims, vec![4, 4], "维度应为 [4, 4]");
    assert_eq!(info.dtype, GgmlDType::F32, "数据类型应为 F32");
    assert_eq!(info.offset, 0, "偏移量应为 0");
    assert_eq!(info.size, 64, "数据大小应为 64 字节 (4×4×4)");
}

/// TC-GGUF-010：不存在的张量名返回 None。
#[test]
fn test_tensor_info_nonexistent_returns_none() {
    let data = create_minimal_gguf_v3();
    let tmp = write_temp_file(&data);
    let gguf = GgufFile::open(tmp.path()).expect("打开 GGUF 文件失败");

    assert!(
        gguf.tensor_info("nonexistent.weight").is_none(),
        "不存在的张量应返回 None"
    );
}

/// TC-GGUF-011：tensor_data 返回的切片长度与 TensorInfo.size 一致。
#[test]
fn test_tensor_data_zero_copy() {
    let data = create_minimal_gguf_v3();
    let tmp = write_temp_file(&data);
    let gguf = GgufFile::open(tmp.path()).expect("打开 GGUF 文件失败");

    let info = gguf
        .tensor_info("token_embd.weight")
        .expect("应存在 token_embd.weight 张量");
    let tensor_data = gguf
        .tensor_data("token_embd.weight")
        .expect("应返回张量数据");

    assert_eq!(
        tensor_data.len(),
        info.size as usize,
        "tensor_data 长度应与 TensorInfo.size 一致"
    );
    assert_eq!(tensor_data.len(), 64, "F32 [4,4] 张量数据应为 64 字节");
    // 验证数据内容（填充字节 0xAB）
    assert!(
        tensor_data.iter().all(|&b| b == 0xAB),
        "张量数据应全部为填充字节 0xAB"
    );
}

/// TC-GGUF-012：张量名列表包含 `token_embd.weight`。
#[test]
fn test_tensor_names_contains_token_embd() {
    let data = create_minimal_gguf_v3();
    let tmp = write_temp_file(&data);
    let gguf = GgufFile::open(tmp.path()).expect("打开 GGUF 文件失败");

    let names: Vec<&str> = gguf.tensor_names().collect();
    assert!(
        names.contains(&"token_embd.weight"),
        "张量名列表应包含 token_embd.weight"
    );
}

/// TC-GGUF-013：Qwen2.5 模型返回 `"qwen2"`。
#[test]
fn test_architecture_returns_qwen() {
    let data = create_minimal_gguf_v3();
    let tmp = write_temp_file(&data);
    let gguf = GgufFile::open(tmp.path()).expect("打开 GGUF 文件失败");

    let arch = gguf.architecture().expect("应成功提取架构名");
    assert_eq!(arch, "qwen2", "架构名应为 qwen2");
}

/// TC-GGUF-014：GgmlDType 枚举值与 GGUF 规范一致。
#[test]
fn test_ggml_dtype_from_value() {
    // 验证所有已知类型的 u32 值映射
    assert_eq!(GgmlDType::from_value(0), GgmlDType::F32);
    assert_eq!(GgmlDType::from_value(1), GgmlDType::F16);
    assert_eq!(GgmlDType::from_value(2), GgmlDType::Q4_0);
    assert_eq!(GgmlDType::from_value(3), GgmlDType::Q4_1);
    assert_eq!(GgmlDType::from_value(6), GgmlDType::Q5_0);
    assert_eq!(GgmlDType::from_value(7), GgmlDType::Q5_1);
    assert_eq!(GgmlDType::from_value(8), GgmlDType::Q8_0);
    assert_eq!(GgmlDType::from_value(9), GgmlDType::Q8_1);
    assert_eq!(GgmlDType::from_value(10), GgmlDType::Q2K);
    assert_eq!(GgmlDType::from_value(11), GgmlDType::Q3K);
    assert_eq!(GgmlDType::from_value(12), GgmlDType::Q4K);
    assert_eq!(GgmlDType::from_value(13), GgmlDType::Q5K);
    assert_eq!(GgmlDType::from_value(14), GgmlDType::Q6K);
    assert_eq!(GgmlDType::from_value(15), GgmlDType::Q8K);
    assert_eq!(GgmlDType::from_value(24), GgmlDType::BF16);

    // 验证未知类型
    assert_eq!(GgmlDType::from_value(99), GgmlDType::Unknown(99));
    assert_eq!(GgmlDType::from_value(255), GgmlDType::Unknown(255));

    // 验证 block_layout 返回值
    let (bs, bb) = GgmlDType::Q8_0.block_layout();
    assert_eq!(bs, 32, "Q8_0 块大小应为 32");
    assert_eq!(bb, 34, "Q8_0 每块字节数应为 34");

    let (bs, bb) = GgmlDType::Q4K.block_layout();
    assert_eq!(bs, 256, "Q4K 块大小应为 256");
    assert_eq!(bb, 144, "Q4K 每块字节数应为 144");

    let (bs, bb) = GgmlDType::F32.block_layout();
    assert_eq!(bs, 0, "F32 非块类型，block_size 应为 0");
    assert_eq!(bb, 4, "F32 每元素 4 字节");

    // 验证 byte_size 计算
    assert_eq!(
        GgmlDType::F32.byte_size(100).unwrap(),
        400,
        "F32 100 元素 = 400 字节"
    );
    assert_eq!(
        GgmlDType::Q8_0.byte_size(32).unwrap(),
        34,
        "Q8_0 32 元素 = 1 块 = 34 字节"
    );
    assert_eq!(
        GgmlDType::Q4K.byte_size(256).unwrap(),
        144,
        "Q4K 256 元素 = 1 块 = 144 字节"
    );
    assert_eq!(
        GgmlDType::Q4K.byte_size(512).unwrap(),
        288,
        "Q4K 512 元素 = 2 块 = 288 字节"
    );
}
