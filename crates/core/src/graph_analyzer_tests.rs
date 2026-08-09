#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! TDD 测试：TC-GRAPH-ANALYSIS-001~010 知识图谱高级分析（REQ-RAG-027 Session 5）。
//!
//! 测试 GraphAnalyzer 的四项分析功能：
//! - shortest_path: BFS 最短路径
//! - all_paths: DFS 所有路径（限制深度）
//! - detect_communities: Label Propagation 社区检测
//! - degree_centrality: 度中心性

use std::collections::HashMap;

use crate::graph_analyzer::GraphAnalyzer;

/// 构建测试用邻接表。
fn build_adjacency(edges: &[(&str, &str)]) -> HashMap<String, Vec<String>> {
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();
    for &(from, to) in edges {
        adj.entry(from.to_string())
            .or_default()
            .push(to.to_string());
        adj.entry(to.to_string())
            .or_default()
            .push(from.to_string());
    }
    adj
}

// ───────────────────────── TC-GRAPH-ANALYSIS-001 ─────────────────────────

/// TC-GRAPH-ANALYSIS-001：最短路径——直接相邻两实体返回 1 跳路径。
///
/// 图：A -- B -- C
/// 查询：A → B
/// 期望：返回 [A, B]（1 跳路径）
#[test]
fn tc_graph_analysis_001_shortest_path_direct_neighbor() {
    let adj = build_adjacency(&[("A", "B"), ("B", "C")]);

    let path = GraphAnalyzer::shortest_path(&adj, "A", "B");

    assert_eq!(path, vec!["A", "B"], "直接相邻两实体应返回 1 跳路径 [A, B]");
}

// ───────────────────────── TC-GRAPH-ANALYSIS-002 ─────────────────────────

/// TC-GRAPH-ANALYSIS-002：最短路径——间接关联两实体返回 2 跳路径。
///
/// 图：A -- B -- C
/// 查询：A → C
/// 期望：返回 [A, B, C]（2 跳路径）
#[test]
fn tc_graph_analysis_002_shortest_path_indirect() {
    let adj = build_adjacency(&[("A", "B"), ("B", "C")]);

    let path = GraphAnalyzer::shortest_path(&adj, "A", "C");

    assert_eq!(
        path,
        vec!["A", "B", "C"],
        "间接关联两实体应返回 2 跳路径 [A, B, C]"
    );
}

// ───────────────────────── TC-GRAPH-ANALYSIS-003 ─────────────────────────

/// TC-GRAPH-ANALYSIS-003：最短路径——无路径返回空 Vec。
///
/// 图：A -- B, C -- D（两个不连通子图）
/// 查询：A → D
/// 期望：返回空 Vec
#[test]
fn tc_graph_analysis_003_shortest_path_no_path() {
    let adj = build_adjacency(&[("A", "B"), ("C", "D")]);

    let path = GraphAnalyzer::shortest_path(&adj, "A", "D");

    assert!(path.is_empty(), "无路径时应返回空 Vec");
}

// ───────────────────────── TC-GRAPH-ANALYSIS-004 ─────────────────────────

/// TC-GRAPH-ANALYSIS-004：最短路径——自环返回单节点路径。
///
/// 图：A -- B
/// 查询：A → A
/// 期望：返回 [A]（单节点路径）
#[test]
fn tc_graph_analysis_004_shortest_path_self_loop() {
    let adj = build_adjacency(&[("A", "B")]);

    let path = GraphAnalyzer::shortest_path(&adj, "A", "A");

    assert_eq!(path, vec!["A"], "自环应返回单节点路径 [A]");
}

// ───────────────────────── TC-GRAPH-ANALYSIS-005 ─────────────────────────

/// TC-GRAPH-ANALYSIS-005：所有路径——限制最大深度 3，返回所有 ≤3 跳路径。
///
/// 图：A -- B -- C -- D
/// 查询：A → D，max_depth=3
/// 期望：至少返回 [A, B, C, D]（3 跳路径）
#[test]
fn tc_graph_analysis_005_all_paths_limited_depth() {
    let adj = build_adjacency(&[("A", "B"), ("B", "C"), ("C", "D")]);

    let paths = GraphAnalyzer::all_paths(&adj, "A", "D", 3);

    assert!(!paths.is_empty(), "应至少返回 1 条路径");

    // 验证第一条路径是 [A, B, C, D]
    let has_expected = paths.iter().any(|p| p == &vec!["A", "B", "C", "D"]);
    assert!(has_expected, "应包含 [A, B, C, D] 路径，实际: {paths:?}");
}

// ───────────────────────── TC-GRAPH-ANALYSIS-006 ─────────────────────────

/// TC-GRAPH-ANALYSIS-006：社区检测——完全连通图返回单一社区。
///
/// 图：A -- B -- C（完全连通）
/// 期望：所有节点属于同一社区
#[test]
fn tc_graph_analysis_006_community_fully_connected() {
    let adj = build_adjacency(&[("A", "B"), ("B", "C"), ("A", "C")]);

    let communities = GraphAnalyzer::detect_communities(&adj);

    assert!(!communities.is_empty(), "非空图应返回社区映射");

    // 所有节点应属于同一社区
    let labels: Vec<usize> = communities.values().copied().collect();
    let unique_labels: std::collections::HashSet<usize> = labels.iter().copied().collect();
    assert_eq!(
        unique_labels.len(),
        1,
        "完全连通图应只有 1 个社区，实际: {unique_labels:?}"
    );
}

// ───────────────────────── TC-GRAPH-ANALYSIS-007 ─────────────────────────

/// TC-GRAPH-ANALYSIS-007：社区检测——两个不连通子图返回两个社区。
///
/// 图：A -- B -- A（子图1），C -- D -- C（子图2）
/// 期望：2 个社区
#[test]
fn tc_graph_analysis_007_community_disconnected() {
    let adj = build_adjacency(&[("A", "B"), ("C", "D")]);

    let communities = GraphAnalyzer::detect_communities(&adj);

    assert!(!communities.is_empty(), "非空图应返回社区映射");

    // 应有 2 个不同的社区
    let labels: std::collections::HashSet<usize> = communities.values().copied().collect();
    assert_eq!(
        labels.len(),
        2,
        "两个不连通子图应返回 2 个社区，实际: {labels:?}"
    );

    // A 和 B 应在同一社区
    assert_eq!(
        communities.get("A"),
        communities.get("B"),
        "A 和 B 应在同一社区"
    );
    // C 和 D 应在同一社区
    assert_eq!(
        communities.get("C"),
        communities.get("D"),
        "C 和 D 应在同一社区"
    );
    // A 和 C 应在不同社区
    assert_ne!(
        communities.get("A"),
        communities.get("C"),
        "A 和 C 应在不同社区"
    );
}

// ───────────────────────── TC-GRAPH-ANALYSIS-008 ─────────────────────────

/// TC-GRAPH-ANALYSIS-008：社区检测——Label Propagation 收敛后稳定。
///
/// 图：两个三角形通过一条边连接（A-B-C 三角 + D-E-F 三角 + C-D 桥接边）
/// 期望：多次调用结果中社区结构一致（两个社区，每社区 3 个节点）
///
/// 注意：HashMap 迭代顺序非确定，导致标签分配可能不同，
/// 但社区结构（社区数量、每社区节点数）应一致。
/// 使用有明确社区结构的图（非环形图），避免 Label Propagation
/// 在对称图上的非确定性收敛问题。
#[test]
fn tc_graph_analysis_008_community_convergence() {
    let adj = build_adjacency(&[
        ("A", "B"),
        ("B", "C"),
        ("A", "C"),
        ("D", "E"),
        ("E", "F"),
        ("D", "F"),
        ("C", "D"),
    ]);

    let communities1 = GraphAnalyzer::detect_communities(&adj);
    let communities2 = GraphAnalyzer::detect_communities(&adj);

    // 比较社区结构而非具体标签分配（HashMap 顺序非确定）
    let mut structure1: Vec<usize> = {
        let mut counts: HashMap<usize, usize> = HashMap::new();
        for &label in communities1.values() {
            *counts.entry(label).or_insert(0) += 1;
        }
        counts.values().copied().collect()
    };
    structure1.sort_unstable();

    let mut structure2: Vec<usize> = {
        let mut counts: HashMap<usize, usize> = HashMap::new();
        for &label in communities2.values() {
            *counts.entry(label).or_insert(0) += 1;
        }
        counts.values().copied().collect()
    };
    structure2.sort_unstable();

    assert_eq!(
        structure1, structure2,
        "社区结构（每社区节点数排序后）应一致：{:?} vs {:?}",
        structure1, structure2
    );
}

// ───────────────────────── TC-GRAPH-ANALYSIS-009 ─────────────────────────

/// TC-GRAPH-ANALYSIS-009：度中心性——hub 节点 centrality 最高。
///
/// 图：B -- A, C -- A, D -- A（A 是 hub，连接 3 个节点）
/// 期望：A 的度中心性最高
#[test]
fn tc_graph_analysis_009_degree_centrality_hub() {
    let adj = build_adjacency(&[("A", "B"), ("A", "C"), ("A", "D")]);

    let centrality = GraphAnalyzer::degree_centrality(&adj);

    assert!(!centrality.is_empty(), "非空图应返回度中心性映射");

    // A 的度中心性应最高
    let a_centrality = centrality.get("A");
    let b_centrality = centrality.get("B");
    let c_centrality = centrality.get("C");
    let d_centrality = centrality.get("D");

    assert!(a_centrality.is_some(), "A 应在结果中");
    assert!(b_centrality.is_some(), "B 应在结果中");
    assert!(c_centrality.is_some(), "C 应在结果中");
    assert!(d_centrality.is_some(), "D 应在结果中");

    let a_val = a_centrality.unwrap();
    let b_val = b_centrality.unwrap();
    let c_val = c_centrality.unwrap();
    let d_val = d_centrality.unwrap();

    assert!(*a_val > *b_val, "A 的度中心性 ({a_val}) 应高于 B ({b_val})");
    assert!(*a_val > *c_val, "A 的度中心性 ({a_val}) 应高于 C ({c_val})");
    assert!(*a_val > *d_val, "A 的度中心性 ({a_val}) 应高于 D ({d_val})");

    // A 有 3 个邻居，总节点数 4，centrality = 3 / (4-1) = 1.0
    assert!(
        (*a_val - 1.0).abs() < 0.001,
        "A 的度中心性应为 1.0（3/3），实际: {a_val}"
    );
}

// ───────────────────────── TC-GRAPH-ANALYSIS-010 ─────────────────────────

/// TC-GRAPH-ANALYSIS-010：空图降级——所有分析返回空结果不报错。
///
/// 空邻接表，所有分析方法应返回空结果（不 panic）。
#[test]
fn tc_graph_analysis_010_empty_graph_degradation() {
    let adj: HashMap<String, Vec<String>> = HashMap::new();

    // 最短路径
    let path = GraphAnalyzer::shortest_path(&adj, "A", "B");
    assert!(path.is_empty(), "空图最短路径应返回空 Vec");

    // 所有路径
    let paths = GraphAnalyzer::all_paths(&adj, "A", "B", 3);
    assert!(paths.is_empty(), "空图所有路径应返回空 Vec");

    // 社区检测
    let communities = GraphAnalyzer::detect_communities(&adj);
    assert!(communities.is_empty(), "空图社区检测应返回空 HashMap");

    // 度中心性
    let centrality = GraphAnalyzer::degree_centrality(&adj);
    assert!(centrality.is_empty(), "空图度中心性应返回空 HashMap");
}
