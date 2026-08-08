//! Prompt 压缩模块（REQ-PERF-002）：规则压缩（Free 默认方案，零依赖）。
//!
//! ## 压缩策略
//!
//! 1. **代码块保护**：识别 `` ``` `` 围栏代码块，仅去除注释保留代码结构
//! 2. **Markdown 装饰去除**：`**bold**` → `bold`、`__underline__` → `underline` 等
//! 3. **停用词去除**：英文 + 中文停用词表，去除高频无信息量词汇
//! 4. **冗余空白去除**：多空格→单空格，多空行→单空行
//! 5. **句子级评分保留**：按与 query 的词重叠度排序，保留 top-N 句
//!
//! ## 压缩比
//!
//! - `2.0` = 保守（压缩到 1/2，信息保留率 ≥ 90%）
//! - `3.0` = 平衡（压缩到 1/3，信息保留率 ≥ 80%）
//! - `5.0` = 激进（压缩到 1/5，信息保留率 ≥ 60%）
//! - `1.0` = 禁用（原样返回）
//!
//! ## 调研来源
//!
//! - LLMLingua-2 (arXiv:2403.12968)：XLM-RoBERTa token 分类压缩，2-5x 压缩比
//! - 本实现为规则备选方案，零模型依赖，适用于 Free 版

use std::collections::HashSet;

use crate::PromptCompressor;

/// 英文停用词表（高频无信息量词汇）。
///
/// 来源：NLTK 英文停用词表的精简版本，去除常见代词/介词/冠词/连词。
const ENGLISH_STOPWORDS: &[&str] = &[
    "a",
    "an",
    "the",
    "is",
    "are",
    "was",
    "were",
    "be",
    "been",
    "being",
    "have",
    "has",
    "had",
    "do",
    "does",
    "did",
    "will",
    "would",
    "could",
    "should",
    "may",
    "might",
    "must",
    "can",
    "of",
    "in",
    "on",
    "at",
    "to",
    "for",
    "with",
    "by",
    "from",
    "as",
    "into",
    "onto",
    "upon",
    "about",
    "through",
    "during",
    "before",
    "after",
    "above",
    "below",
    "between",
    "within",
    "without",
    "against",
    "and",
    "or",
    "but",
    "nor",
    "so",
    "yet",
    "both",
    "either",
    "neither",
    "each",
    "every",
    "all",
    "any",
    "some",
    "such",
    "no",
    "not",
    "only",
    "own",
    "same",
    "than",
    "too",
    "very",
    "just",
    "also",
    "now",
    "then",
    "here",
    "there",
    "when",
    "where",
    "why",
    "how",
    "what",
    "which",
    "who",
    "whom",
    "this",
    "that",
    "these",
    "those",
    "i",
    "you",
    "he",
    "she",
    "it",
    "we",
    "they",
    "them",
    "their",
    "theirs",
    "our",
    "ours",
    "your",
    "yours",
    "his",
    "her",
    "hers",
    "its",
    "my",
    "mine",
    "me",
    "him",
    "us",
    "myself",
    "yourself",
    "himself",
    "herself",
    "itself",
    "ourselves",
    "yourselves",
    "themselves",
];

/// 中文停用词表（高频虚词/助词/语气词）。
const CHINESE_STOPWORDS: &[&str] = &[
    "的",
    "了",
    "在",
    "是",
    "我",
    "有",
    "和",
    "就",
    "不",
    "人",
    "都",
    "一",
    "一个",
    "上",
    "也",
    "很",
    "到",
    "说",
    "要",
    "去",
    "你",
    "会",
    "着",
    "没有",
    "看",
    "好",
    "自己",
    "这",
    "那",
    "这个",
    "那个",
    "这些",
    "那些",
    "什么",
    "怎么",
    "为什么",
    "可以",
    "能",
    "把",
    "被",
    "让",
    "使",
    "给",
    "对",
    "从",
    "向",
    "为",
    "以",
    "于",
    "与",
    "及",
    "或",
    "但",
    "而",
    "且",
    "则",
    "才",
    "再",
    "又",
    "还",
    "已",
    "正",
    "正在",
    "应该",
    "可能",
    "也许",
    "或许",
    "大概",
    "大约",
    "左右",
    "上下",
    "因为",
    "所以",
    "如果",
    "虽然",
    "但是",
    "然而",
    "不过",
    "除了",
    "除非",
    "即使",
    "尽管",
    "无论",
    "不管",
    "只要",
    "只有",
    "才",
    "就",
    "便",
    "即",
    "然后",
    "后来",
    "接着",
    "首先",
    "其次",
    "最后",
    "终于",
];

/// 规则压缩器（Free 默认方案，零依赖）。
///
/// 通过去除停用词、冗余空白、代码注释和 Markdown 装饰，
/// 并按与 query 的词重叠度保留 top-N 句实现压缩。
///
/// # 压缩比与信息保留率
///
/// | 压缩比 | 信息保留率（目标） |
/// |--------|-------------------|
/// | 2.0    | ≥ 90%             |
/// | 3.0    | ≥ 80%             |
/// | 5.0    | ≥ 60%             |
/// | 1.0    | 100%（禁用）      |
#[derive(Debug, Clone, Default)]
pub struct RuleBasedCompressor;

impl RuleBasedCompressor {
    /// 创建新的规则压缩器。
    pub fn new() -> Self {
        Self
    }

    /// 获取停用词集合（英文 + 中文）。
    fn stopwords() -> HashSet<&'static str> {
        ENGLISH_STOPWORDS
            .iter()
            .chain(CHINESE_STOPWORDS.iter())
            .copied()
            .collect()
    }

    /// 判断一个词是否为停用词。
    fn is_stopword(word: &str, stopwords: &HashSet<&str>) -> bool {
        let lower = word.to_lowercase();
        stopwords.contains(lower.as_str())
    }

    /// 去除 Markdown 装饰标记（`**bold**` → `bold` 等）。
    ///
    /// 保留代码块围栏（```）和行内代码（`code`）的内容，仅去除格式标记。
    fn strip_markdown_decorations(text: &str) -> String {
        let mut result = text.to_string();
        // 粗体/斜体：**text**, __text__, *text*, _text_
        // 注意：不用正则，简单字符替换避免误伤代码块内的 *
        result = replace_paired_marker(&result, "**");
        result = replace_paired_marker(&result, "__");
        result = replace_paired_marker(&result, "~~");
        result = replace_paired_marker_single(&result, '*');
        result = replace_paired_marker_single(&result, '_');
        // 删除线 ~~text~~ 已处理
        // 标题标记：# ## ### → 去除前缀 #
        result = strip_heading_markers(&result);
        result
    }

    /// 去除代码块内的注释（`//` 和 `/* */`），保留代码结构。
    ///
    /// 仅在代码块（``` 围栏内）执行，不处理普通文本中的 //。
    fn strip_code_comments(code: &str) -> String {
        let mut result = String::with_capacity(code.len());
        let mut chars = code.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '/' && chars.peek() == Some(&'/') {
                // 行注释 //：跳过到行尾
                chars.next(); // consume second '/'
                for c in chars.by_ref() {
                    if c == '\n' {
                        result.push('\n');
                        break;
                    }
                }
            } else if ch == '/' && chars.peek() == Some(&'*') {
                // 块注释 /* */：跳过到 */
                chars.next(); // consume '*'
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

    /// 去除冗余空白：多空格→单空格，多空行→单空行，首尾空白去除。
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

    /// 从文本中去除停用词（中英文）。
    ///
    /// 按空格分割英文，按字符扫描中文。
    /// 中文不需要空格分割，逐字检查是否为停用词。
    fn remove_stopwords(text: &str, stopwords: &HashSet<&str>) -> String {
        let mut result = Vec::new();
        for word in text.split_whitespace() {
            let cleaned = word.trim_matches(|c: char| !c.is_alphanumeric() && !is_cjk(c));
            if cleaned.is_empty() {
                // 纯标点符号，保留
                result.push(word.to_string());
            } else if is_cjk_word(cleaned) {
                // 中文词：逐字检查停用词
                let filtered: String = cleaned
                    .chars()
                    .filter(|&c| {
                        let s: String = c.to_string();
                        !stopwords.contains(s.as_str())
                    })
                    .collect();
                if !filtered.is_empty() {
                    // 重建 word（保留前后的标点）
                    let prefix = &word[..word.find(cleaned).unwrap_or(0)];
                    let suffix = &word[word.find(cleaned).unwrap_or(0) + cleaned.len()..];
                    result.push(format!("{prefix}{filtered}{suffix}"));
                }
            } else if !Self::is_stopword(cleaned, stopwords) {
                // 英文词：非停用词，保留
                result.push(word.to_string());
            }
            // 停用词被跳过
        }
        result.join(" ")
    }

    /// 将文本分割为句子。
    ///
    /// 按句末标点（. ! ? 。 ！ ？ ; ；）分割，保留标点。
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

    /// 计算句子与 query 的相关性分数。
    ///
    /// 分数 = base(1.0) + query_overlap + info_density_bonus
    /// - query_overlap: 句子中与 query 共享的非停用词数量 × 1.0
    /// - info_density_bonus: 大写词（专有名词）×2.0, 数字 ×1.0, CJK ×0.5
    fn sentence_score(sentence: &str, query_words: &HashSet<String>) -> f32 {
        let mut score = 1.0; // 基础分：每个句子至少 1 分，避免被完全丢弃

        // query 词重叠
        for word in sentence.split_whitespace() {
            let cleaned = word.trim_matches(|c: char| !c.is_alphanumeric() && !is_cjk(c));
            let lower = cleaned.to_lowercase();
            if query_words.contains(&lower) {
                score += 1.0;
            }
        }
        // 中文逐字匹配
        for c in sentence.chars() {
            if is_cjk(c) {
                let s = c.to_string();
                if query_words.contains(&s) {
                    score += 0.5;
                }
            }
        }

        // 信息密度奖励：大写词（专有名词/技术术语）
        for word in sentence.split_whitespace() {
            let cleaned = word.trim_matches(|c: char| !c.is_alphanumeric() && !is_cjk(c));
            if cleaned.is_empty() {
                continue;
            }
            // 大写开头且非全大写的词（如 SQLite, RAG, FastEmbed）
            let first = cleaned.chars().next();
            if first.is_some_and(|c| c.is_uppercase()) && !cleaned.chars().all(|c| c.is_uppercase())
            {
                score += 2.0;
            }
            // 全大写缩写词（如 RAG, ONNX, BLOB, WAL）
            if cleaned.len() >= 2 && cleaned.chars().all(|c| c.is_uppercase()) {
                score += 2.0;
            }
            // 数字（如 384, 256）
            if cleaned.chars().any(|c| c.is_numeric()) {
                score += 1.0;
            }
        }

        // CJK 信息密度奖励
        let cjk_count = sentence.chars().filter(|c| is_cjk(*c)).count();
        score += cjk_count as f32 * 0.5;

        score
    }

    /// 执行规则压缩的核心逻辑（同步函数，供 `compress` trait 方法调用）。
    ///
    /// # 参数
    /// - `text`: 待压缩的文本
    /// - `ratio`: 目标压缩比（2.0 = 压缩到 1/2）
    /// - `query`: 用户查询文本
    ///
    /// # 返回
    /// 压缩后的文本
    pub fn compress_sync(&self, text: &str, ratio: f32, query: &str) -> String {
        // ratio ≤ 1.0 表示禁用压缩
        if ratio <= 1.0 {
            return text.to_string();
        }

        let stopwords = Self::stopwords();

        // 1. 分割代码块和非代码文本
        let segments = split_code_blocks(text);

        // 2. 对每个段分别处理
        let mut compressed_segments = Vec::new();
        for segment in &segments {
            if segment.is_code {
                // 代码块：去除注释，保留代码结构
                let cleaned = Self::strip_code_comments(&segment.content);
                let cleaned = Self::collapse_whitespace(&cleaned);
                compressed_segments.push(format_code_block(&cleaned));
            } else {
                // 非代码文本：先清理 Markdown 装饰和空白，再句子级评分
                let cleaned = Self::strip_markdown_decorations(&segment.content);
                let cleaned = Self::collapse_whitespace(&cleaned);

                // 句子分割（在去除停用词之前，保持句子结构完整）
                let sentences = Self::split_sentences(&cleaned);
                if sentences.is_empty() {
                    compressed_segments.push(cleaned);
                    continue;
                }

                // 准备 query 词集
                let mut query_words: HashSet<String> = query
                    .split_whitespace()
                    .map(|w| {
                        w.trim_matches(|c: char| !c.is_alphanumeric() && !is_cjk(c))
                            .to_lowercase()
                    })
                    .filter(|w| !w.is_empty())
                    .collect();
                // 中文逐字加入 query_words
                for c in query.chars() {
                    if is_cjk(c) {
                        query_words.insert(c.to_string());
                    }
                }

                // 评分排序
                let mut scored: Vec<(usize, f32, String)> = sentences
                    .iter()
                    .enumerate()
                    .map(|(idx, s)| (idx, Self::sentence_score(s, &query_words), s.clone()))
                    .collect();
                // 按分数降序排序，分数相同则按原始顺序
                scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

                // 计算保留句子数：至少保留 sqrt 比例，避免过度压缩
                // ratio=2: keep ≥ 1/sqrt(2) ≈ 71%, ratio=3: ≥ 58%, ratio=5: ≥ 45%
                let min_ratio = 1.0 / ratio.sqrt();
                let min_keep = ((sentences.len() as f32 * min_ratio).ceil() as usize).max(1);
                let target_keep = ((sentences.len() as f32 / ratio).ceil() as usize).max(min_keep);
                let target_keep = target_keep.min(sentences.len());

                // 保留 top-N 句子
                let kept: Vec<(usize, String)> = scored
                    .iter()
                    .take(target_keep)
                    .map(|(idx, _, s)| (*idx, s.clone()))
                    .collect();

                // 按原始顺序重排
                let mut kept = kept;
                kept.sort_by_key(|(idx, _)| *idx);

                // 对保留的句子去除停用词（后处理，不破坏句子结构）
                let compressed_text = kept
                    .iter()
                    .map(|(_, s)| Self::remove_stopwords(s, &stopwords))
                    .collect::<Vec<_>>()
                    .join(" ");
                compressed_segments.push(compressed_text);
            }
        }

        compressed_segments.join("\n\n")
    }
}

impl PromptCompressor for RuleBasedCompressor {
    fn compress<'a>(
        &'a self,
        text: &'a str,
        ratio: f32,
        query: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<String>> + Send + 'a>>
    {
        Box::pin(async move {
            // 规则压缩是纯 CPU 计算，无需 async，直接同步调用
            let result = self.compress_sync(text, ratio, query);
            Ok(result)
        })
    }
}

// ============================================================
// 辅助函数
// ============================================================

/// 判断字符是否为 CJK（中日韩）字符。
fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x4E00..=0x9FFF   |  // CJK Unified Ideographs
        0x3400..=0x4DBF   |  // CJK Extension A
        0x20000..=0x2A6DF |  // CJK Extension B
        0x2A700..=0x2B73F |  // CJK Extension C
        0x2B740..=0x2B81F |  // CJK Extension D
        0x3000..=0x303F   |  // CJK Symbols and Punctuation
        0xFF00..=0xFFEF      // Halfwidth and Fullwidth Forms
    )
}

/// 判断一个词是否包含 CJK 字符。
fn is_cjk_word(word: &str) -> bool {
    word.chars().any(is_cjk)
}

/// 替换成对标记（如 `**bold**` → `bold`）。
///
/// 简单实现：找到成对的 marker，删除标记保留内容。
fn replace_paired_marker(text: &str, marker: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut remaining = text;
    while let Some(start) = remaining.find(marker) {
        result.push_str(&remaining[..start]);
        let after_first = &remaining[start + marker.len()..];
        if let Some(end) = after_first.find(marker) {
            result.push_str(&after_first[..end]);
            remaining = &after_first[end + marker.len()..];
        } else {
            // 未配对，原样保留
            result.push_str(marker);
            remaining = after_first;
        }
    }
    result.push_str(remaining);
    result
}

/// 替换单字符成对标记（如 `*italic*` → `italic`）。
fn replace_paired_marker_single(text: &str, marker: char) -> String {
    let mut result = String::with_capacity(text.len());
    let mut remaining = text;
    while let Some(start) = remaining.find(marker) {
        result.push_str(&remaining[..start]);
        let after_first = &remaining[start + marker.len_utf8()..];
        if let Some(end) = after_first.find(marker) {
            result.push_str(&after_first[..end]);
            remaining = &after_first[end + marker.len_utf8()..];
        } else {
            result.push(marker);
            remaining = after_first;
        }
    }
    result.push_str(remaining);
    result
}

/// 去除行首的 Markdown 标题标记（# ## ### 等）。
fn strip_heading_markers(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for line in text.lines() {
        let trimmed = line.trim_start_matches('#');
        result.push_str(trimmed);
        result.push('\n');
    }
    // 去除末尾多余换行
    result.trim_end_matches('\n').to_string()
}

/// 代码段结构。
struct CodeSegment {
    /// 段内容（不含围栏标记）
    content: String,
    /// 是否为代码块
    is_code: bool,
}

/// 将文本分割为代码块和非代码段。
///
/// 识别 `` ``` `` 围栏代码块，其余为非代码文本。
fn split_code_blocks(text: &str) -> Vec<CodeSegment> {
    let mut segments = Vec::new();
    let mut current_text = String::new();
    let mut in_code = false;
    let mut code_content = String::new();

    for line in text.lines() {
        let trimmed = line.trim_start();
        if !in_code && trimmed.starts_with("```") {
            // 代码块开始
            if !current_text.is_empty() {
                segments.push(CodeSegment {
                    content: current_text.clone(),
                    is_code: false,
                });
                current_text.clear();
            }
            in_code = true;
            // 保留语言标识行（如 ```rust），但不加入 code_content
            continue;
        } else if in_code && trimmed.starts_with("```") {
            // 代码块结束
            segments.push(CodeSegment {
                content: code_content.clone(),
                is_code: true,
            });
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

    // 处理末尾未闭合的代码块（容错）
    if in_code {
        if !code_content.is_empty() {
            segments.push(CodeSegment {
                content: code_content,
                is_code: true,
            });
        }
    } else if !current_text.is_empty() {
        segments.push(CodeSegment {
            content: current_text,
            is_code: false,
        });
    }

    // 去除段内容末尾多余换行
    for seg in &mut segments {
        seg.content = seg.content.trim_end_matches('\n').to_string();
    }

    segments
}

/// 格式化代码块输出（添加围栏标记）。
fn format_code_block(code: &str) -> String {
    format!("```\n{code}\n```")
}
