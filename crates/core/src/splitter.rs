//! 文本分块器（REQ-VEC-001）：基于 tiktoken（cl100k_base）的 token 窗口分块。
//! BPE 编码器为重型资源，经 `OnceLock` 全局单例化；编码/解码为 CPU 密集任务，
//! 经 `spawn_blocking` 执行（体系三异步安全铁律，全程零 unwrap/expect）。

use std::sync::OnceLock;

use anyhow::Context;
use tiktoken_rs::{CoreBPE, cl100k_base};

use crate::Splitter;

/// 默认分块窗口（tokens，REQ-VEC-001）
pub const DEFAULT_CHUNK_TOKENS: usize = 512;
/// 默认重叠窗口（tokens，REQ-VEC-001）
pub const DEFAULT_OVERLAP_TOKENS: usize = 64;

static CL100K: OnceLock<CoreBPE> = OnceLock::new();

/// 获取全局共享的 cl100k_base 编码器；初始化失败显式返回 Err（零 expect）。
fn bpe() -> anyhow::Result<&'static CoreBPE> {
    if let Some(b) = CL100K.get() {
        return Ok(b);
    }
    let encoder = cl100k_base().map_err(|e| anyhow::anyhow!("初始化 cl100k_base 失败: {e}"))?;
    Ok(CL100K.get_or_init(|| encoder))
}

/// 同步分块实现（供 `spawn_blocking` 调用）。
fn split_sync(
    text: &str,
    chunk_tokens: usize,
    overlap_tokens: usize,
) -> anyhow::Result<Vec<String>> {
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    let bpe = bpe()?;
    let tokens = bpe.encode_with_special_tokens(text);
    if tokens.len() <= chunk_tokens {
        return Ok(vec![text.to_string()]);
    }
    let step = chunk_tokens - overlap_tokens;
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < tokens.len() {
        let end = (start + chunk_tokens).min(tokens.len());
        // token 窗口可能切在多字节 UTF-8 字符中间（REQ-VEC-001-AC-2 场景）：
        // 先取原始字节再 lossy 转码，保证输出必为合法 UTF-8（边界处以 U+FFFD 替代，
        // 不产生半个字符；重叠窗口使信息在相邻 chunk 中冗余，检索质量不受影响）。
        let piece_bytes = bpe
            .decode_bytes(&tokens[start..end])
            .context("token 窗口解码失败")?;
        chunks.push(String::from_utf8_lossy(&piece_bytes).into_owned());
        start += step;
    }
    Ok(chunks)
}

/// token 窗口分块器（默认 512 tokens / 重叠 64，REQ-VEC-001）。
pub struct TextSplitter {
    chunk_tokens: usize,
    overlap_tokens: usize,
}

impl TextSplitter {
    /// 以默认窗口构造；构造期即完成编码器初始化，错误前置暴露。
    pub fn new() -> anyhow::Result<Self> {
        bpe()?;
        Ok(Self {
            chunk_tokens: DEFAULT_CHUNK_TOKENS,
            overlap_tokens: DEFAULT_OVERLAP_TOKENS,
        })
    }

    /// 以自定义窗口构造（仅供 QA 测试小窗口场景）。
    #[cfg(test)]
    pub fn with_window(chunk_tokens: usize, overlap_tokens: usize) -> anyhow::Result<Self> {
        if overlap_tokens >= chunk_tokens {
            anyhow::bail!("重叠窗口必须小于分块窗口");
        }
        bpe()?;
        Ok(Self {
            chunk_tokens,
            overlap_tokens,
        })
    }

    /// 统计文本 token 数。
    pub fn count_tokens(&self, text: &str) -> anyhow::Result<usize> {
        Ok(bpe()?.encode_with_special_tokens(text).len())
    }
}

impl Splitter for TextSplitter {
    async fn split(&self, text: &str) -> anyhow::Result<Vec<String>> {
        let owned = text.to_string();
        let (chunk_tokens, overlap_tokens) = (self.chunk_tokens, self.overlap_tokens);
        tokio::task::spawn_blocking(move || split_sync(&owned, chunk_tokens, overlap_tokens))
            .await
            .context("分块任务执行失败")?
    }
}
