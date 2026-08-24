# EchoMind 下个 Session 提示词

你是 CatPaw，在 CatPaw IDE 中工作。用中文回答（代码除外）。工作目录：`/Users/john/freesoft/EchoMind`。

## 项目概要

EchoMind — Rust + Tauri v2 本地 RAG 知识库。六边形架构 8 crate：models → prompt → core → infra → tauri-app（+ compact / context / license-issuer）。v2.1.0 已发布，当前 1498 Free / 1789 Pro 测试通过。fastembed ONNX 嵌入（bge-small-en-v1.5 384-dim 默认 + bge-m3 1024-dim 多语言可选），SQLite 向量存储 + HNSW 索引（已下沉 Free），任意 OpenAI 兼容 LLM。Pro 功能：本地 LLM 推理（mistral.rs）、GPU 加速、ColBERT、tree-sitter、WasmExecutor。

## 上 Session 完成内容

### P1-5 Late Chunking 上下文感知嵌入（REQ-RAG-049）

**问题**: 第三轮复盘 R1 P1-5「缺少 2024-2025 关键 RAG 技术 → Late Chunking / Adaptive Retrieval」。

**实现内容**:

| 变更 | 说明 |
|---|---|
| `crates/core/src/late_chunking.rs` | 新建模块：`build_late_chunking_text(doc_prefix, chunk_content)` 拼接文档前缀摘要 + chunk 内容 + `extract_doc_prefix(text, max_chars)` 提取文档前 N 字符（默认 500）在段落/句子边界截取 + `LateChunkingConfig` 配置结构体（max_prefix_chars=500, separator="\n\n---\n\n"）+ `should_apply()` 条件判断 + `build_late_chunking_text_with_config()` 自定义配置版本 |
| `crates/core/src/late_chunking_tests.rs` | 10 TDD 测试 TC-LATE-CHUNK-001~010（build_late_chunking_text 基本拼接 / extract_doc_prefix 段落边界截取 / 空文本 / 短文本全文 / 与 build_contextual_text 组合 / 嵌入文本变化验证 / Late Chunking + Contextual Retrieval 组合 / 向后兼容 / 长文档截取 / LateChunkingConfig serde 往返） |
| `crates/core/src/lib.rs` | 注册 `late_chunking` 模块 + `late_chunking_tests` 测试模块 |
| `crates/tauri-app/src/commands/mod.rs` | 导入 `build_late_chunking_text` + `extract_doc_prefix` |
| `crates/tauri-app/src/commands/import.rs` | `embed_document_chunks` 集成 Late Chunking（`late_chunking_enabled` 开启时从已入库 chunks 拼接文档全文 → `extract_doc_prefix` 提取前 500 字符 → `build_late_chunking_text` 拼接嵌入文本）+ 与 Contextual Retrieval 可组合（三层：文档前缀 + 文档名 + chunk 内容） |
| `crates/tauri-app/src/commands/performance.rs` | 新增 `set_late_chunking` / `get_late_chunking` IPC 命令 + `set_late_chunking_inner` / `get_late_chunking_inner` 逻辑函数 |
| `crates/tauri-app/src/state.rs` | 新增 `late_chunking_enabled: bool` 字段 + settings 读取 `rag.late_chunking`（默认 false）+ `parse_settings` 元组扩展 |
| `crates/tauri-app/src/lib.rs` | 注册 `set_late_chunking` / `get_late_chunking` 2 IPC 命令 |
| `e2e-tests/tests/bridge/tauri-stub.js` | 新增 `set_late_chunking` / `get_late_chunking` mock + `lateChunking: false` 初始化 |

**核心设计**:
- 借鉴 Jina AI 2024 Late Chunking 技术（arXiv:2409.04701），使 chunk 嵌入向量包含全文语义上下文
- 与 Contextual Retrieval 不同：Late Chunking 拼接文档前 500 字符摘要（非仅文档名），提供更丰富的文档级语义
- 两者可组合：`{doc_prefix}\n\n---\n\n文档《{doc_name}》：\n{chunk_content}`
- 纯规则方案零 LLM 依赖，`extract_doc_prefix` 在段落/句子边界智能截取
- 默认关闭（`rag.late_chunking = false`），用户在性能设置中开启后需 `rebuild_all_embeddings` 重建全库

**测试基线**

- Rust Free: **1498** passed, 0 failed, 32 ignored（+11 新测试）
- Rust Pro: **1789** passed, 0 failed, 49 ignored
- Clippy Free+Pro: 零警告 / cargo fmt: 通过
- tsc: 0 errors / esbuild: 1626 行
- sync-to-github: 8 files committed (437 ins, 4 del)

## 复盘 P0/P1 问题修复状态

| 编号 | 来源 | 问题 | 状态 |
|---|---|---|---|
| P0-1 (R1) | 嵌入模型升级 | bge-m3 1024-dim | ✅ 已修复 |
| P0-2 (R1) | Prompt 注入防护 | 三层防御 | ✅ 已修复 |
| P0-3 (R2) | Pro 功能 ROI | 14 个 Pro 功能 | 📋 待评估 |
| P0-4 (R2) | 持续收入模型 | 无订阅/云服务 | 📋 商业层面 |
| P0-5 (R3) | 数据库回滚 | 迁移安全 | ✅ 已修复 |
| P0-6 (R3) | 插件系统 | 无插件/MCP | 📋 架构级 |
| P1-0 (R1) | RAGAS 真实检索质量评估 | 评估数据集 + 端到端评估 | ✅ 已修复 |
| P1-5 (R1) | 缺少 2024-2025 关键 RAG 技术 | Late Chunking | ✅ 已修复 |

**已修复 4/6 P0 + 2 P1**。

## 本 Session 任务：从以下方向选择实施

### 方向 A: P1 级问题高 ROI 项（推荐）

从 25 个 P1 问题中选择（已完成 P1-0, P1-1, P1-2, P1-5, P1-9）：

**技术驱动高 ROI 项**：
- P1-4 (R1): 缺少 MLX 格式支持 → macOS 优先（mistral.rs 已支持 MLX，需集成）
- P1-6 (R1): 多模型并行对话缺失
- P1-7 (R3): 无 MCP 集成（Model Context Protocol）

**国际化/无障碍项**：
- P1-1 (R3): 日语翻译 31% 缺失 → 补全 443 个缺失翻译键
- P1-2 (R3): 硬编码中文字符串 → 全面国际化
- P1-5 (R3): 无障碍深度审计（屏幕阅读器测试）

**工程合规项**：
- P1-4 (R3): ADR 与代码一致性验证
- P1-1 (R2): 50 文档限制偏低
- P1-2 (R2): License 防破解能力弱
- P1-5 (R2): 无开源许可证
- P1-7 (R2): Windows 剪贴板清除未实现
- P1-8 (R2): Linux 依赖 xclip
- P1-9 (R2): CI 跨平台验证缺失

### 方向 B: P2 剩余项（4/9 待完成）

P2-4: macOS 代码签名与公证（安全层面，用户体验关键）
P2-7: 智能模式默认开启的 RAG 参数调优
P2-8: 嵌入模型下载境内加速

### 方向 C: 新功能开发

SRS Draft 已归零，可规划新版本（v2.2）功能路线图。

### 方向 D: P0 剩余项规划

P0-3（Pro 功能 ROI 评估）、P0-4（商业模型）、P0-6（插件系统）需产品/商业层面决策，可做规划文档。

## 架构速览

```
models → prompt → core → infra → tauri-app
              ↑
        compact / context
```

依赖方向严格向内：`tauri-app → core → models`，严禁反向。Edition 2024 原生 `async fn` in trait。

### Late Chunking 架构（本 Session 新增）

```
Late Chunking 嵌入流程:
  extract_doc_prefix(full_text, max_chars=500)
    → 从文档全文提取前 500 字符
    → 在段落边界（\n\n）或句子边界（。.！！？？\n）截取

  build_late_chunking_text(doc_prefix, chunk_content)
    → {doc_prefix}\n\n---\n\n{chunk_content}
    → 空前缀时返回纯 chunk_content（向后兼容）

  embed_document_chunks 集成:
    late_chunking_enabled = true 时:
      1. 从已入库 chunks 拼接文档全文
      2. extract_doc_prefix 提取前 500 字符
      3. 每个 chunk 的嵌入文本 = build_late_chunking_text(doc_prefix, chunk_content)
      4. 与 Contextual Retrieval 可组合

  IPC 命令:
    set_late_chunking(enabled: bool) → 写入 rag.late_chunking 设置
    get_late_chunking() → 读取 rag.late_chunking 设置
```

### chat_inner 主流程

```
chat IPC → chat_inner()
  → 空库拦截 → 配置检查
  → embedder 初始化（失败降级 BM25）
  → 批量读取 settings → 预计算查询嵌入
  → LlmRouter.resolve() → forward_stream_tracked()
  → 路径分支（coordinator/agent/speculative/standard）
  → forward_stream 两阶段 select!（首 token 60s + 整体 300s）
  → persist_exchange → SQLite 写入
```

## 约束

1. 代理仅限开发调研，软件本身禁止代理（`reqwest` 必须加 `.no_proxy()`）
2. TDD 驱动，先写红灯测试再实现
3. 生产代码严禁 `unwrap()/expect()/panic!()`（workspace deny lint）
4. 代码变更同步更新 `docs/api/` + `docs/SOFTWARE_ASSET.md`
5. 修改源代码后运行 `./sync-to-github.sh --commit`
6. Session 结束更新 `CURRENT_STATE.md` + `LOG.md` + 生成衔接提示词
7. 先写规格再写代码（Spec-driven Development）

## 验证命令

```bash
cargo test 2>&1 | grep -E "^test result:" | awk '{s+=$4;f+=$6;i+=$8} END{print "passed:"s" failed:"f" ignored:"i}'
cargo test --features pro 2>&1 | grep -E "^test result:" | awk '{s+=$4;f+=$6;i+=$8} END{print "passed:"s" failed:"f" ignored:"i}'
cargo clippy --all-targets -- -D warnings
cargo fmt --check
npx tsc --noEmit
node scripts/build-ui.mjs
./sync-to-github.sh --commit
```

## 关键文件索引

| 文件 | 用途 |
|---|---|
| `crates/core/src/late_chunking.rs` | `build_late_chunking_text` / `extract_doc_prefix` / `LateChunkingConfig` |
| `crates/core/src/late_chunking_tests.rs` | 10 TDD 测试 TC-LATE-CHUNK-001~010 |
| `crates/core/src/retriever.rs` | `build_contextual_text`（与 Late Chunking 组合） |
| `crates/tauri-app/src/commands/import.rs` | `embed_document_chunks` Late Chunking 集成 |
| `crates/tauri-app/src/commands/performance.rs` | `set_late_chunking` / `get_late_chunking` IPC |
| `docs/SRS_v1.0.md` | REQ-RAG-049 规格（v13.0.0 changelog） |
| `docs/sessions/CURRENT_STATE.md` | 项目状态快照 |
