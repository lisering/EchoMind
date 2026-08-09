#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! TDD 测试：TC-GRAPH-001~006 知识图谱图遍历检索器（REQ-RAG-027）。

use std::collections::HashMap;
use std::sync::Mutex;

use anyhow::Result;
use echomind_models::{
    ChatMessage, Chunk, Conversation, DocStatus, Document, EntityRelation, RetrievalResult,
};

use crate::Storage;
use crate::graph_retriever::{GraphRetriever, rrf_fuse_four_way};

// ───────────────────────── Mock Storage ─────────────────────────

/// 图遍历检索器测试用 Mock Storage。
///
/// 可分别控制 `get_relations_for_entity` 和 `get_chunk_by_id` 的返回结果。
/// 使用 `Mutex` 包装内部状态以实现 `Send + Sync`。
struct GraphMockStorage {
    /// entity_text → 该实体参与的关系列表
    relations: Mutex<HashMap<String, Vec<EntityRelation>>>,
    /// chunk_id → chunk 内容
    chunks: Mutex<HashMap<String, Chunk>>,
}

impl GraphMockStorage {
    fn new() -> Self {
        Self {
            relations: Mutex::new(HashMap::new()),
            chunks: Mutex::new(HashMap::new()),
        }
    }

    /// 添加一条关系（同时更新 subject 和 object 两个实体的关系索引）。
    fn add_relation(&self, relation: EntityRelation) {
        let mut rels = self.relations.lock().unwrap();
        rels.entry(relation.subject.clone())
            .or_default()
            .push(relation.clone());
        rels.entry(relation.object.clone())
            .or_default()
            .push(relation);
    }

    /// 添加一个 chunk。
    fn add_chunk(&self, chunk: Chunk) {
        self.chunks.lock().unwrap().insert(chunk.id.clone(), chunk);
    }
}

impl Storage for GraphMockStorage {
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
    async fn create_conversation(&self, _conversation: &Conversation) -> Result<()> {
        Ok(())
    }
    async fn list_conversations(&self, _workspace_id: &str) -> Result<Vec<Conversation>> {
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
        Ok(vec![])
    }
    async fn delete_chunks_by_doc(&self, _doc_id: &str) -> Result<()> {
        Ok(())
    }
    async fn keyword_search(&self, _query: &str, _top_k: usize) -> Result<Vec<RetrievalResult>> {
        Ok(vec![])
    }

    // 图遍历检索器使用的两个方法
    async fn get_relations_for_entity(&self, entity_text: &str) -> Result<Vec<EntityRelation>> {
        let rels = self.relations.lock().unwrap();
        Ok(rels.get(entity_text).cloned().unwrap_or_default())
    }

    async fn get_chunk_by_id(&self, chunk_id: &str) -> Result<Option<Chunk>> {
        let chunks = self.chunks.lock().unwrap();
        Ok(chunks.get(chunk_id).cloned())
    }
}

/// 创建测试用 chunk（指定 ID）。
fn make_chunk(chunk_id: &str, doc_id: &str, content: &str, seq: usize) -> Chunk {
    let mut chunk = Chunk::new(doc_id.to_string(), content.to_string(), content.len(), seq);
    chunk.id = chunk_id.to_string();
    chunk
}

/// 创建测试用关系。
fn make_relation(
    subject: &str,
    rel_type: &str,
    object: &str,
    chunk_id: &str,
    confidence: f32,
) -> EntityRelation {
    EntityRelation::new(
        subject.to_string(),
        rel_type.to_string(),
        object.to_string(),
        chunk_id.to_string(),
        confidence,
    )
}

// ───────────────────────── TC-GRAPH-001：单跳图扩展 ─────────────────────────

/// TC-GRAPH-001：单跳图扩展（entity → related chunk）。
///
/// 设置：chunk_c1 包含 "Rust is defined as a systems programming language"
/// 查询："What is Rust?"
/// 期望：图扩展返回 chunk_c1（包含 Rust 的关系）
#[tokio::test]
async fn tc_graph_001_single_hop_expansion() {
    let storage = GraphMockStorage::new();

    // chunk_c1 包含 Rust 的定义关系
    let chunk_c1 = make_chunk(
        "c1",
        "doc1",
        "Rust is defined as a systems programming language.",
        0,
    );
    storage.add_chunk(chunk_c1.clone());

    // 关系：Rust --defined_as--> systems，来源 chunk c1
    storage.add_relation(make_relation("Rust", "defined_as", "systems", "c1", 1.0));

    let retriever = GraphRetriever::new(storage);
    let results = retriever.expand("What is Rust?", 5).await.unwrap();

    assert!(!results.is_empty(), "图扩展应返回结果");
    assert_eq!(
        results[0].chunk.id, "c1",
        "单跳扩展应返回包含 Rust 关系的 chunk c1"
    );
    assert!(results[0].score > 0.0, "图扩展结果应有正分数（置信度加权）");
}

// ───────────────────────── TC-GRAPH-002：多跳图扩展 ─────────────────────────

/// TC-GRAPH-002：多跳图扩展（entity → entity → chunk）。
///
/// 设置：
/// - chunk_c1: "HashMap depends on Rust"（HashMap → Rust, chunk c1）
/// - chunk_c2: "Rust uses LLVM"（Rust → LLVM, chunk c2）
/// 查询："What does HashMap depend on?"
/// 期望：图扩展返回 c1（1 跳）和 c2（2 跳），c1 分数高于 c2
#[tokio::test]
async fn tc_graph_002_multi_hop_expansion() {
    let storage = GraphMockStorage::new();

    // chunk_c1: HashMap depends on Rust
    let chunk_c1 = make_chunk(
        "c1",
        "doc1",
        "HashMap depends on Rust for memory safety.",
        0,
    );
    storage.add_chunk(chunk_c1.clone());

    // chunk_c2: Rust uses LLVM
    let chunk_c2 = make_chunk("c2", "doc2", "Rust uses LLVM as its backend compiler.", 0);
    storage.add_chunk(chunk_c2.clone());

    // 关系 1：HashMap --depends_on--> Rust，来源 chunk c1
    storage.add_relation(make_relation("HashMap", "depends_on", "Rust", "c1", 1.0));

    // 关系 2：Rust --uses--> LLVM，来源 chunk c2
    storage.add_relation(make_relation("Rust", "uses", "LLVM", "c2", 1.0));

    let retriever = GraphRetriever::new(storage);
    let results = retriever
        .expand("What does HashMap depend on?", 5)
        .await
        .unwrap();

    assert_eq!(results.len(), 2, "多跳扩展应返回 2 个 chunk");

    // c1 是 1 跳（HashMap 直接匹配），c2 是 2 跳（Rust 间接匹配）
    let c1 = results.iter().find(|r| r.chunk.id == "c1");
    let c2 = results.iter().find(|r| r.chunk.id == "c2");

    assert!(c1.is_some(), "1 跳结果 c1 应存在");
    assert!(c2.is_some(), "2 跳结果 c2 应存在");

    // 1 跳分数应高于 2 跳分数（距离衰减）
    let c1_score = c1.unwrap().score;
    let c2_score = c2.unwrap().score;
    assert!(
        c1_score > c2_score,
        "1 跳分数 ({c1_score}) 应高于 2 跳分数 ({c2_score})"
    );
}

// ───────────────────────── TC-GRAPH-003：关系类型过滤 ─────────────────────────

/// TC-GRAPH-003：关系类型过滤（只沿 depends_on 扩展）。
///
/// 设置：
/// - chunk_c1: "Rust depends on LLVM"（depends_on 关系）
/// - chunk_c2: "Rust uses cargo"（uses 关系）
/// 查询："Rust"（匹配 Rust 实体）
/// 过滤：仅 depends_on
/// 期望：只返回 c1，不返回 c2
#[tokio::test]
async fn tc_graph_003_relation_type_filter() {
    let storage = GraphMockStorage::new();

    let chunk_c1 = make_chunk("c1", "doc1", "Rust depends on LLVM for code generation.", 0);
    let chunk_c2 = make_chunk("c2", "doc2", "Rust uses cargo as its package manager.", 0);
    storage.add_chunk(chunk_c1.clone());
    storage.add_chunk(chunk_c2.clone());

    // depends_on 关系
    storage.add_relation(make_relation("Rust", "depends_on", "LLVM", "c1", 1.0));

    // uses 关系
    storage.add_relation(make_relation("Rust", "uses", "cargo", "c2", 1.0));

    // 仅沿 depends_on 扩展
    let retriever = GraphRetriever::with_relation_filter(storage, vec!["depends_on".to_string()]);
    let results = retriever.expand("Tell me about Rust", 5).await.unwrap();

    assert_eq!(results.len(), 1, "关系类型过滤后应仅返回 1 个 chunk");
    assert_eq!(
        results[0].chunk.id, "c1",
        "应仅返回 depends_on 关系的 chunk c1，不返回 uses 关系的 c2"
    );
}

// ───────────────────────── TC-GRAPH-004：置信度加权排序 ─────────────────────────

/// TC-GRAPH-004：置信度加权排序。
///
/// 设置：
/// - chunk_c1: 高置信度关系（1.0）
/// - chunk_c2: 低置信度关系（0.5）
/// 查询："Rust"
/// 期望：c1 排在 c2 前面（高置信度优先）
#[tokio::test]
async fn tc_graph_004_confidence_weighted_scoring() {
    let storage = GraphMockStorage::new();

    let chunk_c1 = make_chunk("c1", "doc1", "Rust is defined as a systems language.", 0);
    let chunk_c2 = make_chunk("c2", "doc2", "Rust and Go are both modern languages.", 0);
    storage.add_chunk(chunk_c1.clone());
    storage.add_chunk(chunk_c2.clone());

    // c1: 精确匹配置信度 1.0
    storage.add_relation(make_relation("Rust", "defined_as", "systems", "c1", 1.0));

    // c2: 同句共现兜底置信度 0.5
    storage.add_relation(make_relation("Rust", "related_to", "Go", "c2", 0.5));

    let retriever = GraphRetriever::new(storage);
    let results = retriever.expand("What is Rust?", 5).await.unwrap();

    assert_eq!(results.len(), 2, "应返回 2 个 chunk");

    // 高置信度（1.0）应排在低置信度（0.5）前面
    assert_eq!(
        results[0].chunk.id, "c1",
        "高置信度 (1.0) 的 c1 应排在第一位"
    );
    assert_eq!(
        results[1].chunk.id, "c2",
        "低置信度 (0.5) 的 c2 应排在第二位"
    );
    assert!(
        results[0].score > results[1].score,
        "c1 分数 ({}) 应高于 c2 分数 ({})",
        results[0].score,
        results[1].score
    );
}

// ───────────────────────── TC-GRAPH-005：空图降级 ─────────────────────────

/// TC-GRAPH-005：空图降级（无关系时返回空 Vec）。
///
/// 设置：查询中无实体、或实体无关系
/// 期望：返回空 Vec（不报错），使管线优雅降级为三路 RRF
#[tokio::test]
async fn tc_graph_005_empty_graph_degradation() {
    // 场景 1：空查询
    let storage = GraphMockStorage::new();
    let retriever = GraphRetriever::new(storage);
    let results = retriever.expand("", 5).await.unwrap();
    assert!(results.is_empty(), "空查询应返回空 Vec");

    // 场景 2：查询有实体但无关系
    let storage2 = GraphMockStorage::new();
    let chunk = make_chunk("c1", "doc1", "Some random text without entities.", 0);
    storage2.add_chunk(chunk);
    let retriever2 = GraphRetriever::new(storage2);
    let results2 = retriever2.expand("Rust", 5).await.unwrap();
    assert!(
        results2.is_empty(),
        "有实体但无关系时应返回空 Vec（优雅降级）"
    );

    // 场景 3：关系指向的 chunk 不存在
    let storage3 = GraphMockStorage::new();
    // 添加关系但不添加对应的 chunk
    storage3.add_relation(make_relation(
        "Rust",
        "defined_as",
        "systems",
        "nonexistent_chunk",
        1.0,
    ));
    let retriever3 = GraphRetriever::new(storage3);
    let results3 = retriever3.expand("What is Rust?", 5).await.unwrap();
    assert!(results3.is_empty(), "关系指向不存在的 chunk 时应返回空 Vec");
}

// ───────────────────────── TC-GRAPH-006：图扩展结果与 RRF 融合 ─────────────────────────

/// TC-GRAPH-006：图扩展结果与 RRF 融合。
///
/// 测试四路 RRF 融合函数：Vector + BM25 + Entity + Graph。
/// 图扩展命中的 chunk 应获得额外的 RRF 分数加成。
#[test]
fn tc_graph_006_rrf_fusion_with_graph() {
    fn make_result(chunk_id: &str, score: f32) -> RetrievalResult {
        let mut chunk = Chunk::new("doc1".to_string(), format!("content_{chunk_id}"), 10, 0);
        chunk.id = chunk_id.to_string();
        RetrievalResult {
            chunk,
            score,
            doc_name: "test.md".to_string(),
        }
    }

    // 向量检索返回 c1, c2, c3
    let vector_results = vec![
        make_result("c1", 0.9),
        make_result("c2", 0.8),
        make_result("c3", 0.7),
    ];

    // 关键词检索返回 c2, c4
    let keyword_results = vec![make_result("c2", 0.85), make_result("c4", 0.75)];

    // 实体检索返回 c1, c3
    let entity_results = vec![make_result("c1", 0.6), make_result("c3", 0.55)];

    // 图扩展返回 c3, c5（c3 通过图扩展获得额外加成）
    let graph_results = vec![make_result("c3", 1.0), make_result("c5", 0.5)];

    let fused = rrf_fuse_four_way(
        vector_results,
        keyword_results,
        entity_results,
        graph_results,
        5,
    );

    assert_eq!(fused.len(), 5, "四路融合应返回 5 个唯一 chunk");

    // c1: Vector(rank 0) + Entity(rank 0) = 1/61 + 0.6/61 ≈ 0.0262
    // c2: Vector(rank 1) + Keyword(rank 0) = 1/62 + 0.8/61 ≈ 0.0291
    // c3: Vector(rank 2) + Entity(rank 1) + Graph(rank 0) = 1/63 + 0.6/62 + 0.5/61 ≈ 0.0341
    // c4: Keyword(rank 1) = 0.8/62 ≈ 0.0129
    // c5: Graph(rank 1) = 0.5/62 ≈ 0.0081

    // c3 应排第一（三路同时命中 + 图扩展加成）
    assert_eq!(
        fused[0].chunk.id, "c3",
        "c3（Vector+Entity+Graph 三路命中）应排第一"
    );

    // c5 应排在最后（仅图扩展一路命中，权重最低）
    assert_eq!(
        fused[4].chunk.id, "c5",
        "c5（仅 Graph 一路命中，权重 0.5）应排最后"
    );
}
