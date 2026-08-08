#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! 知识图谱导出模块测试（REQ-EXP-006）。
//!
//! 测试 ID 前缀: TC-EXP-GRAPH-

use crate::graph_export::{export_graphml, export_jsonld};
use echomind_models::{Entity, EntityRelation};

/// 构造测试用实体和关系数据。
fn make_test_data() -> (Vec<Entity>, Vec<EntityRelation>) {
    let entities = vec![
        Entity::new("Rust".to_string(), "tech_term".to_string()),
        Entity::new("Tauri".to_string(), "tech_term".to_string()),
        Entity::new("Cargo".to_string(), "identifier".to_string()),
    ];
    let relations = vec![
        EntityRelation::new(
            "Rust".to_string(),
            "uses".to_string(),
            "Cargo".to_string(),
            "chunk-1".to_string(),
            1.0,
        ),
        EntityRelation::new(
            "Tauri".to_string(),
            "depends_on".to_string(),
            "Rust".to_string(),
            "chunk-2".to_string(),
            0.9,
        ),
    ];
    (entities, relations)
}

/// TC-EXP-GRAPH-001: GraphML 导出格式正确。
///
/// 断言返回字符串含 `<?xml` 声明 + `<graphml` 根 + `<graph` + `<node` + `<edge`，
/// node 数量 = entities 数量，edge 数量 = relations 数量。
#[test]
fn tc_exp_graph_001_graphml_format_correct() {
    let (entities, relations) = make_test_data();

    let xml = export_graphml(&entities, &relations);

    // XML 声明
    assert!(xml.contains("<?xml"), "GraphML 应包含 XML 声明");
    // GraphML 根元素
    assert!(xml.contains("<graphml"), "GraphML 应包含 <graphml> 根元素");
    // graph 元素
    assert!(xml.contains("<graph"), "GraphML 应包含 <graph> 元素");
    // node 元素数量 = entities 数量
    let node_count = xml.matches("<node").count();
    assert_eq!(
        node_count,
        entities.len(),
        "GraphML node 数量应等于 entities 数量"
    );
    // edge 元素数量 = relations 数量
    let edge_count = xml.matches("<edge").count();
    assert_eq!(
        edge_count,
        relations.len(),
        "GraphML edge 数量应等于 relations 数量"
    );
    // 验证实体文本出现在 node 中
    for e in &entities {
        assert!(xml.contains(&e.text), "GraphML 应包含实体文本: {}", e.text);
    }
    // 验证 XML 特殊字符转义（使用含特殊字符的实体名）
    let special_entities = vec![Entity::new(
        "A & B < C > D".to_string(),
        "tech_term".to_string(),
    )];
    let special_xml = export_graphml(&special_entities, &[]);
    assert!(special_xml.contains("&amp;"), "GraphML 应转义 & 为 &amp;");
    assert!(special_xml.contains("&lt;"), "GraphML 应转义 < 为 &lt;");
    assert!(special_xml.contains("&gt;"), "GraphML 应转义 > 为 &gt;");
}

/// TC-EXP-GRAPH-002: JSON-LD 导出格式正确。
///
/// 断言 `serde_json::from_str` 成功，含 `@context` + `@graph` 数组，
/// 数组长度 = entities + relations。
#[test]
fn tc_exp_graph_002_jsonld_format_correct() {
    let (entities, relations) = make_test_data();

    let json_str = export_jsonld(&entities, &relations);

    // JSON 解析成功
    let parsed: serde_json::Value = serde_json::from_str(&json_str).expect("JSON-LD 应为有效 JSON");

    // 含 @context
    assert!(parsed.get("@context").is_some(), "JSON-LD 应包含 @context");

    // 含 @graph 数组
    let graph = parsed
        .get("@graph")
        .expect("JSON-LD 应包含 @graph")
        .as_array()
        .expect("@graph 应为数组");

    // 数组长度 = entities + relations
    assert_eq!(
        graph.len(),
        entities.len() + relations.len(),
        "JSON-LD @graph 数组长度应等于 entities + relations"
    );

    // 验证节点 @type 为 Entity
    let has_entity = graph.iter().any(|item| {
        item.get("@type")
            .and_then(|v| v.as_str())
            .is_some_and(|s| s == "Entity")
    });
    assert!(has_entity, "JSON-LD 应包含 @type 为 Entity 的节点");

    // 验证边 @type 为 Relation
    let has_relation = graph.iter().any(|item| {
        item.get("@type")
            .and_then(|v| v.as_str())
            .is_some_and(|s| s == "Relation")
    });
    assert!(has_relation, "JSON-LD 应包含 @type 为 Relation 的边");
}

/// TC-EXP-GRAPH-003: 空图谱导出不崩溃。
///
/// `export_graphml(&[], &[])` 返回有效 XML（含空 `<graph>`）；
/// `export_jsonld(&[], &[])` 返回有效 JSON（`@graph` 为空数组）。
#[test]
fn tc_exp_graph_003_empty_graph_no_crash() {
    // 空图谱 GraphML
    let empty_xml = export_graphml(&[], &[]);
    assert!(empty_xml.contains("<?xml"), "空 GraphML 应包含 XML 声明");
    assert!(
        empty_xml.contains("<graphml"),
        "空 GraphML 应包含 <graphml> 根元素"
    );
    assert!(
        empty_xml.contains("<graph"),
        "空 GraphML 应包含 <graph> 元素"
    );
    assert_eq!(
        empty_xml.matches("<node").count(),
        0,
        "空 GraphML 不应包含 node"
    );
    assert_eq!(
        empty_xml.matches("<edge").count(),
        0,
        "空 GraphML 不应包含 edge"
    );

    // 空图谱 JSON-LD
    let empty_json = export_jsonld(&[], &[]);
    let parsed: serde_json::Value =
        serde_json::from_str(&empty_json).expect("空 JSON-LD 应为有效 JSON");
    assert!(
        parsed.get("@context").is_some(),
        "空 JSON-LD 应包含 @context"
    );
    let graph = parsed
        .get("@graph")
        .expect("空 JSON-LD 应包含 @graph")
        .as_array()
        .expect("@graph 应为数组");
    assert!(graph.is_empty(), "空 JSON-LD @graph 应为空数组");
}
