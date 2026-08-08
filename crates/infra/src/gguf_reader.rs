//! GGUF 文件解析器（Phase 3：自研量化内核 + 内存层次流式加载）。
//!
//! 独立于 candle-core 的 GGUF 格式解析器，支持 mmap 零拷贝读取。
//! 解析 GGUF v1/v2/v3 格式，提取元数据和张量信息，并提供零拷贝张量数据访问。
//!
//! # GGUF 文件格式
//!
//! ```text
//! [4 bytes]  magic: 0x46554747 ("GGUF" 小端序)
//! [4 bytes]  version: u32 (1/2/3)
//! [4|8 bytes] tensor_count (V1=u32, V2/V3=u64)
//! [4|8 bytes] metadata_kv_count (V1=u32, V2/V3=u64)
//! [metadata_kv_count × KV]  元数据键值对
//! [tensor_count × TensorInfo]  张量元信息
//! [padding]  对齐填充（默认 32 字节）
//! [data]  张量权重数据
//! ```
//!
//! # 安全考量
//!
//! - 所有偏移量使用 `checked_add` 防溢出（铁律五）
//! - 文件大小校验：所有 offset+size 不超过文件总大小
//! - 字符串长度上限：防止恶意文件分配超大内存
//!
//! # 参考
//!
//! - GGUF 规范: <https://github.com/ggerganov/llama.cpp/blob/master/docs/gguf.md>
//! - candle-core 量化模块: `candle-core/src/quantized/gguf_file.rs`
//! - llama.cpp ggml: `ggml/src/ggml.c`

use std::collections::HashMap;
use std::fs::File;
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};

// ---------------------------------------------------------------------------
// 常量
// ---------------------------------------------------------------------------

/// GGUF 文件魔数（"GGUF" 的小端序 u32）。
const GGUF_MAGIC: u32 = 0x4655_4747;

/// GGUF 默认对齐字节数。
const GGUF_DEFAULT_ALIGNMENT: u64 = 32;

/// 字符串长度上限（1 MiB），防止恶意文件分配超大内存。
const MAX_STRING_LEN: usize = 1024 * 1024;

/// 数组元素数量上限（100 万），防止恶意文件分配超大内存。
const MAX_ARRAY_LEN: usize = 1_000_000;

/// 张量数量上限。
const MAX_TENSOR_COUNT: usize = 100_000;

/// 元数据键值对数量上限。
const MAX_METADATA_COUNT: usize = 100_000;

/// 张量维度数量上限。
const MAX_TENSOR_DIMS: usize = 8;

// ---------------------------------------------------------------------------
// GGUF 元数据值类型
// ---------------------------------------------------------------------------

/// GGUF 文件值类型（对应 GGUF 规范的元数据值类型）。
///
/// GGUF 元数据支持标量类型和数组类型，本枚举覆盖全部 13 种值类型。
#[derive(Debug, Clone)]
pub enum GgufValue {
    /// UTF-8 字符串
    String(String),
    /// 无符号 8 位整数
    Uint8(u8),
    /// 无符号 16 位整数
    Uint16(u16),
    /// 无符号 32 位整数
    Uint32(u32),
    /// 无符号 64 位整数
    Uint64(u64),
    /// 有符号 8 位整数
    Int8(i8),
    /// 有符号 16 位整数
    Int16(i16),
    /// 有符号 32 位整数
    Int32(i32),
    /// 有符号 64 位整数
    Int64(i64),
    /// 32 位浮点数
    Float32(f32),
    /// 64 位浮点数
    Float64(f64),
    /// 布尔值
    Bool(bool),
    /// 数组（同类型元素列表）
    Array(Vec<GgufValue>),
}

// ---------------------------------------------------------------------------
// GGML 量化数据类型
// ---------------------------------------------------------------------------

/// GGML 量化数据类型枚举（与 llama.cpp/candle 对齐）。
///
/// 每种类型对应一种量化方案，影响权重数据的内存布局和反量化算法。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GgmlDType {
    /// 32 位浮点数（无量化）
    F32,
    /// 16 位浮点数
    F16,
    /// Brain Float 16
    BF16,
    /// 4 位量化，块大小 32，每块 18 字节
    Q4_0,
    /// 4 位量化带偏移，块大小 32，每块 20 字节
    Q4_1,
    /// 5 位量化，块大小 32，每块 22 字节
    Q5_0,
    /// 5 位量化带偏移，块大小 32，每块 24 字节
    Q5_1,
    /// 8 位量化，块大小 32，每块 34 字节
    Q8_0,
    /// 8 位量化带偏移，块大小 32，每块 36 字节
    Q8_1,
    /// 2 位 K 量化，块大小 256，每块 84 字节
    Q2K,
    /// 3 位 K 量化，块大小 256，每块 110 字节
    Q3K,
    /// 4 位 K 量化，块大小 256，每块 144 字节
    Q4K,
    /// 5 位 K 量化，块大小 256，每块 176 字节
    Q5K,
    /// 6 位 K 量化，块大小 256，每块 210 字节
    Q6K,
    /// 8 位 K 量化，块大小 256，每块 292 字节
    Q8K,
    /// 未知类型（保留原始 u32 值）
    Unknown(u32),
}

impl GgmlDType {
    /// 从 GGUF 规范的 u32 类型值转换为 `GgmlDType`。
    ///
    /// # 参数
    ///
    /// - `v`：GGUF 文件中存储的量化类型 u32 值
    ///
    /// # 返回
    ///
    /// 对应的 `GgmlDType` 枚举变体；未识别的值返回 `Unknown(v)`。
    pub fn from_value(v: u32) -> Self {
        match v {
            0 => Self::F32,
            1 => Self::F16,
            2 => Self::Q4_0,
            3 => Self::Q4_1,
            6 => Self::Q5_0,
            7 => Self::Q5_1,
            8 => Self::Q8_0,
            9 => Self::Q8_1,
            10 => Self::Q2K,
            11 => Self::Q3K,
            12 => Self::Q4K,
            13 => Self::Q5K,
            14 => Self::Q6K,
            15 => Self::Q8K,
            24 => Self::BF16,
            other => Self::Unknown(other),
        }
    }

    /// 返回量化类型的块布局信息 `(block_size, block_bytes)`。
    ///
    /// - `block_size = 0`：非块类型（F32/F16/BF16），`block_bytes` 为每元素字节数。
    /// - `block_size > 0`：块类型，`block_bytes` 为每块字节数。
    pub fn block_layout(self) -> (u64, u64) {
        match self {
            Self::F32 => (0, 4),
            Self::F16 => (0, 2),
            Self::BF16 => (0, 2),
            Self::Q4_0 => (32, 18),
            Self::Q4_1 => (32, 20),
            Self::Q5_0 => (32, 22),
            Self::Q5_1 => (32, 24),
            Self::Q8_0 => (32, 34),
            Self::Q8_1 => (32, 36),
            Self::Q2K => (256, 84),
            Self::Q3K => (256, 110),
            Self::Q4K => (256, 144),
            Self::Q5K => (256, 176),
            Self::Q6K => (256, 210),
            Self::Q8K => (256, 292),
            Self::Unknown(_) => (0, 0),
        }
    }

    /// 根据元素数量计算张量数据的字节大小。
    ///
    /// # 参数
    ///
    /// - `n_elements`：张量中的元素总数
    ///
    /// # 返回
    ///
    /// 张量数据的字节大小。对于未知类型返回 0。
    ///
    /// # 错误
    ///
    /// - 块类型且元素数非块大小整数倍时返回错误
    /// - 计算溢出时返回错误
    pub fn byte_size(self, n_elements: u64) -> Result<u64> {
        let (block_size, block_bytes) = self.block_layout();
        if block_size == 0 {
            // 非块类型：元素数 × 每元素字节数
            n_elements
                .checked_mul(block_bytes)
                .context("tensor byte size overflow (non-block type)")
        } else {
            // 块类型：块数 × 每块字节数
            ensure!(
                n_elements.is_multiple_of(block_size),
                "element count {} not divisible by block size {} for {:?}",
                n_elements,
                block_size,
                self
            );
            let n_blocks = n_elements / block_size;
            n_blocks
                .checked_mul(block_bytes)
                .context("tensor byte size overflow (block type)")
        }
    }
}

// ---------------------------------------------------------------------------
// 张量元信息
// ---------------------------------------------------------------------------

/// 张量元信息。
///
/// 描述 GGUF 文件中一个张量的名称、维度、量化类型和数据位置。
#[derive(Debug, Clone)]
pub struct TensorInfo {
    /// 张量名称（如 `token_embd.weight`）
    pub name: String,
    /// 维度列表（倒序，GGUF 使用 col-major）
    pub dims: Vec<u64>,
    /// 量化数据类型（GGML 量化格式枚举）
    pub dtype: GgmlDType,
    /// 数据在文件中的偏移量（相对于数据区起始）
    pub offset: u64,
    /// 数据长度（字节）
    pub size: u64,
}

impl TensorInfo {
    /// 返回张量的元素总数（各维度乘积）。
    pub fn n_elements(&self) -> u64 {
        self.dims.iter().product()
    }
}

// ---------------------------------------------------------------------------
// 字节游标（安全的小端序读取器）
// ---------------------------------------------------------------------------

/// 字节游标：从 `&[u8]` 中安全地读取小端序值。
///
/// 所有读取方法执行边界检查和溢出检查，返回 `Result`。
/// 读取失败时游标位置不变（实际上会前进，因为确保了不变量后才会前进）。
struct ByteCursor<'a> {
    /// 底层字节切片
    data: &'a [u8],
    /// 当前读取位置
    pos: usize,
}

impl<'a> ByteCursor<'a> {
    /// 从字节切片创建游标，位置初始化为 0。
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    /// 返回当前读取位置。
    fn position(&self) -> usize {
        self.pos
    }

    /// 读取 `n` 个字节，返回切片引用并推进游标。
    ///
    /// # 错误
    ///
    /// 位置 + 长度溢出或超出数据范围时返回错误。
    fn read_bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(n)
            .context("offset overflow in read_bytes")?;
        ensure!(end <= self.data.len(), "unexpected end of file");
        let slice = &self.data[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    /// 读取 1 个无符号字节。
    fn read_u8(&mut self) -> Result<u8> {
        Ok(self.read_bytes(1)?[0])
    }

    /// 读取小端序 u16。
    fn read_u16(&mut self) -> Result<u16> {
        let b = self.read_bytes(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    /// 读取小端序 u32。
    fn read_u32(&mut self) -> Result<u32> {
        let b = self.read_bytes(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// 读取小端序 u64。
    fn read_u64(&mut self) -> Result<u64> {
        let b = self.read_bytes(8)?;
        Ok(u64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }

    /// 读取有符号字节。
    fn read_i8(&mut self) -> Result<i8> {
        Ok(self.read_u8()? as i8)
    }

    /// 读取小端序 i16。
    fn read_i16(&mut self) -> Result<i16> {
        Ok(self.read_u16()? as i16)
    }

    /// 读取小端序 i32。
    fn read_i32(&mut self) -> Result<i32> {
        Ok(self.read_u32()? as i32)
    }

    /// 读取小端序 i64。
    fn read_i64(&mut self) -> Result<i64> {
        Ok(self.read_u64()? as i64)
    }

    /// 读取 f32（小端序 4 字节）。
    fn read_f32(&mut self) -> Result<f32> {
        let val = self.read_u32()?;
        Ok(f32::from_bits(val))
    }

    /// 读取 f64（小端序 8 字节）。
    fn read_f64(&mut self) -> Result<f64> {
        let val = self.read_u64()?;
        Ok(f64::from_bits(val))
    }

    /// 读取布尔值（1 字节，非零为 true）。
    fn read_bool(&mut self) -> Result<bool> {
        Ok(self.read_u8()? != 0)
    }

    /// 读取 GGUF 字符串。
    ///
    /// # 参数
    ///
    /// - `use_u64_len`：V2/V3 使用 u64 长度前缀，V1 使用 u32
    ///
    /// # 错误
    ///
    /// 字符串长度超过 `MAX_STRING_LEN` 或数据不足时返回错误。
    fn read_gguf_string(&mut self, use_u64_len: bool) -> Result<String> {
        let len = if use_u64_len {
            self.read_u64()? as usize
        } else {
            self.read_u32()? as usize
        };
        // 防止恶意文件分配超大内存（铁律五：防御性编程）
        ensure!(
            len <= MAX_STRING_LEN,
            "GGUF string length too large: {} bytes (max {})",
            len,
            MAX_STRING_LEN
        );
        let bytes = self.read_bytes(len)?;
        Ok(String::from_utf8_lossy(bytes).into_owned())
    }
}

// ---------------------------------------------------------------------------
// GGUF 元数据解析
// ---------------------------------------------------------------------------

/// GGUF 元数据值类型枚举（u32 编码，对应 GGUF 规范）。
mod value_type {
    pub const UINT8: u32 = 0;
    pub const INT8: u32 = 1;
    pub const UINT16: u32 = 2;
    pub const INT16: u32 = 3;
    pub const UINT32: u32 = 4;
    pub const INT32: u32 = 5;
    pub const FLOAT32: u32 = 6;
    pub const BOOL: u32 = 7;
    pub const STRING: u32 = 8;
    pub const ARRAY: u32 = 9;
    pub const UINT64: u32 = 10;
    pub const INT64: u32 = 11;
    pub const FLOAT64: u32 = 12;
}

/// 解析单个元数据值。
///
/// # 参数
///
/// - `cursor`：字节游标
/// - `vtype`：值类型 u32 编码
/// - `use_u64_len`：V2/V3 使用 u64 长度/计数前缀
fn parse_value(cursor: &mut ByteCursor, vtype: u32, use_u64_len: bool) -> Result<GgufValue> {
    match vtype {
        value_type::UINT8 => Ok(GgufValue::Uint8(cursor.read_u8()?)),
        value_type::INT8 => Ok(GgufValue::Int8(cursor.read_i8()?)),
        value_type::UINT16 => Ok(GgufValue::Uint16(cursor.read_u16()?)),
        value_type::INT16 => Ok(GgufValue::Int16(cursor.read_i16()?)),
        value_type::UINT32 => Ok(GgufValue::Uint32(cursor.read_u32()?)),
        value_type::INT32 => Ok(GgufValue::Int32(cursor.read_i32()?)),
        value_type::FLOAT32 => Ok(GgufValue::Float32(cursor.read_f32()?)),
        value_type::BOOL => Ok(GgufValue::Bool(cursor.read_bool()?)),
        value_type::STRING => Ok(GgufValue::String(cursor.read_gguf_string(use_u64_len)?)),
        value_type::ARRAY => {
            // 数组：元素类型 + 元素数量 + 元素列表
            let elem_type = cursor.read_u32()?;
            let count = if use_u64_len {
                cursor.read_u64()? as usize
            } else {
                cursor.read_u32()? as usize
            };
            ensure!(
                count <= MAX_ARRAY_LEN,
                "GGUF array length too large: {} (max {})",
                count,
                MAX_ARRAY_LEN
            );
            let mut items = Vec::with_capacity(count);
            for _ in 0..count {
                items.push(parse_value(cursor, elem_type, use_u64_len)?);
            }
            Ok(GgufValue::Array(items))
        }
        value_type::UINT64 => Ok(GgufValue::Uint64(cursor.read_u64()?)),
        value_type::INT64 => Ok(GgufValue::Int64(cursor.read_i64()?)),
        value_type::FLOAT64 => Ok(GgufValue::Float64(cursor.read_f64()?)),
        _ => bail!("unknown GGUF metadata value type: {}", vtype),
    }
}

/// 解析单个元数据键值对。
///
/// 返回 `(key, value)` 元组。
fn parse_metadata_kv(cursor: &mut ByteCursor, use_u64_len: bool) -> Result<(String, GgufValue)> {
    let key = cursor.read_gguf_string(use_u64_len)?;
    let vtype = cursor.read_u32()?;
    let value = parse_value(cursor, vtype, use_u64_len)?;
    Ok((key, value))
}

/// 解析单个张量元信息。
fn parse_tensor_info(cursor: &mut ByteCursor, use_u64_len: bool) -> Result<TensorInfo> {
    let name = cursor.read_gguf_string(use_u64_len)?;
    let n_dims = cursor.read_u32()? as usize;
    ensure!(
        n_dims <= MAX_TENSOR_DIMS,
        "tensor dims count too large: {} (max {})",
        n_dims,
        MAX_TENSOR_DIMS
    );
    let mut dims = Vec::with_capacity(n_dims);
    for _ in 0..n_dims {
        dims.push(cursor.read_u64()?);
    }
    let dtype_val = cursor.read_u32()?;
    let dtype = GgmlDType::from_value(dtype_val);
    let offset = cursor.read_u64()?;
    let n_elements: u64 = dims.iter().product();
    let size = dtype.byte_size(n_elements)?;
    Ok(TensorInfo {
        name,
        dims,
        dtype,
        offset,
        size,
    })
}

// ---------------------------------------------------------------------------
// 对齐计算
// ---------------------------------------------------------------------------

/// 将 `value` 向上对齐到 `alignment` 的倍数。
///
/// # 错误
///
/// `alignment` 为 0 或对齐后值溢出时返回错误。
fn align_up(value: u64, alignment: u64) -> Result<u64> {
    ensure!(alignment > 0, "alignment must be positive");
    let remainder = value % alignment;
    if remainder == 0 {
        Ok(value)
    } else {
        value
            .checked_add(alignment - remainder)
            .context("alignment calculation overflow")
    }
}

// ---------------------------------------------------------------------------
// GgufFile
// ---------------------------------------------------------------------------

/// mmap 映射的 GGUF 文件。
///
/// 通过内存映射实现零拷贝读取：张量数据直接引用 mmap 区域的字节切片，
/// 无需将权重数据拷贝到堆内存。OS 按需分页（page fault 时从磁盘加载）。
///
/// # 生命周期
///
/// `GgufFile` 持有 mmap 映射，所有 `tensor_data()` 返回的切片在
/// `GgufFile` 存活期间有效。调用 `close()` 或 drop 后映射释放。
pub struct GgufFile {
    /// mmap 映射区域
    mmap: memmap2::Mmap,
    /// 元数据键值对
    metadata: HashMap<String, GgufValue>,
    /// 张量信息列表（按文件中的顺序）
    tensors: Vec<TensorInfo>,
    /// 张量名 → 索引映射（快速查找）
    tensor_map: HashMap<String, usize>,
    /// 数据区起始偏移（张量数据相对于文件头的偏移）
    data_offset: u64,
}

impl std::fmt::Debug for GgufFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GgufFile")
            .field("metadata", &self.metadata)
            .field("tensor_count", &self.tensors.len())
            .field("data_offset", &self.data_offset)
            .finish()
    }
}

impl GgufFile {
    /// 打开 GGUF 文件，mmap 映射，解析文件头。
    ///
    /// # 参数
    ///
    /// - `path`：GGUF 文件路径
    ///
    /// # 返回
    ///
    /// 解析完成的 `GgufFile`，可直接访问元数据和张量数据。
    ///
    /// # 错误
    ///
    /// - 文件不存在或无法打开
    /// - mmap 映射失败
    /// - 魔数不匹配（非 GGUF 文件）
    /// - 版本不支持
    /// - 元数据/张量信息解析失败
    /// - 张量数据超出文件范围
    pub fn open(path: &Path) -> Result<Self> {
        let file = File::open(path)
            .with_context(|| format!("failed to open GGUF file: {}", path.display()))?;
        let mmap = unsafe { memmap2::Mmap::map(&file) }.context("failed to mmap GGUF file")?;

        let data = &mmap[..];
        let mut cursor = ByteCursor::new(data);

        // 解析魔数
        let magic = cursor.read_u32()?;
        ensure!(
            magic == GGUF_MAGIC,
            "invalid GGUF magic: 0x{:08X} (expected 0x{:08X})",
            magic,
            GGUF_MAGIC
        );

        // 解析版本
        let version = cursor.read_u32()?;
        ensure!(
            (1..=3).contains(&version),
            "unsupported GGUF version: {} (supported: 1, 2, 3)",
            version
        );
        let use_u64_len = version >= 2;

        // 解析张量数量和元数据数量
        let tensor_count = if use_u64_len {
            cursor.read_u64()? as usize
        } else {
            cursor.read_u32()? as usize
        };
        let metadata_count = if use_u64_len {
            cursor.read_u64()? as usize
        } else {
            cursor.read_u32()? as usize
        };

        // 防御性编程：防止恶意文件分配超大内存
        ensure!(
            tensor_count <= MAX_TENSOR_COUNT,
            "tensor count too large: {} (max {})",
            tensor_count,
            MAX_TENSOR_COUNT
        );
        ensure!(
            metadata_count <= MAX_METADATA_COUNT,
            "metadata count too large: {} (max {})",
            metadata_count,
            MAX_METADATA_COUNT
        );

        // 解析元数据
        let mut metadata = HashMap::with_capacity(metadata_count);
        for _ in 0..metadata_count {
            let (key, value) = parse_metadata_kv(&mut cursor, use_u64_len)?;
            metadata.insert(key, value);
        }

        // 解析张量信息
        let mut tensors = Vec::with_capacity(tensor_count);
        let mut tensor_map = HashMap::with_capacity(tensor_count);
        for i in 0..tensor_count {
            let info = parse_tensor_info(&mut cursor, use_u64_len)?;
            tensor_map.insert(info.name.clone(), i);
            tensors.push(info);
        }

        // 计算数据区起始偏移（对齐填充）
        let header_end = cursor.position() as u64;
        let alignment = match metadata.get("general.alignment") {
            Some(GgufValue::Uint32(v)) => *v as u64,
            Some(GgufValue::Uint64(v)) => *v,
            _ => GGUF_DEFAULT_ALIGNMENT,
        };
        let data_offset = align_up(header_end, alignment)?;

        // 校验张量数据在文件范围内
        let file_size = data.len() as u64;
        for t in &tensors {
            let tensor_end = data_offset
                .checked_add(t.offset)
                .and_then(|o| o.checked_add(t.size))
                .context("tensor offset/size overflow")?;
            ensure!(
                tensor_end <= file_size,
                "tensor '{}' data extends beyond file end \
                 (data_offset={}, offset={}, size={}, file_size={})",
                t.name,
                data_offset,
                t.offset,
                t.size,
                file_size
            );
        }

        Ok(Self {
            mmap,
            metadata,
            tensors,
            tensor_map,
            data_offset,
        })
    }

    /// 返回所有元数据键值对的引用。
    pub fn metadata(&self) -> &HashMap<String, GgufValue> {
        &self.metadata
    }

    /// 返回张量数量。
    pub fn tensor_count(&self) -> usize {
        self.tensors.len()
    }

    /// 返回指定张量的元信息。
    ///
    /// # 参数
    ///
    /// - `name`：张量名称（如 `token_embd.weight`）
    ///
    /// # 返回
    ///
    /// 存在时返回 `Some(&TensorInfo)`，不存在返回 `None`。
    pub fn tensor_info(&self, name: &str) -> Option<&TensorInfo> {
        let &idx = self.tensor_map.get(name)?;
        self.tensors.get(idx)
    }

    /// 返回张量原始数据（零拷贝 mmap 切片）。
    ///
    /// 返回的 `&[u8]` 直接指向 mmap 区域，无数据拷贝。
    /// OS 按需分页（page fault 时从磁盘加载）。
    ///
    /// # 参数
    ///
    /// - `name`：张量名称
    ///
    /// # 返回
    ///
    /// 存在时返回 `Some(&[u8])`，切片长度等于 `TensorInfo.size`。
    /// 不存在返回 `None`。
    pub fn tensor_data(&self, name: &str) -> Option<&[u8]> {
        let &idx = self.tensor_map.get(name)?;
        let info = self.tensors.get(idx)?;
        let start = self.data_offset.checked_add(info.offset)? as usize;
        let end = start.checked_add(info.size as usize)?;
        if end > self.mmap.len() {
            return None;
        }
        Some(&self.mmap[start..end])
    }

    /// 返回所有张量名的迭代器。
    ///
    /// 迭代顺序与文件中的张量顺序一致。
    pub fn tensor_names(&self) -> impl Iterator<Item = &str> {
        self.tensors.iter().map(|t| t.name.as_str())
    }

    /// 从元数据提取模型架构名。
    ///
    /// 查找 `general.architecture` 元数据键，返回其字符串值。
    ///
    /// # 错误
    ///
    /// - 元数据中不存在 `general.architecture` 键
    /// - 值类型不是字符串
    pub fn architecture(&self) -> Result<&str> {
        let val = self
            .metadata
            .get("general.architecture")
            .context("metadata missing 'general.architecture'")?;
        match val {
            GgufValue::String(s) => Ok(s.as_str()),
            _ => bail!("'general.architecture' metadata value is not a string"),
        }
    }

    /// 返回内部 mmap 区域的只读引用。
    ///
    /// 用于 `LayerPrefetcher` 等需要直接操作 mmap 区域的场景。
    /// 调用方需确保不修改 mmap 数据（mmap 以只读方式映射）。
    pub fn mmap(&self) -> &memmap2::Mmap {
        &self.mmap
    }

    /// 返回数据区起始偏移量。
    pub fn data_offset(&self) -> u64 {
        self.data_offset
    }

    /// 释放 mmap 映射。
    ///
    /// 消费 `self`，释放底层 mmap 映射。
    /// 调用后所有从 `tensor_data()` 获取的切片失效。
    pub fn close(self) {
        // GgufFile 被 drop 时，mmap 自动释放
        drop(self.mmap);
    }
}
