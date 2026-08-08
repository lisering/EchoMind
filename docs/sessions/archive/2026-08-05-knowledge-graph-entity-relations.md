# Session 归档：知识图谱实体关系检索（REQ-RAG-027）

> **日期**: 2026-08-05
> **Session 类型**: 后端功能开发
> **依赖**: REQ-PERF-006（EntityExtractor）、REQ-VEC-003（Storage）
> **状态**: ✅ 完成

---

## 1. 目标

借鉴 mnemo 知识图谱 RAG 架构，在现有实体抽取基础上新增实体间关系（知识图谱边），检索时可先匹配实体节点，再沿图边扩展到关联 chunk，提升关联查询召回率。

## 2. 变更清单

| 文件 | 变更类型 | 内容 |
|---|---|---|
| `crates/models/src/lib.rs` | 修改 | 新增 `EntityRelation` 结构体 + `GraphTriple` 结构体 + `to_triple()` 方法 |
| `crates/core/src/entity_extractor.rs` | 修改 | 新增 `extract_relations()` + `detect_relation()` + `match_pattern()` + `match_pattern_cn()` + `split_sentences()` |
| `crates/core/src/lib.rs` | 修改 | Storage trait 新增 5 个图查询方法（默认空操作）+ import `EntityRelation` |
| `crates/infra/src/sqlite_storage.rs` | 修改 | 新增 `entity_relations` 表 + 3 索引 + `KNOWN_TABLES` 更新 + 5 个 trait 方法实现 |
| `crates/core/src/import.rs` | 修改 | 新增 `index_relations()` + 4 条导入路径集成调用 |
| `crates/core/src/entity_extractor_tests.rs` | 修改 | 新增 6 个 TDD 测试（TC-RELATION-001~006） |
| `docs/SRS_v1.0.md` | 修改 | 新增 REQ-RAG-027（8 个 AC） |
| `docs/api/code-contracts.md` | 修改 | §8.8 更新 + §8.8.1 EntityRelation/GraphTriple + §8.8.2 Storage trait 方法 |
| `docs/api/database-schema.md` | 修改 | 新增 §1.8 `entity_relations` 表结构 |
| `docs/api/settings-reference.md` | 修改 | 新增 `rag.graph_retriever_enabled` 设置键（预留） |

## 3. 新增测试

| 测试 ID | 验证内容 |
|---|---|
| TC-RELATION-001 | 英文 `defined_as` 关系识别 |
| TC-RELATION-002 | 中文 `depends_on` 关系识别 |
| TC-RELATION-003 | 无关系文本返回空 Vec |
| TC-RELATION-004 | 多关系文本全部识别 |
| TC-RELATION-005 | 置信度分级（1.0 / 0.7 / 0.5） |
| TC-RELATION-006 | 三元组去重 |

## 4. 关键设计决策

### 4.1 关系抽取策略

采用**纯规则模式匹配**（零 LLM），覆盖中英文 8 种关系类型：
- 英文精确模式（置信度 1.0）：`is defined as` / `is part of` / `depends on` / `uses` / `implements` / `extends` / `references`
- 英文模糊模式（置信度 0.7）：`defines` / `requires` / `utilizes` / `inherits from` 等
- 中文精确模式（置信度 1.0）：`定义为` / `是.*的一部分` / `依赖` / `使用` / `实现` / `继承` / `引用`
- 同句共现兜底（置信度 0.5）：`related_to`

### 4.2 中文模式匹配空白容忍

`match_pattern_cn` 使用 `\s*` 允许中英混排时实体与关系词之间的空格（如 "HashMap 依赖 Rust" 而非 "HashMap依赖Rust"）。

### 4.3 去重策略

同一 chunk 内相同 `(subject, relation_type, object)` 三元组只保留一个，使用 `HashSet` 去重。

### 4.4 导入管线集成

在所有 4 条导入路径的 `index_entities()` 后调用 `index_relations()`：
- `index_with_section()`（Markdown 小文件）
- `index_with_text()`（TXT/PDF 小文件）
- `index_md_mmap()`（Markdown 大文件 >50MB）
- `index_txt_mmap()`（TXT 大文件 >50MB）

## 5. 验证结果

| 检查项 | 结果 |
|---|---|
| `cargo test -p echomind-core` | ✅ 355 passed, 0 failed |
| `cargo test -p echomind-infra` | ✅ 125 passed, 0 failed |
| `cargo test -p echomind-models` | ✅ 17 passed, 0 failed |
| `cargo clippy --all-targets -- -D warnings` (Free) | ✅ 零警告 |
| `cargo clippy --all-targets --features pro -- -D warnings` (Pro) | ✅ 零警告 |
| `cargo fmt --check` | ✅ 通过 |
| 现有 `entity_extractor_tests` | ✅ 8 个原有测试不受影响 |

## 6. 后续可扩展方向

1. **图遍历检索接入**：将 `rag.graph_retriever_enabled` 设置键接入 `chat_inner` 检索管线，实现沿图边扩展检索
2. **前端图谱可视化**：使用 G6/D3 渲染 `GraphTriple` 三元组为交互式知识图谱
3. **LLM 增强关系抽取**：对规则无法覆盖的关系，可选调用 LLM 补充抽取（BYOK 模式）
4. **图算法**：PageRank / 社区发现等图算法应用于检索排序
