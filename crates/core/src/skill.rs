//! Skill 系统发现（B09 Skill Discovery，REQ-ARCH-010）。
//!
//! 借鉴 OpenCode `skill.ts`：从 Markdown 文件中发现技能，
//! 解析 YAML frontmatter 中的 `name` / `description` / `slash` 字段，
//! 提取 Markdown 正文作为技能内容。
//!
//! ## 设计
//!
//! - **纯函数解析**：`parse_skill()` 是纯计算函数，零 I/O、零 async，可独立单元测试
//! - **YAML frontmatter**：`---\n` 开头，`\n---` 结尾的 YAML 块
//! - **必需字段**：`name` 是必需的，缺失则返回 `None`；`description` 和 `slash` 可选
//! - **异步目录扫描**：`discover_from_dir()` 使用 `tokio::fs` 异步读取目录
//! - **零新依赖**：不引入 YAML crate，用简单字符串解析（frontmatter 字段格式简单）

use serde::{Deserialize, Serialize};

/// 技能信息（从 Markdown 文件的 YAML frontmatter 解析）。
///
/// 一个 Skill 对应一个 `.md` 文件，文件以 `---\n` 开头的 YAML frontmatter 块开始，
/// 包含 `name`（必需）、`description`（可选）、`slash`（可选，默认 false）字段。
/// frontmatter 之后的 Markdown 正文即为技能内容。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Skill {
    /// 技能名称（frontmatter `name` 字段，必需）
    pub name: String,
    /// 技能描述（frontmatter `description` 字段，可选）
    pub description: Option<String>,
    /// 是否注册为 /slash 命令（frontmatter `slash` 字段，默认 false）
    pub slash: bool,
    /// Markdown 正文（frontmatter 之后的内容）
    pub content: String,
    /// 源文件路径（`discover_from_dir` 时设置，纯解析时为 None）
    pub source_path: Option<String>,
}

/// 从 Markdown 内容解析 Skill。
///
/// 文件格式：
/// ```markdown
/// ---
/// name: my-skill
/// description: 一个有用的技能
/// slash: true
/// ---
/// # 技能正文
/// 这里是技能的 Markdown 内容...
/// ```
///
/// # 解析规则
///
/// 1. 文件必须以 `---\n` 开头（YAML frontmatter 起始标记）
/// 2. 在 frontmatter 内查找 `\n---` 作为结束标记
/// 3. `name` 字段必需，缺失则返回 `None`
/// 4. `description` 和 `slash` 可选
/// 5. frontmatter 之后的内容为 Skill 正文
///
/// # 参数
/// - `content`: Markdown 文件内容
/// - `source`: 源文件路径（用于 `source_path` 字段）
///
/// # 返回
/// - `Some(Skill)`：解析成功
/// - `None`：无 frontmatter、缺 `name` 字段、或格式错误
///
/// # 示例
///
/// ```
/// use echomind_core::skill::parse_skill;
///
/// let md = "---\nname: hello\ndescription: A greeting skill\nslash: true\n---\n# Hello\nSay hi!";
/// let skill = parse_skill(md, "/path/to/SKILL.md").unwrap();
/// assert_eq!(skill.name, "hello");
/// assert_eq!(skill.description.as_deref(), Some("A greeting skill"));
/// assert!(skill.slash);
/// assert_eq!(skill.content, "# Hello\nSay hi!");
/// ```
pub fn parse_skill(content: &str, source: &str) -> Option<Skill> {
    // 必须以 "---\n" 开头
    if !content.starts_with("---\n") {
        return None;
    }

    // 查找 frontmatter 结束标记 "\n---"
    let after_start = &content[4..];
    let end_pos = after_start.find("\n---")?;
    let frontmatter = &after_start[..end_pos];

    // 正文是 frontmatter 结束标记之后的内容
    // "\n---" 之后可能有 "\n" 或 "\r\n"，跳过
    let body_start = 4 + end_pos + 4; // "---\n".len() + end_pos + "\n---".len()
    let body = if body_start < content.len() {
        // 跳过结束标记后的换行符
        let remaining = &content[body_start..];
        if let Some(stripped) = remaining.strip_prefix("\r\n") {
            stripped
        } else if let Some(stripped) = remaining.strip_prefix('\n') {
            stripped
        } else {
            remaining
        }
    } else {
        ""
    };

    // 解析 frontmatter 字段
    let name = extract_yaml_field(frontmatter, "name")?;
    let description = extract_yaml_field(frontmatter, "description");
    let slash = extract_yaml_field(frontmatter, "slash")
        .map(|v| v.trim() == "true")
        .unwrap_or(false);

    Some(Skill {
        name,
        description,
        slash,
        content: body.to_string(),
        source_path: Some(source.to_string()),
    })
}

/// 从简单 YAML 文本中提取字段值（`key: value` 格式）。
///
/// 仅支持简单标量值，不支持嵌套结构或数组。
/// 值两端的空白和引号会被去除。
fn extract_yaml_field(yaml: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}:");
    for line in yaml.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(&prefix) {
            let value = trimmed[prefix.len()..].trim();
            // 去除引号
            let value = value.trim_matches('"').trim_matches('\'');
            if value.is_empty() {
                return None;
            }
            return Some(value.to_string());
        }
    }
    None
}

/// 从本地目录异步发现 Skill。
///
/// 扫描指定目录中的所有 `.md` 文件，尝试解析每个文件为 Skill。
/// 只有包含有效 YAML frontmatter 且 `name` 字段存在的文件才会被返回。
///
/// # 参数
/// - `dir`: 要扫描的目录路径
///
/// # 返回
/// - `Ok(Vec<Skill>)`：成功扫描的技能列表（可能为空）
/// - `Err`：目录读取失败
///
/// # 示例
///
/// ```no_run
/// use echomind_core::skill::discover_from_dir;
/// use std::path::Path;
///
/// # async fn example() -> anyhow::Result<()> {
/// let skills = discover_from_dir(Path::new("/path/to/skills")).await?;
/// println!("发现 {} 个技能", skills.len());
/// # Ok(())
/// # }
/// ```
pub async fn discover_from_dir(dir: &std::path::Path) -> anyhow::Result<Vec<Skill>> {
    let mut skills = Vec::new();

    let mut entries = tokio::fs::read_dir(dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        // 只处理 .md 文件
        if path.extension().is_some_and(|ext| ext == "md") {
            let content = tokio::fs::read_to_string(&path).await?;
            let source = path.to_string_lossy();
            if let Some(skill) = parse_skill(&content, &source) {
                skills.push(skill);
            }
        }
    }

    Ok(skills)
}

// ============================================================================
// TDD 测试（TC-SKILL-001~005，对应 REQ-ARCH-010 AC-1~AC-5）
// ============================================================================

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    /// TC-SKILL-001：包含 YAML frontmatter 的 Markdown 文件正确解析（AC-1）。
    ///
    /// 验证 `name`、`description`、`content` 字段正确提取。
    #[test]
    fn tc_skill_001_parse_valid_skill() {
        let md = "---\nname: code-reviewer\ndescription: A code review skill\n---\n# Code Reviewer\nYou are a code reviewer.";
        let skill = parse_skill(md, "/skills/reviewer.md");
        assert!(skill.is_some(), "有效 frontmatter 应解析成功");
        let skill = skill.unwrap();
        assert_eq!(skill.name, "code-reviewer");
        assert_eq!(skill.description.as_deref(), Some("A code review skill"));
        assert!(!skill.slash, "默认 slash 应为 false");
        assert_eq!(skill.content, "# Code Reviewer\nYou are a code reviewer.");
        assert_eq!(skill.source_path.as_deref(), Some("/skills/reviewer.md"));
    }

    /// TC-SKILL-002：`slash: true` 字段正确解析（AC-2）。
    #[test]
    fn tc_skill_002_parse_slash_true() {
        let md = "---\nname: summarize\nslash: true\n---\nSummarize the text.";
        let skill = parse_skill(md, "/skills/summarize.md");
        assert!(skill.is_some());
        let skill = skill.unwrap();
        assert_eq!(skill.name, "summarize");
        assert!(skill.slash, "slash 应为 true");
        assert_eq!(skill.content, "Summarize the text.");
    }

    /// TC-SKILL-003：无 frontmatter 的 Markdown 文件返回 None（AC-3）。
    #[test]
    fn tc_skill_003_no_frontmatter_returns_none() {
        let md = "# Plain Markdown\nNo frontmatter here.";
        let skill = parse_skill(md, "/docs/plain.md");
        assert!(skill.is_none(), "无 frontmatter 应返回 None");
    }

    /// TC-SKILL-004：frontmatter 缺少 `name` 字段时返回 None（AC-4）。
    #[test]
    fn tc_skill_004_missing_name_returns_none() {
        let md = "---\ndescription: Missing name field\n---\nSome content.";
        let skill = parse_skill(md, "/skills/noname.md");
        assert!(skill.is_none(), "缺少 name 字段应返回 None");
    }

    /// TC-SKILL-005：`discover_from_dir()` 扫描目录返回 Skill 列表（AC-5）。
    #[test]
    fn tc_skill_005_discover_from_dir() {
        let rt = tokio::runtime::Runtime::new().unwrap();

        let dir = std::env::temp_dir().join("echomind_skill_test");
        std::fs::create_dir_all(&dir).unwrap();

        // 写入两个有效 Skill 文件 + 一个无效文件
        std::fs::write(
            dir.join("skill1.md"),
            "---\nname: alpha\ndescription: First skill\n---\n# Alpha\nContent A",
        )
        .unwrap();
        std::fs::write(
            dir.join("skill2.md"),
            "---\nname: beta\nslash: true\n---\n# Beta\nContent B",
        )
        .unwrap();
        std::fs::write(dir.join("plain.md"), "# No frontmatter\nJust text").unwrap();

        let skills = rt
            .block_on(discover_from_dir(&dir))
            .expect("目录扫描不应失败");

        assert_eq!(skills.len(), 2, "应发现 2 个有效 Skill（跳过无效文件）");

        // 按 name 排序便于断言
        let mut names: Vec<_> = skills.iter().map(|s| s.name.as_str()).collect();
        names.sort();
        assert_eq!(names, ["alpha", "beta"]);

        // 清理
        std::fs::remove_dir_all(&dir).ok();
    }
}
