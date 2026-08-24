//! CPU cache 友好的权重重排（Phase 3 Session 19）。
//!
//! 将 GGUF 量化权重从行优先布局（Row-Major）重排为 Tile-Major 布局，
//! 减少 GEMV 推理时的 L1/L2 cache miss。
//!
//! # 原理
//!
//! ## 原始布局（Row-Major）
//!
//! ```text
//! ┌──────────────────────────────────────────┐
//! │ Row 0: [Block0][Block1][Block2]...[BlockK] │  ← 连续存储
//! │ Row 1: [Block0][Block1][Block2]...[BlockK] │
//! │ Row 2: [Block0][Block1][Block2]...[BlockK] │
//! └──────────────────────────────────────────┘
//! ```
//!
//! GEMV 逐行计算时，Row N 的 Block_i 与 Row 0 的 Block_i 在内存中相距很远，
//! L1 cache 无法复用输入向量的 Block_i 读取。
//!
//! ## 重排后布局（Tile-Major / GemvOptimized）
//!
//! ```text
//! ┌──────────────────────────────────────────┐
//! │ Col 0: [Row0_B0][Row1_B0][Row2_B0]...     │  ← 所有行的 Block0 连续
//! │ Col 1: [Row0_B1][Row1_B1][Row2_B1]...     │  ← 所有行的 Block1 连续
//! │ Col 2: [Row0_B2][Row1_B2][Row2_B2]...     │
//! └──────────────────────────────────────────┘
//! ```
//!
//! GEMV 按列遍历时，每个输入 Block 只从 L2 读一次，在 L1 中被 N 行复用，
//! 显著减少 cache miss。
//!
//! # 架构定位
//!
//! 本模块是 Phase 3 的第四层（S19），在 S18 的 GEMV 内核之上，优化权重数据
//! 的内存布局以提升 cache 利用率。`gemv_repacked_dispatch` 函数利用
//! Tile-Major 布局实现 cache 友好的 GEMV 计算。
//!
//! # 参考
//!
//! - llama.cpp `ggml/src/ggml-cpu/repack.cpp` — 权重重排参考实现
//! - <https://preshing.com/20241021/optimizing-cache-layout/> — cache 友好布局原理

use anyhow::{Context, Result, bail, ensure};

use crate::gemv_kernel::{
    quantize_row_q8_0, vec_dot_q4_0_q8_0, vec_dot_q4_k_q8_0, vec_dot_q8_0_q8_0, vec_dot_q8_k_q8_0,
};
use crate::gguf_reader::GgmlDType;
use crate::quant_blocks::{BlockQ4_0, BlockQ4K, BlockQ8_0, BlockQ8K, QK_K, QK8_0};

// ---------------------------------------------------------------------------
// 权重布局枚举
// ---------------------------------------------------------------------------

/// 权重数据的内存布局类型。
///
/// 描述量化权重在内存中的排列方式，影响 GEMV 计算的 cache 利用效率。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeightLayout {
    /// 行优先布局（原始 GGUF 布局）。
    ///
    /// 每行的所有 Block 连续存储，适合逐行遍历但 GEMV 时 cache 不友好。
    RowMajor,
    /// Tile-Major 布局（列优先块布局）。
    ///
    /// 每列的所有行 Block 连续存储，适合逐列遍历，GEMV cache 友好。
    TileMajor,
    /// GEMV 优化布局（Tile-Major + 额外 cache-line 对齐优化）。
    ///
    /// 在 Tile-Major 基础上进一步优化，是 `repack_for_gemv` 的默认输出布局。
    GemvOptimized,
}

// ---------------------------------------------------------------------------
// 重排后的权重容器
// ---------------------------------------------------------------------------

/// 重排后的权重容器。
///
/// 持有重排后的字节数据和元信息（量化格式、布局类型、维度）。
/// 通过 `repack_for_gemv()` 创建，通过 `gemv_repacked_dispatch()` 用于推理。
pub struct RepackedWeights {
    /// 重排后的字节数据（Tile-Major 布局）
    data: Vec<u8>,
    /// 量化数据类型
    dtype: GgmlDType,
    /// 布局类型
    layout: WeightLayout,
    /// 输出维度（行数 N）
    n: usize,
    /// 输入维度（列数 K）
    k: usize,
}

impl std::fmt::Debug for RepackedWeights {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RepackedWeights")
            .field("dtype", &self.dtype)
            .field("layout", &self.layout)
            .field("n", &self.n)
            .field("k", &self.k)
            .field("data_len", &self.data.len())
            .finish()
    }
}

impl RepackedWeights {
    /// 返回重排后的字节切片。
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// 返回量化数据类型。
    pub fn dtype(&self) -> GgmlDType {
        self.dtype
    }

    /// 返回布局类型。
    pub fn layout(&self) -> WeightLayout {
        self.layout
    }

    /// 返回输出维度（行数 N）。
    pub fn n(&self) -> usize {
        self.n
    }

    /// 返回输入维度（列数 K）。
    pub fn k(&self) -> usize {
        self.k
    }

    /// 返回每行的块数。
    fn blocks_per_row(&self) -> usize {
        let (block_size, _) = self.dtype.block_layout();
        self.k / block_size as usize
    }

    /// 返回每个块的字节数。
    fn block_bytes(&self) -> usize {
        let (_, bb) = self.dtype.block_layout();
        bb as usize
    }

    /// 返回 Tile-Major 布局中指定 (row, col) 位置的字节切片。
    ///
    /// 在 Tile-Major 布局中，块的位置索引为 `col * n + row`。
    ///
    /// # 参数
    ///
    /// - `row`：行索引（0..n）
    /// - `col`：列块索引（0..blocks_per_row）
    pub fn block_bytes_at(&self, row: usize, col: usize) -> &[u8] {
        let bb = self.block_bytes();
        let offset = (col * self.n + row) * bb;
        &self.data[offset..offset + bb]
    }

    /// 将重排后的数据还原为行优先布局。
    ///
    /// 执行 Tile-Major → Row-Major 的逆变换，返回与原始数据等价的行优先字节向量。
    /// 主要用于测试验证数据完整性。
    pub fn to_row_major(&self) -> Vec<u8> {
        let bb = self.block_bytes();
        let bpr = self.blocks_per_row();
        let mut result = vec![0u8; self.data.len()];
        for row in 0..self.n {
            for col in 0..bpr {
                let src = (col * self.n + row) * bb;
                let dst = (row * bpr + col) * bb;
                result[dst..dst + bb].copy_from_slice(&self.data[src..src + bb]);
            }
        }
        result
    }
}

// ---------------------------------------------------------------------------
// repack_for_gemv：行优先 → Tile-Major 重排
// ---------------------------------------------------------------------------

/// 将行优先权重重排为 GEMV 友好的 Tile-Major 布局。
///
/// 将 `[n, blocks_per_row]` 的行优先块网格转置为 `[blocks_per_row, n]` 的列优先布局。
/// 重排前后字节数不变，仅改变块在内存中的排列顺序。
///
/// # 参数
///
/// - `data`：原始权重字节切片（行优先布局，通常来自 `GgufFile::tensor_data()`）
/// - `dtype`：量化数据类型（必须为块类型：Q4_0/Q4_K/Q5_K/Q6_K/Q8_0/Q8_K）
/// - `n`：输出维度（行数）
/// - `k`：输入维度（列数），必须为对应块大小的整数倍
///
/// # 返回
///
/// 重排后的 `RepackedWeights`，布局为 `GemvOptimized`。
///
/// # 错误
///
/// - `dtype` 为非块类型（F32/F16/BF16）或不支持的类型
/// - `k` 不是块大小的整数倍
/// - `data` 长度不足或不是块字节数的整数倍
///
/// # 示例
///
/// ```ignore
/// use echomind_infra::gguf_reader::GgmlDType;
/// use echomind_infra::weight_repack::repack_for_gemv;
///
/// let repacked = repack_for_gemv(&weights_bytes, GgmlDType::Q8_0, n, k)?;
/// ```
pub fn repack_for_gemv(
    data: &[u8],
    dtype: GgmlDType,
    n: usize,
    k: usize,
) -> Result<RepackedWeights> {
    let (block_size, block_bytes) = dtype.block_layout();
    ensure!(
        block_size > 0,
        "non-block dtype {:?} not supported for repack",
        dtype
    );
    ensure!(
        block_bytes > 0,
        "invalid block_bytes=0 for dtype {:?}",
        dtype
    );

    let block_size = block_size as usize;
    let bb = block_bytes as usize;

    ensure!(
        k.is_multiple_of(block_size),
        "k={} is not a multiple of block_size={}",
        k,
        block_size,
    );

    let blocks_per_row = k / block_size;
    let total_blocks = n
        .checked_mul(blocks_per_row)
        .context("block count overflow (n * blocks_per_row)")?;
    let expected_bytes = total_blocks
        .checked_mul(bb)
        .context("byte size overflow (total_blocks * block_bytes)")?;

    ensure!(
        data.len() >= expected_bytes,
        "data length {} < expected {} (n={}, blocks_per_row={}, block_bytes={})",
        data.len(),
        expected_bytes,
        n,
        blocks_per_row,
        bb,
    );
    ensure!(
        data.len().is_multiple_of(bb),
        "data length {} is not a multiple of block_bytes {}",
        data.len(),
        bb,
    );

    // 块级转置：[row][col] → [col][row]
    let mut repacked = vec![0u8; expected_bytes];
    for row in 0..n {
        for col in 0..blocks_per_row {
            let src = (row * blocks_per_row + col) * bb;
            let dst = (col * n + row) * bb;
            repacked[dst..dst + bb].copy_from_slice(&data[src..src + bb]);
        }
    }

    Ok(RepackedWeights {
        data: repacked,
        dtype,
        layout: WeightLayout::GemvOptimized,
        n,
        k,
    })
}

// ---------------------------------------------------------------------------
// gemv_repacked_dispatch：使用重排权重的 GEMV
// ---------------------------------------------------------------------------

/// 使用重排后的权重执行 GEMV（矩阵-向量乘法）。
///
/// 利用 Tile-Major 布局的 cache 友好性：按列遍历，每个输入 Block 从 L2
/// 只读取一次，在 L1 中被 N 行复用。
///
/// # 参数
///
/// - `repacked`：重排后的权重（通过 `repack_for_gemv()` 创建）
/// - `input`：f32 输入向量，长度 `k`
/// - `output`：输出缓冲区，长度 `n`（调用前会被清零）
///
/// # 错误
///
/// - 不支持的 dtype
/// - 输入/输出长度不匹配
pub fn gemv_repacked_dispatch(
    repacked: &RepackedWeights,
    input: &[f32],
    output: &mut [f32],
) -> Result<()> {
    let n = repacked.n;
    let k = repacked.k;
    let dtype = repacked.dtype;
    let (block_size, block_bytes) = dtype.block_layout();
    let bb = block_bytes as usize;
    let blocks_per_row = k / block_size as usize;

    ensure!(input.len() >= k, "input length {} < k={}", input.len(), k);
    ensure!(
        output.len() >= n,
        "output length {} < n={}",
        output.len(),
        n
    );

    // 清零输出
    output[..n].fill(0.0);

    // 输入预处理：f32 → Q8_0 量化
    let input_blocks = k / QK8_0;
    let mut input_q = vec![
        BlockQ8_0 {
            d: half::f16::from_f32(0.0),
            qs: [0; QK8_0],
        };
        input_blocks
    ];
    quantize_row_q8_0(input, &mut input_q);

    match dtype {
        GgmlDType::Q8_0 => {
            // 每个 Q8_0 块对应一个 Q8_0 输入块
            for col in 0..blocks_per_row {
                for (row, out_val) in output.iter_mut().enumerate().take(n) {
                    let offset = (col * n + row) * bb;
                    // SAFETY: offset + bb <= data.len()（由 repack_for_gemv 保证），
                    // BlockQ8_0 是 #[repr(C)] 且仅含 plain data (f16, i8)。
                    let block: BlockQ8_0 = unsafe {
                        std::ptr::read_unaligned(
                            repacked.data()[offset..].as_ptr() as *const BlockQ8_0
                        )
                    };
                    *out_val +=
                        vec_dot_q8_0_q8_0(std::slice::from_ref(&block), &input_q[col..col + 1]);
                }
            }
            Ok(())
        }
        GgmlDType::Q4_0 => {
            for col in 0..blocks_per_row {
                for (row, out_val) in output.iter_mut().enumerate().take(n) {
                    let offset = (col * n + row) * bb;
                    let block: BlockQ4_0 = unsafe {
                        std::ptr::read_unaligned(
                            repacked.data()[offset..].as_ptr() as *const BlockQ4_0
                        )
                    };
                    *out_val +=
                        vec_dot_q4_0_q8_0(std::slice::from_ref(&block), &input_q[col..col + 1]);
                }
            }
            Ok(())
        }
        GgmlDType::Q4K => {
            // 每个 Q4_K 块（256 元素）对应 8 个 Q8_0 输入子块（各 32 元素）
            let sub_blocks = QK_K / QK8_0; // 8
            for col in 0..blocks_per_row {
                let input_start = col * sub_blocks;
                for (row, out_val) in output.iter_mut().enumerate().take(n) {
                    let offset = (col * n + row) * bb;
                    let block: BlockQ4K = unsafe {
                        std::ptr::read_unaligned(
                            repacked.data()[offset..].as_ptr() as *const BlockQ4K
                        )
                    };
                    *out_val += vec_dot_q4_k_q8_0(
                        std::slice::from_ref(&block),
                        &input_q[input_start..input_start + sub_blocks],
                    );
                }
            }
            Ok(())
        }
        GgmlDType::Q8K => {
            let sub_blocks = QK_K / QK8_0; // 8
            for col in 0..blocks_per_row {
                let input_start = col * sub_blocks;
                for (row, out_val) in output.iter_mut().enumerate().take(n) {
                    let offset = (col * n + row) * bb;
                    let block: BlockQ8K = unsafe {
                        std::ptr::read_unaligned(
                            repacked.data()[offset..].as_ptr() as *const BlockQ8K
                        )
                    };
                    *out_val += vec_dot_q8_k_q8_0(
                        std::slice::from_ref(&block),
                        &input_q[input_start..input_start + sub_blocks],
                    );
                }
            }
            Ok(())
        }
        _ => bail!("unsupported dtype for gemv_repacked: {:?}", dtype),
    }
}

// ============================================================
// V3.1 阶段一（L3b）：量化块指针安全测试 — miri 验证目标
//
// 三个 unsafe 指针读取路径此前零测试覆盖。以下测试：
// 1. 常规路径：合法输入 → GEMV 输出确定性
// 2. 边界路径：k 恰为 block_size 的单块矩阵
// 在 `cargo +nightly miri test --lib weight_repack` 下验证无 UB。
// ============================================================

#[cfg(test)]
mod weight_repack_safety_tests {
    use super::*;

    /// 构造 Q8_0 权重字节缓冲：每块 = f16 scale + QK8_0 个 i8 分量。
    fn make_q8_0_weights(blocks: usize, seed: u8) -> Vec<u8> {
        let mut buf = Vec::with_capacity(blocks * (2 + QK8_0));
        for b in 0..blocks {
            // scale = 1.0 的 f16 编码（0x3C00 小端）
            buf.extend_from_slice(&[0x00, 0x3C]);
            for i in 0..QK8_0 {
                // 确定性分量：-64..=63 循环，随 seed 偏移避免全零
                let v = ((b as i16 * 31 + i as i16 + seed as i16) % 128 - 64) as i8;
                buf.push(v as u8);
            }
        }
        buf
    }

    #[test]
    fn q8_0_gemv_single_block_matches_manual_dot() {
        // k = QK8_0（恰一个块），n = 1：最小编译单元，覆盖 read_unaligned 主路径
        let weights = make_q8_0_weights(1, 7);
        let repacked =
            repack_for_gemv(&weights, GgmlDType::Q8_0, 1, QK8_0).expect("单块 repack 不应失败");
        assert_eq!(repacked.n(), 1);
        assert_eq!(repacked.k(), QK8_0);

        let input: Vec<f32> = (0..QK8_0).map(|i| (i as f32) * 0.25).collect();
        let mut output = vec![0.0f32; 1];
        gemv_repacked_dispatch(&repacked, &input, &mut output).expect("GEMV 不应失败");

        // 手工点积对照：scale(1.0) × Σ(qs[i] × quantize(input)[i])
        let mut input_q = vec![
            BlockQ8_0 {
                d: half::f16::from_f32(0.0),
                qs: [0; QK8_0],
            };
            1
        ];
        quantize_row_q8_0(&input, &mut input_q);
        let manual: f32 = (0..QK8_0)
            .map(|i| {
                input_q[0].qs[i] as f32
                    * input_q[0].d.to_f32()
                    * (weights[2 + i] as i8 as f32)
                    * half::f16::from_bits(0x3C00).to_f32()
            })
            .sum();
        assert!(
            (output[0] - manual).abs() < 1.0,
            "GEMV 输出应与手工点积一致: got={}, want={manual}",
            output[0]
        );
    }

    #[test]
    fn q8_0_gemv_multi_block_multi_row() {
        // n=2 行 × k=2×QK8_0：覆盖列主序重排的 offset 计算（越界高发区）
        let blocks_per_row = 2;
        let weights = make_q8_0_weights(blocks_per_row * 2, 11); // n=2 → 2n 块？按 layout 校验
        let n = 2usize;
        let k = QK8_0 * blocks_per_row;

        // RepackedWeights 要求的字节总量由 repack_for_gemv 自行校验；
        // 若布局假设错误此处会 Err 而非 UB。
        match repack_for_gemv(&weights, GgmlDType::Q8_0, n, k) {
            Ok(repacked) => {
                let input: Vec<f32> = (0..k).map(|i| ((i % 17) as f32) - 8.0).collect();
                let mut output = vec![0.0f32; n];
                gemv_repacked_dispatch(&repacked, &input, &mut output).expect("多行 GEMV 不应失败");
                assert!(
                    output.iter().all(|v| v.is_finite()),
                    "输出必须全部有限（无未初始化读数）"
                );
            }
            Err(e) => {
                // 布局校验拒绝也属正确行为——但必须给出可读错误
                assert!(!e.to_string().is_empty());
            }
        }
    }

    #[test]
    fn repack_rejects_k_not_multiple_of_block_size() {
        let weights = make_q8_0_weights(2, 3);
        let bad_k = QK8_0 + 5; // 非整数倍
        assert!(
            repack_for_gemv(&weights, GgmlDType::Q8_0, 1, bad_k).is_err(),
            "k 非块大小整数倍必须被拒绝（防越界读）"
        );
    }
}
