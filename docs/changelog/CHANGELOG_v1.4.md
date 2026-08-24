# EchoMind v1.4.0 发布说明

> **发布日期**: 2026-08-07 | **版本**: 1.4.0 | **许可**: 双许可（AGPL-3.0 + 商业许可）

---

## 新功能

### 1. .xlsx/.csv 电子表格导入（REQ-ING-021）

支持 Excel 和 CSV 文件导入知识库，自动转换为 Markdown 表格格式。

- **calamine v0.36**（纯 Rust，MIT/Apache-2.0）支持 .xlsx/.xls/.ods/.xlsb
- **csv v1.4**（纯 Rust，MIT/Unlicense）支持 CSV 自动分隔符检测
- 多 Sheet 以二级标题分隔，保留表头/数据行结构
- 支持的文档格式从 13 种扩展到 15 种

### 2. 对话全文搜索（REQ-RAG-040）

在所有会话中搜索对话内容，快速定位历史讨论。

- FTS5 全文索引（trigram 分词器，与 chunks_fts 一致）
- 3 触发器自动同步 messages 表变更
- BM25 算法排序，短查询（<3 字符）回退 LIKE 全表扫描
- 前端搜索弹框「会话/对话」模式切换

### 3. Contextual Retrieval 上下文增强嵌入（REQ-RAG-041）

Anthropic 2024 论文实现：为每个 chunk 添加文档名前缀后再嵌入，提升检索精度。

- 复用已有 `build_contextual_text()` 函数（零 LLM 调用成本）
- `contextual_retrieval_enabled` 开关控制（默认开启）
- `set_contextual_retrieval` + `rebuild_contextual_embeddings` 2 IPC 命令
- 前端性能设置面板 toggle + 重建按钮

### 4. 文档标签系统（REQ-ING-022）

用户自定义标签管理，按主题/项目/类型分类文档。

- `Document` 结构体新增 `tags: Vec<String>` 字段（`#[serde(default)]` 向后兼容）
- 数据库 `documents` 表新增 `tags TEXT NOT NULL DEFAULT '[]'` 列（JSON 数组存储）
- 4 个 IPC 命令：`add_document_tag` / `remove_document_tag` / `list_all_tags` / `filter_documents_by_tag`
- 前端文档列表项标签徽章显示 + hover 标签输入框

### 5. commands.rs 模块化拆分（REQ-ARCH-008）

SRP 违例修复：7706 行单文件 → 14 个功能域子模块。

- `commands/mod.rs`（共享类型/常量/辅助函数）
- 14 子模块：chat / import / document / conversation / audit / license / settings / local_llm / security / agent / performance / graph / sync / features
- 零功能变更，纯重构，`generate_handler!` 注册不变

## 技术规格

| 指标 | v1.3.0 | v1.4.0 | 变化 |
|---|---|---|---|
| 支持文档格式 | 13 | 15 | +2（xlsx/csv） |
| IPC 命令数 | 152 | 159 | +7 |
| Rust Free 测试 | 803 | 808 | +5 |
| Rust Pro 测试 | 1094 | 1094 | — |
| E2E 测试 | 955 | 955 | — |
| esbuild 行数 | 12228 | 12422 | +194 |
| 新增依赖 | — | calamine v0.36 + csv v1.4 | 2 |

## 向后兼容性

- 数据库自动迁移：`ALTER TABLE documents ADD COLUMN tags TEXT NOT NULL DEFAULT '[]'`
- 旧版数据库无需手动迁移，首次打开自动升级
- `Document.tags` 使用 `#[serde(default)]`，旧序列化数据反序列化为空数组
- 所有新功能默认关闭或默认安全值，不影响现有使用

## 铁律合规

- 铁律五：零 unwrap/expect/panic/unreachable/todo/unimplemented（workspace deny lint）
- 铁律七：代码变更同步更新 SRS / database-schema / CURRENT_STATE
- 铁律九：calamine + csv 均为纯 Rust crate
- 铁律十：Contextual Retrieval 为 Anthropic 2024 前沿技术实现
- 铁律十一：修改后运行 sync-to-github.sh 同步
- 铁律十四：commands.rs 拆分为 14 子模块（SRP 违例修复）
