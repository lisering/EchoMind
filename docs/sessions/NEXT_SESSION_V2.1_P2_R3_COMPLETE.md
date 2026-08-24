# EchoMind v2.1 P2 行动 R3 衔接提示词（完整自包含版）

你是 CatPaw，在 CatPaw IDE 中工作。用中文回答（代码除外）。工作目录：`/Users/john/freesoft/EchoMind`。

## 项目概要

EchoMind — Rust + Tauri v2 本地 RAG 知识库，六边形架构 8 crate（models / prompt / compact / core / infra / context / tauri-app / license-issuer），v2.0.0 已发布。fastembed ONNX 嵌入（all-MiniLM-L6-v2 / bge-small-en-v1.5，384-dim），SQLite 向量存储，任意 OpenAI 兼容 LLM。Features: SQLCipher AES-256 加密、PII 检测/脱敏、审计哈希链日志、自动锁屏。Pro 功能：本地 LLM 推理（mistral.rs v0.9.0 GGUF）、GPU 加速、HNSW 索引、ColBERT 多向量、tree-sitter 代码符号、WasmExecutor 代码沙箱。

## 上两 Session 完成内容

### P2 行动 R1（前 3 项）

| 项 | 内容 | 关键改动 | 状态 |
|---|---|---|---|
| P2-1 | 文件监听器自动重启 | `file_watcher.rs`: `CancellationToken` + 3 次退避重启（1s/2s/4s）+ `tracing` 日志替代 `eprintln!` | ✅ |
| P2-5 | LLM 首 token 超时 | `chat.rs`: `FIRST_TOKEN_TIMEOUT_SECS=60` + `forward_stream` 两阶段 `select!`（首 token 60s 超时 + 后续 300s 整体兜底）+ 3 TDD 测试 | ✅ |
| P2-6 | ONNX 嵌入失败降级 | `chat.rs`: `embedding_degraded` 标志 → 降级到关键词搜索（BM25）+ 降级提示 + 跳过 L1/L3 缓存 | ✅ |

### P2 行动 R2（后 2 项）

| 项 | 内容 | 关键改动 | 状态 |
|---|---|---|---|
| P2-2 | LLM 无降级策略 | `llm_router.rs`: `remote_failure_count: Arc<AtomicUsize>` + `REMOTE_FAILURE_THRESHOLD=3` + `record_remote_failure()`（达阈值自动 `set_fallback(Local)`）+ `record_remote_success()`（重置计数）+ `remote_failure_count()` 查询。`chat.rs`: `forward_stream_tracked()` 包装器替换 4 处 `forward_stream` 调用（embedding_degraded / coordinator / agent / standard 路径）+ `llm_fallback` 事件发射。`chat.js`: `llm_fallback` 事件监听 + toast warning。7 TDD 测试 TC-FALLBACK-001~007 | ✅ |
| P2-9 | Session 文件整理 | 92 个旧文件归档到 `docs/sessions/archive/`（50 个 `NEXT_SESSION_*.md` + 16 个 `RETROSPECTIVE/SESSION` 文件 + 26 个已有 archive 文件）。保留 4 个活跃文件（CURRENT_STATE / LOG / README / HANDOFF_TEMPLATE）+ 5 个最近 NEXT_SESSION 文件 | ✅ |

## 测试基线（P2 R2 完成后）

- Rust Free: **1373** passed, 0 failed, 31 ignored（+7 fallback 测试）
- Rust Pro: **1669** passed, 0 failed, 48 ignored
- Clippy Free+Pro: 零警告 / cargo fmt: 通过
- tsc: 0 errors / esbuild: 1626 行
- Vitest: 938 passed, 52 files / E2E: 1355 passed, 10 skipped
- sync-to-github: 6 files committed (269 insertions, 26 deletions)

## P2 行动总览（已完成 5/9 项）

| 项 | 状态 | 说明 |
|---|---|---|
| P2-1 文件监听器自动重启 | ✅ | CancellationToken + 3 次退避重启 |
| P2-2 LLM 无降级策略 | ✅ | 远程失败 3 次 → 自动切换 Local |
| P2-3 向量搜索 O(n) 优化 | 📋 待定 | HNSW 接入默认路径（2 Sessions） |
| P2-4 知识图谱渲染 | 📋 暂缓 | 低频场景，暂不紧急 |
| P2-5 LLM 首 token 超时 | ✅ | 60s 首 token + 300s 整体兜底 |
| P2-6 ONNX 嵌入失败降级 | ✅ | 降级到 BM25 关键词搜索 |
| P2-7 重复依赖版本 | 📋 持续优化 | 非阻塞 |
| P2-8 Vanilla JS 架构 | 📋 长期规划 | 非短期 |
| P2-9 Session 文件整理 | ✅ | 92 个文件归档 |

## 本 Session 任务：下个方向选择

P2 行动已完成全部推荐项（5/9），剩余 4 项均为暂缓或持续优化。以下两个方向可选其一：

### 方向 A: P2-3 — 向量搜索 O(n) 优化（2 Sessions，较大）

**问题**: 向量检索全量扫描 O(n)，大知识库（>10k chunks）慢。
**修复**: HNSW 索引接入默认检索路径（Pro 已有 `hnsw_index.rs`，需接入 Free 默认路径）。
**关键文件**: `crates/infra/src/hnsw_index.rs`（已有 Pro 实现）, `crates/infra/src/sqlite_storage.rs`（`vector_search` 全量扫描）, `crates/core/src/retriever.rs`（`VectorRetriever`）
**设计考量**:
- HNSW 索引当前 Pro 门控，需评估是否下沉到 Free
- 索引构建在嵌入写入时触发（增量 vs 全量重建）
- 小知识库（<1k chunks）全量扫描更快，HNSW 有建图开销
- 需要阈值切换：chunks > N 时用 HNSW，否则全量扫描

### 方向 B: 新功能开发（S19+ Draft REQ Implement 候选）

转向新功能开发。Draft REQ 候选见 `docs/SRS_v1.0.md` 中 Draft 状态的 REQ。

## 架构速览

```
crates/models  →  crates/prompt  →  crates/core  →  crates/infra  →  crates/tauri-app
  (contracts)     (prompts)       (ports+logic)  (adapters)     (assembly)
                                     ↑
                              crates/compact
                              crates/context
                              (compaction/epoch)
```

依赖方向严格向内：`tauri-app → core → models`，严禁反向。`core` 严禁依赖 Tauri/reqwest/rusqlite。

### 关键端口 Trait（crates/core/src/lib.rs）

| Trait | 用途 |
|---|---|
| `Loader` | 文件 → 纯文本（md/txt/pdf/docx/html/pptx/epub/xlsx/csv） |
| `Splitter` | 文本 → 有序 chunks |
| `Embedder` | 文本 → f32 向量（单条 + 批量） |
| `Storage` | CRUD documents/chunks/embeddings/conversations/messages/settings；向量搜索；FTS5 关键词搜索 |
| `LLMProvider` | SSE 流式对话 → `BoxStream<Result<String>>` + 4 个辅助方法（generate_title/one_shot/judge/context_token_budget） |
| `Retriever` | 查询 → top-k `RetrievalResult`（支持 `retrieve_with_embedding` 复用预计算嵌入） |
| `Reranker` | Cross-Encoder 重排序（bge-reranker-base） |
| `QueryRewriter` | HyDE 查询改写 |
| `PromptCompressor` | 上下文压缩（RuleBasedCompressor / OnnxCompressor Pro） |
| `ResponseCache` | 三级语义缓存（L0 精确/L1 语义/L3 检索结果） |

### LLM 路由器（crates/core/src/llm_router.rs）

- `LlmRouter` — 管理 Remote/Local 模式路由，per-conversation 模式追踪
- `resolve(conversation_id, requested)` → `(LlmChoice, RouterVerdict)`
- `set_fallback(choice)` — 更新默认选择（`set_llm_mode` 调用时更新）
- **P2-2 新增**: `record_remote_failure()` — 连续失败 3 次自动切换 fallback 到 Local
- **P2-2 新增**: `record_remote_success()` — 重置失败计数
- `remote_failure_count: Arc<AtomicUsize>` — 无锁原子计数器

### chat_inner 主流程（crates/tauri-app/src/commands/chat.rs）

```
chat IPC → chat_inner()
  → 空库拦截 → 配置检查
  → P2-6: embedder 初始化（失败降级到 BM25 关键词搜索）
  → 批量读取 settings（1 次 DB 查询替代 20+ 次）
  → 预计算查询嵌入（复用于 L1/L3 缓存 + 向量检索）
  → LlmRouter.resolve() → 获取 LlmChoice
  → P2-2: forward_stream_tracked() 包装器跟踪成功/失败
  → 路径分支：coordinator / agent / speculative / standard RAG
  → P2-5: forward_stream 两阶段 select!（首 token 60s + 整体 300s）
  → persist_exchange → SQLite 写入
```

### 前端架构

- **源码**: `ui/src/*.js`（76 个 ES module，vanilla JS，无框架）
- **构建**: `node scripts/build-ui.mjs`（esbuild 打包 IIFE + 内联 CSS → `ui/index.html` 单文件）
- **类型检查**: `npx tsc --noEmit`（tsconfig.json + types/*.d.ts）
- **i18n**: `ui/locales/{en,zh-CN}.json`
- **样式**: Tailwind CSS 预构建（36KB，无 CDN）+ `ui/styles/tokens.css`（CSS 变量三层架构）
- **事件监听**: `listen('event_name', callback)` from `./ipc.js`

## 约束

1. **铁律一**: 网络代理仅限开发调研，软件本身禁止代理（所有 `reqwest::Client::builder()` 必须加 `.no_proxy()`）
2. **铁律二**: TDD 驱动 — 先写红灯测试（对应 AC），再实现变绿；禁空断言
3. **铁律五**: 生产代码严禁 `unwrap()/expect()/panic!()/unreachable!()/todo!()/unimplemented!()`（workspace deny lint 强制）
4. **铁律七**: 代码变更同步更新 `docs/api/` 对应文档 + `docs/SOFTWARE_ASSET.md`
5. **铁律十一**: 修改源代码后运行 `./sync-to-github.sh --commit`
6. **铁律十二**: Session 结束更新 `CURRENT_STATE.md` + `LOG.md` + 生成衔接提示词
7. **铁律十五**: Spec-driven Development — 先写规格再写代码
8. **依赖方向**: `tauri-app → core → models`，严禁反向
9. **Edition 2024**: 原生 `async fn` in trait，无 `#[async_trait]` 宏
10. **前端构建**: `node scripts/build-ui.mjs`（esbuild 打包 IIFE，不要手改 `ui/index.html`）

## 关键文件索引

| 文件 | 用途 |
|---|---|
| `docs/sessions/CURRENT_STATE.md` | 项目状态快照（必读，会话开始第一文件） |
| `docs/SRS_v1.0.md` | 需求规格唯一依据（REQ-<域>-<序号> 格式） |
| `docs/api/ipc-commands.md` | IPC 命令签名（~233 Debug / ~212 Release） |
| `docs/api/code-contracts.md` | Trait 签名 + 数据模型 + AppState 字段 |
| `docs/api/database-schema.md` | 数据库表结构 |
| `docs/api/settings-reference.md` | 设置键 + Tauri 事件 |
| `.catpaw/rules/system-constraints.md` | 铁律规则（21 条全文） |
| `crates/core/src/llm_router.rs` | LLM 路由器（P2-2 改动） |
| `crates/tauri-app/src/commands/chat.rs` | chat 主逻辑（P2-2/P2-5/P2-6 改动） |
| `crates/infra/src/hnsw_index.rs` | HNSW 索引（P2-3 候选，Pro 门控） |
| `crates/infra/src/sqlite_storage.rs` | SQLite 存储（vector_search 全量扫描） |
| `crates/core/src/retriever.rs` | VectorRetriever（P2-3 候选） |
| `AGENTS.md` | AI Agent 指南（架构 + 数据流图） |
| `sync-to-github.sh` | GitHub 发布同步脚本 |

## 验证命令

```bash
# 编译
cargo build

# 测试
cargo test 2>&1 | grep -E "^test result:" | awk '{s+=$4;f+=$6;i+=$8} END{print "passed:"s" failed:"f" ignored:"i}'
cargo test --features pro 2>&1 | grep -E "^test result:" | awk '{s+=$4;f+=$6;i+=$8} END{print "passed:"s" failed:"f" ignored:"i}'

# Lint
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --features pro -- -D warnings
cargo fmt --check

# 前端
npx tsc --noEmit
node scripts/build-ui.mjs

# 同步
./sync-to-github.sh --commit
```
