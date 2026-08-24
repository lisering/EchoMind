# EchoMind 下个 Session 提示词

你是 CatPaw，在 CatPaw IDE 中工作。用中文回答（代码除外）。工作目录：`/Users/john/freesoft/EchoMind`。

## 项目概要

EchoMind — Rust + Tauri v2 本地 RAG 知识库。六边形架构 8 crate：models → prompt → core → infra → tauri-app（+ compact / context / license-issuer）。v2.0.0 已发布，当前 1432 Free / 1723 Pro 测试通过。fastembed ONNX 嵌入（bge-small-en-v1.5，384-dim），SQLite 向量存储 + HNSW 索引（已下沉 Free），任意 OpenAI 兼容 LLM。Pro 功能：本地 LLM 推理（mistral.rs）、GPU 加速、ColBERT、tree-sitter、WasmExecutor。

## 上 Session 完成内容

### REQ-SEC-021 Prompt 注入防护（第三轮复盘 R1 P0-2 修复）

| 变更 | 说明 |
|---|---|
| `crates/prompt/src/sanitize.rs` | 新建模块，三层防护：内容边界隔离 `<retrieved_content>` + 防御性声明 + 注入模式标记（中英文 32 种模式检测，标记不删除） |
| `crates/prompt/src/lib.rs` | `build_rag_prompt_segmented()` 调用 `sanitize_dynamic_context()` + `build_final_rag_prompt()` 调用 `sanitize_chunk_content()` + `STATIC_ROLE_PREFIX` 角色强化声明 |
| `docs/architecture/adr/ADR-015` | 新建 ADR：三层防御策略 + 备选方案分析 + 局限性 + 安全机制协同 |
| `docs/SRS_v1.0.md` | REQ-SEC-021 规格定义 + changelog v12.4.0 |

## 测试基线

- Rust Free: **1432** passed, 0 failed, 31 ignored（+10 TC-INJECT）
- Rust Pro: **1723** passed, 0 failed, 48 ignored
- Clippy Free+Pro: 零警告 / cargo fmt: 通过
- sync-to-github: 2 files committed (427 ins, 12 del)

## 复盘 P0 问题修复状态

| P0 编号 | 来源 | 问题 | 状态 |
|---|---|---|---|
| P0-1 (R1) | 嵌入模型升级 | all-MiniLM-L6-v2 → bge-small-en-v1.5 已升级，但 bge-m3 1024-dim 可进一步升级 | 🟡 部分修复（默认模型已改为 bge-small-en-v1.5，但 384→1024 维升级需 DB 迁移） |
| **P0-2 (R1)** | **Prompt 注入防护** | **检索片段直接注入 prompt 无防护** | **✅ 已修复（本 Session）** |
| P0-3 (R2) | Pro 功能 ROI | 14 个 Pro 功能中 6 个 ROI 低 | 📋 待评估 |
| P0-4 (R2) | 持续收入模型 | 无订阅/云服务 | 📋 商业层面，非代码 |
| P0-5 (R3) | 数据库回滚 | 无迁移前备份/回滚 | 📋 待实施 |
| P0-6 (R3) | 插件系统 | 无插件/MCP | 📋 架构级 |

## 本 Session 任务：从以下方向选择实施

### 方向 A: P0-5 数据库迁移安全（工程驱动，1 Session）

**问题**: `init_schema()` 只做向前迁移，无回滚。CREATE-COPY-DROP-RENAME 缺事务保护。
**修复**: 迁移前自动备份数据库文件 + 用显式事务包装批量 DDL + 完整性检查失败时阻止启动。
**关键文件**: `crates/infra/src/storage/migration.rs` + `crates/tauri-app/src/lib.rs:51`

### 方向 B: P0-1 嵌入模型维度升级（技术驱动，1-2 Session）

**问题**: 当前 bge-small-en-v1.5（384-dim），可升级到 BAAI/bge-m3（1024-dim 多语言）。
**注意**: 维度变化需 DB 迁移（向量内容不兼容）+ HNSW 索引重建。
**关键文件**: `crates/infra/src/local_embedder.rs` + `crates/tauri-app/src/stores/model_store.rs`

### 方向 C: P1 级问题高 ROI 项

从 25 个 P1 问题中选择：
- P1-0 (R1): 有 RAGAS 框架但从未评估真实检索质量 → 构建评估数据集
- P1-1 (R1): 过检索倍数 3 偏小 → 调整为 5-8
- P1-9 (R1): PII 检测未集成到 RAG 管线 → chunk 注入前执行 PII redact
- P1-1 (R3): 日语翻译 31% 缺失 → 补全 443 个缺失翻译键

### 方向 D: v2.1.0 版本发布

Draft 归零 + P0-2 已修复 + 功能已足够，可发布：版本号 2.0.0→2.1.0 + CHANGELOG + 全量回归 + Release build + GitHub tag。

## 架构速览

```
models → prompt → core → infra → tauri-app
                  ↑
            compact / context
```

依赖方向严格向内：`tauri-app → core → models`，严禁反向。Edition 2024 原生 `async fn` in trait。

### Prompt 注入防护架构（本 Session 新增）

```
build_rag_prompt_segmented()
  → sanitize_dynamic_context(sources)
    → DEFENSE_DECLARATION (防御性声明)
    → for each source:
      → sanitize_chunk_content(content)
        → mark_injection_patterns(content) (注入模式标记)
        → <retrieved_content> 包裹 (边界隔离)
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

### 前端架构

76 个 vanilla JS ES module，esbuild 打包 IIFE → `ui/index.html`。i18n: `ui/locales/{en,zh-CN,ja}.json`。

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
