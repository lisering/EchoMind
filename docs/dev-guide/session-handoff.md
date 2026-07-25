# 开发会话交接文档

> **最后更新**：2026-07-25（v7：Phase 3 ContextualEmbedding 落地）
> **用途**：新 AI 会话开始时，读取此文件快速恢复全部上下文。

---

## 一、项目当前状态

### 1.1 总体进度

| 指标 | 数值 |
|------|------|
| 需求规格 | `docs/SRS_v1.0.md` v1.1.5（含 VIZ + MM 域 + 前端接线） |
| 项目宪法 | `PROJECT_RULES.md` v1.4.0（10 条铁律，见 `.catpaw/rules/iron-rules.md`） |
| 测试策略 | `docs/TEST_STRATEGY.md` v1.2.5 |
| 资产管理 | `docs/ASSET_MANAGEMENT.md` v1.1.0 |
| 全量编译 | ✅ `cargo check` 通过（2.88s） |
| Clippy | ✅ `cargo clippy --all-targets -- -D warnings` 零警告（4.40s） |
| CI | `.github/workflows/e2e.yml` |

### 1.2 铁律体系（10 条）

完整定义见 `.catpaw/rules/iron-rules.md`。PROJECT_RULES.md v1.4.0 登记了铁律一~八，`.catpaw/rules/iron-rules.md` 已扩展至铁律九~十：

| 铁律 | 内容 | 状态 |
|------|------|------|
| 一 | 网络代理设置（127.0.0.1:7890） | ✅ |
| 二 | TDD 驱动开发（先红灯后绿灯） | ✅ |
| 三 | 代码注释规范（Rust `///` + JS JSDoc） | ✅ |
| 四 | 真实数据测试（真实语料 + 混沌 + LLM 环境变量） | ✅ |
| 五 | 生产级代码质量（零 unwrap/panic/裸溢出） | ✅ |
| 六 | 最新版本依赖原则 | ✅ fastembed 4.0→5.17.3 已升级；pdfium-render v0.9.3 + ocrs v0.12.2 + rten v0.24.0 |
| 七 | 文档实时同步 | ✅ 本轮已修复 |
| 八 | 开发决策必须互联网搜索 | ✅ |
| 九 | Rust 生态优先原则 | ✅ |
| 十 | 最前沿技术追踪（Semantic Chunking / HyDE / Reranking 等） | ✅ |

### 1.3 规则资产合并进度（✅ 全部完成）

从 `/Users/john/rs-pro` 项目吸收规则资产，8 个 Phase 全部完成：

| Phase | 内容 | 状态 |
|---|---|---|
| Phase 1 | PROJECT_RULES.md 铁律一~八 + iron-rules.md | ✅ |
| Phase 2 | AGENTS.md + CLAUDE.md | ✅ |
| Phase 3 | Git Hooks (pre-commit + commit-msg) + ASSET_MANAGEMENT | ✅ |
| Phase 4 | docs/dev-guide/ 五篇文档 | ✅ |
| Phase 5 | deny.toml + supply-chain.md + dependabot.yml | ✅ |
| Phase 6 | architecture/overview.md + ADR 体系（ADR-001/002/003/009/010） | ✅ |
| Phase 7 | docs/api/ 三核心契约文档（code-contracts/ipc-commands/database-schema） | ✅ |
| Phase 8 | CONTRIBUTING.md + CI 增强 | ✅ |

### 1.4 API 契约文档（铁律七）

| 文档 | 内容 | 状态 |
|---|---|---|
| `docs/api/code-contracts.md` | Trait 签名 + 数据模型 + AppState + 适配器 + core 层实现（含 SemanticSplitter） | ✅ |
| `docs/api/ipc-commands.md` | 15 个 Tauri Command 签名 + 6 个事件 + `*_inner` 测试复用 | ✅ |
| `docs/api/database-schema.md` | SQLite 表结构 | ✅ |
| `docs/api/frontend-modules.md` | ui/index.html 模块划分 + 函数职责 + 渲染时序 | ✅ |
| `docs/api/settings-reference.md` | settings 键 + Tauri 事件 + 加密存储 | ✅ |

---

## 二、已交付特性

### 2.1 核心功能（P0 全部完成）

- ✅ 文档导入（拖拽 + 文件选择器，md/txt/pdf）
- ✅ MD5 去重 + 格式白名单 + 路径遍历防护
- ✅ 文本解析与分块（TextSplitter 512/64 + SemanticSplitter 256/32）
- ✅ 本地向量化（fastembed all-MiniLM-L6-v2 384维，多源下载容错）
- ✅ SQLite 持久化（WAL + r2d2 连接池 + 崩溃恢复）
- ✅ RAG 检索（VectorRetriever top-k=5 + 阈值 0.35 + 空上下文拒答）
- ✅ 流式对话（SSE + chat_token/chat_sources/chat_done 事件）
- ✅ 会话持久化（conversations/messages 表 + 标题自动提取）
- ✅ 流中断（CancellationToken + 部分内容保留落库）
- ✅ BYOK 配置（OpenAI 兼容 + Ollama + AES-256-GCM 加密）
- ✅ License 校验（Ed25519 离线签名 + is_pro 门控）
- ✅ 付费墙（50 文件限制 + PDF Pro 专属 + Modal 引导）

### 2.2 VIZ 富内容可视化（ADR-009）

| Phase | 内容 | 状态 | 测试 |
|---|---|---|---|
| Phase 1 | Mermaid.js 图表引擎（12+ 种图表） | ✅ 完成 | TC-VIZ-001~006 全绿（5/5） |
| Phase 2 | KaTeX 数学公式（+ mhchem 化学方程式） | ✅ 完成 | TC-VIZ-005/005b/005c/005d 全绿（4/4） |
| Phase 3 | Chart.js 数据图表（表格→柱/折/饼图） | ✅ 完成 | TC-VIZ-007/007b 全绿（2/2） |
| Phase 4+ | G6/D3/Three.js 专业领域 | 远期 | — |

### 2.3 MM 多模态内容理解（ADR-010）

| Phase | 内容 | 状态 |
|---|---|---|
| Phase 1 | PDF 页面渲染 + 本地 OCR | ✅ 端口 + 适配器已完成（PageRenderer + OcrEngine + PdfiumRenderer + OcrsEngine）|
| Phase 2 | BYOK VLM 图片理解（表格→MD，甘特→Mermaid） | ✅ 完成（端口 + 适配器 + L2 测试 + **前端接线**：set_vlm_enabled 命令 + SettingsPayload.vlm_enabled + vision_provider 运行时开关 + 设置面板 + 隐私确认弹窗 + TC-MM-005/006 全绿）|
| Phase 3 | 多模态导入管线编排 | ✅ 完成（MultimodalPdfLoader + ImportService.index_document_multimodal + 前端 sub_phase + L2 契约测试 TC-MM-001/002/004 全绿）|
| Phase 4 | CLIP 多模态嵌入 / 本地 VLM | 远期 |

---

## 三、待办任务（按优先级排序）

### P0: 铁律合规修复

1. ✅ **依赖升级完成**：`fastembed = "4.0"` → v5.17.3（铁律六），API 变更已适配（`embed()` 需 `&mut self`，用 `Mutex` 包裹修复）
2. ✅ 文档同步（已完成：code-contracts 补充 SemanticSplitter + PageRenderer/OcrEngine 端口、frontend-modules/settings-reference 创建、session-handoff 修订）
3. ✅ SRS 状态修正（REQ-VIZ-001/004 → Done；REQ-MM-001/002 → In-Dev）

### P1: VIZ Phase 3（Chart.js 数据图表）

- ✅ Chart.js v4.5.1 已下载到 `ui/vendor/`
- ✅ `index.html` 已集成 `enhanceTablesForChart`（检测 Markdown 表格 → 「📊 切换图表视图」按钮）
- ✅ E2E 测试 TC-VIZ-007/007b 已在 `viz.spec.ts` 实现，全绿（AC-1/AC-2/AC-3）

### P1: MM Phase 3 收尾（多模态导入管线 L2 测试补齐）✅ 完成

- ✅ Phase 1 已完成：端口 `PageRenderer` + `OcrEngine` + 适配器 `PdfiumRenderer` + `OcrsEngine`
- ✅ Phase 3 核心实现已完成：`MultimodalPdfLoader`（crates/core/src/loader.rs）+ `ImportService.index_document_multimodal`（crates/core/src/import.rs）+ 前端 sub_phase 进度 + AppState 惰性初始化 + commands.rs `index_pdf_multimodal()` 编排 + 降级回退
- ✅ 现有测试：tc_mm_001b（fake PDF 容错）+ tc_mm_001c（不存在文件错误），cargo test 47/47 全绿
- ✅ **L2 契约测试已补齐**：TC-MM-001（PDF 页面渲染为位图）、TC-MM-002（本地 OCR 提取图片文字）、TC-MM-004（多模态导入管线图文混排可检索）—— 全部绿灯
- ✅ SRS 状态已同步：REQ-MM-001/002/004 → Done
- 参考：`/Users/john/rs-pro/crates/core/src/ocr.rs`（完整 LocalOcrEngine 实现）

### P1: MM Phase 2 前端接线 ✅ 完成

- ✅ `set_vlm_enabled(enabled: bool)` IPC 命令 → 写入 settings 表 `mm.vlm_enabled`，`vision_provider()` 下次调用即时生效
- ✅ `SettingsPayload` 新增 `vlm_enabled: bool` 字段，`get_settings_inner` 读取返回
- ✅ `state.rs`：移除 `OnceCell<Option<OpenAIVisionProvider>>` 缓存，`vision_provider()` 改为每次读取 settings 返回 owned `Option<OpenAIVisionProvider>`，支持运行时开关
- ✅ `commands.rs index_pdf_multimodal`：接入 `index_document_multimodal_with_vlm` + VLM provider（None 时用 NoVlm 降级）
- ✅ 前端 `ui/index.html`：新建设置 Modal（LLM 配置展示 + VLM Toggle 开关 + 隐私提示文案）+ VLM 隐私确认弹窗（开启时弹出）+ settingsBtn 接线到 `openSettings()`
- ✅ L2 契约测试：TC-MM-005（VLM 开关持久化与重启恢复）+ TC-MM-006（vision_provider 运行时开关响应，含 OnceCell 移除验证）— 全部绿灯
- ✅ cargo test -p echomind-core -p echomind-tauri-app -p echomind-models → 27/27 全绿
- ✅ cargo clippy --all-targets -- -D warnings → 零警告
- ✅ 文档同步：ipc-commands（15 个命令 + §2.10 set_vlm_enabled）、code-contracts（SettingsPayload.vlm_enabled + vision_provider 签名）、settings-reference（mm.vlm_enabled 运行时切换）、frontend-modules（§2.10 设置面板）、SRS v1.1.5、TEST_STRATEGY v1.2.5

---

## 三、待办任务（按优先级排序）

### P0: 立即收尾

1. ✅ **依赖升级完成**：`fastembed = "4.0"` → v5.17.3（铁律六），API 变更已适配
2. ✅ 文档同步（已完成）
3. ✅ SRS 状态修正

### P1: E2E 测试 + 预存问题修复

1. ✅ **E2E 测试：设置面板 + VLM 开关 + 隐私弹窗** — E2E-MM-001~009 全部绿灯（9/9 passed）。新增 `e2e-tests/tests/settings.spec.ts`，覆盖设置面板打开/关闭 + LLM 配置展示 + VLM 开关初始状态 + 隐私确认弹窗弹出/取消/确认 + VLM 关闭直接生效 + 状态持久化恢复 + LLM 配置修改入口。文档已同步：E2E_COVERAGE v1.1.0 §2.7、TEST_STRATEGY v1.2.6
2. ✅ **pdfium 动态库红灯已修复** — 根因不是「缺动态库」（那是预期状态，生产靠 Tauri resource 打包），而是 `Pdfium::default()` 内部 `bind_to_system_library().unwrap()` 找不到库时 panic。已改为 `Pdfium::new(Pdfium::bind_to_system_library()?)` 显式返回 `Err`（铁律五零 panic）。3 个 pdfium 测试全绿，无需下载动态库即可跑通（均为错误路径测试）。真实渲染测试若未来需要，从 bblanchon/pdfium-binaries 下载 `pdfium-mac-arm64.tgz` 解压到 `target/debug/`（见 `pdfium_renderer.rs::get_pdfium` 注释）

### P1: Draft 需求推进

SRS 中多个 P1 需求仍为 Draft 状态，需逐个推进至 Approved → Contract-Frozen → In-Dev → Done：

| 需求 | 优先级 | 状态 | 内容 |
|------|--------|------|------|
| `REQ-ING-003` | P1 | Draft | 导入格式扩展 |
| `REQ-VEC-004` | P1 | Draft | 向量优化 |
| `REQ-RAG-008` | P1 | ✅ Approved（Phase 1 已落地） | 回答语言自适应（`build_rag_prompt` + `LANG_GUIDANCE`，TC-RAG-006 全绿）；**注意**：SemanticSplitter 分块优化也已落地（`crates/core/src/semantic_splitter.rs`，20 单元测试全绿，已接线 `ImportService.index_with_text`），契约登记 `code-contracts §5.1` |
| `REQ-UI-009` | P1 | Draft | UI 功能 |
| `REQ-NFR-001` | P1 | Draft | 冷启动性能 |
| `REQ-NFR-002` | P1 | Draft | 检索性能 |
| `REQ-NFR-003` | P1 | Draft | 平台覆盖（跨平台 CI） |

> **RAG 增强下一步候选**（基于调研 [[memory:17849201661656590178]]）：
> - ✅ Phase 4：Chunk Expansion **已落地**（`VectorRetriever.expand_neighbors`，TC-RAG-009 绿灯，检索命中后扩展前后各 1 个相邻 chunk，去重 + 按 sequence 排序）
> - ✅ 补齐：SemanticSplitter L1 集成测试 **已完成**（TC-VEC-002 绿灯，端到端分块入库契约）
> - ✅ Phase 3：ContextualEmbedding **已落地**（`build_contextual_text` 拼接文档名前缀，零 LLM 成本规则方案，TC-RAG-010 绿灯，`commands.rs embed_document_chunks` 已接入）

### P2: 远期

- VIZ Phase 4+：G6/D3/Three.js 专业领域可视化
- MM Phase 4：CLIP 多模态嵌入 / 本地 VLM

## 四、关键架构上下文

### 4.1 项目结构

```
EchoMind/
├── crates/
│   ├── models/          # 领域模型（Document, Chunk, ChatMessage 等）
│   ├── core/            # 业务逻辑（6 端口 + SemanticSplitter + ChatEngine + ImportService）
│   ├── infra/           # 适配器（SQLite, fastembed, OpenAI）
│   └── tauri-app/       # Tauri 外壳，15 IPC 命令
├── ui/                  # 前端单文件 SPA（本地打包，禁止 CDN）
│   ├── index.html
│   └── vendor/          # mermaid + katex + marked + purify + highlight + tailwind
├── docs/
│   ├── SRS_v1.0.md      # 需求规格（含 VIZ + MM 域）
│   ├── TEST_STRATEGY.md
│   ├── ASSET_MANAGEMENT.md
│   ├── api/             # 5 篇契约文档
│   ├── architecture/    # overview + ADR 体系
│   ├── dev-guide/       # 本文档 + conventions + testing 等
│   └── security/        # license + supply-chain
├── e2e-tests/           # Playwright E2E
├── .catpaw/rules/       # 铁律规则（10 条）
├── .githooks/           # Git Hooks
└── PROJECT_RULES.md     # 项目宪法 v1.4.0
```

### 4.2 测试运行命令

| 操作 | 命令 |
|------|------|
| 全量后端测试 | `cargo test` |
| 单 crate 测试 | `cargo test -p echomind-core` |
| 集成测试 | `cargo test -p tauri-app --test integration` |
| E2E 测试 | `cd e2e-tests && ECHOMIND_E2E_TEST=1 npx playwright test` |
| 全量编译 | `cargo check` |
| Lint | `cargo clippy --all-targets -- -D warnings` |
| 格式检查 | `cargo fmt --check` |

### 4.3 核心数据流

**文档导入**: `import_files` → `ImportService.import_one()` → `Loader.load()` → MD5 去重 → `Splitter.split()` → `Storage.add_chunk()` → `Embedder.embed_batch()` → `Storage.add_embedding()`

**RAG 查询**: `chat` → 空库拦截 → `chat_phase "preparing"` → 初始化 embedder → `ChatEngine.chat()` → `VectorRetriever.retrieve()` → 阈值过滤 → `build_rag_prompt`（含图表引导）→ `LLMProvider.chat_stream(SSE)` → `chat_token*` → `persist_exchange()`

**流中断**: `abort_chat` → `CancellationToken.cancel()` → `forward_stream()` select! break → 保留已生成内容 → `persist_exchange()` 落库

---

## 五、新会话快速恢复指南

在新 AI 会话中，只需说：

> 请阅读 `docs/dev-guide/session-handoff.md` 获取项目上下文和待办任务，然后继续处理 {任务名}。

AI 会：
1. 读取此文件获取完整上下文
2. 选择待办任务
3. 读取相关源文件和契约文档
4. 实现修复
5. 运行测试验证

---

## 六、铁律规则提醒

> 所有铁律定义在 `.catpaw/rules/iron-rules.md`（10 条），核心要点：
> 1. 网络代理设置（127.0.0.1:7890）用于互联网访问
> 2. TDD 驱动开发（先红灯后绿灯）
> 3. 代码注释规范（Rust `///` + JS JSDoc）
> 4. 真实数据测试（真实语料 + 混沌测试 + LLM 环境变量）
> 5. 生产级代码质量（零 unwrap / 零 panic / 零裸溢出）
> 6. 最新版本依赖原则（查官网，不凭记忆）
> 7. 文档实时同步（代码变更后同步更新对应文档）
> 8. 开发决策必须互联网搜索，不能闭门造车
> 9. Rust 生态优先（纯 Rust 优先，C 绑定需论证）
> 10. 最前沿技术追踪（Semantic Chunking / HyDE / Reranking / Contextual Retrieval）
