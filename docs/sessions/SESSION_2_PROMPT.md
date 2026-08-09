# Session 2 衔接提示词 — Focus Trap 栈 + State 补全 + 会话切换关闭面板

## 上下文

Session 1 已完成 P0 级修复（Z-index 分层 + ESC 面板栈管理器）。以下文件已创建/修改：

### 新建文件
- `ui/src/zindex.js` — 统一 Z-index 常量（`Z_INDEX.LOCK_OVERLAY` ~ `Z_INDEX.BASE`）+ `zClass()` 辅助函数
- `ui/src/panel-stack.js` — 全局面板栈（`pushPanel/removePanel/closeTopPanel/peekTopPanel/isPanelOpen/hasOpenPanels/clearStack`）

### 已改造的模块（16 个）
| 模块 | z-index 改造 | panel-stack 注册 | ESC 处理 |
|---|---|---|---|
| `security.js` | ✅ LOCK_OVERLAY + AUDIT_LOG | ✅ lock-overlay | 预存（无变化） |
| `graph-viewer.js` | ✅ CSS z-99998→90 | ✅ graph-viewer | ✅ stopPropagation |
| `dream-panel.js` | ✅ z-50→PANEL_1 | ✅ dream-panel | ✅ stopPropagation |
| `code-executor.js` | ✅ z-50→PANEL_1 | ✅ code-executor | ✅ **新增 ESC 处理** |
| `symbol-search.js` | ✅ z-50→PANEL_1 | ✅ symbol-search | ✅ stopPropagation |
| `workflow-panel.js` | ✅ z-50→PANEL_1 | ✅ workflow-panel | （依赖 panel-stack） |
| `confirm-dialog.js` | ✅ z-[55]→PANEL_2 | ✅ confirm-dialog | 预存 capture keydown |
| `turn-tree.js` | ✅ CSS z-200→50 | ✅ turn-tree | ✅ **新增 ESC 处理** |
| `paywall.js` | （HTML 静态 z-50） | ✅ paywall | 预存（keyboard.js） |
| `settings.js` | （HTML 静态 z-50） | ✅ settings | 预存（keyboard.js） |
| `command-palette.js` | （HTML 静态 z-[80]） | ✅ command-palette | 预存（keyboard.js） |
| `sidebar.js` | （HTML 静态 z-[75]） | ✅ conv-search-popup | 预存（keyboard.js） |
| `download-manager.js` | （HTML 静态 z-[65]） | ✅ download-manager | （依赖 panel-stack） |
| `keyboard.js` | — | — | ✅ **重构 ESC 处理器**：先 `closeTopPanel()`，回退到静态面板 |
| `styles/graph-viewer.css` | ✅ z-99998→90 | — | — |
| `styles/components.css` | ✅ z-200→50 | — | — |

### 验证结果
- `tsc --noEmit`: **0 errors** ✅
- `esbuild --bundle`: **成功** ✅
- Linter: **0 errors** ✅
- E2E: Playwright 配置预存问题（与本次改动无关）

---

## Session 2 任务清单（P1 级）

### 方案 3：Focus Trap 栈管理（3h）

**目标**：全局 Trap 栈，只激活栈顶 Trap，解决多面板叠加时 Tab 键行为不确定问题。

**改造文件**：`ui/src/focus-trap.js`

**设计要点**：
1. 在 `focus-trap.js` 中维护 `_trapStack` 数组
2. `createFocusTrap(element)` 返回的 trap 对象增加栈感知：
   - `activate()`: 先暂停栈中所有已激活的 Trap → push 新 Trap → 激活
   - `deactivate()`: 从栈中移除当前 Trap → 恢复栈顶 Trap
   - `_pause()` / `_resume()`: 暂停/恢复 Trap 但不从栈中移除
3. 保持 API 向后兼容：现有调用方不需要改动
4. 测试场景：
   - 设置面板打开 → 批量删除触发 confirm-dialog → 确认后焦点恢复到设置面板
   - 代码执行面板打开 → IPC 返回 PRO_REQUIRED → 付费墙弹出 → 关闭后焦点恢复到代码执行面板

**注意事项**：
- 不要破坏现有 `activate/deactivate` 的接口签名
- Trap 暂停时不应释放 `previouslyFocused`（恢复时可能需要）
- 需要处理 "deactivate 时栈中已无此 Trap" 的边界情况（幂等）

### 方案 4：State.js 状态补全 + Toggle 同步（3h）

**目标**：将所有功能 toggle 状态纳入 `state.js` 统一管理，消除 `input-toggles.js` 的模块局部变量。

**改造文件**：
1. `ui/src/state.js` — 新增 initialState 字段
2. `ui/src/input-toggles.js` — 改为通过 `setState()` / `get()` 更新状态
3. `ui/src/settings.js` — toggle 变更时 `setState()` → `input-toggles.js` 通过 `subscribe()` 同步 UI
4. `ui/src/main.js` — 启动时从 `get_settings` IPC 返回值初始化所有 toggle 状态

**新增的 state 字段**：
```javascript
hybridEnabled: false,
agentEnabled: false,
subAgentEnabled: false,
memoryEnabled: false,
webSearchEnabled: false,
docList: [],  // chat.js doc-mention 需要（当前 get('docList') 返回 undefined）
```

**input-toggles.js 改造要点**：
- 删除 `_toggleStates` 模块局部变量
- `createInputToggle(settingKey, initialActive)` 中通过 `get(`${settingKey}Enabled`)` 读取状态
- `doToggle()` 中通过 `setState({ [`${settingKey}Enabled`]: newState })` 更新状态
- `getToggleState(settingKey)` 改为 `get(`${settingKey}Enabled`)`
- `setToggleState(settingKey, value)` 改为 `setState({ [`${settingKey}Enabled`]: value })`

**settings.js 改造要点**：
- `openSettings()` 中从 `settings` IPC 返回值 `setState()` 更新所有 toggle 状态
- toggle 变更回调中 `setState()` 更新对应状态

**main.js 改造要点**：
- 在 `boot()` 的 `get_settings` 返回后，`setState()` 初始化所有 toggle 状态
- Pro 状态变更订阅中刷新 toggle 可见性

### 方案 5：会话切换时关闭面板（2h）

**目标**：`newChat()` 和 `loadConversation()` 切换会话时关闭所有打开的 overlay 面板。

**改造文件**：`ui/src/main.js`

**设计**：
1. 在 `main.js` 中新建 `closeAllPanels()` 函数：
   ```javascript
   function closeAllPanels() {
     // 关闭所有动态面板（通过 panel-stack）
     // 注意：不关闭 lock-overlay（安全相关）
     closeSettings();
     closeGraphViewer();
     closeCodeExecutorPanel();
     closeDreamPanel();
     closeSymbolSearch();
     closeWorkflowPanel();
     closeSearchPopup();
     closeCommandPalette();
     hidePaywall();
     closeConversationTreePanel();
   }
   ```
2. 在 `newChat()` 开头（在 `if (get('streaming')) { await onStop(); }` 之后）调用 `closeAllPanels()`
3. 在 `loadConversation()` 开头（在 `if (get('streaming')) { await onStop(); }` 之后）调用 `closeAllPanels()`
4. 需要 import 各 close 函数（部分已在 main.js import）

**注意事项**：
- `closeAllPanels()` 应在 streaming 中断之后、草稿保存之前调用
- 不关闭 `lock-overlay`（安全相关，用户需手动解锁）
- 不关闭 `confirm-dialog`（它是同步的 Promise，不会被残留）
- 不关闭 `dbError` Modal（需要用户手动确认）

### 方案 6：输入区 keydown 统一处理器（3h）

**目标**：将 `main.js` 和 `chat.js` 中 `queryInput` 的两个 keydown 监听器合并为一个。

**改造文件**：
1. 新建 `ui/src/input-keymap.js` — 统一 keydown 处理器
2. `ui/src/main.js` — 删除 queryInput keydown 监听器，改为委托 input-keymap
3. `ui/src/chat.js` — 删除 queryInput keydown 监听器，改为委托 input-keymap

**设计**：
```javascript
// ui/src/input-keymap.js
export function createInputKeyHandler(handlers) {
  return (e) => {
    const { hasSlashPanel, hasDocMention } = checkPopups();
    
    // 1. 斜杠命令导航（最高优先级）
    if (hasSlashPanel) {
      handleSlashNavigation(e);
      if (e.defaultPrevented) return;
    }
    
    // 2. Enter 发送 / 选择命令
    if (e.key === 'Enter' && !e.shiftKey) {
      if (hasSlashPanel || hasDocMention) return; // chat.js 处理选择
      e.preventDefault();
      if (!e.target.value.trim()) return;
      handlers.send();
      return;
    }
    
    // 3. 历史导航（仅无 popup 时）
    if (!hasSlashPanel && !hasDocMention) {
      if (e.key === 'ArrowUp') { ... }
      if (e.key === 'ArrowDown') { ... }
    }
    
    // 4. Escape 重置历史导航
    if (e.key === 'Escape') {
      handlers.resetHistoryNav();
    }
    
    // 5. ⌘+Arrow 轮播导航
    if ((e.metaKey || e.ctrlKey) && (e.key === 'ArrowLeft' || e.key === 'ArrowRight')) {
      handlers.navigateCarousel(e.key === 'ArrowLeft' ? 'left' : 'right');
    }
  };
}
```

**注意事项**：
- `checkPopups()` 需要检查 `.slash-command-panel` 和 `.doc-mention-popup` 是否存在
- 历史导航需要保留 "仅当光标在第一行/最后一行时触发" 的逻辑
- 斜杠命令导航需要调用 `chat.js` 的 `filterSlashCommands` 和 `navigateSlashCommand`
- 需要将 `handlers.send`、`handlers.navigateHistoryUp/Down`、`handlers.resetHistoryNav` 等作为参数传入

---

## 实施顺序建议

1. **方案 3（Focus Trap 栈）** — 独立改造 `focus-trap.js`，不影响其他模块
2. **方案 4（State 补全）** — 增量改进，需先改 `state.js`，再改 `input-toggles.js`，最后改 `settings.js`
3. **方案 5（会话切换关闭面板）** — 依赖 panel-stack（Session 1 已完成），只需在 `main.js` 中添加 `closeAllPanels()`
4. **方案 6（keydown 统一）** — 最复杂，建议最后做，需要充分测试所有键映射

## 验证标准

- `npx tsc --noEmit`: 0 errors
- `npx esbuild ui/src/main.js --bundle --outfile=/tmp/test.js --format=esm`: 成功
- Linter: 0 errors
- 手动测试：
  - 设置面板 → 批量删除 → 确认对话框 → 确认/取消 → 焦点恢复正确
  - 代码执行面板 → 付费墙 → 关闭付费墙 → 焦点恢复到代码执行面板
  - 切换会话 → 所有面板关闭
  - 输入框 Up/Down 历史导航 + 斜杠命令导航不冲突
  - 输入框 toggle 与设置面板 toggle 状态同步
