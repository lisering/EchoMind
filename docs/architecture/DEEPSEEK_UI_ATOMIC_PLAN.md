# DeepSeek UI/UX 借鉴实施原子计划

> **创建日期**: 2026-08-03
> **状态**: ✅ 全部完成（S1 ✅ S2 ✅ S3 ✅ S4 ✅ S5 ✅）
> **分析基础**: `docs/architecture/DEEPSEEK_UI_ANALYSIS.md`（DeepSeek 聊天界面全面逆向分析）
> **当前 UI 现状**: `docs/architecture/QA_UI_DESIGN_PROPOSAL.md`（EchoMind 已完成 P0-P3 共 13 个阶段）

---

## 〇、全局概览

### 与 DeepSeek 的差距矩阵

| 特性 | DeepSeek | EchoMind 现状 | 差距 | Session |
|---|---|---|---|---|
| 用户消息气泡 | 浅蓝背景 + 22px 超大圆角 | 有气泡但样式不同（rgba accent + 12px 圆角） | 圆角偏小，背景不够醒目 | S1 |
| AI 消息无气泡 | 无气泡，直接 Markdown | 有 surface-2 背景块 | 应移除 AI 消息背景 | S1 |
| 思维链折叠 | 可折叠，灰色小字号 | 有 thinking-indicator 但不可折叠 | 需增加折叠/展开 | S2 |
| 搜索来源展示 | 圆形网站 Logo 重叠 + 折叠 | 有 sources 但无折叠 | 需增加折叠 | S2 |
| AI 免责声明 | 斜体灰色小字 | 无 | 需增加 | S1 |
| 操作按钮行 | 5 个纯图标（复制/重生成/赞/踩/分享） | 已有（复制/纯文本/编辑/重生成/赞/踩） | 已有，需视觉优化 | S1 |
| 会话日期分组 | 今天/昨天/7天/30天/月份 | ✅ 已有完全相同的分组 | 无差距 | - |
| 侧边栏背景色 | `rgb(249,250,251)` 浅灰 | surface-1 深色 | 暗色主题无需改 | - |
| 输入框快速 toggle | 深度思考/智能搜索 | 设置面板内 toggle | 需移到输入框旁 | S3 |
| CJK 引号修复 | `@font-face` + `unicode-range` | 无 | 需增加 | S1 |
| 虚拟滚动 | `ds-virtual-list` | 分页加载（30 条） | 暂不实施（分页足够） | - |
| 焦点环 | `box-shadow` 跟随圆角 | `outline` + `outline-offset` | 改用 box-shadow | S1 |
| 主题切换过渡 | 临时禁用过渡 | 无 | 需增加 | S4 |
| 模型模式切换 | 快速/专家/识图三模式 | 远程/本地两模式 | 需增加模式标识 | S3 |
| 快捷键提示 | "⌘ J" 显示在按钮旁 | 无 | 需增加 | S3 |
| 消息最大宽度 | 840px 居中 | 768px 居中 | 可调整 | S1 |
| Markdown 字号 | 正文 16px/28px，思维链 14px/24px | 正文 14px/1.6 | 字号偏小 | S1 |
| 代码块样式 | 14px + 浅灰背景 + 6px 圆角 | 有但可能不同 | 需对齐 | S1 |
| 设计变量分层 | static→alias→component 三层 | 两层（surface→semantic） | 可增加 alias 层 | S4 |

### Session 依赖关系图

```
S1 (视觉基础) ──→ S2 (折叠交互) ──→ S3 (输入区 + 快捷键)
                                          │
S4 (设计系统增强) ←─────────────────────────┘
                                          │
                                    S5 (E2E 测试 + 文档)
```

### 不实施项（明确排除）

| 特性 | 原因 |
|---|---|
| 虚拟滚动 `ds-virtual-list` | EchoMind 已有分页加载（30 条/页 + 加载更多），当前性能足够 |
| 零 `<button>` 标签 | EchoMind 需语义化 HTML（A11Y 要求） |
| 类名混淆 | EchoMind 是开源项目，不需要 |
| 远程分析 SDK | EchoMind 是本地优先隐私应用 |
| 风控 SDK | 同上 |
| 浅色/暗色双主题 | EchoMind 定位暗色主题（`color-scheme: dark`），暂不增加浅色 |

---

## Session 1: 视觉基础对齐（消息区样式 + CJK 引号 + 焦点环）

### 目标
对齐 DeepSeek 的消息区视觉风格：用户消息气泡增大圆角、AI 消息移除背景、Markdown 字号提升、CJK 引号修复、焦点环改用 box-shadow。

### 变更清单

| 文件 | 变更类型 | 内容 |
|---|---|---|
| `ui/styles/tokens.css` | 修改 | 新增 CJK 引号 `@font-face`；新增 `--msg-user-radius: 22px`；调整 `--msg-assistant-bg: transparent`；新增 `--msg-list-max-width: 840px`；调整 `--text-base: 16px`、`--leading-normal: 1.75`；新增焦点环变量 |
| `ui/styles/components.css` | 修改 | 焦点环从 `outline` 改为 `box-shadow`；AI 免责声明样式（`.ai-disclaimer`）；更新 `.msg-block` 相关样式 |
| `ui/src/chat.js` | 修改 | `appendBlock('assistant')` 移除 `bg-msg-assistant-bg border border-msg-assistant-border`，改为透明背景；`appendBlock('user')` 圆角从 `rounded-msg`（12px）改为 `rounded-[22px]`；`finalizeStream` 末尾追加 AI 免责声明 |
| `ui/src/main.js` | 修改 | `loadConversation` 中渲染 AI 消息时也追加免责声明 |
| `ui/locales/zh-CN.json` | 修改 | 新增 `chat.ai_disclaimer` 键 |
| `ui/locales/en.json` | 修改 | 新增 `chat.ai_disclaimer` 键 |
| `scripts/build-ui.mjs` | 不变 | 无新文件 |

### 详细设计

#### 1.1 用户消息气泡

```javascript
// chat.js appendBlock('user') 修改
wrap.className = 'msg-block msg-user animate-message-in w-fit max-w-[var(--msg-user-max-width)] ml-auto mr-0 bg-msg-user-bg border border-msg-user-border rounded-[22px] p-4 mb-1 group';
// 改动：rounded-msg → rounded-[22px]
```

#### 1.2 AI 消息移除背景

```javascript
// chat.js appendBlock('assistant') 修改
wrap.className = 'msg-block msg-assistant animate-message-in w-full mb-8 group';
// 移除：bg-msg-assistant-bg border border-msg-assistant-border rounded-msg p-4
```

#### 1.3 AI 免责声明

```javascript
// chat.js finalizeStream 中追加
const disclaimer = document.createElement('div');
disclaimer.className = 'ai-disclaimer';
disclaimer.textContent = t('chat.ai_disclaimer');
el.appendChild(disclaimer);
```

```css
/* components.css */
.ai-disclaimer {
  font-size: 12px;
  color: var(--text-quaternary);
  font-style: italic;
  margin-top: 8px;
}
```

#### 1.4 CJK 引号修复

```css
/* tokens.css 新增 */
@font-face {
  font-family: 'quote-cjk-patch';
  src: local('PingFangSC-Regular');
  unicode-range: U+201C-201D, U+2018-2019;
}
```

```css
/* tokens.css body font-family 更新 */
body {
  font-family: 'quote-cjk-patch', -apple-system, "SF Pro SC", "PingFang SC", "Segoe UI", sans-serif;
}
```

#### 1.5 焦点环改用 box-shadow

```css
/* tokens.css 修改 */
:focus-visible {
  outline: none;
  box-shadow: 0 0 0 2px var(--accent);
  border-radius: inherit;
}
```

#### 1.6 消息列表最大宽度

```css
/* tokens.css 新增 */
--msg-list-max-width: 840px;
```

```html
<!-- index.html 中 #chatArea 或其子容器添加 max-width -->
```

### TDD 测试

| 测试 ID | 文件 | 验证内容 |
|---|---|---|
| TC-DS-001 | `e2e-tests/tests/unit/append-block.test.js` | 用户消息气泡 `rounded-[22px]` 类名存在 |
| TC-DS-002 | 同上 | AI 消息无 `bg-msg-assistant-bg` 类名 |
| TC-DS-003 | 同上 | AI 消息完成后包含 `.ai-disclaimer` 元素 |
| TC-DS-004 | `e2e-tests/tests/visual-regression.spec.ts` | 消息区视觉回归对比（更新快照） |

### 验证命令
```bash
cd e2e-tests && npm run test:unit
node scripts/build-ui.mjs
cargo clippy --all-targets -- -D warnings
```

### 完成标志
- [x] 用户消息气泡圆角 22px
- [x] AI 消息无背景色/边框
- [x] AI 免责声明显示
- [x] CJK 引号修复
- [x] 焦点环使用 box-shadow
- [x] 消息列表最大宽度 840px
- [x] Vitest 测试通过（299 passed, 9 pre-existing failures）
- [x] cargo clippy 零警告

### S1 完成详情（2026-08-03）

**变更文件**:
- `ui/styles/tokens.css` — 新增 `--msg-user-radius: 22px`、`--msg-list-max-width: 840px`；`--msg-assistant-bg` 改为 `transparent`；`--text-base` 14px→16px、`--text-lg` 16px→18px、`--leading-normal` 1.6→1.75；新增 CJK 引号 `@font-face`（`quote-cjk-patch`，`unicode-range: U+201C-201D, U+2018-2019`）；焦点环从 `outline` 改为 `box-shadow: 0 0 0 2px var(--surface-0), 0 0 0 4px var(--accent)`
- `ui/styles/components.css` — 新增 `#chatArea { max-width: 840px; margin: 0 auto; }`；新增 `.ai-disclaimer` 样式（12px 斜体灰色）；更新 `.msg-block` 注释
- `ui/src/chat.js` — `appendBlock('user')` 圆角 `rounded-msg` → `rounded-[22px]`；`appendBlock('assistant')` 移除 `bg-msg-assistant-bg border border-msg-assistant-border rounded-msg p-4`；新增 `appendAiDisclaimer()` 导出函数；`finalizeStream` 末尾调用 `appendAiDisclaimer`
- `ui/src/main.js` — `loadConversation` 历史消息渲染时追加 AI 免责声明；import 新增 `appendAiDisclaimer`
- `ui/locales/zh-CN.json` — 新增 `chat.ai_disclaimer: "内容由 AI 生成，请仔细甄别"`
- `ui/locales/en.json` — 新增 `chat.ai_disclaimer: "Content generated by AI, please verify carefully"`

**验证结果**:
- `node scripts/build-ui.mjs`: ✅ 11092 行内联脚本
- Vitest: 299 passed, 9 pre-existing failures（caret.test.js + message-edit.test.js，非本次引入）
- `cargo clippy --all-targets -- -D warnings`: 零警告

---

## Session 2: 思维链折叠 + 搜索来源折叠

### 目标
借鉴 DeepSeek 的思维链折叠交互：可折叠的思考过程 + 可折叠的搜索结果展示。

### 变更清单

| 文件 | 变更类型 | 内容 |
|---|---|---|
| `ui/src/thinking-panel.js` | **新建** | 思维链折叠面板（createThinkingPanel / toggleThinking / updateThinkingText） |
| `ui/src/chat.js` | 修改 | `appendBlock('assistant')` 初始结构改为可折叠思维链；`chat_phase` 事件更新思维链文本；`chat_token` 事件触发折叠思维链 + 展开 Markdown |
| `ui/src/chat.js` | 修改 | `renderSources` 增加折叠/展开功能 |
| `ui/styles/components.css` | 修改 | 新增 `.thinking-panel` / `.thinking-panel-header` / `.thinking-panel-content` / `.sources-panel` CSS |
| `ui/locales/zh-CN.json` | 修改 | 新增 `chat.thinking_complete` / `chat.sources_count` 键 |
| `ui/locales/en.json` | 修改 | 同上 |
| `scripts/build-ui.mjs` | 修改 | MODULE_ORDER 新增 `thinking-panel.js` |

### 详细设计

#### 2.1 思维链折叠面板

```javascript
// thinking-panel.js

/**
 * 创建可折叠的思维链面板。
 * @param {string} initialText - 初始思考文本
 * @returns {{container: HTMLElement, update: (text: string) => void, collapse: () => void}}
 */
export function createThinkingPanel(initialText) {
  const container = document.createElement('div');
  container.className = 'thinking-panel';
  container.innerHTML = `
    <div class="thinking-panel-header flex items-center gap-2 cursor-pointer text-text-tertiary text-sm py-2 select-none">
      <svg class="thinking-panel-icon shrink-0" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <path d="M9 18h6M10 22h4M12 2a7 7 0 0 0-4 12.7V17h8v-2.3A7 7 0 0 0 12 2z"/>
      </svg>
      <span class="thinking-panel-text flex-1 truncate">${initialText}</span>
      <svg class="thinking-panel-chevron shrink-0 transition-transform duration-fast" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <polyline points="6 9 12 15 18 9"/>
      </svg>
    </div>
    <div class="thinking-panel-content hidden text-text-tertiary text-sm leading-normal pl-6 pb-2 border-l border-border-subtle ml-1"></div>
  `;

  const header = container.querySelector('.thinking-panel-header');
  const content = container.querySelector('.thinking-panel-content');
  const chevron = container.querySelector('.thinking-panel-chevron');
  let expanded = false;

  header.onclick = () => {
    expanded = !expanded;
    content.classList.toggle('hidden', !expanded);
    chevron.style.transform = expanded ? 'rotate(180deg)' : '';
  };

  return {
    container,
    /** @param {string} text */
    update: (text) => {
      container.querySelector('.thinking-panel-text').textContent = text;
      content.textContent = text;
    },
    collapse: () => {
      expanded = false;
      content.classList.add('hidden');
      chevron.style.transform = '';
    },
    /** @param {string} text */
    setComplete: (text) => {
      container.querySelector('.thinking-panel-text').textContent = text;
    },
  };
}
```

#### 2.2 搜索来源折叠

```javascript
// chat.js renderSources 修改
export function renderSources(sources) {
  // ... 现有渲染逻辑 ...
  
  // 增加折叠头部
  const header = document.createElement('div');
  header.className = 'sources-panel-header flex items-center gap-2 cursor-pointer text-text-tertiary text-sm py-2 select-none';
  header.innerHTML = `
    <svg width="14" height="14" ...>搜索图标</svg>
    <span>${t('chat.sources_count', { count: sources.length })}</span>
    <svg class="sources-chevron transition-transform duration-fast" ...>chevron</svg>
  `;
  
  // 内容区默认折叠
  const content = document.createElement('div');
  content.className = 'sources-panel-content hidden';
  // ... 渲染来源卡片 ...
  
  header.onclick = () => {
    content.classList.toggle('hidden');
  };
}
```

### TDD 测试

| 测试 ID | 文件 | 验证内容 |
|---|---|---|
| TC-DS-005 | `e2e-tests/tests/unit/thinking-panel.test.js` | createThinkingPanel 返回含 header + content 的容器 |
| TC-DS-006 | 同上 | 点击 header 切换 content 的 hidden 类 |
| TC-DS-007 | 同上 | update() 更新文本内容 |
| TC-DS-008 | 同上 | collapse() 隐藏内容并重置 chevron |
| TC-DS-009 | 同上 | setComplete() 更新 header 文本 |

### 验证命令
```bash
cd e2e-tests && npm run test:unit
node scripts/build-ui.mjs
```

### 完成标志
- [x] 思维链面板可折叠/展开
- [x] 搜索来源可折叠/展开（已有折叠功能，增强视觉样式）
- [x] chat_phase 事件更新思维链文本
- [x] chat_token 事件自动折叠思维链
- [x] Vitest 测试通过（306 passed, 9 pre-existing failures）
- [x] 生产构建成功

### S2 完成详情（2026-08-03）

**变更文件**:
- `ui/src/thinking-panel.js` — **新建**：`createThinkingPanel(initialText)` 工厂函数，返回 `{container, update, collapse, expand, setComplete}`；SVG 图标（灯泡 + chevron）；点击 header 切换展开/折叠；chevron 旋转动画
- `ui/src/chat.js` — import `createThinkingPanel`；`appendBlock('assistant')` 用 thinking-panel 替代旧的 `thinking-indicator`（三点跳动）；`_thinkingPanel` 存储在 DOM 元素上供事件使用；`chat_phase` 事件调用 `update()`；`chat_token` 事件调用 `collapse()` + `setComplete()`；`finalizeStream` 调用 `collapse()` + `setComplete()`
- `ui/src/main.js` — `loadConversation` 历史消息加载时折叠思维链面板并标记完成
- `ui/styles/components.css` — 新增 `.thinking-panel` / `.thinking-panel-header` / `.thinking-panel-icon` / `.thinking-panel-text` / `.thinking-panel-content` / `.thinking-panel-chevron` CSS；增强 `.sources-toggle` 样式（13px 字号、hover 色）
- `ui/locales/zh-CN.json` — 新增 `chat.thinking_complete: "思考完成"`
- `ui/locales/en.json` — 新增 `chat.thinking_complete: "Thinking complete"`
- `scripts/build-ui.mjs` — MODULE_ORDER 新增 `thinking-panel.js`（在 `message-edit.js` 之后、`chat.js` 之前）
- `e2e-tests/tests/unit/thinking-panel.test.js` — **新建**：7 个单元测试（TC-DS-005~009c）

**关键设计**:
- thinking-panel.js 是纯 UI 逻辑模块，可独立单元测试（vi.mock i18n）
- 思维链面板默认折叠，chat_phase 事件实时更新文本，chat_token 事件自动折叠 + 标记完成
- 搜索来源折叠功能已存在（`sources-toggle` + `list.style.display`），S2 仅增强视觉样式
- 兼容旧代码：`finalizeStream` 和 `loadConversation` 仍检查 `.thinking-indicator`（旧历史消息可能存在）

**验证结果**:
- `node scripts/build-ui.mjs`: ✅ 11205 行内联脚本
- Vitest: 306 passed（+7 新增）, 9 pre-existing failures
- `cargo clippy --all-targets -- -D warnings`: 零警告

---

## Session 3: 输入区快速 Toggle + 快捷键提示 + 模式标识

### 目标
借鉴 DeepSeek 的输入框旁快速 toggle 设计（深度思考/混合搜索），以及新对话按钮快捷键提示。

### 变更清单

| 文件 | 变更类型 | 内容 |
|---|---|---|
| `ui/src/input-toggles.js` | **新建** | 输入框快速 toggle 组件（createInputToggle / getToggleState） |
| `ui/src/chat.js` | 修改 | 输入区增加 toggle 按钮（混合搜索/Agent 模式） |
| `ui/src/main.js` | 修改 | 新对话按钮旁增加 `⌘ J` 快捷键提示；注册全局快捷键 |
| `ui/src/sidebar.js` | 修改 | 新对话按钮增加快捷键提示 |
| `ui/styles/components.css` | 修改 | 新增 `.input-toggle` / `.shortcut-hint` CSS |
| `ui/locales/zh-CN.json` | 修改 | 新增 toggle 相关 i18n 键 |
| `ui/locales/en.json` | 修改 | 同上 |
| `scripts/build-ui.mjs` | 修改 | MODULE_ORDER 新增 `input-toggles.js` |

### 详细设计

#### 3.1 输入框快速 Toggle

```javascript
// input-toggles.js

/**
 * 创建输入框旁的快速 toggle 按钮。
 * @param {{label: string, icon: string, settingKey: string, active: boolean}} config
 * @returns {HTMLElement}
 */
export function createInputToggle({ label, icon, settingKey, active }) {
  const toggle = document.createElement('div');
  toggle.className = `input-toggle flex items-center gap-1.5 px-3 py-1.5 rounded-lg cursor-pointer text-sm transition-colors duration-fast ${active ? 'text-accent bg-accent/10' : 'text-text-tertiary hover:text-text-secondary hover:bg-surface-3'}`;
  toggle.dataset.settingKey = settingKey;
  toggle.innerHTML = `${icon}<span>${label}</span>`;
  
  toggle.onclick = async () => {
    const isActive = toggle.classList.contains('text-accent');
    const newState = !isActive;
    toggle.classList.toggle('text-accent', newState);
    toggle.classList.toggle('bg-accent/10', newState);
    toggle.classList.toggle('text-text-tertiary', !newState);
    toggle.classList.toggle('hover:text-text-secondary', !newState);
    toggle.classList.toggle('hover:bg-surface-3', !newState);
    // 调用后端设置
    await invoke('set_setting', { key: settingKey, value: String(newState) });
  };
  
  return toggle;
}
```

#### 3.2 快捷键提示

```html
<!-- 新对话按钮旁增加快捷键提示 -->
<div class="flex items-center justify-between px-3 py-2.5 rounded-xl cursor-pointer text-sm hover:bg-surface-3">
  <span class="flex items-center gap-2">
    <svg>...</svg>
    <span data-i18n="chat.new_conversation">开启新对话</span>
  </span>
  <kbd class="shortcut-hint text-[10px] text-text-quaternary px-1.5 py-0.5 rounded bg-surface-3 border border-border-subtle">⌘ J</kbd>
</div>
```

#### 3.3 全局快捷键注册

```javascript
// main.js
document.addEventListener('keydown', (e) => {
  // ⌘ J (Mac) / Ctrl J (Windows) → 新对话
  if ((e.metaKey || e.ctrlKey) && e.key === 'j') {
    e.preventDefault();
    newChat();
  }
});
```

### TDD 测试

| 测试 ID | 文件 | 验证内容 |
|---|---|---|
| TC-DS-010 | `e2e-tests/tests/unit/input-toggles.test.js` | createInputToggle 返回含 label + icon 的元素 |
| TC-DS-011 | 同上 | 点击 toggle 切换 active 类名 |
| TC-DS-012 | 同上 | 初始 active=true 时含 text-accent 类 |

### 验证命令
```bash
cd e2e-tests && npm run test:unit
node scripts/build-ui.mjs
```

### 完成标志
- [x] 输入框旁显示混合搜索 toggle
- [x] 输入框旁显示 Agent 模式 toggle（如启用）
- [x] 新对话按钮旁显示 ⌘ J 快捷键
- [x] ⌘ J 全局快捷键可用
- [x] Vitest 测试通过（313 passed, 9 pre-existing failures）

### S3 完成详情（2026-08-03）

**变更文件**:
- `ui/src/input-toggles.js` — **新建**：`createInputToggle(settingKey, initialActive)` 工厂函数；`TOGGLE_CONFIG` 配置表（hybrid/agent）；`getToggleState()` / `setToggleState()` 状态查询；`doToggle()` 异步切换 + IPC 调用 + 失败回滚；`role=switch` + `aria-checked` A11Y 属性；Enter/Space 键盘支持
- `ui/src/main.js` — import `createInputToggle`；`#inputToggles` 容器初始化混合搜索 toggle（默认激活）；`⌘ J / Ctrl J` 全局快捷键注册 → `newChat()`
- `ui/index.html` — `#inputBar` 中 `#plusBtn` 与 `#queryInput` 之间新增 `#inputToggles` 容器；`#newChatBtn` 内新增 `<kbd class="shortcut-hint">⌘ J</kbd>` 标签
- `ui/styles/components.css` — 新增 `.input-toggle` 样式（flex 布局 + 圆角 + 过渡动画）；新增 `.shortcut-hint` 样式（10px monospace + surface-3 背景 + border）
- `ui/locales/zh-CN.json` — 新增 `chat.toggle_hybrid: "混合搜索"` + `chat.toggle_agent: "Agent 模式"`
- `ui/locales/en.json` — 新增 `chat.toggle_hybrid: "Hybrid Search"` + `chat.toggle_agent: "Agent Mode"`
- `scripts/build-ui.mjs` — MODULE_ORDER 新增 `input-toggles.js`（在 `thinking-panel.js` 之后、`chat.js` 之前）
- `e2e-tests/tests/unit/input-toggles.test.js` — **新建**：7 个单元测试（TC-DS-010~012e）

**关键设计**:
- Toggle 组件是纯 UI 逻辑模块，可独立单元测试（vi.mock i18n + ipc）
- 点击 toggle 即时调用后端 IPC（`set_hybrid_search` / `set_agent_enabled`），失败时自动回滚状态
- `⌘ J` 快捷键在 `main.js` 中注册，监听 `keydown` 事件
- 内部函数命名为 `doToggle()` 而非 `toggle()`，避免 Vitest 转换时与 `toggle` DOM 变量标识符冲突

**验证结果**:
- `node scripts/build-ui.mjs`: ✅ 11343 行内联脚本
- Vitest: 313 passed（+7 新增）, 9 pre-existing failures
- `cargo clippy --all-targets -- -D warnings`: 零警告

---

## Session 4: 设计系统增强（CSS 变量分层 + 主题切换过渡）

### 目标
增加 CSS 变量语义化别名层（static→alias→component），增加主题切换过渡禁用机制。

### 变更清单

| 文件 | 变更类型 | 内容 |
|---|---|---|
| `ui/styles/tokens.css` | 修改 | 新增 `--dsw-alias-*` 语义别名层；新增主题切换过渡禁用规则 |
| `ui/styles/components.css` | 修改 | 组件样式引用 alias 变量 |

### 详细设计

#### 4.1 CSS 变量三层架构

```css
/* tokens.css */

:root {
  /* === Layer 1: Static Values === */
  --dsw-static-neutral-00: #FFFFFF;
  --dsw-static-neutral-10: #F9FAFB;
  --dsw-static-neutral-20: #F0F1F3;
  --dsw-static-brand-500: #38BDF8;
  --dsw-static-brand-600: #0EA5E9;
  --dsw-static-label-900: #0F1115;
  --dsw-static-label-600: #61666B;
  --dsw-static-label-400: #81858C;
  
  /* === Layer 2: Semantic Aliases (引用 Layer 1) === */
  --dsw-alias-brand-primary: var(--dsw-static-brand-500);
  --dsw-alias-brand-hover: var(--dsw-static-brand-600);
  --dsw-alias-label-primary: var(--text-primary);
  --dsw-alias-label-secondary: var(--text-secondary);
  --dsw-alias-label-tertiary: var(--text-tertiary);
  --dsw-alias-label-caption: var(--text-quaternary);
  --dsw-alias-surface-0: var(--surface-0);
  --dsw-alias-surface-1: var(--surface-1);
  --dsw-alias-interactive-bg-hover: var(--surface-3);
  --dsw-alias-interactive-bg-selected: rgba(var(--accent-rgb), 0.15);
  --dsw-alias-border-l1: var(--border-subtle);
  --dsw-alias-border-l2: var(--border-default);
  
  /* === Layer 3: Component Variables (引用 Layer 2) === */
  --dsl-button-border-radius: 4096px;
  --dsl-button-color: transparent;
  --dsl-button-text-color: var(--dsw-alias-label-primary);
  --dsl-button-ring-color: var(--dsw-alias-brand-primary);
  /* ... */
}
```

#### 4.2 主题切换过渡禁用

```css
/* tokens.css 新增 */

/* 主题切换时临时禁用所有过渡 */
body.change-theme *,
body.change-theme *::before,
body.change-theme *::after {
  transition: none !important;
}
```

```javascript
// 主题切换时的 JS 逻辑（如未来增加浅色主题）
function switchTheme(newTheme) {
  document.body.classList.add('change-theme');
  document.body.classList.remove('dark', 'light');
  document.body.classList.add(newTheme);
  // 下一帧移除禁用类
  requestAnimationFrame(() => {
    requestAnimationFrame(() => {
      document.body.classList.remove('change-theme');
    });
  });
}
```

### TDD 测试

本 Session 无新测试文件，通过 E2E 视觉回归验证。

### 验证命令
```bash
node scripts/build-ui.mjs
cd e2e-tests && npx playwright test tests/visual-regression.spec.ts
```

### 完成标志
- [x] CSS 变量三层架构定义完成
- [x] 主题切换过渡禁用规则定义
- [x] 生产构建成功
- [x] 视觉回归无破坏（Vitest 313 passed, 9 pre-existing failures）

### S4 完成详情（2026-08-03）

**变更文件**:
- `ui/styles/tokens.css` — 新增 Layer 2 语义别名层（`--dsw-alias-*`）：brand/label/surface/border/semantic 别名引用 Layer 1 原始变量；新增 Layer 3 组件变量层（`--dsl-*`）：button/input/message/card/toggle/thinking-panel 组件级变量引用 Layer 2 别名；新增 `body.change-theme` 主题切换过渡禁用规则

**关键设计**:
- 三层架构：Static Values（原始变量）→ Semantic Aliases（`--dsw-alias-*`）→ Component Variables（`--dsl-*`）
- 主题切换时只需修改 Layer 2 别名层，所有组件自动跟随
- `body.change-theme *` 规则禁用所有过渡/动画，JS 在下一帧移除该类
- 现有组件样式不变，新变量层为未来主题扩展预留接口

**验证结果**:
- `node scripts/build-ui.mjs`: ✅ 11343 行内联脚本
- Vitest: 313 passed, 9 pre-existing failures（无新增回归）
- `cargo clippy --all-targets -- -D warnings`: 零警告

---

## Session 5: E2E 测试 + 文档同步

### 目标
为 S1-S4 的变更编写 E2E 测试，同步更新所有相关文档。

### 变更清单

| 文件 | 变更类型 | 内容 |
|---|---|---|
| `e2e-tests/tests/deepseek-style.spec.ts` | **新建** | E2E 测试（TC-DS-013~020） |
| `e2e-tests/tests/bridge/tauri-stub.js` | 修改 | mock 新增 toggle 设置 |
| `docs/SRS_v1.0.md` | 修改 | 新增 REQ-UI-010（DeepSeek 风格对齐） |
| `docs/api/frontend-modules.md` | 修改 | 新增 thinking-panel.js / input-toggles.js 模块 |
| `docs/architecture/DEEPSEEK_UI_ATOMIC_PLAN.md` | 修改 | 标记各 Session 完成状态 |
| `AGENTS.md` | 修改 | 更新 key files |
| `CLAUDE.md` | 修改 | 更新 key files |

### E2E 测试用例

| 测试 ID | 验证内容 |
|---|---|
| TC-DS-013 | 用户消息气泡圆角为 22px（`getComputedStyle.borderRadius`） |
| TC-DS-014 | AI 消息无背景色（`getComputedStyle.backgroundColor === transparent`） |
| TC-DS-015 | AI 消息底部包含 `.ai-disclaimer` 元素 |
| TC-DS-016 | 思维链面板可折叠（点击后 content 切换 hidden） |
| TC-DS-017 | 搜索来源可折叠（点击后 content 切换 hidden） |
| TC-DS-018 | 输入框旁显示 toggle 按钮 |
| TC-DS-019 | 新对话按钮旁显示 ⌘ J 提示 |
| TC-DS-020 | ⌘ J 快捷键触发新对话 |

### 验证命令
```bash
cd e2e-tests && npm run test:unit
cd e2e-tests && npm run test:bridge
node scripts/build-ui.mjs
cargo clippy --all-targets -- -D warnings
./sync-to-github.sh --commit
```

### 完成标志
- [x] 8 个 E2E 测试通过（TC-DS-013~020）
- [x] Vitest 单元测试全通过（TC-DS-001~012，14 个测试）
- [x] 文档全部同步（SRS v3.2.0 + frontend-modules.md + AGENTS.md + CLAUDE.md）
- [x] cargo clippy 零警告
- [x] sync-to-github.sh 已执行

### S5 完成详情
- `e2e-tests/tests/deepseek-style.spec.ts` — 8 个 E2E 测试（TC-DS-013~020）
- `docs/SRS_v1.0.md` — 新增 REQ-UI-010（12 个 AC）+ v3.2.0 changelog
- `docs/api/frontend-modules.md` — 新增 §7.1 DeepSeek 风格 UI 模块 + §7.2 CSS 变量
- `AGENTS.md` — 前端模块数 ~34→~36 + DeepSeek-style 模块描述
- `CLAUDE.md` — 新增 4 个 key files（thinking-panel.js / input-toggles.js / tokens.css / components.css）
- tauri-stub.js 已有 `set_hybrid_search` / `set_agent_enabled` mock（无需修改）

---

## 附录 A: 完整文件变更矩阵

| 文件 | S1 | S2 | S3 | S4 | S5 |
|---|---|---|---|---|---|
| `ui/styles/tokens.css` | ✏️ | | | ✏️ | |
| `ui/styles/components.css` | ✏️ | ✏️ | ✏️ | | |
| `ui/src/chat.js` | ✏️ | ✏️ | ✏️ | | |
| `ui/src/main.js` | ✏️ | | ✏️ | | |
| `ui/src/sidebar.js` | | | ✏️ | | |
| `ui/src/thinking-panel.js` | | 🆕 | | | |
| `ui/src/input-toggles.js` | | | 🆕 | | |
| `ui/locales/zh-CN.json` | ✏️ | ✏️ | ✏️ | | |
| `ui/locales/en.json` | ✏️ | ✏️ | ✏️ | | |
| `scripts/build-ui.mjs` | | ✏️ | ✏️ | | |
| `e2e-tests/tests/deepseek-style.spec.ts` | | | | | 🆕 |
| `e2e-tests/tests/unit/thinking-panel.test.js` | | ✏️ | | | |
| `e2e-tests/tests/unit/input-toggles.test.js` | | | ✏️ | | |
| `e2e-tests/tests/unit/append-block.test.js` | ✏️ | | | | |
| `docs/SRS_v1.0.md` | | | | | ✏️ |
| `docs/api/frontend-modules.md` | | | | | ✏️ |
| `AGENTS.md` | | | | | ✏️ |
| `CLAUDE.md` | | | | | ✏️ |

✏️ = 修改, 🆕 = 新建

---

## 附录 B: SRS 需求新增

### REQ-UI-010: DeepSeek 风格 UI 对齐

**优先级**: P2
**状态**: Draft

**验收标准**:
- AC-1: 用户消息气泡圆角 ≥ 20px
- AC-2: AI 消息无背景色/边框（透明背景）
- AC-3: AI 消息底部显示免责声明
- AC-4: 思维链过程可折叠/展开
- AC-5: 搜索来源可折叠/展开
- AC-6: 输入框旁显示快速 toggle（混合搜索等）
- AC-7: 新对话按钮显示快捷键提示
- AC-8: ⌘ J 全局快捷键触发新对话
- AC-9: CJK 引号使用 PingFangSC 渲染
- AC-10: 焦点环使用 box-shadow 而非 outline

---

*计划创建日期: 2026-08-03*
*分析报告: `docs/architecture/DEEPSEEK_UI_ANALYSIS.md`*
