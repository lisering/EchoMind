# EchoMind 下个 Session 提示词

你是 CatPaw，在 CatPaw IDE 中工作。用中文回答（代码除外）。工作目录：`/Users/john/freesoft/EchoMind`。

## 项目概要

EchoMind — Rust + Tauri v2 本地 RAG 知识库。六边形架构 8 crate：models → prompt → core → infra → tauri-app（+ compact / context / license-issuer）。v2.1.0 已发布，当前 1442 Free / 1733 Pro 测试通过。fastembed ONNX 嵌入（bge-small-en-v1.5，384-dim），SQLite 向量存储 + HNSW 索引（已下沉 Free），任意 OpenAI 兼容 LLM。Pro 功能：本地 LLM 推理（mistral.rs）、GPU 加速、ColBERT、tree-sitter、WasmExecutor。

## 上 Session 完成内容

### v2.1.0 版本发布

| 变更 | 说明 |
|---|---|
| 版本号升级 | 2.0.0 → 2.1.0（8 crate Cargo.toml + tauri.conf.json） |
| CHANGELOG_v2.1.md | 安全加固 + 弹性优化 + 测试增强 |
| SRS changelog v12.6.0 | 发布条目 |

### 测试基线

- Rust Free: **1442** passed, 0 failed, 31 ignored
- Rust Pro: **1733** passed, 0 failed, 48 ignored
- Clippy Free+Pro: 零警告 / cargo fmt: 通过
- tsc: 0 errors / esbuild: 1626 行
- Vitest: 1076 passed, 62 files, 0 failed
- Release build: exit_code 0
- sync-to-github: 8 files committed (15 ins, 15 del) + git tag v2.1.0

### v2.1.0 变更内容总结

1. **安全加固 (P0 修复)**:
   - P0-2: REQ-SEC-021 Prompt 注入防护 — `crates/prompt/src/sanitize.rs` 三层防御（XML 标签隔离 + 防御声明 + 注入模式标记）+ 10 TC-INJECT
   - P0-5: REQ-DB-008 数据库迁移安全防护 — `crates/infra/src/storage/migration.rs` 5 个安全迁移函数 + 10 TC-MIGRATE-SAFE

2. **弹性优化 (P2 行动 5/9)**:
   - P2-1: 文件监听器自动重启（CancellationToken + supervisor）
   - P2-2: 远程 LLM 连续失败 3 次自动切换 Local 模式
   - P2-5: LLM 首 token 超时 60s + 整体 300s
   - P2-6: ONNX 嵌入失败降级到 BM25
   - P2-9: Session 文件整理（92 文件归档）

3. **HNSW 索引下沉 Free** (P2-3):
   - `hnsw_rs` 从 Pro 专属改为 Free 默认
   - `HNSW_AUTO_THRESHOLD = 500`（>500 chunks 自动 HNSW，≤500 全量扫描）
   - 8 TDD 测试 TC-HNSW-FREE-001~008

4. **SRS Draft 归零** (S17):
   - 5 个 Draft REQ 全部实现（对话书签 / prefers-reduced-motion / 网络重试 / 离线模式 / 输入校验）

5. **测试系统深度优化**:
   - Rust +31 测试（duckduckgo 边界 15 + openai_provider 边界 16）
   - Vitest +138 测试 / +10 文件

6. **P1 级行动**:
   - P1-2: 429 限流指数退避重试（7 TC-RATE-LIMIT）
   - P1-3: ADR-004~008 独立归档
   - P1-4: ADR-014 mistral.rs bus factor 监控
   - P1-5: 嵌入模型升级 all-MiniLM-L6-v2 → bge-small-en-v1.5

7. **第三轮 3 次深度复盘**:
   - R1: RAG 质量与 AI 技术演进（2 P0 + 9 P1 + 7 P2）
   - R2: 产品可持续性与生态系统（2 P0 + 9 P1 + 6 P2）
   - R3: 工程质量与未来就绪度（3 P0 + 8 P1 + 8 P2）

## 复盘 P0 问题修复状态

| P0 编号 | 来源 | 问题 | 状态 |
|---|---|---|---|
| P0-1 (R1) | 嵌入模型升级 | all-MiniLM-L6-v2 → bge-small-en-v1.5 已升级，但 bge-m3 1024-dim 可进一步升级 | 🟡 部分修复 |
| P0-2 (R1) | Prompt 注入防护 | 检索片段直接注入 prompt 无防护 | ✅ 已修复 |
| P0-3 (R2) | Pro 功能 ROI | 14 个 Pro 功能中 6 个 ROI 低 | 📋 待评估 |
| P0-4 (R2) | 持续收入模型 | 无订阅/云服务 | 📋 商业层面 |
| P0-5 (R3) | 数据库回滚 | 无迁移前备份/回滚 | ✅ 已修复 |
| P0-6 (R3) | 插件系统 | 无插件/MCP | 📋 架构级 |

## 本 Session 任务：从以下方向选择实施

### 方向 A: P0-1 嵌入模型维度升级（技术驱动，1-2 Session）

**问题**: 当前 bge-small-en-v1.5（384-dim），可升级到 BAAI/bge-m3（1024-dim 多语言）。
**注意**: 维度变化需 DB 迁移（向量内容不兼容）+ HNSW 索引重建。
**关键文件**: `crates/infra/src/local_embedder.rs` + `crates/tauri-app/src/stores/model_store.rs`

### 方向 B: P1 级问题高 ROI 项

从 25 个 P1 问题中选择：
- P1-0 (R1): 有 RAGAS 框架但从未评估真实检索质量 → 构建评估数据集
- P1-1 (R1): 过检索倍数 3 偏小 → 调整为 5-8
- P1-9 (R1): PII 检测未集成到 RAG 管线 → chunk 注入前执行 PII redact
- P1-1 (R3): 日语翻译 31% 缺失 → 补全 443 个缺失翻译键

### 方向 C: P2 剩余项（4/9 待完成）

- P2-4: macOS 代码签名与公证（安全层面，用户体验关键）
- P2-7: 智能模式默认开启的 RAG 参数调优
- P2-8: 嵌入模型下载境内加速
- P2-3: 已完成

### 方向 D: 新功能开发

SRS Draft 已归零，可规划新版本（v2.2）功能路线图。

## 架构速览

```
models → prompt → core → infra → tauri-app
                  ↑
            compact / context
```

依赖方向严格向内：`tauri-app → core → models`，严禁反向。Edition 2024 原生 `async fn` in trait。

### 数据库迁移安全架构

```
init_schema(pool, db_path)
  → 步骤 1: CREATE TABLE IF NOT EXISTS
  → 步骤 2: safe_migrate_schema(pool, db_path)
    → needs_migration? → false: 跳过
    → true: backup_database_file → .bak
    → migrate_schema (ALTER TABLE + CREATE-COPY-DROP-RENAME)
    → run_integrity_check (PRAGMA integrity_check)
    → 失败: restore_from_backup + Err
    → 成功: cleanup_backup (删除 .bak)
  → 步骤 3-6: 索引/FTS/回填/审计
  → 步骤 7: 迁移后完整性检查
```

### Prompt 注入防护架构

```
sanitize.rs (crates/prompt/src/)
  → sanitize_chunk_content(content) → <retrieved_content>...</retrieved_content>
  → sanitize_dynamic_context(sources) → 批量包裹 + 防御声明
  → INJECTION_PATTERNS (32 种中英文模式) → [⚠️ 疑似注入指令] 标记
  → build_rag_prompt_segmented() 调用 sanitize_dynamic_context()
  → build_final_rag_prompt() 调用 sanitize_chunk_content()
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
