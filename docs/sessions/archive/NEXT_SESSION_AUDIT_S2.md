# EchoMind 验收评估 S2 提示词 — 后端 IPC 命令全量审计

## 项目概要

EchoMind — Rust + Tauri v2 本地 RAG 知识库。六边形架构 8 crate，版本 v1.20.0。250 SRS REQ 全部完成。当前**暂停所有新功能开发**，进入全面复盘审计期。

核心理念：**使用越简单，功能越强大**。

## 上 Session 摘要（S1：前端模块全量审计）

### 审计结果

79 个前端 JS 模块逐个评估完成，产出等级 A-F + 处置方案。

**等级分布**：

| 等级 | 数量 | 占比 |
|---|---|---|
| A | 24 | 30% |
| B | 24 | 30% |
| C | 14 | 18% |
| D | 12 | 15% |
| F | 5 | 6% |

**关键发现**：
- ✅ **零僵尸模块**：所有 79 个模块均被至少 1 个其他模块导入
- 🔴 **设置爆炸**：9 个分区（目标 ≤ 8），settings-advanced 内部 13 个子区域
- 🔴 **Toggle 过多**：5 个 toggle（目标 ≤ 2），3 个用户无法理解
- 🔴 **性能开关过度暴露**：12 个学术术语开关全部默认关闭
- 🔴 **5 个 F 级删除候选**：workflow-panel / code-executor / durable-panel / session-strip / perf-settings
- 🔴 **开发者工具混入用户设置**：rag-eval / trace-panel / durable-panel / session-strip

**审计产出**：`docs/sessions/archive/audit_s1_frontend_modules.md`

## 本 Session 目标

**Phase 1 审计冻结期 S2：后端 IPC 命令全量审计**

逐个审查 220 Free / 230 Pro IPC 命令，为每个命令产出评估等级和处置方案。**不写任何新功能代码**，只审计。

## 评估标准（10 条）

| 编号 | 标准 | 目标 |
|---|---|---|
| S3 | 设置面板 ≤ 8 个分区 | 当前 9 个 🔴 |
| S7 | 设置项使用人类语言 | 术语化 🔴 |
| S8 | 默认配置即最佳配置 | 12 个开关默认关闭 🔴 |
| C2 | 无僵尸 IPC（每个命令有前端调用者） | Zombie Zero ✅ 但需验证 |
| C3 | 无过度设计（借鉴功能有用户 UI） | ~40 借鉴 IPC 🔴 |
| C5 | 设置项有用户价值 | 30+ 设置区 🔴 |
| I1 | IPC 命名用户友好 | 检查术语化命名 |
| I2 | IPC 命令有前端调用者 | grep 前端 invoke 调用 |
| I3 | IPC 分组合理 | 按功能域分组 |
| I4 | Pro 门控合理 | Pro 命令确实需要 Pro |

## 审计方法

对每个 IPC 命令执行以下检查：

1. **前端调用者**：grep 搜索 `invoke('命令名'` 确认前端有调用
2. **命令分组**：确认属于哪个功能域子模块（chat / import / document 等）
3. **Pro 门控**：确认是否 `#[cfg(feature = "pro")]` 门控
4. **用户可见性**：是否有 UI 入口？用户能否感知到？
5. **借鉴来源**：是否为借鉴功能（QM / OpenCode / DS4 / OpenMontage）？
6. **用户场景**：用户什么时候会用到？场景是否真实？
7. **评估等级**：A(优秀) / B(良好) / C(需改进) / D(不合格) / F(需删除)
8. **处置方案**：保持 / 优化 / 隐藏 / 删除

## 已知问题热点（优先审查）

| IPC 命令群 | 命令数 | 问题 |
|---|---|---|
| QM 借鉴（security_posture / shadow_screen / scratch / burst_buffer） | ~9 | 用户场景不明确 |
| OpenCode 借鉴（admit_input / promote_input / session_todo） | ~8 | 用户场景不明确 |
| DS4 借鉴（strip_messages / strip_keeping_recent / preview_strip） | ~3 | 用户场景不明确 |
| Cherry Studio 借鉴（trace / budget） | ~6 | 开发者工具混入用户设置 |
| RAG 评估（evaluate_rag_response 等） | ~4 | 开发者工具 |
| Durable 输入 | ~3 | 无用户场景 |
| Session Todo | ~5 | 无用户场景 |
| Budget 追踪 | ~2 | 开发者工具 |

## 当前测试基线

| 指标 | 数值 |
|---|---|
| Rust Free | 1282 passed, 0 failed, 31 ignored |
| Rust Pro | 1573 passed, 0 failed, 48 ignored |
| Clippy Free+Pro | 零警告 |
| Vitest | 430 passed |
| E2E | 1213 passed, 10 skipped |
| tsc / esbuild | 0 errors / 1730 行 |
| IPC 命令数 | 220 Free / 230 Pro |

## 完整规划文档

详细规划见 `docs/architecture/COMPREHENSIVE_ACCEPTANCE_EVALUATION_PLAN.md`（第三章 IPC 命令全量审计）。

## 审计入口

1. `crates/tauri-app/src/lib.rs` — `generate_handler!` 宏注册全部 220/230 IPC 命令
2. `crates/tauri-app/src/commands/` — 14 功能域子模块
3. `ui/src/ipc.js` — 前端 IPC 封装（invoke + API 命名空间）
4. `e2e-tests/tests/bridge/tauri-stub.js` — E2E mock（117+ IPC mock）

## 开发规范

- **本次只审计，不写新功能代码**
- 如审计中发现 bug 需修复，遵循 TDD
- 铁律十一：改源码后必须 `./sync-to-github.sh`

## 产出要求

Session 结束时产出：

1. `docs/sessions/archive/audit_s2_ipc_commands.md` — 220/230 个 IPC 命令逐个评估报告
2. 更新 `docs/sessions/CURRENT_STATE.md`
3. 更新 `docs/architecture/COMPREHENSIVE_ACCEPTANCE_EVALUATION_PLAN.md` 附录 B/C
4. 生成 S3 衔接提示词

## 关键文件

| 文件 | 用途 |
|---|---|
| `crates/tauri-app/src/lib.rs` | IPC 命令注册（generate_handler!） |
| `crates/tauri-app/src/commands/` | 14 功能域子模块 |
| `ui/src/ipc.js` | 前端 IPC 封装 |
| `docs/api/ipc-commands.md` | IPC 命令签名文档 |
| `docs/sessions/archive/audit_s1_frontend_modules.md` | S1 审计报告（参考） |
