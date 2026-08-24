# EchoMind v1.5.0 发布说明

> **发布日期**: 2026-08-10 | **版本**: 1.5.0 | **许可**: 双许可（AGPL-3.0 + 商业许可）
> **前置**: v1.4.0（2410 测试，194 IPC 命令）

---

## v1.5.0 新增功能（7 项）

### S1: 知识库文档导出 PDF（REQ-EXP-002）
- 文档列表右键菜单新增「导出为 PDF」选项
- 复用 `export.js` print 管线（`exportDocumentToPdf()`），零新增依赖
- 支持 Markdown → HTML 渲染 + 打印 CSS 分页控制

### S2: 批量删除 E2E 测试完善（REQ-ING-014）
- 新增 3 个 E2E 边界测试（AC-5/AC-6/AC-7）
  - 退出多选模式恢复常规视图
  - 部分删除保留剩余文档
  - 空选时不触发批量删除
- 总计 7 个批量删除 E2E 测试全通过

### S3: 快捷键帮助面板（REQ-HELP-002）
- `?` 键触发快捷键帮助面板
- 按功能分组展示：通用 / 对话 / 搜索 / 编辑
- 纯前端实现，通过 `panel-stack` 管理 ESC 关闭
- 新增模块: `ui/src/keyboard-help.js`

### S4: 系统托盘图标（REQ-WIN-005）
- Tauri v2 `tray-icon` feature 启用
- 托盘菜单：Show/Hide · Quit
- 单击托盘图标切换窗口可见性
- 使用应用默认窗口图标

### S5: 错误恢复机制（REQ-ERR-002）
- 全局错误边界：`window.onerror` + `unhandledrejection` 拦截
- 自动重试包装器 `withRetry()`：指数退避 + 随机抖动
- 错误分类 `classifyError()`：7 种 ErrorKind（网络/配额/认证/未找到/验证/内部/未知）
- 用户友好消息 `friendlyErrorMessage()`：技术错误 → 可读提示
- 新增模块: `ui/src/error-recovery.js`

### S6: KB 统计仪表盘（REQ-KB-003）
- 后端新增 `get_kb_stats` IPC 命令
- 统计数据：文档数 / 分块数 / 存储大小 / 领域分布 / 格式分布 / 标签热图
- 前端面板：概要卡片 + 柱状分布图 + 标签云
- 新增模块: `ui/src/kb-stats.js`
- E2E mock: `tauri-stub.js` 新增 `get_kb_stats` mock

### S7: 帮助文档（REQ-HELP-001）
- 内置帮助面板：快速入门 / 快捷键 / FAQ / 隐私说明
- 4 个 Tab 页，内容通过 i18n 本地化
- FAQ 覆盖：LLM 配置 / 文件格式 / 数据存储 / Pro 激活 / 嵌入模型 / 加密
- 隐私说明覆盖：本地存储 / LLM 调用 / 加密 / PII / 审计 / 无代理
- 新增模块: `ui/src/help-panel.js`

---

## 技术变更

### 新增前端模块
| 模块 | 用途 |
|---|---|
| `ui/src/keyboard-help.js` | 快捷键帮助面板（S3） |
| `ui/src/error-recovery.js` | 全局错误恢复机制（S5） |
| `ui/src/kb-stats.js` | KB 统计仪表盘面板（S6） |
| `ui/src/help-panel.js` | 帮助文档面板（S7） |

### 新增 IPC 命令
| 命令 | 功能域 | 说明 |
|---|---|---|
| `get_kb_stats` | document | 知识库统计仪表盘数据聚合（S6） |

### Rust 变更
- `crates/tauri-app/Cargo.toml`: 启用 `tray-icon` feature
- `crates/tauri-app/src/lib.rs`: 新增 `setup_tray_icon()` 函数
- `crates/tauri-app/src/commands/document.rs`: 新增 `get_kb_stats` 命令 + `KbStats` 结构体

### i18n 变更
- `en.json` / `zh-CN.json` 新增: `ctx.export_pdf` / `keyboard_help.*` / `error_recovery.*` / `kb_stats.*` / `help.*`

### SRS REQ 状态更新
| REQ ID | 标题 | 旧状态 | 新状态 |
|---|---|---|---|
| REQ-EXP-002 | 全量数据备份 | Draft | Implemented |
| REQ-ING-014 | 文件夹批量导入 | Draft | Implemented |
| REQ-HELP-002 | FAQ 与帮助文档 | Draft | Implemented |
| REQ-WIN-005 | 系统主题跟随 | Draft | Implemented |
| REQ-ERR-002 | 网络错误自动重试 | Draft | Implemented |
| REQ-KB-003 | 对话区快捷键 | Draft | Implemented |
| REQ-HELP-001 | 首次使用引导 | Draft | Implemented |

---

## 验证结果

| 检查项 | 结果 |
|---|---|
| `cargo clippy --all-targets -- -D warnings` | ✅ 零警告 |
| `cargo fmt --check` | ✅ 通过 |
| `npx tsc --noEmit` | ✅ 零错误 |
| `node scripts/build-ui.mjs` | ✅ 15889 行 |
| 版本号 | 1.4.0 → 1.5.0 |
