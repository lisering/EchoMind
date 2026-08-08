# DeepSeek 聊天界面全面分析报告

> 基于对 `chat.deepseek.com` 实际页面的 DOM 结构、计算样式、CSS、JavaScript 及交互行为的深度逆向分析。

---

## 目录

1. [整体架构与技术栈](#1-整体架构与技术栈)
2. [布局系统](#2-布局系统)
3. [侧边栏设计](#3-侧边栏设计)
4. [聊天消息区域](#4-聊天消息区域)
5. [思维链与搜索结果展示](#5-思维链与搜索结果展示)
6. [Markdown 渲染系统](#6-markdown-渲染系统)
7. [输入区域设计](#7-输入区域设计)
8. [设计系统与 Design Tokens](#8-设计系统与-design-tokens)
9. [按钮系统](#9-按钮系统)
10. [主题系统](#10-主题系统)
11. [交互模式与动画](#11-交互模式与动画)
12. [虚拟滚动与性能优化](#12-虚拟滚动与性能优化)
13. [模型配置系统](#13-模型配置系统)
14. [对 EchoMind 的借鉴建议](#14-对-echomind-的借鉴建议)

---

## 1. 整体架构与技术栈

### 1.1 技术栈

| 层面 | 技术 |
|---|---|
| 前端框架 | React（生产构建，webpack 打包） |
| JS 分包 | `default-vendors.js`（第三方库） + `main.js`（业务代码） |
| CSS 方案 | 单一 CSS 文件 + KaTeX CSS，类名混淆（hash-based） |
| 图标系统 | 全部内联 SVG，颜色通过 CSS 变量 `fill="var(--dsw-alias-brand-primary)"` 控制 |
| 数学公式 | KaTeX（独立 CSS: `katex.effce08cb3.css`） |
| 分析埋点 | Volcengine Rangers SDK + APM Plus |
| 安全防护 | 风控 SDK（`smcp.min.js`，Fengkongcloud） |
| 特性开关 | 远程特性存储（localStorage `__ds_remote_feature_store`） |

### 1.2 关键架构特征

- **零 `<button>` 标签**：整个页面没有任何语义化 `<button>` 元素，所有可点击元素使用 `<div>` + 事件处理器。这是 React 自定义组件的典型模式。
- **类名混淆**：所有 CSS 类名为 8 字符 hash（如 `cb86951c`、`dc04ec1d`），生产构建产物。
- **Design System 前缀**：所有设计系统组件使用 `ds-` 前缀（如 `ds-button`、`ds-icon`、`ds-markdown`、`ds-scroll-area`、`ds-message`、`ds-virtual-list`），语义化组件类名保留。
- **CSS 变量驱动**：颜色、尺寸、动画时长全部通过 CSS 自定义属性（`--dsw-*` 和 `--ds-*`）控制，支持运行时主题切换。

### 1.3 Body 类名系统

```html
<body class="apple zh_CN light" style="--app-height: 687px;">
```

- `apple` / `windows` / `linux` — 平台检测
- `zh_CN` / `en_US` — 语言检测
- `light` / `dark` — 主题模式
- `--app-height` — 动态视口高度（用于 TWA/Electron 适配）

---

## 2. 布局系统

### 2.1 整体布局

```
┌─────────────────────────────────────────────────────┐
│  Root Container (1200px × --app-height)             │
│  display: flex                                       │
│                                                      │
│  ┌──────────┐  ┌────────────────────────────────┐  │
│  │ Sidebar  │  │ Main Content Area              │  │
│  │ 261px    │  │ calc(100% - 261px)             │  │
│  │          │  │                                 │  │
│  │ fixed    │  │ Header (48px)                   │  │
│  │ width    │  │ ┌─────────────────────────┐    │  │
│  │          │  │ │ Title + Mode Badge       │    │  │
│  │          │  │ └─────────────────────────┘    │  │
│  │          │  │                                 │  │
│  │          │  │ Message List (flex: 1)          │  │
│  │          │  │ ┌─────────────────────────┐    │  │
│  │          │  │ │ Virtual Scroll Container │    │  │
│  │          │  │ │  max-width: 840px        │    │  │
│  │          │  │ │  padding: 0 44px         │    │  │
│  │          │  │ └─────────────────────────┘    │  │
│  │          │  │                                 │  │
│  │          │  │ Input Area (shrink: 0)          │  │
│  │          │  │ ┌─────────────────────────┐    │  │
│  │          │  │ │ Textarea + Toggles      │    │  │
│  │          │  │ └─────────────────────────┘    │  │
│  └──────────┘  └────────────────────────────────┘  │
└─────────────────────────────────────────────────────┘
```

### 2.2 布局 CSS 变量

```css
:root {
  --sider-width: 261px;
  --message-list-max-width: 840px;
  --message-list-padding-horizontal: 44px;
}
```

### 2.3 侧边栏过渡动画

```css
.sidebar {
  transition: transform 0.3s cubic-bezier(0.4, 0, 0.2, 1),
              max-width 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}
```

侧边栏折叠/展开使用 `transform` + `max-width` 双属性过渡，贝塞尔曲线 `cubic-bezier(0.4, 0, 0.2, 1)` 提供 Material Design 标准缓动效果。

---

## 3. 侧边栏设计

### 3.1 结构

```
Sidebar (261px, background: rgb(249, 250, 251))
├── Header Area (padding: 6px 12px 10px)
│   ├── Logo (SVG, 143×23px, fill: var(--dsw-alias-brand-primary))
│   ├── Collapse Toggle Buttons
│   └── "开启新对话" Button (236×40px, shortcut: ⌘ J)
├── Conversation History (scrollable)
│   ├── 今天 (Today)
│   │   └── [conversation items]
│   ├── 昨天 (Yesterday)
│   │   └── [conversation items]
│   ├── 7 天内 (Within 7 days)
│   │   └── [conversation items]
│   ├── 30 天内 (Within 30 days)
│   │   └── [conversation items]
│   ├── 2026-07
│   │   └── [conversation items]
│   └── 2026-06
│       └── [conversation items]
└── User Profile ("johnsong")
```

### 3.2 会话列表项样式

```css
/* 普通状态 */
.conversation-item {
  display: flex;
  padding: 9px 6px 9px 10px;
  border-radius: 12px;
  color: rgb(15, 17, 21);          /* --dsw-alias-label-primary */
  font-size: 14px;
  font-weight: 400;
  cursor: pointer;
  height: 40px;
  line-height: 22px;
  transition: all;                  /* 全属性过渡 */
}

/* 激活状态 */
.conversation-item.active {
  background: rgb(228, 237, 253);  /* 浅蓝色背景 */
  color: rgb(57, 100, 254);        /* 品牌蓝色文字 */
}
```

### 3.3 日期分组策略

- **今天**：当天的对话
- **昨天**：昨天的对话
- **7 天内**：2-7 天前的对话
- **30 天内**：8-30 天前的对话
- **按月分组**：超过 30 天的对话按 `YYYY-MM` 分组

这种分组方式比简单的时间倒序更符合用户记忆模式。

### 3.4 新对话按钮

- 文本："开启新对话"
- 快捷键提示："⌘ J"（显示在按钮右侧）
- 尺寸：236×40px
- 可点击区域覆盖整行

---

## 4. 聊天消息区域

### 4.1 消息容器

```css
.message-list-items {
  padding: 0 49.5px;
  margin: 0 44px;
}
```

消息列表使用 `ds-virtual-list` 虚拟滚动，只渲染可见区域的消息。

### 4.2 用户消息气泡

```css
.user-message-bubble {
  background: rgb(237, 243, 251);      /* 浅蓝色 --dsw-alias-interactive-bg-selected */
  color: rgb(15, 17, 21);              /* 深色文字 */
  padding: 10px 16px;
  border-radius: 22px;                 /* 超大圆角 */
  max-width: calc(100% - 88px);        /* 不占满全行，留出空间 */
  font-size: 16px;
  line-height: 24px;
  display: block;                       /* 内联块级，右对齐 */
}
```

**设计要点**：
- 圆角 22px，比常规 8-12px 更大，营造"气泡"感
- 最大宽度 `calc(100% - 88px)`，右侧留白，视觉上区分用户/AI
- 背景使用浅蓝色（而非品牌蓝），避免视觉干扰
- 消息容器使用 `flex` 布局，用户消息右对齐

### 4.3 AI 消息（无气泡）

AI 消息**没有气泡**，直接以 Markdown 格式渲染在页面上，与用户消息形成视觉对比：
- 无背景色、无边框、无圆角
- 文字颜色 `rgb(15, 17, 21)`（深色，高对比度）
- 字号 16px，行高 28px（比用户消息的 24px 更大，提升可读性）

### 4.4 消息间距

- 用户消息与 AI 消息之间有 `padding-bottom: 16px`
- AI 回答结束后有"本回答由 AI 生成"免责声明
- 免责声明样式：`font-size: 14px; color: rgb(129, 133, 140); font-style: italic; margin: 16px 0 0`

### 4.5 操作按钮行

每条 AI 消息底部有 5 个操作按钮，使用 `ds-button--iconLabelTertiary` 样式：

| 按钮 | 功能 | 推测图标 |
|---|---|---|
| 1 | 复制 | copy/clipboard icon |
| 2 | 重新生成 | refresh/retry icon |
| 3 | 点赞 | thumbs-up icon |
| 4 | 点踩 | thumbs-down icon |
| 5 | 分享 | share icon |

- 尺寸：16×16px（图标）
- 颜色：`rgb(97, 102, 107)`（灰色，非激活态）
- 悬停：颜色变深或背景变化
- 无文字标签，纯图标操作

---

## 5. 思维链与搜索结果展示

### 5.1 思维链（Thinking Process）

```
┌─────────────────────────────────────────────┐
│  💡 已思考（用时 4 秒）         ▼ (展开/折叠)  │  ← 可点击头部
├─────────────────────────────────────────────┤
│  • (动画指示器)                               │  ← padding-left: 22px
│                                                │
│  用户想用Rust实现Claude Code的逆向版本。      │  ← ds-markdown (小字号)
│  我需要提供一份全面的指南...                   │
│                                                │  ← color: rgb(97, 102, 107) 灰色
└─────────────────────────────────────────────┘
```

**样式细节**：
- 头部：flex 布局，`cursor: pointer`，可折叠
- 图标：灯泡 SVG，颜色 `rgb(57, 100, 254)`（品牌蓝）
- 文字："已思考（用时 N 秒）"，颜色 `rgb(97, 102, 107)`（灰色）
- 展开/折叠图标：chevron SVG
- 内容区：`ds-think-content` 类，`padding: 5px 0 5px 22px`
- 动画指示器：小圆点，`border-radius: 50%`，颜色 `rgb(129, 133, 140)`
- 思维内容使用 `ds-markdown` 渲染，但字号更小（14px vs 16px）、颜色更淡（灰色 vs 深色）

**折叠动画**：
```css
.ds-fade-in-zoom-in-expand-enter {
  opacity: 0;
  transform: scale(0.8);
}
.ds-fade-in-zoom-in-expand-enter-active {
  opacity: 1;
  transform: scale(1);
  transition: max-height var(--ds-ease-in) var(--ds-transition-duration-fast),
              opacity var(--ds-ease-in) var(--ds-transition-duration-fast),
              transform var(--ds-ease-in) var(--ds-transition-duration-fast);
}
```

### 5.2 搜索结果展示

```
┌──────────────────────────────────────────────┐
│  🔍 搜索到 14 个网页  [🌐][🌐][🌐]  ▼        │  ← 可点击头部 + 网站logo
├──────────────────────────────────────────────┤
│  📄 浏览 1 个页面                              │
│  └─ claurst by Kuberwastaken (链接)           │
└──────────────────────────────────────────────┘
```

**样式细节**：
- 搜索图标：颜色 `rgb(97, 102, 107)`（灰色）
- "搜索到 N 个网页"：可点击折叠
- 网站 Logo：圆形（`border-radius: 50%`），16×16px，背景 `rgb(235, 238, 242)`，重叠显示（`margin-left: -4px`）
- "浏览 N 个页面"：子区域，展示实际访问的页面
- 页面链接：可点击跳转

### 5.3 引用标注

AI 回答中的引用使用上标数字（如 `-3`、`-13`），渲染为蓝色链接：
```css
.citation-link {
  color: rgb(57, 100, 254);   /* 品牌蓝 */
  margin: 0 -3px;              /* 微调间距 */
  display: inline;
}
```

---

## 6. Markdown 渲染系统

### 6.1 容器

```css
.ds-markdown {
  font-size: 16px;
  line-height: 28px;
  color: rgb(15, 17, 21);
  font-family: "quote-cjk-patch", "Inter", system-ui, -apple-system, ...;
}
```

### 6.2 元素样式表

| 元素 | font-size | font-weight | line-height | color | margin | 其他 |
|---|---|---|---|---|---|---|
| `h3` | 20px | 700 | 30px | `rgb(15, 17, 21)` | `32px 0 16px` | - |
| `p` | 16px | 400 | 28px | `rgb(15, 17, 21)` | `0 0 16px` | class: `ds-markdown-paragraph` |
| `ul` | 16px | 400 | 28px | `rgb(15, 17, 21)` | `4px 0 0` | `padding-left: 18px` |
| `li` | 16px | 400 | 28px | `rgb(15, 17, 21)` | `0` | `display: list-item` |
| `code` (inline) | 14px | 400 | 22px | `rgb(15, 17, 21)` | `0` | `padding: 0 5px`, `background: rgb(235, 238, 242)`, `border-radius: 6px`, `display: inline-flex` |
| `strong` | 16px | 600 | 28px | `rgb(15, 17, 21)` | `0` | - |
| `a` | 16px | 400 | 28px | `rgb(57, 100, 254)` | `0 -3px` | 品牌蓝色链接 |

### 6.3 思维链中的 Markdown（小字号变体）

思维链内容使用同一 `ds-markdown` 类，但覆盖了字号：
- `font-size: 14px`（vs 正文 16px）
- `line-height: 24px`（vs 正文 28px）
- `color: rgb(97, 102, 107)`（灰色，vs 正文深色）

### 6.4 CJK 引号修复

```css
@font-face {
  font-family: 'quote-cjk-patch';
  src: local('PingFangSC-Regular');
  unicode-range: U+201C-201D, U+2018-2019;
}
```

使用 `unicode-range` 精确匹配左右弯引号（`""''`），用 PingFangSC 字体渲染，解决 Inter 等西文字体引号在中文环境下的视觉不协调问题。

---

## 7. 输入区域设计

### 7.1 布局

```
┌──────────────────────────────────────────────┐
│                                               │
│  给 DeepSeek 发送消息                         │  ← textarea (placeholder)
│                                               │
│                                               │
├──────────────────────────────────────────────┤
│  💡 深度思考    🔍 智能搜索        [发送按钮]  │  ← toggle + send
└──────────────────────────────────────────────┘
  内容由 AI 生成，请仔细甄别                     ← 底部免责声明
```

### 7.2 Textarea 样式

```css
.chat-textarea {
  background: transparent;
  color: rgb(15, 17, 21);
  font-size: 16px;
  line-height: 24px;
  padding: 12px 12px 0 16px;
  border: none;
  border-radius: 0;
  resize: none;
  min-height: 60px;
  /* 使用自定义滚动条 ds-scroll-area */
}
```

### 7.3 功能切换按钮

| 按钮 | 功能 | 激活态颜色 | 默认态颜色 |
|---|---|---|---|
| 💡 深度思考 | 启用 DeepSeek-R1 推理 | `rgb(57, 100, 254)` 品牌蓝 | 灰色 |
| 🔍 智能搜索 | 启用联网搜索 | `rgb(57, 100, 254)` 品牌蓝 | 灰色 |

- 容器 padding: `12px`
- cursor: pointer
- transition: `all`
- 图标颜色随状态变化

### 7.4 发送按钮

- 位于输入框右下角
- 圆形或圆角按钮
- 禁用态：灰色 + 不可点击
- 激活态（有输入内容）：品牌蓝

---

## 8. 设计系统与 Design Tokens

### 8.1 CSS 变量层级

```
--dsw-*        DeepSeek Web 设计令牌（语义化别名）
├── --dsw-alias-*     语义化别名（引用 static 变量）
├── --dsw-static-*    静态颜色值
├── --dsw-font-*      字体规范
└── --dsw-shadow-*    阴影层级

--ds-*         通用设计系统变量
├── --ds-control-height-*   控件高度
├── --ds-transition-duration*   过渡时长
└── --ds-ease-*    缓动函数
```

### 8.2 颜色体系

| 语义名 | Light 模式值 | 用途 |
|---|---|---|
| `--dsw-alias-brand-primary` | `rgb(57, 100, 254)` / `#3964FE` | 品牌蓝，用于链接、激活态、图标 |
| `--dsw-alias-label-primary` | `rgb(15, 17, 21)` / `#0F1115` | 主文字颜色 |
| `--dsw-alias-label-secondary` | `rgb(97, 102, 107)` / `#61666B` | 次要文字（思维链、搜索结果） |
| `--dsw-alias-label-caption` | `rgb(129, 133, 140)` / `#81858C` | 说明文字（免责声明、禁用态） |
| `--dsw-alias-interactive-bg-hover` | 灰色 hover | 悬停背景 |
| `--dsw-alias-interactive-bg-selected` | `rgb(237, 243, 251)` | 选中背景（用户消息气泡） |
| `--dsw-alias-border-l2` | 浅灰色 | 边框 |
| `--dsw-static-neutral-bluish-00` | 白色 | 静态白色 |
| `--dsw-alias-state-error-primary` | 红色 | 错误状态 |
| `--dsw-alias-state-warn-primary` | 橙色 | 警告状态 |
| `--dsw-alias-button-info-fill` | 信息蓝 | 信息按钮 |

### 8.3 排版系统

| 令牌 | font-size | line-height | 用途 |
|---|---|---|---|
| `--dsw-font-s-14` | 14px | 22px | 正文小字号（思维链、侧边栏） |
| `--dsw-font-xs-13` | 13px | - | 超小字号（xs 按钮） |
| 正文默认 | 16px | 28px | AI 回答正文 |
| 思维链 | 14px | 24px | 思维链内容 |
| H3 | 20px | 30px | 三级标题 |

### 8.4 字体栈

```css
--dsw-font-family: "quote-cjk-patch", "Inter", system-ui, -apple-system,
                   BlinkMacSystemFont, "Segoe UI", Roboto, Oxygen, Ubuntu,
                   Cantarell, "Open Sans", "Helvetica Neue", sans-serif;
```

- `quote-cjk-patch`：CJK 引号专用字体（PingFangSC）
- `Inter`：主西文字体（现代、清晰）
- `system-ui` 系列：系统字体回退

### 8.5 阴影层级

| 令牌 | 用途 |
|---|---|
| `--dsw-shadow-lv1` | 卡片、下拉菜单 |
| `--dsw-shadow-lv2` | 弹出层 |
| `--dsw-shadow-lv3` | 浮动按钮、模态框 |

### 8.6 控件高度

| 令牌 | 高度 | 用途 |
|---|---|---|
| `--ds-control-height-xs` | ~24px | 超小按钮 |
| `--ds-control-height-s` | ~28px | 小按钮 |
| `--ds-control-height-m` | ~32px | 中按钮 |
| `--ds-control-height-l` | ~36px | 大按钮 |
| `--ds-control-height-xl` | ~40px | 超大按钮（新对话按钮） |

---

## 9. 按钮系统

### 9.1 按钮变体

| 变体 | class | 特点 |
|---|---|---|
| Filled | `ds-button--filled` | 实心背景 |
| Outlined | `ds-button--outlined` | 描边，透明背景 |
| Borderless | `ds-button--borderless` | 无边框，透明背景 |
| Text | `ds-button--text` | 纯文本按钮 |
| Icon | `ds-button--icon` | 纯图标按钮 |

### 9.2 按钮颜色

| 颜色 | class | 用途 |
|---|---|---|
| Primary | `ds-button--primary` | 品牌蓝 |
| Error | `ds-button--error` | 红色（删除等） |
| Warning | `ds-button--warning` | 橙色（警告） |
| Info | `ds-button--textLink` | 信息蓝 |
| Neutral | `ds-button--outlinedNeutral` | 中性灰 |

### 9.3 按钮形状

| 形状 | border-radius | 用途 |
|---|---|---|
| Pill（默认） | `4096px` | 大圆角药丸形 |
| Circle | `50%` | 圆形 |
| RoundRect (xs) | `8px` | 圆角矩形（小） |
| RoundRect (s/m/l/xl) | `10px` | 圆角矩形（大） |

### 9.4 按钮结构

```html
<div class="ds-button ds-button--icon ds-button--iconLabelTertiary">
  <div class="ds-button__background"></div>  <!-- 背景层 -->
  <div class="ds-button__icon">               <!-- 图标层 -->
    <svg>...</svg>
  </div>
  <div class="ds-button__content">             <!-- 内容层 -->
    文字
  </div>
</div>
```

**三层结构**：
1. `__background`：背景层，绝对定位，处理 hover/focus 背景变化
2. `__icon`：图标层，flex 居中
3. `__content`：内容层，相对定位

### 9.5 焦点环

```css
.ds-button:focus-visible:not(.ds-button--disabled) > .ds-button__background::after {
  box-shadow: 0 0 0 2px var(--dsl-button-ring-color);
  content: "";
  position: absolute;
  top: -2px; bottom: -2px; left: -2px; right: -2px;
  border-radius: calc(var(--dsl-button-border-radius) + 2px);
}
```

使用 `box-shadow` 而非 `outline` 实现焦点环，可跟随 `border-radius` 变化。

### 9.6 禁用态

```css
.ds-button--disabled {
  opacity: 0.45;
  cursor: not-allowed;
}
```

---

## 10. 主题系统

### 10.1 主题存储

```json
// localStorage key: __appKit_@deepseek/chat_themePreference
{"value": "system", "__version": "0"}
```

支持三种模式：
- `system`：跟随系统（默认）
- `light`：强制浅色
- `dark`：强制深色

### 10.2 主题切换机制

```css
/* 主题切换时临时禁用所有过渡 */
body.change-theme * {
  transition: none !important;
}
body.change-theme *::before {
  transition: none !important;
}
body.change-theme *::after {
  transition: none !important;
}
```

切换流程：
1. 添加 `body.change-theme` 类（禁用过渡）
2. 修改 body 上的 `light`/`dark` 类
3. CSS 变量值随主题切换
4. 移除 `body.change-theme` 类（恢复过渡）

### 10.3 暗色模式适配

所有颜色通过 `--dsw-alias-*` 语义化变量引用，暗色模式下只需修改变量值，组件代码无需改动。

---

## 11. 交互模式与动画

### 11.1 过渡系统

| 令牌 | 用途 |
|---|---|
| `--ds-transition-duration` | 常规过渡（颜色、背景） |
| `--ds-transition-duration-fast` | 快速过渡（折叠/展开） |
| `--ds-transition-duration-slow` | 慢速过渡（文字颜色变化） |
| `--ds-ease-in` | 入场缓动 |
| `--ds-ease-out` | 出场缓动 |
| `--ds-ease-in-out` | 双向缓动 |

### 11.2 加载动画

```css
.ds-loading {
  width: 1em;
  height: 1em;
  animation: 0.6s linear infinite ds-loading;
}
@keyframes ds-loading {
  0% { transform: rotate(0); }
  100% { transform: rotate(360deg); }
}
```

### 11.3 折叠/展开动画

思维链和搜索结果使用 `fade-in-zoom-in-expand` 动画：
- 入场：`opacity: 0 → 1` + `scale(0.8) → 1`
- 出场：`opacity: 1 → 0` + `scale(1) → 0.8`
- 属性：`max-height` + `opacity` + `transform` 同步过渡

### 11.4 悬停效果

- 会话列表项：背景色变化（透明 → 浅灰/浅蓝）
- 操作按钮：图标颜色加深
- 网站 Logo：`box-shadow 0.2s` 悬停效果
- 链接：颜色变化

### 11.5 键盘快捷键

| 快捷键 | 功能 |
|---|---|
| `⌘ J` | 开启新对话 |

---

## 12. 虚拟滚动与性能优化

### 12.1 虚拟列表

```html
<div class="ds-virtual-list-items">
  <div class="ds-virtual-list-visible-items">
    <!-- 只渲染可见区域的消息 -->
  </div>
</div>
```

- 只渲染视口内的消息 DOM
- 滚动时动态创建/销毁消息元素
- 支持大量对话历史而不卡顿

### 12.2 自定义滚动条

```html
<div class="ds-scroll-area ds-scroll-area--show-on-focus">
  <div class="ds-scroll-area__gutters">
    <div class="ds-scroll-area__horizontal-gutter"></div>
    <div class="ds-scroll-area__horizontal-bar"></div>
    <div class="ds-scroll-area__vertical-gutter"></div>
    <div class="ds-scroll-area__vertical-bar"></div>
  </div>
</div>
```

- 自定义滚动条样式
- 聚焦时显示（`--show-on-focus`）
- 水平/垂直独立控制

### 12.3 其他优化

- **JS 分包**：vendors + main 分离，利用缓存
- **CSS 单文件**：减少请求数
- **SVG 内联**：避免图标请求
- **远程特性开关**：通过 localStorage 缓存远程配置，减少 API 请求

---

## 13. 模型配置系统

### 13.1 三种模式

从 localStorage `__ds_remote_feature_store_model` 逆向出的模型配置：

| 模式 | 标识 | 特点 | 字符限制 | 搜索 | 文件上传 | 思维链 |
|---|---|---|---|---|---|---|
| 快速模式 | `default` | 日常对话，即时响应 | 2,621,440 | ✅ | ✅ | ✅ |
| 专家模式 | `expert` | 复杂问题，资源紧张 | 163,840 | ❌ | ❌ | ✅ |
| 识图模式 | `vision` | 图片理解 | 2,621,440 | ❌ | ✅(vision) | ✅ |

### 13.2 文件上传配置

```json
{
  "token_limit": 890880,
  "token_limit_with_thinking": 890880,
  "max_input_file_count": 50,
  "max_upload_file_size": 104857600,  // 100MB
  "support_file_exts": ["pdf", "png", "jpg", "txt", "md", "py", "rs", ...],
  "conflict_with_search": false,
  "vision": false,
  "enable_thumbnail": true
}
```

### 13.3 配额系统

- `edit_quota`: 5（消息编辑次数）
- `regenerate_quota`: 5（重新生成次数）
- 远程配置可动态调整

---

## 14. 对 EchoMind 的借鉴建议

### 14.1 高优先级借鉴

| 特性 | DeepSeek 做法 | EchoMind 现状 | 建议 |
|---|---|---|---|
| 用户消息气泡 | 浅蓝背景 + 22px 超大圆角 | 无气泡 | 增加用户消息气泡，提升视觉区分度 |
| 思维链折叠 | 可折叠，灰色小字号 | 无 | 如有 Agent 模式，增加思维链折叠展示 |
| 虚拟滚动 | `ds-virtual-list` 只渲染可见项 | 分页加载 | 长对话场景考虑虚拟滚动 |
| 日期分组会话 | 今天/昨天/7天/30天/月份 | 无分组 | 增加日期分组，提升查找效率 |
| 操作按钮行 | 5 个图标按钮（复制/重生成/赞/踩/分享） | 无 | 增加 AI 消息操作按钮 |
| CJK 引号修复 | `@font-face` + `unicode-range` | 无 | 修复中文引号显示问题 |
| 模型模式切换 | 快速/专家/识图三模式 | 无 | 如有多模型，增加模式切换 UI |

### 14.2 设计系统借鉴

| 特性 | DeepSeek 做法 | EchoMind 现状 | 建议 |
|---|---|---|---|
| CSS 变量分层 | `--dsw-alias-*` 语义化 + `--dsw-static-*` 静态值 | `--text-*` / `--surface-*` | 增加语义化别名层 |
| 按钮系统 | 5 变体 × 5 颜色 × 5 尺寸 × 3 形状 | 简单 Tailwind 类 | 建立完整按钮设计系统 |
| 焦点环 | `box-shadow` 跟随圆角 | `outline` | 改用 box-shadow 实现焦点环 |
| 主题切换 | 临时禁用过渡 + class 切换 | 直接切换 | 增加过渡禁用机制 |
| 字体栈 | Inter + CJK 引号修复 | 系统字体 | 增加 Inter 字体 + CJK 引号修复 |

### 14.3 交互借鉴

| 特性 | DeepSeek 做法 | EchoMind 现状 | 建议 |
|---|---|---|---|
| 快捷键提示 | "⌘ J" 显示在按钮旁 | 无 | 增加快捷键提示 |
| 功能切换 | 深度思考/智能搜索 toggle | 设置面板内 | 增加输入框旁的快速 toggle |
| 搜索结果展示 | 圆形网站 logo 重叠 + 折叠 | 无 | RAG 检索结果可借鉴折叠展示 |
| 免责声明 | 斜体灰色小字 | 无 | 增加 AI 生成内容免责声明 |
| 输入框自适应 | min-height 60px + resize none | 固定高度 | 输入框自适应内容高度 |

### 14.4 性能借鉴

| 特性 | DeepSeek 做法 | EchoMind 现状 | 建议 |
|---|---|---|---|
| 虚拟滚动 | `ds-virtual-list` | 分页加载 | 长对话列表考虑虚拟滚动 |
| JS 分包 | vendors + main | 单 HTML 内联 | 保持内联（桌面应用无需分包） |
| 远程特性开关 | localStorage 缓存 | 设置表 | 可增加特性开关缓存机制 |
| 动画性能 | transform + opacity 优先 | 未知 | 确保动画使用 GPU 加速属性 |

### 14.5 不建议借鉴的部分

| 特性 | 原因 |
|---|---|
| 零 `<button>` 标签 | 语义化 HTML 对无障碍访问很重要，EchoMind 已有 A11Y 要求 |
| 类名混淆 | EchoMind 是开源项目，不需要混淆 |
| 远程分析 SDK | EchoMind 是本地优先的隐私应用，不应集成第三方分析 |
| 风控 SDK | 同上，隐私应用不应引入第三方风控 |

---

## 附录 A: 完整 CSS 变量清单

```css
:root {
  /* 布局变量 */
  --sider-width: 261px;
  --message-list-max-width: 840px;
  --message-list-padding-horizontal: 44px;

  /* 字体 */
  --dsw-font-family: "quote-cjk-patch", "Inter", system-ui, -apple-system,
                     BlinkMacSystemFont, "Segoe UI", Roboto, Oxygen, Ubuntu,
                     Cantarell, "Open Sans", "Helvetica Neue", sans-serif;

  /* 按钮系统 */
  --dsl-button-border-radius: 4096px;
  --dsl-button-height: var(--ds-control-height-*);
  --dsl-button-font-size: var(--dsw-font-s-14-font-size);
  --dsl-button-padding: 0 14px;
  --dsl-button-icon-size: 16px;
  --dsl-button-icon-gap: 6px;
  --dsl-button-min-width: 72px;
  --dsl-button-opacity-disabled: 0.45;

  /* 过渡 */
  --ds-transition-duration: ...;
  --ds-transition-duration-fast: ...;
  --ds-transition-duration-slow: ...;
  --ds-ease-in: ...;
  --ds-ease-out: ...;
  --ds-ease-in-out: ...;

  /* 阴影 */
  --dsw-shadow-lv1: ...;
  --dsw-shadow-lv2: ...;
  --dsw-shadow-lv3: ...;
}
```

## 附录 B: 颜色对照表

| 语义 | RGB | HEX | 用途 |
|---|---|---|---|
| Brand Primary | `rgb(57, 100, 254)` | `#3964FE` | 品牌、链接、激活态 |
| Label Primary | `rgb(15, 17, 21)` | `#0F1115` | 主文字 |
| Label Secondary | `rgb(97, 102, 107)` | `#61666B` | 次要文字 |
| Label Caption | `rgb(129, 133, 140)` | `#81858C` | 说明文字 |
| Surface 0 (Body) | `rgb(255, 255, 255)` | `#FFFFFF` | 主背景 |
| Surface 1 (Sidebar) | `rgb(249, 250, 251)` | `#F9FAFB` | 侧边栏背景 |
| Selected BG | `rgb(228, 237, 253)` | `#E4EDFD` | 激活项背景 |
| User Bubble | `rgb(237, 243, 251)` | `#EDF3FB` | 用户消息气泡 |
| Code BG | `rgb(235, 238, 242)` | `#EBEEF2` | 代码块背景 |
| Logo BG | `rgb(235, 238, 242)` | `#EBEEF2` | 网站 Logo 占位 |

---

*报告生成日期：2026-08-03*
*分析方法：浏览器 DOM 检查 + 计算样式提取 + CSS 文件逆向 + localStorage 配置分析*
