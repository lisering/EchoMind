//! 文档/导入领域数据契约。
//!
//! 对应 SRS：REQ-ING-001（Document）、REQ-VEC-001/004（Chunk / DocStatus）、
//! REQ-RAG-002（RetrievalResult）、REQ-PERF-006（Entity）、REQ-RAG-026（知识图谱）。

use serde::{Deserialize, Serialize};

/// 默认工作空间 ID（REQ-WS-001）。
pub fn default_workspace_id() -> String {
    "default".to_string()
}

/// 文档索引状态机：`Pending → Processing → Indexed / Failed(reason)`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocStatus {
    /// 待索引
    Pending,
    /// 索引中
    Processing,
    /// 已索引
    Indexed,
    /// 索引失败（携带原因）
    Failed(String),
}

/// 已导入文档（对应 REQ-ING-001）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub file_path: String,
    pub file_hash: String,
    pub status: DocStatus,
    pub created_at: i64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub original_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub domain: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "default_workspace_id")]
    pub workspace_id: String,
}

impl Document {
    pub fn new(file_path: String, file_hash: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            file_path,
            file_hash,
            status: DocStatus::Pending,
            created_at: chrono::Utc::now().timestamp(),
            original_path: None,
            domain: None,
            summary: None,
            tags: Vec::new(),
            workspace_id: default_workspace_id(),
        }
    }

    pub fn new_with_original_path(
        file_path: String,
        file_hash: String,
        original_path: String,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            file_path,
            file_hash,
            status: DocStatus::Pending,
            created_at: chrono::Utc::now().timestamp(),
            original_path: Some(original_path),
            domain: None,
            summary: None,
            tags: Vec::new(),
            workspace_id: default_workspace_id(),
        }
    }
}

/// 文档分块（对应 REQ-VEC-001 的元数据要求）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Chunk {
    pub id: String,
    pub doc_id: String,
    pub content: String,
    pub token_count: usize,
    pub sequence: usize,
}

impl Chunk {
    pub fn new(doc_id: String, content: String, token_count: usize, sequence: usize) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            doc_id,
            content,
            token_count,
            sequence,
        }
    }
}

/// 文档内容预览（REQ-ING-010）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentPreview {
    pub id: String,
    pub file_path: String,
    pub status: DocStatus,
    pub created_at: i64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub domain: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub file_hash: String,
    pub content_preview: String,
    pub chunks: Vec<ChunkPreview>,
    pub chunk_count: usize,
}

/// Chunk 预览项（REQ-ING-010）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkPreview {
    pub id: String,
    pub sequence: usize,
    pub content_preview: String,
    pub token_count: usize,
}

/// 检索命中结果（对应 REQ-RAG-002 引用回溯）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalResult {
    pub chunk: Chunk,
    pub score: f32,
    pub doc_name: String,
}

/// 网页搜索结果（REQ-RAG-036 网页搜索集成）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub source: String,
}

/// 抽取的命名实体（REQ-PERF-006 实体链接增强）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Entity {
    pub text: String,
    pub entity_type: String,
}

impl Entity {
    pub fn new(text: String, entity_type: String) -> Self {
        Self { text, entity_type }
    }
}

/// 实体间关系（知识图谱边，REQ-RAG-026）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityRelation {
    pub id: String,
    pub subject: String,
    pub relation_type: String,
    pub object: String,
    pub chunk_id: String,
    pub confidence: f32,
}

impl EntityRelation {
    pub fn new(
        subject: String,
        relation_type: String,
        object: String,
        chunk_id: String,
        confidence: f32,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            subject,
            relation_type,
            object,
            chunk_id,
            confidence,
        }
    }

    pub fn to_triple(&self) -> GraphTriple {
        GraphTriple {
            subject: self.subject.clone(),
            relation: self.relation_type.clone(),
            object: self.object.clone(),
        }
    }
}

/// 图谱三元组（用于检索结果展示，REQ-RAG-026）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphTriple {
    pub subject: String,
    pub relation: String,
    pub object: String,
}

/// Wiki 双向链接（REQ-ING-020 Markdown 笔记双向链接，Obsidian 风格）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiLink {
    pub id: String,
    pub source_doc_id: String,
    pub target: String,
    pub chunk_id: String,
    pub created_at: i64,
}

impl WikiLink {
    pub fn new(source_doc_id: String, target: String, chunk_id: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            source_doc_id,
            target,
            chunk_id,
            created_at: chrono::Utc::now().timestamp(),
        }
    }
}

/// 知识图谱统计信息（REQ-RAG-027 前端图谱可视化用）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStats {
    pub total_entities: usize,
    pub total_relations: usize,
    pub relation_type_counts: std::collections::HashMap<String, usize>,
}

/// 知识图谱路径分析结果（REQ-RAG-027 Session 5 路径分析）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphPath {
    pub path: Vec<String>,
    pub hops: usize,
}

/// 知识图谱社区检测结果（REQ-RAG-027 Session 5 社区检测）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphCommunity {
    pub communities: std::collections::HashMap<String, usize>,
    pub community_count: usize,
}

/// Proposition 级原子事实（REQ-PERF-007 Proposition 级原子分割）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposition {
    pub id: String,
    pub chunk_id: String,
    pub content: String,
    pub sequence: usize,
}

impl Proposition {
    pub fn new(chunk_id: String, content: String, sequence: usize) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            chunk_id,
            content,
            sequence,
        }
    }
}

/// 代码符号（tree-sitter AST 抽取，REQ-RAG-031 代码感知 RAG）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeSymbol {
    pub id: String,
    pub chunk_id: String,
    pub name: String,
    pub kind: SymbolKind,
    pub language: String,
    pub start_line: usize,
    pub end_line: usize,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub signature: Option<String>,
}

impl CodeSymbol {
    pub fn new(
        chunk_id: String,
        name: String,
        kind: SymbolKind,
        language: String,
        start_line: usize,
        end_line: usize,
        signature: Option<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            chunk_id,
            name,
            kind,
            language,
            start_line,
            end_line,
            signature,
        }
    }
}

/// 符号类型（对应 tree-sitter node kind，REQ-RAG-031）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    Function,
    Method,
    Class,
    Struct,
    Interface,
    Enum,
    Constant,
    Module,
}

impl SymbolKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            SymbolKind::Function => "function",
            SymbolKind::Method => "method",
            SymbolKind::Class => "class",
            SymbolKind::Struct => "struct",
            SymbolKind::Interface => "interface",
            SymbolKind::Enum => "enum",
            SymbolKind::Constant => "constant",
            SymbolKind::Module => "module",
        }
    }

    pub fn parse_str(s: &str) -> Self {
        match s {
            "function" => SymbolKind::Function,
            "method" => SymbolKind::Method,
            "class" => SymbolKind::Class,
            "struct" => SymbolKind::Struct,
            "interface" => SymbolKind::Interface,
            "enum" => SymbolKind::Enum,
            "constant" => SymbolKind::Constant,
            "module" => SymbolKind::Module,
            _ => {
                eprintln!("[WARN] SymbolKind::parse_str: 未知符号类型 '{s}'，回退为 Function");
                SymbolKind::Function
            }
        }
    }
}

/// RAPTOR 摘要树节点（REQ-PERF-009 多级摘要树索引）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryNode {
    pub id: String,
    pub doc_id: String,
    pub level: usize,
    pub content: String,
    pub child_ids: Vec<String>,
    pub embedding: Option<Vec<f32>>,
}

impl SummaryNode {
    pub fn new(doc_id: String, level: usize, content: String, child_ids: Vec<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            doc_id,
            level,
            content,
            child_ids,
            embedding: None,
        }
    }
}

/// 导入历史记录条目（REQ-ING-011）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportLogEntry {
    pub id: i64,
    pub timestamp: i64,
    pub file_name: String,
    pub format: String,
    pub result: String,
    pub error_message: Option<String>,
    pub file_size: Option<i64>,
}

/// 批量操作结果（REQ-ING-024 文档批量操作）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResult {
    pub success_count: usize,
    pub failed_count: usize,
    pub failed_ids: Vec<String>,
}

impl BatchResult {
    pub fn all_success(count: usize) -> Self {
        Self {
            success_count: count,
            failed_count: 0,
            failed_ids: Vec::new(),
        }
    }

    pub fn from_results(results: Vec<(String, Result<(), String>)>) -> Self {
        let mut success_count = 0;
        let mut failed_ids = Vec::new();
        for (id, result) in results {
            match result {
                Ok(()) => success_count += 1,
                Err(_) => failed_ids.push(id),
            }
        }
        let failed_count = failed_ids.len();
        Self {
            success_count,
            failed_count,
            failed_ids,
        }
    }
}
