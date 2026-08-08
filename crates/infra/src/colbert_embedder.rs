//! ColBERT 多向量嵌入引擎（REQ-PERF-008, Pro feature）。
//!
//! 基于 ColBERT 论文（arXiv:2004.12832）的 Late Interaction 模型，
//! 为文本中的每个 token 生成独立的嵌入向量，而非整个文本一个向量。
//!
//! ## 与 LocalEmbedder 的关系
//!
//! `ColbertEmbedder` 复用 `LocalEmbedder` 的 ONNX 推理引擎，
//! 但对文本进行 token 级分词后逐 token 嵌入（而非整体嵌入 + mean pooling）。
//!
//! ## 存储成本
//!
//! 多向量存储成本为 N tokens × dim/chunk（vs 单向量的 1 × dim/chunk）。
//! 仅 Pro 用户可用此功能。
//!
//! ## MaxSim 检索
//!
//! 检索时使用 `maxsim()` 函数（`crates/core/src/lib.rs`）：
//! 对 query 的每个 token 向量，在 document 的所有 token 向量中找最大余弦相似度，
//! 然后求和得到 query-document 相关度分数。

use crate::local_embedder::LocalEmbedder;
use anyhow::Context;
use echomind_core::{Embedder as _, MultiVectorEmbedder};

/// ColBERT 式 token 级多向量嵌入器（Pro feature, REQ-PERF-008）。
///
/// 复用 `LocalEmbedder` 的 ONNX 推理引擎，但将文本分词后逐 token 嵌入。
/// 每个 token 的嵌入向量独立存储，检索时使用 MaxSim 交互计算。
///
/// # 分词策略
///
/// 使用简单的 word-level 分词：
/// - 英文：按空格分割，保留标点为独立 token
/// - 中文：逐字符分割（CJK 无空格分隔）
/// - 数字/标识符：保持完整
///
/// 这是一种简化的 ColBERT 实现。真正的 ColBERT 使用 BERT tokenizer
/// 生成 subword tokens，但需要专门训练的模型。本实现复用 all-MiniLM-L6-v2
/// 模型（sentence-level 训练），在 token 级嵌入上精度有所折衷，
/// 但提供了 ColBERT 的基础设施（多向量存储 + MaxSim 检索）。
#[derive(Clone)]
pub struct ColbertEmbedder {
    /// 底层嵌入引擎（复用 LocalEmbedder 的 ONNX 会话池）
    embedder: LocalEmbedder,
}

impl ColbertEmbedder {
    /// 从已有 `LocalEmbedder` 创建 ColBERT 嵌入器。
    ///
    /// 复用底层 ONNX 会话池，无额外模型加载开销。
    pub fn from_local_embedder(embedder: LocalEmbedder) -> Self {
        Self { embedder }
    }

    /// 文本 → token 列表（word-level 分词）。
    ///
    /// 分词规则：
    /// - 按空格分割
    /// - CJK 字符逐字符为独立 token
    /// - 标点符号为独立 token
    /// - 连续的字母/数字/下划线为一个 token
    ///
    /// # 示例
    /// ```
    /// # use echomind_infra::colbert_embedder::ColbertEmbedder;
    /// let tokens = ColbertEmbedder::tokenize("Hello 世界!");
    /// assert!(tokens.len() >= 3); // "Hello" + "世" + "界" + "!"
    /// ```
    pub fn tokenize(text: &str) -> Vec<String> {
        let mut tokens = Vec::new();
        let mut current = String::new();

        for ch in text.chars() {
            if Self::is_cjk(ch) {
                // CJK 字符逐字为独立 token
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
                tokens.push(ch.to_string());
            } else if ch.is_alphanumeric() || ch == '_' || ch == '-' {
                current.push(ch);
            } else {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
                // 标点和空格忽略（不生成 token）
            }
        }
        if !current.is_empty() {
            tokens.push(current);
        }
        tokens
    }

    /// 判断字符是否为 CJK（中日韩）字符。
    fn is_cjk(ch: char) -> bool {
        let code = ch as u32;
        // CJK Unified Ideographs: U+4E00 - U+9FFF
        // CJK Extension A: U+3400 - U+4DBF
        // CJK Compatibility Ideographs: U+F900 - U+FAFF
        (0x4E00..=0x9FFF).contains(&code)
            || (0x3400..=0x4DBF).contains(&code)
            || (0xF900..=0xFAFF).contains(&code)
            || (0x3040..=0x309F).contains(&code) // Hiragana
            || (0x30A0..=0x30FF).contains(&code) // Katakana
            || (0xAC00..=0xD7AF).contains(&code) // Hangul Syllables
    }

    /// 返回底层嵌入引擎的向量维度。
    pub fn dim(&self) -> usize {
        self.embedder.dim()
    }
}

impl MultiVectorEmbedder for ColbertEmbedder {
    /// 为文本中的每个 token 生成独立的嵌入向量。
    ///
    /// # 流程
    /// 1. 将文本分词为 token 列表
    /// 2. 使用 `LocalEmbedder::embed_batch()` 批量嵌入
    /// 3. 返回 `Vec<Vec<f32>>`（每 token 一个向量）
    ///
    /// # 参数
    /// - `text`: 输入文本
    ///
    /// # 返回
    /// `Vec<Vec<f32>>`，每个内层 Vec 是一个 token 的嵌入向量。
    /// 空文本返回空 Vec。
    async fn embed_tokens(&self, text: &str) -> anyhow::Result<Vec<Vec<f32>>> {
        let tokens = Self::tokenize(text);
        if tokens.is_empty() {
            return Ok(Vec::new());
        }
        self.embedder.embed_batch(&tokens).await
    }

    /// 批量多向量嵌入。
    ///
    /// 将多个文本分词后合并所有 token，批量嵌入后按文本拆分返回。
    /// 这比逐条调用 `embed_tokens` 更高效（单次 ONNX 推理处理全部 token）。
    async fn embed_tokens_batch(&self, texts: &[String]) -> anyhow::Result<Vec<Vec<Vec<f32>>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // 收集所有 token + 记录每文本的 token 范围
        let mut all_tokens: Vec<String> = Vec::new();
        let mut ranges: Vec<(usize, usize)> = Vec::new();

        for text in texts {
            let tokens = Self::tokenize(text);
            let start = all_tokens.len();
            all_tokens.extend(tokens);
            let end = all_tokens.len();
            ranges.push((start, end));
        }

        if all_tokens.is_empty() {
            return Ok(texts.iter().map(|_| Vec::new()).collect());
        }

        // 批量嵌入所有 token
        let all_embeddings = self.embedder.embed_batch(&all_tokens).await?;

        // 按文本拆分结果
        let mut results = Vec::with_capacity(texts.len());
        for (start, end) in &ranges {
            results.push(all_embeddings[*start..*end].to_vec());
        }

        Ok(results)
    }
}

/// 将多向量数据序列化为 BLOB 字节（用于 SQLite 存储）。
///
/// 格式：4 字节 token_count（LE u32）+ token_count × (4 字节 dim（LE u32） + dim × 4 字节 f32）
///
/// # 参数
/// - `token_vectors`: 多向量数据（每个 token 一个 Vec<f32>）
///
/// # 返回
/// 序列化后的字节切片。
pub fn multivec_to_bytes(token_vectors: &[Vec<f32>]) -> Vec<u8> {
    let token_count = token_vectors.len() as u32;
    let mut bytes = Vec::with_capacity(4 + token_vectors.len() * (4 + 384 * 4));

    // token count
    bytes.extend_from_slice(&token_count.to_le_bytes());

    for vec in token_vectors {
        let dim = vec.len() as u32;
        bytes.extend_from_slice(&dim.to_le_bytes());
        for &val in vec {
            bytes.extend_from_slice(&val.to_le_bytes());
        }
    }

    bytes
}

/// 从 BLOB 字节反序列化多向量数据。
///
/// 与 `multivec_to_bytes` 互逆。
///
/// # 参数
/// - `bytes`: 序列化后的字节切片
///
/// # 返回
/// 多向量数据（每个 token 一个 Vec<f32>）。
pub fn multivec_from_bytes(bytes: &[u8]) -> anyhow::Result<Vec<Vec<f32>>> {
    if bytes.len() < 4 {
        anyhow::bail!("多向量数据过短：不足 4 字节 token count");
    }

    let token_count =
        u32::from_le_bytes(bytes[0..4].try_into().context("解析 token count 失败")?) as usize;
    let mut result = Vec::with_capacity(token_count);
    let mut offset = 4;

    for _ in 0..token_count {
        if offset + 4 > bytes.len() {
            anyhow::bail!("多向量数据截断：不足读取 dim");
        }
        let dim = u32::from_le_bytes(
            bytes[offset..offset + 4]
                .try_into()
                .context("解析 dim 失败")?,
        ) as usize;
        offset += 4;

        let vec_size = dim * 4;
        if offset + vec_size > bytes.len() {
            anyhow::bail!("多向量数据截断：不足读取 token 向量");
        }

        let mut vec = Vec::with_capacity(dim);
        for i in 0..dim {
            let val = f32::from_le_bytes(
                bytes[offset + i * 4..offset + i * 4 + 4]
                    .try_into()
                    .context("解析 f32 失败")?,
            );
            vec.push(val);
        }
        offset += vec_size;
        result.push(vec);
    }

    Ok(result)
}
