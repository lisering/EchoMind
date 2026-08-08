# 数据库 Schema

> **状态**：活跃 | **最后审阅**：2026-07-25（v2 新增 FTS5 虚拟表 + `rag.hybrid_search` 设置键，REQ-RAG-010）
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
