//! Proposition 分割器（REQ-PERF-007）：将 chunk 分解为自包含的原子事实。
//!
//! 基于 Dense X Retrieval 论文（arXiv:2312.06648），proposition 级检索精度
//! 显著优于 chunk 级（命中率 +30-50%）。
//!
//! ## 规则方案（零 LLM 调用）
//!
//! 1. **句子分割**：按中英文标点分割为独立句子
//! 2. **代词消解**：他/她/它 → 替换为前文实体；he/she/it/they → 替换为前文实体
//! 3. **上下文补全**：以连词开头的句子补全主语上下文
//!
//! ## 调研来源
//!
//! - Dense X Retrieval (arXiv:2312.06648)：proposition 级原子检索粒度

use echomind_models::{Entity, Proposition};

use crate::entity_extractor::EntityExtractor;

/// 中文人称代词
const CN_PRONOUN_PERSON: &[char] = &['他', '她'];

/// 中文物称代词
const CN_PRONOUN_THING: &[char] = &['它'];

/// 英文人称代词（需 word boundary 匹配）
const EN_PRONOUNS_PERSON: &[&str] = &["he", "she", "they", "He", "She", "They"];

/// 英文物称代词（需 word boundary 匹配）
const EN_PRONOUNS_THING: &[&str] = &["it", "It"];

/// 中文连词（指示省略主语）
const CN_CONJUNCTIONS: &[&str] = &[
    "因此", "所以", "然后", "但是", "不过", "而且", "并且", "从而", "于是", "由此",
];

/// 英文连词（指示省略主语）
const EN_CONJUNCTIONS: &[&str] = &[
    "Therefore",
    "So",
    "Then",
    "But",
    "However",
    "And",
    "Thus",
    "Hence",
    "Moreover",
    "Furthermore",
];

/// Proposition 分割器：将 chunk 分解为自包含的原子事实。
///
/// 使用规则方案（句子分割 + 代词消解 + 上下文补全），零 LLM 调用。
///
/// # 工作流程
///
/// ```text
/// chunk_content → split_sentences() → each sentence → self_containize()
///      → Proposition（自包含的原子事实）
/// ```
///
/// # 自包含化策略
///
/// 1. **代词替换**：他/她/它/he/she/it/they → 替换为前文最近的匹配实体
/// 2. **上下文补全**：以连词开头的句子补全主语前缀
pub struct PropositionSplitter;

impl PropositionSplitter {
    /// 将 chunk 内容分割为 proposition 列表。
    ///
    /// # 参数
    /// - `chunk_content`: chunk 文本内容
    /// - `chunk_id`: 关联的 chunk ID
    /// - `doc_name`: 文档名（用于上下文补全）
    ///
    /// # 返回
    /// 自包含的 proposition 列表，每个 proposition 关联到 `chunk_id`。
    /// 空 chunk 返回空列表。
    pub fn split(chunk_content: &str, chunk_id: &str, doc_name: &str) -> Vec<Proposition> {
        if chunk_content.trim().is_empty() {
            return Vec::new();
        }

        let sentences = Self::split_sentences(chunk_content);
        let mut propositions = Vec::with_capacity(sentences.len());
        let mut prev_entities: Vec<Entity> = Vec::new();

        for (seq, sentence) in sentences.into_iter().enumerate() {
            // 使用前一句的实体进行代词消解（更准确的上下文）
            let self_contained = Self::self_containize(&sentence, &prev_entities, doc_name);
            propositions.push(Proposition::new(chunk_id.to_string(), self_contained, seq));

            // 当前句子的实体供下一句的代词消解使用
            prev_entities = EntityExtractor::extract(&sentence);
        }

        propositions
    }

    /// 将文本按中英文句子边界分割。
    ///
    /// 支持的分割符：
    /// - 中文：。！？；
    /// - 英文：`. ! ? ;`（后跟空格或行尾）
    /// - 段落：`\n\n`
    ///
    /// 返回非空句子列表（已 trim），保留原始标点。
    fn split_sentences(text: &str) -> Vec<String> {
        let mut sentences = Vec::new();
        let chars: Vec<char> = text.chars().collect();
        let mut start = 0usize;

        let mut i = 0usize;
        while i < chars.len() {
            let c = chars[i];
            let is_cn_boundary = matches!(c, '。' | '！' | '？' | '；');
            let is_en_boundary = matches!(c, '.' | '!' | '?' | ';');

            if is_cn_boundary {
                let sentence: String = chars[start..=i].iter().collect();
                let trimmed = sentence.trim();
                if !trimmed.is_empty() {
                    sentences.push(trimmed.to_string());
                }
                start = i + 1;
            } else if is_en_boundary {
                // 英文标点：后跟空格、行尾或另一个句界才分割
                let is_end = i + 1 >= chars.len()
                    || chars[i + 1].is_whitespace()
                    || matches!(chars[i + 1], '.' | '!' | '?' | ';');
                if is_end {
                    let sentence: String = chars[start..=i].iter().collect();
                    let trimmed = sentence.trim();
                    if !trimmed.is_empty() {
                        sentences.push(trimmed.to_string());
                    }
                    start = i + 1;
                }
            } else if c == '\n' && i + 1 < chars.len() && chars[i + 1] == '\n' {
                // 段落分割
                let sentence: String = chars[start..i].iter().collect();
                let trimmed = sentence.trim();
                if !trimmed.is_empty() {
                    sentences.push(trimmed.to_string());
                }
                start = i + 2;
                i = start;
                continue;
            }

            i += 1;
        }

        // 处理剩余文本
        if start < chars.len() {
            let remaining: String = chars[start..].iter().collect();
            let trimmed = remaining.trim();
            if !trimmed.is_empty() {
                sentences.push(trimmed.to_string());
            }
        }

        sentences
    }

    /// 使句子自包含：代词消解 + 上下文补全。
    fn self_containize(sentence: &str, prev_entities: &[Entity], doc_name: &str) -> String {
        let resolved = Self::resolve_pronouns(sentence, prev_entities);
        Self::complete_context(&resolved, prev_entities, doc_name)
    }

    /// 代词消解：将代词替换为前文最近的匹配实体。
    ///
    /// 中文：
    /// - 他/她 → 最近的 person 实体（排除"其他"中的"他"）
    /// - 它 → 最近的非 person 实体
    ///
    /// 英文（whole word match）：
    /// - he/she/they → 最近的 person 实体
    /// - it → 最近的非 person 实体
    fn resolve_pronouns(sentence: &str, prev_entities: &[Entity]) -> String {
        if prev_entities.is_empty() {
            return sentence.to_string();
        }

        let person_entity = prev_entities.iter().find(|e| e.entity_type == "person");
        let thing_entity = prev_entities.iter().find(|e| e.entity_type != "person");

        // 中文代词替换（逐字符处理）
        let chars: Vec<char> = sentence.chars().collect();
        let mut output = String::with_capacity(sentence.len());
        let mut i = 0usize;

        while i < chars.len() {
            let c = chars[i];
            // 前一个字符（用于排除"其他"中的"他"）
            let prev_char = if i > 0 { Some(chars[i - 1]) } else { None };

            // 他/她 → 最近的 person 实体（排除“其他”、“此他”等组合）
            if let Some(person) = person_entity
                && CN_PRONOUN_PERSON.contains(&c)
                && !matches!(prev_char, Some('其') | Some('此') | Some('那') | Some('这'))
            {
                output.push_str(&person.text);
                i += 1;
                continue;
            }

            // 它 → 最近的非 person 实体
            if let Some(thing) = thing_entity
                && CN_PRONOUN_THING.contains(&c)
                && !matches!(prev_char, Some('其') | Some('此'))
            {
                output.push_str(&thing.text);
                i += 1;
                continue;
            }

            output.push(c);
            i += 1;
        }

        // 英文代词替换（whole word match）
        let mut result = output;
        if let Some(person) = person_entity {
            for pronoun in EN_PRONOUNS_PERSON {
                result = Self::replace_word(&result, pronoun, &person.text);
            }
        }
        if let Some(thing) = thing_entity {
            for pronoun in EN_PRONOUNS_THING {
                result = Self::replace_word(&result, pronoun, &thing.text);
            }
        }

        result
    }

    /// 上下文补全：以连词开头的句子补全主语前缀。
    ///
    /// 当句子以连词（因此/所以/然后/Therefore/So/Then 等）开头时，
    /// 在句首添加前文最近的实体作为主语上下文。
    fn complete_context(sentence: &str, prev_entities: &[Entity], _doc_name: &str) -> String {
        let trimmed = sentence.trim();
        if trimmed.is_empty() || prev_entities.is_empty() {
            return sentence.to_string();
        }

        // 检查是否以中文连词开头
        let starts_with_cn_conj = CN_CONJUNCTIONS.iter().any(|c| trimmed.starts_with(c));

        // 检查是否以英文连词开头（后跟空格）
        let starts_with_en_conj = EN_CONJUNCTIONS.iter().any(|c| {
            trimmed.starts_with(c)
                && (trimmed.len() == c.len()
                    || trimmed
                        .as_bytes()
                        .get(c.len())
                        .is_some_and(|b| b.is_ascii_whitespace()))
        });

        if let Some(first_entity) = prev_entities.first()
            && (starts_with_cn_conj || starts_with_en_conj)
        {
            // 根据首字符判断中英文，选择适当的前缀格式
            let first_char = trimmed.chars().next().unwrap_or(' ');
            if first_char.is_ascii() {
                return format!("{}: {}", first_entity.text, trimmed);
            }
            return format!("{}{}", first_entity.text, trimmed);
        }

        sentence.to_string()
    }

    /// 替换英文单词（whole word match, case-sensitive）。
    ///
    /// 仅在单词边界处替换（前后为非字母数字字符或字符串首尾）。
    fn replace_word(text: &str, word: &str, replacement: &str) -> String {
        let chars: Vec<char> = text.chars().collect();
        let word_chars: Vec<char> = word.chars().collect();
        let word_len = word_chars.len();
        if word_len == 0 || word_len > chars.len() {
            return text.to_string();
        }

        let mut result = String::with_capacity(text.len());
        let mut i = 0usize;

        while i < chars.len() {
            // 检查前边界
            let is_start = i == 0 || !chars[i - 1].is_alphanumeric();

            if is_start && i + word_len <= chars.len() {
                let candidate: String = chars[i..i + word_len].iter().collect();
                if candidate == word {
                    // 检查后边界
                    let is_end =
                        i + word_len >= chars.len() || !chars[i + word_len].is_alphanumeric();
                    if is_end {
                        result.push_str(replacement);
                        i += word_len;
                        continue;
                    }
                }
            }
            result.push(chars[i]);
            i += 1;
        }

        result
    }
}
