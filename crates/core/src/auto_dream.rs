//! 后台空闲整理引擎（Auto Dream Engine）。
//!
//! 在用户空闲时自动扫描知识库，执行三项分析：
//! 1. **重复文档检测**——基于内容指纹（精确重复）+ 嵌入余弦相似度（近似重复）
//! 2. **跨文档矛盾发现**——embedding 预筛 + LLM 精判，发现不同文档间的信息冲突
//! 3. **整理建议生成**——按领域分组、合并建议、分类缺失提示
//!
//! 设计借鉴 `AuditEngine`（同层、同端口依赖），但作用域为全库而非单文档。
//! 零新依赖：复用 `Embedder` + `Storage` + `LLMProvider` 端口。

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use futures::StreamExt;
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};

use crate::{Embedder, LLMProvider, Storage, idempotency::IdempotencyStore};
use echomind_models::MemoryTier;

// ================== 数据结构 ==================

/// 建议类型分类。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuggestionType {
    /// 重复文档（精确或近似）
    DuplicateDocuments,
    /// 跨文档矛盾
    Contradiction,
    /// 整理建议（合并/分类/标签）
    Organization,
}

/// 建议严重等级。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DreamSeverity {
    /// 高——需要用户立即关注（如核心信息矛盾）
    High,
    /// 中——建议处理（如近似重复文档）
    Medium,
    /// 低——提示性信息（如未分类文档）
    Low,
}

/// 单条整理建议。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DreamSuggestion {
    /// 建议唯一标识
    pub suggestion_id: String,
    /// 建议类型
    pub suggestion_type: SuggestionType,
    /// 标题（简短描述）
    pub title: String,
    /// 详细描述
    pub description: String,
    /// 涉及的文档 ID 列表
    pub doc_ids: Vec<String>,
    /// 涉及的文档名列表（用于前端展示）
    pub doc_names: Vec<String>,
    /// 严重等级
    pub severity: DreamSeverity,
    /// 相似度得分（仅重复文档类型有值，0.0~1.0）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub similarity: Option<f32>,
}

/// Dream 分析结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DreamResult {
    /// 全部建议列表
    pub suggestions: Vec<DreamSuggestion>,
    /// 扫描的文档总数
    pub total_documents: usize,
    /// 发现的建议总数
    pub total_suggestions: usize,
    /// 分析耗时（毫秒）
    pub elapsed_ms: u64,
}

/// Dream 取消信号（与 AuditCancelFlag 同模式）。
pub type DreamCancelFlag = Arc<AtomicBool>;

/// 近似重复检测的余弦相似度阈值。
/// 高于此阈值的文档对被视为近似重复。
const NEAR_DUPLICATE_THRESHOLD: f32 = 0.92;

/// 跨文档矛盾预筛的余弦相似度阈值。
/// 高于此阈值的 chunk 对才进入 LLM 精判（降低 O(n²) 开销）。
const CONTRADICTION_PREFILTER_THRESHOLD: f32 = 0.70;

/// 每个文档参与跨文档矛盾检测的最大 chunk 数（限制 LLM 调用）。
const MAX_CHUNKS_PER_DOC_FOR_CONTRADICTION: usize = 3;

// ================== DreamEngine ==================

/// 后台空闲整理引擎。
///
/// 六边形架构：仅依赖 `Embedder` + `Storage` + `LLMProvider` 端口。
/// 与 `AuditEngine` 平级，作用域为全库（`list_documents` + `list_chunks`）。
pub struct DreamEngine<E: Embedder, S: Storage, L: LLMProvider> {
    embedder: E,
    storage: S,
    llm: L,
    idempotency_store: IdempotencyStore,
}

impl<E: Embedder, S: Storage + Clone, L: LLMProvider> DreamEngine<E, S, L> {
    /// 构造 Dream 引擎。
    pub fn new(embedder: E, storage: S, llm: L, idempotency_store: IdempotencyStore) -> Self {
        Self {
            embedder,
            storage,
            llm,
            idempotency_store,
        }
    }

    /// 执行全库 Dream 分析（三阶段流水线）。
    ///
    /// # 参数
    /// - `cancel`: 取消信号（AtomicBool，true 表示请求取消）
    ///
    /// # 返回
    /// `DreamResult`，包含全部建议列表。
    /// 如果被取消，返回已完成的部分结果。
    pub async fn dream(&self, cancel: DreamCancelFlag) -> anyhow::Result<DreamResult> {
        // 使用幂等性存储防止重复执行
        // 如果已经有执行在进行中或已完成，则跳过这次执行
        let should_execute = self
            .idempotency_store
            .once("auto_dream_full_scan", || {
                Box::pin(async move {
                    anyhow::Ok(()) // 实际执行会在主逻辑中进行
                })
            })
            .await;

        if !should_execute {
            // 已有执行在进行中或已完成，返回空结果
            return Ok(DreamResult {
                suggestions: vec![],
                total_documents: 0,
                total_suggestions: 0,
                elapsed_ms: 0,
            });
        }

        // 执行实际的 Dream 分析
        let start = Instant::now();
        let mut suggestions = Vec::new();

        // ---- Phase 0: 加载全部文档 ----
        let documents = self.storage.list_documents().await?;
        if documents.is_empty() {
            return Ok(DreamResult {
                suggestions: vec![],
                total_documents: 0,
                total_suggestions: 0,
                elapsed_ms: start.elapsed().as_millis() as u64,
            });
        }

        // ---- Phase 1: 重复文档检测 ----
        if !is_cancelled(&cancel) {
            let dup_suggestions = self.detect_duplicates(&documents).await?;
            suggestions.extend(dup_suggestions);
        }

        // ---- Phase 2: 跨文档矛盾发现 ----
        if !is_cancelled(&cancel) {
            let contra_suggestions = self
                .detect_cross_doc_contradictions(&documents, &cancel)
                .await?;
            suggestions.extend(contra_suggestions);
        }

        // ---- Phase 3: 整理建议生成 ----
        if !is_cancelled(&cancel) {
            let org_suggestions = self.generate_organization_suggestions(&documents).await?;
            suggestions.extend(org_suggestions);
        }

        // ---- Phase 4: 记忆整合（REQ-RAG-032）----
        // AutoDream 空闲时自动执行 Wing→Hall→Room 提升 + 低分遗忘
        if !is_cancelled(&cancel) {
            let mem_suggestions = self.consolidate_memories().await?;
            suggestions.extend(mem_suggestions);
        }

        // ---- Phase 5: Scratch 层整合（Q01 借鉴 QM scratch-promote + consolidation）----
        // AutoDream 空闲时自动执行 Scratch 层 LLM 驱动整合
        if !is_cancelled(&cancel) {
            let scratch_suggestions = self.consolidate_scratch_layer().await?;
            suggestions.extend(scratch_suggestions);
        }

        let total_suggestions = suggestions.len();
        Ok(DreamResult {
            suggestions,
            total_documents: documents.len(),
            total_suggestions,
            elapsed_ms: start.elapsed().as_millis() as u64,
        })
    }

    /// Phase 1: 重复文档检测。
    ///
    /// 两种策略：
    /// - **精确重复**：内容指纹（MD5）完全相同
    /// - **近似重复**：首 chunk 嵌入余弦相似度 > 阈值
    async fn detect_duplicates(
        &self,
        documents: &[echomind_models::Document],
    ) -> anyhow::Result<Vec<DreamSuggestion>> {
        let mut suggestions = Vec::new();

        // --- 精确重复：按 hash 分组 ---
        let mut hash_groups: HashMap<&str, Vec<&echomind_models::Document>> = HashMap::new();
        for doc in documents {
            hash_groups
                .entry(doc.file_hash.as_str())
                .or_default()
                .push(doc);
        }
        for docs in hash_groups.values() {
            if docs.len() > 1 {
                let doc_ids: Vec<String> = docs.iter().map(|d| d.id.clone()).collect();
                let doc_names: Vec<String> =
                    docs.iter().map(|d| display_name(&d.file_path)).collect();
                suggestions.push(DreamSuggestion {
                    suggestion_id: format!(
                        "dup-exact-{}",
                        doc_ids.first().unwrap_or(&String::new())
                    ),
                    suggestion_type: SuggestionType::DuplicateDocuments,
                    title: format!("发现 {} 个完全相同的文档", docs.len()),
                    description: format!(
                        "以下文档内容指纹完全一致，建议保留一个并删除其余：\n{}",
                        doc_names
                            .iter()
                            .map(|n| format!("  - {n}"))
                            .collect::<Vec<_>>()
                            .join("\n")
                    ),
                    doc_ids,
                    doc_names,
                    severity: DreamSeverity::Medium,
                    similarity: Some(1.0),
                });
            }
        }

        // --- 近似重复：嵌入相似度 ---
        // 为每个文档获取首个 chunk 的嵌入
        let doc_embeddings = self.compute_doc_representations(documents).await?;

        // 遍历文档对，计算相似度
        for i in 0..documents.len() {
            for j in (i + 1)..documents.len() {
                // 跳过已标记为精确重复的文档对
                if documents[i].file_hash == documents[j].file_hash {
                    continue;
                }

                let Some(emb_i) = &doc_embeddings[i] else {
                    continue;
                };
                let Some(emb_j) = &doc_embeddings[j] else {
                    continue;
                };

                let sim = cosine_similarity(emb_i, emb_j);
                if sim >= NEAR_DUPLICATE_THRESHOLD {
                    let doc_ids = vec![documents[i].id.clone(), documents[j].id.clone()];
                    let doc_names = vec![
                        display_name(&documents[i].file_path),
                        display_name(&documents[j].file_path),
                    ];
                    suggestions.push(DreamSuggestion {
                        suggestion_id: format!("dup-near-{}-{}", documents[i].id, documents[j].id),
                        suggestion_type: SuggestionType::DuplicateDocuments,
                        title: format!("近似重复文档（相似度 {:.0}%）", sim * 100.0),
                        description: format!(
                            "《{}》与《{}》内容高度相似（{:.1}%），建议合并或去重。",
                            doc_names[0],
                            doc_names[1],
                            sim * 100.0
                        ),
                        doc_ids,
                        doc_names,
                        severity: DreamSeverity::Low,
                        similarity: Some(sim),
                    });
                }
            }
        }

        Ok(suggestions)
    }

    /// Phase 2: 跨文档矛盾发现。
    ///
    /// 1. 为每个文档取前 N 个 chunk 作为代表
    /// 2. 批量嵌入所有代表 chunk
    /// 3. 遍历跨文档 chunk 对，预筛高相似度对
    /// 4. 对预筛通过的对调用 LLM 判定矛盾
    async fn detect_cross_doc_contradictions(
        &self,
        documents: &[echomind_models::Document],
        cancel: &DreamCancelFlag,
    ) -> anyhow::Result<Vec<DreamSuggestion>> {
        let mut suggestions = Vec::new();

        // 收集每个文档的代表 chunk（前 N 个）
        let mut rep_chunks: Vec<(usize, echomind_models::Chunk)> = Vec::new(); // (doc_index, chunk)
        for (doc_idx, doc) in documents.iter().enumerate() {
            let chunks = self.storage.list_chunks(&doc.id).await?;
            for chunk in chunks
                .into_iter()
                .take(MAX_CHUNKS_PER_DOC_FOR_CONTRADICTION)
            {
                rep_chunks.push((doc_idx, chunk));
            }
        }

        if rep_chunks.len() < 2 {
            return Ok(suggestions);
        }

        // 批量嵌入所有代表 chunk
        let texts: Vec<String> = rep_chunks.iter().map(|(_, c)| c.content.clone()).collect();
        let embeddings = self.embedder.embed_batch(&texts).await?;

        // 遍历跨文档 chunk 对（仅不同文档之间）
        for i in 0..rep_chunks.len() {
            if is_cancelled(cancel) {
                break;
            }
            for j in (i + 1)..rep_chunks.len() {
                let (doc_idx_i, chunk_i) = &rep_chunks[i];
                let (doc_idx_j, chunk_j) = &rep_chunks[j];

                // 仅比对来自不同文档的 chunk
                if doc_idx_i == doc_idx_j {
                    continue;
                }

                let sim = cosine_similarity(&embeddings[i], &embeddings[j]);
                if sim < CONTRADICTION_PREFILTER_THRESHOLD {
                    continue;
                }

                // LLM 精判
                let prompt = build_contradiction_prompt(
                    &chunk_i.content,
                    &documents[*doc_idx_i].file_path,
                    &chunk_j.content,
                    &documents[*doc_idx_j].file_path,
                );
                let stream = self.llm.chat_stream(&prompt, &[], "").await?;
                let raw = collect_stream(stream).await?;

                // JSON 解析：失败时跳过此对（优雅降级）
                let Ok(resp) = serde_json::from_str::<ContradictionCheckResponse>(&raw) else {
                    continue;
                };

                if resp.is_contradiction {
                    let doc_ids = vec![
                        documents[*doc_idx_i].id.clone(),
                        documents[*doc_idx_j].id.clone(),
                    ];
                    let doc_names = vec![
                        display_name(&documents[*doc_idx_i].file_path),
                        display_name(&documents[*doc_idx_j].file_path),
                    ];
                    suggestions.push(DreamSuggestion {
                        suggestion_id: format!("contra-{}-{}", chunk_i.id, chunk_j.id),
                        suggestion_type: SuggestionType::Contradiction,
                        title: format!("跨文档矛盾：{}", resp.topic),
                        description: resp.explanation,
                        doc_ids,
                        doc_names,
                        severity: match resp.severity.as_str() {
                            "high" => DreamSeverity::High,
                            "medium" => DreamSeverity::Medium,
                            _ => DreamSeverity::Low,
                        },
                        similarity: Some(sim),
                    });
                }
            }
        }

        Ok(suggestions)
    }

    /// Phase 3: 整理建议生成。
    ///
    /// - 未分类文档提示
    /// - 同领域文档过多提示
    async fn generate_organization_suggestions(
        &self,
        documents: &[echomind_models::Document],
    ) -> anyhow::Result<Vec<DreamSuggestion>> {
        let mut suggestions = Vec::new();

        // 未分类文档
        let unclassified: Vec<&echomind_models::Document> =
            documents.iter().filter(|d| d.domain.is_none()).collect();
        if !unclassified.is_empty() {
            let doc_ids: Vec<String> = unclassified.iter().map(|d| d.id.clone()).collect();
            let doc_names: Vec<String> = unclassified
                .iter()
                .map(|d| display_name(&d.file_path))
                .collect();
            suggestions.push(DreamSuggestion {
                suggestion_id: "org-unclassified".to_string(),
                suggestion_type: SuggestionType::Organization,
                title: format!("{} 个文档尚未分类", unclassified.len()),
                description: format!(
                    "以下文档尚未自动分类，建议重新分类以提升检索精度：\n{}",
                    doc_names
                        .iter()
                        .map(|n| format!("  - {n}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                ),
                doc_ids,
                doc_names,
                severity: DreamSeverity::Low,
                similarity: None,
            });
        }

        // 同领域文档统计
        let mut domain_counts: HashMap<&str, Vec<&echomind_models::Document>> = HashMap::new();
        for doc in documents {
            if let Some(domain) = &doc.domain {
                domain_counts.entry(domain.as_str()).or_default().push(doc);
            }
        }
        for (domain, docs) in &domain_counts {
            if docs.len() >= 10 {
                let doc_ids: Vec<String> = docs.iter().map(|d| d.id.clone()).collect();
                let doc_names: Vec<String> =
                    docs.iter().map(|d| display_name(&d.file_path)).collect();
                suggestions.push(DreamSuggestion {
                    suggestion_id: format!("org-domain-{domain}-merge"),
                    suggestion_type: SuggestionType::Organization,
                    title: format!("领域「{}」有 {} 个文档，建议整理", domain, docs.len()),
                    description: format!(
                        "领域「{domain}」下文档较多（{} 个），建议检查是否有可合并或去重的内容。",
                        docs.len()
                    ),
                    doc_ids,
                    doc_names,
                    severity: DreamSeverity::Low,
                    similarity: None,
                });
            }
        }

        Ok(suggestions)
    }

    /// Phase 4: 记忆整合（REQ-RAG-032）。
    ///
    /// 执行三层记忆整合（逻辑与 `MemoryStore::consolidate()` 一致）：
    /// - Wing 层超限时：importance ≥ 0.6 → 提升到 Hall，< 0.3 → 遗忘
    /// - Hall 层超限时：importance ≥ 0.7 → 提升到 Room，< 0.3 → 遗忘
    /// - Room 层超限时：access_count 最低的优先遗忘
    ///
    /// 整合结果生成一条 `DreamSuggestion` 供用户查看。
    async fn consolidate_memories(&self) -> anyhow::Result<Vec<DreamSuggestion>> {
        // 默认上限（与 MemoryStore 默认值一致）
        const MAX_WING: usize = 50;
        const MAX_HALL: usize = 100;
        const MAX_ROOM: usize = 500;

        // 检查是否有记忆条目（避免空库时产生无意义建议）
        let all_memories = self.storage.get_memory_entries(None).await?;
        if all_memories.is_empty() {
            return Ok(Vec::new());
        }

        let mut promoted = 0usize;
        let mut forgotten = 0usize;

        // ---- Wing 层整合 ----
        let wing_entries = self
            .storage
            .get_memory_entries(Some(&MemoryTier::Wing))
            .await?;
        if wing_entries.len() > MAX_WING {
            for entry in &wing_entries {
                if entry.importance >= 0.6 {
                    let mut updated = entry.clone();
                    updated.tier = MemoryTier::Hall;
                    updated.importance = (updated.importance + 0.1).min(1.0);
                    self.storage.update_memory_entry(&updated).await?;
                    promoted += 1;
                } else if entry.importance < 0.3 {
                    self.storage.delete_memory_entry(&entry.id).await?;
                    forgotten += 1;
                }
            }
        }

        // ---- Hall 层整合 ----
        let hall_entries = self
            .storage
            .get_memory_entries(Some(&MemoryTier::Hall))
            .await?;
        if hall_entries.len() > MAX_HALL {
            for entry in &hall_entries {
                if entry.importance >= 0.7 {
                    let mut updated = entry.clone();
                    updated.tier = MemoryTier::Room;
                    updated.importance = (updated.importance + 0.1).min(1.0);
                    self.storage.update_memory_entry(&updated).await?;
                    promoted += 1;
                } else if entry.importance < 0.3 {
                    self.storage.delete_memory_entry(&entry.id).await?;
                    forgotten += 1;
                }
            }
        }

        // ---- Room 层整合 ----
        let room_entries = self
            .storage
            .get_memory_entries(Some(&MemoryTier::Room))
            .await?;
        if room_entries.len() > MAX_ROOM {
            let mut sorted = room_entries.clone();
            sorted.sort_by_key(|a| a.access_count);
            let to_forget = room_entries.len().saturating_sub(MAX_ROOM);
            for entry in sorted.iter().take(to_forget) {
                self.storage.delete_memory_entry(&entry.id).await?;
                forgotten += 1;
            }
        }

        // 仅在有提升或遗忘时生成建议
        if promoted == 0 && forgotten == 0 {
            return Ok(Vec::new());
        }

        // 统计剩余数量
        let remaining_wing = self
            .storage
            .get_memory_entries(Some(&MemoryTier::Wing))
            .await?
            .len();
        let remaining_hall = self
            .storage
            .get_memory_entries(Some(&MemoryTier::Hall))
            .await?
            .len();
        let remaining_room = self
            .storage
            .get_memory_entries(Some(&MemoryTier::Room))
            .await?
            .len();

        let mut description = format!("记忆整合完成：提升 {promoted} 条，遗忘 {forgotten} 条。");
        description.push_str(&format!(
            "\n当前记忆：Wing 层 {wing} 条，Hall 层 {hall} 条，Room 层 {room} 条。",
            wing = remaining_wing,
            hall = remaining_hall,
            room = remaining_room
        ));

        Ok(vec![DreamSuggestion {
            suggestion_id: "memory-consolidate".to_string(),
            suggestion_type: SuggestionType::Organization,
            title: format!("记忆整合：提升 {} 条，遗忘 {} 条", promoted, forgotten),
            description,
            doc_ids: vec![],
            doc_names: vec![],
            severity: DreamSeverity::Low,
            similarity: None,
        }])
    }

    /// Phase 5: Scratch 层 LLM 驱动整合（Q01 借鉴 QM scratch-promote + consolidation）。
    ///
    /// 读取 scratch_logs 表中积累的临时事实，通过 LLM 审查后执行
    /// UPDATE/DELETE/ADD 动作，将有价值的事实 promote 到长期记忆层。
    /// 整合完成后清除已处理的 scratch 条目。
    async fn consolidate_scratch_layer(&self) -> anyhow::Result<Vec<DreamSuggestion>> {
        // 检查是否有 scratch 条目
        let scratch_entries = self.storage.get_scratch_logs(None).await?;
        if scratch_entries.is_empty() {
            return Ok(Vec::new());
        }

        let count = scratch_entries.len();

        // 执行 Scratch 整合（复用 MemoryStore 逻辑）
        let memory_store = crate::memory_store::MemoryStore::new(self.storage.clone());
        let result = memory_store.consolidate_scratch(&self.llm, 14).await?;

        if result.actions.is_empty() && result.expired_cleaned == 0 {
            return Ok(Vec::new());
        }

        let mut description = format!("Scratch 层整合：处理 {count} 条临时事实。");
        description.push_str(&format!(
            "\n动作数：{}，过期清理：{}，剩余：{}。",
            result.actions.len(),
            result.expired_cleaned,
            result.remaining_scratch
        ));

        Ok(vec![DreamSuggestion {
            suggestion_id: "scratch-consolidate".to_string(),
            suggestion_type: SuggestionType::Organization,
            title: format!(
                "Scratch 整合：{} 个动作，清理 {} 条过期",
                result.actions.len(),
                result.expired_cleaned
            ),
            description,
            doc_ids: vec![],
            doc_names: vec![],
            severity: DreamSeverity::Low,
            similarity: None,
        }])
    }

    /// 为每个文档计算代表性嵌入（首个 chunk 的嵌入）。
    ///
    /// 返回与 `documents` 等长的 `Vec<Option<Vec<f32>>>`：
    /// - `Some(embedding)` — 文档有 chunk 且嵌入成功
    /// - `None` — 文档无 chunk 或嵌入失败
    async fn compute_doc_representations(
        &self,
        documents: &[echomind_models::Document],
    ) -> anyhow::Result<Vec<Option<Vec<f32>>>> {
        let mut result = Vec::with_capacity(documents.len());
        for doc in documents {
            let chunks = self.storage.list_chunks(&doc.id).await?;
            if let Some(first_chunk) = chunks.first() {
                match self.embedder.embed(&first_chunk.content).await {
                    Ok(emb) => result.push(Some(emb)),
                    Err(_) => result.push(None),
                }
            } else {
                result.push(None);
            }
        }
        Ok(result)
    }
}

// ================== 辅助函数 ==================

/// 检查取消标志是否已触发。
fn is_cancelled(cancel: &DreamCancelFlag) -> bool {
    cancel.load(Ordering::SeqCst)
}

/// 从文件路径提取展示用文件名。
fn display_name(raw_path: &str) -> String {
    std::path::Path::new(raw_path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| raw_path.to_string())
}

/// 将 LLM token 流收集为完整字符串。
async fn collect_stream(
    mut stream: BoxStream<'static, anyhow::Result<String>>,
) -> anyhow::Result<String> {
    let mut result = String::new();
    while let Some(item) = stream.next().await {
        match item {
            Ok(token) => result.push_str(&token),
            Err(e) => return Err(e),
        }
    }
    Ok(result)
}

/// 计算余弦相似度。
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom == 0.0 { 0.0 } else { dot / denom }
}

/// LLM 跨文档矛盾判定响应。
#[derive(Debug, Deserialize)]
struct ContradictionCheckResponse {
    /// 两段文本是否互相矛盾
    is_contradiction: bool,
    /// 矛盾涉及的主题
    topic: String,
    /// 判定理由
    explanation: String,
    /// 严重等级（high / medium / low）
    severity: String,
}

/// 构建跨文档矛盾判定提示词。
fn build_contradiction_prompt(
    text_a: &str,
    source_a: &str,
    text_b: &str,
    source_b: &str,
) -> String {
    format!(
        "你是知识库整理助手。请判断以下来自不同文档的两段文本是否存在信息矛盾。\n\n\
         文档 A《{source_a}》：\n{text_a}\n\n\
         文档 B《{source_b}》：\n{text_b}\n\n\
         判定标准：\n\
         - 两段文本对同一事实/参数/结论给出互相冲突的信息 → is_contradiction: true\n\
         - 两段文本信息一致或互补 → is_contradiction: false\n\
         - 仅讨论同一主题但无直接冲突 → is_contradiction: false\n\n\
         输出格式：JSON 对象，包含以下字段：\n\
         - is_contradiction: 布尔值，是否矛盾\n\
         - topic: 矛盾涉及的主题（如\"温度参数\"、\"实验结论\"）\n\
         - explanation: 判定理由说明\n\
         - severity: 严重等级（high / medium / low）\n\n\
         仅输出 JSON，不要添加 Markdown 代码块标记。"
    )
}
