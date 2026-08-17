//! Late Chunking 上下文感知嵌入（REQ-RAG-049）
//!
//! 借鉴 Jina AI 2024 Late Chunking 技术（arXiv:2409.04701），在文档导入嵌入阶段
//! 为每个 chunk 注入文档级上下文前缀（前 N 字符摘要），使 chunk 嵌入向量包含
//! 全文语义上下文，显著提升检索质量。
//!
//! # 原理
//!
//! 传统方法：先分块 → 每块独立嵌入 → 每个 chunk 丢失文档上下文。
//! Late Chunking：先嵌入全文获取 contextualized token 表示 → 按 chunk 边界
//! 提取每段的 mean-pooled 向量 → 每个 chunk 向量都包含全文上下文。
//!
//! # EchoMind 适配
//!
//! fastembed/ONNX API 只返回最终 mean-pooled 向量，不暴露 token 级嵌入。
//! 因此采用**近似 Late Chunking**：为每个 chunk 嵌入时，拼接文档前 N 字符
//! 作为上下文前缀，使嵌入模型在编码 chunk 时能"看到"文档全局主题。
//!
//! 与 REQ-RAG-041 Contextual Retrieval（仅拼接文档名）不同，Late Chunking
//! 拼接文档前 500 字符作为上下文摘要，提供更丰富的文档级语义。
//!
//! # 与 Contextual Retrieval 的关系
//!
//! 两者可组合使用：
//! - Contextual Retrieval：`文档《{doc_name}》：\n{chunk_content}`
//! - Late Chunking：`{doc_prefix}\n\n---\n\n{chunk_content}`
//! - 组合：`{doc_prefix}\n\n---\n\n文档《{doc_name}》：\n{chunk_content}`
//!
//! # 零 LLM 依赖
//!
//! 纯规则方案：`extract_doc_prefix()` 从文档文本中提取前 N 字符，不调用 LLM。
//! 可在导入时自动应用，零网络请求、零 API 费用。

use serde::{Deserialize, Serialize};

/// Late Chunking 配置（REQ-RAG-049）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LateChunkingConfig {
    /// 文档前缀最大字符数（默认 500）
    ///
    /// 从文档文本中提取前 N 字符作为上下文前缀。
    /// 500 字符 ≈ 125-250 tokens（取决于语言），不会显著增加嵌入耗时。
    pub max_prefix_chars: usize,
    /// 前缀与 chunk 内容之间的分隔符
    ///
    /// 使用 `\n\n---\n\n` 使嵌入模型能清晰区分前缀和 chunk 内容。
    pub separator: String,
}

impl Default for LateChunkingConfig {
    fn default() -> Self {
        Self {
            max_prefix_chars: 500,
            separator: "\n\n---\n\n".to_string(),
        }
    }
}

impl LateChunkingConfig {
    /// 创建默认配置
    pub fn new() -> Self {
        Self::default()
    }

    /// 创建自定义配置
    pub fn with_config(max_prefix_chars: usize, separator: &str) -> Self {
        Self {
            max_prefix_chars,
            separator: separator.to_string(),
        }
    }

    /// 判断是否应应用 Late Chunking
    ///
    /// 当文档前缀非空时才应用（空文档或空前缀不拼接）
    pub fn should_apply(doc_prefix: &str) -> bool {
        !doc_prefix.trim().is_empty()
    }
}

/// 从文档文本中提取前 N 字符作为上下文前缀（REQ-RAG-049 AC-2/AC-3/AC-4）
///
/// 提取策略：
/// 1. 空文本 → 返回空字符串
/// 2. 短文本（≤ max_chars）→ 返回全文
/// 3. 长文本 → 截取前 max_chars 字符，尽量在段落边界（`\n\n`）或句子边界处截断
///
/// # 参数
/// - `text`: 文档全文（或已加载的纯文本内容）
/// - `max_chars`: 最大字符数（默认 500）
///
/// # 返回
/// 文档前缀字符串，长度 ≤ max_chars
pub fn extract_doc_prefix(text: &str, max_chars: usize) -> String {
    if text.trim().is_empty() || max_chars == 0 {
        return String::new();
    }

    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max_chars {
        return text.to_string();
    }

    // 截取前 max_chars 字符
    let prefix_chars: String = chars[..max_chars].iter().collect();

    // 尝试在段落边界（\n\n）截断
    if let Some(pos) = prefix_chars.rfind("\n\n") {
        // 确保截断后内容不为空
        if pos > max_chars / 2 {
            return chars[..pos].iter().collect();
        }
    }

    // 尝试在句子边界截断（中文句号/英文句号/换行）
    for delimiter in &['。', '.', '！', '!', '？', '?', '\n'] {
        if let Some(pos) = prefix_chars.rfind(*delimiter) {
            // 确保截断后内容不为空且包含分隔符
            let char_pos = pos + delimiter.len_utf8();
            if char_pos > max_chars / 2 {
                return chars[..char_pos].iter().collect();
            }
        }
    }

    // 无法在边界截断，直接返回前 max_chars 字符
    prefix_chars
}

/// 构建 Late Chunking 嵌入文本（REQ-RAG-049 AC-1/AC-5）
///
/// 将文档前缀摘要与 chunk 内容拼接，中间用分隔符隔开，
/// 使嵌入模型在编码 chunk 时能感知文档全局上下文。
///
/// # 参数
/// - `doc_prefix`: 文档前缀摘要（由 `extract_doc_prefix` 提取）
/// - `chunk_content`: chunk 原始内容
///
/// # 返回
/// 拼接后的嵌入文本：`{doc_prefix}{separator}{chunk_content}`
///
/// # 示例
/// ```
/// use echomind_core::late_chunking::{build_late_chunking_text, extract_doc_prefix};
///
/// let doc_text = "本文档介绍 Rust 异步编程。async fn 是核心关键字。";
/// let prefix = extract_doc_prefix(doc_text, 500);
/// let chunk = "tokio::spawn 创建异步任务。";
/// let embedding_text = build_late_chunking_text(&prefix, chunk);
/// assert!(embedding_text.contains(&prefix));
/// assert!(embedding_text.contains(chunk));
/// ```
pub fn build_late_chunking_text(doc_prefix: &str, chunk_content: &str) -> String {
    if doc_prefix.trim().is_empty() {
        return chunk_content.to_string();
    }

    let config = LateChunkingConfig::default();
    format!("{}{}{}", doc_prefix, config.separator, chunk_content)
}

/// 使用自定义配置构建 Late Chunking 嵌入文本
///
/// 与 `build_late_chunking_text` 相同，但使用自定义配置（分隔符、前缀长度）
pub fn build_late_chunking_text_with_config(
    doc_prefix: &str,
    chunk_content: &str,
    config: &LateChunkingConfig,
) -> String {
    if doc_prefix.trim().is_empty() {
        return chunk_content.to_string();
    }

    format!("{}{}{}", doc_prefix, config.separator, chunk_content)
}
