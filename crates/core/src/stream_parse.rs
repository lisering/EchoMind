//! SSE 流式解析（REQ-LLM-001）：OpenAI 兼容协议。
//! 字节级缓冲：事件按 `\n\n` 边界切分，不完整尾部暂存待下一字节块；
//! 仅在完整事件上执行解码，多字节 UTF-8 字符跨块时不受损（TC-LLM-001）。

/// SSE 事件流解析器（粘包/拆包安全）。
#[derive(Default)]
pub struct SseParser {
    buffer: Vec<u8>,
}

impl SseParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// 喂入字节块，返回本批解析出的完整 data 载荷（已去 `data:` 前缀）。
    /// 不完整尾部保留在内部缓冲，等待下一次喂入。
    pub fn feed(&mut self, chunk: &[u8]) -> Vec<String> {
        self.buffer.extend_from_slice(chunk);
        let mut events = Vec::new();
        while let Some(pos) = find_subslice(&self.buffer, b"\n\n") {
            let raw: Vec<u8> = self.buffer.drain(..pos).collect();
            self.buffer.drain(..2); // 丢弃事件分隔符 \n\n
            let text = String::from_utf8_lossy(&raw);
            let data = extract_data_lines(&text);
            if !data.is_empty() {
                events.push(data);
            }
        }
        events
    }
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// 提取事件内全部 `data:` 行并按 SSE 规范以 `\n` 拼接（兼容 \r\n）。
fn extract_data_lines(event: &str) -> String {
    let mut lines = Vec::new();
    for line in event.lines() {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if let Some(data) = line.strip_prefix("data:") {
            lines.push(data.trim_start());
        }
    }
    lines.join("\n")
}

/// OpenAI 流式元素。
#[derive(Debug, PartialEq)]
pub enum StreamItem {
    /// 增量 token
    Token(String),
    /// `[DONE]` 结束标记
    Done,
}

/// 解析单条 SSE data 载荷：token 提取与 `[DONE]` 识别；非 token 载荷返回 None。
pub fn parse_openai_payload(payload: &str) -> Option<StreamItem> {
    let trimmed = payload.trim();
    if trimmed == "[DONE]" {
        return Some(StreamItem::Done);
    }
    let chunk: ChatCompletionChunk = serde_json::from_str(trimmed).ok()?;
    chunk
        .choices
        .into_iter()
        .next()
        .and_then(|c| c.delta.content)
        .filter(|content| !content.is_empty())
        .map(StreamItem::Token)
}

#[derive(serde::Deserialize)]
struct ChatCompletionChunk {
    choices: Vec<ChunkChoice>,
}

#[derive(serde::Deserialize)]
struct ChunkChoice {
    delta: ChunkDelta,
}

#[derive(serde::Deserialize)]
struct ChunkDelta {
    #[serde(default)]
    content: Option<String>,
}
