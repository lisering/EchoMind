# EchoMind 交互设计规格 (Interaction Design Specification)

> **版本**: v1.0.0 | **日期**: 2026-08-22
>
> 本文档定义 EchoMind 前端所有交互模式、状态机、过渡行为、无障碍规范。
> 是交互测试的唯一依据（Spec-driven Development 铁律十五）。
>
> 参考来源：DeepSeek chat UI + WCAG 2.1 + WAI-ARIA Authoring Practices 1.2。

---

## §1 交互状态机规格

### §1.1 按钮状态机 (Button State Machine)

每个交互按钮必须实现以下 5 种视觉状态：

| 状态 | CSS 伪类 | 视觉表现 | 像素值 |
|---|---|---|---|
| Default | `:default` | 正常态 | `background: transparent; color: var(--text-tertiary)` |
| Hover | `:hover` | 悬停态 | `background: var(--surface-2); color: var(--text-primary)` |
| Active | `:active` | 按下态 | `transform: scale(0.95)` |
| Focus | `:focus-visible` | 聚焦态 | `box-shadow: var(--shadow-focus)` |
| Disabled | `:disabled` | 禁用态 | `opacity: 0.45; cursor: not-allowed` |

**状态转换规则：**
- Default → Hover（鼠标进入元素区域）
- Hover → Active（鼠标按下）
- Active → Hover（鼠标释放）
- 任何状态 → Focus（键盘 Tab 导航到元素）
- 任何状态 → Disabled（JS 设置 `disabled` 属性）
- Hover/Focus → Default（鼠标离开 / Tab 导航走）

### §1.2 输入框状态机 (#queryInput / #inputBar)

| 状态 | 触发条件 | 视觉表现 |
|---|---|---|
| Empty | 输入框无内容 | placeholder 可见，发送按钮禁用 |
| Typing | 用户正在输入 | 发送按钮启用，IME 组合中不发送 |
| Focused | 输入框获得焦点 | #inputBar border-color 变为 `rgba(accent, 0.6)` |
| Blurred | 输入框失去焦点 | #inputBar border-color 恢复默认 |
| Disabled | 知识库为空 | 输入框 disabled，placeholder 提示先导入文档 |
| Sending | 消息发送中 | 发送按钮变为停止按钮 (#stopBtn)，输入框禁用 |

### §1.3 模态框/面板状态机

| 状态 | 触发条件 | 视觉表现 | 动画 |
|---|---|---|---|
| Closed | 初始/关闭后 | `display: none` 或 `opacity: 0; visibility: hidden` | 无 |
| Opening | 打开动作触发 | 从 `scale(0.96); opacity:0` 过渡到 `scale(1); opacity:1` | `scaleIn 250ms ease-out` |
| Open | 完全显示 | `display: flex/block; opacity: 1; visibility: visible` | 无 |
| Closing | 关闭动作触发 | 从 `opacity:1` 过渡到 `opacity:0` | `fadeOut 200ms` |
| Panel Stack | 同时只能显示 1 个 overlay | 新面板 push 到栈，ESC 关闭栈顶 | `panelIn 150ms ease-out` |

### §1.4 流式对话状态机

| 状态 | 触发条件 | 前端事件 | 视觉表现 |
|---|---|---|---|
| Idle | 空闲 | — | 输入框可交互，发送按钮可见 |
| Preparing | 用户发送消息 | `chat_phase: "preparing"` | 思维链面板展开，显示"准备中..." |
| Retrieving | 开始检索 | `chat_phase: "retrieving"` | 思维链面板显示"检索知识库..." |
| Generating | LLM 开始流式输出 | `chat_phase: "generating"` | 打字机效果，光标闪烁 |
| Streaming | 持续接收 token | `chat_token` 事件 | 逐字追加，自动滚动到底部 |
| Done | 流式完成 | `chat_done` 事件 | 光标消失，操作栏显示 |
| Aborted | 用户中断 | `abort_chat` IPC | 显示"⏹ 生成已中断"，部分内容保留 |
| Error | LLM 返回错误 | `chat_error` 事件 | 错误提示显示，输入框恢复可用 |

### §1.5 侧栏折叠状态机

| 状态 | 触发条件 | 视觉表现 | 动画 |
|---|---|---|---|
| Expanded | 默认/点击展开按钮 | `#sidebar` visible, `main { padding-left: 240px }` | 无 |
| Collapsing | 点击折叠按钮 | `#sidebar` transform 滑出, `main` padding-left 渐变 | `transform 300ms cubic-bezier(0.4,0,0.2,1)` |
| Collapsed | 折叠完成 | `#sidebar.sidebar-collapsed { transform: translateX(-100%) }`, `main { padding-left: 0 }` | 无 |
| Expanding | 点击展开按钮 | `#sidebar` transform 滑入, `main` padding-left 渐变 | `transform 300ms cubic-bezier(0.4,0,0.2,1)` |

### §1.6 拖拽状态机

| 状态 | 触发条件 | 视觉表现 |
|---|---|---|
| Idle | 无拖拽 | `#dragOverlay` hidden |
| DragEnter | 文件拖入窗口 | `#dragOverlay` visible, 显示虚线框 |
| DragOver | 文件在窗口上方 | 保持 visible |
| DragLeave | 文件离开窗口 | `#dragOverlay` hidden |
| Drop | 文件释放 | `#dragOverlay` hidden, 触发 import_files |

---

## §2 键盘交互规格

### §2.1 全局快捷键

| 快捷键 | 动作 | 状态条件 |
|---|---|---|
| `⌘J` / `Ctrl+J` | 新建对话 | 主界面可见 |
| `⌘K` / `Ctrl+K` | 打开命令面板 | 主界面可见 |
| `⌘,` / `Ctrl+,` | 打开设置 | 主界面可见 |
| `⌘E` / `Ctrl+E` | 导出对话 | 有活跃对话 |
| `⌘⇧F` / `Ctrl+Shift+F` | 全局搜索 | 主界面可见 |
| `Escape` | 关闭栈顶面板 | 有打开的面板 |

### §2.2 输入框键盘交互

| 按键 | 动作 | 条件 |
|---|---|---|
| `Enter` | 发送消息 | 非 IME 组合状态，输入框非空 |
| `Shift+Enter` | 插入换行 | 始终 |
| `↑` (空输入框) | 加载上一条历史 | 输入框为空 |
| `↓` (历史导航中) | 加载下一条历史 | 历史导航激活 |
| `Escape` (历史导航中) | 退出历史导航 | 历史导航激活 |
| `/` | 打开斜杠命令面板 | 输入框为空 |
| `Tab` (斜杠面板) | 选择下一项 | 斜杠面板打开 |
| `Home` (斜杠面板) | 跳到第一项 | 斜杠面板打开 |
| `End` (斜杠面板) | 跳到最后一项 | 斜杠面板打开 |

### §2.3 Tab 导航顺序

主界面 Tab 导航顺序（focusable 元素）：
1. `#newChatBtn` (新建对话按钮)
2. `#kbBtn` (知识库按钮)
3. 侧栏文档列表项
4. 侧栏会话列表项
5. `#collapseBtn` (折叠按钮)
6. `#queryInput` (输入框)
7. `#sendBtn` (发送按钮)

模态框打开时 Focus Trap：
- 打开模态框 → 焦点移到模态框内第一个可聚焦元素
- Tab → 在模态框内循环
- Shift+Tab → 反向循环
- Escape → 关闭模态框，焦点返回触发元素

### §2.4 文档列表键盘交互

| 按键 | 动作 | 条件 |
|---|---|---|
| `↑` / `↓` | 上下导航文档项 | 文档列表聚焦 |
| `Enter` | 选中/打开文档 | 文档项聚焦 |
| `Delete` | 删除文档（带确认） | 文档项聚焦 |
| `Escape` | 取消选择 | 有选中项 |

---

## §3 滚动交互规格

### §3.1 聊天区滚动

| 行为 | 条件 | 动作 |
|---|---|---|
| 自动滚动到底部 | 新消息到达 | `scrollTop = scrollHeight` |
| 滚动到底部按钮显示 | 用户手动上滚 | `#scrollBottomBtn` opacity 0 → 1 |
| 滚动到底部按钮隐藏 | 已在底部 | `#scrollBottomBtn` opacity 1 → 0 |
| 点击滚动到底部 | 用户点击按钮 | 平滑滚动到底部 |
| 流式输出时自动滚动 | 用户在底部 | 持续跟随滚动 |
| 流式输出时不自动滚动 | 用户手动上滚 | 不强制滚动 |

### §3.2 侧栏滚动

| 行为 | 条件 | 动作 |
|---|---|---|
| 会话列表滚动 | 会话数量超出可见区域 | 独立滚动条 |
| 文档列表滚动 | 文档数量超出可见区域 | 独立滚动条 |
| Sticky 日期头 | 会话按日期分组 | `position: sticky; top: 0` |

### §3.3 滚动条样式

| 属性 | 值 |
|---|---|
| width | 6px |
| thumb border-radius | 3px |
| thumb background (暗色) | `rgba(255, 255, 255, 0.12)` |
| thumb background (浅色) | `rgba(0, 0, 0, 0.15)` |

---

## §4 反馈交互规格

### §4.1 悬停反馈

| 元素 | Hover 效果 | 过渡时间 |
|---|---|---|
| 文档列表项 | `background: var(--hover-bg-subtle)` | 150ms |
| 会话列表项 | `background: var(--hover-bg-subtle)` | 150ms |
| 操作栏按钮 | `background: var(--surface-2); color: var(--text-primary)` | 200ms (bg) / 300ms (color) |
| 引用芯片 | `background: rgba(accent, 0.18)` | 300ms |
| 发送按钮 | `opacity: 0.9` | 150ms |
| 链接 (.md a) | `border-bottom-color: var(--accent)` | 150ms |
| 代码复制按钮 | `color: var(--accent); background: var(--accent-subtle)` | 150ms |

### §4.2 加载反馈

| 场景 | 反馈 | 元素 |
|---|---|---|
| 导入文档中 | 进度条 + 文档状态 | `#importProgress` + doc-status-changed |
| 嵌入模型下载 | 进度条 | embedding_progress 事件 |
| LLM 流式响应 | 打字机效果 + 光标闪烁 | chat_token 事件 + .animate-caret-blink |
| 向量检索中 | 思维链面板"检索中..." | chat_phase: "retrieving" |
| 骨架屏 | `animate-pulse` 占位 | `.skeleton` 元素 |

### §4.3 Toast 通知反馈

| 类型 | 背景色 | 图标 | 持续时间 |
|---|---|---|---|
| success | `rgba(var(--success-rgb), 0.15)` | ✓ | 3000ms |
| error | `rgba(var(--danger-rgb), 0.15)` | ✗ | 5000ms |
| warning | `rgba(var(--warning-rgb), 0.15)` | ⚠ | 4000ms |
| info | `rgba(var(--info-rgb), 0.15)` | ℹ | 3000ms |

Toast 动画：`toastIn 250ms ease-out` (滑入) + `fadeOut 200ms` (滑出)

### §4.4 错误反馈

| 错误类型 | 用户可见信息 | 恢复建议 |
|---|---|---|
| API Key 无效 | "API 密钥无效或已过期，请在设置中检查" | 打开设置 |
| 网络超时 | "请求超时，请检查网络连接" | 重试 |
| 知识库为空 | "请先导入文档到知识库" | 打开知识库 |
| Pro 功能限制 | "此功能需要 Pro 版本" | 激活 Pro |
| 对话过长 | "对话内容过长，请新建会话或压缩历史记录" | 新建会话 |
| 嵌入超时 | "向量嵌入超时，请重试" | 重试 |
| 模型加载失败 | "本地 AI 模型加载失败" | 检查模型文件 |

---

## §5 无障碍 (A11y) 交互规格

### §5.1 ARIA 属性

| 元素 | aria 属性 | 值 |
|---|---|---|
| 按钮 (无文字) | `aria-label` | 描述性文字 |
| 模态框 | `role="dialog"` `aria-modal="true"` | — |
| 模态框 | `aria-labelledby` | 指向标题元素 |
| Toast 容器 | `aria-live="polite"` | 自动播报 |
| 错误消息 | `aria-live="assertive"` | 立即播报 |
| 加载状态 | `aria-busy="true"` | 区域加载中 |
| 展开折叠 | `aria-expanded` | true/false |
| 选中状态 | `aria-selected` | true/false |
| 隐藏内容 | `aria-hidden="true"` | 屏幕阅读器跳过 |
| 视觉隐藏 | `.sr-only` class | 仅屏幕阅读器可读 |

### §5.2 Focus 管理

- 所有交互元素必须可通过键盘聚焦（`tabindex ≥ 0`）
- `:focus-visible` 必须有可见的 focus 环（`box-shadow: var(--shadow-focus)`）
- 模态框打开时焦点陷阱（Focus Trap）：Tab 键在模态框内循环
- 模态框关闭后焦点返回触发元素
- `prefers-reduced-motion: reduce` 时所有动画降级为 0.01ms

### §5.3 颜色对比度

- 正文文字对背景对比度 ≥ 4.5:1 (WCAG AA)
- 大文字 (≥ 18px) 对比度 ≥ 3.0:1
- 交互元素边界对比度 ≥ 3.0:1
- 高对比度模式：所有文字对比度 ≥ 7.0:1 (WCAG AAA)

### §5.4 屏幕阅读器

- `.sr-only` 类用于视觉隐藏但屏幕阅读器可读的文本
- `aria-live` 区域用于动态内容播报
- 按钮有 `aria-label`（当无文字内容时）
- 图标按钮有 `title` 属性

---

## §6 交互流程规格

### §6.1 首启向导流程

```
Step 1 (选择预设)
  → 点击预设卡片 或 点击"自定义"
  → 自动填充 API Key / Base URL / Model

Step 2 (配置)
  → 输入 API Key (#wizKey)
  → 输入 Base URL (#wizUrl) — 可选，有默认值
  → 输入/选择 Model (#wizModel)
  → 点击"验证并开始" (#wizStart)
  → 验证 API Key → 成功 → 进入主界面
  → 验证失败 → 显示错误提示，留在 Step 2
```

### §6.2 文档导入流程

```
点击知识库按钮 (#kbBtn)
  → 打开知识库模态框
  → 点击"导入文档" 或 拖拽文件
  → 文件选择器 / 拖拽释放
  → import_files IPC 调用
  → 进度条显示 (#importProgress)
  → doc-status-changed 事件
  → 导入完成 → 文档列表更新
  → 嵌入进度事件 (embedding_progress)
  → 嵌入完成 → 文档可检索
```

### §6.3 RAG 对话流程

```
输入框聚焦 → 输入问题
  → Enter 发送 / 点击发送按钮
  → chat IPC 调用
  → chat_phase: "preparing" → 思维链面板展开
  → chat_phase: "retrieving" → 显示检索状态
  → chat_sources 事件 → 引用来源显示
  → chat_phase: "generating" → 开始生成
  → chat_token 事件（循环）→ 打字机效果
  → 用户可点击停止按钮中断
  → chat_done 事件 → 生成完成
  → 操作栏显示（复制/重新生成/引用）
  → 消息持久化到 SQLite
```

### §6.4 设置面板交互

```
⌘, 打开设置
  → 设置模态框打开（animate-modal-in）
  → 8 个分区 Tab 可点击切换
  → Tab 滚动定位（scroll-padding-top: 52px）
  → 修改设置 → 实时保存
  → ESC / 点击外部 → 关闭设置
```

### §6.5 确认对话框交互

```
触发删除操作
  → showConfirmDialog(title, message, onConfirm)
  → 确认对话框显示（role="alertdialog"）
  → Focus Trap 激活
  → 焦点在"确认"按钮
  → 点击确认 → 执行回调 + 关闭
  → 点击取消 / ESC → 关闭，不执行
  → 焦点返回触发元素
```

---

## §7 测试用例编号规则

### §7.1 交互测试用例前缀

| 前缀 | 类型 | 文件 |
|---|---|---|
| TC-INT-STATE | 状态机验证 | `interaction-state.spec.ts` |
| TC-INT-KB | 键盘交互验证 | `interaction-keyboard.spec.ts` |
| TC-INT-SCROLL | 滚动交互验证 | `interaction-scroll.spec.ts` |
| TC-INT-FB | 反馈交互验证 | `interaction-feedback-comprehensive.spec.ts` |
| TC-INT-A11Y | 无障碍验证 | `interaction-a11y.spec.ts` |
| TC-INT-FLOW | 交互流程验证 | `interaction-flow.spec.ts` |
| TC-UNIT-INT | Vitest 单元交互验证 | `interaction-spec.test.js` |
