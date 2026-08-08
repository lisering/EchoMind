# EchoMind v1.3.0 发布说明

发布日期: 2026-08-07

## 概述

EchoMind（灵犀）v1.3.0 是 v1.2.0 之后的第三个功能更新版本。本次发布新增 5 项功能，涵盖知识图谱导出、文档摘要自动生成、自定义快捷指令模板、Obsidian 风格双向链接和 ChatGPT 风格对话分支树，进一步强化了知识管理与对话探索能力。

## 新增功能

### 1. 知识图谱导出 GraphML/JSON-LD（REQ-EXP-006）

- 双格式导出：GraphML（XML，Gephi/Cytoscape 兼容）+ JSON-LD（W3C 标准，Neo4j 兼容）
- `graph_export.rs` 新增模块：`export_graphml()` 手动拼接 XML + XML 特殊字符转义；`export_jsonld()` 使用 `serde_json` 构建 `@context` + `@graph` 数组
- `export_graph` IPC 命令（format 参数选择 graphml/jsonld）
- 前端 `graph-viewer.js` 新增 GraphML + JSON-LD 导出按钮 + `exportGraphData()` 函数
- 3 个单元测试（TC-EXP-GRAPH-001~003）+ 2 个 E2E 测试（TC-EXP-GRAPH-004~005）
- 零新增依赖（复用 `serde_json`）

### 2. 文档摘要自动生成（REQ-ING-019）

- 导入文档后自动调用 LLM 生成 200-300 字摘要，显示在文档列表中
- `Document` 结构体新增 `summary: Option<String>` 字段（`#[serde(default)]` 向后兼容）
- `documents` 表新增 `summary TEXT` 列 + `ALTER TABLE` 迁移
- `ImportService` 导入管线集成 `generate_summary()` 异步任务（不阻塞导入，失败优雅降级）
- 文档文本截断：取前 4000 字符作为 LLM 输入
- 2 个 IPC 命令：`get_document_summary` / `regenerate_summary`
- 前端文档列表摘要展开/折叠（懒加载）+ 重新生成按钮
- `import.auto_summary` 设置键（默认 `true`）
- 5 个 TDD 测试（TC-ING-SUMMARY-001~005）
- 零新增依赖（复用 `LLMProvider` trait）

### 3. 自定义快捷指令模板系统（REQ-RAG-038）

- 用户可创建自定义快捷指令模板（如 `/legal`、`/review`、`/paper`），`{query}` 占位符自动展开
- `PromptTemplate` 结构体（id/name/label/description/icon/prompt_template/created_at/updated_at + 验证方法）
- 3 个 CRUD IPC 命令：`save_prompt_template` / `list_prompt_templates` / `delete_prompt_template`（settings 表 JSON 存储）
- `slash-commands.js` 合并 6 个系统指令 + 自定义模板（`processSlashCommand` 展开 `/command query` → `promptTemplate.replace('{query}', query)`）
- 设置面板模板管理 UI（列表 + 新增/编辑表单 + 删除确认）
- `.slash-command-badge` 角标样式区分系统/自定义指令
- 8 个 E2E 测试
- 零新增依赖

### 4. Markdown 笔记双向链接（REQ-ING-020）

- Obsidian 风格 `[[wiki-link]]` 双向链接：导入时自动解析、建立索引、前端渲染可点击链接
- `WikiLink` 结构体（id/source_doc_id/target/chunk_id/created_at + `new()` 方法）
- `wiki_link_parser.rs` 纯函数解析器（UTF-8 安全字符遍历 `Vec<char>`，支持 `[[target]]` / `[[target|alias]]` / `[[target#heading]]` / `[[target#heading|alias]]` 语法，跳过代码块和行内代码，同 chunk 内 target 去重）
- `wiki_links` 表（5 列 + 3 索引 + `ON DELETE CASCADE`）
- Storage trait 3 方法：`add_wiki_links` / `get_forward_links` / `get_backlinks`
- 导入管线集成 `index_wiki_links` 方法（5 路径覆盖：index_inner/index_with_section/index_text_mmap/index_md_mmap/index_code_inner）
- 3 个 IPC 命令：`get_forward_links` / `get_backlinks` / `rebuild_wiki_links`
- 前端 `wiki-links.js` 模块（`enhanceWikiLinks` DOM 渲染 + 面板渲染）
- `markdown.js` 集成（`renderMarkdown` 流程中调用 `enhanceWikiLinks`）
- 15 个单元测试（TC-ING-WIKI-001~015）+ 8 个 E2E 测试
- 零新增依赖（复用已有 Rust crate）

### 5. 对话分支/版本树（REQ-RAG-039）

- ChatGPT 风格可视化的对话分支树，让用户直观看到对话的所有分支路径
- `ConversationTreeNode` + `ConversationTree` 结构体（4 单元测试，UTF-8 安全截断）
- 复用 `messages` 表 `turn_group` + `version` 列 + `turn_active_versions` 表，零新增数据库表
- 2 个 IPC 命令：`get_conversation_tree` / `branch_from_message`
- 前端 `turn-tree.js` 树视图面板（`openConversationTreePanel` / `renderTreeNode` / `switchToBranch`）
- 对话区新增「分支树」按钮
- `components.css` 新增 `.conversation-tree-*` 树样式
- 5 个 E2E 测试（TC-RAG-BRANCH-001~005）
- 零新增依赖

## Bug 修复

- **DOC_COLS 列不匹配**（S55 遗留）：`DOC_COLS` 常量缺少 `summary` 列，导致 `SELECT` 查询返回 8 列而 `row_to_document` 访问第 9 列时报 `Invalid column index: 8`。修复：`DOC_COLS` 添加 `summary` + `INSERT OR REPLACE` 添加 `?9` + `doc.summary` 参数。

## 技术规格

| 指标 | v1.2.0 | v1.3.0 | 变化 |
|---|---|---|---|
| Rust 测试 (Free) | 766 passed | 803 passed | +37 |
| Rust 测试 (Pro) | 1057 passed | 1094 passed | +37 |
| E2E 桥接测试 | 950 passed | 955 passed | +5 |
| Vitest 前端测试 | 407 passed | 407 passed | — |
| IPC 命令数 | 141 | 152 | +11 |
| 前端 ES module | 50 | 52 (+wiki-links.js, turn-tree.js 扩展) | +2 |
| esbuild 构建行数 | 11670 | 12228 | +558 |
| TypeScript errors | 0 | 0 | — |
| 新增 SRS 需求 | — | 5 条 | +5 |
| 新增依赖 | — | — | 0 |

## 新增 SRS 需求

| REQ ID | 域 | 描述 | 状态 | Free/Pro |
|---|---|---|---|---|
| REQ-EXP-006 | EXP | 本地知识图谱导出（GraphML/JSON-LD） | Implemented | Free |
| REQ-ING-019 | ING | 文档摘要自动生成 | Implemented | Free |
| REQ-RAG-038 | RAG | 对话模板/快捷指令系统 | Implemented | Free |
| REQ-ING-020 | ING | Markdown 笔记双向链接 | Implemented | Free |
| REQ-RAG-039 | RAG | 对话分支/版本树 | Implemented | Free |

## 新增 IPC 命令

| 命令 | 说明 | Session | 门控 |
|---|---|---|---|
| `export_graph` | 导出知识图谱为 GraphML/JSON-LD | S54 | Free |
| `get_document_summary` | 获取文档摘要 | S55 | Free |
| `regenerate_summary` | 重新生成文档摘要 | S55 | Free |
| `save_prompt_template` | 保存自定义快捷指令模板 | S56 | Free |
| `list_prompt_templates` | 列出自定义模板 | S56 | Free |
| `delete_prompt_template` | 删除自定义模板 | S56 | Free |
| `get_forward_links` | 获取文档正向链接 | S57 | Free |
| `get_backlinks` | 获取文档反向链接 | S57 | Free |
| `rebuild_wiki_links` | 重建 wiki-link 索引 | S57 | Free |
| `get_conversation_tree` | 获取对话分支树 | S58 | Free |
| `branch_from_message` | 从消息创建新分支 | S58 | Free |

## 新增设置键

| 设置键 | 类型 | 默认值 | 说明 | Session |
|---|---|---|---|---|
| `import.auto_summary` | bool | `true` | 导入时自动生成文档摘要 | S55 |

## 新增数据库表

### wiki_links 表（S57）

```sql
CREATE TABLE IF NOT EXISTS wiki_links (
    id TEXT PRIMARY KEY,
    source_doc_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    target_doc_name TEXT NOT NULL,
    target_doc_id TEXT REFERENCES documents(id) ON DELETE SET NULL,
    chunk_id TEXT NOT NULL REFERENCES chunks(id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_wiki_links_source ON wiki_links(source_doc_id);
CREATE INDEX IF NOT EXISTS idx_wiki_links_target ON wiki_links(target_doc_id);
CREATE INDEX IF NOT EXISTS idx_wiki_links_chunk ON wiki_links(chunk_id);
```

### documents 表扩展（S55）

```sql
ALTER TABLE documents ADD COLUMN summary TEXT;
```

## 向后兼容性

- v1.2.0 数据库无缝升级（`summary` 列通过 `ALTER TABLE` 迁移，旧文档 `summary = NULL`）
- 所有新功能默认关闭或降级（opt-in）：
  - 文档摘要默认开启但失败优雅降级（`summary` 保持 `None`）
  - 自定义模板需用户手动创建
  - Wiki 链接在导入时自动解析（无匹配目标仍存储，`target_doc_id = None`）
  - 对话分支树是只读展示 + 分支创建操作
- 原有功能不受影响
- Free 版本全部可用

## 零新增依赖

v1.3 版本全部 5 个功能均使用已有 Rust crate 和前端库实现，零新增依赖：
- `serde_json`（已有）→ JSON-LD 导出序列化
- `regex`（已有）→ Wiki-link 模式匹配（后改为字符遍历）
- `pulldown-cmark`（已有）→ Markdown 解析
- `D3.js v7`（已有）→ 树视图

## 平台支持

- macOS (Intel + Apple Silicon)
- Windows 10/11
- Linux (Ubuntu 20.04+)
