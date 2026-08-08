# 性能优化进度交接文档

> 生成于 2026-08-03 会话。新会话接手时先读本文档,再 `git status` 核对实际改动。
> 构建流程:改 `ui/src/*.js` / `ui/styles/*.css` 后执行 `node scripts/build-ui.mjs` 重建 `ui/index.html`;
> Rust 侧 `cargo check --workspace`(默认)与 `cargo check --features pro --workspace`(Pro)双配置验证。

## 一、已完成的性能优化(全部经编译零警告 + 全量测试零失败验证)

### A. 对话出答案速度
| 优化 | 文件 | 说明 |
|---|---|---|
| 查询缓存接入 chat 链路(L0 精确 + L1 语义) | `tauri-app/src/commands.rs` | 相同/语义相似问题直接返回缓存答案(秒回),事件流与落库完整 |
| L3 检索结果缓存接入 | 同上 | 语义相似查询复用上次检索结果,跳过 embed+检索+重排阶段 |
| 缓存按文档级失效(新鲜度感知) | `infra/src/sqlite_cache.rs` `invalidate_by_doc`;commands.rs | 删除文档只失效引用该文档的条目(JSON 引号精确匹配),替代全库清空 |
| compaction 与检索并行 | commands.rs | 标准 RAG 路径下上下文压缩与知识库检索 `tokio::join!` 并行 |
| `chat_with_sources` 预检索入口 | `core/src/chat.rs` + `chat_tests.rs` | 支撑并行化的契约 |
| embedder 启动预热 | `tauri-app/src/lib.rs` | 启动后台加载 ONNX,消除首次对话卡顿 |
| Prompt Caching 深化 | `infra/src/openai_provider.rs` | messages 顺序 static→history→dynamic→user,多轮历史前缀命中 context cache |
| 推理内容(reasoning_content)转发 | `core/src/stream_parse.rs` + `infra/openai_provider.rs` + commands.rs | DeepSeek R1 等思考过程 → `chat_reasoning` 事件 → 前端思考面板 |

### B. 检索精度
| 优化 | 文件 | 说明 |
|---|---|---|
| 层次化重排默认开启 | commands.rs + settings_registry.rs + integration.rs | 过检索 5× 候选 → Cross-Encoder 精排;reranker 初始化失败降级不阻塞对话 |
| HNSW 索引接入检索链路(Pro) | `infra/src/sqlite_storage.rs` | 懒构建 + 脏标记自动重建,大知识库检索 O(log n);非 Pro 构建不受影响 |
| 嵌入模型升级选项 | `infra/src/local_embedder.rs` + `stores/model_store.rs` | 新增 bge-base-en-v1.5(768 维);已有 bge-small-zh-v1.5 / e5-small-v2 |

### C. 文件导入(小/大/超大文件)
| 优化 | 文件 | 说明 |
|---|---|---|
| 流式 MD5 + 内核拷贝 | `core/src/import.rs` | GB 级文件内存从 2× 文件大小降为 64KB 缓冲 |
| 分批入库 | 同上 | chunks/entities/propositions 按 CHUNK_BATCH_SIZE=5000 分批,控制单事务与峰值内存 |
| 文件大小上限提升 | commands.rs | 警告阈值 2GB、硬上限 10GB |
| 正则静态化 | `core/src/entity_extractor.rs` | 10 个正则 OnceLock 编译一次;10MB 管线 366s → 7.6s(48×) |

### D. UI/交互(用户反馈项)
- 侧栏切换零抖动(逐帧视觉锚定,rAF + 锚点补偿)—— `ui/src/sidebar.js` + `e2e-tests/tests/sidebar-jitter.spec.ts`
- 发送/停止按钮合二为一(单按钮形态切换)—— `ui/index.html` + `ui/src/queue-send.js` + `chat.js` + `main.js`;16 个 e2e spec 同步
- 编辑模式按钮可见性/一致性、textarea 撑满外框、输入框 16px、思考面板阶段时间线 + 推理内容—— `message-edit.js` / `thinking-panel.js` / `chat.js` / `components.css`

## 二、验证状态
- `cargo test --workspace` 与 `--features pro`：零失败（默认 882+ / Pro 882+ 全绿）
- `cargo clippy --all-targets -- -D warnings` 与 `--features pro`：零警告（2026-08-03 会话修复了预存 lint：stream_parse `.iter().next()`、commands.rs 多处嵌套 if 折叠、sqlite_storage HNSW `unwrap()`、`prefetched_sources.expect()`）
- e2e 桥接测试（Playwright mock __TAURI__）：802 通过 / 8 跳过 / 0 失败；vitest 单元 347/347 全绿
- 关键性能数据：10MB 文件导入管线 366s → 7.6s；正则静态化为主因；10k chunks 检索 P95 < 500ms（tc_nfr_002）

### 2026-08-03 会话新增
- **P2-1 StepCache 步骤级缓存**（见下）：Agent/Coordinator 中间步骤复用，跳过重复 LLM 调用与嵌入+检索
- **全表扫描 top-k 小顶堆**：`infra/src/sqlite_storage.rs` vector_search 非 HNSW 路径从「收集全部 O(n log n)」改为固定容量 BinaryHeap O(n log k)，内存 O(k)；doc_name Path 提取延迟到堆定型后仅对 top-k 执行
- **HNSW 维度防御修复**：旧 schema 迁移遗留的维度不匹配向量不再使 anndists 构建断言崩溃（tc_db_003 由红转绿）；不匹配向量以 0.0 分追加返回，与全表扫描语义一致
- **思考过程可重现（reasoning 落库）**：`ChatMessage.reasoning` 字段 + messages 表 `reasoning` 列（migrate_schema ALTER TABLE 兼容旧库）+ `spawn_reasoning_forwarder` 收集器 + `persist_exchange` 落库 + 前端 `loadConversation` 历史消息恢复思考面板（TC-IPC-EXP-001 增加 reasoning 往返断言）
- **流式渲染 KaTeX 跳过（前端性能）**：`renderMarkdown` 新增 `skipHeavy` 参数，流式 token 期间跳过 KaTeX 渲染（公式不完整必然失败 + 每帧重复尝试是 O(n²) 热点）；`chat_done` 后 `renderRichContent` 全量渲染一次 + `retryPendingKatex` 兜底。e2e viz/markdown 35 项验证通过
- **UI 测试修复（陈旧断言）**：`message-edit.js` 补 `msg-edit-btn/msg-edit-cancel/msg-edit-resend` 语义类名；`queue-send.test.js` TC-QA-063 断言更新为 stop-mode（发送/停止合二为一）；`thinking-panel.test.js` TC-DS-007 断言匹配 update() 只更新 header 的设计。vitest 从 330/347 → **347/347 全绿**
- 关键文件：`crates/core/src/step_cache.rs`（新）、`crates/core/src/step_cache_tests.rs`（新）、agent.rs、coordinator.rs、sqlite_storage.rs、commands.rs、models/lib.rs、ui/src/main.js、ui/src/markdown.js、ui/src/chat.js、ui/src/message-edit.js

## 三、待办（新会话继续）

### ~~P2-1: StepCache（多步 Agent 步骤级缓存复用）~~ ✅ 已完成（2026-08-03 会话）
- 来源：arXiv 2026-03 "StepCache: Step-Level Reuse with Lightweight Verification and Selective Patching for LLM Serving"
- 实现：
  - 新增 `crates/core/src/step_cache.rs`：`StepCache` 端口 trait + `InMemoryStepCache`（Mutex + FIFO 容量淘汰，默认 256 条）+ `step_cache_key()`（SHA-256 归一化 `tool|input`，大小写/空白差异视为同一查询）
  - `AgentEngine::with_step_cache`：`search_kb` 工具结果（观察文本 + 引用来源）按 `(tool, input)` 缓存复用，跳过嵌入+检索
  - `CoordinatorEngine::with_step_cache`：子查询分解（LLM 调用）、逐子查询检索（嵌入+检索）、综合分析（LLM 调用）三阶段全部缓存
  - 失效：文档导入/删除/手动清缓存时 `step_cache.clear()`；`cache.enabled=false`（隐私模式）不注入缓存
  - 测试：TC-STEPCACHE-001~006（单元）+ TC-AGENT-012 + TC-COORD-008（引擎级复用验证）
- 命中日志：`[PERF] StepCache 命中 search_kb/decompose/synthesis`

### P2-2: NeocorRAG 思路（证据链检索）
- 来源:arXiv 2026 "NeocorRAG: Less Irrelevant Information, More Explicit Evidence"
- 思路:检索结果带证据链,减少无关上下文注入(省 token + 提精度)
- 涉及:`crates/core/src/retriever.rs`、prompt 构建(`crates/prompt`)
- 建议:检索后按证据关联过滤低相关片段,或在 prompt 中标注证据来源层级

### 其他候选(按需)
- 向量二值化/量化(RaBitQ 等 2026 论文)—— 大知识库内存↓32×、速度↑;需评估与 HNSW 组合
- USearch(4k+ stars)替换自研 hnsw_rs —— 对比选型
- 缓存阈值动态校准("Closing the Calibration Gap")—— 当前固定 0.92
- 嵌入批次并行 —— **已否决**(实测多会话分片线程爆炸 66.9s vs 单会话 1-2s,见 local_embedder.rs 注释)

## 四、关键代码位置速查
- chat 命令编排(缓存/并行/检索):`crates/tauri-app/src/commands.rs` chat_inner(约 850-1500 行)
- 缓存实现:`crates/infra/src/sqlite_cache.rs`、`crates/core/src/cache.rs`
- HNSW:`crates/infra/src/hnsw_index.rs` + sqlite_storage.rs vector_search
- 嵌入模型:`crates/infra/src/local_embedder.rs`(EmbeddingModel 枚举 + UserDefinedEmbeddingModel 加载)
- 思考面板:`ui/src/thinking-panel.js`(appendReasoning/appendStage)

## 五、注意事项
- `ui/index.html` 是构建产物,勿手改;改源码后 `node scripts/build-ui.mjs`
- 重排默认开启后,首次对话会触发 bge-reranker 下载(~280MB,懒加载 + 失败降级)
- 切换嵌入模型(`vec.embedding_model`)需重建全部 embeddings(不同模型维度不同)
- HNSW 是 Pro feature,默认构建不含 hnsw_rs 依赖
