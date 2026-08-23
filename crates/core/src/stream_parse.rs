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

use crate::finish_reason::FinishReason;

/// OpenAI 流式元素。
#[derive(Debug, PartialEq)]
pub enum StreamItem {
    /// 增量 token
    Token(String),
    /// 推理内容增量（reasoning_content，DeepSeek R1 / Qwen 等推理模型的思考过程）
    Reasoning(String),
    /// Token 用量统计（流末尾的 usage 字段）
    Usage(echomind_models::TokenUsage),
    /// 停止原因（finish_reason 字段，通常出现在最后一个 chunk）
    Finish(FinishReason),
    /// `[DONE]` 结束标记
    Done,
}

/// 解析单条 SSE data 载荷：token 提取、usage 统计与 `[DONE]` 识别；非有效载荷返回 None。
///
/// 当 `stream_options.include_usage` 开启时，API 在流末尾发送一个含 `usage` 字段、
/// `choices` 为空的最终 chunk，此函数将其解析为 `StreamItem::Usage`。
///
/// 推理模型（DeepSeek R1 等）在正式回答前会通过 `delta.reasoning_content` 逐段推送
/// 思考过程，这里优先提取并返回 `StreamItem::Reasoning`；两种字段在同一 chunk 中
/// 不会同时出现（推理阶段与回答阶段互斥）。
pub fn parse_openai_payload(payload: &str) -> Option<StreamItem> {
    let trimmed = payload.trim();
    if trimmed == "[DONE]" {
        return Some(StreamItem::Done);
    }
    let chunk: ChatCompletionChunk = serde_json::from_str(trimmed).ok()?;
    let ChatCompletionChunk { choices, usage } = chunk;

    // 推理阶段：优先提取 reasoning_content
    if let Some(reasoning) = choices
        .first()
        .and_then(|c| c.delta.reasoning_content.as_deref())
        .filter(|content| !content.is_empty())
    {
        return Some(StreamItem::Reasoning(reasoning.to_string()));
    }

    // 流末尾的 finish_reason（通常在最后一个含 choices 的 chunk 中）
    // 注意：必须在 `into_iter()` 消费 choices 之前检查
    if let Some(finish_raw) = choices
        .first()
        .and_then(|c| c.finish_reason.as_deref())
        .filter(|s| !s.is_empty())
    {
        return Some(StreamItem::Finish(FinishReason::from_provider_str(finish_raw)));
    }

    // 回答阶段：提取 content token（绝大多数 chunk 携带 token）
    if let Some(token) = choices
        .into_iter()
        .next()
        .and_then(|c| c.delta.content)
        .filter(|content| !content.is_empty())
    {
        return Some(StreamItem::Token(token));
    }

    // 流末尾的 usage chunk（choices 为空 + usage 非空）
    usage.map(|u| {
        StreamItem::Usage(echomind_models::TokenUsage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
        })
    })
}

#[derive(serde::Deserialize)]
struct ChatCompletionChunk {
    #[serde(default)]
    choices: Vec<ChunkChoice>,
    #[serde(default)]
    usage: Option<UsagePayload>,
}

#[derive(serde::Deserialize)]
struct UsagePayload {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

#[derive(serde::Deserialize)]
struct ChunkChoice {
    delta: ChunkDelta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(serde::Deserialize)]
struct ChunkDelta {
    #[serde(default)]
    content: Option<String>,
    /// 推理内容（DeepSeek R1 / Qwen 等推理模型在回答前推送的思考过程）
    #[serde(default)]
    reasoning_content: Option<String>,
}
