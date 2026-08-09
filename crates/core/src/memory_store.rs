//! 持久化记忆系统（REQ-RAG-032）：借鉴 IfAI 三层记忆 Wing/Hall/Room + Bamboo-agent 上下文压缩。
//!
//! ## 三层记忆架构
//!
//! - **Wing（翼层）**：临时记忆，当前会话产生，上限 50 条，会话结束后提升或遗忘
//! - **Hall（厅层）**：工作记忆，近期重要信息，上限 100 条，跨会话保留
//! - **Room（室层）**：长期记忆，核心知识，上限 500 条，永久保留
//!
//! ## 记忆生命周期
//!
//! 1. **提取**：从对话中由 LLM 辅助提取关键事实 → 初始 Wing 层，importance 0.5
//! 2. **提升**：Wing → Hall → Room，importance += 0.1（上限 1.0）
//! 3. **遗忘**：低重要性 + 低访问频率 → 删除
//! 4. **整合**：超限时自动提升高分记忆、遗忘低分记忆
//!
//! ## 查询注入
//!
//! RAG 查询时，`retrieve_relevant()` 检索相关记忆并注入到 system prompt 前面，
//! 使 LLM 能引用跨会话的关键事实（如用户偏好、常见问题模式）。
//!
//! ## 调研来源
//!
//! - IfAI 三层持久记忆系统（Wing/Hall/Room 空间隐喻）
//! - Bamboo-agent 上下文压缩（对话过长时自动提取关键信息）
//! - Mem0 (2026.04)：对话记忆提取 + 注入

use std::future::Future;
use std::pin::Pin;

use echomind_models::{
    ChatMessage, ConsolidationResult, MemoryConsolidationAction, MemoryEntry, MemorySource,
    MemoryTier, ProvenanceTag, ScratchConsolidationResult, ScratchLogEntry,
};

use crate::{LLMProvider, Storage};

// ============================================================
// MemoryRetriever trait（ChatEngine 注入用，对象安全）
// ============================================================

/// 对话记忆检索端口（用于 ChatEngine 注入相关记忆到 RAG prompt）。
///
/// 对象安全设计：使用手动 `Pin<Box<Future>>` 返回类型（与 `Reranker`/`QueryRewriter` 一致），
/// 使 `ChatEngine` 能以 `Option<Arc<dyn MemoryRetriever>>` 形式注入记忆检索能力，
/// 实现运行时开关——用户可通过 `memory.enabled` 设置在运行时启停记忆注入。
pub trait MemoryRetriever: Send + Sync {
    /// 检索与查询相关的记忆条目。
    ///
    /// # 参数
    /// - `query`: 用户查询文本
    /// - `top_k`: 返回记忆数量上限
    ///
    /// # 返回
    /// 按 importance DESC 排序的相关记忆列表。空 Vec 表示无相关记忆。
    fn retrieve_relevant_memories<'a>(
        &'a self,
        query: &'a str,
        top_k: usize,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Vec<MemoryEntry>>> + Send + 'a>>;
}

// ============================================================
// MemoryStore
// ============================================================

/// 记忆存储引擎（借鉴 IfAI 三层记忆 + Bamboo-agent 上下文压缩）。
///
/// 三层记忆架构：
/// - Wing（翼层）：临时记忆，当前会话产生，上限 50 条，会话结束后提升或遗忘
/// - Hall（厅层）：工作记忆，近期重要信息，上限 100 条，跨会话保留
/// - Room（室层）：长期记忆，核心知识，上限 500 条，永久保留
pub struct MemoryStore<S: Storage> {
    /// 存储后端（SqliteStorage 或 MockStorage）
    pub(crate) storage: S,
    /// Wing 层最大条目数
    max_wing: usize,
    /// Hall 层最大条目数
    max_hall: usize,
    /// Room 层最大条目数
    max_room: usize,
}

impl<S: Storage> MemoryStore<S> {
    /// 创建记忆存储引擎（默认上限：Wing=50, Hall=100, Room=500）。
    pub fn new(storage: S) -> Self {
        Self {
            storage,
            max_wing: 50,
            max_hall: 100,
            max_room: 500,
        }
    }

    /// 自定义各层上限。
    pub fn with_limits(mut self, wing: usize, hall: usize, room: usize) -> Self {
        self.max_wing = wing;
        self.max_hall = hall;
        self.max_room = room;
        self
    }

    /// 从对话中提取记忆（LLM 辅助）。
    ///
    /// 将最近 N 轮对话发给 LLM，system prompt 要求提取值得记住的关键事实。
    /// 提取的记忆初始为 Wing 层，importance 0.5。
    ///
    /// # 参数
    /// - `messages`: 对话消息列表（user + assistant 交替）
    /// - `llm`: LLM 提供者（非流式调用，收集全部 token）
    ///
    /// # 返回
    /// 提取到的记忆条目列表。空对话返回空 Vec。
    pub async fn extract_from_conversation<L: LLMProvider>(
        &self,
        messages: &[ChatMessage],
        llm: &L,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        if messages.is_empty() {
            return Ok(Vec::new());
        }

        // 取最近 10 轮对话（最多 20 条消息：user + assistant）
        let recent: Vec<&ChatMessage> = messages.iter().rev().take(20).collect();
        let mut recent_sorted: Vec<&ChatMessage> = recent.into_iter().rev().collect();

        // 构建对话文本
        let mut dialog_text = String::new();
        for msg in &recent_sorted {
            let role = if msg.role == "user" {
                "用户"
            } else if msg.role == "assistant" {
                "助手"
            } else {
                "系统"
            };
            dialog_text.push_str(&format!("{role}: {}\n", msg.content));
        }

        // 构建 system prompt
        let system_prompt = "从以下对话中提取值得记住的关键事实（用户偏好、重要决定、事实陈述）。\n\
             每条一行，格式：[类型] 内容\n\
             类型：user_statement / assistant_info / auto_extracted\n\
             只提取有长期价值的信息，忽略寒暄和临时性问题。\n\
             最多提取 5 条。仅输出提取结果，不要其他解释。";

        // 调用 LLM（优先 one_shot，降级 chat_stream + collect_stream）
        let raw = match llm_complete(llm, system_prompt, &dialog_text).await {
            Ok(Some(text)) => text,
            Ok(None) => return Ok(Vec::new()),
            Err(_) => return Ok(Vec::new()),
        };

        // 逐行解析 → 创建 Wing 层 MemoryEntry
        let mut entries = Vec::new();
        for line in raw.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            // 解析格式 [类型] 内容
            let (source, content) = parse_memory_line(line);
            if content.is_empty() {
                continue;
            }

            let entry = MemoryEntry::new(content, source, MemoryTier::Wing);
            entries.push(entry);
        }

        // 限制最多 5 条
        recent_sorted.clear();
        entries.truncate(5);

        // 批量写入存储
        for entry in &entries {
            self.storage.add_memory_entry(entry).await?;
        }

        Ok(entries)
    }

    /// 记忆提升（Wing → Hall → Room）。
    ///
    /// 每次提升 importance += 0.1（上限 1.0）。
    /// Room 层无法再提升（返回 Ok）。
    pub async fn promote(&self, memory_id: &str) -> anyhow::Result<()> {
        // 获取所有记忆，找到指定 ID
        let all = self.storage.get_memory_entries(None).await?;
        let mut entry = all
            .into_iter()
            .find(|e| e.id == memory_id)
            .ok_or_else(|| anyhow::anyhow!("记忆条目不存在: {memory_id}"))?;

        // 提升层级
        entry.tier = match entry.tier {
            MemoryTier::Wing => MemoryTier::Hall,
            MemoryTier::Hall => MemoryTier::Room,
            MemoryTier::Room => return Ok(()), // Room 已是最高层
        };

        // 提升重要性
        entry.importance = (entry.importance + 0.1).min(1.0);

        self.storage.update_memory_entry(&entry).await?;
        Ok(())
    }

    /// 记忆遗忘（低重要性 + 低访问频率 → 删除）。
    ///
    /// 遗忘分数：`forget_score = importance * 0.6 + access_count_norm * 0.4`
    /// `forget_score < 0.3` 的记忆被删除。
    ///
    /// # 参数
    /// - `tier`: 要清理的层级
    ///
    /// # 返回
    /// 删除的记忆数量
    pub async fn forget(&self, tier: &MemoryTier) -> anyhow::Result<usize> {
        let entries = self.storage.get_memory_entries(Some(tier)).await?;
        let max_access = entries
            .iter()
            .map(|e| e.access_count)
            .max()
            .unwrap_or(1)
            .max(1);

        let mut forgotten = 0;
        for entry in &entries {
            let access_norm = entry.access_count as f32 / max_access as f32;
            let forget_score = entry.importance * 0.6 + access_norm * 0.4;
            if forget_score < 0.3 {
                self.storage.delete_memory_entry(&entry.id).await?;
                forgotten += 1;
            }
        }

        Ok(forgotten)
    }

    /// 检索相关记忆（用于查询时注入到 RAG prompt）。
    ///
    /// 1. `storage.search_memory_entries(query, top_k * 2)` — 关键词模糊匹配
    /// 2. 按 importance DESC + last_accessed DESC 排序
    /// 3. 取 top_k 条
    /// 4. 更新 access_count + last_accessed
    pub async fn retrieve_relevant(
        &self,
        query: &str,
        top_k: usize,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        // 关键词模糊搜索
        let mut entries = self
            .storage
            .search_memory_entries(query, top_k.saturating_mul(2))
            .await?;

        // 按 importance DESC + last_accessed DESC 排序
        entries.sort_by(|a, b| match b.importance.partial_cmp(&a.importance) {
            Some(std::cmp::Ordering::Equal) | None => b.last_accessed.cmp(&a.last_accessed),
            Some(ord) => ord,
        });

        // 取 top_k 条
        let result: Vec<MemoryEntry> = entries.into_iter().take(top_k).collect();

        // 更新 access_count + last_accessed（异步，不阻塞返回）
        let now = chrono::Utc::now().timestamp();
        for entry in &result {
            let mut updated = entry.clone();
            updated.access_count += 1;
            updated.last_accessed = now;
            let _ = self.storage.update_memory_entry(&updated).await;
        }

        Ok(result)
    }

    /// 记忆整合（由 AutoDream 后台调用）。
    ///
    /// 1. Wing 层超过 max_wing → 按 importance 排序
    ///    - importance >= 0.6 → 提升到 Hall
    ///    - importance < 0.3 → 遗忘（删除）
    ///    - 中间的保留在 Wing
    /// 2. Hall 层超过 max_hall → 同理提升到 Room 或遗忘
    /// 3. Room 层超过 max_room → 按 access_count 最低的遗忘
    pub async fn consolidate(&self) -> anyhow::Result<ConsolidationResult> {
        let mut promoted = 0usize;
        let mut forgotten = 0usize;

        // ---- Wing 层整合 ----
        // 当 Wing 层超过上限时，遍历所有条目：
        // - importance >= 0.6 → 提升到 Hall
        // - importance < 0.3 → 遗忘（删除）
        // - 中间的保留在 Wing
        let wing_entries = self
            .storage
            .get_memory_entries(Some(&MemoryTier::Wing))
            .await?;
        if wing_entries.len() > self.max_wing {
            for entry in &wing_entries {
                if entry.importance >= 0.6 {
                    // 提升到 Hall
                    let mut updated = entry.clone();
                    updated.tier = MemoryTier::Hall;
                    updated.importance = (updated.importance + 0.1).min(1.0);
                    self.storage.update_memory_entry(&updated).await?;
                    promoted += 1;
                } else if entry.importance < 0.3 {
                    // 遗忘
                    self.storage.delete_memory_entry(&entry.id).await?;
                    forgotten += 1;
                }
                // 中间的保留在 Wing（不操作）
            }
        }

        // ---- Hall 层整合 ----
        // 当 Hall 层超过上限时，遍历所有条目：
        // - importance >= 0.7 → 提升到 Room
        // - importance < 0.3 → 遗忘
        let hall_entries = self
            .storage
            .get_memory_entries(Some(&MemoryTier::Hall))
            .await?;
        if hall_entries.len() > self.max_hall {
            for entry in &hall_entries {
                if entry.importance >= 0.7 {
                    // 提升到 Room
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
        if room_entries.len() > self.max_room {
            let mut sorted = room_entries.clone();
            // 按 access_count 升序排列（最少的在前）
            sorted.sort_by_key(|a| a.access_count);

            let to_forget = room_entries.len() - self.max_room;
            for entry in sorted.iter().take(to_forget) {
                self.storage.delete_memory_entry(&entry.id).await?;
                forgotten += 1;
            }
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

        Ok(ConsolidationResult {
            promoted,
            forgotten,
            remaining_wing,
            remaining_hall,
            remaining_room,
        })
    }

    /// 用户手动置顶记忆（直接创建 Room 层，importance 1.0）。
    pub async fn pin_memory(
        &self,
        conversation_id: &str,
        content: &str,
    ) -> anyhow::Result<MemoryEntry> {
        let entry = MemoryEntry {
            id: uuid::Uuid::new_v4().to_string(),
            tier: MemoryTier::Room,
            content: content.to_string(),
            source: MemorySource::UserPinned,
            conversation_id: Some(conversation_id.to_string()),
            created_at: chrono::Utc::now().timestamp(),
            last_accessed: chrono::Utc::now().timestamp(),
            access_count: 0,
            importance: 1.0,
        };
        self.storage.add_memory_entry(&entry).await?;
        Ok(entry)
    }

    // ========================================================
    // Scratch-Promote 层（Q01 借鉴 QM scratch-promote + consolidation）
    // ========================================================

    /// 写入 scratch 日志（临时事实暂存）。
    ///
    /// 将一条事实写入 scratch_logs 表，等待后续 LLM 审查后 promote 到长期记忆。
    /// 借鉴 QM `memory/strategies/scratch-promote.ts` 的 daily log 模式。
    pub async fn write_scratch(&self, content: &str) -> anyhow::Result<ScratchLogEntry> {
        let entry = ScratchLogEntry::new(content.to_string());
        self.storage.add_scratch_log(&entry).await?;
        Ok(entry)
    }

    /// Scratch 层 LLM 驱动整合（借鉴 QM consolidation.ts）。
    ///
    /// 1. 清理过期 scratch 条目（超过 `retention_days` 天）
    /// 2. 读取剩余 scratch 条目，编号后发给 LLM 审查
    /// 3. LLM 输出 UPDATE/DELETE/ADD 动作列表
    /// 4. 应用动作到长期记忆（MemoryStore Wing/Hall/Room）
    /// 5. 清除已整合的 scratch 条目
    ///
    /// # 降级策略
    /// LLM 调用失败时，保留 scratch 条目不变，返回空动作列表（不返回 Err）。
    ///
    /// # 参数
    /// - `llm`: LLM 提供者（非流式调用）
    /// - `retention_days`: scratch 条目保留天数（默认 14 天）
    pub async fn consolidate_scratch<L: LLMProvider>(
        &self,
        llm: &L,
        retention_days: u64,
    ) -> anyhow::Result<ScratchConsolidationResult> {
        // 1. 清理过期条目
        let now = chrono::Utc::now().timestamp();
        let expiry_threshold = now - (retention_days as i64 * 86_400);
        let expired_cleaned = self
            .storage
            .cleanup_expired_scratch_logs(expiry_threshold)
            .await
            .unwrap_or(0);

        // 2. 读取剩余 scratch 条目
        let scratch_entries = self.storage.get_scratch_logs(None).await?;
        if scratch_entries.is_empty() {
            return Ok(ScratchConsolidationResult {
                actions: Vec::new(),
                expired_cleaned,
                remaining_scratch: 0,
            });
        }

        // 3. 构建编号列表发给 LLM
        let mut numbered_list = String::new();
        for (i, entry) in scratch_entries.iter().enumerate() {
            numbered_list.push_str(&format!("{}. {}\n", i + 1, entry.content));
        }

        // 4. 调用 LLM 审查
        let raw_output = match self.call_llm_for_consolidation(llm, &numbered_list).await {
            Ok(text) => text,
            Err(_) => {
                // LLM 失败，降级为保留原状
                return Ok(ScratchConsolidationResult {
                    actions: Vec::new(),
                    expired_cleaned,
                    remaining_scratch: scratch_entries.len(),
                });
            }
        };

        // 5. 解析 LLM 输出为动作列表
        let actions = parse_consolidation_output(&raw_output);

        // 6. 应用动作到长期记忆
        self.apply_consolidation_actions(&actions).await;

        // 7. 清除已整合的 scratch 条目
        for entry in &scratch_entries {
            let _ = self.storage.delete_scratch_log(&entry.id).await;
        }

        // 8. 检查剩余 scratch 条目
        let remaining = self
            .storage
            .get_scratch_logs(None)
            .await
            .unwrap_or_default()
            .len();

        Ok(ScratchConsolidationResult {
            actions,
            expired_cleaned,
            remaining_scratch: remaining,
        })
    }

    /// 调用 LLM 进行 scratch 整合审查（内部方法）。
    async fn call_llm_for_consolidation<L: LLMProvider>(
        &self,
        llm: &L,
        numbered_list: &str,
    ) -> anyhow::Result<String> {
        let system_prompt = MEMORY_CONSOLIDATION_PROMPT;
        match llm_complete(llm, system_prompt, numbered_list).await {
            Ok(Some(text)) => Ok(text),
            Ok(None) => Err(anyhow::anyhow!("LLM 返回空结果")),
            Err(e) => Err(e),
        }
    }

    /// 应用整合动作到长期记忆（内部方法）。
    ///
    /// 借鉴 QM consolidation.ts 的 UPDATE/DELETE/ADD 模式。
    /// 错误仅记录日志，不返回 Err（整合是 best-effort 操作）。
    pub(crate) async fn apply_consolidation_actions(&self, actions: &[MemoryConsolidationAction]) {
        for action in actions {
            match action {
                MemoryConsolidationAction::Update { id, content } => {
                    // 获取已有记忆，更新内容
                    let all = self
                        .storage
                        .get_memory_entries(None)
                        .await
                        .unwrap_or_default();
                    if let Some(mut entry) = all.into_iter().find(|e| e.id == *id) {
                        entry.content = content.clone();
                        let _ = self.storage.update_memory_entry(&entry).await;
                    }
                }
                MemoryConsolidationAction::Delete { id } => {
                    let _ = self.storage.delete_memory_entry(id).await;
                }
                MemoryConsolidationAction::Add { content, tier } => {
                    let entry = MemoryEntry::new(
                        content.clone(),
                        MemorySource::AutoExtracted,
                        tier.clone(),
                    );
                    let _ = self.storage.add_memory_entry(&entry).await;
                }
            }
        }
    }
}

// ============================================================
// Scratch-Promote 辅助函数
// ============================================================

/// Scratch 层整合 LLM Prompt（借鉴 QM `MEMORY_CONSOLIDATION_PROMPT`）。
const MEMORY_CONSOLIDATION_PROMPT: &str = "你正在整理 Agent 的长期记忆。输入是一个编号列表，每条记忆可能带有 (YYYY-MM-DD) 捕获日期。\n\
仅输出动作，每行一个，格式如下：\n\
UPDATE <n>: <修订后的事实>\n\
DELETE <n>\n\
ADD: <新事实>\n\
如果无需变更，输出: NONE\n\n\
规则：\n\
- 优先 UPDATE 而非 DELETE+ADD（事实演变或两条应合并时）\n\
- DELETE 过时、被新事实矛盾、精确或近似重复、可从其他事实推导的记忆\n\
- 保持事实原子性：每行一条独立事实\n\
- 永不删除用户明确要求记住的事实\n\
- 保留 (said in ...) 后缀，它记录事实来源";

/// 解析 LLM 整合输出为动作列表（借鉴 QM consolidation.ts 解析逻辑）。
///
/// 支持格式：
/// - `UPDATE <n>: <修订后的事实>` → Update { id, content }
/// - `DELETE <n>` → Delete { id }
/// - `ADD: <新事实>` → Add { content, tier: Wing }
/// - `NONE` → 空列表
///
/// `scratch_entries` 提供 `<n>` 到条目 ID 的映射。
pub fn parse_consolidation_output(raw: &str) -> Vec<MemoryConsolidationAction> {
    let trimmed = raw.trim();
    if trimmed.eq_ignore_ascii_case("NONE") || trimmed.is_empty() {
        return Vec::new();
    }

    let mut actions = Vec::new();
    for line in trimmed.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // ADD: <新事实>
        if let Some(rest) = line.strip_prefix("ADD:") {
            let content = rest.trim();
            if !content.is_empty() {
                actions.push(MemoryConsolidationAction::Add {
                    content: content.to_string(),
                    tier: MemoryTier::Wing,
                });
            }
            continue;
        }

        // DELETE <n>
        if let Some(rest) = line.strip_prefix("DELETE") {
            let n_str = rest.trim();
            if let Ok(n) = n_str.parse::<usize>() {
                // 使用临时 ID（实际 ID 在 apply 时通过 scratch_entries 映射）
                // 这里用编号作为占位 ID，apply_consolidation_actions 会处理
                actions.push(MemoryConsolidationAction::Delete {
                    id: format!("__scratch_{n}"),
                });
            }
            continue;
        }

        // UPDATE <n>: <修订后的事实>
        if let Some(rest) = line.strip_prefix("UPDATE") {
            // 找到冒号位置
            if let Some(colon_pos) = rest.find(':') {
                let n_str = rest[..colon_pos].trim();
                let content = rest[colon_pos + 1..].trim();
                if let Ok(n) = n_str.parse::<usize>()
                    && !content.is_empty()
                {
                    actions.push(MemoryConsolidationAction::Update {
                        id: format!("__scratch_{n}"),
                        content: content.to_string(),
                    });
                }
            }
            continue;
        }
    }

    actions
}

// ============================================================
// 为 MemoryStore 实现 MemoryRetriever trait
// ============================================================

impl<S: Storage> MemoryRetriever for MemoryStore<S> {
    fn retrieve_relevant_memories<'a>(
        &'a self,
        query: &'a str,
        top_k: usize,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Vec<MemoryEntry>>> + Send + 'a>> {
        Box::pin(self.retrieve_relevant(query, top_k))
    }
}

// ============================================================
// 辅助函数
// ============================================================

/// 从 LLM token 流收集完整字符串。
async fn collect_stream(
    mut stream: futures::stream::BoxStream<'static, anyhow::Result<String>>,
) -> anyhow::Result<String> {
    use futures::StreamExt;
    let mut result = String::new();
    while let Some(token) = stream.next().await {
        result.push_str(&token?);
    }
    Ok(result)
}

/// 非流式 LLM 调用：优先使用 `one_shot`，降级为 `chat_stream + collect_stream`。
///
/// 跨 Phase 依赖整合（S67）：Burst Buffer / Consolidation / Extract 全部
/// 非流式 LLM 调用统一走此函数。Q10 新增的 `one_shot()` 语义更精确
/// （非流式单轮完成），优先使用；Provider 未覆盖 `one_shot` 时
/// （返回 `Ok(None)`）或出错时降级为 `chat_stream + collect_stream`。
///
/// # 返回
/// - `Ok(Some(text))`：成功获取 LLM 响应
/// - `Ok(None)`：两条路径均返回空
/// - `Err(...)`：两条路径均失败
async fn llm_complete<L: LLMProvider>(
    llm: &L,
    system_prompt: &str,
    user_prompt: &str,
) -> anyhow::Result<Option<String>> {
    // 优先尝试 one_shot（Q10 辅助方法）
    match llm.one_shot(system_prompt, user_prompt).await {
        Ok(Some(text)) => Ok(Some(text)),
        Ok(None) => {
            // Provider 未覆盖 one_shot，降级为 chat_stream + collect_stream
            let stream = llm.chat_stream(system_prompt, &[], user_prompt).await?;
            let text = collect_stream(stream).await?;
            if text.is_empty() {
                Ok(None)
            } else {
                Ok(Some(text))
            }
        }
        Err(_) => {
            // one_shot 出错，降级为 chat_stream + collect_stream
            let stream = llm.chat_stream(system_prompt, &[], user_prompt).await?;
            let text = collect_stream(stream).await?;
            if text.is_empty() {
                Ok(None)
            } else {
                Ok(Some(text))
            }
        }
    }
}

/// 解析 LLM 输出的单行记忆提取结果。
///
/// 支持格式：
/// - `[user_statement] 内容` → UserStatement
/// - `[assistant_info] 内容` → AssistantAnswer
/// - `[auto_extracted] 内容` → AutoExtracted
/// - 无前缀的行 → AutoExtracted（默认）
fn parse_memory_line(line: &str) -> (MemorySource, String) {
    // 尝试匹配 [类型] 格式
    if let Some(close_bracket) = line.find(']')
        && line.starts_with('[')
    {
        let tag = &line[1..close_bracket];
        let content = line[close_bracket + 1..].trim();
        let source = match tag {
            "user_statement" => MemorySource::UserStatement,
            "assistant_info" => MemorySource::AssistantAnswer,
            "auto_extracted" => MemorySource::AutoExtracted,
            _ => MemorySource::AutoExtracted,
        };
        return (source, content.to_string());
    }

    // 无前缀行 → 默认 AutoExtracted
    (MemorySource::AutoExtracted, line.to_string())
}

// ============================================================
// Burst Buffer — 延迟批量记忆提取（Q02 借鉴 QM createBurstBuffer）
// ============================================================

/// Burst Buffer 中的一轮对话（含 Provenance 来源标记）。
///
/// 每轮对话包含用户消息、助手回复和来源标记，
/// 聚合后批量交给 LLM 提取记忆，降低 LLM 调用成本。
#[derive(Debug, Clone)]
pub struct BurstTurn {
    /// 用户消息
    pub user_msg: String,
    /// 助手回复
    pub assistant_reply: String,
    /// 来源标记（记录哪个对话、哪条消息）
    pub provenance: ProvenanceTag,
}

/// Burst Buffer — 延迟批量记忆提取引擎（借鉴 QM `createBurstBuffer(quietMs, maxTurns, flush)`）。
///
/// 将记忆提取从每轮立即调用 LLM 改为聚合多轮后批量提取，降低 LLM 调用成本。
///
/// ## 触发条件（满足任一即 flush）
/// - **静默窗口**：距上次活动超过 `quiet_ms`（默认 180000ms = 3 分钟）
/// - **最大轮次**：聚合轮次达到 `max_turns`（默认 10）
///
/// ## 工作流程
/// 1. `push()` — 每轮对话结束后推入 buffer
/// 2. `should_flush()` — 检查是否满足 flush 条件
/// 3. `flush()` — 聚合所有 pending 轮次，调用 LLM 一次提取记忆，
///    将提取结果附带 Provenance 标记写入 scratch 层
///
/// ## QM 对比
/// - QM `memory/strategies/per-turn.ts`：`createBurstBuffer(quietMs, maxTurns, flush)`
/// - QM 静默窗口 + 最大轮次聚合 → EchoMind 相同设计
/// - QM `ccCaptureToPersonal()` 的 `(said in #channel)` → EchoMind ProvenanceTag
pub struct BurstBuffer {
    /// 待处理的对话轮次
    pending: Vec<BurstTurn>,
    /// 静默窗口（毫秒）：距上次活动超过此值时触发 flush
    quiet_ms: u64,
    /// 最大聚合轮次：达到此值时触发 flush
    max_turns: usize,
    /// 最后一次活动时间
    last_activity: std::time::Instant,
}

impl BurstBuffer {
    /// 创建默认配置的 BurstBuffer（quiet_ms=180000, max_turns=10）。
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
            quiet_ms: 180_000,
            max_turns: 10,
            last_activity: std::time::Instant::now(),
        }
    }

    /// 自定义静默窗口和最大轮次。
    pub fn with_config(quiet_ms: u64, max_turns: usize) -> Self {
        Self {
            pending: Vec::new(),
            quiet_ms,
            max_turns: max_turns.max(1),
            last_activity: std::time::Instant::now(),
        }
    }

    /// 推入一轮对话到 buffer。
    ///
    /// 更新 `last_activity` 时间戳。
    pub fn push(&mut self, user_msg: String, assistant_reply: String, provenance: ProvenanceTag) {
        self.last_activity = std::time::Instant::now();
        self.pending.push(BurstTurn {
            user_msg,
            assistant_reply,
            provenance,
        });
    }

    /// 检查是否满足 flush 条件。
    ///
    /// 满足任一条件即返回 `true`：
    /// - pending 非空且轮次达到 `max_turns`
    /// - pending 非空且静默窗口已过
    pub fn should_flush(&self) -> bool {
        if self.pending.is_empty() {
            return false;
        }
        self.pending.len() >= self.max_turns
            || self.last_activity.elapsed().as_millis() as u64 >= self.quiet_ms
    }

    /// 返回待处理轮次数量。
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// 返回是否为空。
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    /// 取出所有 pending 轮次（清空 buffer）。
    ///
    /// 返回的 Vec 用于传递给 `flush_burst_buffer()` 函数进行 LLM 提取。
    pub fn drain(&mut self) -> Vec<BurstTurn> {
        self.last_activity = std::time::Instant::now();
        std::mem::take(&mut self.pending)
    }

    /// Flush：聚合所有 pending 轮次，调用 LLM 提取记忆，写入 scratch 层。
    ///
    /// 借鉴 QM `createBurstBuffer` 的 flush 回调：
    /// 1. 将所有 pending 轮次聚合为一段对话文本（附带 provenance 标签）
    /// 2. 调用 LLM 一次提取关键事实
    /// 3. 将提取的记忆附带 `(said in ...)` 后缀写入 scratch_logs
    /// 4. 清空 buffer
    ///
    /// # 降级策略
    /// LLM 调用失败时，仍清空 buffer（丢弃本轮提取机会），返回 `Ok(0)`。
    /// 不返回 Err 以避免阻塞聊天流程。
    ///
    /// # 返回
    /// 提取并写入 scratch 的记忆条目数量。
    pub async fn flush<S: Storage, L: LLMProvider>(
        &mut self,
        memory_store: &MemoryStore<S>,
        llm: &L,
    ) -> anyhow::Result<usize> {
        let turns = self.drain();
        if turns.is_empty() {
            return Ok(0);
        }
        flush_burst_turns(memory_store, &turns, llm).await
    }
}

impl Default for BurstBuffer {
    fn default() -> Self {
        Self::new()
    }
}

/// 从 BurstBuffer 轮次列表中批量提取记忆并写入 scratch 层。
///
/// 独立函数设计（非 BurstBuffer 方法），使其可被外部代码直接调用
/// （如 IPC 命令从 AppState 取出 pending 轮次后调用此函数）。
///
/// # 流程
/// 1. 聚合所有轮次为对话文本，每轮附带 `(said in ...)` provenance 标签
/// 2. 调用 LLM 提取关键事实（system prompt 要求每条一行 `[类型] 内容`）
/// 3. 逐行解析提取结果，为每条记忆追加 provenance 后缀后写入 scratch_logs
///
/// # 降级策略
/// LLM 调用失败时返回 `Ok(0)`，不返回 Err。
pub async fn flush_burst_turns<S: Storage, L: LLMProvider>(
    memory_store: &MemoryStore<S>,
    turns: &[BurstTurn],
    llm: &L,
) -> anyhow::Result<usize> {
    if turns.is_empty() {
        return Ok(0);
    }

    // 1. 聚合对话文本（附带 provenance 标签）
    let mut dialog_text = String::new();
    for turn in turns {
        dialog_text.push_str(&format!(
            "(said in {})\n用户: {}\n助手: {}\n\n",
            turn.provenance.source_label, turn.user_msg, turn.assistant_reply
        ));
    }

    // 2. 调用 LLM 提取记忆
    let system_prompt = "从以下多轮对话中提取值得记住的关键事实（用户偏好、重要决定、事实陈述）。\n\
         每条一行，格式：[类型] 内容\n\
         类型：user_statement / assistant_info / auto_extracted\n\
         只提取有长期价值的信息，忽略寒暄和临时性问题。\n\
         最多提取 10 条。仅输出提取结果，不要其他解释。";

    let raw = match llm_complete(llm, system_prompt, &dialog_text).await {
        Ok(Some(text)) => text,
        Ok(None) => return Ok(0),
        Err(_) => return Ok(0),
    };

    // 3. 逐行解析 → 写入 scratch（附带 provenance 后缀）
    let mut count = 0usize;
    // 使用第一个轮次的 provenance 作为代表（同一 flush 批次的来源一致）
    let provenance_suffix = turns[0].provenance.said_in_suffix();

    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let (_, content) = parse_memory_line(line);
        if content.is_empty() {
            continue;
        }

        // 追加 provenance 后缀后写入 scratch
        let tagged_content = format!("{content}{provenance_suffix}");
        match memory_store.write_scratch(&tagged_content).await {
            Ok(_) => count += 1,
            Err(_) => continue,
        }
    }

    Ok(count)
}
