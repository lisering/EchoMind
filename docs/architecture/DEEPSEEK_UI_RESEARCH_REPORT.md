# DeepSeek Chat UI 深度调研报告

> 调研日期：2026-08-14
> 调研方法：下载 DeepSeek Chat 前端全量 JS/CSS 资源（main.js 1.39MB + vendors.js 1.03MB + main.css 252KB），逆向分析组件结构、交互逻辑、设计系统
> 资源来源：`https://fe-static.deepseek.com/chat/static/`（commit-id: e444e47）

---

## 目录

1. [整体架构](#1-整体架构)
2. [设计系统](#2-设计系统)
3. [侧栏与会话管理](#3-侧栏与会话管理)
4. [对话区域与消息渲染](#4-对话区域与消息渲染)
5. [输入区域](#5-输入区域)
6. [思考链（DeepThink）](#6-思考链deepthink)
7. [联网搜索（DeepSearch）](#7-联网搜索deepsearch)
8. [工具调用（Tool Open/Search/Find）](#8-工具调用tool-opensearchfind)
9. [文件上传系统](#9-文件上传系统)
10. [消息编辑与重新生成](#10-消息编辑与重新生成)
11. [分享功能](#11-分享功能)
12. [会话历史搜索](#12-会话历史搜索)
13. [模型切换](#13-模型切换)
14. [主题与国际化](#14-主题与国际化)
15. [动画与过渡](#15-动画与过渡)
16. [Markdown 渲染系统](#16-markdown-渲染系统)
17. [虚拟列表与滚动管理](#17-虚拟列表与滚动管理)
18. [错误处理与恢复](#18-错误处理与恢复)
19. [EchoMind 借鉴建议](#19-echomind-借鉴建议)

---

## 1. 整体架构

### 技术栈

| 层面 | 技术 |
|---|---|
| 框架 | React (SPA，函数组件 + Hooks) |
| 构建工具 | Webpack（模块化打包 + 代码分割） |
| 样式方案 | CSS-in-JS 变量体系（`dsw-` 前缀自定义属性 + `ds-` 前缀组件类） |
| 状态管理 | Zustand（`en.y.storageHandles.*` + `tY.F.getState()`） |
| 路由 | SPA 路由（URL query params 传递 session_id / share_id） |
| 监控 | APMPlus (火山引擎 APM) + DataFinder (事件追踪) |
| 反爬 | 数美验证 (Shumei) + Cloudflare Turnstile + hCaptcha |

### 资源结构

```
fe-static.deepseek.com/chat/static/
├── default-vendors.{hash}.js    — 第三方依赖 (1.03MB)
├── main.{hash}.js               — 主应用逻辑 (1.39MB)
├── main.{hash}.css              — 全部样式 (252KB)
├── favicon.svg
├── manifest.json
├── icon-180.png
├── web-logo.svg
├── web-error-logo-light.png
└── web-error-logo-dark.png
```

### 关键架构模式

1. **Provider 嵌套模式**：`SearchViewProvider` → `MessageListScrollProvider` → `ModalProvider` → `ToastProvider` → `ToolOpenListViewProvider` — 每个功能域有独立的 Context Provider
2. **状态分层**：持久化状态（localStorage `__ds_remote_feature_store`）→ 会话状态（Zustand store）→ 组件局部状态（useState/useRef）
3. **错误边界**：三层 Error Boundary — 路由级 ErrorBoundary + 预览级 ErrorBoundary + Mermaid 渲染 ErrorBoundary
4. **SRI 完整性**：所有 `<script>` 标签带 `integrity="sha384-..."` + `onerror="_sriCk(this)"` — 资源加载失败时显示重试页面

---

## 2. 设计系统

### CSS 变量三层架构（与 EchoMind 高度相似）

DeepSeek 的设计系统使用 **三层 CSS 变量架构**：

```
Static (原子变量)  →  Alias (语义别名)  →  Component (组件变量)
dsw-static-*          dsw-alias-*            dsl-*-*
```

#### Static 层（原子设计令牌）

| 类别 | 变量前缀 | 示例 |
|---|---|---|
| 品牌色 | `dsw-static-deepseek-*` | 50/100/200/300/400/450/500/600/800/900 |
| 中性色（偏蓝） | `dsw-static-neutral-bluish-*` | 00/50/60/75/100/150/200/300/400/500/600/700/750/800/850/875/900/950/1000 |
| 中性色 | `dsw-static-neutral-*` | 00/50/100/150/200/250/300/400/500/550/600/700/800/850/900/1000 |
| 功能色 | `dsw-static-{red,blue,green,amber}-*` | 各自有 50-900 色阶 |
| 字体 | `dsw-font-*-font-{family,size,style,weight}` | base-16/s/xs/xsp/m/l/s/lsp |

#### Alias 层（语义别名）

| 类别 | 别名 | 用途 |
|---|---|---|
| 背景 | `dsw-alias-bg-base` | 页面基础背景 |
| | `dsw-alias-bg-layer-1/2/3` | 层级背景（侧栏/面板/弹窗） |
| | `dsw-alias-bg-overlay` | 遮罩背景 |
| | `dsw-alias-bg-mask-1/2/3` | 遮罩层级 |
| | `dsw-alias-bg-skeleton` | 骨架屏背景 |
| | `dsw-alias-bg-multi-select` | 多选背景 |
| 边框 | `dsw-alias-border-l1/2/3/4` | 边框层级 1-4 |
| | `dsw-alias-border-inverted/inverted2` | 反色边框 |
| | `dsw-alias-border-l2-darkmode-thin` | 暗色模式细边框 |
| 标签 | `dsw-alias-label-primary` | 主标签 |
| | `dsw-alias-label-secondary` | 次标签 |
| | `dsw-alias-label-tertiary` | 三级标签 |
| | `dsw-alias-label-caption` | 说明文字 |
| | `dsw-alias-label-dimmed` | 暗淡文字 |
| | `dsw-alias-label-primary-foreground` | 主标签前景色 |
| | `dsw-alias-label-primary-inverted` | 主标签反色 |
| 按钮 | `dsw-alias-button-primary-fill/hover` | 主按钮填充/悬停 |
| | `dsw-alias-button-ghost-active-fill/border/hover` | 幽灵按钮激活态 |
| | `dsw-alias-button-tool-bar-fill/hover` | 工具栏按钮 |
| | `dsw-alias-button-floating-fill/hover` | 浮动按钮 |
| | `dsw-alias-button-elevated-fill` | 凸起按钮 |
| | `dsw-alias-button-info-fill/hover` | 信息按钮 |
| 交互 | `dsw-alias-interactive-bg-hover` | 通用悬停背景 |
| | `dsw-alias-interactive-bg-active` | 通用激活背景 |
| | `dsw-alias-interactive-bg-hover-accent/danger/solid` | 特殊悬停态 |
| 状态 | `dsw-alias-state-error-primary/secondary` | 错误状态 |
| | `dsw-alias-state-success-primary/secondary/tertiary` | 成功状态 |
| | `dsw-alias-state-warn-primary/secondary/tertiary/label` | 警告状态 |
| | `dsw-alias-state-business-primary/tertiary` | 业务状态 |
| Markdown | `dsw-alias-markdown-citation` | 引用标注 |
| | `dsw-alias-markdown-code-block` | 代码块 |
| | `dsw-alias-markdown-code-block-banner` | 代码块横幅 |
| | `dsw-alias-markdown-inline-code` | 行内代码 |
| | `dsw-alias-markdown-placeholder` | Markdown 占位 |
| | `dsw-alias-markdown-tag` | Markdown 标签 |
| | `dsw-alias-markdown-code-segment-selected/unselected` | 代码段选中/未选中 |
| 滚动条 | `dsw-alias-scrollbar-bg-l1/l2` | 滚动条背景 |
| | `dsw-alias-scrollbar-hover-l1/l2` | 滚动条悬停 |

#### Component 层（组件级变量）

组件内部使用 `dsl-*` 前缀变量（如 `dsl-button-height`、`dsl-button-font-size`、`dsl-toast-color`），通过 CSS 变量级联实现主题切换。

#### Specific 层（特定组件覆盖）

| 变量 | 用途 |
|---|---|
| `dsw-specific-bubble` | 气泡背景 |
| `dsw-specific-bubble-highlight` | 高亮气泡 |
| `dsw-specific-input-major` | 主输入框 |
| `dsw-specific-login-input` | 登录输入框 |
| `dsw-specific-menu` | 菜单 |
| `dsw-specific-selector` | 选择器 |
| `dsw-specific-sidebar-fill` | 侧栏填充 |
| `dsw-specific-sidebar-nav-item-active` | 侧栏导航项激活态 |
| `dsw-specific-sidebar-nav-item-active-accent` | 侧栏导航项激活强调色 |
| `dsw-specific-sidebar-nav-item-hover` | 侧栏导航项悬停 |
| `dsw-specific-tip` | 提示文字 |

### 组件类名体系（`ds-` 前缀）

共 284 个 `ds-` 前缀 CSS 类，涵盖完整组件库：

| 组件类别 | 类名 | 变体 |
|---|---|---|
| 按钮 | `ds-button` | `--primary/--error/--warning/--text/--icon/--circle/--floating/--block/--loading/--disabled/--outlined/--filled/--borderless/--underlined` |
| | | 尺寸：`--xs/--s/--m/--l/--xl` |
| 输入框 | `ds-input` | `--bordered/--filled/--capsule/--none/--error/--disabled/--auto-height/--auto-width/--hide-scrollbar` |
| 文本域 | `ds-textarea` | 同上 |
| 选择器 | `ds-select` | `--bordered/--filled/--none/--open/--disabled/--error` |
| 复选框 | `ds-checkbox` | `--active/--disabled/--error` |
| 开关 | `ds-switch` | `--checked/--disabled/--size-xs/--size-s/--size-l` |
| 单选按钮 | `ds-radio-button` | `--active/--focus` |
| 分段控件 | `ds-segmented` | `--selected/--disabled` |
| 切换按钮 | `ds-toggle-button` | `--selected/--disabled` |
| 图标按钮 | `ds-icon-button` | — |
| 标签 | `ds-text` | `--label1/--label2/--label3/--block/--monospace` |
| 滚动区域 | `ds-scroll-area` | `--enabled/--show-on-focus-within` |
| 虚拟列表 | `ds-virtual-list` | `--empty-centered/--ios-compensation/--printable/--show-scrollbar/--x-scrollable` |
| 骨架屏 | `ds-skeleton` | `--circle/--rect/--text/--has-children/--sharp` |
| 通知 | `ds-notification` | `--bottom-left/--top-right` |
| Toast | `ds-toast` | `--error/--success/--warning` |
| 提示 | `ds-tooltip` | `--popover/--show-shadow` |
| 模态框 | `ds-modal` | `--dialog/--hide` |
| 侧栏 | `ds-sider` | — |
| 浮动容器 | `ds-floating-container` | — |
| 焦点环 | `ds-focus-ring` | — |
| Markdown | `ds-markdown` | `--cite/--html/--math/--math-display/--math-svg/--paragraph/--task-checkbox/--task-list-item` |
| 代码块 | `md-code-block` | `-banner/--banner-wrap/--banner-lite/--action/--footer/--infostring/--dark/--light` |
| 思考内容 | `ds-think-content` | — |
| 搜索视图卡片 | `search-view-card` | `__title/__snippet` |
| 复制行 | `ds-copyable-text-line` | — |
| Alert | `ds-alert` | `--bordered/--error/--info/--success/--warning` |
| Banner | `ds-banner` | `--error/--info/--primary/--warning` |
| 下拉菜单 | `ds-dropdown-menu` | `--option/--option--error/--option--pending` |
| 加载 | `ds-loading` | — |

---

## 3. 侧栏与会话管理

### 会话分组

会话列表按时间分组显示：

```
今天 (todaySessions)
├── 昨天 (yesterdaySessions)
├── 7天内 (days7Sessions)
├── 30天内 (days30Sessions)
└── 更早 (previousSessions)
```

每个分组有独立的 section title，空分组不显示。

### 会话项功能

每个会话项（session item）支持以下操作：

| 功能 | 触发方式 | i18n 键 |
|---|---|---|
| 重命名 | 右键菜单 / 双击 | `sessionItemMenuRenameOption` |
| 置顶 | 右键菜单 | `pinSession` |
| 取消置顶 | 右键菜单（已置顶时） | `unpinSession` |
| 删除 | 右键菜单 | `deleteSession` |
| 分享 | 右键菜单 | `shareEntryClick` |

### 置顶会话区域

- 置顶会话单独显示在顶部区域 `pinnedSectionTitle`
- 支持折叠/展开置顶会话区域 `pinnedSessionsFolded`（boolean 状态）
- 置顶数量上限 `pinSessionTooMany`（超过时 toast 提示）
- 操作过快限制 `operationTooFastToast`

### 侧栏折叠/展开

- 折叠按钮 `collapseSiderbarButton` / 展开按钮 `expandSiderbarButton`
- 折叠状态下悬停可展开（`hideTooltip` + `onCollapsedChange`）
- 侧栏宽度通过 CSS 变量 `--sidebar-width` 控制
- 侧栏缩放 `sidebarZoomIn` 功能

### 新建对话

- 新建对话按钮 `chatInputNewChatButton` + tooltip `chatInputNewChatButtonTooltip`
- 快捷键支持
- 已在新对话页面时再点新建 → toast `alreadyInNewChatToast`

### 会话删除流程

1. 点击删除 → 打开 `deleteSessionModal`（模态框）
2. 模态框内容：
   - 标题 `deleteSessionModalTitle` / `deleteSessionModalContentTitle`
   - 描述 `deleteSessionModalDescription` / `deleteSessionModalContentDescription`
   - 确认按钮 `deleteSessionModalConfirm` / `deleteSessionModalContentConfirmButton`
   - 取消按钮 `deleteSessionModalContentCancelButton`
3. 确认 → 调用 API → 成功后 `sessionDeleted` toast

### 批量删除历史

- 头像菜单 → 设置 → 删除所有历史
- 模态框：`deleteAllHistoryModalTitle` / `deleteAllHistoryModalContent` / `deleteAllHistoryModalConfirm` / `deleteAllHistoryModalCancel`
- 成功 toast `deleteAllHistoryOkToast`
- 另有删除所有会话模态框：`deleteAllSessionsModalTitle` / `deleteAllSessionsModalDescription` / `deleteAllSessionsModalConfirm`

---

## 4. 对话区域与消息渲染

### 消息角色

```
MessageRole.USER      — 用户消息
MessageRole.ASSISTANT — AI 消息
MessageRole.SYSTEM    — 系统消息（不直接显示）
```

### 消息状态机

```
MessageStatus.WIP                    — 生成中（Work In Progress）
MessageStatus.OK                     — 正常完成
MessageStatus.INCOMPLETE             — 不完整（流中断但保留了部分内容）
MessageStatus.FINISHED               — 完全结束
MessageStatus.CONTENT_FILTER         — 内容审查过滤
MessageStatus.CONTEXT_LENGTH_EXCEEDED — 上下文超长
MessageStatus.TIMEOUT                — 超时
```

### 生成状态机（GenerateState）

```
GenerateState.INITIALIZING         — 初始化（请求发出，等待响应）
GenerateState.AWAITING_FIRST_CHANGE — 等待首个 token（消除空白等待）
GenerateState.GENERATING           — 正在生成（持续接收 token）
```

**关键交互**：`AWAITING_FIRST_CHANGE` 状态确保在首个 token 到达前显示 loading 动画，避免用户看到空白界面。

### 最小加载延迟

`ensureMinLoadingDelay` — 即使 AI 响应极快（< 某个阈值），也会保持 loading 状态至少一定时间，避免闪烁。

### 消息渲染结构

```html
<!-- 用户消息 -->
<div class="ds-specific-bubble">
  <div class="messageBody">用户消息内容</div>
</div>

<!-- AI 消息 -->
<div class="ds-assistant-message-main-content">
  <!-- 思考链区域（可折叠） -->
  <div class="ds-think-content">...</div>
  <!-- 搜索结果区域（可折叠） -->
  <div class="search-view-card">...</div>
  <!-- 主回答内容（Markdown 渲染） -->
  <div class="messageBody">
    <div class="ds-markdown">...</div>
  </div>
  <!-- 消息操作按钮 -->
  <div class="message-actions">...</div>
</div>
```

### 消息操作（hover 出现）

| 操作 | 功能 | 触发 |
|---|---|---|
| 复制 | 复制消息全文 | `getCopyContent` |
| 重新生成 | 重新生成 AI 回答 | `regenerate` |
| 编辑 | 编辑用户消息并重新发送 | `editMessage` |

### AI 免责声明

每条 AI 消息底部显示 AI 免责声明（`ai-disclaimer` 类似组件）。

### 消息引用（Citation）

- AI 回答中的引用标记使用 `ds-markdown-cite` 类
- 引用编号 `[1]` `[2]` 等可点击
- 点击后跳转到对应的搜索结果/来源
- `CiteWrapper` 组件封装引用交互
- `getLink` / `useLink` hooks 获取引用链接

---

## 5. 输入区域

### 输入框特性

| 特性 | 说明 |
|---|---|
| 自动高度 | `ds-textarea--auto-height` — 随内容增长 |
| 自动宽度 | `ds-textarea--auto-width` |
| 隐藏滚动条 | `ds-textarea--hide-scrollbar` |
| 组合输入支持 | `onCompositionStart` / `onCompositionEnd` — 中文 IME 输入法支持 |
| 字符计数器 | `charCounterVisible` / `charCounterNode` — 超限时显示计数 |
| 超限拦截 | `interceptSubmit` — 超过字符限制时拦截发送 |
| 粘贴超限提示 | `notifyPasteOverLimit` + `inputOverLimitPercentage` |
| 尝试提交 | `onTryToSubmit` — 超限时点击发送显示提示 |

### 输入区 Toggle 按钮

两个核心 toggle 按钮：

1. **DeepThink（思考）按钮**
   - 图标：动画 Lottie（`animationData: vb` + `animateOff: true` + `size: 14`）
   - Tooltip：`"Think before responding to solve reasoning problems"`
   - 状态：`thinkingEnabled` (boolean)
   - 切换事件：`thinkingSwitchToggled`
   - 文本标签：`"Think"` / `"Search"`（内联显示）
   - 自动折叠：`autoCollapse` 属性（窄屏时只显示图标）
   - 样式变换：`transform` 属性

2. **Search（联网搜索）按钮**
   - Tooltip：`"Search the web when necessary"`
   - 状态：`searchEnabled` (boolean)
   - 切换事件：`searchEnabledToggle`
   - 搜索状态触发版本控制：`searchStateTriggerAppliedVersionStorageHandle`

### Toggle 按钮 CSS

```css
.ds-toggle-button {
  --dsl-button-selected-*: 选中态变量
  --dsl-button-disabled-*: 禁用态变量
}
.ds-toggle-button--selected { /* 选中态样式 */ }
.ds-toggle-button--disabled { /* 禁用态样式 */ }
```

### 文件上传按钮

- 位于输入框左侧
- Tooltip 标题 `uploadFileButtonTooltipTitle` + 描述（含最大数量 `maxCount` + 最大大小 `maxSizeMB`）
- 拖拽上传支持 `dragFileErrorTip` + `dragFileDescOCR`
- 文件类型不支持时提示 `searchNotSupportFiles`
- 搜索模式下不支持文件 → `deleteFilesToSearchTooltip`

### 发送逻辑

- **Enter** 发送（非 IME 组合输入状态）
- **Shift+Enter** 换行
- **发送中** 按钮变为停止按钮 `stop`
- 停止后保留已生成内容，消息状态变为 `INCOMPLETE`

---

## 6. 思考链（DeepThink）

### 核心交互

1. 用户开启 `thinkingEnabled` toggle
2. 发送消息时，请求带 `thinkingEnabled: true` 参数
3. AI 响应中先返回思考过程，再返回正式回答
4. 思考过程显示在可折叠的 `ds-think-content` 区域
5. 思考完成后显示思考耗时

### 思考状态

| 状态 | i18n 键 | 说明 |
|---|---|---|
| 思考中 | `messageThinking` | "正在思考..." |
| 思考耗时 | `messageThinkDuration` | "思考了 X 秒" |
| 思考耗时（复数） | `messageThinkDurationPlural` | "思考了 X 秒" |
| 思考停止 | `messageThinkStopped` | 用户主动停止 |
| 是否在思考 | `isThinking` | 状态判断 |
| 思考开关 | `thinkingSwitch` | toggle 按钮 |
| 思考停止按钮 | `thinkingStop` | 停止思考 |
| 思考动画环 | `think-ring-final` | 思考中的动画效果 |

### 思考链折叠/展开

- 使用 `fade-in-zoom-in-expand` 动画
- 展开状态：`expanded` / `expanding`
- 折叠状态：`collapsed` / `collapsing`
- 折叠区域标题高度 CSS 变量：`--collapsible-area-title-height`
- 切换函数：`toggleCollapsibleArea`

### 限制条件

| 条件 | i18n 键 | 说明 |
|---|---|---|
| 思考配额超限 | `THINKING_QUOTA_EXCEEDED` | 思考功能配额用完 |
| 思考与文件不兼容 | `THINKING_CANNOT_BE_WITH_FILE` | 上传文件时不能使用思考 |
| 思考被停用 | `deactivatedThinkTooltip` | 思考功能被停用时的提示 |

---

## 7. 联网搜索（DeepSearch）

### 核心交互

1. 用户开启 `searchEnabled` toggle
2. 发送消息时，请求带 `searchEnabled: true` 参数
3. AI 先执行网页搜索，获取搜索结果
4. 基于搜索结果生成回答
5. 回答中包含引用标注 `[1]` `[2]` 等
6. 搜索结果以卡片形式显示在可折叠区域

### 搜索状态

| 状态 | i18n 键 | 说明 |
|---|---|---|
| 搜索中 | `messageSearching` | "正在搜索..." |
| 带查询搜索 | `messageSearchingWithQuery` | "正在搜索: {query}" |
| 搜索结果 | `messageSearchResult` / `messageSearchResults` | "找到 X 个结果" |
| 无结果 | `messageSearchNoResult` | "未找到相关结果" |
| 搜索摘要 | `messageSearchSummary` / `messageSearchSummaries` | 结果摘要 |
| 搜索停止 | `searchStopped` | 用户停止搜索 |
| 深度搜索思考 | `deepSearchThinking` | DeepSearch 思考阶段 |
| 深度搜索中 | `deepSearchSearching` | DeepSearch 搜索阶段 |
| 搜索引用 | `deepSearchReference` | 搜索引用标注 |
| 组合搜索结果 | `composedSearchResults` | 多来源搜索结果合并 |

### 搜索结果卡片

```html
<div class="search-view-card">
  <div class="search-view-card__title">网页标题</div>
  <div class="search-view-card__snippet">内容摘要</div>
</div>
```

### 搜索结果追踪事件

| 事件名 | 触发时机 |
|---|---|
| `search_result_show` | 搜索结果展示 |
| `search_result_click` | 点击搜索结果 |
| `search_no_result_show` | 无结果展示 |
| `search_fail_show` | 搜索失败展示 |
| `search_citation_show` | 引用标注展示 |
| `search_citation_click` | 点击引用标注 |
| `search_list_icon_show` | 搜索列表图标展示 |
| `search_list_expand` | 搜索列表展开 |
| `search_link_show` | 搜索链接展示 |
| `search_link_click` | 点击搜索链接 |
| `search_panel_click` | 搜索面板点击 |
| `search_force_on` | 强制开启搜索 |
| `search_force_off` | 强制关闭搜索 |

### 搜索状态触发

搜索功能的启用/禁用状态根据场景自动触发：

| 场景 | i18n 键 |
|---|---|
| 启动时 | `search_state_on_launch` |
| 登录时 | `search_state_on_login` |
| 手动创建会话时 | `search_state_on_manually_created_chat` |
| 自动创建会话时 | `search_state_on_automatically_created_chat` |

### 搜索限制

| 限制 | i18n 键 | 说明 |
|---|---|---|
| 字符不兼容 | `SEARCH_NOT_ALLOWED_WITH_CHARACTER` | 特殊字符不能搜索 |
| 文件不兼容 | `SEARCH_CANNOT_WITH_FILE` | 上传文件时不能搜索 |
| 删除文件提示 | `deleteFilesToSearchTooltip` | 需先删除文件才能搜索 |
| 搜索不支持文件 | `searchNotSupportFiles` | 搜索不支持文件类型 |
| 搜索限制 | `searchLimit` | 搜索数量限制提示 |
| 搜索无结果 | `searchGotNothing` | 搜索无任何结果 |
| 搜索被停用 | `deactivatedSearchTooltip` | 搜索功能被停用 |
| 搜索禁用 | `searchDisabled` | 搜索功能不可用 |
| 文件搜索错误 | `hintMessageSearchWithFileError` | 文件搜索错误提示 |

### 搜索 API 参数

```javascript
{
  search_request_id: "...",
  parent_search_request_id: "...",
  query: "搜索查询",
  search_page_num: 1,
  search_no_result_type: "..."
}
```

### 引用交互

- `CiteWrapper` 组件封装引用标注
- `getLink(sessionId, messageId)` — 获取引用链接
- `useLink(sessionId, messageId, fragmentId)` — React hook 获取引用
- `useReferenceIndexMap` — 引用索引映射
- `useReferenceSchema` — 引用 schema（`conversationMode` 判断）
- 点击引用 → 跳转到搜索结果卡片 → 高亮

---

## 8. 工具调用（Tool Open/Search/Find）

DeepSeek 支持三种工具调用类型：

| 工具类型 | 常量 | 功能 |
|---|---|---|
| `TOOL_OPEN` | 打开网页 | 浏览网页内容（`toolOpenBrowse` / `toolOpenBrowseResults` / `toolOpenViewAll`） |
| `TOOL_SEARCH` | 搜索 | 执行搜索查询 |
| `TOOL_FIND` | 查找 | 在页面中查找信息（`toolFindWip` = "Finding..." / `toolFindNoResult` = "未找到"） |

### 工具调用交互

1. AI 回答中嵌入工具调用标记
2. 显示工具调用分组：`groupTitleToolOpen` / `groupTitleToolFind`
3. 每个工具调用结果显示为可点击的链接
4. `TOOL_OPEN` 点击 → 打开外部链接（`en.y.openLink`）
5. `TOOL_FIND` → 在页面内查找
6. "查看全部" 按钮 `toolOpenViewAll` → 展开所有工具调用结果

### 工具搜索状态

| 状态 | i18n 键 |
|---|---|
| 进行中 | `toolSearchWip` |
| 无结果 | `toolSearchNoResult` |
| 1个结果 | `toolSearch1Result` |
| 完成 | `toolSearchFinished` |
| 失败 | `toolSearchFailed` / `toolSearchFail` |
| 结果 | `toolSearchResults` |
| 查询进行中 | `toolSearchQueryWIP` |
| 介绍 | `toolSearchIntro` |

### Provider

- `ToolOpenListViewProvider` — 工具打开列表视图 Context
- `useToolOpenListViewController` — 必须在 Provider 内使用

---

## 9. 文件上传系统

### 文件解析状态

```
FileParseStatus.PENDING                  — 等待解析
FileParseStatus.PARSING                  — 解析中
FileParseStatus.SUCCESS                   — 解析成功
FileParseStatus.FAILED                    — 解析失败
FileParseStatus.CONTENT_EMPTY             — 内容为空
FileParseStatus.CONTENT_TOO_LONG          — 内容过长
FileParseStatus.CONTENT_FILTER            — 内容审查过滤
FileParseStatus.CANCELLED                 — 已取消
FileParseStatus._CUSTOM_SYSTEM_ERROR_FAIL — 自定义系统错误
```

### 上传错误码

```
UploadFileErrorCode.OK                       — 成功
UploadFileErrorCode.EMPTY_FILE               — 空文件
UploadFileErrorCode.EMPTY_FILE_NAME          — 空文件名
UploadFileErrorCode.FILE_NAME_TOO_LONG       — 文件名过长
UploadFileErrorCode.FILE_OVER_SIZE           — 文件超大小
UploadFileErrorCode.UNSUPPORTED_FILE_TYPE    — 不支持的文件类型
UploadFileErrorCode.CONCURRENCY_RATE_LIMIT   — 并发限流
UploadFileErrorCode.RPD_RATE_LIMIT           — RPD 限流
UploadFileErrorCode.SERVER_BUSY              — 服务器忙
UploadFileErrorCode.API_ERROR                — API 错误
UploadFileErrorCode.BROKEN_DATA_RECEIVED     — 数据损坏
UploadFileErrorCode.MUTE                     — 静默错误
```

### 上传交互

- 拖拽上传 `dragFileErrorTip` + `dragFileDescOCR`
- 文件选择器 `file_picker`
- 图片压缩 `photoPickerCompressRatio` + WebP 格式 `image/webp`
- 文件信息映射 `fileInfoMap` + 文件归属信息 `fileBelongInfo`
- 文件源 `fileSource`（标记文件来源）
- 上传面板 `upload_panel`
- 上传动作点击 `upload_action_click`
- 上传失败 `upload_failed` / `uploadFileFailed` / `uploadFileStatusFailed`
- 上传成功 `uploadFileSuccess`
- 多文件 Token 超限 `uploadMultipleFileTokenCountExceededPrompt` / `uploadMultipleFileTokenCountExceededPrompt2`
- 最大大小 `uploadMaxMBSize`
- 文件轮询 `ensurePollingAllFilesInfo` — 持续轮询文件解析状态
- 文件重试 `retryableFileIds` — 可重试的文件 ID 列表

### 文件上传与模型/搜索的关系

- 不同模型有不同的上传 tooltip：`modelSwitchDefaultUploadTooltip` / `modelSwitchExpUploadTooltip` / `modelSwitchVisUploadTooltip`
- 实验模型可能禁用上传：`modelSwitchExpUploadDisabledTooltip`
- 无文本/图片模型 banner：`modelSwitchNoTextImagesBanner`
- 文件上传时携带 `thinkingEnabled` 和 `modelType` 参数

---

## 10. 消息编辑与重新生成

### 消息编辑

1. 悬停用户消息 → 显示编辑按钮 `editMessageTooltip`
2. 点击编辑 → 消息内容变为可编辑文本域
3. 编辑界面有发送按钮 `editMessageSendButton` 和取消按钮 `editMessageCancelButton`
4. 点击发送 → 以新消息重新发送（保留后续对话或截断）
5. 追踪事件 `clickEditMessageButton`

### 重新生成

| 选项 | i18n 键 | 说明 |
|---|---|---|
| 重新生成 | `regenerate` | 默认重新生成 |
| 简洁模式 | `regenerateConcise` | 简洁风格重新生成 |
| 详细模式 | `regenerateDetailed` | 详细风格重新生成 |
| 强制搜索 | `regenerateForceSearch` | 重新生成并强制开启搜索 |
| 强制不搜索 | `regenerateForceNoSearch` | 重新生成并强制关闭搜索 |
| 不可用 | `regenerateNotAvailable` | 重新生成不可用 |
| 配额超限 | `regenerateQuotaExceededToast` | 重新生成配额超限 toast |
| 重试 | `regenerateTryAgain` | 重试 |
| 坏消息重新生成 | `completionRegenerateOnBadMessageToast` | 消息质量差时重新生成 |

### 流恢复

- `resumeStream` — 恢复中断的流式响应
- `autoResume` — 自动恢复
- `autoResumeAbort` — 自动恢复中止
- `autoResumeBreak` — 自动恢复中断
- `autoResumeDeferred` — 延迟自动恢复
- `autoResumeAbort` — 恢复请求中止

---

## 11. 分享功能

### 分享链接管理

| 功能 | i18n 键 |
|---|---|
| 分享链接管理按钮 | `shareLinksManageButton` |
| 管理描述 | `shareLinksManageDescription` |
| 复制 | `shareLinksManageCopy` |
| 删除 | `shareLinksManageDelete` |
| 删除确认标题 | `shareLinksManageDeleteConfirmTitle` |
| 删除确认描述 | `shareLinksManageDeleteConfirmDesc` |
| 删除成功 toast | `shareLinksManageDeleteOkToast` |
| 删除失败 toast | `shareLinksManageDeleteFailedToast` |
| 分享时间 | `shareLinksManageSharedAt` |
| 管理页空状态 | `shareLinksManagementEmpty` |

### 分享选择

| 功能 | i18n 键 |
|---|---|
| 全选 | `shareSelectAll` |
| 选择超限 toast | `shareSelectLimitExceededToast` |
| 消息未就绪 toast | `shareSelectMessageNotReadyToast` |
| 消息禁用 toast | `shareSelectMessageDisabledToast` |
| 创建分享链接 | `clickCreateShareLink` |
| 确认创建并复制 | `clickConfirmCreateAndCopy` |
| 取消分享选择 | `clickCancelShareSelect` |
| 继续对话 | `shareContinueChat` |

### 分享 ID

- 每个分享有唯一 `shareId`
- 分享 URL 格式：`/a/chat/s/{shareId}`
- 支持从分享页面继续对话

---

## 12. 会话历史搜索

### 历史搜索功能

| 功能 | i18n 键 / 变量 |
|---|---|
| 搜索触发按钮 | `historySearchTriggerButton` |
| 搜索占位符 | `historySearchPlaceholder` |
| 搜索年月日格式 | `historySearchYMD` |
| Markdown 搜索结果 | `historySearchMD` |
| 列表 Aria 标签 | `historySearchListAriaLabel` |
| 无结果提示 | `historySearchNoResultTip` |
| 最近无结果 | `historySearchNoResultRecently` (复数模板) |
| 无消息找到 toast | `historySearchNoMessageFoundToast` |
| 无更多结果 | `historySearchNoLoadEarlierResult` |
| 搜索失败 | `historySearchFailed` (含重试按钮 `historySearchFailedRetryButton`) |
| 加载更多失败 | `historySearchLoadMoreFailed` (含超时 `historySearchLoadMoreTimeout`) |
| 文件搜索错误 | `hintMessageSearchWithFileError` |
| 快捷键 | `shortcutOpenConversationSearch` |
| 防抖延迟 | `conversation_search_debounce_ms` |
| 是否启用 | `conversation_search_enabled` |
| 搜索结果前缀 | `conversation-search-result-` |
| 搜索结果索引 | `data-conversation-search-result-index` |
| 搜索结果容器 | `conversation-search-results` |
| 搜索结果切换 | `toggleSearchList` |
| 跳转到搜索结果 | `gotoSearchResult` |
| 高亮目标 | `pendingSearchHighlightTarget$` |
| 高亮 ID | `pendingHighlightId$` |

### 历史搜索交互流程

1. 用户按快捷键 `shortcutOpenConversationSearch` 或点击搜索按钮
2. 侧栏顶部出现搜索输入框 `historySearchPlaceholder`
3. 输入关键词，防抖 `conversation_search_debounce_ms` 后触发搜索
4. 搜索结果按会话分组显示，每个结果显示匹配的消息摘要
5. 点击搜索结果 → 跳转到对应会话 → 滚动到匹配消息 → 高亮匹配文本
6. 支持加载更多 `loadEarlierOnly` — 仅加载更早的结果
7. 搜索结果为空时显示 `historySearchNoResultTip`
8. 搜索失败时显示重试按钮

### 搜索结果视图

- 使用 `SearchViewProvider` Context 管理
- `useSearchViewController` hook 必须在 Provider 内使用
- `SearchContext` 提供搜索上下文
- 搜索结果使用虚拟列表渲染

---

## 13. 模型切换

### 模型类型

DeepSeek 支持多种模型类型，通过 `modelType` / `model_type` 字段标识：

- 模型配置存储在 `model_configs`
- 模型列表 `nh.find(e => e === t)` 查找匹配模型
- `modelType` 状态在会话级别维护
- 切换模型时追踪事件 `user_click` + `model_type`

### 模型切换 UI

- 模型选择器位于输入框上方或侧栏
- 不同模型有不同的文件上传 tooltip：
  - `modelSwitchDefaultUploadTooltip` — 默认模型上传提示
  - `modelSwitchExpUploadTooltip` — 实验模型上传提示
  - `modelSwitchVisUploadTooltip` — 视觉模型上传提示
  - `modelSwitchExpUploadDisabledTooltip` — 实验模型禁用上传
  - `modelSwitchNoTextImagesBanner` — 不支持文本/图片的模型 banner

### 会话模式

`conversationMode` — 对话模式，影响消息渲染方式：
- 普通模式
- 引用模式（`useReferenceSchema` 判断）
- 不同模式的引用索引映射不同（`useReferenceIndexMap`）

---

## 14. 主题与国际化

### 主题系统

| 选项 | i18n 键 | 说明 |
|---|---|---|
| 深色 | `avatarMenuSettingsDialogThemeSelectDarkOption` | 深色主题 |
| 浅色 | `avatarMenuSettingsDialogThemeSelectLightOption` | 浅色主题 |
| 跟随系统 | `avatarMenuSettingsDialogThemeSelectSystemOption` | 跟随系统主题 |
| 主题设置项 | `avatarMenuSettingsDialogThemeItemLabel` | 主题设置标签 |

### 主题实现

```javascript
// 主题存储
_themePreference  // 用户主题偏好

// 系统主题监听
systemThemeChange  // 系统主题变化事件
getOsTheme()       // 获取系统主题

// DOM 标记
document.body.dataset.dsDarkTheme = "true" / "false"

// CSS 媒体查询
@media (prefers-color-scheme: dark) { ... }
@media (prefers-color-scheme: light) { ... }

// 暗色主题类
ds-dark-theme  // 暗色主题 CSS 类
```

### 国际化

| 功能 | 说明 |
|---|---|
| 可用语言 | en_US, zh_CN, zh_HK, 以及 nl/kn/uk/tr/vi/ms/nb/fil/sk/hr/sr/ca/rm/hy/fi 等 20+ 语言 |
| 语言存储 | `_localePreference` |
| 语言切换 | `settingsLocalePreferenceSystem` (跟随系统) + 手动选择 |
| 语言代理 | Proxy 对象懒加载语言包 |
| 请求头 | `x-client-locale` — API 请求带语言标识 |
| 组件翻译 | `en.R.useTranslate()` + `translateComponent.*` |
| 多语言模板 | 支持 ICU MessageFormat（复数、选择等） |

---

## 15. 动画与过渡

### CSS 动画

| 动画名 | 用途 |
|---|---|
| `ds-loading` | 加载旋转动画 |
| `ds-skeleton-loading` | 骨架屏闪烁动画 |
| `slide-fade-in` | 滑入淡入 |
| `slide-fade-out` | 滑出淡出 |
| `a9ca0bf3` / `b6902930` / `b7134c06` / `c3eff0bd` / `cd8dce10` | 哈希命名的动画（混淆后） |

### 过渡效果

| 类名 | 效果 |
|---|---|
| `ds-fade-in-enter` / `ds-fade-in-enter-active` | 淡入 |
| `ds-fade-in-exit` / `ds-fade-in-exit-active` | 淡出 |
| `ds-fade-in-zoom-in-enter` / `-active` | 缩放淡入 |
| `ds-fade-in-zoom-in-exit` / `-active` | 缩放淡出 |
| `ds-fade-in-zoom-in-expand-enter` / `-active` | 展开缩放淡入 |
| `ds-fade-in-zoom-in-expand-exit` / `-active` | 展开缩放淡出 |
| `ds-cross-fade-enter` / `-active` / `-exit` / `-active` | 交叉淡入淡出 |
| `ds-animated-size-item` | 尺寸动画 |

### 过渡变量

```css
--ds-transition-duration       — 标准过渡时长
--ds-transition-duration-fast  — 快速过渡时长
--ds-transition-duration-slow  — 慢速过渡时长
--ds-ease-in                   — 缓入
--ds-ease-out                  — 缓出
--ds-ease-in-out               — 缓入缓出
```

### 展开缩放动画（思考链/搜索结果折叠）

```css
.ds-fade-in-zoom-in-expand-enter {
  opacity: 0;
  transform: scale(0.8);
}
.ds-fade-in-zoom-in-expand-enter-active {
  opacity: 1;
  transition: max-height var(--ds-ease-in) var(--ds-transition-duration-fast),
              opacity var(--ds-ease-in) var(--ds-transition-duration-fast),
              transform var(--ds-ease-in) var(--ds-transition-duration-fast);
  transform: scale(1);
}
.ds-fade-in-zoom-in-expand-exit {
  opacity: 1;
  transform: scale(1);
}
.ds-fade-in-zoom-in-expand-exit-active {
  opacity: 0;
  transition: max-height var(--ds-ease-out) var(--ds-transition-duration-fast),
              opacity var(--ds-ease-out) var(--ds-transition-duration-fast),
              transform var(--ds-ease-out) var(--ds-transition-duration-fast);
  transform: scale(0.8);
}
/* 展开方向 */
.ds-fade-in-zoom-in-expand-top { align-items: flex-start; }
.ds-fade-in-zoom-in-expand-center { align-items: center; }
```

### Toast 动画

- Toast 出现时 `ds-toast-animation` — `margin-top: 16px` 滑入
- 消失延迟 `disappearDelay` — 可配置
- 出现延迟 `enterDelay: 300` — 300ms 延迟

---

## 16. Markdown 渲染系统

### Markdown 组件结构

```html
<div class="ds-markdown">
  <!-- 段落 -->
  <div class="ds-markdown-paragraph">...</div>

  <!-- 代码块 -->
  <div class="md-code-block">
    <div class="md-code-block-banner-wrap">
      <div class="md-code-block-banner">
        <span class="md-code-block-infostring">language</span>
        <div class="md-code-block-action">
          <button class="ds-markdown-code-copy-button">Copy</button>
        </div>
      </div>
    </div>
    <pre><code class="language-xxx">...</code></pre>
    <div class="md-code-block-footer">...</div>
  </div>

  <!-- 行内代码 -->
  <code class="ds-markdown-inline-code">...</code>

  <!-- 数学公式 -->
  <div class="ds-markdown-math ds-markdown-math-svg ds-markdown-math-display">
    <div class="katex">...</div>
  </div>

  <!-- 引用标注 -->
  <sup class="ds-markdown-cite">[1]</sup>

  <!-- 任务列表 -->
  <input type="checkbox" class="ds-markdown-task-checkbox" />
  <li class="ds-markdown-task-list-item">...</li>

  <!-- 表格 -->
  <div class="markdown-table-wrapper">
    <table>...</table>
  </div>

  <!-- HTML -->
  <div class="ds-markdown-html">...</div>
</div>
```

### 代码块交互

- **复制按钮**：`ds-markdown-code-copy-button` — 点击复制代码内容
- **语言标签**：`md-code-block-infostring` — 显示编程语言名称
- **横幅**：`md-code-block-banner` — 代码块顶部横幅（深色 `md-code-block-dark` / 浅色 `md-code-block-light`）
- **精简横幅**：`md-code-block-banner-lite` — 精简模式横幅
- **底部**：`md-code-block-footer` — 代码块底部
- **代码执行**：`"运行markdown code"` — 支持运行代码块
- **代码高亮**：Prism.js 语法高亮（token 类：`.keyword` `.string` `.function` `.comment` `.number` `.operator` `.boolean` `.class-name` `.builtin` 等）
- **彩虹括号**：`.rainbow-braces` + `.brace-level-1` 到 `.brace-level-12` — 12 级彩虹括号嵌套着色
- **行号**：`.line-numbers` + `.line-numbers-rows` — 行号显示
- **行高亮**：`.line-highlight` — 行高亮
- **命令行**：`.command-line` + `.command-line-prompt` — 命令行样式
- **差异高亮**：`.diff-highlight` + `.inserted` / `.deleted` — 代码差异高亮

### Mermaid 图表

- Mermaid 图表渲染支持
- Mermaid 下载：`mermaidDownloadCode` / `mermaidDownloadPng` — 下载为代码/PNG
- Mermaid 错误边界：`"触发mermaid渲染ErrorBoundary"` — Mermaid 渲染失败时不崩溃
- 图片查看器（Fancy Box）：`ds-fancy-box` + `ds-fancy-box__top-toolbar` / `ds-fancy-box__side-toolbar` — 图片查看面板
  - 缩放：`zoomIn` / `zoomOut` / `setZoomOut`
  - 下载：`downloadSvg` / `downloadPng`
  - 加载失败：`ds-fancy-box__load-fail`

### Markdown 提示（Tips）

- `useTips` — 获取消息提示
- `messageBody.useTips()` — 消息体使用提示

---

## 17. 虚拟列表与滚动管理

### 虚拟列表

```css
.ds-virtual-list                  — 虚拟列表容器
.ds-virtual-list--empty-centered  — 空状态居中
.ds-virtual-list--ios-compensation-enabled — iOS 补偿
.ds-virtual-list--printable       — 可打印
.ds-virtual-list--show-scrollbar  — 显示滚动条
.ds-virtual-list--x-scrollable    — 水平滚动
.ds-virtual-list-visible-items    — 可见项容器
.ds-virtual-list-empty            — 空状态
.ds-virtual-list-empty--center    — 空状态居中
```

### 滚动管理

| 功能 | 变量/函数 | 说明 |
|---|---|---|
| 滚动 DOM 引用 | `scrollDomRef` | 滚动容器 ref |
| 滚动位置 | `scrollTop` | 当前滚动位置 |
| 滚动高度 | `scrollHeight` | 内容总高度 |
| 滚动到 | `scrollTo` | 滚动到指定位置 |
| 滚动事件 | `onScroll` | 滚动事件处理 |
| 列表滚动 | `listScroll` | 列表滚动逻辑 |
| 会话更新时调整滚动 | `adjustScrollOnSessionUpdated` | 会话更新时自动调整滚动位置 |
| 渲染异常 | `virtualListRenderAnomaly` | 虚拟列表渲染异常检测 |

### 滚动区域组件

```css
.ds-scroll-area                     — 滚动区域
.ds-scroll-area--enabled            — 启用状态
.ds-scroll-area--show-on-focus-within — 聚焦时显示滚动条
.ds-scroll-area__gutters            — 滚动条沟槽
.ds-scroll-area__horizontal-bar     — 水平滚动条
.ds-scroll-area__horizontal-gutter  — 水平沟槽
.ds-scroll-area__vertical-bar       — 垂直滚动条
.ds-scroll-area__vertical-gutter    — 垂直沟槽
```

### 消息列表滚动 Provider

- `MessageListScrollProvider` — 消息列表滚动 Context
- `useMessageListScroll` — 必须在 Provider 内使用
- `pendingHighlightId$` — 待高亮消息 ID（用于搜索跳转后高亮）

---

## 18. 错误处理与恢复

### 错误边界

| 层级 | 错误边界 | 触发 |
|---|---|---|
| 路由级 | `"触发路由 Error Boundary"` | 路由级错误 |
| 预览级 | `"触发预览 Error Boundary"` | 预览组件错误 |
| Mermaid 级 | `"触发mermaid渲染ErrorBoundary"` | Mermaid 渲染错误 |
| 独立恢复 | `"展示独立的ErrorRecovery"` | 独立错误恢复组件 |

### 错误 Toast

| Toast | i18n 键 |
|---|---|
| 通用网络错误 | `commonNetworkErrorToast` |
| 访客网络错误 | `guestPowNetworkErrorToast` |
| 无效 token | `commonInvalidTokenToast` |
| 用户被封禁 | `userIsBannedToast` |
| IP 访问限制 | `ipAccessRestrictedToast` |
| Google 登录 mainland | `googleSignInFromMainlandToast` |
| 微信绑定失败 | `bindWechatFailed` / `bindWechatLoadFailed` / `bindWechatUnbindFailed` |
| 微信登录失败 | `signInWithWechatFailed` / `"Wechat 登录失败"` |
| Apple 登录失败 | `signInWithAppleFailed` |
| Google 登录失败 | `signInWithGoogleFailed` |
| 注册失败 | `signUpFailed` / `signUpByMobileFailed` |
| 登录失败 | `signInFailed` |
| SMS 发送失败 | `smsCodeSendFailed` |
| 邮件发送失败 | `emailCodeSendFailed` |
| SMS 验证失败 | `checkSmsCodeFailed` |
| 邮件验证失败 | `checkEmailCodeFailed` |
| 重置密码失败 | `resetPasswordMobileFailed` / `resetPasswordEmailFailed` |
| 微信解绑失败 | `unbindWechatFailed` |
| 微信手机验证失败 | `mobileVerificationWithWechatFailed` / `mobileVerificationWithAppleFailed` |
| 重命名会话失败 | `renameFailed` / `renameSessionApiFailed` |
| 置顶会话失败 | `pinSessionFailed` |
| 取消置顶失败 | `unpinSessionFailed` |
| 分享链接删除失败 | `shareLinksManageDeleteFailedToast` |
| 上传文件失败 | `uploadFileFailed` / `uploadFileStatusFailed` |
| 重新生成配额超限 | `regenerateQuotaExceededToast` |
| 操作过快 | `operationTooFastToast` |
| 注册密码无效 | `registerInvalidPasswordToast` |
| Unicode 错误 | `"你输入的消息包含错误的 Unicode 字符，请调整后重试"` |

### 流式错误处理

| 错误类型 | 变量 | 说明 |
|---|---|---|
| 非流式错误 | `nonStreamError` / `_nonStreamError` | 非流式响应错误 |
| 流式转换失败 | `SSENetUnableConvertNonStreamError` | SSE 无法转换为非流式 |
| SSE 非流式错误 | `SSENetNonStreamError` | SSE 非流式错误 |
| 流中止 | `aborted` / `_userStopped` / `_cancelRequested` | 用户主动中止 |
| 中止完成失败 | `abortCompletionFailed` | 中止完成阶段失败 |
| 获取会话中止 | `abortFetchSessions` | 获取会话列表中止 |
| 流恢复中止 | `autoResumeAbort` | 自动恢复中止 |
| 系统错误 | `systemError` / `fatalError` | 系统级/致命错误 |
| 聊天禁用 | `chattingDisabled` | 聊天功能被禁用 |

### 文件拖拽错误

- `"拖拽文件file方法未成功"` — 拖拽文件方法失败
- `dragFileErrorTip` — 拖拽错误提示

---

## 19. EchoMind 借鉴建议

### 已有实现对比

| DeepSeek 功能 | EchoMind 现状 | 建议改进 |
|---|---|---|
| 思考链折叠面板 | ✅ `thinking-panel.js` | 增加 `fade-in-zoom-in-expand` 动画 + 思考耗时显示 + `AWAITING_FIRST_CHANGE` 状态 |
| 输入区 Toggle | ✅ `input-toggles.js` | 增加 Lottie 动画图标 + `autoCollapse` 窄屏自动折叠 |
| 消息操作（hover） | ✅ 基本实现 | 增加 `editMessage` 编辑用户消息 + 重新生成变体（简洁/详细/强制搜索） |
| Markdown 渲染 | ✅ marked.js + highlight.js | 增加彩虹括号 + 代码块 banner/footer + `ds-markdown-cite` 引用标注 |
| 会话分组 | ✅ 按时间分组 | 增加置顶会话区域 + 折叠/展开 + `pinSessionTooMany` 上限 |
| 主题切换 | ✅ dark/light/system | 已实现三选一 |
| 虚拟列表 | ❌ 未使用 | **建议引入** — 大量消息时使用虚拟列表 |
| 会话历史搜索 | ❌ 未实现 | **建议引入** — 侧栏搜索 + 跳转高亮 |
| 分享功能 | ❌ 未实现 | **建议引入** — 分享链接管理 + 选择消息 + 创建/复制/删除 |
| 消息编辑 | ❌ 未实现 | **建议引入** — 编辑用户消息并重新发送 |
| 流恢复 | ❌ 未实现 | **建议引入** — `autoResume` 断线自动恢复 |
| 最小加载延迟 | ❌ 未实现 | **建议引入** — `ensureMinLoadingDelay` 防闪烁 |
| 工具调用展示 | ❌ 未实现 | **建议引入** — `TOOL_OPEN/SEARCH/FIND` 工具调用结果展示 |
| 搜索结果卡片 | ❌ 未实现 | **建议引入** — `search-view-card` 搜索结果可折叠展示 |
| 文件上传限制 | ✅ 基本实现 | 增加 Token 计数超限提示 + 文件压缩（WebP）+ 拖拽 OCR 描述 |
| Mermaid 图片查看器 | ❌ 未实现 | **建议引入** — `ds-fancy-box` 图片缩放/下载/全屏 |
| 错误恢复组件 | ✅ `error-detail.js` | 增加独立 `ErrorRecovery` 组件 + 三层 Error Boundary |
| APM 监控 | ❌ 未实现 | 可选引入 — 性能监控 + 白屏检测 + 资源加载状态 |
| 组合输入（IME）| ✅ 基本支持 | 增加 `onCompositionStart/End` 精确判断 |
| 字符计数器 | ❌ 未实现 | **建议引入** — 超限时显示计数 + `interceptSubmit` 拦截 |
| 操作过快限制 | ❌ 未实现 | **建议引入** — `operationTooFastToast` 防频繁操作 |
| 消息状态机 | ✅ 简化版 | 增加 `AWAITING_FIRST_CHANGE` + `INCOMPLETE` + `CONTENT_FILTER` + `TIMEOUT` 状态 |

### 高优先级借鉴（P0）

1. **消息编辑功能** — 编辑用户消息 → 重新发送 → 保留/截断后续对话
2. **重新生成变体** — 简洁/详细/强制搜索/强制不搜索 4 种重新生成模式
3. **会话历史搜索** — 侧栏搜索 + 跳转高亮 + 虚拟列表
4. **思考链增强** — `AWAITING_FIRST_CHANGE` 状态 + 思考耗时显示 + `fade-in-zoom-in-expand` 动画
5. **流恢复机制** — `autoResume` 断线自动恢复 + `resumeStream` 手动恢复

### 中优先级借鉴（P1）

6. **分享功能** — 分享链接管理 + 消息选择 + 创建/复制/删除
7. **搜索结果卡片** — 可折叠的搜索结果展示 + 引用标注交互
8. **最小加载延迟** — `ensureMinLoadingDelay` 防闪烁
9. **虚拟列表** — 大量消息时的性能优化
10. **置顶会话** — 置顶区域 + 折叠/展开 + 上限提示
11. **工具调用展示** — `TOOL_OPEN/SEARCH/FIND` 结果展示
12. **字符计数器** — 超限拦截 + 粘贴超限提示
13. **Mermaid 图片查看器** — 缩放/下载/全屏
14. **代码块增强** — 彩虹括号 + banner/footer + 语言标签
15. **操作过快限制** — 防频繁操作 toast

### 设计系统借鉴

16. **CSS 变量三层架构** — Static → Alias → Component（EchoMind 已有类似架构，可参考 DeepSeek 的 `dsw-alias-*` 语义别名命名）
17. **按钮变体系统** — `ds-button` 的 15+ 变体（primary/error/warning/text/icon/circle/floating/block/outlined/filled/borderless/underlined + 5 种尺寸）
18. **过渡变量体系** — `--ds-transition-duration/fast/slow` + `--ds-ease-in/out/in-out` 标准化过渡
19. **Markdown CSS 变量** — `dsw-alias-markdown-*` 系列（citation/code-block/inline-code/placeholder/tag/code-segment-selected/unselected）
20. **特定组件覆盖层** — `dsw-specific-*` 用于特殊场景的 CSS 变量覆盖

### 交互细节借鉴

21. **`renderTrigger` 机制** — 51 次引用的渲染触发器，控制条件渲染
22. **`data-autofocus` 属性** — 自动聚焦标记（6 次使用）
23. **`disappearDelay` / `enterDelay`** — Toast/Tooltip 出现/消失延迟控制
24. **`autoToggle`** — 自动切换 toggle 状态（7 次引用）
25. **会话更新时滚动调整** — `adjustScrollOnSessionUpdated` — 新消息到达时智能调整滚动位置
26. **文件轮询** — `ensurePollingAllFilesInfo` — 持续轮询文件解析状态直到完成
27. **会话 ID 幂等创建** — 前端生成 UUID，后端幂等创建
28. **搜索状态版本控制** — `searchStateTriggerAppliedVersionStorageHandle` — 搜索功能按版本触发
29. **缓存控制** — `cacheResetAt` + `cache_control` — 会话级缓存控制
30. **并行流** — `allow_parallel_streams` — 支持并行流式响应

---

## 附录 A: DeepSeek JS 关键变量索引

| 变量名 | 出现次数 | 用途 |
|---|---|---|
| `renderTrigger` | 51 | 渲染触发器 |
| `scrollTop` | 48 | 滚动位置 |
| `modelType` | 121 | 模型类型 |
| `thinkingEnabled` | 76 | 思考开关 |
| `searchEnabled` | 57 | 搜索开关 |
| `regenerate` | 49 | 重新生成 |
| `MessageStatus` | 44 | 消息状态枚举 |
| `deleteSession` | 41 | 删除会话 |
| `FileParseStatus` | 37 | 文件解析状态 |
| `SessionHistoryStateEnum` | 31 | 会话历史状态 |
| `userIsBannedToast` | 29 | 用户封禁 toast |
| `editMessage` | 29 | 编辑消息 |
| `generateStateManager` | 27 | 生成状态管理器 |
| `conversationMode` | 27 | 对话模式 |
| `MessageRole` | 26 | 消息角色 |
| `search_request_id` | 23 | 搜索请求 ID |
| `UploadFileErrorCode` | 22 | 上传错误码 |
| `parent_search_request_id` | 21 | 父搜索请求 ID |
| `ensureMinLoadingDelay` | 19 | 最小加载延迟 |
| `commonNetworkErrorToast` | 19 | 通用网络错误 toast |
| `TOOL_OPEN` | 18 | 工具打开 |
| `sessionDeleted` | 17 | 会话已删除 |
| `TOOL_SEARCH` | 14 | 工具搜索 |
| `completion_like` | 12 | 完成类似 |
| `resumeStream` | 10 | 恢复流 |
| `TOOL_FIND` | 7 | 工具查找 |
| `DEEP_SEARCH` | 8 | 深度搜索 |
| `autoResume` | 14 | 自动恢复 |
| `pinSession` | 11 | 置顶会话 |
| `unpinSession` | 6 | 取消置顶 |
| `pinnedSessionsFolded` | 3 | 置顶折叠 |
| `deepSearchReference` | 8 | 深度搜索引用 |
| `linkReference` | 5 | 链接引用 |
| `searchGroup` | 4 | 搜索分组 |
| `composedSearchResults` | 2 | 组合搜索结果 |
| `adjustScrollOnSessionUpdated` | 4 | 会话更新调整滚动 |
| `virtualListRenderAnomaly` | 1 | 虚拟列表渲染异常 |
| `_userStopped` | 3 | 用户停止 |
| `_cancelRequested` | 3 | 取消请求 |
| `_loadingDelayPromise` | 4 | 加载延迟 Promise |
| `interceptSubmit` | 2 | 拦截提交 |
| `notifyPasteOverLimit` | 6 | 粘贴超限通知 |
| `charCounterVisible` | 1 | 字符计数器可见 |
| `autoToggle` | 7 | 自动切换 |
| `data-autofocus` | 6 | 自动聚焦 |
| `enterDelay` | 14 | 进入延迟 |
| `disappearDelay` | 3 | 消失延迟 |
| `search_state_on` | 12 | 搜索状态开启 |
| `conversation_search_enabled` | 3 | 会话搜索启用 |
| `conversation_search_debounce_ms` | 1 | 会话搜索防抖 |
| `search-force-on` | 2 | 强制搜索开 |
| `search-force-off` | 2 | 强制搜索关 |
| `x-thinking-enabled` | 1 | 思考启用标记 |
| `x-debug-search-extras` | 2 | 调试搜索扩展 |
| `x-debug-search-channel` | 2 | 调试搜索频道 |

---

## 附录 B: DeepSeek CSS 设计令牌完整索引

### 颜色令牌（Static 层）

| 色系 | 色阶 |
|---|---|
| DeepSeek 品牌色 | 50, 100, 200, 300, 400, 450, 500, 600, 800, 900 |
| 中性色（偏蓝） | 00, 50, 60, 75, 100, 150, 200, 300, 400, 500, 600, 700, 750, 800, 850, 875, 900, 950, 1000 |
| 中性色 | 00, 50, 100, 150, 200, 250, 300, 400, 500, 550, 600, 700, 800, 850, 900, 1000 |
| 红色 | 50, 100, 400, 500, 600, 900 |
| 蓝色 | 50, 75, 100, 300, 400, 450, 500, 600, 800, 950 |
| 绿色 | 100, 400, 500, 900 |
| 琥珀色 | 100, 400, 500, 600, 900 |

### 组件尺寸

| 尺寸 | 变量 |
|---|---|
| 控件高度 | `ds-control-height-xs/s/m/l/xl` |
| 字体大小 | `dsw-font-xs-13/s-14/m-14/l-14/sp/base-16` |
| 行高 | `dsw-line-height-xs/xsp/s/sp/m/l` |

---

## 附录 C: DeepSeek 追踪事件索引

| 事件类别 | 事件名 |
|---|---|
| 搜索 | `search_result_show/click`, `search_no_result_show`, `search_fail_show`, `search_citation_show/click`, `search_list_icon_show/expand`, `search_link_show/click`, `search_panel_click`, `search_force_on/off` |
| 文件上传 | `file_upload`, `upload_file`, `upload_failed`, `upload_panel`, `upload_action_click` |
| 用户交互 | `user_click`, `file_picker`, `message_banner`, `fetch-image` |
| 会话 | `sessionDeleted`, `sessionChanged`, `update_session`, `update_parent_message`, `update_file` |
| 分享 | `shareEntryClick`, `clickCreateShareLink`, `clickConfirmCreateAndCopy`, `clickCancelShareSelect` |
| 消息 | `clickEditMessageButton`, `completion_like`, `user_cancelled_authorize` |
| 系统状态 | `systemThemeChange`, `systemError`, `rateLimit`, `ipAccessRestrictedToast`, `accountBanned` |
| 历史搜索 | `historySearchTriggerButton`, `historySearchMD`, `historySearchYMD` |
| Mermaid | `mermaidDownloadCode`, `mermaidDownloadPng` |
| 思考 | `thinkingSwitchToggled`, `thinkingEnabled`, `isThinking` |
| 搜索开关 | `searchEnabledToggle`, `searchEnabled` |
| 会话搜索 | `shortcutOpenConversationSearch`, `toggleSearchList`, `gotoSearchResult` |
| 折叠 | `toggleCollapsibleArea` |
| 模型 | `model_configs`, `masksProperties` |
| 设置 | `closeSettingModal`, `closeDeleteSessionModal`, `closeDeleteAllHistoryModal` |
| 移动端 | `mobile-guest`, `mobile_sms` |
| 限流 | `rateLimit`, `RPD_RATE_LIMIT`, `CONCURRENCY_RATE_LIMIT` |
| 调试 | `x-debug-search-extras`, `x-debug-search-channel`, `_debugPanelEnabled`, `__appKit_debugPanelPosition/Fold` |
| 虚拟列表 | `virtualListRenderAnomaly` |
| 窗口 | `windowResize`, `resize` |
| 网络 | `window.navigator.onLine` |
| 加载 | `loadEarlierOnly`, `listScroll`, `scroll` |

---

*报告结束。如需针对某个具体功能进行更深入的分析或开始实施借鉴，请告知。*