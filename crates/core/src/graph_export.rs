//! 知识图谱导出模块（REQ-EXP-006）。
//!
//! 提供将知识图谱（实体 + 关系）导出为标准格式的纯函数：
//! - GraphML（XML 格式）：Gephi/Cytoscape/yEd 原生支持
//! - JSON-LD（JSON 格式）：Neo4j/语义 Web 工具支持
//!
//! ## 设计原则
//!
//! - 纯函数，无副作用，无 I/O 依赖
//! - 零新增依赖（`serde_json` 已在 core 中）
//! - GraphML XML 手动拼接（不引入 quick-xml），XML 特殊字符严格转义
//! - 空图谱（0 实体 0 关系）返回有效格式，不 panic

use echomind_models::{Entity, EntityRelation};

/// 转义 XML 特殊字符。
///
/// 将 `&` → `&amp;`、`<` → `&lt;`、`>` → `&gt;`、`"` → `&quot;`。
/// `>` 在 XML 规范中仅在 `]]>` 上下文需要转义，但为安全起见统一转义。
fn escape_xml(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => result.push_str("&amp;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '"' => result.push_str("&quot;"),
            _ => result.push(ch),
        }
    }
    result
}

/// 将知识图谱导出为 GraphML XML 格式。
///
/// GraphML 是图交换标准格式，Gephi/Cytoscape/yEd 原生支持。
///
/// # 参数
/// - `entities`: 全量实体列表（图节点）
/// - `relations`: 全量关系列表（图边）
///
/// # 返回
/// GraphML XML 字符串。空图谱返回包含空 `<graph>` 的有效 XML。
///
/// # 示例
///
/// ```
/// use echomind_core::graph_export::export_graphml;
/// use echomind_models::{Entity, EntityRelation};
///
/// let entities = vec![Entity::new("Rust".into(), "tech_term".into())];
/// let relations = vec![EntityRelation::new("Rust".into(), "uses".into(), "Cargo".into(), "c1".into(), 1.0)];
/// let xml = export_graphml(&entities, &relations);
/// assert!(xml.contains("<?xml"));
/// assert!(xml.contains("<node"));
/// assert!(xml.contains("<edge"));
/// ```
pub fn export_graphml(entities: &[Entity], relations: &[EntityRelation]) -> String {
    let mut xml = String::new();

    // XML 声明
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");

    // GraphML 根元素（带命名空间声明）
    xml.push_str("<graphml xmlns=\"http://graphml.graphdrawing.org/xmlns\">\n");

    // graph 元素：有向图
    xml.push_str("  <graph id=\"G\" edgedefault=\"directed\">\n");

    // 遍历 entities → <node>
    for entity in entities {
        let id = escape_xml(&entity.text);
        let etype = escape_xml(&entity.entity_type);
        xml.push_str(&format!("    <node id=\"{id}\">\n"));
        xml.push_str(&format!("      <data key=\"entity_type\">{etype}</data>\n"));
        xml.push_str("    </node>\n");
    }

    // 遍历 relations → <edge>
    for relation in relations {
        let source = escape_xml(&relation.subject);
        let target = escape_xml(&relation.object);
        let rtype = escape_xml(&relation.relation_type);
        let confidence = relation.confidence;
        xml.push_str(&format!(
            "    <edge source=\"{source}\" target=\"{target}\">\n"
        ));
        xml.push_str(&format!(
            "      <data key=\"relation_type\">{rtype}</data>\n"
        ));
        xml.push_str(&format!(
            "      <data key=\"confidence\">{confidence}</data>\n"
        ));
        xml.push_str("    </edge>\n");
    }

    // 关闭标签
    xml.push_str("  </graph>\n");
    xml.push_str("</graphml>\n");

    xml
}

/// 将知识图谱导出为 JSON-LD 格式。
///
/// JSON-LD 是 W3C 标准，Neo4j/语义 Web 工具支持。
/// 使用 `serde_json` 构建 `@context` + `@graph` 数组。
///
/// # 参数
/// - `entities`: 全量实体列表（图节点，`@type: "Entity"`）
/// - `relations`: 全量关系列表（图边，`@type: "Relation"`）
///
/// # 返回
/// JSON-LD 字符串（pretty-printed）。空图谱返回 `@graph` 为空数组的有效 JSON。
///
/// # 示例
///
/// ```
/// use echomind_core::graph_export::export_jsonld;
/// use echomind_models::{Entity, EntityRelation};
///
/// let entities = vec![Entity::new("Rust".into(), "tech_term".into())];
/// let relations = vec![EntityRelation::new("Rust".into(), "uses".into(), "Cargo".into(), "c1".into(), 1.0)];
/// let json = export_jsonld(&entities, &relations);
/// assert!(json.contains("@context"));
/// assert!(json.contains("@graph"));
/// assert!(json.contains("Entity"));
/// assert!(json.contains("Relation"));
/// ```
pub fn export_jsonld(entities: &[Entity], relations: &[EntityRelation]) -> String {
    let mut graph = Vec::with_capacity(entities.len() + relations.len());

    // 节点：实体
    for entity in entities {
        let node = serde_json::json!({
            "@id": entity.text,
            "@type": "Entity",
            "entityType": entity.entity_type,
        });
        graph.push(node);
    }

    // 边：关系
    for relation in relations {
        let edge = serde_json::json!({
            "@id": relation.id,
            "@type": "Relation",
            "subject": relation.subject,
            "relationType": relation.relation_type,
            "object": relation.object,
            "confidence": relation.confidence,
        });
        graph.push(edge);
    }

    // 构建 JSON-LD 根对象
    let jsonld = serde_json::json!({
        "@context": {
            "Entity": "https://echomind.local/ontology#Entity",
            "Relation": "https://echomind.local/ontology#Relation",
            "entityType": "https://echomind.local/ontology#entityType",
            "relationType": "https://echomind.local/ontology#relationType",
            "subject": "https://echomind.local/ontology#subject",
            "object": "https://echomind.local/ontology#object",
            "confidence": "https://echomind.local/ontology#confidence"
        },
        "@graph": graph
    });

    serde_json::to_string_pretty(&jsonld)
        .unwrap_or_else(|_| "{\n  \"@context\": {},\n  \"@graph\": []\n}".to_string())
}
