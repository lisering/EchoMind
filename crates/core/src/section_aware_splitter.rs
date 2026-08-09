//! 章节感知分块器（Section-Aware Splitter，REQ-VEC-006）。
//!
//! 在 SemanticSplitter 前置结构感知层：识别 Markdown 标题层级（`#`/`##`/`###`），
//! 将「标题 + 其下正文」作为同一 section；section 内部委托 SemanticSplitter 切分；
//! 每 chunk 前缀追加章节路径标记（如 `[引言 > 1.1 背景]`），使 embedding 与
//! LLM 均能感知片段在文档中的结构位置。
//!
//! 无标题的纯文本退化为 SemanticSplitter 行为（AC-3）。
//!
//! 调研依据：LangChain `MarkdownHeaderTextSplitter`（按标题层级分块 + metadata）；
//! LlamaIndex `SentenceSplitter`（段落优先策略）；Anthropic Contextual Retrieval 博文
//!（chunk 上下文增强使检索失败率 ↓49%）。

use anyhow::{Context, Result};
use pulldown_cmark::{Event, Parser, Tag, TagEnd};

use crate::Splitter;
use crate::semantic_splitter::SemanticSplitter;

/// 一个 section：章节路径 + section 内的正文（含标题文本 + 正文段落）。
#[derive(Default)]
struct Section {
    /// 章节路径，如 "引言 > 1.1 背景"（标题栈以 ` > ` 连接）
    path: String,
    /// section 内的正文（含标题文本 + 正文段落，用 `\n\n` 分隔段落边界）
    content: String,
}

/// 章节感知分块器：SemanticSplitter + 标题层级结构感知。
///
/// 组合模式：持有 SemanticSplitter 实例，section 内部委托其切分。
/// 对含 Markdown 标题的文档，按 section 切分并追加章节路径前缀；
/// 对无标题的纯文本，退化为 SemanticSplitter 行为（AC-3）。
#[derive(Debug)]
pub struct SectionAwareSplitter {
    /// 内部分块器：section 内部的语义分块
    inner: SemanticSplitter,
}

impl SectionAwareSplitter {
    /// 以默认配置创建（256 tokens / 32 overlap，与 SemanticSplitter 一致）。
    pub fn default_config() -> Result<Self> {
        Ok(Self {
            inner: SemanticSplitter::default_config()?,
        })
    }

    /// 以自定义 token 窗口创建（GB 级文档加速：大文件用 1024 tokens 减少 chunk 总量 4x）。
    ///
    /// # 参数
    /// - `target_tokens`: 目标块大小（tokens）。256=精确检索，1024=大文档加速
    /// - `overlap_tokens`: 块间重叠（建议 target_tokens 的 1/8~1/4）
    pub fn new_with_config(target_tokens: usize, overlap_tokens: usize) -> Result<Self> {
        Ok(Self {
            inner: SemanticSplitter::new(target_tokens, overlap_tokens)?,
        })
    }

    /// 统计文本 token 数（委托给内部分块器）。
    pub fn count_tokens(&self, text: &str) -> Result<usize> {
        self.inner.count_tokens(text)
    }

    /// 用 pulldown-cmark 解析 markdown，按标题层级切分 section。
    ///
    /// 返回 `Vec<Section>`。如果文本不含任何标题（`#`/`##`/...），
    /// 返回空 Vec（AC-3 退化条件，交由 SemanticSplitter 处理）。
    fn extract_sections(text: &str) -> Vec<Section> {
        let parser = Parser::new(text);
        let mut sections: Vec<Section> = Vec::new();
        let mut current = Section::default();
        // 标题栈：(level, title)，用于构建层级路径
        let mut heading_stack: Vec<(u8, String)> = Vec::new();
        let mut in_heading = false;
        let mut heading_text = String::new();
        let mut has_heading = false;

        for event in parser {
            match event {
                Event::Start(Tag::Heading { .. }) => {
                    has_heading = true;
                    in_heading = true;
                    heading_text.clear();
                    // 遇到新标题：保存当前 section（如果有内容）
                    if !current.content.trim().is_empty() {
                        sections.push(std::mem::take(&mut current));
                    }
                }
                Event::End(TagEnd::Heading(level)) => {
                    in_heading = false;
                    let level_num = level as u8;
                    // 弹出同级或更深的标题（维持层级栈）
                    while let Some((l, _)) = heading_stack.last() {
                        if *l >= level_num {
                            heading_stack.pop();
                        } else {
                            break;
                        }
                    }
                    heading_stack.push((level_num, heading_text.clone()));
                    // 构建章节路径（标题栈以 " > " 连接）
                    current.path = heading_stack
                        .iter()
                        .map(|(_, t)| t.as_str())
                        .collect::<Vec<_>>()
                        .join(" > ");
                    // 标题文本加入 section content（使 SemanticSplitter 能合并标题与正文到同一 chunk）
                    current.content.push_str(&heading_text);
                    current.content.push_str("\n\n");
                }
                Event::Text(t) => {
                    if in_heading {
                        heading_text.push_str(&t);
                    }
                    current.content.push_str(&t);
                }
                Event::Code(t) => {
                    current.content.push_str(&t);
                }
                Event::SoftBreak | Event::HardBreak => {
                    current.content.push('\n');
                }
                Event::End(TagEnd::Paragraph | TagEnd::CodeBlock | TagEnd::Item) => {
                    current.content.push_str("\n\n");
                }
                _ => {}
            }
        }

        // 保存最后一个 section
        if !current.content.trim().is_empty() {
            sections.push(current);
        }

        // AC-3：无标题文本退化为 SemanticSplitter 行为
        if !has_heading {
            return Vec::new();
        }

        sections
    }
}

impl Splitter for SectionAwareSplitter {
    async fn split(&self, text: &str) -> Result<Vec<String>> {
        if text.trim().is_empty() {
            return Ok(Vec::new());
        }

        // Step 1: pulldown-cmark 解析标题层级（CPU 密集，spawn_blocking）
        let owned = text.to_string();
        let sections = tokio::task::spawn_blocking(move || Self::extract_sections(&owned))
            .await
            .context("章节解析任务执行失败")?;

        // AC-3：无标题，退化为 SemanticSplitter 行为
        if sections.is_empty() {
            return self.inner.split(text).await;
        }

        // Step 2: 每 section 调 SemanticSplitter.split()，前缀追加章节路径
        let mut all_chunks = Vec::new();
        for section in sections {
            let pieces = self.inner.split(&section.content).await?;
            for piece in pieces {
                if !piece.trim().is_empty() {
                    if section.path.is_empty() {
                        // 标题前的"前言"section：无章节路径，不加前缀
                        all_chunks.push(piece);
                    } else {
                        all_chunks.push(format!("[{}] {}", section.path, piece));
                    }
                }
            }
        }
        Ok(all_chunks)
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::todo,
        clippy::unimplemented
    )]
    use super::*;
    use crate::Splitter;

    /// TC-VEC-010：章节感知分块（Section-Aware Chunking，REQ-VEC-006）。
    ///
    /// AC-1：含 # / ## / ### 三级标题的 Markdown，分块后每个 chunk 以 [章节路径] 前缀开头，
    /// 路径正确反映标题层级。
    /// AC-2：标题与其下首段正文出现在同一 chunk，不因 token 窗口恰好满而分离。
    /// AC-3：无标题的纯文本文档退化为现有 SemanticSplitter 行为，不产生空章节路径前缀。
    /// AC-4：章节路径前缀不破坏 SemanticSplitter 的中文句子/子句分割逻辑（旁证）。
    #[tokio::test]
    async fn tc_vec_010_section_aware_chunking() {
        let md = "# 引言\n\n\
这是引言部分的内容，介绍全文主旨。\n\n\
## 1.1 背景\n\n\
背景介绍文字，说明研究动机。\n\n\
# 方法\n\n\
方法论描述，包括实验设计和数据分析方法。";
        let splitter = SectionAwareSplitter::default_config().unwrap();
        let chunks = splitter.split(md).await.unwrap();

        assert!(!chunks.is_empty(), "必须产生至少 1 个 chunk");

        // AC-1：每个 chunk 以 [章节路径] 前缀开头
        for chunk in &chunks {
            assert!(
                chunk.starts_with('['),
                "chunk 应以 [章节路径] 前缀开头，实际开头: {:?}",
                chunk.chars().take(10).collect::<String>()
            );
        }

        // AC-2：标题"引言"与正文"这是引言部分"在同一 chunk
        let intro_chunk = chunks.iter().find(|c| c.contains("引言"));
        assert!(intro_chunk.is_some(), "应存在包含标题引言的 chunk");
        assert!(
            intro_chunk.unwrap().contains("这是引言部分"),
            "标题与正文应在同一 chunk（段落感知）"
        );

        // AC-1 补充：章节路径应反映标题层级
        // "1.1 背景" section 的 path 应为 "引言 > 1.1 背景"
        let bg_chunk = chunks.iter().find(|c| c.contains("背景介绍文字"));
        assert!(bg_chunk.is_some(), "应存在包含背景介绍的 chunk");
        assert!(
            bg_chunk.unwrap().contains("引言 > 1.1 背景"),
            "章节路径应反映标题层级（引言 > 1.1 背景）"
        );

        // AC-3：无标题的纯文本退化为 SemanticSplitter 行为（无 [章节路径] 前缀）
        let plain_text = "这是纯文本内容，没有标题标记。只有一段话用于验证退化行为。";
        let plain_chunks = splitter.split(plain_text).await.unwrap();
        for chunk in &plain_chunks {
            assert!(!chunk.starts_with('['), "无标题文本不应有章节路径前缀");
        }

        // AC-4：章节路径前缀不破坏中文分割（含中文的 chunk 应为合法 UTF-8）
        for chunk in &chunks {
            assert!(
                chunk.contains("引言") || chunk.contains("背景") || chunk.contains("方法"),
                "chunk 应包含中文内容"
            );
        }
    }
}
