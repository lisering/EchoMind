//! 文档加载适配器（REQ-ING-001/002）：Markdown 与纯文本。
//! pulldown-cmark 为纯计算解析库（无 I/O、无网络），经架构评审准入 core。
//! 体系三：Markdown 解析为 CPU 密集任务，经 `spawn_blocking` 执行，不阻塞 async executor。

use anyhow::Context;
use pulldown_cmark::{Event, Parser, TagEnd};

use crate::Loader;

/// Markdown 加载器：剥离标记符号，保留正文与代码块内容（TC-ING-005 断言依据）。
pub struct MarkdownLoader;

impl MarkdownLoader {
    /// 同步提取纯文本（供 `spawn_blocking` 调用）。
    fn extract_text(markdown: &str) -> String {
        let parser = Parser::new(markdown);
        let mut out = String::with_capacity(markdown.len());
        for event in parser {
            match event {
                Event::Text(t) | Event::Code(t) => out.push_str(&t),
                Event::SoftBreak | Event::HardBreak => out.push('\n'),
                Event::End(
                    TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::CodeBlock | TagEnd::Item,
                ) => out.push('\n'),
                _ => {}
            }
        }
        out.trim().to_string()
    }
}

impl Loader for MarkdownLoader {
    async fn load(&self, file_path: &str) -> anyhow::Result<String> {
        let raw = tokio::fs::read_to_string(file_path)
            .await
            .with_context(|| format!("读取 Markdown 失败: {file_path}"))?;
        tokio::task::spawn_blocking(move || Self::extract_text(&raw))
            .await
            .context("Markdown 解析任务执行失败")
    }
}

/// 纯文本加载器：内容原样读取（TC-ING-006 断言依据）。
pub struct TextLoader;

impl Loader for TextLoader {
    async fn load(&self, file_path: &str) -> anyhow::Result<String> {
        tokio::fs::read_to_string(file_path)
            .await
            .with_context(|| format!("读取文本失败: {file_path}"))
    }
}
