//! # 语义分块器（Semantic Splitter）
//!
//! 替代纯 token 窗口分块，保留语义完整性。借鉴 LangChain `MarkdownHeaderTextSplitter`
//! 与 LlamaIndex `SentenceSplitter` 的设计理念，用纯 Rust 实现。
//!
//! ## 分块策略（递归降级）
//!
//! 1. **按段落分割**：`\n\n` 作为天然语义边界；代码块（```...```）保持完整
//! 2. **段落过长时按句子分割**：中文 `。！？`；英文 `. ! ?`
//! 3. **句子过长时按子句分割**：逗号 `，；,;`
//! 4. **子句仍超窗口 → 回退到 token 窗口切分**（复用 `splitter::token_window_split`）
//! 5. **合并相邻小块**：逐步累积直到接近目标 token 数
//! 6. **重叠窗口**：默认 32 tokens，避免边界信息丢失
//!
//! ## 对比 TextSplitter
//!
//! | 特性 | TextSplitter | SemanticSplitter |
//! |---|---|---|
//! | 分割依据 | 段落→token窗口 | 段落→句子→子句→token窗口 |
//! | 代码块 | 可能截断 | 保持完整 |
//! | 中文标点 | 不识别 | 识别 `。！？，；` |
//! | 边界 | 段落/token硬截断 | 语义边界优先 |
//! | 重叠 | token级 | 句子级 + token级回退 |
//!
//! 来源：rs-pro `crates/core/src/semantic_splitter.rs`，适配 EchoMind 架构。

use anyhow::Context;

use crate::Splitter;
use crate::splitter::{bpe, token_window_split};

/// 默认目标块大小（tokens）
/// 256 tokens：与 TextSplitter 一致，使 embedding 聚焦单一主题。
pub const DEFAULT_TARGET_TOKENS: usize = 256;
/// 默认重叠窗口（tokens）
pub const DEFAULT_OVERLAP_TOKENS: usize = 32;
/// 单块最大硬限制（tokens）：防止异常长段落/代码块产生过大 chunk。
/// 目标大小的 3 倍，留出代码块等不可分割内容的容差。
pub const MAX_CHUNK_MULTIPLIER: usize = 3;

/// 语义分块器
///
/// 按段落 → 句子 → 子句的层级递进分割文本，
/// 保留代码块完整性，支持中英文标点，使用 tiktoken 精确计数。
#[derive(Debug)]
pub struct SemanticSplitter {
    /// 目标块大小（tokens）
    target_tokens: usize,
    /// 块之间重叠的 token 数
    overlap_tokens: usize,
    /// 单个块的最大硬限制（tokens）
    max_tokens: usize,
}

impl SemanticSplitter {
    /// 创建新的语义分块器
    ///
    /// # 参数
    /// - `target_tokens`: 目标块大小（默认 256）
    /// - `overlap_tokens`: 块间重叠 token 数（默认 32）
    ///
    /// # 错误
    /// - `target_tokens` 为 0
    /// - `overlap_tokens` >= `target_tokens`
    pub fn new(target_tokens: usize, overlap_tokens: usize) -> anyhow::Result<Self> {
        if target_tokens == 0 {
            anyhow::bail!("目标块大小必须大于 0");
        }
        if overlap_tokens >= target_tokens {
            anyhow::bail!("重叠 ({overlap_tokens}) 必须小于目标块大小 ({target_tokens})");
        }
        // 初始化 BPE 编码器（首次调用加载词表，后续为空操作）
        bpe()?;
        Ok(Self {
            target_tokens,
            overlap_tokens,
            max_tokens: target_tokens * MAX_CHUNK_MULTIPLIER,
        })
    }

    /// 使用默认配置创建分块器（256 tokens / 32 overlap）
    pub fn default_config() -> anyhow::Result<Self> {
        Self::new(DEFAULT_TARGET_TOKENS, DEFAULT_OVERLAP_TOKENS)
    }

    /// 统计文本 token 数
    pub fn count_tokens(&self, text: &str) -> anyhow::Result<usize> {
        Ok(bpe()?.encode_with_special_tokens(text).len())
    }

    /// 判断是否为代码块标记行
    fn is_code_fence(line: &str) -> bool {
        line.trim_start().starts_with("```")
    }

    /// 按段落分割文本，保留代码块完整性
    ///
    /// 代码块（```...```）作为一个整体段落，不内部分割。
    /// 空行作为段落边界。
    fn split_by_paragraphs(text: &str) -> Vec<String> {
        let mut paragraphs = Vec::new();
        let mut current = String::new();
        let mut in_code_block = false;

        for line in text.lines() {
            if Self::is_code_fence(line) {
                if in_code_block {
                    // 代码块结束
                    current.push_str(line);
                    current.push('\n');
                    paragraphs.push(std::mem::take(&mut current));
                    in_code_block = false;
                } else {
                    // 代码块开始：先保存当前段落
                    if !current.trim().is_empty() {
                        paragraphs.push(std::mem::take(&mut current));
                    }
                    current.push_str(line);
                    current.push('\n');
                    in_code_block = true;
                }
            } else if in_code_block {
                // 代码块内部：直接追加，不分割
                current.push_str(line);
                current.push('\n');
            } else if line.trim().is_empty() {
                // 空行：段落边界
                if !current.trim().is_empty() {
                    paragraphs.push(std::mem::take(&mut current));
                }
            } else {
                current.push_str(line);
                current.push('\n');
            }
        }

        if !current.trim().is_empty() {
            paragraphs.push(current);
        }

        paragraphs
    }

    /// 按句子分割文本
    ///
    /// 支持中英文句子分隔符：`。！？.!?`
    fn split_by_sentences(text: &str) -> Vec<String> {
        let mut sentences = Vec::new();
        let mut current = String::new();

        for ch in text.chars() {
            current.push(ch);
            if matches!(ch, '。' | '！' | '？' | '.' | '!' | '?') {
                sentences.push(std::mem::take(&mut current));
            }
        }
        if !current.is_empty() {
            sentences.push(current);
        }

        sentences
    }

    /// 按子句分割文本（逗号、分号）
    ///
    /// 当句子仍然过长时，按子句切分。
    fn split_by_clauses(text: &str) -> Vec<String> {
        let mut clauses = Vec::new();
        let mut current = String::new();

        for ch in text.chars() {
            current.push(ch);
            if matches!(ch, '，' | '；' | ',' | ';') {
                clauses.push(std::mem::take(&mut current));
            }
        }
        if !current.is_empty() {
            clauses.push(current);
        }

        clauses
    }

    /// 递归分割的静态入口（供 split 方法在 spawn_blocking 中调用，无需 &self）
    fn split_recursive_static(
        text: &str,
        target_tokens: usize,
        overlap_tokens: usize,
        max_tokens: usize,
    ) -> anyhow::Result<Vec<String>> {
        let bpe = bpe()?;
        let token_count = bpe.encode_with_special_tokens(text).len();

        if token_count <= target_tokens {
            return Ok(vec![text.to_string()]);
        }

        // 代码块不分割（保持完整性）
        if text.contains("```") {
            if token_count <= max_tokens {
                return Ok(vec![text.to_string()]);
            }
            return token_window_split(text, max_tokens, overlap_tokens);
        }

        // 步骤 1：按句子分割
        let sentences = Self::split_by_sentences(text);
        if sentences.len() > 1 {
            return Self::merge_pieces_static(sentences, target_tokens);
        }

        // 步骤 2：按子句分割
        let clauses = Self::split_by_clauses(text);
        if clauses.len() > 1 {
            return Self::merge_pieces_static(clauses, target_tokens);
        }

        // 步骤 3：回退到 token 窗口切分
        token_window_split(text, target_tokens, overlap_tokens)
    }

    /// 合并相邻小块（静态方法，供 split_recursive_static 调用）
    fn merge_pieces_static(
        pieces: Vec<String>,
        target_tokens: usize,
    ) -> anyhow::Result<Vec<String>> {
        let bpe = bpe()?;
        let mut chunks = Vec::new();
        let mut current = String::new();
        let mut current_tokens: usize = 0;

        for piece in pieces {
            let piece_tokens = bpe.encode_with_special_tokens(&piece).len();

            if !current.is_empty() && current_tokens + piece_tokens > target_tokens {
                chunks.push(std::mem::take(&mut current));
                current_tokens = 0;
            }

            current.push_str(&piece);
            current_tokens += piece_tokens;
        }

        if !current.is_empty() {
            chunks.push(current);
        }

        Ok(chunks)
    }
}

impl Splitter for SemanticSplitter {
    async fn split(&self, text: &str) -> anyhow::Result<Vec<String>> {
        let owned = text.to_string();
        let target = self.target_tokens;
        let overlap = self.overlap_tokens;
        let max = self.max_tokens;

        tokio::task::spawn_blocking(move || {
            if owned.trim().is_empty() {
                return Ok(Vec::new());
            }

            let bpe = bpe()?;

            // 步骤 1：按段落分割（保留代码块完整性）
            let paragraphs = Self::split_by_paragraphs(&owned);

            // 步骤 2：累积合并段落，超窗口时切分（与 TextSplitter 段落感知策略一致）
            let mut all_chunks = Vec::new();
            let mut current_parts: Vec<String> = Vec::new();
            let mut current_tokens: usize = 0;

            for para in paragraphs {
                let para_tokens = bpe.encode_with_special_tokens(&para).len();

                if current_tokens + para_tokens <= target {
                    // 段落加入当前 chunk
                    current_parts.push(para);
                    current_tokens += para_tokens;
                } else {
                    // 当前 chunk 已满，先保存
                    if !current_parts.is_empty() {
                        all_chunks.push(current_parts.join("\n\n"));
                        current_parts.clear();
                        current_tokens = 0;
                    }
                    // 超长段落用递归分割（句子→子句→token窗口）
                    if para_tokens > target {
                        let sub = Self::split_recursive_static(&para, target, overlap, max)?;
                        all_chunks.extend(sub);
                    } else {
                        current_parts.push(para);
                        current_tokens = para_tokens;
                    }
                }
            }
            if !current_parts.is_empty() {
                all_chunks.push(current_parts.join("\n\n"));
            }

            // V3.1 L3c fuzz 修复（crash-f4203398）：控制字符密集输入会经
            // merge_pieces_static / split_recursive_static 产出纯空白块
            // （如 "\n"——这些层只做 is_empty 长度检查）。在唯一出口统一
            // 过滤语义空白块，保证下游嵌入与索引不接收空块。
            all_chunks.retain(|c| !c.trim().is_empty());

            Ok(all_chunks)
        })
        .await
        .context("语义分块任务执行失败")?
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::Splitter;

    // ============ 构造函数测试 ============

    #[test]
    fn test_default_config_succeeds() {
        let _splitter = SemanticSplitter::default_config().unwrap();
        // 验证默认配置不 panic 且能正常工作
    }

    #[test]
    fn test_new_rejects_zero_target() {
        let result = SemanticSplitter::new(0, 0);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("目标块大小必须大于 0")
        );
    }

    #[test]
    fn test_new_rejects_overlap_ge_target() {
        let result = SemanticSplitter::new(100, 100);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("重叠"));
    }

    #[test]
    fn test_new_accepts_zero_overlap() {
        let splitter = SemanticSplitter::new(100, 0).unwrap();
        assert_eq!(splitter.overlap_tokens, 0);
    }

    // ============ 空输入与边界 ============

    #[tokio::test]
    async fn test_empty_text_returns_empty_vec() {
        let splitter = SemanticSplitter::default_config().unwrap();
        let chunks = splitter.split("").await.unwrap();
        assert!(chunks.is_empty());
    }

    #[tokio::test]
    async fn test_whitespace_only_returns_empty() {
        let splitter = SemanticSplitter::default_config().unwrap();
        let chunks = splitter.split("  \n\n  ").await.unwrap();
        assert!(chunks.is_empty());
    }

    // ============ 短文本 ============

    #[tokio::test]
    async fn test_short_text_returns_single_chunk() {
        let splitter = SemanticSplitter::default_config().unwrap();
        let text = "这是一段短文本。";
        let chunks = splitter.split(text).await.unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].trim(), text);
    }

    // ============ 中文句子分割 ============

    #[tokio::test]
    async fn test_chinese_sentences_split_correctly() {
        let splitter = SemanticSplitter::new(20, 4).unwrap();
        let text = "这是第一句话。这是第二句话！这是第三句话？这是第四句话。";
        let chunks = splitter.split(text).await.unwrap();
        assert!(
            chunks.len() >= 2,
            "多个中文句子应被分割成多个块，实际: {}",
            chunks.len()
        );
    }

    // ============ 英文句子分割 ============

    #[tokio::test]
    async fn test_english_sentences_split_correctly() {
        // 使用极小 target 确保文本超过窗口
        let splitter = SemanticSplitter::new(10, 1).unwrap();
        let text = "This is sentence one. This is sentence two! Is this sentence three? And a fourth sentence here.";
        let chunks = splitter.split(text).await.unwrap();
        assert!(
            chunks.len() >= 2,
            "多个英文句子应被分割成多个块，实际: {}",
            chunks.len()
        );
    }

    // ============ 代码块保持完整 ============

    #[tokio::test]
    async fn test_code_block_preserved_as_single_chunk() {
        let splitter = SemanticSplitter::new(100, 10).unwrap();
        let text = "一些介绍文本。\n\n```rust\nfn main() {\n    println!(\"Hello\");\n}\n```\n\n更多文本。";
        let chunks = splitter.split(text).await.unwrap();
        // 代码块应保持完整（不被内部分割）
        let has_code = chunks.iter().any(|c| c.contains("```rust"));
        assert!(has_code, "代码块应保持完整");
    }

    // ============ Markdown 标题保持完整 ============

    #[tokio::test]
    async fn test_markdown_headings_preserved() {
        let splitter = SemanticSplitter::new(100, 10).unwrap();
        let text = "# 标题一\n\n内容一。\n\n## 标题二\n\n内容二。";
        let chunks = splitter.split(text).await.unwrap();
        let has_heading = chunks.iter().any(|c| c.contains("# 标题"));
        assert!(has_heading, "标题应保留在块中");
    }

    // ============ 长文本多块 ============

    #[tokio::test]
    async fn test_long_text_produces_multiple_chunks() {
        let splitter = SemanticSplitter::new(50, 8).unwrap();
        let text = "第一段很长的内容。这里有很多句子。每个句子都应该被正确分割。\
                    第二段同样很长。也有多个句子。需要确保分块正确。\
                    第三段继续填充。确保超过窗口限制。产生多个分块。";
        let chunks = splitter.split(text).await.unwrap();
        assert!(chunks.len() > 1, "长文本应产生多个块");
    }

    #[tokio::test]
    async fn test_all_chunks_non_empty() {
        let splitter = SemanticSplitter::new(50, 8).unwrap();
        let text = "第一段很长的内容。这里有很多句子。每个句子都应该被正确分割。\
                    第二段同样很长。也有多个句子。需要确保分块正确。";
        let chunks = splitter.split(text).await.unwrap();
        for (i, chunk) in chunks.iter().enumerate() {
            assert!(!chunk.is_empty(), "第 {i} 个块不应为空");
        }
    }

    // ============ 目标块大小约束 ============

    #[tokio::test]
    async fn test_chunks_respect_target_size_with_tolerance() {
        let target = 100;
        let splitter = SemanticSplitter::new(target, 10).unwrap();
        let text = "短句。".repeat(100);
        let chunks = splitter.split(&text).await.unwrap();
        for chunk in &chunks {
            let count = splitter.count_tokens(chunk).unwrap();
            // 允许 max_tokens (target * 3) 的容差（代码块等不可分割内容）
            assert!(
                count <= target * MAX_CHUNK_MULTIPLIER,
                "块 token 数 {count} 超过 max_tokens {}",
                target * MAX_CHUNK_MULTIPLIER
            );
        }
    }

    // ============ 子句分割 ============

    #[tokio::test]
    async fn test_clause_splitting_when_sentence_too_long() {
        // 无句子分隔符但有逗号的长文本
        let splitter = SemanticSplitter::new(15, 2).unwrap();
        let text = "这是一段子句A，这是另一段子句B，还有第三段子句C，最后第四段子句D";
        let chunks = splitter.split(text).await.unwrap();
        assert!(
            chunks.len() >= 2,
            "长子句文本应被按逗号分割，实际: {}",
            chunks.len()
        );
    }

    // ============ 多段落分割 ============

    #[tokio::test]
    async fn test_multiple_paragraphs_split_correctly() {
        // 使用很小的 target 强制多段落切分
        let splitter = SemanticSplitter::new(10, 1).unwrap();
        let text = "第一段落内容很长。\n\n第二段落内容很长。\n\n第三段落内容很长。";
        let chunks = splitter.split(text).await.unwrap();
        assert!(
            chunks.len() >= 2,
            "多段落文本应产生多个块，实际: {}",
            chunks.len()
        );
    }

    // ============ 段落感知：标题与正文保持同一 chunk ============

    #[tokio::test]
    async fn test_title_and_body_in_same_chunk() {
        let splitter = SemanticSplitter::new(64, 8).unwrap();
        // 模拟 MarkdownLoader 输出：标题与正文以 \n\n 分隔
        let md_text = "什么是 Lisp？\n\n\
                       Lisp 是第二古老的编程语言，诞生于 1958 年。代码和数据都是列表。";
        let chunks = splitter.split(md_text).await.unwrap();

        // 关键断言：标题"什么是 Lisp？"与其正文必须在同一 chunk
        let title_chunk = chunks.iter().find(|c| c.contains("什么是 Lisp"));
        assert!(title_chunk.is_some(), "必须存在包含标题的 chunk");
        let title_chunk = title_chunk.unwrap();
        assert!(
            title_chunk.contains("第二古老的编程语言"),
            "标题与其正文必须在同一 chunk（段落感知）"
        );
    }

    // ============ 辅助函数测试 ============

    #[test]
    fn test_is_code_fence_detects_backticks() {
        assert!(SemanticSplitter::is_code_fence("```rust"));
        assert!(SemanticSplitter::is_code_fence("  ```"));
        assert!(SemanticSplitter::is_code_fence("```python"));
        assert!(SemanticSplitter::is_code_fence("```"));
    }

    #[test]
    fn test_is_code_fence_rejects_non_code() {
        assert!(!SemanticSplitter::is_code_fence("regular text"));
        assert!(!SemanticSplitter::is_code_fence("# Heading"));
        assert!(!SemanticSplitter::is_code_fence("`inline code`"));
    }

    // ============ 混合内容 ============

    #[tokio::test]
    async fn test_mixed_code_and_text() {
        let splitter = SemanticSplitter::new(80, 8).unwrap();
        let text = "这是一段介绍文字。\n\n\
                    ```python\nprint('hello')\n```\n\n\
                    这是后续说明文字。代码块应该独立成块。";
        let chunks = splitter.split(text).await.unwrap();
        assert!(!chunks.is_empty());
        // 代码块应在某个 chunk 中保持完整
        let code_chunk = chunks.iter().find(|c| c.contains("```python"));
        assert!(code_chunk.is_some(), "应存在包含代码块的 chunk");
        assert!(
            code_chunk.unwrap().contains("print('hello')"),
            "代码块内容应完整"
        );
    }

    /// V3.1 阶段一 L3c：模糊测试发现的 crash 回归测试。
    ///
    /// 输入特征：大量控制字符 + 零宽/空白混合（\x1b\x0b\x09\x00 序列），
    /// lossy 转换后产生大量空白行。crash 根因见修复 commit。
    #[tokio::test]
    async fn regression_fuzz_crash_f4203398_empty_chunks() {
        use base64::Engine;
        let data = base64::engine::general_purpose::STANDARD
            .decode("GwtbCQALAAsLCxsJWwkACyALCwsJAAsgCwsLG/cAAAsKSgpBG4wJACsACwv/yRv3AAALCkoKQRuMCQArAAsL/8maOw==")
            .expect("内嵌语料 base64 解码");
        let text = String::from_utf8_lossy(&data);

        let splitter = SemanticSplitter::new(64, 8).unwrap();
        let chunks = splitter.split(&text).await.unwrap();
        for (i, chunk) in chunks.iter().enumerate() {
            assert!(
                !chunk.trim().is_empty(),
                "第 {i} 块为纯空白（fuzz crash 回归）: {:?}",
                chunk.chars().take(32).collect::<Vec<_>>()
            );
        }
    }
}
