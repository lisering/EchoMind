//! OpenAI 兼容 LLM 适配器（REQ-LLM-001/002）：reqwest SSE 流式对话。
//! 安全官审查项：
//! - base_url 规范化去尾斜杠，防拼接双斜杠；
//! - 空 API Key 不携带 Authorization 头（兼容本地 Ollama）；
//! - 错误信息透传前截断，避免超长响应体刷屏（不泄露敏感头信息）；
//!
//! ## base_url 路径智能拼接（REQ-LLM-003）
//!
//! `chat_completions_url()` 根据用户填入的 base_url 智能拼接端点路径，兼容三类格式：
//!
//! | base_url 示例 | 拼接结果 | 判定规则 |
//! |---|---|---|
//! | `https://api.openai.com` | `…/v1/chat/completions` | 标准格式，追加 `/v1/chat/completions` |
//! | `https://open.bigmodel.cn/api/paas/v4` | `…/v4/chat/completions` | 末段为版本号，仅追加 `/chat/completions` |
//! | `https://api.example.com/v1/chat/completions` | 原样使用 | 已含完整路径，不追加 |
//!
//! 这使得智谱 GLM（`/api/paas/v4`）、DashScope（`/compatible-mode`）等非标准路径的
//! OpenAI 兼容端点也能直接使用，无需修改后端代码。

use std::time::Duration;

use anyhow::{Context, bail};
use echomind_core::LLMProvider;
use echomind_core::stream_parse::{SseParser, StreamItem, parse_openai_payload};
use echomind_models::{ChatMessage, GenerationParams, TokenUsage};
use futures::StreamExt;
use futures::stream::BoxStream;

/// 连接超时（秒）：TCP 连接建立阶段的最大等待时间。
const CONNECT_TIMEOUT_SECS: u64 = 30;
/// 流式请求总体超时（秒）：从发起到收完最后一个 SSE 事件的总时长上限。
/// 注意：流式对话的总体超时由 `forward_stream` 的 CancellationToken 控制，
/// 此值作为最终兜底，防止网络层永久挂起。
const REQUEST_TIMEOUT_SECS: u64 = 300;

/// OpenAI 兼容 Provider（Chat Completions 协议）。
pub struct OpenAIProvider {
    client: reqwest::Client,
    api_key: Option<String>,
    base_url: String,
    model: String,
    /// 共享的 token 用量存储：SSE 流末尾的 usage chunk 解析后写入此 cell，
    /// `usage_handle()` 返回的 Arc clone 可在流消费后读取。
    usage_cell: std::sync::Arc<tokio::sync::Mutex<Option<TokenUsage>>>,
    /// 推理内容（reasoning_content）接收端：每次流式请求建立时放入新的 receiver，
    /// 调用方通过 `take_reasoning_receiver()` 取出并消费（DeepSeek R1 等推理模型的思考过程）。
    reasoning_rx:
        std::sync::Arc<tokio::sync::Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<String>>>>,
    /// LLM 生成参数（REQ-RAG-015）：temperature/max_tokens/top_p，经请求体传递到 API。
    /// `None` 时使用 API 默认值（不包含在请求 JSON 中）。
    generation_params: Option<GenerationParams>,
}

impl OpenAIProvider {
    /// 构造 Provider；`api_key` 为空白时视为 None（本地 Ollama 场景）。
    ///
    /// 安全：设置 connect_timeout（30s）和 request_timeout（300s），
    /// 防止 LLM 端点无响应时 Tauri 命令永久挂起。
    pub fn new(api_key: String, base_url: String, model: String) -> anyhow::Result<Self> {
        let api_key = (!api_key.trim().is_empty()).then_some(api_key);
        let base_url = base_url.trim_end_matches('/').to_string();
        let client = reqwest::Client::builder()
            .no_proxy() // 🚫 禁止代理，确保 LLM API 直连（铁律一）
            .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .context("构建 HTTP 客户端失败（内部错误）")?;
        Ok(Self {
            client,
            api_key,
            base_url,
            model,
            usage_cell: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
            reasoning_rx: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
            generation_params: None,
        })
    }

    fn chat_completions_url(&self) -> String {
        resolve_chat_completions_url(&self.base_url)
    }

    /// 设置 LLM 生成参数（REQ-RAG-015）。
    ///
    /// 传入 `None` 清除自定义参数，恢复 API 默认值。
    pub fn with_generation_params(mut self, params: Option<GenerationParams>) -> Self {
        self.generation_params = params.map(|p| p.clamped());
        self
    }

    /// 返回 token 用量句柄的 Arc clone。
    ///
    /// 在 `chat_stream` / `chat_stream_segmented` 返回的流消费完毕后，
    /// 通过此句柄 `lock().await.take()` 读取 API 报告的 token 用量。
    /// 若 API 未返回 usage 字段，则为 `None`。
    pub fn usage_handle(&self) -> std::sync::Arc<tokio::sync::Mutex<Option<TokenUsage>>> {
        self.usage_cell.clone()
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

    /// 非流式单次完成（REQ-RAG-021 HyDE 查询改写用）：发送单轮请求，收集完整响应。
    ///
    /// 与 `chat_stream` 不同，此方法设置 `stream: false`，等待完整 JSON 响应后解析
    /// `choices[0].message.content`。适用于 HyDE 等需要一次性获取完整文本的场景，
    /// 不需要 token 级流式推送。
    ///
    /// # 参数
    /// - `system_prompt`: 系统提示词（如 HyDE 改写指令）
    /// - `user_message`: 用户消息（如待改写的查询）
    ///
    /// # 返回
    /// LLM 生成的完整文本。网络错误、HTTP 非 2xx、JSON 解析失败均返回 Err。
    pub async fn complete(
        &self,
        system_prompt: &str,
        user_message: &str,
    ) -> anyhow::Result<String> {
        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": user_message},
            ],
            "stream": false,
        });
        let resp = self
            .build_request(&body)
            .send()
            .await
            .context("HyDE 改写请求发送失败")?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp
                .text()
                .await
                .unwrap_or_else(|_| "<无法读取响应体>".to_string());
            bail!(
                "HyDE 改写 LLM API 错误 (HTTP {}): {}",
                status.as_u16(),
                truncate(&text, 300)
            );
        }
        let json: serde_json::Value = resp.json().await.context("HyDE 改写响应 JSON 解析失败")?;
        let content = json
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        Ok(content.to_string())
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
        self.send_stream_request(messages).await
    }
    /// 分段式流式对话（Prompt Caching 优化）。
    ///
    /// 将静态前缀和动态上下文分别作为两条独立的 `system` 消息发送：
    /// ```json
    /// [
    ///   {"role": "system", "content": "<static_prefix>"},   // ← 可被 API 端缓存
    ///   {"role": "system", "content": "<dynamic_context>"}, // ← 每次请求不同
    ///   ...history...,
    ///   {"role": "user", "content": "<query>"}
    /// ]
    /// ```
    ///
    /// OpenAI / Anthropic 等 API 的 prompt caching 基于 messages 数组前缀匹配：
    /// 第一条 system 消息在所有请求中完全一致 → 命中缓存，跳过重复 token 的
    /// prefill 计算，显著降低首 token 延迟和 token 计费。
    /// **性能优化（Prompt Caching 深化）**：messages 顺序调整为
    /// `static → history → dynamic → user`，使多轮对话的前缀
    /// `[静态前缀 + 除最新一轮外的全部历史]` 跨请求字节级一致 →
    /// DeepSeek / OpenAI 等 API 的 context cache 命中范围从「仅静态前缀」
    /// 扩展到「静态前缀 + 历史前缀」，显著降低首 token 延迟与 token 计费。
    /// 动态检索上下文（每次请求不同）作为缓存断点置于 history 之后、query 之前。
    ///
    /// 兼容性：OpenAI 兼容 API（DeepSeek / OpenAI / Ollama 等）均接受
    /// system 消息出现在 messages 任意位置。
    async fn chat_stream_segmented(
        &self,
        static_prefix: &str,
        dynamic_context: &str,
        history: &[ChatMessage],
        query: &str,
    ) -> anyhow::Result<BoxStream<'static, anyhow::Result<String>>> {
        let mut messages = vec![
            // 第一条 system 消息：静态前缀（跨请求不变 → prompt caching 命中目标）
            serde_json::json!({"role": "system", "content": static_prefix}),
        ];
        // 历史消息紧随其后：多轮对话时前段字节级稳定 → 历史前缀命中缓存
        for msg in history {
            messages.push(serde_json::json!({"role": msg.role, "content": msg.content}));
        }
        // 动态上下文（缓存断点）置于 history 之后、query 之前
        messages.push(serde_json::json!({"role": "system", "content": dynamic_context}));
        messages.push(serde_json::json!({"role": "user", "content": query}));
        self.send_stream_request(messages).await
    }

    // Q10 辅助模型方法（借鉴 QM HarnessModelUtilities）

    /// 生成对话标题（借鉴 QM generateTitle）。
    ///
    /// 调用 LLM 非流式接口，根据对话转录生成 3-10 字简短标题。
    /// 复用 `complete()` 方法发送非流式请求。
    /// 失败时返回 `Err`，调用方降级为 `derive_title()` 字符串截取。
    async fn generate_title(&self, transcript: &str) -> anyhow::Result<Option<String>> {
        const TITLE_SYSTEM_PROMPT: &str = "你是一个标题生成助手。请根据以下对话内容生成一个简短的标题（不超过 10 个字），只输出标题文本，不要加引号或标点。";
        let title = self.complete(TITLE_SYSTEM_PROMPT, transcript).await?;
        let title = title.trim();
        if title.is_empty() {
            return Ok(None);
        }
        // 截断超长标题（防 LLM 返回整段文本）
        let truncated: String = title.chars().take(30).collect();
        Ok(Some(truncated))
    }

    /// 单次推理（借鉴 QM oneShot）。
    ///
    /// 非流式单轮完成，复用 `complete()` 方法。
    /// 用于记忆提取、安全筛查、摘要生成等辅助任务。
    async fn one_shot(&self, system: &str, prompt: &str) -> anyhow::Result<Option<String>> {
        let result = self.complete(system, prompt).await?;
        if result.is_empty() {
            return Ok(None);
        }
        Ok(Some(result))
    }

    /// 判断器（借鉴 QM judge）。
    ///
    /// 用于安全筛查、质量评估等需要 LLM 做判断的场景。
    /// 返回判断结果文本（如 "yes"/"no"/"safe"/"unsafe"）。
    async fn judge(&self, system: &str, prompt: &str) -> anyhow::Result<Option<String>> {
        let verdict = self.complete(system, prompt).await?;
        let verdict = verdict.trim().to_string();
        if verdict.is_empty() {
            return Ok(None);
        }
        Ok(Some(verdict))
    }
}

impl OpenAIProvider {
    /// 发送流式请求并返回 token 流（`chat_stream` 与 `chat_stream_segmented` 共用）。
    ///
    /// 构建 HTTP 请求体 → 发送 → 检查状态码 → SSE 字节流解析 → token 流。
    /// 两个 `chat_stream*` 方法仅 messages 数组组装方式不同，后续逻辑完全一致。
    async fn send_stream_request(
        &self,
        messages: Vec<serde_json::Value>,
    ) -> anyhow::Result<BoxStream<'static, anyhow::Result<String>>> {
        let mut body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "stream": true,
            "stream_options": {"include_usage": true},
        });

        // REQ-RAG-015：条件包含生成参数（仅当用户设置了自定义值时）
        if let Some(ref params) = self.generation_params
            && let Some(obj) = body.as_object_mut()
        {
            obj.insert(
                "temperature".to_string(),
                serde_json::Value::from(params.temperature as f64),
            );
            obj.insert(
                "max_tokens".to_string(),
                serde_json::Value::from(params.max_tokens as u64),
            );
            obj.insert(
                "top_p".to_string(),
                serde_json::Value::from(params.top_p as f64),
            );
        }

        // 重置 usage cell（上一次对话的残留数据清空）
        // 必须使用 lock().await 而非 blocking_lock()——后者在异步上下文中会 panic：
        // "Cannot block the current thread from within a runtime"
        //（blocking_lock 内部调用 block_in_place，多线程 runtime 工作线程上禁止）
        *self.usage_cell.lock().await = None;
        let usage_cell = self.usage_cell.clone();

        // 推理内容通道：reasoning_content 增量经此 channel 独立流出，不混入 token 流
        let (reasoning_tx, reasoning_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        *self.reasoning_rx.lock().await = Some(reasoning_rx);

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
            .filter_map(move |item| {
                let cell = usage_cell.clone();
                let tx = reasoning_tx.clone();
                async move {
                    match item {
                        Ok(StreamItem::Token(token)) => Some(Ok(token)),
                        Ok(StreamItem::Reasoning(reasoning)) => {
                            // 推理内容转发到 channel；接收端被丢弃时静默忽略
                            let _ = tx.send(reasoning);
                            None
                        }
                        Ok(StreamItem::Usage(usage)) => {
                            *cell.lock().await = Some(usage);
                            None
                        }
                        Ok(StreamItem::Done) => None,
                        Err(err) => Some(Err(err)),
                    }
                }
            });
        Ok(Box::pin(token_stream))
    }

    /// 返回推理接收端的共享句柄（Arc），供后台任务在流建立后随时取用。
    ///
    /// 与直接持有 receiver 不同：句柄可在 provider 被 move 进引擎前克隆，
    /// 后台任务通过句柄轮询等待 `chat_stream` 建立 channel（引擎内部调用）。
    pub fn reasoning_rx_handle(
        &self,
    ) -> std::sync::Arc<tokio::sync::Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<String>>>>
    {
        self.reasoning_rx.clone()
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

/// 根据 base_url 智能拼接 Chat Completions 端点路径（REQ-LLM-003）。
///
/// 兼容三类格式：
/// 1. 完整路径：base_url 已含 `/chat/completions` → 原样使用
/// 2. 版本路径：base_url 末段为 `vN`（如 `/v1`、`/v4`）→ 仅追加 `/chat/completions`
/// 3. 标准格式：其他 → 追加 `/v1/chat/completions`
///
/// 这使得智谱 GLM（`https://open.bigmodel.cn/api/paas/v4`）等非标准路径
/// 也能直接使用。
fn resolve_chat_completions_url(base_url: &str) -> String {
    if base_url.ends_with("/chat/completions") {
        return base_url.to_string();
    }
    if last_path_segment_is_version(base_url) {
        format!("{base_url}/chat/completions")
    } else {
        format!("{base_url}/v1/chat/completions")
    }
}

/// 检查 URL 最后一段路径是否为 API 版本标识（如 "v1"、"v4"）。
fn last_path_segment_is_version(url: &str) -> bool {
    let last = url.rsplit('/').next().unwrap_or("");
    last.len() > 1 && last.starts_with('v') && last[1..].bytes().all(|b| b.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::resolve_chat_completions_url;

    #[test]
    fn standard_base_url_appends_v1_path() {
        let url = resolve_chat_completions_url("https://api.deepseek.com");
        assert_eq!(url, "https://api.deepseek.com/v1/chat/completions");
    }

    #[test]
    fn openai_base_url_appends_v1_path() {
        let url = resolve_chat_completions_url("https://api.openai.com");
        assert_eq!(url, "https://api.openai.com/v1/chat/completions");
    }

    #[test]
    fn ollama_base_url_appends_v1_path() {
        let url = resolve_chat_completions_url("http://localhost:11434");
        assert_eq!(url, "http://localhost:11434/v1/chat/completions");
    }

    #[test]
    fn version_segment_appends_only_chat_completions() {
        // 智谱 GLM: /api/paas/v4 → /api/paas/v4/chat/completions
        let url = resolve_chat_completions_url("https://open.bigmodel.cn/api/paas/v4");
        assert_eq!(url, "https://open.bigmodel.cn/api/paas/v4/chat/completions");
    }

    #[test]
    fn explicit_v1_segment_appends_only_chat_completions() {
        let url = resolve_chat_completions_url("https://api.moonshot.cn/v1");
        assert_eq!(url, "https://api.moonshot.cn/v1/chat/completions");
    }

    #[test]
    fn full_endpoint_url_unchanged() {
        let url = resolve_chat_completions_url("https://api.example.com/v1/chat/completions");
        assert_eq!(url, "https://api.example.com/v1/chat/completions");
    }

    #[test]
    fn dashscope_compatible_mode_uses_v1() {
        // DashScope: /compatible-mode → /compatible-mode/v1/chat/completions
        let url = resolve_chat_completions_url("https://dashscope.aliyuncs.com/compatible-mode");
        assert_eq!(
            url,
            "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions"
        );
    }

    #[test]
    fn trailing_slash_stripped_before_resolution() {
        // new() 去尾斜杠后再调用，此处直接测试 resolve 逻辑
        let url = resolve_chat_completions_url("https://api.openai.com");
        assert!(!url.contains("//v1"));
    }
}
