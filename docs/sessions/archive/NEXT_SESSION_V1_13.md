# EchoMind v1.13 Session 提示词

> 复制本文件 --- 之间的全部内容作为新会话的第一条消息。

---

## 项目

EchoMind — Rust + Tauri v2 本地 RAG 知识库（律师/研究者高隐私场景）。六边形架构 8 crate，版本 v1.12.0，1257 Rust passed / 0 failed / 24 ignored。206 Free IPC / 214 Pro。esbuild 17330 行。

上一 Session（v1.12）已完成：REQ-EXP-004 文档原文导出 + REQ-VEC-009 索引重建前端 + REQ-I18N-003 数字本地化 + REQ-NAV-005 回到顶部 + SRS 修正（REQ-ING-012/REQ-VEC-011）。11 E2E 测试通过，GitHub 已同步 + git tag v1.12.0。

## 本 Session 任务：v1.13 规划 + 可实施项评估

v1.12 完成了文档管理深化 + UX 打磨。剩余 29 个 Draft REQ 需评估。

### 任务 1：评估 v1.13 候选功能

从 29 个 Draft REQ 中识别 v1.13 可实施项（ROI ≥ 5.0，单 Session 可完成）：

**高 ROI 候选**（预估）：
- REQ-VEC-010 知识库索引统计（P2，已有 get_kb_stats IPC，缺前端统计仪表盘）
- REQ-KB-005 快捷键帮助面板（P2，纯前端，信息展示类）
- REQ-WIN-004 开机自启（P2，Tauri 插件，1 文件改动）
- REQ-ERR-003 离线模式降级（P2，检测网络状态 + 友好提示）
- REQ-HELP-004 应用更新检查（P2，GitHub Releases API 对比版本号）
- REQ-IX-001 右键上下文菜单增强（P2，已有 context-menu.js 基础）
- REQ-NAV-004 面包屑与上下文指示（P2，纯前端 UI）
- REQ-OBS-002 性能指标采集（P2，已有 trace 系统，扩展指标）
- REQ-ING-011 导入历史记录（P2，已有 audit_logs 基础）
- REQ-NFR-019 构建产物体积（P2，分析 + 优化）

**评估维度**：用户价值 40% + 实现易度 30% + 架构收益 20% + 风险控制 10%。

### 任务 2：创建 `docs/architecture/V1.13_ATOMIC_PLAN.md`

含 ROI 矩阵 + 选定功能 + 实施步骤 + TDD 测试计划。格式参考 `docs/architecture/V1.12_ATOMIC_PLAN.md`。

### 任务 3：TDD 实现 + 文档同步 + 发布验证

按规划执行 TDD。验证：clippy + fmt + test + tsc + esbuild + E2E。同步 docs + CHANGELOG + `sync-to-github.sh --commit` + git tag。

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
| Rust Free | 1257 passed, 0 failed, 24 ignored |
| Clippy | 零警告 |
| cargo fmt | 通过 |
| TypeScript | 0 errors |
| esbuild | 17330 行 |
| E2E | 1071 passed, 10 skipped |
| Vitest | 427 passed, 3 pre-existing failures |
| 版本号 | 1.12.0 |
| IPC 命令 | 206 Free / 214 Pro |

## 关键文件

| 文件 | 用途 |
|---|---|
| `docs/sessions/CURRENT_STATE.md` | 项目状态快照（必读） |
| `docs/SRS_v1.0.md` | 需求规格（29 个 Draft REQ） |
| `docs/architecture/V1.12_ATOMIC_PLAN.md` | v1.12 规划（已完成） |
| `docs/CHANGELOG_v1.12.md` | v1.12 变更日志 |
| `.catpaw/rules/system-constraints.md` | 铁律 21 条 |
