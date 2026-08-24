# 下一个 Session 提示词

> 将此文件内容作为新会话的第一条用户消息发送。

---

你是 CatPaw，在 CatPaw IDE 中工作。用中文回答（代码除外）。工作目录：/Users/john/freesoft/EchoMind。

## 项目概要

EchoMind — Rust + Tauri v2 本地 RAG 知识库。六边形架构 8 crate：models → prompt → core → infra → tauri-app（+ compact / context / license-issuer）。v2.1.0 已发布，当前 1457 Free / 1748 Pro 测试通过。fastembed ONNX 嵌入（bge-small-en-v1.5，384-dim），SQLite 向量存储 + HNSW 索引（已下沉 Free），任意 OpenAI 兼容 LLM。Pro 功能：本地 LLM 推理（mistral.rs）、GPU 加速、ColBERT、tree-sitter、WasmExecutor。

## 上 Session 完成内容

### P1-9 PII 脱敏集成到 RAG 管线（REQ-SEC-022）

变更说明：
- `crates/prompt/src/sanitize.rs` 新增 PII 脱敏作为 prompt 安全防护第四层（在注入标记之前执行）
- `redact_pii()` + `detect_pii()` + 7 类 PII 检测（邮箱/手机号/身份证/IP/国际手机号/SSN/护照）
- 7 个脱敏函数 + `PiiRedaction` 结构体
- `sanitize_chunk_content()` 新增步骤 1 PII 脱敏
- 独立于 `crates/core::privacy` 实现（因 `prompt` crate 不可依赖 `core`），正则模式与 `privacy.rs` 保持一致
- 新增 `regex = "1.11"` 依赖
- 10 TDD 测试 TC-PII-RAG-001~010

### P1-1 过检索倍数调优

- `crates/core/src/hybrid_retriever.rs` `OVER_RETRIEVE_FACTOR` 从 3 提升到 5
- 扩大 RRF 融合候选池
- 5 TDD 测试 TC-ORETRIEVE-001~005

### 文档同步

- `docs/api/code-contracts.md`：HybridRetriever 签名更新（新增 reranker/rewriter/mmr_config/quality_gate_config 字段）+ sanitize.rs 函数文档 + OVER_RETRIEVE_FACTOR=5
- `docs/SOFTWARE_ASSET.md`：sanitize.rs 模块信息 + 两层 PII 防护说明 + 关键文件索引
- `docs/SRS_v1.0.md`：v12.7.0 changelog + REQ-SEC-022 Implemented

## 测试基线

- Rust Free: 1457 passed, 0 failed, 31 ignored
- Rust Pro: 1748 passed, 0 failed, 48 ignored
- Clippy Free: 零警告 / cargo fmt: 通过
- tsc: 0 errors / esbuild: 1626 行
- sync-to-github: 5 files committed (619 ins, 14 del)

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

问题: 当前 bge-small-en-v1.5（384-dim），可升级到 BAAI/bge-m3（1024-dim 多语言）。
注意: 维度变化需 DB 迁移（向量内容不兼容）+ HNSW 索引重建。
关键文件: `crates/infra/src/local_embedder.rs` + `crates/tauri-app/src/stores/model_store.rs`

### 方向 B: P1 级问题高 ROI 项

从 25 个 P1 问题中选择（已完成 P1-1 和 P1-9）：
- P1-0 (R1): 有 RAGAS 框架但从未评估真实检索质量 → 构建评估数据集
- P1-2 (R1): 检索质量门控仅观测不阻断 → 考虑低质量时增强重试
- P1-4 (R1): 缺少 MLX 格式支持 → macOS 优先
- P1-5 (R1): 缺少 2024-2025 关键 RAG 技术 → Contextual Retrieval 已有，考虑 Late Chunking
- P1-6 (R1): 多模型并行对话缺失
- P1-1 (R3): 日语翻译 31% 缺失 → 补全 443 个缺失翻译键
- P1-2 (R3): 硬编码中文字符串 → 全面国际化
- P1-4 (R3): ADR 与代码一致性验证
- P1-5 (R3): 无障碍深度审计（屏幕阅读器测试）
- P1-7 (R3): 无 MCP 集成
- P1-1 (R2): 50 文档限制偏低
- P1-2 (R2): License 防破解能力弱
- P1-5 (R2): 无开源许可证
- P1-7 (R2): Windows 剪贴板清除未实现
- P1-8 (R2): Linux 依赖 xclip
- P1-9 (R2): CI 跨平台验证缺失

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

依赖方向严格向内：tauri-app → core → models，严禁反向。Edition 2024 原生 `async fn` in trait。

## Prompt 安全防护架构（四层防御）

```
sanitize.rs (crates/prompt/src/)
  → sanitize_chunk_content(content)
    → 步骤 1: redact_pii(content) — PII 脱敏（REQ-SEC-022）
    → 步骤 2: mark_injection_patterns(redacted) — 注入模式标记
    → 步骤 3: <retrieved_content>...</retrieved_content> 边界包裹
  → sanitize_dynamic_context(sources) → 批量包裹 + 防御声明
  → INJECTION_PATTERNS (32 种中英文模式) → [⚠️ 疑似注入指令] 标记
  → redact_pii() / detect_pii() — 7 类 PII 检测 + 脱敏
  → build_rag_prompt_segmented() 调用 sanitize_dynamic_context()
  → build_final_rag_prompt() 调用 sanitize_chunk_content()
```

## chat_inner 主流程

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

- 代理仅限开发调研，软件本身禁止代理（reqwest 必须加 `.no_proxy()`）
- TDD 驱动，先写红灯测试再实现
- 生产代码严禁 `unwrap()/expect()/panic!()`（workspace deny lint）
- 代码变更同步更新 `docs/api/` + `docs/SOFTWARE_ASSET.md`
- 修改源代码后运行 `./sync-to-github.sh --commit`
- Session 结束更新 `CURRENT_STATE.md` + `LOG.md` + 生成衔接提示词
- 先写规格再写代码（Spec-driven Development）

## 验证命令

```bash
cargo test  2>&1 | grep -E "^test result:" | awk '{s+=$4;f+=$6;i+=$8} END{print "passed:"s" failed:"f" ignored:"i}'
cargo test --features pro  2>&1 | grep -E "^test result:" | awk '{s+=$4;f+=$6;i+=$8} END{print "passed:"s" failed:"f" ignored:"i}'
cargo clippy --all-targets -- -D warnings
cargo fmt --check
npx tsc --noEmit
node scripts/build-ui.mjs
./sync-to-github.sh --commit
```
