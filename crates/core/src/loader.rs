//! 文档加载适配器（REQ-ING-001/002）：Markdown 与纯文本。
//! pulldown-cmark 为纯计算解析库（无 I/O、无网络），经架构评审准入 core。
//! 体系三：Markdown 解析为 CPU 密集任务，经 `spawn_blocking` 执行，不阻塞 async executor。

use anyhow::{Context, bail};
use calamine::Data as CalData;
use calamine::{Reader as CalReader, open_workbook_auto};
use pulldown_cmark::{Event, Parser, TagEnd};

use crate::Loader;
#[cfg(feature = "pro")]
use crate::{NoVlm, OcrEngine, PageRenderer, VisionLanguageModel};

/// Markdown 加载器：剥离标记符号，保留正文与代码块内容（TC-ING-005 断言依据）。
pub struct MarkdownLoader;

impl MarkdownLoader {
    /// 同步提取纯文本（供 `spawn_blocking` 调用）。
    /// 段落边界以双换行 `\n\n` 保留，使下游 Splitter 能在语义边界切分。
    fn extract_text(markdown: &str) -> String {
        let parser = Parser::new(markdown);
        let mut out = String::with_capacity(markdown.len());
        for event in parser {
            match event {
                Event::Text(t) | Event::Code(t) => out.push_str(&t),
                Event::SoftBreak | Event::HardBreak => out.push('\n'),
                // 标题/段落/代码块/列表项结束 → 双换行保持段落边界
                // （REQ-VEC-001-AC-4：段落感知分块前置条件）
                Event::End(
                    TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::CodeBlock | TagEnd::Item,
                ) => out.push_str("\n\n"),
                _ => {}
            }
        }
        // 压缩连续 3+ 换行为双换行（列表项间可能产生多余空行）
        while out.contains("\n\n\n") {
            out = out.replace("\n\n\n", "\n\n");
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
///
/// 使用 `read` + `from_utf8_lossy` 而非 `read_to_string`，容错处理混合编码文件
/// （REQ-CHAOS-003：真实世界 txt 文件可能包含 GBK / 非法字节，不应因编码问题崩溃）。
/// 对合法 UTF-8 文本，`from_utf8_lossy` 输出与 `read_to_string` 完全一致。
pub struct TextLoader;

impl Loader for TextLoader {
    async fn load(&self, file_path: &str) -> anyhow::Result<String> {
        let bytes = tokio::fs::read(file_path)
            .await
            .with_context(|| format!("读取文本失败: {file_path}"))?;
        // from_utf8_lossy：非法字节以 U+FFFD 替代，保证返回合法 UTF-8 字符串
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }
}

/// PDF 加载器（REQ-PDF-001）：基于 lopdf 逐页提取文本。
/// 容错策略：单页失败跳过并记录；整体失败优雅返回 Err；任何非法输入不得 Panic。
pub struct PdfLoader;

impl PdfLoader {
    fn extract(file_path: &str) -> anyhow::Result<String> {
        let doc = lopdf::Document::load(file_path)
            .with_context(|| format!("PDF 加载失败: {file_path}"))?;
        let mut out = String::new();
        for (page_number, _) in doc.get_pages() {
            match doc.extract_text(&[page_number]) {
                Ok(text) => {
                    // REQ-PDF-001：from_utf8_lossy 容错解码，杜绝非法字节导致的崩溃
                    out.push_str(&String::from_utf8_lossy(text.as_bytes()));
                    out.push('\n');
                }
                Err(err) => eprintln!("PDF 第 {page_number} 页提取失败，已跳过: {err}"),
            }
        }
        let trimmed = out.trim();
        if trimmed.is_empty() {
            bail!("PDF 未提取到任何文本: {file_path}");
        }
        Ok(trimmed.to_string())
    }
}

impl Loader for PdfLoader {
    async fn load(&self, file_path: &str) -> anyhow::Result<String> {
        let owned = file_path.to_string();
        // lopdf 解析为 CPU 密集任务，经 spawn_blocking 执行（体系三）
        tokio::task::spawn_blocking(move || Self::extract(&owned))
            .await
            .context("PDF 解析任务执行失败")?
    }
}

/// .docx 文档加载器（REQ-ING-015）。
///
/// 使用 docx-rs 解析 Office Open XML 格式（.docx ZIP 包）。
/// 读取 `word/document.xml`，提取段落文本 + 表格 + 标题层级。
/// 段落边界以双换行保留（与 MarkdownLoader 一致，使下游 Splitter 能在语义边界切分）。
/// 表格转换为 Markdown 表格语法。图片提取为 `[图片]` 占位文本，不提取二进制内容。
pub struct DocxLoader;

impl DocxLoader {
    /// 同步提取纯文本（供 `spawn_blocking` 调用）。
    fn extract(file_path: &str) -> anyhow::Result<String> {
        let bytes = std::fs::read(file_path)
            .with_context(|| format!("读取 .docx 文件失败: {file_path}"))?;

        let docx = docx_rs::read_docx(&bytes)
            .map_err(|e| anyhow::anyhow!("解析 .docx 失败: {file_path}: {e}"))?;

        let mut blocks: Vec<String> = Vec::new();

        for child in &docx.document.children {
            match child {
                docx_rs::DocumentChild::Paragraph(p) => {
                    let text = extract_paragraph_text(p);
                    if !text.trim().is_empty() {
                        blocks.push(text);
                    }
                }
                docx_rs::DocumentChild::Table(t) => {
                    let md = extract_table_markdown(t);
                    if !md.is_empty() {
                        blocks.push(md);
                    }
                }
                _ => {}
            }
        }

        let mut out = blocks.join("\n\n");

        // 压缩连续 3+ 换行为双换行（与 MarkdownLoader 一致）
        while out.contains("\n\n\n") {
            out = out.replace("\n\n\n", "\n\n");
        }

        let trimmed = out.trim();
        if trimmed.is_empty() {
            anyhow::bail!(".docx 未提取到任何文本: {file_path}");
        }
        Ok(trimmed.to_string())
    }
}

impl Loader for DocxLoader {
    async fn load(&self, file_path: &str) -> anyhow::Result<String> {
        let owned = file_path.to_string();
        // docx-rs 解析为 CPU 密集任务，经 spawn_blocking 执行（体系三）
        tokio::task::spawn_blocking(move || Self::extract(&owned))
            .await
            .context("DocxLoader 解析任务执行失败")?
    }
}

/// 从 `Paragraph` 提取纯文本，包括 Run 文本 + 图片占位符。
///
/// 遍历段落的子元素（Run / Hyperlink / Insert / MoveTo），
/// 递归提取文本内容。图片节点（`RunChild::Drawing`）替换为 `[图片]`。
fn extract_paragraph_text(p: &docx_rs::Paragraph) -> String {
    let mut text = String::new();
    for child in &p.children {
        extract_paragraph_child_text(child, &mut text);
    }
    text
}

/// 递归提取 `ParagraphChild` 的文本内容。
fn extract_paragraph_child_text(child: &docx_rs::ParagraphChild, text: &mut String) {
    match child {
        docx_rs::ParagraphChild::Run(run) => {
            extract_run_text(run, text);
        }
        docx_rs::ParagraphChild::Hyperlink(h) => {
            // Hyperlink 的 children 是 Vec<ParagraphChild>，递归处理
            for c in &h.children {
                extract_paragraph_child_text(c, text);
            }
        }
        docx_rs::ParagraphChild::Insert(ins) => {
            for c in &ins.children {
                if let docx_rs::InsertChild::Run(run) = c {
                    extract_run_text(run, text);
                }
            }
        }
        docx_rs::ParagraphChild::MoveTo(mt) => {
            for c in &mt.children {
                if let docx_rs::MoveToChild::Run(run) = c {
                    extract_run_text(run, text);
                }
            }
        }
        _ => {}
    }
}

/// 从 `Run` 提取文本，处理 Text / Tab / Break / Drawing 等子元素。
fn extract_run_text(run: &docx_rs::Run, text: &mut String) {
    for c in &run.children {
        match c {
            docx_rs::RunChild::Text(t) => text.push_str(&t.text),
            docx_rs::RunChild::Tab(_) => text.push('\t'),
            docx_rs::RunChild::PTab(_) => text.push('\t'),
            docx_rs::RunChild::Break(_) => text.push('\n'),
            docx_rs::RunChild::CarriageReturn(_) => text.push('\n'),
            docx_rs::RunChild::Drawing(_) => text.push_str("[图片]"),
            _ => {}
        }
    }
}

/// 将 `Table` 转换为 Markdown 表格语法。
///
/// 第一行作为表头，自动生成 `| --- |` 分隔行，其余为数据行。
fn extract_table_markdown(table: &docx_rs::Table) -> String {
    let mut rows: Vec<Vec<String>> = Vec::new();

    for row_child in &table.rows {
        let docx_rs::TableChild::TableRow(row) = row_child;
        let mut cells = Vec::new();
        for cell_child in &row.cells {
            let docx_rs::TableRowChild::TableCell(cell) = cell_child;
            let mut cell_text = String::new();
            for content in &cell.children {
                if let docx_rs::TableCellContent::Paragraph(p) = content {
                    let pt = extract_paragraph_text(p);
                    if !pt.is_empty() {
                        if !cell_text.is_empty() {
                            cell_text.push(' ');
                        }
                        cell_text.push_str(&pt);
                    }
                }
            }
            cells.push(cell_text);
        }
        rows.push(cells);
    }

    if rows.is_empty() {
        return String::new();
    }

    let col_count = rows[0].len();
    let mut md = String::new();

    // 表头行
    md.push('|');
    for cell in &rows[0] {
        md.push_str(&format!(" {cell} |"));
    }
    md.push('\n');

    // 分隔行
    md.push('|');
    for _ in 0..col_count {
        md.push_str(" --- |");
    }
    md.push('\n');

    // 数据行
    for row in rows.iter().skip(1) {
        md.push('|');
        for cell in row {
            md.push_str(&format!(" {cell} |"));
        }
        md.push('\n');
    }

    md.trim().to_string()
}

/// .html 文档加载器（REQ-ING-016）。
///
/// 使用 html2text 解析 HTML，提取正文内容。
/// 保留标题层级（h1-h6 → Markdown `#` 语法）、链接文本、列表结构。
/// 去除 nav/header/footer/script/style 等非正文元素。
/// 段落边界以双换行保留（与 MarkdownLoader 一致）。
pub struct HtmlLoader;

impl HtmlLoader {
    /// 同步提取纯文本（供 `spawn_blocking` 调用）。
    ///
    /// 管线：读取文件 → 去除非正文元素 → 标题转 Markdown ATX → html2text 转换 → 后处理。
    fn extract(file_path: &str) -> anyhow::Result<String> {
        let bytes = std::fs::read(file_path)
            .with_context(|| format!("读取 .html 文件失败: {file_path}"))?;
        let html = String::from_utf8_lossy(&bytes);

        // Step 1: 去除非正文元素（nav/header/footer/aside）
        let cleaned = Self::remove_non_content(&html)?;
        // Step 2: 将 h1-h6 转换为 Markdown ATX 格式（# ~ ######）
        let with_headings = Self::convert_headings(&cleaned)?;
        // Step 3: html2text 转换（自动去除 script/style，保留段落/列表/链接文本）
        let raw_text = html2text::from_read(with_headings.as_bytes(), 80)
            .map_err(|e| anyhow::anyhow!("html2text 转换失败: {e}"))?;
        // Step 4: 后处理（列表标记标准化 + 压缩换行）
        let processed = Self::post_process(&raw_text)?;

        let trimmed = processed.trim();
        if trimmed.is_empty() {
            bail!(".html 未提取到任何文本: {file_path}");
        }
        Ok(trimmed.to_string())
    }

    /// 去除 nav/header/footer/aside 元素（html2text 不自动去除这些语义标签）。
    ///
    /// 使用正则匹配 `<nav>...</nav>` 等块级元素并整体移除。
    /// 支持标签属性（如 `<nav class="main">`）和大小写混写。
    /// Rust `regex` crate 不支持反向引用，故为每个标签编译独立正则。
    fn remove_non_content(html: &str) -> anyhow::Result<String> {
        let patterns = [
            r"(?is)<nav(\s[^>]*)?>.*?</nav\s*>",
            r"(?is)<header(\s[^>]*)?>.*?</header\s*>",
            r"(?is)<footer(\s[^>]*)?>.*?</footer\s*>",
            r"(?is)<aside(\s[^>]*)?>.*?</aside\s*>",
        ];
        let mut result = html.to_string();
        for pattern in &patterns {
            let re =
                regex::Regex::new(pattern).map_err(|e| anyhow::anyhow!("正则编译失败: {e}"))?;
            result = re.replace_all(&result, "").into_owned();
        }
        Ok(result)
    }

    /// 将 HTML 标题标签（h1-h6）转换为 Markdown ATX 格式（`#` ~ `######`）。
    ///
    /// html2text 默认以 setext 风格渲染标题（文本下方 `=`/`-` 线），
    /// 此方法在传入 html2text 前将标题替换为 `# 文本` 格式，
    /// 确保下游 Markdown 分块器能识别标题层级。
    /// 标题内的嵌套标签（`<a>`/`<span>` 等）被剥离，仅保留文本。
    fn convert_headings(html: &str) -> anyhow::Result<String> {
        let tag_strip_re =
            regex::Regex::new(r"<[^>]+>").map_err(|e| anyhow::anyhow!("正则编译失败: {e}"))?;

        let mut result = html.to_string();
        // 逆序处理 h6→h1，避免 `<h1>` 误匹配 `<h10>`（虽然 HTML 规范无 h10）
        for (tag, prefix) in [
            ("h6", "######"),
            ("h5", "#####"),
            ("h4", "####"),
            ("h3", "###"),
            ("h2", "##"),
            ("h1", "#"),
        ] {
            let pattern = format!(r"(?is)<{tag}(\s[^>]*)?>(.*?)</{tag}\s*>");
            let re =
                regex::Regex::new(&pattern).map_err(|e| anyhow::anyhow!("正则编译失败: {e}"))?;
            result = re
                .replace_all(&result, |caps: &regex::Captures| {
                    let inner = &caps[2];
                    let text = tag_strip_re.replace_all(inner, "");
                    format!("\n\n{prefix} {text}\n\n")
                })
                .into_owned();
        }
        Ok(result)
    }

    /// 后处理 html2text 输出：列表标记标准化 + 压缩换行。
    ///
    /// - html2text 以 `*` 渲染列表项，替换为 Markdown 标准 `-`
    /// - 压缩连续 3+ 换行为双换行（与 MarkdownLoader / DocxLoader 一致）
    fn post_process(text: &str) -> anyhow::Result<String> {
        let list_re = regex::Regex::new(r"(?m)^(\s*)\* ")
            .map_err(|e| anyhow::anyhow!("正则编译失败: {e}"))?;
        let mut result = list_re.replace_all(text, "$1- ").into_owned();

        while result.contains("\n\n\n") {
            result = result.replace("\n\n\n", "\n\n");
        }
        Ok(result)
    }
}

impl Loader for HtmlLoader {
    async fn load(&self, file_path: &str) -> anyhow::Result<String> {
        let owned = file_path.to_string();
        // html2text 解析为 CPU 密集任务，经 spawn_blocking 执行（体系三）
        tokio::task::spawn_blocking(move || Self::extract(&owned))
            .await
            .context("HtmlLoader 解析任务执行失败")?
    }
}

/// .pptx 演示文稿加载器（REQ-ING-017）。
///
/// 使用 pptx-to-md 解析 Office Open XML 格式（.pptx ZIP 包）。
/// 提取每张幻灯片的标题（→ `#` 标题）、要点（→ `-` 列表）、备注，
/// 表格转换为 Markdown 表格，图片以 `[图片]` 占位符替代。
/// 每张幻灯片以 `---` 分隔。
///
/// 配置策略：
/// - `extract_images = false`：不提取图片二进制数据（避免 base64 膨胀输出）
/// - `include_speaker_notes = true`：备注含语义信息，纳入提取范围
/// - `include_slide_number_as_comment = false`：清理 HTML 注释噪声
/// - `include_presentation_metadata = false`：清理元数据头部
pub struct PptxLoader;

impl PptxLoader {
    /// 同步提取纯文本（供 `spawn_blocking` 调用）。
    fn extract(file_path: &str) -> anyhow::Result<String> {
        let path = std::path::Path::new(file_path);

        // 配置解析器：禁用图片提取（图片二进制不入向量库），启用备注
        let config = pptx_to_md::ParserConfig::builder()
            .extract_images(false)
            .include_speaker_notes(true)
            .include_slide_number_as_comment(false)
            .include_presentation_metadata(false)
            .build();

        let mut container = pptx_to_md::PptxContainer::open(path, config)
            .map_err(|e| anyhow::anyhow!("解析 .pptx 失败: {file_path}: {e}"))?;

        let markdown = container
            .convert_to_md()
            .map_err(|e| anyhow::anyhow!("转换 .pptx 为 Markdown 失败: {file_path}: {e}"))?;

        // 后处理：压缩连续 3+ 换行为双换行（与 DocxLoader / MarkdownLoader 一致）
        let mut out = markdown;
        while out.contains("\n\n\n") {
            out = out.replace("\n\n\n", "\n\n");
        }

        let trimmed = out.trim();
        if trimmed.is_empty() {
            anyhow::bail!(".pptx 未提取到任何文本: {file_path}");
        }
        Ok(trimmed.to_string())
    }
}

impl Loader for PptxLoader {
    async fn load(&self, file_path: &str) -> anyhow::Result<String> {
        let owned = file_path.to_string();
        // pptx-to-md 解析为 CPU 密集任务，经 spawn_blocking 执行（体系三）
        tokio::task::spawn_blocking(move || Self::extract(&owned))
            .await
            .context("PptxLoader 解析任务执行失败")?
    }
}

/// .epub 电子书加载器（REQ-ING-018）：基于 rbook 解析 EPUB 2/3。
///
/// 按 spine（阅读顺序）遍历章节，提取 XHTML 内容并通过 html2text 转为纯文本。
/// 每章以 `---` 分隔。HTML 标签（`<b>`/`<i>`/`<a>` 等）被剥离，纯文本保留。
///
/// 配置策略：
/// - `rbook` 默认配置（`threadsafe` + `prelude` feature）
/// - `html2text` 宽度 80 列（与 HtmlLoader 一致）
/// - 压缩连续 3+ 换行为双换行（与 PptxLoader / HtmlLoader 一致）
pub struct EpubLoader;

impl EpubLoader {
    /// 同步提取纯文本（供 `spawn_blocking` 调用）。
    ///
    /// 管线：rbook 打开 EPUB → 遍历 spine 章节逐个 read_str → html2text 转纯文本 → 合并。
    fn extract(file_path: &str) -> anyhow::Result<String> {
        let epub = rbook::Epub::open(file_path)
            .map_err(|e| anyhow::anyhow!("解析 .epub 失败: {file_path}: {e}"))?;

        let mut chapters: Vec<String> = Vec::new();

        for spine_entry in epub.spine().iter() {
            // 获取 spine 条目对应的 manifest 条目（XHTML 资源）
            let Some(manifest_entry) = spine_entry.manifest_entry() else {
                continue;
            };

            // 读取 XHTML 内容为字符串
            let xhtml = manifest_entry
                .read_str()
                .map_err(|e| anyhow::anyhow!("读取 EPUB 章节内容失败: {e}"))?;

            // 使用 html2text 将 XHTML 转为纯文本（与 HtmlLoader 一致）
            let text = html2text::from_read(xhtml.as_bytes(), 80)
                .map_err(|e| anyhow::anyhow!("html2text 转换失败: {e}"))?;

            let trimmed = text.trim();
            if !trimmed.is_empty() {
                chapters.push(trimmed.to_string());
            }
        }

        // 章节间以 --- 分隔
        let mut result = chapters.join("\n\n---\n\n");

        // 压缩连续 3+ 换行为双换行（与 PptxLoader / HtmlLoader 一致）
        while result.contains("\n\n\n") {
            result = result.replace("\n\n\n", "\n\n");
        }

        let trimmed = result.trim();
        if trimmed.is_empty() {
            anyhow::bail!(".epub 未提取到任何文本: {file_path}");
        }
        Ok(trimmed.to_string())
    }
}

impl Loader for EpubLoader {
    async fn load(&self, file_path: &str) -> anyhow::Result<String> {
        let owned = file_path.to_string();
        // rbook 解析为 CPU 密集任务，经 spawn_blocking 执行（体系三）
        tokio::task::spawn_blocking(move || Self::extract(&owned))
            .await
            .context("EpubLoader 解析任务执行失败")?
    }
}

/// 多模态 PDF 加载器（REQ-MM-001/002）：文本层提取 + 图片检测 + 页面渲染 + OCR。
///
/// 在 `PdfLoader`（纯文本提取）基础上扩展多模态管线：
/// 1. `lopdf` 逐页提取文本层（现有逻辑）
/// 2. `lopdf::Document::get_page_images()` 检测含图片的页面
/// 3. 对含图片的页面调用 `PageRenderer.render_page()` 渲染为位图
/// 4. 对位图调用 `OcrEngine.recognize()` 提取文字
/// 5. 合并文本层 + OCR 文本
///
/// # 隐私
///
/// - PDF 渲染：✅ 完全本地（pdfium），零网络请求
/// - OCR：✅ 完全本地（ocrs），零网络请求
/// - 符合「隐私不出域」承诺（ADR-010 安全官要求）
///
/// # 架构
///
/// 使用泛型注入引擎（`async fn` in trait 不支持 `dyn`，Edition 2024 限制）。
/// 组装层（`AppState`）负责实例化 `PdfiumRenderer` + `OcrsEngine` 并注入。
#[cfg(feature = "pro")]
pub struct MultimodalPdfLoader<R: PageRenderer, O: OcrEngine, V: VisionLanguageModel = NoVlm> {
    /// PDF 页面渲染引擎
    page_renderer: R,
    /// OCR 文字识别引擎
    ocr_engine: O,
    /// VLM 图片理解引擎（REQ-MM-003；默认 NoVlm 表示禁用）
    vlm: V,
}

/// 多模态 PDF 渲染默认 DPI（平衡清晰度与内存，ADR-010 建议 150-200）
#[cfg(feature = "pro")]
const MM_PDF_RENDER_DPI: u32 = 150;

// VLM 提示词集中定义于 `vlm_prompt` 模块（REQ-MM-005 分级图表理解），
// 消除 loader 与 import 之间的重复（铁律三）。
#[cfg(feature = "pro")]
use crate::vlm_prompt::VLM_TIERED_PROMPT;

#[cfg(feature = "pro")]
impl<R: PageRenderer, O: OcrEngine> MultimodalPdfLoader<R, O, NoVlm> {
    /// 创建多模态 PDF 加载器（不含 VLM，仅 OCR）。
    ///
    /// # 参数
    /// - `page_renderer`: PDF 页面渲染引擎（如 `PdfiumRenderer`）
    /// - `ocr_engine`: OCR 文字识别引擎（如 `OcrsEngine`）
    #[must_use]
    pub fn new(page_renderer: R, ocr_engine: O) -> Self {
        Self {
            page_renderer,
            ocr_engine,
            vlm: NoVlm,
        }
    }
}

#[cfg(feature = "pro")]
impl<R: PageRenderer, O: OcrEngine, V: VisionLanguageModel> MultimodalPdfLoader<R, O, V> {
    /// 创建含 VLM 增强的多模态 PDF 加载器（REQ-MM-003）。
    ///
    /// 在 OCR 基础上额外调用 VLM，将图片中的表格、甘特图等结构化内容
    /// 转换为 Markdown / Mermaid 格式，追加到页面文本中。
    ///
    /// # 参数
    /// - `page_renderer`: PDF 页面渲染引擎
    /// - `ocr_engine`: OCR 文字识别引擎
    /// - `vlm`: VLM 图片理解引擎（如 `OpenAIVisionProvider`）
    #[must_use]
    pub fn with_vlm(page_renderer: R, ocr_engine: O, vlm: V) -> Self {
        Self {
            page_renderer,
            ocr_engine,
            vlm,
        }
    }

    /// lopdf 文本层提取 + 图片页面检测（同步，在 `spawn_blocking` 中执行）。
    ///
    /// # 返回
    /// `(文本层内容, 含图片的页面列表)` — 页码为 0-based（适配 `PageRenderer` 端口）
    pub fn extract_text_and_detect_images(file_path: &str) -> anyhow::Result<(String, Vec<usize>)> {
        let doc = lopdf::Document::load(file_path)
            .with_context(|| format!("PDF 加载失败: {file_path}"))?;

        let mut text = String::new();
        let mut image_pages = Vec::new();

        for (page_num, page_id) in doc.get_pages() {
            // 文本层提取
            match doc.extract_text(&[page_num]) {
                Ok(t) => {
                    text.push_str(&String::from_utf8_lossy(t.as_bytes()));
                    text.push('\n');
                }
                Err(err) => {
                    eprintln!("PDF 第 {page_num} 页文本提取失败，已跳过: {err}");
                }
            }

            // 图片检测：get_page_images 返回页面中的 XObject Image 列表
            match doc.get_page_images(page_id) {
                Ok(images) if !images.is_empty() => {
                    // lopdf 页码从 1 开始，PageRenderer 端口从 0 开始
                    image_pages.push((page_num - 1) as usize);
                }
                Ok(_) => {} // 无图片，跳过
                Err(e) => {
                    eprintln!("PDF 第 {page_num} 页图片检测失败: {e}");
                }
            }
        }

        Ok((text, image_pages))
    }
}

#[cfg(feature = "pro")]
impl<R: PageRenderer, O: OcrEngine, V: VisionLanguageModel> Loader
    for MultimodalPdfLoader<R, O, V>
{
    async fn load(&self, file_path: &str) -> anyhow::Result<String> {
        let owned = file_path.to_string();

        // Step 1: lopdf 文本提取 + 图片检测（CPU 密集，spawn_blocking）
        let (text_layer, image_pages) =
            tokio::task::spawn_blocking(move || Self::extract_text_and_detect_images(&owned))
                .await
                .context("PDF 文本提取 + 图片检测任务执行失败")??;

        // Step 2: 对含图片的页面执行渲染 + OCR（async I/O + CPU 密集）
        let mut full_text = text_layer;
        for page_num in image_pages {
            let image_bytes = self
                .page_renderer
                .render_page(file_path, page_num, MM_PDF_RENDER_DPI)
                .await
                .with_context(|| format!("渲染 PDF 第 {page_num} 页失败"))?;

            let ocr_text = self
                .ocr_engine
                .recognize(&image_bytes)
                .await
                .with_context(|| format!("OCR 识别 PDF 第 {page_num} 页失败"))?;

            if !ocr_text.is_empty() {
                if !full_text.is_empty() {
                    full_text.push_str("\n\n");
                }
                full_text.push_str(&ocr_text);
            }

            // REQ-MM-005: VLM 分级图表理解（表格→MD / 流程图→Mermaid / 数据图表→CSV / 公式→LaTeX）
            // 图片仅发送到用户配置的 LLM 端点（BYOK），符合隐私不出域
            let vlm_text = self
                .vlm
                .describe_image(&image_bytes, VLM_TIERED_PROMPT)
                .await
                .with_context(|| format!("VLM 理解 PDF 第 {page_num} 页失败"))?;

            if !vlm_text.is_empty() {
                if !full_text.is_empty() {
                    full_text.push_str("\n\n");
                }
                full_text.push_str(&vlm_text);
            }
        }

        if full_text.trim().is_empty() {
            bail!("PDF 未提取到任何文本: {file_path}");
        }
        Ok(full_text)
    }
}

/// .xlsx/.xls/.ods 电子表格加载器（REQ-ING-021）。
///
/// 使用 calamine 读取 Office Open XML / ODF / Legacy 格式电子表格。
/// 每个 Sheet 转换为 Markdown 表格格式（`| col1 | col2 |`），首行作为表头。
/// 多 Sheet 以 `---` 分隔，每个 Sheet 前添加 `## Sheet 名` 二级标题。
///
/// 支持格式：.xlsx / .xls / .xlsm / .xlsb / .ods（calamine 支持的全部格式）
pub struct XlsxLoader;

impl XlsxLoader {
    /// 同步提取纯文本（供 `spawn_blocking` 调用）。
    ///
    /// 管线：calamine 打开工作簿 → 遍历 Sheet → 行数据转 Markdown 表格 → 合并。
    fn extract(file_path: &str) -> anyhow::Result<String> {
        let mut workbook = open_workbook_auto(file_path)
            .map_err(|e| anyhow::anyhow!("解析电子表格失败: {file_path}: {e}"))?;

        let sheet_names = workbook.sheet_names().to_vec();
        let mut sheets: Vec<String> = Vec::new();

        for name in &sheet_names {
            match workbook.worksheet_range(name) {
                Ok(range) => {
                    let rows: Vec<Vec<CalData>> = range.rows().map(|r| r.to_vec()).collect();
                    if rows.is_empty() {
                        continue;
                    }
                    let table = Self::rows_to_markdown_table(&rows);
                    if !table.is_empty() {
                        sheets.push(format!("## {name}\n\n{table}"));
                    }
                }
                Err(e) => {
                    eprintln!("读取 Sheet '{name}' 失败，已跳过: {e}");
                }
            }
        }

        let result = sheets.join("\n\n---\n\n");
        let trimmed = result.trim();
        if trimmed.is_empty() {
            anyhow::bail!("电子表格未提取到任何文本: {file_path}");
        }
        Ok(trimmed.to_string())
    }

    /// 将 calamine 行数据转换为 Markdown 表格。
    ///
    /// 首行作为表头，生成 `| header |` + `| --- |` 分隔行 + 数据行。
    /// 单元格值按 `Data` 类型转字符串：Int/Float → 数字、String → 原样、
    /// DateTime → ISO 8601、Bool → true/false、Empty → 空字符串、Error → 空字符串。
    fn rows_to_markdown_table(rows: &[Vec<CalData>]) -> String {
        if rows.is_empty() {
            return String::new();
        }

        let header = &rows[0];
        let col_count = header.len();

        let mut out = String::new();

        // 表头行
        out.push('|');
        for cell in header.iter().take(col_count) {
            out.push(' ');
            out.push_str(&Self::cell_to_string(cell));
            out.push_str(" |");
        }
        out.push('\n');

        // 分隔行
        out.push('|');
        for _ in 0..col_count {
            out.push_str(" --- |");
        }
        out.push('\n');

        // 数据行
        for row in rows.iter().skip(1) {
            out.push('|');
            for cell in row.iter().take(col_count) {
                out.push(' ');
                out.push_str(&Self::cell_to_string(cell));
                out.push_str(" |");
            }
            // 补齐缺失列
            if row.len() < col_count {
                for _ in row.len()..col_count {
                    out.push_str("  |");
                }
            }
            out.push('\n');
        }

        out.trim().to_string()
    }

    /// 将 calamine `Data` 单元格值转换为字符串。
    fn cell_to_string(cell: &CalData) -> String {
        match cell {
            CalData::Int(n) => n.to_string(),
            CalData::Float(f) => {
                // 整数浮点不显示小数点（如 30.0 → "30"）
                if *f == f.trunc() && f.is_finite() {
                    format!("{:.0}", f)
                } else {
                    format!("{f}")
                }
            }
            CalData::String(s) => s.clone(),
            CalData::DateTime(dt) => {
                let s = format!("{dt}");
                s
            }
            CalData::Bool(b) => b.to_string(),
            CalData::DurationIso(s) | CalData::DateTimeIso(s) => s.clone(),
            CalData::Empty => String::new(),
            CalData::Error(_) => String::new(),
        }
    }
}

impl Loader for XlsxLoader {
    async fn load(&self, file_path: &str) -> anyhow::Result<String> {
        let owned = file_path.to_string();
        tokio::task::spawn_blocking(move || Self::extract(&owned))
            .await
            .context("XlsxLoader 解析任务执行失败")?
    }
}

/// .csv 逗号分隔值加载器（REQ-ING-021）。
///
/// 使用 csv crate 读取 CSV 文件，自动检测分隔符（逗号/分号/Tab）。
/// 首行作为表头，转换为 Markdown 表格格式。
pub struct CsvLoader;

impl CsvLoader {
    /// 同步提取纯文本（供 `spawn_blocking` 调用）。
    ///
    /// 管线：读取文件字节 → 检测分隔符 → csv crate 解析 → Markdown 表格。
    fn extract(file_path: &str) -> anyhow::Result<String> {
        let bytes =
            std::fs::read(file_path).with_context(|| format!("读取 CSV 失败: {file_path}"))?;

        let content = String::from_utf8_lossy(&bytes);

        // 检测分隔符：统计首行中各分隔符出现次数
        let delimiter = Self::detect_delimiter(&content);

        let mut reader = csv::ReaderBuilder::new()
            .delimiter(delimiter)
            .has_headers(false)
            .flexible(true)
            .from_reader(content.as_bytes());

        let records: Vec<csv::StringRecord> = reader.records().filter_map(Result::ok).collect();

        if records.is_empty() {
            anyhow::bail!("CSV 文件为空或无数据行: {file_path}");
        }

        // 过滤空行（全字段为空字符串的记录）
        let non_empty: Vec<&csv::StringRecord> = records
            .iter()
            .filter(|r| r.iter().any(|f| !f.trim().is_empty()))
            .collect();

        if non_empty.is_empty() {
            anyhow::bail!("CSV 文件无有效数据行: {file_path}");
        }

        let mut out = String::new();

        // 首行作为表头
        let header = non_empty[0];
        let col_count = header.len();

        out.push('|');
        for field in header.iter() {
            out.push(' ');
            out.push_str(field.trim());
            out.push_str(" |");
        }
        out.push('\n');

        // 分隔行
        out.push('|');
        for _ in 0..col_count {
            out.push_str(" --- |");
        }
        out.push('\n');

        // 数据行
        for record in non_empty.iter().skip(1) {
            out.push('|');
            for field in record.iter() {
                out.push(' ');
                out.push_str(field.trim());
                out.push_str(" |");
            }
            out.push('\n');
        }

        let result = out.trim().to_string();
        if result.is_empty() {
            anyhow::bail!("CSV 未提取到任何文本: {file_path}");
        }
        Ok(result)
    }

    /// 检测 CSV 分隔符。
    ///
    /// 统计首行中逗号、分号、Tab 出现次数，取最多的作为分隔符。
    /// 默认回退为逗号。
    fn detect_delimiter(content: &str) -> u8 {
        let first_line = content.lines().next().unwrap_or("");
        let comma_count = first_line.matches(',').count();
        let semicolon_count = first_line.matches(';').count();
        let tab_count = first_line.matches('\t').count();

        if semicolon_count > comma_count && semicolon_count > tab_count {
            b';'
        } else if tab_count > comma_count && tab_count > semicolon_count {
            b'\t'
        } else {
            b','
        }
    }
}

impl Loader for CsvLoader {
    async fn load(&self, file_path: &str) -> anyhow::Result<String> {
        let owned = file_path.to_string();
        tokio::task::spawn_blocking(move || Self::extract(&owned))
            .await
            .context("CsvLoader 解析任务执行失败")?
    }
}
