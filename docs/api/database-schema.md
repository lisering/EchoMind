# 数据库 Schema

> **状态**：活跃 | **最后审阅**：2026-08-16（v4 v2.0 S17 文档同步：新增 workspaces / conversation_bookmarks / import_logs 表文档，更新表总数 23→26 张表 + 2 FTS5 虚拟表；v3 全量同步 23 张表：新增 entities / propositions / summary_nodes / retrieval_memory / turn_active_versions / session_todos / scratch_logs / idempotency_records / budget_records / audit_log / embeddings_cache + messages.security_tainted 列；v2 新增 FTS5 虚拟表）
>
> 源码：`crates/infra/src/sqlite_storage.rs`。
> 数据库文件：`<app_data_dir>/echomind.db`（SQLite + WAL 模式）。

---

## 1. 表结构

### 1.1 documents

| 列 | 类型 | 约束 | 说明 |
|---|---|---|---|
| `id` | TEXT | PRIMARY KEY | UUID v4 |
| `file_path` | TEXT | UNIQUE NOT NULL | 入库后本地路径 |
| `file_hash` | TEXT | NOT NULL | MD5 内容指纹（去重用） |
| `status` | TEXT | NOT NULL | Pending / Processing / Indexed / Failed(reason) |
| `status_reason` | TEXT | | 失败原因（status=Failed 时填充） |
| `created_at` | INTEGER | NOT NULL | Unix 秒级时间戳 |
| `original_path` | TEXT | | 源文件 canonical 路径（REQ-SYNC-002；手动导入为 NULL，监听导入为源路径） |
| `domain` | TEXT | | 领域分类标签（REQ-VEC-013；16 预定义领域之一，NULL=未分类） |
| `summary` | TEXT | | 文档摘要（REQ-ING-019；LLM 生成 200-300 字，NULL=未生成或生成失败） |
| `tags` | TEXT | NOT NULL DEFAULT '[]' | 用户自定义标签（REQ-ING-022；JSON 数组字符串，如 `["法律","重要"]`，默认空数组） |
| `workspace_id` | TEXT | NOT NULL DEFAULT 'default' | 所属工作空间 ID（REQ-WS-001；旧数据迁移默认 `default`） |

### 1.2 chunks

| 列 | 类型 | 约束 | 说明 |
|---|---|---|---|
| `id` | TEXT | PRIMARY KEY | UUID v4 |
| `doc_id` | TEXT | REFERENCES documents(id) ON DELETE CASCADE | 所属文档 ID |
| `content` | TEXT | NOT NULL | 分块文本内容 |
| `token_count` | INTEGER | NOT NULL | token 计数 |
| `sequence` | INTEGER | NOT NULL | 文档内顺序号 |

### 1.3 embeddings

| 列 | 类型 | 约束 | 说明 |
|---|---|---|---|
| `chunk_id` | TEXT | PRIMARY KEY / REFERENCES chunks(id) ON DELETE CASCADE | 关联分块 |
| `vector` | BLOB | NOT NULL | 原始 f32 字节序列（384 维 = 1536 bytes） |

### 1.4 conversations

| 列 | 类型 | 约束 | 说明 |
|---|---|---|---|
| `id` | TEXT | PRIMARY KEY | UUID v4 |
| `workspace_id` | TEXT | NOT NULL | 工作区标识（v1.0 固定 "default"） |
| `title` | TEXT | NOT NULL | 会话标题（首轮问答后自动提取） |
| `created_at` | INTEGER | NOT NULL | Unix 秒级时间戳 |
| `sort_order` | INTEGER | NOT NULL DEFAULT 0 | 拖拽排序位置（REQ-IX-002 会话拖拽排序） |

### 1.5 messages

| 列 | 类型 | 约束 | 说明 |
|---|---|---|---|
| `id` | TEXT | PRIMARY KEY | 自增或 UUID |
| `conversation_id` | TEXT | REFERENCES conversations(id) ON DELETE CASCADE | 所属会话 |
| `role` | TEXT | NOT NULL | user / assistant / system |
| `content` | TEXT | NOT NULL | 消息正文 |
| `sources` | TEXT | | JSON 序列化的 `Vec<RetrievalResult>`（仅助手消息） |
| `reasoning` | TEXT | | 助手消息的推理思考过程（reasoning_content，DeepSeek R1 等推理模型） |
| `turn_group` | TEXT | NOT NULL DEFAULT '' | 轮次分组 ID（用户消息编辑版本管理；同一问题的不同编辑版本共享同一个 turn_group；旧消息为空字符串） |
| `version` | INTEGER | NOT NULL DEFAULT 1 | 轮次内版本号（从 1 递增；用户编辑问题后产生新版本，旧版本保留供分页查看） |
| `created_at` | INTEGER | NOT NULL | Unix 秒级时间戳 |
| `security_tainted` | INTEGER | NOT NULL DEFAULT 0 | 安全污染标记（Q05 安全态势分层；0=未标记，1=被安全筛查标记为不可信；上下文构建时过滤被标记的消息） |

**索引**：`idx_messages_conv` (conversation_id)、`idx_messages_turn` (conversation_id, turn_group, version)

### 1.6 settings

| 列 | 类型 | 约束 | 说明 |
|---|---|---|---|
| `key` | TEXT | PRIMARY KEY | 设置键名 |
| `value` | TEXT | NOT NULL | 设置值（敏感值加密存储） |

**工作流存储（REQ-RAG-030）**：DAG 工作流模板以 JSON 序列化存储在 settings 表中，不新建独立表：
- `workflow.{id}` — 单个工作流模板的 JSON 序列化（`Workflow` 结构体）
- `workflow.index` — 所有工作流 ID 的 JSON 数组（`string[]`），用于 `list_workflows` 命令快速枚举

### 1.7 chunks_fts（FTS5 虚拟表）

| 列 | 类型 | 说明 |
|---|---|---|
| `content` | TEXT | 分块文本内容（使用 `build_contextual_text()` 拼接文档名上下文前缀，REQ-RAG-041 Contextual Retrieval） |
| `doc_id` | TEXT | 所属文档 ID（unindexed，仅存储不索引） |
| `chunk_id` | TEXT | 分块 ID（unindexed，仅存储不索引） |

```sql
CREATE VIRTUAL TABLE chunks_fts USING fts5(
    content,
    doc_id UNINDEXED,
    chunk_id UNINDEXED,
    tokenize = 'trigram'
);
```

- **分词器**：`trigram`（SQLite 3.34+），支持中英文子串匹配
- **同步机制**：`add_chunk` / `delete_chunks_by_doc` / `delete_document` 写入时自动同步 FTS5 索引
- **迁移**：旧库启动时 `migrate_schema` 检测缺失表自动创建并回填（`backfill_fts_if_needed`）
- **关联需求**：REQ-RAG-010 混合检索（关键词检索通道）

### 1.7b messages_fts（FTS5 虚拟表，REQ-RAG-040）

| 列 | 类型 | 说明 |
|---|---|---|
| `content` | TEXT | 消息文本内容（与 `messages.content` 同步） |
| `conversation_id` | TEXT | 所属会话 ID（unindexed，仅存储不索引） |
| `message_id` | TEXT | 消息 ID（unindexed，仅存储不索引，关联 `messages.id`） |

```sql
CREATE VIRTUAL TABLE messages_fts USING fts5(
    content,
    conversation_id UNINDEXED,
    message_id UNINDEXED,
    tokenize = 'trigram'
);
```

- **分词器**：`trigram`（与 `chunks_fts` 一致），支持中英文子串匹配
- **同步机制**：3 触发器自动同步 `messages` 表 INSERT/DELETE/UPDATE
  - `messages_fts_ai`：AFTER INSERT ON messages → INSERT INTO messages_fts
  - `messages_fts_ad`：AFTER DELETE ON messages → DELETE FROM messages_fts WHERE message_id = old.id
  - `messages_fts_au`：AFTER UPDATE ON messages（content 变化时）→ DELETE + INSERT
- **迁移**：旧库启动时 `backfill_messages_fts_if_needed` 检测 messages 有数据但 messages_fts 为空时回填
- **短查询回退**：<3 字符的查询回退为 SQL LIKE 全表扫描（trigram 分词器要求 ≥3 字符）
- **关联需求**：REQ-RAG-040 对话全文搜索

### 1.8 entity_relations（知识图谱边，REQ-RAG-027）

| 列 | 类型 | 约束 | 说明 |
|---|---|---|---|
| `id` | TEXT | PRIMARY KEY | UUID v4 |
| `subject` | TEXT | NOT NULL | 主体实体文本 |
| `relation_type` | TEXT | NOT NULL | 关系类型（defined_as / part_of / depends_on / uses / implements / extends / references / related_to） |
| `object` | TEXT | NOT NULL | 客体实体文本 |
| `chunk_id` | TEXT | NOT NULL / REFERENCES chunks(id) ON DELETE CASCADE | 来源 chunk ID |
| `confidence` | REAL | NOT NULL | 置信度（0.5-1.0，精确匹配 1.0，模糊匹配 0.7，同句共现 0.5） |

**索引**：`idx_relations_subject` (subject)、`idx_relations_object` (object)、`idx_relations_chunk` (chunk_id)

- **写入时机**：文档导入时 `ImportService::index_relations()` 在 `index_entities()` 后调用
- **级联删除**：删除 chunk 自动清理其所有关系（`ON DELETE CASCADE`）
- **关联需求**：REQ-RAG-027 知识图谱实体关系检索

### 1.9 code_symbols（代码符号索引，REQ-RAG-031）

| 列 | 类型 | 约束 | 说明 |
|---|---|---|---|
| `id` | TEXT | PRIMARY KEY | UUID v4 |
| `chunk_id` | TEXT | NOT NULL / REFERENCES chunks(id) ON DELETE CASCADE | 关联 chunk ID |
| `name` | TEXT | NOT NULL | 符号名称（函数名/类名/结构体名等） |
| `kind` | TEXT | NOT NULL | 符号类型（function/method/class/struct/interface/enum/constant/module） |
| `language` | TEXT | NOT NULL | 编程语言（rust/typescript/python/go） |
| `start_line` | INTEGER | NOT NULL | 起始行号（1-based） |
| `end_line` | INTEGER | NOT NULL | 结束行号（1-based） |
| `signature` | TEXT | | 函数签名（仅函数/方法有值） |

**索引**：`idx_symbols_name` (name)、`idx_symbols_chunk` (chunk_id)、`idx_symbols_kind` (kind)

- **写入时机**：代码文件导入时 `ImportService::index_symbols()` 通过 tree-sitter AST 抽取（Pro feature）
- **级联删除**：删除 chunk 自动清理其所有符号（`ON DELETE CASCADE`）
- **关联需求**：REQ-RAG-031 代码感知 RAG（Pro feature）

### 1.10 memory_entries（对话记忆条目，REQ-RAG-033）

| 列 | 类型 | 约束 | 说明 |
|---|---|---|---|
| `id` | TEXT | PRIMARY KEY | UUID v4 |
| `tier` | TEXT | NOT NULL | 记忆层级（wing/hall/room） |
| `content` | TEXT | NOT NULL | 记忆内容文本 |
| `source` | TEXT | NOT NULL | 记忆来源（user_statement/assistant_answer/auto_extracted/user_pinned） |
| `conversation_id` | TEXT | | 关联会话 ID（可选） |
| `created_at` | INTEGER | NOT NULL | 创建时间（Unix 秒级时间戳） |
| `last_accessed` | INTEGER | NOT NULL | 最后访问时间（Unix 秒级时间戳） |
| `access_count` | INTEGER | NOT NULL DEFAULT 0 | 访问次数 |
| `importance` | REAL | NOT NULL DEFAULT 0.5 | 重要性分数（0.0-1.0） |

**索引**：`idx_memory_tier` (tier)、`idx_memory_content` (content)

**写入时机**：
- LLM 辅助从对话中提取关键事实 → Wing 层（importance 0.5）
- 用户手动置顶 → Room 层（importance 1.0）
- AutoDream 后台整合 → 提升/遗忘

**关联需求**：REQ-RAG-033 持久化记忆系统（IfAI 三层记忆 + Bamboo-agent 上下文压缩）

---

### 1.11 wiki_links（Wiki 双向链接，REQ-ING-020）

| 列 | 类型 | 约束 | 说明 |
|---|---|---|---|
| `id` | TEXT | PRIMARY KEY | UUID v4 |
| `source_doc_id` | TEXT | NOT NULL, FK → documents(id) ON DELETE CASCADE | 源文档 ID（包含 `[[link]]` 的文档） |
| `target` | TEXT | NOT NULL | wiki-link 目标文本（`[[target]]` 中的 `target`） |
| `chunk_id` | TEXT | NOT NULL, FK → chunks(id) ON DELETE CASCADE | 来源 chunk ID |
| `created_at` | INTEGER | NOT NULL | 创建时间（Unix 秒级时间戳） |

**索引**：`idx_wiki_links_source` (source_doc_id)、`idx_wiki_links_target` (target)、`idx_wiki_links_chunk` (chunk_id)

**写入时机**：
- 导入 Markdown 文档时，解析 `[[wiki-link]]` 语法并批量写入（`ImportService.index_wiki_links`）
- `rebuild_wiki_links` IPC 命令重建全量索引

**查询方式**：
- 正向链接：`WHERE source_doc_id = ?` → 该文档引用了哪些文档
- 反向链接：`WHERE target LIKE '%doc_name%'` → 哪些文档引用了该文档

**关联需求**：REQ-ING-020 Markdown 笔记双向链接（Obsidian 风格 `[[wiki-link]]` 语法）

---

### 1.12 pending_inputs（Durable Prompt Admission，B05）

**用途**：存储已接纳但未提升为正式消息的用户输入。借鉴 OpenCode 的 Prompt Admission / Promotion 机制，用户消息先持久化到此表，在安全边界点（如流式生成完成后）再提升为正式消息。

| 列名 | 类型 | 约束 | 说明 |
|---|---|---|---|
| `id` | TEXT | PRIMARY KEY | 接纳记录唯一标识（UUID v4） |
| `conversation_id` | TEXT | NOT NULL, FK → conversations(id) ON DELETE CASCADE | 所属会话 ID |
| `content` | TEXT | NOT NULL | 用户输入内容 |
| `delivery` | TEXT | NOT NULL DEFAULT 'queue' | 投递模式：`steer`（优先中断当前生成）或 `queue`（排队等待） |
| `created_at` | INTEGER | NOT NULL | 创建时间（Unix 秒级时间戳） |
| `promoted_seq` | INTEGER | NULL | 提升为正式消息后的 seq 序号；NULL = 未提升 |

**索引**：`idx_pending_inputs_conv` (conversation_id)、`idx_pending_inputs_promoted` (promoted_seq)

**写入时机**：
- `admit_input` IPC 命令接纳用户输入（持久化但不加入消息历史）
- 初始 `promoted_seq` 为 NULL

**查询方式**：
- `get_pending_inputs`：`WHERE conversation_id = ? AND promoted_seq IS NULL ORDER BY CASE delivery WHEN 'steer' THEN 0 ELSE 1 END, created_at ASC`
- `promote_input`：`UPDATE pending_inputs SET promoted_seq = ? WHERE id = ?`

**关联需求**：REQ-RAG-042 Durable Prompt Admission（持久化提示接纳）

---

### 1.13 entities（实体索引，REQ-PERF-006）

| 列 | 类型 | 约束 | 说明 |
|---|---|---|---|
| `id` | TEXT | PRIMARY KEY | UUID v4 |
| `chunk_id` | TEXT | NOT NULL, FK → chunks(id) ON DELETE CASCADE | 关联 chunk ID |
| `entity_text` | TEXT | NOT NULL | 实体文本 |
| `entity_type` | TEXT | NOT NULL | 实体类型（person / proper_noun / tech_term / identifier / date） |

**索引**：`idx_entities_chunk` (chunk_id)、`idx_entities_text` (entity_text)

- **写入时机**：文档导入时 `ImportService::index_entities()` 通过 `EntityExtractor::extract()` 纯规则 NER 抽取
- **关联需求**：REQ-PERF-006 实体链接增强（三路 RRF 实体匹配通道）

### 1.14 propositions（Proposition 索引，REQ-PERF-007）

| 列 | 类型 | 约束 | 说明 |
|---|---|---|---|
| `id` | TEXT | PRIMARY KEY | UUID v4 |
| `chunk_id` | TEXT | NOT NULL, FK → chunks(id) ON DELETE CASCADE | 关联 chunk ID |
| `content` | TEXT | NOT NULL | Proposition 文本（自包含的原子事实） |
| `embedding` | BLOB | | 嵌入向量（f32 字节序列） |
| `sequence` | INTEGER | NOT NULL | chunk 内顺序号 |

**索引**：`idx_propositions_chunk` (chunk_id)

- **写入时机**：文档导入时 `ImportService::index_propositions()` 通过 `PropositionSplitter` 分割 + 嵌入
- **关联需求**：REQ-PERF-007 Proposition 级原子分割

### 1.15 summary_nodes（RAPTOR 摘要树，REQ-PERF-009）

| 列 | 类型 | 约束 | 说明 |
|---|---|---|---|
| `id` | TEXT | PRIMARY KEY | UUID v4 |
| `doc_id` | TEXT | NOT NULL, FK → documents(id) ON DELETE CASCADE | 关联文档 ID |
| `level` | INTEGER | NOT NULL | 摘要层级（0=组摘要，1=主题摘要） |
| `content` | TEXT | NOT NULL | 摘要文本 |
| `child_ids` | TEXT | NOT NULL | 子节点 ID 列表（JSON 数组） |
| `embedding` | BLOB | | 嵌入向量 |

**索引**：`idx_summary_nodes_doc` (doc_id)、`idx_summary_nodes_level` (level)

- **写入时机**：`build_summary_tree` IPC 命令触发 `SummaryTreeBuilder` 构建
- **关联需求**：REQ-PERF-009 RAPTOR 多级摘要树

### 1.16 retrieval_memory（检索记忆，REQ-PERF-012）

| 列 | 类型 | 约束 | 说明 |
|---|---|---|---|
| `query_type` | TEXT | NOT NULL, PRIMARY KEY (复合) | 查询类型（factual / analytical / code / conversational） |
| `method` | TEXT | NOT NULL, PRIMARY KEY (复合) | 检索方法（vector_only / hybrid / hybrid_rerank / proposition / colbert） |
| `hit_count` | INTEGER | DEFAULT 0 | 命中次数（用户反馈正信号） |
| `miss_count` | INTEGER | DEFAULT 0 | 未命中次数（用户反馈负信号） |
| `avg_score` | REAL | DEFAULT 0 | 平均检索分数 |

- **写入时机**：`record_retrieval_feedback` IPC 命令记录用户反馈信号
- **读取时机**：`chat_inner` 启用检索记忆时，选择 hit_rate 最高的方法
- **关联需求**：REQ-PERF-012 自进化检索记忆

### 1.17 turn_active_versions（轮次活跃版本，分支切换持久化）

| 列 | 类型 | 约束 | 说明 |
|---|---|---|---|
| `conversation_id` | TEXT | NOT NULL, PRIMARY KEY (复合) | 会话 ID |
| `turn_group` | TEXT | NOT NULL, PRIMARY KEY (复合) | 轮次分组 ID |
| `active_version` | INTEGER | NOT NULL | 当前活跃版本号 |

- **写入时机**：`set_turn_active_version` IPC 命令（用户切换分页器版本时）
- **读取时机**：`get_turn_active_versions` IPC 命令（加载会话时恢复活跃版本）
- **关联需求**：REQ-RAG-039 对话分支树

### 1.18 session_todos（会话待办，B08）

| 列 | 类型 | 约束 | 说明 |
|---|---|---|---|
| `id` | TEXT | PRIMARY KEY | UUID v4 |
| `conversation_id` | TEXT | NOT NULL, FK → conversations(id) ON DELETE CASCADE | 所属会话 ID |
| `content` | TEXT | NOT NULL | 待办内容 |
| `status` | TEXT | NOT NULL DEFAULT 'pending' | 状态（pending / in_progress / completed） |
| `priority` | TEXT | NOT NULL DEFAULT 'medium' | 优先级（low / medium / high） |
| `position` | INTEGER | NOT NULL | 排序位置（升序） |
| `created_at` | INTEGER | NOT NULL | 创建时间（Unix 秒级时间戳） |

- **写入时机**：`add_session_todo` / `update_todo_status` / `delete_session_todo` IPC 命令
- **关联需求**：B08 Session Todo 持久化（OpenCode 借鉴）

### 1.19 scratch_logs（Scratch-Promote 记忆整合，Q01）

| 列 | 类型 | 约束 | 说明 |
|---|---|---|---|
| `id` | TEXT | PRIMARY KEY | UUID v4 |
| `date` | TEXT | NOT NULL | 日期（YYYY-MM-DD 格式，用于按日聚合） |
| `content` | TEXT | NOT NULL | 临时事实内容 |
| `created_at` | INTEGER | NOT NULL | 创建时间（Unix 秒级时间戳，用于过期清理，默认 14 天） |

- **写入时机**：`push_burst_turn` IPC 命令将对话中的事实写入 scratch 层
- **清理时机**：`cleanup_expired_scratch_logs()` 清理过期记录（默认 14 天）
- **整合时机**：`trigger_memory_consolidation` IPC 命令触发 LLM 审查 → promote 到长期记忆层
- **关联需求**：Q01 Scratch-Promote 两层记忆 + Consolidation 引擎（QM 借鉴）

### 1.20 idempotency_records（幂等性记录）

| 列 | 类型 | 约束 | 说明 |
|---|---|---|---|
| `key` | TEXT | PRIMARY KEY | 幂等键（如 `chat:{conversation_id}:{query_hash}`） |
| `timestamp` | INTEGER | NOT NULL | 记录时间（Unix 秒级时间戳） |

- **写入时机**：IPC 命令执行前检查幂等键，执行后写入
- **清理时机**：过期记录自动清理
- **关联需求**：Q03 幂等性保证（QM 借鉴）

### 1.21 budget_records（预算追踪，Q08）

| 列 | 类型 | 约束 | 说明 |
|---|---|---|---|
| `id` | INTEGER | PRIMARY KEY AUTOINCREMENT | 自增 ID |
| `principal` | TEXT | NOT NULL | 调用主体（如 conversation_id） |
| `timestamp` | INTEGER | NOT NULL | 记录时间（Unix 秒级时间戳） |
| `input_tokens` | INTEGER | NOT NULL | 输入 token 数 |
| `output_tokens` | INTEGER | NOT NULL | 输出 token 数 |
| `cost_usd` | REAL | NOT NULL | 估算费用（USD） |
| `model_name` | TEXT | NOT NULL | 模型名称 |

- **写入时机**：每次 LLM API 调用后自动记录
- **读取时机**：`get_budget_stats` / `set_budget_limit` IPC 命令
- **费用计算**：`LLM_COST_PER_MTOK` 环境变量控制每百万 token 价格
- **关联需求**：Q08 预算追踪（QM 借鉴 — LLM API 费用控制和速率限制）

### 1.22 audit_log（审计日志，REQ-SEC-015）

| 列 | 类型 | 约束 | 说明 |
|---|---|---|---|
| `id` | TEXT | PRIMARY KEY | UUID v4 |
| `action` | TEXT | NOT NULL | 审计动作（如 `pii_detected` / `encrypt_database` / `unlock_database`） |
| `details` | TEXT | NOT NULL | 审计详情（JSON 字符串） |
| `pii_count` | INTEGER | NOT NULL DEFAULT 0 | PII 检测计数 |
| `timestamp` | INTEGER | NOT NULL | Unix 秒级时间戳 |
| `prev_hash` | TEXT | | 前一条记录的哈希值（哈希链） |
| `curr_hash` | TEXT | | 当前记录的哈希值（`SHA256(prev_hash || entry_data)`） |

**索引**：`idx_audit_log_timestamp` (timestamp)

- **写入时机**：安全相关操作（PII 检测、加密、解锁、锁定等）自动记录
- **验证完整性**：`verify_audit_chain()` 重放哈希链检测篡改
- **关联需求**：REQ-SEC-015 审计日志哈希链

### 1.23 workspaces（工作空间，REQ-WS-001）

| 列 | 类型 | 约束 | 说明 |
|---|---|---|---|
| `id` | TEXT | PRIMARY KEY | UUID v4（默认工作空间 ID 为 `"default"`） |
| `name` | TEXT | NOT NULL | 工作空间名称 |
| `created_at` | INTEGER | NOT NULL | Unix 秒级时间戳 |

- **初始化**：首次启动时自动插入 `default` 工作空间（`INSERT OR IGNORE`）
- **关联需求**：REQ-WS-001 多知识库管理

### 1.24 conversation_bookmarks（对话书签，REQ-RAG-047）

| 列 | 类型 | 约束 | 说明 |
|---|---|---|---|
| `conversation_id` | TEXT | PRIMARY KEY | 会话 ID |
| `note` | TEXT | | 书签备注（可选） |
| `created_at` | INTEGER | NOT NULL | 创建时间（Unix 秒级时间戳） |

**索引**：`idx_bookmarks_created` (created_at)

- **写入时机**：`add_bookmark` IPC 命令
- **关联需求**：REQ-RAG-047 对话书签

### 1.25 import_logs（导入历史，REQ-ING-011）

| 列 | 类型 | 约束 | 说明 |
|---|---|---|---|
| `id` | INTEGER | PRIMARY KEY AUTOINCREMENT | 自增 ID |
| `timestamp` | INTEGER | NOT NULL | 导入时间戳 |
| `file_name` | TEXT | NOT NULL | 导入文件名 |
| `format` | TEXT | NOT NULL | 文件格式 |
| `result` | TEXT | NOT NULL | 导入结果（success/failed/skipped） |
| `error_message` | TEXT | | 错误信息（失败时填充） |
| `file_size` | INTEGER | | 文件大小（字节） |

**索引**：`idx_import_logs_timestamp` (timestamp)

- **写入时机**：每次导入操作自动记录
- **上限**：100 条，超过自动淘汰最旧记录
- **关联需求**：REQ-ING-011 导入历史记录

### 1.26 embeddings_cache（嵌入缓存）

| 列 | 类型 | 约束 | 说明 |
|---|---|---|---|
| `content_hash` | TEXT | PRIMARY KEY | 内容哈希（MD5） |
| `embedding` | BLOB | NOT NULL | 嵌入向量（f32 字节序列） |

- **写入时机**：`embed_batch` 时缓存已计算过的嵌入
- **读取时机**：`embed` / `embed_batch` 先查缓存避免重复计算
- **关联需求**：性能优化（嵌入去重缓存）

---

## 2. Settings 键值

| 键 | 用途 | 示例值 |
|---|---|---|
| `llm.api_key` | BYOK API Key | `sk-xxxx` |
| `llm.base_url` | OpenAI 兼容端点 | `https://api.openai.com` |
| `llm.model` | 模型名 | `gpt-4o-mini` |
| `license.is_pro` | Pro 授权状态 | `true` |
| `mm.vlm_enabled` | VLM 图片理解增强开关（REQ-MM-003，默认关闭） | `true` |
| `rag.hybrid_search` | 混合检索开关（默认启用） | `true` |
| `memory.enabled` | 持久化记忆系统开关（REQ-RAG-033，默认关闭） | `false` |

---

## 3. 存储特性

### 3.1 WAL 模式

```sql
PRAGMA journal_mode = WAL;
PRAGMA busy_timeout = 5000;
```

- WAL（Write-Ahead Logging）模式提升并发读写性能
- `busy_timeout` 防止并发写入时立即报错

### 3.2 向量存储

- 向量以 BLOB 存储（原始 f32 字节序列）
- 384 维向量 = 384 × 4 bytes = 1536 bytes
- 检索时全量加载所有向量到 Rust，计算余弦相似度
- 排序后返回 top-k 结果

### 3.3 级联删除

- `documents` 删除 → `chunks` 级联删除 → `embeddings` 级联删除
- `conversations` 删除 → `messages` 级联删除

### 3.4 FTS5 全文检索

- 关键词检索通道使用 SQLite FTS5 `trigram` 分词器
- 短查询（<3 字符）回退 SQL `LIKE` 匹配，避免 trigram 分词器对短查询无效果
- `keyword_search` 返回 `RetrievalResult`（含 `score` 为 BM25 rank），与向量检索结果格式一致以便 RRF 融合
- 检索时 JOIN `chunks` + `documents` 表补全 `doc_name` 与 `token_count` 字段

### 3.5 崩溃恢复

- 启动时执行 `cleanup_zombies()`：
  ```sql
  UPDATE documents SET status = 'Failed' WHERE status = 'Processing';
  ```
- 将上次会话遗留的 Processing 状态文档标记为 Failed
- 返回清理条数

---

## 4. 连接池

- 使用 `tokio::task::spawn_blocking` 包装所有 DB 操作
- 避免阻塞 async executor
- 数据库文件位于 `<app_data_dir>/echomind.db`
- 测试可通过 `ECHOMIND_DATA_DIR` 环境变量重定向数据目录

---

## 5. 数据库迁移安全（REQ-DB-008）

### 安全迁移流程

`init_schema(pool, db_path)` 执行步骤：

1. **创建表**（`CREATE TABLE IF NOT EXISTS`）
2. **安全迁移**（`safe_migrate_schema`）：
   - 检查是否有需要迁移的旧 schema（`needs_migration`）
   - 备份 DB 文件 → `.bak`（`backup_database_file`）
   - 执行 `migrate_schema`（ALTER TABLE + CREATE-COPY-DROP-RENAME）
   - `PRAGMA integrity_check` 完整性检查
   - 失败 → 从 `.bak` 恢复（`restore_from_backup`）
   - 成功 → 清理 `.bak`（`cleanup_backup`）
3. **创建索引**
4. **创建 FTS5 虚拟表**
5. **回填 FTS5 索引**（旧库升级场景）
6. **创建审计日志表**
7. **迁移后完整性检查**（`PRAGMA integrity_check`）

### 备份文件

- 路径：`<db_path>.bak`（与数据库同目录）
- 权限：0600（Unix，仅所有者可读写）
- 生命周期：迁移开始时创建 → 迁移成功后删除
- 备份失败时降级为无备份模式（warning 日志，不阻塞迁移）
