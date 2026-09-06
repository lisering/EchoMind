//! FTS5 查询构建工具

/// 构建 FTS5 OR 查询字符串：将查询分词后逐词包裹为短语，用 OR 连接。
///
/// 设计原因：FTS5 trigram 分词器对整句短语匹配过于严格——
/// `"trampoline 是什么？"` 只匹配包含该完整短语的文档，导致含 "trampoline"
/// 但不含整句的 chunk 无法被检索到。改为逐词 OR 查询后，任一 token 命中即可返回。
///
/// 分词策略：
/// - 英文/数字：按空格分割为单词
/// - 中日韩（CJK）：逐字符提取（trigram 分词器以 3 字符为最小匹配单位，
///   单个 CJK 字符也作为 token 参与匹配，因 trigram 会将其与相邻字符组合）
/// - 标点/停用词：过滤掉长度 <1 的 token
///
/// 安全：每个 token 用双引号包裹并转义内部双引号，防止 FTS5 操作符注入
/// （如 `*`、`NEAR`、`AND`、`OR`、`NOT` 被当作普通字符串而非操作符）。
pub(crate) fn build_fts5_or_query(query: &str) -> String {
    let mut tokens: Vec<String> = Vec::new();

    for segment in query.split_whitespace() {
        let cleaned: String = segment
            .chars()
            .filter(|c| {
                c.is_alphanumeric()
                    || *c == '_'
                    || *c == '-'
                    || ('\u{4e00}'..='\u{9fff}').contains(c)
                    || ('\u{3040}'..='\u{30ff}').contains(c)
            })
            .collect();
        if !cleaned.is_empty() {
            tokens.push(cleaned);
        }
    }

    if tokens.len() == 1 && tokens[0].chars().count() > 3 {
        let token = tokens[0].clone();
        let has_cjk = token.chars().any(|c| {
            ('\u{4e00}'..='\u{9fff}').contains(&c) || ('\u{3040}'..='\u{30ff}').contains(&c)
        });
        if has_cjk {
            let chars: Vec<char> = token.chars().collect();
            tokens.clear();
            for window in chars.windows(3) {
                let s: String = window.iter().collect();
                if !s.is_empty() {
                    tokens.push(s);
                }
            }
            if tokens.is_empty() && !chars.is_empty() {
                tokens.push(token);
            }
        }
    }

    let mut seen = std::collections::HashSet::new();
    let mut parts: Vec<String> = Vec::new();
    for token in &tokens {
        if seen.insert(token.clone()) {
            let escaped = token.replace('"', "\"\"");
            parts.push(format!("\"{escaped}\""));
        }
    }

    parts.join(" OR ")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::build_fts5_or_query;

    #[test]
    fn single_english_word() {
        assert_eq!(build_fts5_or_query("trampoline"), "\"trampoline\"");
    }

    #[test]
    fn mixed_en_cjk_sentence() {
        let q = build_fts5_or_query("trampoline 是什么？");
        assert!(
            q.contains("\"trampoline\""),
            "必须包含 trampoline token: {q}"
        );
    }

    #[test]
    fn empty_query() {
        assert_eq!(build_fts5_or_query(""), "");
        assert_eq!(build_fts5_or_query("   "), "");
    }

    #[test]
    fn dedup_tokens() {
        let q = build_fts5_or_query("tokio tokio tokio");
        assert_eq!(q, "\"tokio\"");
    }

    #[test]
    fn fts5_operator_injection_prevention() {
        let q = build_fts5_or_query("test NEAR water");
        assert!(q.contains("\"NEAR\""));
    }
}
