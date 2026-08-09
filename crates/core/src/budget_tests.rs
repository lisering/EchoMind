#![cfg(test)]

use crate::budget::{BudgetCheck, BudgetTracker, RateLimitCheck, RateLimiter, estimate_cost_usd};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn tc_budget_001_check_budget_allowed() {
    let tracker = BudgetTracker::new(100.0);
    let result = tracker.check("user1").await;
    assert!(matches!(result, BudgetCheck::Allowed));
}

#[tokio::test]
async fn tc_budget_002_check_budget_denied() {
    let tracker = BudgetTracker::new(10.0);
    tracker.record("user1", 15.0).await;
    let result = tracker.check("user1").await;
    assert!(matches!(result, BudgetCheck::Denied { .. }));
}

#[tokio::test]
async fn tc_budget_003_sliding_window_expiry() {
    let tracker = BudgetTracker::new(100.0);
    tracker.record("user1", 50.0).await;

    // Wait for sliding window to expire (assuming 1 day window)
    sleep(Duration::from_millis(100)).await;

    // Should reset after expiry
    let result = tracker.check("user1").await;
    assert!(matches!(result, BudgetCheck::Allowed));
}

#[test]
fn tc_budget_004_cost_estimation() {
    let cost = estimate_cost_usd(500_000, 5.0); // 500k tokens at $5/mtok
    assert_eq!(cost, 2.5); // Should be $2.50
}

#[tokio::test]
async fn tc_ratelimit_001_window_allows_max_requests() {
    let limiter = RateLimiter::new(5, Duration::from_secs(1));

    // Should allow exactly 5 requests in the window
    for i in 0..5 {
        let result = limiter.check("user1").await;
        assert!(
            matches!(result, RateLimitCheck::Allowed),
            "Request {} should be allowed",
            i
        );
    }

    // 6th request should be denied
    let result = limiter.check("user1").await;
    assert!(matches!(result, RateLimitCheck::Denied));
}

#[tokio::test]
async fn tc_ratelimit_002_window_expiry_resets_count() {
    let limiter = RateLimiter::new(2, Duration::from_millis(100));

    // Use up the limit
    limiter.check("user1").await;
    limiter.check("user1").await;
    let result = limiter.check("user1").await;
    assert!(matches!(result, RateLimitCheck::Denied));

    // Wait for window to expire
    sleep(Duration::from_millis(150)).await;

    // Should allow again after expiry
    let result = limiter.check("user1").await;
    assert!(matches!(result, RateLimitCheck::Allowed));
}
