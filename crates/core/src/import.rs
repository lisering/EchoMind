//! 文件导入领域服务（REQ-ING-001/002/004、REQ-LIC-001、REQ-SEC 路径安全审查）。
//! 管线：路径安全校验 → 格式白名单 → 内容指纹去重（MD5）→ 免费版配额与 PDF 付费门
//! → 复制入数据目录 → 状态机推进（Pending → Processing → Indexed / Failed）。
//! 本模块只依赖 Storage / Loader / Splitter 端口，不感知 Tauri（六边形架构）。

use std::path::{Component, Path, PathBuf};

use anyhow::{Context, anyhow, bail};
use echomind_models::{Chunk, DocStatus, Document};

use crate::{Loader, NoVlm, OcrEngine, PageRenderer, Splitter, Storage, VisionLanguageModel};

/// VLM 图片理解系统提示（REQ-MM-003）：引导 VLM 将图片中的结构化内容
/// 转换为 Markdown 表格、Mermaid gantt/flowchart 等可检索格式。
///
/// 与 `loader::VLM_IMAGE_PROMPT` 保持一致（管线编排层与加载器层共用同一提示）。
const VLM_IMAGE_PROMPT: &str = "\
你是一个文档图片理解助手。请分析图片中的内容，并将其转换为可检索的结构化文本格式：\n\
- 表格 → Markdown 表格语法\n\
- 甘特图 → Mermaid gantt 语法\n\
- 流程图 → Mermaid flowchart 语法\n\
- 其他图表 → 简洁文字描述\n\
- 纯文字图片 → 提取文字内容\n\
\n\
请仅输出转换后的文本，不要添加额外解释。";

/// 免费版文件数上限（REQ-LIC-001）
pub const FREE_TIER_MAX_FILES: usize = 50;
/// 格式白名单（REQ-ING-002）
pub const ALLOWED_EXTENSIONS: [&str; 3] = ["md", "txt", "pdf"];

/// 配额触顶错误前缀（REQ-LIC-002；前端据此弹出付费墙）
pub const ERR_LIMIT_REACHED: &str = "LIMIT_REACHED";
/// PDF 付费门错误前缀（REQ-LIC-002；前端据此弹出付费墙）
pub const ERR_PRO_REQUIRED: &str = "PRO_REQUIRED";

/// 单文件导入结果。
#[derive(Debug)]
pub enum ImportOutcome {
    /// 成功入库（携带文档记录）
    Imported(Document),
    /// 内容指纹重复，跳过（REQ-ING-004）
    SkippedDuplicate(String),
}

/// 文件导入服务。
pub struct ImportService<S: Storage> {
    storage: S,
    data_dir: PathBuf,
    max_free_files: usize,
}

impl<S: Storage> ImportService<S> {
    /// 构造导入服务；`data_dir` 为应用数据目录（文档副本存放于其下 `documents/`）。
    pub fn new(storage: S, data_dir: PathBuf) -> Self {
        Self {
            storage,
            data_dir,
            max_free_files: FREE_TIER_MAX_FILES,
        }
    }

    /// 安全官审查项：路径安全校验。
    /// - 拒绝含 `..` 上级引用的路径（防路径遍历，TC-ING-001b）
    /// - canonicalize 必须成功（文件存在）且目标为普通文件
    fn sanitize_path(raw: &str) -> anyhow::Result<PathBuf> {
        let path = Path::new(raw);
        if path.components().any(|c| matches!(c, Component::ParentDir)) {
            bail!("非法路径（含 .. 上级引用）: {raw}");
        }
        let canonical = path
            .canonicalize()
            .with_context(|| format!("文件不存在或不可读: {raw}"))?;
        if !canonical.is_file() {
            bail!("目标不是普通文件: {raw}");
        }
        Ok(canonical)
    }

    /// 格式白名单校验（REQ-ING-002），返回小写扩展名。
    fn validate_extension(path: &Path) -> anyhow::Result<String> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_lowercase)
            .ok_or_else(|| anyhow!("无法识别文件扩展名: {}", path.display()))?;
        if !ALLOWED_EXTENSIONS.contains(&ext.as_str()) {
            bail!(
                "不支持的文件格式 .{ext}（当前支持: {}）",
                ALLOWED_EXTENSIONS.join("/")
            );
        }
        Ok(ext)
    }

    /// 内容指纹（REQ-ING-004，MD5；仅用于去重，非安全场景）。
    fn content_hash(bytes: &[u8]) -> String {
        use md5::{Digest, Md5};
        let digest = Md5::digest(bytes);
        format!("{digest:x}")
    }

    /// 免费版限制检查：PDF 付费门（REQ-ING-002-AC-1）与配额（REQ-LIC-001-AC-1）。
    async fn check_free_tier_limits(&self, is_pro: bool, ext: &str) -> anyhow::Result<()> {
        if is_pro {
            return Ok(());
        }
        if ext == "pdf" {
            bail!("{ERR_PRO_REQUIRED}: PDF 导入为 Pro 版功能，请升级后重试");
        }
        let count = self.storage.count_documents().await?;
        if count >= self.max_free_files {
            bail!(
                "{ERR_LIMIT_REACHED}: 免费版最多导入 {} 个文件（当前已达上限）",
                self.max_free_files
            );
        }
        Ok(())
    }

    /// 导入单个文件：校验 → 去重 → 配额 → 复制入库 → 状态 Pending。
    pub async fn import_one(&self, raw_path: &str, is_pro: bool) -> anyhow::Result<ImportOutcome> {
        let canonical = Self::sanitize_path(raw_path)?;
        let ext = Self::validate_extension(&canonical)?;

        let bytes = tokio::fs::read(&canonical)
            .await
            .with_context(|| format!("读取文件失败: {}", canonical.display()))?;
        let hash = Self::content_hash(&bytes);

        if self.storage.find_document_by_hash(&hash).await?.is_some() {
            return Ok(ImportOutcome::SkippedDuplicate(Self::file_name(&canonical)));
        }

        self.check_free_tier_limits(is_pro, &ext).await?;

        let dest = self
            .data_dir
            .join("documents")
            .join(format!("{hash}-{}", Self::file_name(&canonical)));
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("创建数据目录失败: {}", parent.display()))?;
        }
        tokio::fs::write(&dest, &bytes)
            .await
            .with_context(|| format!("写入数据目录失败: {}", dest.display()))?;

        let doc = Document::new(dest.to_string_lossy().into_owned(), hash);
        self.storage.add_document(&doc).await?;
        Ok(ImportOutcome::Imported(doc))
    }

    /// 执行索引（load → split → chunks 入库），并推进状态机。
    /// 支持 md / txt / pdf（REQ-PDF-001）；失败时状态置 Failed 并携带可读原因。
    pub async fn index_document(&self, doc: &Document) -> anyhow::Result<()> {
        self.storage
            .update_doc_status(&doc.id, DocStatus::Processing)
            .await?;
        let result = self.index_inner(doc).await;
        match &result {
            Ok(()) => {
                self.storage
                    .update_doc_status(&doc.id, DocStatus::Indexed)
                    .await?;
            }
            Err(err) => {
                self.storage
                    .update_doc_status(&doc.id, DocStatus::Failed(err.to_string()))
                    .await?;
            }
        }
        result
    }

    async fn index_inner(&self, doc: &Document) -> anyhow::Result<()> {
        let ext = Self::validate_extension(Path::new(&doc.file_path))?;
        match ext.as_str() {
            "md" => {
                // .md 用 SectionAwareSplitter（需要原始 markdown，不经过 MarkdownLoader 剥离 #）
                let raw = tokio::fs::read_to_string(&doc.file_path)
                    .await
                    .with_context(|| format!("读取 Markdown 失败: {}", doc.file_path))?;
                self.index_with_section(doc, raw).await
            }
            "txt" => {
                let text = crate::loader::TextLoader.load(&doc.file_path).await?;
                self.index_with_text(doc, text).await
            }
            "pdf" => {
                let text = crate::loader::PdfLoader.load(&doc.file_path).await?;
                self.index_with_text(doc, text).await
            }
            other => bail!("不支持的解析格式: .{other}"),
        }
    }

    /// .md 专用：SectionAwareSplitter 章节感知分块 + 入库（REQ-VEC-006）。
    /// 需要**原始 markdown 文本**（含 # 标题标记），不经过 MarkdownLoader。
    async fn index_with_section(&self, doc: &Document, text: String) -> anyhow::Result<()> {
        let splitter = crate::section_aware_splitter::SectionAwareSplitter::default_config()?;
        let pieces = splitter.split(&text).await?;
        for (sequence, content) in pieces.into_iter().enumerate() {
            let token_count = splitter.count_tokens(&content)?;
            let chunk = Chunk::new(doc.id.clone(), content, token_count, sequence);
            self.storage.add_chunk(&chunk).await?;
        }
        Ok(())
    }

    /// 分块 + 入库（复用逻辑，供 `index_inner` 和 `index_multimodal_inner` 共享）。
    async fn index_with_text(&self, doc: &Document, text: String) -> anyhow::Result<()> {
        // 使用 SemanticSplitter 替代 TextSplitter：
        // 段落→句子→子句递归分割，保留代码块完整性，支持中文标点。
        // 来源：rs-pro 语义分块器，借鉴 LangChain/LlamaIndex 最佳实践。
        let splitter = crate::semantic_splitter::SemanticSplitter::default_config()?;
        let pieces = splitter.split(&text).await?;
        for (sequence, content) in pieces.into_iter().enumerate() {
            let token_count = splitter.count_tokens(&content)?;
            let chunk = Chunk::new(doc.id.clone(), content, token_count, sequence);
            self.storage.add_chunk(&chunk).await?;
        }
        Ok(())
    }

    fn file_name(path: &Path) -> String {
        path.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "unnamed".to_string())
    }
}

/// 多模态索引扩展（REQ-MM-004）：为 `ImportService` 增加泛型方法，
/// 支持注入 `PageRenderer` + `OcrEngine` 对 PDF 执行多模态提取。
///
/// 设计原因：`async fn` in trait 不支持 `dyn Trait`（Edition 2024），
/// 故采用泛型方法注入而非 trait object。
impl<S: Storage> ImportService<S> {
    /// 多模态索引文档（REQ-MM-004）：文本提取 → 图片检测 → 渲染 → OCR → 分块入库。
    ///
    /// 对 PDF 格式：使用 `MultimodalPdfLoader` 逻辑（文本层 + 图片渲染 + OCR）。
    /// 对 md/txt 格式：与 `index_document` 行为一致（仅文本提取）。
    ///
    /// # 参数
    /// - `doc`: 已导入的文档记录
    /// - `page_renderer`: PDF 页面渲染引擎
    /// - `ocr_engine`: OCR 文字识别引擎
    pub async fn index_document_multimodal<R, O>(
        &self,
        doc: &Document,
        page_renderer: &R,
        ocr_engine: &O,
    ) -> anyhow::Result<()>
    where
        R: PageRenderer,
        O: OcrEngine,
    {
        // VLM 禁用：使用 NoVlm 占位实现，管线天然跳过 VLM 阶段
        self.index_document_multimodal_with_vlm(doc, page_renderer, ocr_engine, &NoVlm)
            .await
    }

    /// 多模态索引文档（含 VLM 增强，REQ-MM-003/004）：
    /// 文本提取 → 图片检测 → 渲染 → OCR → VLM 增强 → 分块入库。
    ///
    /// 在 `index_document_multimodal` 基础上，对含图片的页面额外调用 VLM，
    /// 将表格→Markdown、甘特图→Mermaid 等结构化内容追加到文本中。
    ///
    /// # 隐私
    /// 图片仅发送到用户配置的 LLM 端点（BYOK），符合「隐私不出域」。
    ///
    /// # 参数
    /// - `doc`: 已导入的文档记录
    /// - `page_renderer`: PDF 页面渲染引擎
    /// - `ocr_engine`: OCR 文字识别引擎
    /// - `vlm`: VLM 图片理解引擎（`NoVlm` 表示禁用）
    pub async fn index_document_multimodal_with_vlm<R, O, V>(
        &self,
        doc: &Document,
        page_renderer: &R,
        ocr_engine: &O,
        vlm: &V,
    ) -> anyhow::Result<()>
    where
        R: PageRenderer,
        O: OcrEngine,
        V: VisionLanguageModel,
    {
        self.storage
            .update_doc_status(&doc.id, DocStatus::Processing)
            .await?;
        let result = self
            .index_multimodal_inner(doc, page_renderer, ocr_engine, vlm)
            .await;
        match &result {
            Ok(()) => {
                self.storage
                    .update_doc_status(&doc.id, DocStatus::Indexed)
                    .await?;
            }
            Err(err) => {
                self.storage
                    .update_doc_status(&doc.id, DocStatus::Failed(err.to_string()))
                    .await?;
            }
        }
        result
    }

    /// 多模态索引内部逻辑（不处理状态机，供 `index_document_multimodal_with_vlm` 调用）。
    async fn index_multimodal_inner<R, O, V>(
        &self,
        doc: &Document,
        page_renderer: &R,
        ocr_engine: &O,
        vlm: &V,
    ) -> anyhow::Result<()>
    where
        R: PageRenderer,
        O: OcrEngine,
        V: VisionLanguageModel,
    {
        let ext = Self::validate_extension(Path::new(&doc.file_path))?;
        let text = match ext.as_str() {
            "md" => crate::loader::MarkdownLoader.load(&doc.file_path).await?,
            "txt" => crate::loader::TextLoader.load(&doc.file_path).await?,
            "pdf" => {
                // 多模态 PDF 加载：lopdf 文本提取 + 图片检测 → 渲染 → OCR → 合并
                let owned = doc.file_path.clone();
                // Step 1: lopdf 文本提取 + 图片检测（CPU 密集，spawn_blocking）
                let (text_layer, image_pages) = tokio::task::spawn_blocking(move || {
                    crate::loader::MultimodalPdfLoader::<R, O, V>::extract_text_and_detect_images(
                        &owned,
                    )
                })
                .await
                .context("PDF 文本提取 + 图片检测任务执行失败")??;

                // Step 2: 对含图片的页面执行渲染 + OCR
                let mut full_text = text_layer;
                for page_num in image_pages {
                    let image_bytes = page_renderer
                        .render_page(&doc.file_path, page_num, 150)
                        .await
                        .with_context(|| format!("渲染 PDF 第 {page_num} 页失败"))?;

                    let ocr_text = ocr_engine
                        .recognize(&image_bytes)
                        .await
                        .with_context(|| format!("OCR 识别 PDF 第 {page_num} 页失败"))?;

                    if !ocr_text.is_empty() {
                        if !full_text.is_empty() {
                            full_text.push_str("\n\n");
                        }
                        full_text.push_str(&ocr_text);
                    }

                    // REQ-MM-003: VLM 图片理解（表格→Markdown，甘特图→Mermaid）
                    // 图片仅发送到用户配置的 LLM 端点（BYOK），符合隐私不出域
                    let vlm_text = vlm
                        .describe_image(&image_bytes, VLM_IMAGE_PROMPT)
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
                    bail!("PDF 未提取到任何文本: {}", doc.file_path);
                }
                full_text
            }
            other => bail!("不支持的解析格式: .{other}"),
        };
        self.index_with_text(doc, text).await
    }
}
