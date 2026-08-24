# EchoMind UI 设计差距审计报告

> **审计日期**: 2026-08-18
> **对比基准**: chat.deepseek.com 实际界面（浏览器实时抓取数据）
> **审计方法**: Playwright 浏览器自动化 → 抓取 DeepSeek 计算样式 + DOM 结构 → 与 EchoMind 源码对比
> **审计人**: AI Agent

---

## 一、核心结论

**EchoMind 的 DeepSeek 借鉴只做了"形似"（圆角 22px、透明背景、CSS 变量三层），没有"神似"。** 差距不在单个属性值，而在整体视觉比例、信息密度、色彩层次和交互细节的精心调校。

---

## 二、五大差距领域

### 差距 1：布局比例与留白体系

| 维度 | DeepSeek 实测 | EchoMind 当前 | 差距分析 |
|---|---|---|---|
| 消息区域宽度 | 752px 居中 | `max-width: 100%` 全宽 + 20px padding | **致命差距**：DeepSeek 精心控制 752px 居中营造"阅读区"感，EchoMind 全宽铺满显得粗糙 |
| 消息块上下间距 | `padding: 34px 0 16px`（上 34px 下 16px） | `margin-bottom: 20px` 或 `mb-8`（32px） | EchoMind 间距不分上下、不分问答对间距 vs 消息内间距 |
| 侧栏宽度 | 261px | ~256px | 接近 |
| 输入框宽度 | 774px | 全宽 | DeepSeek 输入框与消息区对齐，EchoMind 不对齐 |
| 整体留白感 | 消息区 752px + 左右大量留白 | 全宽内容 + 少量 padding | **视觉密度完全不同**：DeepSeek "呼吸感"强，EchoMind 紧凑 |
| 消息列表最大宽度 | 840px（CSS 变量定义，实际 752px 含 padding） | `--msg-list-max-width: 100%` | **矛盾**：tokens.css 定义了 840px 但 components.css 覆盖为 100% |

### 差距 2：字号与排版体系

| 维度 | DeepSeek 实测 | EchoMind 当前 | 差距分析 |
|---|---|---|---|
| AI 正文 fontSize | 14px | 16px (`--text-base`) | **过大**：14px→16px 是 33% 增幅，信息密度大幅下降 |
| AI 正文 lineHeight | 24px (1.71) | 1.75 (`--leading-normal`) | 接近 |
| AI 正文颜色 | `rgb(97, 102, 107)` 灰色 | `--text-primary` (#F8FAFC 白色) | **致命差距**：DeepSeek 用灰色降低 AI 文字视觉权重，EchoMind 用纯白太抢眼 |
| 用户消息 fontSize | 14px | 16px | 同上 |
| 思维链 fontSize | 14px | 14px（正确） | 已对齐 |
| 思维链缩进 | `padding-left: 22px` | 不明确 | DeepSeek 用缩进表示层级关系 |

### 差距 3：色彩与视觉层次

| 维度 | DeepSeek | EchoMind | 差距分析 |
|---|---|---|---|
| AI 文字色 | `rgb(97, 102, 107)` — 中灰 | `--text-primary` 纯白 | DeepSeek 降低 AI 回复的视觉权重，引导用户关注用户消息 |
| 消息背景色 | 全透明 | 全透明 | ✅ 已对齐 |
| 侧栏背景色 | `rgb(249, 250, 251)` 浅灰（浅色模式） | `--surface-1` 深色 | 暗色模式不可比，但 DeepSeek 的侧栏与主区有色差层次 |
| 强调色运用 | 品牌色仅用于交互态（hover/active） | 品牌色用于文字、图标、边框多处 | **过度使用**：EchoMind 的 accent 色出现频率太高，稀释了强调意义 |
| 操作栏可见性 | 悬停淡入 | `opacity: 0 → group-hover:opacity-100` | ✅ 已对齐 |
| 输入框背景 | 微弱底色 | `--surface-2` | 接近 |

### 差距 4：交互细节差距

| 特性 | DeepSeek | EchoMind | 差距 |
|---|---|---|---|
| 思考链动画 | `fade-in-zoom-in-expand` 三段动画 | 无动画，直接显示 | **缺失**：DeepSeek 思考链展开有精心设计的动画 |
| 消息出现动画 | `message-in` 平移淡入 | `animate-message-in`（有但 6px 偏移过小） | 偏移量太小，视觉存在感弱 |
| 滚动行为 | `adjustScrollOnSessionUpdated` 智能调整 | 直接 `scrollToBottom` | DeepSeek 新消息到达时智能判断是否滚动（用户向上滚时不打断） |
| 最小加载延迟 | `ensureMinLoadingDelay` 防闪烁 | 无 | DeepSeek 确保 loading 至少显示一定时间，避免闪烁 |
| 输入框自适应 | `min-height: 60px` + `resize: none` + JS auto-grow | 有 auto-grow 但无 min-height 保证 | 输入框太矮时视觉不稳定 |
| 停止生成 | 立即中断 + 保留已生成内容 | 有但可能不如 DeepSeek 流畅 | 需验证 |
| 编辑消息 | hover 显示编辑 + 编辑后截断后续 | 有编辑功能 | ✅ 已实现 |
| 重新生成 | 4 种变体（简洁/详细/搜索/不搜索） | 单一重新生成 | **缺失变体** |

### 差距 5：缺失的核心功能

| 功能 | DeepSeek | EchoMind | 影响 |
|---|---|---|---|
| 会话搜索 | 侧栏搜索 + 跳转高亮 | 无侧栏搜索 | **严重影响**：历史会话多了无法查找 |
| 置顶会话 | 置顶区域 + 折叠 | 无 | 常用会话无法固定 |
| 分享功能 | 分享链接 + 选择消息 | 无 | 无法分享对话 |
| 搜索结果卡片 | 圆形 Logo + 折叠 + 引用标注 | 有来源但不折叠 | 来源信息不够精炼 |
| 消息状态机 | `AWAITING_FIRST_CHANGE` / `INCOMPLETE` / `CONTENT_FILTER` / `TIMEOUT` | 简化版 | 错误状态展示不精细 |
| 字符计数器 | 超限拦截 + 计数提示 | 无 | 用户不知道输入超限 |
| 操作过快限制 | `operationTooFastToast` | 无 | 频繁操作无保护 |
| Mermaid 图片查看器 | 缩放/下载/全屏 | 无 | Mermaid 图无法交互 |
| 代码块增强 | 彩虹括号 + banner/footer + 语言标签 | 基础代码高亮 | 代码可读性差 |
| 并行流 | `allow_parallel_streams` | 不支持 | 不支持并行流式响应 |

---

## 三、根因分析

### 为什么"形似神不似"？

1. **CSS 变量定义了但没覆盖到组件**
   - `tokens.css` 定义了 `--msg-list-max-width: 840px`，但 `components.css` 中 `#chatArea` 覆盖为 `max-width: 100%`
   - 变量定义层和组件使用层脱节

2. **字号体系盲目放大**
   - DeepSeek 用 14px 正文 → EchoMind 改成 16px
   - 可能是为了"可读性"，但结果是信息密度下降、视觉粗糙感上升

3. **色彩层次缺失**
   - DeepSeek 用灰色 `rgb(97,102,107)` 区分 AI 文字和用户文字
   - EchoMind 用 `--text-primary` 纯白，导致 AI 回复和用户消息视觉权重相同

4. **测试只验证功能不验证设计**
   - 现有测试验证"元素是否存在"、"按钮是否可见"
   - **从不验证**"字号是否合理"、"间距是否与设计一致"、"色彩对比度是否达标"

---

## 四、优先级排序

### P0 — 立即修复（影响整体视觉感）

| ID | 问题 | 修复方案 | 预期效果 |
|---|---|---|---|
| GAP-001 | 消息区域全宽 → 居中固定宽度 | `#chatArea { max-width: 840px; margin: 0 auto; }` 恢复居中 | 营造"阅读区"感 |
| GAP-002 | AI 文字纯白 → 灰色 | AI 消息 `color: var(--text-secondary)` 而非 `--text-primary` | 降低 AI 回复视觉权重 |
| GAP-003 | 正文字号 16px → 14px | `--text-base: 14px` | 提升信息密度 |
| GAP-004 | 消息间距不分层 → 精确控制 | 问答对间距 34px+16px，消息内间距 4px | 视觉节奏感 |

### P1 — 尽快修复（交互体验差距）

| ID | 问题 | 修复方案 |
|---|---|---|
| GAP-005 | 思考链无展开动画 | 添加 `fade-in-zoom-in-expand` 动画 |
| GAP-006 | 滚动行为粗暴 | 实现 `adjustScrollOnSessionUpdated` 智能滚动 |
| GAP-007 | 无最小加载延迟 | 添加 `ensureMinLoadingDelay` 防闪烁 |
| GAP-008 | 输入框无 min-height | `min-height: 60px` 保证视觉稳定 |

### P2 — 规划实施（功能差距）

| ID | 问题 | 修复方案 |
|---|---|---|
| GAP-009 | 无会话搜索 | 侧栏搜索 + 跳转高亮 |
| GAP-010 | 无置顶会话 | 置顶区域 + 折叠 |
| GAP-011 | 重新生成无变体 | 4 种变体 |
| GAP-012 | 无字符计数器 | 超限拦截 + 计数 |

---

## 五、设计质量验证测试需求

现有测试用例只验证"功能存在"，不验证"设计质量"。需要新增以下测试类别：

### TC-DESIGN — 设计质量验证测试

| 测试 ID | 验证内容 | 验证方法 |
|---|---|---|
| TC-DESIGN-001 | 消息区域居中且 max-width ≤ 840px | `getComputedStyle(el).maxWidth` |
| TC-DESIGN-002 | AI 正文 fontSize ≤ 15px | `getComputedStyle(el).fontSize` |
| TC-DESIGN-003 | AI 正文颜色非纯白 | `getComputedStyle(el).color !== 'rgb(248, 250, 252)'` |
| TC-DESIGN-004 | 用户消息右对齐 | `getComputedStyle(el).marginRight === '0px'` |
| TC-DESIGN-005 | 问答对间距 ≥ 30px | `getBoundingClientRect` 差值 |
| TC-DESIGN-006 | 消息内间距 ≤ 8px | 同上 |
| TC-DESIGN-007 | 思考链展开有动画 | `getComputedStyle` animationName 非空 |
| TC-DESIGN-008 | 输入框 min-height ≥ 56px | `getComputedStyle(el).minHeight` |
| TC-DESIGN-009 | 输入框与消息区宽度对齐 | offsetWidth 差值 ≤ 20px |
| TC-DESIGN-010 | 暗色主题对比度 ≥ 4.5:1 | WCAG 色差计算 |
| TC-DESIGN-011 | 侧栏宽度 ≤ 280px | `offsetWidth` |
| TC-DESIGN-012 | accent 色仅用于交互态 | CSS 选择器扫描 |
| TC-DESIGN-013 | 消息出现动画位移 ≥ 8px | `@keyframes` 解析 |
| TC-DESIGN-014 | 操作栏默认不可见 | `opacity < 0.1` |
| TC-DESIGN-015 | 焦点环使用 box-shadow | `getComputedStyle:focus-visible` |

---

## 六、对比截图参考

- DeepSeek 首页：`/tmp/.playwright-mcp/deepseek-home.png`
- DeepSeek 对话页：`/tmp/.playwright-mcp/deepseek-chat.png`
- DeepSeek 全页：`/tmp/.playwright-mcp/deepseek-full.png`
