# EchoMind 验收评估 S1 提示词

## 项目概要

EchoMind — Rust + Tauri v2 本地 RAG 知识库。六边形架构 8 crate，版本 v1.20.0。250 SRS REQ 全部完成。当前**暂停所有新功能开发**，进入全面复盘审计期。

核心理念：**使用越简单，功能越强大**。

## 本 Session 目标

**Phase 1 审计冻结期 S1：前端模块逐个审查**

逐个审查 79 个前端 JS 模块（`ui/src/*.js`），为每个模块产出评估等级（A-F）和处置方案。**不写任何新功能代码**，只审计。

## 评估标准（10 条）

| 编号 | 标准 | 目标 |
|---|---|---|
| S1 | 核心操作路径 ≤ 2 次点击 | — |
| S3 | 设置面板 ≤ 8 个分区（当前 30 个 🔴） | — |
| S4 | 输入区 toggle ≤ 2 个（当前 5 个 🔴） | — |
| S5 | 侧栏按钮 ≤ 5 个（当前 7+ 🟡） | — |
| S7 | 设置项使用人类语言（不用学术术语） | — |
| S8 | 默认配置即最佳配置（12 个性能开关默认关闭 🔴） | — |
| C1 | 无僵尸模块（每个模块被导入） | 0 僵尸 |
| C3 | 无过度设计（借鉴功能有用户 UI） | — |
| C4 | 模块职责清晰（≤ 500 行） | — |
| C5 | 设置项有用户价值 | — |

## 审计方法

对每个模块执行以下检查：
1. **被导入情况**：grep 搜索 `import.*模块名` 确认被谁导入
2. **行数**：`wc -l` 确认是否过大或过小
3. **职责**：读头部注释确认职责是否清晰
4. **用户可见性**：是否有 UI 入口？用户能否感知到？
5. **用户场景**：用户什么时候会用到？场景是否真实？
6. **评估等级**：A(优秀) / B(良好) / C(需改进) / D(不合格) / F(需重写)
7. **处置方案**：保持 / 优化 / 拆分 / 合并 / 隐藏 / 删除

## 已知问题热点（优先审查）

| 模块 | 行数 | 问题 |
|---|---|---|
| `main.js` | 1604 | 过大，职责过多，导入了几乎所有模块 |
| `settings-advanced.js` | 1320 | 过大，13 个高级设置区域 |
| `settings.js` | 414 | openSettings 渲染 20+ 子区域，设置爆炸 |
| `perf-settings.js` | 404 | 12 个性能开关，全部默认关闭 |
| `input-toggles.js` | 170 | 5 个 toggle 用户无法理解 |
| `durable-panel.js` | 245 | 借鉴功能，用户场景不明确 |
| `session-strip.js` | 260 | DS4 借鉴，场景不明确 |
| `trace-panel.js` | 400 | 开发者工具混入用户设置 |
| `rag-eval.js` | 316 | 开发者工具混入用户设置 |
| `workflow-panel.js` | 330 | DAG 工作流场景不明确 |
| `code-executor.js` | 268 | RAG 应用里执行代码？ |
| `dream-panel.js` | 244 | AutoDream 场景模糊 |
| `memory-panel.js` | 447 | Wing/Hall/Room 三层用户无法理解 |

## 当前测试基线

| 指标 | 数值 |
|---|---|
| Rust Free | 1282 passed, 0 failed, 31 ignored |
| Rust Pro | 1573 passed, 0 failed, 48 ignored |
| Clippy Free+Pro | 零警告 |
| Vitest | 430 passed |
| E2E | 1213 passed, 10 skipped |
| tsc / esbuild | 0 errors / 1730 行 |
| 前端模块数 | 79 |
| IPC 命令数 | 220 Free / 230 Pro |

## 完整规划文档

详细规划见 `docs/architecture/COMPREHENSIVE_ACCEPTANCE_EVALUATION_PLAN.md`（10 章 + 4 附录，覆盖 79 前端模块 + 220 IPC + 89 Rust 模块的全量审计 + 6 Phase 18 Session 路线图）。

## 开发规范（铁律摘要）

- **本次只审计，不写新功能代码**
- TDD 先红后绿（如果审计中发现 bug 需修复）
- 零 unwrap/expect/panic
- 改前端后 `node scripts/build-ui.mjs` 重建
- 铁律十一：改源码后必须 `./sync-to-github.sh`

## 验证命令

```bash
cargo clippy --all-targets -- -D warnings
cargo fmt --check
npx tsc --noEmit
node scripts/build-ui.mjs
cd e2e-tests && npm run test:unit
cd e2e-tests && npx playwright test --reporter=line
```

## 产出要求

Session 结束时产出：
1. `docs/sessions/archive/audit_s1_frontend_modules.md` — 79 个模块逐个评估报告
2. 更新 `docs/sessions/CURRENT_STATE.md`
3. 更新 `docs/architecture/COMPREHENSIVE_ACCEPTANCE_EVALUATION_PLAN.md` 附录 A 的评估列
4. 生成 S2 衔接提示词
