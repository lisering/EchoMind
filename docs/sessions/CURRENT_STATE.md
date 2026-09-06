# EchoMind 当前项目状态快照

> 本文件是新 AI 会话的第一份必读文件。每次 session 结束时**必须更新**。详细历史见 `HISTORY.md`。
> **最后更新**: 2026-09-06 | **更新人**: AI Agent

---

## 1. 项目健康度

| 指标 | 数值 |
|---|---|
| Rust 测试 (Free) | **1229 passed**, 0 failed, 37 ignored |
| Rust 集成测试 | **25 passed** (新增跨模块协作测试) |
| Rust 测试 (Pro) | 待验证 |
| Clippy / fmt | 零警告 ✅ / 通过 ✅ |
| Vitest (前端) | **1821 passed**, 92 files, 0 failed |
| UI 类型检查 | **0 errors** ✅ |
| E2E 桥接 (Playwright) | **1355 passed**, 0 failed, 10 skipped |
| 版本号 | **3.1.0** ✅ |
| IPC 命令数 | **179 Debug / 169 Release** (唯一 179) |
| 僵尸命令/幻影调用 | **0** / **0** ✅ |
| 前端 | 56 modules / 1584 行 / ~1.02MB JS / ~100KB eager |
| Rust Edition | 2024 (≥1.85) |

---

## 2. 已完成功能摘要

**核心**: 文档导入(10格式)、标签、浅色主题、网页搜索、语音/TTS、分块(token/语义/章节)、向量化(5模型ONNX, bge-m3)、RAG评估、Late Chunking、混合检索(BM25 RRF)、重排序、HyDE、FTS5全文搜索、Contextual Retrieval、流式对话、会话持久化、文件同步、导出(MD/PDF)、License、Free/Pro编译
**安全**: SQLCipher AES-256、Argon2id、状态机+自动锁、8类PII脱敏、哈希链审计、暴力破解防护
**多模态(Pro)**: pdfium渲染、OCR、BYOK VLM、多模态PDF
**性能**: 语义缓存、Prompt压缩、CRAG+Self-RAG、知识图谱、ColBERT、RAPTOR、渐进注入、Speculative RAG
**已Deprecated**: 本地LLM、ReAct Agent、多代理协调、AutoDream/DAG/记忆面板
> 详细功能矩阵+关键文件见 `HISTORY.md §1`。

---

## 3. 待办任务

| # | 问题 | 状态 |
|---|---|---|
| P0-1 | bge-m3 嵌入模型 | ✅ |
| P0-2 | Prompt 注入防护 | ✅ |
| P0-3 | Pro 功能 ROI 评估 | ✅ |
| P0-4 | 持续收入模型 | 📋 商业层面 |
| P0-5 | 数据库回滚 | ✅ |
| P0-6 | MCP 插件 Phase 1 | ✅ |

**S5 行动**: 智能模式开关 / toggle精简(5→2) / 设置重整(35→8) / 术语通俗化 / 幻影修复✅ / 开发者隐藏 → 📋
> 版本规划详情见 `HISTORY.md §2`。

---

## 4. 关键约束

1. **铁律五**: 严禁 `unwrap()/expect()/panic!()`
2. **铁律七**: 变更同步 `docs/api/` + `docs/SOFTWARE_ASSET.md`
3. **铁律十一**: `./sync-to-github.sh`
4. **铁律十二**: Session 结束更新 `CURRENT_STATE.md` + `LOG.md`
5. **依赖方向**: `tauri-app → core → models`，严禁反向
6. **Edition 2024**: 原生 `async fn` in trait
7. **前端构建**: `node scripts/build-ui.mjs`
8. **TypeScript**: `npx tsc --noEmit`
9. **代理**: HF/GitHub 失败 → `export https_proxy=http://127.0.0.1:7890`

---

## 5. 关键文件

| 文件 | 用途 |
|---|---|
| `docs/sessions/HISTORY.md` | 功能矩阵 + 版本规划 + 审计 |
| `docs/SOFTWARE_ASSET.md` | 软件资产全景 |
| `docs/SRS_v1.0.md` | 需求规格 |
| `docs/api/` | IPC/契约/Schema/Settings/Modules |
| `.catpaw/rules/system-constraints.md` | 铁律规则 |
| `AGENTS.md` / `CLAUDE.md` / `PROJECT_RULES.md` | Agent 指南 + 项目宪法 |

---

## 6. 验证命令

```bash
cargo build / cargo build --release --features pro
cargo test / cargo test --features pro
cargo clippy --all-targets -- -D warnings && cargo fmt --check
node scripts/build-ui.mjs && npx tsc --noEmit
cd e2e-tests && npm run test:unit && npm run test:bridge
./sync-to-github.sh
```

---

## 7. Git 状态

- **分支**: `refactor/great-simplification` (基线 `v3.1.0-final`)
- **Phase 1 R3**: 删除 10 个本地 LLM 模块，累计削减 ~37,862 行 (-28.9%)
- **验证**: Free 1326 passed / Clippy 零警告 / fmt 通过

> **新会话**: 详情查 `HISTORY.md`。新功能开发先读 `docs/architecture/` 原子计划或 `docs/SRS_v1.0.md`。
