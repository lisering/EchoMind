#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! QA 红灯测试：TC-ING-005 / TC-ING-006 文档加载器（REQ-ING-001 加载链路旁证）。

use crate::Loader;
use crate::loader::{
    CsvLoader, DocxLoader, EpubLoader, HtmlLoader, MarkdownLoader, PdfLoader, PptxLoader,
    TextLoader, XlsxLoader,
};
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

/// TC-PDF-001 PDF 加载器容错：伪造字节流不得 Panic，必须返回 Result（REQ-PDF-001-AC-1）。
#[tokio::test]
async fn tc_pdf_001_fake_pdf_never_panics() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("fake.pdf");
    std::fs::write(&path, b"%PDF-1.7\nthis is not a real pdf body\n%%EOF").unwrap();

    // 执行到这里即证明未 Panic；Ok（容错提取）与 Err（优雅失败）均合规
    let result = PdfLoader.load(path.to_str().unwrap()).await;
    if let Err(err) = &result {
        assert!(!err.to_string().is_empty(), "Err 必须携带可读原因");
    }
}

// ---- TC-ING-DOCX-001 ~ TC-ING-DOCX-004 .docx 文档加载器（REQ-ING-015）----

/// TC-ING-DOCX-001 简单段落 .docx 导入：提取文本包含全部段落，段落间双换行。
#[tokio::test]
async fn tc_ing_docx_001_simple_paragraphs() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("simple.docx");
    let file = std::fs::File::create(&path).unwrap();
    docx_rs::Docx::new()
        .add_paragraph(
            docx_rs::Paragraph::new().add_run(docx_rs::Run::new().add_text("Hello World")),
        )
        .add_paragraph(
            docx_rs::Paragraph::new().add_run(docx_rs::Run::new().add_text("Second paragraph")),
        )
        .build()
        .pack(file)
        .unwrap();

    let text = DocxLoader.load(path.to_str().unwrap()).await.unwrap();

    assert!(text.contains("Hello World"), "第一段文本必须保留");
    assert!(text.contains("Second paragraph"), "第二段文本必须保留");
    assert!(text.contains('\n'), "段落间必须有换行分隔");
}

/// TC-ING-DOCX-002 含表格 .docx 导入：表格转换为 Markdown 表格语法。
#[tokio::test]
async fn tc_ing_docx_002_table_to_markdown() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("table.docx");
    let file = std::fs::File::create(&path).unwrap();
    let table = docx_rs::Table::new(vec![
        docx_rs::TableRow::new(vec![
            docx_rs::TableCell::new().add_paragraph(
                docx_rs::Paragraph::new().add_run(docx_rs::Run::new().add_text("Header A")),
            ),
            docx_rs::TableCell::new().add_paragraph(
                docx_rs::Paragraph::new().add_run(docx_rs::Run::new().add_text("Header B")),
            ),
        ]),
        docx_rs::TableRow::new(vec![
            docx_rs::TableCell::new().add_paragraph(
                docx_rs::Paragraph::new().add_run(docx_rs::Run::new().add_text("Cell 1")),
            ),
            docx_rs::TableCell::new().add_paragraph(
                docx_rs::Paragraph::new().add_run(docx_rs::Run::new().add_text("Cell 2")),
            ),
        ]),
    ]);
    docx_rs::Docx::new()
        .add_table(table)
        .build()
        .pack(file)
        .unwrap();

    let text = DocxLoader.load(path.to_str().unwrap()).await.unwrap();

    assert!(
        text.contains('|'),
        "表格必须转换为 Markdown 表格语法（含 | 分隔符）"
    );
    assert!(text.contains("Header A"), "表头单元格文本必须保留");
    assert!(text.contains("Header B"), "表头单元格文本必须保留");
    assert!(text.contains("Cell 1"), "数据单元格文本必须保留");
    assert!(text.contains("Cell 2"), "数据单元格文本必须保留");
    assert!(text.contains("---"), "Markdown 表格分隔行必须存在");
}

/// TC-ING-DOCX-003 含标题层级 .docx 导入：标题文本被提取，段落边界正确。
#[tokio::test]
async fn tc_ing_docx_003_heading_levels() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("headings.docx");
    let file = std::fs::File::create(&path).unwrap();
    docx_rs::Docx::new()
        .add_paragraph(
            docx_rs::Paragraph::new()
                .style("Heading1")
                .add_run(docx_rs::Run::new().add_text("Main Title")),
        )
        .add_paragraph(
            docx_rs::Paragraph::new()
                .style("Heading2")
                .add_run(docx_rs::Run::new().add_text("Subtitle")),
        )
        .add_paragraph(
            docx_rs::Paragraph::new()
                .add_run(docx_rs::Run::new().add_text("Body content paragraph")),
        )
        .build()
        .pack(file)
        .unwrap();

    let text = DocxLoader.load(path.to_str().unwrap()).await.unwrap();

    assert!(text.contains("Main Title"), "Heading1 标题文本必须保留");
    assert!(text.contains("Subtitle"), "Heading2 标题文本必须保留");
    assert!(text.contains("Body content paragraph"), "正文段落必须保留");
    assert!(text.contains('\n'), "段落间必须有换行分隔");
}

/// TC-ING-DOCX-004 损坏 .docx 文件错误处理：返回 Err，不 Panic。
#[tokio::test]
async fn tc_ing_docx_004_corrupt_file_error() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("corrupt.docx");
    std::fs::write(&path, b"not a docx file").unwrap();

    let result = DocxLoader.load(path.to_str().unwrap()).await;

    assert!(result.is_err(), "损坏的 .docx 必须返回 Err");
    let err_msg = result.err().unwrap().to_string();
    assert!(
        err_msg.contains("解析 .docx 失败"),
        "错误信息必须包含 '解析 .docx 失败'，实际: {err_msg}"
    );
}

// ---- TC-ING-HTML-001 ~ TC-ING-HTML-004 .html 文档加载器（REQ-ING-016）----

/// TC-ING-HTML-001 简单 HTML 导入：提取正文文本，去除 `<script>`/`<style>` 内容（AC-1）。
#[tokio::test]
async fn tc_ing_html_001_simple_html_removes_script_style() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("simple.html");
    std::fs::write(
        &path,
        "<html><body>".to_string()
            + "<script>alert('xss')</script>"
            + "<style>body { color: red; }</style>"
            + "<p>Hello World</p>"
            + "<p>Second paragraph</p>"
            + "</body></html>",
    )
    .unwrap();

    let text = HtmlLoader.load(path.to_str().unwrap()).await.unwrap();

    assert!(text.contains("Hello World"), "正文段落必须保留");
    assert!(text.contains("Second paragraph"), "第二段必须保留");
    assert!(!text.contains("alert"), "script 内容必须被去除");
    assert!(!text.contains("color: red"), "style 内容必须被去除");
    assert!(!text.contains("<script"), "script 标签必须被去除");
    assert!(!text.contains("<style"), "style 标签必须被去除");
}

/// TC-ING-HTML-002 含标题层级 HTML 导入：`<h1>`~`<h3>` 转换为 `#`~`###`（AC-2）。
#[tokio::test]
async fn tc_ing_html_002_heading_levels_to_markdown() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("headings.html");
    std::fs::write(
        &path,
        "<html><body>".to_string()
            + "<h1>Main Title</h1>"
            + "<h2>Subtitle</h2>"
            + "<h3>Section Heading</h3>"
            + "<p>Body content paragraph</p>"
            + "</body></html>",
    )
    .unwrap();

    let text = HtmlLoader.load(path.to_str().unwrap()).await.unwrap();

    assert!(text.contains("# Main Title"), "h1 必须转换为 # 前缀");
    assert!(text.contains("## Subtitle"), "h2 必须转换为 ## 前缀");
    assert!(
        text.contains("### Section Heading"),
        "h3 必须转换为 ### 前缀"
    );
    assert!(text.contains("Body content paragraph"), "正文段落必须保留");
}

/// TC-ING-HTML-003 含导航栏 HTML 导入：`<nav>` 内容被去除（AC-3）。
#[tokio::test]
async fn tc_ing_html_003_nav_content_removed() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("nav.html");
    std::fs::write(
        &path,
        "<html><body>"
            .to_string()
            + "<nav><a href='/'>Home</a><a href='/about'>About</a><a href='/contact'>Contact</a></nav>"
            + "<main><article><p>Article body text here.</p></article></main>"
            + "<footer>Copyright 2026</footer>"
            + "</body></html>",
    )
    .unwrap();

    let text = HtmlLoader.load(path.to_str().unwrap()).await.unwrap();

    assert!(text.contains("Article body text here."), "正文内容必须保留");
    assert!(!text.contains("Home"), "nav 内容必须被去除");
    assert!(!text.contains("About"), "nav 内容必须被去除");
    assert!(!text.contains("Contact"), "nav 内容必须被去除");
    assert!(!text.contains("Copyright"), "footer 内容必须被去除");
}

/// TC-ING-HTML-004 损坏 HTML 文件容错：未闭合标签不崩溃，提取文本（AC-4）。
#[tokio::test]
async fn tc_ing_html_004_corrupt_html_tolerant() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("corrupt.html");
    std::fs::write(&path, "<html><body><p>Unclosed paragraph").unwrap();

    let result = HtmlLoader.load(path.to_str().unwrap()).await;

    assert!(result.is_ok(), "损坏的 HTML 必须容错解析，不返回 Err");
    let text = result.unwrap();
    assert!(
        text.contains("Unclosed paragraph"),
        "即使标签未闭合，文本内容也必须被提取"
    );
}

// ---- TC-ING-PPTX-001 ~ TC-ING-PPTX-004 .pptx 演示文稿加载器（REQ-ING-017）----

/// 获取测试 fixture 文件路径（相对于 crate 根目录的 ../../tests/fixtures/）。
fn fixture_path(filename: &str) -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    format!("{manifest_dir}/../../tests/fixtures/{filename}")
}

/// TC-ING-PPTX-001 简单幻灯片 .pptx 导入：提取标题 + 要点文本（AC-1）。
#[tokio::test]
async fn tc_ing_pptx_001_simple_slides() {
    let path = fixture_path("test_simple.pptx");

    let text = PptxLoader
        .load(&path)
        .await
        .unwrap_or_else(|e| panic!("加载 .pptx 失败: {e}"));

    assert!(
        text.contains("Test Title"),
        "标题文本必须保留，实际: {text}"
    );
    assert!(text.contains("Bullet One"), "要点一必须保留，实际: {text}");
    assert!(text.contains("Bullet Two"), "要点二必须保留，实际: {text}");
}

/// TC-ING-PPTX-002 含表格 .pptx 导入：表格转换为 Markdown 表格语法（AC-2）。
#[tokio::test]
async fn tc_ing_pptx_002_table_to_markdown() {
    let path = fixture_path("test_table.pptx");

    let text = PptxLoader
        .load(&path)
        .await
        .unwrap_or_else(|e| panic!("加载 .pptx 失败: {e}"));

    assert!(
        text.contains("Header A"),
        "表头单元格文本必须保留，实际: {text}"
    );
    assert!(
        text.contains("Header B"),
        "表头单元格文本必须保留，实际: {text}"
    );
    assert!(
        text.contains("Cell 1"),
        "数据单元格文本必须保留，实际: {text}"
    );
    assert!(
        text.contains("Cell 2"),
        "数据单元格文本必须保留，实际: {text}"
    );
}

/// TC-ING-PPTX-003 含图片 .pptx 导入：图片不崩溃，文本正常提取（AC-3）。
#[tokio::test]
async fn tc_ing_pptx_003_image_slide_no_crash() {
    let path = fixture_path("test_image.pptx");

    // .pptx 含图片引用但无实际图片二进制数据 → 库应优雅跳过，不 Panic
    let result = PptxLoader.load(&path).await;

    // 只要不 Panic 就通过；Ok（提取到文本）与 Err（优雅失败）均合规
    if let Ok(text) = &result {
        assert!(!text.trim().is_empty(), "即使含图片，提取的文本不得为空");
    } else {
        let err = result.err().unwrap();
        assert!(!err.to_string().is_empty(), "Err 必须携带可读原因");
    }
}

/// TC-ING-PPTX-004 损坏 .pptx 文件错误处理：返回 Err，不 Panic（AC-4）。
#[tokio::test]
async fn tc_ing_pptx_004_corrupt_file_error() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("corrupt.pptx");
    std::fs::write(&path, b"not a pptx file").unwrap();

    let result = PptxLoader.load(path.to_str().unwrap()).await;

    assert!(result.is_err(), "损坏的 .pptx 必须返回 Err");
}

// ---- TC-ING-EPUB-001 ~ TC-ING-EPUB-004 .epub 电子书加载器（REQ-ING-018）----

/// TC-ING-EPUB-001 简单章节 .epub 导入：提取章节文本（AC-1）。
#[tokio::test]
async fn tc_ing_epub_001_simple_chapter() {
    let path = fixture_path("test_simple.epub");

    let text = EpubLoader
        .load(&path)
        .await
        .unwrap_or_else(|e| panic!("加载 .epub 失败: {e}"));

    assert!(
        text.contains("first paragraph"),
        "第一段文本必须保留，实际: {text}"
    );
    assert!(
        text.contains("second paragraph"),
        "第二段文本必须保留，实际: {text}"
    );
}

/// TC-ING-EPUB-002 多章节 .epub 导入：章节按 spine 顺序提取，章节间分隔（AC-2）。
#[tokio::test]
async fn tc_ing_epub_002_multi_chapter_spine_order() {
    let path = fixture_path("test_multi.epub");

    let text = EpubLoader
        .load(&path)
        .await
        .unwrap_or_else(|e| panic!("加载 .epub 失败: {e}"));

    assert!(
        text.contains("Chapter One"),
        "第一章标题必须保留，实际: {text}"
    );
    assert!(
        text.contains("Chapter Two"),
        "第二章标题必须保留，实际: {text}"
    );
    assert!(
        text.contains("Chapter Three"),
        "第三章标题必须保留，实际: {text}"
    );
    assert!(
        text.contains("Section 2.1"),
        "子章节标题必须保留，实际: {text}"
    );
    // 章节间以 --- 分隔
    assert!(
        text.contains("---"),
        "章节间必须有 --- 分隔符，实际: {text}"
    );
    // 验证 spine 顺序：Chapter One 在 Chapter Two 之前
    let pos_one = text.find("Chapter One");
    let pos_two = text.find("Chapter Two");
    assert!(
        pos_one.is_some() && pos_two.is_some() && pos_one < pos_two,
        "章节必须按 spine 顺序提取：Chapter One 应在 Chapter Two 之前"
    );
}

/// TC-ING-EPUB-003 含 HTML 标签 .epub 导入：HTML 标签被剥离，纯文本保留（AC-3）。
#[tokio::test]
async fn tc_ing_epub_003_html_tags_stripped() {
    let path = fixture_path("test_html.epub");

    let text = EpubLoader
        .load(&path)
        .await
        .unwrap_or_else(|e| panic!("加载 .epub 失败: {e}"));

    // HTML 标签应被剥离
    assert!(
        !text.contains("<b>"),
        "HTML <b> 标签必须被剥离，实际: {text}"
    );
    assert!(
        !text.contains("<i>"),
        "HTML <i> 标签必须被剥离，实际: {text}"
    );
    assert!(
        !text.contains("<a href"),
        "HTML <a> 标签必须被剥离，实际: {text}"
    );
    // 纯文本内容应保留
    assert!(
        text.contains("bold text"),
        "粗体文本内容必须保留，实际: {text}"
    );
    assert!(
        text.contains("italic text"),
        "斜体文本内容必须保留，实际: {text}"
    );
    assert!(
        text.contains("hyperlink"),
        "链接文本内容必须保留，实际: {text}"
    );
    assert!(
        text.contains("First item"),
        "列表项一必须保留，实际: {text}"
    );
    assert!(
        text.contains("Second item"),
        "列表项二必须保留，实际: {text}"
    );
}

/// TC-ING-EPUB-004 损坏 .epub 文件错误处理：返回 Err，不 Panic（AC-4）。
#[tokio::test]
async fn tc_ing_epub_004_corrupt_file_error() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("corrupt.epub");
    std::fs::write(&path, b"not a valid epub file").unwrap();

    let result = EpubLoader.load(path.to_str().unwrap()).await;

    assert!(result.is_err(), "损坏的 .epub 必须返回 Err");
}

// ---- TC-ING-XLSX-001 ~ TC-ING-XLSX-004 .xlsx 电子表格加载器（REQ-ING-021）----

/// TC-ING-XLSX-001 简单 .xlsx 导入：提取表头 + 数据行（AC-1）。
#[tokio::test]
async fn tc_ing_xlsx_001_simple_sheet() {
    let path = fixture_path("test_simple.xlsx");

    let text = XlsxLoader
        .load(&path)
        .await
        .unwrap_or_else(|e| panic!("加载 .xlsx 失败: {e}"));

    assert!(text.contains("Name"), "表头 Name 必须保留，实际: {text}");
    assert!(text.contains("Age"), "表头 Age 必须保留，实际: {text}");
    assert!(
        text.contains("Alice"),
        "数据行 Alice 必须保留，实际: {text}"
    );
    assert!(text.contains("Bob"), "数据行 Bob 必须保留，实际: {text}");
    assert!(
        text.contains("Charlie"),
        "数据行 Charlie 必须保留，实际: {text}"
    );
    assert!(text.contains("30"), "数值 30 必须保留，实际: {text}");
    assert!(text.contains("25"), "数值 25 必须保留，实际: {text}");
    // 必须包含 Markdown 表格语法
    assert!(text.contains("|"), "必须包含 Markdown 表格管道符");
    assert!(text.contains("---"), "必须包含 Markdown 表格分隔行");
}

/// TC-ING-XLSX-002 多 Sheet .xlsx 导入：所有 Sheet 内容提取（AC-2）。
#[tokio::test]
async fn tc_ing_xlsx_002_multi_sheet() {
    let path = fixture_path("test_multi_sheet.xlsx");

    let text = XlsxLoader
        .load(&path)
        .await
        .unwrap_or_else(|e| panic!("加载 .xlsx 失败: {e}"));

    // Sheet 1 内容
    assert!(
        text.contains("Employees"),
        "Sheet 名 Employees 必须保留，实际: {text}"
    );
    assert!(
        text.contains("Engineering"),
        "Sheet 1 数据必须保留，实际: {text}"
    );
    assert!(text.contains("Sales"), "Sheet 1 数据必须保留，实际: {text}");

    // Sheet 2 内容
    assert!(
        text.contains("Products"),
        "Sheet 名 Products 必须保留，实际: {text}"
    );
    assert!(
        text.contains("Widget"),
        "Sheet 2 数据必须保留，实际: {text}"
    );
    assert!(
        text.contains("Gadget"),
        "Sheet 2 数据必须保留，实际: {text}"
    );

    // Sheet 之间必须有分隔符
    assert!(text.contains("---"), "多 Sheet 之间必须有 --- 分隔符");
}

/// TC-ING-XLSX-003 空 Sheet .xlsx 不崩溃：返回 Err 或空（AC-3）。
#[tokio::test]
async fn tc_ing_xlsx_003_empty_sheet() {
    let path = fixture_path("test_empty.xlsx");

    let result = XlsxLoader.load(&path).await;

    // 空 Sheet 应返回 Err（无可提取文本）
    assert!(
        result.is_err(),
        "空 Sheet 的 .xlsx 必须返回 Err，实际: {result:?}"
    );
}

/// TC-ING-XLSX-004 损坏 .xlsx 文件错误处理：返回 Err，不 Panic（AC-4）。
#[tokio::test]
async fn tc_ing_xlsx_004_corrupt_file_error() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("corrupt.xlsx");
    std::fs::write(&path, b"not an xlsx file").unwrap();

    let result = XlsxLoader.load(path.to_str().unwrap()).await;

    assert!(result.is_err(), "损坏的 .xlsx 必须返回 Err");
}

// ---- TC-ING-CSV-001 ~ TC-ING-CSV-003 .csv 逗号分隔值加载器（REQ-ING-021）----

/// TC-ING-CSV-001 逗号分隔 .csv 导入：提取表头 + 数据行（AC-1）。
#[tokio::test]
async fn tc_ing_csv_001_comma_delimited() {
    let path = fixture_path("test_simple.csv");

    let text = CsvLoader
        .load(&path)
        .await
        .unwrap_or_else(|e| panic!("加载 .csv 失败: {e}"));

    assert!(text.contains("Name"), "表头 Name 必须保留，实际: {text}");
    assert!(text.contains("Age"), "表头 Age 必须保留，实际: {text}");
    assert!(text.contains("City"), "表头 City 必须保留，实际: {text}");
    assert!(
        text.contains("Alice"),
        "数据行 Alice 必须保留，实际: {text}"
    );
    assert!(
        text.contains("New York"),
        "数据行 New York 必须保留，实际: {text}"
    );
    assert!(
        text.contains("Charlie"),
        "数据行 Charlie 必须保留，实际: {text}"
    );
    assert!(
        text.contains("Tokyo"),
        "数据行 Tokyo 必须保留，实际: {text}"
    );
    // 必须包含 Markdown 表格语法
    assert!(text.contains("|"), "必须包含 Markdown 表格管道符");
    assert!(text.contains("---"), "必须包含 Markdown 表格分隔行");
}

/// TC-ING-CSV-002 分号分隔 .csv 导入：自动检测分隔符（AC-2）。
#[tokio::test]
async fn tc_ing_csv_002_semicolon_delimited() {
    let path = fixture_path("test_semicolon.csv");

    let text = CsvLoader
        .load(&path)
        .await
        .unwrap_or_else(|e| panic!("加载 .csv 失败: {e}"));

    assert!(text.contains("Name"), "表头 Name 必须保留，实际: {text}");
    assert!(text.contains("Age"), "表头 Age 必须保留，实际: {text}");
    assert!(
        text.contains("Alice"),
        "数据行 Alice 必须保留，实际: {text}"
    );
    assert!(
        text.contains("London"),
        "数据行 London 必须保留，实际: {text}"
    );
    // 分号分隔符应被正确解析，不应出现在输出中
    assert!(!text.contains(";"), "分号分隔符不应出现在输出中");
}

/// TC-ING-CSV-003 含空行 .csv 导入：跳过空行不崩溃（AC-3）。
#[tokio::test]
async fn tc_ing_csv_003_empty_lines() {
    let path = fixture_path("test_empty_lines.csv");

    let text = CsvLoader
        .load(&path)
        .await
        .unwrap_or_else(|e| panic!("加载 .csv 失败: {e}"));

    assert!(text.contains("Name"), "表头 Name 必须保留，实际: {text}");
    assert!(
        text.contains("Alice"),
        "数据行 Alice 必须保留，实际: {text}"
    );
    assert!(text.contains("Bob"), "数据行 Bob 必须保留，实际: {text}");
    // 空行不应产生额外的 Markdown 表格行
    assert!(!text.contains("|  |"), "空行不应产生空单元格行");
}

#[cfg(feature = "pro")]
mod multimodal_tests {
    use crate::Loader;
    use crate::loader::MultimodalPdfLoader;
    use crate::{OcrEngine, PageRenderer};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tempfile::TempDir;

    /// Mock 页面渲染器：返回 fake PNG 字节，计数调用次数。
    struct MockPageRenderer {
        /// render_page 被调用次数（测试断言用）
        render_calls: Arc<AtomicUsize>,
    }

    impl PageRenderer for MockPageRenderer {
        async fn render_page(
            &self,
            _pdf_path: &str,
            _page_num: usize,
            _dpi: u32,
        ) -> anyhow::Result<Vec<u8>> {
            self.render_calls.fetch_add(1, Ordering::Relaxed);
            // fake PNG header（OcrEngine 收到后不关心具体内容）
            Ok(vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A])
        }
    }

    /// Mock OCR 引擎：返回固定文本，计数调用次数。
    struct MockOcrEngine {
        /// recognize 被调用次数（测试断言用）
        recognize_calls: Arc<AtomicUsize>,
    }

    impl OcrEngine for MockOcrEngine {
        async fn recognize(&self, _image_bytes: &[u8]) -> anyhow::Result<String> {
            self.recognize_calls.fetch_add(1, Ordering::Relaxed);
            Ok("OCR提取的示例文本".to_string())
        }
    }

    /// TC-MM-001b fake PDF 容错：多模态加载器对伪造字节流不得 Panic，
    /// 且不应触发渲染/OCR（无法解析的 PDF 检测不到图片）。
    #[tokio::test]
    async fn tc_mm_001b_fake_pdf_never_panics_no_render() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("fake.pdf");
        std::fs::write(&path, b"%PDF-1.7\nfake body\n%%EOF").unwrap();

        let render_calls = Arc::new(AtomicUsize::new(0));
        let recognize_calls = Arc::new(AtomicUsize::new(0));
        let loader = MultimodalPdfLoader::new(
            MockPageRenderer {
                render_calls: render_calls.clone(),
            },
            MockOcrEngine {
                recognize_calls: recognize_calls.clone(),
            },
        );

        // 执行到这里即证明未 Panic
        let _result = loader.load(path.to_str().unwrap()).await;

        // fake PDF 无法被 lopdf 正确解析 → 检测不到图片 → 不触发渲染/OCR
        assert_eq!(
            render_calls.load(Ordering::Relaxed),
            0,
            "fake PDF 不应触发页面渲染"
        );
        assert_eq!(
            recognize_calls.load(Ordering::Relaxed),
            0,
            "fake PDF 不应触发 OCR"
        );
    }

    /// TC-MM-001c 不存在的文件返回错误：多模态加载器对不存在文件返回 Err，不 Panic。
    #[tokio::test]
    async fn tc_mm_001c_nonexistent_file_returns_error() {
        let render_calls = Arc::new(AtomicUsize::new(0));
        let recognize_calls = Arc::new(AtomicUsize::new(0));
        let loader = MultimodalPdfLoader::new(
            MockPageRenderer {
                render_calls: render_calls.clone(),
            },
            MockOcrEngine {
                recognize_calls: recognize_calls.clone(),
            },
        );

        let result = loader.load("/tmp/nonexistent_multimodal.pdf").await;
        assert!(result.is_err(), "不存在的文件必须返回 Err");
        assert_eq!(
            render_calls.load(Ordering::Relaxed),
            0,
            "文件不存在时不应触发渲染"
        );
    }
}
