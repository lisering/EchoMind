# EchoMind v1.10.0 变更日志

> **发布日期**：2026-08-11 | **上一个版本**：v1.9.0

## 新增功能

### REQ-RAG-014 检索参数可配置 — 前端 UI 同步 ✅

v1.9 后端已完成，v1.10 补齐前端设置面板：

- 设置面板新增 RAG 检索参数区
  - Top-K 滑块（1-20，默认 8）
  - 相似度阈值滑块（0.0-1.0，默认 0.0）
  - Chunk Expansion 开关（默认开启）
  - 扩展窗口滑块（0-3，默认 1）
- 参数保存到 settings 表，对新查询生效
- 中英文 i18n 支持

### REQ-RAG-015 LLM 生成参数可配置 — 前端 UI 同步 ✅

v1.9 后端已完成，v1.10 补齐前端设置面板：

- 设置面板新增 LLM 生成参数区
  - Temperature 滑块（0.0-2.0，默认 0.7）
  - Max Tokens 输入框（256-8192，默认 4096）
  - Top-P 滑块（0.0-1.0，默认 1.0）
- 参数保存到 settings 表，对新查询生效
- 中英文 i18n 支持

### REQ-ING-008 文档列表排序 — 前端 UI 同步 ✅

v1.9 后端已完成，v1.10 补齐前端排序下拉：

- 知识库弹框新增排序按钮 + 下拉面板
- 6 个排序选项：导入时间（新→旧/旧→新）、文件名（A-Z/Z-A）、文件大小（大→小/小→大）
- 排序选择在当前会话内保持
- 排序参数传递到后端 `get_documents(sortBy, sortOrder)` IPC

## 改动文件

| 文件 | 改动 |
|---|---|
| `ui/src/ipc.js` | 新增 `ragParamsApi` + `generationParamsApi` IPC 封装 + `docApi.list` 支持 sort 参数 |
| `ui/src/settings.js` | 新增 `renderRagLlmParams()` + `_makeSlider()` + `_makeNumberInput()` + `_makeToggle()` 辅助函数 |
| `ui/src/main.js` | `loadDocuments()` 传递排序参数 + 排序状态管理 + 排序选择事件 |
| `ui/src/sidebar.js` | 排序面板切换事件 |
| `ui/index.html` | 设置面板新增 `ragLlmParamsContainer` + KB 弹框新增排序按钮/下拉 |
| `ui/locales/en.json` | 新增 26 个 i18n 键 |
| `ui/locales/zh-CN.json` | 新增 26 个 i18n 键 |
| `e2e-tests/tests/bridge/tauri-stub.js` | 4 个 IPC mock + `get_documents` sort 支持 + 状态重置 |
| `e2e-tests/tests/v1.10-features.spec.ts` | 5 个 E2E 测试 |
| `crates/tauri-app/tauri.conf.json` | 版本号 1.9.0 → 1.10.0 |

## 测试验证

| 指标 | 数值 |
|---|---|
| Rust Free | 1255 passed, 0 failed, 31 ignored |
| Clippy (Free) | 零警告 |
| cargo fmt | 通过 |
| TypeScript | 0 errors |
| esbuild | 16901 行 |
| E2E (v1.10) | 5 passed, 0 failed |

## 零新增依赖

本版本无新增 Rust 或前端依赖。
