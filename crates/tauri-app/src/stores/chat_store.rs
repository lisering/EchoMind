//! 会话管理 Store — 流中断令牌 + 审计取消（REQ-RAG-005, REQ-AUDIT-005）。
//!
//! 借鉴 Zed `bookmark_store`/`task_store` 的 Store 模式：
//! 会话级运行时状态封装在独立 Store 中。

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use echomind_infra::sqlite_storage::SqliteStorage;
use tokio_util::sync::CancellationToken;

/// 会话管理 Store（流中断令牌 + 审计取消标志）。
///
/// # 职责
/// - 每个会话的 `CancellationToken`（REQ-RAG-005；会话结束清理）
/// - 每个文档的审计取消标志（REQ-AUDIT-005；审计结束后清理）
///
/// # 线程安全
/// 内部使用 `tokio::sync::Mutex` 保护两个 HashMap。
pub struct ChatStore {
    /// 共享存储引用
    storage: Arc<SqliteStorage>,
    /// 每个会话的流中断令牌（REQ-RAG-005；会话结束清理）
    abort_tokens: tokio::sync::Mutex<HashMap<String, CancellationToken>>,
    /// 每个文档的审计取消标志（REQ-AUDIT-005；审计结束后清理）
    audit_cancels: tokio::sync::Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl ChatStore {
    /// 创建新的会话管理 Store。
    pub fn new(storage: Arc<SqliteStorage>) -> Self {
        Self {
            storage,
            abort_tokens: tokio::sync::Mutex::new(HashMap::new()),
            audit_cancels: tokio::sync::Mutex::new(HashMap::new()),
        }
    }

    /// 获取存储引用。
    pub fn storage(&self) -> &SqliteStorage {
        &self.storage
    }

    /// 获取（或创建）指定会话的中断令牌（REQ-RAG-005）。
    pub async fn abort_token_for(&self, conversation_id: &str) -> CancellationToken {
        self.abort_tokens
            .lock()
            .await
            .entry(conversation_id.to_string())
            .or_default()
            .clone()
    }

    /// 触发指定会话的中断（abort_chat 命令行为）。
    pub async fn abort_chat(&self, conversation_id: &str) {
        let token = self.abort_tokens.lock().await.get(conversation_id).cloned();
        if let Some(token) = token {
            token.cancel();
        }
    }

    /// 清理指定会话的中断令牌（对话结束或会话删除后调用）。
    pub async fn clear_abort(&self, conversation_id: &str) {
        self.abort_tokens.lock().await.remove(conversation_id);
    }

    /// 获取（或创建）指定文档的审计取消标志（REQ-AUDIT-005）。
    pub async fn audit_cancel_for(&self, doc_id: &str) -> Arc<AtomicBool> {
        self.audit_cancels
            .lock()
            .await
            .entry(doc_id.to_string())
            .or_insert_with(|| Arc::new(AtomicBool::new(false)))
            .clone()
    }

    /// 触发指定文档的审计取消（abort_audit 命令行为）。
    pub async fn abort_audit(&self, doc_id: &str) {
        let flag = self.audit_cancels.lock().await.get(doc_id).cloned();
        if let Some(flag) = flag {
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }

    /// 清理指定文档的审计取消标志（审计结束后调用）。
    pub async fn clear_audit_cancel(&self, doc_id: &str) {
        self.audit_cancels.lock().await.remove(doc_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_abort_token_lifecycle() {
        // 测试 CancellationToken 生命周期（不需要真实 DB）
        let token = CancellationToken::new();
        assert!(!token.is_cancelled());

        token.cancel();
        assert!(token.is_cancelled());
    }

    #[tokio::test]
    async fn test_atomic_bool_cancel() {
        let flag = Arc::new(AtomicBool::new(false));
        assert!(!flag.load(std::sync::atomic::Ordering::SeqCst));

        flag.store(true, std::sync::atomic::Ordering::SeqCst);
        assert!(flag.load(std::sync::atomic::Ordering::SeqCst));
    }
}
