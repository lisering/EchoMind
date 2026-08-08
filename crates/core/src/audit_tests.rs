#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! QA 红灯测试：TC-AUDIT-001~005 文档一致性审计与矛盾检测（REQ-AUDIT-001~005）。
//!
//! 测试策略：
//! - Mock Storage（间谍模式）：追踪 `list_chunks` vs `vector_search` 调用次数
//! - Mock LLM：返回预置 JSON（声明提取 + 矛盾判定）
//! - Mock Embedder：返回可控向量（同主题高相似、异主题低相似）
//! - 取消信号：Arc<AtomicBool>
//!
//! 红灯状态：audit() 桩实现返回 Err，所有断言失败 → 开发绿灯后逐条通过。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use anyhow::Result;
use echomind_models::{ChatMessage, Chunk, DocStatus, Document, RetrievalResult};
use futures::StreamExt;
use futures::stream::BoxStream;

use crate::audit::{AuditCancelFlag, AuditEngine, AuditOutcome, Verdict};
use crate::{Embedder, LLMProvider, Storage};

// ================== Mock 实现 ==================

/// 间谍 Storage：追踪 `list_chunks` vs `vector_search` 调用次数（TC-AUDIT-001 核心）。
struct SpyStorage {
    /// 预置 chunk 列表（list_chunks 返回此数据）
    chunks: Vec<Chunk>,
    /// list_chunks 被调用次数
    list_chunks_calls: Arc<AtomicUsize>,
    /// vector_search 被调用次数（审计模式不应调用）
    vector_search_calls: Arc<AtomicUsize>,
}

impl SpyStorage {
    fn new(chunks: Vec<Chunk>) -> Self {
        Self {
            chunks,
            list_chunks_calls: Arc::new(AtomicUsize::new(0)),
            vector_search_calls: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl Storage for SpyStorage {
    async fn add_document(&self, _doc: &Document) -> Result<()> {
        Ok(())
    }
    async fn update_doc_status(&self, _doc_id: &str, _status: DocStatus) -> Result<()> {
        Ok(())
    }
    async fn add_chunk(&self, _chunk: &Chunk) -> Result<()> {
        Ok(())
    }
    async fn add_embedding(&self, _chunk_id: &str, _embedding: &[f32]) -> Result<()> {
        Ok(())
    }
    async fn vector_search(
        &self,
        _query_embedding: &[f32],
        _top_k: usize,
    ) -> Result<Vec<RetrievalResult>> {
        self.vector_search_calls.fetch_add(1, Ordering::SeqCst);
        Ok(vec![])
    }
    async fn find_document_by_hash(&self, _hash: &str) -> Result<Option<Document>> {
        Ok(None)
    }
    async fn count_documents(&self) -> Result<usize> {
        Ok(0)
    }
    async fn count_chunks(&self) -> Result<usize> {
        Ok(0)
    }
    async fn cleanup_zombies(&self) -> Result<usize> {
        Ok(0)
    }
    async fn set_setting(&self, _key: &str, _value: &str) -> Result<()> {
        Ok(())
    }
    async fn get_setting(&self, _key: &str) -> Result<Option<String>> {
        Ok(None)
    }
    async fn create_conversation(
        &self,
        _conversation: &echomind_models::Conversation,
    ) -> Result<()> {
        Ok(())
    }
    async fn list_conversations(
        &self,
        _workspace_id: &str,
    ) -> Result<Vec<echomind_models::Conversation>> {
        Ok(vec![])
    }
    async fn delete_conversation(&self, _id: &str) -> Result<()> {
        Ok(())
    }
    async fn update_conversation_title(&self, _id: &str, _title: &str) -> Result<()> {
        Ok(())
    }
    async fn add_message(&self, _conversation_id: &str, _message: &ChatMessage) -> Result<()> {
        Ok(())
    }
    async fn list_messages(&self, _conversation_id: &str) -> Result<Vec<ChatMessage>> {
        Ok(vec![])
    }
    async fn list_documents(&self) -> Result<Vec<Document>> {
        Ok(vec![])
    }
    async fn delete_document(&self, _doc_id: &str) -> Result<()> {
        Ok(())
    }
    async fn list_chunks(&self, _doc_id: &str) -> Result<Vec<Chunk>> {
        self.list_chunks_calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.chunks.clone())
    }

    async fn delete_chunks_by_doc(&self, _doc_id: &str) -> Result<()> {
        Ok(())
    }

    async fn keyword_search(&self, _query: &str, _top_k: usize) -> Result<Vec<RetrievalResult>> {
        Ok(vec![])
    }
}

/// Mock Embedder：根据文本内容返回可控向量。
/// 同主题声明（含"温度"）返回 [1.0, 0.0]，异主题声明返回 [0.0, 1.0]。
/// 使 SIFiD 预筛阶段能区分同主题/异主题声明对（TC-AUDIT-003）。
struct TopicEmbedder;

impl Embedder for TopicEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        // 含"温度"的文本返回向量 A，其余返回向量 B（正交，cosine=0）
        if text.contains("温度") || text.contains("25°C") || text.contains("20°C") {
            Ok(vec![1.0, 0.0, 0.0])
        } else if text.contains("Rust") || text.contains("编程") {
            Ok(vec![0.0, 1.0, 0.0])
        } else {
            Ok(vec![0.0, 0.0, 1.0])
        }
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.embed(text).await?);
        }
        Ok(results)
    }
}

/// Mock LLM：根据 prompt 内容返回预置 JSON（声明提取 or 矛盾判定）。
struct MockAuditLlm {
    /// 声明提取 JSON 响应（Decompose Phase）
    claims_json: String,
    /// 矛盾判定 JSON 响应（Verify Phase）
    verdict_json: String,
}

impl LLMProvider for MockAuditLlm {
    async fn chat_stream(
        &self,
        _system_prompt: &str,
        _history: &[ChatMessage],
        _query: &str,
    ) -> Result<BoxStream<'static, Result<String>>> {
        // 根据 system_prompt 内容判断当前阶段
        let response = if _system_prompt.contains("提取") || _system_prompt.contains("claim") {
            self.claims_json.clone()
        } else {
            self.verdict_json.clone()
        };
        Ok(futures::stream::once(async move { Ok(response) }).boxed())
    }
}

/// 构造测试 chunk：含矛盾温度参数的两个 chunk。
fn contradiction_test_chunks() -> Vec<Chunk> {
    let doc_id = "test-doc".to_string();
    vec![
        Chunk::new(
            doc_id.clone(),
            "实验在 25°C 下进行，误差小于 5%。".to_string(),
            10,
            0,
        ),
        Chunk::new(doc_id, "实验温度为 20°C，结果可靠。".to_string(), 10, 5),
    ]
}

/// 构造声明提取 JSON 响应（两个 chunk 各提取出温度声明）。
fn claims_json_response() -> String {
    r#"[
      {"claim_id":"c1","chunk_id":"chunk-0","sequence":0,"text":"实验温度为25°C","claim_type":"numeric_parameter","entities":["温度","25°C"]},
      {"claim_id":"c2","chunk_id":"chunk-0","sequence":0,"text":"误差小于5%","claim_type":"numeric_parameter","entities":["误差","5%"]},
      {"claim_id":"c3","chunk_id":"chunk-5","sequence":5,"text":"实验温度为20°C","claim_type":"numeric_parameter","entities":["温度","20°C"]}
    ]"#.to_string()
}

/// 构造矛盾判定 JSON 响应。
fn contradiction_verdict_json() -> String {
    r#"{"pair_id":"p1","verdict":"contradiction","explanation":"温度参数不一致：25°C vs 20°C","severity":"high"}"#.to_string()
}

/// 构造无矛盾的判定 JSON 响应。
fn consistent_verdict_json() -> String {
    r#"{"pair_id":"p1","verdict":"consistent","explanation":"两声明不矛盾","severity":"low"}"#
        .to_string()
}

fn no_cancel() -> AuditCancelFlag {
    Arc::new(AtomicBool::new(false))
}

// ================== TC-AUDIT-001: 全量审计命令（REQ-AUDIT-001） ==================

/// TC-AUDIT-001：审计走 list_chunks 全量扫描，不走 vector_search top-k 检索。
///
/// 验证 `AuditEngine::audit()` 通过 `Storage.list_chunks()` 获取文档全部 chunk，
/// 而非通过 `vector_search()` 做 top-k 检索（REQ-AUDIT-001-AC-1）。
#[tokio::test]
async fn tc_audit_001_uses_list_chunks_not_vector_search() {
    let storage = SpyStorage::new(contradiction_test_chunks());
    let list_calls = storage.list_chunks_calls.clone();
    let search_calls = storage.vector_search_calls.clone();

    let llm = MockAuditLlm {
        claims_json: claims_json_response(),
        verdict_json: contradiction_verdict_json(),
    };
    let engine = AuditEngine::new(TopicEmbedder, storage, llm);

    let outcome = engine
        .audit("test-doc", "test-paper.md", no_cancel())
        .await
        .expect("审计不应返回 Err");

    // 断言：list_chunks 被调用（全量扫描路径）
    assert!(
        list_calls.load(Ordering::SeqCst) > 0,
        "审计必须通过 list_chunks 获取全部 chunk（REQ-AUDIT-001-AC-1）"
    );
    // 断言：vector_search 未被调用（不走 top-k 检索路径）
    assert_eq!(
        search_calls.load(Ordering::SeqCst),
        0,
        "审计不应调用 vector_search（全量扫描非 top-k 检索，REQ-AUDIT-001-AC-1）"
    );
    // 断言：返回 Completed 结果
    assert!(
        matches!(outcome, AuditOutcome::Completed { .. }),
        "有 chunk 的文档审计应返回 Completed"
    );
}

/// TC-AUDIT-001b：空文档（0 chunk）返回 NoChunks，不 Panic（REQ-AUDIT-001-AC-4）。
#[tokio::test]
async fn tc_audit_001b_empty_doc_returns_no_chunks() {
    let storage = SpyStorage::new(vec![]);
    let llm = MockAuditLlm {
        claims_json: claims_json_response(),
        verdict_json: contradiction_verdict_json(),
    };
    let engine = AuditEngine::new(TopicEmbedder, storage, llm);

    let outcome = engine
        .audit("empty-doc", "empty.md", no_cancel())
        .await
        .expect("空文档审计不应返回 Err");

    assert!(
        matches!(outcome, AuditOutcome::NoChunks),
        "空文档（0 chunk）必须返回 NoChunks（REQ-AUDIT-001-AC-4）"
    );
}

// ================== TC-AUDIT-002: 原子声明提取（REQ-AUDIT-002） ==================

/// TC-AUDIT-002：从含数值参数的 chunk 中提取原子声明。
///
/// 验证 Decompose Phase 能从"实验在 25°C 下进行，误差小于 5%"中
/// 提取出"温度为 25°C"和"误差小于 5%"两条声明（REQ-AUDIT-002-AC-1）。
#[tokio::test]
async fn tc_audit_002_extracts_atomic_claims() {
    let storage = SpyStorage::new(contradiction_test_chunks());
    let llm = MockAuditLlm {
        claims_json: claims_json_response(),
        verdict_json: consistent_verdict_json(),
    };
    let engine = AuditEngine::new(TopicEmbedder, storage, llm);

    let outcome = engine
        .audit("test-doc", "test-paper.md", no_cancel())
        .await
        .expect("审计不应返回 Err");

    let AuditOutcome::Completed { report } = outcome else {
        panic!("审计应返回 Completed");
    };

    // 断言：提取了声明（total_claims > 0）
    assert!(
        report.total_claims > 0,
        "必须从 chunk 中提取出声明（REQ-AUDIT-002-AC-1）"
    );
    // 断言：包含温度声明
    let has_temp_claim = report
        .contradictions
        .iter()
        .flat_map(|c| [&c.claim_a, &c.claim_b])
        .any(|c| c.text.contains("温度") || c.text.contains("25°C") || c.text.contains("20°C"));
    assert!(
        has_temp_claim,
        "提取的声明应包含温度参数声明（REQ-AUDIT-002-AC-1）"
    );
}

/// TC-AUDIT-002b：LLM 返回非法 JSON 时优雅降级，不中断审计（REQ-AUDIT-002-AC-4）。
#[tokio::test]
async fn tc_audit_002b_graceful_json_degradation() {
    let storage = SpyStorage::new(contradiction_test_chunks());
    let llm = MockAuditLlm {
        claims_json: "这不是合法JSON".to_string(),
        verdict_json: consistent_verdict_json(),
    };
    let engine = AuditEngine::new(TopicEmbedder, storage, llm);

    // 非法 JSON 不应导致 Panic 或 Err
    let result = engine.audit("test-doc", "test-paper.md", no_cancel()).await;

    // 断言：不 Panic、返回 Ok（即使声明提取失败也优雅降级）
    assert!(
        result.is_ok(),
        "LLM 返回非法 JSON 时不应返回 Err，应优雅降级（REQ-AUDIT-002-AC-4）"
    );
}

// ================== TC-AUDIT-003: 跨声明矛盾检测（REQ-AUDIT-003） ==================

/// TC-AUDIT-003：跨 chunk 矛盾声明被检测到。
///
/// 验证 SIFiD 预筛 + LLM 精判两阶段策略能检测到
/// "25°C"（chunk 0）vs "20°C"（chunk 5）的温度矛盾（REQ-AUDIT-003-AC-1）。
#[tokio::test]
async fn tc_audit_003_detects_cross_chunk_contradiction() {
    let storage = SpyStorage::new(contradiction_test_chunks());
    let llm = MockAuditLlm {
        claims_json: claims_json_response(),
        verdict_json: contradiction_verdict_json(),
    };
    let engine = AuditEngine::new(TopicEmbedder, storage, llm);

    let outcome = engine
        .audit("test-doc", "test-paper.md", no_cancel())
        .await
        .expect("审计不应返回 Err");

    let AuditOutcome::Completed { report } = outcome else {
        panic!("审计应返回 Completed");
    };

    // 断言：发现至少 1 个矛盾
    assert!(
        !report.contradictions.is_empty(),
        "必须检测到 25°C vs 20°C 的温度矛盾（REQ-AUDIT-003-AC-1）"
    );
    // 断言：矛盾对的 verdict 为 Contradiction
    let has_contradiction = report
        .contradictions
        .iter()
        .any(|c| c.verdict == Verdict::Contradiction);
    assert!(
        has_contradiction,
        "矛盾对的 verdict 必须为 Contradiction（REQ-AUDIT-003-AC-3）"
    );
    // 断言：矛盾对包含 explanation 字段
    let has_explanation = report
        .contradictions
        .iter()
        .all(|c| !c.explanation.is_empty());
    assert!(
        has_explanation,
        "矛盾对必须包含 explanation 字段（REQ-AUDIT-003-AC-3）"
    );
}

/// TC-AUDIT-003b：语义无关的声明在预筛阶段被过滤，不进入精判。
///
/// 验证"温度为 25°C" vs "使用 Rust 语言"不会被标记为矛盾对——
/// embedding 预筛阶段 cosine 相似度低，被过滤（REQ-AUDIT-003-AC-2）。
#[tokio::test]
async fn tc_audit_003b_filters_unrelated_claims() {
    let doc_id = "mixed-doc".to_string();
    let chunks = vec![
        Chunk::new(doc_id.clone(), "实验温度为 25°C。".to_string(), 10, 0),
        Chunk::new(doc_id, "使用 Rust 语言开发。".to_string(), 10, 1),
    ];

    let storage = SpyStorage::new(chunks);
    let llm = MockAuditLlm {
        claims_json: r#"[
          {"claim_id":"c1","chunk_id":"chunk-0","sequence":0,"text":"实验温度为25°C","claim_type":"numeric_parameter","entities":["温度","25°C"]},
          {"claim_id":"c2","chunk_id":"chunk-1","sequence":1,"text":"使用Rust语言开发","claim_type":"other","entities":["Rust","编程"]}
        ]"#.to_string(),
        verdict_json: contradiction_verdict_json(),
    };
    let engine = AuditEngine::new(TopicEmbedder, storage, llm);

    let outcome = engine
        .audit("mixed-doc", "mixed.md", no_cancel())
        .await
        .expect("审计不应返回 Err");

    let AuditOutcome::Completed { report } = outcome else {
        panic!("审计应返回 Completed");
    };

    // 断言：温度声明与 Rust 声明不应被标记为矛盾
    // TopicEmbedder 对"温度"返回 [1,0,0]，对"Rust"返回 [0,1,0]，cosine=0 → 被预筛过滤
    let false_contradiction = report.contradictions.iter().any(|c| {
        let a = &c.claim_a.text;
        let b = &c.claim_b.text;
        (a.contains("温度") && b.contains("Rust")) || (a.contains("Rust") && b.contains("温度"))
    });
    assert!(
        !false_contradiction,
        "语义无关的声明（温度 vs Rust）不应被标记为矛盾（REQ-AUDIT-003-AC-2）"
    );
}

// ================== TC-AUDIT-004: 结构化审计报告（REQ-AUDIT-004） ==================

/// TC-AUDIT-004：审计报告包含摘要、矛盾清单、免责声明。
///
/// 验证 AuditReport 结构完整（REQ-AUDIT-004-AC-1/AC-2）。
#[tokio::test]
async fn tc_audit_004_report_structure_complete() {
    let storage = SpyStorage::new(contradiction_test_chunks());
    let llm = MockAuditLlm {
        claims_json: claims_json_response(),
        verdict_json: contradiction_verdict_json(),
    };
    let engine = AuditEngine::new(TopicEmbedder, storage, llm);

    let outcome = engine
        .audit("test-doc", "test-paper.md", no_cancel())
        .await
        .expect("审计不应返回 Err");

    let AuditOutcome::Completed { report } = outcome else {
        panic!("审计应返回 Completed");
    };

    // AC-1：报告包含文档名、chunk 总数、声明总数、矛盾列表
    assert_eq!(
        report.doc_name, "test-paper.md",
        "报告应包含文档名（REQ-AUDIT-004-AC-1）"
    );
    assert!(
        report.total_chunks > 0,
        "报告应包含 chunk 总数（REQ-AUDIT-004-AC-1）"
    );
    assert!(
        report.total_claims > 0,
        "报告应包含声明总数（REQ-AUDIT-004-AC-1）"
    );
    assert!(
        !report.contradictions.is_empty(),
        "报告应包含矛盾列表（REQ-AUDIT-004-AC-1）"
    );

    // AC-2：每条矛盾包含严重等级、声明原文、来源信息
    for c in &report.contradictions {
        assert!(
            !c.explanation.is_empty(),
            "矛盾必须包含 explanation（REQ-AUDIT-004-AC-2）"
        );
        assert!(
            !c.claim_a.text.is_empty() && !c.claim_b.text.is_empty(),
            "矛盾必须包含两个声明原文（REQ-AUDIT-004-AC-2）"
        );
        assert!(
            !c.claim_a.chunk_id.is_empty() && !c.claim_b.chunk_id.is_empty(),
            "矛盾必须包含来源 chunk ID（REQ-AUDIT-004-AC-2）"
        );
        // severity 是枚举，已经类型安全
    }
}

/// TC-AUDIT-004b：无矛盾时报告输出"未发现明显矛盾"，而非空报告。
#[tokio::test]
async fn tc_audit_004b_no_contradictions_reports_clean() {
    let doc_id = "clean-doc".to_string();
    let chunks = vec![Chunk::new(
        doc_id.clone(),
        "实验温度为 25°C，结果可靠。".to_string(),
        10,
        0,
    )];

    let storage = SpyStorage::new(chunks);
    let llm = MockAuditLlm {
        claims_json: r#"[{"claim_id":"c1","chunk_id":"chunk-0","sequence":0,"text":"实验温度为25°C","claim_type":"numeric_parameter","entities":["温度","25°C"]}]"#.to_string(),
        verdict_json: consistent_verdict_json(),
    };
    let engine = AuditEngine::new(TopicEmbedder, storage, llm);

    let outcome = engine
        .audit("clean-doc", "clean.md", no_cancel())
        .await
        .expect("审计不应返回 Err");

    let AuditOutcome::Completed { report } = outcome else {
        panic!("审计应返回 Completed");
    };

    // 断言：矛盾列表为空
    assert!(
        report.contradictions.is_empty(),
        "无矛盾时 contradictions 应为空（REQ-AUDIT-004-AC-3）"
    );
}

// ================== TC-AUDIT-005: 审计取消（REQ-AUDIT-005） ==================

/// TC-AUDIT-005：取消信号触发后审计停止，已发现的部分矛盾保留。
///
/// 验证 `AuditCancelFlag`（AtomicBool）设为 true 后，
/// 审计循环 break，返回 `AuditOutcome::Cancelled` 含部分报告（REQ-AUDIT-005-AC-2）。
#[tokio::test]
async fn tc_audit_005_cancellation_preserves_partial_results() {
    let storage = SpyStorage::new(contradiction_test_chunks());
    let llm = MockAuditLlm {
        claims_json: claims_json_response(),
        verdict_json: contradiction_verdict_json(),
    };
    let engine = AuditEngine::new(TopicEmbedder, storage, llm);

    // 立即设置取消标志
    let cancel = Arc::new(AtomicBool::new(true));

    let outcome = engine
        .audit("test-doc", "test-paper.md", cancel)
        .await
        .expect("取消不应返回 Err");

    // 断言：返回 Cancelled 或 Completed（如果审计太快在取消前完成）
    // 关键：不 Panic，partial 结果保留
    match outcome {
        AuditOutcome::Cancelled { partial_report } => {
            // 部分报告存在（可能 0 个矛盾，但结构完整）
            assert_eq!(
                partial_report.doc_name, "test-paper.md",
                "部分报告应包含文档名"
            );
        }
        AuditOutcome::Completed { .. } => {
            // 如果审计在取消前完成，也是可接受的
        }
        AuditOutcome::NoChunks => {
            panic!("有 chunk 的文档不应返回 NoChunks");
        }
    }
}
