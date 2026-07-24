//! 文件导入领域服务（REQ-ING-001/002/004、REQ-LIC-001、REQ-SEC 路径安全审查）。
//! 管线：路径安全校验 → 格式白名单 → 内容指纹去重（MD5）→ 免费版配额与 PDF 付费门
//! → 复制入数据目录 → 状态机推进（Pending → Processing → Indexed / Failed）。
//! 本模块只依赖 Storage / Loader / Splitter 端口，不感知 Tauri（六边形架构）。

use std::path::{Component, Path, PathBuf};

use anyhow::{Context, anyhow, bail};
use echomind_models::{Chunk, DocStatus, Document};

use crate::{Loader, Splitter, Storage};

/// 免费版文件数上限（REQ-LIC-001）
pub const FREE_TIER_MAX_FILES: usize = 50;
/// 格式白名单（REQ-ING-002）
pub const ALLOWED_EXTENSIONS: [&str; 3] = ["md", "txt", "pdf"];

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
            bail!("PDF 导入为 Pro 版功能，请升级后重试");
        }
        let count = self.storage.count_documents().await?;
        if count >= self.max_free_files {
            bail!(
                "免费版最多导入 {} 个文件（当前已达上限）",
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
    /// 注：本 Phase 覆盖 md/txt；PDF 解析适配器随 REQ-VEC-001 后续迭代交付，
    /// PDF 文档将标记 Failed 并携带明确原因（不影响入库事实）。
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
        let text = match ext.as_str() {
            "md" => crate::loader::MarkdownLoader.load(&doc.file_path).await?,
            "txt" => crate::loader::TextLoader.load(&doc.file_path).await?,
            other => bail!("暂不支持解析 .{other}（PDF 解析适配器将于后续迭代交付）"),
        };
        let splitter = crate::splitter::TextSplitter::new()?;
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
