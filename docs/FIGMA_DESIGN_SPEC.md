# EchoMind 灵犀 — Figma 设计规格文档 v2.0

> **暗色主题** · 本地 AI 知识库桌面应用 · Tauri v2 + Vanilla JS
>
> 设计灵感：Arc Browser（侧栏 Tab）+ Linear（命令面板）+ Claude（消息排版）+ GitHub（代码块/Alerts）

---

## §1 设计系统概览

### 1.1 设计哲学

| 原则 | 说明 |
|---|---|
| **隐私优先** | 所有视觉元素传达"数据不出域"的安全感，无任何云图标暗示上传 |
| **暗色为主** | 纯黑底色 `#0A0A0B` 减少屏幕反光，适合长时间阅读 |
| **克制强调** | 仅一个强调色 `#38BDF8`（天蓝），用于 CTA/链接/激活态 |
| **排版为王** | 消息区是核心交互区，Markdown 排版体系是设计重点 |
| **动效有意义** | 所有动效服务于状态反馈，不做纯装饰动画 |

### 1.2 尺寸规格

| 容器 | 宽度 | 高度 | 说明 |
|---|---|---|---|
| 桌面窗口 | 1200px | 800px | 最小 900×600 |
| 侧栏（展开） | 240px | 100% | `w-60` |
| 侧栏（折叠） | 56px | 100% | `w-14` |
| 主区域 | flex-1 | 100% | 至少 660px |
| 消息气泡（用户） | max 78% | auto | 右对齐 |
| 消息气泡（AI） | max 92% | auto | 左对齐 |
| 命令面板 | 560px | max 320px | 距顶 15vh |
| Modal | max 448px | auto | 居中 |

---

## §2 Design Token 体系

### 2.1 Surface 色阶（背景层次）

| Token | Hex | 用途 |
|---|---|---|
| `surface-0` | `#0A0A0B` | App 背景（最暗） |
| `surface-1` | `#131316` | 侧栏/面板背景 |
| `surface-2` | `#1C1C20` | 消息气泡/卡片背景 |
| `surface-3` | `#26262C` | Tab 激活态/Hover |
| `surface-4` | `#303036` | Toggle OFF/禁用态 |

### 2.2 Border 色阶

| Token | Hex | 用途 |
|---|---|---|
| `border-subtle` | `#1F1F23` | 侧栏分隔线（最淡） |
| `border-default` | `#2A2A2E` | 输入框/卡片边框 |
| `border-strong` | `#3A3A40` | 命令面板/Modal 边框 |

### 2.3 Text 色阶

| Token | Hex | 用途 |
|---|---|---|
| `text-primary` | `#F8FAFC` | 主标题/高亮文本 |
| `text-secondary` | `#CBD5E1` | 正文/对话内容 |
| `text-tertiary` | `#94A3B8` | 辅助说明/标签 |
| `text-quaternary` | `#64748B` | 占位符/禁用态 |

### 2.4 Accent

| Token | Hex | 用途 |
|---|---|---|
| `accent` | `#38BDF8` | 主强调色（CTA/链接/激活） |
| `accent-hover` | `#0EA5E9` | Hover 态 |
| `accent-subtle` | `rgba(56,189,248,0.08)` | 淡强调背景 |

### 2.5 Semantic

| Token | Hex | 用途 |
|---|---|---|
| `success` | `#4ADE80` | 已索引/成功 |
| `warning` | `#FBBF24` | 索引中/警告 |
| `danger` | `#F87171` | 失败/错误 |
| `info` | `#60A5FA` | 信息提示 |

### 2.6 Border Radius

| Token | 值 | 用途 |
|---|---|---|
| `xs` | 4px | Badge/代码标签 |
| `sm` | 8px | 输入框/按钮 |
| `md` | 12px | 消息气泡/卡片 |
| `lg` | 16px | 输入栏容器 |
| `xl` | 20px | 预设卡片 |
| `2xl` | 24px | 大容器/Modal |

### 2.7 Typography Scale

| Token | Size | Weight | LH | 用途 |
|---|---|---|---|---|
| `display-lg` | 32px | 700 | 40 | 向导大标题 |
| `display-md` | 28px | 700 | 36 | Modal 标题 |
| `title-lg` | 20px | 600 | 28 | 分区标题 |
| `title-md` | 16px | 600 | 24 | 副标题 |
| `body-md` | 14px | 400 | 22 | 正文/Messages |
| `body-sm` | 13px | 400 | 20 | 辅助正文 |
| `caption` | 11px | 500 | 16 | Badge/标注 |
| `overline` | 10px | 600 | 14 | 大写分区头 |

### 2.8 Shadow / Elevation

| Token | 值 | 用途 |
|---|---|---|
| `float` | `0 4px 12px rgba(0,0,0,0.15)` | 浮动卡片 |
| `pop` | `0 8px 24px rgba(0,0,0,0.2)` | 弹出层 |
| `modal` | `0 16px 48px rgba(0,0,0,0.25)` | Modal |
| `glow` | `0 0 0 3px rgba(56,189,248,0.25)` | Focus 环 |

---

## §3 页面布局

### 3.1 向导页面（REQ-UI-007）

```
┌──────────────────────────────────────┐
│            灵犀 EchoMind              │  display-lg
│  本地 AI 知识库 · 数据不出域 · BYOK  │  body-sm, quaternary
│                                      │
│  ┌────────┐ ┌────────┐ ┌────────┐  │
│  │DeepSeek│ │ OpenAI │ │ Ollama │  │  预设卡片 xl 圆角
│  │高性价比 │ │GPT系列 │ │完全离线│  │
│  └────────┘ └────────┘ └────────┘  │
│                                      │
│  API Key                             │  overline
│  ┌──────────────────────────────┐   │  input sm 圆角
│  │ sk-...                       │   │
│  └──────────────────────────────┘   │
│                                      │
│  Base URL         Model              │
│  ┌──────────┐    ┌──────────┐       │
│  │ https://..│    │deepseek..│       │
│  └──────────┘    └──────────┘       │
│                                      │
│  去获取 API Key →     [验证并开始]   │
└──────────────────────────────────────┘
```

### 3.2 主界面（REQ-UI-001/002/003）

```
┌─────────┬──────────────────────────────────────────┐
│ 灵犀 · ‹ │                                          │
│         │                                          │
│ [💬会话]│          ◈                               │
│ [📚知识]│     向你的知识库提问                       │
│         │                                          │
│ ＋新对话 │  回答仅基于本地数据与你自己配置的模型。    │
│         │  点击左下角 ＋ 或直接把文件拖进来          │
│ 会话历史 │                                          │
│ ┌─────┐ │                                          │
│ │对话1 │ │                                          │
│ ├─────┤ │                                          │
│ │对话2 │ │                                          │
│ └─────┘ │                                          │
│         │  ┌──────────────────────────────────┐   │
│ 免费·⚙  │  │ ＋ [输入问题，Enter 发送...]  [发送]│   │
└─────────┴──────────────────────────────────────────┘
  240px                      flex-1
```

### 3.3 侧栏 Tab 切换

```
展开态 (w-60)                     折叠态 (w-14)
┌─────────────┐                 ┌──┐
│ 灵犀 ·     ‹ │                 │灵│
│             │                 │  │
│ [💬][📚]     │                 │💬│
│             │                 │  │
│ ┌─────────┐ │                 │📚│
│ │＋新对话  │ │                 │  │
│ └─────────┘ │                 │  │
│             │                 │＋│
│ 会话列表    │                 │  │
│             │                 │  │
│ ┌─────────┐ │                 │  │
│ │对话项目  │ │                 │⚙│
│ └─────────┘ │                 └──┘
│             │
│ 免费·⚙      │
└─────────────┘
```

### 3.4 命令面板（⌘K）

```
        距顶 15vh
    ┌──────────────────────┐
    │ 🔍 输入命令或搜索…  Esc│  48px 高搜索栏
    ├──────────────────────┤
    │ 导航                  │  overline 分组头
    │ 💬 新建会话      ⌘N  │  ← 选中项高亮
    │ ＋ 导入文件      ⌘O  │
    │ ⚙ 打开设置      ⌘,  │
    │                      │
    │ 对话                  │
    │ ■ 停止生成      Esc  │
    │ 🗑 清空当前对话 ⌘⇧⌫ │
    └──────────────────────┘
       max 320px 列表区
```

### 3.5 付费墙 Modal

```
┌──────────────────────────┐
│                          │
│         ◈                │  48px accent
│     升级到 Pro 版         │  display-md
│  配额已满（50/50）        │  body-sm, quaternary
│                          │
│  License Key             │
│  ┌──────────────────┐   │  input
│  │ 粘贴你的 License..│   │
│  └──────────────────┘   │
│                          │
│  [稍后再说]  [激活 Pro]  │
└──────────────────────────┘
        max 448px
```

### 3.6 设置 Modal

```
┌──────────────────────────┐
│ 设置                      │  display-md
│                          │
│ LLM 配置                  │  overline
│ ┌──────────────────┐    │
│ │ 端点   api.dee..  │    │  info card
│ │ 模型   deepse..  │    │
│ │ Key    sk-****.. │    │
│ └──────────────────┘    │
│ [修改 LLM 配置]          │
│                          │
│ PDF 图片理解（VLM）       │  overline
│ ┌──────────────────┐    │
│ │ VLM 增强开关  [○─]│    │  toggle
│ │ 表格→MD·甘特→Mmd  │    │
│ └──────────────────┘    │
│                          │
│ 授权状态                  │  overline
│ ┌──────────────────┐    │
│ │ Pro 版 · 已激活    │    │
│ └──────────────────┘    │
│                          │
│ 模型缓存                  │  overline
│ ┌──────────────────┐    │
│ │ 总大小  32.5 MB   │    │
│ └──────────────────┘    │
│ [下载模型]  [清理缓存]   │
│                          │
│       [完成]             │  primary button
└──────────────────────────┘
```

---

## §4 组件库

### 4.1 Button 状态矩阵

| 变体 | Default | Hover | Active | Disabled |
|---|---|---|---|---|
| **Primary** | accent 底+ink 字 | accent-hover 底 | opacity 0.8 | surface-3 底+0.4 透明 |
| **Secondary** | transparent+border | surface-3 底 | border-strong | 0.4 透明 |
| **Danger** | transparent+danger border | danger/10 底 | danger/20 底 | 0.4 透明 |

### 4.2 Input 状态

| 状态 | 边框 | 文字色 | 说明 |
|---|---|---|---|
| Default | border-default | quaternary | 占位符 |
| Focus | accent 1.5px | secondary | 输入中 |
| Error | danger 1.5px | danger | 错误提示 |

### 4.3 Document Status Badge

| 状态 | 标签 | 文字色 | 边框色 |
|---|---|---|---|
| Pending | 待索引 | text-tertiary | slate-500/40 |
| Processing | 索引中 | warning | warning/40 |
| Indexed | 已索引 | accent | accent/40 |
| Failed | 失败 | danger | danger/40 |

### 4.4 Message Block

**User Block：**
- 容器：`max-w-[78%]` 右对齐
- 背景：`accent/15`
- 边框：`accent/30` 1px
- 圆角：`rounded-2xl rounded-br-md`（右下角小圆角）
- 字号：`text-sm leading-6 whitespace-pre-wrap`

**Assistant Block：**
- 容器：`max-w-[92%]` 左对齐
- 背景：`surface-2`
- 边框：`border-default` 1px
- 圆角：`rounded-2xl rounded-tl-md`（左上角小圆角）
- 内含：思考指示器 → `.md` 内容区 → `.sources` 引用 → `.msg-actions` 操作栏

### 4.5 Code Block

```
┌─ code-header ──────────────────┐
│ RUST              [复制]       │  lang 标签 + 复制按钮
├────────────────────────────────┤
│ fn main() {                    │  代码内容
│     println!("Hello!");        │  font: SF Mono 13px
│ }                              │  bg: #0D1117
└────────────────────────────────┘
```

---

## §5 动效系统

### 5.1 动效定义

| 类名 | 动画 | 时长 | 缓动 | 用途 |
|---|---|---|---|---|
| `fade-in` | opacity+translateY | 250ms | ease-out | Toast/消息出现 |
| `scale-in` | opacity+scale | 150ms | ease-out | Modal/命令面板 |
| `slide-up` | translateY | 200ms | `cubic-bezier(0.34,1.56,0.64,1)` | 底部弹层 |
| `message-in` | opacity+translateY+scale | 400ms | `cubic-bezier(0.34,1.56,0.64,1)` | 消息气泡 |
| `spin` | rotate | 800ms | linear infinite | 加载指示器 |
| `thinkingPulse` | opacity+scale | 1400ms | ease-in-out infinite | 思考指示器 |

### 5.2 prefers-reduced-motion

所有动效在 `prefers-reduced-motion: reduce` 时降级为 0.01ms，确保无障碍访问。

---

## §6 Figma 插件使用指南

### 6.1 安装插件

1. 打开 Figma（桌面版或 figma.com）
2. 菜单 → Plugins → Development → Import plugin from manifest
3. 选择 `figma-plugin/manifest.json`
4. 在任意文件中运行：Plugins → Development → "EchoMind Design System Generator"

### 6.2 插件生成内容

插件自动创建 3 个 Figma 页面：

| 页面 | 内容 |
|---|---|
| **🎨 Design Tokens** | 5 组色板 + 圆角展示 + 字号样例 + 间距标尺 + 阴影样例 |
| **🧩 Components** | 按钮(4状态) + 输入框(3状态) + 徽标(4种) + Toggle + 引用芯片 + 思考指示器 + 消息气泡 + 代码块 + 预设卡片 |
| **📐 Layouts** | 向导界面 + 主界面(含侧栏+聊天区+输入栏) + 命令面板 + 付费墙 + 设置面板 |

### 6.3 导入 Token JSON

`design-tokens.json` 符合 [Design Tokens Format Module](https://design-tokens.github.io/community-group/format/) 标准，可通过 Figma Tokens 插件或 Style Dictionary 导入。
