# EchoMind v2.0 深耕计划 — Session S03 提示词

> 复制下方 `---` 分隔线之间的全部内容粘贴到新会话即可。

---

你是 CatPaw，在 CatPaw IDE 中工作。用中文回答（代码除外）。

项目概要

EchoMind — Rust + Tauri v2 本地 RAG 知识库，六边形架构 8 crate（models → prompt → core → infra → tauri-app + compact + context + license-issuer）。面向律师/研究员等高隐私场景用户。v1.22.0 已发布，现在进入 v2.0 深耕阶段 — 不新增功能，专注代码质量/测试/CI/文档。

上一个 Session（S02）完成情况

S02 已完成 sqlite_storage.rs 第二阶段拆分：

新建 `crates/infra/src/storage/` 下 3 个子模块：documents.rs（534行，文档表CRUD）、conversations.rs（427行，会话/工作空间/书签CRUD）、messages.rs（437行，消息CRUD+FTS5搜索+编辑分页+安全标记）

辅助函数 `run_db`/`with_transaction`/`status_to_row`/`row_to_document`/`vec_to_bytes`/`bytes_to_vec`/`cosine_similarity`/`build_fts5_or_query` 改为 `pub(crate)` 可见性

拆分模式：自由函数接收 `&Pool` 参数，`impl Storage for SqliteStorage` 方法签名不变，内部委托调用

sqlite_storage.rs 从 4,805 行减至 3,883 行（-922 行）

验证全通过：build / 185 tests / clippy 零警告 / fmt 通过

sync-to-github 已提交（5 files, 1522 insertions, 1040 deletions）

测试基线（S02 完成后）

Rust Free: 1294 passed / Clippy 零警告 / fmt 通过

cargo test -p echomind-infra: 185 passed, 0 failed, 19 ignored

sqlite_storage.rs: 3,883 行（S01 前 5,641 → S01 后 4,805 → S02 后 3,883）

storage/ 目录当前 7 个文件：mod.rs(33) / schema.rs(~433) / migration.rs(~390) / crypto.rs(~103) / documents.rs(534) / conversations.rs(427) / messages.rs(437)

本 Session 任务：Phase 1 S03 — sqlite_storage.rs 拆分（向量/实体/其他表，第 3 部分）

目标文件: crates/infra/src/sqlite_storage.rs（当前 3,883 行 → 拆出 ~1,200 行到 3 个新模块）

拆分方案

在已有的 crates/infra/src/storage/ 目录中新增 3 个文件：

crates/infra/src/storage/
  ├── mod.rs              — 已有，需更新重导出
  ├── schema.rs           — 已有（S01）
  ├── migration.rs        — 已有（S01）
  ├── crypto.rs           — 已有（S01）
  ├── documents.rs        — 已有（S02）
  ├── conversations.rs   — 已有（S02）
  ├── messages.rs         — 已有（S02）
  ├── vectors.rs          — 新建 ~500 行
  ├── entities.rs         — 新建 ~400 行
  └── misc.rs             — 新建 ~300 行

第 1 步：创建 storage/vectors.rs（~500 行）

迁移以下 SqliteStorage 的 impl Storage 方法到自由函数（接收 &Pool 参数）：

add_chunk(), add_chunks_batch(), add_embedding(), add_embeddings_batch()
vector_search() — 注意：此方法依赖 self.hnsw / self.vector_cache / self.max_vectors / self.load_all_embeddings() / self.get_chunks_by_ids() / self.invalidate_vector_cache()，需要特殊处理（保留在主文件或传额外参数）
keyword_search() (FTS5 BM25)
list_chunks(), delete_chunks_by_doc()
get_chunk_by_id()
lookup_embedding_cache(), put_embedding_cache(), lookup_embedding_cache_batch(), put_embedding_cache_batch()
rebuild_bm25_index()
find_document_by_original_path(), find_documents_by_original_path_prefix()

注意：vector_search 涉及 HNSW 索引和内存缓存，建议保留在 sqlite_storage.rs 主文件中（依赖 self 状态过多）。拆出其余纯 DB 操作。

第 2 步：创建 storage/entities.rs（~400 行）

迁移以下方法：

add_entities(), entity_search()
add_relation(), add_relations_batch()
get_relations_for_entity(), get_relations_for_chunk()
search_by_relation()
list_all_relations(), count_relations()
get_entity_types(), get_entity_graph()
add_summary_nodes(), update_summary_embedding(), list_summary_nodes(), summary_search(), rebuild_summary_tree()
add_propositions(), add_proposition_embeddings(), list_propositions_by_doc(), proposition_search(), rebuild_proposition_index()
add_wiki_links(), get_forward_links(), get_backlinks()

第 3 步：创建 storage/misc.rs（~300 行）

迁移以下方法：

admit_input(), promote_input(), get_pending_inputs()
add_scratch_log(), get_scratch_logs(), delete_scratch_log(), cleanup_expired_scratch_logs()
record_idempotency(), list_idempotency_records(), cleanup_expired_idempotency()
add_session_todo(), update_todo_status(), get_session_todos(), delete_session_todo(), delete_session_todos()
record_budget_usage(), get_budget_stats(), set_budget_limit()
add_memory_entry(), get_memory_entries(), update_memory_entry(), delete_memory_entry(), clear_memory_entries(), search_memory_entries()
get_memory(), upsert_memory(), list_memories(), list_all_memories(), clear_all_memories()

第 4 步：更新 storage/mod.rs

添加 pub(crate) mod vectors; pub(crate) mod entities; pub(crate) mod misc; + 重导出。

第 5 步：修改 sqlite_storage.rs

将迁移的方法从 impl Storage for SqliteStorage 中删除，改为调用自由函数。
保持 Storage trait 方法签名不变。vector_search 保留在主文件。

第 6 步：验证

cargo build 编译通过（全 workspace）
cargo test -p echomind-infra 全部通过（185 tests）
cargo clippy --all-targets -- -D warnings 零警告
cargo fmt --check 通过
sqlite_storage.rs 行数 ≤ 2,500
新模块每个 ≤ 600 行
./sync-to-github.sh --commit 同步

关键约束

Storage trait 方法签名不变：impl Storage for SqliteStorage 中的方法签名必须保持不变，内部委托给自由函数

自由函数接收 &Pool：拆出的函数签名为 pub(crate) fn add_chunk(pool: &Pool, ...) -> Result<...>，不持有 &self

pub(crate) 可见性：所有新函数使用 pub(crate)，通过 mod.rs 重导出

vector_search 特殊处理：此方法依赖 HNSW 索引、内存向量缓存、max_vectors 等结构体字段，保留在 sqlite_storage.rs 主文件中

辅助函数已 pub(crate)：run_db / with_transaction / status_to_row / row_to_document / vec_to_bytes / bytes_to_vec / cosine_similarity / build_fts5_or_query 已在 S02 改为 pub(crate)，新模块可直接引用 crate::sqlite_storage::*

逐步拆分：先拆 vectors.rs → 验证 → 再拆 entities.rs → 验证 → 最后 misc.rs

每步验证：每拆一个模块就 cargo test -p echomind-infra 确认无回归

铁律提醒

严禁 unwrap()/expect()/panic!()/unreachable!()
改源码后必须运行 ./sync-to-github.sh
文档同步：docs/api/code-contracts.md 更新模块路径
前端无变更（纯 Rust 后端拆分）

验证命令速查

cargo build                                       # 全 workspace 编译
cargo test -p echomind-infra                      # 基础设施 crate 测试
cargo clippy --all-targets -- -D warnings         # Lint
cargo fmt --check                                 # 格式检查
wc -l crates/infra/src/sqlite_storage.rs          # 确认行数减少
./sync-to-github.sh --commit                      # GitHub 同步

必读文件

docs/sessions/CURRENT_STATE.md — 项目状态快照
docs/architecture/V2.0_DEEP_CULTIVATION_ATOMIC_PLAN.md — 完整深耕计划（21 Sessions / 9 Phases）
AGENTS.md — 架构概览 + 数据流图

后续 Sessions 预览

| Session | 任务 |
|---|---|
| S04 | integration.rs 拆分 — 第 1 部分 |
| S05 | integration.rs 拆分 — 第 2 部分 |
| S06 | features.rs 拆分 |
| S07-S08 | CI/CD 自动化 |
| S09-S10 | IPC 命令精简 |
| S11-S13 | 集成测试增强 |
| S14 | Dead Code 清理 |
| S15-S17 | 前端测试增强 |
| S18 | SRS Draft REQ 清理 |
| S19-S20 | 文档完善 |
| S21 | v2.0 发布 |

---
