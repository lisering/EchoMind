# EchoMind v1.2.0 发布说明

发布日期: 2026-08-07

## 概述

EchoMind（灵犀）v1.2.0 是 v1.1.0 之后的第二个功能更新版本。本次发布新增 4 项功能，涵盖 PDF 导出、演示文稿导入、电子书导入和自定义嵌入模型上传，进一步扩展了知识库的格式支持与模型灵活性。

## 新增功能

### 1. PDF 导出（REQ-EXP-005）

- `window.print()` 零依赖方案，复用 Markdown 渲染管线（marked.js + DOMPurify + highlight.js）
- 支持导出对话和单篇文档为 PDF
- 页面大小可选 A4 / Letter
- 可选包含引用来源
- `@media print` CSS 规则：隐藏 UI 元素 + serif 字体 + 分页控制
- 零新增 Rust 依赖，零新增 IPC 命令
- 新增模块：`ui/src/export.js` + `ui/styles/print.css`
- 5 个 E2E 测试（TC-EXP-PDF-001~005）

### 2. .pptx 演示文稿导入（REQ-ING-017）

- 使用 `pptx-to-md` v1.0.0（纯 Rust，MIT/Apache-2.0）解析 PowerPoint 文件
- 提取幻灯片文本 + 演讲者备注，转换为 Markdown 格式
- `PptxLoader` 实现 `Loader` trait：`PptxContainer::open()` → `convert_to_md()` → 后处理压缩换行
- 配置策略：不提取图片 + 包含演讲者备注 + 不包含幻灯片编号 + 不包含元数据
- `ALLOWED_EXTENSIONS` 11 → 12（新增 `pptx`）
- 4 个 TDD 测试（TC-ING-PPTX-001~004）

### 3. .epub 电子书导入（REQ-ING-018）

- 使用 `rbook` v0.7.10（纯 Rust，Apache-2.0，EPUB 2/3 标准）解析电子书
- 遍历 spine 条目 → 读取 XHTML → `html2text` 转纯文本 → 章节间 `---` 分隔
- `EpubLoader` 实现 `Loader` trait：`Epub::open()` → `spine().iter()` → `read_str()` → `from_read()`
- 排除 `epub` crate（GPL-3.0 不兼容），选择 `rbook`（Apache-2.0 兼容）
- `ALLOWED_EXTENSIONS` 12 → 13（新增 `epub`）
- 4 个 TDD 测试（TC-ING-EPUB-001~004）

### 4. 自定义 ONNX 嵌入模型上传（REQ-VEC-014，Pro 门控）

- 支持用户上传自定义 ONNX 嵌入模型（复用 fastembed `UserDefinedEmbeddingModel`）
- `EmbeddingModel::Custom(String)` 枚举变体 + `new_with_custom_model()` / `init_custom()` 加载
- 模型文件验证：`find_onnx_file()` / `validate_onnx_file()` / `validate_tokenizer_files()`
- 模型管理：`list_custom_models()` / `delete_custom_model()` / `copy_custom_model_files()`
- `model_store.rs` 解析 `"custom:{name}"` 前缀，走 `new_with_custom_model()` 路径
- 路径遍历防护（名称中的 `/\\..` 替换为 `_`）
- 3 个 Pro 门控 IPC 命令：`upload_custom_embedding_model` / `list_custom_models` / `delete_custom_model`
- IPC 命令数 138 → 141
- 零新增 Rust 依赖（复用 fastembed）
- 7 个集成测试（TC-VEC-CUSTOM-001~007）+ 8 个单元测试（TC-VEC-CUSTOM-UNIT-001~008）

## 技术规格

| 指标 | v1.1.0 | v1.2.0 | 变化 |
|---|---|---|---|
| Rust 测试 (Free) | 749 passed | 766 passed | +17 |
| Rust 测试 (Pro) | 1034 passed | 1057 passed | +23 |
| E2E 桥接测试 | 999 passed | 950 passed | -49（测试合并优化）|
| Vitest 前端测试 | 407 passed | 407 passed | — |
| IPC 命令数 | 138 | 141 | +3 |
| 支持文档格式 | 11 (md/txt/pdf/docx/html/pptx) | 13 (+epub) | +2 |
| 前端 ES module | 49 | 50 (+export.js) | +1 |
| esbuild 构建行数 | 11142 | 11670 | +528 |
| TypeScript errors | 0 | 0 | — |
| 新增 SRS 需求 | — | 4 条 | +4 |
| 新增依赖 | — | pptx-to-md, rbook | +2 |

## 新增依赖

| 依赖 | 版本 | 用途 | crate |
|---|---|---|---|
| `pptx-to-md` | 1.0.0 | .pptx 文件解析（MIT/Apache-2.0） | echomind-core |
| `rbook` | 0.7.10 | .epub 文件解析（Apache-2.0，EPUB 2/3） | echomind-core |

## 新增 SRS 需求

| REQ ID | 域 | 描述 | 状态 | Free/Pro |
|---|---|---|---|---|
| REQ-EXP-005 | EXP | PDF 导出（window.print 零依赖） | Implemented | Free |
| REQ-ING-017 | ING | .pptx 演示文稿导入 | Implemented | Free |
| REQ-ING-018 | ING | .epub 电子书导入 | Implemented | Free |
| REQ-VEC-014 | VEC | 自定义 ONNX 嵌入模型上传 | Implemented | Pro |

## 新增 IPC 命令

| 命令 | 说明 | 门控 |
|---|---|---|
| `upload_custom_embedding_model` | 上传自定义 ONNX 嵌入模型 | Pro |
| `list_custom_models` | 列出已上传的自定义模型 | Pro |
| `delete_custom_model` | 删除自定义模型 | Pro |

## 新增设置键

| 设置键 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `export.pdf_page_size` | String | `"A4"` | PDF 导出页面大小（A4 / Letter） |
| `export.pdf_include_sources` | bool | `true` | PDF 导出是否包含引用来源 |

## 向后兼容性

- v1.1.0 数据库无缝升级（无 schema 变更）
- 所有新功能默认关闭或降级（opt-in）
- PDF 导出默认可用（Free 功能）
- .pptx / .epub 导入默认可用（Free 功能）
- 自定义模型上传为 Pro 功能（Free 版本返回 PRO_REQUIRED 错误）
- 原有嵌入模型（all-MiniLM-L6-v2）不受影响

## 平台支持

- macOS (Intel + Apple Silicon)
- Windows 10/11
- Linux (Ubuntu 20.04+)
