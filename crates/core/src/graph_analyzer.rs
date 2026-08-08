//! 知识图谱高级分析引擎（REQ-RAG-027 Session 5）。
//!
//! 在 Session 1-4 的实体关系数据层 + 图遍历检索 + D3 可视化基础上，
//! 新增三大高级分析功能：
//!
//! 1. **最短路径分析**：BFS 查找两个实体节点间的最短路径
//! 2. **社区检测**：Label Propagation 算法自动发现实体社区
//! 3. **度中心性**：计算每个节点的度中心性，识别 hub 节点
//!
//! ## 设计约束
//!
//! - 算法接收邻接表 `HashMap<String, Vec<String>>` 作为输入，不依赖具体 Storage 实现
//! - 纯 Rust 实现，零外部依赖
//! - Label Propagation 迭代直到收敛或最大 10 轮
//! - 所有路径分析最大深度 5 防止指数爆炸
//! - 空图降级：所有分析返回空结果不报错
//!
//! ## 算法参考
//!
//! - BFS 最短路径：经典广度优先搜索，保证最短跳数
//! - Label Propagation: Raghavan et al. (2007) "Near-linear time algorithm for community detection"
//! - 度中心性：节点连接数 / (总节点数 - 1)

use std::collections::{HashMap, HashSet, VecDeque};

/// 所有路径分析的最大深度（防止指数爆炸）。
const MAX_PATH_DEPTH: usize = 5;

/// Label Propagation 最大迭代轮数。
const MAX_LP_ITERATIONS: usize = 10;

/// 图分析引擎（REQ-RAG-027 Session 5）。
///
/// 接收邻接表作为输入，提供最短路径、所有路径、社区检测、度中心性分析。
/// 无状态设计：每次分析独立计算，不缓存中间结果。
///
/// ## 空图降级
///
/// 如果邻接表为空，所有分析方法返回空结果（不报错）。
pub struct GraphAnalyzer;

impl GraphAnalyzer {
    /// 创建图分析引擎实例。
    ///
    /// 图分析引擎无状态，此方法仅用于 API 一致性。
    pub fn new() -> Self {
        Self
    }

    /// BFS 最短路径分析（REQ-RAG-027 AC-32~AC-33）。
    ///
    /// 使用广度优先搜索查找从 `from` 到 `to` 的最短路径。
    /// 保证返回的是跳数最少的路径。
    ///
    /// # 参数
    /// - `adjacency`: 邻接表 `HashMap<entity, Vec<neighbor>>`
    /// - `from`: 起点实体文本
    /// - `to`: 终点实体文本
    ///
    /// # 返回
    /// 最短路径上的实体节点列表（含起点和终点）。
    /// - 无路径时返回空 Vec
    /// - `from == to` 时返回单节点路径 `[from]`
    /// - `from` 或 `to` 不在图中时返回空 Vec
    pub fn shortest_path(
        adjacency: &HashMap<String, Vec<String>>,
        from: &str,
        to: &str,
    ) -> Vec<String> {
        // 空图或起点/终点不在图中
        if adjacency.is_empty() || !adjacency.contains_key(from) {
            return Vec::new();
        }

        // 自环：起点 == 终点
        if from == to {
            // 确认 from 在图中（已检查），返回单节点路径
            return vec![from.to_string()];
        }

        // BFS
        let mut visited: HashSet<String> = HashSet::new();
        let mut parent: HashMap<String, String> = HashMap::new();
        let mut queue: VecDeque<String> = VecDeque::new();

        visited.insert(from.to_string());
        queue.push_back(from.to_string());

        let mut found = false;
        while let Some(current) = queue.pop_front() {
            if current == to {
                found = true;
                break;
            }

            // 获取邻居列表（如果当前节点没有邻居，跳过）
            if let Some(neighbors) = adjacency.get(&current) {
                for neighbor in neighbors {
                    if visited.insert(neighbor.clone()) {
                        parent.insert(neighbor.clone(), current.clone());
                        queue.push_back(neighbor.clone());
                    }
                }
            }
        }

        if !found {
            return Vec::new();
        }

        // 回溯路径
        let mut path: Vec<String> = Vec::new();
        let mut current = to.to_string();
        path.push(current.clone());
        while let Some(p) = parent.get(&current) {
            path.push(p.clone());
            current = p.clone();
        }
        path.reverse();
        path
    }

    /// 所有路径分析（DFS，限制最大深度防止指数爆炸）。
    ///
    /// 查找从 `from` 到 `to` 的所有路径，最大深度 `max_depth`。
    /// 如果 `max_depth` 超过 `MAX_PATH_DEPTH`（5），自动限制为 5。
    ///
    /// # 参数
    /// - `adjacency`: 邻接表
    /// - `from`: 起点实体文本
    /// - `to`: 终点实体文本
    /// - `max_depth`: 最大路径深度（跳数上限）
    ///
    /// # 返回
    /// 所有 ≤ max_depth 跳的路径列表，每条路径是实体节点列表。
    pub fn all_paths(
        adjacency: &HashMap<String, Vec<String>>,
        from: &str,
        to: &str,
        max_depth: usize,
    ) -> Vec<Vec<String>> {
        if adjacency.is_empty() || !adjacency.contains_key(from) {
            return Vec::new();
        }

        let depth = max_depth.min(MAX_PATH_DEPTH);
        let mut results: Vec<Vec<String>> = Vec::new();
        let mut visited: HashSet<String> = HashSet::new();
        let mut current_path: Vec<String> = Vec::new();

        Self::dfs_paths(
            adjacency,
            from,
            to,
            depth,
            &mut visited,
            &mut current_path,
            &mut results,
        );

        results
    }

    /// DFS 递归查找所有路径（内部辅助）。
    fn dfs_paths(
        adjacency: &HashMap<String, Vec<String>>,
        current: &str,
        target: &str,
        remaining_depth: usize,
        visited: &mut HashSet<String>,
        current_path: &mut Vec<String>,
        results: &mut Vec<Vec<String>>,
    ) {
        if current_path.len() > remaining_depth + 1 {
            return;
        }

        current_path.push(current.to_string());
        visited.insert(current.to_string());

        if current == target {
            results.push(current_path.clone());
        } else if current_path.len() <= remaining_depth
            && let Some(neighbors) = adjacency.get(current)
        {
            for neighbor in neighbors {
                if !visited.contains(neighbor) {
                    Self::dfs_paths(
                        adjacency,
                        neighbor,
                        target,
                        remaining_depth,
                        visited,
                        current_path,
                        results,
                    );
                }
            }
        }

        current_path.pop();
        visited.remove(current);
    }

    /// Label Propagation 社区检测（REQ-RAG-027 AC-38~AC-39）。
    ///
    /// 使用 Label Propagation 算法自动发现实体社区。
    /// 每个节点初始标签为其自身 ID，每轮迭代中节点采用其邻居中出现频率最高的标签。
    /// 迭代直到收敛（无标签变化）或最大 10 轮。
    ///
    /// # 参数
    /// - `adjacency`: 邻接表
    ///
    /// # 返回
    /// `HashMap<entity, community_id>` — 实体 → 社区 ID 映射。
    /// 空图返回空 HashMap。
    pub fn detect_communities(adjacency: &HashMap<String, Vec<String>>) -> HashMap<String, usize> {
        if adjacency.is_empty() {
            return HashMap::new();
        }

        // 初始化：每个节点的标签为其自身索引
        let nodes: Vec<String> = adjacency.keys().cloned().collect();
        let mut labels: HashMap<String, usize> = HashMap::new();
        for (idx, node) in nodes.iter().enumerate() {
            labels.insert(node.clone(), idx);
        }

        // 迭代 Label Propagation
        for _iteration in 0..MAX_LP_ITERATIONS {
            let mut changed = false;
            let nodes_ordered: Vec<String> = nodes.clone();

            for node in &nodes_ordered {
                // 收集邻居的标签频率
                let mut label_counts: HashMap<usize, usize> = HashMap::new();
                if let Some(neighbors) = adjacency.get(node) {
                    for neighbor in neighbors {
                        if let Some(&label) = labels.get(neighbor) {
                            *label_counts.entry(label).or_insert(0) += 1;
                        }
                    }
                }

                if label_counts.is_empty() {
                    continue;
                }

                // 找到出现频率最高的标签
                let best_label = label_counts
                    .iter()
                    .max_by_key(|(_, count)| *count)
                    .map(|(label, _)| *label);

                if let Some(best) = best_label {
                    let current_label = labels.get(node).copied().unwrap_or(0);
                    if best != current_label {
                        labels.insert(node.clone(), best);
                        changed = true;
                    }
                }
            }

            if !changed {
                break;
            }
        }

        // 重新编号社区标签为连续的 0, 1, 2, ...
        let mut label_remap: HashMap<usize, usize> = HashMap::new();
        let mut next_id = 0usize;
        for node in &nodes {
            let label = *labels.get(node).unwrap_or(&0);
            use std::collections::hash_map::Entry;
            if let Entry::Vacant(e) = label_remap.entry(label) {
                e.insert(next_id);
                next_id += 1;
            }
        }
        for node in &nodes {
            if let Some(&label) = labels.get(node) {
                let remapped = *label_remap.get(&label).unwrap_or(&0);
                labels.insert(node.clone(), remapped);
            }
        }

        labels
    }

    /// 度中心性分析（REQ-RAG-027 AC-37）。
    ///
    /// 计算每个节点的度中心性：节点的连接数 / (总节点数 - 1)。
    /// 度中心性最高的节点是 hub 节点（连接最多的实体）。
    ///
    /// # 参数
    /// - `adjacency`: 邻接表
    ///
    /// # 返回
    /// `HashMap<entity, centrality>` — 实体 → 度中心性值 [0.0, 1.0]。
    /// 空图返回空 HashMap。单节点图返回 `{node: 0.0}`。
    pub fn degree_centrality(adjacency: &HashMap<String, Vec<String>>) -> HashMap<String, f32> {
        if adjacency.is_empty() {
            return HashMap::new();
        }

        let n = adjacency.len();
        let denominator = if n > 1 {
            (n - 1) as f32
        } else {
            // 单节点：度中心性为 0（没有其他节点可连接）
            let mut result = HashMap::new();
            let node = adjacency.keys().next();
            if let Some(node) = node {
                result.insert(node.clone(), 0.0);
            }
            return result;
        };

        let mut result: HashMap<String, f32> = HashMap::new();
        for (node, neighbors) in adjacency {
            // 度 = 邻居数（出度）+ 可能的入度（其他节点指向此节点的边）
            // 但由于邻接表是无向的（双向），度 = neighbors.len()
            // 不过为了更精确，统计所有指向此节点的边
            let degree = neighbors.len();
            result.insert(node.clone(), degree as f32 / denominator);
        }

        result
    }
}

impl Default for GraphAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}
