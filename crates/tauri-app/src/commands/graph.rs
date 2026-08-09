//! graph 域 IPC 命令子模块（从 commands.rs 拆分，纯重构）。
use super::*;

/// 设置知识图谱图遍历检索开关（REQ-RAG-027）。
///
/// 持久化到 settings 表 `rag.graph_retriever_enabled` 键，下次 chat 命令调用时即时生效。
/// 启用后，检索时沿实体关系图边扩展到关联 chunk，作为 RRF 融合的第四路检索通道。
#[tauri::command]
pub async fn set_graph_retriever_enabled(
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    set_graph_retriever_enabled_inner(enabled, state.inner()).await
}

/// 图遍历检索开关写入逻辑（命令与集成测试复用）。
pub async fn set_graph_retriever_enabled_inner(
    enabled: bool,
    state: &AppState,
) -> Result<(), String> {
    let value = if enabled { "true" } else { "false" };
    state
        .storage
        .set_setting("rag.graph_retriever_enabled", value)
        .await
        .map_err(|e| format!("{e:#}"))?;
    Ok(())
}

/// 获取图谱数据（全量三元组，limit 控制返回数量）。
///
/// 返回 `GraphTriple` 列表，前端 D3 force-directed graph 直接消费。
/// 默认 limit=200，避免大量数据导致前端渲染卡顿。
#[tauri::command]
pub async fn get_graph_data(
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<GraphTriple>, String> {
    get_graph_data_inner(limit, state.inner()).await
}

/// 图谱数据查询逻辑（命令与集成测试复用）。
pub async fn get_graph_data_inner(
    limit: Option<usize>,
    state: &AppState,
) -> Result<Vec<GraphTriple>, String> {
    let max = limit.unwrap_or(200);
    let relations = state
        .storage
        .list_all_relations(max, 0)
        .await
        .map_err(|e| format!("{e:#}"))?;
    Ok(relations.iter().map(|r| r.to_triple()).collect())
}

/// 获取指定实体的所有关系。
///
/// 返回 `EntityRelation` 列表，用于双击节点时展开该实体的详细关系列表。
#[tauri::command]
pub async fn get_entity_relations(
    entity_text: String,
    state: State<'_, AppState>,
) -> Result<Vec<EntityRelation>, String> {
    get_entity_relations_inner(entity_text, state.inner()).await
}

/// 实体关系查询逻辑（命令与集成测试复用）。
pub async fn get_entity_relations_inner(
    entity_text: String,
    state: &AppState,
) -> Result<Vec<EntityRelation>, String> {
    state
        .storage
        .get_relations_for_entity(&entity_text)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 获取图谱统计信息。
///
/// 返回 `GraphStats`（实体总数 / 关系总数 / 关系类型分布），
/// 用于前端图谱面板头部概览。
#[tauri::command]
pub async fn get_graph_stats(state: State<'_, AppState>) -> Result<GraphStats, String> {
    get_graph_stats_inner(state.inner()).await
}

/// 图谱统计查询逻辑（命令与集成测试复用）。
pub async fn get_graph_stats_inner(state: &AppState) -> Result<GraphStats, String> {
    let relations = state
        .storage
        .list_all_relations(100_000, 0)
        .await
        .map_err(|e| format!("{e:#}"))?;
    let total_relations = relations.len();
    // 去重统计实体
    let mut entities = std::collections::HashSet::new();
    let mut relation_type_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for r in &relations {
        entities.insert(r.subject.clone());
        entities.insert(r.object.clone());
        *relation_type_counts
            .entry(r.relation_type.clone())
            .or_insert(0) += 1;
    }
    Ok(GraphStats {
        total_entities: entities.len(),
        total_relations,
        relation_type_counts,
    })
}

/// 批量查询实体类型（REQ-RAG-027 前端图谱可视化增强）。
///
/// 接收实体文本列表，返回 `HashMap<entity_text, entity_type>` 映射，
/// 用于前端图谱面板为每个节点渲染对应的实体类型图标。
/// 后端使用 SQL `WHERE entity_text IN (...)` 单次批量查询，避免 N+1 问题。
#[tauri::command]
pub async fn get_entity_types(
    entities: Vec<String>,
    state: State<'_, AppState>,
) -> Result<std::collections::HashMap<String, String>, String> {
    get_entity_types_inner(entities, state.inner()).await
}

/// 实体类型批量查询逻辑（命令与集成测试复用）。
pub async fn get_entity_types_inner(
    entities: Vec<String>,
    state: &AppState,
) -> Result<std::collections::HashMap<String, String>, String> {
    state
        .storage
        .get_entity_types(&entities)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 获取两个实体间的最短路径（REQ-RAG-027 AC-32~AC-33）。
///
/// 在后端获取全量邻接表后，使用 BFS 计算最短路径。
/// 返回 `GraphPath`（路径节点列表 + 跳数）。
#[tauri::command]
pub async fn get_shortest_path(
    from: String,
    to: String,
    state: State<'_, AppState>,
) -> Result<GraphPath, String> {
    get_shortest_path_inner(from, to, state.inner()).await
}

/// 最短路径计算逻辑（命令与集成测试复用）。
pub async fn get_shortest_path_inner(
    from: String,
    to: String,
    state: &AppState,
) -> Result<GraphPath, String> {
    let adjacency = state
        .storage
        .get_entity_graph()
        .await
        .map_err(|e| format!("{e:#}"))?;

    let path = echomind_core::graph_analyzer::GraphAnalyzer::shortest_path(&adjacency, &from, &to);
    let hops = path.len().saturating_sub(1);

    Ok(GraphPath { path, hops })
}

/// 社区检测（REQ-RAG-027 AC-38~AC-39）。
///
/// 在后端获取全量邻接表后，使用 Label Propagation 算法发现社区。
/// 返回 `GraphCommunity`（实体→社区 ID 映射 + 社区总数）。
#[tauri::command]
pub async fn get_communities(state: State<'_, AppState>) -> Result<GraphCommunity, String> {
    get_communities_inner(state.inner()).await
}

/// 社区检测逻辑（命令与集成测试复用）。
pub async fn get_communities_inner(state: &AppState) -> Result<GraphCommunity, String> {
    let adjacency = state
        .storage
        .get_entity_graph()
        .await
        .map_err(|e| format!("{e:#}"))?;

    let communities = echomind_core::graph_analyzer::GraphAnalyzer::detect_communities(&adjacency);
    let community_count = communities
        .values()
        .copied()
        .max()
        .map(|m| m + 1)
        .unwrap_or(0);

    Ok(GraphCommunity {
        communities,
        community_count,
    })
}

/// 获取图布局模式（REQ-RAG-027 AC-34~AC-36）。
///
/// 此命令为前端布局切换提供后端确认接口。
/// 布局计算在前端 D3.js 完成（force/hierarchical/radial），
/// 后端仅返回当前支持的布局模式列表，供前端校验。
#[tauri::command]
pub async fn get_graph_layout(mode: Option<String>) -> Result<Vec<String>, String> {
    get_graph_layout_inner(mode).await
}

/// 布局模式查询逻辑（命令与集成测试复用）。
pub async fn get_graph_layout_inner(mode: Option<String>) -> Result<Vec<String>, String> {
    let supported = vec![
        "force".to_string(),
        "hierarchical".to_string(),
        "radial".to_string(),
    ];

    // 如果传入 mode，验证是否在支持列表中
    if let Some(m) = mode
        && !supported.contains(&m)
    {
        return Err(format!(
            "不支持的布局模式: {m}（可选: force/hierarchical/radial）"
        ));
    }

    Ok(supported)
}

/// 导出知识图谱为 GraphML 或 JSON-LD 格式（REQ-EXP-006）。
///
/// 从数据库读取全量实体和关系数据，导出为标准图格式字符串。
/// 前端通过 Blob 下载文件。
///
/// # 参数
/// - `format`: 导出格式，`"graphml"` 或 `"jsonld"`
///
/// # 返回
/// 导出格式的字符串内容
#[tauri::command]
pub async fn export_graph(format: String, state: State<'_, AppState>) -> Result<String, String> {
    export_graph_inner(format, state.inner()).await
}

/// 知识图谱导出逻辑（命令与集成测试复用）。
///
/// 从 `entity_relations` 表读取全量关系，从关系中提取去重实体文本，
/// 再通过 `get_entity_types` 查询实体类型，构建 `Vec<Entity>`，
/// 最后调用 `export_graphml` 或 `export_jsonld` 生成格式字符串。
pub async fn export_graph_inner(format: String, state: &AppState) -> Result<String, String> {
    use echomind_core::graph_export::{export_graphml, export_jsonld};
    use echomind_models::Entity;

    // 读取全量关系
    let relations = state
        .storage
        .list_all_relations(usize::MAX, 0)
        .await
        .map_err(|e| format!("{e:#}"))?;

    // 从关系中提取去重实体文本
    let mut entity_texts: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for r in &relations {
        if seen.insert(r.subject.clone()) {
            entity_texts.push(r.subject.clone());
        }
        if seen.insert(r.object.clone()) {
            entity_texts.push(r.object.clone());
        }
    }

    // 查询实体类型
    let type_map = state
        .storage
        .get_entity_types(&entity_texts)
        .await
        .map_err(|e| format!("{e:#}"))?;

    // 构建 Vec<Entity>
    let entities: Vec<Entity> = entity_texts
        .iter()
        .map(|text| {
            let etype = type_map
                .get(text)
                .cloned()
                .unwrap_or_else(|| "unknown".to_string());
            Entity::new(text.clone(), etype)
        })
        .collect();

    // 根据 format 参数选择导出格式
    match format.as_str() {
        "graphml" => Ok(export_graphml(&entities, &relations)),
        "jsonld" => Ok(export_jsonld(&entities, &relations)),
        _ => Err(format!(
            "不支持的导出格式: {format}（可选: graphml/jsonld）"
        )),
    }
}
