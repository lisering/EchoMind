# S5 衔接提示词 — 综合行动方案实施

> 将此提示词完整粘贴到下一个 AI 会话中开始 S5 实施。

---

EchoMind — Rust + Tauri v2 本地 RAG 知识库。六边形架构 8 crate，版本 v1.20.0。250 SRS REQ 全部完成。当前暂停所有新功能开发，进入审计冻结期。核心理念：使用越简单，功能越强大。

## 已完成审计（S1-S4 四轮闭环）

**S1 前端模块审计**：79 个 JS 模块逐个评估，A(24)/B(24)/C(14)/D(12)/F(5)。零僵尸模块 ✅。5 个 F 级删除候选。报告：`docs/sessions/archive/audit_s1_frontend_modules.md`

**S2 IPC 命令审计**：236 个命令逐个评估，A(62)/B(58)/C(42)/D(38)/F(36)。零僵尸命令 ✅。11 个幻影调用 🔴。~45 借鉴命令无用户场景。报告：`docs/sessions/archive/audit_s2_ipc_commands.md`

**S3 UI 界面验收评估**：10 条标准全量审查。设置面板 **35 分区** 🔴🔴（目标 ≤ 8）、输入区 **5 toggle** 🔴（目标 ≤ 2）、侧栏 **8 按钮** 🟡（目标 ≤ 5）、**20 术语化设置项** 🔴、**13/20 功能默认关闭** 🔴、**60% 错误信息技术化** 🟡。首启 3 步 ✅、核心 ≤ 2 点击 ✅、面板栈 ✅、快捷键 ✅。报告：`docs/sessions/archive/audit_s3_ui_evaluation.md`

**S4 用户场景验收评估**：3 类画像 × 5 场景 = 15 场景全覆盖。U1-U8 评估 6/8 达标 + 2/8 基本达标。三大方案全部验证通过：智能模式 93.3% 覆盖 ✅、8 分区 1 点击可达 ✅、2 toggle 5/5 场景覆盖 ✅。8 个新发现（N1~N8）。报告：`docs/sessions/archive/audit_s4_user_scenarios.md`

## 四轮审计核心结论

EchoMind 功能矩阵完全覆盖三类用户核心场景（15/15 = 100%），但 UI 暴露过度导致 3 个场景有卡点。三大重整方案（智能模式 + 8 分区 + 2 toggle）经验证全部可行，能将场景流畅度从 80% 提升至 100%。

## 本 Session 目标

**S5：综合行动方案实施。** 基于 S1-S4 四轮审计结果，开始实施 P0 行动清单。本轮从审计转入实施。

## S4 验证的修正后 8 分区方案

S4 审计对 S3 方案提出 3 项修正（N3/N6/N7）：

| 新分区 # | 名称 | 修正后内容 | 变更说明 |
|---|---|---|---|
| 1 | 模型配置 | LLM 配置 + 模式 + 嵌入模型 + 模型缓存 + **VLM 图片理解** | ← N7: VLM 从分区 5 移入 |
| 2 | 外观与语言 | 主题 + 语言 + 语音 + PDF | 不变 |
| 3 | 知识库管理 | 文件同步 + 分块 + RAG 参数 | 不变 |
| 4 | 安全与隐私 | 安全 + 备份 | 不变 |
| 5 | 智能检索 | **智能模式单一开关** + Rerank + HyDE + Agent + 记忆 | ← N7: VLM 移出 |
| 6 | 高级调优 | Token + 采样 + 压缩比 + 实验性功能 | ← N6: PagedAttention/GEMV/KV Cache 移出 |
| 7 | 应用设置 | 窗口 + 自启 + 日志 + 更新 | 不变 |
| 8 | 快捷指令 | 模板管理 | 不变 |
| 隐藏 | 开发者菜单 | RAG 评估 + Trace + Durable + Todo + 记忆管理 + PagedAttention + GEMV + KV Cache + 错误日志 | ← N3/N6: 从用户设置移入 |

## S4 验证的修正后 2 Toggle 方案

| 保留 Toggle | 理由 | S4 验证 |
|---|---|---|
| **混合搜索** | 核心检索增强，始终开启型 | ✅ 5/5 场景覆盖 |
| **Agent 模式** | 核心推理增强，按需开启型 | ✅ 5/5 场景覆盖 |

移除的 3 个 toggle：sub_agent/memory/web_search → 移入设置面板

**N8 网页搜索替代方案**（P1 优先级）：
- 方案 A（推荐）：hover 显示第 3 个 toggle
- 方案 B：`/web` 斜杠命令
- 方案 C：智能模式自动判断

## S4 验证的智能模式方案

**智能模式 ON 时自动启用**：
- 混合搜索 = true
- Cross-Encoder 重排序 = true
- HyDE 查询改写 = true
- 知识图谱检索 = true
- 质量门控 = true
- 压缩比 = 2x
- Contextual Retrieval = true

**智能模式 OFF 时**：以上全部恢复用户手动设置值

**覆盖率**：93.3% 场景无需手动配置（新手 100% / 知识工作者 100% / 高级用户 80%）

## P0 行动清单（立即实施）

| # | 行动 | 来源 | 预计工作量 | 实施要点 |
|---|---|---|---|---|
| P0-1 | 智能模式单一开关 | S3 P0-4 + S4 U4 | 1 Session | 后端 `set_smart_mode` IPC + 前端 toggle + 默认开启 |
| P0-2 | 输入区 5 → 2 toggle | S3 P0-2 + S4 U6 | 0.5 Session | 保留 hybrid + agent，移除 sub_agent/memory/web_search |
| P0-3 | 设置面板 35 → 8 分区 | S3 P0-1 + S4 U5 | 2 Sessions | Tab 导航 + 合并 35 分区为 8 + 隐藏开发者菜单 |
| P0-4 | 20 术语通俗化 | S3 P0-3 + S4 U8 | 1 Session | i18n key 修改 + tooltip 补充 |
| P0-5 | 11 幻影调用修复 | S2 P0 | 0.5 Session | ipc.js 删除旧封装 + 迁移到新命令名 |
| P0-6 | 4 开发者工具移入隐藏菜单 | S3 P0-5 + S4 N3/N6 | 0.5 Session | rag-eval/trace/durable/session-strip 移入 ⌘Shift+D |

## P1 行动清单（后续优化）

| # | 行动 | 来源 | 预计工作量 |
|---|---|---|---|
| P1-1 | 顶栏 8 → 5 按钮 | S3 P1-1 | 1 Session |
| P1-2 | 错误信息统一分类 | S3 P1-2 | 1 Session |
| P1-3 | N8 网页搜索替代方案 | S4 N8 | 0.5 Session |
| P1-4 | N4/N5 Agent 发现性 | S4 N4/N5 | 0.5 Session |
| P1-5 | 5 个 F 级模块删除 | S1 P1 | 1 Session |
| P1-6 | N1 导出发现性 | S4 N1 | 0.5 Session |

## 实施入口文件

| 文件 | 用途 | P0 行动 |
|---|---|---|
| `ui/src/settings.js` | 设置面板主协调器 | P0-3 重整 8 分区 |
| `ui/src/input-toggles.js` | 输入区 toggle | P0-2 精简 2 toggle |
| `ui/src/perf-settings.js` | 性能设置 12 术语开关 | P0-1 智能模式替换 |
| `ui/src/settings-advanced.js` | 高级设置 1320 行 | P0-3 合并分区 |
| `ui/src/settings-general.js` | 通用设置 | P0-3 合并分区 |
| `ui/src/settings-local-llm.js` | 本地 LLM 设置 | P0-3 合并分区 |
| `ui/src/main.js` | 入口（toggle 创建 1281-1292 行） | P0-2 修改 toggle 创建 |
| `ui/src/ipc.js` | IPC 封装 | P0-5 修复幻影调用 |
| `ui/locales/zh-CN.json` | 中文 i18n | P0-4 术语通俗化 |
| `ui/locales/en.json` | 英文 i18n | P0-4 术语通俗化 |
| `crates/tauri-app/src/commands/settings.rs` | 后端设置默认值 | P0-1 智能模式默认值 |
| `crates/tauri-app/src/commands/performance.rs` | 性能 IPC 命令 | P0-1 智能模式联动 |
| `docs/sessions/archive/audit_s3_ui_evaluation.md` | S3 审计报告 | 参考 8 分区方案 |
| `docs/sessions/archive/audit_s4_user_scenarios.md` | S4 审计报告 | 参考 N1~N8 修正建议 |

## 测试基线

| 指标 | 数值 |
|---|---|
| Rust Free | 1282 passed, 0 failed |
| Rust Pro | 1573 passed, 0 failed |
| Clippy | 零警告 |
| Vitest | 430 passed |
| E2E | 1213 passed, 10 skipped |
| tsc / esbuild | 0 errors / 1730 行 |
| IPC 命令 | 224 Free + 12 Pro = 236 |
| 前端模块 | 79 |
| 设置面板分区 | 35（目标 ≤ 8） |
| 输入区 toggle | 5（目标 ≤ 2） |

## 规范

- 本轮从审计转入实施，开始写代码
- 如发现 bug 需修复，遵循 TDD
- 铁律五：生产代码严禁 `unwrap()/expect()/panic!()`
- 铁律十一：改源码后必须 `./sync-to-github.sh`
- 铁律十二：Session 结束必须更新 `CURRENT_STATE.md` + `LOG.md` + 生成衔接提示词
- 前端构建：`node scripts/build-ui.mjs`（esbuild 打包 IIFE）
- TypeScript：`npx tsc --noEmit` 类型检查

## 建议实施顺序

1. **P0-5 幻影调用修复**（0.5 Session）— 最简单且消除运行时崩溃
2. **P0-2 toggle 精简**（0.5 Session）— 改动小、效果立竿见影
3. **P0-1 智能模式**（1 Session）— 后端 + 前端联动
4. **P0-6 开发者工具隐藏**（0.5 Session）— 移入口即可
5. **P0-4 术语通俗化**（1 Session）— i18n key 替换
6. **P0-3 设置面板重整**（2 Sessions）— 最大的改动，放最后

## 产出要求

1. 实施后的源代码变更
2. 全量回归测试通过（Rust + Clippy + Vitest + E2E + tsc）
3. 更新 `docs/sessions/CURRENT_STATE.md`
4. 生成 S6 衔接提示词（P1 行动或下一阶段）
5. `./sync-to-github.sh` 同步
