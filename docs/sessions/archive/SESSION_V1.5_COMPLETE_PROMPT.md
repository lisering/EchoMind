# EchoMind v1.5 → v1.6 开发会话提示词

## 项目

EchoMind — Rust + Tauri v2 本地 RAG 知识库（律师/研究者高隐私场景）。六边形架构 8 crate: `models → prompt → compact → context → core → infra → tauri-app` + `license-issuer`。版本 **v1.5.0**，~99K 行 Rust + ~26K 行前端，2450+ 测试。

## 上 Session 成果（v1.5 全部交付 ✅）

7 个功能全部实现，clippy/fmt/tsc/esbuild 零错误，E2E 全通过，已同步 GitHub：

1. **S1 知识库文档导出 PDF** (REQ-EXP-002) — 文档右键菜单新增「导出为 PDF」，复用 `exportDocumentToPdf()`
2. **S2 批量删除 E2E** (REQ-ING-014) — 新增 AC-5/6/7 边界测试，共 7 个 E2E 全通过
3. **S3 快捷键帮助面板** (REQ-HELP-002) — `?` 键触发，新模块 `ui/src/keyboard-help.js`
4. **S4 系统托盘图标** (REQ-WIN-005) — Tauri v2 `tray-icon` feature，`setup_tray_icon()` in `lib.rs`
5. **S5 错误恢复机制** (REQ-ERR-002) — 全局错误边界 + `withRetry()` 指数退避，新模块 `ui/src/error-recovery.js`
6. **S6 KB 统计仪表盘** (REQ-KB-003) — `get_kb_stats` IPC + 前端面板，新模块 `ui/src/kb-stats.js`
7. **S7 帮助文档** (REQ-HELP-001) — 4 Tab 面板（快速入门/快捷键/FAQ/隐私），新模块 `ui/src/help-panel.js`

### v1.5 新增文件
- `ui/src/keyboard-help.js` / `ui/src/error-recovery.js` / `ui/src/kb-stats.js` / `ui/src/help-panel.js`
- `docs/CHANGELOG_v1.5.md`

### v1.5 关键修改
- `crates/tauri-app/Cargo.toml`: 启用 `tray-icon` feature
- `crates/tauri-app/src/lib.rs`: `setup_tray_icon()` + `get_kb_stats` 命令注册
- `crates/tauri-app/src/commands/document.rs`: `get_kb_stats` + `KbStats` 结构体
- `crates/tauri-app/tauri.conf.json`: 版本 → `1.5.0`
- `ui/src/context-menu.js`: 文档右键新增「导出为 PDF」
- `ui/src/ipc.js`: 新增 `kbStatsApi`
- `e2e-tests/tests/bridge/tauri-stub.js`: 新增 `get_kb_stats` mock

## 当前测试基线

- Rust Free: 1224 passed | Clippy: 零警告 | fmt: 通过
- TypeScript: 0 errors | esbuild: 15889 行 | E2E: 1076+ passed
- 版本号: **1.5.0**

## 本 Session 任务：v1.6 规划 + 实现

从 `docs/architecture/V1.5_ATOMIC_PLAN.md` §5 的 18 个未选入 REQ 中，选 Top 5-7 进入 v1.6。候选（按 ROI 排序）：

| # | REQ ID | 标题 | ROI | 要点 |
|---|---|---|---|---|
| 1 | REQ-I18N-002 | 日期时间本地化 | 12.5 | i18n 渐进改进 |
| 2 | REQ-WIN-003 | 窗口状态持久化 | 12.0 | 已部分实现（窗口位置恢复） |
| 3 | REQ-EXP-003 | 导出为 HTML | 12.0 | 交互式导出 |
| 4 | REQ-HELP-003 | Tooltip 系统 | 11.3 | UI 渐进改进 |
| 5 | REQ-ERR-003 | 错误日志 | 9.6 | 已有 tracing 日志系统 |
| 6 | REQ-OBS-002 | 指标仪表盘 | 8.4 | 可观测性渐进改进 |
| 7 | REQ-SEC-001 | 安全审计导出 | 8.0 | 合规需求 |

**建议优先**：REQ-WIN-003（窗口状态持久化完善）+ REQ-EXP-003（HTML 导出）+ REQ-HELP-003（Tooltip）+ REQ-ERR-003（错误日志导出）+ REQ-I18N-002（日期本地化）

## 开发规范（必读）

- **前端源码**: 只改 `ui/src/*.js` / `ui/styles/*.css` / `ui/locales/*.json`，**不改 `ui/index.html`**（构建产物）
- **构建**: `node scripts/build-ui.mjs`（esbuild IIFE + 内联 CSS）
- **类型检查**: `npx tsc --noEmit`；新增 `window.__xxx` 需在 `types/dom-ext.d.ts` 声明
- **样式**: Tailwind utility + design tokens（`bg-surface-2` / `text-text-primary`），禁止硬编码 hex
- **IPC 封装**: 在 `ui/src/ipc.js` 添加 API 对象，参数名用 snake_case
- **面板管理**: `panel-stack.js` 的 `pushPanel()`/`removePanel()`，ESC 统一关闭
- **Z-index**: `zindex.js` 的 `zClass()`，禁止硬编码
- **生产代码**: 零 `unwrap`/`expect`/`panic`（workspace deny lint）
- **Rust 排序**: 用 `sort_by_key(|b| Reverse(b.1))` 而非 `sort_by(|a,b| b.1.cmp(&a.1))`（clippy lint）
- **文档同步**: 代码变更同步 `docs/api/` + `docs/SRS_v1.0.md` REQ 状态
- **GitHub 同步**: `./sync-to-github.sh --commit` 然后 `--push`

## 关键文件索引

| 文件 | 用途 |
|---|---|
| `docs/SRS_v1.0.md` | 需求规格（REQ 状态更新目标） |
| `docs/architecture/V1.5_ATOMIC_PLAN.md` | v1.5 原子计划（§5 有 18 个 v1.6 候选） |
| `ui/src/ipc.js` | IPC 封装（新增 API 对象在此添加） |
| `ui/src/main.js` | 前端入口（模块注册在此添加） |
| `ui/src/export.js` | PDF 导出（HTML 导出可复用 print 管线） |
| `ui/src/settings.js` | 设置面板（动态容器用链式 `.after()` 插入） |
| `ui/src/panel-stack.js` | 面板栈管理（pushPanel/removePanel） |
| `ui/src/zindex.js` | Z-index 常量表 |
| `types/dom-ext.d.ts` | Window 属性声明（E2E 暴露的 `__xxx` 需在此声明） |
| `e2e-tests/tests/bridge/tauri-stub.js` | Mock __TAURI__（新增 IPC mock 在此添加） |
| `e2e-tests/tests/helpers.mjs` | E2E 测试辅助（enterApp/importDocs/injectStub 等） |
| `crates/tauri-app/src/lib.rs` | IPC 命令注册（`generate_handler!`）+ 托盘图标 |
| `crates/tauri-app/src/commands/` | 14 功能域子模块（document.rs/conversation.rs 等） |
| `crates/tauri-app/Cargo.toml` | Tauri 依赖（已启用 `tray-icon` feature） |
| `crates/tauri-app/tauri.conf.json` | Tauri 配置（版本号在此修改） |
| `docs/api/ipc-commands.md` | IPC 文档 |
| `docs/api/frontend-modules.md` | 前端模块文档 |

## 验证命令

```bash
cargo clippy --all-targets -- -D warnings      # Rust lint
cargo fmt --check                               # Rust 格式
npx tsc --noEmit                                # 前端类型
node scripts/build-ui.mjs                       # 前端构建
cd e2e-tests && npx playwright test <spec> --reporter=list  # E2E
./sync-to-github.sh --commit                    # GitHub 提交
./sync-to-github.sh --push                      # GitHub 推送
```

## 验收标准

1. 选出的 v1.6 功能全部实现 + SRS REQ 状态 Draft → Implemented
2. `cargo clippy` + `cargo fmt` + `npx tsc` + `node scripts/build-ui.mjs` 全通过
3. 新增 E2E 测试通过
4. `docs/api/` 文档同步 + `docs/SRS_v1.0.md` changelog
5. 版本号 1.5.0 → 1.6.0（tauri.conf.json）
6. `./sync-to-github.sh` 执行
