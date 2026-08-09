# EchoMind（灵犀）编码模式

> 本文件总结 EchoMind 项目中反复使用的编码模式和约定，AI Agent 开发时必须遵循。

---

## 1. `*_inner` 模式（IPC 命令与测试复用）

每个 Tauri IPC 命令拆分为两层：`#[tauri::command]` 包装器 + `*_inner` 逻辑函数。

```rust
// ✅ 正确：包装器 + inner 分离
#[tauri::command]
pub async fn get_documents(state: State<'_, AppState>) -> Result<Vec<Document>, String> {
    get_documents_inner(state.inner()).await
}

pub async fn get_documents_inner(state: &AppState) -> Result<Vec<Document>, String> {
    let storage = state.storage.clone();
    tokio::task::spawn_blocking(move || storage.list_documents())
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}
```

**为什么**：集成测试（`tests/integration.rs`）使用 `*_inner` 函数，传入 mock `&AppState`，无需 Tauri 运行时。

---

## 2. `#[cfg(feature = "pro")]` 条件编译门控

Pro 功能通过编译时 feature flag 门控，Free 版编译时完全移除 Pro 代码。

```rust
// crates/core/Cargo.toml + crates/infra/Cargo.toml
[features]
default = []
pro = ["dep:pdfium-render", "dep:ocrs", "dep:mistralrs", ...]

// 代码中
#[cfg(feature = "pro")]
pub struct AuditEngine { ... }

#[cfg(not(feature = "pro"))]
pub fn activate_pro_inner(_state: &AppState, _key: &str) -> Result<(), String> {
    Err("Pro feature not available in free version".to_string())
}
```

**关键**：IPC 命令层始终注册（Free 版返回 PRO_REQUIRED 错误），前端不区分编译。

---

## 3. 错误处理（铁律五强制）

**禁止**：`unwrap()`, `expect()`, `panic!()`, `unreachable!()`, `todo!()`, `unimplemented()`

```rust
// ✅ 正确：返回 Result，错误映射为 String
pub async fn import_files_inner(state: &AppState, paths: Vec<String>) -> Result<(), String> {
    let storage = state.storage.clone();
    spawn_blocking(move || storage.count_documents())
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

// ✅ 正确：溢出安全
let idx = checked_add(1).ok_or("index overflow")?;
let val = saturating_add(1);

// ❌ 错误：workspace deny lint 强制失败
let val = unwrap();  // compile error!
let idx = expect("should exist");  // compile error!
```

测试代码通过 `#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]` 豁免。

---

## 4. 异步 Trait（Edition 2024 原生）

```rust
// ✅ 正确：原生 async fn in trait，无 #[async_trait]
pub trait LLMProvider: Send + Sync {
    async fn chat_stream(
        &self,
        system_prompt: &str,
        history: &[ChatMessage],
        query: &str,
    ) -> Result<BoxStream<'static, Result<String>>>;
}

// ✅ 默认方法（可选覆盖）
async fn generate_title(&self, _answer: &str) -> Option<String> { None }
async fn one_shot(&self, _prompt: &str) -> Result<Option<String>> { Ok(None) }
async fn judge(&self, _prompt: &str) -> Result<Option<String>> { Ok(None) }
```

**注意**：`#![allow(async_fn_in_trait)]` 是有意的豁免。所有 Trait 有 `Send + Sync` supertrait 约束。

---

## 5. 对象安全 Trait（`Pin<Box<dyn Future>>`）

需要 `dyn` 的 Trait 使用 `Pin<Box<dyn Future>>` 返回类型：

```rust
// ✅ 正确：对象安全
pub trait Reranker: Send + Sync {
    fn rerank(
        &self,
        query: &str,
        results: &[RetrievalResult],
    ) -> Pin<Box<dyn Future<Output = Result<Vec<RetrievalResult>>> + Send>>;
}

pub trait SecurityScreener: Send + Sync {
    fn classify(&self, text: &str)
        -> Pin<Box<dyn Future<Output = Option<SecurityScreenVerdict>> + Send>>;
}
```

**为什么**：`async fn` in trait 生成 `impl Future`，不支持 `dyn`。`Pin<Box<dyn Future>>` 是对象安全替代方案。

---

## 6. SQLite 操作（spawn_blocking + r2d2 池）

所有 SQLite 操作必须通过 `tokio::task::spawn_blocking`，不阻塞 async executor：

```rust
// ✅ 正确：spawn_blocking + pool clone
let pool = self.pool.clone();
tokio::task::spawn_blocking(move || {
    let conn = pool.get().map_err(|e| e.to_string())?;
    conn.execute("INSERT INTO ...", params![...]).map_err(|e| e.to_string())?;
    Ok(())
})
.await
.map_err(|e| e.to_string())?
```

向量存储为 BLOB（raw f32 little-endian bytes），余弦相似度在 Rust 中全表扫描计算。

---

## 7. 流式 SSE 前进器（CancellationToken）

```rust
// forward_stream 循环：select! biased 分支
loop {
    tokio::select! {
        biased;
        _ = cancel_token.cancelled() => {
            emit("⏹ 生成已中断");
            break;
        }
        item = stream.next() => {
            match item {
                Some(Ok(token)) => { emit_chat_token(&token); buffer.push_str(&token); }
                Some(Err(e)) => { emit_chat_error(&e); break; }
                None => break,
            }
        }
    }
}
emit_chat_done();
persist_exchange().await;  // 保留已生成内容
```

---

## 8. 前端模块化（esbuild IIFE）

- 源码在 `ui/src/*.js`（50 个 ES module）
- 构建产物 `ui/index.html` 是 esbuild 打包的 IIFE（126K 行）
- **不要手改 `ui/index.html`**，修改源码后运行 `node scripts/build-ui.mjs`
- TypeScript 类型声明在 `types/*.d.ts`（tauri / vendor / dom-ext）
- Tailwind CSS 本地 JIT 构建（`ui/styles/tokens.css` 三层变量体系），**禁止 CDN**
- 铁律十三：用 Tailwind utility classes + design tokens，自定义 CSS 仅用于 `@keyframes`/伪元素/第三方覆盖

---

## 9. 文档同步（铁律七）

代码变更必须同步更新文档：

| 代码变更 | 必须同步的文档 |
|---|---|
| 新增/修改 IPC 命令 | `docs/api/ipc-commands.md` |
| 新增/修改数据库表 | `docs/api/database-schema.md` |
| 新增/修改 Trait 方法 | `docs/api/code-contracts.md` |
| 新增前端 JS 文件 | `docs/api/frontend-modules.md` |
| 新增 settings 键 | `docs/api/settings-reference.md` |
| 功能实现 | `docs/SRS_v1.0.md` REQ 状态 |
| 架构决策 | `docs/architecture/adr/` 新建 ADR |

---

## 10. KeyedQueue 并发控制

`persist_exchange` 按会话 ID 序列化，同会话串行，不同会话并行：

```rust
// state.rs
persist_queue: KeyedQueue<String>

// chat.rs
state.persist_queue
    .run(conversation_id.to_string(), || async {
        persist_exchange_inner(state, conversation_id, ...).await
    })
    .await
```

---

## 11. Settings 键命名

```
llm.api_key / llm.base_url / llm.model / llm.mode / llm.local_model
cache.enabled / cache.ttl_secs / cache.semantic_threshold / cache.privacy_mode
compression.ratio
rag.hybrid_search / rag.agent_enabled / rag.progressive_injection
rag.speculative_enabled / rag.retrieval_memory_enabled / rag.web_search_enabled
rag.contextual_retrieval / rag.graph_retriever_enabled
security.posture
vec.embedding_model
ui.theme
export.pdf_page_size / export.pdf_include_sources
```

---

## 12. IPC 命令命名

```
动词_名词: get_documents / delete_document / import_files / abort_chat
set/get 配对: set_hybrid_enabled / get_hybrid_enabled
Pro 门控: activate_pro / get_pro_status
*_inner 后缀: 所有逻辑函数
```
