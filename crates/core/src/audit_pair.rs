//! 审计对 + Turn-Enclosed 模块（DH-04 借鉴 DeepSeek Harness Turn-Enclosed Audit）。
//!
//! 借鉴 DeepSeek Harness 的 Turn-Enclosed 审计设计：每次对话 turn 生成一对审计条目
//!（turn_start + turn_end），通过 `turn_id` 关联，确保每个 turn 都有完整的开始和结束记录。
//!
//! ## 设计
//!
//! 1. `TurnEvent` — 一次 turn 的开始/结束事件
//! 2. `AuditPairError` — 审计对不完整时的错误类型
//! 3. `TurnAuditChecker` — 验证审计对完整性的检查器
//!
//! ## 与 DeepSeek Harness 的对应关系
//!
//! | DeepSeek Harness | EchoMind |
//! |---|---|
//! | `Turn-Enclosed Audit` | `TurnEvent` + `TurnAuditChecker` |
//! | `turnStart` / `turnEnd` | `TurnEvent::Start` / `TurnEvent::End` |
//! | 审计对验证 | `TurnAuditChecker::verify_pairs()` |

use std::collections::{HashMap, HashSet};

/// Turn 事件类型（借鉴 DeepSeek Harness turnStart/turnEnd）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnEvent {
    /// Turn 开始（用户发送消息）
    Start,
    /// Turn 结束（AI 回答完成/中断/出错）
    End,
}

/// Turn 结束状态。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnEndStatus {
    /// 正常完成
    Completed,
    /// 用户中断
    Cancelled,
    /// 出错
    Error,
}

/// Turn 审计对中的单个事件。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TurnAuditEvent {
    /// Turn 唯一标识（关联 start/end 对）
    pub turn_id: String,
    /// 会话 ID
    pub conversation_id: String,
    /// 事件类型
    pub event: TurnEvent,
    /// 结束状态（仅 End 事件有值）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_status: Option<TurnEndStatus>,
    /// 时间戳（Unix epoch seconds）
    pub timestamp: i64,
}

/// 审计对验证错误。
#[derive(Debug, Clone)]
pub enum AuditPairError {
    /// 有 Start 但无 End（未关闭的 turn）
    UnclosedTurn(String),
    /// 有 End 但无 Start（孤立的 End）
    OrphanedEnd(String),
    /// 时间戳倒序（End 在 Start 之前）
    TimeInversion(String),
}

impl std::fmt::Display for AuditPairError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnclosedTurn(id) => {
                write!(
                    f,
                    "turn \"{id}\" has a Start event but no matching End event"
                )
            }
            Self::OrphanedEnd(id) => {
                write!(
                    f,
                    "turn \"{id}\" has an End event but no matching Start event"
                )
            }
            Self::TimeInversion(id) => {
                write!(f, "turn \"{id}\" End timestamp is before Start timestamp")
            }
        }
    }
}

impl std::error::Error for AuditPairError {}

/// Turn 审计对验证结果。
#[derive(Debug, Clone)]
pub struct TurnAuditVerification {
    /// 验证通过的 turn 数
    pub valid_pairs: usize,
    /// 错误列表
    pub errors: Vec<AuditPairError>,
}

impl TurnAuditVerification {
    /// 是否全部通过验证。
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Turn 审计检查器（验证审计对完整性）。
///
/// 借鉴 DeepSeek Harness 的 Turn-Enclosed 验证逻辑：
/// 1. 每个 `turn_id` 应同时有 Start 和 End 事件
/// 2. End 时间戳应 >= Start 时间戳
/// 3. 无孤立事件
pub struct TurnAuditChecker;

impl TurnAuditChecker {
    /// 验证审计事件列表的完整性。
    ///
    /// # 参数
    /// - `events`: 按 timestamp 排序的审计事件列表
    ///
    /// # 返回
    /// `TurnAuditVerification` 包含有效对数和错误列表
    pub fn verify(events: &[TurnAuditEvent]) -> TurnAuditVerification {
        let mut starts: HashMap<&str, &TurnAuditEvent> = HashMap::new();
        let mut ends: HashMap<&str, &TurnAuditEvent> = HashMap::new();
        let mut errors = Vec::new();

        // 第一遍：收集所有 start 和 end
        for ev in events {
            match ev.event {
                TurnEvent::Start => {
                    if starts.contains_key(ev.turn_id.as_str()) {
                        // 重复 start — 忽略，只保留第一个
                    } else {
                        starts.insert(&ev.turn_id, ev);
                    }
                }
                TurnEvent::End => {
                    if ends.contains_key(ev.turn_id.as_str()) {
                        // 重复 end — 忽略，只保留第一个
                    } else {
                        ends.insert(&ev.turn_id, ev);
                    }
                }
            }
        }

        // 第二遍：验证配对
        let start_ids: HashSet<&str> = starts.keys().copied().collect();
        let end_ids: HashSet<&str> = ends.keys().copied().collect();

        // 有 start 但无 end
        for id in start_ids.difference(&end_ids) {
            errors.push(AuditPairError::UnclosedTurn(id.to_string()));
        }

        // 有 end 但无 start
        for id in end_ids.difference(&start_ids) {
            errors.push(AuditPairError::OrphanedEnd(id.to_string()));
        }

        // 交集：验证时间戳
        let valid_pairs = start_ids.intersection(&end_ids).count();
        for id in start_ids.intersection(&end_ids) {
            let start_ev = starts.get(id).copied();
            let end_ev = ends.get(id).copied();
            if let (Some(s), Some(e)) = (start_ev, end_ev)
                && e.timestamp < s.timestamp
            {
                errors.push(AuditPairError::TimeInversion(id.to_string()));
            }
        }

        TurnAuditVerification {
            valid_pairs,
            errors,
        }
    }

    /// 创建一个 turn start 事件。
    pub fn create_start(turn_id: &str, conversation_id: &str) -> TurnAuditEvent {
        TurnAuditEvent {
            turn_id: turn_id.to_string(),
            conversation_id: conversation_id.to_string(),
            event: TurnEvent::Start,
            end_status: None,
            timestamp: chrono::Utc::now().timestamp(),
        }
    }

    /// 创建一个 turn end 事件。
    pub fn create_end(
        turn_id: &str,
        conversation_id: &str,
        status: TurnEndStatus,
    ) -> TurnAuditEvent {
        TurnAuditEvent {
            turn_id: turn_id.to_string(),
            conversation_id: conversation_id.to_string(),
            event: TurnEvent::End,
            end_status: Some(status),
            timestamp: chrono::Utc::now().timestamp(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_pairs_valid() {
        let events = vec![
            TurnAuditChecker::create_start("t1", "c1"),
            TurnAuditChecker::create_end("t1", "c1", TurnEndStatus::Completed),
            TurnAuditChecker::create_start("t2", "c1"),
            TurnAuditChecker::create_end("t2", "c1", TurnEndStatus::Completed),
        ];
        let result = TurnAuditChecker::verify(&events);
        assert!(result.is_ok());
        assert_eq!(result.valid_pairs, 2);
    }

    #[test]
    fn test_unclosed_turn() {
        let events = vec![
            TurnAuditChecker::create_start("t1", "c1"),
            TurnAuditChecker::create_start("t2", "c1"),
            TurnAuditChecker::create_end("t2", "c1", TurnEndStatus::Completed),
        ];
        let result = TurnAuditChecker::verify(&events);
        assert!(!result.is_ok());
        assert_eq!(result.valid_pairs, 1);
        assert_eq!(result.errors.len(), 1);
        assert!(matches!(
            result.errors[0],
            AuditPairError::UnclosedTurn(ref id) if id == "t1"
        ));
    }

    #[test]
    fn test_orphaned_end() {
        let events = vec![
            TurnAuditChecker::create_start("t1", "c1"),
            TurnAuditChecker::create_end("t1", "c1", TurnEndStatus::Completed),
            TurnAuditChecker::create_end("t2", "c1", TurnEndStatus::Error),
        ];
        let result = TurnAuditChecker::verify(&events);
        assert!(!result.is_ok());
        assert_eq!(result.valid_pairs, 1);
        assert_eq!(result.errors.len(), 1);
        assert!(matches!(
            result.errors[0],
            AuditPairError::OrphanedEnd(ref id) if id == "t2"
        ));
    }

    #[test]
    fn test_time_inversion() {
        let mut start = TurnAuditChecker::create_start("t1", "c1");
        start.timestamp = 1000;
        let mut end = TurnAuditChecker::create_end("t1", "c1", TurnEndStatus::Completed);
        end.timestamp = 500; // Before start
        let events = vec![start, end];
        let result = TurnAuditChecker::verify(&events);
        assert!(!result.is_ok());
        assert_eq!(result.valid_pairs, 1); // Still counted as a pair
        assert_eq!(result.errors.len(), 1);
        assert!(matches!(
            result.errors[0],
            AuditPairError::TimeInversion(ref id) if id == "t1"
        ));
    }

    #[test]
    fn test_empty_events() {
        let result = TurnAuditChecker::verify(&[]);
        assert!(result.is_ok());
        assert_eq!(result.valid_pairs, 0);
    }

    #[test]
    fn test_cancelled_turn() {
        let events = vec![
            TurnAuditChecker::create_start("t1", "c1"),
            TurnAuditChecker::create_end("t1", "c1", TurnEndStatus::Cancelled),
        ];
        let result = TurnAuditChecker::verify(&events);
        assert!(result.is_ok());
        assert_eq!(result.valid_pairs, 1);
    }

    #[test]
    fn test_error_turn() {
        let events = vec![
            TurnAuditChecker::create_start("t1", "c1"),
            TurnAuditChecker::create_end("t1", "c1", TurnEndStatus::Error),
        ];
        let result = TurnAuditChecker::verify(&events);
        assert!(result.is_ok());
        assert_eq!(result.valid_pairs, 1);
    }

    #[test]
    fn test_multiple_conversations() {
        let events = vec![
            TurnAuditChecker::create_start("t1", "c1"),
            TurnAuditChecker::create_end("t1", "c1", TurnEndStatus::Completed),
            TurnAuditChecker::create_start("t2", "c2"),
            TurnAuditChecker::create_end("t2", "c2", TurnEndStatus::Completed),
            TurnAuditChecker::create_start("t3", "c1"),
            TurnAuditChecker::create_end("t3", "c2", TurnEndStatus::Completed),
        ];
        let result = TurnAuditChecker::verify(&events);
        assert!(result.is_ok());
        assert_eq!(result.valid_pairs, 3);
    }

    #[test]
    fn test_duplicate_start_ignored() {
        let events = vec![
            TurnAuditChecker::create_start("t1", "c1"),
            TurnAuditChecker::create_start("t1", "c1"), // Duplicate
            TurnAuditChecker::create_end("t1", "c1", TurnEndStatus::Completed),
        ];
        let result = TurnAuditChecker::verify(&events);
        assert!(result.is_ok());
        assert_eq!(result.valid_pairs, 1);
    }

    #[test]
    fn test_turn_event_serde_roundtrip() {
        let event = TurnAuditEvent {
            turn_id: "t1".to_string(),
            conversation_id: "c1".to_string(),
            event: TurnEvent::Start,
            end_status: None,
            timestamp: 1234567890,
        };
        let json = serde_json::to_string(&event).unwrap_or_default();
        let deserialized: TurnAuditEvent =
            serde_json::from_str(&json).unwrap_or_else(|_| TurnAuditEvent {
                turn_id: String::new(),
                conversation_id: String::new(),
                event: TurnEvent::Start,
                end_status: None,
                timestamp: 0,
            });
        assert_eq!(deserialized.turn_id, "t1");
        assert_eq!(deserialized.event, TurnEvent::Start);
    }

    #[test]
    fn test_turn_end_status_serde() {
        let json = "\"completed\"";
        let status: TurnEndStatus = serde_json::from_str(json).unwrap_or(TurnEndStatus::Error);
        assert_eq!(status, TurnEndStatus::Completed);
    }
}
