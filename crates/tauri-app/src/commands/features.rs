//! features 域 IPC 命令薄入口（S06 SRP 重构：2534 行 → 12 子模块）。
//!
//! 实际函数分布在 `features/` 子目录的 12 个功能域文件中。
//! `pub use` 重导出保持 `lib.rs` `generate_handler!` 不变。

mod code_exec;
mod custom_model;
mod skill;
mod stt;
mod symbol;
mod template;
mod trace;
mod web_search;
mod wiki;

pub use code_exec::*;
pub use custom_model::*;
pub use skill::*;
pub use stt::*;
pub use symbol::*;
pub use template::*;
pub use trace::*;
pub use web_search::*;
pub use wiki::*;
