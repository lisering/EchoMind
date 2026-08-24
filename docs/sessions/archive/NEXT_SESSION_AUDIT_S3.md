# EchoMind 验收评估 S3 提示词 — UI 界面验收评估

## 项目概要

EchoMind — Rust + Tauri v2 本地 RAG 知识库。六边形架构 8 crate，版本 v1.20.0。250 SRS REQ 全部完成。当前**暂停所有新功能开发**，进入全面复盘审计期。

核心理念：**使用越简单，功能越强大**。

## 上 Session 摘要（S2：后端 IPC 命令全量审计）

### 审计结果

236 个 IPC 命令（224 Free + 12 Pro-only）逐个评估完成，产出等级 A-F + 处置方案。

**等级分布**：

| 等级 | 数量 | 占比 |
|---|---|---|
| A | 62 | 26% |
| B | 58 | 25% |
| C | 42 | 18% |
| D | 38 | 16% |
| F | 36 | 15% |

**关键发现**：
- ✅ **零僵尸命令**：全部 236 个注册命令有前端调用者
- 🔴 **11 个幻影调用**：前端调用后端未注册命令（encrypt_database / unlock_database / redact_pii 等），运行时崩溃
- 🔴 **~65 个借鉴命令**（~45 无用户场景）：QM(22) / OpenCode(10) / DS4(3) / Cherry Studio(10) / RAGAS(4) / 学术(16)
- 🔴 **21 个开发者工具混入用户功能**：trace(4) + budget(2) + rag_eval(4) + durable(3) + session_todo(5) + strip(3)
- 🔴 **19 个性能优化命令全部默认关闭**
- 🟠 **~30 对 set/get 命令**可合并为 ~15 个

**审计产出**：`docs/sessions/archive/audit_s2_ipc_commands.md`

## 本 Session 目标

**Phase 1 审计冻结期 S3：UI 界面验收评估**

从用户视角审查全部 UI 界面元素，评估交互简洁度和用户旅程完整性。**不写任何新功能代码**，只审计。

## 评估标准（10 条）

| 编号 | 标准 | 目标 |
|---|---|---|
| S1 | 首次使用 ≤ 3 步开始对话 | 当前 3 步 ✅ |
| S2 | 核心操作路径 ≤ 2 次点击 | 导入 2 点击 / 提问 1 点击 ✅ |
| S3 | 设置面板 ≤ 8 个分区 | 当前 9 个 🔴 |
| S4 | 输入区 toggle ≤ 2 个 | 当前 5 个 🔴 |
| S5 | 侧栏功能按钮 ≤ 5 个 | 当前 7+ 个 🟡 |
| S6 | 同时只显示 1 个 overlay 面板 | panel-stack 管理 ✅ |
| S7 | 设置项使用人类语言 | 术语化 🔴 |
| S8 | 默认配置即最佳配置 | 12 个开关默认关闭 🔴 |
| S9 | 错误信息可理解 + 提供恢复建议 | 部分技术化 🔡 |
| S10 | 快捷键覆盖核心操作 | 已有快捷键 ✅ |

## 审计方法

1. **界面截图审查**：逐个界面截图，评估元素必要性
2. **设置面板分区审计**：统计当前分区数，识别可合并/移除的分区
3. **Toggle 数量审计**：统计输入区 toggle，识别可移除的 toggle
4. **侧栏按钮审计**：统计侧栏功能按钮，识别可移除的按钮
5. **用户旅程走查**：从新用户到高级用户的完整旅程
6. **交互路径长度**：统计核心操作的点击次数
7. **设置项术语化审计**：识别用户无法理解的设置项
8. **默认配置审计**：统计默认关闭的功能开关

## 审计入口

1. `ui/index.html` — 前端 SPA 构建产物（查看 HTML 结构）
2. `ui/src/main.js` — 入口编排（查看初始化流程）
3. `ui/src/settings.js` + `settings-general.js` + `settings-advanced.js` — 设置面板
4. `ui/src/input-toggles.js` — 输入区 toggle
5. `ui/src/sidebar.js` — 侧栏
6. `ui/src/perf-settings.js` — 性能设置面板
7. S1 审计报告 `docs/sessions/archive/audit_s1_frontend_modules.md` — 前端模块评估
8. S2 审计报告 `docs/sessions/archive/audit_s2_ipc_commands.md` — IPC 命令评估

## 已知问题热点（优先审查）

| 界面区域 | 问题 | 来源 |
|---|---|---|
| 设置面板分区数 | 9 个分区（目标 ≤ 8） | S1 审计 |
| 输入区 toggle | 5 个（目标 ≤ 2） | S1 审计 |
| 侧栏按钮 | 7+ 个（目标 ≤ 5） | S1 审计 |
| 性能设置面板 | 12 个术语化开关 | S1 + S2 审计 |
| 开发者工具面板 | trace / rag-eval / durable / session-strip 混入用户设置 | S1 + S2 审计 |
| 设置项术语化 | Speculative / Proposition / ColBERT / RAPTOR 等 | S1 + S2 审计 |
| 幻影调用 UI | encrypt_database / unlock_database 等 11 个调用无后端 | S2 审计 |

## 完整规划文档

详细规划见 `docs/architecture/COMPREHENSIVE_ACCEPTANCE_EVALUATION_PLAN.md`（第四章 UI 界面验收评估）。

## 开发规范

- **本次只审计，不写新功能代码**
- 如审计中发现 bug 需修复，遵循 TDD
- 铁律十一：改源码后必须 `./sync-to-github.sh`

## 产出要求

Session 结束时产出：

1. `docs/sessions/archive/audit_s3_ui_evaluation.md` — UI 界面验收评估报告
2. 更新 `docs/sessions/CURRENT_STATE.md`
3. 更新 `docs/architecture/COMPREHENSIVE_ACCEPTANCE_EVALUATION_PLAN.md` 附录 D
4. 生成 S4 衔接提示词（用户场景验收评估）

## 当前测试基线

| 指标 | 数值 |
|---|---|
| Rust Free | 1282 passed, 0 failed, 31 ignored |
| Rust Pro | 1573 passed, 0 failed, 48 ignored |
| Clippy Free+Pro | 零警告 |
| Vitest | 430 passed |
| E2E | 1213 passed, 10 skipped |
| tsc / esbuild | 0 errors / 1730 行 |
| IPC 命令数 | 224 Free + 12 Pro-only = 236 |
| 前端模块数 | 79 |
