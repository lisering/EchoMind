//! 语义缓存金字塔（REQ-PERF-001）：三级缓存策略，减少重复查询的 token 消耗。
//!
//! ## 缓存层级
//!
//! ```text
//! 用户查询
//!   │
//!   ▼
//! L0: 精确匹配缓存 (query hash → answer)     ← 命中率 5-10%, 0 token
//!   │ miss
//!   ▼
//! L1: 语义缓存 (embedding similarity → answer) ← 命中率 10-20%, 0 token
//!   │ miss
//!   ▼
//! L3: 检索结果缓存 (query embedding → chunks)  ← 节省嵌入计算
//!   │
//!   ▼
//! 完整 RAG pipeline
//! ```
//!
//! ## 调研来源
//!
//! - Anthropic Prompt Caching (2024.08)：静态前缀缓存 → 成本 ↓90%，延迟 ↓85%
//! - Mem0 (2026.04)：语义缓存 + 实体链接，LoCoMo 基准 92.5
//!
//! ## 失效策略
//!
//! - 文档导入/删除 → 清空所有缓存
//! - TTL 过期 → 懒删除 + 定期清理
//! - privacy_mode → 完全禁用缓存

use sha2::{Digest, Sha256};

/// 归一化查询文本：小写化 + 去除多余空白 + 去除首尾空白。
///
/// 用于 L0 精确匹配缓存——相同语义但不同大小写/空格的查询应命中同一缓存条目。
///
/// # 示例
///
/// ```
/// # use echomind_core::cache::normalize_query;
/// assert_eq!(normalize_query("  Hello   World  "), "hello world");
/// assert_eq!(normalize_query("RAG  优化"), "rag 优化");
/// assert_eq!(normalize_query("Hello\nWorld"), "hello world");
/// ```
pub fn normalize_query(query: &str) -> String {
    query
        .trim()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// 计算归一化查询的 SHA-256 哈希（十六进制字符串）。
///
/// 用于 L0 精确匹配缓存的索引键——相同归一化查询产生相同哈希。
///
/// # 示例
///
/// ```
/// # use echomind_core::cache::query_hash;
/// // 相同语义不同格式产生相同哈希
/// assert_eq!(query_hash("  Hello   World  "), query_hash("hello world"));
/// assert_eq!(query_hash("Hello\nWorld"), query_hash("HELLO WORLD"));
/// // 不同查询产生不同哈希
/// assert_ne!(query_hash("Hello World"), query_hash("Goodbye World"));
/// ```
pub fn query_hash(query: &str) -> String {
    let normalized = normalize_query(query);
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    let result = hasher.finalize();
    result.iter().map(|b| format!("{b:02x}")).collect()
}

/// 计算两个向量的余弦相似度。
///
/// 用于 L1 语义缓存匹配——查询嵌入与缓存条目嵌入的相似度比较。
///
/// # 参数
/// - `a`：向量 A（查询嵌入）
/// - `b`：向量 B（缓存嵌入）
///
/// # 返回
/// 余弦相似度，范围 [-1, 1]。值越接近 1 表示越相似。
/// 如果任一向量长度为零或长度不匹配，返回 0.0。
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a < 1e-10 || norm_b < 1e-10 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

/// 将 f32 向量序列化为原始字节（用于 SQLite BLOB 存储）。
///
/// 每个浮点数占 4 字节（little-endian），与 `SqliteStorage` 的向量存储格式一致。
pub fn embedding_to_bytes(embedding: &[f32]) -> Vec<u8> {
    embedding.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// 从原始字节反序列化为 f32 向量（从 SQLite BLOB 读取）。
///
/// # 错误
/// 如果字节长度不是 4 的整数倍，返回空向量。
pub fn embedding_from_bytes(bytes: &[u8]) -> Vec<f32> {
    if !bytes.len().is_multiple_of(4) {
        return Vec::new();
    }
    bytes
        .chunks_exact(4)
        .map(|chunk| {
            let arr = [chunk[0], chunk[1], chunk[2], chunk[3]];
            f32::from_le_bytes(arr)
        })
        .collect()
}

/// 判断缓存条目是否已过期。
///
/// # 参数
/// - `created_at`：缓存条目创建时间（Unix 秒级时间戳）
/// - `ttl_secs`：存活时间（秒）
/// - `now`：当前时间（Unix 秒级时间戳）
///
/// # 返回
/// `true` 表示已过期（应失效），`false` 表示仍在有效期内。
pub fn is_expired(created_at: i64, ttl_secs: u64, now: i64) -> bool {
    let ttl = ttl_secs as i64;
    now - created_at > ttl
}

/// 估算单次 RAG 查询的 token 消耗（用于缓存命中时统计节省量）。
///
/// 粗略估算：系统提示 ~300 + 检索片段 ~2000 + 历史 ~500 + 查询 ~100 + 输出 ~800 = ~3700 tokens
pub fn estimate_rag_token_cost() -> u64 {
    3700
}
