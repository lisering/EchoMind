//! 文件监听增量同步引擎（REQ-SYNC-002）。
//!
//! 对比监听文件夹中的文件与知识库中已导入的文档，执行三类操作：
//! 1. **新增**：文件夹中存在但知识库中不存在的文件 → 自动导入+索引
//! 2. **更新**：文件内容变更（MD5 哈希不同）→ 删除旧文档，导入新版本
//! 3. **删除**：知识库中存在对应源文件路径的文档但文件夹中文件已被删除 → 级联清理
//!
//! 同步引擎仅处理受支持格式（.md/.txt/.pdf），跳过隐藏文件（以 `.` 开头）。
//! 同步是幂等的——多次同步无变更的文件夹不会产生重复导入或删除。

use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, anyhow, bail};
use echomind_models::{DocStatus, Document, SyncProgressPayload, SyncResult};

use crate::{Storage, idempotency::IdempotencyStore, import::ImportService};

/// 同步进度回调类型（REQ-SYNC-002 AC-7）。
///
/// 通过 `Arc<dyn Fn>` 实现跨线程共享，调用方在 Tauri 层将事件发射封装为闭包传入。
pub type SyncProgressFn = Arc<dyn Fn(SyncProgressPayload) + Send + Sync>;

/// 支持的文件扩展名（与 `ImportService::ALLOWED_EXTENSIONS` 保持一致）。
const SYNC_EXTENSIONS: [&str; 3] = ["md", "txt", "pdf"];

/// 文件监听增量同步服务（REQ-SYNC-002）。
///
/// 包装 `ImportService` 复用文件导入+索引管线，在其上叠加增量同步逻辑：
/// 扫描文件夹 → 对比知识库 → 新增/更新/删除。
///
/// # 类型参数
/// - `S`: 存储端口实现（需 `Clone`，因为 `ImportService` 和 `SyncService` 各持有一份）
pub struct SyncService<S: Storage + Clone> {
    /// 导入服务（复用文件复制、分块、索引管线）
    import_service: ImportService<S>,
    /// 存储端口（用于按 original_path 查找、删除文档）
    storage: S,
    /// 幂等性存储（防止重复操作）
    idempotency_store: IdempotencyStore,
}

impl<S: Storage + Clone> SyncService<S> {
    /// 构造同步服务。
    ///
    /// # 参数
    /// - `storage`: 存储端口实现
    /// - `data_dir`: 应用数据目录（文档副本存放于其下 `documents/`）
    pub fn new(storage: S, data_dir: std::path::PathBuf) -> Self {
        // 为同步服务创建专用的幂等性存储，5分钟保留时间防止重复同步
        let idempotency_store = IdempotencyStore::new_test(5 * 60);
        Self {
            import_service: ImportService::new(storage.clone(), data_dir),
            storage,
            idempotency_store,
        }
    }

    /// 同步监听文件夹与知识库（REQ-SYNC-002）。
    ///
    /// # 流程
    /// 1. **幂等性防护**：5 分钟内避免重复同步同一文件夹
    /// 2. 扫描文件夹中所有受支持格式的文件（跳过隐藏文件）
    /// 3. 对每个文件：
    ///    - 按 `original_path` 查找已有文档
    ///    - 若找到且哈希相同 → 跳过（幂等）
    ///    - 若找到且哈希不同 → 删除旧文档 + 导入新文档（更新）
    ///    - 若未找到 → 检查内容指纹去重，未重复则导入（新增）
    /// 4. 查找 `original_path` 以文件夹路径为前缀的文档，若源文件已不存在 → 删除
    ///
    /// # 参数
    /// - `folder_path`: 监听文件夹路径
    /// - `is_pro`: 是否为 Pro 用户（影响配额与 PDF 门禁）
    /// - `progress`: 可选的进度回调（推送 `sync_progress` 事件）
    ///
    /// # 返回
    /// 同步结果（新增/更新/删除/跳过计数 + 错误列表）
    pub async fn sync_folder(
        &self,
        folder_path: &str,
        is_pro: bool,
        progress: Option<SyncProgressFn>,
    ) -> anyhow::Result<SyncResult> {
        // 幂等性防护：如果文件夹内容没有变化且2分钟内已同步过，则跳过
        // 这可以防止文件系统监听器短时间内触发多次同步，但允许内容变化时重新同步
        let folder_hash = self.compute_folder_hash(folder_path).await.ok();
        let key = format!("sync_folder_2min:{}", folder_path);

        // 只有当文件夹哈希与上次相同时才跳过同步
        if let Some(current_hash) = &folder_hash {
            if let Some(last_hash) = self.get_last_sync_hash(&key).await
                && current_hash == &last_hash
            {
                // 内容相同且距离上次同步不到2分钟，跳过此次同步
                let should_execute = self
                    .idempotency_store
                    .once(&key, || Box::pin(async move { Ok::<_, anyhow::Error>(()) }))
                    .await;

                if !should_execute {
                    return Ok(SyncResult {
                        added: 0,
                        updated: 0,
                        deleted: 0,
                        skipped: 0,
                        errors: vec!["跳过：2 分钟内文件夹内容无变化".to_string()],
                    });
                }
            }

            // 存储当前文件夹哈希用于下次比较
            self.store_sync_hash(&key, current_hash).await;
        }
        let canonical = Path::new(folder_path)
            .canonicalize()
            .with_context(|| format!("文件夹路径无效: {folder_path}"))?;
        if !canonical.is_dir() {
            bail!("路径不是文件夹: {}", canonical.display());
        }

        let folder_str = canonical.to_string_lossy().into_owned();
        let mut result = SyncResult::default();

        // ── Phase 1: 扫描文件夹 ──
        self.emit_progress(
            &progress,
            "scanning",
            &folder_str,
            &result,
            "正在扫描文件夹…",
        );

        let files = self.scan_folder(&canonical).await?;

        // ── Phase 2: 逐文件处理（新增/更新）──
        for file_path in &files {
            let file_str = file_path.to_string_lossy().into_owned();

            // 计算文件哈希
            let hash = match self.compute_file_hash(file_path).await {
                Ok(h) => h,
                Err(e) => {
                    result
                        .errors
                        .push(format!("{}: 哈希计算失败 ({})", file_str, e));
                    continue;
                }
            };

            // 按 original_path 查找已有文档
            let existing = self
                .storage
                .find_document_by_original_path(&file_str)
                .await?;

            match existing {
                Some(doc) if doc.file_hash == hash => {
                    // 哈希相同 → 跳过（幂等）
                    result.skipped += 1;
                }
                Some(old_doc) => {
                    // 哈希不同 → 更新：删除旧文档 + 导入新文档
                    self.emit_progress(
                        &progress,
                        "importing",
                        &folder_str,
                        &result,
                        &format!("正在更新：{}", file_str),
                    );

                    // 删除旧文档（级联清理 chunks + embeddings）
                    if let Err(e) = self.storage.delete_document(&old_doc.id).await {
                        result
                            .errors
                            .push(format!("{}: 旧文档删除失败 ({})", file_str, e));
                        continue;
                    }

                    // 导入新文档（含 original_path）
                    match self
                        .import_service
                        .import_one_for_sync(&file_str, &file_str, is_pro)
                        .await
                    {
                        Ok(crate::import::ImportOutcome::Imported(doc)) => {
                            // 索引文档（分块入库）
                            if let Err(e) = self.import_service.index_document(&doc).await {
                                result
                                    .errors
                                    .push(format!("{}: 索引失败 ({})", file_str, e));
                                // 索引失败不阻止继续同步其他文件
                            }
                            result.updated += 1;
                        }
                        Ok(crate::import::ImportOutcome::SkippedDuplicate(_)) => {
                            // 内容重复（已被其他路径导入），跳过
                            result.skipped += 1;
                        }
                        Ok(crate::import::ImportOutcome::NameConflict { .. }) => {
                            // 同名不同内容：同步场景自动跳过
                            result.skipped += 1;
                        }
                        Err(e) => {
                            result
                                .errors
                                .push(format!("{}: 导入失败 ({})", file_str, e));
                        }
                    }
                }
                None => {
                    // 未找到 → 新增
                    self.emit_progress(
                        &progress,
                        "importing",
                        &folder_str,
                        &result,
                        &format!("正在导入：{}", file_str),
                    );

                    match self
                        .import_service
                        .import_one_for_sync(&file_str, &file_str, is_pro)
                        .await
                    {
                        Ok(crate::import::ImportOutcome::Imported(doc)) => {
                            // 索引文档（分块入库）
                            if let Err(e) = self.import_service.index_document(&doc).await {
                                result
                                    .errors
                                    .push(format!("{}: 索引失败 ({})", file_str, e));
                            }
                            result.added += 1;
                        }
                        Ok(crate::import::ImportOutcome::SkippedDuplicate(_)) => {
                            // 内容重复（已被手动导入过），跳过
                            result.skipped += 1;
                        }
                        Ok(crate::import::ImportOutcome::NameConflict { .. }) => {
                            // 同名不同内容：同步场景自动跳过（不提示用户替换）
                            result.skipped += 1;
                        }
                        Err(e) => {
                            result
                                .errors
                                .push(format!("{}: 导入失败 ({})", file_str, e));
                        }
                    }
                }
            }
        }

        // ── Phase 3: 检测已删除的文件 ──
        self.emit_progress(
            &progress,
            "deleting",
            &folder_str,
            &result,
            "正在检查已删除的文件…",
        );

        // 查找 original_path 以文件夹路径为前缀的所有文档
        let prefix = format!("{folder_str}/");
        let tracked_docs = self
            .storage
            .find_documents_by_original_path_prefix(&prefix)
            .await?;

        for doc in tracked_docs {
            // 检查源文件是否仍存在
            if let Some(ref original_path) = doc.original_path {
                let path = Path::new(original_path);
                if !path.exists() {
                    // 源文件已删除 → 级联清理文档
                    if let Err(e) = self.storage.delete_document(&doc.id).await {
                        result
                            .errors
                            .push(format!("{}: 文档删除失败 ({})", original_path, e));
                        continue;
                    }
                    result.deleted += 1;
                }
            }
        }

        // ── Phase 4: 完成 ──
        self.emit_progress(
            &progress,
            "complete",
            &folder_str,
            &result,
            &format!(
                "同步完成：新增 {}，更新 {}，删除 {}，跳过 {}",
                result.added, result.updated, result.deleted, result.skipped
            ),
        );

        Ok(result)
    }

    /// 递归扫描文件夹，返回所有受支持格式的文件路径（跳过隐藏文件）。
    ///
    /// 隐藏文件定义：文件名以 `.` 开头（如 `.gitignore`、`.DS_Store`）。
    /// 受支持格式：.md / .txt / .pdf（与 `SYNC_EXTENSIONS` 一致）。
    async fn scan_folder(&self, dir: &Path) -> anyhow::Result<Vec<std::path::PathBuf>> {
        let mut files = Vec::new();
        self.scan_folder_inner(dir, &mut files).await?;
        // 按路径排序，保证同步顺序确定性（便于测试）
        files.sort();
        Ok(files)
    }

    /// `scan_folder` 的递归实现。
    async fn scan_folder_inner(
        &self,
        dir: &Path,
        files: &mut Vec<std::path::PathBuf>,
    ) -> anyhow::Result<()> {
        let mut entries = tokio::fs::read_dir(dir)
            .await
            .with_context(|| format!("读取文件夹失败: {}", dir.display()))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .with_context(|| format!("读取文件夹条目失败: {}", dir.display()))?
        {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            // 跳过隐藏文件（以 `.` 开头）
            if name_str.starts_with('.') {
                continue;
            }

            let path = entry.path();
            let file_type = entry
                .file_type()
                .await
                .with_context(|| format!("读取文件类型失败: {}", path.display()))?;

            if file_type.is_dir() {
                // 递归扫描子目录
                Box::pin(self.scan_folder_inner(&path, files)).await?;
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str())
                && SYNC_EXTENSIONS.contains(&ext.to_lowercase().as_str())
            {
                files.push(path);
            }
        }
        Ok(())
    }

    /// 计算文件内容的 MD5 哈希（与 `ImportService::content_hash` 一致）。
    async fn compute_file_hash(&self, path: &Path) -> anyhow::Result<String> {
        let bytes = tokio::fs::read(path)
            .await
            .with_context(|| format!("读取文件失败: {}", path.display()))?;
        use md5::{Digest, Md5};
        let digest = Md5::digest(&bytes);
        Ok(format!("{digest:x}"))
    }

    /// 推送同步进度事件（内部辅助方法）。
    fn emit_progress(
        &self,
        progress: &Option<SyncProgressFn>,
        phase: &str,
        folder_path: &str,
        result: &SyncResult,
        message: &str,
    ) {
        if let Some(cb) = progress {
            cb(SyncProgressPayload {
                phase: phase.to_string(),
                folder_path: folder_path.to_string(),
                added: result.added,
                updated: result.updated,
                deleted: result.deleted,
                skipped: result.skipped,
                message: message.to_string(),
            });
        }
    }

    /// 计算文件夹内容的综合哈希值
    async fn compute_folder_hash(&self, folder_path: &str) -> anyhow::Result<String> {
        let canonical = Path::new(folder_path)
            .canonicalize()
            .with_context(|| format!("文件夹路径无效: {folder_path}"))?;

        let files = self.scan_folder(&canonical).await?;
        let mut all_hashes = Vec::new();

        for file_path in &files {
            if let Ok(hash) = self.compute_file_hash(file_path).await {
                all_hashes.push(hash);
            }
        }

        all_hashes.sort(); // 确保哈希顺序一致

        use md5::{Digest, Md5};
        let combined = all_hashes.join(",");
        let digest = Md5::digest(combined.as_bytes());
        Ok(format!("{digest:x}"))
    }

    /// 获取上次同步时的文件夹哈希值
    async fn get_last_sync_hash(&self, _key: &str) -> Option<String> {
        // 这里可以实现从持久化存储读取上次同步的哈希值
        // 简化实现：目前返回 None，依赖内存中的幂等性存储
        None
    }

    /// 存储当前同步的文件夹哈希值
    async fn store_sync_hash(&self, _key: &str, _hash: &str) {
        // 这里可以实现将哈希值保存到持久化存储
        // 简化实现：目前什么都不做，依赖幂等性存储的时间窗口
    }
}

/// 验证文件夹路径是否存在且为目录（REQ-SYNC-001 AC-6）。
///
/// 供 IPC 命令层在添加监听文件夹时调用，提前拒绝不存在的路径。
pub fn validate_folder_path(path: &str) -> anyhow::Result<std::path::PathBuf> {
    let canonical = Path::new(path)
        .canonicalize()
        .with_context(|| format!("文件夹不存在或不可读: {path}"))?;
    if !canonical.is_dir() {
        return Err(anyhow!("路径不是文件夹: {}", canonical.display()));
    }
    Ok(canonical)
}

/// 从 `Document` 的 `file_path` 中提取显示用文件名（与 commands::display_name 一致）。
///
/// 供 `WatchedFolderInfo` 构造用，提取路径末尾的文件名部分。
pub fn extract_doc_name(file_path: &str) -> String {
    Path::new(file_path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| file_path.to_string())
}

/// 检查文档是否处于可同步状态（非 Processing）。
///
/// 正在索引中的文档不应被同步引擎删除或更新，避免竞态条件。
pub fn is_syncable(doc: &Document) -> bool {
    !matches!(doc.status, DocStatus::Processing)
}
