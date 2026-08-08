//! OpenAI 兼容 Vision LLM 适配器（REQ-MM-003）：将图片发送到用户配置的 VLM 端点。
//!
//! 安全官审查项（ADR-010 §4）：
//! - 请求 URL 必须且仅发往用户配置的 `base_url`（AC-4 网络审计）；
//! - 空 API Key 不携带 Authorization 头（兼容本地 Ollama）；
//! - 图片以 base64 data URL 嵌入请求体，不落盘、不缓存；
//! - 错误信息截断，不含 API Key 等敏感信息；
//! - VLM 不可用时返回空字符串（优雅降级，不崩溃）。

use std::time::Duration;

use anyhow::Context;
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use echomind_core::VisionLanguageModel;

/// 连接超时（秒）：VLM 端点 TCP 连接建立阶段的最大等待时间。
const VISION_CONNECT_TIMEOUT_SECS: u64 = 30;
/// VLM 请求总体超时（秒）：图片理解比文本对话耗时更长，设 120s 上限。
const VISION_REQUEST_TIMEOUT_SECS: u64 = 120;

/// OpenAI 兼容 Vision Provider（Chat Completions + image_url base64）。
///
/// 复用用户的 BYOK 配置（api_key / base_url / model），无需额外设置。
/// 支持所有 OpenAI 兼容 Vision API（GPT-4o / Claude 3.5 / Qwen-VL 等）。
pub struct OpenAIVisionProvider {
    client: reqwest::Client,
    api_key: Option<String>,
    base_url: String,
    model: String,
}

impl OpenAIVisionProvider {
    /// 构造 Vision Provider；`api_key` 为空白时视为 None（本地 Ollama 场景）。
    ///
    /// # 安全
    /// 设置 connect_timeout（30s）和 request_timeout（120s），
    /// 防止 VLM 端点无响应时索引管线永久挂起。
    pub fn new(api_key: String, base_url: String, model: String) -> anyhow::Result<Self> {
        let api_key = (!api_key.trim().is_empty()).then_some(api_key);
        let base_url = base_url.trim_end_matches('/').to_string();
        let client = reqwest::Client::builder()
            .no_proxy() // 🚫 禁止代理，确保 VLM API 直连（铁律一）
            .connect_timeout(Duration::from_secs(VISION_CONNECT_TIMEOUT_SECS))
            .timeout(Duration::from_secs(VISION_REQUEST_TIMEOUT_SECS))
            .build()
            .context("构建 Vision HTTP 客户端失败（内部错误）")?;
        Ok(Self {
            client,
            api_key,
            base_url,
            model,
        })
    }

    /// 构造 Chat Completions URL（复用 LLM 端点，AC-4 网络审计）。
    fn chat_completions_url(&self) -> String {
        format!("{}/v1/chat/completions", self.base_url)
    }

    /// 构建带认证头的请求。
    fn build_request(&self, body: &serde_json::Value) -> reqwest::RequestBuilder {
        let mut req = self.client.post(self.chat_completions_url()).json(body);
        if let Some(key) = &self.api_key {
            req = req.bearer_auth(key);
        }
        req
    }

    /// 从 OpenAI Chat Completions 响应体中提取助手消息文本。
    ///
    /// OpenAI 格式：`choices[0].message.content`（字符串或数组）。
    /// 兼容部分实现返回数组内容的情况（取所有 text 类型拼接）。
    fn extract_content(response: &serde_json::Value) -> Option<String> {
        let choice = response.get("choices")?.get(0)?;
        let message = choice.get("message")?;
        match message.get("content") {
            Some(serde_json::Value::String(s)) => Some(s.clone()),
            Some(serde_json::Value::Array(parts)) => {
                let texts: Vec<&str> = parts
                    .iter()
                    .filter_map(|p| {
                        if p.get("type")?.as_str()? == "text" {
                            p.get("text")?.as_str()
                        } else {
                            None
                        }
                    })
                    .collect();
                if texts.is_empty() {
                    None
                } else {
                    Some(texts.join(""))
                }
            }
            _ => None,
        }
    }
}

impl VisionLanguageModel for OpenAIVisionProvider {
    /// 将图片发送到 VLM，获取结构化文本描述（REQ-MM-003）。
    ///
    /// 流程：
    /// 1. 将图片字节 base64 编码为 data URL
    /// 2. 构建 Chat Completions 请求（system prompt + user message with image_url）
    /// 3. 非流式发送（VLM 需完整理解图片后一次性返回）
    /// 4. 提取 `choices[0].message.content` 作为描述文本
    ///
    /// # 优雅降级
    /// VLM 不可用（HTTP 错误 / 超时 / 响应格式异常）时返回空字符串，不崩溃。
    async fn describe_image(&self, image_bytes: &[u8], prompt: &str) -> anyhow::Result<String> {
        // base64 编码图片 → data URL（OpenAI Vision API 格式）
        let b64 = B64.encode(image_bytes);
        let data_url = format!("data:image/png;base64,{b64}");

        // 构建请求体：system prompt 引导 + user message 含文本+图片
        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {
                    "role": "system",
                    "content": prompt
                },
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "text",
                            "text": "请分析这张图片中的内容并转换为结构化文本。"
                        },
                        {
                            "type": "image_url",
                            "image_url": {
                                "url": data_url
                            }
                        }
                    ]
                }
            ],
            "stream": false,
            // 限制 max_tokens 防止 VLM 返回过长描述拖慢索引管线
            "max_tokens": 2000
        });

        let resp = self
            .build_request(&body)
            .send()
            .await
            .context("VLM 请求发送失败（请检查 base_url 与网络）")?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp
                .text()
                .await
                .unwrap_or_else(|_| "<无法读取响应体>".to_string());
            // 优雅降级：VLM 端点返回错误时返回空字符串，不阻断索引管线
            eprintln!(
                "VLM 请求失败 (HTTP {}): {} — 将降级为纯 OCR",
                status.as_u16(),
                truncate(&text, 300)
            );
            return Ok(String::new());
        }

        let response_json: serde_json::Value =
            resp.json().await.context("VLM 响应 JSON 解析失败")?;

        // 提取助手消息内容
        match Self::extract_content(&response_json) {
            Some(content) => Ok(content),
            None => {
                // 响应格式异常时优雅降级
                eprintln!(
                    "VLM 响应格式异常，无法提取 content — 将降级为纯 OCR。响应: {}",
                    truncate(&response_json.to_string(), 300)
                );
                Ok(String::new())
            }
        }
    }
}

/// 截断文本（保持字符边界，防超长输出）。
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
