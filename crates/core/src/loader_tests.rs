#![allow(clippy::unwrap_used)]
//! QA 红灯测试：TC-ING-005 / TC-ING-006 文档加载器（REQ-ING-001 加载链路旁证）。

use crate::Loader;
use crate::loader::{MarkdownLoader, TextLoader};
use tempfile::TempDir;

/// TC-ING-005 Markdown 加载器：剥离标记符号、保留正文与代码块内容。
#[tokio::test]
async fn tc_ing_005_markdown_loader_extracts_plain_text() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("note.md");
    std::fs::write(
        &path,
        "# 标题\n\n正文 **加粗**。\n\n```rust\nfn main() {}\n```\n",
    )
    .unwrap();

    let text = MarkdownLoader.load(path.to_str().unwrap()).await.unwrap();

    assert!(text.contains("标题"), "标题文本必须保留");
    assert!(text.contains("正文"), "正文必须保留");
    assert!(text.contains("加粗"), "强调内容必须保留");
    assert!(text.contains("fn main()"), "代码块内容必须保留");
    assert!(!text.contains("**"), "强调标记必须被剥离");
    assert!(!text.contains("```"), "代码围栏必须被剥离");
}

/// TC-ING-006 纯文本加载器：内容原样读取。
#[tokio::test]
async fn tc_ing_006_text_loader_reads_verbatim() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("note.txt");
    std::fs::write(&path, "纯文本内容\n第二行").unwrap();

    let text = TextLoader.load(path.to_str().unwrap()).await.unwrap();

    assert_eq!(text, "纯文本内容\n第二行");
}
