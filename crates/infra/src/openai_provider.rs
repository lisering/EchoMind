//! OpenAI 兼容 LLM 适配器（REQ-LLM-001/002）：reqwest SSE 流式对话。
//! 安全官审查项：
//! - base_url 规范化去尾斜杠，防拼接双斜杠；
//! - 空 API Key 不携带 Authorization 头（兼容本地 Ollama）；
//! - 错误信息透传前截断，避免超长响应体刷屏（不泄露敏感头信息）。

use anyhow::{Context, bail};
use echomind_core::LLMProvider;
use echomind_core::stream_parse::{SseParser, StreamItem, parse_openai_payload};
use echomind_models::ChatMessage;
use futures::StreamExt;
use futures::stream::BoxStream;

/// OpenAI 兼容 Provider（Chat Completions 协议）。
pub struct OpenAIProvider {
    client: reqwest::Client,
    api_key: Option<String>,
    base_url: String,
    model: String,
}

impl OpenAIProvider {
    /// 构造 Provider；`api_key` 为空白时视为 None（本地 Ollama 场景）。
    pub fn new(api_key: String, base_url: String, model: String) -> Self {
        let api_key = (!api_key.trim().is_empty()).then_some(api_key);
        let base_url = base_url.trim_end_matches('/').to_string();
        Self {
            client: reqwest::Client::new(),
            api_key,
            base_url,
            model,
        }
    }

    fn chat_completions_url(&self) -> String {
        format!("{}/v1/chat/completions", self.base_url)
    }

    fn build_request(&self, body: &serde_json::Value) -> reqwest::RequestBuilder {
        let mut req = self.client.post(self.chat_completions_url()).json(body);
        if let Some(key) = &self.api_key {
            req = req.bearer_auth(key);
        }
        req
    }

    /// 极简请求验证配置（REQ-LLM-002）：max_tokens=1，非流式。
    pub async fn test_connection(&self) -> anyhow::Result<String> {
        let body = serde_json::json!({
            "model": self.model,
            "messages": [{"role": "user", "content": "ping"}],
            "max_tokens": 1,
            "stream": false,
        });
        let resp = self
            .build_request(&body)
            .send()
            .await
            .context("无法连接到 LLM 端点（请检查 base_url 与网络）")?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp
                .text()
                .await
                .unwrap_or_else(|_| "<无法读取响应体>".to_string());
            bail!(
                "LLM API 错误 (HTTP {}): {}",
                status.as_u16(),
                truncate(&text, 300)
            );
        }
        Ok(format!(
            "连接成功：{} 响应正常 (HTTP {})",
            self.model,
            status.as_u16()
        ))
    }
}

impl LLMProvider for OpenAIProvider {
    async fn chat_stream(
        &self,
        system_prompt: &str,
        history: &[ChatMessage],
        query: &str,
    ) -> anyhow::Result<BoxStream<'static, anyhow::Result<String>>> {
        let mut messages = vec![serde_json::json!({"role": "system", "content": system_prompt})];
        for msg in history {
            messages.push(serde_json::json!({"role": msg.role, "content": msg.content}));
        }
        messages.push(serde_json::json!({"role": "user", "content": query}));
        let body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "stream": true,
        });

        let resp = self
            .build_request(&body)
            .send()
            .await
            .context("LLM 请求发送失败")?;
        let status = resp.status();
        if !status.is_success() {
            // 精准捕获 HTTP 状态码并透传真实错误体（REQ-LLM-001-AC-2）
            let text = resp
                .text()
                .await
                .unwrap_or_else(|_| "<无法读取响应体>".to_string());
            bail!(
                "LLM API 错误 (HTTP {}): {}",
                status.as_u16(),
                truncate(&text, 500)
            );
        }

        // SSE 字节流 → SseParser 缓冲切分 → token 流；[DONE] 处截断
        let token_stream = resp
            .bytes_stream()
            .scan(SseParser::new(), |parser, chunk| {
                let items: Vec<anyhow::Result<StreamItem>> = match chunk {
                    Ok(bytes) => parser
                        .feed(&bytes)
                        .into_iter()
                        .filter_map(|payload| parse_openai_payload(&payload))
                        .map(Ok)
                        .collect(),
                    Err(err) => vec![Err(anyhow::anyhow!("SSE 流读取失败: {err}"))],
                };
                std::future::ready(Some(items))
            })
            .flat_map(futures::stream::iter)
            .take_while(|item| std::future::ready(!matches!(item, Ok(StreamItem::Done))))
            .filter_map(|item| async move {
                match item {
                    Ok(StreamItem::Token(token)) => Some(Ok(token)),
                    Ok(StreamItem::Done) => None, // 已被 take_while 截断，防御性保留
                    Err(err) => Some(Err(err)),
                }
            });
        Ok(Box::pin(token_stream))
    }
}

/// 截断错误响应体（保持字符边界，防超长错误刷屏）。
fn truncate(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}
