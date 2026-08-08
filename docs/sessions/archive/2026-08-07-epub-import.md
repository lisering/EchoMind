# Session Archive: v1.2 S51 — .epub 电子书导入（REQ-ING-018）

> **日期**: 2026-08-07 | **Session**: v1.2 S51 | **REQ**: REQ-ING-018 | **状态**: ✅ Implemented

## 实现内容

### 新增依赖
- `crates/core/Cargo.toml` — 新增 `rbook = { version = "0.7", default-features = false, features = ["threadsafe", "prelude"] }`（纯 Rust，Apache-2.0，EPUB 2/3）

### 核心实现
- `crates/core/src/loader.rs` — 新增 `EpubLoader` struct + `impl EpubLoader { fn extract(file_path: &str) -> anyhow::Result<String> }` + `impl Loader for EpubLoader`
  - 加载流程：`Epub::open(path)` → `epub.spine().iter()` 遍历 spine 条目（阅读顺序） → `spine_entry.manifest_entry().read_str()` 读取 XHTML 内容 → `html2text::from_read(xhtml.as_bytes(), 80)` 转纯文本 → 章节间 `---` 分隔 + 压缩换行
  - rbook API 使用 Epub 具体类型的固有方法（spine/iter/manifest_entry/read_str），非 trait 方法，故 prelude import 不需要（但保留 feature 以备用）
  - 空文本 bail!

### TDD 测试
- `crates/core/src/loader_tests.rs` — 新增 TC-ING-EPUB-001~004 测试
  - TC-ING-EPUB-001: 简单章节 .epub 导入，提取章节文本（AC-1）
  - TC-ING-EPUB-002: 多章节 .epub 导入，按 spine 顺序提取，章节间分隔（AC-2）
  - TC-ING-EPUB-003: 含 HTML 标签 .epub 导入，标签被剥离，纯文本保留（AC-3）
  - TC-ING-EPUB-004: 损坏 .epub 文件返回 Err，不 Panic（AC-4）
  - 复用已有 `fixture_path()` 辅助函数

### Fixture 生成器
- `tests/fixtures/gen_epub.py` — Python stdlib zipfile 生成 EPUB 测试 fixture
  - `test_simple.epub` — 单章节，含纯文本
  - `test_multi.epub` — 多章节，含标题层级
  - `test_html.epub` — 含 HTML 标签（`<b>`/`<i>`/`<a>`/`<ul>` 等）
  - `test_corrupt.epub` — 损坏文件（纯文本）

### 导入管线集成
- `crates/core/src/import.rs` — (a) `ALLOWED_EXTENSIONS` 12→13（新增 `"epub"`）；(b) `index_inner()` 新增 `"epub" =>` 分支；(c) `index_multimodal_inner()` 新增 `"epub" =>` 分支

### 前端 / i18n / Mock 更新
- `ui/locales/en.json` + `ui/locales/zh-CN.json` — 3 处格式列表加 `.epub`（empty_state_formats / empty_formats / drag_hint）
- `e2e-tests/tests/bridge/tauri-stub.js` — WHITELIST 新增 `'epub'` + 错误消息加 `.epub`
- `ui/src/wizard.js` — 文件对话框 extensions 数组加 `'epub'`（2 处）
- `ui/src/chat-render.js` — 文件对话框 extensions 数组加 `'epub'`（1 处）
- `ui/src/import.js` — 文件对话框 extensions 数组加 `'epub'`（1 处）

### 文档同步
- `docs/SRS_v1.0.md` — REQ-ING-018 Draft → Implemented + changelog v5.4.0
- `docs/architecture/V1.2_ATOMIC_PLAN.md` — S51 验收标准 checkbox 全部勾选
- `docs/api/code-contracts.md` — 新增 §5.5c EpubLoader 契约
- `docs/api/ipc-commands.md` — 支持格式列表加 `.epub`
- `docs/SOFTWARE_ASSET.md` — ALLOWED_EXTENSIONS 变更记录更新（12→13）
- `AGENTS.md` — loader.rs 描述加 Epub + 数据流加 epub
- `CLAUDE.md` — loader.rs 描述加 Epub + 数据流加 epub

## rbook API 调研

- **版本**: rbook v0.7.10（Apache-2.0）
- **Features**: `threadsafe`（Send+Sync）+ `prelude`（trait 便捷导入）+ `write`（EPUB 写入，默认启用但我们 `default-features = false` 排除）
- **关键 API**:
  - `Epub::open(path: impl AsRef<Path>) -> EbookResult<Epub>` — 打开 EPUB 文件
  - `epub.spine() -> EpubSpine<'_>` — 获取 spine（阅读顺序），固有方法
  - `spine.iter() -> EpubSpineIter<'_>` — 遍历 spine 条目
  - `spine_entry.manifest_entry() -> Option<EpubManifestEntry<'_>>` — 获取 manifest 条目
  - `manifest_entry.read_str() -> ArchiveResult<String>` — 读取 XHTML 内容为字符串
- **发现**: rbook Epub 的 `spine()`/`manifest_entry()`/`read_str()` 均为固有方法（inherent methods），非 trait 方法。prelude feature 虽然导入但实际未使用（编译器 warning 后移除 import）。

## 验证结果

| 检查项 | 结果 |
|---|---|
| cargo test (Free) | 757 passed, 0 failed, 27 ignored (+4 TC-ING-EPUB) |
| cargo test --features pro (Pro) | 1042 passed, 0 failed, 44 ignored (+4 TC-ING-EPUB) |
| cargo clippy --all-targets -- -D warnings (Free) | 零警告 |
| cargo clippy --all-targets --features pro -- -D warnings (Pro) | 零警告 |
| cargo fmt --check | 通过 |
| node scripts/build-ui.mjs | 11566 行，成功 |
| npx tsc --noEmit | 0 errors |

## 衔接提示词（S52 自定义 ONNX 嵌入模型上传 REQ-VEC-014 Pro）

下一 Session S52 目标：实现自定义 ONNX 嵌入模型上传功能（REQ-VEC-014 Draft → Implemented，Pro 门控）。

**必读文档**:
1. `docs/architecture/V1.2_ATOMIC_PLAN.md` — S52 章节（第 643-680 行）
2. `docs/SRS_v1.0.md` — REQ-VEC-014
3. `crates/infra/src/local_embedder.rs` — 现有 fastembed EmbeddingModel 枚举 + LocalEmbedder 实现
4. `crates/tauri-app/src/commands.rs` — 现有 Pro 门控 IPC 命令模式
5. `crates/tauri-app/src/state.rs` — AppState.custom_model_dir() 需新增

**关键设计**: 复用 fastembed `UserDefinedEmbeddingModel`，新增 3 个 Pro 门控 IPC 命令（upload_custom_embedding_model / list_custom_models / delete_custom_model）。
