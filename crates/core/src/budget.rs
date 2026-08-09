use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

/// 成本估算（借鉴 QM estimateCostUsd）
pub fn estimate_cost_usd(input_tokens: usize, usd_per_mtok: f64) -> f64 {
    (input_tokens as f64 / 1_000_000.0) * usd_per_mtok
}

/// 预算检查结果
#[derive(Debug, Clone, PartialEq)]
pub enum BudgetCheck {
    Allowed,
    Denied { spent: f64, limit: f64 },
}

/// 速率限制检查结果
#[derive(Debug, Clone, PartialEq)]
pub enum RateLimitCheck {
    Allowed,
    Denied,
}

/// 按时间戳和金额的花费记录
#[derive(Debug, Clone)]
pub struct ExpenseRecord {
    timestamp: i64,
    amount: f64,
}

/// 预算追踪器（借鉴 QM BudgetTracker）
pub struct BudgetTracker {
    /// 每日预算上限（USD）
    daily_limit: f64,
    /// 已花费金额（按日重置）
    spent: Arc<RwLock<HashMap<String, Vec<ExpenseRecord>>>>,
}

impl BudgetTracker {
    /// 创建新的预算追踪器
    pub fn new(daily_limit: f64) -> Self {
        Self {
            daily_limit,
            spent: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 检查是否超预算
    pub async fn check(&self, principal: &str) -> BudgetCheck {
        if self.daily_limit <= 0.0 {
            return BudgetCheck::Allowed;
        }

        let now = current_timestamp();
        let one_day_ago = now - 86400; // 24 * 60 * 60

        let spent_map = self.spent.read().await;
        let principal_spent = spent_map.get(principal).map_or(0.0, |records| {
            records
                .iter()
                .filter(|record| record.timestamp > one_day_ago)
                .map(|record| record.amount)
                .sum()
        });

        if principal_spent >= self.daily_limit {
            BudgetCheck::Denied {
                spent: principal_spent,
                limit: self.daily_limit,
            }
        } else {
            BudgetCheck::Allowed
        }
    }

    /// 记录花费
    pub async fn record(&self, principal: &str, cost_usd: f64) {
        let now = current_timestamp();
        let mut spent_map = self.spent.write().await;

        let records = spent_map
            .entry(principal.to_string())
            .or_insert_with(Vec::new);
        records.push(ExpenseRecord {
            timestamp: now,
            amount: cost_usd,
        });

        // Clean up old records (older than 24 hours)
        let one_day_ago = now - 86400;
        records.retain(|record| record.timestamp > one_day_ago);
    }

    /// 获取预算统计
    pub async fn get_stats(&self, principal: &str) -> BudgetStats {
        let now = current_timestamp();
        let one_day_ago = now - 86400;

        let spent_map = self.spent.read().await;
        let principal_spent = spent_map.get(principal).map_or(0.0, |records| {
            records
                .iter()
                .filter(|record| record.timestamp > one_day_ago)
                .map(|record| record.amount)
                .sum()
        });

        BudgetStats {
            daily_limit: self.daily_limit,
            spent_today: principal_spent,
            remaining: if self.daily_limit > 0.0 {
                (self.daily_limit - principal_spent).max(0.0)
            } else {
                f64::INFINITY
            },
        }
    }

    /// 设置新的每日预算限制
    pub fn set_daily_limit(&mut self, limit: f64) {
        self.daily_limit = limit;
    }
}

/// 滑动窗口状态
#[derive(Debug, Clone)]
pub struct WindowState {
    requests: Vec<i64>, // millisecond timestamps
}

/// 速率限制器（借鉴 QM RateLimiter）
pub struct RateLimiter {
    max_per_window: u32,
    window: Duration,
    state: Arc<RwLock<HashMap<String, WindowState>>>,
}

impl RateLimiter {
    /// 创建新的速率限制器
    pub fn new(max_per_window: u32, window: Duration) -> Self {
        Self {
            max_per_window,
            window,
            state: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 检查请求是否允许
    pub async fn check(&self, principal: &str) -> RateLimitCheck {
        let now_ms = current_timestamp_ms();
        let window_start_ms = now_ms - self.window.as_millis() as i64;

        let mut state_map = self.state.write().await;
        let window_state = state_map
            .entry(principal.to_string())
            .or_insert_with(|| WindowState {
                requests: Vec::new(),
            });

        // Clean up old requests outside the window
        window_state
            .requests
            .retain(|&timestamp| timestamp > window_start_ms);

        if window_state.requests.len() >= self.max_per_window as usize {
            RateLimitCheck::Denied
        } else {
            window_state.requests.push(now_ms);
            RateLimitCheck::Allowed
        }
    }

    /// 重置指定主体的计数
    pub async fn reset(&self, principal: &str) {
        let mut state_map = self.state.write().await;
        state_map.remove(principal);
    }

    /// 获取速率限制统计
    pub async fn get_stats(&self, principal: &str) -> RateLimitStats {
        let now_ms = current_timestamp_ms();
        let window_start_ms = now_ms - self.window.as_millis() as i64;

        let state_map = self.state.read().await;
        let count = state_map.get(principal).map_or(0, |window_state| {
            window_state
                .requests
                .iter()
                .filter(|&&timestamp| timestamp > window_start_ms)
                .count()
        });

        RateLimitStats {
            max_per_window: self.max_per_window,
            current_count: count as u32,
            window_seconds: self.window.as_secs(),
        }
    }
}

use echomind_models::BudgetStats;
use echomind_models::RateLimitStats;

/// 获取当前时间戳（秒级精度，用于预算）
fn current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// 获取当前时间戳（毫秒级精度，用于速率限制）
fn current_timestamp_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_cost_usd() {
        assert_eq!(estimate_cost_usd(1_000_000, 1.0), 1.0);
        assert_eq!(estimate_cost_usd(500_000, 2.0), 1.0);
        assert_eq!(estimate_cost_usd(100_000, 10.0), 1.0);
    }
}
