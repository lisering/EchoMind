# Session 3 衔接提示词 — 验证收尾 + E2E 测试 + 下阶段规划

## 上下文

Session 1 完成了 P0 级修复（Z-index 分层 + ESC 面板栈管理器），Session 2 完成了 P1 级修复（Focus Trap 栈 + State 补全 + 会话切换关闭面板 + 输入区 keydown 统一处理器）。所有四个方案的代码改造已全部完成，TypeScript 类型检查零错误，esbuild 打包成功（13582 行）。

### Session 1 已完成的改造

**新建文件**：
- `ui/src/zindex.js` — 统一 Z-index 常量 + `zClass()` 辅助函数
- `ui/src/panel-stack.js` — 全局面板栈（push/remove/closeTop/peek/isOpen/hasOpen/clear）

**改造的模块（16 个）**：security.js / graph-viewer.js / dream-panel.js / code-executor.js / symbol-search.js / workflow-panel.js / confirm-dialog.js / turn-tree.js / paywall.js / settings.js / command-palette.js / sidebar.js / download-manager.js / keyboard.js / graph-viewer.css / components.css

### Session 2 已完成的改造

**方案3：Focus Trap 栈管理（`ui/src/focus-trap.js`）**
- 全局 `_trapStack` 数组（LIFO），只激活栈顶 Trap
- `activate()`: 暂停栈中所有已激活的 Trap → push 新 Trap → 激活
- `deactivate()`: 从栈中移除 → 恢复栈顶 Trap
- `_pause()` / `_resume()`: 暂停/恢复 Trap 但不从栈中移除
- API 向后兼容：现有调用方不需要改动

**方案4：State.js 状态补全（`ui/src/state.js` + `ui/src/input-toggles.js` + `ui/src/settings.js` + `ui/src/main.js`）**
- `state.js` 新增字段：`hybridEnabled` / `agentEnabled` / `subAgentEnabled` / `memoryEnabled` / `webSearchEnabled` / `docList`
- `input-toggles.js` 删除模块局部变量 `_toggleStates`，改为通过 `get()` / `setState()` / `subscribe()` 管理状态
- 乐观 UI 更新 + IPC 错误时回滚
- `settings.js` 的 `openSettings()` 从 `get_settings` IPC 同步状态到 `state.js`
- `main.js` 的 `boot()` 初始化所有 toggle 状态

**方案5：会话切换时关闭面板（`ui/src/main.js`）**
- 新建 `closeAllPanels()` 函数，关闭所有动态面板
- 在 `newChat()` 和 `loadConversation()` 开头（streaming 中断之后）调用
- 不关闭 `lock-overlay`（安全相关）、`confirm-dialog`（同步 Promise）、`dbError` Modal

**方案6：输入区 keydown 统一处理器（新建 `ui/src/input-keymap.js`）**
- `createInputKeyHandler(handlers)` 工厂函数，统一处理输入区 keydown 事件
- 合并 `main.js` 和 `chat.js` 中 `queryInput` 的两个 keydown 监听器
- 优先级：斜杠命令导航 > Enter 发送 > 历史导航 > Escape 重置
- `main.js` 删除原有 keydown 监听器，改为 `const inputKeyHandler = createInputKeyHandler({ send })`

**新建文件**：
- `ui/src/input-keymap.js` — 统一输入 keydown 处理器
- `ui/src/input-history.js` — 输入历史导航 + 草稿持久化 + Token 估算
- `ui/src/zindex.js` — Z-index 常量（Session 1）
- `ui/src/panel-stack.js` — 全局面板栈（Session 1）

### 验证结果
- `npx tsc --noEmit`: **0 errors** ✅
- `node scripts/build-ui.mjs`: **成功，13582 行** ✅
- Linter: **0 errors** ✅

---

## Session 3 任务清单

### 任务 1：E2E 测试验证与修复（2h）

**目标**：运行 E2E 桥接测试，验证 Session 1+2 的改造没有引入回归，修复发现的问题。

**执行步骤**：
1. `cd e2e-tests && npm install && npm run test:bridge` — 运行全部 E2E 桥接测试
2. 重点关注以下测试文件：
   - `keyboard-a11y.spec.ts` — Focus Trap 栈行为
   - `deepseek-style.spec.ts` — Toggle + 输入区交互
   - `theme.spec.ts` — 主题切换（验证 closeAllPanels 不影响主题）
   - `navigation-advanced.spec.ts` — 面板导航 + ESC 关闭
   - `interaction-feedback.spec.ts` — Toggle 反馈
3. 修复所有回归失败（预期 0 回归，但需验证）
4. 如有 pre-existing 失败，记录但不修复（非本次改动引入）

**验收标准**：
- E2E 桥接测试 0 回归（与改造前相同的通过率）
- 如有新失败，必须修复

### 任务 2：Vitest 前端单元测试验证（30min）

**目标**：运行 Vitest 单元测试，验证前端模块功能正常。

**执行步骤**：
1. `cd e2e-tests && npm run test:unit` — 运行 Vitest 测试
2. 重点关注：
   - `guards.test.js` — 输入守卫逻辑
   - `action.test.js` — Action 注册表
3. 修复任何与 toggle 状态管理相关的失败

**验收标准**：
- Vitest 407+ passed, 0 failed

### 任务 3：手动测试场景验证（1h）

**目标**：通过手动测试验证 Focus Trap 栈 + Toggle 同步 + 会话切换关闭面板在真实交互中正确工作。

**测试场景清单**：

| # | 场景 | 预期行为 | 涉及模块 |
|---|---|---|---|
| 1 | 设置面板 → 批量删除 → 确认对话框 → 确认/取消 | 确认后焦点恢复到设置面板 | focus-trap.js |
| 2 | 代码执行面板 → IPC 返回 PRO_REQUIRED → 付费墙 → 关闭 | 焦点恢复到代码执行面板 | focus-trap.js |
| 3 | 图谱面板 → 点击符号搜索按钮 → 符号搜索面板 | 图谱 Trap 暂停，符号搜索 Trap 激活 | focus-trap.js |
| 4 | 输入框点击「混合搜索」toggle → 打开设置面板 | 设置面板中混合搜索开关与输入框 toggle 同步 | state.js + input-toggles.js |
| 5 | 打开设置面板 → 关闭 → 输入框 toggle 状态 | toggle 状态保持一致（不回退） | state.js |
| 6 | 打开图谱面板 → 点击新对话按钮 | 图谱面板关闭，新会话创建 | main.js closeAllPanels |
| 7 | 打开设置面板 → 点击历史会话 | 设置面板关闭，历史会话加载 | main.js closeAllPanels |
| 8 | 输入框输入 `/` → 斜杠命令面板 → ↑↓ 导航 → Enter 选择 | 命令正确执行 | input-keymap.js |
| 9 | 输入框空输入 → ↑ 键 → 历史导航 | 上一条历史输入恢复 | input-keymap.js + input-history.js |
| 10 | 输入框输入文字 → 切换会话 → 切回 | 草稿恢复 | input-history.js |

**验收标准**：
- 所有 10 个手动测试场景通过
- 无焦点丢失、面板残留、状态不同步等问题

### 任务 4：Rust 后端回归测试（30min）

**目标**：运行 Rust 测试确保后端无回归（本次改造仅涉及前端，但需确认）。

**执行步骤**：
```bash
cargo test                    # Free
cargo test --features pro     # Pro
cargo clippy --all-targets -- -D warnings          # Free Clippy
cargo clippy --all-targets --features pro -- -D warnings  # Pro Clippy
cargo fmt --check
```

**验收标准**：
- Free 822+ passed, 0 failed
- Pro 1094 passed, 0 failed
- Clippy 零警告

### 任务 5：文档更新与 GitHub 同步（30min）

**目标**：更新项目文档反映 Session 1-3 的前端架构改造，同步到 GitHub 发布仓库。

**更新清单**：
1. `docs/api/frontend-modules.md` — 新增 `input-keymap.js` / `input-history.js` / `zindex.js` / `panel-stack.js` 模块文档
2. `docs/sessions/CURRENT_STATE.md` — 更新前端模块数（50→54）+ 测试基线
3. `docs/sessions/LOG.md` — 追加 Session 1-3 记录
4. `docs/SOFTWARE_ASSET.md` — 更新前端架构章节（新增 4 个模块）
5. `AGENTS.md` — 更新 key files 列表
6. `CLAUDE.md` — 更 key files 列表
7. `./sync-to-github.sh --commit` — 同步到发布仓库

**验收标准**：
- 文档与代码一致
- `./sync-to-github.sh` 执行成功

### 任务 6：下阶段方向选择（规划，不实施）

**目标**：基于当前项目状态，选择下一个开发方向。

**可选方向**（从 `docs/sessions/NEXT_SESSION_PROMPT.md` 摘录）：

| 方向 | 描述 | 预计工时 |
|---|---|---|
| v1.5 新功能 | MCP 协议 / Late Chunking / RAGAS 评估 / 多窗口 | 5-10 天 |
| Git 状态清理 | 提交 101 个未提交文件 | 0.5 天 |
| E2E 质量提升 | 修复 pre-existing 失败 + 补充测试覆盖 | 2-3 天 |
| 架构优化 | 编译速度 / Pro 审计 / 依赖更新 | 2-3 天 |
| 生产就绪 | 日志增强 / 错误恢复 / 数据迁移 | 3-5 天 |
| 商业化准备 | License 购买流程 / 用户文档 / 官网 | 5-10 天 |

---

## 实施顺序建议

1. **任务 1（E2E 测试）** — 最重要，验证无回归
2. **任务 2（Vitest）** — 快速验证
3. **任务 4（Rust 回归）** — 确认后端无影响
4. **任务 3（手动测试）** — 验证真实交互
5. **任务 5（文档+同步）** — 记录成果
6. **任务 6（下阶段规划）** — 规划后续

## 验证标准（全部通过后 Session 3 完成）

- `npx tsc --noEmit`: 0 errors
- `node scripts/build-ui.mjs`: 成功
- `cd e2e-tests && npm run test:unit`: 407+ passed
- `cd e2e-tests && npm run test:bridge`: 0 回归
- `cargo test`: Free 822+ passed
- `cargo test --features pro`: Pro 1094 passed
- `cargo clippy --all-targets -- -D warnings`: 零警告
- `cargo fmt --check`: 通过
- 10 个手动测试场景全部通过
- `./sync-to-github.sh --commit`: 成功
- `docs/sessions/CURRENT_STATE.md` + `LOG.md` + `SOFTWARE_ASSET.md` 已更新
