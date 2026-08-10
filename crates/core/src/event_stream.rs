//! 事件流自动埋点（借鉴 OpenMontage events.py）。
//!
//! ## 背景
//!
//! OpenMontage 的 `events.py` 实现了零负担自动埋点：
//! 1. **零 Agent 负担**：工具调用自动推断所属项目，无需显式传参
//! 2. **永不中断生产**：所有 I/O 错误静默吞掉，观测性不能破坏功能
//! 3. **追加写入 JSONL**：每行一个 JSON 事件，容错读取跳过损坏行
//!
//! EchoMind 借鉴此模式，为 IPC 命令和 Agent 操作提供轻量级事件流，
//! 写入 `events.jsonl` 文件（JSONL 格式），供诊断和可观测性使用。
//!
//! ## 设计
//!
//! - 线程安全：`Arc<Mutex<File>>` 串行化追加写入
//! - 永不 panic：所有 I/O 错误静默吞掉（`emit()` 返回 `()`）
//! - 容错读取：`read()` 跳过无法解析的行
//! - 自动推断会话 ID：从工具参数中推断 `conversation_id`（最佳努力）

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// 事件类型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventType {
    /// IPC 命令调用
    IpcCommand,
    /// 检索操作
    Retrieval,
    /// LLM 调用
    LlmCall,
    /// Agent 步骤
    AgentStep,
    /// 文档导入
    Import,
    /// 安全事件
    Security,
    /// 自定义事件
    Custom(String),
}

impl EventType {
    /// 转字符串标识。
    pub fn as_str(&self) -> &str {
        match self {
            Self::IpcCommand => "ipc_command",
            Self::Retrieval => "retrieval",
            Self::LlmCall => "llm_call",
            Self::AgentStep => "agent_step",
            Self::Import => "import",
            Self::Security => "security",
            Self::Custom(s) => s.as_str(),
        }
    }
}

/// 事件条目。
#[derive(Debug, Clone)]
pub struct EventEntry {
    /// 时间戳（epoch 秒）
    pub timestamp: u64,
    /// 事件类型
    pub event_type: String,
    /// 会话 ID（最佳努力推断）
    pub conversation_id: Option<String>,
    /// 事件负载（自由 key-value）
    pub payload: HashMap<String, String>,
}

impl EventEntry {
    /// 创建新事件条目。
    pub fn new(event_type: impl Into<String>) -> Self {
        Self {
            timestamp: current_epoch_secs(),
            event_type: event_type.into(),
            conversation_id: None,
            payload: HashMap::new(),
        }
    }

    /// 设置会话 ID。
    #[must_use]
    pub fn with_conversation(mut self, id: impl Into<String>) -> Self {
        self.conversation_id = Some(id.into());
        self
    }

    /// 添加负载字段。
    #[must_use]
    pub fn with_payload(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.payload.insert(key.into(), value.into());
        self
    }

    /// 序列化为 JSONL 单行。
    pub fn to_jsonl(&self) -> String {
        // 手动构建 JSON 避免引入 serde 依赖到此模块
        let mut fields: Vec<String> = vec![
            format!("\"ts\":{}", self.timestamp),
            format!("\"event_type\":\"{}\"", escape_json(&self.event_type)),
        ];
        if let Some(ref id) = self.conversation_id {
            fields.push(format!("\"conversation_id\":\"{}\"", escape_json(id)));
        }
        if !self.payload.is_empty() {
            let pairs: Vec<String> = self
                .payload
                .iter()
                .map(|(k, v)| format!("\"{}\":\"{}\"", escape_json(k), escape_json(v)))
                .collect();
            fields.push(format!("\"payload\":{{{}}}", pairs.join(",")));
        }
        format!("{{{}}}", fields.join(","))
    }
}

/// 事件流写入器（借鉴 OpenMontage `events.py` 的 `emit_event` + `read_events`）。
///
/// 追加写入 JSONL 文件，所有操作永不 panic / 永不返回 Err。
///
/// # 线程安全
///
/// 内部使用 `Arc<Mutex<File>>` 串行化写入，可安全跨线程共享。
///
/// # 使用方式
///
/// ```ignore
/// let stream = EventStream::open("data/events.jsonl");
/// stream.emit(EventEntry::new("retrieval")
///     .with_conversation("conv-1")
///     .with_payload("query", "Rust memory safety")
///     .with_payload("results", "5"));
/// let events = stream.read(100); // 最近 100 条
/// ```
#[derive(Clone)]
pub struct EventStream {
    /// 文件路径
    path: PathBuf,
    /// 文件句柄（Arc<Mutex> 线程安全）
    file: Arc<Mutex<Option<File>>>,
}

impl EventStream {
    /// 打开事件流文件（不存在则创建）。
    ///
    /// 文件打开失败时 `file` 为 `None`，后续 `emit()` 静默跳过。
    pub fn open(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&path)
            .ok();
        Self {
            path,
            file: Arc::new(Mutex::new(file)),
        }
    }

    /// 追加一个事件（永不 panic / 永不返回 Err）。
    ///
    /// 借鉴 OpenMontage `emit_event()`：所有 I/O 错误静默吞掉。
    pub fn emit(&self, entry: EventEntry) {
        let line = entry.to_jsonl();
        if let Ok(mut guard) = self.file.lock()
            && let Some(ref mut file) = *guard
        {
            // 写入失败静默忽略
            let _ = writeln!(file, "{line}");
            let _ = file.flush();
        }
    }

    /// 读取事件列表（容错：跳过无法解析的行）。
    ///
    /// 借鉴 OpenMontage `read_events()`：逐行读取，跳过空行和损坏行。
    ///
    /// # 参数
    /// - `limit`：最多返回的事件数量（None = 全部）
    ///
    /// # 返回
    /// 事件列表（按时间升序）。文件不可读时返回空 Vec。
    pub fn read(&self, limit: Option<usize>) -> Vec<EventEntry> {
        let file = match File::open(&self.path) {
            Ok(f) => f,
            Err(_) => return Vec::new(),
        };
        let reader = BufReader::new(file);
        let mut entries: Vec<EventEntry> = Vec::new();

        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => continue,
            };
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            // 尝试解析 JSONL 行
            if let Some(entry) = parse_jsonl(trimmed) {
                entries.push(entry);
            }
        }

        if let Some(n) = limit {
            let start = entries.len().saturating_sub(n);
            entries.drain(..start);
        }
        entries
    }

    /// 清空事件文件。
    pub fn clear(&self) {
        if let Ok(mut guard) = self.file.lock() {
            // 关闭旧文件句柄，以 truncate 模式重新打开
            *guard = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&self.path)
                .ok();
            // 重新以 append 模式打开供后续写入
            if guard.is_some() {
                *guard = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .read(true)
                    .open(&self.path)
                    .ok();
            }
        }
    }

    /// 返回事件文件路径。
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// 从工具参数中推断会话 ID（最佳努力）。
    ///
    /// 借鉴 OpenMontage `infer_project_dir()`：从输入参数中查找
    /// `conversation_id` 字段。找不到返回 `None`。
    pub fn infer_conversation_id(inputs: &HashMap<String, String>) -> Option<String> {
        inputs.get("conversation_id").cloned()
    }
}

impl std::fmt::Debug for EventStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventStream")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

/// 从 JSONL 行解析事件条目。
fn parse_jsonl(line: &str) -> Option<EventEntry> {
    // 极简 JSON 解析：提取 ts、event_type、conversation_id、payload
    // 不使用 serde，避免将 serde 依赖引入此纯逻辑模块
    let ts = extract_json_number(line, "ts")?;
    let event_type = extract_json_string(line, "event_type")?;
    let conversation_id = extract_json_string(line, "conversation_id");

    let mut entry = EventEntry {
        timestamp: ts,
        event_type,
        conversation_id,
        payload: HashMap::new(),
    };

    // 尝试提取 payload 中的简单 key-value 对（最佳努力）
    if let Some(payload_str) = extract_json_object(line, "payload") {
        // 极简解析：查找 "key":"value" 模式
        for pair in payload_str.split(',') {
            if let Some((k, v)) = parse_json_pair(pair) {
                entry.payload.insert(k, v);
            }
        }
    }

    Some(entry)
}

/// 从 JSON 字符串中提取数字字段值。
fn extract_json_number(json: &str, key: &str) -> Option<u64> {
    let pattern = format!("\"{key}\":");
    let idx = json.find(&pattern)?;
    let rest = &json[idx + pattern.len()..];
    // 读取连续的数字字符
    let num_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    num_str.parse().ok()
}

/// 从 JSON 字符串中提取字符串字段值。
fn extract_json_string(json: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{key}\":\"");
    let start = json.find(&pattern)? + pattern.len();
    let rest = &json[start..];
    // 查找未转义的引号
    let mut end = 0;
    let mut chars = rest.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            // 跳过转义字符
            chars.next();
            end += 2;
            continue;
        }
        if c == '"' {
            break;
        }
        end += c.len_utf8();
    }
    Some(unescape_json(&rest[..end]))
}

/// 从 JSON 字符串中提取对象字段值（返回对象内部字符串）。
fn extract_json_object(json: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{key}\":{{");
    let start = json.find(&pattern)? + pattern.len();
    let rest = &json[start..];
    // 查找匹配的闭合括号
    let mut depth = 1;
    let mut end = 0;
    for c in rest.chars() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            _ => {}
        }
        end += c.len_utf8();
    }
    Some(rest[..end].to_string())
}

/// 解析简单的 JSON 键值对 `"key":"value"`（处理转义引号）。
fn parse_json_pair(pair: &str) -> Option<(String, String)> {
    let pair = pair.trim();
    if !pair.starts_with('"') {
        return None;
    }
    // 找到 key 的结束引号（跳过转义）
    let key_end = find_unescaped_quote(&pair[1..])?;
    let key = &pair[1..1 + key_end];
    let rest = &pair[1 + key_end + 1..];
    // 跳过冒号
    let rest = rest.trim_start();
    if !rest.starts_with(':') {
        return None;
    }
    let rest = rest[1..].trim_start();
    if !rest.starts_with('"') {
        return None;
    }
    let value_end = find_unescaped_quote(&rest[1..])?;
    let value = &rest[1..1 + value_end];
    Some((unescape_json(key), unescape_json(value)))
}

/// 在 JSON 字符串值中查找未转义的引号位置（返回引号前的字符数）。
fn find_unescaped_quote(s: &str) -> Option<usize> {
    let mut chars = s.char_indices();
    while let Some((i, c)) = chars.next() {
        if c == '\\' {
            chars.next(); // 跳过转义字符
            continue;
        }
        if c == '"' {
            return Some(i);
        }
    }
    None
}

/// JSON 字符串转义。
fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// JSON 字符串反转义（逐字符解析，避免 replace 歧义）。
fn unescape_json(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('r') => result.push('\r'),
                Some('t') => result.push('\t'),
                Some('"') => result.push('"'),
                Some('\\') => result.push('\\'),
                Some(other) => {
                    result.push('\\');
                    result.push(other);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// 获取当前 epoch 秒。
fn current_epoch_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use std::env;

    /// TC-EVT-001: EventEntry 序列化为 JSONL
    #[test]
    fn test_entry_to_jsonl() {
        let entry = EventEntry::new("retrieval")
            .with_conversation("conv-1")
            .with_payload("query", "test")
            .with_payload("results", "5");
        let jsonl = entry.to_jsonl();
        assert!(jsonl.contains("\"event_type\":\"retrieval\""));
        assert!(jsonl.contains("\"conversation_id\":\"conv-1\""));
        assert!(jsonl.contains("\"query\":\"test\""));
        assert!(jsonl.contains("\"results\":\"5\""));
        assert!(jsonl.starts_with('{') && jsonl.ends_with('}'));
    }

    /// TC-EVT-002: EventEntry 无 conversation_id 和 payload
    #[test]
    fn test_entry_minimal() {
        let entry = EventEntry::new("llm_call");
        let jsonl = entry.to_jsonl();
        assert!(jsonl.contains("\"event_type\":\"llm_call\""));
        assert!(!jsonl.contains("conversation_id"));
        assert!(!jsonl.contains("payload"));
    }

    /// TC-EVT-003: 写入和读取事件
    #[test]
    fn test_write_read_events() {
        let path = env::temp_dir().join(format!(
            "echomind_evt_test_{}.jsonl",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let stream = EventStream::open(&path);

        stream.emit(
            EventEntry::new("retrieval")
                .with_conversation("c1")
                .with_payload("query", "hello"),
        );
        stream.emit(
            EventEntry::new("llm_call")
                .with_conversation("c1")
                .with_payload("model", "gpt-4"),
        );

        let events = stream.read(None);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, "retrieval");
        assert_eq!(events[1].event_type, "llm_call");

        // 清理
        let _ = std::fs::remove_file(&path);
    }

    /// TC-EVT-004: read with limit
    #[test]
    fn test_read_with_limit() {
        let path = env::temp_dir().join(format!(
            "echomind_evt_limit_{}.jsonl",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let stream = EventStream::open(&path);
        for i in 0..10 {
            stream.emit(EventEntry::new("test").with_payload("index", i.to_string()));
        }
        let events = stream.read(Some(3));
        assert_eq!(events.len(), 3);
        // 应返回最后 3 条
        assert_eq!(events[2].payload.get("index"), Some(&"9".to_string()));

        let _ = std::fs::remove_file(&path);
    }

    /// TC-EVT-005: 容错读取——跳过损坏行
    #[test]
    fn test_read_tolerant() {
        let path = env::temp_dir().join(format!(
            "echomind_evt_tol_{}.jsonl",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        // 手动写入混合内容
        std::fs::write(&path, "{\"ts\":100,\"event_type\":\"ok\"}\n\ncorrupted line\n{\"ts\":200,\"event_type\":\"ok2\"}\n").unwrap();
        let stream = EventStream::open(&path);
        let events = stream.read(None);
        assert_eq!(events.len(), 2); // 跳过空行和损坏行
        assert_eq!(events[0].timestamp, 100);
        assert_eq!(events[1].timestamp, 200);

        let _ = std::fs::remove_file(&path);
    }

    /// TC-EVT-006: infer_conversation_id 从参数中推断
    #[test]
    fn test_infer_conversation_id() {
        let mut inputs = HashMap::new();
        inputs.insert("conversation_id".to_string(), "conv-123".to_string());
        inputs.insert("query".to_string(), "test".to_string());
        assert_eq!(
            EventStream::infer_conversation_id(&inputs),
            Some("conv-123".to_string())
        );

        let empty: HashMap<String, String> = HashMap::new();
        assert_eq!(EventStream::infer_conversation_id(&empty), None);
    }

    /// TC-EVT-007: EventType as_str
    #[test]
    fn test_event_type_as_str() {
        assert_eq!(EventType::IpcCommand.as_str(), "ipc_command");
        assert_eq!(EventType::Retrieval.as_str(), "retrieval");
        assert_eq!(EventType::LlmCall.as_str(), "llm_call");
        assert_eq!(EventType::AgentStep.as_str(), "agent_step");
        assert_eq!(EventType::Import.as_str(), "import");
        assert_eq!(EventType::Security.as_str(), "security");
        assert_eq!(
            EventType::Custom("custom_event".to_string()).as_str(),
            "custom_event"
        );
    }

    /// TC-EVT-008: clear 清空事件文件
    #[test]
    fn test_clear() {
        let path = env::temp_dir().join(format!(
            "echomind_evt_clr_{}.jsonl",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let stream = EventStream::open(&path);
        stream.emit(EventEntry::new("test"));
        assert_eq!(stream.read(None).len(), 1);

        stream.clear();
        assert_eq!(stream.read(None).len(), 0);

        let _ = std::fs::remove_file(&path);
    }

    /// TC-EVT-009: JSON 字符串转义/反转义
    #[test]
    fn test_json_escape() {
        let original = "hello \"world\" with \\ backslash";
        let escaped = escape_json(original);
        let unescaped = unescape_json(&escaped);
        assert_eq!(unescaped, original);
    }

    /// TC-EVT-010: payload 包含特殊字符
    #[test]
    fn test_payload_special_chars() {
        let entry = EventEntry::new("test").with_payload("text", "hello \"world\" with \\ slashes");
        let jsonl = entry.to_jsonl();
        // 确保序列化后是有效的（无未转义引号）
        assert!(jsonl.contains(r#"\"world\""#));

        // 反序列化验证
        let parsed = parse_jsonl(&jsonl).expect("parse should succeed");
        assert_eq!(
            parsed.payload.get("text"),
            Some(&"hello \"world\" with \\ slashes".to_string())
        );
    }

    /// TC-EVT-011: 文件不存在时 read 返回空
    #[test]
    fn test_read_nonexistent() {
        let stream = EventStream::open("/nonexistent/path/events.jsonl");
        assert_eq!(stream.read(None).len(), 0);
    }

    /// TC-EVT-012: emit 到不存在的目录静默失败
    #[test]
    fn test_emit_silent_failure() {
        let stream = EventStream::open("/nonexistent/dir/events.jsonl");
        // 不应 panic
        stream.emit(EventEntry::new("test"));
    }
}
