# S8 会话提示词

你是 CatPaw，在 CatPaw IDE 中工作。用中文回答（代码除外），简洁直接。

## 项目

EchoMind — Rust + Tauri v2 本地 RAG 知识库。六边形架构 8 crate，版本 v1.20.0，250 SRS REQ 全部完成。

已完成 S1-S7 七轮审计+实施闭环：
- S1 前端模块审计（79 模块逐个评估，5 个 F 级删除候选）
- S2 IPC 命令审计（236 命令，零僵尸命令 ✅，11 幻影调用修复）
- S3 UI 界面验收（10 标准，设置 35→8 分区，toggle 5→2）
- S4 用户场景验收（15 场景 100% 覆盖，8 个新发现 N1~N8）
- S5 P0 行动（6 项全部完成）
- S6 P1 行动（6 项全部完成）
- S7 P2 行动（4 项全部完成）

## S7 完成内容

| 项 | 内容 |
|---|---|
| P2-1 | HTML 导出完整性增强：hljs 语法高亮 + Mermaid 标注 + CSS 主题 + 预加载 loadHighlight() |
| P2-2 | VLM 设置分区从「智能检索」移入「模型配置」（LLM 配置之后） |
| P2-3 | 设置面板 Tab 滚动定位优化：scroll-padding-top: 52px 防 sticky 遮挡 |
| P2-4 | 斜杠命令键盘导航：Tab 选择 + Home/End 跳转 + 3 个新增单元测试 |

## 测试基线

| 指标 | 数值 |
|---|---|
| Rust Free | 1269 passed, 0 failed, 23 ignored |
| Clippy | 零警告 |
| cargo fmt | 通过 |
| Vitest | 440 passed, 0 failed（+3 P2-4 新增） |
| tsc | 0 errors |
| esbuild | 1622 行 |
| E2E 全量 | 1163 passed, 205 failed (pre-existing), 10 skipped |
| E2E 相关 | 导出 12/12 ✅、设置/VLM 9/9 ✅、斜杠命令 8/8 ✅ |
| IPC 命令 | 242（230 Free + 12 Pro） |
| 幻影调用 | 0 ✅ |

## 已知问题（S5/S6 遗留）

E2E 有 205 个失败，均为 S5/S6 UI 重构遗留（非 S7 引入）：
- dream-panel/trace-panel/graph-viewer 测试（S6 P1-1 顶栏重构 + P1-5 模块删除）
- audit/bookmark/document-action 测试（UI 结构变更）
- enter-vs-button 部分测试（可能 flaky）
- 需要在后续 session 中更新这些 E2E 测试以匹配新 UI 结构

## 开发规范

- 铁律五：生产代码严禁 unwrap()/expect()/panic!()
- 铁律十一：改源码后必须 ./sync-to-github.sh
- 前端构建：node scripts/build-ui.mjs
- TypeScript：npx tsc --noEmit
- E2E：cd e2e-tests && E2E_SPEED=0.2 npm run test:bridge

## 下一阶段建议

### 方向 A：E2E 测试修复（推荐优先）
- 修复 205 个 pre-existing E2E 失败（dream-panel/trace-panel/graph-viewer/audit 等）
- 更新测试选择器以匹配 S5/S6 后的 UI 结构
- 目标：E2E 恢复到 0 failed

### 方向 B：版本发布 v1.22.0
- 更新版本号（全部 crate Cargo.toml + tauri.conf.json）
- 整理 CHANGELOG（S5/S6/S7 变更）
- Release build 验证 + git tag

### 方向 C：新功能规划 v1.23
- 参考 docs/SRS_v1.0.md 中 Needed REQ
- 参考 V1.14_ATOMIC_PLAN.md 未实施项

## 关键文件

| 文件 | 用途 |
|---|---|
| docs/sessions/CURRENT_STATE.md | 项目状态快照（必读） |
| docs/sessions/archive/audit_s1~s4*.md | S1-S4 审计报告 |
| ui/src/export.js | 导出模块（P2-1 增强：hljs 高亮 + Mermaid 标注） |
| ui/src/error-detail.js | 11 类错误分类规则 |
| ui/src/slash-commands.js | 8 个斜杠命令 + Home/End 导航（P2-4） |
| ui/src/chat.js | /web + /agent 临时启用逻辑 |
| ui/src/action.js | 快捷键 Action Registry |
| ui/src/input-toggles.js | 2 个 toggle（hybrid+agent）+ tooltip |
| ui/src/input-keymap.js | 输入键映射（P2-4: Tab/Home/End 导航） |
| ui/src/settings.js | 设置面板 8 分区 Tab + scroll-padding-top（P2-3） |
| ui/index.html | 顶栏 5 按钮 + ToolsDropdown + VLM 移至模型配置（P2-2） |
