//! Wiki-link 解析器（REQ-ING-020 Markdown 笔记双向链接）。
//!
//! 解析 Obsidian 风格 `[[wiki-link]]` 语法，提取链接目标文本。
//! 纯函数实现，零依赖（仅使用标准库字符串操作），零 LLM 调用。
//!
//! ## 语法规则
//!
//! | 语法 | 含义 | 示例 |
//! |---|---|---|
//! | `[[target]]` | 基本链接 | `[[设计文档]]` → target="设计文档" |
//! | `[[target|alias]]` | 别名链接 | `[[设计文档|设计]] → target="设计文档" |
//! | `[[target#heading]]` | 标题引用 | `[[设计文档#架构]] → target="设计文档" |
//! | `[[target#heading|alias]]` | 标题+别名 | `[[设计文档#架构|设计]] → target="设计文档" |
//!
//! ## 转义规则
//!
//! - 代码块中的 `[[link]]` 不被解析（``` `...` ``` 或 ``` ```...``` ``` 内）
//! - 行内代码 `` `[[link]]` `` 不被解析
//! - 空 target `[[ ]]` 不被解析
//! - 已解码的链接不递归解析（无嵌套 `[[a[[b]]]]`）
//!
//! ## 性能
//!
//! 纯 CPU 计算（字符串扫描），无网络/模型依赖。
//! 典型 chunk（256 tokens）解析耗时 < 0.1ms。

use echomind_models::WikiLink;

/// 解析文本中的所有 wiki-link，返回 WikiLink 列表。
///
/// # 参数
/// - `text`: 待解析的文本（通常是 chunk 内容）
/// - `source_doc_id`: 源文档 ID（包含 `[[link]]` 的文档）
/// - `chunk_id`: 来源 chunk ID
///
/// # 返回
/// 去重后的 WikiLink 列表（同一 chunk 内重复出现的 target 只保留一个）。
/// 空文本或无 wiki-link 返回空 Vec。
pub fn parse_wiki_links(text: &str, source_doc_id: &str, chunk_id: &str) -> Vec<WikiLink> {
    if text.is_empty() {
        return Vec::new();
    }

    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut links: Vec<WikiLink> = Vec::new();

    // 使用逐字符遍历（UTF-8 安全），避免字节索引越界
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut i = 0;

    while i < n {
        // 检查代码块围栏（``` 或 ~~~）
        if i + 2 < n && chars[i] == '`' && chars[i + 1] == '`' && chars[i + 2] == '`' {
            if let Some(end) = find_closing_fence_chars(&chars, i + 3, '`', 3) {
                i = end + 3;
                continue;
            } else {
                break;
            }
        }
        if i + 2 < n && chars[i] == '~' && chars[i + 1] == '~' && chars[i + 2] == '~' {
            if let Some(end) = find_closing_fence_chars(&chars, i + 3, '~', 3) {
                i = end + 3;
                continue;
            } else {
                break;
            }
        }

        // 检查行内代码（单个反引号）
        if chars[i] == '`' {
            // 找到闭合的反引号
            let mut j = i + 1;
            while j < n && chars[j] != '`' {
                j += 1;
            }
            if j < n {
                i = j + 1;
                continue;
            } else {
                // 未闭合行内代码，跳到末尾
                break;
            }
        }

        // 检查 wiki-link 模式 `[[`
        if i + 1 < n && chars[i] == '[' && chars[i + 1] == '[' {
            // 找到闭合 `]]`
            if let Some(close_pos) = find_wiki_link_close_chars(&chars, i + 2) {
                // 提取 `[[` 和 `]]` 之间的内容
                let inner: String = chars[i + 2..close_pos].iter().collect();

                // 提取 target：去掉 `|alias` 和 `#heading` 部分
                let target = extract_target(&inner);

                // 验证 target 非空
                if !target.is_empty() {
                    // 去重：同一 chunk 内重复 target 只保留一个
                    if seen.insert(target.clone()) {
                        links.push(WikiLink::new(
                            source_doc_id.to_string(),
                            target,
                            chunk_id.to_string(),
                        ));
                    }
                }

                i = close_pos + 2; // 跳过 `]]`
                continue;
            }
            // 未找到闭合 `]]`，跳过此 `[[`
        }

        i += 1;
    }

    links
}

/// 从 wiki-link 内部文本提取 target。
///
/// - `[[target]]` → "target"
/// - `[[target|alias]]` → "target"
/// - `[[target#heading]]` → "target"
/// - `[[target#heading|alias]]` → "target"
fn extract_target(inner: &str) -> String {
    // 先去掉 alias（`|` 后面的部分）
    let without_alias = match inner.find('|') {
        Some(idx) => &inner[..idx],
        None => inner,
    };

    // 再去掉 heading（`#` 后面的部分）
    let target = match without_alias.find('#') {
        Some(idx) => &without_alias[..idx],
        None => without_alias,
    };

    target.trim().to_string()
}

/// 从 `start` 位置开始查找 `]]` 闭合标记（字符级遍历，UTF-8 安全）。
///
/// 返回 `]]` 的起始位置（第一个 `]` 的索引）。
///
/// # 参数
/// - `chars`: 字符数组
/// - `start`: 开始搜索的位置（`[[` 之后的位置）
///
/// # 返回
/// `]]` 的起始索引，未找到返回 `None`。
fn find_wiki_link_close_chars(chars: &[char], start: usize) -> Option<usize> {
    let n = chars.len();
    let mut i = start;

    while i + 1 < n {
        if chars[i] == ']' && chars[i + 1] == ']' {
            return Some(i);
        }
        i += 1;
    }

    None
}

/// 查找代码块闭合围栏的位置（字符级遍历，UTF-8 安全）。
///
/// 返回闭合围栏的起始位置。
///
/// # 参数
/// - `chars`: 字符数组
/// - `start`: 开始搜索的位置（开头围栏之后的位置）
/// - `ch`: 围栏字符（`` ` `` 或 `~`）
/// - `count`: 围栏字符数（3）
fn find_closing_fence_chars(chars: &[char], start: usize, ch: char, count: usize) -> Option<usize> {
    let n = chars.len();
    let mut i = start;

    while i + count - 1 < n {
        let mut match_count = 0;
        for j in 0..count {
            if i + j < n && chars[i + j] == ch {
                match_count += 1;
            } else {
                break;
            }
        }
        if match_count == count {
            return Some(i);
        }
        i += 1;
    }

    None
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    // ───────────────────────── 基本解析 ─────────────────────────

    /// TC-ING-WIKI-001：基本 wiki-link 解析 `[[target]]`
    #[test]
    fn tc_wiki_001_basic_link() {
        let text = "请参考 [[设计文档]] 了解更多细节。";
        let links = parse_wiki_links(text, "doc-1", "chunk-1");

        assert_eq!(links.len(), 1, "应解析出 1 个 wiki-link");
        assert_eq!(links[0].target, "设计文档");
        assert_eq!(links[0].source_doc_id, "doc-1");
        assert_eq!(links[0].chunk_id, "chunk-1");
    }

    /// TC-ING-WIKI-002：多个 wiki-link 解析
    #[test]
    fn tc_wiki_002_multiple_links() {
        let text = "参考 [[文档A]] 和 [[文档B]] 以及 [[文档C]]。";
        let links = parse_wiki_links(text, "doc-1", "chunk-1");

        assert_eq!(links.len(), 3, "应解析出 3 个 wiki-link");
        let targets: Vec<&str> = links.iter().map(|l| l.target.as_str()).collect();
        assert!(targets.contains(&"文档A"));
        assert!(targets.contains(&"文档B"));
        assert!(targets.contains(&"文档C"));
    }

    // ───────────────────────── 别名和标题 ─────────────────────────

    /// TC-ING-WIKI-003：别名语法 `[[target|alias]]`
    #[test]
    fn tc_wiki_003_alias_syntax() {
        let text = "请参考 [[设计文档|设计]] 了解架构。";
        let links = parse_wiki_links(text, "doc-1", "chunk-1");

        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target, "设计文档", "应提取 target 而非 alias");
    }

    /// TC-ING-WIKI-004：标题引用 `[[target#heading]]`
    #[test]
    fn tc_wiki_004_heading_ref() {
        let text = "详见 [[架构设计#数据库]] 章节。";
        let links = parse_wiki_links(text, "doc-1", "chunk-1");

        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target, "架构设计", "应提取 target 而非 heading");
    }

    /// TC-ING-WIKI-005：标题+别名 `[[target#heading|alias]]`
    #[test]
    fn tc_wiki_005_heading_alias() {
        let text = "参见 [[架构设计#数据库|DB]] 部分。";
        let links = parse_wiki_links(text, "doc-1", "chunk-1");

        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target, "架构设计");
    }

    // ───────────────────────── 代码块跳过 ─────────────────────────

    /// TC-ING-WIKI-006：代码块中的 `[[link]]` 不被解析
    #[test]
    fn tc_wiki_006_skip_code_block() {
        let text = "正文中有 [[真实链接]]。\n\n```\n这是代码 [[不应解析]]\n```\n\n后续文本。";
        let links = parse_wiki_links(text, "doc-1", "chunk-1");

        assert_eq!(links.len(), 1, "代码块中的 wiki-link 不应被解析");
        assert_eq!(links[0].target, "真实链接");
    }

    /// TC-ING-WIKI-006b：行内代码中的 `[[link]]` 不被解析
    #[test]
    fn tc_wiki_006b_skip_inline_code() {
        let text = "正文 [[真实链接]] 和行内代码 `[[不应解析]]`。";
        let links = parse_wiki_links(text, "doc-1", "chunk-1");

        assert_eq!(links.len(), 1, "行内代码中的 wiki-link 不应被解析");
        assert_eq!(links[0].target, "真实链接");
    }

    // ───────────────────────── 去重和边界 ─────────────────────────

    /// TC-ING-WIKI-007：同 chunk 内重复 target 去重
    #[test]
    fn tc_wiki_007_dedup_same_chunk() {
        let text = "参见 [[文档A]] 和 [[文档A]] 以及 [[文档A]]。";
        let links = parse_wiki_links(text, "doc-1", "chunk-1");

        assert_eq!(links.len(), 1, "同一 chunk 内重复 target 只保留一个");
    }

    /// TC-ING-WIKI-008：空文本返回空列表
    #[test]
    fn tc_wiki_008_empty_text() {
        assert!(parse_wiki_links("", "doc-1", "chunk-1").is_empty());
        assert!(parse_wiki_links("   ", "doc-1", "chunk-1").is_empty());
    }

    /// TC-ING-WIKI-009：无 wiki-link 的文本返回空列表
    #[test]
    fn tc_wiki_009_no_links() {
        let text = "这是一段普通文本，没有任何 wiki-link 语法。";
        assert!(parse_wiki_links(text, "doc-1", "chunk-1").is_empty());
    }

    /// TC-ING-WIKI-010：未闭合的 `[[` 不被解析
    #[test]
    fn tc_wiki_010_unclosed_link() {
        let text = "未闭合的链接 [[不完整";
        assert!(parse_wiki_links(text, "doc-1", "chunk-1").is_empty());
    }

    /// TC-ING-WIKI-011：空 target `[[ ]]` 不被解析
    #[test]
    fn tc_wiki_011_empty_target() {
        let text = "空链接 [[ ]] 和 [[]] 不应解析。";
        assert!(parse_wiki_links(text, "doc-1", "chunk-1").is_empty());
    }

    /// TC-ING-WIKI-012：纯英文 wiki-link
    #[test]
    fn tc_wiki_012_english_target() {
        let text = "See [[API Design]] and [[Database Schema]] for details.";
        let links = parse_wiki_links(text, "doc-1", "chunk-1");

        assert_eq!(links.len(), 2);
        let targets: Vec<&str> = links.iter().map(|l| l.target.as_str()).collect();
        assert!(targets.contains(&"API Design"));
        assert!(targets.contains(&"Database Schema"));
    }

    /// TC-ING-WIKI-013：波浪号代码块中的 `[[link]]` 不被解析
    #[test]
    fn tc_wiki_013_tilde_code_block() {
        let text = "正文 [[真实链接]]。\n\n~~~\n代码 [[不应解析]]\n~~~\n后续。";
        let links = parse_wiki_links(text, "doc-1", "chunk-1");

        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target, "真实链接");
    }

    /// TC-ING-WIKI-014：WikiLink ID 和时间戳自动生成
    #[test]
    fn tc_wiki_014_auto_id_and_timestamp() {
        let text = "[[测试文档]]";
        let links = parse_wiki_links(text, "doc-1", "chunk-1");

        assert_eq!(links.len(), 1);
        assert!(!links[0].id.is_empty(), "ID 应自动生成");
        assert!(links[0].created_at > 0, "时间戳应为有效值");
    }

    /// TC-ING-WIKI-015：多行文本混合解析
    #[test]
    fn tc_wiki_015_multiline_mixed() {
        let text = r#"
# 文档标题

正文引用 [[第一篇]] 和 [[第二篇]]。

```
代码块中的 [[不应解析]]
```

行内 `[[也不应解析]]` 代码。

再次引用 [[第三篇]]。
"#;
        let links = parse_wiki_links(text, "doc-1", "chunk-1");

        assert_eq!(links.len(), 3, "应解析出 3 个有效 wiki-link");
        let targets: Vec<&str> = links.iter().map(|l| l.target.as_str()).collect();
        assert!(targets.contains(&"第一篇"));
        assert!(targets.contains(&"第二篇"));
        assert!(targets.contains(&"第三篇"));
    }
}
