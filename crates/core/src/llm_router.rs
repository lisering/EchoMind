//! LLM 后端路由器（Q11 借鉴 QM HarnessRouter）。
//!
//! 借鉴 QM `harness/harness-router.ts` 的 `createHarnessRouter(adapters, utility, resolve)` 模式，
//! 为 EchoMind 引入 LLM 后端路由器，支持按对话或用户偏好动态切换 LLM 后端
//!（OpenAI / Local LLM / 未来新增后端），切换时通过 `RouterVerdict::ModeChanged`
//! 通知调用方重置会话状态（借鉴 QM `resetSession`）。
//!
//! # 设计约束
//!
//! 由于 `LLMProvider` trait 使用 `async fn in trait`（Edition 2024），
//! 不支持 `dyn` trait 对象，因此 Router **不持有 provider 实例**。
//! Router 仅负责路由决策（选择哪个模式 + 检测模式变更），
//! 实际 provider 创建由调用方（`chat_inner`）基于 `LlmChoice` 完成。
//!
//! # 并发安全
//!
//! 所有状态通过 `Arc<RwLock<...>>` 共享，可安全跨线程并发访问。

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use echomind_models::LlmMode;
use tokio::sync::RwLock;

/// LLM 选择结果（借鉴 QM `RuntimeChoice`）。
///
/// 描述一次 LLM 调用所选择的推理后端及模型标识。
/// `model_id` 语义因模式而异：
/// - `Remote`：模型名（如 `"gpt-4o-mini"`）
/// - `Local`：GGUF 文件名（如 `"qwen2.5-7b-instruct-q4_k_m.gguf"`）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmChoice {
    /// 推理模式（Remote / Local）
    pub mode: LlmMode,
    /// 模型标识（Remote=模型名, Local=GGUF 文件名）
    pub model_id: String,
}

impl LlmChoice {
    /// 创建新的 LLM 选择。
    pub fn new(mode: LlmMode, model_id: impl Into<String>) -> Self {
        Self {
            mode,
            model_id: model_id.into(),
        }
    }

    /// 创建 Remote 模式的选择（便捷构造函数）。
    pub fn remote(model_id: impl Into<String>) -> Self {
        Self::new(LlmMode::Remote, model_id)
    }

    // Local 模式已移除（Phase 1 R3 重构）
}

/// 路由裁决（借鉴 QM harness-router 的 resolve 返回值）。
///
/// 告知调用方本次路由结果相对于上次的变化情况，
/// 用于决定是否需要重置会话状态（如 KV cache、压缩基线等）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouterVerdict {
    /// 首次为此会话路由（无历史记录）
    Initial,
    /// 模式与上次相同（无需重置）
    SameMode,
    /// 模式发生变更（需要重置会话状态，借鉴 QM `resetSession`）
    ModeChanged,
}

/// 路由错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouterError {
    /// 请求的模式不可用（如 Free 版请求 Local 模式）
    ModeUnavailable(LlmMode),
}

impl std::fmt::Display for RouterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RouterError::ModeUnavailable(mode) => {
                write!(f, "请求的 LLM 模式不可用: {mode:?}")
            }
        }
    }
}

impl std::error::Error for RouterError {}

/// 连续远程 LLM 失败阈值。
///
/// 注：Local 模式已移除（Phase 1 R3），此常量保留但不再用于自动切换。
#[allow(dead_code)]
const REMOTE_FAILURE_THRESHOLD: usize = 3;

/// LLM 路由器（借鉴 QM `HarnessRouter`）。
///
/// 管理多个 LLM 后端的路由逻辑，按运行时配置和会话上下文动态选择推理后端。
/// 切换后端时通过 `RouterVerdict::ModeChanged` 通知调用方重置会话状态。
///
/// 注：Local 模式已移除（Phase 1 R3），路由器当前仅支持 Remote 模式。
/// P2-2 弹性降级到 Local 的功能已废弃。
///
/// # 使用方式
///
/// ```ignore
/// let router = LlmRouter::new_free_default();
/// let (choice, verdict) = router.resolve("conv-1", None).await?;
/// // 基于 choice.mode 创建 LlmProvider::Remote
/// if verdict == RouterVerdict::ModeChanged {
///     // 重置会话状态
/// }
/// ```
pub struct LlmRouter {
    /// 默认选择（当无显式请求时使用）
    fallback: Arc<RwLock<LlmChoice>>,
    /// 每会话上次使用的模式（借鉴 QM `lastHarness`）
    last_mode: Arc<RwLock<HashMap<String, LlmMode>>>,
    /// 可用模式集合（当前仅 `{Remote}`）
    available_modes: Arc<RwLock<HashSet<LlmMode>>>,
    /// 远程 LLM 连续失败计数（保留用于诊断，不再触发自动切换）
    remote_failure_count: Arc<std::sync::atomic::AtomicUsize>,
}

impl LlmRouter {
    /// 创建新的路由器。
    ///
    /// # 参数
    /// - `fallback`：默认 LLM 选择
    /// - `available_modes`：可用模式集合
    pub fn new(fallback: LlmChoice, available_modes: HashSet<LlmMode>) -> Self {
        Self {
            fallback: Arc::new(RwLock::new(fallback)),
            last_mode: Arc::new(RwLock::new(HashMap::new())),
            available_modes: Arc::new(RwLock::new(available_modes)),
            remote_failure_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    /// 创建默认路由器（Remote 模式，仅 Remote 可用——Free 版本配置）。
    pub fn new_free_default() -> Self {
        let mut modes = HashSet::new();
        modes.insert(LlmMode::Remote);
        Self::new(LlmChoice::new(LlmMode::Remote, String::new()), modes)
    }

    /// 创建 Pro 版路由器（仅 Remote 可用——Local 模式已移除）。
    pub fn new_pro_default() -> Self {
        Self::new_free_default()
    }

    /// 更新默认选择（`set_llm_mode` 调用时更新）。
    pub async fn set_fallback(&self, choice: LlmChoice) {
        let mut fb = self.fallback.write().await;
        *fb = choice;
    }

    /// 更新可用模式集合（Pro 激活/降级时调用）。
    pub async fn set_available_modes(&self, modes: HashSet<LlmMode>) {
        let mut am = self.available_modes.write().await;
        *am = modes;
    }

    /// 路由到正确的 LLM 后端（借鉴 QM harness-router 的 `resolve`）。
    ///
    /// # 参数
    /// - `conversation_id`：会话 ID（用于 per-conversation 模式追踪）
    /// - `requested`：显式请求的 LLM 选择（`None` 表示使用 fallback）
    ///
    /// # 返回
    /// - `Ok((choice, verdict))`：路由成功
    /// - `Err(RouterError::ModeUnavailable)`：请求的模式不在可用集合中
    ///
    /// # 副作用
    ///
    /// 成功时更新 `last_mode[conversation_id]` 为本次选择的模式。
    /// 失败时不修改任何状态。
    pub async fn resolve(
        &self,
        conversation_id: &str,
        requested: Option<LlmChoice>,
    ) -> Result<(LlmChoice, RouterVerdict), RouterError> {
        // 确定选择
        let choice = match requested {
            Some(c) => c,
            None => self.fallback.read().await.clone(),
        };

        // 校验可用性
        {
            let available = self.available_modes.read().await;
            if !available.contains(&choice.mode) {
                return Err(RouterError::ModeUnavailable(choice.mode));
            }
        }

        // 记录并判断模式变更
        let verdict = {
            let mut last = self.last_mode.write().await;
            let v = match last.get(conversation_id) {
                None => RouterVerdict::Initial,
                Some(prev) if *prev == choice.mode => RouterVerdict::SameMode,
                Some(_) => RouterVerdict::ModeChanged,
            };
            last.insert(conversation_id.to_string(), choice.mode);
            v
        };

        Ok((choice, verdict))
    }

    /// 查询指定会话的上次模式（不修改状态）。
    pub async fn last_mode_for(&self, conversation_id: &str) -> Option<LlmMode> {
        self.last_mode.read().await.get(conversation_id).copied()
    }

    /// 清除指定会话的模式记录（会话删除时调用）。
    pub async fn clear(&self, conversation_id: &str) {
        self.last_mode.write().await.remove(conversation_id);
    }

    /// 获取当前默认选择。
    pub async fn fallback(&self) -> LlmChoice {
        self.fallback.read().await.clone()
    }

    // ========================================================================
    // P2-2: 远程 LLM 连续失败 → 自动切换 Local 模式
    // ========================================================================

    /// 记录远程 LLM 成功调用，重置连续失败计数。
    ///
    /// 在 `forward_stream` 成功完成（`completed == true`）后调用。
    /// 使用 `Relaxed` 排序保证性能，因为这是一个统计计数器，不要求强一致性。
    pub fn record_remote_success(&self) {
        self.remote_failure_count
            .store(0, std::sync::atomic::Ordering::Relaxed);
    }

    /// 记录远程 LLM 失败调用。
    ///
    /// 注：Local 模式已移除（Phase 1 R3），自动切换功能废弃。
    /// 保留此方法以维持 API 兼容性，但不再触发任何自动切换。
    /// 计数器仍维护用于诊断目的。
    pub async fn record_remote_failure(&self) -> bool {
        let count = self
            .remote_failure_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            + 1;

        // Local 模式已移除，不再触发自动切换
        let _ = count;
        false
    }

    /// 查询当前远程 LLM 连续失败计数（主要用于测试和诊断）。
    pub fn remote_failure_count(&self) -> usize {
        self.remote_failure_count
            .load(std::sync::atomic::Ordering::Relaxed)
    }
}

impl Default for LlmRouter {
    fn default() -> Self {
        Self::new_free_default()
    }
}
