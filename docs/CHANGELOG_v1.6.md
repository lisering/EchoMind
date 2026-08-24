# EchoMind v1.6.0 变更日志

> **发布日期**: 2026-08-10  
> **版本**: v1.5.0 → v1.6.0  
> **测试基线**: Rust 1224+ passed | Clippy 零警告 | tsc 0 errors | esbuild 16485 行 | E2E 11/11 passed

---

## 新增功能（5 个）

### S1: 日期时间本地化 (REQ-I18N-002)
- **新模块**: `ui/src/date-utils.js`
- **功能**: `formatDate()` 输出 YYYY-MM-DD HH:mm 格式；`formatRelativeTime()` 输出相对时间（刚刚/N 分钟前/N 小时前/N 天前/绝对时间）；`formatFileSize()` 1024 进制文件大小；`formatNumber()` 千分位分隔符；`formatPercent()` 整数百分比
- **i18n**: 新增 `date.*` 键（中英文 4 键）
- **类型**: `types/dom-ext.d.ts` 新增 `__formatDate` / `__formatRelativeTime` / `__formatFileSize` / `__formatNumber` Window 声明

### S2: 导出为 HTML (REQ-EXP-007)
- **扩展模块**: `ui/src/export.js` 新增 `buildExportHtml()` / `exportConversationToHtml()` / `exportDocumentToHtml()`
- **功能**: 生成独立可打开的 HTML 文件（内联 CSS，无外部依赖），复用 Markdown 渲染管线（marked.js + DOMPurify + highlight.js），通过 `save_text_file` IPC 保存
- **右键菜单**: `ui/src/context-menu.js` 文档右键新增「导出为 HTML」选项
- **i18n**: 新增 `export_html.*` 键（中英文 8 键）
- **类型**: Window 新增 `exportConversationToHtml` / `exportDocumentToHtml` 声明

### S3: 关于页面 (REQ-HELP-003)
- **新模块**: `ui/src/about-panel.js` — `openAboutPanel()` / `closeAboutPanel()` / `initAboutPanel()`
- **功能**: 关于面板显示应用名称/Logo/版本号/构建日期/技术栈/隐私政策摘要/开源许可/版权声明
- **帮助面板集成**: `ui/src/help-panel.js` 新增第 5 个「关于」Tab
- **i18n**: 新增 `about.*` 键（中英文 12 键）
- **类型**: Window 新增 `__openAboutPanel` / `__closeAboutPanel` 声明

### S4: 窗口关闭行为 (REQ-WIN-003)
- **后端 IPC**: `crates/tauri-app/src/commands/settings.rs` 新增 `get_close_to_tray` / `set_close_to_tray` 命令
- **窗口拦截**: `crates/tauri-app/src/lib.rs` `on_window_event` 拦截 `CloseRequested`，close_to_tray 开启时 `prevent_close()` + `window.hide()`
- **设置面板**: `ui/src/settings.js` 新增 `renderWindowSettings()` 动态渲染 toggle 开关
- **IPC 封装**: `ui/src/ipc.js` 新增 `windowSettingsApi`
- **i18n**: 新增 `window_settings.*` 键（中英文 5 键）
- **E2E mock**: `tauri-stub.js` 新增 `get_close_to_tray` / `set_close_to_tray` mock

### S5: 错误日志导出 (REQ-ERR-006)
- **后端 IPC**: `crates/tauri-app/src/commands/features.rs` 新增 `export_error_logs` 命令（过滤 ERROR 级别日志）
- **设置面板**: `ui/src/settings.js` 新增 `renderErrorLogsSettings()` 导出按钮
- **IPC 封装**: `ui/src/ipc.js` 新增 `errorLogsApi`
- **i18n**: 新增 `error_logs.*` 键（中英文 5 键）
- **E2E mock**: `tauri-stub.js` 新增 `export_error_logs` mock

---

## 版本号变更
- `crates/tauri-app/tauri.conf.json`: 1.5.0 → **1.6.0**

## 新增文件
- `ui/src/date-utils.js` — 日期时间本地化工具
- `ui/src/about-panel.js` — 关于面板
- `e2e-tests/tests/v1.6-features.spec.ts` — v1.6 E2E 测试（11 个测试）
- `docs/CHANGELOG_v1.6.md` — 本文件

## 关键修改
- `ui/src/export.js`: 新增 HTML 导出功能（3 函数 + CSS 构建）
- `ui/src/context-menu.js`: 文档右键新增「导出为 HTML」+ `exportDocumentToHtml` 导入
- `ui/src/help-panel.js`: 新增第 5 个「关于」Tab + `_renderAbout()` 函数
- `ui/src/settings.js`: 新增 `renderWindowSettings()` + `renderErrorLogsSettings()` + IPC 导入
- `ui/src/ipc.js`: 新增 `windowSettingsApi` + `errorLogsApi`
- `ui/src/main.js`: 新增 `date-utils` / `about-panel` 导入 + 全局暴露
- `ui/locales/zh-CN.json` + `en.json`: 新增 5 个 section（date/export_html/about/window_settings/error_logs）+ ctx.export_html 键
- `types/dom-ext.d.ts`: 新增 10 个 Window 属性声明
- `crates/tauri-app/src/lib.rs`: 新增 3 个 IPC 命令注册 + close-to-tray 窗口事件拦截
- `crates/tauri-app/src/commands/settings.rs`: 新增 `get_close_to_tray` / `set_close_to_tray` + `_inner` 函数
- `crates/tauri-app/src/commands/features.rs`: 新增 `export_error_logs` + `_inner` 函数
- `e2e-tests/tests/bridge/tauri-stub.js`: 新增 3 个 IPC mock

## IPC 命令变更
- 新增 3 个 Free IPC 命令: `get_close_to_tray` / `set_close_to_tray` / `export_error_logs`

## SRS REQ 状态变更
- REQ-I18N-002: Draft → **Implemented**（日期时间本地化）
- REQ-WIN-003: Draft → **Implemented**（窗口关闭行为 close-to-tray）
- REQ-HELP-003: Draft → **Implemented**（关于页面）
- REQ-EXP-007: 新增 → **Implemented**（导出为 HTML）
- REQ-ERR-006: 新增 → **Implemented**（错误日志导出）
