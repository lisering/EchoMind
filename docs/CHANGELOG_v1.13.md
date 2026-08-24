# EchoMind v1.13.0 变更日志

**发布日期**: 2026-08-11
**版本号**: 1.13.0
**上一版本**: 1.12.0

## 新增功能（5 个 REQ Implemented）

### S1: REQ-ERR-003 离线模式降级
- **新建模块**: `ui/src/offline-detector.js`
- **功能**: 监听 `online`/`offline` 浏览器事件，离线时：
  - 侧栏显示离线指示器（`#offlineIndicator`）
  - 发送按钮 disabled
  - 输入框 placeholder 变为离线提示
  - 在线恢复后自动还原 UI
- **i18n**: 新增 `offline_indicator` / `offline_placeholder` / `offline_hint` 键

### S2: REQ-WIN-004 开机自启
- **新增依赖**: `tauri-plugin-autostart = "2"`（MIT/Apache-2.0）
- **新增 IPC 命令**: `get_autostart` / `set_autostart`
- **功能**: 设置面板新增「开机自启动」toggle 开关
- **设计**: 优先从 settings 表读取持久化状态，回退到 autostart 插件实际状态
- **ACL**: capabilities/default.json 新增 `autostart:default` / `allow-enable` / `allow-disable` / `allow-is-enabled`

### S3: REQ-HELP-004 应用更新检查
- **新建核心模块**: `crates/core/src/update_check.rs`（纯逻辑，无 I/O 依赖）
  - `parse_semver()` — 支持 `1.0.0` / `v1.0.0` / `1.0.0-alpha` 格式
  - `is_newer_version()` — semver 比较（预发布低于正式版）
  - `should_check()` — 24 小时频率控制
  - `UpdateCheckResult` / `UpdateCheckConfig` 数据模型
  - 14 个 TDD 测试
- **新增 IPC 命令**: `check_for_updates` / `get_update_check_config` / `set_update_check_enabled`
- **GitHub API**: tauri-app 层 `fetch_github_release()` 调用 GitHub Releases API（`.no_proxy()` 铁律一合规）
- **前端**: 设置面板新增「自动检查更新」toggle

### S4: REQ-OBS-002 性能指标采集
- **新建核心模块**: `crates/core/src/perf_metrics.rs`
  - `PerfMetric` 结构体（operation / duration_ms / data_size / timestamp）
  - `PerfTimer` 计时器（`start()` / `elapsed_ms()` / `finish()` / `finish_with_size()`）
  - tracing INFO 级别日志记录（`PERF_METRIC` 标签）
  - 无用户数据（仅元数据）
  - 5 个 TDD 测试

### S5: REQ-NAV-004 面包屑与上下文指示
- **新建前端模块**: `ui/src/breadcrumb.js`
  - 面包屑导航：`知识库 > 会话标题`
  - 点击知识库名 → 跳转知识库分区
  - 点击会话标题 → 进入重命名编辑模式
  - 右侧显示消息数与创建时间
  - 空会话显示「新对话」
- **HTML**: `ui/index.html` 新增 `#breadcrumbBar` 容器
- **CSS**: `ui/styles/components.css` 新增面包屑样式
- **i18n**: 新增 `breadcrumb` 命名空间（7 键中英文）

## 测试验证

| 指标 | 数值 |
|---|---|
| Rust Free 测试 | 1276 passed, 0 failed (+19 新测试) |
| Clippy | 零警告 |
| cargo fmt | 通过 |
| TypeScript | 0 errors |
| esbuild | 17704 行 (+374 行) |
| E2E (v1.13) | 13 passed, 0 failed |

## IPC 命令变更

| 命令 | Free/Pro | 说明 |
|---|---|---|
| `get_autostart` | Free | 获取开机自启状态 |
| `set_autostart` | Free | 设置开机自启 |
| `check_for_updates` | Free | 检查 GitHub Releases 新版本 |
| `get_update_check_config` | Free | 获取更新检查配置 |
| `set_update_check_enabled` | Free | 设置自动检查开关 |

IPC 命令数：206 → 209 Free / 214 → 217 Pro

## 新增依赖

- `tauri-plugin-autostart = "2"`（MIT/Apache-2.0，Tauri 官方开机自启插件）

## 文件变更摘要

| 文件 | 变更类型 |
|---|---|
| `crates/core/src/update_check.rs` | 新建 |
| `crates/core/src/perf_metrics.rs` | 新建 |
| `crates/core/src/lib.rs` | 修改（注册 2 模块） |
| `crates/tauri-app/src/commands/settings.rs` | 修改（+5 IPC 命令） |
| `crates/tauri-app/src/lib.rs` | 修改（注册插件 + 5 命令） |
| `crates/tauri-app/Cargo.toml` | 修改（+1 依赖） |
| `crates/tauri-app/capabilities/default.json` | 修改（+4 权限） |
| `crates/tauri-app/tauri.conf.json` | 修改（版本号 1.13.0） |
| `ui/src/offline-detector.js` | 新建 |
| `ui/src/breadcrumb.js` | 新建 |
| `ui/src/main.js` | 修改（导入 + 初始化） |
| `ui/src/settings.js` | 修改（renderStartupSettings） |
| `ui/src/ipc.js` | 修改（+2 API 对象） |
| `ui/index.html` | 修改（+面包屑容器 + 离线指示器） |
| `ui/styles/components.css` | 修改（+面包屑样式） |
| `ui/locales/en.json` | 修改（+10 键） |
| `ui/locales/zh-CN.json` | 修改（+10 键） |
| `e2e-tests/tests/v1.13-features.spec.ts` | 新建 |
| `e2e-tests/tests/bridge/tauri-stub.js` | 修改（+5 mock + 5 state） |
| `docs/SRS_v1.0.md` | 修改（5 REQ Draft→Implemented + changelog） |
