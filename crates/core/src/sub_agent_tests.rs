#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! 子代理舰队 TDD 测试（TC-SUBAGENT-001~008）。
//!
//! 验证 `SubAgent` / `SubAgentFleet` / mailbox 通信的完整行为：
//! - 单个子代理正确执行任务
//! - 多个子代理并行执行
//! - mailbox 消息正确传递
//! - max_agents 限制
//! - join_all 结果收集
//! - 失败容错
//! - 超时处理
//! - Fleet 计数

use std::time::Duration;

use anyhow::Result;
use echomind_models::{ChatMessage, RetrievalResult};
use futures::StreamExt;
use futures::future::join_all;
use futures::stream::BoxStream;

use crate::sub_agent::*;
use crate::{LLMProvider, Retriever};

// ================== Mock 实现 ==================

/// 固定回答 Mock LLM：始终返回同一回答。
struct MockLlm {
    answer: String,
}

impl MockLlm {
    fn new(answer: &str) -> Self {
        Self {
            answer: answer.to_string(),
        }
    }
}

impl LLMProvider for MockLlm {
    async fn chat_stream(
        &self,
        _system_prompt: &str,
        _history: &[ChatMessage],
        _query: &str,
    ) -> Result<BoxStream<'static, Result<String>>> {
        let answer = self.answer.clone();
        Ok(futures::stream::once(async move { Ok(answer) }).boxed())
    }
}

/// 空检索 Mock Retriever：始终返回空结果。
struct MockRetriever;

impl Retriever for MockRetriever {
    async fn retrieve(&self, _query: &str, _top_k: usize) -> Result<Vec<RetrievalResult>> {
        Ok(vec![])
    }
}

/// 失败 Mock LLM：chat_stream 始终返回 Err。
struct FailingLlm;

impl LLMProvider for FailingLlm {
    async fn chat_stream(
        &self,
        _system_prompt: &str,
        _history: &[ChatMessage],
        _query: &str,
    ) -> Result<BoxStream<'static, Result<String>>> {
        Err(anyhow::anyhow!("LLM 不可用"))
    }
}

/// 慢速 Mock LLM：延迟后返回回答（用于超时测试）。
struct SlowLlm {
    delay_secs: u64,
}

impl LLMProvider for SlowLlm {
    async fn chat_stream(
        &self,
        _system_prompt: &str,
        _history: &[ChatMessage],
        _query: &str,
    ) -> Result<BoxStream<'static, Result<String>>> {
        let delay = self.delay_secs;
        Ok(futures::stream::once(async move {
            tokio::time::sleep(Duration::from_secs(delay)).await;
            Ok("慢速回答".to_string())
        })
        .boxed())
    }
}

// ================== 测试用例 ==================

// TC-SUBAGENT-001：单个子代理正确执行任务
#[tokio::test]
async fn tc_subagent_001_single_agent_executes() {
    let (agent, mut rx) = SubAgent::new(
        "1".to_string(),
        "分析器".to_string(),
        "分析法律风险".to_string(),
    );

    let llm = MockLlm::new("最终答案");
    // 使用 tokio::join! 并发执行 run 和 recv
    let (_, msg) = tokio::join!(agent.run(&MockRetriever, &llm, None), async {
        rx.recv().await
    });

    let msg = msg.expect("应收到 mailbox 消息");
    assert_eq!(msg.kind, MessageKind::Result);
    assert_eq!(msg.to, "coordinator");

    let result: SubAgentResult = serde_json::from_str(&msg.payload).expect("应反序列化成功");
    assert_eq!(result.status, SubAgentStatus::Completed);
    assert_eq!(result.agent_name, "分析器");
}

// TC-SUBAGENT-002：多个子代理并行执行
#[tokio::test]
async fn tc_subagent_002_multiple_agents_parallel() {
    let mut fleet = SubAgentFleet::with_max_agents(3);

    let agents = fleet
        .dispatch(vec![
            ("agent_1".to_string(), "任务1".to_string()),
            ("agent_2".to_string(), "任务2".to_string()),
            ("agent_3".to_string(), "任务3".to_string()),
        ])
        .expect("dispatch 应成功");

    let llm = MockLlm::new("答案");
    let futures: Vec<_> = agents
        .into_iter()
        .map(|agent| agent.run(&MockRetriever, &llm, None))
        .collect();
    join_all(futures).await;

    let results = fleet.join_all().await;
    assert_eq!(results.len(), 3, "应收到 3 个结果");
}

// TC-SUBAGENT-003：mailbox 消息正确传递
#[tokio::test]
async fn tc_subagent_003_mailbox_message_fields() {
    let (agent, mut rx) = SubAgent::new(
        "agent_xyz".to_string(),
        "测试代理".to_string(),
        "测试任务".to_string(),
    );

    let llm = MockLlm::new("结果");
    let (_, msg) = tokio::join!(agent.run(&MockRetriever, &llm, None), async {
        rx.recv().await
    });

    let msg = msg.expect("应收到消息");

    // 验证消息字段
    assert_eq!(msg.from, "agent_xyz", "from 应为子代理 ID");
    assert_eq!(msg.to, "coordinator", "to 应为 coordinator");
    assert_eq!(msg.kind, MessageKind::Result, "kind 应为 Result");
    assert!(!msg.payload.is_empty(), "payload 不应为空");
    assert!(msg.timestamp > 0, "timestamp 应为有效 Unix 秒");
}

// TC-SUBAGENT-004：max_agents 限制生效
#[tokio::test]
async fn tc_subagent_004_max_agents_limit() {
    let mut fleet = SubAgentFleet::with_max_agents(2);

    let result = fleet.dispatch(vec![
        ("a1".to_string(), "t1".to_string()),
        ("a2".to_string(), "t2".to_string()),
        ("a3".to_string(), "t3".to_string()),
    ]);

    assert!(result.is_err(), "超过 max_agents 应返回 Err");
    let err_msg = match &result {
        Err(e) => e.to_string(),
        Ok(_) => String::new(),
    };
    assert!(
        err_msg.contains("超过最大子代理数"),
        "错误信息应包含限制说明: {err_msg}"
    );
}

// TC-SUBAGENT-005：join_all 收集所有结果
#[tokio::test]
async fn tc_subagent_005_join_all_collects_all() {
    let mut fleet = SubAgentFleet::with_max_agents(5);

    let agents = fleet
        .dispatch(vec![
            ("w1".to_string(), "任务一".to_string()),
            ("w2".to_string(), "任务二".to_string()),
            ("w3".to_string(), "任务三".to_string()),
        ])
        .expect("dispatch 应成功");

    let llm = MockLlm::new("ok");
    let futures: Vec<_> = agents
        .into_iter()
        .map(|agent| agent.run(&MockRetriever, &llm, None))
        .collect();
    join_all(futures).await;

    let results = fleet.join_all().await;
    assert_eq!(results.len(), 3, "join_all 应收集全部 3 个结果");

    // 所有结果应为 Completed 状态
    for r in &results {
        assert_eq!(
            r.status,
            SubAgentStatus::Completed,
            "每个子代理应成功完成: {:?}",
            r
        );
    }
}

// TC-SUBAGENT-006：子代理失败时返回 Failed 状态
#[tokio::test]
async fn tc_subagent_006_failure_returns_failed_status() {
    let (agent, mut rx) = SubAgent::new(
        "fail_agent".to_string(),
        "失败代理".to_string(),
        "不可能完成的任务".to_string(),
    );

    // MockRetriever 返回空，子代理不会调用 LLM，所以用 FailingRetriever 来触发失败
    // 实际上 MockRetriever 返回空 Vec（Ok），不会触发 Failed。
    // 我们需要 FailingRetriever 来让 retrieve 返回 Err。
    struct FailingRetriever;
    impl Retriever for FailingRetriever {
        async fn retrieve(&self, _q: &str, _k: usize) -> Result<Vec<RetrievalResult>> {
            Err(anyhow::anyhow!("检索失败"))
        }
    }

    let (_, msg) = tokio::join!(agent.run(&FailingRetriever, &FailingLlm, None), async {
        rx.recv().await
    });

    let msg = msg.expect("应收到消息");
    let result: SubAgentResult = serde_json::from_str(&msg.payload).expect("应反序列化成功");

    assert_eq!(result.status, SubAgentStatus::Failed, "应为 Failed 状态");
    assert!(
        result.output.contains("子代理执行失败"),
        "output 应包含错误信息: {}",
        result.output
    );
    assert_eq!(result.agent_name, "失败代理");
}

// TC-SUBAGENT-007：超时处理
#[tokio::test]
async fn tc_subagent_007_timeout_handling() {
    // 使用返回结果的 Retriever，但 LLM 很慢
    struct SourceRetriever;
    impl Retriever for SourceRetriever {
        async fn retrieve(&self, _q: &str, _k: usize) -> Result<Vec<RetrievalResult>> {
            Ok(vec![RetrievalResult {
                chunk: echomind_models::Chunk::new("c1".to_string(), "内容".to_string(), 5, 0),
                score: 0.9,
                doc_name: "doc.md".to_string(),
            }])
        }
    }

    let (agent, mut rx) = SubAgent::new(
        "slow_agent".to_string(),
        "慢速代理".to_string(),
        "需要长时间处理的任务".to_string(),
    );

    // timeout = 1 秒，SlowLlm 延迟 10 秒
    let slow = SlowLlm { delay_secs: 10 };
    let (_, msg) = tokio::join!(agent.run(&SourceRetriever, &slow, Some(1)), async {
        rx.recv().await
    });

    let msg = msg.expect("应收到消息");
    let result: SubAgentResult = serde_json::from_str(&msg.payload).expect("应反序列化成功");

    assert_eq!(
        result.status,
        SubAgentStatus::TimedOut,
        "应为 TimedOut 状态"
    );
    assert!(
        result.output.contains("超时"),
        "output 应包含超时信息: {}",
        result.output
    );
}

// TC-SUBAGENT-008：Fleet count / is_empty 正确
#[tokio::test]
async fn tc_subagent_008_fleet_count_and_empty() {
    let mut fleet = SubAgentFleet::new();

    // 初始状态
    assert!(fleet.is_empty(), "新舰队应为空");
    assert_eq!(fleet.count(), 0, "初始 count 应为 0");

    // dispatch 后
    let _agents = fleet
        .dispatch(vec![
            ("a1".to_string(), "t1".to_string()),
            ("a2".to_string(), "t2".to_string()),
            ("a3".to_string(), "t3".to_string()),
        ])
        .expect("dispatch 应成功");

    assert!(!fleet.is_empty(), "dispatch 后不应为空");
    assert_eq!(fleet.count(), 3, "dispatch 3 个后 count 应为 3");
}
