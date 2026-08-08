//! 对话导出模块（REQ-EXP-001）。
//!
//! 将会话（Conversation）及其消息（ChatMessage）导出为 Markdown 文件。
//! 导出内容包括：会话标题、创建时间、全部消息（user / assistant 按序排列），
//! 每条 assistant 消息附带引用来源列表（文档名 + chunk 序号）。
//! 代码块、Mermaid 语法、LaTeX 公式以原始 Markdown 语法保留。

use echomind_models::{ChatMessage, Conversation};

/// 格式化 Unix 时间戳为可读日期时间字符串（UTC ISO 8601）。
///
/// 使用 `chrono` 格式化为 `YYYY-MM-DD HH:MM:SS UTC` 格式，
/// 便于导出文件中的时间可读性。
fn format_timestamp(ts: i64) -> String {
    let dt = chrono::DateTime::from_timestamp(ts, 0).unwrap_or_else(chrono::Utc::now);
    dt.format("%Y-%m-%d %H:%M:%S UTC").to_string()
}

/// 将会话及其消息导出为 Markdown 字符串（REQ-EXP-001）。
///
/// # 参数
/// - `conversation`: 会话元数据（标题、创建时间）
/// - `messages`: 会话全部消息（按写入顺序正序排列）
///
/// # 返回
/// 完整的 Markdown 字符串，包含：
/// 1. 会话标题（H1）
/// 2. 创建时间
/// 3. 全部消息（user / assistant 按序交替），角色标记清晰
/// 4. 每条 assistant 消息末尾附带引用来源列表（文档名 + chunk 序号）
/// 5. 代码块、Mermaid 语法、LaTeX 公式以原始 Markdown 语法保留
///
/// # 错误处理
/// 本函数为纯函数，不返回 `Result`——空消息列表产生仅含标题和时间的 Markdown。
pub fn export_conversation_to_markdown(
    conversation: &Conversation,
    messages: &[ChatMessage],
) -> String {
    let mut md = String::new();

    // AC-2: 导出的 Markdown 文件包含会话标题、创建时间、全部消息
    md.push_str(&format!("# {}\n\n", conversation.title));
    md.push_str(&format!(
        "> **{}** {}\n\n",
        "Created at",
        format_timestamp(conversation.created_at)
    ));
    md.push_str("---\n\n");

    // AC-3: 消息按 user / assistant 交替排列，角色标记清晰
    for msg in messages {
        match msg.role.as_str() {
            "user" => {
                md.push_str("## 🧑 User\n\n");
                md.push_str(&msg.content);
                md.push_str("\n\n");
            }
            "assistant" => {
                md.push_str("## 🤖 Assistant\n\n");
                // AC-6: 代码块、Mermaid 语法、LaTeX 公式以原始 Markdown 语法保留
                md.push_str(&msg.content);
                md.push_str("\n\n");

                // AC-4: Assistant 消息末尾附带引用来源列表（文档名 + chunk 序号）
                if let Some(sources) = &msg.sources
                    && !sources.is_empty()
                {
                    md.push_str("**📊 Citation Sources**\n\n");
                    for (idx, src) in sources.iter().enumerate() {
                        md.push_str(&format!(
                            "{}. **{}** (chunk #{}, score: {:.2})\n",
                            idx + 1,
                            src.doc_name,
                            src.chunk.sequence,
                            src.score
                        ));
                    }
                    md.push('\n');
                }
            }
            "system" => {
                // 系统消息不导出（内部使用）
            }
            _ => {
                md.push_str(&format!("## {}\n\n", msg.role));
                md.push_str(&msg.content);
                md.push_str("\n\n");
            }
        }
    }

    md.push_str("---\n\n");
    md.push_str(&format!(
        "*Exported by EchoMind · {}*\n",
        format_timestamp(chrono::Utc::now().timestamp())
    ));

    md
}

/// 从会话标题生成安全的文件名（替换文件系统不安全字符）。
///
/// 将 `/ \ : * ? " < > |` 等不安全字符替换为 `_`，
/// 去除首尾空格和点号，空标题回退为 `conversation`。
pub fn sanitize_filename(title: &str) -> String {
    let sanitized: String = title
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect();

    let trimmed = sanitized.trim().trim_matches('.');

    if trimmed.is_empty() {
        "conversation".to_string()
    } else {
        trimmed.to_string()
    }
}
