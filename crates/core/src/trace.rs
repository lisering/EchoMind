//! 轻量 RAG 链路追踪系统（S70：Cherry Studio 借鉴）。
//!
//! 为每次 RAG 查询生成 span 级追踪记录，记录检索链路每步耗时：
//! 嵌入 → 向量搜索 → BM25 → RRF → 重排 → LLM 首 token → LLM 总耗时。
//!
//! 设计原则：
//! - 零外部依赖（不用 OpenTelemetry，纯 Rust 实现）
//! - 线程安全（Arc<RwLock> 保护区 RingBuffer）
//! - 低开销（span 记录仅 HashMap + Instant）
//! - 可视化（前端设置面板展示最近 N 次 trace）

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
/// 单个 span 记录：一个命名步骤的开始/结束时间和元数据。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TraceSpan {
    /// span 名称（如 "embedding"、"vector_search"、"llm_stream"）
    pub name: String,
    /// 开始时间戳（Unix 毫秒）
    pub start_ms: u64,
    /// 持续时长（毫秒）
    pub duration_ms: u64,
    /// span 元数据（如检索到的 chunk 数、嵌入维度等）
    pub attributes: HashMap<String, String>,
    /// span 状态
    pub status: SpanStatus,
}

/// span 执行状态。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SpanStatus {
    /// 成功完成
    Ok,
    /// 失败（attributes 中 error 字段记录原因）
    Error,
}

/// 一次完整 RAG 查询的 trace 记录。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TraceRecord {
    /// 唯一 ID
    pub id: String,
    /// 查询文本（前 100 字符）
    pub query_preview: String,
    /// 查询时间戳（Unix 毫秒）
    pub timestamp: u64,
    /// span 列表（按开始时间排序）
    pub spans: Vec<TraceSpan>,
    /// 总耗时（毫秒）
    pub total_duration_ms: u64,
}

/// Trace 收集器：在 RAG 查询过程中收集 span。
pub struct TraceCollector {
    /// 查询预览
    query_preview: String,
    /// span 列表
    spans: Vec<TraceSpan>,
    /// 当前活跃 span（开始时间已记录，结束时间未记录）
    active_spans: Vec<(String, Instant, HashMap<String, String>)>,
    /// 查询开始时间
    start: Instant,
}

impl TraceCollector {
    /// 创建新的 trace 收集器。
    pub fn new(query: &str) -> Self {
        Self {
            query_preview: query.chars().take(100).collect(),
            spans: Vec::new(),
            active_spans: Vec::new(),
            start: Instant::now(),
        }
    }

    /// 开始一个 span。
    pub fn start_span(&mut self, name: &str) {
        self.start_span_with_attrs(name, HashMap::new());
    }

    /// 开始一个 span（带初始属性）。
    pub fn start_span_with_attrs(&mut self, name: &str, attrs: HashMap<String, String>) {
        self.active_spans
            .push((name.to_string(), Instant::now(), attrs));
    }

    /// 添加属性到当前活跃 span。
    pub fn add_attr(&mut self, key: &str, value: &str) {
        if let Some((_, _, attrs)) = self.active_spans.last_mut() {
            attrs.insert(key.to_string(), value.to_string());
        }
    }

    /// 结束当前活跃 span（标记为成功）。
    pub fn end_span(&mut self) {
        self.end_span_with_status(SpanStatus::Ok);
    }

    /// 结束当前活跃 span（标记为指定状态）。
    pub fn end_span_with_status(&mut self, status: SpanStatus) {
        if let Some((name, start, attrs)) = self.active_spans.pop() {
            let duration = start.elapsed();
            let start_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0)
                .saturating_sub(duration.as_millis() as u64);

            self.spans.push(TraceSpan {
                name,
                start_ms,
                duration_ms: duration.as_millis() as u64,
                attributes: attrs,
                status,
            });
        }
    }

    /// 完成 trace 收集，生成 TraceRecord。
    pub fn finish(self) -> TraceRecord {
        let total_duration_ms = self.start.elapsed().as_millis() as u64;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let id = format!("trace-{timestamp}");

        TraceRecord {
            id,
            query_preview: self.query_preview,
            timestamp,
            spans: self.spans,
            total_duration_ms,
        }
    }
}

/// Trace 存储：环形缓冲区保存最近 N 条 trace 记录。
pub struct TraceStore {
    /// 环形缓冲区
    records: RwLock<Vec<TraceRecord>>,
    /// 最大容量
    max_capacity: usize,
}

impl TraceStore {
    /// 创建 trace 存储（默认保留最近 50 条）。
    pub fn new(max_capacity: usize) -> Self {
        Self {
            records: RwLock::new(Vec::with_capacity(max_capacity)),
            max_capacity,
        }
    }

    /// 添加一条 trace 记录。
    pub async fn add(&self, record: TraceRecord) {
        let mut records = self.records.write().await;
        if records.len() >= self.max_capacity {
            records.remove(0); // 移除最旧
        }
        records.push(record);
    }

    /// 获取最近 N 条 trace 记录。
    pub async fn recent(&self, limit: usize) -> Vec<TraceRecord> {
        let records = self.records.read().await;
        let start = records.len().saturating_sub(limit);
        records[start..].to_vec()
    }

    /// 获取指定 ID 的 trace 记录。
    pub async fn get(&self, id: &str) -> Option<TraceRecord> {
        let records = self.records.read().await;
        records.iter().find(|r| r.id == id).cloned()
    }

    /// 清空所有 trace 记录。
    pub async fn clear(&self) {
        self.records.write().await.clear();
    }

    /// 获取 trace 记录数量。
    pub async fn count(&self) -> usize {
        self.records.read().await.len()
    }
}

/// 默认 TraceStore 容量。
pub const DEFAULT_TRACE_CAPACITY: usize = 50;

/// 便捷函数：创建默认容量的 TraceStore。
pub fn default_trace_store() -> Arc<TraceStore> {
    Arc::new(TraceStore::new(DEFAULT_TRACE_CAPACITY))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_trace_collector_empty() {
        let collector = TraceCollector::new("test query");
        let record = collector.finish();
        assert_eq!(record.query_preview, "test query");
        assert!(record.spans.is_empty());
        assert!(record.total_duration_ms < 100);
    }

    #[test]
    fn test_trace_collector_single_span() {
        let mut collector = TraceCollector::new("hello");
        collector.start_span("embedding");
        std::thread::sleep(Duration::from_millis(10));
        collector.end_span();
        let record = collector.finish();

        assert_eq!(record.spans.len(), 1);
        assert_eq!(record.spans[0].name, "embedding");
        assert!(record.spans[0].duration_ms >= 8); // 至少 10ms（允许误差）
        assert_eq!(record.spans[0].status, SpanStatus::Ok);
    }

    #[test]
    fn test_trace_collector_multiple_spans() {
        let mut collector = TraceCollector::new("test");
        collector.start_span("step1");
        std::thread::sleep(Duration::from_millis(5));
        collector.end_span();
        collector.start_span("step2");
        std::thread::sleep(Duration::from_millis(5));
        collector.end_span();
        collector.start_span("step3");
        std::thread::sleep(Duration::from_millis(5));
        collector.end_span();
        let record = collector.finish();

        assert_eq!(record.spans.len(), 3);
        assert!(record.total_duration_ms >= 12);
    }

    #[test]
    fn test_trace_collector_with_attrs() {
        let mut collector = TraceCollector::new("test");
        let mut attrs = HashMap::new();
        attrs.insert("chunk_count".to_string(), "5".to_string());
        attrs.insert("top_k".to_string(), "10".to_string());
        collector.start_span_with_attrs("vector_search", attrs);
        collector.end_span();
        let record = collector.finish();

        assert_eq!(
            record.spans[0].attributes.get("chunk_count"),
            Some(&"5".to_string())
        );
        assert_eq!(
            record.spans[0].attributes.get("top_k"),
            Some(&"10".to_string())
        );
    }

    #[test]
    fn test_trace_collector_add_attr() {
        let mut collector = TraceCollector::new("test");
        collector.start_span("llm_stream");
        collector.add_attr("model", "gpt-4");
        collector.add_attr("stream", "true");
        collector.end_span();
        let record = collector.finish();

        assert_eq!(
            record.spans[0].attributes.get("model"),
            Some(&"gpt-4".to_string())
        );
        assert_eq!(
            record.spans[0].attributes.get("stream"),
            Some(&"true".to_string())
        );
    }

    #[test]
    fn test_trace_collector_error_status() {
        let mut collector = TraceCollector::new("test");
        collector.start_span("llm_call");
        collector.add_attr("error", "timeout");
        collector.end_span_with_status(SpanStatus::Error);
        let record = collector.finish();

        assert_eq!(record.spans[0].status, SpanStatus::Error);
        assert_eq!(
            record.spans[0].attributes.get("error"),
            Some(&"timeout".to_string())
        );
    }

    #[test]
    fn test_trace_collector_query_preview_truncated() {
        let long_query = "x".repeat(200);
        let collector = TraceCollector::new(&long_query);
        let record = collector.finish();
        assert_eq!(record.query_preview.len(), 100);
    }

    #[tokio::test]
    async fn test_trace_store_add_and_recent() {
        let store = TraceStore::new(5);
        for i in 0..3 {
            let collector = TraceCollector::new(&format!("query {i}"));
            store.add(collector.finish()).await;
        }
        assert_eq!(store.count().await, 3);
        let recent = store.recent(2).await;
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].query_preview, "query 1");
        assert_eq!(recent[1].query_preview, "query 2");
    }

    #[tokio::test]
    async fn test_trace_store_capacity_limit() {
        let store = TraceStore::new(3);
        for i in 0..5 {
            let collector = TraceCollector::new(&format!("query {i}"));
            store.add(collector.finish()).await;
        }
        assert_eq!(store.count().await, 3);
        let recent = store.recent(10).await;
        // 应该只保留最后 3 条
        assert_eq!(recent[0].query_preview, "query 2");
        assert_eq!(recent[2].query_preview, "query 4");
    }

    #[tokio::test]
    async fn test_trace_store_get_by_id() {
        let store = TraceStore::new(10);
        let collector = TraceCollector::new("find me");
        let record = collector.finish();
        let id = record.id.clone();
        store.add(record).await;

        let found = store.get(&id).await;
        assert!(found.is_some());
        assert_eq!(found.as_ref().unwrap().query_preview, "find me");
    }

    #[tokio::test]
    async fn test_trace_store_clear() {
        let store = TraceStore::new(10);
        store.add(TraceCollector::new("test").finish()).await;
        assert_eq!(store.count().await, 1);
        store.clear().await;
        assert_eq!(store.count().await, 0);
    }

    #[tokio::test]
    async fn test_trace_store_empty_recent() {
        let store = TraceStore::new(10);
        let recent = store.recent(5).await;
        assert!(recent.is_empty());
    }

    #[test]
    fn test_default_trace_store_creation() {
        let _store = default_trace_store();
        // 应该创建成功且容量为 DEFAULT_TRACE_CAPACITY
        assert_eq!(DEFAULT_TRACE_CAPACITY, 50);
    }
}
