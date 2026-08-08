#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! TDD 测试：TC-ENT-001~008 NER 实体抽取器（REQ-PERF-006）。

use echomind_models::Entity;
use echomind_models::EntityRelation;

use crate::entity_extractor::EntityExtractor;

// ───────────────────────── 中文人名识别 ─────────────────────────

/// TC-ENT-001：中文人名识别（规则：常见姓氏 + 2-4 字组合）。
#[test]
fn tc_ent_001_chinese_person() {
    let text = "张三今天完成了代码审查，李明也提交了 PR。";
    let entities = EntityExtractor::extract(text);

    let persons: Vec<&Entity> = entities
        .iter()
        .filter(|e| e.entity_type == "person")
        .collect();
    let texts: Vec<&str> = persons.iter().map(|e| e.text.as_str()).collect();

    assert!(texts.contains(&"张三"), "应识别出人名「张三」");
    assert!(texts.contains(&"李明"), "应识别出人名「李明」");
}

// ───────────────────────── 英文专有名词识别 ─────────────────────────

/// TC-ENT-002：英文专有名词识别（大写首词，非句首）。
#[test]
fn tc_ent_002_proper_noun() {
    let text = "We use Rust and Python for backend. OpenAI provides GPT models.";
    let entities = EntityExtractor::extract(text);

    let nouns: Vec<&Entity> = entities
        .iter()
        .filter(|e| e.entity_type == "proper_noun")
        .collect();
    let texts: Vec<&str> = nouns.iter().map(|e| e.text.as_str()).collect();

    // "Rust" 和 "OpenAI" 是专有名词（非句首）
    assert!(texts.contains(&"Rust"), "应识别出专有名词「Rust」");
    assert!(texts.contains(&"OpenAI"), "应识别出专有名词「OpenAI」");
    // "We" 是句首 + 停用词，不应被识别
    assert!(!texts.contains(&"We"), "句首停用词「We」不应被识别");
    // "Python" 应被识别
    assert!(texts.contains(&"Python"), "应识别出专有名词「Python」");
}

// ───────────────────────── 技术术语识别 ─────────────────────────

/// TC-ENT-003：技术术语识别（camelCase/PascalCase/UPPER_SNAKE）。
#[test]
fn tc_ent_003_tech_term() {
    let text = "The HashMap uses HTTP_STATUS codes. fetchData returns JSON and HTTP format.";
    let entities = EntityExtractor::extract(text);

    let terms: Vec<&Entity> = entities
        .iter()
        .filter(|e| e.entity_type == "tech_term")
        .collect();
    let texts: Vec<&str> = terms.iter().map(|e| e.text.as_str()).collect();

    assert!(texts.contains(&"HashMap"), "应识别出 PascalCase「HashMap」");
    assert!(
        texts.contains(&"HTTP_STATUS"),
        "应识别出 UPPER_SNAKE「HTTP_STATUS」"
    );
    assert!(
        texts.contains(&"fetchData"),
        "应识别出 camelCase「fetchData」"
    );
    assert!(texts.contains(&"JSON"), "应识别出缩写「JSON」");
    assert!(texts.contains(&"HTTP"), "应识别出缩写「HTTP」");
}

// ───────────────────────── 数字标识符识别 ─────────────────────────

/// TC-ENT-004：数字标识符识别（版本号/错误码/IP）。
#[test]
fn tc_ent_004_identifier() {
    let text = "Upgrade to v2.0.0. Server at 192.168.1.1 returns error 404.";
    let entities = EntityExtractor::extract(text);

    let ids: Vec<&Entity> = entities
        .iter()
        .filter(|e| e.entity_type == "identifier")
        .collect();
    let texts: Vec<&str> = ids.iter().map(|e| e.text.as_str()).collect();

    assert!(texts.contains(&"v2.0.0"), "应识别出版本号「v2.0.0」");
    assert!(
        texts.contains(&"192.168.1.1"),
        "应识别出 IP 地址「192.168.1.1」"
    );
    assert!(texts.contains(&"404"), "应识别出错误码「404」");
}

// ───────────────────────── 实体去重 ─────────────────────────

/// TC-ENT-005：实体去重（同文档内重复实体只索引一次）。
#[test]
fn tc_ent_005_dedup() {
    let text = "Rust is fast. Rust is safe. Rust is concurrent.";
    let entities = EntityExtractor::extract(text);

    let rust_entities: Vec<&Entity> = entities.iter().filter(|e| e.text == "Rust").collect();
    assert_eq!(rust_entities.len(), 1, "重复的「Rust」实体应只保留一个");
}

// ───────────────────────── 实体关联到 chunk_id ─────────────────────────

/// TC-ENT-006：实体关联到 chunk_id。
#[test]
fn tc_ent_006_chunk_id_association() {
    let text = "OpenAI released GPT-4 with v1.2.3";
    let chunk_id = "chunk-abc-123";
    let pairs = EntityExtractor::extract_with_chunk_id(text, chunk_id);

    assert!(
        pairs.iter().all(|(_, _, cid)| cid == chunk_id),
        "所有实体都应关联到正确的 chunk_id"
    );
    assert!(!pairs.is_empty(), "应至少抽取到一个实体");
    // 验证三元组结构
    for (text, entity_type, cid) in &pairs {
        assert!(!text.is_empty(), "实体文本不应为空");
        assert!(!entity_type.is_empty(), "实体类型不应为空");
        assert_eq!(cid, chunk_id, "chunk_id 应正确关联");
    }
}

// ───────────────────────── 抽取不依赖 LLM ─────────────────────────

/// TC-ENT-007：抽取不依赖 LLM（纯规则）。
#[test]
fn tc_ent_007_no_llm_dependency() {
    // 纯 CPU 计算，无网络/模型依赖
    // 验证：传入任意文本都能立即返回结果，不阻塞
    let text = "The function parseJson handles HTTP requests.";
    let start = std::time::Instant::now();
    let entities = EntityExtractor::extract(text);
    let elapsed = start.elapsed();

    assert!(!entities.is_empty(), "应返回非空实体列表");
    // 应在 100ms 内完成（纯正则匹配）
    assert!(
        elapsed.as_millis() < 100,
        "纯规则抽取应在 100ms 内完成，实际 {}ms",
        elapsed.as_millis()
    );
}

// ───────────────────────── 空文本返回空列表 ─────────────────────────

/// TC-ENT-008：空文本返回空实体列表。
#[test]
fn tc_ent_008_empty_text() {
    let entities = EntityExtractor::extract("");
    assert!(entities.is_empty(), "空文本应返回空实体列表");

    let entities = EntityExtractor::extract("   ");
    assert!(entities.is_empty(), "纯空白文本应返回空实体列表");

    let entities = EntityExtractor::extract("\n\t\r");
    assert!(entities.is_empty(), "纯控制字符应返回空实体列表");
}

// ───────────────────────── 实体关系抽取（REQ-RAG-026） ─────────────────────────

/// TC-RELATION-001：英文 "X is defined as Y" → defined_as 关系。
#[test]
fn tc_relation_001_english_defined_as() {
    let text = "Rust is defined as Cargo.";
    let relations = EntityExtractor::extract_relations(text, "chunk-001");

    let defined_as: Vec<&EntityRelation> = relations
        .iter()
        .filter(|r| r.relation_type == "defined_as")
        .collect();

    assert!(
        !defined_as.is_empty(),
        "应识别出 defined_as 关系，实际找到 {} 条关系: {:?}",
        relations.len(),
        relations
            .iter()
            .map(|r| &r.relation_type)
            .collect::<Vec<_>>()
    );

    // 验证 subject 和 object
    let rel = defined_as[0];
    assert!(
        rel.subject.contains("Rust"),
        "subject 应包含 Rust，实际: {}",
        rel.subject
    );
    assert_eq!(rel.chunk_id, "chunk-001");
}

/// TC-RELATION-002：中文 "X 依赖 Y" → depends_on 关系。
#[test]
fn tc_relation_002_chinese_depends_on() {
    let text = "HashMap 依赖 Rust 标准库。";
    let relations = EntityExtractor::extract_relations(text, "chunk-002");

    let depends_on: Vec<&EntityRelation> = relations
        .iter()
        .filter(|r| r.relation_type == "depends_on")
        .collect();

    assert!(
        !depends_on.is_empty(),
        "应识别出 depends_on 关系，实际找到 {} 条关系: {:?}",
        relations.len(),
        relations
            .iter()
            .map(|r| &r.relation_type)
            .collect::<Vec<_>>()
    );

    let rel = depends_on[0];
    assert!(
        rel.subject.contains("HashMap"),
        "subject 应包含 HashMap，实际: {}",
        rel.subject
    );
    assert_eq!(rel.chunk_id, "chunk-002");
}

/// TC-RELATION-003：无关系的文本返回空 Vec。
#[test]
fn tc_relation_003_no_relations() {
    // 只有单个实体，无法形成关系
    let text = "Hello world.";
    let relations = EntityExtractor::extract_relations(text, "chunk-003");
    assert!(
        relations.is_empty(),
        "单个实体应返回空关系列表，实际: {:?}",
        relations
    );

    // 空文本
    let relations = EntityExtractor::extract_relations("", "chunk-003b");
    assert!(relations.is_empty(), "空文本应返回空关系列表");
}

/// TC-RELATION-004：一段包含多个关系的文本全部识别。
#[test]
fn tc_relation_004_multiple_relations() {
    let text = "Rust uses Cargo. Cargo depends on crates.io. Rust implements safety guarantees.";
    let relations = EntityExtractor::extract_relations(text, "chunk-004");

    assert!(
        relations.len() >= 2,
        "应识别出至少 2 条关系，实际: {} 条",
        relations.len()
    );

    // 验证关系类型多样性
    let rel_types: std::collections::HashSet<&str> =
        relations.iter().map(|r| r.relation_type.as_str()).collect();
    assert!(
        rel_types.len() >= 2,
        "应包含至少 2 种关系类型，实际: {:?}",
        rel_types
    );
}

/// TC-RELATION-005：置信度计算（精确匹配 1.0，模糊匹配 0.7）。
#[test]
fn tc_relation_005_confidence() {
    // 精确匹配："X is defined as Y" → 置信度 1.0
    let text_exact = "Rust is defined as Cargo.";
    let relations_exact = EntityExtractor::extract_relations(text_exact, "chunk-005a");

    let defined_as_exact: Vec<&EntityRelation> = relations_exact
        .iter()
        .filter(|r| r.relation_type == "defined_as")
        .collect();

    if let Some(rel) = defined_as_exact.first() {
        assert!(
            rel.confidence >= 1.0,
            "精确匹配置信度应 >= 1.0，实际: {}",
            rel.confidence
        );
    }

    // 模糊匹配："X defines Y" → 置信度 0.7
    let text_fuzzy = "Rust defines memory safety rules for Cargo.";
    let relations_fuzzy = EntityExtractor::extract_relations(text_fuzzy, "chunk-005b");

    let defined_as_fuzzy: Vec<&EntityRelation> = relations_fuzzy
        .iter()
        .filter(|r| r.relation_type == "defined_as" && r.confidence < 1.0)
        .collect();

    if let Some(rel) = defined_as_fuzzy.first() {
        assert!(
            (rel.confidence - 0.7).abs() < 0.01,
            "模糊匹配置信度应 ≈ 0.7，实际: {}",
            rel.confidence
        );
    }

    // related_to 兜底：置信度 0.5
    let related: Vec<&EntityRelation> = relations_exact
        .iter()
        .chain(relations_fuzzy.iter())
        .filter(|r| r.relation_type == "related_to")
        .collect();
    if let Some(rel) = related.first() {
        assert!(
            (rel.confidence - 0.5).abs() < 0.01,
            "同句共现兜底置信度应 ≈ 0.5，实际: {}",
            rel.confidence
        );
    }
}

/// TC-RELATION-006：关系去重（同一 chunk 内相同三元组只保留一个）。
#[test]
fn tc_relation_006_dedup() {
    // 同一句中重复出现的实体对应只产生一个关系
    let text = "Rust uses Cargo because Rust uses Cargo for building.";
    let relations = EntityExtractor::extract_relations(text, "chunk-006");

    // 统计 (subject, relation_type, object) 三元组
    let mut triples: std::collections::HashSet<(String, String, String)> =
        std::collections::HashSet::new();
    for rel in &relations {
        let triple = (
            rel.subject.clone(),
            rel.relation_type.clone(),
            rel.object.clone(),
        );
        // 如果已存在相同三元组，说明去重失败
        assert!(
            triples.insert(triple.clone()),
            "相同三元组不应重复出现: {:?}",
            triple
        );
    }
}
