#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented,
    clippy::type_complexity
)]

//! TDD 测试：System Context Registry（TC-CTX-001~008）
//!
//! 测试策略：
//! - TC-CTX-001~004: 使用简单 MockSource 测试 trait 行为
//! - TC-CTX-005~006: 测试多 Source 组合和快照
//! - TC-CTX-007~008: 测试持久化和移除

use crate::*;

// ============================================================
// 辅助 MockSource
// ============================================================

/// 可控 MockSource — 用于测试 trait 行为
struct MockSource {
    key: SourceKey,
    value: SourceValue,
    baseline_fn: Box<dyn Fn(&SourceValue) -> String + Send + Sync>,
    update_fn: Box<dyn Fn(&SourceValue, &SourceValue) -> String + Send + Sync>,
    removed_fn: Box<dyn Fn(&SourceValue) -> Option<String> + Send + Sync>,
}

impl MockSource {
    fn new_text(key: &str, text: &str) -> Self {
        Self {
            key: key.to_string(),
            value: SourceValue::Text(text.to_string()),
            baseline_fn: Box::new(|v| match v {
                SourceValue::Text(t) => t.clone(),
                _ => String::new(),
            }),
            update_fn: Box::new(|_, _| String::new()),
            removed_fn: Box::new(|_| None),
        }
    }

    fn new_counter(key: &str, count: usize) -> Self {
        Self {
            key: key.to_string(),
            value: SourceValue::Json(serde_json::json!({"count": count})),
            baseline_fn: Box::new(|v| match v {
                SourceValue::Json(j) => {
                    let c = j.get("count").and_then(|v| v.as_u64()).unwrap_or(0);
                    format!("计数: {}", c)
                }
                _ => String::new(),
            }),
            update_fn: Box::new(|prev, curr| {
                let p = prev.as_count();
                let c = curr.as_count();
                if p == c {
                    String::new()
                } else {
                    format!("[更新] {} → {}", p, c)
                }
            }),
            removed_fn: Box::new(|_| Some("[移除] Source 已注销".to_string())),
        }
    }

    fn with_removed(mut self, msg: String) -> Self {
        self.removed_fn = Box::new(move |_| Some(msg.clone()));
        self
    }
}

impl ContextSource for MockSource {
    fn key(&self) -> &SourceKey {
        &self.key
    }

    fn load(&self) -> anyhow::Result<SourceValue> {
        Ok(self.value.clone())
    }

    fn baseline(&self, value: &SourceValue) -> String {
        (self.baseline_fn)(value)
    }

    fn update(&self, previous: &SourceValue, current: &SourceValue) -> String {
        (self.update_fn)(previous, current)
    }

    fn removed(&self, previous: &SourceValue) -> Option<String> {
        (self.removed_fn)(previous)
    }
}

// SourceValue 辅助方法（测试用）
impl SourceValue {
    fn as_count(&self) -> usize {
        match self {
            SourceValue::Json(v) => v
                .get("count")
                .and_then(|c| c.as_u64())
                .map(|c| c as usize)
                .unwrap_or(0),
            _ => 0,
        }
    }
}

// ============================================================
// TC-CTX-001: 单个 Source 的 baseline 渲染正确
// ============================================================

#[test]
fn tc_ctx_001_single_source_baseline() {
    let source = MockSource::new_text("system/role", "你是 EchoMind 助手。");
    let value = source.load().unwrap();
    let baseline = source.baseline(&value);
    assert_eq!(baseline, "你是 EchoMind 助手。");
}

// ============================================================
// TC-CTX-002: Source 值不变时 reconcile 返回 Unchanged
// ============================================================

#[test]
fn tc_ctx_002_unchanged_returns_unchanged() {
    let mut ctx = SystemContext::new();
    ctx.register(Box::new(MockSource::new_counter("kb/summary", 5)));

    let (baseline, snapshot) = ctx.initialize().unwrap();
    assert!(!baseline.is_empty());

    // 值不变 → Unchanged
    let result = ctx.reconcile(&snapshot).unwrap();
    assert!(
        matches!(result, ReconcileResult::Unchanged),
        "值不变时应返回 Unchanged"
    );
}

// ============================================================
// TC-CTX-003: Source 值变化时 reconcile 返回 Updated + 正确差异文本
// ============================================================

#[test]
fn tc_ctx_003_changed_returns_updated() {
    let mut ctx = SystemContext::new();
    let source = MockSource::new_counter("kb/summary", 5);
    ctx.register(Box::new(source));

    let (_, snapshot) = ctx.initialize().unwrap();

    // 重新创建 ctx，值变为 10
    let mut ctx2 = SystemContext::new();
    ctx2.register(Box::new(MockSource::new_counter("kb/summary", 10)));

    let result = ctx2.reconcile(&snapshot).unwrap();
    match result {
        ReconcileResult::Updated {
            text,
            snapshot: new_snap,
        } => {
            assert!(
                text.contains("[更新]"),
                "差异文本应包含 [更新]，实际: {}",
                text
            );
            assert!(text.contains("5"), "差异文本应包含旧值 5");
            assert!(text.contains("10"), "差异文本应包含新值 10");
            // 新快照应有 10
            let new_val = new_snap.get("kb/summary").unwrap();
            assert_eq!(new_val.as_count(), 10, "新快照应包含更新后的值 10");
        }
        _ => panic!("值变化时应返回 Updated"),
    }
}

// ============================================================
// TC-CTX-004: Source 不可用时 reconcile 返回 ReplacementBlocked
// ============================================================

struct UnavailableSource {
    key: SourceKey,
}

impl ContextSource for UnavailableSource {
    fn key(&self) -> &SourceKey {
        &self.key
    }
    fn load(&self) -> anyhow::Result<SourceValue> {
        Ok(SourceValue::Unavailable)
    }
    fn baseline(&self, _: &SourceValue) -> String {
        String::new()
    }
    fn update(&self, _: &SourceValue, _: &SourceValue) -> String {
        String::new()
    }
}

#[test]
fn tc_ctx_004_unavailable_returns_blocked() {
    let mut ctx = SystemContext::new();
    ctx.register(Box::new(MockSource::new_counter("kb/summary", 5)));
    let (_, snapshot) = ctx.initialize().unwrap();

    // 重新创建 ctx，Source 不可用
    let mut ctx2 = SystemContext::new();
    ctx2.register(Box::new(UnavailableSource {
        key: "kb/summary".to_string(),
    }));

    let result = ctx2.reconcile(&snapshot).unwrap();
    assert!(
        matches!(result, ReconcileResult::ReplacementBlocked),
        "Source 不可用时应返回 ReplacementBlocked"
    );
}

// ============================================================
// TC-CTX-005: 多 Source 组合时 baseline 按注册顺序拼接
// ============================================================

#[test]
fn tc_ctx_005_multi_source_baseline_ordered() {
    let mut ctx = SystemContext::new();
    ctx.register(Box::new(MockSource::new_text("system/role", "角色描述。")));
    ctx.register(Box::new(MockSource::new_text("kb/summary", "知识库概况。")));
    ctx.register(Box::new(MockSource::new_text("conv/history", "对话历史。")));

    let (baseline, snapshot) = ctx.initialize().unwrap();

    // 验证顺序：角色 → 知识库 → 对话历史
    let role_pos = baseline.find("角色描述。").unwrap();
    let kb_pos = baseline.find("知识库概况。").unwrap();
    let conv_pos = baseline.find("对话历史。").unwrap();
    assert!(role_pos < kb_pos, "角色应在知识库之前");
    assert!(kb_pos < conv_pos, "知识库应在对话历史之前");

    // 快照应包含 3 个 Source
    assert_eq!(snapshot.len(), 3);
}

// ============================================================
// TC-CTX-006: 压缩后 replace 生成新基线
// ============================================================

#[test]
fn tc_ctx_006_replace_generates_new_baseline() {
    let mut ctx = SystemContext::new();
    ctx.register(Box::new(MockSource::new_text("system/role", "角色描述。")));
    ctx.register(Box::new(MockSource::new_counter("kb/summary", 10)));

    let (_, snapshot) = ctx.initialize().unwrap();

    // replace 应生成新基线
    let result = ctx.replace(&snapshot).unwrap();
    match result {
        ReconcileResult::ReplacementReady {
            baseline,
            snapshot: new_snap,
        } => {
            assert!(baseline.contains("角色描述。"), "新基线应包含角色描述");
            assert!(baseline.contains("计数: 10"), "新基线应包含知识库概况");
            assert_eq!(new_snap.len(), 2, "新快照应有 2 个 Source");
        }
        _ => panic!("replace 应返回 ReplacementReady"),
    }
}

// ============================================================
// TC-CTX-007: 快照持久化后跨会话恢复正确
// ============================================================

#[test]
fn tc_ctx_007_snapshot_persistence_roundtrip() {
    // 创建快照
    let mut snapshot = ContextSnapshot::new();
    snapshot.set(
        "system/role".to_string(),
        SourceValue::Text("角色".to_string()),
    );
    snapshot.set(
        "kb/summary".to_string(),
        SourceValue::Json(serde_json::json!({"count": 5})),
    );

    // 序列化 → 反序列化（模拟持久化）
    let json = serde_json::to_string(&snapshot).unwrap();
    let restored: ContextSnapshot = serde_json::from_str(&json).unwrap();

    // 验证恢复
    assert_eq!(restored.len(), 2, "恢复后应有 2 个 Source");
    let role = restored.get("system/role").unwrap();
    match role {
        SourceValue::Text(t) => assert_eq!(t, "角色"),
        _ => panic!("role 应为 Text 类型"),
    }
    let kb = restored.get("kb/summary").unwrap();
    assert_eq!(kb.as_count(), 5, "kb/summary 计数应为 5");
}

// ============================================================
// TC-CTX-008: Source 移除时调用 removed() 渲染移除消息
// ============================================================

#[test]
fn tc_ctx_008_removed_source_calls_removed() {
    let mut ctx = SystemContext::new();
    ctx.register(Box::new(MockSource::new_text("system/role", "角色。")));
    ctx.register(Box::new(
        MockSource::new_counter("kb/summary", 5).with_removed("[移除] 知识库源已注销".to_string()),
    ));

    let (_, snapshot) = ctx.initialize().unwrap();
    assert_eq!(snapshot.len(), 2);

    // 重新创建 ctx，只保留角色 Source（移除知识库 Source）
    let mut ctx2 = SystemContext::new();
    ctx2.register(Box::new(MockSource::new_text("system/role", "角色。")));

    let result = ctx2.reconcile(&snapshot).unwrap();
    match result {
        ReconcileResult::Updated {
            text,
            snapshot: new_snap,
        } => {
            // 应包含移除消息
            assert!(
                text.contains("[移除]"),
                "移除消息应包含 [移除]，实际: {}",
                text
            );
            // 新快照不应包含已移除的 Source
            assert_eq!(new_snap.len(), 1, "新快照应只有 1 个 Source");
            assert!(
                new_snap.get("kb/summary").is_none(),
                "kb/summary 应已被移除"
            );
        }
        _ => panic!("Source 移除时应返回 Updated"),
    }
}

// ============================================================
// 额外测试：SourceValue 序列化/反序列化
// ============================================================

#[test]
fn tc_ctx_extra_source_value_serde() {
    let text_val = SourceValue::Text("hello".to_string());
    let json = serde_json::to_string(&text_val).unwrap();
    let restored: SourceValue = serde_json::from_str(&json).unwrap();
    assert_eq!(text_val, restored);

    let json_val = SourceValue::Json(serde_json::json!({"key": "value", "num": 42}));
    let json_str = serde_json::to_string(&json_val).unwrap();
    let restored: SourceValue = serde_json::from_str(&json_str).unwrap();
    assert_eq!(json_val, restored);

    let unavail = SourceValue::Unavailable;
    let json_str = serde_json::to_string(&unavail).unwrap();
    let restored: SourceValue = serde_json::from_str(&json_str).unwrap();
    assert_eq!(unavail, restored);
}

// ============================================================
// 额外测试：内置 Source baseline 渲染
// ============================================================

#[test]
fn tc_ctx_extra_builtin_source_baseline() {
    // RoleSource
    let role = source::RoleSource::new("你是助手。".to_string());
    let val = role.load().unwrap();
    assert_eq!(role.baseline(&val), "你是助手。");

    // KnowledgeBaseSource
    let kb = source::KnowledgeBaseSource::new(
        42,
        vec![("法律".to_string(), 10), ("技术".to_string(), 32)],
    );
    let val = kb.load().unwrap();
    let baseline = kb.baseline(&val);
    assert!(baseline.contains("42"), "基线应包含文档数 42");

    // ConversationHistorySource
    let conv = source::ConversationHistorySource::new(10);
    let val = conv.load().unwrap();
    let baseline = conv.baseline(&val);
    assert!(baseline.contains("5"), "基线应包含轮次数 5（10/2）");

    // AgentModeSource
    let agent = source::AgentModeSource::new(true);
    let val = agent.load().unwrap();
    let baseline = agent.baseline(&val);
    assert!(baseline.contains("Agent"), "基线应包含 Agent");
    assert!(baseline.contains("启用"), "基线应包含「启用」");

    // AgentModeSource disabled
    let agent_off = source::AgentModeSource::new(false);
    let val = agent_off.load().unwrap();
    let baseline = agent_off.baseline(&val);
    assert!(baseline.is_empty(), "Agent 禁用时基线应为空");
}
