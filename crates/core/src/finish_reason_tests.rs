#![allow(clippy::unwrap_used, clippy::expect_used)]

//! FinishReason 归一化枚举 TDD 测试（TC-FR-001~015，借鉴 Rig FinishReason）。

use crate::finish_reason::FinishReason;

// ─── from_provider_str ───

#[test]
fn tc_fr_001_parse_stop() {
    assert_eq!(FinishReason::from_provider_str("stop"), FinishReason::Stop);
}

#[test]
fn tc_fr_002_parse_length() {
    assert_eq!(
        FinishReason::from_provider_str("length"),
        FinishReason::Length
    );
}

#[test]
fn tc_fr_003_parse_tool_calls() {
    assert_eq!(
        FinishReason::from_provider_str("tool_calls"),
        FinishReason::ToolCalls
    );
}

#[test]
fn tc_fr_004_parse_function_call() {
    assert_eq!(
        FinishReason::from_provider_str("function_call"),
        FinishReason::ToolCalls
    );
}

#[test]
fn tc_fr_005_parse_content_filter() {
    assert_eq!(
        FinishReason::from_provider_str("content_filter"),
        FinishReason::ContentFilter
    );
}

#[test]
fn tc_fr_006_parse_unknown() {
    assert_eq!(FinishReason::from_provider_str("max_tokens"), FinishReason::Other);
    assert_eq!(FinishReason::from_provider_str("null"), FinishReason::Other);
    assert_eq!(FinishReason::from_provider_str(""), FinishReason::Other);
}

#[test]
fn tc_fr_007_parse_case_insensitive() {
    assert_eq!(FinishReason::from_provider_str("STOP"), FinishReason::Stop);
    assert_eq!(FinishReason::from_provider_str("Length"), FinishReason::Length);
    assert_eq!(
        FinishReason::from_provider_str("TOOL_CALLS"),
        FinishReason::ToolCalls
    );
}

#[test]
fn tc_fr_008_parse_with_whitespace() {
    assert_eq!(
        FinishReason::from_provider_str("  stop  "),
        FinishReason::Stop
    );
    assert_eq!(
        FinishReason::from_provider_str("\tlength\n"),
        FinishReason::Length
    );
}

// ─── reconcile_with_output ───

#[test]
fn tc_fr_009_reconcile_stop_without_action_stays_stop() {
    let reason = FinishReason::Stop;
    let output = "This is a normal answer without any actions.";
    assert_eq!(
        reason.reconcile_with_output(output),
        FinishReason::Stop
    );
}

#[test]
fn tc_fr_010_reconcile_stop_with_action_upgrades_to_tool_calls() {
    let reason = FinishReason::Stop;
    let output = "Thought: I need to search\nAction: search_kb\nAction Input: rust async";
    assert_eq!(
        reason.reconcile_with_output(output),
        FinishReason::ToolCalls
    );
}

#[test]
fn tc_fr_011_reconcile_stop_with_numbered_action_upgrades() {
    let reason = FinishReason::Stop;
    let output = "Thought: I need to search multiple things\nAction 1: search_kb\nAction Input 1: rust\nAction 2: search_kb\nAction Input 2: tokio";
    assert_eq!(
        reason.reconcile_with_output(output),
        FinishReason::ToolCalls
    );
}

#[test]
fn tc_fr_012_reconcile_length_unchanged() {
    let reason = FinishReason::Length;
    let output = "Thought: I need to search\nAction: search_kb";
    // Length 不被 reconcile 修改（只有 Stop → ToolCalls 升级）
    assert_eq!(
        reason.reconcile_with_output(output),
        FinishReason::Length
    );
}

#[test]
fn tc_fr_013_reconcile_tool_calls_unchanged() {
    let reason = FinishReason::ToolCalls;
    let output = "Some output";
    assert_eq!(
        reason.reconcile_with_output(output),
        FinishReason::ToolCalls
    );
}

// ─── is_truncated ───

#[test]
fn tc_fr_014_truncated_length_and_content_filter() {
    assert!(FinishReason::Length.is_truncated());
    assert!(FinishReason::ContentFilter.is_truncated());
}

#[test]
fn tc_fr_015_not_truncated_stop_tool_calls_other() {
    assert!(!FinishReason::Stop.is_truncated());
    assert!(!FinishReason::ToolCalls.is_truncated());
    assert!(!FinishReason::Other.is_truncated());
}

// ─── as_str + Display ───

#[test]
fn tc_fr_016_as_str() {
    assert_eq!(FinishReason::Stop.as_str(), "stop");
    assert_eq!(FinishReason::Length.as_str(), "length");
    assert_eq!(FinishReason::ToolCalls.as_str(), "tool_calls");
    assert_eq!(FinishReason::ContentFilter.as_str(), "content_filter");
    assert_eq!(FinishReason::Other.as_str(), "other");
}

#[test]
fn tc_fr_017_display() {
    assert_eq!(format!("{}", FinishReason::Stop), "stop");
    assert_eq!(format!("{}", FinishReason::Length), "length");
}

// ─── serde roundtrip ───

#[test]
fn tc_fr_018_serde_roundtrip() {
    let reason = FinishReason::ToolCalls;
    let json = serde_json::to_string(&reason).unwrap();
    assert_eq!(json, "\"tool_calls\"");
    let deserialized: FinishReason = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, reason);
}

#[test]
fn tc_fr_019_serde_all_variants() {
    for variant in [
        FinishReason::Stop,
        FinishReason::Length,
        FinishReason::ToolCalls,
        FinishReason::ContentFilter,
        FinishReason::Other,
    ] {
        let json = serde_json::to_string(&variant).unwrap();
        let back: FinishReason = serde_json::from_str(&json).unwrap();
        assert_eq!(back, variant);
    }
}

// ─── Default ───

#[test]
fn tc_fr_020_default_is_other() {
    assert_eq!(FinishReason::default(), FinishReason::Other);
}

// ─── reconcile 边界：Action 出现在非行首位置不触发升级 ───

#[test]
fn tc_fr_021_reconcile_action_in_text_not_upgraded() {
    let reason = FinishReason::Stop;
    // "Action:" 出现在句子中间，不是行首 Action 模式
    let output = "The user asked about Action: what is it?";
    assert_eq!(
        reason.reconcile_with_output(output),
        FinishReason::Stop
    );
}
