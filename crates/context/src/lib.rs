//! # System Context Registry（系统上下文注册表）
//!
//! 借鉴 OpenCode v1.18.15 的 System Context 架构，将系统提示拆分为多个独立的、
//! 类型化的 Context Source。每个 Source 有自己的 `load()` / `baseline()` / `update()`
//! / `removed()` 方法，在 Provider-Turn Boundary 处懒加载比较，只生成差异消息。
//!
//! ## 核心概念
//!
//! - **ContextSource**: 一个独立的上下文源（如角色描述、知识库概况、对话历史）
//! - **SourceValue**: Source 的当前值（Text / Json / Unavailable）
//! - **ContextSnapshot**: 持久化的比较状态（key → value 映射）
//! - **SystemContext**: 多个 Source 的组合，提供 initialize / reconcile / replace
//! - **ReconcileResult**: 协调结果（Unchanged / Updated / ReplacementReady / ReplacementBlocked）
//!
//! ## 依赖方向
//!
//! `crates/context → crates/core → crates/models`
//! 严禁反向依赖。

use std::collections::HashMap;

use anyhow::Result;
use serde::{Deserialize, Serialize};

// ============================================================
// 类型定义
// ============================================================

/// 稳定的命名空间标识（如 "system/role"、"kb/summary"、"conv/history"）
pub type SourceKey = String;

/// 一个独立的上下文源的值（JSON 可序列化）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SourceValue {
    /// 纯文本值
    Text(String),
    /// JSON 结构化值
    Json(serde_json::Value),
    /// 临时不可用（如数据库锁定、网络故障）
    Unavailable,
}

/// 上下文快照：持久化的比较状态
///
/// 存储每个 Source 的当前值和可选的移除消息。
/// 用于跨轮次比较上下文变化，决定是否需要发送更新消息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSnapshot {
    /// key → (value, 可选的移除消息)
    pub sources: HashMap<SourceKey, (SourceValue, Option<String>)>,
}

impl ContextSnapshot {
    /// 创建空快照
    pub fn new() -> Self {
        Self {
            sources: HashMap::new(),
        }
    }

    /// 获取指定 Source 的值
    pub fn get(&self, key: &str) -> Option<&SourceValue> {
        self.sources.get(key).map(|(v, _)| v)
    }

    /// 设置指定 Source 的值
    pub fn set(&mut self, key: SourceKey, value: SourceValue) {
        self.sources.insert(key, (value, None));
    }

    /// 设置指定 Source 的值和移除消息
    pub fn set_with_removed(
        &mut self,
        key: SourceKey,
        value: SourceValue,
        removed: Option<String>,
    ) {
        self.sources.insert(key, (value, removed));
    }

    /// 移除指定 Source
    pub fn remove(&mut self, key: &str) -> Option<(SourceValue, Option<String>)> {
        self.sources.remove(key)
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }

    /// Source 数量
    pub fn len(&self) -> usize {
        self.sources.len()
    }
}

impl Default for ContextSnapshot {
    fn default() -> Self {
        Self::new()
    }
}

/// 三种协调结果
#[derive(Debug)]
pub enum ReconcileResult {
    /// 无变化 — 复用基线，仅追加检索片段
    Unchanged,
    /// 有更新 — 生成 Mid-Conversation System Message
    Updated {
        /// 差异消息文本
        text: String,
        /// 更新后的快照
        snapshot: ContextSnapshot,
    },
    /// 需要替换基线 — 压缩后或上下文不可用后恢复
    ReplacementReady {
        /// 新基线文本
        baseline: String,
        /// 更新后的快照
        snapshot: ContextSnapshot,
    },
    /// 替换被阻塞 — 上下文不可用，保留旧值
    ReplacementBlocked,
}

// ============================================================
// ContextSource trait
// ============================================================

/// 一个独立的上下文源：观察当前值 → 比较快照 → 渲染基线/更新/移除文本
///
/// 实现者需要提供：
/// - `key()`: 稳定标识（如 "system/role"）
/// - `load()`: 异步观察当前值
/// - `baseline()`: 首次渲染完整基线文本
/// - `update()`: 变更差异渲染（前值 → 当前值 → 差异消息）
/// - `removed()`: 移除渲染（Source 被注销时，默认 None）
pub trait ContextSource: Send + Sync {
    /// 稳定标识
    fn key(&self) -> &SourceKey;

    /// 观察当前值（可能返回 Unavailable 表示临时不可用）
    fn load(&self) -> Result<SourceValue>;

    /// 首次渲染：完整基线文本
    fn baseline(&self, value: &SourceValue) -> String;

    /// 变更差异渲染：前值 → 当前值 → 差异消息
    fn update(&self, previous: &SourceValue, current: &SourceValue) -> String;

    /// 移除渲染（Source 被注销时）
    fn removed(&self, _previous: &SourceValue) -> Option<String> {
        None
    }
}

// ============================================================
// SystemContext
// ============================================================

/// 系统上下文：多个 Source 的组合
///
/// 管理多个 ContextSource 的生命周期，提供：
/// - `initialize()`: 首次渲染完整基线
/// - `reconcile()`: 比较当前值与快照，决定下一步动作
/// - `replace()`: 压缩后重新生成基线
pub struct SystemContext {
    sources: Vec<Box<dyn ContextSource>>,
}

impl SystemContext {
    /// 创建空 SystemContext
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
        }
    }

    /// 注册一个 Source
    pub fn register(&mut self, source: Box<dyn ContextSource>) -> &mut Self {
        self.sources.push(source);
        self
    }

    /// 获取已注册的 Source 数量
    pub fn source_count(&self) -> usize {
        self.sources.len()
    }

    /// 获取所有 Source 的 key
    pub fn source_keys(&self) -> Vec<&SourceKey> {
        self.sources.iter().map(|s| s.key()).collect()
    }

    /// 初始化：首次渲染完整基线
    ///
    /// 遍历所有 Source，调用 `load()` 获取当前值，调用 `baseline()` 渲染基线文本。
    /// 如果任何 Source 返回 `Unavailable`，整体返回 `ReplacementBlocked` 语义的错误。
    ///
    /// 返回 (基线文本, 初始快照)
    pub fn initialize(&self) -> Result<(String, ContextSnapshot)> {
        let mut parts = Vec::with_capacity(self.sources.len());
        let mut snapshot = ContextSnapshot::new();

        for source in &self.sources {
            let value = source.load()?;
            match &value {
                SourceValue::Unavailable => {
                    // 首次初始化时不可用 — 跳过此 Source，不加入基线
                    continue;
                }
                _ => {
                    let text = source.baseline(&value);
                    if !text.is_empty() {
                        parts.push(text);
                    }
                    // 预计算 removed() 消息并存储到快照
                    let removed_msg = source.removed(&value);
                    snapshot.set_with_removed(source.key().clone(), value, removed_msg);
                }
            }
        }

        Ok((parts.join("\n\n"), snapshot))
    }

    /// 协调：比较当前值与快照，决定下一步动作
    ///
    /// 遍历所有 Source，调用 `load()` 获取当前值，与快照中的前值比较：
    /// - 值相同 → Unchanged
    /// - 值变化 → Updated（生成差异消息）
    /// - 当前不可用 → ReplacementBlocked
    /// - Source 被移除 → 调用 `removed()` 生成移除消息
    pub fn reconcile(&self, snapshot: &ContextSnapshot) -> Result<ReconcileResult> {
        let mut updates = Vec::new();
        let mut new_snapshot = ContextSnapshot::new();
        let mut has_blocked = false;
        let mut has_changes = false;

        for source in &self.sources {
            let value = source.load()?;
            let key = source.key();

            match &value {
                SourceValue::Unavailable => {
                    has_blocked = true;
                    // 保留旧值
                    if let Some((old_val, _)) = snapshot.sources.get(key) {
                        new_snapshot.set(key.clone(), old_val.clone());
                    }
                }
                _ => {
                    if let Some((prev_val, _)) = snapshot.sources.get(key) {
                        // 已存在的 Source — 比较变化
                        if prev_val != &value {
                            let text = source.update(prev_val, &value);
                            if !text.is_empty() {
                                updates.push(text);
                            }
                            has_changes = true;
                        }
                    } else {
                        // 新增的 Source — 渲染基线
                        let text = source.baseline(&value);
                        if !text.is_empty() {
                            updates.push(text);
                        }
                        has_changes = true;
                    }
                    new_snapshot.set(key.clone(), value);
                }
            }
        }

        // 检查被移除的 Source — 使用快照中预存的 removed() 消息
        for (key, (_, removed_msg)) in &snapshot.sources {
            if !self.sources.iter().any(|s| s.key() == key) {
                // Source 已从当前 SystemContext 移除 — 使用快照中的预存移除消息
                if let Some(msg) = removed_msg {
                    updates.push(msg.clone());
                    has_changes = true;
                }
            }
        }

        if has_blocked {
            return Ok(ReconcileResult::ReplacementBlocked);
        }

        if has_changes {
            return Ok(ReconcileResult::Updated {
                text: updates.join("\n\n"),
                snapshot: new_snapshot,
            });
        }

        Ok(ReconcileResult::Unchanged)
    }

    /// 替换：压缩后重新生成基线
    ///
    /// 遍历所有 Source，重新渲染完整基线。用于压缩后或上下文不可用后恢复。
    pub fn replace(&self, _snapshot: &ContextSnapshot) -> Result<ReconcileResult> {
        let (baseline, new_snapshot) = self.initialize()?;
        Ok(ReconcileResult::ReplacementReady {
            baseline,
            snapshot: new_snapshot,
        })
    }
}

impl Default for SystemContext {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// 内置 Source 实现
// ============================================================

pub mod epoch;
pub mod source;

// ============================================================
// TDD 测试
// ============================================================

#[cfg(test)]
mod tests;

#[cfg(test)]
mod epoch_tests;

/// 重新导出常用类型
pub use epoch::{ContextEpochManager, EpochRecord, PreparedBaseline};
pub use source::*;
