//! Coordinator 策略注册表（DH-03 借鉴 DeepSeek Harness SubagentRuntime Provider 注册表）。
//!
//! 借鉴 DeepSeek Harness 的 `SubagentRuntime` 设计：多个 Provider 共存，
//! 按名字查找，能力验证在执行前完成。
//!
//! ## 设计
//!
//! 由于 Rust 的 `async fn` in trait 不支持 `dyn`，且 `Retriever`/`LLMProvider`
//! trait 的 async 方法返回 `Future`（非 `dyn` compatible），本模块采用
//! **策略描述 + 引擎分发** 模式：
//!
//! 1. `CoordinatorStrategyDesc` — 策略描述（name + config + capabilities），可 `dyn`
//! 2. `CoordinatorStrategyRegistry` — 命名策略注册表
//! 3. `DefaultCoordinatorStrategy` — 默认四阶段策略描述
//! 4. 调用方根据描述中的 `strategy_type` 选择对应的 `CoordinatorEngine` 配置
//!
//! 这样策略注册表是 `dyn` compatible 的，而实际执行仍由泛型 `CoordinatorEngine<R, L>` 完成。
//!
//! ## 与 DeepSeek Harness 的对应关系
//!
//! | DeepSeek Harness | EchoMind |
//! |---|---|
//! | `SubagentRuntime` | `CoordinatorStrategyRegistry` |
//! | `SubagentProvider` | `CoordinatorStrategyDesc` |
//! | `SubagentProvider.name` | `CoordinatorStrategyDesc::name()` |
//! | `SubagentRuntime.start()` | 调用方根据描述构建 CoordinatorEngine |
//! | `assertCapabilities()` | `supports()` 能力检查 |

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Coordinator 策略能力标志（借鉴 SubagentCapabilities）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CoordinatorCapabilities {
    /// 是否支持子查询分解
    pub sub_query_decomposition: bool,
    /// 是否支持并行检索
    pub parallel_retrieval: bool,
    /// 是否支持综合分析
    pub synthesis: bool,
    /// 是否支持流式生成
    pub streaming_generation: bool,
    /// 是否支持子代理舰队
    pub sub_agent_fleet: bool,
}

/// 策略类型标识（调用方根据类型构建对应的 CoordinatorEngine 配置）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StrategyType {
    /// 默认四阶段流水线
    Default,
    /// 子代理舰队模式
    SubAgentFleet,
    /// 单轮直接检索（无分解）
    Direct,
}

/// Coordinator 策略描述（dyn compatible）。
///
/// 策略不直接执行（因 async trait 不支持 dyn），而是提供配置参数。
/// 调用方根据 `strategy_type` 选择对应的 `CoordinatorEngine` 构建方式。
pub trait CoordinatorStrategyDesc: Send + Sync {
    /// 策略名（唯一标识，用于注册表查找）。
    fn name(&self) -> &str;

    /// 策略类型（决定调用方构建哪个引擎配置）。
    fn strategy_type(&self) -> StrategyType;

    /// 声明策略支持的能力。
    fn capabilities(&self) -> CoordinatorCapabilities;

    /// 最大并行 worker 数。
    fn max_workers(&self) -> usize;

    /// 每个 worker 检索 top-k。
    fn research_top_k(&self) -> usize;

    /// 子代理超时秒数（None = 不启用子代理）。
    fn sub_agent_timeout(&self) -> Option<u64>;

    /// 检查策略是否支持请求的能力。
    fn supports(&self, request_caps: &CoordinatorCapabilities) -> bool {
        let caps = self.capabilities();
        (!request_caps.sub_query_decomposition || caps.sub_query_decomposition)
            && (!request_caps.parallel_retrieval || caps.parallel_retrieval)
            && (!request_caps.synthesis || caps.synthesis)
            && (!request_caps.streaming_generation || caps.streaming_generation)
            && (!request_caps.sub_agent_fleet || caps.sub_agent_fleet)
    }
}

/// 默认四阶段策略描述。
#[derive(Debug, Clone)]
pub struct DefaultCoordinatorStrategy {
    max_workers: usize,
    research_top_k: usize,
    sub_agent_timeout: Option<u64>,
}

impl DefaultCoordinatorStrategy {
    /// 创建默认策略。
    pub fn new() -> Self {
        Self {
            max_workers: 3,
            research_top_k: 5,
            sub_agent_timeout: None,
        }
    }

    /// 启用子代理舰队模式。
    pub fn with_sub_agent_timeout(mut self, secs: u64) -> Self {
        self.sub_agent_timeout = Some(secs);
        self
    }
}

impl Default for DefaultCoordinatorStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl CoordinatorStrategyDesc for DefaultCoordinatorStrategy {
    fn name(&self) -> &str {
        "default"
    }

    fn strategy_type(&self) -> StrategyType {
        if self.sub_agent_timeout.is_some() {
            StrategyType::SubAgentFleet
        } else {
            StrategyType::Default
        }
    }

    fn capabilities(&self) -> CoordinatorCapabilities {
        CoordinatorCapabilities {
            sub_query_decomposition: true,
            parallel_retrieval: true,
            synthesis: true,
            streaming_generation: true,
            sub_agent_fleet: self.sub_agent_timeout.is_some(),
        }
    }

    fn max_workers(&self) -> usize {
        self.max_workers
    }

    fn research_top_k(&self) -> usize {
        self.research_top_k
    }

    fn sub_agent_timeout(&self) -> Option<u64> {
        self.sub_agent_timeout
    }
}

/// 直接检索策略描述（无子查询分解，单轮直接检索）。
pub struct DirectStrategy {
    max_workers: usize,
    research_top_k: usize,
}

impl DirectStrategy {
    pub fn new() -> Self {
        Self {
            max_workers: 1,
            research_top_k: 5,
        }
    }
}

impl Default for DirectStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl CoordinatorStrategyDesc for DirectStrategy {
    fn name(&self) -> &str {
        "direct"
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::Direct
    }

    fn capabilities(&self) -> CoordinatorCapabilities {
        CoordinatorCapabilities {
            sub_query_decomposition: false,
            parallel_retrieval: false,
            synthesis: false,
            streaming_generation: true,
            sub_agent_fleet: false,
        }
    }

    fn max_workers(&self) -> usize {
        self.max_workers
    }

    fn research_top_k(&self) -> usize {
        self.research_top_k
    }

    fn sub_agent_timeout(&self) -> Option<u64> {
        None
    }
}

/// 策略注册表错误。
#[derive(Debug, Clone)]
pub enum StrategyRegistryError {
    /// 重复注册同名策略
    Duplicate(String),
    /// 未找到策略
    NotFound(String),
    /// 策略不支持请求的能力
    Unsupported(String),
}

impl std::fmt::Display for StrategyRegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Duplicate(name) => {
                write!(
                    f,
                    "a coordinator strategy named \"{name}\" is already registered"
                )
            }
            Self::NotFound(name) => {
                write!(f, "no coordinator strategy registered for \"{name}\"")
            }
            Self::Unsupported(name) => {
                write!(
                    f,
                    "coordinator strategy \"{name}\" does not support the requested capabilities"
                )
            }
        }
    }
}

impl std::error::Error for StrategyRegistryError {}

/// Coordinator 策略注册表（借鉴 SubagentRuntime）。
///
/// 多个策略共存，按名字查找。注册是线程安全的（内部 RwLock）。
/// 默认策略 "default" 和 "direct" 在创建时自动注册。
pub struct CoordinatorStrategyRegistry {
    strategies: RwLock<HashMap<String, Arc<dyn CoordinatorStrategyDesc>>>,
}

impl CoordinatorStrategyRegistry {
    /// 创建注册表并注册默认策略。
    pub fn new() -> Self {
        let mut map = HashMap::new();
        map.insert(
            "default".to_string(),
            Arc::new(DefaultCoordinatorStrategy::new()) as Arc<dyn CoordinatorStrategyDesc>,
        );
        map.insert(
            "direct".to_string(),
            Arc::new(DirectStrategy::new()) as Arc<dyn CoordinatorStrategyDesc>,
        );
        Self {
            strategies: RwLock::new(map),
        }
    }

    /// 注册策略。
    pub fn register(
        &self,
        strategy: Arc<dyn CoordinatorStrategyDesc>,
    ) -> Result<(), StrategyRegistryError> {
        let name = strategy.name().to_string();
        let mut map = self
            .strategies
            .write()
            .map_err(|_| StrategyRegistryError::Duplicate(name.clone()))?;
        if map.contains_key(&name) {
            return Err(StrategyRegistryError::Duplicate(name));
        }
        map.insert(name, strategy);
        Ok(())
    }

    /// 注销策略。
    pub fn unregister(&self, name: &str) -> Option<Arc<dyn CoordinatorStrategyDesc>> {
        self.strategies
            .write()
            .ok()
            .and_then(|mut map| map.remove(name))
    }

    /// 按名字查找策略。
    pub fn get(&self, name: &str) -> Option<Arc<dyn CoordinatorStrategyDesc>> {
        self.strategies.read().ok()?.get(name).cloned()
    }

    /// 列出所有注册的策略名。
    pub fn list(&self) -> Vec<String> {
        self.strategies
            .read()
            .map(|map| map.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// 查找策略并验证能力。
    ///
    /// 借鉴 SubagentRuntime.start() 的前置验证流程：
    /// 1. 查找 Provider
    /// 2. 能力验证
    pub fn resolve(
        &self,
        name: &str,
        required_caps: &CoordinatorCapabilities,
    ) -> Result<Arc<dyn CoordinatorStrategyDesc>, StrategyRegistryError> {
        let strategy = self
            .get(name)
            .ok_or_else(|| StrategyRegistryError::NotFound(name.to_string()))?;

        if !strategy.supports(required_caps) {
            return Err(StrategyRegistryError::Unsupported(name.to_string()));
        }

        Ok(strategy)
    }
}

impl Default for CoordinatorStrategyRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_default_strategy_registered() {
        let registry = CoordinatorStrategyRegistry::new();
        assert!(registry.get("default").is_some());
        assert!(registry.get("direct").is_some());
        assert!(registry.list().contains(&"default".to_string()));
        assert!(registry.list().contains(&"direct".to_string()));
    }

    #[test]
    fn test_registry_duplicate_registration() {
        let registry = CoordinatorStrategyRegistry::new();
        let strategy: Arc<dyn CoordinatorStrategyDesc> =
            Arc::new(DefaultCoordinatorStrategy::new());
        let result = registry.register(strategy);
        assert!(result.is_err());
    }

    #[test]
    fn test_registry_unregister() {
        let registry = CoordinatorStrategyRegistry::new();
        let removed = registry.unregister("default");
        assert!(removed.is_some());
        assert!(registry.get("default").is_none());
        // direct should still be there
        assert!(registry.get("direct").is_some());
    }

    #[test]
    fn test_default_capabilities() {
        let strategy = DefaultCoordinatorStrategy::new();
        let caps = strategy.capabilities();
        assert!(caps.sub_query_decomposition);
        assert!(caps.parallel_retrieval);
        assert!(caps.synthesis);
        assert!(caps.streaming_generation);
        assert!(!caps.sub_agent_fleet);
    }

    #[test]
    fn test_default_capabilities_with_sub_agent() {
        let strategy = DefaultCoordinatorStrategy::new().with_sub_agent_timeout(60);
        let caps = strategy.capabilities();
        assert!(caps.sub_agent_fleet);
        assert_eq!(strategy.strategy_type(), StrategyType::SubAgentFleet);
    }

    #[test]
    fn test_direct_strategy_capabilities() {
        let strategy = DirectStrategy::new();
        let caps = strategy.capabilities();
        assert!(!caps.sub_query_decomposition);
        assert!(!caps.parallel_retrieval);
        assert!(!caps.synthesis);
        assert!(caps.streaming_generation);
        assert!(!caps.sub_agent_fleet);
        assert_eq!(strategy.strategy_type(), StrategyType::Direct);
    }

    #[test]
    fn test_supports_all_capabilities() {
        let strategy = DefaultCoordinatorStrategy::new();
        let required = CoordinatorCapabilities {
            sub_query_decomposition: true,
            parallel_retrieval: true,
            synthesis: true,
            streaming_generation: true,
            sub_agent_fleet: false,
        };
        assert!(strategy.supports(&required));
    }

    #[test]
    fn test_supports_fails_for_unsupported_capability() {
        let strategy = DefaultCoordinatorStrategy::new();
        let required = CoordinatorCapabilities {
            sub_query_decomposition: true,
            parallel_retrieval: true,
            synthesis: true,
            streaming_generation: true,
            sub_agent_fleet: true,
        };
        assert!(!strategy.supports(&required));
    }

    #[test]
    fn test_resolve_success() {
        let registry = CoordinatorStrategyRegistry::new();
        let caps = CoordinatorCapabilities {
            sub_query_decomposition: true,
            parallel_retrieval: true,
            synthesis: true,
            streaming_generation: true,
            sub_agent_fleet: false,
        };
        let strategy = registry.resolve("default", &caps);
        assert!(strategy.is_ok());
        if let Ok(s) = strategy {
            assert_eq!(s.name(), "default");
        }
    }

    #[test]
    fn test_resolve_not_found() {
        let registry = CoordinatorStrategyRegistry::new();
        let caps = CoordinatorCapabilities::default();
        let result = registry.resolve("nonexistent", &caps);
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_unsupported_capability() {
        let registry = CoordinatorStrategyRegistry::new();
        let caps = CoordinatorCapabilities {
            sub_agent_fleet: true,
            ..Default::default()
        };
        let result = registry.resolve("direct", &caps);
        assert!(result.is_err());
    }

    #[test]
    fn test_default_strategy_config() {
        let strategy = DefaultCoordinatorStrategy::new();
        assert_eq!(strategy.max_workers(), 3);
        assert_eq!(strategy.research_top_k(), 5);
        assert_eq!(strategy.sub_agent_timeout(), None);
    }
}
