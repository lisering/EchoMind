//! 流水线阶段门控（借鉴 OpenMontage checkpoint.py）。
//!
//! ## 背景
//!
//! OpenMontage 的 `checkpoint.py` 为每个流水线阶段写入检查点文件，实现：
//! 1. **阶段前置检查**：后序阶段不能在前序阶段未完成时推进
//! 2. **人审门控**：标记为需要人审的阶段在 `awaiting_human` → `completed` 时需要显式批准
//! 3. **历史归档**：被覆盖的检查点归档到 `history/` 保留审计轨迹
//!
//! EchoMind 的 `CoordinatorEngine` 有四阶段流水线（Research→Synthesis→Implementation→
//! Verification），本模块为其提供轻量级阶段门控能力。
//!
//! ## 设计
//!
//! 与 OpenMontage 不同，EchoMind 的检查点不写文件，而是纯内存结构
//! （`CheckpointGate` 持有 `HashMap<PipelineStage, PipelineCheckpoint>`），
//! 因为 Coordinator 是无状态的多步推理，不需要跨进程恢复。
//! 门控逻辑保持一致：前置阶段检查 + 人审门控。

use std::collections::HashMap;

/// 流水线阶段（对应 `CoordinatorEngine` 的四阶段）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PipelineStage {
    /// Phase 1: 子查询分解 + 并行检索
    Research,
    /// Phase 2: 综合分析多源信息
    Synthesis,
    /// Phase 3: 流式生成最终答案
    Implementation,
    /// Phase 4: 验证答案引用
    Verification,
}

impl PipelineStage {
    /// 返回阶段的字符串标识。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Research => "research",
            Self::Synthesis => "synthesis",
            Self::Implementation => "implementation",
            Self::Verification => "verification",
        }
    }

    /// 返回所有阶段的有序列表。
    pub fn all() -> Vec<Self> {
        vec![
            Self::Research,
            Self::Synthesis,
            Self::Implementation,
            Self::Verification,
        ]
    }

    /// 返回当前阶段的所有前序阶段。
    pub fn predecessors(&self) -> Vec<Self> {
        let all = Self::all();
        let idx = all.iter().position(|s| s == self).unwrap_or(0);
        all[..idx].to_vec()
    }
}

/// 检查点状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckpointStatus {
    /// 进行中
    InProgress,
    /// 已完成
    Completed,
    /// 等待人审
    AwaitingHuman,
    /// 失败
    Failed,
}

impl CheckpointStatus {
    /// 返回状态的字符串标识。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
            Self::AwaitingHuman => "awaiting_human",
            Self::Failed => "failed",
        }
    }
}

/// 单个阶段的检查点记录。
#[derive(Debug, Clone)]
pub struct PipelineCheckpoint {
    /// 阶段
    pub stage: PipelineStage,
    /// 状态
    pub status: CheckpointStatus,
    /// 时间戳（ISO 8601，UTC）
    pub timestamp: String,
    /// 是否需要人审
    pub human_approval_required: bool,
    /// 是否已获人审批准
    pub human_approved: bool,
    /// 阶段产出物摘要（如子查询数量、来源数量等）
    pub artifacts: HashMap<String, String>,
    /// 错误信息（失败时）
    pub error: Option<String>,
}

impl PipelineCheckpoint {
    /// 创建新的检查点。
    pub fn new(stage: PipelineStage, status: CheckpointStatus) -> Self {
        Self {
            stage,
            status,
            timestamp: current_iso8601(),
            human_approval_required: false,
            human_approved: false,
            artifacts: HashMap::new(),
            error: None,
        }
    }

    /// 标记需要人审。
    #[must_use]
    pub fn with_human_approval(mut self) -> Self {
        self.human_approval_required = true;
        self
    }

    /// 标记已获人审批准。
    #[must_use]
    pub fn with_approval(mut self) -> Self {
        self.human_approved = true;
        self
    }

    /// 添加产出物。
    #[must_use]
    pub fn with_artifact(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.artifacts.insert(key.into(), value.into());
        self
    }

    /// 设置错误信息。
    #[must_use]
    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.error = Some(error.into());
        self
    }
}

/// 门控验证错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateError {
    /// 前序阶段未完成
    PrerequisiteNotMet {
        stage: PipelineStage,
        incomplete: Vec<PipelineStage>,
    },
    /// 需要人审但未获批准
    GateViolation {
        stage: PipelineStage,
        reason: String,
    },
}

impl std::fmt::Display for GateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PrerequisiteNotMet { stage, incomplete } => {
                let names: Vec<&str> = incomplete.iter().map(|s| s.as_str()).collect();
                write!(
                    f,
                    "阶段 {:?} 的前序阶段未完成: [{}]",
                    stage,
                    names.join(", ")
                )
            }
            Self::GateViolation { stage, reason } => {
                write!(f, "阶段 {:?} 门控违规: {reason}", stage)
            }
        }
    }
}

impl std::error::Error for GateError {}

/// 流水线阶段门控管理器（借鉴 OpenMontage `checkpoint.py` 的门控逻辑）。
///
/// 纯内存结构，持有所有阶段的检查点记录。在阶段推进时验证前置条件和门控规则。
///
/// # 门控规则
///
/// 1. **前置检查**：阶段 N 推进到 `Completed` 或 `AwaitingHuman` 时，
///    所有前序阶段必须已 `Completed`。
/// 2. **人审门控**：标记为 `human_approval_required` 的阶段，
///    只能从 `AwaitingHuman` → `Completed` 且 `human_approved = true`。
///
/// # 向后兼容
///
/// `CoordinatorEngine` 的 `checkpoint_gate` 字段为 `Option<CheckpointGate>`，
/// `None` 时行为与之前完全一致（零开销）。
#[derive(Debug, Default)]
pub struct CheckpointGate {
    /// 各阶段的检查点记录
    checkpoints: HashMap<PipelineStage, PipelineCheckpoint>,
}

impl CheckpointGate {
    /// 创建空门控管理器。
    pub fn new() -> Self {
        Self::default()
    }

    /// 记录阶段检查点（不执行门控验证，仅记录）。
    pub fn record(&mut self, checkpoint: PipelineCheckpoint) {
        self.checkpoints.insert(checkpoint.stage, checkpoint);
    }

    /// 验证阶段是否可以推进到指定状态。
    ///
    /// 借鉴 OpenMontage `_enforce_stage_prerequisites()` + gate enforcement。
    ///
    /// # 参数
    /// - `stage`：要推进的阶段
    /// - `target_status`：目标状态
    ///
    /// # 返回
    /// - `Ok(())`：可以推进
    /// - `Err(GateError)`：门控违规
    pub fn validate(
        &self,
        stage: PipelineStage,
        target_status: CheckpointStatus,
    ) -> Result<(), GateError> {
        // 只有 Completed 和 AwaitingHuman 需要前置检查
        if target_status != CheckpointStatus::Completed
            && target_status != CheckpointStatus::AwaitingHuman
        {
            return Ok(());
        }

        // 1. 前置检查：所有前序阶段必须已完成
        let predecessors = stage.predecessors();
        let incomplete: Vec<PipelineStage> = predecessors
            .iter()
            .filter(|pred| {
                match self.checkpoints.get(pred) {
                    None => true, // 无检查点 → 未完成
                    Some(cp) => cp.status != CheckpointStatus::Completed,
                }
            })
            .copied()
            .collect();

        if !incomplete.is_empty() {
            return Err(GateError::PrerequisiteNotMet { stage, incomplete });
        }

        // 2. 人审门控：检查当前阶段的检查点是否需要人审
        if target_status == CheckpointStatus::Completed
            && let Some(cp) = self.checkpoints.get(&stage)
            && cp.human_approval_required
            && !cp.human_approved
        {
            return Err(GateError::GateViolation {
                stage,
                reason: "阶段需要人审但未获批准 (human_approved=false)".to_string(),
            });
        }

        Ok(())
    }

    /// 获取指定阶段的检查点。
    pub fn get(&self, stage: PipelineStage) -> Option<&PipelineCheckpoint> {
        self.checkpoints.get(&stage)
    }

    /// 返回所有已完成的阶段。
    pub fn completed_stages(&self) -> Vec<PipelineStage> {
        PipelineStage::all()
            .into_iter()
            .filter(|s| {
                self.checkpoints
                    .get(s)
                    .is_some_and(|cp| cp.status == CheckpointStatus::Completed)
            })
            .collect()
    }

    /// 返回下一个未完成的阶段。
    pub fn next_stage(&self) -> Option<PipelineStage> {
        PipelineStage::all().into_iter().find(|s| {
            self.checkpoints
                .get(s)
                .is_none_or(|cp| cp.status != CheckpointStatus::Completed)
        })
    }

    /// 返回所有阶段的检查点摘要（用于事件推送 / 日志）。
    pub fn summary(&self) -> Vec<(PipelineStage, CheckpointStatus)> {
        PipelineStage::all()
            .into_iter()
            .map(|s| {
                let status = self
                    .checkpoints
                    .get(&s)
                    .map(|cp| cp.status)
                    .unwrap_or(CheckpointStatus::InProgress);
                (s, status)
            })
            .collect()
    }

    /// 清除所有检查点（新查询时重置）。
    pub fn clear(&mut self) {
        self.checkpoints.clear();
    }
}

/// 生成当前时间的 ISO 8601 字符串（UTC）。
fn current_iso8601() -> String {
    // 使用 std::time 而非 chrono，避免额外依赖
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("epoch:{secs}")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    /// TC-CHK-001: 前序阶段未完成时推进后序阶段 → PrerequisiteNotMet
    #[test]
    fn test_prerequisite_not_met() {
        let gate = CheckpointGate::new();
        // 没有任何前序阶段完成，直接推进 Synthesis → 应失败
        let result = gate.validate(PipelineStage::Synthesis, CheckpointStatus::Completed);
        assert!(result.is_err());
        match result.unwrap_err() {
            GateError::PrerequisiteNotMet { stage, incomplete } => {
                assert_eq!(stage, PipelineStage::Synthesis);
                assert!(incomplete.contains(&PipelineStage::Research));
            }
            other => panic!("expected PrerequisiteNotMet, got {other:?}"),
        }
    }

    /// TC-CHK-002: 前序阶段完成后推进后序阶段 → Ok
    #[test]
    fn test_prerequisite_met() {
        let mut gate = CheckpointGate::new();
        gate.record(PipelineCheckpoint::new(
            PipelineStage::Research,
            CheckpointStatus::Completed,
        ));
        let result = gate.validate(PipelineStage::Synthesis, CheckpointStatus::Completed);
        assert!(result.is_ok());
    }

    /// TC-CHK-003: 人审门控 — 需要人审但未批准 → GateViolation
    #[test]
    fn test_gate_violation_no_approval() {
        let mut gate = CheckpointGate::new();
        // 先完成前序阶段
        gate.record(PipelineCheckpoint::new(
            PipelineStage::Research,
            CheckpointStatus::Completed,
        ));
        gate.record(PipelineCheckpoint::new(
            PipelineStage::Synthesis,
            CheckpointStatus::Completed,
        ));
        gate.record(
            PipelineCheckpoint::new(
                PipelineStage::Implementation,
                CheckpointStatus::AwaitingHuman,
            )
            .with_human_approval(),
        );
        // 尝试推进到 Completed 但未获批准
        let result = gate.validate(PipelineStage::Implementation, CheckpointStatus::Completed);
        assert!(result.is_err());
        match result.unwrap_err() {
            GateError::GateViolation { stage, .. } => {
                assert_eq!(stage, PipelineStage::Implementation);
            }
            other => panic!("expected GateViolation, got {other:?}"),
        }
    }

    /// TC-CHK-004: 人审门控 — 需要人审且已批准 → Ok
    #[test]
    fn test_gate_approved() {
        let mut gate = CheckpointGate::new();
        gate.record(PipelineCheckpoint::new(
            PipelineStage::Research,
            CheckpointStatus::Completed,
        ));
        gate.record(PipelineCheckpoint::new(
            PipelineStage::Synthesis,
            CheckpointStatus::Completed,
        ));
        gate.record(
            PipelineCheckpoint::new(
                PipelineStage::Implementation,
                CheckpointStatus::AwaitingHuman,
            )
            .with_human_approval()
            .with_approval(),
        );
        let result = gate.validate(PipelineStage::Implementation, CheckpointStatus::Completed);
        assert!(result.is_ok());
    }

    /// TC-CHK-005: InProgress 状态不需要前置检查
    #[test]
    fn test_in_progress_no_prerequisite() {
        let gate = CheckpointGate::new();
        // InProgress 不需要前置检查
        let result = gate.validate(PipelineStage::Synthesis, CheckpointStatus::InProgress);
        assert!(result.is_ok());
    }

    /// TC-CHK-006: completed_stages 返回已完成阶段
    #[test]
    fn test_completed_stages() {
        let mut gate = CheckpointGate::new();
        gate.record(PipelineCheckpoint::new(
            PipelineStage::Research,
            CheckpointStatus::Completed,
        ));
        gate.record(PipelineCheckpoint::new(
            PipelineStage::Synthesis,
            CheckpointStatus::InProgress,
        ));
        let completed = gate.completed_stages();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0], PipelineStage::Research);
    }

    /// TC-CHK-007: next_stage 返回第一个未完成阶段
    #[test]
    fn test_next_stage() {
        let mut gate = CheckpointGate::new();
        gate.record(PipelineCheckpoint::new(
            PipelineStage::Research,
            CheckpointStatus::Completed,
        ));
        assert_eq!(gate.next_stage(), Some(PipelineStage::Synthesis));
    }

    /// TC-CHK-008: next_stage 全部完成返回 None
    #[test]
    fn test_next_stage_all_completed() {
        let mut gate = CheckpointGate::new();
        for stage in PipelineStage::all() {
            gate.record(PipelineCheckpoint::new(stage, CheckpointStatus::Completed));
        }
        assert_eq!(gate.next_stage(), None);
    }

    /// TC-CHK-009: summary 返回所有阶段状态
    #[test]
    fn test_summary() {
        let mut gate = CheckpointGate::new();
        gate.record(PipelineCheckpoint::new(
            PipelineStage::Research,
            CheckpointStatus::Completed,
        ));
        let summary = gate.summary();
        assert_eq!(summary.len(), 4);
        assert_eq!(summary[0].1, CheckpointStatus::Completed);
        assert_eq!(summary[1].1, CheckpointStatus::InProgress); // 无记录默认 InProgress
    }

    /// TC-CHK-010: clear 清除所有检查点
    #[test]
    fn test_clear() {
        let mut gate = CheckpointGate::new();
        gate.record(PipelineCheckpoint::new(
            PipelineStage::Research,
            CheckpointStatus::Completed,
        ));
        gate.clear();
        assert!(gate.completed_stages().is_empty());
    }

    /// TC-CHK-011: PipelineStage::predecessors 正确性
    #[test]
    fn test_predecessors() {
        assert!(PipelineStage::Research.predecessors().is_empty());
        assert_eq!(
            PipelineStage::Synthesis.predecessors(),
            vec![PipelineStage::Research]
        );
        assert_eq!(
            PipelineStage::Implementation.predecessors(),
            vec![PipelineStage::Research, PipelineStage::Synthesis]
        );
        assert_eq!(
            PipelineStage::Verification.predecessors(),
            vec![
                PipelineStage::Research,
                PipelineStage::Synthesis,
                PipelineStage::Implementation
            ]
        );
    }

    /// TC-CHK-012: CheckpointStatus as_str
    #[test]
    fn test_status_as_str() {
        assert_eq!(CheckpointStatus::InProgress.as_str(), "in_progress");
        assert_eq!(CheckpointStatus::Completed.as_str(), "completed");
        assert_eq!(CheckpointStatus::AwaitingHuman.as_str(), "awaiting_human");
        assert_eq!(CheckpointStatus::Failed.as_str(), "failed");
    }

    /// TC-CHK-013: PipelineCheckpoint builder 方法
    #[test]
    fn test_checkpoint_builder() {
        let cp = PipelineCheckpoint::new(PipelineStage::Research, CheckpointStatus::Completed)
            .with_artifact("sub_queries", "3")
            .with_artifact("sources", "12");
        assert_eq!(cp.artifacts.len(), 2);
        assert_eq!(cp.artifacts.get("sub_queries"), Some(&"3".to_string()));
    }

    /// TC-CHK-014: 完整流水线推进
    #[test]
    fn test_full_pipeline_progression() {
        let mut gate = CheckpointGate::new();

        // Research → Completed
        assert!(
            gate.validate(PipelineStage::Research, CheckpointStatus::Completed)
                .is_ok()
        );
        gate.record(PipelineCheckpoint::new(
            PipelineStage::Research,
            CheckpointStatus::Completed,
        ));

        // Synthesis → Completed
        assert!(
            gate.validate(PipelineStage::Synthesis, CheckpointStatus::Completed)
                .is_ok()
        );
        gate.record(PipelineCheckpoint::new(
            PipelineStage::Synthesis,
            CheckpointStatus::Completed,
        ));

        // Implementation → Completed
        assert!(
            gate.validate(PipelineStage::Implementation, CheckpointStatus::Completed)
                .is_ok()
        );
        gate.record(PipelineCheckpoint::new(
            PipelineStage::Implementation,
            CheckpointStatus::Completed,
        ));

        // Verification → Completed
        assert!(
            gate.validate(PipelineStage::Verification, CheckpointStatus::Completed)
                .is_ok()
        );
        gate.record(PipelineCheckpoint::new(
            PipelineStage::Verification,
            CheckpointStatus::Completed,
        ));

        assert_eq!(gate.completed_stages().len(), 4);
        assert_eq!(gate.next_stage(), None);
    }

    /// TC-CHK-015: Failed 状态不需要前置检查
    #[test]
    fn test_failed_no_prerequisite() {
        let gate = CheckpointGate::new();
        let result = gate.validate(PipelineStage::Synthesis, CheckpointStatus::Failed);
        assert!(result.is_ok());
    }
}
