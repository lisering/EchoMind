# EchoMind v1.17.0 变更日志

**发布日期**: 2026-08-12
**版本号**: 1.17.0
**上一版本**: 1.16.0

## SRS 状态修正（5 个已实现 REQ）

以下 REQ 已在之前版本实现但 SRS 仍标 Draft，本次修正为 Implemented：

| REQ ID | 描述 | 实现版本 | 实现位置 |
|---|---|---|---|
| REQ-ING-016 | .html 文档导入 | v1.1 S45 | `crates/core/src/loader.rs` |
| REQ-RAG-011 | 会话搜索 | v1.16 | `ui/src/sidebar.js` |
| REQ-ERR-003 | 离线模式降级 | v1.13 | `ui/src/offline-detector.js` |
| REQ-OBS-002 | 性能指标采集 | v1.13 | `crates/core/src/perf_metrics.rs` |
| REQ-WIN-004 | 开机自启 | v1.13 | `crates/tauri-app/src/lib.rs` |

## 新增功能（3 个 REQ Implemented）

### S1: REQ-A11Y-005 高对比度模式适配
- **纯 CSS 实现**: `ui/styles/tokens.css` 新增 `[data-theme="high-contrast"]` ~25 个 CSS 变量覆盖
- **设计**: 纯黑背景 (#000000) + 纯白文字 (#FFFFFF) + 高对比度边框 (2px) + 禁用半透明
- **前端 UI**: `ui/index.html` 设置面板新增高对比度主题按钮 (`data-theme-value="high-contrast"`)
- **主题切换**: `ui/src/main.js` `applyTheme()` 支持 high-contrast
- **Mermaid 同步**: 高对比度模式下 Mermaid 使用暗色主题
- **i18n**: 新增 `theme_high_contrast` 键（中英文）
- **E2E 测试**: TC-V17-HC-001~004（4 个测试）

### S2: REQ-RAG-018 用户提问建议
- **动态建议生成**: `ui/src/empty-state.js` `generateRecommendations()` 增强为基于文档名动态生成建议
- **建议模板**: `summarize_doc` / `compare_docs` / `key_topics_doc` 模板 + `{doc}` / `{doc1}` / `{doc2}` 占位符替换
- **集成**: `ui/src/chat-render.js` `resetChatArea()` 传入 `docNames` + 点击建议卡片自动填入并发送
- **状态管理**: `ui/src/main.js` `loadDocuments()` 存储 `kbAllDocs` 到 state + `ui/src/state.js` 新增 `kbAllDocs` 属性
- **空知识库**: 不显示建议提问，仅显示导入引导
- **i18n**: 6 键中英文（`empty_state_suggestion_summarize_doc` / `compare_docs` / `key_topics_doc`）
- **E2E 测试**: TC-V17-SUG-001~003（3 个测试）

### S3: REQ-EXP-002/003 全量数据备份与恢复
- **后端 IPC**: `crates/tauri-app/src/commands/features.rs` 新增 `export_backup` / `import_backup` IPC
- **备份格式**: JSON（conversations + messages + documents + settings），API Key 脱敏 `****XXXX`
- **恢复逻辑**: 版本号校验 + 幂等恢复（INSERT OR REPLACE）
- **辅助 IPC**: `crates/tauri-app/src/commands/chat.rs` 新增 `read_text_file` IPC
- **前端 UI**: `ui/src/settings.js` 新增 `exportBackup()` / `importBackup()` 函数
- **交互流程**: 确认对话框 → 文件选择 → 恢复 → 重启提示
- **类型声明**: `types/dom-ext.d.ts` Window 类型声明
- **tauri-stub.js**: 3 个 mock
- **E2E 测试**: TC-V17-BACKUP-001~003（3 个测试）

## 验证结果

| 指标 | 数值 |
|---|---|
| Rust Free | 1279 passed, 0 failed, 31 ignored |
| Clippy | 零警告 |
| cargo fmt | 通过 |
| tsc | 0 errors |
| esbuild | 18388 行 |
| E2E (v1.17) | 10 passed |
| 版本号 | 1.17.0 |
| IPC 命令 | 211 Free / 223 Pro |
