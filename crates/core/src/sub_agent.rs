//! 子代理舰队管理（借鉴 Bamboo-agent 子代理 + mailbox 通信）。
//!
//! ## 架构
//!
//! 主代理（CoordinatorEngine）将复杂查询分解为子任务，为每个子任务创建独立的
//! SubAgent。每个 SubAgent 有自己的 AgentEngine 实例和独立的检索上下文，
//! 通过 mailbox 消息传递与主代理协调。
//!
//! ## 消息流
//!
//! ```text
//! CoordinatorEngine
//!   ├── SubAgentFleet::dispatch([("agent_1", "法律风险分析"), ("agent_2", "条款对比")])
//!   │     ├── SubAgent 1: AgentEngine.run("法律风险分析") → mailbox → Result
//!   │     └── SubAgent 2: AgentEngine.run("条款对比") → mailbox → Result
//!   └── SubAgentFleet::join_all() → [(agent_name, result_text), ...]
//! ```
//!
//! ## 共享设计
//!
//! `Retriever` 和 `LLMProvider` trait 没有 `Clone` 约束，但子代理舰队需要
//! 共享同一检索器/LLM 实例。解决方案：在 `lib.rs` 中为 `Arc<R>` / `Arc<L>`
//! 添加 blanket impl，使 `Arc<R>: Retriever` / `Arc<L>: LLMProvider`。
//! `CoordinatorEngine` 内部将 retriever/llm 包装为 `Arc<R>` / `Arc<L>`，
//! 子代理通过 `Arc::clone()` 共享只读引用。

use std::collections::HashMap;

use anyhow::Result;
use echomind_models::RetrievalResult;
use futures::StreamExt;
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

/// 子代理间消息类型。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    /// 发送者 ID
    pub from: String,
    /// 接收者 ID（"coordinator" = 主代理）
    pub to: String,
    /// 消息类型
    pub kind: MessageKind,
    /// 消息内容
    pub payload: String,
    /// 时间戳（Unix 秒）
    pub timestamp: i64,
}

impl AgentMessage {
    /// 创建新消息。
    pub fn new(from: String, to: String, kind: MessageKind, payload: String) -> Self {
        Self {
            from,
            to,
            kind,
            payload,
            timestamp: chrono::Utc::now().timestamp(),
        }
    }
}

/// 消息类型。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MessageKind {
    /// 任务分配（主代理 → 子代理）
    Task,
    /// 结果返回（子代理 → 主代理）
    Result,
    /// 子代理向主代理查询
    Query,
    /// 主代理回复子代理
    Response,
    /// 终止指令（主代理 → 子代理）
    Terminate,
}

/// 子代理执行结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAgentResult {
    /// 子代理名称
    pub agent_name: String,
    /// 执行状态
    pub status: SubAgentStatus,
    /// 结果文本（成功时为 Agent 最终答案，失败时为错误信息）
    pub output: String,
    /// 聚合的引用来源
    pub sources: Vec<RetrievalResult>,
    /// 执行耗时（毫秒）
    pub duration_ms: u64,
}

/// 子代理执行状态。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SubAgentStatus {
    /// 成功完成
    Completed,
    /// 执行失败
    Failed,
    /// 被终止
    Terminated,
    /// 超时
    TimedOut,
}

/// 子代理：独立的 AgentEngine 实例 + mailbox 通信。
///
/// 每个子代理有自己的检索上下文和 ReAct 推理循环，
/// 互不干扰。通过 `tokio::sync::mpsc` channel 与主代理通信。
pub struct SubAgent {
    /// 子代理唯一标识
    pub id: String,
    /// 人类可读名称
    pub name: String,
    /// 分配的任务描述
    pub task: String,
    /// mailbox 发送端（子代理 → 主代理）
    mailbox_tx: mpsc::UnboundedSender<AgentMessage>,
}

impl SubAgent {
    /// 创建子代理，返回 (SubAgent, mailbox 接收端)。
    ///
    /// 调用方持有接收端，用于收集子代理发送的消息。
    pub fn new(
        id: String,
        name: String,
        task: String,
    ) -> (Self, mpsc::UnboundedReceiver<AgentMessage>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let agent = Self {
            id,
            name,
            task,
            mailbox_tx: tx,
        };
        (agent, rx)
    }

    /// 执行子任务：使用共享的检索器和 LLM 处理子查询。
    ///
    /// 每个子代理独立检索 + 生成简要回答，通过 mailbox 发送结果。
    /// 子代理共享 `&R` / `&L` 不可变引用（`Retriever: Send + Sync` / `LLMProvider: Send + Sync`
    /// 保证并发安全），无需 Clone 或 Arc。
    ///
    /// # 参数
    /// - `retriever`: 检索器引用（共享只读）
    /// - `llm`: LLM Provider 引用（共享只读）
    /// - `timeout_secs`: 超时秒数（None = 不超时）
    ///
    /// # 返回
    /// 通过 mailbox 发送 `SubAgentResult` 给主代理。方法本身返回 `()`。
    pub async fn run<R, L>(self, retriever: &R, llm: &L, timeout_secs: Option<u64>)
    where
        R: crate::Retriever,
        L: crate::LLMProvider,
    {
        let start = std::time::Instant::now();
        let agent_name = self.name.clone();
        let agent_name_err = agent_name.clone();
        let agent_id = self.id.clone();
        let tx = self.mailbox_tx.clone();
        let task = self.task;

        let run_future = async move {
            // 检索相关内容
            let sources = retriever.retrieve(&task, 5).await?;

            // 如果有检索结果，使用 LLM 生成简要回答
            let answer = if sources.is_empty() {
                String::new()
            } else {
                let context: String = sources
                    .iter()
                    .map(|s| s.chunk.content.as_str())
                    .collect::<Vec<_>>()
                    .join("\n\n");
                let prompt =
                    format!("根据以下信息简要回答问题。\n\n问题：{task}\n\n参考资料：\n{context}");
                let stream = llm
                    .chat_stream("你是一个知识库助手，请根据参考资料简要回答。", &[], &prompt)
                    .await?;
                collect_stream_to_string(stream).await
            };

            Ok::<_, anyhow::Error>(SubAgentResult {
                agent_name,
                status: SubAgentStatus::Completed,
                output: answer,
                sources,
                duration_ms: start.elapsed().as_millis() as u64,
            })
        };

        let result = match timeout_secs {
            Some(secs) => {
                match tokio::time::timeout(std::time::Duration::from_secs(secs), run_future).await {
                    Ok(Ok(result)) => result,
                    Ok(Err(e)) => SubAgentResult {
                        agent_name: agent_name_err.clone(),
                        status: SubAgentStatus::Failed,
                        output: format!("子代理执行失败: {e:#}"),
                        sources: Vec::new(),
                        duration_ms: start.elapsed().as_millis() as u64,
                    },
                    Err(_) => SubAgentResult {
                        agent_name: agent_name_err.clone(),
                        status: SubAgentStatus::TimedOut,
                        output: format!("子代理超时（{secs}s）"),
                        sources: Vec::new(),
                        duration_ms: start.elapsed().as_millis() as u64,
                    },
                }
            }
            None => match run_future.await {
                Ok(result) => result,
                Err(e) => SubAgentResult {
                    agent_name: agent_name_err.clone(),
                    status: SubAgentStatus::Failed,
                    output: format!("子代理执行失败: {e:#}"),
                    sources: Vec::new(),
                    duration_ms: start.elapsed().as_millis() as u64,
                },
            },
        };

        // 通过 mailbox 发送结果给主代理
        let payload = serde_json::to_string(&result).unwrap_or_default();
        let _ = tx.send(AgentMessage::new(
            agent_id,
            "coordinator".to_string(),
            MessageKind::Result,
            payload,
        ));
    }
}

/// 子代理舰队：管理多个子代理的创建、派发和结果收集。
pub struct SubAgentFleet {
    /// 已派发的子代理数量
    agent_count: usize,
    /// 最大子代理数（默认 4）
    max_agents: usize,
    /// mailbox 接收端集合（主代理侧）
    receivers: Vec<mpsc::UnboundedReceiver<AgentMessage>>,
    /// 子代理 ID → 名称映射
    agent_names: HashMap<String, String>,
}

impl SubAgentFleet {
    /// 创建空舰队（最大 4 个子代理）。
    pub fn new() -> Self {
        Self::with_max_agents(4)
    }

    /// 创建指定最大代理数的舰队。
    pub fn with_max_agents(max_agents: usize) -> Self {
        Self {
            agent_count: 0,
            max_agents: max_agents.max(1),
            receivers: Vec::new(),
            agent_names: HashMap::new(),
        }
    }

    /// 派发子代理任务，返回创建的 SubAgent 列表。
    ///
    /// 调用方需自行 `tokio::spawn` 每个 SubAgent 的 `run()` 方法，
    /// 然后调用 `join_all()` 收集结果。
    ///
    /// # 参数
    /// - `tasks`: `(agent_name, task_description)` 列表
    ///
    /// # 错误
    /// 任务数超过 `max_agents` 时返回错误。
    pub fn dispatch(&mut self, tasks: Vec<(String, String)>) -> Result<Vec<SubAgent>> {
        if tasks.len() > self.max_agents {
            anyhow::bail!(
                "任务数 {} 超过最大子代理数 {}",
                tasks.len(),
                self.max_agents
            );
        }

        let mut agents = Vec::with_capacity(tasks.len());
        for (name, task) in tasks {
            self.agent_count += 1;
            let id = format!("sub_agent_{}", self.agent_count);
            let (agent, rx) = SubAgent::new(id.clone(), name.clone(), task);
            self.agent_names.insert(id, name);
            self.receivers.push(rx);
            agents.push(agent);
        }

        Ok(agents)
    }

    /// 等待所有子代理完成并收集结果。
    ///
    /// 从所有 mailbox 接收 Result 消息，反序列化为 `SubAgentResult`。
    /// 任一子代理失败不影响其他子代理的结果收集。
    ///
    /// # 返回
    /// `Vec<SubAgentResult>` — 各子代理的执行结果
    pub async fn join_all(&mut self) -> Vec<SubAgentResult> {
        let mut results = Vec::with_capacity(self.receivers.len());

        for rx in &mut self.receivers {
            while let Some(msg) = rx.recv().await {
                if msg.kind == MessageKind::Result {
                    if let Ok(result) = serde_json::from_str::<SubAgentResult>(&msg.payload) {
                        results.push(result);
                    }
                    break;
                }
            }
        }

        results
    }

    /// 已派发的子代理数量。
    pub fn count(&self) -> usize {
        self.agent_count
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.agent_count == 0
    }
}

impl Default for SubAgentFleet {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// 辅助函数
// ============================================================

/// 收集 token 流为完整字符串（用于子代理最终答案收集）。
async fn collect_stream_to_string(mut stream: BoxStream<'_, anyhow::Result<String>>) -> String {
    let mut result = String::new();
    while let Some(item) = stream.next().await {
        match item {
            Ok(token) => result.push_str(&token),
            Err(_) => break,
        }
    }
    result
}
