//! 内置 ContextSource 实现
//!
//! 5 个内置 Source：
//! 1. RoleSource — 角色描述（静态不变）
//! 2. KnowledgeBaseSource — 知识库概况（文档数/领域分布）
//! 3. ConversationHistorySource — 对话历史（消息数/轮次）
//! 4. RetrievalMemorySource — 检索记忆（记忆条目数）
//! 5. AgentModeSource — Agent 模式（启用/禁用状态）
//!
//! 设计：Source 接受预加载的数据（由 chat_inner 异步查询后传入），
//! load() 仅做同步格式化，保证 ContextSource trait 的对象安全。

use crate::{ContextSource, SourceKey, SourceValue};

// ============================================================
// RoleSource — 角色描述（静态不变）
// ============================================================

/// 角色描述源 — 静态不变，不产生更新
pub struct RoleSource {
    key: SourceKey,
    role_text: String,
}

impl RoleSource {
    pub fn new(role_text: String) -> Self {
        Self {
            key: "system/role".to_string(),
            role_text,
        }
    }
}

impl ContextSource for RoleSource {
    fn key(&self) -> &SourceKey {
        &self.key
    }

    fn load(&self) -> anyhow::Result<SourceValue> {
        Ok(SourceValue::Text(self.role_text.clone()))
    }

    fn baseline(&self, value: &SourceValue) -> String {
        match value {
            SourceValue::Text(t) => t.clone(),
            _ => String::new(),
        }
    }

    fn update(&self, _previous: &SourceValue, _current: &SourceValue) -> String {
        String::new() // 角色不变，不产生更新
    }
}

// ============================================================
// KnowledgeBaseSource — 知识库概况
// ============================================================

/// 知识库概况源 — 导入新文档时变化
pub struct KnowledgeBaseSource {
    key: SourceKey,
    doc_count: usize,
    domains: Vec<(String, usize)>,
}

impl KnowledgeBaseSource {
    /// 创建知识库概况源
    ///
    /// 参数由 chat_inner 异步查询后传入：
    /// - `doc_count`: 文档总数
    /// - `domains`: 领域分布 [(domain, count), ...]
    pub fn new(doc_count: usize, domains: Vec<(String, usize)>) -> Self {
        Self {
            key: "kb/summary".to_string(),
            doc_count,
            domains,
        }
    }
}

impl ContextSource for KnowledgeBaseSource {
    fn key(&self) -> &SourceKey {
        &self.key
    }

    fn load(&self) -> anyhow::Result<SourceValue> {
        Ok(SourceValue::Json(serde_json::json!({
            "doc_count": self.doc_count,
            "domains": self.domains.iter()
                .map(|(d, c)| serde_json::json!({"domain": d, "count": c}))
                .collect::<Vec<_>>(),
        })))
    }

    fn baseline(&self, value: &SourceValue) -> String {
        match value {
            SourceValue::Json(v) => {
                let count = v.get("doc_count").and_then(|c| c.as_u64()).unwrap_or(0);
                format!("当前知识库包含 {} 篇文档。", count)
            }
            _ => String::new(),
        }
    }

    fn update(&self, previous: &SourceValue, current: &SourceValue) -> String {
        let prev_count = previous.as_doc_count();
        let curr_count = current.as_doc_count();
        if prev_count == curr_count {
            String::new()
        } else if curr_count > prev_count {
            format!(
                "[知识库更新] 文档数量从 {} 增至 {}（新增 {} 篇）",
                prev_count,
                curr_count,
                curr_count - prev_count
            )
        } else {
            format!(
                "[知识库更新] 文档数量从 {} 减至 {}（删除 {} 篇）",
                prev_count,
                curr_count,
                prev_count - curr_count
            )
        }
    }
}

// ============================================================
// ConversationHistorySource — 对话历史
// ============================================================

/// 对话历史源 — 每轮对话变化
pub struct ConversationHistorySource {
    key: SourceKey,
    message_count: usize,
}

impl ConversationHistorySource {
    pub fn new(message_count: usize) -> Self {
        Self {
            key: "conv/history".to_string(),
            message_count,
        }
    }
}

impl ContextSource for ConversationHistorySource {
    fn key(&self) -> &SourceKey {
        &self.key
    }

    fn load(&self) -> anyhow::Result<SourceValue> {
        Ok(SourceValue::Json(serde_json::json!({
            "message_count": self.message_count,
        })))
    }

    fn baseline(&self, value: &SourceValue) -> String {
        match value {
            SourceValue::Json(v) => {
                let count = v.get("message_count").and_then(|c| c.as_u64()).unwrap_or(0);
                format!("当前对话已进行 {} 轮交互。", count / 2)
            }
            _ => String::new(),
        }
    }

    fn update(&self, previous: &SourceValue, current: &SourceValue) -> String {
        let prev = previous.as_message_count();
        let curr = current.as_message_count();
        if curr > prev {
            format!("[对话进展] 新增 {} 条消息", curr - prev)
        } else {
            String::new()
        }
    }
}

// ============================================================
// RetrievalMemorySource — 检索记忆
// ============================================================

/// 检索记忆源 — 记忆更新时变化
pub struct RetrievalMemorySource {
    key: SourceKey,
    memory_count: usize,
}

impl RetrievalMemorySource {
    pub fn new(memory_count: usize) -> Self {
        Self {
            key: "mem/retrieval".to_string(),
            memory_count,
        }
    }
}

impl ContextSource for RetrievalMemorySource {
    fn key(&self) -> &SourceKey {
        &self.key
    }

    fn load(&self) -> anyhow::Result<SourceValue> {
        Ok(SourceValue::Json(serde_json::json!({
            "memory_count": self.memory_count,
        })))
    }

    fn baseline(&self, value: &SourceValue) -> String {
        match value {
            SourceValue::Json(v) => {
                let count = v.get("memory_count").and_then(|c| c.as_u64()).unwrap_or(0);
                if count > 0 {
                    format!("检索记忆系统已积累 {} 条记忆。", count)
                } else {
                    String::new()
                }
            }
            _ => String::new(),
        }
    }

    fn update(&self, previous: &SourceValue, current: &SourceValue) -> String {
        let prev = previous.as_memory_count();
        let curr = current.as_memory_count();
        if curr > prev {
            format!("[记忆更新] 新增 {} 条检索记忆", curr - prev)
        } else {
            String::new()
        }
    }
}

// ============================================================
// AgentModeSource — Agent 模式
// ============================================================

/// Agent 模式源 — 用户切换 Agent 时变化
pub struct AgentModeSource {
    key: SourceKey,
    is_agent_enabled: bool,
}

impl AgentModeSource {
    pub fn new(is_agent_enabled: bool) -> Self {
        Self {
            key: "system/agent".to_string(),
            is_agent_enabled,
        }
    }
}

impl ContextSource for AgentModeSource {
    fn key(&self) -> &SourceKey {
        &self.key
    }

    fn load(&self) -> anyhow::Result<SourceValue> {
        Ok(SourceValue::Json(serde_json::json!({
            "agent_enabled": self.is_agent_enabled,
        })))
    }

    fn baseline(&self, value: &SourceValue) -> String {
        match value {
            SourceValue::Json(v) => {
                let enabled = v
                    .get("agent_enabled")
                    .and_then(|e| e.as_bool())
                    .unwrap_or(false);
                if enabled {
                    "Agent 模式已启用：系统将使用 ReAct 多步推理。".to_string()
                } else {
                    String::new()
                }
            }
            _ => String::new(),
        }
    }

    fn update(&self, previous: &SourceValue, current: &SourceValue) -> String {
        let prev = previous.as_agent_enabled();
        let curr = current.as_agent_enabled();
        if prev == curr {
            String::new()
        } else if curr {
            "[模式切换] Agent 模式已启用。".to_string()
        } else {
            "[模式切换] Agent 模式已禁用。".to_string()
        }
    }
}

// ============================================================
// SourceValue 辅助方法
// ============================================================

impl SourceValue {
    /// 从 SourceValue 提取文档计数
    fn as_doc_count(&self) -> usize {
        match self {
            SourceValue::Json(v) => v
                .get("doc_count")
                .and_then(|c| c.as_u64())
                .map(|c| c as usize)
                .unwrap_or(0),
            _ => 0,
        }
    }

    /// 从 SourceValue 提取消息计数
    fn as_message_count(&self) -> usize {
        match self {
            SourceValue::Json(v) => v
                .get("message_count")
                .and_then(|c| c.as_u64())
                .map(|c| c as usize)
                .unwrap_or(0),
            _ => 0,
        }
    }

    /// 从 SourceValue 提取记忆计数
    fn as_memory_count(&self) -> usize {
        match self {
            SourceValue::Json(v) => v
                .get("memory_count")
                .and_then(|c| c.as_u64())
                .map(|c| c as usize)
                .unwrap_or(0),
            _ => 0,
        }
    }

    /// 从 SourceValue 提取 Agent 模式状态
    fn as_agent_enabled(&self) -> bool {
        match self {
            SourceValue::Json(v) => v
                .get("agent_enabled")
                .and_then(|e| e.as_bool())
                .unwrap_or(false),
            _ => false,
        }
    }
}
