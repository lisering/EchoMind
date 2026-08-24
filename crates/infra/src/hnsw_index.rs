//! HNSW 近似最近邻索引（REQ-NFR-005）。
//!
//! 基于 `hnsw_rs` crate（Malkov & Yashunin 2016/2018 论文纯 Rust 实现），
//! 将向量检索复杂度从 O(n) 暴力扫描降至 O(log n)。
//!
//! ## 架构
//!
//! - `HnswIndex` 封装 HNSW 图 + chunk_id 映射表
//! - `build()` 批量构建索引（CPU 密集，调用方应经 `spawn_blocking`）
//! - `search()` 查询 top-k 最近邻，返回 `(chunk_id, cosine_distance)` 对
//! - `save()` / `load()` 持久化到文件（保存原始向量 + ID 映射为 JSON，
//!   加载时重建 HNSW 图——对桌面应用可接受的方案，避免 hnsw_rs 复杂的
//!   二进制 dump API，且支持版本升级后自动重建索引）
//!
//! ## 参数（论文推荐值）
//!
//! - `max_nb_connection = 32`（每节点最大连接数，16-64 标准）
//! - `max_layer = 16`（最大层级）
//! - `ef_construction = 400`（构建时搜索宽度，200-800 标准）
//! - `ef_search = 64`（查询时搜索宽度，≥ top_k）

use std::path::Path;

use anyhow::{Context, Result, bail};
use hnsw_rs::prelude::*;
use serde::{Deserialize, Serialize};
// serde derive 宏在 infra crate 中已通过 Cargo.toml serde feature 启用

/// HNSW 索引默认参数（论文推荐值，REQ-NFR-005）。
const MAX_NB_CONNECTION: usize = 32;
const MAX_LAYER: usize = 16;
const EF_CONSTRUCTION: usize = 400;
const EF_SEARCH: usize = 64;

/// 二进制持久化魔数（REQ-PERF-015 AC-5）：`b"HNSW"`。
const MAGIC: [u8; 4] = *b"HNSW";
/// 二进制持久化格式版本（REQ-PERF-015 AC-5）。
const FORMAT_VERSION: u32 = 1;
/// 文件头字节数：magic(4) + version(4) + dim(4) + count(8)。
const HEADER_LEN: usize = 4 + 4 + 4 + 8;

/// 持久化数据结构（JSON 格式）。
#[derive(Serialize, Deserialize)]
struct IndexData {
    /// `usize ID → chunk_id` 映射（与 HNSW 内部 ID 对齐）
    id_map: Vec<String>,
    /// 原始向量列表（加载时用于重建 HNSW 图）
    vectors: Vec<Vec<f32>>,
}

/// HNSW 近似最近邻索引（REQ-NFR-005）。
///
/// 封装 `hnsw_rs` 的 `Hnsw` 结构，维护 `usize ID → chunk_id` 映射。
/// 索引构建为 CPU 密集任务，调用方应经 `spawn_blocking` 执行。
pub struct HnswIndex {
    /// HNSW 图结构（Cosine 距离）
    hnsw: Hnsw<'static, f32, DistCosine>,
    /// `usize ID → chunk_id` 映射（与 HNSW 内部 ID 对齐）
    id_map: Vec<String>,
    /// 原始向量列表（持久化用，加载时重建 HNSW 图）
    vectors: Vec<Vec<f32>>,
}

impl HnswIndex {
    /// 批量构建 HNSW 索引（REQ-NFR-005-AC-1）。
    ///
    /// 接受 `(chunk_id, vector)` 对列表，构建 HNSW 图。
    /// 返回构建好的 `HnswIndex`，可立即用于搜索或持久化。
    ///
    /// # 参数
    /// - `vectors`: `(chunk_id, embedding)` 对列表
    pub fn build(vectors: &[(String, Vec<f32>)]) -> Result<Self> {
        if vectors.is_empty() {
            return Ok(Self::empty());
        }

        let nb_elem = vectors.len();

        let mut hnsw = Hnsw::<f32, DistCosine>::new(
            MAX_NB_CONNECTION,
            nb_elem,
            MAX_LAYER,
            EF_CONSTRUCTION,
            DistCosine {},
        );

        // 构建 usize ID → chunk_id 映射
        let id_map: Vec<String> = vectors.iter().map(|(id, _)| id.clone()).collect();

        // 批量插入数据：使用 parallel_insert_slice（接受 &[f32]）
        let data_for_insertion: Vec<(&[f32], usize)> = vectors
            .iter()
            .enumerate()
            .map(|(idx, (_, vec))| (vec.as_slice(), idx))
            .collect();

        hnsw.parallel_insert_slice(&data_for_insertion);
        hnsw.set_searching_mode(true);

        // 保留原始向量用于持久化
        let vectors: Vec<Vec<f32>> = vectors.iter().map(|(_, v)| v.clone()).collect();
        Ok(Self {
            hnsw,
            id_map,
            vectors,
        })
    }

    /// 创建空索引（无数据时使用）。
    fn empty() -> Self {
        let hnsw = Hnsw::<f32, DistCosine>::new(
            MAX_NB_CONNECTION,
            1,
            MAX_LAYER,
            EF_CONSTRUCTION,
            DistCosine {},
        );
        Self {
            hnsw,
            id_map: vec![],
            vectors: vec![],
        }
    }

    /// 查询 top-k 最近邻（REQ-NFR-005-AC-2）。
    ///
    /// 返回 `(chunk_id, cosine_distance)` 对列表，按距离升序排列（距离越小越相似）。
    /// HNSW 是近似算法，结果可能略有偏差（召回率 ≥ 90%，AC-5）。
    ///
    /// # 参数
    /// - `query`: 查询向量
    /// - `top_k`: 返回的最近邻数量
    pub fn search(&self, query: &[f32], top_k: usize) -> Vec<(String, f32)> {
        if self.id_map.is_empty() || top_k == 0 {
            return vec![];
        }

        let neighbours = self.hnsw.search(query, top_k, EF_SEARCH);

        neighbours
            .into_iter()
            .filter_map(|n| {
                self.id_map
                    .get(n.d_id)
                    .map(|id| (id.clone(), n.get_distance()))
            })
            .collect()
    }

    /// 持久化索引到 JSON 文件（REQ-NFR-005-AC-3）。
    ///
    /// 保存原始向量 + ID 映射为 JSON 文件。
    /// 加载时从 JSON 重建 HNSW 图（对桌面应用可接受）。
    ///
    /// # 参数
    /// - `path`: JSON 文件路径
    pub fn save(&self, path: &Path) -> Result<()> {
        let data = IndexData {
            id_map: self.id_map.clone(),
            vectors: self.vectors.clone(),
        };

        let json = serde_json::to_string(&data).context("索引序列化失败")?;
        std::fs::write(path, json)
            .with_context(|| format!("写入索引文件失败: {}", path.display()))?;
        Ok(())
    }

    /// 从 JSON 文件加载并重建索引（REQ-NFR-005-AC-3）。
    ///
    /// 读取 `save()` 保存的 JSON 文件，重建 HNSW 图。
    ///
    /// # 参数
    /// - `path`: JSON 文件路径
    pub fn load(path: &Path) -> Result<Self> {
        let json = std::fs::read_to_string(path)
            .with_context(|| format!("读取索引文件失败: {}", path.display()))?;
        let data: IndexData = serde_json::from_str(&json).context("索引反序列化失败")?;

        if data.vectors.is_empty() {
            return Ok(Self::empty());
        }

        // 重建 HNSW 索引
        let vectors: Vec<(String, Vec<f32>)> = data.id_map.into_iter().zip(data.vectors).collect();

        Self::build(&vectors)
    }

    /// 返回索引中的向量数量。
    pub fn len(&self) -> usize {
        self.id_map.len()
    }

    /// 索引是否为空。
    pub fn is_empty(&self) -> bool {
        self.id_map.is_empty()
    }

    /// REQ-PERF-015：二进制持久化（魔数 + 版本 + 维度 + 向量 + ID 映射）。
    ///
    /// 相比 `save()` 的 JSON 格式，二进制格式序列化/反序列化快数倍，
    /// 且自带魔数/版本/维度校验（AC-5）。文件布局（小端）：
    /// `MAGIC(4) | version u32 | dim u32 | count u64 | [id_len u32 | id bytes | dim×f32]...`
    pub fn save_binary(&self, path: &Path) -> Result<()> {
        let bytes = self.serialize_binary()?;
        std::fs::write(path, bytes)
            .with_context(|| format!("写入二进制索引文件失败: {}", path.display()))?;
        Ok(())
    }

    /// REQ-PERF-015：从二进制文件加载并重建索引（明文包装）。
    ///
    /// 校验魔数 + 版本 + 维度（`expected_dim` 不匹配时返回 `Err`）。
    pub fn load_binary(path: &Path, expected_dim: usize) -> Result<Self> {
        let bytes = std::fs::read(path)
            .with_context(|| format!("读取二进制索引文件失败: {}", path.display()))?;
        Self::deserialize_binary(&bytes, expected_dim)
    }

    /// REQ-PERF-015：序列化为二进制字节（供加密落盘使用）。
    ///
    /// 布局同 [`Self::save_binary`]，但不写盘——调用方（SqliteStorage）
    /// 用 AES-256-GCM 加密后写入，避免加密 DB 模式泄露明文向量。
    pub fn serialize_binary(&self) -> Result<Vec<u8>> {
        let dim = self.vectors.first().map_or(0usize, Vec::len);
        let mut buf: Vec<u8> = Vec::with_capacity(
            HEADER_LEN + self.vectors.iter().map(|v| v.len() * 4 + 8).sum::<usize>(),
        );
        buf.extend_from_slice(&MAGIC);
        buf.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
        buf.extend_from_slice(&(dim as u32).to_le_bytes());
        buf.extend_from_slice(&(self.id_map.len() as u64).to_le_bytes());
        for (i, id) in self.id_map.iter().enumerate() {
            let idb = id.as_bytes();
            buf.extend_from_slice(&(idb.len() as u32).to_le_bytes());
            buf.extend_from_slice(idb);
            for x in &self.vectors[i] {
                buf.extend_from_slice(&x.to_le_bytes());
            }
        }
        Ok(buf)
    }

    /// REQ-PERF-015：从二进制字节反序列化并重建索引。
    ///
    /// 校验魔数 + 版本 + 维度（`expected_dim` 不匹配时返回 `Err`，
    /// 调用方回退全量构建）。损坏数据返回 `Err`，不 panic。
    pub fn deserialize_binary(bytes: &[u8], expected_dim: usize) -> Result<Self> {
        if bytes.len() < HEADER_LEN {
            bail!("二进制索引文件过短（{} < {}）", bytes.len(), HEADER_LEN);
        }
        if bytes[0..4] != MAGIC {
            bail!("二进制索引魔数不匹配");
        }
        let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        if version != FORMAT_VERSION {
            bail!("二进制索引版本不匹配: {version}");
        }
        let dim = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
        if dim != expected_dim {
            bail!("二进制索引维度不匹配: {dim} != {expected_dim}");
        }
        let count = u64::from_le_bytes([
            bytes[12], bytes[13], bytes[14], bytes[15], bytes[16], bytes[17], bytes[18], bytes[19],
        ]) as usize;

        let mut offset = HEADER_LEN;
        let mut id_map: Vec<String> = Vec::with_capacity(count);
        let mut vectors: Vec<Vec<f32>> = Vec::with_capacity(count);
        for _ in 0..count {
            if offset + 4 > bytes.len() {
                bail!("二进制索引 ID 长度字段越界");
            }
            let id_len = u32::from_le_bytes([
                bytes[offset],
                bytes[offset + 1],
                bytes[offset + 2],
                bytes[offset + 3],
            ]) as usize;
            offset += 4;
            if offset + id_len > bytes.len() {
                bail!("二进制索引 ID 内容越界");
            }
            let id = std::str::from_utf8(&bytes[offset..offset + id_len])
                .context("二进制索引 ID 非 UTF-8")?
                .to_string();
            offset += id_len;

            let vec_len = dim * 4;
            if offset + vec_len > bytes.len() {
                bail!("二进制索引向量越界");
            }
            let mut vec = Vec::with_capacity(dim);
            for k in 0..dim {
                let s = offset + k * 4;
                vec.push(f32::from_le_bytes([
                    bytes[s],
                    bytes[s + 1],
                    bytes[s + 2],
                    bytes[s + 3],
                ]));
            }
            offset += vec_len;
            id_map.push(id);
            vectors.push(vec);
        }

        // 重建 HNSW 图
        let pairs: Vec<(String, Vec<f32>)> = id_map.into_iter().zip(vectors).collect();
        Self::build(&pairs)
    }
}

// ================== 单元测试 ==================

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use tempfile::TempDir;

    /// 生成测试向量（确定性伪随机，模拟 embedding 输出）。
    ///
    /// 使用多频混合正弦函数避免单一周期导致的向量碰撞
    /// （如 `sin(i * 0.1)` 在 i=63 时因 2π≈6.28 周期性与 i=0 近似）。
    fn make_test_vectors(n: usize, dim: usize) -> Vec<(String, Vec<f32>)> {
        (0..n)
            .map(|i| {
                let vec: Vec<f32> = (0..dim)
                    .map(|j| {
                        // 多频混合 + 非整数步长，破坏周期性
                        let v = (i as f32 * 0.137 + j as f32 * 0.731).sin() * 0.6
                            + (i as f32 * 0.913 - j as f32 * 0.197).sin() * 0.3
                            + (i as f32 * 1.571 + j as f32 * 0.421).cos() * 0.1;
                        v * 0.5 + 0.5
                    })
                    .collect();
                (format!("chunk-{i}"), vec)
            })
            .collect()
    }

    /// TC-NFR-005：`HnswIndex::build` + `search` 基本功能（AC-1 + AC-2）。
    #[test]
    fn tc_nfr_005_build_and_search() {
        let vectors = make_test_vectors(100, 384);

        // AC-1：构建索引
        let index = HnswIndex::build(&vectors).expect("构建 HNSW 索引失败");
        assert_eq!(index.len(), 100, "索引应包含 100 个向量");

        // AC-2：搜索 top-5
        let query = &vectors[0].1;
        let results = index.search(query, 5);
        assert_eq!(results.len(), 5, "应返回 5 个最近邻");

        // 搜索自身应排在第一位（距离最小）
        assert_eq!(results[0].0, "chunk-0", "查询 chunk-0 自身应排第一");
    }

    /// TC-NFR-005b：HNSW 查询结果召回率 ≥ 90%（AC-5）。
    #[test]
    fn tc_nfr_005b_recall_rate() {
        let vectors = make_test_vectors(200, 64);
        let index = HnswIndex::build(&vectors).expect("构建索引失败");

        // 对每个向量查询 top-5，验证自身是否在结果中
        let mut hits = 0;
        let total = vectors.len();
        for (i, (_, query)) in vectors.iter().enumerate() {
            let results = index.search(query, 5);
            // 自身应出现在 top-5 中
            if results.iter().any(|(id, _)| id == &format!("chunk-{i}")) {
                hits += 1;
            }
        }

        let recall = hits as f32 / total as f32;
        // AC-5：召回率 ≥ 90%
        assert!(
            recall >= 0.90,
            "HNSW 召回率应 ≥ 90%，实际 {recall:.2}（{hits}/{total}）"
        );
    }

    /// TC-NFR-005c：空向量列表构建空索引（边界测试）。
    #[test]
    fn tc_nfr_005c_empty_vectors() {
        let vectors: Vec<(String, Vec<f32>)> = vec![];
        let index = HnswIndex::build(&vectors).expect("构建空索引应成功");
        assert!(index.is_empty(), "空索引 is_empty 应为 true");

        // 空索引搜索返回空结果
        let results = index.search(&[0.0; 384], 5);
        assert!(results.is_empty(), "空索引搜索应返回空结果");
    }

    /// TC-NFR-005d：`save` + `load` 持久化往返测试（AC-3）。
    #[test]
    fn tc_nfr_005d_save_and_load() {
        let vectors = make_test_vectors(50, 128);
        let index = HnswIndex::build(&vectors).expect("构建索引失败");

        let tmpdir = TempDir::new().expect("创建临时目录失败");
        let index_path = tmpdir.path().join("test_index.json");

        // 保存
        index.save(&index_path).expect("保存索引失败");

        // 加载
        let loaded = HnswIndex::load(&index_path).expect("加载索引失败");
        assert_eq!(loaded.len(), 50, "加载后索引大小应一致");

        // 验证搜索结果一致
        let query = &vectors[10].1;
        let original_results = index.search(query, 5);
        let loaded_results = loaded.search(query, 5);

        // 验证召回的 chunk_id 集合一致
        let original_ids: std::collections::HashSet<&str> =
            original_results.iter().map(|(id, _)| id.as_str()).collect();
        let loaded_ids: std::collections::HashSet<&str> =
            loaded_results.iter().map(|(id, _)| id.as_str()).collect();

        // 至少 80% 的结果一致（HNSW 近似性允许少量差异）
        let overlap = original_ids.intersection(&loaded_ids).count();
        let min_expected = (original_ids.len() as f32 * 0.8) as usize;
        assert!(
            overlap >= min_expected,
            "加载后搜索结果应与原始 ≥80% 一致，实际 {overlap}/{}",
            original_ids.len()
        );
    }

    /// TC-NFR-005e：top_k=0 时返回空结果（边界测试）。
    #[test]
    fn tc_nfr_005e_top_k_zero() {
        let vectors = make_test_vectors(10, 32);
        let index = HnswIndex::build(&vectors).expect("构建索引失败");

        let results = index.search(&vectors[0].1, 0);
        assert!(results.is_empty(), "top_k=0 应返回空结果");
    }

    // ========================================================================
    // REQ-PERF-015 TC-PERSIST-001~004：二进制持久化单元测试
    // ========================================================================

    /// TC-PERSIST-001：serialize/deserialize 二进制往返（AC-6 语义不变）。
    #[test]
    fn tc_persist_001_binary_roundtrip() {
        let vectors = make_test_vectors(50, 128);
        let index = HnswIndex::build(&vectors).expect("构建索引失败");

        let bytes = index.serialize_binary().expect("序列化失败");
        let loaded = HnswIndex::deserialize_binary(&bytes, 128).expect("反序列化失败");

        assert_eq!(loaded.len(), 50, "加载后索引大小应一致");
        // 搜索语义一致（HNSW 近似，≥80% 重叠）
        let query = &vectors[10].1;
        let a: std::collections::HashSet<String> = index
            .search(query, 5)
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        let b: std::collections::HashSet<String> = loaded
            .search(query, 5)
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        let overlap = a.intersection(&b).count();
        assert!(overlap >= 4, "往返后 top-5 应 ≥80% 重叠，实际 {overlap}");
    }

    /// TC-PERSIST-002：魔数损坏 → 返回 Err（AC-5 不 panic）。
    #[test]
    fn tc_persist_002_corrupt_magic_returns_err() {
        let vectors = make_test_vectors(10, 32);
        let index = HnswIndex::build(&vectors).expect("构建索引失败");
        let mut bytes = index.serialize_binary().expect("序列化失败");
        bytes[0] ^= 0xFF; // 破坏魔数

        let res = HnswIndex::deserialize_binary(&bytes, 32);
        assert!(res.is_err(), "魔数损坏必须返回 Err");
    }

    /// TC-PERSIST-003：维度不匹配 → 返回 Err（AC-3 回退全量构建）。
    #[test]
    fn tc_persist_003_dim_mismatch_returns_err() {
        let vectors = make_test_vectors(10, 32);
        let index = HnswIndex::build(&vectors).expect("构建索引失败");
        let bytes = index.serialize_binary().expect("序列化失败");

        let res = HnswIndex::deserialize_binary(&bytes, 64);
        assert!(res.is_err(), "维度不匹配必须返回 Err（触发回退全量构建）");
    }

    /// TC-PERSIST-004：截断数据 → 返回 Err（不 panic，不越界）。
    #[test]
    fn tc_persist_004_truncated_returns_err() {
        let vectors = make_test_vectors(10, 32);
        let index = HnswIndex::build(&vectors).expect("构建索引失败");
        let bytes = index.serialize_binary().expect("序列化失败");

        let truncated = &bytes[..bytes.len() / 2];
        let res = HnswIndex::deserialize_binary(truncated, 32);
        assert!(res.is_err(), "截断数据必须返回 Err");
    }
}
