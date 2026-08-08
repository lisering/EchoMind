//! Store 模式（借鉴 Zed `buffer_store`/`git_store`/`lsp_store` 架构）。
//!
//! 将 `AppState` 巨型结构体按职责拆分为 5 个独立 Store，每个 Store 持有
//! 共享 `SqliteStorage` 的 Arc 引用，可独立测试、减少锁竞争。
//!
//! 拆分对照表：
//! | Store | 职责 | 原 AppState 字段 |
//! |---|---|---|
//! | `DocumentStore` | 文档管理 + 导入取消 + 文件监听 | `import_cancel`, `file_watchers` |
//! | `ChatStore` | 会话中断令牌 + 审计取消 | `abort_tokens`, `audit_cancels` |
//! | `SecurityStore` | 安全管理器 + 剪贴板清除 | `security`, `clipboard_guard` |
//! | `ModelStore` | 嵌入/重排/本地LLM/模型管理 | `embedder`, `reranker`, `model_manager`, `local_llm`, `page_renderer`, `ocr_engine` |
//! | `ConfigStore` | LLM 配置 + Pro 授权 + LLM 模式 | `llm_config`, `is_pro`, `llm_mode` |

pub mod chat_store;
pub mod config_store;
pub mod document_store;
pub mod model_store;
pub mod security_store;

pub use chat_store::ChatStore;
pub use config_store::ConfigStore;
pub use document_store::DocumentStore;
pub use model_store::{ModelStore, parse_embedding_model};
pub use security_store::SecurityStore;
