//! 系统化性能基准测试框架（借鉴 Zed `util_macros::perf`/`cargo perf-test`）。
//!
//! 提供 `PerfReport` 结构体和 `PerfRunner` 测试运行器，
//! 统一管理性能测试的计时、吞吐量计算和结果输出。
//!
//! # 使用方式
//! ```rust,ignore
//! use echomind_infra::perf::{PerfRunner, PerfReport};
//!
//! #[tokio::test]
//! #[ignore] // 性能测试默认不运行，需 --ignored 显式触发
//! async fn perf_embedding_throughput() {
//!     let report = PerfRunner::new("embedding_single")
//!         .run_async(|| async {
//!             embedder.embed("test text").await
//!         })
//!         .await;
//!     report.assert_throughput(100.0); // 至少 100 ops/sec
//! }
//! ```

use std::time::{Duration, Instant};

/// 性能测试报告。
///
/// 记录操作名、耗时、迭代次数和吞吐量。
#[derive(Debug, Clone)]
pub struct PerfReport {
    /// 操作名称（如 "embedding_single"）
    pub name: String,
    /// 总耗时（纳秒）
    pub elapsed_ns: u128,
    /// 迭代次数
    pub iterations: usize,
    /// 单次操作平均耗时（微秒）
    pub avg_us: f64,
    /// 吞吐量（ops/sec）
    pub throughput: f64,
}

impl PerfReport {
    /// 从耗时和迭代次数创建报告。
    pub fn new(name: &str, elapsed: Duration, iterations: usize) -> Self {
        let elapsed_ns = elapsed.as_nanos();
        let avg_us = if iterations > 0 {
            (elapsed_ns as f64 / 1000.0) / iterations as f64
        } else {
            0.0
        };
        let throughput = if elapsed_ns > 0 {
            (iterations as f64 * 1_000_000_000.0) / elapsed_ns as f64
        } else {
            0.0
        };
        Self {
            name: name.to_string(),
            elapsed_ns,
            iterations,
            avg_us,
            throughput,
        }
    }

    /// 断言吞吐量不低于指定值（ops/sec）。
    ///
    /// 失败时打印详细报告。
    pub fn assert_throughput(&self, min_ops_per_sec: f64) {
        assert!(
            self.throughput >= min_ops_per_sec,
            "性能基准不达标: {} 吞吐量 {:.2} ops/sec < 期望 {:.2} ops/sec\n\
             耗时: {:.2}ms, 迭代: {}, 平均: {:.2}μs/op",
            self.name,
            self.throughput,
            min_ops_per_sec,
            self.elapsed_ns as f64 / 1_000_000.0,
            self.iterations,
            self.avg_us,
        );
    }

    /// 断言平均延迟不高于指定值（微秒）。
    pub fn assert_latency_us(&self, max_us: f64) {
        assert!(
            self.avg_us <= max_us,
            "延迟基准不达标: {} 平均延迟 {:.2}μs > 期望 {:.2}μs\n\
             耗时: {:.2}ms, 迭代: {}, 吞吐量: {:.2} ops/sec",
            self.name,
            self.avg_us,
            max_us,
            self.elapsed_ns as f64 / 1_000_000.0,
            self.iterations,
            self.throughput,
        );
    }

    /// 打印报告到标准输出。
    pub fn print(&self) {
        println!(
            "[PERF] {}: {:.2}ms ({} iterations, {:.2}μs/op, {:.2} ops/sec)",
            self.name,
            self.elapsed_ns as f64 / 1_000_000.0,
            self.iterations,
            self.avg_us,
            self.throughput,
        );
    }
}

/// 性能测试运行器。
///
/// 提供同步和异步两种运行模式，自动计时并生成报告。
pub struct PerfRunner {
    name: String,
    warmup: usize,
}

impl PerfRunner {
    /// 创建新的性能测试运行器。
    ///
    /// # 参数
    /// - `name` — 测试名称（用于报告标识）
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            warmup: 0,
        }
    }

    /// 设置预热迭代次数（不计入计时）。
    pub fn with_warmup(mut self, warmup: usize) -> Self {
        self.warmup = warmup;
        self
    }

    /// 运行同步性能测试。
    ///
    /// 返回 `PerfReport`，包含耗时、吞吐量等信息。
    pub fn run<F, R>(&self, iterations: usize, mut f: F) -> PerfReport
    where
        F: FnMut() -> R,
    {
        // 预热
        for _ in 0..self.warmup {
            let _ = f();
        }

        let start = Instant::now();
        for _ in 0..iterations {
            let _ = f();
        }
        let elapsed = start.elapsed();

        PerfReport::new(&self.name, elapsed, iterations)
    }

    /// 运行异步性能测试。
    pub async fn run_async<F, Fut, R>(&self, iterations: usize, mut f: F) -> PerfReport
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = R>,
    {
        // 预热
        for _ in 0..self.warmup {
            let _ = f().await;
        }

        let start = Instant::now();
        for _ in 0..iterations {
            let _ = f().await;
        }
        let elapsed = start.elapsed();

        PerfReport::new(&self.name, elapsed, iterations)
    }

    /// 运行单次操作并返回报告（iterations=1）。
    pub fn run_once<F, R>(&self, f: F) -> PerfReport
    where
        F: FnOnce() -> R,
    {
        let start = Instant::now();
        let _ = f();
        let elapsed = start.elapsed();
        PerfReport::new(&self.name, elapsed, 1)
    }

    /// 运行单次异步操作并返回报告（iterations=1）。
    pub async fn run_once_async<F, Fut, R>(&self, f: F) -> PerfReport
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = R>,
    {
        let start = Instant::now();
        let _ = f().await;
        let elapsed = start.elapsed();
        PerfReport::new(&self.name, elapsed, 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perf_report_creation() {
        let report = PerfReport::new("test_op", Duration::from_millis(100), 10);
        assert_eq!(report.name, "test_op");
        assert_eq!(report.iterations, 10);
        // 100ms / 10 iterations = 10ms = 10000μs per op
        assert!((report.avg_us - 10_000.0).abs() < 100.0);
        // 10 ops / 0.1s = 100 ops/sec
        assert!((report.throughput - 100.0).abs() < 5.0);
    }

    #[test]
    fn test_perf_report_zero_iterations() {
        let report = PerfReport::new("zero_test", Duration::from_millis(0), 0);
        assert_eq!(report.avg_us, 0.0);
        assert_eq!(report.throughput, 0.0);
    }

    #[test]
    fn test_perf_runner_sync() {
        let report = PerfRunner::new("sync_test").run(1000, || {
            // 模拟工作
            let mut sum = 0u64;
            for i in 0..1000 {
                sum = sum.wrapping_add(i);
            }
            sum
        });

        assert_eq!(report.iterations, 1000);
        assert!(report.throughput > 0.0);
        assert!(report.avg_us > 0.0);
    }

    #[tokio::test]
    async fn test_perf_runner_async() {
        let report = PerfRunner::new("async_test")
            .run_async(100, || async {
                tokio::task::yield_now().await;
            })
            .await;

        assert_eq!(report.iterations, 100);
        assert!(report.throughput > 0.0);
    }

    #[test]
    fn test_perf_runner_warmup() {
        let report = PerfRunner::new("warmup_test")
            .with_warmup(10)
            .run(100, || 42u64);

        assert_eq!(report.iterations, 100);
        // 预热不计入计时，所以报告 iterations 应为 100
    }

    #[test]
    fn test_perf_runner_once() {
        let report = PerfRunner::new("once_test").run_once(|| {
            std::thread::sleep(Duration::from_millis(10));
        });

        assert_eq!(report.iterations, 1);
        assert!(report.avg_us >= 10_000.0); // 至少 10ms
    }

    #[test]
    fn test_perf_report_assert_throughput_pass() {
        let report = PerfReport::new("pass_test", Duration::from_millis(10), 1000);
        // 1000 ops / 0.01s = 100,000 ops/sec >> 1000
        report.assert_throughput(1000.0);
    }

    #[test]
    #[should_panic(expected = "性能基准不达标")]
    fn test_perf_report_assert_throughput_fail() {
        let report = PerfReport::new("fail_test", Duration::from_secs(1), 1);
        // 1 op / 1s = 1 ops/sec < 1000
        report.assert_throughput(1000.0);
    }

    #[test]
    fn test_perf_report_assert_latency_pass() {
        let report = PerfReport::new("latency_pass", Duration::from_millis(10), 100);
        // 10ms / 100 = 100μs/op << 10000μs
        report.assert_latency_us(10_000.0);
    }

    #[test]
    #[should_panic(expected = "延迟基准不达标")]
    fn test_perf_report_assert_latency_fail() {
        let report = PerfReport::new("latency_fail", Duration::from_millis(100), 1);
        // 100ms / 1 = 100,000μs >> 1000μs
        report.assert_latency_us(1000.0);
    }

    #[test]
    fn test_perf_report_print_doesnt_panic() {
        let report = PerfReport::new("print_test", Duration::from_millis(5), 200);
        report.print(); // 不应 panic
    }
}
