//! 文本分块器（REQ-VEC-001）：段落感知 + token 窗口分块。
//! 策略：优先在段落边界（`\n\n`）切分，保持语义完整性；单段超窗口时回退到
//! token 窗口切分（含重叠）。BPE 编码器为重型资源，经 `OnceLock` 全局单例化；
//! 编码/解码为 CPU 密集任务，经 `spawn_blocking` 执行。
//!
//! 段落感知分块解决「大分块语义稀释」问题（TC-RAG-001b 回归教训）：
//! 标题与其正文保持在同一 chunk，embedding 聚焦单一主题，检索质量显著提升。

use std::sync::OnceLock;

use anyhow::Context;
use tiktoken_rs::{CoreBPE, cl100k_base};

use crate::Splitter;

/// 默认分块窗口（tokens，REQ-VEC-001）
/// 256 tokens：较小的窗口使 embedding 聚焦于单一主题，
/// 避免 512 tokens 大分块导致语义稀释、检索分数偏低（TC-RAG-001b 回归教训）。
pub const DEFAULT_CHUNK_TOKENS: usize = 256;
/// 默认重叠窗口（tokens，REQ-VEC-001）
/// 仅用于单段超窗口时的 token 窗口切分；段落感知切分天然在语义边界分块，无需重叠。
pub const DEFAULT_OVERLAP_TOKENS: usize = 32;

static CL100K: OnceLock<CoreBPE> = OnceLock::new();

/// 获取全局共享的 cl100k_base 编码器；初始化失败显式返回 Err（零 expect）。
///
/// `pub` 可见性使 `echomind-compact` 等外部 crate 能复用同一 BPE 实例，
/// 避免重复加载词表（词表加载约 100ms+）。
pub fn bpe() -> anyhow::Result<&'static CoreBPE> {
    if let Some(b) = CL100K.get() {
        return Ok(b);
    }
    let encoder = cl100k_base().map_err(|e| anyhow::anyhow!("初始化 cl100k_base 失败: {e}"))?;
    Ok(CL100K.get_or_init(|| encoder))
}

/// 段落感知分块（供 `spawn_blocking` 调用）。
///
/// 1. 按 `\n\n` 分段（MarkdownLoader 保留的段落边界）
/// 2. 逐段累加，超过 token 窗口时在段落边界切分
/// 3. 单段超窗口 → 回退到 token 窗口切分（含重叠）
/// 4. 无段落分隔（纯文本/代码） → 直接 token 窗口切分
fn split_sync(
    text: &str,
    chunk_tokens: usize,
    overlap_tokens: usize,
) -> anyhow::Result<Vec<String>> {
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    let bpe = bpe()?;

    // 短文本直接返回（含标题+正文的小段落保持完整）
    let total = bpe.encode_with_special_tokens(text);
    if total.len() <= chunk_tokens {
        return Ok(vec![text.to_string()]);
    }

    // 尝试段落感知切分
    let paragraphs: Vec<&str> = text
        .split("\n\n")
        .filter(|p| !p.trim().is_empty())
        .collect();
    if paragraphs.len() <= 1 {
        // 无段落分隔 → 回退到 token 窗口切分
        return token_window_split(text, chunk_tokens, overlap_tokens);
    }

    let mut chunks = Vec::new();
    let mut current_parts: Vec<&str> = Vec::new();
    let mut current_tokens: usize = 0;

    for para in &paragraphs {
        let para_tokens = bpe.encode_with_special_tokens(para).len();

        if current_tokens + para_tokens <= chunk_tokens {
            // 段落加入当前 chunk
            current_parts.push(para);
            current_tokens += para_tokens;
        } else {
            // 当前 chunk 已满，先保存
            if !current_parts.is_empty() {
                chunks.push(current_parts.join("\n\n"));
                current_parts.clear();
                current_tokens = 0;
            }
            // 超长段落单独 token 窗口切分
            if para_tokens > chunk_tokens {
                let sub = token_window_split(para, chunk_tokens, overlap_tokens)?;
                chunks.extend(sub);
            } else {
                current_parts.push(para);
                current_tokens = para_tokens;
            }
        }
    }
    if !current_parts.is_empty() {
        chunks.push(current_parts.join("\n\n"));
    }
    Ok(chunks)
}

/// token 窗口切分（回退路径）：按固定 token 窗口 + 重叠切分。
/// 用于无段落分隔的纯文本/代码，或超长单段。
///
/// `pub(crate)` 可见性使 `semantic_splitter` 在子句仍超窗口时复用此函数。
pub(crate) fn token_window_split(
    text: &str,
    chunk_tokens: usize,
    overlap_tokens: usize,
) -> anyhow::Result<Vec<String>> {
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

/// token 窗口分块器（默认 256 tokens / 重叠 32，REQ-VEC-001）。
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
