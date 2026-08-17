//! `storage` 子模块：从 `sqlite_storage.rs` 拆分的 CRUD 组件。
//!
//! 本模块是 v2.0 深耕计划的产物，将 5641 行的 `sqlite_storage.rs`
//! 中的纯 SQL 常量、迁移逻辑、加密辅助函数和数据表 CRUD 操作拆分为独立模块，
//! 便于维护和测试。
//!
//! ## 模块结构
//!
//! | 子模块 | 职责 | 原行数 |
//! |---|---|---|
//! | [`schema`] | SQL DDL 常量（表/索引/FTS/审计）+ 白名单 + 枚举 | ~444 |
//! | [`migration`] | `init_schema` / `migrate_schema` / FTS 回填 / 表名校验 | ~387 |
//! | [`crypto`] | AES-256-GCM 加密/解密 + Unix 文件权限辅助 | ~105 |
//! | [`documents`] | 文档表 CRUD：写入/状态/查重/统计/列表/标签/导入日志/工作空间 | S02 新增 |
//! | [`conversations`] | 会话表 CRUD：创建/列表/分页/删除/标题/排序/工作空间/书签 | S02 新增 |
//! | [`messages`] | 消息表 CRUD：写入/列表/分页/批量删除/编辑分页/安全标记/FTS5 | S02 新增 |
//! | [`vectors`] | 分块/向量/嵌入缓存/FTS5 关键词检索/BM25 重建/路径查找 | S03 新增 |
//! | [`entities`] | 实体/关系/摘要/命题/wiki 链接/代码符号 CRUD | S03 新增 |
//! | [`misc`] | 待处理输入/scratch/幂等性/todo/budget/检索记忆 CRUD | S03 新增 |
//!
//! ## 可见性策略
//!
//! 所有常量和函数使用 `pub(crate)` 可见性——仅 infra crate 内部访问。
//! `SqliteStorage` 通过 `use storage::*` 引入后，在自身 `impl` 块中调用。

pub(crate) mod conversations;
pub(crate) mod crypto;
pub(crate) mod documents;
pub(crate) mod entities;
pub(crate) mod messages;
pub(crate) mod migration;
#[cfg(test)]
pub(crate) mod migration_tests;
pub(crate) mod misc;
pub(crate) mod schema;
pub(crate) mod vectors;

// 重导出：SqliteStorage 主文件通过 `use storage::*` 访问。
pub(crate) use crypto::{
    decrypt, decrypt_bytes, encrypt, encrypt_bytes, ensure_dir_0700, load_or_create_cipher,
};
pub(crate) use migration::{Pool, init_schema};
pub(crate) use schema::PRAGMAS;
