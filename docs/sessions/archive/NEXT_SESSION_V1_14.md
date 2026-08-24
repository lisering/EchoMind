# EchoMind v1.14 Session 提示词

> 复制本文件 --- 之间的全部内容作为新会话的第一条消息。

---

## 项目

EchoMind — Rust + Tauri v2 本地 RAG 知识库（律师/研究者高隐私场景）。六边形架构 8 crate，版本 v1.13.0，1276 Rust passed / 0 failed / 24 ignored。209 Free IPC / 217 Pro。esbuild 17704 行。

上一 Session（v1.13）已完成：REQ-ERR-003 离线模式降级 + REQ-WIN-004 开机自启 + REQ-HELP-004 应用更新检查 + REQ-OBS-002 性能指标采集 + REQ-NAV-004 面包屑导航。13 E2E 测试通过，GitHub 已同步 + git tag v1.13.0。

## 本 Session 任务：v1.14 规划 + 可实施项评估

v1.13 完成了 UX 增强 + 系统能力。剩余约 24 个 Draft REQ 需评估。

### 任务 1：评估 v1.14 候选功能

从 24 个 Draft REQ 中识别 v1.14 可实施项（ROI ≥ 5.0，单 Session 可完成）：

高 ROI 候选（预估）：

- REQ-VEC-010 知识库索引统计仪表盘（P2，已有 get_kb_stats IPC，缺前端统计图表）
- REQ-KB-005 快捷键帮助面板（P2，纯前端，信息展示类）
- REQ-IX-001 右键上下文菜单增强（P2，已有 context-menu.js 基础）
- REQ-ING-011 导入历史记录（P2，已有 audit_logs 基础）
- REQ-NFR-019 构建产物体积优化（P2，分析 + 优化）
- REQ-ERR-004 数据目录管理增强（P2，已有 open_data_dir IPC）
- REQ-DS-006 对话恢复与草稿持久化（P2，已有 input-history.js 基础）
- REQ-KB-004 文档列表快捷键（P2，纯前端键盘交互）
- REQ-UI-006 窗口标题动态显示（P2，1 文件改动）
- REQ-SEC-011 安全审计报告导出（P2，已有审计日志基础）

评估维度：用户价值 40% + 实现易度 30% + 架构收益 20% + 风险控制 10%。

### 任务 2：创建 `docs/architecture/V1.14_ATOMIC_PLAN.md`

含 ROI 矩阵 + 选定功能 + 实施步骤 + TDD 测试计划。格式参考 `docs/architecture/V1.13_ATOMIC_PLAN.md`。

### 任务 3：TDD 实现 + 文档同步 + 发布验证

按规划执行 TDD。验证：clippy + fmt + test + tsc + esbuild + E2E。同步 docs + CHANGELOG + sync-to-github.sh --commit + git tag。

## 开发规范

- TDD（铁律二）：先写红灯测试再实现
- 零 unwrap/expect/panic（workspace deny lint）
- forbid unsafe
- 文档同步（铁律七）：代码变更同步 docs
- GitHub 同步（铁律十一）：会话结束前 `./sync-to-github.sh`
- Tailwind 样式规范（铁律十三）：用 design tokens，禁止硬编码 hex
- Spec-driven（铁律十五）：先写规格再写代码

## 验证命令

```bash
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo test
npx tsc --noEmit
node scripts/build-ui.mjs
cd e2e-tests && npm run test:bridge
./sync-to-github.sh --commit
```

## 当前测试基线

| 指标 | 数值 |
|---|---|
| Rust Free | 1276 passed, 0 failed, 24 ignored |
| Clippy | 零警告 |
| cargo fmt | 通过 |
| TypeScript | 0 errors |
| esbuild | 17704 行 |
| E2E | 1084 passed, 10 skipped |
| 版本号 | 1.13.0 |
| IPC 命令 | 209 Free / 217 Pro |

## v1.13 已完成功能摘要

| 功能 | REQ | 关键文件 |
|---|---|---|
| 离线模式降级 | REQ-ERR-003 | `ui/src/offline-detector.js`（新建） |
| 开机自启 | REQ-WIN-004 | `tauri-plugin-autostart` + `commands/settings.rs`（get/set_autostart） |
| 应用更新检查 | REQ-HELP-004 | `crates/core/src/update_check.rs`（新建，14 TDD 测试）+ `commands/settings.rs`（check_for_updates 等 3 IPC） |
| 性能指标采集 | REQ-OBS-002 | `crates/core/src/perf_metrics.rs`（新建，5 TDD 测试） |
| 面包屑导航 | REQ-NAV-004 | `ui/src/breadcrumb.js`（新建）+ `ui/index.html`（#breadcrumbBar） |

## 关键文件

| 文件 | 用途 |
|---|---|
| `docs/sessions/CURRENT_STATE.md` | 项目状态快照（必读） |
| `docs/SRS_v1.0.md` | 需求规格（24 个 Draft REQ） |
| `docs/architecture/V1.13_ATOMIC_PLAN.md` | v1.13 规划（已完成） |
| `docs/CHANGELOG_v1.13.md` | v1.13 变更日志 |
| `.catpaw/rules/system-constraints.md` | 铁律 21 条 |
