# EchoMind UI 像素级设计规格 (Pixel-Level Design Specification)

> **版本**: v1.0.0 | **日期**: 2026-08-22
>
> 本文档定义 EchoMind 前端所有 UI 元素的精确像素值、颜色值、间距值、字号值、动效参数。
> 是像素级测试的唯一依据（Spec-driven Development 铁律十五）。
>
> 所有数值来源于 `ui/styles/tokens.css` + `ui/styles/components.css` + `ui/styles/typography.css` 的实测值，
> 并对照 DeepSeek (chat.deepseek.com) 实测样式校准。

---

## §1 设计令牌 (Design Tokens) 精确值

### §1.1 颜色令牌 — 暗色主题 (默认)

| 令牌名 | CSS 变量 | 精确值 | RGB | 用途 |
|---|---|---|---|---|
| Surface 0 | `--surface-0` | `#0A0A0B` | rgb(10,10,11) | 主背景 |
| Surface 1 | `--surface-1` | `#131316` | rgb(19,19,22) | 侧栏/模态框背景 |
| Surface 2 | `--surface-2` | `#1C1C20` | rgb(28,28,32) | 卡片背景 |
| Surface 3 | `--surface-3` | `#26262C` | rgb(38,38,44) | 悬停态背景 |
| Surface 4 | `--surface-4` | `#303036` | rgb(48,48,54) | 选中态背景 |
| Border Subtle | `--border-subtle` | `#1F1F23` | rgb(31,31,35) | 最浅边框 |
| Border Default | `--border-default` | `#2A2A2E` | rgb(42,42,46) | 默认边框 |
| Border Strong | `--border-strong` | `#3A3A40` | rgb(58,58,64) | 强边框 |
| Text Primary | `--text-primary` | `#F8FAFC` | rgb(248,250,252) | 标题/重点文字 |
| Text Secondary | `--text-secondary` | `#CBD5E1` | rgb(203,213,225) | 正文文字 |
| Text Tertiary | `--text-tertiary` | `#94A3B8` | rgb(148,163,184) | 辅助文字 |
| Text Quaternary | `--text-quaternary` | `#8995A8` | rgb(137,149,168) | 最浅文字 |
| Accent | `--accent` | `#38BDF8` | rgb(56,189,248) | 品牌色/链接色 |
| Accent Hover | `--accent-hover` | `#0EA5E9` | rgb(14,165,233) | 悬停态品牌色 |
| Success | `--success` | `#4ADE80` | rgb(74,222,128) | 成功状态 |
| Warning | `--warning` | `#FBBF24` | rgb(251,191,36) | 警告状态 |
| Danger | `--danger` | `#F87171` | rgb(248,113,113) | 危险状态 |
| Info | `--info` | `#60A5FA` | rgb(96,165,250) | 信息状态 |

### §1.2 颜色令牌 — 浅色主题

| 令牌名 | 精确值 | 与暗色对比度变化 |
|---|---|---|
| Surface 0 | `#FFFFFF` | 白色主背景 |
| Surface 1 | `#F8FAFC` | slate-50 |
| Surface 2 | `#F1F5F9` | slate-100 |
| Surface 3 | `#E2E8F0` | slate-200 |
| Text Primary | `#0F172A` | slate-900 |
| Text Secondary | `#334155` | slate-700 |
| Text Tertiary | `#475569` | slate-600 |
| Accent | `#0EA5E9` | sky-500 |
| Accent Text | `#0369A1` | sky-700（文本专用） |
| Success | `#15803D` | green-700 |

### §1.3 间距令牌 (4px 网格)

| 令牌名 | 值 | 用途 |
|---|---|---|
| `--space-0` | 0px | 无间距 |
| `--space-1` | 4px | 微间距（图标内间距、操作栏 gap） |
| `--space-2` | 8px | 小间距（按钮内边距、列表项 gap） |
| `--space-3` | 12px | 中间距（卡片内边距、消息块小内边距） |
| `--space-4` | 16px | 标准间距（消息块内边距、模态框内边距） |
| `--space-5` | 20px | 大间距（侧栏内边距、输入栏内边距） |
| `--space-6` | 24px | 超大间距（分区标题间距） |
| `--space-8` | 32px | 问答间距（用户消息 ↔ AI 消息） |
| `--space-10` | 40px | 分区间距 |
| `--space-12` | 48px | 页面级间距 |

### §1.4 排版令牌

| 令牌名 | 值 | 用途 |
|---|---|---|
| `--text-xs` | 11px | 最小字号（代码语言标签、时间戳、徽标） |
| `--text-sm` | 12px | 小字号（侧栏列表项、辅助文本、source-card） |
| `--text-base` | 16px | 基准字号（body、输入框） |
| `--text-lg` | 18px | 大字号（标题、强调） |
| `--leading-tight` | 1.4 | 紧凑行高（标题、按钮） |
| `--leading-normal` | 1.75 | 标准行高（正文 .md） |
| `.md` font-size | 14px | AI 回复正文字号 |
| `.md` line-height | 1.8 | AI 回复正文行高 |
| `.md code` font-size | 13px (code-block) / 0.88em (inline) | 代码块字号 |

### §1.5 圆角令牌

| 令牌名 | 值 | 用途 |
|---|---|---|
| `--radius-none` | 0px | 无圆角 |
| `--radius-sm` | 4px | copy-btn / code-lang / 文档徽标 |
| `--radius-md` | 8px | 代码块 pre / .md details / toggle |
| `--radius-lg` | 12px | 按钮 / 消息块 / Toast / 卡片 |
| `--radius-xl` | 16px | assistant block / user block / #inputBar |
| `--radius-2xl` | 24px | 拖拽遮罩虚线框 |
| `--radius-full` | 9999px | toggle switch / 引用芯片 / 进度条 |
| `--msg-user-radius` | 22px | 用户消息气泡圆角（DeepSeek 风格） |
| `--dsl-button-radius` | 4096px | 胶囊形按钮 |

### §1.6 阴影令牌

| 令牌名 | 值 | 用途 |
|---|---|---|
| `--shadow-sm` | `0 4px 12px rgba(0,0,0,0.15)` | Toast |
| `--shadow-md` | `0 8px 24px rgba(0,0,0,0.35)` | 右键菜单 / 浮动按钮 |
| `--shadow-lg` | `0 16px 48px rgba(0,0,0,0.4)` | 大型模态框 |
| `--shadow-focus` | `0 0 0 2px var(--surface-0), 0 0 0 4px var(--accent)` | Focus 环 |

### §1.7 动效令牌

| 令牌名 | 值 | 用途 |
|---|---|---|
| `--duration-micro` | 100ms | 微交互（按钮按下、选中色变化） |
| `--duration-fast` | 150ms | 快速过渡（hover、focus、copy-btn） |
| `--duration-normal` | 250ms | 常规过渡（fade-in、modal 打开） |
| `--duration-slow` | 400ms | 慢速过渡（侧栏折叠、大型面板展开） |
| `--ease-out` | ease-out | 默认缓出 |
| `--ease-in-out` | `cubic-bezier(0.4, 0, 0.2, 1)` | Material Design 标准缓动 |
| `--ease-spring` | `cubic-bezier(0.34, 1.56, 0.64, 1)` | 弹簧缓动 |
| 侧栏折叠 transition | `transform 300ms cubic-bezier(0.4, 0, 0.2, 1)` | 侧栏滑动 |
| main padding transition | `padding-left 300ms cubic-bezier(0.4, 0, 0.2, 1)` | 主区域宽度变化 |

### §1.8 Z-index 层级

| 层级 | 值 | 用途 |
|---|---|---|
| Base | 0 | 正常文档流 |
| TopBar | 0 (position:relative) | 顶栏 |
| Sidebar | 20 | 侧栏（position:fixed） |
| Sticky Header | 10 | 会话时间分组 sticky |
| Overlay | 50 | 遮罩层 |
| Modal | 60 | 模态框 |
| Toast | 70 | Toast 通知 |
| Context Menu | 80 | 右键菜单 |
| Tooltip | 90 | 工具提示 |
| Drag Overlay | 100 | 拖拽遮罩 |

---

## §2 组件像素规格

### §2.1 侧栏 (#sidebar)

| 属性 | 精确值 | 验证方法 |
|---|---|---|
| width | 240px | `offsetWidth === 240` |
| position | fixed | `getComputedStyle.position` |
| top | 28px (= #topBar height) | `getComputedStyle.top` |
| bottom | 0 | `getComputedStyle.bottom` |
| left | 0 | `getComputedStyle.left` |
| background | `var(--surface-1)` = `#131316` | `getComputedStyle.backgroundColor` |
| border-right | `1px solid var(--border-subtle)` | `getComputedStyle.borderRight` |
| transform (折叠) | `translateX(-100%)` | `getComputedStyle.transform` |
| transition | `transform 300ms cubic-bezier(0.4, 0, 0.2, 1)` | `getComputedStyle.transition` |
| will-change | transform | `getComputedStyle.willChange` |
| z-index | 20 | `getComputedStyle.zIndex` |

### §2.2 顶栏 (#topBar)

| 属性 | 精确值 |
|---|---|
| height | 28px (h-7) |
| padding-left | 78px (macOS traffic lights 空间) |
| padding-left (全屏) | 8px |
| position | relative |
| z-index | 0 |

### §2.3 消息列表 (#chatArea)

| 属性 | 精确值 |
|---|---|
| max-width | 840px (`--msg-list-max-width`) |
| 居中方式 | margin: 0 auto 或 flex justify-center |
| padding-left | ≥ 16px |
| padding-right | ≥ 16px |

### §2.4 用户消息 (.msg-user)

| 属性 | 精确值 |
|---|---|
| background | `rgba(56, 189, 248, 0.08)` (暗色) / `rgba(14, 165, 233, 0.08)` (浅色) |
| border | `1px solid rgba(56, 189, 248, 0.12)` (暗色) |
| border-radius | 22px |
| max-width | `calc(100% - 88px)` |
| 对齐方式 | 右对齐 (justify-end / margin-left:auto) |
| padding | 16px (`--msg-padding`) |

### §2.5 AI 消息 (.msg-assistant)

| 属性 | 精确值 |
|---|---|
| background | transparent |
| border | transparent |
| border-radius | 0px (无圆角) |
| 对齐方式 | 左对齐 (justify-start) |
| padding | 16px (`--msg-padding`) |

### §2.6 输入栏 (#inputBar)

| 属性 | 精确值 |
|---|---|
| border-radius | 16px (`--radius-xl` / `--dsl-input-radius`) |
| background | `var(--surface-2)` = `#1C1C20` (暗色) |
| border | `1px solid var(--border-default)` |
| border (focus) | `1px solid rgba(var(--accent-rgb), 0.6)` |
| min-height (textarea) | ≥ 40px |
| font-size | ≥ 14px |

### §2.7 操作按钮 (.msg-action-btn)

| 属性 | 精确值 |
|---|---|
| width | 28px |
| height | 28px |
| border-radius | 9999px (full) |
| background | transparent |
| color | `var(--text-tertiary)` = `#94A3B8` |
| background (hover) | `var(--surface-2)` = `#1C1C20` |
| color (hover) | `var(--text-primary)` = `#F8FAFC` |
| transform (active) | scale(0.95) |
| transition | `background-color 0.2s, color 0.3s` |

### §2.8 发送按钮 (#sendBtn) / 加号按钮 (#plusBtn)

| 属性 | 精确值 |
|---|---|
| height | sendBtn.height === plusBtn.height |
| 位置关系 | plusBtn.x < sendBtn.x (plus 在左, send 在右) |
| 底部对齐 | `|sendBtn.bottom - plusBtn.bottom| ≤ 2px` |

### §2.9 Markdown 正文 (.md)

| 属性 | 精确值 |
|---|---|
| font-size | 14px |
| line-height | 1.8 |
| color | `var(--text-secondary)` = `#CBD5E1` |
| p margin | 10px 0 |
| h1 font-size | 1.45em (= 20.3px) |
| h2 font-size | 1.25em (= 17.5px) |
| h3 font-size | 1.12em (= 15.68px) |
| code (inline) font-size | 0.88em (= 12.32px) |
| code (block) font-size | 13px |
| code (block) line-height | 1.65 |
| code (block) padding | 14px |
| ul/ol padding-left | 24px |
| blockquote border-left | 3px solid var(--accent) |
| blockquote border-radius | 0 8px 8px 0 |

### §2.10 代码块 (.code-block)

| 属性 | 精确值 |
|---|---|
| border-radius | 8px |
| border | 1px solid var(--border-subtle) |
| background | var(--surface-0) |
| margin | 12px 0 |
| .code-header padding | 6px 14px |
| .code-lang font-size | 11px |
| .code-lang text-transform | uppercase |
| .code-lang letter-spacing | 0.5px |
| pre code padding | 14px |
| pre code font-size | 13px |
| pre code line-height | 1.65 |

---

## §3 布局像素规格

### §3.1 视口约束

| 属性 | 值 | 来源 |
|---|---|---|
| minWidth | 960px | tauri.conf.json |
| minHeight | 640px | tauri.conf.json |
| 默认测试视口 | 1280×800 | E2E 标准 |

### §3.2 元素在视口内断言

每个交互元素必须满足：
- `x ≥ 0` (左边界不溢出)
- `y ≥ 0` (上边界不溢出)
- `right ≤ viewportWidth` (右边界不溢出)
- `bottom ≤ viewportHeight` (下边界不溢出)
- `width > 0` (有宽度)
- `height > 0` (有高度)

### §3.3 侧栏与主区域不重叠

- `sidebar.right ≤ chatArea.x`
- `sidebar.right ≤ inputBar.x`
- `sidebar.height ≥ viewportHeight * 0.9`

### §3.4 输入栏位置

- 输入栏底部距视口底边 `≥ 4px`
- 输入栏在容器内左右间距对称：`|leftMargin - rightMargin| ≤ 2px`
- 滚动后输入栏仍在视口内
- 输入栏顶部 ≥ 聊天区底部 - 1（不重叠）

### §3.5 响应式降级

| 视口宽度 | 期望行为 |
|---|---|
| ≥ 960px | 正常布局 |
| 960px → 400px | 侧栏不变（fixed 240px），主区域被挤压 |
| ≤ 400px | 布局不崩溃，输入框可见 |

---

## §4 主题一致性规格

### §4.1 暗色主题一致性

- 全页无纯白背景 (`rgb(255,255,255)`) 元素
- 例外：toggle switch 旋钮（`bg-white rounded-full`）
- `body` color 不为 `rgb(0,0,0)`

### §4.2 浅色主题一致性

- 全页无纯黑背景 (`rgb(0,0,0)`) 元素（除高对比度模式）
- 文字颜色不为 `rgb(255,255,255)` 纯白

### §4.3 主题切换防闪烁

- `body.change-theme` 类存在时，所有过渡禁用
- CSS 规则 `body.change-theme * { transition: none !important; animation: none !important; }` 存在

### §4.4 CSS 变量三层架构存在性

- Layer 1 (Surface): `--surface-0` ~ `--surface-4` 均非空
- Layer 2 (Alias): `--dsw-alias-surface-0` / `--dsw-alias-brand-primary` 非空
- Layer 3 (Component): `--dsl-button-radius` / `--dsl-msg-user-radius` 非空

---

## §5 WCAG 对比度规格

### §5.1 暗色主题对比度 (背景 #0A0A0B / #131316)

| 文字层级 | 颜色 | 对背景对比度 | WCAG 级别 |
|---|---|---|---|
| Text Primary | #F8FAFC on #0A0A0B | 19.3:1 | AAA |
| Text Secondary | #CBD5E1 on #0A0A0B | 13.5:1 | AAA |
| Text Tertiary | #94A3B8 on #0A0A0B | 7.8:1 | AAA |
| Text Quaternary | #8995A8 on #0A0A0B | 6.2:1 | AA |
| Accent | #38BDF8 on #0A0A0B | 8.9:1 | AAA |

### §5.2 浅色主题对比度 (背景 #FFFFFF)

| 文字层级 | 颜色 | 对背景对比度 | WCAG 级别 |
|---|---|---|---|
| Text Primary | #0F172A on #FFFFFF | 18.7:1 | AAA |
| Text Secondary | #334155 on #FFFFFF | 12.6:1 | AAA |
| Text Tertiary | #475569 on #FFFFFF | 7.6:1 | AAA |
| Text Quaternary | #475569 on #E2E8F0 | 6.2:1 | AA |
| Accent Text | #0369A1 on #FFFFFF | 6.4:1 | AA |

---

## §6 动画/过渡规格

### §6.1 关键 @keyframes 定义

| 名称 | from → to | 用途 |
|---|---|---|
| fadeIn | `opacity:0; translateY(6px)` → `opacity:1; none` | 元素淡入 |
| scaleIn | `opacity:0; scale(0.96)` → `opacity:1; scale(1)` | 模态框打开 |
| messageIn | `opacity:0; translateY(6px)` → `opacity:1; none` | 消息出现 |
| panelIn | `opacity:0; translateY(4px)` → `opacity:1; translateY(0)` | 面板打开 |
| toastIn | `opacity:0; translateY(-100%)` → `opacity:1; translateY(0)` | Toast 滑入 |
| caretBlink | `0%,50%: opacity:1` → `51%,100%: opacity:0` | 光标闪烁 |

### §6.2 animate-* 类

| 类名 | animation 值 |
|---|---|
| `.animate-fade-in` | `fadeIn 0.25s ease-out` |
| `.animate-scale-in` | `scaleIn 0.15s ease-out` |
| `.animate-message-in` | `messageIn 0.25s ease-out` |
| `.animate-panel-in` | `panelIn 0.15s ease-out` |
| `.animate-toast-in` | `toastIn var(--duration-normal) var(--ease-out)` |
| `.animate-modal-in` | `modalIn var(--duration-normal) var(--ease-out)` |
| `.animate-spin` | `spin 0.8s linear infinite` |
| `.animate-caret-blink` | `caretBlink 600ms infinite step-start` |

### §6.3 prefers-reduced-motion 降级

- `animation-duration: 0.01ms !important`
- `animation-iteration-count: 1 !important`
- `transition-duration: 0.01ms !important`
- `scroll-behavior: auto !important`

---

## §7 Focus 环规格

| 属性 | 值 |
|---|---|
| outline | none (`:focus-visible`) |
| box-shadow | `0 0 0 2px var(--surface-0), 0 0 0 4px var(--accent)` |
| 高对比度 outline | `3px solid var(--accent)` |
| 高对比度 outline-offset | `2px` |
| 高对比度 box-shadow | `none !important` |

---

## §8 测试用例编号规则

### §8.1 像素级测试用例前缀

| 前缀 | 类型 | 文件 |
|---|---|---|
| TC-PIX-TOKEN | 设计令牌验证 | `pixel-tokens.spec.ts` |
| TC-PIX-COMP | 组件像素验证 | `pixel-components.spec.ts` |
| TC-PIX-LAYOUT | 布局像素验证 | `pixel-layout.spec.ts` |
| TC-PIX-THEME | 主题一致性验证 | `pixel-theme.spec.ts` |
| TC-PIX-A11Y | WCAG 对比度验证 | `pixel-a11y.spec.ts` |
| TC-PIX-ANIM | 动画/过渡验证 | `pixel-animation.spec.ts` |
| TC-UNIT-PIX | Vitest 单元像素验证 | `pixel-spec.test.js` |
