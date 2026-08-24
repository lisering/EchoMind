# S6 衔接提示词 — P1 行动清单实施

## 当前状态

S5 综合行动方案 P0-1~P0-6 六项全部完成。审计冻结期 P0 行动清单已清零。

### S5 完成清单

| # | 行动 | 状态 | 关键变更 |
|---|---|---|---|
| P0-5 | 11 幻影调用修复 | ✅ | 4 后端命令实现 + 5 前端委托迁移 + get_document_chunks 新增 |
| P0-2 | 输入区 5→2 toggle | ✅ | 保留 hybrid+agent，移除 sub_agent/memory/web_search |
| P0-1 | 智能模式单一开关 | ✅ | set_smart_mode/get_smart_mode IPC + 默认开启 + 性能面板 toggle |
| P0-6 | 开发者工具隐藏 | ✅ | ⌘Shift+D 切换开发者模式，4 工具仅开发者可见 |
| P0-4 | 20 术语通俗化 | ✅ | zh-CN+en 双语 26 个 i18n key 更新 |
| P0-3 | 设置面板 8 分区 Tab | ✅ | 8 Tab 导航栏 + 滚动定位 + 智能模式联动 |

### 验证基线

| 指标 | 数值 |
|---|---|
| Clippy Free | 零警告 ✅ |
| cargo fmt | 通过 ✅ |
| tsc | 0 errors ✅ |
| esbuild | 1744 行 ✅ |
| Vitest | 437 passed ✅ |
| Rust Free tests | 全通过（除 bench_real_import_phases 网络依赖） |
| E2E | 运行中（Playwright headless） |
| IPC 命令 | 242（230 Free + 12 Pro） |
| 幻影调用 | 0 ✅ |

## S6 目标：P1 行动清单

| # | 行动 | 来源 | 预计工作量 |
|---|---|---|---|
| P1-1 | 顶栏 8→5 按钮 | S3 P1-1 | 1 Session |
| P1-2 | 错误信息统一分类 | S3 P1-2 | 1 Session |
| P1-3 | N8 网页搜索替代方案 | S4 N8 | 0.5 Session |
| P1-4 | N4/N5 Agent 发现性 | S4 N4/N5 | 0.5 Session |
| P1-5 | 5 个 F 级模块删除 | S1 P1 | 1 Session |
| P1-6 | N1 导出发现性 | S4 N1 | 0.5 Session |

### 建议实施顺序

1. P1-5（5 个 F 级模块删除）— 清理无用代码
2. P1-1（顶栏精简）— UI 简化
3. P1-2（错误信息统一）— 用户体验
4. P1-3（网页搜索替代）— 功能完善
5. P1-4（Agent 发现性）— 功能发现
6. P1-6（导出发现性）— 功能发现

### 关键文件

- `docs/sessions/archive/audit_s1_frontend_modules.md` — S1 审计报告（F 级模块清单）
- `docs/sessions/archive/audit_s3_ui_evaluation.md` — S3 审计报告（顶栏/错误信息）
- `docs/sessions/archive/audit_s4_user_scenarios.md` — S4 审计报告（N1~N8 发现）

### 注意事项

1. 智能模式默认开启 — 新用户无需手动配置即可获得最佳检索效果
2. 开发者模式通过 ⌘Shift+D 切换 — 不影响普通用户
3. 设置面板 8 Tab 导航 — 点击 Tab 滚动到对应分区
4. 幻影调用已全部修复 — 运行时不再崩溃
5. 术语已通俗化 — HyDE→智能改写、PagedAttention→分页注意力等
