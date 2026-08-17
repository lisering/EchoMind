//! EchoMind 六边形架构核心层（对应 REQ-ARCH-000）。
//! 铁律（PROJECT_RULES.md 体系三）：本 crate 只定义端口 Trait 与领域逻辑，
//! 严禁依赖 Tauri、reqwest、rusqlite 等任何具体实现；适配器一律落于 crates/infra（后续 Phase）。

// 体系三锁定技术栈：使用原生 `async fn` in trait（Edition 2024），禁用 async-trait 宏。
// `async_fn_in_trait` lint 提醒「pub trait 无法约束返回 Future 的 auto traits」；
// 本 crate 通过 `Send + Sync` supertrait 约束全部实现者，且端口仅供 workspace 内部消费，
// 属经架构评审的刻意豁免，非疏漏。
#![allow(async_fn_in_trait)]

/// Agentic RAG 多步推理引擎（REQ-RAG-022）：ReAct 范式 Thought→Action→Observation 循环。
pub mod agent;
/// 文档一致性审计引擎：全量扫描式矛盾检测（REQ-AUDIT-001~005）。
#[cfg(feature = "pro")]
pub mod audit;
/// 审计对 + Turn-Enclosed 模块（DH-04 借鉴 DeepSeek Harness Turn-Enclosed Audit）：
/// 每次 turn 生成 start/end 审计对，验证完整性。
pub mod audit_pair;
/// 后台空闲整理引擎：重复文档检测、跨文档矛盾发现、整理建议生成。
pub mod auto_dream;
/// 预算追踪和速率限制（QM 借鉴）：LLM API 费用控制和请求限流。
pub mod budget;
/// 语义缓存金字塔（REQ-PERF-001）：L0 精确 + L1 语义 + L3 检索结果三级缓存。
pub mod cache;
pub mod chat;
/// 代码执行沙箱（REQ-RAG-032，Pro feature）：安全执行代码片段并返回结果。
///
/// 借鉴 CodeForge 多语言执行器：Agent 在回答代码相关问题时可执行代码片段验证结果。
/// 端口定义在 core，真实实现在 infra（`WasmExecutor`，Pro 门控）。
pub mod code_executor;
/// 并发控制模块（Q09 借鉴 QM createKeyedQueue）：按 key 序列化异步操作，消除 SQLite 并发写入竞态。
pub mod concurrency;
/// 多代理协调引擎（REQ-RAG-025）：Research→Synthesis→Implementation→Verification 四阶段。
pub mod coordinator;
/// Coordinator 策略注册表（DH-03 借鉴 DeepSeek Harness SubagentRuntime Provider 注册表）：
/// 多个协调策略共存，按名字查找，能力验证在执行前完成。
pub mod coordinator_strategy;
/// 领域画像自动分类（REQ-VEC-013）：16 领域嵌入质心分类。
pub mod domain;
/// 数据库加密模块（Argon2id 密钥派生 + SQLCipher 加密 + 暴力破解防护 + 密码强度检测）。
pub mod encryption;
/// NER 实体抽取器（REQ-PERF-006）：纯规则 + 正则，零 LLM/模型依赖。
pub mod entity_extractor;
/// 统一错误分类体系（REQ-ERR-001）：错误前缀常量 + 分类辅助函数。
pub mod errors;
/// 事件流自动埋点（借鉴 OpenMontage events.py）：追加写入 JSONL + 容错读取 + 零负担。
pub mod event_stream;
/// 对话导出模块：会话 → Markdown 文件（REQ-EXP-001）。
pub mod export;
/// 知识图谱高级分析引擎（REQ-RAG-027 Session 5）：最短路径 + 社区检测 + 度中心性。
pub mod graph_analyzer;
/// 知识图谱导出模块（REQ-EXP-006）：GraphML + JSON-LD 双格式导出。
pub mod graph_export;
/// 知识图谱图遍历检索器（REQ-RAG-027）：沿实体关系图边扩展到关联 chunk。
pub mod graph_retriever;
/// Agent 生命周期 Hooks 系统（REQ-RAG-029）：在 Agent/Coordinator 关键节点插入可扩展的插件式 hook。
pub mod hooks;
/// 混合检索器：向量 + 关键词 RRF 融合（REQ-RAG-010）。
pub mod hybrid_retriever;
/// 幂等性存储 + 统一周期任务抽象：防重复操作（文件同步/AutoDream）+ 后台任务管理。
pub mod idempotency;
pub mod import;
/// Late Chunking 上下文感知嵌入（REQ-RAG-049）：借鉴 Jina AI 2024 Late Chunking
/// 技术，在嵌入阶段为 chunk 注入文档级上下文前缀。
pub mod late_chunking;
pub mod license;
/// 多协议 LLM 适配（B10 LLM Protocol Types，REQ-ARCH-012）：
/// LLM 协议类型枚举 + 协议感知 Provider trait。
pub mod llm_protocol;
/// LLM 后端路由器（Q11 借鉴 QM HarnessRouter）：
/// 按对话或用户偏好动态切换 LLM 后端，切换时通知重置会话状态。
pub mod llm_router;
pub mod loader;
/// 持久化记忆系统（REQ-RAG-032）：借鉴 IfAI 三层记忆 Wing/Hall/Room + Bamboo-agent 上下文压缩。
///
/// 三层记忆架构：
/// - Wing（翼层）：临时记忆，当前会话产生，上限 50 条
/// - Hall（厅层）：工作记忆，近期重要，上限 100 条
/// - Room（室层）：长期记忆，核心知识，上限 500 条
///
/// 自动从对话中提取关键事实（LLM 辅助），查询时注入相关记忆到 RAG prompt，
/// AutoDream 后台空闲时自动整合（提升/遗忘）。
pub mod memory_store;
/// 语义分块器：段落→句子→子句递归分割，保留代码块完整性。
/// MMR 多样性重排（借鉴 OpenMontage corpus.py）：Maximal Marginal Relevance。
pub mod mmr_diversifier;
/// 性能指标采集（REQ-OBS-002）：关键操作耗时追踪。
pub mod perf_metrics;
/// Permission 细粒度控制（B11 Permission Rule Engine，REQ-ARCH-011）：
/// Wildcard 匹配的 RBAC 权限规则引擎（Allow / Deny / Ask）。
pub mod permission;
/// 流水线阶段门控（借鉴 OpenMontage checkpoint.py）：前置检查 + 人审门控 + 历史记录。
pub mod pipeline_checkpoint;
pub mod privacy;
/// 渐进式上下文注入（REQ-PERF-010）：按需注入 chunk，LLM 上下文足够时停止追加。
pub mod progressive_injector;
/// Prompt 压缩模块（REQ-PERF-002）：规则压缩（Free 默认方案）。
pub mod prompt_compressor;
/// 隐私保护模块（PII 检测脱敏 8 类 + 审计日志防篡改哈希链）。
/// Proposition 级原子分割（REQ-PERF-007）：将 chunk 分解为自包含的原子事实。
pub mod proposition_splitter;
/// RAG 质量门控系统（REQ-RAG-028）：检索后评估结果质量，低质量时触发降级策略。
pub mod quality_gate;
/// RAG 评估指标系统（REQ-RAG-045，RAGAS 风格）：
/// 纯 Rust 检索指标 + LLM-as-Judge 生成指标。
pub mod rag_eval;
/// RAG 评估数据集与端到端检索质量评估（REQ-RAG-048）：
/// 标准评估数据集 + run_retrieval_eval 管线。
pub mod rag_eval_dataset;
/// 自进化检索记忆（REQ-PERF-012）：记录检索方法效果，自适应选择最佳策略。
pub mod retrieval_memory;
/// 检索质量门控（借鉴 OpenMontage slideshow_risk.py）：多维度评分 + verdict 系统。
pub mod retrieval_quality_gate;
pub mod retriever;
/// LLM Provider 评分选择引擎（借鉴 OpenMontage scoring.py）：多维度加权评分 + 可解释选择。
pub mod scored_selector;
/// 章节感知分块器：Markdown 标题层级 → section 切分 → 章节路径前缀（REQ-VEC-006）。
pub mod section_aware_splitter;
/// 安全管理模块（自动锁屏 + 剪贴板清除 + 紧急销毁 + 系统睡眠检测）。
pub mod security;
pub mod semantic_splitter;
/// Session Run Coordinator（B06 会话运行协调器）：
/// 同会话串行 / 跨会话并发 / wake 合并 / interrupt 中断。
pub mod session_coordinator;
/// Session Strip（会话条带化，REQ-RAG-046）：借鉴 ds4 /strip 命令，移除消息减少上下文消耗。
pub mod session_strip;
/// Skill 系统发现（B09 Skill Discovery，REQ-ARCH-010）：
/// 从 Markdown 文件的 YAML frontmatter 解析技能信息。
pub mod skill;
/// Speculative RAG（REQ-PERF-011）：小模型快速生成草稿 → 大模型验证/修正。
pub mod speculative_rag;
pub mod splitter;
/// StepCache 步骤级缓存（P2-1）：Agent/Coordinator 多步推理中间步骤结果复用。
pub mod step_cache;
pub mod stream_parse;
/// 子代理舰队管理（REQ-RAG-025 扩展）：独立 AgentEngine 实例 + mailbox 通信。
///
/// 借鉴 Bamboo-agent 子代理系统：主代理可动态创建子代理，每个子代理有独立上下文
/// 和工具集，通过 mailbox 消息传递协调。支持「分工→并行→汇总」模式。
pub mod sub_agent;
/// RAPTOR 摘要树模块（REQ-PERF-009）：多级摘要树索引构建与检索。
pub mod summary_tree;
/// 文件监听增量同步模块（REQ-SYNC-002）：对比文件夹与知识库，增量导入/更新/删除。
pub mod sync;
/// 工具输出有界截断（B07 Tool Output Bounding，REQ-RAG-043）：
/// 超大工具输出保留首尾截断，防止 LLM 上下文被冗长输出淹没。
pub mod tool_output;
/// 轻量 RAG 链路追踪系统（S70：Cherry Studio 借鉴 — span 级耗时追踪）。
pub mod trace;
/// 应用更新检查（REQ-HELP-004）：GitHub Releases 版本比较。
pub mod update_check;
/// VLM 分级图表理解提示词（REQ-MM-005）：4 级精度策略集中定义。
#[cfg(feature = "pro")]
pub mod vlm_prompt;
/// 网页搜索融合引擎（REQ-RAG-036）：阈值触发判断 + RRF 融合本地与 Web 结果。
///
/// 当本地检索 top-1 score 低于阈值时，调用 `WebSearchProvider` 搜索互联网，
/// 将搜索结果转换为 `RetrievalResult` 后通过 RRF 融合到本地结果中。
/// 搜索失败时优雅降级为仅使用本地结果。
pub mod web_search;
/// 网页搜索端口（REQ-RAG-036）：当本地知识库检索结果不足时搜索互联网补充 context。
///
/// ## 触发条件
///
/// 当本地检索 top-1 score < `web_search_threshold`（默认 0.3）时触发搜索。
/// 搜索结果通过 RRF 融合到本地检索结果中，在 prompt 中标注来源（🌐 Web）。
///
/// ## 优雅降级
///
/// 搜索失败时返回空 Vec（不报错），管线仅使用本地检索结果。
/// 网络超时、API 不可达等情况不影响正常对话。
///
/// ## 对象安全设计
///
/// 与 `Reranker`/`QueryRewriter` 端口相同，使用手动 `Pin<Box<Future>>` 返回类型
/// 以保证对象安全（dyn-compatible）。`WebSearchProvider` 以 `Option<Arc<dyn WebSearchProvider>>`
/// 形式注入 `ChatEngine`，实现运行时开关——用户可通过 `rag.web_search_enabled` 设置
/// 在运行时启停网页搜索，无需重编译。
///
/// ## 适配器
///
/// - `DuckDuckGoProvider`（`crates/infra/src/duckduckgo_provider.rs`）—
///   DuckDuckGo Instant Answer API，免费无需 API Key
pub trait WebSearchProvider: Send + Sync {
    /// 执行网页搜索，返回搜索结果列表。
    ///
    /// # 参数
    /// - `query`: 搜索查询文本
    ///
    /// # 返回
    /// 搜索结果列表（`SearchResult`），按相关性降序排列。
    /// 搜索失败或无结果时返回空 Vec（不返回 Err，优雅降级）。
    fn search<'a>(
        &'a self,
        query: &'a str,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = anyhow::Result<Vec<echomind_models::SearchResult>>>
                + Send
                + 'a,
        >,
    >;
}

/// Wiki-link 解析器（REQ-ING-020）：Obsidian 风格 `[[link]]` 双向链接解析。
///
/// 在导入时解析 Markdown 文档中的 `[[wiki-link]]` 语法，建立双向链接索引，
/// 支持正向链接和反向链接查询。
pub mod wiki_link_parser;

/// DAG 工作流引擎（REQ-RAG-030）：用户自定义多步骤 RAG 管线。
///
/// 借鉴 IfAI 的 DAG 工作流引擎：用户定义由节点和边组成的有向无环图，
/// 引擎负责拓扑排序、并行执行独立节点、串行执行依赖节点。
/// 支持 5 种节点类型（Retrieval / Generation / Condition / Aggregate / Output）
/// 和条件分支、聚合策略、容错传播。
pub mod workflow;

use echomind_models::{
    ChatMessage, Chunk, ChunkPreview, CodeExecutorConfig, CodeSymbol, Conversation, DocStatus,
    Document, DocumentPreview, EntityRelation, ExecutionResult, MemoryEntry, MemoryTier,
    MessageSearchResult, PendingInput, Proposition, RetrievalResult, ScratchLogEntry, SessionTodo,
    SummaryNode, SymbolKind, TodoStatus, TurnActiveVersion, WikiLink,
};
use futures::stream::BoxStream;

/// 文档加载端口：从文件路径提取纯文本（对应 REQ-ING-001/002）。
pub trait Loader: Send + Sync {
    /// 读取并解析指定文件，返回纯文本内容；不支持的格式或 I/O 错误返回 Err。
    async fn load(&self, file_path: &str) -> anyhow::Result<String>;
}

/// 文本分块端口（对应 REQ-VEC-001）。
pub trait Splitter: Send + Sync {
    /// 将整篇文本切分为有序分块；空文本返回空 Vec 而非 Err。
    async fn split(&self, text: &str) -> anyhow::Result<Vec<String>>;
}

/// Embedding 端口（对应 REQ-VEC-002）：本地模型与 BYOK 远程端点均实现本端口。
pub trait Embedder: Send + Sync {
    /// 计算单条文本的向量。
    async fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>>;

    /// 批量计算向量；默认实现逐条调用 `embed`，适配器可覆盖为真正的批量请求。
    async fn embed_batch(&self, texts: &[String]) -> anyhow::Result<Vec<Vec<f32>>>;
}

/// 持久化端口：文档、分块、向量与检索（对应 REQ-VEC-003/004、REQ-ING-005）。
pub trait Storage: Send + Sync {
    /// 写入新文档记录。
    async fn add_document(&self, doc: &Document) -> anyhow::Result<()>;

    /// 推进文档索引状态机；每次迁移由组装层负责发射 Tauri Event。
    async fn update_doc_status(&self, doc_id: &str, status: DocStatus) -> anyhow::Result<()>;

    /// 写入分块记录。
    async fn add_chunk(&self, chunk: &Chunk) -> anyhow::Result<()>;

    /// 批量写入分块（单事务，性能优化）。
    ///
    /// 生产实现应在单个 SQLite 事务中提交全部 chunks，避免逐条 INSERT 的隐式事务开销。
    /// 默认实现委托 `add_chunk` 逐条写入；适配器**必须覆盖**此方法以获得批量性能收益。
    async fn add_chunks_batch(&self, chunks: &[Chunk]) -> anyhow::Result<()> {
        for chunk in chunks {
            self.add_chunk(chunk).await?;
        }
        Ok(())
    }

    /// 为指定分块写入向量。
    async fn add_embedding(&self, chunk_id: &str, embedding: &[f32]) -> anyhow::Result<()>;

    /// 批量写入向量（性能优化：单事务提交全部 embeddings）。
    ///
    /// 生产实现应在单个事务中提交全部 embeddings，避免逐条 INSERT 的
    /// spawn_blocking + 连接获取开销。默认实现逐条调用 `add_embedding`；
    /// 适配器**必须覆盖**此方法以获得批量性能收益（400+ chunks 从 ~8s 降至 <0.1s）。
    async fn add_embeddings_batch(&self, embeddings: &[(String, Vec<f32>)]) -> anyhow::Result<()> {
        for (chunk_id, vector) in embeddings {
            self.add_embedding(chunk_id, vector).await?;
        }
        Ok(())
    }

    /// 以查询向量执行 top-k 相似度检索。
    async fn vector_search(
        &self,
        query_embedding: &[f32],
        top_k: usize,
    ) -> anyhow::Result<Vec<RetrievalResult>>;

    /// 按内容指纹查找文档（REQ-ING-004 去重）。
    async fn find_document_by_hash(&self, hash: &str) -> anyhow::Result<Option<Document>>;

    /// 按文件名查找文档（REQ-ING-012 同名不同内容检测）。
    ///
    /// 在文档列表中查找 `file_path` 以 `-{name}` 结尾的文档。
    /// file_path 格式为 `{data_dir}/documents/{hash}-{filename}`，
    /// 因此匹配 `-{name}` 后缀可正确识别同名文档。
    /// 默认实现通过 `list_documents` 遍历过滤；生产实现可覆盖为 SQL LIKE 查询。
    async fn find_document_by_name(&self, name: &str) -> anyhow::Result<Option<Document>> {
        let docs = self.list_documents().await?;
        let suffix = format!("-{name}");
        Ok(docs.into_iter().find(|d| d.file_path.ends_with(&suffix)))
    }

    /// 统计已入库文档总数（REQ-LIC-001 配额判定）。
    async fn count_documents(&self) -> anyhow::Result<usize>;

    /// 统计已入库分块总数（REQ-OBS-002 诊断信息导出）。
    async fn count_chunks(&self) -> anyhow::Result<usize>;

    /// 统计已入库向量总数（REQ-VEC-010 索引统计仪表盘）。
    ///
    /// 默认实现返回 0；生产实现覆盖为 SQL COUNT。
    async fn count_embeddings(&self) -> anyhow::Result<usize> {
        Ok(0)
    }

    /// 崩溃恢复（REQ-DB-001）：将上次会话遗留的 Processing 僵尸文档置为 Failed，返回清理条数。
    async fn cleanup_zombies(&self) -> anyhow::Result<usize>;

    /// 写入设置项（REQ-UI-008；敏感值由实现方负责加密存储）。
    async fn set_setting(&self, key: &str, value: &str) -> anyhow::Result<()>;

    /// 读取设置项（REQ-UI-008）。
    async fn get_setting(&self, key: &str) -> anyhow::Result<Option<String>>;

    /// 批量读取多个设置项（性能优化：单次 DB 查询替代 N 次串行查询）。
    ///
    /// 默认实现逐个调用 `get_setting`（N 次 spawn_blocking）；
    /// 生产实现应覆盖为单次 SQL `WHERE key IN (...)` 查询。
    async fn get_settings_batch(&self, keys: &[&str]) -> anyhow::Result<Vec<(String, String)>> {
        let mut results = Vec::with_capacity(keys.len());
        for &key in keys {
            if let Some(value) = self.get_setting(key).await? {
                results.push((key.to_string(), value));
            }
        }
        Ok(results)
    }

    /// 创建会话（REQ-RAG-006；重复 ID 忽略，保证幂等）。
    async fn create_conversation(&self, conversation: &Conversation) -> anyhow::Result<()>;

    /// 列出工作区会话（按创建时间倒序）。
    async fn list_conversations(&self, workspace_id: &str) -> anyhow::Result<Vec<Conversation>>;

    /// 列出工作区会话（分页，按创建时间倒序）。
    ///
    /// 默认实现委托 `list_conversations` 后截取；生产实现应覆盖为 SQL LIMIT/OFFSET。
    async fn list_conversations_paginated(
        &self,
        workspace_id: &str,
        limit: usize,
        offset: usize,
    ) -> anyhow::Result<Vec<Conversation>> {
        let all = self.list_conversations(workspace_id).await?;
        Ok(all.into_iter().skip(offset).take(limit).collect())
    }

    /// 统计工作区会话总数。
    ///
    /// 默认实现委托 `list_conversations` 后计数；生产实现应覆盖为 SQL COUNT。
    async fn count_conversations(&self, workspace_id: &str) -> anyhow::Result<usize> {
        Ok(self.list_conversations(workspace_id).await?.len())
    }

    // ========================================================================
    // 工作空间管理（REQ-WS-001 多知识库创建与切换）
    // ========================================================================

    /// 创建工作空间（REQ-WS-001）。
    ///
    /// 插入 `workspaces` 表。重复 ID 忽略（幂等）。
    /// 默认实现为空操作（兼容旧版 MockStorage）；生产实现必须覆盖。
    async fn create_workspace(
        &self,
        _workspace: &echomind_models::Workspace,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    /// 列出全部工作空间（按创建时间正序，REQ-WS-001 AC-1）。
    ///
    /// 默认实现返回单个默认工作空间（兼容旧版）；生产实现必须覆盖。
    async fn list_workspaces(&self) -> anyhow::Result<Vec<echomind_models::Workspace>> {
        Ok(vec![echomind_models::Workspace::with_id(
            "default".to_string(),
            "Default".to_string(),
        )])
    }

    /// 重命名工作空间（REQ-WS-003 AC-1/AC-2）。
    ///
    /// 默认实现为空操作；生产实现必须覆盖。
    async fn rename_workspace(&self, _id: &str, _name: &str) -> anyhow::Result<()> {
        Ok(())
    }

    /// 删除工作空间（REQ-WS-003 AC-4 级联清理）。
    ///
    /// 删除指定工作空间及其全部文档、chunks、向量、会话、消息。
    /// 默认实现为空操作；生产实现必须覆盖。
    async fn delete_workspace(&self, _id: &str) -> anyhow::Result<()> {
        Ok(())
    }

    /// 获取工作空间数据量预览（REQ-WS-003 AC-3 删除确认对话框）。
    ///
    /// 返回该工作空间的文档数和会话数，用于删除前确认对话框展示。
    /// 默认实现返回零值；生产实现必须覆盖。
    async fn get_workspace_stats(
        &self,
        _id: &str,
    ) -> anyhow::Result<echomind_models::WorkspaceStats> {
        Ok(echomind_models::WorkspaceStats {
            document_count: 0,
            conversation_count: 0,
        })
    }

    /// 统计工作空间文档数（REQ-WS-001 AC-4 数据隔离）。
    ///
    /// 默认实现返回全部文档数；生产实现应覆盖为按 workspace_id 过滤。
    async fn count_documents_in_workspace(&self, _workspace_id: &str) -> anyhow::Result<usize> {
        self.count_documents().await
    }

    /// 列出工作空间文档（REQ-WS-001 AC-3 切换后同步刷新）。
    ///
    /// 默认实现返回全部文档；生产实现应覆盖为按 workspace_id 过滤。
    async fn list_documents_in_workspace(
        &self,
        _workspace_id: &str,
    ) -> anyhow::Result<Vec<Document>> {
        self.list_documents().await
    }

    /// 迁移文档到目标工作空间（REQ-WS-004 跨知识库迁移）。
    ///
    /// 仅更新 `documents.workspace_id`，chunks / 向量 / propositions 等通过
    /// 外键关联文档 ID 自动归属新工作空间，无需额外修改。
    ///
    /// 默认实现为空操作（返回 Ok(())）；生产实现应覆盖为 SQL UPDATE。
    async fn migrate_document(
        &self,
        _doc_id: &str,
        _target_workspace_id: &str,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    /// 删除会话（外键级联清理其消息）。
    async fn delete_conversation(&self, id: &str) -> anyhow::Result<()>;

    /// 按 ID 查找单个会话（REQ-EXP-001 导出功能用）。
    ///
    /// 默认实现委托 `list_conversations` 后过滤；生产实现应覆盖为直接 SQL 查询。
    async fn get_conversation(&self, id: &str) -> anyhow::Result<Option<Conversation>> {
        // v1.0 单工作区，直接遍历 default 工作区
        let all = self.list_conversations("default").await?;
        Ok(all.into_iter().find(|c| c.id == id))
    }

    /// 更新会话标题（首轮问答后自动提取）。
    async fn update_conversation_title(&self, id: &str, title: &str) -> anyhow::Result<()>;

    /// 批量更新会话排序（REQ-IX-002 拖拽排序持久化）。
    ///
    /// 接收有序的会话 ID 列表，按列表顺序为每个会话设置递增的 `sort_order`。
    /// 排序后 `list_conversations` 返回结果按 `sort_order ASC, created_at DESC` 排序。
    async fn reorder_conversations(&self, _ordered_ids: &[String]) -> anyhow::Result<()> {
        Ok(())
    }

    /// 写入一条消息。
    async fn add_message(&self, conversation_id: &str, message: &ChatMessage)
    -> anyhow::Result<()>;

    /// 列出会话消息（按写入顺序正序）。
    async fn list_messages(&self, conversation_id: &str) -> anyhow::Result<Vec<ChatMessage>>;

    /// 列出会话消息（分页，从最新消息向前加载）。
    ///
    /// 返回按写入顺序正序排列的消息（最旧在前），但分页从最新消息开始：
    /// - `offset=0` 返回最近 `limit` 条消息
    /// - `offset=N` 返回倒数第 N+1 到 N+limit 条消息
    ///
    /// 默认实现委托 `list_messages` 后截取；生产实现应覆盖为 SQL LIMIT/OFFSET。
    async fn list_messages_paginated(
        &self,
        conversation_id: &str,
        limit: usize,
        offset: usize,
    ) -> anyhow::Result<Vec<ChatMessage>> {
        let all = self.list_messages(conversation_id).await?;
        let total = all.len();
        let start = total.saturating_sub(offset + limit);
        let end = total.saturating_sub(offset);
        Ok(all[start..end].to_vec())
    }

    /// 统计会话消息总数。
    ///
    /// 默认实现委托 `list_messages` 后计数；生产实现应覆盖为 SQL COUNT。
    async fn count_messages(&self, conversation_id: &str) -> anyhow::Result<usize> {
        Ok(self.list_messages(conversation_id).await?.len())
    }

    /// 按 ID 批量删除消息（REQ-RAG-046 Session Strip）。
    ///
    /// 从指定会话中删除消息 ID 列表对应的消息行。
    /// 默认实现为空操作（内存存储）；SqliteStorage 覆盖为 DELETE IN (?)。
    ///
    /// # 参数
    /// - `conversation_id`：会话 ID（安全约束：只删除该会话的消息）
    /// - `message_ids`：要删除的消息 ID 列表
    ///
    /// # 返回
    /// 实际删除的行数。
    async fn delete_messages_by_ids(
        &self,
        _conversation_id: &str,
        _message_ids: &[String],
    ) -> anyhow::Result<usize> {
        Ok(0)
    }

    /// 设置轮次的活跃版本号（分支切换状态持久化）。
    ///
    /// 默认实现为空操作（内存存储无需持久化）；生产实现覆盖为 SQL UPSERT。
    async fn set_turn_active_version(
        &self,
        _conversation_id: &str,
        _turn_group: &str,
        _active_version: i32,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    /// 获取会话中所有轮次的活跃版本号。
    ///
    /// 默认实现返回空列表；生产实现覆盖为 SQL SELECT。
    async fn get_turn_active_versions(
        &self,
        _conversation_id: &str,
    ) -> anyhow::Result<Vec<TurnActiveVersion>> {
        Ok(Vec::new())
    }

    /// 首次编辑升级：把「原始无 turn_group 的消息行」及其紧随其后的 assistant 行
    /// 原地标记为指定 turn_group 的 version=1（REQ-QA 首次编辑分页）。
    ///
    /// 默认实现为空操作（内存存储无需落库）；SqliteStorage 覆盖为真实 UPDATE。
    /// `original_message_id` 为原始 user 消息行 id（前端从 get_messages 返回的 `ChatMessage.id` 取得）。
    async fn promote_original_turn(
        &self,
        _conversation_id: &str,
        _original_message_id: &str,
        _turn_group: &str,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    /// 列出全部文档（按创建时间倒序，REQ-UI-006）。
    async fn list_documents(&self) -> anyhow::Result<Vec<Document>>;

    /// 列出全部文档（分页，按创建时间倒序）。
    ///
    /// 默认实现委托 `list_documents` 后截取；生产实现应覆盖为 SQL LIMIT/OFFSET。
    async fn list_documents_paginated(
        &self,
        limit: usize,
        offset: usize,
    ) -> anyhow::Result<Vec<Document>> {
        let all = self.list_documents().await?;
        Ok(all.into_iter().skip(offset).take(limit).collect())
    }

    /// 删除文档（外键级联清理 chunks 与 embeddings，REQ-ING-005）。
    async fn delete_document(&self, doc_id: &str) -> anyhow::Result<()>;

    /// 列出文档全部分块（按 sequence 正序，嵌入写入链路用，REQ-VEC-003）。
    async fn list_chunks(&self, doc_id: &str) -> anyhow::Result<Vec<Chunk>>;

    /// 获取文档内容预览（REQ-ING-010）。
    ///
    /// 返回文档元数据 + 前 500 字内容预览 + chunk 列表（每个含 sequence + 前 200 字）。
    /// 默认实现通过 `list_chunks` 拼接内容预览；生产实现可覆盖为单次 SQL 查询。
    async fn get_document_preview(&self, doc_id: &str) -> anyhow::Result<Option<DocumentPreview>> {
        // 查找文档
        let docs = self.list_documents().await?;
        let doc = docs.into_iter().find(|d| d.id == doc_id);
        let Some(doc) = doc else { return Ok(None) };

        // 获取 chunks
        let chunks = self.list_chunks(doc_id).await?;
        let chunk_count = chunks.len();

        // 拼接前 500 字内容预览
        let mut content_preview = String::new();
        for chunk in &chunks {
            if content_preview.len() >= 500 {
                break;
            }
            if !content_preview.is_empty() {
                content_preview.push(' ');
            }
            content_preview.push_str(&chunk.content);
        }
        // 截断到 500 字（按字符边界）
        let preview_chars: Vec<char> = content_preview.chars().collect();
        let limit = 500.min(preview_chars.len());
        content_preview = preview_chars[..limit].iter().collect();
        if chunk_count > 0 && content_preview.len() >= 500 {
            content_preview.push('…');
        }

        // 构建 chunk 预览列表（每个前 200 字）
        let chunk_previews: Vec<ChunkPreview> = chunks
            .iter()
            .map(|chunk| {
                let cp_chars: Vec<char> = chunk.content.chars().collect();
                let cp_limit = 200.min(cp_chars.len());
                let mut cp: String = cp_chars[..cp_limit].iter().collect();
                if cp_chars.len() > 200 {
                    cp.push('…');
                }
                ChunkPreview {
                    id: chunk.id.clone(),
                    sequence: chunk.sequence,
                    content_preview: cp,
                    token_count: chunk.token_count,
                }
            })
            .collect();

        Ok(Some(DocumentPreview {
            id: doc.id,
            file_path: doc.file_path,
            status: doc.status,
            created_at: doc.created_at,
            domain: doc.domain,
            summary: doc.summary,
            tags: doc.tags,
            file_hash: doc.file_hash,
            content_preview,
            chunks: chunk_previews,
            chunk_count,
        }))
    }

    /// 删除指定文档的全部分块与向量（保留文档记录，REQ-VEC-005 重试索引用）。
    /// 外键级联自动清理 embeddings；文档记录本身不受影响。
    async fn delete_chunks_by_doc(&self, doc_id: &str) -> anyhow::Result<()>;

    /// 以关键词执行全文检索（BM25 排序），返回 top-k 结果。
    ///
    /// 用于混合检索（Hybrid Retrieval）：向量检索 + 关键词检索 → RRF 融合。
    /// 弥补纯向量检索在精确匹配（代码片段、API 名称、专有名词）上的不足。
    /// 空查询或无匹配时返回空 Vec，不返回 Err。
    async fn keyword_search(
        &self,
        query: &str,
        top_k: usize,
    ) -> anyhow::Result<Vec<RetrievalResult>>;

    /// 对话全文搜索（REQ-RAG-040）。
    ///
    /// 使用 FTS5 全文索引搜索所有会话中的消息内容。
    /// 返回按 BM25 分数降序排列的搜索结果列表。
    /// 空查询返回空列表，不返回 Err。
    ///
    /// 默认实现返回空列表（不支持全文搜索的适配器）。
    async fn search_messages(
        &self,
        _query: &str,
        _limit: usize,
    ) -> anyhow::Result<Vec<MessageSearchResult>> {
        Ok(Vec::new())
    }

    /// 按内容指纹查找缓存的嵌入向量（全尺度性能优化）。
    ///
    /// 默认实现返回 `None`（缓存未命中）；生产适配器应覆盖以实现实际缓存查询。
    async fn lookup_embedding_cache(
        &self,
        _content_hash: &str,
    ) -> anyhow::Result<Option<Vec<f32>>> {
        Ok(None)
    }

    /// 批量查找缓存的嵌入向量（性能优化：单次 DB 查询替代 N 次串行查询）。
    ///
    /// 输入 `(content_hash,)` 列表，返回 `(batch_index, embedding)` 列表（仅命中项）。
    /// 默认实现逐个调用 `lookup_embedding_cache`（N 次 spawn_blocking）；
    /// 生产适配器应覆盖为单次 SQL `WHERE hash IN (...)` 查询（1 次 spawn_blocking）。
    async fn lookup_embedding_cache_batch(
        &self,
        hashes: &[String],
    ) -> anyhow::Result<Vec<(usize, Vec<f32>)>> {
        let mut hits = Vec::new();
        for (i, hash) in hashes.iter().enumerate() {
            if let Some(vec) = self.lookup_embedding_cache(hash).await? {
                hits.push((i, vec));
            }
        }
        Ok(hits)
    }

    /// 将嵌入向量写入缓存（全尺度性能优化）。
    ///
    /// 默认实现为空操作；生产适配器应覆盖以实现实际缓存写入。
    async fn put_embedding_cache(
        &self,
        _content_hash: &str,
        _embedding: &[f32],
    ) -> anyhow::Result<()> {
        Ok(())
    }

    /// 批量写入嵌入向量缓存（性能优化：单事务提交全部缓存项）。
    ///
    /// 默认实现逐个调用 `put_embedding_cache`（N 次 spawn_blocking）；
    /// 生产适配器应覆盖为单事务批量 INSERT（1 次 spawn_blocking）。
    async fn put_embedding_cache_batch(&self, items: &[(String, Vec<f32>)]) -> anyhow::Result<()> {
        for (hash, embedding) in items {
            self.put_embedding_cache(hash, embedding).await?;
        }
        Ok(())
    }

    /// 按源文件路径查找文档（REQ-SYNC-002 增量同步用）。
    ///
    /// 监听文件夹导入的文档 `original_path` 非空，
    /// 此方法按源文件的 canonical 路径精确查找已导入的文档。
    ///
    /// 默认实现遍历 `list_documents` 后过滤；生产适配器应覆盖为 SQL 查询。
    async fn find_document_by_original_path(&self, path: &str) -> anyhow::Result<Option<Document>> {
        let all = self.list_documents().await?;
        Ok(all
            .into_iter()
            .find(|d| d.original_path.as_deref() == Some(path)))
    }

    /// 按源文件路径前缀查找文档（REQ-SYNC-002 增量同步用）。
    ///
    /// 用于同步文件夹时找出所有 `original_path` 以 `prefix` 开头的文档，
    /// 以检测哪些文件已被删除（在文件夹中不存在但在 DB 中存在）。
    ///
    /// 默认实现遍历 `list_documents` 后过滤；生产适配器应覆盖为 SQL LIKE 查询。
    async fn find_documents_by_original_path_prefix(
        &self,
        prefix: &str,
    ) -> anyhow::Result<Vec<Document>> {
        let all = self.list_documents().await?;
        Ok(all
            .into_iter()
            .filter(|d| {
                d.original_path
                    .as_deref()
                    .is_some_and(|p| p.starts_with(prefix))
            })
            .collect())
    }

    /// 更新文档领域分类标签（REQ-VEC-013 领域画像）。
    ///
    /// 由 `EmbeddingDomainClassifier` 在文档向量化完成后调用，
    /// 将分类结果持久化到 `documents.domain` 列。
    ///
    /// 默认实现为空操作（内存 Mock Storage 无需持久化）；
    /// 生产适配器应覆盖为 SQL UPDATE。
    async fn update_document_domain(&self, _doc_id: &str, _domain: &str) -> anyhow::Result<()> {
        Ok(())
    }

    /// 更新文档摘要（REQ-ING-019 文档摘要自动生成）。
    ///
    /// 将 LLM 生成的摘要持久化到 `documents.summary` 列。
    /// 摘要在导入完成后异步生成，失败时保持 None（优雅降级）。
    ///
    /// 默认实现为空操作（内存 Mock Storage 无需持久化）；
    /// 生产适配器应覆盖为 SQL UPDATE。
    async fn update_document_summary(&self, _doc_id: &str, _summary: &str) -> anyhow::Result<()> {
        Ok(())
    }

    // ------------------------------------------------------------------
    // 文档标签系统（REQ-ING-022 用户自定义标签管理）
    // ------------------------------------------------------------------

    /// 添加文档标签（REQ-ING-022）。
    ///
    /// 将 `tag` 追加到指定文档的 `tags` 列（JSON 数组），若标签已存在则幂等跳过。
    ///
    /// 默认实现为空操作；生产适配器应覆盖为 SQL UPDATE。
    async fn add_document_tag(&self, _doc_id: &str, _tag: &str) -> anyhow::Result<()> {
        Ok(())
    }

    /// 移除文档标签（REQ-ING-022）。
    ///
    /// 从指定文档的 `tags` 列（JSON 数组）中移除 `tag`，若标签不存在则幂等跳过。
    ///
    /// 默认实现为空操作；生产适配器应覆盖为 SQL UPDATE。
    async fn remove_document_tag(&self, _doc_id: &str, _tag: &str) -> anyhow::Result<()> {
        Ok(())
    }

    /// 列出所有文档标签（REQ-ING-022）。
    ///
    /// 扫描全部文档的 `tags` 列，返回去重后的标签列表（含每个标签的文档计数）。
    /// 用于前端标签筛选侧栏渲染。
    ///
    /// 默认实现返回空 Vec；生产适配器应覆盖为 SQL 全表扫描 + JSON 解析。
    async fn list_all_tags(&self) -> anyhow::Result<Vec<(String, usize)>> {
        Ok(Vec::new())
    }

    /// 按标签筛选文档（REQ-ING-022）。
    ///
    /// 返回 `tags` 列（JSON 数组）中包含 `tag` 的所有文档。
    /// 用于前端标签筛选时缩小文档列表范围。
    ///
    /// 默认实现返回空 Vec；生产适配器应覆盖为 SQL `WHERE tags LIKE` 查询。
    async fn filter_documents_by_tag(&self, _tag: &str) -> anyhow::Result<Vec<Document>> {
        Ok(Vec::new())
    }

    /// 批量写入实体索引（REQ-PERF-006 实体链接增强）。
    ///
    /// 导入文档时抽取实体并批量写入 `entities` 表，供三路 RRF 实体检索通道使用。
    ///
    /// # 参数
    /// - `entities`: `(entity_text, entity_type, chunk_id)` 三元组列表
    ///
    /// 默认实现为空操作；生产适配器应覆盖为批量 SQL INSERT。
    async fn add_entities(&self, _entities: &[(String, String, String)]) -> anyhow::Result<()> {
        Ok(())
    }

    /// 实体检索（REQ-PERF-006 实体链接增强）。
    ///
    /// 从查询中抽取实体后，在 `entities` 表中精确匹配，
    /// 返回包含匹配实体的 chunk 列表（按命中实体数降序排列）。
    ///
    /// # 参数
    /// - `query_entities`: 从查询中抽取的实体文本列表
    /// - `top_k`: 返回结果数量上限
    ///
    /// 默认实现返回空 Vec；生产适配器应覆盖为 SQL 查询。
    async fn entity_search(
        &self,
        _query_entities: &[String],
        _top_k: usize,
    ) -> anyhow::Result<Vec<RetrievalResult>> {
        Ok(vec![])
    }

    /// 重建 BM25 全文索引（REQ-PERF-005 Contextual BM25）。
    ///
    /// 清空并重建 FTS5 索引，使用 `build_contextual_text()` 拼接文档名前缀，
    /// 提升精确匹配查询命中率（Anthropic Contextual Retrieval：失败率 ↓49%）。
    ///
    /// 默认实现为空操作；生产适配器应覆盖为 DROP + INSERT 重建。
    async fn rebuild_bm25_index(&self) -> anyhow::Result<()> {
        Ok(())
    }

    // ------------------------------------------------------------------
    // Proposition 级原子分割（REQ-PERF-007）
    // ------------------------------------------------------------------

    /// 批量写入 proposition 索引（REQ-PERF-007）。
    ///
    /// 导入文档时将 chunk 分割为 proposition 并批量写入 `propositions` 表。
    /// proposition 的嵌入向量在导入后由嵌入管线单独计算写入。
    ///
    /// # 参数
    /// - `propositions`: Proposition 列表（id / chunk_id / content / sequence）
    ///
    /// 默认实现为空操作；生产适配器应覆盖为批量 SQL INSERT。
    async fn add_propositions(&self, _propositions: &[Proposition]) -> anyhow::Result<()> {
        Ok(())
    }

    /// 批量写入 proposition 嵌入向量（REQ-PERF-007）。
    ///
    /// 嵌入管线计算完 proposition 嵌入后调用此方法写入。
    ///
    /// # 参数
    /// - `embeddings`: (proposition_id, embedding) 对列表
    ///
    /// 默认实现为空操作；生产适配器应覆盖为批量 SQL UPDATE。
    async fn add_proposition_embeddings(
        &self,
        _embeddings: &[(String, Vec<f32>)],
    ) -> anyhow::Result<()> {
        Ok(())
    }

    /// 列出文档的所有 proposition（REQ-PERF-007）。
    ///
    /// 返回指定文档全部分块对应的所有 proposition，
    /// 按 (chunk_sequence, proposition_sequence) 排序。
    /// 用于嵌入管线批量计算 proposition 嵌入。
    ///
    /// 默认实现返回空 Vec；生产适配器应覆盖为 SQL 查询。
    async fn list_propositions_by_doc(&self, _doc_id: &str) -> anyhow::Result<Vec<Proposition>> {
        Ok(vec![])
    }

    /// Proposition 向量检索（REQ-PERF-007）。
    ///
    /// 以查询向量在 proposition 嵌入表中执行 top-k 余弦相似度检索，
    /// 返回命中的 proposition 对应的 chunk（已去重，每个 chunk 只保留最高分）。
    ///
    /// # 参数
    /// - `query_embedding`: 查询的嵌入向量
    /// - `top_k`: 返回结果数量上限
    ///
    /// 默认实现返回空 Vec；生产适配器应覆盖为 SQL + 向量检索。
    async fn proposition_search(
        &self,
        _query_embedding: &[f32],
        _top_k: usize,
    ) -> anyhow::Result<Vec<RetrievalResult>> {
        Ok(vec![])
    }

    /// 重建 proposition 索引（REQ-PERF-007）。
    ///
    /// 清空 propositions 表后，遍历所有 chunk → 分割为 proposition → 重新写入。
    /// proposition 嵌入需要由嵌入管线单独重建。
    ///
    /// 默认实现为空操作；生产适配器应覆盖为实际重建逻辑。
    async fn rebuild_proposition_index(&self) -> anyhow::Result<()> {
        Ok(())
    }

    // ------------------------------------------------------------------
    // RAPTOR 摘要树（REQ-PERF-009）
    // ------------------------------------------------------------------

    /// 批量写入摘要树节点（REQ-PERF-009）。
    ///
    /// 将 RAPTOR 摘要树构建结果写入 `summary_nodes` 表。
    /// 节点的 embedding 字段在嵌入管线计算后单独更新。
    ///
    /// 默认实现为空操作；生产适配器应覆盖为批量 SQL INSERT。
    async fn add_summary_nodes(&self, _nodes: &[SummaryNode]) -> anyhow::Result<()> {
        Ok(())
    }

    /// 更新摘要节点嵌入向量（REQ-PERF-009）。
    ///
    /// 嵌入管线计算完摘要节点嵌入后调用此方法。
    ///
    /// 默认实现为空操作；生产适配器应覆盖为 SQL UPDATE。
    async fn update_summary_embedding(
        &self,
        _node_id: &str,
        _embedding: &[f32],
    ) -> anyhow::Result<()> {
        Ok(())
    }

    /// 列出文档的所有摘要节点（REQ-PERF-009）。
    ///
    /// 返回指定文档的全部摘要节点（所有层级），按 level 正序排列。
    ///
    /// 默认实现返回空 Vec；生产适配器应覆盖为 SQL 查询。
    async fn list_summary_nodes(&self, _doc_id: &str) -> anyhow::Result<Vec<SummaryNode>> {
        Ok(vec![])
    }

    /// 摘要树向量检索（REQ-PERF-009）。
    ///
    /// 以查询向量在摘要节点嵌入中执行 top-k 余弦相似度检索。
    /// 命中摘要节点后，可通过 `child_ids` 向下展开到具体 chunks。
    ///
    /// 默认实现返回空 Vec；生产适配器应覆盖为 SQL + 向量检索。
    async fn summary_search(
        &self,
        _query_embedding: &[f32],
        _top_k: usize,
    ) -> anyhow::Result<Vec<RetrievalResult>> {
        Ok(vec![])
    }

    /// 重建摘要树索引（REQ-PERF-009）。
    ///
    /// 清空 summary_nodes 表，供用户在需要时重建。
    ///
    /// 默认实现为空操作；生产适配器应覆盖为 DELETE 重建。
    async fn rebuild_summary_tree(&self) -> anyhow::Result<()> {
        Ok(())
    }

    // ------------------------------------------------------------------
    // 代码符号索引（REQ-RAG-031 代码感知 RAG）
    // ------------------------------------------------------------------

    /// 批量写入代码符号索引（REQ-RAG-031）。
    ///
    /// 导入代码文件时通过 tree-sitter AST 抽取符号并批量写入 `code_symbols` 表。
    /// 符号索引使代码查询能精确定位到函数/类定义，而非模糊匹配整个文件。
    ///
    /// 默认实现为空操作；生产适配器应覆盖为批量 SQL INSERT。
    async fn add_symbols(&self, _symbols: &[CodeSymbol]) -> anyhow::Result<()> {
        Ok(())
    }

    /// 按符号名精确搜索（REQ-RAG-031）。
    ///
    /// 在 `code_symbols` 表中按 `name` 精确匹配，可选按 `kind` 过滤。
    /// 用于查询中包含函数/类名时优先精确匹配符号。
    ///
    /// 默认实现返回空 Vec；生产适配器应覆盖为 SQL `WHERE name = ?` 查询。
    async fn search_by_symbol(
        &self,
        _name: &str,
        _kind: Option<&SymbolKind>,
    ) -> anyhow::Result<Vec<CodeSymbol>> {
        Ok(Vec::new())
    }

    /// 获取指定 chunk 的所有符号（REQ-RAG-031）。
    ///
    /// 返回 `code_symbols` 表中 `chunk_id` 等于指定值的所有符号，按 `start_line` 正序。
    ///
    /// 默认实现返回空 Vec；生产适配器应覆盖为 SQL `WHERE chunk_id = ?` 查询。
    async fn get_symbols_for_chunk(&self, _chunk_id: &str) -> anyhow::Result<Vec<CodeSymbol>> {
        Ok(Vec::new())
    }

    /// 模糊搜索符号（REQ-RAG-031）。
    ///
    /// 在 `code_symbols` 表中按 `name LIKE '%query%'` 模糊匹配，返回前 `limit` 条。
    /// 用于查询中包含部分函数名时进行模糊匹配。
    ///
    /// 默认实现返回空 Vec；生产适配器应覆盖为 SQL `WHERE name LIKE ?` 查询。
    async fn search_symbols_fuzzy(
        &self,
        _query: &str,
        _limit: usize,
    ) -> anyhow::Result<Vec<CodeSymbol>> {
        Ok(Vec::new())
    }

    // ------------------------------------------------------------------
    // 实体关系图谱（REQ-RAG-026 知识图谱实体关系检索）
    // ------------------------------------------------------------------

    /// 写入单条实体关系（知识图谱边，REQ-RAG-026）。
    ///
    /// 默认实现为空操作；生产适配器应覆盖为 SQL INSERT。
    async fn add_relation(&self, _relation: &EntityRelation) -> anyhow::Result<()> {
        Ok(())
    }

    /// 批量写入实体关系（REQ-RAG-026）。
    ///
    /// 导入文档时抽取实体间关系并批量写入 `entity_relations` 表。
    ///
    /// # 参数
    /// - `relations`: EntityRelation 列表
    ///
    /// 默认实现为空操作；生产适配器应覆盖为批量 SQL INSERT。
    async fn add_relations_batch(&self, _relations: &[EntityRelation]) -> anyhow::Result<()> {
        Ok(())
    }

    /// 查询指定实体参与的所有关系（REQ-RAG-026 图遍历）。
    ///
    /// 返回 subject 或 object 等于 `entity_text` 的所有关系。
    /// 用于检索时沿图边扩展到关联 chunk。
    ///
    /// 默认实现返回空 Vec；生产适配器应覆盖为 SQL 查询。
    async fn get_relations_for_entity(
        &self,
        _entity_text: &str,
    ) -> anyhow::Result<Vec<EntityRelation>> {
        Ok(vec![])
    }

    /// 查询指定 chunk 的所有关系（REQ-RAG-026）。
    ///
    /// 返回 chunk_id 等于指定值的所有关系。
    /// 用于查看某个 chunk 中包含的实体关系。
    ///
    /// 默认实现返回空 Vec；生产适配器应覆盖为 SQL 查询。
    async fn get_relations_for_chunk(
        &self,
        _chunk_id: &str,
    ) -> anyhow::Result<Vec<EntityRelation>> {
        Ok(vec![])
    }

    /// 按主体 + 关系类型查询关系（REQ-RAG-026）。
    ///
    /// 返回 subject 等于 `subject` 且 relation_type 等于 `relation_type` 的所有关系。
    /// 用于精确查找特定实体间的特定关系。
    ///
    /// 默认实现返回空 Vec；生产适配器应覆盖为 SQL 查询。
    async fn search_by_relation(
        &self,
        _subject: &str,
        _relation_type: &str,
    ) -> anyhow::Result<Vec<EntityRelation>> {
        Ok(vec![])
    }

    /// 按 chunk ID 查找单个 chunk（REQ-RAG-027 图遍历检索用）。
    ///
    /// 图遍历检索器在沿关系边扩展后，需要通过 chunk_id 查找 chunk 内容
    /// 以构建 `RetrievalResult`。此方法避免遍历所有文档的 chunk 列表。
    ///
    /// 默认实现返回 `None`（内存 Mock Storage 无需实现）；
    /// 生产适配器应覆盖为 SQL `WHERE id = ?` 查询。
    async fn get_chunk_by_id(&self, _chunk_id: &str) -> anyhow::Result<Option<Chunk>> {
        Ok(None)
    }

    /// 分页查询全部实体关系（REQ-RAG-027 前端图谱可视化用）。
    ///
    /// 返回 `entity_relations` 表中前 `limit` 条记录（跳过 `offset` 条），
    /// 用于前端知识图谱面板渲染节点和边。
    ///
    /// 默认实现返回空 Vec；生产适配器应覆盖为 SQL `LIMIT ? OFFSET ?` 查询。
    async fn list_all_relations(
        &self,
        _limit: usize,
        _offset: usize,
    ) -> anyhow::Result<Vec<EntityRelation>> {
        Ok(vec![])
    }

    /// 统计实体关系总数（REQ-RAG-027 前端图谱可视化用）。
    ///
    /// 返回 `entity_relations` 表中的记录数，用于前端显示「共 N 条关系」。
    ///
    /// 默认实现返回 0；生产适配器应覆盖为 SQL `COUNT(*)` 查询。
    async fn count_relations(&self) -> anyhow::Result<usize> {
        Ok(0)
    }

    /// 批量查询实体类型（REQ-RAG-027 前端图谱可视化增强）。
    ///
    /// 从 `entities` 表批量查询指定实体文本列表的 `entity_type`，
    /// 返回 `HashMap<entity_text, entity_type>` 映射。
    /// 用于前端图谱面板为每个节点渲染对应的实体类型图标。
    ///
    /// # 参数
    /// - `entities`: 实体文本列表（通常是图谱中所有节点的 id）
    ///
    /// # 返回
    /// `HashMap<String, String>` — 实体文本 → 类型映射。
    /// 未在 `entities` 表中的实体不出现在映射中。
    ///
    /// 默认实现返回空 HashMap；生产适配器应覆盖为 SQL `WHERE entity_text IN (...)` 批量查询。
    async fn get_entity_types(
        &self,
        _entities: &[String],
    ) -> anyhow::Result<std::collections::HashMap<String, String>> {
        Ok(std::collections::HashMap::new())
    }

    /// 获取全量实体邻接表（REQ-RAG-027 Session 5 高级分析用）。
    ///
    /// 一次查询返回全量邻接表 `HashMap<entity, Vec<neighbor>>`，
    /// 供 GraphAnalyzer 在后端内存中完成路径分析、社区检测等高级分析，
    /// 避免多次 IPC 往返。
    ///
    /// # 返回
    /// `HashMap<String, Vec<String>>` — 实体文本 → 邻居实体列表。
    /// 空图返回空 HashMap。
    ///
    /// 默认实现返回空 HashMap；生产适配器应覆盖为 SQL 全量查询。
    async fn get_entity_graph(
        &self,
    ) -> anyhow::Result<std::collections::HashMap<String, Vec<String>>> {
        Ok(std::collections::HashMap::new())
    }

    // ------------------------------------------------------------------
    // 对话记忆系统（REQ-RAG-032 持久化记忆系统增强）
    // ------------------------------------------------------------------

    /// 写入对话记忆条目（REQ-RAG-032）。
    ///
    /// 默认实现为空操作；生产适配器应覆盖为 SQL INSERT OR REPLACE。
    fn add_memory_entry(
        &self,
        _entry: &MemoryEntry,
    ) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
        async { Ok(()) }
    }

    /// 查询对话记忆条目（REQ-RAG-032）。
    ///
    /// `tier` 为 `None` 时返回所有层级的记忆。
    /// 默认实现返回空 Vec；生产适配器应覆盖为 SQL SELECT。
    fn get_memory_entries(
        &self,
        _tier: Option<&MemoryTier>,
    ) -> impl std::future::Future<Output = anyhow::Result<Vec<MemoryEntry>>> + Send {
        async { Ok(Vec::new()) }
    }

    /// 更新对话记忆条目（REQ-RAG-032）。
    ///
    /// 更新 tier / access_count / last_accessed / importance。
    /// 默认实现为空操作；生产适配器应覆盖为 SQL UPDATE。
    fn update_memory_entry(
        &self,
        _entry: &MemoryEntry,
    ) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
        async { Ok(()) }
    }

    /// 删除对话记忆条目（REQ-RAG-032）。
    ///
    /// 默认实现为空操作；生产适配器应覆盖为 SQL DELETE。
    fn delete_memory_entry(
        &self,
        _id: &str,
    ) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
        async { Ok(()) }
    }

    /// 清空对话记忆条目（REQ-RAG-032）。
    ///
    /// `tier` 为 `None` 时清空所有层级。返回删除的行数。
    /// 默认实现返回 0；生产适配器应覆盖为 SQL DELETE。
    fn clear_memory_entries(
        &self,
        _tier: Option<&MemoryTier>,
    ) -> impl std::future::Future<Output = anyhow::Result<usize>> + Send {
        async { Ok(0) }
    }

    /// 关键词模糊搜索对话记忆（REQ-RAG-032）。
    ///
    /// `content LIKE '%query%'` 模糊匹配，按 importance DESC 排序，返回 top-limit 条。
    /// 默认实现返回空 Vec；生产适配器应覆盖为 SQL LIKE 查询。
    fn search_memory_entries(
        &self,
        _query: &str,
        _limit: usize,
    ) -> impl std::future::Future<Output = anyhow::Result<Vec<MemoryEntry>>> + Send {
        async { Ok(Vec::new()) }
    }

    // ------------------------------------------------------------------
    // Wiki 双向链接（REQ-ING-020 Markdown 笔记双向链接）
    // ------------------------------------------------------------------

    /// 批量写入 wiki-link 索引（REQ-ING-020）。
    ///
    /// 导入 Markdown 文档时解析 `[[wiki-link]]` 语法并批量写入 `wiki_links` 表。
    /// wiki-link 表示文档间的引用关系，支持正向链接和反向链接查询。
    ///
    /// # 参数
    /// - `links`: WikiLink 列表（source_doc_id / target / chunk_id）
    ///
    /// 默认实现为空操作；生产适配器应覆盖为批量 SQL INSERT。
    async fn add_wiki_links(&self, _links: &[WikiLink]) -> anyhow::Result<()> {
        Ok(())
    }

    /// 查询文档的正向链接（REQ-ING-020）。
    ///
    /// 返回 `wiki_links` 表中 `source_doc_id` 等于指定值的所有链接，
    /// 即该文档引用了哪些其他文档。
    ///
    /// 默认实现返回空 Vec；生产适配器应覆盖为 SQL `WHERE source_doc_id = ?` 查询。
    async fn get_forward_links(&self, _doc_id: &str) -> anyhow::Result<Vec<WikiLink>> {
        Ok(vec![])
    }

    /// 查询文档的反向链接（REQ-ING-020）。
    ///
    /// 返回 `wiki_links` 表中 `target` 模糊匹配指定文档文件名的所有链接，
    /// 即哪些文档引用了该文档。用于显示「反向链接」面板。
    ///
    /// 默认实现返回空 Vec；生产适配器应覆盖为 SQL `WHERE target LIKE ?` 查询。
    async fn get_backlinks(&self, _doc_name: &str) -> anyhow::Result<Vec<WikiLink>> {
        Ok(vec![])
    }

    // ------------------------------------------------------------------
    // Durable Prompt Admission（B05 持久化提示接纳）
    // ------------------------------------------------------------------

    /// 接纳用户输入（B05 Durable Prompt Admission）。
    ///
    /// 将用户消息持久化到 `pending_inputs` 表，但不加入消息历史。
    /// 在安全边界点（如流式生成完成后）通过 `promote_input` 提升为正式消息。
    ///
    /// # 参数
    /// - `conversation_id`: 所属会话 ID
    /// - `content`: 用户输入内容
    /// - `delivery`: 投递模式（`"steer"` 优先中断 / `"queue"` 排队等待）
    ///
    /// # 返回
    /// 新创建的接纳记录 ID。
    ///
    /// 默认实现返回空字符串（不支持持久化提示接纳的适配器）；
    /// 生产适配器应覆盖为 SQL INSERT。
    async fn admit_input(
        &self,
        _conversation_id: &str,
        _content: &str,
        _delivery: &str,
    ) -> anyhow::Result<String> {
        Ok(String::new())
    }

    /// 提升接纳记录为正式消息（B05 Durable Prompt Admission）。
    ///
    /// 将指定接纳记录标记为已提升（设置 `promoted_seq`），
    /// 使其成为消息历史的一部分。提升后该记录不再出现在待处理列表中。
    ///
    /// # 参数
    /// - `input_id`: 接纳记录 ID
    ///
    /// 默认实现为空操作；生产适配器应覆盖为 SQL UPDATE。
    async fn promote_input(&self, _input_id: &str) -> anyhow::Result<()> {
        Ok(())
    }

    /// 获取会话的待处理输入列表（B05 Durable Prompt Admission）。
    ///
    /// 返回指定会话中所有未提升的接纳记录，按优先级排序：
    /// `steer` 模式排在前面，然后按创建时间 FIFO 排序。
    ///
    /// # 参数
    /// - `conversation_id`: 所属会话 ID
    ///
    /// 默认实现返回空 Vec；生产适配器应覆盖为 SQL SELECT。
    async fn get_pending_inputs(
        &self,
        _conversation_id: &str,
    ) -> anyhow::Result<Vec<PendingInput>> {
        Ok(vec![])
    }

    // ------------------------------------------------------------------
    // Scratch-Promote 记忆整合（Q01 借鉴 QM scratch-promote + consolidation）
    // ------------------------------------------------------------------

    /// 追加一条 scratch 日志条目（Q01）。
    ///
    /// 将临时事实写入 `scratch_logs` 表，等待 LLM 审查后 promote 到长期记忆。
    ///
    /// 默认实现为空操作；生产适配器应覆盖为 SQL INSERT。
    async fn add_scratch_log(&self, _entry: &ScratchLogEntry) -> anyhow::Result<()> {
        Ok(())
    }

    /// 获取 scratch 日志条目列表（Q01）。
    ///
    /// 返回按创建时间正序排列的 scratch 日志，可选限制数量。
    /// `limit = None` 表示返回全部。
    ///
    /// 默认实现返回空 Vec；生产适配器应覆盖为 SQL SELECT。
    async fn get_scratch_logs(
        &self,
        _limit: Option<usize>,
    ) -> anyhow::Result<Vec<ScratchLogEntry>> {
        Ok(vec![])
    }

    /// 删除指定的 scratch 日志条目（Q01）。
    ///
    /// 整合完成后清除已处理的 scratch 条目。
    ///
    /// 默认实现为空操作；生产适配器应覆盖为 SQL DELETE。
    async fn delete_scratch_log(&self, _id: &str) -> anyhow::Result<()> {
        Ok(())
    }

    /// 清理过期的 scratch 日志条目（Q01）。
    ///
    /// 删除创建时间早于 `before_timestamp` 的所有条目。
    /// 返回已清理的条目数。
    ///
    /// 默认实现返回 0；生产适配器应覆盖为 SQL DELETE WHERE。
    async fn cleanup_expired_scratch_logs(&self, _before_timestamp: i64) -> anyhow::Result<usize> {
        Ok(0)
    }

    // --- 幂等性存储支持 ---

    /// 记录幂等性操作（key + 时间戳）。
    ///
    /// 默认空操作；生产适配器应覆盖为 SQL INSERT。
    async fn record_idempotency(&self, _key: &str, _timestamp: i64) -> anyhow::Result<()> {
        Ok(())
    }

    /// 列出所有幂等性记录。
    ///
    /// 默认空操作；生产适配器应覆盖为 SQL SELECT。
    async fn list_idempotency_records(&self) -> anyhow::Result<Vec<(String, i64)>> {
        Ok(Vec::new())
    }

    /// 清理过期的幂等性记录。
    ///
    /// 默认空操作；生产适配器应覆盖为 SQL DELETE。
    async fn cleanup_expired_idempotency(&self, _before_timestamp: i64) -> anyhow::Result<usize> {
        Ok(0)
    }

    // --- Session Todo 持久化（REQ-RAG-044）---

    /// 创建 Todo 项，持久化到 `session_todos` 表。
    ///
    /// 默认空操作；生产适配器应覆盖为 SQL INSERT。
    async fn add_session_todo(&self, _todo: &SessionTodo) -> anyhow::Result<()> {
        Ok(())
    }

    /// 更新 Todo 状态（pending → in_progress → completed）。
    ///
    /// 默认空操作；生产适配器应覆盖为 SQL UPDATE。
    async fn update_todo_status(&self, _todo_id: &str, _status: &TodoStatus) -> anyhow::Result<()> {
        Ok(())
    }

    /// 获取会话的全部 Todo 列表，按 position 升序排列。
    ///
    /// 默认返回空 Vec；生产适配器应覆盖为 SQL SELECT。
    async fn get_session_todos(&self, _conversation_id: &str) -> anyhow::Result<Vec<SessionTodo>> {
        Ok(vec![])
    }

    /// 删除单个 Todo 项。
    ///
    /// 默认空操作；生产适配器应覆盖为 SQL DELETE。
    async fn delete_session_todo(&self, _todo_id: &str) -> anyhow::Result<()> {
        Ok(())
    }

    /// 删除会话的全部 Todo 项（用于清空/重置）。
    ///
    /// 默认空操作；生产适配器应覆盖为 SQL DELETE WHERE conversation_id = ?。
    async fn delete_session_todos(&self, _conversation_id: &str) -> anyhow::Result<()> {
        Ok(())
    }

    // --- Security-Tainted 条目标记（Q05 借鉴 QM securityTainted）---

    /// 标记指定消息条目为 security-tainted（安全污染）。
    ///
    /// 被标记的条目在上下文构建时默认过滤（不发送给 LLM），
    /// 除非调用方显式请求 `include_security_tainted = true`。
    ///
    /// 默认空操作；生产适配器应覆盖为 SQL UPDATE。
    async fn set_entry_security_tainted(
        &self,
        _message_id: &str,
        _tainted: bool,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    /// 查询指定消息条目的 security-tainted 标记。
    ///
    /// 返回 `true` 表示该条目被标记为安全污染。
    /// 默认实现返回 `false`（未标记）；生产适配器应覆盖为 SQL SELECT。
    async fn get_entry_security_tainted(&self, _message_id: &str) -> anyhow::Result<bool> {
        Ok(false)
    }

    // ------------------------------------------------------------------
    // 预算追踪（QM 借鉴）
    // ------------------------------------------------------------------

    /// 记录预算使用（用于 LLM API 费用追踪）。
    ///
    /// 在 chat_inner 调用 LLM 后记录 token 使用量和成本。
    /// 默认实现为空操作；生产适配器应覆盖为 SQL INSERT。
    async fn record_budget_usage(
        &self,
        _principal: &str,
        _input_tokens: usize,
        _output_tokens: usize,
        _cost_usd: f64,
        _model_name: &str,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    /// 查询指定主体的预算使用统计。
    ///
    /// 返回该主体在滑动窗口内的使用情况和成本统计。
    /// 默认实现返回空统计；生产适配器应覆盖为 SQL 聚合查询。
    async fn get_budget_stats(
        &self,
        _principal: &str,
    ) -> anyhow::Result<echomind_models::BudgetStats> {
        Ok(echomind_models::BudgetStats {
            daily_limit: 0.0,
            spent_today: 0.0,
            remaining: f64::INFINITY,
        })
    }

    /// 设置预算每日限制。
    ///
    /// 更新指定主体的每日预算上限（USD）。
    /// 默认实现为空操作；生产适配器应覆盖为 SQL UPDATE 或 INSERT OR REPLACE。
    async fn set_budget_limit(
        &self,
        _principal: &str,
        _daily_limit_usd: f64,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    // ------------------------------------------------------------------
    // 导入历史记录（REQ-ING-011）
    // ------------------------------------------------------------------

    /// 写入一条导入历史记录。
    ///
    /// 记录每次导入操作的时间戳、文件名、格式、结果（成功/失败/跳过）。
    /// 历史记录上限 100 条，超过自动淘汰最旧记录。
    async fn add_import_log(
        &self,
        _file_name: &str,
        _format: &str,
        _result: &str,
        _error_message: Option<&str>,
        _file_size: Option<i64>,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    /// 查询导入历史记录（最近 100 条，按时间倒序）。
    ///
    /// 可选按结果筛选（success / failed / skipped）。
    async fn get_import_logs(
        &self,
        _result_filter: Option<&str>,
    ) -> anyhow::Result<Vec<echomind_models::ImportLogEntry>> {
        Ok(Vec::new())
    }

    /// 清空导入历史记录。
    async fn clear_import_logs(&self) -> anyhow::Result<()> {
        Ok(())
    }

    // ------------------------------------------------------------------
    // 对话书签（REQ-RAG-047）
    // ------------------------------------------------------------------

    /// 添加对话书签（REQ-RAG-047 AC-1/AC-2）。
    ///
    /// 将指定会话标记为书签，可附带备注。重复添加时更新备注。
    /// 默认实现为空操作；生产适配器应覆盖为 SQL INSERT OR REPLACE。
    async fn add_bookmark(
        &self,
        _conversation_id: &str,
        _note: Option<&str>,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    /// 移除对话书签（REQ-RAG-047 AC-5）。
    ///
    /// 默认实现为空操作；生产适配器应覆盖为 SQL DELETE。
    async fn remove_bookmark(&self, _conversation_id: &str) -> anyhow::Result<()> {
        Ok(())
    }

    /// 列出全部书签（REQ-RAG-047 AC-3/AC-4）。
    ///
    /// 返回所有书签列表，按创建时间倒序排列。
    /// 默认实现返回空 Vec；生产适配器应覆盖为 SQL SELECT。
    async fn list_bookmarks(&self) -> anyhow::Result<Vec<echomind_models::ConversationBookmark>> {
        Ok(vec![])
    }

    /// 检查指定会话是否已加书签（REQ-RAG-047 AC-2 图标显示）。
    ///
    /// 默认实现返回 false；生产适配器应覆盖为 SQL SELECT。
    async fn is_bookmarked(&self, _conversation_id: &str) -> anyhow::Result<bool> {
        Ok(false)
    }
}
pub trait LLMProvider: Send + Sync {
    /// 发起流式对话；返回的流在首个 token 到达前即应建立，逐 token 产出增量文本。
    async fn chat_stream(
        &self,
        system_prompt: &str,
        history: &[ChatMessage],
        query: &str,
    ) -> anyhow::Result<BoxStream<'static, anyhow::Result<String>>>;

    /// 分段式流式对话（Prompt Caching 优化）。
    ///
    /// 将系统提示词拆分为**静态前缀**（跨请求不变，可被 API 端 prompt caching 命中）
    /// 和**动态上下文**（每次请求不同的检索片段），使 OpenAI / Anthropic 等 API
    /// 的 prompt caching 能缓存静态前缀部分，显著降低重复 token 计费与首 token 延迟。
    ///
    /// # 参数
    /// - `static_prefix`：静态前缀——角色描述 + 回答规则 + 引导指令，跨请求不变
    /// - `dynamic_context`：动态上下文——检索到的知识库片段，每次请求不同
    /// - `history`：对话历史
    /// - `query`：用户当前提问
    ///
    /// # 默认实现
    ///
    /// 将两段拼接为单个 `system_prompt` 后委托 `chat_stream`，不享受缓存优化。
    /// `OpenAIProvider` 覆盖此方法，将静态前缀和动态上下文分别作为两条独立的
    /// `system` 消息发送，使 API 端能命中前缀缓存。
    async fn chat_stream_segmented(
        &self,
        static_prefix: &str,
        dynamic_context: &str,
        history: &[ChatMessage],
        query: &str,
    ) -> anyhow::Result<BoxStream<'static, anyhow::Result<String>>> {
        let combined = format!("{static_prefix}\n\n{dynamic_context}");
        self.chat_stream(&combined, history, query).await
    }

    // -----------------------------------------------------------------------
    // Q10 辅助模型方法（借鉴 QM HarnessModelUtilities）
    //
    // 以下方法均有默认实现（返回 None），现有 LLMProvider 实现者无需修改。
    // 各后端可按需覆盖以提供优化实现。
    // -----------------------------------------------------------------------

    /// 生成对话标题（借鉴 QM generateTitle）。
    ///
    /// 输入对话转录文本（用户首条消息 + 助手回复摘要），返回简短标题。
    /// 默认返回 `Ok(None)`，由调用方降级为 `derive_title()` 字符串截取。
    ///
    /// # 参数
    /// - `transcript`：对话转录文本（通常是首条用户消息 + 助手回复的前若干句）
    ///
    /// # 返回
    /// - `Ok(Some(title))`：成功生成标题
    /// - `Ok(None)`：此 Provider 不支持标题生成，调用方应降级
    /// - `Err(...)`：网络/API 错误，调用方应降级
    async fn generate_title(&self, _transcript: &str) -> anyhow::Result<Option<String>> {
        Ok(None)
    }

    /// 单次推理（借鉴 QM oneShot）。
    ///
    /// 非流式单轮完成，用于记忆提取、安全筛查、摘要生成等辅助任务。
    /// 与 `chat_stream` 不同，此方法等待完整响应后返回，不逐 token 推送。
    ///
    /// # 参数
    /// - `system`：系统提示词
    /// - `prompt`：用户输入
    ///
    /// # 返回
    /// - `Ok(Some(text))`：成功生成完整响应
    /// - `Ok(None)`：此 Provider 不支持单次推理
    /// - `Err(...)`：网络/API 错误
    async fn one_shot(&self, _system: &str, _prompt: &str) -> anyhow::Result<Option<String>> {
        Ok(None)
    }

    /// 判断器（借鉴 QM judge）。
    ///
    /// 用于安全筛查、质量评估、内容审核等需要 LLM 做判断的场景。
    /// 返回判断结果文本（如 "yes"/"no"/"safe"/"unsafe"），由调用方解析。
    ///
    /// # 参数
    /// - `system`：系统提示词（如 "你是安全审查员"）
    /// - `prompt`：待判断的内容
    ///
    /// # 返回
    /// - `Ok(Some(verdict))`：成功返回裁决
    /// - `Ok(None)`：此 Provider 不支持判断
    /// - `Err(...)`：网络/API 错误
    async fn judge(&self, _system: &str, _prompt: &str) -> anyhow::Result<Option<String>> {
        Ok(None)
    }

    /// 上下文 token 预算（借鉴 QM contextTokenBudget）。
    ///
    /// 返回此模型的最大上下文长度（token 数）。用于压缩引擎计算可用预算。
    ///
    /// # 返回
    /// - `Some(n)`：已知模型上限
    /// - `None`：未知上限，调用方应使用保守默认值
    fn context_token_budget(&self) -> Option<usize> {
        None
    }
}

/// Prompt 压缩端口（REQ-PERF-002）：检索片段注入前做 token 级压缩，2-5x 减少注入 token。
///
/// ## 两层压缩策略
///
/// - **Free 版（规则压缩，零依赖）**：`RuleBasedCompressor` — 去除停用词/冗余空白/
///   代码注释/Markdown 装饰 + 句子级 word-overlap 评分保留 top-N
/// - **Pro 版（ONNX 模型压缩）**：`OnnxCompressor` — 使用已有 ONNX embedder 做句子级
///   嵌入相似度评分，精确度更高但需 embedder 初始化
///
/// ## 压缩比
///
/// - `2.0` = 保守（压缩到 1/2，信息保留率 ≥ 90%）
/// - `3.0` = 平衡（压缩到 1/3，信息保留率 ≥ 80%）
/// - `5.0` = 激进（压缩到 1/5，信息保留率 ≥ 60%）
/// - `1.0` = 禁用（原样返回）
///
/// ## 压缩范围
///
/// 仅压缩检索片段（`SegmentedPrompt.dynamic_context`），
/// 不压缩系统提示和用户查询。
///
/// ## 调研来源
///
/// - LLMLingua-2 (arXiv:2403.12968)：XLM-RoBERTa token 分类压缩，2-5x 压缩比
///
/// ## 对象安全设计
///
/// 与 `Reranker`/`QueryRewriter` 端口相同，使用手动 `Pin<Box<Future>>` 返回类型
/// 以保证对象安全（dyn-compatible）。`PromptCompressor` 以 `Option<Arc<dyn PromptCompressor>>`
/// 形式注入 `ChatEngine`，实现运行时开关——用户可通过 `compression_ratio` 设置
/// 在运行时启停压缩，无需重编译。
pub trait PromptCompressor: Send + Sync {
    /// 压缩文本，返回压缩后的文本。
    ///
    /// # 参数
    /// - `text`: 待压缩的文本（通常是检索到的 chunk 内容）
    /// - `ratio`: 目标压缩比（2.0 = 压缩到 1/2，5.0 = 压缩到 1/5，1.0 = 不压缩）
    /// - `query`: 用户查询文本（用于句子级相关性评分）
    ///
    /// # 返回
    /// 压缩后的文本。如果压缩不可用或失败，应返回原始文本（优雅降级），
    /// 而非返回 Err（Err 会中断整个对话管线）。
    fn compress<'a>(
        &'a self,
        text: &'a str,
        ratio: f32,
        query: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<String>> + Send + 'a>>;

    /// 批量压缩多个 chunk。
    ///
    /// 默认实现逐条调用 `compress`；适配器可覆盖为真正的批量处理。
    fn compress_batch<'a>(
        &'a self,
        chunks: &'a [String],
        ratio: f32,
        query: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<Vec<String>>> + Send + 'a>>
    {
        Box::pin(async move {
            let mut results = Vec::with_capacity(chunks.len());
            for chunk in chunks {
                results.push(self.compress(chunk, ratio, query).await?);
            }
            Ok(results)
        })
    }
}

/// 压缩禁用占位实现（REQ-PERF-002 AC-10）：用户未开启压缩时使用。
///
/// `compress` 原样返回文本，使管线天然跳过压缩阶段。
/// 设计为零大小类型 + `Send + Sync`，可安全跨线程共享。
#[derive(Debug, Clone, Copy, Default)]
pub struct NoCompressor;

impl PromptCompressor for NoCompressor {
    fn compress<'a>(
        &'a self,
        text: &'a str,
        _ratio: f32,
        _query: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<String>> + Send + 'a>>
    {
        Box::pin(async move { Ok(text.to_string()) })
    }
}

/// 响应缓存端口（REQ-PERF-001）：三级缓存金字塔，减少重复查询的 token 消耗。
///
/// ## 缓存层级
///
/// - **L0 精确匹配**：`query_hash → answer`，命中率 5-10%，0 token
/// - **L1 语义匹配**：`embedding similarity → answer`，命中率 10-20%，0 token
/// - **L3 检索结果**：`query_embedding → chunks`，跳过嵌入+检索计算
///
/// ## 实现方
///
/// `SqliteCache`（`crates/infra/src/sqlite_cache.rs`）— 使用 SQLite + 向量 BLOB 存储。
///
/// ## 调研来源
///
/// - Anthropic Prompt Caching (2024.08)：成本 ↓90%，延迟 ↓85%
/// - Mem0 (2026.04)：语义缓存 + 实体链接，LoCoMo 92.5
pub trait ResponseCache: Send + Sync {
    /// L0 精确匹配查询：以归一化查询的 SHA-256 哈希为键查找缓存答案。
    ///
    /// # 参数
    /// - `query_hash`：归一化查询的 SHA-256 十六进制哈希（由 `cache::query_hash()` 计算）
    /// - `ttl_secs`：缓存存活时间（秒），超过此时间的条目视为未命中
    /// - `now`：当前时间（Unix 秒级时间戳）
    ///
    /// # 返回
    /// `Some(CacheHit)` 表示命中（包含答案和引用来源 JSON），
    /// `None` 表示未命中或已过期。
    async fn lookup_exact(
        &self,
        query_hash: &str,
        ttl_secs: u64,
        now: i64,
    ) -> anyhow::Result<Option<echomind_models::CacheHit>>;

    /// L1 语义匹配查询：以查询嵌入向量查找余弦相似度 ≥ 阈值的缓存答案。
    ///
    /// # 参数
    /// - `query_embedding`：查询的嵌入向量
    /// - `threshold`：余弦相似度阈值（默认 0.92），低于此值不命中
    /// - `ttl_secs`：缓存存活时间（秒）
    /// - `now`：当前时间（Unix 秒级时间戳）
    ///
    /// # 返回
    /// `Some(CacheHit)` 表示命中，`None` 表示未命中。
    async fn lookup_semantic(
        &self,
        query_embedding: &[f32],
        threshold: f32,
        ttl_secs: u64,
        now: i64,
    ) -> anyhow::Result<Option<echomind_models::CacheHit>>;

    /// L3 检索结果缓存查询：以查询嵌入向量查找缓存的检索结果。
    ///
    /// # 参数
    /// - `query_embedding`：查询的嵌入向量
    /// - `threshold`：余弦相似度阈值（默认 0.90）
    /// - `ttl_secs`：缓存存活时间（秒）
    /// - `now`：当前时间（Unix 秒级时间戳）
    ///
    /// # 返回
    /// `Some(String)` 包含序列化的 `Vec<RetrievalResult>` JSON，`None` 表示未命中。
    async fn lookup_retrieval(
        &self,
        query_embedding: &[f32],
        threshold: f32,
        ttl_secs: u64,
        now: i64,
    ) -> anyhow::Result<Option<String>>;

    /// 写入 L0 精确匹配缓存条目。
    ///
    /// # 参数
    /// - `query_hash`：归一化查询的 SHA-256 哈希
    /// - `query_text`：原始查询文本（用于调试/展示）
    /// - `answer_text`：LLM 生成的答案
    /// - `sources_json`：引用来源的 JSON 序列化
    /// - `conversation_id`：关联的会话 ID（可选）
    async fn insert_exact(
        &self,
        query_hash: &str,
        query_text: &str,
        answer_text: &str,
        sources_json: &str,
        conversation_id: Option<&str>,
    ) -> anyhow::Result<()>;

    /// 写入 L1 语义匹配缓存条目。
    ///
    /// # 参数
    /// - `query_text`：原始查询文本
    /// - `query_embedding`：查询的嵌入向量
    /// - `answer_text`：LLM 生成的答案
    /// - `sources_json`：引用来源的 JSON 序列化
    /// - `conversation_id`：关联的会话 ID（可选）
    async fn insert_semantic(
        &self,
        query_text: &str,
        query_embedding: &[f32],
        answer_text: &str,
        sources_json: &str,
        conversation_id: Option<&str>,
    ) -> anyhow::Result<()>;

    /// 写入 L3 检索结果缓存条目。
    ///
    /// # 参数
    /// - `query_text`：原始查询文本
    /// - `query_embedding`：查询的嵌入向量
    /// - `results_json`：检索结果的 JSON 序列化
    async fn insert_retrieval(
        &self,
        query_text: &str,
        query_embedding: &[f32],
        results_json: &str,
    ) -> anyhow::Result<()>;

    /// 清空所有缓存（文档导入/删除时触发）。
    async fn clear_all(&self) -> anyhow::Result<()>;

    /// 获取缓存统计信息。
    async fn get_stats(&self) -> anyhow::Result<echomind_models::CacheStats>;
}

/// 检索端口（对应 REQ-RAG-003）：面向自然语言查询的 top-k 召回。
pub trait Retriever: Send + Sync {
    /// 对查询做向量化并检索最相关的 top_k 个分块。
    async fn retrieve(&self, query: &str, top_k: usize) -> anyhow::Result<Vec<RetrievalResult>>;

    /// 使用预计算的查询嵌入进行检索（性能优化：避免冗余 ONNX 推理）。
    ///
    /// 当调用方已计算查询嵌入（如语义缓存查找时）时，可直接传入复用，
    /// 省去重复 `embedder.embed(query)` 调用（~50-100ms ONNX 推理）。
    ///
    /// 默认实现忽略预计算嵌入，回退到 `retrieve()`。适配器应覆盖此方法以获得性能收益。
    async fn retrieve_with_embedding(
        &self,
        query: &str,
        query_embedding: &[f32],
        top_k: usize,
    ) -> anyhow::Result<Vec<RetrievalResult>> {
        let _ = query_embedding; // 默认实现不使用预计算嵌入
        self.retrieve(query, top_k).await
    }
}

/// Cross-Encoder 重排序端口（REQ-RAG-020）：对初检候选结果进行二次精排。
///
/// 初检阶段（向量检索 + 关键词 RRF 融合）快速召回 top-N 候选，
/// 重排序阶段使用 Cross-Encoder（如 bge-reranker-base）对 query-document 对
/// 逐对打分，按精确相关性重排，显著提升 top-k 精度。
///
/// # 对象安全设计
///
/// 与 `Retriever`/`Embedder` 等端口不同，`Reranker` 使用手动 `Pin<Box<Future>>`
/// 返回类型以保证对象安全（dyn-compatible）。这是因为 `Reranker` 需要以
/// `Option<Arc<dyn Reranker>>` 形式注入 `HybridRetriever`，实现运行时开关——
/// 用户可通过 `rag.rerank_enabled` 设置在运行时启停重排序，无需重编译。
///
/// # 调研依据
///
/// - Anthropic Contextual Retrieval 博文（2024）：重排序可将检索失败率降低 49%
/// - BGE-reranker-base：BAAI 开源 Cross-Encoder，支持中英文，ONNX 量化约 280MB
/// - fastembed `TextRerank`：纯 Rust ONNX 推理，无需 Python 依赖
pub trait Reranker: Send + Sync {
    /// 对候选结果进行 Cross-Encoder 精排，返回按相关性降序排列的结果。
    ///
    /// # 参数
    /// - `query`: 用户查询文本
    /// - `candidates`: 初检候选结果（通常 top-N，N > top_k）
    ///
    /// # 返回
    /// 重排序后的结果列表，按 Cross-Encoder 分数降序排列。
    /// 结果数量与 `candidates` 相同（不截断），由调用方按需取 top-k。
    fn rerank<'a>(
        &'a self,
        query: &'a str,
        candidates: &'a [RetrievalResult],
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = anyhow::Result<Vec<RetrievalResult>>> + Send + 'a>,
    >;
}

/// 重排序禁用占位实现（REQ-RAG-020 AC-3）：用户未开启重排序时使用。
///
/// `rerank` 原样返回候选列表（不重排、不截断），使管线天然跳过重排序阶段。
/// 设计为零大小类型 + `Send + Sync`，可安全跨线程共享。
#[derive(Debug, Clone, Copy, Default)]
pub struct NoReranker;

impl Reranker for NoReranker {
    fn rerank<'a>(
        &'a self,
        _query: &'a str,
        candidates: &'a [RetrievalResult],
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = anyhow::Result<Vec<RetrievalResult>>> + Send + 'a>,
    > {
        // 直接返回候选的克隆，不做任何重排
        Box::pin(async move { Ok(candidates.to_vec()) })
    }
}

/// 查询改写端口（REQ-RAG-021）：在检索前对用户查询进行改写，提升向量检索召回质量。
///
/// HyDE（Hypothetical Document Embeddings）是典型实现：让 LLM 生成假设性答案文档，
/// 用该文档的嵌入替代原始查询嵌入进行向量检索——因为答案文档比短查询更接近
/// 知识库中的实际答案片段（论文 arXiv:2212.10496）。
///
/// # 对象安全设计
///
/// 与 `Reranker` 端口相同，使用手动 `Pin<Box<Future>>` 返回类型以保证对象安全
/// （dyn-compatible）。`QueryRewriter` 以 `Option<Arc<dyn QueryRewriter>>` 形式
/// 注入 `HybridRetriever`，实现运行时开关——用户可通过 `rag.hyde_enabled` 设置
/// 在运行时启停查询改写，无需重编译。
///
/// # 改写范围
///
/// 改写后的文本**仅用于向量检索**（`embed()`），关键词检索（FTS5 BM25）仍使用
/// 原始查询——因为关键词检索擅长精确匹配，改写后的文本可能丢失原始查询中的
/// 精确术语（代码片段、API 名称、专有名词）。
///
/// # 降级策略
///
/// 如果改写失败（LLM 网络错误、超时等），实现方应返回原始查询而非 Err，
/// 使管线优雅降级为无改写模式。
pub trait QueryRewriter: Send + Sync {
    /// 对用户查询进行改写，返回改写后的文本。
    ///
    /// # 参数
    /// - `query`: 用户原始查询
    ///
    /// # 返回
    /// 改写后的文本。如果改写不可用或失败，应返回原始查询（优雅降级），
    /// 而非返回 Err（Err 会中断整个检索管线）。
    fn rewrite<'a>(
        &'a self,
        query: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<String>> + Send + 'a>>;
}

/// 查询改写禁用占位实现（REQ-RAG-021 AC-3）：用户未开启 HyDE 时使用。
///
/// `rewrite` 原样返回查询文本，使管线天然跳过改写阶段。
/// 设计为零大小类型 + `Send + Sync`，可安全跨线程共享。
#[derive(Debug, Clone, Copy, Default)]
pub struct NoRewriter;

impl QueryRewriter for NoRewriter {
    fn rewrite<'a>(
        &'a self,
        query: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<String>> + Send + 'a>>
    {
        // 原样返回查询，不做任何改写
        Box::pin(async move { Ok(query.to_string()) })
    }
}

/// PDF 页面渲染端口（REQ-MM-001）：将 PDF 页面渲染为位图，供 OCR 或 VLM 处理。
///
/// 渲染引擎优先选择 Rust 可用的 PDF 渲染库（pdfium 绑定），经架构评审确定（ADR-010）。
/// 渲染为 CPU 密集任务，实现方必须经 `spawn_blocking` 执行。
pub trait PageRenderer: Send + Sync {
    /// 将指定页面渲染为位图（PNG 字节），供 OCR 或 VLM 处理。
    ///
    /// # 参数
    /// - `pdf_path`: PDF 文件路径
    /// - `page_num`: 页码（从 0 开始）
    /// - `dpi`: 渲染分辨率（建议 150-200，平衡清晰度与内存）
    ///
    /// # 错误
    /// - 文件不存在 / 非 PDF 格式 / 页码越界 / 渲染引擎初始化失败
    async fn render_page(
        &self,
        pdf_path: &str,
        page_num: usize,
        dpi: u32,
    ) -> anyhow::Result<Vec<u8>>;
}

/// OCR 文字识别端口（REQ-MM-002）：对位图执行 OCR，提取文字内容。
///
/// OCR 引擎使用 ocrs（纯 Rust PaddleOCR PP-OCRv4 + rten 推理引擎，ADR-010 架构评审确定），
/// 完全本地运行，零网络请求（符合「隐私不出域」承诺）。
/// OCR 为 CPU 密集任务，实现方必须经 `spawn_blocking` 执行。
pub trait OcrEngine: Send + Sync {
    /// 对位图执行 OCR，返回识别出的文本。
    ///
    /// # 参数
    /// - `image_bytes`: 位图数据（PNG/JPEG 字节）
    ///
    /// # 返回
    /// 识别出的文本内容。如果 OCR 不可用或图片中无文字，返回空字符串。
    async fn recognize(&self, image_bytes: &[u8]) -> anyhow::Result<String>;
}

/// VLM 图片理解端口（REQ-MM-003）：将位图发送到视觉语言模型，获取结构化文本描述。
///
/// 与 OCR（提取印刷文字）互补：VLM 能理解表格→Markdown 表格、甘特图→Mermaid gantt、
/// 流程图→Mermaid flowchart 等结构化内容。OCR 对这类结构化内容识别效果有限。
///
/// # 隐私边界
///
/// 图片仅发送到用户自行配置的 LLM 端点（BYOK），符合「隐私不出域」——数据出口由用户控制。
/// 网络请求必须且仅发往用户配置的 `base_url`（ADR-010 安全官要求 AC-4）。
/// 用户可在设置中关闭 VLM 增强（默认关闭），此时管线仅使用 OCR。
///
/// # 适配器
///
/// - `OpenAIVisionProvider`（`crates/infra/src/openai_vision.rs`）— 复用 reqwest，
///   调用 OpenAI 兼容 Vision API（Chat Completions + image_url base64）
pub trait VisionLanguageModel: Send + Sync {
    /// 将图片发送到 VLM，获取结构化文本描述。
    ///
    /// # 参数
    /// - `image_bytes`: 位图数据（PNG/JPEG 字节，base64 编码后嵌入请求体）
    /// - `prompt`: 系统提示，引导 VLM 将图片中的结构化内容转换为目标格式
    ///   （表格→Markdown 表格、甘特图→Mermaid gantt、流程图→Mermaid flowchart）
    ///
    /// # 返回
    /// VLM 生成的结构化文本描述。如果 VLM 不可用或调用失败，返回空字符串（优雅降级）。
    async fn describe_image(&self, image_bytes: &[u8], prompt: &str) -> anyhow::Result<String>;
}

/// VLM 禁用占位实现（REQ-MM-003 AC-3）：用户未开启 VLM 增强时使用。
///
/// `describe_image` 始终返回空字符串，使管线天然跳过 VLM 阶段，仅使用 OCR。
/// 设计为零大小类型 + `Send + Sync`，可安全跨线程共享。
#[derive(Debug, Clone, Copy, Default)]
pub struct NoVlm;

impl VisionLanguageModel for NoVlm {
    /// 始终返回空字符串，表示 VLM 未启用。
    async fn describe_image(&self, _image_bytes: &[u8], _prompt: &str) -> anyhow::Result<String> {
        Ok(String::new())
    }
}

/// 多向量嵌入端口（REQ-PERF-008 ColBERT Late Interaction, Pro feature）。
///
/// 与 `Embedder`（单向量）不同，`MultiVectorEmbedder` 为文本中的每个 token
/// 生成独立的嵌入向量，而非整个文本一个向量。检索时使用 MaxSim 交互计算：
///
/// - 对 query 的每个 token 向量，在 document 的所有 token 向量中找最大余弦相似度
/// - 将所有 query token 的最大相似度求和，得到 query-document 相关度
///
/// ColBERT 论文（arXiv:2004.12832）的 Late Interaction 模型在精确匹配场景
/// 显著优于单向量检索（预期 +20-30% 命中率），但存储成本为 N tokens × dim/chunk。
///
/// # 与 Embedder trait 的关系
///
/// `MultiVectorEmbedder` 与 `Embedder` 共存：用户可通过 `set_embedder_model` IPC
/// 命令在运行时切换嵌入模式。默认使用单向量（`Embedder`），Pro 用户可切换为
/// 多向量（`MultiVectorEmbedder`）。
///
/// # 调研来源
///
/// - ColBERT (arXiv:2004.12832) — Late Interaction, token 级多向量
pub trait MultiVectorEmbedder: Send + Sync {
    /// 为文本中的每个 token 生成独立的嵌入向量。
    ///
    /// # 参数
    /// - `text`: 输入文本
    ///
    /// # 返回
    /// `Vec<Vec<f32>>`，每个内层 Vec 是一个 token 的嵌入向量。
    /// token 数量取决于分词器对 `text` 的切分结果。
    async fn embed_tokens(&self, text: &str) -> anyhow::Result<Vec<Vec<f32>>>;

    /// 批量多向量嵌入。
    ///
    /// 默认实现逐条调用 `embed_tokens`；适配器可覆盖为真正的批量处理。
    async fn embed_tokens_batch(&self, texts: &[String]) -> anyhow::Result<Vec<Vec<Vec<f32>>>> {
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.embed_tokens(text).await?);
        }
        Ok(results)
    }
}

/// MaxSim 交互计算（ColBERT Late Interaction 核心算法, REQ-PERF-008）。
///
/// 对 query 的每个 token 向量，在 document 的所有 token 向量中找最大余弦相似度，
/// 然后将所有 query token 的最大相似度求和，得到 query-document 相关度分数。
///
/// 复杂度：O(q × d)，其中 q = query token 数，d = document token 数。
///
/// # 参数
/// - `query_token_vectors`: query 的 token 级嵌入向量列表
/// - `doc_token_vectors`: document 的 token 级嵌入向量列表
///
/// # 返回
/// MaxSim 分数（越高越相关）。如果任一输入为空返回 0.0。
pub fn maxsim(query_token_vectors: &[Vec<f32>], doc_token_vectors: &[Vec<f32>]) -> f32 {
    if query_token_vectors.is_empty() || doc_token_vectors.is_empty() {
        return 0.0;
    }
    let mut total = 0.0;
    for q_vec in query_token_vectors {
        let mut max_sim = f32::MIN;
        for d_vec in doc_token_vectors {
            let sim = cosine_sim(q_vec, d_vec);
            if sim > max_sim {
                max_sim = sim;
            }
        }
        total += max_sim.max(0.0);
    }
    total
}

/// 余弦相似度（内部辅助函数）。
fn cosine_sim(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    let mut dot = 0.0;
    let mut norm_a = 0.0;
    let mut norm_b = 0.0;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        norm_a += a[i] * a[i];
        norm_b += b[i] * b[i];
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom == 0.0 { 0.0 } else { dot / denom }
}

/// 代码符号抽取端口（REQ-RAG-031 代码感知 RAG, Pro feature）。
///
/// 使用 tree-sitter 对代码文件进行 AST 级符号分析，建立「符号 → 位置 → chunk」三级映射。
/// 端口定义在 core，实现在 infra（`SymbolEngine`，Pro 门控）。
/// 通过泛型注入 `ImportService`（同 `PageRenderer` / `OcrEngine` 模式）。
pub trait SymbolExtractor: Send + Sync {
    /// 检测文件语言（根据扩展名）。
    /// 返回 `None` 表示非代码文件（.md / .txt / .pdf 等）。
    fn detect_language(&self, file_path: &str) -> Option<String>;

    /// 从代码文本中抽取符号。
    ///
    /// # 参数
    /// - `content`: 代码文本（可能是整个文件或单个 chunk）
    /// - `language`: 语言标识（rust / typescript / python / go）
    /// - `chunk_id`: 关联的 chunk ID
    ///
    /// # 返回
    /// 抽取到的符号列表。解析失败返回空 Vec（不报错，优雅降级）。
    fn extract_symbols(&self, content: &str, language: &str, chunk_id: &str) -> Vec<CodeSymbol>;

    /// 代码感知分块：按函数/类边界分块（而非 token 窗口）。
    ///
    /// 每个顶级函数/类/结构体为一个独立 chunk。
    /// 超大函数（超过 max_tokens）内部按行分割。
    /// 每块附带符号上下文前缀（如 `// Symbol: foo (Function)\n`）。
    ///
    /// # 返回
    /// `Vec<(chunk_text, start_line, end_line)>`，行号为 1-based。
    fn split_by_symbols(
        &self,
        content: &str,
        language: &str,
        max_tokens: usize,
    ) -> Vec<(String, usize, usize)>;
}

/// 领域分类端口（REQ-VEC-013）：将文档内容自动分类到预定义领域。
///
/// 实现方案为嵌入质心分类（Embedding Centroid Classification）——
/// 预计算每个领域的代表性关键词嵌入质心，文档取首 N 个 chunk 的嵌入均值
/// 与各领域质心做余弦相似度比较，取最高者。
///
/// # 零 LLM 成本
///
/// 分类完全基于本地嵌入模型（fastembed/ONNX），不调用 LLM，
/// 零网络请求、零 API 费用，完全离线可用。
///
/// # 16 预定义领域
///
/// technology / legal / medical / finance / science / education / business /
/// engineering / literature / government / marketing / hr / design / data /
/// security / general
pub trait DomainClassifier: Send + Sync {
    /// 对文档内容进行领域分类，返回领域标识字符串。
    ///
    /// # 参数
    /// - `chunks`: 文档分块文本列表（通常取前 5 个 chunk 作为样本）
    ///
    /// # 返回
    /// 16 个预定义领域之一的字符串标识。分类失败时返回 `"general"`。
    async fn classify(&self, chunks: &[String]) -> anyhow::Result<String>;
}

/// 代码执行器端口（REQ-RAG-032，Pro feature）。
///
/// 借鉴 CodeForge 多语言执行器：在沙箱中安全执行代码片段并返回结果。
/// Agent 在回答代码相关问题时可调用 `execute_code` 工具验证结果，
/// 大幅提升回答可信度。
///
/// # 安全限制
///
/// - 超时：默认 10s（`CodeExecutorConfig::timeout_secs`，硬编码上限 30s）
/// - 内存：默认 64MB（`CodeExecutorConfig::memory_limit_mb`，硬编码上限 256MB）
/// - 网络：永远禁用（`CodeExecutorConfig::allow_network` 恒为 false）
/// - 文件系统：仅 stdout/stderr，无持久化写入
///
/// # 对象安全设计
///
/// 与 `Reranker`/`QueryRewriter` 端口相同，使用手动 `Pin<Box<Future>>` 返回类型
/// 以保证对象安全（dyn-compatible）。`CodeExecutor` 以 `Option<Arc<dyn CodeExecutor>>`
/// 形式注入 `AgentEngine`，实现运行时开关——Free 版本不注入 executor，
/// `execute_code` 工具返回 Pro 错误提示（优雅降级）。
///
/// # 实现方
///
/// - `WasmExecutor`（`crates/infra/src/wasm_executor.rs`，Pro 门控）—
///   第一阶段 MVP 使用本地进程执行（python3/node），后续替换为 WASM 沙箱
/// - `MockExecutor`（core，测试用）— 返回预定义结果
/// - `NoExecutor`（core，Free 版本占位）— 始终返回错误
pub trait CodeExecutor: Send + Sync {
    /// 执行代码片段，返回输出结果。
    ///
    /// # 参数
    /// - `code`: 代码文本
    /// - `language`: 语言标识（python / javascript / rust）
    /// - `stdin`: 标准输入（可选）
    ///
    /// # 返回
    /// `ExecutionResult` 包含 stdout / stderr / exit_code / duration_ms / timed_out。
    fn execute<'a>(
        &'a self,
        code: &'a str,
        language: &'a str,
        stdin: Option<&'a str>,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = anyhow::Result<ExecutionResult>> + Send + 'a>,
    >;

    /// 返回支持的语言列表。
    fn supported_languages(&self) -> Vec<&str>;

    /// 返回执行器配置。
    fn config(&self) -> CodeExecutorConfig;
}

#[cfg(test)]
mod active_gate_tests;
#[cfg(test)]
mod agent_tests;
#[cfg(all(test, feature = "pro"))]
mod audit_tests;
#[cfg(test)]
mod auto_dream_tests;
#[cfg(test)]
mod benchmark_tests;
#[cfg(test)]
pub mod budget_tests;
#[cfg(test)]
mod cache_tests;
#[cfg(test)]
mod chat_tests;
#[cfg(test)]
mod code_executor_tests;
#[cfg(test)]
mod concurrency_tests;
#[cfg(test)]
mod coordinator_tests;
#[cfg(test)]
mod domain_tests;
#[cfg(test)]
mod entity_extractor_tests;
#[cfg(test)]
mod export_tests;
#[cfg(test)]
mod graph_analyzer_tests;
#[cfg(test)]
mod graph_export_tests;
#[cfg(test)]
mod graph_retriever_tests;
#[cfg(test)]
mod hooks_tests;
#[cfg(test)]
mod hybrid_retriever_tests;
#[cfg(test)]
mod idempotency_tests;
#[cfg(test)]
mod import_tests;
#[cfg(test)]
mod late_chunking_tests;
#[cfg(test)]
mod license_tests;
#[cfg(test)]
mod llm_aux_tests;
#[cfg(test)]
mod llm_router_tests;
#[cfg(test)]
mod loader_tests;
#[cfg(test)]
mod memory_store_tests;
#[cfg(test)]
mod progressive_injector_tests;
#[cfg(test)]
mod prompt_compressor_tests;
#[cfg(test)]
mod property_tests;
#[cfg(test)]
mod proposition_splitter_tests;
#[cfg(test)]
mod quality_gate_tests;
#[cfg(test)]
mod rag_eval_dataset_tests;
#[cfg(test)]
mod rag_eval_tests;
#[cfg(test)]
mod retrieval_memory_tests;
#[cfg(test)]
mod retriever_tests;
#[cfg(test)]
mod security_tests;
#[cfg(test)]
mod session_strip_tests;
#[cfg(test)]
mod speculative_rag_tests;
#[cfg(test)]
mod splitter_tests;
#[cfg(test)]
mod step_cache_tests;
#[cfg(test)]
mod stream_parse_tests;
#[cfg(test)]
mod sub_agent_tests;
#[cfg(test)]
mod summary_tree_tests;
#[cfg(test)]
mod sync_tests;
#[cfg(all(test, feature = "pro"))]
mod vlm_prompt_tests;
#[cfg(test)]
mod web_search_tests;
#[cfg(test)]
mod workflow_tests;
