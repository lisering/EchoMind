# S7 衔接提示词 — 下一阶段规划

> **生成时间**: 2026-08-13 S6 结束
> **当前版本**: v1.20.0
> **上一 Session**: S6 P1 行动清单实施（6 项全部完成）

## 当前状态

S6 完成了 P1 行动清单的全部 6 项优化：

| # | 行动 | 状态 | 关键产出 |
|---|---|---|---|
| P1-5 | F 级模块删除 | ✅ | 移除 workflow-panel/code-executor/durable-panel/session-strip 引用 |
| P1-1 | 顶栏 8→5 精简 | ✅ | ToolsDropdown 收纳 4 个低频按钮 |
| P1-2 | 错误信息通俗化 | ✅ | 新增 5 类错误规则 + 3 类现有错误通俗化 |
| P1-3 | 网页搜索替代 | ✅ | /web 斜杠命令实际启用网页搜索 |
| P1-4 | Agent 发现性 | ✅ | /agent 斜杠命令 + toggle tooltip |
| P1-6 | 导出发现性 | ✅ | ⌘E 快捷键 + 键盘帮助面板 |

## 测试基线

| 指标 | 数值 |
|---|---|
| Rust Free | 1269 passed, 0 failed, 23 ignored |
| Clippy | 零警告 |
| cargo fmt | 通过 |
| Vitest | 437 passed, 0 failed |
| tsc | 0 errors |
| esbuild | 1575 行 |

## 下一阶段建议

### 方向 A：版本发布 v1.22.0
- 整理 CHANGELOG
- 版本号更新（全部 crate + tauri.conf.json）
- Release build 验证
- GitHub tag 创建

### 方向 B：P2 行动清单
S3/S4 审计中标记为 P2 的项目：
- P2-1: N2 HTML 导出完整性验证
- P2-2: N7 VLM 设置分区归属调整
- P2-3: 设置面板 8 分区 Tab 导航改进（滚动定位优化）
- P2-4: 斜杠命令面板键盘导航优化

### 方向 C：新功能规划 v1.23
- 参考 docs/SRS_v1.0.md 中 Needed REQ
- 参考 V1.14_ATOMIC_PLAN.md 未实施项

## 关键文件

| 文件 | 用途 |
|---|---|
| `ui/src/error-detail.js` | P1-2 错误分类规则（11 类） |
| `ui/src/slash-commands.js` | P1-3/P1-4 斜杠命令（8 个） |
| `ui/src/chat.js` | P1-3/P1-4 临时启用逻辑 |
| `ui/src/input-toggles.js` | P1-4 toggle tooltip |
| `ui/src/action.js` | P1-6 ⌘E 快捷键注册 |
| `ui/src/keyboard-help.js` | P1-6 快捷键帮助面板 |
| `ui/index.html` | P1-1 顶栏 ToolsDropdown |

## 注意事项

1. E2E 测试在后台运行中，结果待确认
2. `ui/index.html` 已通过 build-ui.mjs 自动同步到发布仓库
3. 其他源文件需手动 `./sync-to-github.sh` 同步
4. 前端模块数从 79 减至 78（删除 F 级模块引用，但模块文件仍存在）
