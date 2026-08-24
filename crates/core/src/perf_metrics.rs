//! 性能指标采集模块（REQ-OBS-002）。
//!
//! 在关键操作处采集性能指标并记录到日志。
//! 指标以 INFO 级别日志记录，含操作类型、耗时(ms)、数据规模。

use serde::{Deserialize, Serialize};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// 性能指标记录。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerfMetric {
    /// 操作类型（如 "import.parse", "rag.vector_search"）
    pub operation: String,
    /// 耗时（毫秒）
    pub duration_ms: u64,
    /// 数据规模（如 chunk 数、向量维度）
    pub data_size: Option<u64>,
    /// 时间戳（Unix 毫秒）
    pub timestamp: u64,
}

impl PerfMetric {
    /// 创建新的性能指标记录。
    pub fn new(operation: &str, duration_ms: u64) -> Self {
        Self {
            operation: operation.to_string(),
            duration_ms,
            data_size: None,
            timestamp: current_timestamp_ms(),
        }
    }

    /// 设置数据规模。
    pub fn with_size(mut self, size: u64) -> Self {
        self.data_size = Some(size);
        self
    }

    /// 记录到日志（INFO 级别）。
    pub fn record(&self) {
        match self.data_size {
            Some(size) => {
                tracing::info!(
                    operation = %self.operation,
                    duration_ms = self.duration_ms,
                    data_size = size,
                    "PERF_METRIC"
                );
            }
            None => {
                tracing::info!(
                    operation = %self.operation,
                    duration_ms = self.duration_ms,
                    "PERF_METRIC"
                );
            }
        }
    }
}

/// 计时器：用于测量操作耗时。
pub struct PerfTimer {
    start: Instant,
    operation: String,
}

impl PerfTimer {
    /// 开始计时。
    pub fn start(operation: &str) -> Self {
        Self {
            start: Instant::now(),
            operation: operation.to_string(),
        }
    }

    /// 结束计时并返回耗时（毫秒）。
    pub fn elapsed_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }

    /// 结束计时并记录性能指标（无数据规模）。
    pub fn finish(&self) {
        let metric = PerfMetric::new(&self.operation, self.elapsed_ms());
        metric.record();
    }

    /// 结束计时并记录性能指标（含数据规模）。
    pub fn finish_with_size(&self, size: u64) {
        let metric = PerfMetric::new(&self.operation, self.elapsed_ms()).with_size(size);
        metric.record();
    }
}

/// 获取当前 Unix 时间戳（毫秒）。
pub fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn test_perf_timer_elapsed() {
        let timer = PerfTimer::start("test.operation");
        std::thread::sleep(std::time::Duration::from_millis(10));
        let elapsed = timer.elapsed_ms();
        assert!(elapsed >= 10, "elapsed should be >= 10ms, got {elapsed}");
    }

    #[test]
    fn test_perf_metric_new() {
        let metric = PerfMetric::new("import.parse", 150);
        assert_eq!(metric.operation, "import.parse");
        assert_eq!(metric.duration_ms, 150);
        assert!(metric.data_size.is_none());
        assert!(metric.timestamp > 0);
    }

    #[test]
    fn test_perf_metric_with_size() {
        let metric = PerfMetric::new("rag.vector_search", 50).with_size(1280);
        assert_eq!(metric.data_size, Some(1280));
    }

    #[test]
    fn test_perf_metric_serde() {
        let metric = PerfMetric::new("test", 100).with_size(500);
        let json = serde_json::to_string(&metric).unwrap();
        let decoded: PerfMetric = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.operation, "test");
        assert_eq!(decoded.duration_ms, 100);
        assert_eq!(decoded.data_size, Some(500));
    }

    #[test]
    fn test_perf_metric_no_user_data() {
        // 性能指标不应包含用户数据内容，仅记录元数据与耗时
        let metric = PerfMetric::new("import.parse", 200).with_size(1024);
        let json = serde_json::to_string(&metric).unwrap();
        // 确保序列化结果不包含 "content" 或 "text" 等用户数据字段
        assert!(!json.contains("content"));
        assert!(!json.contains("text"));
        assert!(!json.contains("user_input"));
    }
}
