# EchoMind v1.1.0 发布说明

发布日期: 2026-08-06

## 概述

EchoMind（灵犀）v1.1.0 是 v1.0.0 之后的第一个功能更新版本。本次发布新增 5 项功能，涵盖文档格式扩展、主题切换、语音交互和互联网搜索集成。

## 新增功能

### 1. .docx 文档导入（REQ-ING-015）

- 使用 `docx-rs` v0.4.22 纯 Rust 解析 Office Open XML
- 支持段落文本提取、表格转 Markdown、标题层级保留
- 图片以 `[图片]` 占位符替代
- 导入格式扩展：md / txt / pdf / docx / html（5 种）

### 2. .html 文档导入（REQ-ING-016）

- 使用 `html2text` v0.17.1 纯 Rust HTML → 纯文本转换
- 保留标题层级（`<h1>`~`<h6>` → `#`~`######`）
- 去除 `<nav>`/`<header>`/`<footer>`/`<script>`/`<style>`
- `from_utf8_lossy` 容错编码处理

### 3. 浅色主题切换（REQ-UI-011）

- CSS 变量覆盖实现零组件代码改动（~40 个变量）
- 三种主题模式：暗色 / 浅色 / 跟随系统（`prefers-color-scheme`）
- 双 rAF 过渡禁用防闪烁
- Mermaid 主题同步
- 后端持久化到 settings 表（`ui.theme` 键）
- WCAG AA 对比度达标（≥ 4.5:1）
- 2 个新 IPC 命令：`set_theme` / `get_theme`

### 4. 语音输入 + TTS 朗读（REQ-RAG-034/035）

- **语音输入**：`SpeechRecognition` API 麦克风实时转写
- **TTS 朗读**：`speechSynthesis` API 助手回答语音播放
- 语言跟随当前 UI 语言（中英文）
- 语速可配置（0.5~2.0，设置面板滑块）
- Web Speech API 不支持时优雅降级隐藏入口
- 零新增依赖（浏览器原生 API）

### 5. 网页搜索集成（REQ-RAG-036）

- DuckDuckGo Instant Answer API（零 API Key，HTTPS）
- 本地检索不足时自动触发搜索（阈值 0.3）
- 搜索结果与本地知识库 RRF 融合（本地权重 1.0 / Web 权重 0.4）
- 搜索结果标注 🌐 前缀区分来源
- opt-in 功能（默认关闭，用户手动开启）
- 搜索失败优雅降级（不崩溃，仅用本地结果）
- 2 个新 IPC 命令：`set_web_search_enabled` / `web_search`
- `WebSearchProvider` trait + `DuckDuckGoProvider` 实现

## 技术规格

| 指标 | v1.0.0 | v1.1.0 | 变化 |
|---|---|---|---|
| Rust 测试 (Free) | 718 passed | 749 passed | +31 |
| Rust 测试 (Pro) | 1003 passed | 1034 passed | +31 |
| E2E 桥接测试 | 985 passed | 999 passed | +14 |
| Vitest 前端测试 | 389 passed | 407 passed | +18 |
| IPC 命令数 | 134 | 138 | +4 |
| 支持文档格式 | 4 (md/txt/pdf/docx) | 5 (+html) | +1 |
| 前端 ES module | 47 | 49 | +2 |
| TypeScript errors | 42 | 0 | -42 |
| 新增 SRS 需求 | — | 6 条 | +6 |
| 新增依赖 | — | docx-rs, html2text | +2 |

## 新增依赖

| 依赖 | 版本 | 用途 | crate |
|---|---|---|---|
| `docx-rs` | 0.4.22 | .docx 文件解析 | echomind-core |
| `html2text` | 0.17.1 | HTML → 纯文本转换 | echomind-core |

## 新增 SRS 需求

| REQ ID | 域 | 描述 | 状态 |
|---|---|---|---|
| REQ-ING-015 | ING | .docx 文档导入 | Implemented |
| REQ-ING-016 | ING | .html 文档导入 | Implemented |
| REQ-UI-011 | UI | 浅色主题切换 | Implemented |
| REQ-RAG-034 | RAG | 语音输入 | Implemented |
| REQ-RAG-035 | RAG | TTS 回答朗读 | Implemented |
| REQ-RAG-036 | RAG | 网页搜索集成 | Implemented |

## 新增设置键

| 设置键 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `ui.theme` | String | `"dark"` | 主题模式：dark / light / system |
| `voice.rate` | f32 | `1.0` | TTS 语速（0.5~2.0） |
| `rag.web_search_enabled` | bool | `false` | 网页搜索开关（opt-in） |

## 向后兼容性

- v1.0.0 数据库无缝升级（无 schema 变更）
- 所有新功能默认关闭或降级（opt-in）
- 浅色主题默认不启用（保持暗色）
- 网页搜索默认关闭
- 语音功能在 WebView 不支持时自动隐藏

## 平台支持

- macOS (Intel + Apple Silicon)
- Windows 10/11
- Linux (Ubuntu 20.04+)
