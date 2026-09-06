//! DAG 工作流引擎数据契约。
//!
//! 对应 SRS：REQ-RAG-030。

use serde::{Deserialize, Serialize};

/// 工作流定义（DAG 有向无环图）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub nodes: Vec<WorkflowNode>,
    pub edges: Vec<WorkflowEdge>,
    pub created_at: i64,
}

impl Workflow {
    pub fn new(
        name: String,
        description: String,
        nodes: Vec<WorkflowNode>,
        edges: Vec<WorkflowEdge>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            description,
            nodes,
            edges,
            created_at: chrono::Utc::now().timestamp(),
        }
    }
}

/// 工作流节点。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowNode {
    pub id: String,
    pub label: String,
    pub node_type: NodeType,
}

/// 节点类型（决定执行行为）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum NodeType {
    Retrieval {
        top_k: usize,
        retrieval_mode: String,
    },
    Generation {
        system_prompt: String,
        model: Option<String>,
    },
    Condition {
        expression: String,
    },
    Aggregate {
        strategy: String,
    },
    Output {
        format: String,
    },
}

/// 工作流边（数据流方向）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowEdge {
    pub source: String,
    pub target: String,
    pub mapping: Option<String>,
}

/// 节点执行状态。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeStatus {
    Pending,
    Running,
    Completed { output: String },
    Failed { error: String },
    Skipped,
}

/// 工作流执行结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowResult {
    pub node_results: std::collections::HashMap<String, NodeStatus>,
    pub final_output: String,
    pub duration_ms: u64,
}
