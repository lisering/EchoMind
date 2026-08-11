//! 文件导入领域服务（REQ-ING-001/002/004、REQ-LIC-001、REQ-SEC 路径安全审查）。
//! 管线：路径安全校验 → 格式白名单 → 内容指纹去重（MD5）→ 免费版配额与 PDF 付费门
//! → 复制入数据目录 → 状态机推进（Pending → Processing → Indexed / Failed）。
//! 本模块只依赖 Storage / Loader / Splitter 端口，不感知 Tauri（六边形架构）。

use std::path::{Component, Path, PathBuf};

use anyhow::{Context, anyhow, bail};
use echomind_models::{Chunk, DocStatus, Document};

use crate::proposition_splitter::PropositionSplitter;
use crate::{LLMProvider, Loader, Splitter, Storage};
#[cfg(feature = "pro")]
use crate::{NoVlm, OcrEngine, PageRenderer, SymbolExtractor, VisionLanguageModel};

use futures::StreamExt;

// VLM 提示词集中定义于 `vlm_prompt` 模块（REQ-MM-005 分级图表理解），
// 消除 loader 与 import 之间的重复（铁律三）。
#[cfg(feature = "pro")]
use crate::vlm_prompt::VLM_TIERED_PROMPT;

/// 免费版文件数上限（REQ-LIC-001）
pub const FREE_TIER_MAX_FILES: usize = 50;
/// Pro 门控格式：免费版仅支持个人知识管理格式（md/txt/代码/HTML），
/// 专业文档格式（PDF/DOCX/PPTX/EPUB/XLSX/CSV）需 Pro 许可证（REQ-LIC-002）。
pub const PRO_GATED_EXTENSIONS: [&str; 6] = ["pdf", "docx", "pptx", "epub", "xlsx", "csv"];
/// 格式白名单（REQ-ING-002 + REQ-RAG-031 代码文件支持 + REQ-ING-016 .html + REQ-ING-017 .pptx + REQ-ING-018 .epub 支持）
pub const ALLOWED_EXTENSIONS: [&str; 15] = [
    "md", "txt", "pdf", "rs", "ts", "tsx", "py", "go", "docx", "html", "htm", "pptx", "epub",
    "xlsx", "csv",
];

/// 默认分块窗口（tokens）：≤50MB 文件用 256 tokens，embedding 聚焦单一主题。
pub const DEFAULT_CHUNK_TOKENS: usize = 256;
/// 中文件分块窗口（tokens）：50-200MB 文件用 512 tokens。
pub const MEDIUM_CHUNK_TOKENS: usize = 512;
/// 大文件分块窗口（tokens）：200MB-1GB 文件用 1024 tokens。
pub const LARGE_CHUNK_TOKENS: usize = 1024;
/// 超大文件分块窗口（tokens）：>1GB 文件用 2048 tokens，大幅减少 chunk 总数。
pub const XLARGE_CHUNK_TOKENS: usize = 2048;
/// mmap 流式 I/O 阈值（字节）：超过此大小使用 memmap2 零拷贝加载。
pub const MMAP_THRESHOLD: u64 = 50_000_000; // 50MB
/// 批量入库批次大小（GB 级文件优化）：每次事务写入的 chunk 数上限。
/// 控制单事务大小与峰值内存——GB 级文件可达十余万 chunk，一次写入
/// 单个巨型事务会拖慢 SQLite 且峰值内存高；分批后每批 5000 条。
pub const CHUNK_BATCH_SIZE: usize = 5000;

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
    /// 同名不同内容，需用户确认是否替换（REQ-ING-012）
    /// 携带旧文档 ID 和文件名，前端弹出替换确认对话框
    NameConflict {
        old_doc_id: String,
        file_name: String,
    },
}

/// 文件导入服务。
pub struct ImportService<S: Storage> {
    storage: S,
    data_dir: PathBuf,
    max_free_files: usize,
    /// 分块 token 窗口（GB 级文档加速：大文件用 1024，小文件用 256）
    chunk_tokens: usize,
}

impl<S: Storage> ImportService<S> {
    /// 构造导入服务；`data_dir` 为应用数据目录（文档副本存放于其下 `documents/`）。
    pub fn new(storage: S, data_dir: PathBuf) -> Self {
        Self {
            storage,
            data_dir,
            max_free_files: FREE_TIER_MAX_FILES,
            chunk_tokens: DEFAULT_CHUNK_TOKENS,
        }
    }

    /// 以自定义分块窗口构造（GB 级文档加速：大文件用 1024 tokens）。
    ///
    /// # 参数
    /// - `chunk_tokens`: 分块 token 窗口（256=精确检索，1024=大文档加速）
    pub fn with_chunk_tokens(storage: S, data_dir: PathBuf, chunk_tokens: usize) -> Self {
        Self {
            storage,
            data_dir,
            max_free_files: FREE_TIER_MAX_FILES,
            chunk_tokens,
        }
    }

    /// 根据文件大小自动选择分块窗口（GB 级文档加速）。
    ///
    /// 四级自适应窗口（全尺度优化）：
    /// - ≤50MB: 256 tokens（小文件精细分块，检索精度最高）
    /// - 50-200MB: 512 tokens
    /// - 200MB-1GB: 1024 tokens
    /// - >1GB: 2048 tokens（超大文件大幅减少 chunk 数）
    pub fn auto_chunk_tokens(file_size: u64) -> usize {
        if file_size > 1_000_000_000 {
            XLARGE_CHUNK_TOKENS
        } else if file_size > 200_000_000 {
            LARGE_CHUNK_TOKENS
        } else if file_size > MMAP_THRESHOLD {
            MEDIUM_CHUNK_TOKENS
        } else {
            DEFAULT_CHUNK_TOKENS
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

    /// 流式计算文件 MD5（64KB 块分块读取），避免整文件读入内存。
    ///
    /// **性能优化（GB 级文件）**：原实现 `tokio::fs::read` 将整个文件读入堆内存，
    /// 2GB 文件峰值内存 ≈ 2× 文件大小（读 + hash + 副本写）。改为分块读取仅占
    /// 64KB 缓冲，峰值内存与文件大小无关。
    async fn content_hash_stream(path: &Path) -> anyhow::Result<String> {
        use md5::{Digest, Md5};
        use tokio::io::AsyncReadExt;
        let mut file = tokio::fs::File::open(path)
            .await
            .with_context(|| format!("读取文件失败: {}", path.display()))?;
        let mut hasher = Md5::new();
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = file
                .read(&mut buf)
                .await
                .with_context(|| format!("读取文件失败: {}", path.display()))?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        Ok(format!("{:x}", hasher.finalize()))
    }

    /// 免费版限制检查：专业格式付费门（REQ-LIC-002）与配额（REQ-LIC-001-AC-1）。
    ///
    /// Pro 门控格式：pdf / docx / pptx / epub / xlsx / csv（专业文档场景）。
    /// 免费格式：md / txt / rs / ts / tsx / py / go / html / htm（个人知识管理）。
    async fn check_free_tier_limits(&self, is_pro: bool, ext: &str) -> anyhow::Result<()> {
        if is_pro {
            return Ok(());
        }
        if PRO_GATED_EXTENSIONS.contains(&ext) {
            bail!("{ERR_PRO_REQUIRED}: .{ext} 导入为 Pro 版功能，请升级后重试");
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

    /// 导入单个文件：校验 → 去重 → 同名检测 → 配额 → 复制入库 → 状态 Pending。
    ///
    /// REQ-ING-012：同名不同内容检测。导入同名但内容不同的文件时返回 `NameConflict`，
    /// 由调用方（前端/IPC 层）决定是否替换。确认替换后调用 `replace_and_import`。
    pub async fn import_one(&self, raw_path: &str, is_pro: bool) -> anyhow::Result<ImportOutcome> {
        let canonical = Self::sanitize_path(raw_path)?;
        let ext = Self::validate_extension(&canonical)?;

        // GB 级文件优化：流式 MD5（64KB 块），不整读文件
        let hash = Self::content_hash_stream(&canonical).await?;

        // 内容指纹去重（REQ-ING-004）
        if self.storage.find_document_by_hash(&hash).await?.is_some() {
            // REQ-ING-011：记录跳过的导入
            let file_name = Self::file_name(&canonical);
            let _ = self
                .storage
                .add_import_log(&file_name, &ext, "skipped", Some("duplicate content"), None)
                .await;
            return Ok(ImportOutcome::SkippedDuplicate(file_name));
        }

        // 同名不同内容检测（REQ-ING-012）
        let file_name = Self::file_name(&canonical);
        if let Some(existing) = self.storage.find_document_by_name(&file_name).await? {
            return Ok(ImportOutcome::NameConflict {
                old_doc_id: existing.id,
                file_name,
            });
        }

        self.check_free_tier_limits(is_pro, &ext).await?;

        let dest = self
            .data_dir
            .join("documents")
            .join(format!("{hash}-{}", file_name));
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("创建数据目录失败: {}", parent.display()))?;
        }
        // GB 级文件优化：内核级拷贝副本（零用户态内存），替代 read+write
        let file_size = tokio::fs::copy(&canonical, &dest)
            .await
            .with_context(|| format!("写入数据目录失败: {}", dest.display()))?;

        let doc = Document::new(dest.to_string_lossy().into_owned(), hash);
        self.storage.add_document(&doc).await?;
        // REQ-ING-011：记录成功的导入
        let _ = self
            .storage
            .add_import_log(&file_name, &ext, "success", None, Some(file_size as i64))
            .await;
        Ok(ImportOutcome::Imported(doc))
    }

    /// 替换文档并重新导入（REQ-ING-012）。
    ///
    /// 删除旧文档（级联清理 chunks/向量/propositions/entities/relations/wiki_links），
    /// 然后执行完整导入管线。用于用户确认替换同名不同内容文件后调用。
    ///
    /// # 参数
    /// - `raw_path`: 新文件路径
    /// - `old_doc_id`: 旧文档 ID（将被删除）
    /// - `is_pro`: 是否 Pro 用户
    pub async fn replace_and_import(
        &self,
        raw_path: &str,
        old_doc_id: &str,
        is_pro: bool,
    ) -> anyhow::Result<ImportOutcome> {
        // 先删除旧文档（级联清理）
        self.storage.delete_document(old_doc_id).await?;

        // 重新导入（旧文档已删除，同名检测不会触发）
        self.import_one(raw_path, is_pro).await
    }

    /// 导入单个文件（监听文件夹增量同步用，REQ-SYNC-002）。
    ///
    /// 与 `import_one` 的区别：创建的 `Document` 携带 `original_path` 字段，
    /// 用于增量同步时按源文件路径追踪文档。
    ///
    /// # 参数
    /// - `raw_path`: 源文件路径（将被 canonicalize）
    /// - `original_path`: 源文件的 canonical 路径（存储到 `Document.original_path`）
    /// - `is_pro`: 是否为 Pro 用户（影响配额与 PDF 门禁）
    ///
    /// # 返回
    /// - `ImportOutcome::Imported(doc)`: 成功导入（doc.original_path = Some(original_path)）
    /// - `ImportOutcome::SkippedDuplicate(name)`: 内容指纹重复（已被手动导入过）
    pub async fn import_one_for_sync(
        &self,
        raw_path: &str,
        original_path: &str,
        is_pro: bool,
    ) -> anyhow::Result<ImportOutcome> {
        let canonical = Self::sanitize_path(raw_path)?;
        let ext = Self::validate_extension(&canonical)?;

        // GB 级文件优化：流式 MD5（64KB 块），不整读文件
        let hash = Self::content_hash_stream(&canonical).await?;

        // 内容指纹去重：如果用户之前手动导入了同一文件（无 original_path），
        // 跳过导入以避免重复。后续文件修改时 hash 变化，会作为新文件导入。
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
        // GB 级文件优化：内核级拷贝副本（零用户态内存），替代 read+write
        tokio::fs::copy(&canonical, &dest)
            .await
            .with_context(|| format!("写入数据目录失败: {}", dest.display()))?;

        let doc = Document::new_with_original_path(
            dest.to_string_lossy().into_owned(),
            hash,
            original_path.to_string(),
        );
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

    /// 摘要输入文本的最大字符数（REQ-ING-019）。
    /// 超长文档截取前 4000 字符作为 LLM 输入，避免 token 爆炸。
    pub const SUMMARY_MAX_INPUT_CHARS: usize = 4000;

    /// 异步生成文档摘要（REQ-ING-019 文档摘要自动生成）。
    ///
    /// 取文档前 `SUMMARY_MAX_INPUT_CHARS` 字符作为 LLM 输入，生成 200-300 字摘要。
    /// 摘要存储到 `documents.summary` 列。
    ///
    /// # 参数
    /// - `doc_id`: 文档 ID
    /// - `chunks`: 文档分块列表（用于拼接摘要输入文本）
    /// - `llm_provider`: LLM 提供者（实现 `LLMProvider` trait）
    ///
    /// # 错误处理
    /// 摘要生成失败时返回 `Err`，调用方应静默降级（summary 保持 None），
    /// 不影响导入流程。
    ///
    /// # 向后兼容
    /// 若 chunks 为空，直接返回 `Ok(())`（不生成摘要）。
    pub async fn generate_summary<L: LLMProvider>(
        &self,
        doc_id: &str,
        chunks: &[Chunk],
        llm_provider: &L,
    ) -> anyhow::Result<()> {
        if chunks.is_empty() {
            return Ok(());
        }

        // 拼接分块文本，截取前 SUMMARY_MAX_INPUT_CHARS 字符
        let mut full_text = String::new();
        for chunk in chunks {
            full_text.push_str(&chunk.content);
            full_text.push('\n');
            if full_text.len() >= Self::SUMMARY_MAX_INPUT_CHARS {
                break;
            }
        }
        let truncated: String = full_text
            .chars()
            .take(Self::SUMMARY_MAX_INPUT_CHARS)
            .collect();

        // 构建摘要 prompt（中英双语适配）
        let system_prompt = "你是一个文档摘要助手。请用简洁的中文总结以下文档的核心内容，\
控制在 200-300 字以内。聚焦主要观点、关键信息和文档目的，不要逐句复述。\
如果文档是英文，摘要仍用中文撰写。";
        let query = format!("请总结以下文档：\n\n{truncated}");

        // 调用 LLM 生成摘要（非流式收集完整响应）
        let stream = llm_provider.chat_stream(system_prompt, &[], &query).await?;
        let mut summary = String::new();
        let mut stream = stream;
        while let Some(token_result) = stream.next().await {
            match token_result {
                Ok(token) => summary.push_str(&token),
                Err(e) => return Err(anyhow!("LLM 流式生成摘要失败: {e}")),
            }
        }

        // 清理摘要（去除首尾空白）
        let summary = summary.trim().to_string();
        if summary.is_empty() {
            return Err(anyhow!("LLM 生成的摘要为空"));
        }

        // 持久化摘要
        self.storage
            .update_document_summary(doc_id, &summary)
            .await?;
        Ok(())
    }

    async fn index_inner(&self, doc: &Document) -> anyhow::Result<()> {
        let ext = Self::validate_extension(Path::new(&doc.file_path))?;
        let file_size = std::fs::metadata(&doc.file_path)
            .map(|m| m.len())
            .unwrap_or(0);

        match ext.as_str() {
            "md" => {
                if file_size > MMAP_THRESHOLD {
                    // 大文件路径：memmap2 零拷贝 + 流式分块批量写入
                    self.index_md_mmap(doc).await
                } else {
                    // 小文件快速路径：read_to_string + 批量写入
                    let raw = tokio::fs::read_to_string(&doc.file_path)
                        .await
                        .with_context(|| format!("读取 Markdown 失败: {}", doc.file_path))?;
                    self.index_with_section(doc, raw).await
                }
            }
            "txt" => {
                if file_size > MMAP_THRESHOLD {
                    // 大文件路径：memmap2 零拷贝 + 流式分块批量写入
                    self.index_txt_mmap(doc).await
                } else {
                    // 小文件快速路径：read_to_string + 批量写入
                    let text = crate::loader::TextLoader.load(&doc.file_path).await?;
                    self.index_with_text(doc, text).await
                }
            }
            "pdf" => {
                // PDF 使用 lopdf 解析（需要完整文件），但可分块写入
                let text = crate::loader::PdfLoader.load(&doc.file_path).await?;
                self.index_with_text(doc, text).await
            }
            // .docx 文档（REQ-ING-015）：DocxLoader 解析 → 语义分块
            "docx" => {
                let text = crate::loader::DocxLoader.load(&doc.file_path).await?;
                self.index_with_text(doc, text).await
            }
            // .html 文档（REQ-ING-016）：HtmlLoader 解析 → 语义分块
            "html" | "htm" => {
                let text = crate::loader::HtmlLoader.load(&doc.file_path).await?;
                self.index_with_text(doc, text).await
            }
            // .pptx 演示文稿（REQ-ING-017）：PptxLoader 解析 → 语义分块
            "pptx" => {
                let text = crate::loader::PptxLoader.load(&doc.file_path).await?;
                self.index_with_text(doc, text).await
            }
            // .epub 电子书（REQ-ING-018）：EpubLoader 解析 → 语义分块
            "epub" => {
                let text = crate::loader::EpubLoader.load(&doc.file_path).await?;
                self.index_with_text(doc, text).await
            }
            // .xlsx 电子表格（REQ-ING-021）：XlsxLoader 解析 → 语义分块
            "xlsx" => {
                let text = crate::loader::XlsxLoader.load(&doc.file_path).await?;
                self.index_with_text(doc, text).await
            }
            // .csv 逗号分隔值（REQ-ING-021）：CsvLoader 解析 → 语义分块
            "csv" => {
                let text = crate::loader::CsvLoader.load(&doc.file_path).await?;
                self.index_with_text(doc, text).await
            }
            // 代码文件（REQ-RAG-031）：Free 模式按纯文本加载 + 语义分块
            // Pro 模式由调用方使用 `index_document_with_symbols` 走符号感知分块路径
            "rs" | "ts" | "tsx" | "py" | "go" => {
                let text = crate::loader::TextLoader.load(&doc.file_path).await?;
                self.index_with_text(doc, text).await
            }
            other => bail!("不支持的解析格式: .{other}"),
        }
    }

    /// .md 专用：SectionAwareSplitter 章节感知分块 + 批量入库（REQ-VEC-006）。
    /// 需要**原始 markdown 文本**（含 # 标题标记），不经过 MarkdownLoader。
    async fn index_with_section(&self, doc: &Document, text: String) -> anyhow::Result<()> {
        // 根据文件大小自动选择分块窗口（GB 级文档加速）
        let chunk_tokens = if self.chunk_tokens != DEFAULT_CHUNK_TOKENS {
            self.chunk_tokens
        } else {
            let file_size = std::fs::metadata(&doc.file_path)
                .map(|m| m.len())
                .unwrap_or(0);
            Self::auto_chunk_tokens(file_size)
        };
        let overlap = chunk_tokens / 8; // 12.5% overlap
        let splitter = crate::section_aware_splitter::SectionAwareSplitter::new_with_config(
            chunk_tokens,
            overlap,
        )?;
        let pieces = splitter.split(&text).await?;
        let chunks: Vec<Chunk> = pieces
            .into_iter()
            .enumerate()
            .map(|(sequence, content)| {
                let token_count = splitter.count_tokens(&content).unwrap_or(0);
                Chunk::new(doc.id.clone(), content, token_count, sequence)
            })
            .collect();
        // GB 级文件优化：分批批量入库，控制单事务大小与峰值内存
        for batch in chunks.chunks(CHUNK_BATCH_SIZE) {
            self.storage.add_chunks_batch(batch).await?;
        }
        // 实体抽取 + 索引（REQ-PERF-006 实体链接增强，内部按批处理）
        self.index_entities(&chunks).await?;
        // 实体关系抽取 + 索引（REQ-RAG-026 知识图谱实体关系检索）
        self.index_relations(&chunks).await?;
        // Proposition 分割 + 索引（REQ-PERF-007 Proposition 级原子分割）
        self.index_propositions(doc, &chunks).await?;
        // Wiki-link 解析 + 索引（REQ-ING-020 Markdown 笔记双向链接）
        self.index_wiki_links(doc, &chunks).await?;
        Ok(())
    }

    /// 分块 + 批量入库（复用逻辑，供 `index_inner` 和 `index_multimodal_inner` 共享）。
    async fn index_with_text(&self, doc: &Document, text: String) -> anyhow::Result<()> {
        // 根据文件大小自动选择分块窗口（GB 级文档加速）
        let chunk_tokens = if self.chunk_tokens != DEFAULT_CHUNK_TOKENS {
            self.chunk_tokens
        } else {
            let file_size = std::fs::metadata(&doc.file_path)
                .map(|m| m.len())
                .unwrap_or(0);
            Self::auto_chunk_tokens(file_size)
        };
        let overlap = chunk_tokens / 8; // 12.5% overlap
        // 使用 SemanticSplitter：
        // 段落→句子→子句递归分割，保留代码块完整性，支持中文标点。
        // 来源：rs-pro 语义分块器，借鉴 LangChain/LlamaIndex 最佳实践。
        let splitter = crate::semantic_splitter::SemanticSplitter::new(chunk_tokens, overlap)?;
        let pieces = splitter.split(&text).await?;
        let chunks: Vec<Chunk> = pieces
            .into_iter()
            .enumerate()
            .map(|(sequence, content)| {
                let token_count = splitter.count_tokens(&content).unwrap_or(0);
                Chunk::new(doc.id.clone(), content, token_count, sequence)
            })
            .collect();
        // GB 级文件优化：分批批量入库，控制单事务大小与峰值内存
        for batch in chunks.chunks(CHUNK_BATCH_SIZE) {
            self.storage.add_chunks_batch(batch).await?;
        }
        // 实体抽取 + 索引（REQ-PERF-006 实体链接增强，内部按批处理）
        self.index_entities(&chunks).await?;
        // 实体关系抽取 + 索引（REQ-RAG-026 知识图谱实体关系检索）
        self.index_relations(&chunks).await?;
        // Proposition 分割 + 索引（REQ-PERF-007 Proposition 级原子分割）
        self.index_propositions(doc, &chunks).await?;
        // Wiki-link 解析 + 索引（REQ-ING-020 Markdown 笔记双向链接）
        self.index_wiki_links(doc, &chunks).await?;
        Ok(())
    }

    /// memmap2 零拷贝路径：Markdown 大文件（>50MB）。
    ///
    /// 使用 memmap2 将文件映射到虚拟内存，Splitter 直接对 mmap 的 `&str` 分块，
    /// 避免 `read_to_string` 的堆分配。分块结果通过 `add_chunks_batch` 批量写入 DB。
    /// 峰值内存：mmap 元数据 + chunk Vec（自适应窗口控制 chunk 数），<500MB。
    async fn index_md_mmap(&self, doc: &Document) -> anyhow::Result<()> {
        let file = std::fs::File::open(&doc.file_path)
            .with_context(|| format!("打开 Markdown 文件失败: {}", doc.file_path))?;
        // SAFETY: mmap 映射的文件内容不可变；映射后仅在 Rust 侧通过 `&str` 读取。
        let mmap =
            unsafe { memmap2::Mmap::map(&file).with_context(|| "内存映射文件失败")? };
        let text = std::str::from_utf8(&mmap).with_context(|| "Markdown 文件非 UTF-8 编码")?;

        let chunk_tokens = Self::auto_chunk_tokens(mmap.len() as u64);
        let overlap = chunk_tokens / 8;
        let splitter = crate::section_aware_splitter::SectionAwareSplitter::new_with_config(
            chunk_tokens,
            overlap,
        )?;
        let pieces = splitter.split(text).await?;
        let chunks: Vec<Chunk> = pieces
            .into_iter()
            .enumerate()
            .map(|(seq, content)| {
                let token_count = splitter.count_tokens(&content).unwrap_or(0);
                Chunk::new(doc.id.clone(), content, token_count, seq)
            })
            .collect();
        // GB 级文件优化：分批批量入库，控制单事务大小与峰值内存
        for batch in chunks.chunks(CHUNK_BATCH_SIZE) {
            self.storage.add_chunks_batch(batch).await?;
        }
        self.index_entities(&chunks).await?;
        self.index_relations(&chunks).await?;
        self.index_propositions(doc, &chunks).await?;
        // Wiki-link 解析 + 索引（REQ-ING-020 Markdown 笔记双向链接）
        self.index_wiki_links(doc, &chunks).await?;
        Ok(())
    }

    /// memmap2 零拷贝路径：纯文本大文件（>50MB）。
    ///
    /// `memmap2` 映射 + `from_utf8_lossy` 容错解码 + `SemanticSplitter` 分块 + 批量写入。
    /// 注意：`from_utf8_lossy` 对合法 UTF-8 文本输出零拷贝 `Cow::Borrowed`，
    /// 仅在遇到非法字节时才分配堆内存。峰值内存 <500MB。
    async fn index_txt_mmap(&self, doc: &Document) -> anyhow::Result<()> {
        let file = std::fs::File::open(&doc.file_path)
            .with_context(|| format!("打开文本文件失败: {}", doc.file_path))?;
        let mmap =
            unsafe { memmap2::Mmap::map(&file).with_context(|| "内存映射文件失败")? };
        let text = String::from_utf8_lossy(&mmap);

        let chunk_tokens = Self::auto_chunk_tokens(mmap.len() as u64);
        let overlap = chunk_tokens / 8;
        let splitter = crate::semantic_splitter::SemanticSplitter::new(chunk_tokens, overlap)?;
        let pieces = splitter.split(&text).await?;
        let chunks: Vec<Chunk> = pieces
            .into_iter()
            .enumerate()
            .map(|(seq, content)| {
                let token_count = splitter.count_tokens(&content).unwrap_or(0);
                Chunk::new(doc.id.clone(), content, token_count, seq)
            })
            .collect();
        // GB 级文件优化：分批批量入库，控制单事务大小与峰值内存
        for batch in chunks.chunks(CHUNK_BATCH_SIZE) {
            self.storage.add_chunks_batch(batch).await?;
        }
        self.index_entities(&chunks).await?;
        self.index_relations(&chunks).await?;
        self.index_propositions(doc, &chunks).await?;
        // Wiki-link 解析 + 索引（REQ-ING-020 Markdown 笔记双向链接）
        self.index_wiki_links(doc, &chunks).await?;
        Ok(())
    }

    /// 实体抽取 + 批量索引（REQ-PERF-006 实体链接增强）。
    ///
    /// 从每个 chunk 中抽取命名实体（人名/专有名词/技术术语/标识符/日期），
    /// 批量写入 `entities` 表，供三路 RRF 实体检索通道使用。
    /// GB 级文件优化：按 CHUNK_BATCH_SIZE 分批抽取+写入，控制峰值内存。
    async fn index_entities(&self, chunks: &[Chunk]) -> anyhow::Result<()> {
        for batch in chunks.chunks(CHUNK_BATCH_SIZE) {
            let entities: Vec<(String, String, String)> = batch
                .iter()
                .flat_map(|chunk| {
                    crate::entity_extractor::EntityExtractor::extract_with_chunk_id(
                        &chunk.content,
                        &chunk.id,
                    )
                })
                .collect();
            if !entities.is_empty() {
                self.storage.add_entities(&entities).await?;
            }
        }
        Ok(())
    }

    /// 实体关系抽取 + 批量索引（REQ-RAG-026 知识图谱实体关系检索）。
    ///
    /// 从每个 chunk 中抽取实体间关系（defined_as / part_of / depends_on / uses /
    /// implements / extends / references / related_to），批量写入 `entity_relations` 表。
    /// 纯规则模式匹配（零 LLM），典型 chunk < 2ms。
    /// GB 级文件优化：按 CHUNK_BATCH_SIZE 分批抽取+写入，控制峰值内存。
    async fn index_relations(&self, chunks: &[Chunk]) -> anyhow::Result<()> {
        for batch in chunks.chunks(CHUNK_BATCH_SIZE) {
            let relations: Vec<echomind_models::EntityRelation> = batch
                .iter()
                .flat_map(|chunk| {
                    crate::entity_extractor::EntityExtractor::extract_relations(
                        &chunk.content,
                        &chunk.id,
                    )
                })
                .collect();
            if !relations.is_empty() {
                self.storage.add_relations_batch(&relations).await?;
            }
        }
        Ok(())
    }

    /// Proposition 分割 + 批量索引（REQ-PERF-007 Proposition 级原子分割）。
    ///
    /// 将每个 chunk 分割为自包含的原子事实（proposition），批量写入 `propositions` 表。
    /// proposition 的嵌入向量在导入后由嵌入管线单独计算。
    ///
    /// 零 LLM 调用：使用规则方案（句子分割 + 代词消解 + 上下文补全）。
    /// GB 级文件优化：按 CHUNK_BATCH_SIZE 分批分割+写入，控制峰值内存。
    async fn index_propositions(&self, doc: &Document, chunks: &[Chunk]) -> anyhow::Result<()> {
        let doc_name = Self::file_name(Path::new(&doc.file_path));
        for batch in chunks.chunks(CHUNK_BATCH_SIZE) {
            let propositions: Vec<echomind_models::Proposition> = batch
                .iter()
                .flat_map(|chunk| PropositionSplitter::split(&chunk.content, &chunk.id, &doc_name))
                .collect();
            if !propositions.is_empty() {
                self.storage.add_propositions(&propositions).await?;
            }
        }
        Ok(())
    }

    /// Wiki-link 解析 + 批量索引（REQ-ING-020 Markdown 笔记双向链接）。
    ///
    /// 从每个 chunk 中解析 `[[wiki-link]]` 语法，批量写入 `wiki_links` 表。
    /// 纯规则解析（正则 + 字符串扫描），零 LLM 调用。
    /// GB 级文件优化：按 CHUNK_BATCH_SIZE 分批解析+写入，控制峰值内存。
    async fn index_wiki_links(&self, doc: &Document, chunks: &[Chunk]) -> anyhow::Result<()> {
        for batch in chunks.chunks(CHUNK_BATCH_SIZE) {
            let links: Vec<echomind_models::WikiLink> = batch
                .iter()
                .flat_map(|chunk| {
                    crate::wiki_link_parser::parse_wiki_links(&chunk.content, &doc.id, &chunk.id)
                })
                .collect();
            if !links.is_empty() {
                self.storage.add_wiki_links(&links).await?;
            }
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
#[cfg(feature = "pro")]
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

                    // REQ-MM-005: VLM 分级图表理解（表格→MD / 流程图→Mermaid / 数据图表→CSV / 公式→LaTeX）
                    // 图片仅发送到用户配置的 LLM 端点（BYOK），符合隐私不出域
                    let vlm_text = vlm
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
                    bail!("PDF 未提取到任何文本: {}", doc.file_path);
                }
                full_text
            }
            "docx" => crate::loader::DocxLoader.load(&doc.file_path).await?,
            "html" | "htm" => crate::loader::HtmlLoader.load(&doc.file_path).await?,
            "pptx" => crate::loader::PptxLoader.load(&doc.file_path).await?,
            "epub" => crate::loader::EpubLoader.load(&doc.file_path).await?,
            "xlsx" => crate::loader::XlsxLoader.load(&doc.file_path).await?,
            "csv" => crate::loader::CsvLoader.load(&doc.file_path).await?,
            other => bail!("不支持的解析格式: .{other}"),
        };
        self.index_with_text(doc, text).await
    }

    /// 代码符号感知索引（REQ-RAG-031 代码感知 RAG）。
    ///
    /// 使用 tree-sitter AST 按函数/类边界分块（而非 token 窗口），
    /// 并抽取符号索引存储到 `code_symbols` 表。
    /// 每个 chunk 附带符号上下文前缀（`// Symbol: {name} ({kind})\n`），
    /// 使向量嵌入能捕获符号语义。
    ///
    /// # 参数
    /// - `doc`: 已导入的文档记录
    /// - `extractor`: 代码符号抽取引擎（`SymbolEngine` 实例）
    pub async fn index_document_with_symbols<E: SymbolExtractor>(
        &self,
        doc: &Document,
        extractor: &E,
    ) -> anyhow::Result<()> {
        self.storage
            .update_doc_status(&doc.id, DocStatus::Processing)
            .await?;
        let result = self.index_code_inner(doc, extractor).await;
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

    /// 代码符号感知索引内部逻辑（REQ-RAG-031）。
    async fn index_code_inner<E: SymbolExtractor>(
        &self,
        doc: &Document,
        extractor: &E,
    ) -> anyhow::Result<()> {
        let _ext = Self::validate_extension(Path::new(&doc.file_path))?;
        let text = crate::loader::TextLoader.load(&doc.file_path).await?;
        let language = extractor
            .detect_language(&doc.file_path)
            .ok_or_else(|| anyhow!("无法检测代码语言: {}", doc.file_path))?;

        // 符号感知分块：按函数/类边界分块，每块附带符号上下文前缀
        let chunks_raw = extractor.split_by_symbols(&text, &language, self.chunk_tokens);
        let doc_name = Self::file_name(Path::new(&doc.file_path));

        // 创建 Chunk 对象
        let chunks: Vec<Chunk> = chunks_raw
            .iter()
            .enumerate()
            .map(|(seq, (content, _start, _end))| {
                // 添加文件名前缀（向量嵌入上下文）
                let prefixed = format!("// File: {}\n{}", doc_name, content);
                let token_count = prefixed.len() / 4; // 粗略估算
                Chunk::new(doc.id.clone(), prefixed, token_count, seq)
            })
            .collect();

        // 分批写入 chunks
        for batch in chunks.chunks(CHUNK_BATCH_SIZE) {
            self.storage.add_chunks_batch(batch).await?;
        }

        // 符号抽取 + 索引
        self.index_symbols(&chunks, &language, extractor).await?;

        // 实体/关系/Proposition 索引（与文本路径一致）
        self.index_entities(&chunks).await?;
        self.index_relations(&chunks).await?;
        self.index_propositions(doc, &chunks).await?;
        // Wiki-link 解析 + 索引（与文本路径一致）
        self.index_wiki_links(doc, &chunks).await?;

        Ok(())
    }

    /// 代码符号抽取 + 批量索引（REQ-RAG-031）。
    ///
    /// 遍历每个 chunk → `SymbolExtractor::extract_symbols` → `storage.add_symbols`。
    /// 符号行号相对于 chunk 内容（含前缀注释），不调整到原文件绝对行号。
    async fn index_symbols<E: SymbolExtractor>(
        &self,
        chunks: &[Chunk],
        language: &str,
        extractor: &E,
    ) -> anyhow::Result<()> {
        let mut all_symbols = Vec::new();
        for chunk in chunks {
            let symbols = extractor.extract_symbols(&chunk.content, language, &chunk.id);
            all_symbols.extend(symbols);
        }
        if !all_symbols.is_empty() {
            self.storage.add_symbols(&all_symbols).await?;
        }
        Ok(())
    }
}
