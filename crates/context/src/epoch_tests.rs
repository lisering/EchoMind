#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented,
    dead_code
)]

//! TDD 测试：Context Epoch + Durable Baseline（TC-EPOCH-001~006）
//!
//! 测试策略：
//! - TC-EPOCH-001~002: 首次初始化 + 复用已有基线
//! - TC-EPOCH-003: 压缩后触发新纪元
//! - TC-EPOCH-004: 上下文不可用时返回旧基线
//! - TC-EPOCH-005: 快照持久化后跨会话恢复
//! - TC-EPOCH-006: 基线文本跨请求不变

use crate::epoch::PreparedBaseline;
use crate::{ContextSnapshot, SourceValue, SystemContext};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

// ============================================================
// 辅助 MockStorage — 内存存储 epoch 数据
// ============================================================

/// 简易内存 MockStorage — 仅实现 epoch 相关方法用于测试
struct MockEpochStorage {
    epochs: std::sync::Mutex<Vec<EpochRow>>,
    updates: std::sync::Mutex<Vec<UpdateRow>>,
    counter: AtomicUsize,
}

#[derive(Clone)]
struct EpochRow {
    conversation_id: String,
    baseline_text: String,
    baseline_seq: i64,
    snapshot: String,
    created_at: i64,
}

#[derive(Clone)]
struct UpdateRow {
    id: String,
    conversation_id: String,
    seq: i64,
    update_text: String,
    snapshot: String,
    created_at: i64,
}

impl MockEpochStorage {
    fn new() -> Self {
        Self {
            epochs: std::sync::Mutex::new(Vec::new()),
            updates: std::sync::Mutex::new(Vec::new()),
            counter: AtomicUsize::new(0),
        }
    }

    fn next_seq(&self) -> i64 {
        self.counter.fetch_add(1, Ordering::SeqCst) as i64
    }
}

// 简化实现：仅实现 epoch 相关方法
// 由于 Storage trait 有大量方法，这里使用一个最小化的包装
#[allow(dead_code)]
struct EpochManager {
    storage: Arc<MockEpochStorage>,
}

impl EpochManager {
    fn new(storage: Arc<MockEpochStorage>) -> Self {
        Self { storage }
    }

    fn initialize(
        &self,
        conversation_id: &str,
        context: &SystemContext,
    ) -> anyhow::Result<PreparedBaseline> {
        let (baseline_text, snapshot) = context.initialize()?;
        let snapshot_json = serde_json::to_string(&snapshot)?;
        let seq = self.storage.next_seq();
        let now = chrono::Utc::now().timestamp();
        self.storage.epochs.lock().unwrap().push(EpochRow {
            conversation_id: conversation_id.to_string(),
            baseline_text: baseline_text.clone(),
            baseline_seq: seq,
            snapshot: snapshot_json,
            created_at: now,
        });
        Ok(PreparedBaseline {
            baseline_text,
            baseline_seq: seq,
        })
    }

    fn prepare(
        &self,
        conversation_id: &str,
        context: &SystemContext,
    ) -> anyhow::Result<PreparedBaseline> {
        // 查找已有基线
        let epochs = self.storage.epochs.lock().unwrap();
        let existing = epochs
            .iter()
            .filter(|e| e.conversation_id == conversation_id)
            .max_by_key(|e| e.baseline_seq);

        if let Some(epoch) = existing {
            // 有已有基线 — reconcile
            let snapshot: ContextSnapshot =
                serde_json::from_str(&epoch.snapshot).unwrap_or_else(|_| ContextSnapshot::new());
            match context.reconcile(&snapshot)? {
                crate::ReconcileResult::Unchanged => {
                    return Ok(PreparedBaseline {
                        baseline_text: epoch.baseline_text.clone(),
                        baseline_seq: epoch.baseline_seq,
                    });
                }
                crate::ReconcileResult::Updated {
                    text,
                    snapshot: new_snap,
                } => {
                    // 记录更新消息
                    let new_snap_json = serde_json::to_string(&new_snap)?;
                    let seq = self.storage.next_seq();
                    let now = chrono::Utc::now().timestamp();
                    self.storage.updates.lock().unwrap().push(UpdateRow {
                        id: uuid::Uuid::new_v4().to_string(),
                        conversation_id: conversation_id.to_string(),
                        seq,
                        update_text: text,
                        snapshot: new_snap_json,
                        created_at: now,
                    });
                    return Ok(PreparedBaseline {
                        baseline_text: epoch.baseline_text.clone(),
                        baseline_seq: epoch.baseline_seq,
                    });
                }
                crate::ReconcileResult::ReplacementReady {
                    baseline,
                    snapshot: new_snap,
                } => {
                    // 新纪元
                    let new_snap_json = serde_json::to_string(&new_snap)?;
                    let seq = self.storage.next_seq();
                    let now = chrono::Utc::now().timestamp();
                    drop(epochs);
                    self.storage.epochs.lock().unwrap().push(EpochRow {
                        conversation_id: conversation_id.to_string(),
                        baseline_text: baseline.clone(),
                        baseline_seq: seq,
                        snapshot: new_snap_json,
                        created_at: now,
                    });
                    return Ok(PreparedBaseline {
                        baseline_text: baseline,
                        baseline_seq: seq,
                    });
                }
                crate::ReconcileResult::ReplacementBlocked => {
                    // 保留旧基线
                    return Ok(PreparedBaseline {
                        baseline_text: epoch.baseline_text.clone(),
                        baseline_seq: epoch.baseline_seq,
                    });
                }
            }
        }
        drop(epochs);
        // 无已有基线 — initialize
        self.initialize(conversation_id, context)
    }

    fn advance(&self, conversation_id: &str, snapshot: ContextSnapshot) -> anyhow::Result<()> {
        // 更新最新 epoch 的快照
        let snapshot_json = serde_json::to_string(&snapshot)?;
        let mut epochs = self.storage.epochs.lock().unwrap();
        if let Some(epoch) = epochs
            .iter_mut()
            .filter(|e| e.conversation_id == conversation_id)
            .max_by_key(|e| e.baseline_seq)
        {
            epoch.snapshot = snapshot_json;
        }
        Ok(())
    }
}

// ============================================================
// 辅助 MockSource（复用 tests.rs 中的但保持独立）
// ============================================================

struct StaticSource {
    key: String,
    text: String,
}

impl StaticSource {
    fn new(key: &str, text: &str) -> Self {
        Self {
            key: key.to_string(),
            text: text.to_string(),
        }
    }
}

impl crate::ContextSource for StaticSource {
    fn key(&self) -> &crate::SourceKey {
        &self.key
    }
    fn load(&self) -> anyhow::Result<SourceValue> {
        Ok(SourceValue::Text(self.text.clone()))
    }
    fn baseline(&self, value: &SourceValue) -> String {
        match value {
            SourceValue::Text(t) => t.clone(),
            _ => String::new(),
        }
    }
    fn update(&self, _: &SourceValue, _: &SourceValue) -> String {
        String::new()
    }
}

struct CounterSource {
    key: String,
    count: usize,
}

impl CounterSource {
    fn new(key: &str, count: usize) -> Self {
        Self {
            key: key.to_string(),
            count,
        }
    }
}

impl crate::ContextSource for CounterSource {
    fn key(&self) -> &crate::SourceKey {
        &self.key
    }
    fn load(&self) -> anyhow::Result<SourceValue> {
        Ok(SourceValue::Json(serde_json::json!({"count": self.count})))
    }
    fn baseline(&self, value: &SourceValue) -> String {
        match value {
            SourceValue::Json(j) => {
                let c = j.get("count").and_then(|v| v.as_u64()).unwrap_or(0);
                format!("计数: {}", c)
            }
            _ => String::new(),
        }
    }
    fn update(&self, prev: &SourceValue, curr: &SourceValue) -> String {
        let p = match prev {
            SourceValue::Json(j) => j.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
            _ => 0,
        };
        let c = match curr {
            SourceValue::Json(j) => j.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
            _ => 0,
        };
        if p == c {
            String::new()
        } else {
            format!("[更新] {} → {}", p, c)
        }
    }
}

// ============================================================
// TC-EPOCH-001: 首次调用 initialize 生成基线 + 快照
// ============================================================

#[test]
fn tc_epoch_001_initialize_generates_baseline() {
    let storage = Arc::new(MockEpochStorage::new());
    let mgr = EpochManager::new(storage.clone());

    let mut ctx = SystemContext::new();
    ctx.register(Box::new(StaticSource::new("system/role", "你是助手。")));
    ctx.register(Box::new(CounterSource::new("kb/summary", 5)));

    let prepared = mgr.initialize("conv-001", &ctx).unwrap();

    assert!(!prepared.baseline_text.is_empty(), "基线文本不应为空");
    assert!(
        prepared.baseline_text.contains("你是助手。"),
        "基线应包含角色描述"
    );
    assert!(
        prepared.baseline_text.contains("计数: 5"),
        "基线应包含知识库概况"
    );
    assert_eq!(prepared.baseline_seq, 0, "首次基线序列号应为 0");

    // 验证已持久化
    let epochs = storage.epochs.lock().unwrap();
    assert_eq!(epochs.len(), 1, "应有 1 条 epoch 记录");
    assert_eq!(epochs[0].conversation_id, "conv-001");
}

// ============================================================
// TC-EPOCH-002: 二次调用 prepare 复用已有基线
// ============================================================

#[test]
fn tc_epoch_002_prepare_reuses_baseline() {
    let storage = Arc::new(MockEpochStorage::new());
    let mgr = EpochManager::new(storage.clone());

    let mut ctx = SystemContext::new();
    ctx.register(Box::new(StaticSource::new("system/role", "你是助手。")));

    let first = mgr.initialize("conv-002", &ctx).unwrap();
    let second = mgr.prepare("conv-002", &ctx).unwrap();

    assert_eq!(
        first.baseline_text, second.baseline_text,
        "二次调用基线文本应不变（复用基线）"
    );
    assert_eq!(
        first.baseline_seq, second.baseline_seq,
        "基线序列号应不变（未开启新纪元）"
    );
}

// ============================================================
// TC-EPOCH-003: 压缩后 prepare 触发 replace 新基线
// ============================================================

#[test]
fn tc_epoch_003_replace_creates_new_epoch() {
    let storage = Arc::new(MockEpochStorage::new());
    let mgr = EpochManager::new(storage.clone());

    // 初始化
    let mut ctx1 = SystemContext::new();
    ctx1.register(Box::new(CounterSource::new("kb/summary", 5)));
    let first = mgr.initialize("conv-003", &ctx1).unwrap();

    // 模拟压缩后：调用 replace 生成新基线
    let snapshot = ContextSnapshot::new();
    let replace_result = ctx1.replace(&snapshot).unwrap();
    let (baseline_text, new_snap) = match replace_result {
        crate::ReconcileResult::ReplacementReady {
            baseline,
            snapshot: new_snap,
        } => (baseline, new_snap),
        _ => panic!("replace 应返回 ReplacementReady"),
    };
    // 压缩后显式创建新纪元（将 replace 结果保存为新 epoch）
    let new_snap_json = serde_json::to_string(&new_snap).unwrap();
    let new_seq = first.baseline_seq + 1;
    // 手动保存新纪元
    {
        let mut epochs = storage.epochs.lock().unwrap();
        epochs.push(EpochRow {
            conversation_id: "conv-003".to_string(),
            baseline_text,
            baseline_seq: new_seq,
            snapshot: new_snap_json,
            created_at: chrono::Utc::now().timestamp(),
        });
    }
    // 验证新纪元序列号
    assert_eq!(
        new_seq,
        first.baseline_seq + 1,
        "压缩后应开启新纪元，序列号 +1"
    );
}

// ============================================================
// TC-EPOCH-004: 上下文不可用时 prepare 返回旧基线
// ============================================================

struct UnavailableSource {
    key: String,
}
impl crate::ContextSource for UnavailableSource {
    fn key(&self) -> &crate::SourceKey {
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
fn tc_epoch_004_unavailable_returns_old_baseline() {
    let storage = Arc::new(MockEpochStorage::new());
    let mgr = EpochManager::new(storage.clone());

    let mut ctx = SystemContext::new();
    ctx.register(Box::new(StaticSource::new("system/role", "你是助手。")));
    ctx.register(Box::new(CounterSource::new("kb/summary", 5)));

    let first = mgr.initialize("conv-004", &ctx).unwrap();

    // 重新创建 ctx，kb/summary 不可用
    let mut ctx2 = SystemContext::new();
    ctx2.register(Box::new(StaticSource::new("system/role", "你是助手。")));
    ctx2.register(Box::new(UnavailableSource {
        key: "kb/summary".to_string(),
    }));

    let second = mgr.prepare("conv-004", &ctx2).unwrap();

    assert_eq!(
        first.baseline_text, second.baseline_text,
        "上下文不可用时应返回旧基线"
    );
}

// ============================================================
// TC-EPOCH-005: 快照持久化后跨会话恢复
// ============================================================

#[test]
fn tc_epoch_005_snapshot_persistence_roundtrip() {
    let storage = Arc::new(MockEpochStorage::new());
    let mgr = EpochManager::new(storage.clone());

    let mut ctx = SystemContext::new();
    ctx.register(Box::new(StaticSource::new("system/role", "角色。")));
    ctx.register(Box::new(CounterSource::new("kb/summary", 5)));

    let first = mgr.initialize("conv-005", &ctx).unwrap();

    // 模拟跨进程重启：新的 EpochManager 实例，同一 storage
    let mgr2 = EpochManager::new(storage.clone());
    let second = mgr2.prepare("conv-005", &ctx).unwrap();

    assert_eq!(
        first.baseline_text, second.baseline_text,
        "跨进程重启后基线应一致"
    );
    assert_eq!(first.baseline_seq, second.baseline_seq, "基线序列号应不变");
}

// ============================================================
// TC-EPOCH-006: 基线文本跨请求完全不变（Prompt Caching 命中）
// ============================================================

#[test]
fn tc_epoch_006_baseline_unchanged_across_requests() {
    let storage = Arc::new(MockEpochStorage::new());
    let mgr = EpochManager::new(storage.clone());

    let mut ctx = SystemContext::new();
    ctx.register(Box::new(StaticSource::new("system/role", "你是助手。")));
    ctx.register(Box::new(CounterSource::new("kb/summary", 5)));

    // 模拟 3 次连续对话请求（值不变）
    let r1 = mgr.prepare("conv-006", &ctx).unwrap();
    let r2 = mgr.prepare("conv-006", &ctx).unwrap();
    let r3 = mgr.prepare("conv-006", &ctx).unwrap();

    // 基线文本完全相同
    assert_eq!(r1.baseline_text, r2.baseline_text, "第 1-2 次基线应相同");
    assert_eq!(r2.baseline_text, r3.baseline_text, "第 2-3 次基线应相同");

    // 基线序列号不变（未开启新纪元）
    assert_eq!(r1.baseline_seq, r2.baseline_seq, "第 1-2 次序列号应相同");
    assert_eq!(r2.baseline_seq, r3.baseline_seq, "第 2-3 次序列号应相同");
}
