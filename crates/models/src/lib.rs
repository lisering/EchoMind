//! EchoMind 数据契约层（六边形架构共享模型）。
//! 对应 SRS：REQ-ARCH-000；字段语义对应 REQ-ING-001（Document）、
//! REQ-VEC-001/004（Chunk / DocStatus）、REQ-RAG-002（RetrievalResult / ChatMessage）。
//! 本 crate 只定义纯数据结构，不含任何业务逻辑与外部服务依赖。

use serde::{Deserialize, Serialize};

/// 文档索引状态机：`Pending → Processing → Indexed / Failed(reason)`
/// 对应 REQ-VEC-004 的生命周期定义。
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
    /// 文档唯一标识（UUID v4）
    pub id: String,
    /// 入库后的本地路径
    pub file_path: String,
    /// 内容指纹（MD5，对应 REQ-ING-004 去重；v1.0.2 由 SHA-256 修订）
    pub file_hash: String,
    /// 索引状态
    pub status: DocStatus,
    /// 创建时间（Unix 秒级时间戳）
    pub created_at: i64,
}

impl Document {
    /// 创建新文档：自动生成 UUID 与时间戳，初始状态为 `Pending`。
    pub fn new(file_path: String, file_hash: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            file_path,
            file_hash,
            status: DocStatus::Pending,
            created_at: chrono::Utc::now().timestamp(),
        }
    }
}

/// 文档分块（对应 REQ-VEC-001 的元数据要求）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    /// 分块唯一标识（UUID v4）
    pub id: String,
    /// 所属文档 ID
    pub doc_id: String,
    /// 分块文本内容
    pub content: String,
    /// token 计数
    pub token_count: usize,
    /// 文档内顺序号
    pub sequence: usize,
}

impl Chunk {
    /// 创建新分块：自动生成 UUID。
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

/// 检索命中结果（对应 REQ-RAG-002 引用回溯）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalResult {
    /// 命中的分块
    pub chunk: Chunk,
    /// 相似度得分
    pub score: f32,
    /// 来源文档名（用于引用块展示）
    pub doc_name: String,
}

/// 对话消息（对应 REQ-RAG-001/004）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// 角色（user / assistant / system）
    pub role: String,
    /// 消息正文
    pub content: String,
    /// 助手消息的引用来源（用户消息为 None）
    pub sources: Option<Vec<RetrievalResult>>,
}

/// 文档索引状态事件负载（Tauri Event `doc-status-changed`，REQ-ING-001 冻结契约）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocStatusPayload {
    /// 状态标识：indexing / done / error
    pub status: String,
    /// 可读消息（进度说明或错误原因）
    pub message: String,
}
