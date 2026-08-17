//! EchoMind 数据契约层（六边形架构共享模型）。
//! 对应 SRS：REQ-ARCH-000；字段语义对应 REQ-ING-001（Document）、
//! REQ-VEC-001/004（Chunk / DocStatus）、REQ-RAG-002（RetrievalResult / ChatMessage）。
//! 本 crate 只定义纯数据结构，不含任何业务逻辑与外部服务依赖。

use serde::{Deserialize, Serialize};

/// 默认工作空间 ID（REQ-WS-001）。
///
/// 旧文档和未指定工作空间的文档自动归属此工作空间。
fn default_workspace_id() -> String {
    "default".to_string()
}

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
    /// 源文件路径（REQ-SYNC-002 文件监听增量同步用）。
    /// 手动导入为 `None`；监听文件夹导入为源文件的 canonical 路径。
    /// 用于增量同步时按源路径查找已导入的文档。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub original_path: Option<String>,
    /// 领域分类标签（REQ-VEC-013 领域画像自动分类）。
    /// 由 `EmbeddingDomainClassifier` 在导入后自动填充，取值为 16 个预定义领域之一
    /// （technology/legal/medical/finance/science/education/business/engineering/
    /// literature/government/marketing/hr/design/data/security/general）。
    /// `None` 表示尚未分类；分类失败时设为 `Some("general")`。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub domain: Option<String>,
    /// 文档摘要（REQ-ING-019 导入时自动生成，200-300 字）。
    ///
    /// 由 `ImportService` 在嵌入完成后异步调用 LLM 生成，取文档前 4000 字符作为输入。
    /// 摘要生成失败时保持 `None`（优雅降级，不影响导入流程）。
    /// 用户可通过 `regenerate_summary` IPC 命令手动重新生成。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub summary: Option<String>,
    /// 用户自定义标签（REQ-ING-022 文档标签系统）。
    ///
    /// 存储为 JSON 数组字符串（如 `["法律","重要"]`），旧数据反序列化时为空数组。
    /// 由用户通过 `add_document_tag` / `remove_document_tag` IPC 命令管理，
    /// 支持按标签筛选文档列表。
    #[serde(default)]
    pub tags: Vec<String>,
    /// 工作空间标识（REQ-WS-001 多知识库）。
    ///
    /// 默认为 `"default"`。多知识库模式下通过此字段隔离不同知识库的文档。
    /// 旧数据迁移时自动设为 `"default"`。
    #[serde(default = "default_workspace_id")]
    pub workspace_id: String,
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
            original_path: None,
            domain: None,
            summary: None,
            tags: Vec::new(),
            workspace_id: default_workspace_id(),
        }
    }

    /// 创建新文档（含源文件路径，REQ-SYNC-002 监听文件夹导入用）。
    ///
    /// # 参数
    /// - `file_path`: 入库后的本地路径（数据目录中的副本）
    /// - `file_hash`: 内容指纹（MD5）
    /// - `original_path`: 源文件的 canonical 路径（监听文件夹中的原始文件）
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

/// 文档内容预览（REQ-ING-010）。
///
/// 包含文档元数据、前 500 字内容预览、以及 chunk 列表（每个 chunk 含 sequence + 前 200 字内容）。
/// 用于文档管理面板中点击文档名时弹出预览面板。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentPreview {
    /// 文档 ID
    pub id: String,
    /// 文件路径
    pub file_path: String,
    /// 索引状态
    pub status: DocStatus,
    /// 创建时间（Unix 秒级时间戳）
    pub created_at: i64,
    /// 文档领域分类（如有）
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub domain: Option<String>,
    /// 文档摘要（如有）
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub summary: Option<String>,
    /// 用户标签
    #[serde(default)]
    pub tags: Vec<String>,
    /// 内容指纹
    pub file_hash: String,
    /// 前 500 字内容预览（从 chunks 拼接）
    pub content_preview: String,
    /// Chunk 列表（每个含 sequence + 前 200 字内容）
    pub chunks: Vec<ChunkPreview>,
    /// Chunk 总数
    pub chunk_count: usize,
}

/// Chunk 预览项（REQ-ING-010）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkPreview {
    /// 分块 ID
    pub id: String,
    /// 文档内顺序号
    pub sequence: usize,
    /// 前 200 字内容
    pub content_preview: String,
    /// token 计数
    pub token_count: usize,
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

/// 网页搜索结果（REQ-RAG-036 网页搜索集成）。
///
/// 当本地知识库检索结果不足时（top-1 score < 阈值），自动搜索互联网补充 context。
/// 搜索结果通过 RRF 融合到本地检索结果中，在 prompt 中标注来源（🌐 Web）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// 搜索结果标题
    pub title: String,
    /// 搜索结果 URL
    pub url: String,
    /// 搜索结果摘要
    pub snippet: String,
    /// 搜索引擎来源（如 "duckduckgo"）
    pub source: String,
}

/// 抽取的命名实体（REQ-PERF-006 实体链接增强）。
///
/// 纯规则抽取（正则 + 停用词），零 LLM/模型依赖。
/// 用于三路 RRF 检索的实体匹配通道（Vector + BM25 + Entity）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Entity {
    /// 实体文本（归一化后，如 "Rust"、"张三"、"v2.0.0"）
    pub text: String,
    /// 实体类型（person / proper_noun / tech_term / identifier / date）
    pub entity_type: String,
}

impl Entity {
    /// 创建新实体。
    pub fn new(text: String, entity_type: String) -> Self {
        Self { text, entity_type }
    }
}

/// 实体间关系（知识图谱边，REQ-RAG-026）。
///
/// 借鉴 mnemo 知识图谱架构：在实体节点间建立有向关系边，
/// 检索时可先匹配实体节点，再沿图边扩展到关联 chunk，
/// 显著提升关联查询的召回率。
///
/// 关系抽取使用纯规则模式匹配（零 LLM），覆盖中英文 8 种关系类型：
/// `defined_as` / `part_of` / `depends_on` / `uses` / `implements` /
/// `extends` / `references` / `related_to`。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityRelation {
    /// 关系唯一标识（UUID v4）
    pub id: String,
    /// 主体实体文本
    pub subject: String,
    /// 关系类型（defined_as / part_of / depends_on / uses / implements / extends / references / related_to）
    pub relation_type: String,
    /// 客体实体文本
    pub object: String,
    /// 来源 chunk ID
    pub chunk_id: String,
    /// 置信度（规则抽取 0.5-1.0，精确匹配 1.0，模糊匹配 0.7）
    pub confidence: f32,
}

impl EntityRelation {
    /// 创建新关系：自动生成 UUID v4。
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

    /// 转换为三元组（用于检索结果展示）。
    pub fn to_triple(&self) -> GraphTriple {
        GraphTriple {
            subject: self.subject.clone(),
            relation: self.relation_type.clone(),
            object: self.object.clone(),
        }
    }
}

/// 图谱三元组（用于检索结果展示，REQ-RAG-026）。
///
/// 表示 `(subject, relation, object)` 三元组，
/// 由 `EntityRelation::to_triple()` 生成，用于前端图谱可视化展示。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphTriple {
    /// 主体实体
    pub subject: String,
    /// 关系类型
    pub relation: String,
    /// 客体实体
    pub object: String,
}

/// Wiki 双向链接（REQ-ING-020 Markdown 笔记双向链接，Obsidian 风格）。
///
/// 表示 Markdown 文档中 `[[link-target]]` 语法引用的链接关系。
/// 导入时自动解析并建立索引，支持正向链接和反向链接查询。
///
/// ## 正向链接（Forward Links）
///
/// 文档 A 中包含 `[[B]]` → A 的正向链接为 B（A → B）。
/// `get_forward_links(doc_id)` 返回该文档引用的所有目标。
///
/// ## 反向链接（Backlinks）
///
/// 文档 B 被其他文档 `[[B]]` 引用 → B 的反向链接为引用方。
/// `get_backlinks(doc_id)` 返回引用该文档的所有来源文档。
///
/// ## 链接匹配
///
/// wiki-link target 使用**文件名模糊匹配**（不含扩展名）：
/// `[[设计文档]]` 匹配文件名含「设计文档」的文档（如 `设计文档.md`）。
/// 匹配不区分大小写，支持中文。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiLink {
    /// 链接唯一标识（UUID v4）
    pub id: String,
    /// 源文档 ID（包含 `[[link]]` 的文档）
    pub source_doc_id: String,
    /// 目标 wiki-link 文本（`[[target]]` 中的 `target`）
    pub target: String,
    /// 来源 chunk ID（解析到具体分块）
    pub chunk_id: String,
    /// 创建时间（Unix 秒级时间戳）
    pub created_at: i64,
}

impl WikiLink {
    /// 创建新的 wiki-link：自动生成 UUID 和时间戳。
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
///
/// 提供图谱的汇总数据，用于前端面板展示概览信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStats {
    /// 去重后的实体总数（subject + object 去重）
    pub total_entities: usize,
    /// 关系总数（entity_relations 表记录数）
    pub total_relations: usize,
    /// 按关系类型统计数量（key: relation_type, value: count）
    pub relation_type_counts: std::collections::HashMap<String, usize>,
}

/// 知识图谱路径分析结果（REQ-RAG-027 Session 5 路径分析）。
///
/// 由 `get_shortest_path` IPC 命令返回，表示两个实体节点间的最短路径。
/// 前端据此高亮路径上的节点和边，并显示路径长度和经过的节点。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphPath {
    /// 路径上的实体节点列表（含起点和终点）
    pub path: Vec<String>,
    /// 路径跳数（path.len() - 1）
    pub hops: usize,
}

/// 知识图谱社区检测结果（REQ-RAG-027 Session 5 社区检测）。
///
/// 由 `get_communities` IPC 命令返回，包含每个实体所属的社区 ID。
/// 前端据此为不同社区的节点着不同颜色。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphCommunity {
    /// 实体 → 社区 ID 映射
    pub communities: std::collections::HashMap<String, usize>,
    /// 社区总数
    pub community_count: usize,
}

/// Proposition 级原子事实（REQ-PERF-007 Proposition 级原子分割）。
///
/// 将 chunk 分解为自包含的原子事实（proposition），proposition 级检索精度
/// 显著优于 chunk 级（Dense X Retrieval 论文 arXiv:2312.06648：命中率 +30-50%）。
///
/// 每个 proposition 是一个自包含的句子，无代词依赖，可独立理解。
/// 通过 `PropositionSplitter`（规则方案，零 LLM 调用）在导入时自动生成。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposition {
    /// Proposition 唯一标识（UUID v4）
    pub id: String,
    /// 关联的原 chunk ID
    pub chunk_id: String,
    /// 自包含的原子事实文本
    pub content: String,
    /// 在原 chunk 中的顺序号
    pub sequence: usize,
}

impl Proposition {
    /// 创建新 proposition。
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
///
/// 借鉴 IfAI SymbolEngine：使用 tree-sitter 对代码文件进行 AST 级符号分析，
/// 建立「符号 → 位置 → chunk」三级映射，使代码查询能精确定位到函数定义而非模糊匹配整个文件。
///
/// 支持 Rust / TypeScript / Python / Go 四种语言，纯 CPU 解析（零 LLM 调用）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeSymbol {
    /// 符号唯一标识（UUID v4）
    pub id: String,
    /// 关联的 chunk ID
    pub chunk_id: String,
    /// 符号名称（函数名 / 类名 / 结构体名等）
    pub name: String,
    /// 符号类型
    pub kind: SymbolKind,
    /// 编程语言（rust / typescript / python / go）
    pub language: String,
    /// 起始行号（1-based）
    pub start_line: usize,
    /// 结束行号（1-based）
    pub end_line: usize,
    /// 函数签名（参数列表，仅函数/方法有值）
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub signature: Option<String>,
}

impl CodeSymbol {
    /// 创建新代码符号：自动生成 UUID v4。
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
    /// 函数（顶级 function_item / function_declaration / function_definition）
    Function,
    /// 方法（impl 块内的函数 / 类方法）
    Method,
    /// 类（class_declaration / class_definition）
    Class,
    /// 结构体（struct_item / struct_specifier）
    Struct,
    /// 接口 / Trait（trait_item / interface_declaration）
    Interface,
    /// 枚举（enum_item / enum_declaration）
    Enum,
    /// 常量（const_item / const_spec）
    Constant,
    /// 模块（mod_item）
    Module,
}

impl SymbolKind {
    /// 转换为字符串（用于 SQLite 存储）。
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

    /// 从字符串解析（避免与 `FromStr` trait 冲突，clippy `should_implement_trait`）。
    ///
    /// 未知字符串回退为 `Function` 并输出 stderr 警告，避免静默吞掉数据损坏。
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
///
/// 基于 RAPTOR 论文（Recursive Abstrative Tree for Retrieval-Ordered Access），
/// 将原始 chunks 组织为多级摘要树：
/// - Level 0: 原始 chunks（不在 summary_nodes 表中，直接引用 chunks 表）
/// - Level 1: 组摘要（将相邻 chunks 聚类后由 LLM 生成摘要）
/// - Level 2: 主题摘要（将 Level 1 摘要聚类后由 LLM 生成更高层摘要）
///
/// 查询路由：
/// - 局部事实查询 → Level 0 检索
/// - 全局分析查询 → Level 2 → Level 1 → Level 0 展开
///
/// 压缩比：摘要节点数 < 原始 chunk 数（预期 Level 1 ≈ N/4, Level 2 ≈ N/16）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryNode {
    /// 节点唯一标识（UUID v4）
    pub id: String,
    /// 所属文档 ID
    pub doc_id: String,
    /// 树层级（0 = 组摘要, 1 = 主题摘要, …）
    pub level: usize,
    /// 摘要文本（LLM 生成）
    pub content: String,
    /// 子节点 ID 列表（Level 0 的子节点是 chunk_id；Level N 的子节点是下一级 SummaryNode.id）
    pub child_ids: Vec<String>,
    /// 摘要的嵌入向量（用于检索）
    pub embedding: Option<Vec<f32>>,
}

impl SummaryNode {
    /// 创建新摘要节点。
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

/// 对话消息（对应 REQ-RAG-001/004）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChatMessage {
    /// 消息唯一标识（DB 主键 UUID v4）。仅由 get_messages 等读路径填充；
    /// 构造新消息时不设置（add_message 内部生成）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// 角色（user / assistant / system）
    pub role: String,
    /// 消息正文
    pub content: String,
    /// 助手消息的引用来源（用户消息为 None）
    pub sources: Option<Vec<RetrievalResult>>,
    /// 助手消息的推理思考过程（reasoning_content，DeepSeek R1 等推理模型）。
    /// 落库持久化，历史消息加载时可重现思考过程；用户消息为 None。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    /// 轮次分组 ID（用户消息编辑版本管理）。
    /// 同一问题的不同编辑版本共享同一个 `turn_group`；首次消息为 None。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_group: Option<String>,
    /// 轮次内版本号（从 1 开始递增）。
    /// 用户编辑问题后产生新版本，旧版本保留用于分页查看；首次消息为 None。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<i32>,
}

/// 会话（REQ-RAG-006）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    /// 会话唯一标识（UUID v4）
    pub id: String,
    /// 工作区标识（v1.0 单工作区 "default"）
    pub workspace_id: String,
    /// 会话标题（首轮问答后自动提取自用户问题前缀）
    pub title: String,
    /// 创建时间（Unix 秒级时间戳）
    pub created_at: i64,
    /// 用户自定义排序序号（REQ-IX-002 拖拽排序；0 = 按创建时间倒序默认排序）
    pub sort_order: i64,
}

impl Conversation {
    /// 创建新会话：自动生成 UUID 与时间戳。
    pub fn new(workspace_id: String, title: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            sort_order: 0,
            workspace_id,
            title,
            created_at: chrono::Utc::now().timestamp(),
        }
    }

    /// 以指定 ID 构造（chat 兜底幂等创建用，REQ-RAG-006）。
    pub fn with_id(id: String, workspace_id: String, title: String) -> Self {
        Self {
            id,
            workspace_id,
            title,
            created_at: chrono::Utc::now().timestamp(),
            sort_order: 0,
        }
    }
}

/// 知识库/工作空间（REQ-WS-001 多知识库创建与切换）。
///
/// 每个工作空间拥有独立的文档集和会话列表，通过 `workspace_id` 字段隔离。
/// 默认工作空间 ID 为 `"default"`，用户可创建多个自定义知识库。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    /// 工作空间唯一标识（UUID v4 或 `"default"`）
    pub id: String,
    /// 知识库显示名称
    pub name: String,
    /// 创建时间（Unix 秒级时间戳）
    pub created_at: i64,
}

impl Workspace {
    /// 创建新工作空间：自动生成 UUID 与时间戳。
    pub fn new(name: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            created_at: chrono::Utc::now().timestamp(),
        }
    }

    /// 以指定 ID 构造（用于默认工作空间 `"default"`）。
    pub fn with_id(id: String, name: String) -> Self {
        Self {
            id,
            name,
            created_at: chrono::Utc::now().timestamp(),
        }
    }
}

/// 工作空间数据量预览（REQ-WS-003 删除前确认对话框）。
///
/// 删除工作空间前查询将级联清理的数据量，前端展示给用户确认。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceStats {
    /// 文档数量
    pub document_count: usize,
    /// 会话数量
    pub conversation_count: usize,
}

/// 对话全文搜索结果（REQ-RAG-040）。
///
/// 由 FTS5 全文索引查询返回，包含匹配消息及其所属会话信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageSearchResult {
    /// 消息 ID
    pub message_id: String,
    /// 会话 ID
    pub conversation_id: String,
    /// 会话标题
    pub conversation_title: String,
    /// 消息角色（user / assistant）
    pub role: String,
    /// 消息内容（原文）
    pub content: String,
    /// BM25 搜索分数（越高越相关）
    pub score: f64,
    /// 消息创建时间（Unix 秒级时间戳）
    pub created_at: i64,
}

/// 持久化提示接纳（B05 Durable Prompt Admission）。
///
/// 借鉴 OpenCode 的 Prompt Admission / Promotion 机制：用户消息先「接纳」到
/// `pending_inputs` 表（持久化但未加入消息历史），在安全边界点（如流式生成完成后）
/// 再「提升」为正式消息。支持两种投递模式：
///
/// - `steer`：优先模式，中断当前生成并立即处理（如 LLM 正在生成时用户发送新消息）
/// - `queue`：排队模式，等待当前生成完成后按 FIFO 顺序处理
///
/// `promoted_seq` 为 `None` 表示未提升；非 `None` 表示已提升为正式消息
/// （值为 messages 表的 seq）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingInput {
    /// 接纳记录唯一标识（UUID v4）
    pub id: String,
    /// 所属会话 ID
    pub conversation_id: String,
    /// 用户输入内容
    pub content: String,
    /// 投递模式：`"steer"`（优先中断）或 `"queue"`（排队等待）
    pub delivery: String,
    /// 创建时间（Unix 秒级时间戳）
    pub created_at: i64,
    /// 提升为正式消息后的 seq 序号；`None` = 未提升
    pub promoted_seq: Option<i64>,
}

impl PendingInput {
    /// 创建新的接纳记录：自动生成 UUID 和时间戳，初始未提升。
    ///
    /// # 参数
    /// - `conversation_id`: 所属会话 ID
    /// - `content`: 用户输入内容
    /// - `delivery`: 投递模式（`"steer"` 或 `"queue"`）
    pub fn new(conversation_id: String, content: String, delivery: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            conversation_id,
            content,
            delivery,
            created_at: chrono::Utc::now().timestamp(),
            promoted_seq: None,
        }
    }
}

/// Todo 状态枚举（REQ-RAG-044 Session Todo 持久化）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TodoStatus {
    /// 待处理
    Pending,
    /// 进行中
    InProgress,
    /// 已完成
    Completed,
}

impl TodoStatus {
    /// 转为数据库存储字符串。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
        }
    }

    /// 从数据库字符串解析。
    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "in_progress" => Some(Self::InProgress),
            "completed" => Some(Self::Completed),
            _ => None,
        }
    }
}

/// Todo 优先级枚举（REQ-RAG-044 Session Todo 持久化）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TodoPriority {
    /// 低
    Low,
    /// 中
    Medium,
    /// 高
    High,
}

impl TodoPriority {
    /// 转为数据库存储字符串。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }

    /// 从数据库字符串解析。
    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            _ => None,
        }
    }
}

/// 会话待办项（REQ-RAG-044 Session Todo 持久化）。
///
/// 借鉴 OpenCode `session/todo.ts`，将 Agent 的 Todo 列表持久化到 SQLite，
/// 支持跨会话恢复。每个 Todo 包含内容、状态、优先级、位置排序。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTodo {
    /// 唯一标识（UUID v4）
    pub id: String,
    /// 所属会话 ID
    pub conversation_id: String,
    /// Todo 内容
    pub content: String,
    /// 状态（pending / in_progress / completed）
    pub status: TodoStatus,
    /// 优先级（low / medium / high）
    pub priority: TodoPriority,
    /// 排序位置（升序）
    pub position: i64,
    /// 创建时间（Unix 秒级时间戳）
    pub created_at: i64,
}

impl SessionTodo {
    /// 创建新的 Todo 项：自动生成 UUID 和时间戳，默认状态 Pending、优先级 Medium。
    ///
    /// # 参数
    /// - `conversation_id`: 所属会话 ID
    /// - `content`: Todo 内容
    /// - `position`: 排序位置
    pub fn new(conversation_id: String, content: String, position: i64) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            conversation_id,
            content,
            status: TodoStatus::Pending,
            priority: TodoPriority::Medium,
            position,
            created_at: chrono::Utc::now().timestamp(),
        }
    }
}

/// 文档索引状态事件负载（Tauri Event `doc-status-changed`，REQ-ING-001 冻结契约）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocStatusPayload {
    /// 状态标识：indexing / done / error
    pub status: String,
    /// 可读消息（进度说明或错误原因）
    pub message: String,
    /// 子阶段标识（REQ-MM-004 多模态管线进度）：
    /// - `text_extracting`：文本层提取中
    /// - `image_rendering`：PDF 页面渲染中
    /// - `ocr`：OCR 文字识别中
    /// - `vlm_enhancing`：VLM 增强中（Phase 2 预留）
    ///
    /// 非 PDF 文档或旧前端忽略此字段。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_phase: Option<String>,
}

/// 对话阶段事件负载（Tauri Event `chat_phase`，REQ-RAG-001 扩展）。
/// 在首个 token 到达前，向前端推送当前处理阶段，消除「发了消息什么都没看见」的空白等待。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatPhasePayload {
    /// 阶段标识：preparing（初始化引擎）/ retrieving（检索知识库）/ generating（生成回答）
    pub phase: String,
    /// 可读消息（展示给用户的进度文案）
    pub message: String,
}

/// 导入进度事件负载（Tauri Event `import-progress`，REQ-ING-006）。
/// 批量导入时逐文件推送整体进度，前端据此渲染进度条与当前文件名。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportProgressPayload {
    /// 已完成文件数
    pub completed: usize,
    /// 文件总数
    pub total: usize,
    /// 当前正在处理的文件名
    pub current_file: String,
    /// 是否已被取消（取消后前端停止进度条）
    pub cancelled: bool,
}

/// 向量化进度事件负载（Tauri Event `embedding_progress`，GB 级文档加速）。
///
/// 大文档（数千 chunks）的向量化可能耗时数分钟。
/// 此事件在每个微批次完成后推送进度，前端据此渲染向量化进度条，
/// 让用户感知到「正在处理」而非「卡住了」。
///
/// 分块完成（FTS5 可用）后立即开始向量化；
/// 向量化未完成的文档仍可通过关键词检索（BM25）查询。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingProgressPayload {
    /// 文档 ID
    pub doc_id: String,
    /// 文档展示名
    pub doc_name: String,
    /// 已向量化的 chunk 数
    pub embedded: usize,
    /// chunk 总数
    pub total: usize,
}

/// BYOK LLM 配置（REQ-UI-008：经 SQLite settings 表 AES-256-GCM 加密持久化）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// API Key（允许为空串：兼容本地 Ollama，REQ-LLM-001）
    pub api_key: String,
    /// OpenAI 兼容端点（如 `https://api.openai.com` 或 `http://localhost:11434/v1` 的 host 部分）
    pub base_url: String,
    /// 模型名（如 gpt-4o-mini / deepseek-chat / qwen2.5）
    pub model: String,
}

/// RAG 检索参数（REQ-RAG-014）：用户可配置的检索行为参数。
///
/// 所有字段经 `clamp()` 保证合法范围。设置表键：`rag.top_k` / `rag.score_threshold` /
/// `rag.chunk_expansion_enabled` / `rag.chunk_expansion_window`。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RagParams {
    /// 检索 top-k 值（范围 1-20，默认 8）
    pub top_k: usize,
    /// 最低相似度阈值（范围 0.0-1.0，默认 0.0 即不过滤）
    pub score_threshold: f32,
    /// Chunk Expansion 开关（默认 true，启用后扩展相邻 chunk 提供上下文）
    pub chunk_expansion_enabled: bool,
    /// Chunk Expansion 窗口大小（范围 0-3，默认 1，每侧扩展的相邻 chunk 数）
    pub chunk_expansion_window: usize,
}

impl Default for RagParams {
    fn default() -> Self {
        Self {
            top_k: 8,
            score_threshold: 0.0,
            chunk_expansion_enabled: true,
            chunk_expansion_window: 1,
        }
    }
}

impl RagParams {
    /// 将参数钳制到合法范围。
    #[must_use]
    pub fn clamped(self) -> Self {
        Self {
            top_k: self.top_k.clamp(1, 20),
            score_threshold: self.score_threshold.clamp(0.0, 1.0),
            chunk_expansion_enabled: self.chunk_expansion_enabled,
            chunk_expansion_window: self.chunk_expansion_window.clamp(0, 3),
        }
    }
}

/// LLM 生成参数（REQ-RAG-015）：用户可配置的 LLM 生成行为参数。
///
/// 经 `OpenAIProvider` 传递到 OpenAI 兼容 API 请求体。
/// 设置表键：`llm.temperature` / `llm.max_tokens` / `llm.top_p`。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GenerationParams {
    /// 采样温度（范围 0.0-2.0，默认 0.7）
    pub temperature: f32,
    /// 最大生成 token 数（范围 256-8192，默认 4096）
    pub max_tokens: usize,
    /// nucleus sampling 概率阈值（范围 0.0-1.0，默认 1.0 即不过滤）
    pub top_p: f32,
}

impl Default for GenerationParams {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            max_tokens: 4096,
            top_p: 1.0,
        }
    }
}

impl GenerationParams {
    /// 将参数钳制到合法范围。
    #[must_use]
    pub fn clamped(self) -> Self {
        Self {
            temperature: self.temperature.clamp(0.0, 2.0),
            max_tokens: self.max_tokens.clamp(256, 8192),
            top_p: self.top_p.clamp(0.0, 1.0),
        }
    }
}

/// get_settings 返回载荷（REQ-UI-008；api_key 仅以脱敏形式返回，安全官要求）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsPayload {
    /// 是否已完成 LLM 配置
    pub has_llm_config: bool,
    /// OpenAI 兼容端点
    pub base_url: String,
    /// 模型名
    pub model: String,
    /// 脱敏 API Key（**** 前缀 + 末四位）
    pub api_key_masked: String,
    /// VLM 图片理解增强开关（REQ-MM-003；默认 false，用户主动开启后图片发送到 BYOK LLM 端点）
    pub vlm_enabled: bool,
    /// 混合检索开关（REQ-RAG-010；默认 true，向量+关键词 RRF 融合检索）
    pub hybrid_search: bool,
    /// Cross-Encoder 重排序开关（REQ-RAG-020；默认 false，启用后对 RRF 融合结果进行 bge-reranker-base 精排）
    pub rerank_enabled: bool,
    /// HyDE 查询改写开关（REQ-RAG-021；默认 false，启用后向量检索使用 LLM 生成的假设性答案文档嵌入）
    pub hyde_enabled: bool,
    /// Agentic RAG 多步推理开关（REQ-RAG-022；默认 false，启用后复杂查询触发 ReAct 多步检索）
    pub agent_enabled: bool,
    /// 当前嵌入模型标识（REQ-VEC-012；默认 "all-MiniLM-L6-v2"，可选 "bge-small-zh-v1.5" / "e5-small-v2"）
    pub embedding_model: String,
    /// 对话上下文 token 限制（REQ-RAG-017；默认 4096，范围 2048-32768，超限时截断中间历史消息）
    #[serde(default = "default_context_token_limit")]
    pub context_token_limit: usize,
    /// LLM 推理模式（REQ-LLM-003；默认空串等价于 "remote"，可选 "local"）
    #[serde(default)]
    pub llm_mode: String,
    /// 当前选中的本地模型文件名（REQ-LLM-003；空串表示未选择）
    #[serde(default)]
    pub local_model: String,
    /// 是否启用 PagedAttention 高效 KV cache 管理（REQ-LLM-003 扩展；默认 false）。
    ///
    /// 仅在 GPU 模式（metal/cuda feature）下生效，CPU 模式下忽略。
    /// 启用后降低多轮对话的首 token 延迟，但需要更多 GPU 显存。
    #[serde(default)]
    pub llm_paged_attn: bool,
    /// PagedAttention 块大小（REQ-LLM-003 扩展；默认 32）。
    ///
    /// 支持的值：8、16、32。仅在 `llm_paged_attn` 为 `true` 时生效。
    #[serde(default = "default_paged_attn_block_size")]
    pub llm_block_size: usize,
    /// PagedAttention GPU 上下文 token 数（REQ-LLM-003 扩展；默认 4096）。
    ///
    /// 控制 KV cache 可使用的 GPU 显存量。仅在 `llm_paged_attn` 为 `true` 时生效。
    #[serde(default = "default_paged_attn_gpu_memory")]
    pub llm_gpu_memory_ctx: usize,
    /// 采样参数（REQ-LLM-003 扩展；默认 `None` = 全部使用引擎默认值）。
    ///
    /// 通过 `set_sampling_params` IPC 命令持久化到 settings 表 `llm.sampling` 键（JSON）。
    /// `local_llm()` 加载引擎时读取并应用。运行时可通过 `set_sampling_params` 即时修改。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub llm_sampling: Option<LlmSamplingParams>,
    /// Token 预算上限（0 = 不限制）。
    ///
    /// 用户设置每月 token 消耗上限。`get_conversation_cost` 返回的累计 token 数
    /// 超过此值时，前端显示预算超限警告。后端不阻断 chat，仅前端提示。
    /// 存储在 settings 表 `usage.token_budget` 键。
    #[serde(default)]
    pub token_budget: u64,
    /// 语义缓存启用开关（REQ-PERF-001；默认 true，关闭后每次查询都走完整 RAG pipeline）。
    #[serde(default = "default_cache_enabled")]
    pub cache_enabled: bool,
    /// 缓存存活时间（秒，REQ-PERF-001；默认 86400 = 24h）。
    #[serde(default = "default_cache_ttl")]
    pub cache_ttl_secs: u64,
    /// L1 语义匹配余弦相似度阈值（REQ-PERF-001；默认 0.92）。
    #[serde(default = "default_cache_semantic_threshold")]
    pub cache_semantic_threshold: f32,
    /// 隐私模式（REQ-PERF-001；默认 false，高隐私场景关闭缓存）。
    #[serde(default)]
    pub cache_privacy_mode: bool,
    /// RAG 质量门控开关（REQ-RAG-028；默认 false，启用后检索结果质量低于阈值时记录告警）
    #[serde(default)]
    pub quality_gate_enabled: bool,
    /// 子代理舰队开关（REQ-RAG-025 扩展；默认 false，启用后 Coordinator Research 阶段
    /// 使用独立 AgentEngine ReAct 循环处理每个子查询，通过 mailbox 消息传递协调）
    #[serde(default)]
    pub sub_agent_enabled: bool,
    /// 渐进式注入开关（REQ-PERF-010；默认 false，启用后仅注入初始检索子集，不足时逐步追加）
    #[serde(default)]
    pub progressive_injection: bool,
    /// Speculative RAG 开关（REQ-PERF-011；默认 false，启用后小模型草稿 → 大模型验证）
    #[serde(default)]
    pub speculative_enabled: bool,
    /// 知识图谱图遍历检索开关（REQ-RAG-027；默认 false，启用后沿实体关系图边扩展到关联 chunk）
    #[serde(default)]
    pub graph_retriever_enabled: bool,
    /// Contextual Retrieval 开关（REQ-RAG-041；默认 true，嵌入时拼接文档名上下文前缀）
    #[serde(default = "default_true")]
    pub contextual_retrieval: bool,
}

/// LLM 推理模式（REQ-LLM-003）。
///
/// - `Remote`：BYOK 远程 API（现有行为，OpenAI 兼容 SSE 流式）
/// - `Local`：本地推理（mistral.rs，GGUF 模型，零网络请求）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum LlmMode {
    /// 远程 API（BYOK 模式）
    #[default]
    Remote,
    /// 本地推理（mistral.rs）
    Local,
}

/// 本地模型元信息（REQ-LLM-004）。
///
/// 由 `ModelManager::list_models()` 返回，前端用于渲染已下载模型列表。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// 文件名（如 `qwen2.5-7b-instruct-q4_k_m.gguf`）
    pub filename: String,
    /// 完整路径
    pub path: String,
    /// 文件大小（字节）
    pub size_bytes: u64,
    /// 推断的模型架构（从文件名解析，如 `qwen2.5` / `llama3.2` / `phi3.5`）
    pub architecture: String,
    /// 推断的参数规模（如 `7B` / `3B` / `3.8B`）
    pub param_size: String,
    /// 量化格式（如 `Q4_K_M` / `Q5_K_M` / `Q8_0`）
    pub quantization: String,
}

/// 模型下载进度事件载荷（REQ-LLM-004 AC-2）。
///
/// 通过 `model_download_progress` Tauri 事件推送到前端。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDownloadProgressPayload {
    /// 正在下载的模型文件名
    pub filename: String,
    /// 已下载字节数
    pub downloaded: u64,
    /// 文件总大小（字节）；未知时为 0
    pub total: u64,
    /// 下载速度（字节/秒）
    pub speed: u64,
}

/// 模型加载状态事件载荷（REQ-LLM-003 AC-4）。
///
/// 通过 `model_load_progress` Tauri 事件推送到前端。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelLoadStatusPayload {
    /// 正在加载的模型文件名
    pub model: String,
    /// 加载状态：`loading` / `ready` / `error`
    pub status: String,
    /// 错误信息（仅 `status == "error"` 时有值）
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub error: Option<String>,
}

/// 分页查询结果（列表懒加载，REQ-RAG-007/REQ-UI-009）。
///
/// 包含当前页数据与总条数，前端据此判断是否还有更多数据可加载。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResult<T> {
    /// 当前页数据
    pub items: Vec<T>,
    /// 总条数（用于判断是否还有更多）
    pub total: usize,
}

/// 对话上下文截断信息（REQ-RAG-017 AC-2：截断后推送事件，前端显示「部分历史已折叠」提示）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryTruncationPayload {
    /// 被截断的消息数量
    pub truncated_count: usize,
    /// 截断前的总 token 数
    pub total_tokens: usize,
    /// 截断后保留的 token 数
    pub retained_tokens: usize,
    /// token 限制阈值
    pub token_limit: usize,
}

/// 压缩触发类型（Q03：双阈值压缩，区分后台异步 vs 同步）。
///
/// 借鉴 QM `core/orchestrator/compaction.ts` 的双阈值机制：
/// - Soft 阈值（70%）触发后台异步压缩（不阻塞当前轮次）
/// - Hard 阈值（90%）触发同步压缩（阻塞兜底）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactionKind {
    /// 后台异步压缩（Soft 阈值触发）
    Background,
    /// 同步压缩（Hard 阈值触发）
    Synchronous,
}

impl CompactionKind {
    /// 转为字符串标识。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Background => "background",
            Self::Synchronous => "synchronous",
        }
    }
}

/// 对话历史压缩信息（压缩后推送 `chat_context_compacted` 事件，
/// 前端显示「部分历史已压缩为摘要」提示，替代旧的纯截断策略）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionInfo {
    /// 被压缩的消息数量（这些消息被 LLM 生成的摘要替代）
    pub compacted_count: usize,
    /// 压缩前的总 token 数
    pub total_tokens: usize,
    /// 压缩后的 token 数（摘要 + 保留的最近消息）
    pub compacted_tokens: usize,
    /// token 限制阈值
    pub token_limit: usize,
    /// 压缩触发类型（Q03：区分后台异步 vs 同步，`None` = 旧版兼容）
    #[serde(default)]
    pub compaction_kind: Option<CompactionKind>,
}

/// 对话历史压缩结果。
///
/// 当历史消息总 token 数超过阈值时，旧消息被 LLM 压缩为一条摘要 system 消息，
/// 替代 `truncate_history` 的纯截断策略（丢弃中间消息）。
/// 压缩后的历史 = [摘要 system 消息] + [保留的最近 N 轮消息]。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionResult {
    /// 压缩后的历史消息（摘要 + 最近 N 轮）
    pub history: Vec<ChatMessage>,
    /// 压缩信息（`None` 表示无需压缩，历史未超限）
    pub info: Option<CompactionInfo>,
}

/// Token 预算配置（S69：Cherry Studio 借鉴 — token-budget 驱动的 in-loop compaction）。
///
/// 控制对话历史何时触发压缩、保留多少最近消息、压缩阈值等。
/// 可通过设置面板配置，适配不同 LLM 的 context window 大小。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBudgetConfig {
    /// 上下文窗口大小（token 数），如 GPT-4o = 128000、Claude 3.5 = 200000
    #[serde(default = "default_context_token_limit")]
    pub max_tokens: usize,
    /// 压缩触发阈值（0.0-1.0），如 0.8 表示历史 token 数达到 max_tokens * 80% 时触发压缩
    #[serde(default = "default_compaction_threshold")]
    pub compaction_threshold: f32,
    /// 最近消息保留比例（0.0-1.0），如 0.67 表示保留 2/3 的 token 预算给最近消息
    #[serde(default = "default_recent_keep_ratio")]
    pub recent_keep_ratio: f32,
    /// 触发压缩的最小消息数（防止短对话频繁压缩）
    #[serde(default = "default_min_messages_to_compact")]
    pub min_messages_to_compact: usize,
}

impl Default for TokenBudgetConfig {
    fn default() -> Self {
        Self {
            max_tokens: default_context_token_limit(),
            compaction_threshold: default_compaction_threshold(),
            recent_keep_ratio: default_recent_keep_ratio(),
            min_messages_to_compact: default_min_messages_to_compact(),
        }
    }
}

impl TokenBudgetConfig {
    /// 计算压缩触发点（max_tokens * compaction_threshold）
    pub fn compaction_trigger(&self) -> usize {
        (self.max_tokens as f32 * self.compaction_threshold) as usize
    }

    /// 计算最近消息保留预算（max_tokens * recent_keep_ratio）
    pub fn recent_budget(&self) -> usize {
        (self.max_tokens as f32 * self.recent_keep_ratio) as usize
    }

    /// 判断是否需要压缩
    pub fn should_compact(&self, total_tokens: usize, message_count: usize) -> bool {
        message_count >= self.min_messages_to_compact && total_tokens >= self.compaction_trigger()
    }
}

fn default_compaction_threshold() -> f32 {
    0.8
}

fn default_recent_keep_ratio() -> f32 {
    0.67
}

fn default_min_messages_to_compact() -> usize {
    3
}

/// 默认上下文 token 限制（REQ-RAG-017）
pub fn default_context_token_limit() -> usize {
    4096
}

/// serde 默认值辅助：返回 `true`（用于 `#[serde(default = "default_true")]`）。
pub fn default_true() -> bool {
    true
}

/// 默认 PagedAttention 块大小（REQ-LLM-003 扩展）
pub fn default_paged_attn_block_size() -> usize {
    32
}

/// 默认 PagedAttention GPU 上下文 token 数（REQ-LLM-003 扩展）
pub fn default_paged_attn_gpu_memory() -> usize {
    4096
}

/// 默认缓存启用开关（REQ-PERF-001）
pub fn default_cache_enabled() -> bool {
    true
}

/// 默认缓存存活时间（秒，REQ-PERF-001）
pub fn default_cache_ttl() -> u64 {
    86400
}

/// 默认语义匹配阈值（REQ-PERF-001）
pub fn default_cache_semantic_threshold() -> f32 {
    0.92
}

/// 对话成本统计（累计 token 用量追踪）。
///
/// 由 `get_conversation_cost` IPC 命令返回，前端据此展示累计 token 消耗与预算使用进度条。
/// 数据来源于 settings 表中的累计计数器（`usage.total_prompt_tokens` 等），
/// 每次 `chat_done` 事件携带 `TokenUsage` 后由后端自动累加。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConversationCost {
    /// 累计提示词 token 数（所有对话的输入 token 之和）
    pub total_prompt_tokens: u64,
    /// 累计生成 token 数（所有对话的输出 token 之和）
    pub total_completion_tokens: u64,
    /// 累计总 token 数（prompt + completion）
    pub total_tokens: u64,
    /// 对话轮次数（chat_done 事件触发次数）
    pub exchange_count: u32,
    /// Token 预算上限（0 = 不限制）
    pub token_budget: u64,
}

/// LLM 采样参数（REQ-LLM-003 扩展）。
///
/// 远程 API 和本地推理共用此参数集。各字段为 `Option`，
/// `None` 表示使用引擎默认值（不向 RequestBuilder / HTTP 请求体注入该参数）。
///
/// # 字段范围
///
/// | 字段 | 范围 | 默认值 |
/// |---|---|---|
/// | `temperature` | 0.0 ~ 2.0 | 0.7（引擎默认） |
/// | `top_p` | 0.0 ~ 1.0 | 0.9（引擎默认） |
/// | `top_k` | 1 ~ 100 | 40（引擎默认） |
/// | `max_tokens` | > 0 | 无限制（引擎默认） |
/// | `frequency_penalty` | -2.0 ~ 2.0 | 0.0（引擎默认） |
/// | `presence_penalty` | -2.0 ~ 2.0 | 0.0（引擎默认） |
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct LlmSamplingParams {
    /// 采样温度（0.0~2.0）。值越高输出越随机，值越低越确定。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub temperature: Option<f64>,
    /// Top-P 核采样（0.0~1.0）。仅从累积概率超过此阈值的 token 中采样。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub top_p: Option<f64>,
    /// Top-K 采样（1~100）。仅从概率最高的 K 个 token 中采样。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub top_k: Option<usize>,
    /// 最大生成 token 数。限制单次推理输出的 token 上限。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub max_tokens: Option<usize>,
    /// 频率惩罚（-2.0~2.0）。正值惩罚已出现的 token，减少重复。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub frequency_penalty: Option<f32>,
    /// 存在惩罚（-2.0~2.0）。正值鼓励引入新话题。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub presence_penalty: Option<f32>,
}

/// 增量同步结果（REQ-SYNC-002）。
///
/// 记录一次文件夹同步操作中各类操作的计数与错误信息，
/// 前端据此渲染同步报告 toast。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncResult {
    /// 新增导入的文件数
    pub added: usize,
    /// 内容变更后更新的文件数（删除旧文档 + 导入新文档）
    pub updated: usize,
    /// 因源文件被删除而清理的文档数
    pub deleted: usize,
    /// 因内容重复或格式不支持而跳过的文件数
    pub skipped: usize,
    /// 同步过程中遇到的错误列表（文件名 + 原因）
    pub errors: Vec<String>,
}

/// 监听文件夹信息（REQ-SYNC-001）。
///
/// 前端 `get_watched_folders` IPC 命令返回此结构列表，
/// 用于渲染监听文件夹列表 UI。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchedFolderInfo {
    /// 监听文件夹的 canonical 路径
    pub path: String,
    /// 文件夹显示名（路径末尾目录名）
    pub name: String,
    /// 同步状态：idle / syncing / error
    pub sync_status: String,
    /// 最后同步时间（Unix 秒级时间戳，None 表示尚未同步）
    pub last_synced_at: Option<i64>,
}

/// 同步进度事件负载（Tauri Event `sync_progress`，REQ-SYNC-002 AC-7）。
///
/// 同步过程中向前端推送进度，包含当前阶段与计数信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncProgressPayload {
    /// 同步阶段：scanning（扫描文件夹）/ importing（导入中）/ deleting（删除中）/ complete（完成）/ error（错误）
    pub phase: String,
    /// 关联的文件夹路径
    pub folder_path: String,
    /// 新增计数
    pub added: usize,
    /// 更新计数
    pub updated: usize,
    /// 删除计数
    pub deleted: usize,
    /// 跳过计数
    pub skipped: usize,
    /// 可读消息（进度说明或错误原因）
    pub message: String,
}

/// Agentic RAG 步骤事件负载（Tauri Event `agent_step`，REQ-RAG-022）。
///
/// 在 ReAct 多步推理过程中，每个 Thought/Action/Observation 步骤
/// 通过此事件推送前端，展示 Agent 的思考过程。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStepPayload {
    /// 步骤类型：thought（推理）/ action（工具调用）/ observation（观察结果）/ answer（最终答案开始）
    pub step_type: String,
    /// 步骤内容（推理文本 / 工具名+输入 / 观察摘要）
    pub content: String,
    /// 工具名称（仅 action 步骤有值）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    /// 工具输入（仅 action 步骤有值）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<String>,
    /// 当前迭代轮次（从 1 开始）
    pub iteration: usize,
}

/// Token 用量统计（对话 token 消耗追踪）。
///
/// 由 OpenAI 兼容 API 在流式响应末尾通过 `stream_options.include_usage`
/// 返回。前端据此展示每次对话的 token 消耗。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TokenUsage {
    /// 提示词 token 数（输入）
    pub prompt_tokens: u32,
    /// 生成 token 数（输出）
    pub completion_tokens: u32,
    /// 总 token 数（prompt + completion）
    pub total_tokens: u32,
}

/// KV cache 持久化状态（REQ-LLM-009，Phase 4 Session 25）。
///
/// 由 `get_kv_cache_status` IPC 命令返回，描述当前 KV cache 持久化的
/// 启用状态、缓存目录路径、文件数量和总占用磁盘空间。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KvCacheStatus {
    /// KV cache 持久化是否已启用（`llm.kv_cache_enabled` 设置键）。
    pub enabled: bool,
    /// KV cache 文件存储目录绝对路径。
    pub cache_dir: String,
    /// 已保存的 `.emkv` 缓存文件数量。
    pub file_count: usize,
    /// 所有缓存文件总占用磁盘空间（字节）。
    pub total_size_bytes: u64,
}

/// 轮次活跃版本记录（分支切换状态持久化）。
///
/// 用户在分页器中切换查看不同编辑版本时，活跃版本号被持久化到 DB，
/// 下次加载会话时恢复到最后一次查看的版本。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnActiveVersion {
    /// 轮次分组 ID
    pub turn_group: String,
    /// 当前活跃版本号
    pub active_version: i32,
}

/// 对话分支树节点（REQ-RAG-039 对话分支/版本树）。
///
/// 表示分支树中的一个节点，对应一个 `turn_group` 的某个版本。
/// 节点间的父子关系通过 `parent_message_id` 建立：
/// 编辑用户消息时，新版本成为旧版本的「子节点」，形成版本树。
///
/// 数据来源：复用 `messages` 表已有的 `turn_group` + `version` 列，
/// 无需新增数据库表。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationTreeNode {
    /// 节点唯一标识（格式：`{turn_group}#{version}`）
    pub node_id: String,
    /// 所属会话 ID
    pub conversation_id: String,
    /// 父消息 ID（根节点为 None；编辑分支时指向被编辑的原始消息 ID）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_message_id: Option<String>,
    /// 子节点 ID 列表（从该版本进一步编辑/分叉产生的版本）
    #[serde(default)]
    pub child_message_ids: Vec<String>,
    /// 当前活跃的子节点 ID（用户最后查看的分支；叶子节点为 None）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_child: Option<String>,
    /// 创建时间（Unix 秒级时间戳，取自消息行）
    pub created_at: i64,
    /// 该节点的 turn_group
    pub turn_group: String,
    /// 该节点的版本号
    pub version: i32,
    /// 用户消息内容（预览，前 100 字符）
    #[serde(default)]
    pub preview: String,
}

impl ConversationTreeNode {
    /// 创建新的对话树节点。
    ///
    /// `node_id` 格式为 `{turn_group}#{version}`，保证在同一会话内唯一。
    pub fn new(
        conversation_id: &str,
        turn_group: &str,
        version: i32,
        created_at: i64,
        preview: &str,
    ) -> Self {
        let node_id = format!("{turn_group}#{version}");
        let preview: String = if preview.chars().count() > 100 {
            let truncated: String = preview.chars().take(100).collect();
            format!("{truncated}…")
        } else {
            preview.to_string()
        };
        Self {
            node_id,
            conversation_id: conversation_id.to_string(),
            parent_message_id: None,
            child_message_ids: Vec::new(),
            active_child: None,
            created_at,
            turn_group: turn_group.to_string(),
            version,
            preview,
        }
    }

    /// 判断是否为根节点（无父消息）。
    pub fn is_root(&self) -> bool {
        self.parent_message_id.is_none()
    }

    /// 判断是否为叶子节点（无子节点）。
    pub fn is_leaf(&self) -> bool {
        self.child_message_ids.is_empty()
    }
}

/// 对话分支树（REQ-RAG-039）。
///
/// 包含整棵分支树的所有节点，以及根节点 ID 列表和当前活跃路径。
/// 活跃路径是从根节点到当前查看节点的路径。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationTree {
    /// 会话 ID
    pub conversation_id: String,
    /// 所有节点（node_id → node）
    pub nodes: Vec<ConversationTreeNode>,
    /// 根节点 ID 列表（按创建时间排序）
    pub root_ids: Vec<String>,
    /// 当前活跃路径（从根到当前查看节点的 node_id 序列）
    pub active_path: Vec<String>,
}

impl ConversationTree {
    /// 创建空的对话分支树。
    pub fn empty(conversation_id: &str) -> Self {
        Self {
            conversation_id: conversation_id.to_string(),
            nodes: Vec::new(),
            root_ids: Vec::new(),
            active_path: Vec::new(),
        }
    }

    /// 根据 node_id 查找节点。
    pub fn find(&self, node_id: &str) -> Option<&ConversationTreeNode> {
        self.nodes.iter().find(|n| n.node_id == node_id)
    }

    /// 获取节点的所有子节点。
    pub fn children_of(&self, node_id: &str) -> Vec<&ConversationTreeNode> {
        self.nodes
            .iter()
            .filter(|n| n.parent_message_id.as_deref() == Some(node_id))
            .collect()
    }
}

/// 缓存级别（REQ-PERF-001 语义缓存金字塔）。
///
/// 三级缓存策略，从最快到最慢：
/// - `Exact`：L0 精确匹配（query hash → answer），命中率 5-10%，0 token
/// - `Semantic`：L1 语义匹配（embedding similarity → answer），命中率 10-20%，0 token
/// - `Retrieval`：L3 检索结果缓存（query embedding → chunks），跳过嵌入+检索计算
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CacheLevel {
    /// L0: 精确匹配（query hash → answer）
    Exact,
    /// L1: 语义匹配（embedding similarity → answer）
    Semantic,
    /// L3: 检索结果缓存（query embedding → chunks）
    Retrieval,
}

/// 缓存命中结果（REQ-PERF-001）。
///
/// 当缓存命中时，直接返回缓存的答案和引用来源，不调用 LLM。
/// `level` 标识命中哪一级缓存，用于统计和调试。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheHit {
    /// 命中的缓存级别
    pub level: CacheLevel,
    /// 缓存的答案文本（L0/L1 命中时有值）
    pub answer_text: Option<String>,
    /// 缓存的引用来源（序列化的 `Vec<RetrievalResult>` JSON，L0/L1 命中时有值）
    pub sources_json: Option<String>,
    /// 缓存的检索结果（序列化的 `Vec<RetrievalResult>` JSON，L3 命中时有值）
    pub retrieval_results_json: Option<String>,
}

/// 缓存统计信息（REQ-PERF-001）。
///
/// 由 `get_cache_stats` IPC 命令返回，前端据此展示缓存命中率和 token 节省量。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CacheStats {
    /// 缓存是否已启用
    pub enabled: bool,
    /// L0 精确匹配命中次数
    pub exact_hits: u32,
    /// L1 语义匹配命中次数
    pub semantic_hits: u32,
    /// L3 检索结果缓存命中次数
    pub retrieval_hits: u32,
    /// 总查询次数（含未命中）
    pub total_queries: u32,
    /// 缓存条目总数
    pub cache_size_entries: u32,
    /// 估算的节省 token 总量
    pub estimated_token_saved: u64,
}

/// 缓存设置载荷（REQ-PERF-001）。
///
/// 由 `set_cache_settings` / `get_cache_settings` IPC 命令使用，
/// 持久化到 settings 表 `cache.enabled` / `cache.ttl_secs` / `cache.semantic_threshold` / `cache.privacy_mode` 键。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheSettingsPayload {
    /// 是否启用缓存（默认 true）
    pub enabled: bool,
    /// 缓存存活时间（秒，默认 86400 = 24h）
    pub ttl_secs: u64,
    /// L1 语义匹配余弦相似度阈值（默认 0.92，范围 0.85-0.99）
    pub semantic_threshold: f32,
    /// 隐私模式（高隐私场景关闭缓存，默认 false）
    pub privacy_mode: bool,
}

/// 预算使用统计（QM 借鉴）。
///
/// 记录主体的 LLM API 使用情况和费用统计。
/// 在滑动窗口（24 小时）内的 token 使用量和费用。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetStats {
    /// 每日预算限制（USD，0 表示不限制）
    pub daily_limit: f64,
    /// 今日已花费金额（USD）
    pub spent_today: f64,
    /// 剩余可用金额（USD，daily_limit <= 0 时为无穷大）
    pub remaining: f64,
}

/// 导入历史记录条目（REQ-ING-011）。
///
/// 记录每次导入操作的时间戳、文件名、格式、结果（成功/失败/跳过）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportLogEntry {
    /// 记录 ID
    pub id: i64,
    /// Unix 时间戳（秒）
    pub timestamp: i64,
    /// 文件名
    pub file_name: String,
    /// 文件格式（扩展名）
    pub format: String,
    /// 导入结果：success / failed / skipped
    pub result: String,
    /// 失败原因（仅 result=failed 时有值）
    pub error_message: Option<String>,
    /// 文件大小（字节）
    pub file_size: Option<i64>,
}

/// 速率限制统计。
///
/// 记录主体在滑动窗口内的请求频率统计。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitStats {
    /// 窗口内允许的最大请求数
    pub max_per_window: u32,
    /// 当前窗口内已发出的请求数
    pub current_count: u32,
    /// 滑动窗口时长（秒）
    pub window_seconds: u64,
}

impl Default for CacheSettingsPayload {
    fn default() -> Self {
        Self {
            enabled: true,
            ttl_secs: 86400,
            semantic_threshold: 0.92,
            privacy_mode: false,
        }
    }
}

// ============================================================
// 健壮下载系统数据模型（RobustDownloader，REQ-LLM-004 v2）
// ============================================================
// 参考：Ollama download.go + HuggingFace hf_transfer + HuggingFace Hub file_download.py
// 设计目标：断点续传、多源容错、并发分块、崩溃恢复、完整性校验

/// 下载源定义（多源容错）。
///
/// 源顺序由 `RobustDownloader` 根据系统语言动态决定：
/// - 中文系统：ModelScope（魔搭）→ hf-mirror → HuggingFace
/// - 其他系统：HuggingFace → hf-mirror → ModelScope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadSourceDef {
    /// 源名称（日志用，如 "HuggingFace" / "ModelScope" / "hf-mirror"）
    pub name: String,
    /// Base URL 前缀（不含 repo 和文件路径）
    pub base_url: String,
    /// 分支名：HuggingFace 用 `main`，ModelScope 用 `master`
    pub branch: String,
}

/// 下载分块信息（并发分块下载用）。
///
/// 大文件切分为多个分块，每块独立 Range 请求 + 重试 + stall 检测。
/// 参考：Ollama blobDownloadPart + hf_transfer chunk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadPart {
    /// 分块序号（0-based）
    pub index: usize,
    /// 文件内偏移（字节）
    pub offset: u64,
    /// 分块大小（字节）
    pub size: u64,
    /// 已下载字节数（运行时更新，持久化到 .meta.json 用于崩溃恢复）
    pub completed: u64,
    /// 重试次数
    pub retries: u32,
}

/// 下载状态机。
///
/// 状态转换：Queued → Downloading → (Paused) → Verifying → Completed/Failed
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum DownloadStatus {
    /// 排队等待（下载队列中）
    #[default]
    Queued,
    /// 正在下载
    /// - completed: 已下载字节
    /// - total: 总字节
    /// - speed: 速度（字节/秒）
    Downloading {
        completed: u64,
        total: u64,
        speed: u64,
    },
    /// 已暂停（保留 .partial + .meta.json，可恢复）
    Paused { completed: u64, total: u64 },
    /// 正在校验完整性（SHA256）
    Verifying { completed: u64, total: u64 },
    /// 下载完成（文件已原子重命名为最终路径）
    Completed,
    /// 下载失败
    Failed {
        error: String,
        completed: u64,
        total: u64,
    },
}

/// 下载清单 — 持久化到 `.meta.json`，崩溃后可恢复。
///
/// 参考：Ollama blobDownload (JSON 序列化 part 元数据) +
///      HuggingFace Hub (ETag + commit_hash 持久化)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadManifest {
    /// 目标文件名（仅文件名，不含路径）
    pub filename: String,
    /// 原始下载 URL
    pub url: String,
    /// 文件总大小（字节）
    pub total_size: u64,
    /// SHA256 哈希（从 HuggingFace API ETag 获取；LFS 文件的 ETag 即 SHA256）
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub sha256: Option<String>,
    /// 多源列表（按优先级排序）
    pub sources: Vec<DownloadSourceDef>,
    /// 分块信息
    pub parts: Vec<DownloadPart>,
    /// 当前状态
    #[serde(default)]
    pub status: DownloadStatus,
    /// 偏好源索引（成功的源优先，None 表示未确定）
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub prefer_source: Option<usize>,
    /// 创建时间（Unix 秒级时间戳）
    pub created_at: i64,
    /// 最后更新时间（Unix 秒级时间戳）
    pub updated_at: i64,
}

/// 下载进度事件载荷（增强版，REQ-LLM-004 v2）。
///
/// 通过 `model_download_progress` Tauri 事件推送到前端。
/// 向后兼容旧版 `ModelDownloadProgressPayload`。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgressPayload {
    /// 正在下载的文件名
    pub filename: String,
    /// 已下载字节
    pub downloaded: u64,
    /// 文件总大小（字节）；未知时为 0
    pub total: u64,
    /// 下载速度（字节/秒）
    pub speed: u64,
    /// 当前状态
    #[serde(default = "default_status_str")]
    pub status: String,
    /// 错误信息（仅 status == "failed" 时有值）
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub error: Option<String>,
}

fn default_status_str() -> String {
    "downloading".to_string()
}

/// 下载状态摘要（用于 `list_pending_downloads` IPC 命令）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadStatusSummary {
    /// 文件名
    pub filename: String,
    /// 当前状态
    pub status: DownloadStatus,
    /// 总大小（字节）
    pub total_size: u64,
    /// 已完成字节
    pub completed: u64,
    /// 创建时间
    pub created_at: i64,
    /// 最后更新时间
    pub updated_at: i64,
}

// ============================================================
// DAG 工作流引擎数据模型（REQ-RAG-030）
// ============================================================

/// 工作流定义（DAG 有向无环图）。
///
/// 用户定义由节点（`WorkflowNode`）和边（`WorkflowEdge`）组成的有向无环图，
/// 引擎负责拓扑排序、并行执行独立节点、串行执行依赖节点。
/// 支持条件分支和聚合节点，实现多步骤 RAG 管线编排。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    /// 工作流唯一标识（UUID v4）
    pub id: String,
    /// 工作流名称
    pub name: String,
    /// 描述
    pub description: String,
    /// 节点列表
    pub nodes: Vec<WorkflowNode>,
    /// 边列表（数据流方向）
    pub edges: Vec<WorkflowEdge>,
    /// 创建时间（Unix 秒）
    pub created_at: i64,
}

impl Workflow {
    /// 创建新工作流：自动生成 UUID 与时间戳。
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
    /// 节点唯一标识（DAG 内唯一，如 "node_1"）
    pub id: String,
    /// 人类可读标签
    pub label: String,
    /// 节点类型与配置
    pub node_type: NodeType,
}

/// 节点类型（决定执行行为）。
///
/// 使用 `#[serde(tag = "kind")]` 标签枚举，序列化时自动添加 `"kind"` 字段
/// 区分变体，便于前端 JSON 构造和后端反序列化。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum NodeType {
    /// RAG 检索节点：使用上游输出作为查询，调用 Retriever 检索知识库。
    Retrieval {
        /// 检索 top-k 数量
        top_k: usize,
        /// 检索模式："vector" / "hybrid" / "hybrid_rerank"
        retrieval_mode: String,
    },
    /// LLM 生成节点：使用上游输出填充 system_prompt 模板，调用 LLM 生成。
    Generation {
        /// 系统提示词模板（支持 `{input}` 占位符，运行时替换为上游输出）
        system_prompt: String,
        /// 模型名（None = 使用当前 LLM 配置）
        model: Option<String>,
    },
    /// 条件分支节点：根据表达式评估结果选择下游路径。
    ///
    /// 评估结果为 "true" 或 "false"，下游边通过 `mapping` 字段
    /// （`Some("true")` / `Some("false")`）匹配激活路径。
    Condition {
        /// 表达式（"contains:关键词" / "length>N" / "length<N" / "nonempty" / "true"）
        expression: String,
    },
    /// 聚合节点：合并多个上游输出。
    Aggregate {
        /// 聚合策略："concat" / "summarize" / "best_of"
        strategy: String,
    },
    /// 输出节点（工作流终点）。
    Output {
        /// 输出格式："text" / "markdown" / "json"
        format: String,
    },
}

/// 工作流边（数据流方向）。
///
/// 对于 Condition 节点的下游边，`mapping` 字段可用作条件标签：
/// - `Some("true")` — 仅当条件评估为 true 时激活
/// - `Some("false")` — 仅当条件评估为 false 时激活
/// - `None` — 无条件激活（普通数据流边）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowEdge {
    /// 源节点 ID
    pub source: String,
    /// 目标节点 ID
    pub target: String,
    /// 输出字段映射（None = 直接传递上游完整输出；
    /// 对 Condition 下游边可用作条件标签 "true"/"false"）
    pub mapping: Option<String>,
}

/// 节点执行状态。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeStatus {
    /// 待执行
    Pending,
    /// 执行中
    Running,
    /// 已完成（携带输出文本）
    Completed { output: String },
    /// 失败（携带错误信息）
    Failed { error: String },
    /// 跳过（条件分支未选中或上游失败）
    Skipped,
}

/// 工作流执行结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowResult {
    /// 各节点执行状态（key = node_id）
    pub node_results: std::collections::HashMap<String, NodeStatus>,
    /// 最终输出文本（Output 节点的输出）
    pub final_output: String,
    /// 执行耗时（毫秒）
    pub duration_ms: u64,
}

// ============================================================
// 代码执行沙箱（REQ-RAG-032 代码片段执行，Pro feature）
// ============================================================

/// 代码执行结果（REQ-RAG-032）。
///
/// 借鉴 CodeForge 多语言执行器：在沙箱中执行代码片段并返回结果。
/// Agent 在回答代码相关问题时可调用 `execute_code` 工具验证结果，
/// 大幅提升回答可信度。
///
/// 安全限制由 `CodeExecutorConfig` 控制（超时 / 内存 / 网络）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutionResult {
    /// 标准输出
    pub stdout: String,
    /// 标准错误输出
    pub stderr: String,
    /// 退出码（0 = 成功，非 0 = 失败；超时为 -1）
    pub exit_code: i32,
    /// 执行耗时（毫秒）
    pub duration_ms: u64,
    /// 是否因超时被终止
    pub timed_out: bool,
}

/// 代码执行器配置（REQ-RAG-032）。
///
/// 安全限制参数：
/// - `timeout_secs`：执行超时时间（秒），默认 10s，硬编码上限 30s
/// - `memory_limit_mb`：内存限制（MB），默认 64MB，硬编码上限 256MB
/// - `allow_network`：是否允许网络访问，**永远为 false**（不可配置为 true）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeExecutorConfig {
    /// 执行超时时间（秒），默认 10s，硬编码上限 30s
    pub timeout_secs: u64,
    /// 内存限制（MB），默认 64MB，硬编码上限 256MB
    pub memory_limit_mb: usize,
    /// 是否允许网络访问（永远为 false，安全审查要求）
    pub allow_network: bool,
}

impl Default for CodeExecutorConfig {
    fn default() -> Self {
        Self {
            timeout_secs: 10,
            memory_limit_mb: 64,
            allow_network: false,
        }
    }
}

impl CodeExecutorConfig {
    /// 超时硬编码上限（秒），即使用户配置也不能超过此值。
    pub const MAX_TIMEOUT_SECS: u64 = 30;

    /// 内存硬编码上限（MB），即使用户配置也不能超过此值。
    pub const MAX_MEMORY_MB: usize = 256;

    /// 创建配置并应用安全限制（超时/内存不超过上限，网络永远禁用）。
    pub fn safe_new(timeout_secs: u64, memory_limit_mb: usize, _allow_network: bool) -> Self {
        Self {
            timeout_secs: timeout_secs.min(Self::MAX_TIMEOUT_SECS),
            memory_limit_mb: memory_limit_mb.min(Self::MAX_MEMORY_MB),
            allow_network: false,
        }
    }
}

// ============================================================
// 对话记忆系统（REQ-RAG-032 持久化记忆系统增强）
// ============================================================

/// 记忆层级（借鉴 IfAI Wing/Hall/Room 空间隐喻）。
///
/// 三层记忆架构按重要性和时效性分层：
/// - **Wing（翼层）**：临时记忆，当前会话产生，会话结束后提升或遗忘
/// - **Hall（厅层）**：工作记忆，近期重要信息，跨会话保留
/// - **Room（室层）**：长期记忆，核心知识，永久保留
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryTier {
    /// 翼层：临时记忆（当前会话），会话结束后提升或遗忘
    Wing,
    /// 厅层：工作记忆（近期重要），跨会话保留
    Hall,
    /// 室层：长期记忆（核心知识），永久保留
    Room,
}

impl MemoryTier {
    /// 从字符串解析记忆层级。
    pub fn parse_str(s: &str) -> Option<Self> {
        match s {
            "wing" => Some(Self::Wing),
            "hall" => Some(Self::Hall),
            "room" => Some(Self::Room),
            _ => None,
        }
    }

    /// 返回层级字符串标识（用于数据库存储）。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Wing => "wing",
            Self::Hall => "hall",
            Self::Room => "room",
        }
    }
}

/// 记忆来源类型。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemorySource {
    /// 用户明确陈述的事实
    UserStatement,
    /// 助手回答中的关键信息
    AssistantAnswer,
    /// AutoDream 自动提取
    AutoExtracted,
    /// 用户手动置顶
    UserPinned,
}

impl MemorySource {
    /// 从字符串解析记忆来源。
    pub fn parse_str(s: &str) -> Option<Self> {
        match s {
            "user_statement" => Some(Self::UserStatement),
            "assistant_answer" => Some(Self::AssistantAnswer),
            "auto_extracted" => Some(Self::AutoExtracted),
            "user_pinned" => Some(Self::UserPinned),
            _ => None,
        }
    }

    /// 返回来源字符串标识（用于数据库存储）。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UserStatement => "user_statement",
            Self::AssistantAnswer => "assistant_answer",
            Self::AutoExtracted => "auto_extracted",
            Self::UserPinned => "user_pinned",
        }
    }
}

/// 对话记忆条目（跨会话知识沉淀，REQ-RAG-032）。
///
/// 与 `MemoryRecord`（检索策略记忆）区分：
/// - `MemoryRecord`：记录检索方法效果统计（QueryType × RetrievalMethod → hit_rate）
/// - `MemoryEntry`：记录对话内容中的关键事实（用户偏好、重要决定、事实陈述）
///
/// 两者独立存储，互不干扰。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// 记忆唯一标识（UUID v4）
    pub id: String,
    /// 记忆层级（Wing/Hall/Room）
    pub tier: MemoryTier,
    /// 记忆内容文本
    pub content: String,
    /// 记忆来源
    pub source: MemorySource,
    /// 关联的会话 ID（可选）
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub conversation_id: Option<String>,
    /// 创建时间（Unix 秒级时间戳）
    pub created_at: i64,
    /// 最后访问时间（Unix 秒级时间戳）
    pub last_accessed: i64,
    /// 访问计数
    pub access_count: u32,
    /// 重要性分数（0.0-1.0）
    pub importance: f32,
}

impl MemoryEntry {
    /// 创建新的记忆条目（默认 Wing 层、importance 0.5、access_count 0）。
    pub fn new(content: String, source: MemorySource, tier: MemoryTier) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            tier,
            content,
            source,
            conversation_id: None,
            created_at: now,
            last_accessed: now,
            access_count: 0,
            importance: 0.5,
        }
    }

    /// 创建新记忆条目（含会话 ID）。
    pub fn new_with_conversation(
        content: String,
        source: MemorySource,
        tier: MemoryTier,
        conversation_id: String,
    ) -> Self {
        let mut entry = Self::new(content, source, tier);
        entry.conversation_id = Some(conversation_id);
        entry
    }
}

/// 记忆整合结果（由 AutoDream 后台调用 consolidate() 返回）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidationResult {
    /// 提升到更高层级的记忆数量
    pub promoted: usize,
    /// 被遗忘（删除）的记忆数量
    pub forgotten: usize,
    /// Wing 层剩余数量
    pub remaining_wing: usize,
    /// Hall 层剩余数量
    pub remaining_hall: usize,
    /// Room 层剩余数量
    pub remaining_room: usize,
}

// ============================================================
// Scratch-Promote 记忆整合（Q01 借鉴 QM scratch-promote + consolidation）
// ============================================================

/// 记忆整合动作（借鉴 QM consolidation.ts 的 UPDATE/DELETE/ADD 模式）。
///
/// LLM 审查积累的 scratch 日志后，输出一系列动作指令，
/// 由 `MemoryStore::consolidate_scratch()` 解析并应用到长期记忆层。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MemoryConsolidationAction {
    /// 更新已有记忆（事实演变或合并时）
    Update {
        /// 目标记忆 ID
        id: String,
        /// 修订后的内容
        content: String,
    },
    /// 删除过时/重复/可推导的记忆
    Delete {
        /// 目标记忆 ID
        id: String,
    },
    /// 新增记忆到指定层级
    Add {
        /// 新事实内容
        content: String,
        /// 目标层级
        tier: MemoryTier,
    },
}

/// Scratch 层日志条目（借鉴 QM scratch-promote 的 daily log）。
///
/// 日常写入的临时事实，累积后由 LLM 审查并 promote 到长期记忆层。
/// 超过保留期（默认 14 天）的条目自动清理。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScratchLogEntry {
    /// 条目唯一标识（UUID v4）
    pub id: String,
    /// 日期（YYYY-MM-DD 格式，用于按日聚合）
    pub date: String,
    /// 日志内容（markdown bullet 格式: "- (date) fact"）
    pub content: String,
    /// 创建时间（Unix 秒级时间戳）
    pub created_at: i64,
}

impl ScratchLogEntry {
    /// 创建新的 scratch 日志条目：自动生成 UUID 和当前时间戳。
    ///
    /// # 参数
    /// - `content`: 日志内容文本
    pub fn new(content: String) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            date: now.format("%Y-%m-%d").to_string(),
            content,
            created_at: now.timestamp(),
        }
    }

    /// 创建带指定日期的 scratch 日志条目（测试用）。
    ///
    /// # 参数
    /// - `content`: 日志内容文本
    /// - `date`: 日期字符串（YYYY-MM-DD 格式）
    /// - `created_at`: 创建时间戳
    pub fn with_date(content: String, date: String, created_at: i64) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            date,
            content,
            created_at,
        }
    }
}

/// Scratch 层整合结果（由 `consolidate_scratch()` 返回）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScratchConsolidationResult {
    /// 解析出的动作列表
    pub actions: Vec<MemoryConsolidationAction>,
    /// 清理的过期 scratch 条目数
    pub expired_cleaned: usize,
    /// 整合后剩余的 scratch 条目数
    pub remaining_scratch: usize,
}

// ============================================================
// Burst Buffer + Provenance 标记（Q02 借鉴 QM createBurstBuffer）
// ============================================================

/// 记忆来源标记（借鉴 QM provenance tracking）。
///
/// 记录每条记忆的来源信息：哪个对话、哪条消息、何时捕获。
/// 借鉴 QM `memory/memory-service.ts` 的 `ccCaptureToPersonal()` 中
/// `(said in #channel)` 的 provenance 标记模式。
///
/// 当 BurstBuffer flush 时，提取的记忆会附带 ProvenanceTag 写入 scratch 层，
/// 后续 consolidate 时保留 `(said in ...)` 后缀以追溯来源。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProvenanceTag {
    /// 关联的会话 ID
    pub conversation_id: String,
    /// 消息序号（该会话中的第几轮对话，从 1 开始）
    pub message_seq: usize,
    /// 捕获时间（Unix 秒级时间戳）
    pub captured_at: i64,
    /// 来源标签（如 "对话：知识产权法咨询 的第 3 轮"）
    pub source_label: String,
}

impl ProvenanceTag {
    /// 创建新的来源标记，自动填充当前时间戳。
    ///
    /// # 参数
    /// - `conversation_id`: 会话 ID
    /// - `message_seq`: 消息序号（从 1 开始）
    /// - `source_label`: 来源描述标签
    pub fn new(conversation_id: String, message_seq: usize, source_label: String) -> Self {
        Self {
            conversation_id,
            message_seq,
            captured_at: chrono::Utc::now().timestamp(),
            source_label,
        }
    }

    /// 创建带指定捕获时间的来源标记（测试用）。
    pub fn with_timestamp(
        conversation_id: String,
        message_seq: usize,
        captured_at: i64,
        source_label: String,
    ) -> Self {
        Self {
            conversation_id,
            message_seq,
            captured_at,
            source_label,
        }
    }

    /// 生成 `(said in ...)` 后缀字符串（用于 scratch 日志内容追加）。
    ///
    /// 借鉴 QM 的 `(said in #channel)` 标记格式。
    pub fn said_in_suffix(&self) -> String {
        format!(" (said in {})", self.source_label)
    }
}

// ============================================================
// 对话模板/快捷指令系统（S56 自定义快捷指令模板）
// ============================================================

/// 自定义快捷指令模板（S56）。
///
/// 用户在设置面板中创建的自定义 prompt 模板，通过 `/` 前缀触发。
/// 与系统内置的 6 个快捷指令（summary/compare/extract/translate/timeline/mindmap）
/// 合并展示在 slash-commands 面板中。
///
/// 存储方式：settings 表 JSON 序列化，键 `prompt_template.{id}` + 索引 `prompt_template.index`。
///
/// 模板中的 `{query}` 占位符在发送时替换为用户输入的查询文本。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PromptTemplate {
    /// 模板唯一标识（UUID v4）
    pub id: String,
    /// 指令名（不含 `/`，仅小写字母/数字/下划线，≤32 字符）
    pub name: String,
    /// 显示标签（人类可读名称）
    pub label: String,
    /// 描述说明
    pub description: String,
    /// 图标 emoji
    pub icon: String,
    /// Prompt 模板内容（支持 `{query}` 占位符）
    pub prompt_template: String,
    /// 创建时间（Unix 秒级时间戳）
    pub created_at: i64,
    /// 最后更新时间（Unix 秒级时间戳）
    pub updated_at: i64,
}

impl PromptTemplate {
    /// 创建新的模板：自动生成 UUID 与时间戳。
    pub fn new(
        name: String,
        label: String,
        description: String,
        icon: String,
        prompt_template: String,
    ) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            label,
            description,
            icon,
            prompt_template,
            created_at: now,
            updated_at: now,
        }
    }

    /// 验证模板名称是否合法（仅小写字母/数字/下划线，1-32 字符）。
    pub fn is_valid_name(name: &str) -> bool {
        let char_count = name.chars().count();
        !name.is_empty()
            && char_count <= 32
            && name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    }

    /// 验证模板内容是否包含 `{query}` 占位符。
    pub fn has_query_placeholder(template: &str) -> bool {
        template.contains("{query}")
    }
}

// ============================================================
// RAG 评估指标系统（REQ-RAG-045，RAGAS 风格）
// ============================================================

/// RAG 评估指标类型（REQ-RAG-045）。
///
/// 借鉴 RAGAS（Retrieval-Augmented Generation Assessment）框架，
/// 将 RAG 评估指标分为两大类：
///
/// **纯 Rust 检索指标（不需要 LLM）**：
/// - `HitRate`：相关文档是否在 top-k 检索结果中
/// - `MRR`：第一个相关文档的倒数排名
/// - `NDCG`：归一化折扣累积增益（排序质量）
/// - `ContextSimilarity`：查询与上下文的平均嵌入余弦相似度
/// - `KeywordOverlap`：查询与上下文的关键词重叠率
///
/// **LLM-as-Judge 生成指标（需要 LLMProvider）**：
/// - `Faithfulness`：答案是否忠实于检索上下文（无幻觉）
/// - `AnswerRelevance`：答案是否切题回答了用户问题
/// - `ContextPrecision`：检索上下文是否与问题相关
/// - `ContextRecall`：检索上下文是否包含回答问题所需的信息
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RagMetricType {
    /// 答案忠实度（LLM-as-Judge）：答案中的声明是否可从上下文推导
    Faithfulness,
    /// 答案相关性（LLM-as-Judge）：答案是否切题回答了问题
    AnswerRelevance,
    /// 上下文精度（LLM-as-Judge）：检索到的上下文是否与问题相关
    ContextPrecision,
    /// 上下文召回（LLM-as-Judge）：上下文是否包含回答所需信息（需要 ground truth）
    ContextRecall,
    /// 命中率（纯 Rust）：相关文档是否在 top-k 中
    HitRate,
    /// 平均倒数排名（纯 Rust）：第一个相关文档的排名倒数
    MRR,
    /// 归一化折扣累积增益（纯 Rust）：排序质量
    NDCG,
    /// 上下文嵌入相似度（纯 Rust）：查询与上下文的平均余弦相似度
    ContextSimilarity,
    /// 关键词重叠率（纯 Rust）：查询与上下文的 token 重叠比例
    KeywordOverlap,
}

impl RagMetricType {
    /// 转换为字符串（用于序列化和显示）。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Faithfulness => "faithfulness",
            Self::AnswerRelevance => "answer_relevance",
            Self::ContextPrecision => "context_precision",
            Self::ContextRecall => "context_recall",
            Self::HitRate => "hit_rate",
            Self::MRR => "mrr",
            Self::NDCG => "ndcg",
            Self::ContextSimilarity => "context_similarity",
            Self::KeywordOverlap => "keyword_overlap",
        }
    }

    /// 从字符串解析。
    pub fn parse_str(s: &str) -> Option<Self> {
        match s {
            "faithfulness" => Some(Self::Faithfulness),
            "answer_relevance" => Some(Self::AnswerRelevance),
            "context_precision" => Some(Self::ContextPrecision),
            "context_recall" => Some(Self::ContextRecall),
            "hit_rate" => Some(Self::HitRate),
            "mrr" => Some(Self::MRR),
            "ndcg" => Some(Self::NDCG),
            "context_similarity" => Some(Self::ContextSimilarity),
            "keyword_overlap" => Some(Self::KeywordOverlap),
            _ => None,
        }
    }

    /// 是否需要 LLM 调用。
    pub fn needs_llm(&self) -> bool {
        matches!(
            self,
            Self::Faithfulness
                | Self::AnswerRelevance
                | Self::ContextPrecision
                | Self::ContextRecall
        )
    }

    /// 是否需要 ground truth 答案。
    pub fn needs_ground_truth(&self) -> bool {
        matches!(self, Self::ContextRecall)
    }

    /// 是否需要嵌入向量。
    pub fn needs_embedding(&self) -> bool {
        matches!(self, Self::ContextSimilarity)
    }
}

/// RAG 评估样本（单次问答评估输入，REQ-RAG-045）。
///
/// 一个样本代表一次完整的 RAG 问答交互：
/// 用户提问 → 检索上下文 → LLM 生成答案。
///
/// # 字段说明
/// - `query`：用户问题
/// - `answer`：LLM 生成的答案
/// - `contexts`：检索到的上下文片段列表（按相关性排序）
/// - `ground_truth`：参考答案（可选，用于 ContextRecall 指标）
/// - `relevance_scores`：每个上下文的相关性分数（可选，用于 NDCG 指标）
/// - `relevant_indices`：相关文档的索引列表（可选，用于 HitRate/MRR 指标）
/// - `query_embedding`：查询的嵌入向量（可选，用于 ContextSimilarity 指标）
/// - `context_embeddings`：上下文的嵌入向量列表（可选，用于 ContextSimilarity 指标）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagEvalSample {
    /// 用户问题
    pub query: String,
    /// LLM 生成的答案
    pub answer: String,
    /// 检索到的上下文片段列表（按相关性排序）
    pub contexts: Vec<String>,
    /// 参考答案（可选，用于 ContextRecall 指标）
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub ground_truth: Option<String>,
    /// 每个上下文的相关性分数 0.0-1.0（可选，用于 NDCG 指标）
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub relevance_scores: Option<Vec<f32>>,
    /// 相关文档的索引列表（可选，用于 HitRate/MRR 指标）
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub relevant_indices: Option<Vec<usize>>,
    /// 查询的嵌入向量（可选，用于 ContextSimilarity 指标）
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub query_embedding: Option<Vec<f32>>,
    /// 上下文的嵌入向量列表（可选，用于 ContextSimilarity 指标）
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub context_embeddings: Option<Vec<Vec<f32>>>,
}

impl RagEvalSample {
    /// 创建新的评估样本（必需字段：query + answer + contexts）。
    pub fn new(query: String, answer: String, contexts: Vec<String>) -> Self {
        Self {
            query,
            answer,
            contexts,
            ground_truth: None,
            relevance_scores: None,
            relevant_indices: None,
            query_embedding: None,
            context_embeddings: None,
        }
    }

    /// 设置参考答案。
    pub fn with_ground_truth(mut self, truth: String) -> Self {
        self.ground_truth = Some(truth);
        self
    }

    /// 设置相关性分数。
    pub fn with_relevance_scores(mut self, scores: Vec<f32>) -> Self {
        self.relevance_scores = Some(scores);
        self
    }

    /// 设置相关文档索引。
    pub fn with_relevant_indices(mut self, indices: Vec<usize>) -> Self {
        self.relevant_indices = Some(indices);
        self
    }
}

/// RAG 评估指标结果（REQ-RAG-045）。
///
/// 单个指标的评估结果，包含分数和可选的详细信息。
/// 分数范围 [0.0, 1.0]，1.0 表示最优。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagEvalMetric {
    /// 指标类型
    pub metric_type: RagMetricType,
    /// 分数 [0.0, 1.0]，1.0 = 最优
    pub score: f32,
    /// 详细信息（如 LLM 判断的理由、统计明细等）
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub details: Option<String>,
}

impl RagEvalMetric {
    /// 创建新的指标结果。
    pub fn new(metric_type: RagMetricType, score: f32) -> Self {
        Self {
            metric_type,
            score: score.clamp(0.0, 1.0),
            details: None,
        }
    }

    /// 创建带详细信息的指标结果。
    pub fn with_details(metric_type: RagMetricType, score: f32, details: String) -> Self {
        Self {
            metric_type,
            score: score.clamp(0.0, 1.0),
            details: Some(details),
        }
    }
}

/// RAG 评估报告（REQ-RAG-045）。
///
/// 批量评估多个样本后的聚合报告，包含每个样本的指标和整体平均值。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagEvalReport {
    /// 评估样本数量
    pub sample_count: usize,
    /// 聚合指标（各指标的平均值）
    pub aggregate_metrics: Vec<RagEvalMetric>,
    /// 每个样本的指标列表
    pub per_sample_metrics: Vec<Vec<RagEvalMetric>>,
}

impl RagEvalReport {
    /// 创建空报告。
    pub fn empty() -> Self {
        Self {
            sample_count: 0,
            aggregate_metrics: Vec::new(),
            per_sample_metrics: Vec::new(),
        }
    }

    /// 从多个样本的指标列表构建报告（自动计算平均值）。
    pub fn from_samples(per_sample: Vec<Vec<RagEvalMetric>>) -> Self {
        let sample_count = per_sample.len();
        if sample_count == 0 {
            return Self::empty();
        }

        // 收集所有指标类型
        let mut type_scores: std::collections::HashMap<String, Vec<f32>> =
            std::collections::HashMap::new();
        for metrics in &per_sample {
            for m in metrics {
                let name = m.metric_type.as_str().to_string();
                type_scores.entry(name).or_default().push(m.score);
            }
        }

        // 计算平均值
        let aggregate: Vec<RagEvalMetric> = type_scores
            .iter()
            .filter_map(|(name, scores)| {
                RagMetricType::parse_str(name).map(|mt| {
                    let avg = scores.iter().sum::<f32>() / scores.len() as f32;
                    RagEvalMetric::new(mt, avg)
                })
            })
            .collect();

        Self {
            sample_count,
            aggregate_metrics: aggregate,
            per_sample_metrics: per_sample,
        }
    }

    /// 获取指定指标的聚合分数。
    pub fn get_metric(&self, metric_type: &RagMetricType) -> Option<f32> {
        self.aggregate_metrics
            .iter()
            .find(|m| &m.metric_type == metric_type)
            .map(|m| m.score)
    }
}

/// RAG 评估设置（REQ-RAG-045）。
///
/// 控制哪些指标启用，以及 LLM 评估的参数。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagEvalSettings {
    /// 启用 Faithfulness 指标
    pub enable_faithfulness: bool,
    /// 启用 Answer Relevance 指标
    pub enable_answer_relevance: bool,
    /// 启用 Context Precision 指标
    pub enable_context_precision: bool,
    /// 启用 Context Recall 指标
    pub enable_context_recall: bool,
    /// 启用检索指标（HitRate/MRR/NDCG）
    pub enable_retrieval_metrics: bool,
    /// 启用嵌入相似度指标
    pub enable_embedding_metrics: bool,
    /// 启用关键词重叠指标
    pub enable_keyword_overlap: bool,
}

impl Default for RagEvalSettings {
    fn default() -> Self {
        Self {
            enable_faithfulness: true,
            enable_answer_relevance: true,
            enable_context_precision: true,
            enable_context_recall: false, // 需要 ground truth，默认关闭
            enable_retrieval_metrics: true,
            enable_embedding_metrics: false, // 需要嵌入向量，默认关闭
            enable_keyword_overlap: true,
        }
    }
}

// ============================================================
// Session Strip（REQ-RAG-046，DS4 DS-03 借鉴）
// ============================================================

/// Strip 操作配置（REQ-RAG-046）。
///
/// 定义从对话历史中移除消息的范围和选项。
/// 借鉴 ds4 (DwarfStar) `/strip` 命令——移除数据以减少上下文窗口消耗。
///
/// # 字段
/// - `from_index` / `to_index`：0-based 闭区间消息索引
/// - `replace_with_summary`：是否插入摘要 system 消息替代被移除的消息
/// - `summary_text`：预生成的摘要文本（`None` 时不插入摘要）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StripConfig {
    /// 0-based 起始消息索引（包含）
    pub from_index: usize,
    /// 0-based 结束消息索引（包含）
    pub to_index: usize,
    /// 是否插入摘要 system 消息
    pub replace_with_summary: bool,
    /// 预生成的摘要文本
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub summary_text: Option<String>,
}

impl StripConfig {
    /// 创建新的 strip 配置。
    pub fn new(from_index: usize, to_index: usize) -> Self {
        Self {
            from_index,
            to_index,
            replace_with_summary: false,
            summary_text: None,
        }
    }

    /// 启用摘要替代。
    pub fn with_summary(mut self, summary: String) -> Self {
        self.replace_with_summary = true;
        self.summary_text = Some(summary);
        self
    }
}

/// Strip 操作结果（REQ-RAG-046 + DS-03 Session Strip）。
///
/// 记录 strip 操作的执行结果。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StripResult {
    /// 被移除的消息数量
    pub stripped_count: usize,
    /// 是否插入了摘要 system 消息
    pub summary_inserted: bool,
    /// 被移除消息的 ID 列表
    pub stripped_message_ids: Vec<String>,
    /// 估算的 token 节省量（4 字符 ≈ 1 token）
    pub estimated_tokens_saved: usize,
    /// 生成的摘要文本（DS-03 扩展；空串表示未生成摘要）
    #[serde(default)]
    pub summary: String,
    /// 保留的消息数量（DS-03 扩展）
    #[serde(default)]
    pub kept_count: usize,
}

impl StripResult {
    /// 创建空结果（无消息被 strip）。
    pub fn empty() -> Self {
        Self {
            stripped_count: 0,
            summary_inserted: false,
            stripped_message_ids: Vec::new(),
            estimated_tokens_saved: 0,
            summary: String::new(),
            kept_count: 0,
        }
    }
}

/// Strip 预览（REQ-RAG-046）。
///
/// 预览 strip 操作将影响哪些消息，不执行实际删除。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StripPreview {
    /// 将被 strip 的消息列表
    pub messages: Vec<ChatMessage>,
    /// 对话中的消息总数
    pub total_messages: usize,
    /// 估算的 token 节省量
    pub estimated_tokens_saved: usize,
}

impl StripPreview {
    /// 创建空预览。
    pub fn empty(total_messages: usize) -> Self {
        Self {
            messages: Vec::new(),
            total_messages,
            estimated_tokens_saved: 0,
        }
    }
}

/// 对话书签（REQ-RAG-047）。
///
/// 用户可将重要对话标记为书签，支持自定义备注。
/// 持久化到 `conversation_bookmarks` 表。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationBookmark {
    /// 书签所属会话 ID
    pub conversation_id: String,
    /// 书签备注（可选，用户自定义）
    pub note: Option<String>,
    /// 创建时间戳（Unix 秒）
    pub created_at: i64,
}

impl ConversationBookmark {
    /// 创建新书签。
    pub fn new(conversation_id: String) -> Self {
        Self {
            conversation_id,
            note: None,
            created_at: chrono::Utc::now().timestamp(),
        }
    }

    /// 设置备注。
    pub fn with_note(mut self, note: String) -> Self {
        self.note = Some(note);
        self
    }
}

// 缓存设置新增到 SettingsPayload（REQ-PERF-001）。
//
// 通过 `get_settings` 返回给前端，前端据此渲染缓存设置 UI。
// 通过 `save_settings` 持久化到 settings 表。
// ============================================================
// RAG 评估数据集（REQ-RAG-048）
// ============================================================

/// RAG 评估数据集中的单个评估样本（REQ-RAG-048）。
///
/// 每个样本定义了一个查询及其对应的 ground truth（标准答案）和
/// 相关文档/chunk ID 列表，用于端到端检索质量评估。
///
/// # 字段
/// - `query`：用户查询文本
/// - `ground_truth`：标准答案（用于 Context Recall 指标）
/// - `relevant_doc_ids`：相关文档 ID 列表
/// - `relevant_chunk_ids`：相关 chunk ID 列表
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagEvalDatasetSample {
    /// 用户查询文本
    pub query: String,
    /// 标准答案（ground truth）
    pub ground_truth: String,
    /// 相关文档 ID 列表
    #[serde(default)]
    pub relevant_doc_ids: Vec<String>,
    /// 相关 chunk ID 列表
    #[serde(default)]
    pub relevant_chunk_ids: Vec<String>,
}

impl RagEvalDatasetSample {
    /// 创建新的评估样本。
    pub fn new(query: String, ground_truth: String) -> Self {
        Self {
            query,
            ground_truth,
            relevant_doc_ids: Vec::new(),
            relevant_chunk_ids: Vec::new(),
        }
    }

    /// 设置相关文档 ID 列表。
    pub fn with_doc_ids(mut self, doc_ids: Vec<String>) -> Self {
        self.relevant_doc_ids = doc_ids;
        self
    }

    /// 设置相关 chunk ID 列表。
    pub fn with_chunk_ids(mut self, chunk_ids: Vec<String>) -> Self {
        self.relevant_chunk_ids = chunk_ids;
        self
    }
}

/// RAG 评估数据集（REQ-RAG-048）。
///
/// 包含多个评估样本，用于端到端检索质量评估。
/// JSON 格式，可序列化/反序列化，可扩展、可复现。
///
/// # 字段
/// - `name`：数据集名称
/// - `description`：数据集描述
/// - `samples`：评估样本列表
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagEvalDataset {
    /// 数据集名称
    pub name: String,
    /// 数据集描述
    #[serde(default)]
    pub description: String,
    /// 评估样本列表
    pub samples: Vec<RagEvalDatasetSample>,
}

impl RagEvalDataset {
    /// 创建新的评估数据集。
    pub fn new(name: String, samples: Vec<RagEvalDatasetSample>) -> Self {
        Self {
            name,
            description: String::new(),
            samples,
        }
    }

    /// 设置数据集描述。
    pub fn with_description(mut self, description: String) -> Self {
        self.description = description;
        self
    }

    /// 获取样本数量。
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    /// 是否为空数据集。
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// 序列化为 JSON 字符串。
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// 从 JSON 字符串反序列化。
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

#[cfg(test)]
mod llm_model_tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    /// TC-MODEL-LLM-001：LlmMode 默认值为 Remote
    #[test]
    fn test_llm_mode_default() {
        assert_eq!(LlmMode::default(), LlmMode::Remote);
    }

    /// TC-MODEL-LLM-002：LlmMode 序列化/反序列化往返一致
    #[test]
    fn test_llm_mode_serde() {
        let mode = LlmMode::Local;
        let json = serde_json::to_string(&mode).expect("serialize");
        let deserialized: LlmMode = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(mode, deserialized);
    }

    /// TC-MODEL-LLM-003：ModelInfo 序列化包含所有字段
    #[test]
    fn test_model_info_serialize() {
        let info = ModelInfo {
            filename: "qwen2.5-7b-instruct-q4_k_m.gguf".to_string(),
            path: "/data/models/llm/qwen2.5-7b-instruct-q4_k_m.gguf".to_string(),
            size_bytes: 4_400_000_000,
            architecture: "qwen2.5".to_string(),
            param_size: "7B".to_string(),
            quantization: "Q4_K_M".to_string(),
        };
        let json = serde_json::to_string(&info).expect("serialize");
        assert!(json.contains("qwen2.5-7b-instruct-q4_k_m.gguf"));
        assert!(json.contains("Q4_K_M"));
    }

    /// TC-MODEL-LLM-004：SettingsPayload 新增字段默认值正确
    #[test]
    fn test_settings_payload_llm_mode_default() {
        let json = r#"{
            "has_llm_config": false,
            "base_url": "",
            "model": "",
            "api_key_masked": "",
            "vlm_enabled": false,
            "hybrid_search": true,
            "rerank_enabled": false,
            "hyde_enabled": false,
            "agent_enabled": false,
            "embedding_model": "all-MiniLM-L6-v2",
            "context_token_limit": 4096
        }"#;
        let payload: SettingsPayload = serde_json::from_str(json).expect("deserialize");
        assert_eq!(payload.llm_mode, "");
        assert_eq!(payload.local_model, "");
        // S10：PagedAttention 默认值
        assert!(!payload.llm_paged_attn);
        assert_eq!(payload.llm_block_size, 32);
        assert_eq!(payload.llm_gpu_memory_ctx, 4096);
        // S11：采样参数默认为 None
        assert!(payload.llm_sampling.is_none());
        // token_budget 默认为 0（不限制）
        assert_eq!(payload.token_budget, 0);
    }

    /// TC-MODEL-LLM-005：LlmSamplingParams 默认值全为 None
    #[test]
    fn test_sampling_params_default_all_none() {
        let params = LlmSamplingParams::default();
        assert!(params.temperature.is_none());
        assert!(params.top_p.is_none());
        assert!(params.top_k.is_none());
        assert!(params.max_tokens.is_none());
        assert!(params.frequency_penalty.is_none());
        assert!(params.presence_penalty.is_none());
    }

    /// TC-MODEL-LLM-006：LlmSamplingParams 序列化/反序列化往返一致
    #[test]
    fn test_sampling_params_serde_roundtrip() {
        let params = LlmSamplingParams {
            temperature: Some(0.8),
            top_p: Some(0.95),
            top_k: Some(40),
            max_tokens: Some(2048),
            frequency_penalty: Some(0.5),
            presence_penalty: Some(0.2),
        };
        let json = serde_json::to_string(&params).expect("serialize");
        let deserialized: LlmSamplingParams = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(params, deserialized);
    }

    /// TC-MODEL-LLM-007：LlmSamplingParams skip_serializing_if 生效（None 字段不出现在 JSON）
    #[test]
    fn test_sampling_params_skip_none_fields() {
        let params = LlmSamplingParams {
            temperature: Some(0.7),
            top_p: None,
            top_k: None,
            max_tokens: Some(1024),
            frequency_penalty: None,
            presence_penalty: None,
        };
        let json = serde_json::to_string(&params).expect("serialize");
        assert!(json.contains("temperature"));
        assert!(json.contains("max_tokens"));
        assert!(!json.contains("top_p"));
        assert!(!json.contains("top_k"));
        assert!(!json.contains("frequency_penalty"));
        assert!(!json.contains("presence_penalty"));
    }

    /// TC-MODEL-LLM-008：SettingsPayload 含采样参数的序列化/反序列化
    #[test]
    fn test_settings_payload_with_sampling_params() {
        let params = LlmSamplingParams {
            temperature: Some(0.5),
            max_tokens: Some(512),
            ..Default::default()
        };
        let payload = SettingsPayload {
            has_llm_config: true,
            base_url: "https://api.openai.com".to_string(),
            model: "gpt-4o-mini".to_string(),
            api_key_masked: "****1234".to_string(),
            vlm_enabled: false,
            hybrid_search: true,
            rerank_enabled: false,
            hyde_enabled: false,
            agent_enabled: false,
            embedding_model: "all-MiniLM-L6-v2".to_string(),
            context_token_limit: 4096,
            llm_mode: "local".to_string(),
            local_model: "test.gguf".to_string(),
            llm_paged_attn: false,
            llm_block_size: 32,
            llm_gpu_memory_ctx: 4096,
            llm_sampling: Some(params),
            token_budget: 0,
            cache_enabled: true,
            cache_ttl_secs: 86400,
            cache_semantic_threshold: 0.92,
            cache_privacy_mode: false,
            quality_gate_enabled: false,
            sub_agent_enabled: false,
            progressive_injection: false,
            speculative_enabled: false,
            graph_retriever_enabled: false,
            contextual_retrieval: true,
        };
        let json = serde_json::to_string(&payload).expect("serialize");
        let back: SettingsPayload = serde_json::from_str(&json).expect("deserialize");
        let sampling = back.llm_sampling.expect("sampling should exist");
        assert_eq!(sampling.temperature, Some(0.5));
        assert_eq!(sampling.max_tokens, Some(512));
        assert!(sampling.top_p.is_none());
    }

    /// TC-MODEL-LLM-009：TokenUsage 默认值全为零
    #[test]
    fn test_token_usage_default() {
        let usage = TokenUsage::default();
        assert_eq!(usage.prompt_tokens, 0);
        assert_eq!(usage.completion_tokens, 0);
        assert_eq!(usage.total_tokens, 0);
    }

    /// TC-MODEL-LLM-010：TokenUsage 序列化/反序列化往返一致
    #[test]
    fn test_token_usage_serde_roundtrip() {
        let usage = TokenUsage {
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
        };
        let json = serde_json::to_string(&usage).expect("serialize");
        let back: TokenUsage = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(usage, back);
        assert!(json.contains("prompt_tokens"));
        assert!(json.contains("completion_tokens"));
        assert!(json.contains("total_tokens"));
    }

    /// TC-MODEL-LLM-011：ConversationCost 默认值全为零
    #[test]
    fn test_conversation_cost_default() {
        let cost = ConversationCost::default();
        assert_eq!(cost.total_prompt_tokens, 0);
        assert_eq!(cost.total_completion_tokens, 0);
        assert_eq!(cost.total_tokens, 0);
        assert_eq!(cost.exchange_count, 0);
        assert_eq!(cost.token_budget, 0);
    }

    /// TC-MODEL-LLM-012：ConversationCost 序列化/反序列化往返一致
    #[test]
    fn test_conversation_cost_serde_roundtrip() {
        let cost = ConversationCost {
            total_prompt_tokens: 5000,
            total_completion_tokens: 2500,
            total_tokens: 7500,
            exchange_count: 10,
            token_budget: 100000,
        };
        let json = serde_json::to_string(&cost).expect("serialize");
        let back: ConversationCost = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(cost, back);
        assert!(json.contains("total_prompt_tokens"));
        assert!(json.contains("token_budget"));
        assert!(json.contains("exchange_count"));
    }

    /// TC-MODEL-CACHE-001：CacheLevel 序列化/反序列化往返一致
    #[test]
    fn test_cache_level_serde_roundtrip() {
        for level in [
            CacheLevel::Exact,
            CacheLevel::Semantic,
            CacheLevel::Retrieval,
        ] {
            let json = serde_json::to_string(&level).expect("serialize");
            let back: CacheLevel = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(level, back);
        }
    }

    /// TC-MODEL-CACHE-002：CacheStats 默认值全为零
    #[test]
    fn test_cache_stats_default() {
        let stats = CacheStats::default();
        assert!(!stats.enabled);
        assert_eq!(stats.exact_hits, 0);
        assert_eq!(stats.semantic_hits, 0);
        assert_eq!(stats.retrieval_hits, 0);
        assert_eq!(stats.total_queries, 0);
        assert_eq!(stats.cache_size_entries, 0);
        assert_eq!(stats.estimated_token_saved, 0);
    }

    /// TC-MODEL-CACHE-003：CacheSettingsPayload 默认值正确
    #[test]
    fn test_cache_settings_default() {
        let settings = CacheSettingsPayload::default();
        assert!(settings.enabled);
        assert_eq!(settings.ttl_secs, 86400);
        assert!((settings.semantic_threshold - 0.92).abs() < 1e-6);
        assert!(!settings.privacy_mode);
    }

    /// TC-MODEL-CACHE-004：CacheSettingsPayload 序列化/反序列化往返一致
    #[test]
    fn test_cache_settings_serde_roundtrip() {
        let settings = CacheSettingsPayload {
            enabled: false,
            ttl_secs: 3600,
            semantic_threshold: 0.95,
            privacy_mode: true,
        };
        let json = serde_json::to_string(&settings).expect("serialize");
        let back: CacheSettingsPayload = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(settings.enabled, back.enabled);
        assert_eq!(settings.ttl_secs, back.ttl_secs);
        assert!((settings.semantic_threshold - back.semantic_threshold).abs() < 1e-6);
        assert_eq!(settings.privacy_mode, back.privacy_mode);
    }

    /// TC-MODEL-CACHE-005：SettingsPayload 缓存字段默认值正确
    #[test]
    fn test_settings_payload_cache_defaults() {
        let json = r#"{
            "has_llm_config": false,
            "base_url": "",
            "model": "",
            "api_key_masked": "",
            "vlm_enabled": false,
            "hybrid_search": true,
            "rerank_enabled": false,
            "hyde_enabled": false,
            "agent_enabled": false,
            "embedding_model": "all-MiniLM-L6-v2",
            "context_token_limit": 4096
        }"#;
        let payload: SettingsPayload = serde_json::from_str(json).expect("deserialize");
        assert!(payload.cache_enabled);
        assert_eq!(payload.cache_ttl_secs, 86400);
        assert!((payload.cache_semantic_threshold - 0.92).abs() < 1e-6);
        assert!(!payload.cache_privacy_mode);
    }

    /// TC-MODEL-TEMPLATE-001：PromptTemplate 序列化/反序列化往返一致
    #[test]
    fn test_prompt_template_serde_roundtrip() {
        let tmpl = PromptTemplate {
            id: "test-id-123".to_string(),
            name: "explain".to_string(),
            label: "解释概念".to_string(),
            description: "用通俗易懂的方式解释概念".to_string(),
            icon: "💡".to_string(),
            prompt_template: "请用通俗易懂的方式解释以下概念：{query}".to_string(),
            created_at: 1700000000,
            updated_at: 1700000001,
        };
        let json = serde_json::to_string(&tmpl).expect("serialize");
        let back: PromptTemplate = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(tmpl, back);
        assert!(json.contains("prompt_template"));
        assert!(json.contains("{query}"));
    }

    /// TC-MODEL-TEMPLATE-002：PromptTemplate::new 自动生成 UUID 和时间戳
    #[test]
    fn test_prompt_template_new() {
        let tmpl = PromptTemplate::new(
            "review".to_string(),
            "代码审查".to_string(),
            "审查代码质量".to_string(),
            "🔍".to_string(),
            "请审查以下代码：{query}".to_string(),
        );
        assert!(!tmpl.id.is_empty());
        assert_eq!(tmpl.name, "review");
        assert_eq!(tmpl.label, "代码审查");
        assert_eq!(tmpl.created_at, tmpl.updated_at);
    }

    /// TC-MODEL-TEMPLATE-003：is_valid_name 验证合法名称
    #[test]
    fn test_prompt_template_valid_name() {
        assert!(PromptTemplate::is_valid_name("summary"));
        assert!(PromptTemplate::is_valid_name("my_command_1"));
        assert!(PromptTemplate::is_valid_name("a"));
        assert!(!PromptTemplate::is_valid_name(""));
        assert!(!PromptTemplate::is_valid_name("Summary")); // 大写不允许
        assert!(!PromptTemplate::is_valid_name("my-command")); // 横线不允许
        assert!(!PromptTemplate::is_valid_name("中文")); // 非 ASCII 不允许
        assert!(!PromptTemplate::is_valid_name(&"x".repeat(33))); // 超长
    }

    /// TC-MODEL-TEMPLATE-004：has_query_placeholder 验证占位符
    #[test]
    fn test_prompt_template_query_placeholder() {
        assert!(PromptTemplate::has_query_placeholder("请总结：{query}"));
        assert!(PromptTemplate::has_query_placeholder("{query}"));
        assert!(!PromptTemplate::has_query_placeholder("请总结以下内容"));
        assert!(!PromptTemplate::has_query_placeholder(""));
    }

    // ------------------------------------------------------------------
    // WikiLink 结构体测试（REQ-ING-020）
    // ------------------------------------------------------------------

    /// TC-MODEL-WIKI-001：WikiLink::new 自动生成 UUID 和时间戳
    #[test]
    fn test_wiki_link_new() {
        let link = WikiLink::new(
            "doc-001".to_string(),
            "设计文档".to_string(),
            "chunk-001".to_string(),
        );
        assert!(!link.id.is_empty());
        assert_eq!(link.source_doc_id, "doc-001");
        assert_eq!(link.target, "设计文档");
        assert_eq!(link.chunk_id, "chunk-001");
        assert!(link.created_at > 0);
    }

    /// TC-MODEL-WIKI-002：WikiLink 序列化/反序列化往返一致
    #[test]
    fn test_wiki_link_serde_roundtrip() {
        let link = WikiLink::new(
            "doc-abc".to_string(),
            "API Design".to_string(),
            "chunk-xyz".to_string(),
        );
        let json = serde_json::to_string(&link).expect("serialize");
        let back: WikiLink = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(link.id, back.id);
        assert_eq!(link.source_doc_id, back.source_doc_id);
        assert_eq!(link.target, back.target);
        assert_eq!(link.chunk_id, back.chunk_id);
        assert_eq!(link.created_at, back.created_at);
    }

    /// TC-MODEL-WIKI-003：WikiLink 字段非空验证
    #[test]
    fn test_wiki_link_fields_non_empty() {
        let link = WikiLink::new(
            "source-doc".to_string(),
            "target-note".to_string(),
            "chunk-id".to_string(),
        );
        assert!(!link.id.is_empty(), "id 不应为空");
        assert!(!link.source_doc_id.is_empty(), "source_doc_id 不应为空");
        assert!(!link.target.is_empty(), "target 不应为空");
        assert!(!link.chunk_id.is_empty(), "chunk_id 不应为空");
    }
}

/// 对话分支树节点测试（REQ-RAG-039）。
#[cfg(test)]
mod conversation_tree_tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    /// TC-MODEL-TREE-001：ConversationTreeNode 构造与字段验证
    #[test]
    fn test_conversation_tree_node_new() {
        let node =
            ConversationTreeNode::new("conv-001", "turn-abc", 1, 1700000000, "这是一条测试消息");
        assert_eq!(node.node_id, "turn-abc#1");
        assert_eq!(node.conversation_id, "conv-001");
        assert_eq!(node.turn_group, "turn-abc");
        assert_eq!(node.version, 1);
        assert_eq!(node.created_at, 1700000000);
        assert_eq!(node.preview, "这是一条测试消息");
        assert!(node.is_root(), "无 parent_message_id 时应为根节点");
        assert!(node.is_leaf(), "无 child_message_ids 时应为叶子节点");
        assert!(node.parent_message_id.is_none());
        assert!(node.child_message_ids.is_empty());
        assert!(node.active_child.is_none());
    }

    /// TC-MODEL-TREE-002：ConversationTree 空树与查找
    #[test]
    fn test_conversation_tree_empty_and_find() {
        let tree = ConversationTree::empty("conv-002");
        assert_eq!(tree.conversation_id, "conv-002");
        assert!(tree.nodes.is_empty());
        assert!(tree.root_ids.is_empty());
        assert!(tree.active_path.is_empty());
        assert!(tree.find("nonexistent").is_none());

        // 添加一个节点后查找
        let node = ConversationTreeNode::new("conv-002", "turn-1", 1, 1700000000, "hello");
        let mut tree = ConversationTree::empty("conv-002");
        tree.nodes.push(node.clone());
        tree.root_ids.push(node.node_id.clone());
        let found = tree.find(&node.node_id);
        assert!(found.is_some());
        assert_eq!(found.unwrap().node_id, node.node_id);
        assert_eq!(tree.root_ids.len(), 1);
    }

    /// TC-MODEL-TREE-003：预览截断（超 100 字符时截断并添加省略号）
    #[test]
    fn test_preview_truncation() {
        let long_text = "a".repeat(200);
        let node = ConversationTreeNode::new("conv", "turn", 1, 0, &long_text);
        assert_eq!(node.preview.chars().count(), 101); // 100 chars + …
        assert!(node.preview.ends_with('…'));

        let short_text = "short message";
        let node2 = ConversationTreeNode::new("conv", "turn", 1, 0, short_text);
        assert_eq!(node2.preview, short_text);
    }

    /// TC-MODEL-TREE-004：UTF-8 安全截断（多字节字符不 panic）
    #[test]
    fn test_preview_truncation_utf8_safe() {
        // 中文字符每个 3 字节，100 个中文字符 = 300 字节
        let long_chinese = "测".repeat(150);
        let node = ConversationTreeNode::new("conv", "turn", 1, 0, &long_chinese);
        assert_eq!(node.preview.chars().count(), 101); // 100 chars + …
        assert!(node.preview.ends_with('…'));
    }
}

#[cfg(test)]
mod pending_input_tests {
    use super::PendingInput;

    /// TC-MODEL-PENDING-001：new() 创建的 PendingInput 初始状态正确
    #[test]
    fn test_pending_input_new() {
        let pi = PendingInput::new(
            "conv-001".to_string(),
            "hello world".to_string(),
            "queue".to_string(),
        );
        assert!(!pi.id.is_empty());
        assert_eq!(pi.conversation_id, "conv-001");
        assert_eq!(pi.content, "hello world");
        assert_eq!(pi.delivery, "queue");
        assert!(pi.promoted_seq.is_none());
    }

    /// TC-MODEL-PENDING-002：steer 模式创建
    #[test]
    fn test_pending_input_steer() {
        let pi = PendingInput::new(
            "conv-002".to_string(),
            "stop and answer this".to_string(),
            "steer".to_string(),
        );
        assert_eq!(pi.delivery, "steer");
        assert!(pi.promoted_seq.is_none());
    }

    /// TC-MODEL-PENDING-003：每个 new() 生成不同 UUID
    #[test]
    fn test_pending_input_unique_id() {
        let pi1 = PendingInput::new("c".to_string(), "a".to_string(), "queue".to_string());
        let pi2 = PendingInput::new("c".to_string(), "a".to_string(), "queue".to_string());
        assert_ne!(pi1.id, pi2.id);
    }

    /// TC-MODEL-PENDING-004：created_at 有效（Unix 时间戳）
    #[test]
    fn test_pending_input_created_at() {
        let before = chrono::Utc::now().timestamp();
        let pi = PendingInput::new("c".to_string(), "a".to_string(), "queue".to_string());
        let after = chrono::Utc::now().timestamp();
        assert!(pi.created_at >= before && pi.created_at <= after);
    }
}

#[cfg(test)]
mod session_todo_tests {
    use super::*;

    /// TC-MODEL-TODO-001：new() 创建的 SessionTodo 初始状态正确
    #[test]
    fn test_session_todo_new() {
        let todo = SessionTodo::new("conv-001".to_string(), "完成数据分析".to_string(), 0);
        assert!(!todo.id.is_empty());
        assert_eq!(todo.conversation_id, "conv-001");
        assert_eq!(todo.content, "完成数据分析");
        assert_eq!(todo.status, TodoStatus::Pending);
        assert_eq!(todo.priority, TodoPriority::Medium);
        assert_eq!(todo.position, 0);
        assert!(todo.created_at > 0);
    }

    /// TC-MODEL-TODO-002：TodoStatus as_str / from_str 往返正确
    #[test]
    fn test_todo_status_roundtrip() {
        for status in [
            TodoStatus::Pending,
            TodoStatus::InProgress,
            TodoStatus::Completed,
        ] {
            let s = status.as_str();
            assert_eq!(TodoStatus::from_db_str(s), Some(status));
        }
        assert!(TodoStatus::from_db_str("invalid").is_none());
    }

    /// TC-MODEL-TODO-003：TodoPriority as_str / from_str 往返正确
    #[test]
    fn test_todo_priority_roundtrip() {
        for prio in [TodoPriority::Low, TodoPriority::Medium, TodoPriority::High] {
            let s = prio.as_str();
            assert_eq!(TodoPriority::from_db_str(s), Some(prio));
        }
        assert!(TodoPriority::from_db_str("invalid").is_none());
    }

    /// TC-MODEL-TODO-004：每个 new() 生成不同 UUID
    #[test]
    fn test_session_todo_unique_id() {
        let t1 = SessionTodo::new("c".to_string(), "a".to_string(), 0);
        let t2 = SessionTodo::new("c".to_string(), "a".to_string(), 0);
        assert_ne!(t1.id, t2.id);
    }
}

#[cfg(test)]
mod provenance_tag_tests {
    #![allow(clippy::unwrap_used)]
    use super::{GenerationParams, ProvenanceTag, RagParams, StripResult};

    /// TC-MODEL-PROV-001：new() 创建的 ProvenanceTag 字段正确
    #[test]
    fn test_provenance_tag_new() {
        let tag = ProvenanceTag::new(
            "conv-001".to_string(),
            3,
            "对话：知识产权法咨询 的第 3 轮".to_string(),
        );
        assert_eq!(tag.conversation_id, "conv-001");
        assert_eq!(tag.message_seq, 3);
        assert!(tag.captured_at > 0);
        assert_eq!(tag.source_label, "对话：知识产权法咨询 的第 3 轮");
    }

    /// TC-MODEL-PROV-002：with_timestamp 创建带指定时间戳的标记
    #[test]
    fn test_provenance_tag_with_timestamp() {
        let tag = ProvenanceTag::with_timestamp(
            "conv-002".to_string(),
            1,
            1700000000,
            "对话：测试".to_string(),
        );
        assert_eq!(tag.captured_at, 1700000000);
    }

    /// TC-MODEL-PROV-003：said_in_suffix 生成 (said in ...) 后缀
    #[test]
    fn test_provenance_tag_said_in_suffix() {
        let tag = ProvenanceTag::new(
            "conv-001".to_string(),
            2,
            "对话：法律咨询 的第 2 轮".to_string(),
        );
        let suffix = tag.said_in_suffix();
        assert_eq!(suffix, " (said in 对话：法律咨询 的第 2 轮)");
    }

    /// TC-MODEL-PROV-004：Serialize/Deserialize 往返一致
    #[test]
    fn test_provenance_tag_serde_roundtrip() {
        let tag = ProvenanceTag::with_timestamp(
            "conv-003".to_string(),
            5,
            1700000000,
            "对话：Rust 讨论 的第 5 轮".to_string(),
        );
        let json = serde_json::to_string(&tag).unwrap();
        let deserialized: ProvenanceTag = serde_json::from_str(&json).unwrap();
        assert_eq!(tag, deserialized);
    }

    /// TC-RAG-PARAMS-001：RagParams 默认值
    #[test]
    fn test_rag_params_default() {
        let params = RagParams::default();
        assert_eq!(params.top_k, 8);
        assert!((params.score_threshold - 0.0).abs() < f32::EPSILON);
        assert!(params.chunk_expansion_enabled);
        assert_eq!(params.chunk_expansion_window, 1);
    }

    /// TC-RAG-PARAMS-002：RagParams clamp 超范围值
    #[test]
    fn test_rag_params_clamped() {
        let params = RagParams {
            top_k: 100,
            score_threshold: -1.5,
            chunk_expansion_enabled: false,
            chunk_expansion_window: 10,
        };
        let clamped = params.clamped();
        assert_eq!(clamped.top_k, 20);
        assert!((clamped.score_threshold - 0.0).abs() < f32::EPSILON);
        assert!(!clamped.chunk_expansion_enabled);
        assert_eq!(clamped.chunk_expansion_window, 3);
    }

    /// TC-RAG-PARAMS-003：RagParams serde 往返
    #[test]
    fn test_rag_params_serde_roundtrip() {
        let params = RagParams::default();
        let json = serde_json::to_string(&params).unwrap();
        let deserialized: RagParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    /// TC-LLM-PARAMS-001：GenerationParams 默认值
    #[test]
    fn test_generation_params_default() {
        let params = GenerationParams::default();
        assert!((params.temperature - 0.7).abs() < f32::EPSILON);
        assert_eq!(params.max_tokens, 4096);
        assert!((params.top_p - 1.0).abs() < f32::EPSILON);
    }

    /// TC-LLM-PARAMS-002：GenerationParams clamp 超范围值
    #[test]
    fn test_generation_params_clamped() {
        let params = GenerationParams {
            temperature: 5.0,
            max_tokens: 100,
            top_p: 2.5,
        };
        let clamped = params.clamped();
        assert!((clamped.temperature - 2.0).abs() < f32::EPSILON);
        assert_eq!(clamped.max_tokens, 256);
        assert!((clamped.top_p - 1.0).abs() < f32::EPSILON);
    }

    /// TC-LLM-PARAMS-003：GenerationParams serde 往返
    #[test]
    fn test_generation_params_serde_roundtrip() {
        let params = GenerationParams::default();
        let json = serde_json::to_string(&params).unwrap();
        let deserialized: GenerationParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    /// TC-STRIP-RESULT-001：StripResult serde 往返
    #[test]
    fn test_strip_result_serde_roundtrip() {
        let result = StripResult {
            stripped_count: 10,
            summary_inserted: true,
            stripped_message_ids: vec!["msg-1".to_string(), "msg-2".to_string()],
            estimated_tokens_saved: 500,
            summary: "用户讨论了 Rust 异步编程和 Tauri 框架".to_string(),
            kept_count: 6,
        };
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: StripResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }
}
