//! ONNX 模型压缩器（Pro feature，REQ-PERF-002）。
//!
//! 使用已有 ONNX embedder（all-MiniLM-L6-v2）做句子级嵌入相似度评分，
//! 按与 query 的余弦相似度排序保留 top-N 句，实现更精准的 prompt 压缩。
//!
//! ## 与 LLMLingua-2 的关系
//!
//! LLMLingua-2 (arXiv:2403.12968) 使用 XLM-RoBERTa 做 token 级保留/删除分类。
//! 本实现为嵌入式方案（embedding-based），使用已有 embedder 评分句子级相关性，
//! 无需额外下载 ~1.1GB 模型，以更低的成本实现相近的压缩效果。
//!
//! ## 压缩流程
//!
//! 1. 分割代码块（保护代码结构）
//! 2. 非代码文本按句子分割
//! 3. 嵌入每句 + 嵌入 query
//! 4. 计算每句与 query 的余弦相似度
//! 5. 按相似度降序保留 top-N 句（N = 总句数 / ratio）
//! 6. 按原始顺序重排保留的句子

#![cfg(feature = "pro")]

use echomind_core::Embedder as _;
use echomind_core::PromptCompressor;

use crate::local_embedder::LocalEmbedder;

/// ONNX 嵌入式压缩器（Pro feature）。
///
/// 使用 `LocalEmbedder` 做句子级嵌入相似度评分，
/// 按与 query 的相似度保留 top-N 句实现压缩。
///
/// # 与 RuleBasedCompressor 的区别
///
/// - `RuleBasedCompressor`（Free）：停用词去除 + word overlap 评分，零依赖
/// - `OnnxCompressor`（Pro）：嵌入余弦相似度评分，精确度更高但需 embedder 初始化
pub struct OnnxCompressor {
    /// 嵌入器（`LocalEmbedder` 已实现 `Clone`，内部 `Arc` 开销极低）
    embedder: LocalEmbedder,
}

impl OnnxCompressor {
    /// 创建新的 ONNX 压缩器。
    ///
    /// # 参数
    /// - `embedder`: 已初始化的 `LocalEmbedder` 实例
    pub fn new(embedder: LocalEmbedder) -> Self {
        Self { embedder }
    }

    /// 分割文本为句子。
    ///
    /// 按句末标点（. ! ? 。 ！ ？ ; ；）分割。
    fn split_sentences(text: &str) -> Vec<String> {
        let mut sentences = Vec::new();
        let mut current = String::new();
        for ch in text.chars() {
            current.push(ch);
            if matches!(ch, '.' | '!' | '?' | '。' | '！' | '？' | ';' | '；') {
                let trimmed = current.trim().to_string();
                if !trimmed.is_empty() {
                    sentences.push(trimmed);
                }
                current.clear();
            }
        }
        let remaining = current.trim();
        if !remaining.is_empty() {
            sentences.push(remaining.to_string());
        }
        sentences
    }

    /// 分割代码块和非代码段。
    fn split_code_blocks(text: &str) -> Vec<(String, bool)> {
        let mut segments = Vec::new();
        let mut current_text = String::new();
        let mut in_code = false;
        let mut code_content = String::new();

        for line in text.lines() {
            let trimmed = line.trim_start();
            if !in_code && trimmed.starts_with("```") {
                if !current_text.is_empty() {
                    segments.push((current_text.trim_end_matches('\n').to_string(), false));
                    current_text.clear();
                }
                in_code = true;
                continue;
            } else if in_code && trimmed.starts_with("```") {
                segments.push((code_content.trim_end_matches('\n').to_string(), true));
                code_content.clear();
                in_code = false;
                continue;
            }
            if in_code {
                code_content.push_str(line);
                code_content.push('\n');
            } else {
                current_text.push_str(line);
                current_text.push('\n');
            }
        }
        if in_code {
            segments.push((code_content.trim_end_matches('\n').to_string(), true));
        } else if !current_text.is_empty() {
            segments.push((current_text.trim_end_matches('\n').to_string(), false));
        }
        segments
    }

    /// 去除冗余空白。
    fn collapse_whitespace(text: &str) -> String {
        let mut result = String::with_capacity(text.len());
        let mut prev_was_space = false;
        let mut prev_was_newline = false;
        for ch in text.chars() {
            if ch == '\n' {
                if !prev_was_newline {
                    result.push('\n');
                }
                prev_was_newline = true;
                prev_was_space = false;
            } else if ch.is_whitespace() {
                if !prev_was_space && !prev_was_newline {
                    result.push(' ');
                }
                prev_was_space = true;
            } else {
                result.push(ch);
                prev_was_space = false;
                prev_was_newline = false;
            }
        }
        result.trim().to_string()
    }

    /// 去除代码块内的注释。
    fn strip_code_comments(code: &str) -> String {
        let mut result = String::with_capacity(code.len());
        let mut chars = code.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '/' && chars.peek() == Some(&'/') {
                chars.next();
                for c in chars.by_ref() {
                    if c == '\n' {
                        result.push('\n');
                        break;
                    }
                }
            } else if ch == '/' && chars.peek() == Some(&'*') {
                chars.next();
                let mut prev = '\0';
                for c in chars.by_ref() {
                    if prev == '*' && c == '/' {
                        break;
                    }
                    prev = c;
                }
            } else {
                result.push(ch);
            }
        }
        result
    }

    /// 计算余弦相似度。
    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
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
}

impl PromptCompressor for OnnxCompressor {
    fn compress<'a>(
        &'a self,
        text: &'a str,
        ratio: f32,
        query: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<String>> + Send + 'a>>
    {
        Box::pin(async move {
            if ratio <= 1.0 {
                return Ok(text.to_string());
            }

            // 1. 分割代码块和非代码段
            let segments = Self::split_code_blocks(text);

            // 2. 嵌入 query
            let query_embedding = self.embedder.embed(query).await?;

            // 3. 处理每个段
            let mut compressed_segments = Vec::new();
            for (content, is_code) in &segments {
                if *is_code {
                    // 代码块：去除注释，保留结构
                    let cleaned = Self::strip_code_comments(content);
                    let cleaned = Self::collapse_whitespace(&cleaned);
                    compressed_segments.push(format!("```\n{cleaned}\n```"));
                } else {
                    // 非代码文本：句子级嵌入相似度评分
                    let sentences = Self::split_sentences(content);
                    if sentences.is_empty() {
                        compressed_segments.push(content.clone());
                        continue;
                    }

                    // 计算保留句子数：至少保留 sqrt 比例
                    let min_ratio = 1.0 / ratio.sqrt();
                    let min_keep = ((sentences.len() as f32 * min_ratio).ceil() as usize).max(1);
                    let keep_count = ((sentences.len() as f32 / ratio).ceil() as usize)
                        .max(min_keep)
                        .min(sentences.len());

                    // 批量嵌入句子
                    let sentence_refs: Vec<String> = sentences.clone();
                    let embeddings = self.embedder.embed_batch(&sentence_refs).await?;

                    // 评分
                    let mut scored: Vec<(usize, f32)> = embeddings
                        .iter()
                        .enumerate()
                        .map(|(idx, emb)| (idx, Self::cosine_similarity(emb, &query_embedding)))
                        .collect();

                    // 按分数降序排序
                    scored
                        .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

                    // 保留 top-N
                    let mut kept: Vec<(usize, String)> = Vec::new();
                    for (idx, _score) in scored.iter().take(keep_count) {
                        kept.push((*idx, sentences[*idx].clone()));
                    }

                    // 按原始顺序重排
                    kept.sort_by_key(|(idx, _)| *idx);
                    let compressed_text = kept
                        .iter()
                        .map(|(_, s)| s.as_str())
                        .collect::<Vec<_>>()
                        .join(" ");
                    compressed_segments.push(compressed_text);
                }
            }

            Ok(compressed_segments.join("\n\n"))
        })
    }
}
