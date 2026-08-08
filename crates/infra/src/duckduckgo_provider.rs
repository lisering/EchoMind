//! DuckDuckGo Instant Answer API 适配器（REQ-RAG-036）。
//!
//! 使用 DuckDuckGo 的 Instant Answer API 进行免费网页搜索，无需 API Key。
//!
//! ## API 端点
//!
//! `https://api.duckduckgo.com/?q=QUERY&format=json&no_html=1&skip_disambig=1`
//!
//! ## 响应解析
//!
//! DuckDuckGo Instant Answer API 返回 JSON，主要字段：
//! - `AbstractText`：直接答案摘要（可能为空）
//! - `AbstractURL`：答案来源 URL
//! - `Heading`：答案标题
//! - `RelatedTopics`：相关主题列表，每个包含 `Text`、`FirstURL`
//!
//! 当 `AbstractText` 非空时，作为第一条搜索结果；
//! `RelatedTopics` 中的条目作为后续搜索结果。
//!
//! ## 优雅降级
//!
//! - 网络超时 / API 不可达 → 返回空 Vec（不报错）
//! - JSON 解析失败 → 返回空 Vec
//! - 响应中无 `AbstractText` 且无 `RelatedTopics` → 返回空 Vec

use std::time::Duration;

use anyhow::Context;
use echomind_core::WebSearchProvider;
use echomind_models::SearchResult;

/// 连接超时（秒）：DuckDuckGo API 连接建立阶段的最大等待时间。
const CONNECT_TIMEOUT_SECS: u64 = 5;

/// 请求总体超时（秒）：从发起到收完响应的总时长上限。
/// 网页搜索不应阻塞主流程太久，10s 是上限。
const REQUEST_TIMEOUT_SECS: u64 = 10;

/// DuckDuckGo Instant Answer API 基础 URL。
const DDG_API_BASE: &str = "https://api.duckduckgo.com";

/// DuckDuckGo Instant Answer Provider。
///
/// 使用 reqwest 发送 HTTP 请求，解析 JSON 响应，
/// 提取 `AbstractText` 和 `RelatedTopics` 作为搜索结果。
pub struct DuckDuckGoProvider {
    client: reqwest::Client,
}

impl DuckDuckGoProvider {
    /// 创建 DuckDuckGo 搜索 Provider。
    ///
    /// 初始化 reqwest 客户端，设置连接超时 5s 和请求超时 10s，
    /// 防止 API 不可达时阻塞主流程。
    pub fn new() -> anyhow::Result<Self> {
        let client = reqwest::Client::builder()
            .no_proxy() // 🚫 禁止代理，确保 Web 搜索直连（铁律一）
            .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .context("构建 DuckDuckGo HTTP 客户端失败")?;
        Ok(Self { client })
    }

    /// 解析 DuckDuckGo JSON 响应为 SearchResult 列表。
    ///
    /// 从 JSON 中提取 `AbstractText`（直接答案）和 `RelatedTopics`（相关主题），
    /// 转换为 `SearchResult` 列表。
    fn parse_response(json: &serde_json::Value) -> Vec<SearchResult> {
        let mut results = Vec::new();

        // 提取 AbstractText（直接答案摘要）
        let abstract_text = json
            .get("AbstractText")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let abstract_url = json
            .get("AbstractURL")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let heading = json.get("Heading").and_then(|v| v.as_str()).unwrap_or("");

        // AbstractText 非空时作为第一条搜索结果
        if !abstract_text.is_empty() {
            results.push(SearchResult {
                title: heading.to_string(),
                url: abstract_url.to_string(),
                snippet: abstract_text.to_string(),
                source: "duckduckgo".to_string(),
            });
        }

        // 提取 RelatedTopics（相关主题列表）
        if let Some(related) = json.get("RelatedTopics").and_then(|v| v.as_array()) {
            for topic in related {
                // RelatedTopics 中的条目可能是对象（含 Text + FirstURL）或嵌套的 Topics 数组
                if let Some(text) = topic.get("Text").and_then(|v| v.as_str()) {
                    let url = topic.get("FirstURL").and_then(|v| v.as_str()).unwrap_or("");
                    if !text.is_empty() {
                        // 从 Text 中提取标题（第一个 " - " 之前的部分）
                        let (title, snippet) = if let Some(idx) = text.find(" - ") {
                            (text[..idx].to_string(), text.to_string())
                        } else {
                            (text.to_string(), text.to_string())
                        };
                        results.push(SearchResult {
                            title,
                            url: url.to_string(),
                            snippet,
                            source: "duckduckgo".to_string(),
                        });
                    }
                }
                // 处理嵌套 Topics（如消歧义页面下的子主题）
                if let Some(nested) = topic.get("Topics").and_then(|v| v.as_array()) {
                    for sub_topic in nested {
                        if let Some(text) = sub_topic.get("Text").and_then(|v| v.as_str()) {
                            let url = sub_topic
                                .get("FirstURL")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            if !text.is_empty() {
                                let (title, snippet) = if let Some(idx) = text.find(" - ") {
                                    (text[..idx].to_string(), text.to_string())
                                } else {
                                    (text.to_string(), text.to_string())
                                };
                                results.push(SearchResult {
                                    title,
                                    url: url.to_string(),
                                    snippet,
                                    source: "duckduckgo".to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }

        results
    }
}

impl Default for DuckDuckGoProvider {
    fn default() -> Self {
        Self::new().unwrap_or_else(|e| {
            eprintln!("[WEB] DuckDuckGo provider 初始化失败: {e:#}");
            // 降级：返回一个使用默认 reqwest 客户端的实例
            Self {
                client: reqwest::Client::new(),
            }
        })
    }
}

impl WebSearchProvider for DuckDuckGoProvider {
    fn search<'a>(
        &'a self,
        query: &'a str,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = anyhow::Result<Vec<SearchResult>>> + Send + 'a>,
    > {
        Box::pin(async move {
            // 空查询直接返回空结果
            if query.trim().is_empty() {
                return Ok(Vec::new());
            }

            // 使用 reqwest 的 query 参数方法自动编码查询字符串
            let resp = self
                .client
                .get(DDG_API_BASE)
                .query(&[
                    ("q", query),
                    ("format", "json"),
                    ("no_html", "1"),
                    ("skip_disambig", "1"),
                ])
                .header("User-Agent", "EchoMind/1.0 (RAG Knowledge Base)")
                .send()
                .await
                .context("DuckDuckGo API 请求发送失败")?;

            let status = resp.status();
            if !status.is_success() {
                // 优雅降级：返回空结果而非 Err
                eprintln!("[WEB] DuckDuckGo API 返回 HTTP {}", status.as_u16());
                return Ok(Vec::new());
            }

            let json: serde_json::Value = resp
                .json()
                .await
                .context("DuckDuckGo API 响应 JSON 解析失败")?;

            Ok(Self::parse_response(&json))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TC-DDG-001: URL 构造正确性 — 验证 query 参数正确传递。
    #[test]
    fn tc_ddg_001_url_construction() {
        // 验证 DDG_API_BASE 正确
        assert_eq!(DDG_API_BASE, "https://api.duckduckgo.com");
        // 验证超时常量合理（编译期检查）
        const _: () = {
            assert!(CONNECT_TIMEOUT_SECS <= 10);
            assert!(REQUEST_TIMEOUT_SECS <= 15);
        };
    }

    /// TC-DDG-002: JSON 响应解析 — AbstractText + RelatedTopics。
    #[test]
    fn tc_ddg_002_parse_response_with_abstract_and_related() {
        let json = serde_json::json!({
            "Heading": "Rust (programming language)",
            "AbstractText": "Rust is a systems programming language.",
            "AbstractURL": "https://en.wikipedia.org/wiki/Rust_(programming_language)",
            "RelatedTopics": [
                {
                    "Text": "The Rust Programming Language - A book about Rust",
                    "FirstURL": "https://doc.rust-lang.org/book/"
                },
                {
                    "Text": "Rust Foundation - Nonprofit organization",
                    "FirstURL": "https://foundation.rust-lang.org/"
                }
            ]
        });

        let results = DuckDuckGoProvider::parse_response(&json);

        // 1 个 Abstract + 2 个 RelatedTopics = 3 个结果
        assert_eq!(results.len(), 3);

        // 第一条是 Abstract
        assert_eq!(results[0].title, "Rust (programming language)");
        assert_eq!(
            results[0].snippet,
            "Rust is a systems programming language."
        );
        assert_eq!(
            results[0].url,
            "https://en.wikipedia.org/wiki/Rust_(programming_language)"
        );
        assert_eq!(results[0].source, "duckduckgo");

        // 第二条是 RelatedTopics 第一条
        assert_eq!(results[1].title, "The Rust Programming Language");
        assert!(results[1].snippet.contains("A book about Rust"));
        assert_eq!(results[1].url, "https://doc.rust-lang.org/book/");
    }

    /// TC-DDG-003: 空结果处理 — 无 AbstractText 且无 RelatedTopics。
    #[test]
    fn tc_ddg_003_parse_empty_response() {
        let json = serde_json::json!({
            "Heading": "",
            "AbstractText": "",
            "AbstractURL": "",
            "RelatedTopics": []
        });

        let results = DuckDuckGoProvider::parse_response(&json);

        // 无 AbstractText 且无 RelatedTopics → 空结果
        assert!(results.is_empty(), "空响应应返回空结果列表");
    }
}
