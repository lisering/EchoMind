# EchoMind 下个 Session 提示词

你是 CatPaw，在 CatPaw IDE 中工作。用中文回答（代码除外）。工作目录：`/Users/john/freesoft/EchoMind`。

## 项目概要

EchoMind — Rust + Tauri v2 本地 RAG 知识库。六边形架构 8 crate：models → prompt → core → infra → tauri-app（+ compact / context / license-issuer）。v2.1.0 已发布。fastembed ONNX 嵌入，SQLite 向量存储 + HNSW 索引，任意 OpenAI 兼容 LLM。Pro 功能：本地 LLM 推理（mistral.rs）、GPU 加速、ColBERT、tree-sitter、WasmExecutor。

## 上 Session 完成内容

### P1-1 R3: 日语翻译完整补全（i18n）

| 变更 | 说明 |
|---|---|
| `ui/locales/ja.json` | 从 68.9% → **100%** 覆盖率，补全全部 443 个缺失键 |
| 翻译范围 | 53 个顶层 section，1424 个键值对全部翻译 |
| 新增 section | paywall, command_palette, global_search, markdown, doc_phases, presets, menu, export, errors, backend, time, ctx, security, db_error, download, graph, code_executor, workflow, memory, dream, symbol, perf, voice, web_search, export_pdf, wiki, tree, session_strip, trace, durable, todo, kv_cache, keyboard_help, error_recovery, kb_stats, help, date, export_html, about, window_settings, error_logs, startup_settings, breadcrumb, workspace |
| 验证 | tsc 0 errors ✓ / esbuild 1626 行 ✓ / linter 0 errors ✓ |

### 测试基线

- tsc: 0 errors
- esbuild: 1626 行，构建成功
- Linter: ja.json 无错误
- Clippy: echomind-infra 有 pre-existing 编译错误（hnsw_path 字段缺失），与 i18n 改动无关

### 技术要点

1. **翻译方法论**: 使用 Python 脚本将 en.json 结构与扁平化的翻译字典合并，生成结构完全匹配的 ja.json
2. **100% 覆盖**: 所有 1424 个 en.json 键都有对应的日语翻译，无 fallback 到英语
3. **术语一致性**: 保持品牌名（EchoMind）、技术缩写（LLM, API, Token, RAG, ONNX, VLM, PII）的跨语言一致性
4. **日语本地化**: 使用自然的日语表达（です/ます 调），保持 UI 文案的简洁性

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
