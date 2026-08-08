//! HyDE 查询改写适配器（REQ-RAG-021）：使用 OpenAI 兼容 LLM 生成假设性答案文档。
//!
//! ## 原理
//!
//! HyDE（Hypothetical Document Embeddings，论文 arXiv:2212.10496）的核心思想：
//! 用户查询通常是简短的自然语言问题（如「Rust 的所有权机制是什么？」），
//! 而知识库中的文档片段是详细的答案文本。短查询与答案文档在嵌入空间中的
//! 距离较远，导致向量检索召回率低。
//!
//! HyDE 通过让 LLM 先生成一段假设性答案文档，再用该文档的嵌入进行向量检索。
//! 因为假设性答案在语义上更接近知识库中的实际答案片段，检索质量显著提升。
//!
//! ## 流程
//!
//! ```text
//! 用户查询 → HydeRewriter.rewrite(query)
//!   → OpenAIProvider.complete(hyde_prompt, query)  // 非流式 LLM 调用
//!   → 假设性答案文档（~200 字）
//! → HybridRetriever.embed(假设性答案文档)  // 向量检索使用改写后文本
//! → HybridRetriever.keyword_search(原始查询)  // 关键词检索仍用原始查询
//! → RRF 融合
//! ```
//!
//! ## 降级策略
//!
//! - LLM 调用失败（网络错误、超时、HTTP 非 2xx）→ 返回原始查询（优雅降级）
//! - LLM 返回空字符串 → 返回原始查询
//! - 改写超时（15s）→ 返回原始查询
//!
//! ## 隐私边界
//!
//! 查询文本仅发送到用户自行配置的 LLM 端点（BYOK），符合「隐私不出域」——
//! 数据出口由用户控制。与 chat 命令使用相同的 base_url / api_key / model。

use std::time::Duration;
use tracing::warn;

use echomind_core::QueryRewriter;

use crate::openai_provider::OpenAIProvider;

/// HyDE 改写请求超时时间（秒）。
/// 改写是一次性短文本生成（~200 字），不需要长超时。
/// 15 秒覆盖大部分 LLM 端点的首 token + 生成时间。
const HYDE_TIMEOUT_SECS: u64 = 15;

/// HyDE 系统提示词：引导 LLM 生成假设性答案文档。
///
/// 设计要点：
/// 1. 要求直接输出答案内容，不要包含「我不知道」等不确定性表述
/// 2. 限制 200 字以内，避免过长文本增加嵌入计算开销
/// 3. 不要求事实准确——假设性文档仅用于向量检索，不作为最终答案
/// 4. 保持与用户提问相同的语言，使嵌入与知识库语言一致
const HYDE_SYSTEM_PROMPT: &str = "\
你是一个知识库检索助手。请根据用户的问题，写一段简短的假设性答案文档（200字以内）。\
直接写出你认为的答案内容，不要回答「我不知道」或「需要更多信息」。\
即使答案不完全准确也可以——这段文字仅用于检索，不会展示给用户。\
请使用与用户提问相同的语言撰写答案。";

/// HyDE 查询改写器（REQ-RAG-021）。
///
/// 使用用户配置的 OpenAI 兼容 LLM 端点生成假设性答案文档，
/// 用该文档替代原始查询进行向量检索。
///
/// 改写后的文本仅用于向量检索（`embed()`），关键词检索仍使用原始查询。
///
/// # 降级策略
///
/// 如果 LLM 调用失败（网络错误、超时等），返回原始查询而非 Err，
/// 使检索管线优雅降级为无改写模式。
pub struct HydeRewriter {
    provider: OpenAIProvider,
}

impl HydeRewriter {
    /// 创建 HyDE 改写器。
    ///
    /// # 参数
    /// - `api_key`: LLM API Key（空字符串兼容本地 Ollama）
    /// - `base_url`: OpenAI 兼容端点 URL
    /// - `model`: 模型名称
    pub fn new(api_key: String, base_url: String, model: String) -> anyhow::Result<Self> {
        let provider = OpenAIProvider::new(api_key, base_url, model)?;
        Ok(Self { provider })
    }
}

impl QueryRewriter for HydeRewriter {
    fn rewrite<'a>(
        &'a self,
        query: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<String>> + Send + 'a>>
    {
        Box::pin(async move {
            // 使用 tokio::time::timeout 包装 LLM 调用，防止 HyDE 改写阻塞检索管线
            let result = tokio::time::timeout(
                Duration::from_secs(HYDE_TIMEOUT_SECS),
                self.provider.complete(HYDE_SYSTEM_PROMPT, query),
            )
            .await;

            match result {
                // 超时：返回原始查询（优雅降级）
                Err(_) => {
                    warn!("HyDE 改写超时（{HYDE_TIMEOUT_SECS}s），使用原始查询");
                    Ok(query.to_string())
                }
                // LLM 调用成功
                Ok(Ok(content)) => {
                    let trimmed = content.trim();
                    if trimmed.is_empty() {
                        warn!("HyDE 改写返回空内容，使用原始查询");
                        return Ok(query.to_string());
                    }
                    Ok(trimmed.to_string())
                }
                // LLM 调用失败：返回原始查询（优雅降级）
                Ok(Err(e)) => {
                    warn!("HyDE 改写失败，使用原始查询: {e:#}");
                    Ok(query.to_string())
                }
            }
        })
    }
}
