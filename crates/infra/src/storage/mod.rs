//! `storage` 子模块：从 `sqlite_storage.rs` 拆分的 schema / migration / crypto 组件。
//!
//! 本模块是 v2.0 深耕 Phase 1 S01 的产物，将 5641 行的 `sqlite_storage.rs`
//! 中的纯 SQL 常量、迁移逻辑和加密辅助函数拆分为独立模块，
//! 便于维护和测试。
//!
//! ## 模块结构
//!
//! | 子模块 | 职责 | 原行数 |
//! |---|---|---|
//! | [`schema`] | SQL DDL 常量（表/索引/FTS/审计）+ 白名单 + 枚举 | ~444 |
//! | [`migration`] | `init_schema` / `migrate_schema` / FTS 回填 / 表名校验 | ~387 |
//! | [`crypto`] | AES-256-GCM 加密/解密 + Unix 文件权限辅助 | ~105 |
//!
//! ## 可见性策略
//!
//! 所有常量和函数使用 `pub(crate)` 可见性——仅 infra crate 内部访问。
//! `SqliteStorage` 通过 `use storage::*` 引入后，在自身 `impl` 块中调用。

pub(crate) mod crypto;
pub(crate) mod migration;
pub(crate) mod schema;

// 重导出：SqliteStorage 主文件通过 `use storage::*` 访问。
pub(crate) use crypto::{decrypt, encrypt, ensure_dir_0700, load_or_create_cipher};
pub(crate) use migration::{Pool, init_schema};
pub(crate) use schema::{DOC_COLS, PRAGMAS};
