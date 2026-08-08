#![allow(clippy::type_complexity)]

//! # Context Epoch + Durable Baseline（上下文纪元 + 持久化基线）
//!
//! 借鉴 OpenCode v1.18.15 的 `packages/core/src/session/context-epoch.ts`。
//!
//! 每个会话有一个不可变的 Baseline System Context，持久化到数据库。
//! 跨进程重启时复用同一基线。只有压缩、会话移动或上下文不可用时才开启新纪元。
//!
//! ## 核心概念
//!
//! - **ContextEpochManager**: 管理会话上下文纪元的生命周期
//! - **PreparedBaseline**: 准备好的基线（基线文本 + 序列号），供 ChatEngine 使用
//!
//! ## 纪元触发条件
//!
//! 1. **首次初始化**：会话首次调用时，`SystemContext.initialize()` 生成基线
//! 2. **增量更新**：Source 值变化时，生成 Mid-Conversation System Message（不换基线）
//! 3. **新纪元**：压缩后或上下文不可用后恢复，`SystemContext.replace()` 生成新基线
//! 4. **保留旧基线**：Source 不可用时，保留旧基线（ReplacementBlocked）

use anyhow::Result;

use crate::{ContextSnapshot, SystemContext};

/// 准备好的基线（供 ChatEngine 使用）
///
/// - `baseline_text`：不可变基线文本，作为 Prompt Caching 前缀
/// - `baseline_seq`：基线序列号，每次新纪元 +1
#[derive(Debug, Clone)]
pub struct PreparedBaseline {
    /// 基线文本（不可变，作为 Prompt Caching 前缀）
    pub baseline_text: String,
    /// 基线序列号
    pub baseline_seq: i64,
}

/// 上下文纪元管理器
///
/// 管理 `SystemContext` 的持久化基线，提供 `initialize()` / `prepare()` / `advance()` 方法。
///
/// **设计**：不直接依赖 `Storage` trait（避免循环依赖和 async trait 对象安全问题）。
/// 由调用方（`chat_inner`）在 infra 层实现 epoch 持久化逻辑，
/// 本模块仅提供类型定义和逻辑编排。
pub struct ContextEpochManager {
    /// epoch 存储回调（由调用方注入）
    load_epoch: Box<dyn Fn(&str) -> Result<Option<EpochRecord>> + Send + Sync>,
    /// epoch 保存回调
    save_epoch: Box<dyn Fn(&str, &EpochRecord) -> Result<()> + Send + Sync>,
    /// update 记录回调
    #[allow(clippy::type_complexity)]
    save_update: Box<dyn Fn(&str, &str, &ContextSnapshot) -> Result<()> + Send + Sync>,
}

/// epoch 持久化记录（对应 context_epochs 表的一行）
#[derive(Debug, Clone)]
pub struct EpochRecord {
    /// 基线文本
    pub baseline_text: String,
    /// 基线序列号
    pub baseline_seq: i64,
    /// 快照 JSON
    pub snapshot_json: String,
}

impl ContextEpochManager {
    /// 创建 ContextEpochManager，注入存储回调
    ///
    /// # 参数
    /// - `load_epoch`: 加载会话最新 epoch 记录（返回 None 表示无已有基线）
    /// - `save_epoch`: 保存/更新 epoch 记录
    /// - `save_update`: 保存 Mid-Conversation System Message 更新
    pub fn new(
        load_epoch: Box<dyn Fn(&str) -> Result<Option<EpochRecord>> + Send + Sync>,
        save_epoch: Box<dyn Fn(&str, &EpochRecord) -> Result<()> + Send + Sync>,
        save_update: Box<dyn Fn(&str, &str, &ContextSnapshot) -> Result<()> + Send + Sync>,
    ) -> Self {
        Self {
            load_epoch,
            save_epoch,
            save_update,
        }
    }

    /// 首次初始化：生成基线 + 快照
    ///
    /// 调用 `SystemContext.initialize()` 生成完整基线文本和初始快照，
    /// 持久化到数据库。
    pub fn initialize(
        &self,
        conversation_id: &str,
        context: &SystemContext,
    ) -> Result<PreparedBaseline> {
        let (baseline_text, snapshot) = context.initialize()?;
        let snapshot_json = serde_json::to_string(&snapshot)?;
        let seq = 0;
        let record = EpochRecord {
            baseline_text: baseline_text.clone(),
            baseline_seq: seq,
            snapshot_json,
        };
        (self.save_epoch)(conversation_id, &record)?;
        Ok(PreparedBaseline {
            baseline_text,
            baseline_seq: seq,
        })
    }

    /// 准备：加载已有基线或创建新的
    ///
    /// - 有存储的基线 + 无压缩 → reconcile（增量更新）
    /// - 有存储的基线 + 有压缩 → replace（新纪元）
    /// - 无存储的基线 → initialize
    pub fn prepare(
        &self,
        conversation_id: &str,
        context: &SystemContext,
    ) -> Result<PreparedBaseline> {
        let existing = (self.load_epoch)(conversation_id)?;

        if let Some(epoch) = existing {
            // 有已有基线 — reconcile
            let snapshot: ContextSnapshot =
                serde_json::from_str(&epoch.snapshot_json).unwrap_or_default();

            match context.reconcile(&snapshot)? {
                crate::ReconcileResult::Unchanged => {
                    // 基线不变
                    Ok(PreparedBaseline {
                        baseline_text: epoch.baseline_text,
                        baseline_seq: epoch.baseline_seq,
                    })
                }
                crate::ReconcileResult::Updated {
                    text,
                    snapshot: new_snap,
                } => {
                    // 记录更新消息（不换基线）
                    (self.save_update)(conversation_id, &text, &new_snap)?;
                    // 更新 epoch 快照
                    let new_snap_json = serde_json::to_string(&new_snap)?;
                    let baseline_text = epoch.baseline_text.clone();
                    let baseline_seq = epoch.baseline_seq;
                    let updated = EpochRecord {
                        baseline_text: baseline_text.clone(),
                        baseline_seq,
                        snapshot_json: new_snap_json,
                    };
                    (self.save_epoch)(conversation_id, &updated)?;
                    Ok(PreparedBaseline {
                        baseline_text,
                        baseline_seq,
                    })
                }
                crate::ReconcileResult::ReplacementReady {
                    baseline,
                    snapshot: new_snap,
                } => {
                    // 新纪元
                    let new_snap_json = serde_json::to_string(&new_snap)?;
                    let new_seq = epoch.baseline_seq + 1;
                    let record = EpochRecord {
                        baseline_text: baseline.clone(),
                        baseline_seq: new_seq,
                        snapshot_json: new_snap_json,
                    };
                    (self.save_epoch)(conversation_id, &record)?;
                    Ok(PreparedBaseline {
                        baseline_text: baseline,
                        baseline_seq: new_seq,
                    })
                }
                crate::ReconcileResult::ReplacementBlocked => {
                    // 保留旧基线
                    Ok(PreparedBaseline {
                        baseline_text: epoch.baseline_text,
                        baseline_seq: epoch.baseline_seq,
                    })
                }
            }
        } else {
            // 无已有基线 — initialize
            self.initialize(conversation_id, context)
        }
    }

    /// 推进快照：上下文变更后更新快照
    ///
    /// 在 reconcile 返回 Updated 后，调用方可调用此方法更新快照。
    pub fn advance(&self, conversation_id: &str, snapshot: &ContextSnapshot) -> Result<()> {
        let snapshot_json = serde_json::to_string(snapshot)?;
        let existing = (self.load_epoch)(conversation_id)?;
        if let Some(epoch) = existing {
            let updated = EpochRecord {
                baseline_text: epoch.baseline_text,
                baseline_seq: epoch.baseline_seq,
                snapshot_json,
            };
            (self.save_epoch)(conversation_id, &updated)?;
        }
        Ok(())
    }
}
