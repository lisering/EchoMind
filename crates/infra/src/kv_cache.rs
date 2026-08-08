//! KV cache 序列化/反序列化（Phase 4 Session 24）。
//!
//! 设计磁盘格式用于保存/恢复 transformer 模型的 Key-Value 缓存快照，
//! 消除多轮对话中重复前缀计算的开销。
//!
//! # 磁盘格式
//!
//! ```text
//! [4 bytes]   magic: "EMKV"
//! [2 bytes]   version: u16 (LE)，当前 = 1
//! [2 bytes]   model_name_len: u16 (LE)
//! [N bytes]   model_name (UTF-8)
//! [8 bytes]   context_length: u64 (LE)
//! [4 bytes]   layer_count: u32 (LE)
//! --- 重复 layer_count 次 ---
//! [4 bytes]   layer_idx: u32 (LE)
//! [8 bytes]   seq_len: u64 (LE)
//! [8 bytes]   k_bytes_len: u64 (LE)
//! [k_bytes_len bytes]  K 张量原始字节
//! [8 bytes]   v_bytes_len: u64 (LE)
//! [v_bytes_len bytes]  V 张量原始字节
//! ```
//!
//! # 安全考量
//!
//! - 所有偏移量使用 `checked_add` 防溢出（铁律五）
//! - 反序列化时严格校验剩余字节数，防止越界读取
//! - 字符串长度上限防止恶意文件分配超大内存
//! - 层数和字节长度上限防止整数溢出攻击

#![cfg(feature = "pro")]

use std::path::Path;

use anyhow::{Context, Result, ensure};

// ---------------------------------------------------------------------------
// 常量
// ---------------------------------------------------------------------------

/// 文件魔数（"EMKV" 的 4 字节小端序表示）。
const MAGIC: [u8; 4] = *b"EMKV";

/// 当前序列化格式版本。
const CURRENT_VERSION: u16 = 1;

/// 模型名最大长度（4 KiB），防止恶意文件分配超大内存。
const MAX_MODEL_NAME_LEN: usize = 4 * 1024;

/// 层数上限，防止恶意文件导致过大分配。
const MAX_LAYER_COUNT: usize = 1_000_000;

/// 单层 K 或 V 字节上限（4 GiB），防止恶意文件导致超大内存分配。
const MAX_TENSOR_BYTES: usize = 4 * 1024 * 1024 * 1024;

// ---------------------------------------------------------------------------
// 数据结构
// ---------------------------------------------------------------------------

/// KV cache 快照（序列化格式）。
///
/// 包含模型名、上下文长度和每层的 K/V 张量原始字节。
/// 通过 [`serialize`](Self::serialize) 序列化为字节流，
/// 通过 [`deserialize`](Self::deserialize) 从字节流恢复。
///
/// # 示例
///
/// ```no_run
/// # use echomind_infra::kv_cache::*;
/// let snapshot = KvCacheSnapshot::new("qwen2.5-3b".to_string(), 128);
/// let bytes = snapshot.serialize().expect("序列化失败");
/// let restored = KvCacheSnapshot::deserialize(&bytes).expect("反序列化失败");
/// assert_eq!(restored.model_name(), "qwen2.5-3b");
/// ```
#[derive(Debug, Clone)]
pub struct KvCacheSnapshot {
    /// 模型名（用于反序列化时验证模型匹配）。
    model_name: String,
    /// 当前缓存的上下文 token 数（所有层应一致）。
    context_length: usize,
    /// 按层索引排序的 K/V 缓存数据。
    layers: Vec<LayerKvCache>,
}

/// 单层的 KV cache 数据。
///
/// `k_bytes` 和 `v_bytes` 存储 f32 或量化后的张量原始字节，
/// 具体格式取决于模型的量化方式。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerKvCache {
    /// 层索引（从 0 开始）。
    layer_idx: usize,
    /// K 张量原始字节（f32 或量化格式）。
    k_bytes: Vec<u8>,
    /// V 张量原始字节（f32 或量化格式）。
    v_bytes: Vec<u8>,
    /// 该层已缓存的 token 数。
    seq_len: usize,
}

// ---------------------------------------------------------------------------
// KvCacheSnapshot 实现
// ---------------------------------------------------------------------------

impl KvCacheSnapshot {
    /// 创建空的 KV cache 快照。
    ///
    /// # 参数
    /// - `model_name` — 模型名（用于反序列化时验证模型匹配）
    /// - `context_length` — 当前上下文 token 数
    pub fn new(model_name: String, context_length: usize) -> Self {
        Self {
            model_name,
            context_length,
            layers: Vec::new(),
        }
    }

    /// 获取模型名。
    pub fn model_name(&self) -> &str {
        &self.model_name
    }

    /// 获取上下文长度（token 数）。
    pub fn context_length(&self) -> usize {
        self.context_length
    }

    /// 获取所有层的引用。
    pub fn layers(&self) -> &[LayerKvCache] {
        &self.layers
    }

    /// 获取层数。
    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    /// 添加一层 KV cache 数据。
    ///
    /// 层会按 `layer_idx` 排序，以确保序列化/反序列化顺序一致。
    pub fn add_layer(&mut self, layer: LayerKvCache) {
        self.layers.push(layer);
        // 保持按 layer_idx 排序
        self.layers.sort_by_key(|l| l.layer_idx);
    }

    /// 序列化为字节流。
    ///
    /// 返回紧凑的二进制表示，可直接写入文件或网络传输。
    pub fn serialize(&self) -> Result<Vec<u8>> {
        // 预分配缓冲区（header + layers 估算）
        let model_bytes = self.model_name.as_bytes();
        let model_len = u16::try_from(model_bytes.len()).context("模型名过长（超过 u16 范围）")?;
        ensure!(
            model_bytes.len() <= MAX_MODEL_NAME_LEN,
            "模型名超过 {} 字节上限",
            MAX_MODEL_NAME_LEN
        );
        ensure!(
            self.layers.len() <= MAX_LAYER_COUNT,
            "层数超过 {} 上限",
            MAX_LAYER_COUNT
        );

        // 计算总大小（防止溢出）
        let mut total: usize = 4 + 2 + 2; // magic + version + model_name_len
        total = total
            .checked_add(model_bytes.len())
            .context("序列化大小溢出")?;
        total = total.checked_add(8).context("序列化大小溢出")?; // context_length
        total = total.checked_add(4).context("序列化大小溢出")?; // layer_count

        for layer in &self.layers {
            total = total.checked_add(4).context("序列化大小溢出")?; // layer_idx
            total = total.checked_add(8).context("序列化大小溢出")?; // seq_len
            total = total.checked_add(8).context("序列化大小溢出")?; // k_bytes_len
            total = total
                .checked_add(layer.k_bytes.len())
                .context("序列化大小溢出")?;
            total = total.checked_add(8).context("序列化大小溢出")?; // v_bytes_len
            total = total
                .checked_add(layer.v_bytes.len())
                .context("序列化大小溢出")?;
        }

        let mut buf = Vec::with_capacity(total);

        // ---- 文件头 ----
        buf.extend_from_slice(&MAGIC);
        buf.extend_from_slice(&CURRENT_VERSION.to_le_bytes());
        buf.extend_from_slice(&model_len.to_le_bytes());
        buf.extend_from_slice(model_bytes);
        buf.extend_from_slice(&(self.context_length as u64).to_le_bytes());
        buf.extend_from_slice(
            &u32::try_from(self.layers.len())
                .context("层数超过 u32 范围")?
                .to_le_bytes(),
        );

        // ---- 每层 K/V 数据 ----
        for layer in &self.layers {
            buf.extend_from_slice(
                &u32::try_from(layer.layer_idx)
                    .context("层索引超过 u32 范围")?
                    .to_le_bytes(),
            );
            buf.extend_from_slice(&(layer.seq_len as u64).to_le_bytes());

            buf.extend_from_slice(
                &u64::try_from(layer.k_bytes.len())
                    .context("K 字节长度超过 u64 范围")?
                    .to_le_bytes(),
            );
            buf.extend_from_slice(&layer.k_bytes);

            buf.extend_from_slice(
                &u64::try_from(layer.v_bytes.len())
                    .context("V 字节长度超过 u64 范围")?
                    .to_le_bytes(),
            );
            buf.extend_from_slice(&layer.v_bytes);
        }

        Ok(buf)
    }

    /// 从字节流反序列化。
    ///
    /// # 错误
    ///
    /// - 魔数不匹配
    /// - 版本不兼容
    /// - 数据截断（字节不足）
    /// - 层数或字节长度超过上限
    pub fn deserialize(data: &[u8]) -> Result<Self> {
        let mut cursor = Cursor::new(data);

        // ---- 文件头 ----
        let magic = cursor.read_bytes(4).context("读取魔数失败：数据不足")?;
        ensure!(magic == MAGIC, "魔数不匹配：期望 EMKV，得到 {:?}", magic);

        let version_bytes = cursor.read_bytes(2).context("读取版本失败：数据不足")?;
        let version = u16::from_le_bytes([version_bytes[0], version_bytes[1]]);
        ensure!(
            version == CURRENT_VERSION,
            "版本不兼容：期望 {CURRENT_VERSION}，得到 {version}"
        );

        let model_len_bytes = cursor
            .read_bytes(2)
            .context("读取模型名长度失败：数据不足")?;
        let model_len = u16::from_le_bytes([model_len_bytes[0], model_len_bytes[1]]) as usize;
        ensure!(
            model_len <= MAX_MODEL_NAME_LEN,
            "模型名长度 {model_len} 超过上限 {MAX_MODEL_NAME_LEN}"
        );

        let model_name_bytes = cursor
            .read_bytes(model_len)
            .context("读取模型名失败：数据不足")?;
        let model_name = String::from_utf8(model_name_bytes.to_vec())
            .context("模型名不是合法的 UTF-8 字符串")?;

        let ctx_bytes = cursor
            .read_bytes(8)
            .context("读取上下文长度失败：数据不足")?;
        let context_length = read_u64_le(ctx_bytes) as usize;

        let layer_count_bytes = cursor.read_bytes(4).context("读取层数失败：数据不足")?;
        let layer_count = u32::from_le_bytes([
            layer_count_bytes[0],
            layer_count_bytes[1],
            layer_count_bytes[2],
            layer_count_bytes[3],
        ]) as usize;
        ensure!(
            layer_count <= MAX_LAYER_COUNT,
            "层数 {layer_count} 超过上限 {MAX_LAYER_COUNT}"
        );

        // ---- 逐层读取 K/V 数据 ----
        let mut layers = Vec::with_capacity(layer_count);
        for i in 0..layer_count {
            let layer =
                read_layer(&mut cursor).with_context(|| format!("读取第 {i} 层 KV 数据失败"))?;
            layers.push(layer);
        }

        Ok(Self {
            model_name,
            context_length,
            layers,
        })
    }

    /// 保存到文件。
    ///
    /// 原子写入：先写入临时文件，再重命名为目标路径，
    /// 防止写入过程中崩溃导致文件损坏。
    pub fn save_to_file(&self, path: &Path) -> Result<()> {
        let data = self.serialize().context("序列化快照失败")?;

        // 原子写入：先写 .tmp，再 rename
        let tmp_path = path.with_extension("emkv.tmp");
        std::fs::write(&tmp_path, &data)
            .with_context(|| format!("写入临时文件失败: {}", tmp_path.display()))?;
        std::fs::rename(&tmp_path, path).with_context(|| {
            format!(
                "重命名临时文件失败: {} → {}",
                tmp_path.display(),
                path.display()
            )
        })?;

        Ok(())
    }

    /// 从文件加载。
    ///
    /// # 错误
    ///
    /// - 文件不存在或不可读
    /// - 文件内容无法反序列化（魔数/版本/数据截断等）
    pub fn load_from_file(path: &Path) -> Result<Self> {
        let data = std::fs::read(path)
            .with_context(|| format!("读取 KV cache 文件失败: {}", path.display()))?;
        Self::deserialize(&data)
    }
}

// ---------------------------------------------------------------------------
// LayerKvCache 实现
// ---------------------------------------------------------------------------

impl LayerKvCache {
    /// 创建一层 KV cache 数据。
    ///
    /// # 参数
    /// - `layer_idx` — 层索引（从 0 开始）
    /// - `k_bytes` — K 张量原始字节
    /// - `v_bytes` — V 张量原始字节
    /// - `seq_len` — 该层已缓存的 token 数
    pub fn new(layer_idx: usize, k_bytes: Vec<u8>, v_bytes: Vec<u8>, seq_len: usize) -> Self {
        Self {
            layer_idx,
            k_bytes,
            v_bytes,
            seq_len,
        }
    }

    /// 获取层索引。
    pub fn layer_idx(&self) -> usize {
        self.layer_idx
    }

    /// 获取 K 张量字节切片。
    pub fn k_bytes(&self) -> &[u8] {
        &self.k_bytes
    }

    /// 获取 V 张量字节切片。
    pub fn v_bytes(&self) -> &[u8] {
        &self.v_bytes
    }

    /// 获取该层已缓存的 token 数。
    pub fn seq_len(&self) -> usize {
        self.seq_len
    }

    /// 获取 K 张量字节数。
    pub fn k_len(&self) -> usize {
        self.k_bytes.len()
    }

    /// 获取 V 张量字节数。
    pub fn v_len(&self) -> usize {
        self.v_bytes.len()
    }
}

// ---------------------------------------------------------------------------
// 内部辅助：游标读取器
// ---------------------------------------------------------------------------

/// 字节游标读取器，封装偏移量管理和边界检查。
///
/// 所有读取操作都会检查剩余字节数，防止越界读取。
struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    /// 读取 `n` 个字节，返回切片引用。
    ///
    /// # 错误
    ///
    /// 剩余字节数不足时返回错误。
    fn read_bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        ensure!(
            n <= MAX_TENSOR_BYTES,
            "请求的字节数 {n} 超过上限 {MAX_TENSOR_BYTES}"
        );
        let end = self.pos.checked_add(n).context("读取偏移溢出")?;
        ensure!(
            end <= self.data.len(),
            "数据不足：需要 {n} 字节但仅剩 {} 字节",
            self.data.len().saturating_sub(self.pos)
        );
        let slice = &self.data[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    /// 获取当前已读取的位置（用于错误信息）。
    #[allow(dead_code)]
    fn position(&self) -> usize {
        self.pos
    }
}

/// 从 8 字节小端序切片读取 u64。
fn read_u64_le(bytes: &[u8]) -> u64 {
    let mut arr = [0u8; 8];
    arr.copy_from_slice(bytes);
    u64::from_le_bytes(arr)
}

/// 读取单层 KV cache 数据。
///
/// 依次读取 layer_idx、seq_len、k_bytes_len + k_bytes、v_bytes_len + v_bytes。
fn read_layer(cursor: &mut Cursor) -> Result<LayerKvCache> {
    let idx_bytes = cursor.read_bytes(4).context("读取层索引失败")?;
    let layer_idx =
        u32::from_le_bytes([idx_bytes[0], idx_bytes[1], idx_bytes[2], idx_bytes[3]]) as usize;

    let seq_bytes = cursor.read_bytes(8).context("读取 seq_len 失败")?;
    let seq_len = read_u64_le(seq_bytes) as usize;

    // K 张量
    let k_len_bytes = cursor.read_bytes(8).context("读取 K 字节长度失败")?;
    let k_len = read_u64_le(k_len_bytes) as usize;
    ensure!(
        k_len <= MAX_TENSOR_BYTES,
        "K 张量字节数 {k_len} 超过上限 {MAX_TENSOR_BYTES}"
    );
    let k_bytes = cursor
        .read_bytes(k_len)
        .with_context(|| format!("读取 K 张量数据失败（{k_len} 字节）"))?
        .to_vec();

    // V 张量
    let v_len_bytes = cursor.read_bytes(8).context("读取 V 字节长度失败")?;
    let v_len = read_u64_le(v_len_bytes) as usize;
    ensure!(
        v_len <= MAX_TENSOR_BYTES,
        "V 张量字节数 {v_len} 超过上限 {MAX_TENSOR_BYTES}"
    );
    let v_bytes = cursor
        .read_bytes(v_len)
        .with_context(|| format!("读取 V 张量数据失败（{v_len} 字节）"))?
        .to_vec();

    Ok(LayerKvCache {
        layer_idx,
        k_bytes,
        v_bytes,
        seq_len,
    })
}
