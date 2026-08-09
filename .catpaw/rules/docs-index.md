# EchoMind 索引 & Docs — 官方文档添加清单

> CatPaw Settings -> 索引 & Docs -> Docs
>
> 当前已添加：`tauri - Rust`
>
> 以下为结合 EchoMind 项目实际依赖，按开发优先级排序的推荐添加清单。
> 优先添加 P0 级文档（开发中最常查阅），再按需添加 P1。

---

## P0 — 开发高频查阅（必加）

| 序号 | 文档名称 | URL | 为什么需要 |
|---|---|---|---|
| 1 | **Rust std** | https://doc.rust-lang.org/std/ | Rust 标准库，开发中最高频查阅 |
| 2 | **Rust Edition 2024** | https://doc.rust-lang.org/edition-guide/ | EchoMind 使用 Edition 2024，async fn in trait 等新特性 |
| 3 | **Tokio** | https://docs.rs/tokio/ | EchoMind 全异步架构，spawn_blocking/Mutex/OnceCell 高频使用 |
| 4 | **SQLite 文档** | https://www.sqlite.org/docs.html | SQLite 是核心存储，PRAGMA/SQL 语法/WAL 模式开发必查 |
| 5 | **rusqlite** | https://docs.rs/rusqlite/ | SQLite Rust 绑定，params/prepare/transaction API |
| 6 | **serde** | https://serde.rs/ | 序列化框架，所有数据模型 derive Serialize/Deserialize |
| 7 | **tracing** | https://docs.rs/tracing/ | 结构化日志，tracing::instrument/span/info/warn 开发高频 |
| 8 | **reqwest** | https://docs.rs/reqwest/ | HTTP 客户端，OpenAI provider/SSE stream/模型下载核心依赖 |

## P1 — 按需添加（特定功能开发时查阅）

| 序号 | 文档名称 | URL | 适用场景 |
|---|---|---|---|
| 9 | **Clippy Lint** | https://rust-lang.github.io/rust-clippy/master/ | 铁律五 deny lint 规则，修复 clippy 警告时查阅 |
| 10 | **Cargo Book** | https://doc.rust-lang.org/cargo/ | workspace/feature/dependencies 配置 |
| 11 | **Tauri v2 Develop** | https://v2.tauri.app/develop/ | IPC command/event/permission 开发 |
| 12 | **Tauri v2 Plugin** | https://v2.tauri.app/plugin/ | dialog/opener 插件配置 |
| 13 | **fastembed** | https://docs.rs/fastembed/ | 嵌入引擎，Embedder/UserDefinedModel |
| 14 | **ONNX Runtime** | https://onnxruntime.ai/docs/ | 底层推理引擎，Pro 功能开发时查阅 |
| 15 | **rusqlite r2d2** | https://docs.rs/r2d2/ | 连接池管理，SqliteStorage 池化 |
| 16 | **argon2** | https://docs.rs/argon2/ | 密码哈希，SecurityManager 加密 |
| 17 | **ed25519-dalek** | https://docs.rs/ed25519-dalek/ | License 签发/验证 |
| 16 | **zeroize** | https://docs.rs/zeroize/ | SecureString 内存擦除 |
| 19 | **tokio-util** | https://docs.rs/tokio-util/ | CancellationToken（abort_chat 核心） |
| 20 | **futures** | https://docs.rs/futures/ | BoxStream/StreamExt（SSE 流处理） |
| 21 | **anyhow** | https://docs.rs/anyhow/ | 错误处理，Context trait 全项目使用 |
| 22 | **pulldown-cmark** | https://docs.rs/pulldown-cmark/ | Markdown 解析，wiki link 解析器使用 |
| 23 | **lopdf** | https://docs.rs/lopdf/ | PDF 文本层提取（标准导入路径） |
| 24 | **memmap2** | https://docs.rs/memmap2/ | GGUF 文件 mmap 零拷贝（Pro） |
| 25 | **notify** | https://docs.rs/notify/ | 文件监听器（Folder Sync 功能） |
| 26 | **tiktoken-rs** | https://docs.rs/tiktoken-rs/ | BPE tokenizer（分块/压缩/token 计数） |
| 27 | **chrono** | https://docs.rs/chrono/ | 时间处理，全项目时间戳 |
| 28 | **uuid** | https://docs.rs/uuid/ | UUID 生成，文档/会话/记忆 ID |
| 29 | **regex** | https://docs.rs/regex/ | PII 检测正则（8 种类型） |
| 30 | **sha2** | https://docs.rs/sha2/ | 审计日志 hash-chain |
| 31 | **aes-gcm** | https://docs.rs/aes-gcm/ | SQLCipher AES-256 加密 |

## P2 — 前端开发时添加

| 序号 | 文档名称 | URL | 适用场景 |
|---|---|---|---|
| 32 | **esbuild** | https://esbuild.github.io/ | 前端打包工具，build-ui.mjs 脚本 |
| 33 | **Tailwind CSS** | https://tailwindcss.com/docs/ | 前端样式（铁律十三强制使用） |
| 34 | **Playwright** | https://playwright.dev/docs/ | E2E 桥接测试 |
| 35 | **Vitest** | https://vitest.dev/ | 前端单元测试 |
| 36 | **marked.js** | https://marked.js.org/ | Markdown 渲染引擎 |
| 37 | **DOMPurify** | https://github.com/cure53/DOMPurify | XSS 防护 |
| 38 | **D3.js** | https://d3js.org/ | 知识图谱可视化 |
| 39 | **Mermaid.js** | https://mermaid.js.org/ | 图表渲染 |
| 40 | **KaTeX** | https://katex.org/docs/ | 数学公式渲染 |

## P2 — Pro 功能开发时添加

| 序号 | 文档名称 | URL | 适用场景 |
|---|---|---|---|
| 41 | **mistral.rs** | https://github.com/EricLBuehler/mistral.rs | 本地 LLM 推理引擎 |
| 42 | **pdfium-render** | https://docs.rs/pdfium-render/ | PDF 页面渲染（多模态提取） |
| 43 | **ocrs** | https://docs.rs/ocrs/ | 本地 OCR 引擎 |
| 44 | **tree-sitter** | https://tree-sitter.github.io/tree-sitter/ | 代码符号抽取 |
| 45 | **hnsw_rs** | https://docs.rs/hnsw_rs/ | HNSW 近似最近邻索引 |
| 46 | **SQLCipher** | https://www.zetetic.net/sqlcipher/ | 数据库加密 |

---

## 添加建议

1. **立即添加 P0 的 1-8 项**（8 个），这是日常开发最高频查阅的文档
2. P1 项在开发对应功能时按需添加
3. P2 项在前端 / Pro 功能开发时添加
4. 添加后 CatPaw 会在后续对话中自动引用这些文档作为上下文
