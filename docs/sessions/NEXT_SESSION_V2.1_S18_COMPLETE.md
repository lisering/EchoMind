# EchoMind v2.1 S18 衔接提示词（完整自包含版）

你是 CatPaw，在 CatPaw IDE 中工作。用中文回答（代码除外）。工作目录：`/Users/john/freesoft/EchoMind`。

## 项目概要

EchoMind — Rust + Tauri v2 本地 RAG 知识库，六边形架构 8 crate（models / prompt / compact / core / infra / context / tauri-app / license-issuer），v2.0.0 已发布。fastembed ONNX 嵌入（all-MiniLM-L6-v2 / bge-small-en-v1.5，384-dim），SQLite 向量存储，任意 OpenAI 兼容 LLM。Features: SQLCipher AES-256 加密、PII 检测/脱敏、审计哈希链日志、自动锁屏。Pro 功能：本地 LLM 推理（mistral.rs v0.9.0 GGUF）、GPU 加速、HNSW 索引、ColBERT 多向量、tree-sitter 代码符号、WasmExecutor 代码沙箱。

## 上 Session 完成内容

### S17 — 5 个 Draft REQ 全部实现（SRS Draft 归零 5→0）

| REQ | 内容 | 关键改动 | 状态 |
|---|---|---|---|
| REQ-RAG-047 | 对话书签与收藏夹 | `context-menu.js` AC-1 书签菜单项 + `bookmarks.js` 完整面板 + 后端 4 IPC 已就绪 | ✅ |
| REQ-DS-006 AC-6 | prefers-reduced-motion | `tokens.css` `@media (prefers-reduced-motion: reduce)` 已在 v1.15 实现，AC-6 状态修正 | ✅ |
| REQ-ERR-002 AC-1~5 | 网络错误自动重试 | `openai_provider.rs` 5xx + 超时重试 + `with_retry_notifier` + `compute_server_error_backoff_delay` + `chat.js` retrying phase + i18n + 5 TDD 测试 | ✅ |
| REQ-ERR-003 AC-1~5 | 离线模式降级 | `offline-detector.js` 已在 v1.13 完整实现，AC-1~5 状态修正 | ✅ |
| REQ-ERR-005 AC-7~9 | 输入校验 | `settings.rs` API Key ≤256 + Base URL 格式 + `chat.rs` 查询 ≤8000 已实现，AC-7~9 状态修正 | ✅ |

### 遗留 TODO 已全部清理

- ✅ SRS 5 个 Draft REQ/AC 状态全部更新为 Implemented
- ✅ CURRENT_STATE.md 已更新（S17 信息在第 7 行和第 15 行）
- ✅ LOG.md 已追加 S17 记录
- ✅ sync-to-github.sh 已同步（4 files, 402 insertions, 246 deletions）

## 测试基线（S17 完成后）

- Rust Free: **1378** passed, 0 failed, 31 ignored（+5 retry 测试 TC-RETRY-001~005）
- Rust Pro: **1674** passed, 0 failed, 48 ignored（+5 retry 测试）
- Clippy Free: 零警告 ✅
- cargo fmt: 通过 ✅
- tsc: 0 errors ✅
- esbuild: 1626 行 ✅
- Vitest: 938 passed, 52 files, 0 failed
- E2E: 1355 passed, 0 failed, 10 skipped
- sync-to-github: 4 files committed (402 ins, 246 del)

## 项目状态总结

### 已完成的所有复盘 + 行动

| 复盘轮次 | 状态 | P0 | P1 | P2 |
|---|---|---|---|---|
| 第一轮（S1-S4）| ✅ | 0 | — | — |
| 第二轮验收评估（S1-S7）| ✅ | 0 | 6/6 完成 | 4/4 完成 |
| 第三轮超大规模复盘 | ✅ 8.5/10 | 0 | — | — |
| v2.0 深耕（S1-S18）| ✅ | 0 | — | — |
| 第二轮 R1（产品视角）| ✅ 8.1/10 | 2 (UX-1/UX-2) | 6 | 7 |
| 第二轮 R2（技术架构）| ✅ 8.1/10 | 0 | 5 (全部完成) | 8 |
| 第二轮 R3 P1 行动 | ✅ | 0 | 5/5 完成 | — |
| 第二轮 R3 P2 行动 | ✅ | 0 | — | 5/5 完成 |
| S17 Draft REQ 实现 | ✅ | 0 | — | — |

### R1 P0 问题现状

| P0 | 描述 | 代码层面状态 | 说明 |
|---|---|---|---|
| UX-1 | macOS 未签名/未公证 | ⚠️ 非代码问题 | 需 Apple Developer 证书，是发布流程问题 |
| UX-2 | HuggingFace 模型下载超时 | ✅ 已解决 | `local_embedder.rs` 已实现 3 源容错（ModelScope → hf-mirror → HuggingFace），中文系统优先 ModelScope |

### P2 行动总览（已完成 5/9 项）

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

SRS Draft 已归零，所有已知 P0/P1 问题已解决。以下是可选方向：

### 方向 A: P2-3 — 向量搜索 O(n) 优化（2 Sessions，较大）

**问题**: 向量检索全量扫描 O(n)，大知识库（>10k chunks）慢。
**修复**: HNSW 索引接入默认检索路径（Pro 已有 `hnsw_index.rs`，需接入 Free 默认路径）。
**关键文件**: `crates/infra/src/hnsw_index.rs`（已有 Pro 实现）, `crates/infra/src/sqlite_storage.rs`（`vector_search` 全量扫描）, `crates/core/src/retriever.rs`（`VectorRetriever`）
**设计考量**:
- HNSW 索引当前 Pro 门控，需评估是否下沉到 Free
- 索引构建在嵌入写入时触发（增量 vs 全量重建）
- 小知识库（<1k chunks）全量扫描更快，HNSW 有建图开销
- 需要阈值切换：chunks > N 时用 HNSW，否则全量扫描

### 方向 B: 新功能开发

转向新功能开发。需要先在 `docs/SRS_v1.0.md` 中定义新的 Draft REQ + AC + TC，然后实施。
候选功能领域（基于 R1 复盘用户反馈）：
- **无 Key 演示模式** — 首次使用无需 API Key 即可体验基础功能（R1 P1 UX-3）
- **文件夹拖拽导入** — 支持拖拽文件夹批量导入（R1 P1 UX-4）
- **Agent 步骤可视化增强** — ReAct 步骤前端展示完善（R1 P1 UX-6）
- **PDF 导入质量提升** — Free 版 PDF 导入文本提取增强（R1 P1 F1）

### 方向 C: v2.1 版本发布

当前 SRS Draft 归零 + 所有 P0/P1 已解决，可以考虑发布 v2.1.0 版本：
- 版本号升级 2.0.0 → 2.1.0
- CHANGELOG_v2.1.md 编写
- 全量回归测试
- Release build
- GitHub tag

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
| `LLMProvider` | SSE 流式对话 → `BoxStream<Result<String>>` + 4 个辅助方法 |
| `Retriever` | 查询 → top-k `RetrievalResult` |
| `Reranker` | Cross-Encoder 重排序 |
| `QueryRewriter` | HyDE 查询改写 |
| `PromptCompressor` | 上下文压缩 |
| `ResponseCache` | 三级语义缓存 |

### LLM 路由器（crates/core/src/llm_router.rs）

- `LlmRouter` — 管理 Remote/Local 模式路由
- `resolve()` → `(LlmChoice, RouterVerdict)`
- `record_remote_failure()` — 连续失败 3 次自动切换 Local
- `record_remote_success()` — 重置失败计数

### chat_inner 主流程（crates/tauri-app/src/commands/chat.rs）

```
chat IPC → chat_inner()
  → 空库拦截 → 配置检查
  → embedder 初始化（失败降级到 BM25）
  → 批量读取 settings
  → 预计算查询嵌入（复用缓存 + 检索）
  → LlmRouter.resolve() → 获取 LlmChoice
  → forward_stream_tracked() 包装器跟踪成功/失败
  → 路径分支：coordinator / agent / speculative / standard RAG
  → forward_stream 两阶段 select!（首 token 60s + 整体 300s）
  → persist_exchange → SQLite 写入
```

### 前端架构

- **源码**: `ui/src/*.js`（76 个 ES module，vanilla JS，无框架）
- **构建**: `node scripts/build-ui.mjs`（esbuild 打包 IIFE + 内联 CSS → `ui/index.html` 单文件）
- **类型检查**: `npx tsc --noEmit`
- **i18n**: `ui/locales/{en,zh-CN}.json`
- **样式**: Tailwind CSS 预构建（36KB）+ `ui/styles/tokens.css`

## 约束

1. **铁律一**: 网络代理仅限开发调研，软件本身禁止代理
2. **铁律二**: TDD 驱动 — 先写红灯测试，再实现变绿
3. **铁律五**: 生产代码严禁 `unwrap()/expect()/panic!()` 等
4. **铁律七**: 代码变更同步更新 `docs/api/` + `docs/SOFTWARE_ASSET.md`
5. **铁律十一**: 修改源代码后运行 `./sync-to-github.sh --commit`
6. **铁律十二**: Session 结束更新 `CURRENT_STATE.md` + `LOG.md` + 生成衔接提示词
7. **铁律十五**: Spec-driven Development — 先写规格再写代码
8. **依赖方向**: `tauri-app → core → models`，严禁反向
9. **Edition 2024**: 原生 `async fn` in trait
10. **前端构建**: `node scripts/build-ui.mjs`（不要手改 `ui/index.html`）

## 关键文件索引

| 文件 | 用途 |
|---|---|
| `docs/sessions/CURRENT_STATE.md` | 项目状态快照（必读） |
| `docs/SRS_v1.0.md` | 需求规格唯一依据 |
| `docs/api/ipc-commands.md` | IPC 命令签名 |
| `docs/api/code-contracts.md` | Trait 签名 + 数据模型 |
| `docs/api/database-schema.md` | 数据库表结构 |
| `docs/api/settings-reference.md` | 设置键 + 事件 |
| `.catpaw/rules/system-constraints.md` | 铁律规则（21 条） |
| `crates/core/src/llm_router.rs` | LLM 路由器 |
| `crates/tauri-app/src/commands/chat.rs` | chat 主逻辑 |
| `crates/infra/src/hnsw_index.rs` | HNSW 索引（Pro 门控） |
| `crates/infra/src/sqlite_storage.rs` | SQLite 存储 |
| `crates/infra/src/local_embedder.rs` | ONNX 嵌入引擎（含多源容错下载） |
| `crates/core/src/retriever.rs` | VectorRetriever |
| `AGENTS.md` | AI Agent 指南 |
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
