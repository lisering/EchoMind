# EchoMind UI 与交互全面测试用例集

> **文档定位**：本文件是 EchoMind 全部 UI 与交互的原子级测试用例清单。
> 从 `UI_INTERACTION_SPEC.md` 规格 + `UI_TEST_STANDARD.md` 标准派生。
>
> **版本**：v1.0.0 | **更新日期**：2026-08-18 | **适用版本**：EchoMind v2.2.0+
> **用例总数**：~1350 个 | **预计运行时间**：~75 分钟/轮（桥接模式）

---

## 目录

1. [聊天核心 (TC-CHAT)](#1-聊天核心-tc-chat)
2. [侧栏/导航 (TC-NAV)](#2-侧栏导航-tc-nav)
3. [设置面板 (TC-SET)](#3-设置面板-tc-set)
4. [文档管理 (TC-DOC)](#4-文档管理-tc-doc)
5. [安全/加密 (TC-SEC)](#5-安全加密-tc-sec)
6. [知识图谱 (TC-GRAPH)](#6-知识图谱-tc-graph)
7. [Agentic AI 面板 (TC-AGENT)](#7-agentic-ai-面板-tc-agent)
8. [输入区/快捷键 (TC-INPUT)](#8-输入区快捷键-tc-input)
9. [国际化/主题 (TC-I18N)](#9-国际化主题-tc-i18n)
10. [导出/打印 (TC-EXPORT)](#10-导出打印-tc-export)
11. [面板管理 (TC-PANEL)](#11-面板管理-tc-panel)
12. [无障碍 (TC-A11Y)](#12-无障碍-tc-a11y)
13. [错误处理 (TC-ERR)](#13-错误处理-tc-err)
14. [视觉回归 (TC-VIS)](#14-视觉回归-tc-vis)
15. [性能 (TC-PERF)](#15-性能-tc-perf)
16. [边界/压力 (TC-EDGE)](#16-边界压力-tc-edge)
17. [端到端用户流程 (TC-E2E)](#17-端到端用户流程-tc-e2e)

---

## 1. 聊天核心 (TC-CHAT)

### 1.1 消息发送 (20 cases)

| ID | 场景 | 前置条件 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|---|
| TC-CHAT-001 | 发送消息显示用户气泡 | 知识库非空 | 1. 输入文本 2. Enter | `.msg-user` 可见 + 文本正确 | P0 |
| TC-CHAT-002 | AI 流式回复渲染 | Mock chat_token | 1. 发送消息 2. 等待流 | `.msg-assistant .md` 逐步增长 | P0 |
| TC-CHAT-003 | 流式完成后显示操作栏 | 流完成 | 1. waitForStreamDone | `.msg-actions` 可见 | P0 |
| TC-CHAT-004 | 流式完成后显示AI免责声明 | 流完成 | 1. waitForStreamDone | `.ai-disclaimer` 可见 + 文本非空 | P0 |
| TC-CHAT-005 | 点击发送按钮发送消息 | 知识库非空 | 1. 输入文本 2. 点击 #sendBtn | `.msg-user` 可见 | P0 |
| TC-CHAT-006 | 空白输入不发送 | 输入框为空 | 1. Enter | 无 `.msg-user` 元素 | P1 |
| TC-CHAT-007 | 空知识库显示错误 | count_documents=0 | 1. 输入文本 2. Enter | Toast 显示「知识库为空」 | P0 |
| TC-CHAT-008 | 空知识库发送按钮禁用 | count_documents=0 | 1. 检查 #sendBtn | `disabled` 属性 | P1 |
| TC-CHAT-009 | Shift+Enter 换行 | 输入框聚焦 | 1. 输入文本 2. Shift+Enter | 输入框包含换行符 | P1 |
| TC-CHAT-010 | Enter 发送不换行 | 输入框聚焦 | 1. 输入文本 2. Enter | 消息已发送 + 输入框清空 | P0 |
| TC-CHAT-011 | 流式中停止生成 | 流式进行中 | 1. 点击 #stopBtn | 流中断 + 部分内容保存 + 显示「⏹」 | P0 |
| TC-CHAT-012 | Esc 停止生成 | 流式进行中 | 1. 按 Esc | 流中断 + 部分内容保存 | P1 |
| TC-CHAT-013 | 流式中发送排队 | 流式进行中 | 1. 输入文本 2. Enter | 消息加入队列 + 队列计数+1 | P1 |
| TC-CHAT-014 | 流完成后队列自动发送 | 队列有1条 | 1. waitForStreamDone | 第二条消息自动发送 | P1 |
| TC-CHAT-015 | 队列满5条提示 | 队列5条 | 1. 第6次发送 | Toast「队列已满」 | P2 |
| TC-CHAT-016 | Markdown 基础渲染 | AI 回复含 `**bold**` | 1. waitForStreamDone | `<strong>` 元素存在 | P0 |
| TC-CHAT-017 | 代码块语法高亮 | AI 回复含 ```` ```js ```` | 1. waitForStreamDone 2. loadHighlight | `<code class="hljs">` 存在 | P1 |
| TC-CHAT-018 | Mermaid 图表渲染 | AI 回复含 mermaid | 1. waitForStreamDone 2. loadMermaid | `<svg>` 元素存在 | P2 |
| TC-CHAT-019 | KaTeX 数学公式渲染 | AI 回复含 `$E=mc^2$` | 1. waitForStreamDone 2. loadKaTeX | `.katex` 元素存在 | P2 |
| TC-CHAT-020 | 表格渲染 | AI 回复含 Markdown 表格 | 1. waitForStreamDone | `<table>` 元素存在 | P1 |

### 1.2 消息操作 (15 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-CHAT-021 | 复制 Markdown 原文 | 1. hover AI 消息 2. 点击复制按钮 | 剪贴板含 Markdown 原文 | P0 |
| TC-CHAT-022 | 编辑用户消息 | 1. hover 用户消息 2. 点击编辑按钮 | 进入编辑模式 + 文本可编辑 | P0 |
| TC-CHAT-023 | 编辑后创建新版本 | 1. 编辑 2. 发送 | 新版本 + 分页器显示 `2/2` | P1 |
| TC-CHAT-024 | 重新生成 AI 回复 | 1. hover AI 消息 2. 点击重新生成 | 新 AI 回复 + 分页器 `2/2` | P0 |
| TC-CHAT-025 | 版本切换（←） | 1. 点击分页器 ← | 显示上一版本内容 | P1 |
| TC-CHAT-026 | 版本切换（→） | 1. 点击分页器 → | 显示下一版本内容 | P1 |
| TC-CHAT-027 | 点赞反馈 | 1. 点击 👍 | 按钮高亮 + localStorage 存储 | P1 |
| TC-CHAT-028 | 点踩反馈弹原因 | 1. 点击 👎 | 原因选择弹框可见 | P1 |
| TC-CHAT-029 | 点踩选择原因 | 1. 点击 👎 2. 选择原因 | localStorage 存储原因 | P2 |
| TC-CHAT-030 | TTS 朗读消息 | 1. 点击喇叭按钮 | speechSynthesis.speak 被调用 | P2 |
| TC-CHAT-031 | 停止 TTS | 1. 切换会话 | speechSynthesis.cancel 被调用 | P2 |
| TC-CHAT-032 | 操作栏默认隐藏 | 1. 不 hover | `opacity: 0` | P2 |
| TC-CHAT-033 | 操作栏 hover 显示 | 1. hover AI 消息 | `opacity: 1` | P1 |
| TC-CHAT-034 | 导出单条消息 | 1. 点击导出按钮 | 下载文件 | P2 |
| TC-CHAT-035 | 多版本消息分页器显示 | 1. 重新生成 | `← 2/2 →` 可见 | P1 |

### 1.3 流式阶段 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-CHAT-036 | preparing 阶段显示 | 1. 发送消息 2. chat_phase preparing | 思维链面板「准备中…」 | P0 |
| TC-CHAT-037 | retrieving 阶段显示 | 1. chat_phase retrieving | 思维链面板「检索知识库…」 | P0 |
| TC-CHAT-038 | generating 阶段显示 | 1. chat_phase generating | 思维链面板「生成回答…」 | P0 |
| TC-CHAT-039 | 首个 token 最小延迟 | 1. 发送 2. 计时首 token | ≥ 200ms loading 展示 | P2 |
| TC-CHAT-040 | 思维链展开/折叠 | 1. 点击标题栏 | content hidden 切换 | P1 |
| TC-CHAT-041 | 思维链状态持久化 | 1. 展开 2. 切换会话 3. 切回 | 保持展开状态 | P2 |
| TC-CHAT-042 | chat_sources 事件渲染 | 1. chat_sources 事件 | 引用来源面板可见 | P0 |
| TC-CHAT-043 | chat_done 事件收尾 | 1. chat_done 事件 | 操作栏 + 免责声明 + 输入框恢复 | P0 |
| TC-CHAT-044 | chat_error 事件 | 1. chat_error 事件 | 错误详情卡片可见 | P0 |
| TC-CHAT-045 | 看门狗超时 | 1. 设 2000ms 超时 2. 发送 | 超时错误卡片可见 | P1 |

### 1.4 滚动行为 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-CHAT-046 | 新消息自动滚到底部 | 1. 在底部 2. 发送消息 | scrollTop = scrollHeight | P0 |
| TC-CHAT-047 | 用户上滚不自动跟随 | 1. 滚到中部 2. 新消息到达 | 不自动滚底 | P1 |
| TC-CHAT-048 | 「跳到最新」按钮显示 | 1. 滚到中部 2. 新消息到达 | `#jumpToLatest` 可见 | P1 |
| TC-CHAT-049 | 点击「跳到最新」 | 1. 点击 #jumpToLatest | 滚到底部 + 按钮消失 | P1 |
| TC-CHAT-050 | 「回到顶部」按钮显示 | 1. 滚下一段 2. 滚上 | `#backToTop` 可见 | P1 |
| TC-CHAT-051 | 点击「回到顶部」 | 1. 点击 #backToTop | 滚到顶部 + 按钮消失 | P1 |
| TC-CHAT-052 | 流式期间自动滚底 | 1. 在底部 2. token 流入 | scrollTop 跟随 | P0 |
| TC-CHAT-053 | 用户滚动停止自动跟随 | 1. 在底部 2. 用户上滚 3. token 流入 | 不自动滚底 | P1 |
| TC-CHAT-054 | 滚动锁定防止跳动 | 1. 流式中 2. 用户上滚 | 不跳动到底部 | P1 |
| TC-CHAT-055 | 长消息平滑滚动 | 1. 长回复 | 滚动流畅无卡顿 | P2 |

### 1.5 引用来源 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-CHAT-056 | 来源面板可折叠 | 1. 点击标题 | 列表 hidden 切换 | P0 |
| TC-CHAT-057 | 来源芯片显示 | 1. chat_sources | `.source-chip` 可见 | P0 |
| TC-CHAT-058 | 来源芯片含文档名 | 1. chat_sources | 文本含文档名 | P1 |
| TC-CHAT-059 | 来源芯片含分数 | 1. chat_sources | 文本含相似度分数 | P2 |
| TC-CHAT-060 | 点击来源滚动到chunk | 1. 点击芯片 | 滚动到 chunk + 高亮 | P1 |
| TC-CHAT-061 | 右键来源菜单 | 1. 右键芯片 | 显示「复制引用片段」 | P2 |
| TC-CHAT-062 | 来源芯片悬停tooltip | 1. hover 芯片 | 显示完整 chunk 内容 | P2 |
| TC-CHAT-063 | 无来源不显示面板 | 1. chat_sources 空 | 来源面板不显示 | P1 |
| TC-CHAT-064 | 多来源排序 | 1. 5 个来源 | 按分数降序 | P2 |
| TC-CHAT-065 | 来源芯片样式 | 1. chat_sources | accent 边框 + subtle 背景 | P2 |

### 1.6 关注建议 (5 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-CHAT-066 | Followup 建议显示 | 1. AI 回复完成 | `.followup` 可见 | P2 |
| TC-CHAT-067 | 点击建议填入输入框 | 1. 点击建议项 | 输入框含建议文本 | P2 |
| TC-CHAT-068 | 点击建议自动发送 | 1. 点击建议项 | 消息已发送 | P2 |
| TC-CHAT-069 | Followup 折叠 | 1. 点击折叠 | 列表 hidden | P3 |
| TC-CHAT-070 | 多条建议排列 | 1. 3 条建议 | 水平/垂直排列 | P3 |

---

## 2. 侧栏/导航 (TC-NAV)

### 2.1 侧栏折叠/展开 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-NAV-001 | ⌘B 折叠侧栏 | 1. 按 ⌘B | `#sidebar` translateX(-100%) | P0 |
| TC-NAV-002 | ⌘B 展开侧栏 | 1. 折叠后按 ⌘B | `#sidebar` 可见 | P0 |
| TC-NAV-003 | 折叠按钮点击 | 1. 点击折叠按钮 | 侧栏折叠 | P1 |
| TC-NAV-004 | 折叠动画时长 | 1. 计时 | 300ms ± 50ms | P2 |
| TC-NAV-005 | 折叠后main宽度 | 1. 折叠 | main padding-left = 0 | P1 |
| TC-NAV-006 | 展开后main宽度 | 1. 展开 | main padding-left = 260px | P1 |
| TC-NAV-007 | 折叠状态持久化 | 1. 折叠 2. 刷新 | 保持折叠 | P2 |
| TC-NAV-008 | 折叠动画底部锚定 | 1. 在底部 2. 折叠 | scrollTop 钉住 | P2 |
| TC-NAV-009 | 折叠动画中部锚定 | 1. 在中部 2. 折叠 | 视口内容不变 | P2 |
| TC-NAV-010 | 快速连点不竞态 | 1. 连点 5 次 | 最后一次生效 | P2 |

### 2.2 会话列表 (15 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-NAV-011 | 新建会话 | 1. ⌘N | 聊天区清空 + 输入框聚焦 | P0 |
| TC-NAV-012 | 新对话按钮 | 1. 点击 #newChatBtn | 同上 | P0 |
| TC-NAV-013 | 新对话快捷键提示 | 1. 检查 #newChatBtn | 含 `⌘ J` 文本 | P2 |
| TC-NAV-014 | 切换会话加载消息 | 1. 点击会话项 | 消息列表渲染 | P0 |
| TC-NAV-015 | 活跃会话高亮 | 1. 点击会话项 | `bg-accent-subtle` | P1 |
| TC-NAV-016 | 会话搜索过滤 | 1. 输入搜索词 | 仅显示匹配会话 | P1 |
| TC-NAV-017 | 搜索无匹配 | 1. 输入不存在词 | 显示「无匹配结果」 | P2 |
| TC-NAV-018 | 流式中切换会话 | 1. 流式中 2. 点击其他会话 | 流中断 + 内容保存 + 切换 | P0 |
| TC-NAV-019 | 会话标题自动提取 | 1. 首次发送 | 标题 = 前 24 字符 | P1 |
| TC-NAV-020 | 会话标题LLM生成 | 1. 首次发送 2. 等待 | 标题更新为 LLM 生成 | P2 |
| TC-NAV-021 | 会话列表排序 | 1. 检查列表 | 按更新时间降序 | P2 |
| TC-NAV-022 | 切换会话关闭面板 | 1. 打开设置 2. 切换会话 | 设置面板关闭 | P1 |
| TC-NAV-023 | 窗口标题更新 | 1. 切换会话 | document.title 含会话标题 | P2 |
| TC-NAV-024 | 切换会话保存草稿 | 1. 输入文本 2. 切换会话 | 草稿保存 | P1 |
| TC-NAV-025 | 切回恢复草稿 | 1. 切回会话 | 输入框恢复草稿 | P1 |

### 2.3 文档列表 (15 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-NAV-026 | 文档列表显示 | 1. 有导入文档 | 文档项可见 | P0 |
| TC-NAV-027 | 文档搜索过滤 | 1. 输入搜索词 | 仅显示匹配文档 | P1 |
| TC-NAV-028 | 文档状态徽标 | 1. 导入中文档 | 黄色徽标 | P1 |
| TC-NAV-029 | 已索引文档徽标 | 1. 已索引文档 | 蓝色徽标 | P1 |
| TC-NAV-030 | 失败文档徽标 | 1. 失败文档 | 红色徽标 | P1 |
| TC-NAV-031 | 点击文档打开预览 | 1. 点击文档名 | 预览面板可见 | P1 |
| TC-NAV-032 | 文档拖拽导入 | 1. 拖拽文件到侧栏 | 导入进度显示 | P0 |
| TC-NAV-033 | 文件选择器导入 | 1. 点击导入按钮 2. 选择文件 | 导入进度显示 | P0 |
| TC-NAV-034 | 不支持格式错误 | 1. 拖拽 .bin 文件 | Toast 错误 | P1 |
| TC-NAV-035 | 多文件批量导入 | 1. 拖拽 3 个文件 | 3 个文档项出现 | P1 |
| TC-NAV-036 | 导入进度事件 | 1. 导入大文件 | embedding_progress 事件 | P2 |
| TC-NAV-037 | 文档标签显示 | 1. 有标签文档 | 标签可见 | P2 |
| TC-NAV-038 | 文档大小显示 | 1. 检查文档项 | 文件大小可见 | P2 |
| TC-NAV-039 | 文档多选 | 1. Shift+↑/↓ | 多选高亮 | P2 |
| TC-NAV-040 | 批量删除文档 | 1. 多选 2. Delete | 确认对话框 → 删除 | P1 |

### 2.4 知识库选择器 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-NAV-041 | 选择器下拉展开 | 1. 点击 #workspaceToggle | 下拉列表可见 | P1 |
| TC-NAV-042 | 切换知识库 | 1. 选择其他 KB | 文档+会话列表刷新 | P0 |
| TC-NAV-043 | 新建知识库 | 1. 点击新建 2. 输入名称 | 新 KB 创建 + 切换 | P1 |
| TC-NAV-044 | 重命名知识库 | 1. 双击 KB 名 2. 编辑 | 名称更新 | P2 |
| TC-NAV-045 | 删除知识库 | 1. 删除按钮 2. 确认 | KB 删除 + 切换到 default | P1 |
| TC-NAV-046 | 删除显示数据量 | 1. 删除按钮 | 确认对话框显示文档数 | P2 |
| TC-NAV-047 | 切换持久化 | 1. 切换 KB 2. 刷新 | 恢复选择 | P2 |
| TC-NAV-048 | 配额显示 | 1. 检查选择器 | Free: N/50 | P2 |
| TC-NAV-049 | default KB 不可删 | 1. 尝试删除 default | 删除按钮禁用 | P2 |
| TC-NAV-050 | 切换关闭面板 | 1. 打开面板 2. 切换 KB | 面板关闭 | P2 |

### 2.5 书签 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-NAV-051 | 书签面板展开 | 1. 点击收藏夹标题 | 列表展开 | P2 |
| TC-NAV-052 | 添加书签 | 1. 点击星星按钮 | 书签列表更新 | P1 |
| TC-NAV-053 | 移除书签 | 1. 再次点击星星 | 书签列表更新 | P1 |
| TC-NAV-054 | 点击书签跳转 | 1. 点击书签项 | 切换到对应会话 | P1 |
| TC-NAV-055 | 右键书签菜单 | 1. 右键书签项 | 菜单可见 | P2 |
| TC-NAV-056 | 编辑备注 | 1. 右键→编辑备注 | 输入框出现 | P3 |
| TC-NAV-057 | 书签折叠 | 1. 点击标题 | 列表折叠 | P2 |
| TC-NAV-058 | 空书签 | 1. 无书签 | 显示空文本 | P2 |
| TC-NAV-059 | 书签排序 | 1. 检查列表 | 按创建时间降序 | P3 |
| TC-NAV-060 | 刷新书签 | 1. 添加书签后 | 列表自动刷新 | P2 |

---

## 3. 设置面板 (TC-SET)

### 3.1 面板交互 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-SET-001 | ⌘, 打开设置 | 1. 按 ⌘, | 面板可见 | P0 |
| TC-SET-002 | Esc 关闭设置 | 1. 按 Esc | 面板隐藏 | P0 |
| TC-SET-003 | 点击遮罩关闭 | 1. 点击遮罩 | 面板隐藏 | P1 |
| TC-SET-004 | Focus Trap 激活 | 1. 打开设置 2. Tab | Tab 在面板内循环 | P1 |
| TC-SET-005 | Tab 切换 | 1. 点击 Tab 2 | 内容区更新 | P0 |
| TC-SET-006 | Tab scroll-padding | 1. 切换 Tab | scroll-padding-top: 52px | P2 |
| TC-SET-007 | 面板 scaleIn 动画 | 1. 打开 | 动画 250ms | P2 |
| TC-SET-008 | 设置即保存 | 1. 修改设置 | IPC 调用 | P1 |
| TC-SET-009 | 保存 Toast 反馈 | 1. 保存设置 | Toast 成功可见 | P2 |
| TC-SET-010 | 重置参数 | 1. 点击重置 | 恢复默认 + Toast | P2 |

### 3.2 LLM 配置 (15 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-SET-011 | API Key 输入 | 1. 输入 key | IPC 保存 | P0 |
| TC-SET-012 | API Key 脱敏 | 1. 保存后重开 | `****` + 后 4 位 | P0 |
| TC-SET-013 | Base URL 输入 | 1. 输入 URL | IPC 保存 | P0 |
| TC-SET-014 | Model 输入 | 1. 输入模型名 | IPC 保存 | P0 |
| TC-SET-015 | 预设选择 | 1. 点击预设卡片 | 字段自动填充 | P1 |
| TC-SET-016 | 本地 LLM 模式切换 | 1. 切换到 Local | Free: 拒绝 + 付费墙 | P0 |
| TC-SET-017 | 本地模型列表 | 1. Local 模式 | 模型列表可见 | P1 |
| TC-SET-018 | 模型下载 | 1. 点击下载 | 进度条显示 | P1 |
| TC-SET-019 | 模型删除 | 1. 删除按钮 | 模型移除 | P2 |
| TC-SET-020 | GPU 加速选项 | 1. Pro 模式 | GPU toggle 可见 | P2 |
| TC-SET-021 | 采样参数 | 1. 调整 temperature | 值更新 | P2 |
| TC-SET-022 | 采样参数重置 | 1. 点击重置 | 恢复默认 | P2 |
| TC-SET-023 | KV Cache 设置 | 1. Pro 模式 | KV cache toggle | P3 |
| TC-SET-024 | PagedAttention | 1. Pro 模式 | PA toggle | P3 |
| TC-SET-025 | 模型指示器更新 | 1. 修改 model | 顶栏 pill 更新 | P1 |

### 3.3 嵌入模型 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-SET-026 | 模型选择器 | 1. 点击下拉 | 5 个模型选项 | P0 |
| TC-SET-027 | 切换嵌入模型 | 1. 选择 BgeM3 | IPC 保存 | P0 |
| TC-SET-028 | 模型缓存信息 | 1. 检查 | 缓存大小可见 | P2 |
| TC-SET-029 | 清空模型缓存 | 1. 点击清除 | Toast 确认 | P2 |
| TC-SET-030 | 自定义模型上传 | 1. Pro 2. 上传 ONNX | 上传成功 | P1 |
| TC-SET-031 | 自定义模型切换 | 1. 选择自定义 | 切换成功 | P1 |
| TC-SET-032 | 自定义模型删除 | 1. 删除 | 移除 | P2 |
| TC-SET-033 | Late Chunking toggle | 1. 切换 | IPC 保存 | P2 |
| TC-SET-034 | 全库嵌入重建 | 1. 点击重建 | 进度显示 | P1 |
| TC-SET-035 | Contextual Retrieval | 1. toggle | IPC 保存 | P2 |

### 3.4 安全隐私 (15 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-SET-036 | 加密数据库 | 1. 输入密码 2. 确认 | 状态→EncryptedUnlocked | P0 |
| TC-SET-037 | 解锁数据库 | 1. 输入密码 | 状态→EncryptedUnlocked | P0 |
| TC-SET-038 | 错误密码 | 1. 输入错误密码 | 错误提示 | P0 |
| TC-SET-039 | 暴力破解退避 | 1. 5 次错误 | 退避 30s | P1 |
| TC-SET-040 | 自动锁屏设置 | 1. 设置超时 | IPC 保存 | P1 |
| TC-SET-041 | PII 检测开关 | 1. toggle | IPC 保存 | P1 |
| TC-SET-042 | PII 检测运行 | 1. 输入含邮箱文本 | `[REDACTED-EMAIL]` | P1 |
| TC-SET-043 | 8 种 PII 类型 | 1. 逐项测试 | 8 类全部检测 | P1 |
| TC-SET-044 | 剪贴板清除设置 | 1. 设置超时 | IPC 保存 | P2 |
| TC-SET-045 | 剪贴板清除执行 | 1. 复制敏感内容 2. 等待 | 剪贴板清空 | P2 |
| TC-SET-046 | 审计日志查看 | 1. 打开审计日志 | 日志列表可见 | P2 |
| TC-SET-047 | 审计链验证 | 1. 点击验证 | 验证结果 | P2 |
| TC-SET-048 | 审计日志导出 | 1. 点击导出 | 文件下载 | P3 |
| TC-SET-049 | 紧急销毁密码 | 1. 输入销毁密码 | 数据库销毁 | P1 |
| TC-SET-050 | 密码强度评估 | 1. 输入弱密码 | 强度提示 | P2 |

### 3.5 智能检索 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-SET-051 | 智能模式开关 | 1. toggle | 6 个开关批量设置 | P0 |
| TC-SET-052 | 混合搜索 toggle | 1. toggle | IPC 保存 | P0 |
| TC-SET-053 | Agent toggle | 1. toggle | IPC 保存 | P0 |
| TC-SET-054 | 知识图谱 toggle | 1. toggle | IPC 保存 | P1 |
| TC-SET-055 | 质量门控 toggle | 1. toggle | IPC 保存 | P1 |
| TC-SET-056 | 压缩比滑块 | 1. 拖拽到 3.0 | IPC 保存 | P1 |
| TC-SET-057 | 检索记忆 toggle | 1. toggle | IPC 保存 | P2 |
| TC-SET-058 | VLM toggle | 1. toggle | 确认弹框 | P2 |
| TC-SET-059 | Rerank toggle | 1. toggle | IPC 保存 | P1 |
| TC-SET-060 | HyDE toggle | 1. toggle | IPC 保存 | P1 |

### 3.6 高级调优 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-SET-061 | 缓存统计展示 | 1. 打开高级调优 | 命中率可见 | P2 |
| TC-SET-062 | 清空缓存 | 1. 点击清空 2. 确认 | Toast 确认 | P2 |
| TC-SET-063 | 渐进式注入 toggle | 1. toggle | IPC 保存 | P2 |
| TC-SET-064 | Speculative RAG toggle | 1. toggle | IPC 保存 | P2 |
| TC-SET-065 | Coordinator toggle | 1. toggle | IPC 保存 | P3 |
| TC-SET-066 | SubAgent toggle | 1. toggle | IPC 保存 | P3 |
| TC-SET-067 | Token 预算设置 | 1. 修改 max_tokens | IPC 保存 | P2 |
| TC-SET-068 | 文件夹同步添加 | 1. 点击添加 2. 选择文件夹 | 文件夹列表更新 | P1 |
| TC-SET-069 | 文件夹同步移除 | 1. 点击移除 | 文件夹列表更新 | P2 |
| TC-SET-070 | 对话模板管理 | 1. 新增模板 2. 编辑 3. 删除 | CRUD 正常 | P1 |

### 3.7 外观语言 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-SET-071 | 暗色主题（默认） | 1. 检查 | `data-theme="dark"` | P0 |
| TC-SET-072 | 浅色主题切换 | 1. 点击 Light | `data-theme="light"` | P0 |
| TC-SET-073 | 系统跟随主题 | 1. 点击 System | `data-theme="system"` | P1 |
| TC-SET-074 | 主题防闪烁 | 1. 切换主题 | 双 rAF 无闪烁 | P2 |
| TC-SET-075 | Mermaid 主题同步 | 1. 切换主题 | Mermaid 主题更新 | P2 |
| TC-SET-076 | 语言切换 | 1. 切换到 zh-CN | UI 文本更新 | P0 |
| TC-SET-077 | 语言持久化 | 1. 切换 2. 刷新 | 恢复语言 | P1 |
| TC-SET-078 | i18n 缺失回退 | 1. 缺失 key | 回退到 en | P2 |
| TC-SET-079 | 备份导出 | 1. 点击导出备份 | 文件下载 | P2 |
| TC-SET-080 | 备份恢复 | 1. 点击恢复 2. 选择文件 | 数据恢复 + Toast | P2 |

### 3.8 应用设置 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-SET-081 | 窗口置顶 toggle | 1. toggle | IPC 保存 | P2 |
| TC-SET-082 | 开机自启 toggle | 1. toggle | IPC 保存 | P2 |
| TC-SET-083 | 诊断信息导出 | 1. 点击导出 | 文件下载 | P3 |
| TC-SET-084 | 日志导出 | 1. 点击导出 | 文件下载 | P3 |
| TC-SET-085 | 错误日志查看 | 1. 打开日志 | 日志列表可见 | P2 |
| TC-SET-086 | RAG 参数 | 1. 修改 top_k | IPC 保存 | P2 |
| TC-SET-087 | LLM 参数 | 1. 修改 max_tokens | IPC 保存 | P2 |
| TC-SET-088 | 内核模式 | 1. Pro 2. 切换 KernelMode | IPC 保存 | P3 |
| TC-SET-089 | 设备类型 | 1. Pro 2. 检查 device_kind | 显示 GPU 类型 | P3 |
| TC-SET-090 | 镜像源选择 | 1. 选择镜像 | IPC 保存 | P2 |

---

## 4. 文档管理 (TC-DOC)

### 4.1 文档导入 (20 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-DOC-001 | Markdown 导入 | 1. 导入 .md | 文档项出现 + Indexed | P0 |
| TC-DOC-002 | Text 导入 | 1. 导入 .txt | 文档项出现 + Indexed | P0 |
| TC-DOC-003 | PDF 导入 (Pro) | 1. Pro 2. 导入 .pdf | 文档项出现 + Indexed | P0 |
| TC-DOC-004 | DOCX 导入 | 1. 导入 .docx | 文档项出现 + Indexed | P0 |
| TC-DOC-005 | HTML 导入 | 1. 导入 .html | 文档项出现 + Indexed | P0 |
| TC-DOC-006 | PPTX 导入 | 1. 导入 .pptx | 文档项出现 + Indexed | P1 |
| TC-DOC-007 | EPUB 导入 | 1. 导入 .epub | 文档项出现 + Indexed | P1 |
| TC-DOC-008 | XLSX 导入 | 1. 导入 .xlsx | 文档项出现 + Indexed | P1 |
| TC-DOC-009 | CSV 导入 | 1. 导入 .csv | 文档项出现 + Indexed | P1 |
| TC-DOC-010 | PDF Free 拒绝 | 1. Free 2. 导入 .pdf | 付费墙弹出 | P0 |
| TC-DOC-011 | 重复文件检测 | 1. 导入相同 MD5 | Toast「文档已存在」 | P1 |
| TC-DOC-012 | 导入进度显示 | 1. 导入大文件 | 进度条可见 | P1 |
| TC-DOC-013 | 导入成功 Toast | 1. 导入完成 | Toast 成功 | P1 |
| TC-DOC-014 | 导入失败 Toast | 1. 导入损坏文件 | Toast 错误 | P1 |
| TC-DOC-015 | 批量导入 | 1. 导入 5 个文件 | 5 个文档项 | P1 |
| TC-DOC-016 | 取消导入 | 1. 导入中 2. 点击取消 | 导入中止 | P2 |
| TC-DOC-017 | 崩溃恢复 | 1. Processing 状态重启 | 标记为 Failed | P1 |
| TC-DOC-018 | 文档摘要生成 | 1. 导入后 | 摘要自动生成 | P2 |
| TC-DOC-019 | Wiki 链接索引 | 1. 导入含 `[[link]]` | wiki_links 表更新 | P2 |
| TC-DOC-020 | 实体抽取 | 1. 导入后 | entities 表更新 | P2 |

### 4.2 文档操作 (15 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-DOC-021 | 文档预览 | 1. 点击文档名 | 预览面板可见 | P0 |
| TC-DOC-022 | 预览元数据 | 1. 打开预览 | 文件名/格式/大小/时间可见 | P1 |
| TC-DOC-023 | 预览原文 | 1. 打开预览 | 前 500 字可见 | P1 |
| TC-DOC-024 | 预览 Chunk | 1. 打开预览 | Chunk 列表可见 | P2 |
| TC-DOC-025 | 单文档删除 | 1. 右键→删除 2. 确认 | 文档移除 | P0 |
| TC-DOC-026 | 批量删除 | 1. 多选 2. Delete 3. 确认 | 全部移除 | P1 |
| TC-DOC-027 | 文档标签添加 | 1. 右键→添加标签 | 标签可见 | P2 |
| TC-DOC-028 | 文档标签删除 | 1. 右键→删除标签 | 标签移除 | P2 |
| TC-DOC-029 | 文档导出 HTML | 1. 右键→导出 HTML | 文件下载 | P2 |
| TC-DOC-030 | 文档导出 PDF | 1. 右键→导出 PDF | 打印对话框 | P2 |
| TC-DOC-031 | 文档审计 | 1. 右键→审计 | 审计进度显示 | P2 |
| TC-DOC-032 | Free 配额限制 | 1. 50 个文档 2. 导入 | 配额错误 | P0 |
| TC-DOC-033 | 文档搜索 | 1. 输入搜索词 | 过滤结果 | P1 |
| TC-DOC-034 | 文档排序 | 1. 检查排序 | 按导入时间降序 | P2 |
| TC-DOC-035 | 文件夹同步 | 1. 添加监听文件夹 | 自动同步 | P1 |

### 4.3 文档预览面板 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-DOC-036 | 面板打开 | 1. 点击文档名 | overlay 可见 | P0 |
| TC-DOC-037 | 面板关闭 | 1. Esc | overlay 隐藏 | P0 |
| TC-DOC-038 | Focus Trap | 1. 打开 2. Tab | Tab 在面板内循环 | P1 |
| TC-DOC-039 | 元数据完整 | 1. 打开 | 6 项元数据 | P1 |
| TC-DOC-040 | 原文截断 | 1. 长文档 | 显示前 500 字 | P2 |
| TC-DOC-041 | Chunk 列表 | 1. Indexed 文档 | Chunk 列表可见 | P2 |
| TC-DOC-042 | 未索引无 Chunk | 1. Pending 文档 | 无 Chunk 列表 | P2 |
| TC-DOC-043 | 点击遮罩关闭 | 1. 点击遮罩 | 面板关闭 | P1 |
| TC-DOC-044 | 滚动 | 1. 长内容 | 可滚动 | P2 |
| TC-DOC-045 | 面板 Z-index | 1. 检查 | z-index: 75 | P3 |

---

## 5. 安全/加密 (TC-SEC)

### 5.1 数据库加密 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-SEC-001 | 加密数据库 | 1. 输入密码 2. 确认 | 状态→EncryptedUnlocked | P0 |
| TC-SEC-002 | 密码强度要求 | 1. 输入弱密码 | 拒绝 + 强度提示 | P0 |
| TC-SEC-003 | Argon2id KDF | 1. 加密 | Argon2id 派生密钥 | P1 |
| TC-SEC-004 | SQLCipher AES-256 | 1. 加密 | PRAGMA key 生效 | P1 |
| TC-SEC-005 | SecureString 清除 | 1. 加密后 | 密码从内存清除 | P1 |
| TC-SEC-006 | 解锁正确密码 | 1. 输入正确密码 | 状态→EncryptedUnlocked | P0 |
| TC-SEC-007 | 错误密码 | 1. 输入错误密码 | 错误提示 | P0 |
| TC-SEC-008 | 暴力破解 5 次 | 1. 5 次错误 | 退避 30s | P0 |
| TC-SEC-009 | 指数退避 | 1. 6/7/8 次错误 | 60s/120s/240s | P1 |
| TC-SEC-010 | 锁屏遮罩 | 1. 自动锁屏 | lock-overlay 可见 | P0 |

### 5.2 自动锁屏 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-SEC-011 | 自动锁屏触发 | 1. 闲置超时 | 状态→Locked | P0 |
| TC-SEC-012 | 活动重置计时 | 1. record_activity | 计时重置 | P1 |
| TC-SEC-013 | 锁屏遮罩最高 Z | 1. 锁屏 | z-index: 99999 | P1 |
| TC-SEC-014 | 锁屏不可 ESC 关闭 | 1. 锁屏 2. Esc | 不关闭 | P0 |
| TC-SEC-015 | 锁屏 Focus Trap | 1. 锁屏 2. Tab | Tab 在锁屏内循环 | P1 |
| TC-SEC-016 | 锁屏后解锁 | 1. 输入密码 2. Enter | 状态→EncryptedUnlocked | P0 |
| TC-SEC-017 | 超时设置 | 1. 修改超时 | IPC 保存 | P2 |
| TC-SEC-018 | 系统唤醒锁 | 1. 聊天中 | 系统不睡眠 | P2 |
| TC-SEC-019 | security-state 事件 | 1. 状态变化 | 事件触发 | P1 |
| TC-SEC-020 | 锁屏密码输入框 | 1. 锁屏 2. 检查 | `#lockPasswordInput` 聚焦 | P1 |

### 5.3 PII 检测 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-SEC-021 | 邮箱检测 | 1. 输入含邮箱 | `[REDACTED-EMAIL]` | P0 |
| TC-SEC-022 | 手机号检测 | 1. 输入含手机号 | `[REDACTED-PHONE]` | P0 |
| TC-SEC-023 | 身份证检测 | 1. 输入含身份证 | `[REDACTED-IDCARD]` | P0 |
| TC-SEC-024 | 银行卡检测 | 1. 输入含银行卡 | `[REDACTED-BANKCARD]` | P0 |
| TC-SEC-025 | IP 地址检测 | 1. 输入含 IP | `[REDACTED-IP]` | P1 |
| TC-SEC-026 | SSN 检测 | 1. 输入含 SSN | `[REDACTED-SSN]` | P1 |
| TC-SEC-027 | 护照检测 | 1. 输入含护照 | `[REDACTED-PASSPORT]` | P1 |
| TC-SEC-028 | 国际电话检测 | 1. 输入含国际号码 | `[REDACTED-INTL_PHONE]` | P1 |
| TC-SEC-029 | PII toggle | 1. 关闭 PII | 不检测 | P1 |
| TC-SEC-030 | 审计日志记录 | 1. PII 检测 | audit_logs 表更新 | P2 |

### 5.4 剪贴板清除 (5 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-SEC-031 | 剪贴板自动清除 | 1. 复制 2. 等待超时 | 剪贴板清空 | P1 |
| TC-SEC-032 | 清除超时设置 | 1. 修改超时 | IPC 保存 | P2 |
| TC-SEC-033 | 手动清除 | 1. 点击清除 | 剪贴板清空 | P2 |
| TC-SEC-034 | 跨平台支持 | 1. macOS/Linux/Windows | 平台适配 | P1 |
| TC-SEC-035 | Wayland 支持 | 1. Linux Wayland | wl-copy 检测 | P2 |

---

## 6. 知识图谱 (TC-GRAPH)

### 6.1 图谱查看器 (20 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-GRAPH-001 | 打开图谱查看器 | 1. 工具菜单→知识图谱 | overlay 可见 | P0 |
| TC-GRAPH-002 | 关闭图谱查看器 | 1. Esc | overlay 隐藏 | P0 |
| TC-GRAPH-003 | D3 force simulation | 1. 打开 | SVG 节点+连线渲染 | P0 |
| TC-GRAPH-004 | 节点拖拽 | 1. 拖拽节点 | 节点跟随 + 重新布局 | P1 |
| TC-GRAPH-005 | 缩放 | 1. 滚轮 | transform scale 变化 | P1 |
| TC-GRAPH-006 | 平移 | 1. 拖拽空白 | transform translate 变化 | P1 |
| TC-GRAPH-007 | 节点点击高亮 | 1. 点击节点 | 节点高亮 + 实体详情 | P1 |
| TC-GRAPH-008 | 搜索定位 | 1. 输入 + Enter | 节点居中 + 缩放 | P2 |
| TC-GRAPH-009 | 导出 SVG | 1. 点击导出 SVG | SVG 文件下载 | P2 |
| TC-GRAPH-010 | 导出 PNG | 1. 点击导出 PNG | PNG 文件下载 | P2 |
| TC-GRAPH-011 | 导出 GraphML | 1. 点击导出 | GraphML 文件下载 | P2 |
| TC-GRAPH-012 | 导出 JSON-LD | 1. 点击导出 | JSON-LD 文件下载 | P2 |
| TC-GRAPH-013 | 布局切换 force | 1. 点击 force | force 布局 | P2 |
| TC-GRAPH-014 | 布局切换 hierarchical | 1. 点击 hierarchical | 层次布局 | P2 |
| TC-GRAPH-015 | 布局切换 radial | 1. 点击 radial | 放射布局 | P2 |
| TC-GRAPH-016 | 路径分析 | 1. 选起止 2. 查找 | BFS 最短路径高亮 | P2 |
| TC-GRAPH-017 | 社区检测 | 1. 点击社区 | 10 色着色 | P2 |
| TC-GRAPH-018 | 子图过滤 | 1. 取消勾选 | 对应关系隐藏 | P2 |
| TC-GRAPH-019 | 重置视图 | 1. 点击重置 | 回到初始视图 | P2 |
| TC-GRAPH-020 | Focus Trap | 1. 打开 2. Tab | Tab 在面板内循环 | P1 |

### 6.2 实体/关系 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-GRAPH-021 | 实体类型图标 | 1. 检查节点 | 5 种类型 SVG 图标 | P2 |
| TC-GRAPH-022 | 关系颜色编码 | 1. 检查连线 | 8 种颜色 | P2 |
| TC-GRAPH-023 | 实体详情面板 | 1. 点击节点 | 详情可见 | P1 |
| TC-GRAPH-024 | 图例显示 | 1. 检查图例 | 关系类型图例可见 | P2 |
| TC-GRAPH-025 | 空图谱 | 1. 无实体 | 空状态 | P2 |
| TC-GRAPH-026 | 大图谱性能 | 1. 100+ 节点 | 无卡顿 | P2 |
| TC-GRAPH-027 | 图谱 Z-index | 1. 检查 | z-index: 90 | P3 |
| TC-GRAPH-028 | 全屏模式 | 1. 打开 | 全屏覆盖 | P2 |
| TC-GRAPH-029 | ESC 关闭 | 1. Esc | 面板关闭 | P0 |
| TC-GRAPH-030 | 节点 hover | 1. hover 节点 | 高亮 + tooltip | P2 |

---

## 7. Agentic AI 面板 (TC-AGENT)

### 7.1 AutoDream (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-AGENT-001 | 打开 Dream 面板 | 1. 工具菜单→AutoDream | overlay 可见 | P0 |
| TC-AGENT-002 | 关闭 Dream 面板 | 1. Esc | overlay 隐藏 | P0 |
| TC-AGENT-003 | 触发分析 | 1. 点击触发 | trigger_dream IPC | P1 |
| TC-AGENT-004 | 进度条显示 | 1. dream_progress 事件 | 进度条更新 | P1 |
| TC-AGENT-005 | 中止分析 | 1. 点击中止 | abort_dream IPC | P1 |
| TC-AGENT-006 | 历史建议 | 1. 打开面板 | 建议列表可见 | P2 |
| TC-AGENT-007 | 严重等级颜色 | 1. 检查建议 | high/medium/low 颜色 | P2 |
| TC-AGENT-008 | Focus Trap | 1. 打开 2. Tab | Tab 在面板内循环 | P1 |
| TC-AGENT-009 | 空建议 | 1. 无历史 | 空状态 | P2 |
| TC-AGENT-010 | 面板 Z-index | 1. 检查 | z-index: 50 | P3 |

### 7.2 符号搜索 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-AGENT-011 | 打开符号搜索 | 1. 工具菜单→符号搜索 | overlay 可见 | P0 |
| TC-AGENT-012 | 关闭符号搜索 | 1. Esc | overlay 隐藏 | P0 |
| TC-AGENT-013 | 搜索符号 | 1. 输入文本 | 过滤结果 | P1 |
| TC-AGENT-014 | 重建索引 | 1. 点击重建 | IPC + Toast | P2 |
| TC-AGENT-015 | 符号列表 | 1. 打开 | 符号列表可见 | P1 |
| TC-AGENT-016 | 符号类型 | 1. 检查 | 函数/类/变量 | P2 |
| TC-AGENT-017 | Focus Trap | 1. 打开 2. Tab | Tab 在面板内循环 | P1 |
| TC-AGENT-018 | 搜索框聚焦 | 1. 打开 | 搜索框自动聚焦 | P1 |
| TC-AGENT-019 | 空符号 | 1. 无符号 | 空状态 | P2 |
| TC-AGENT-020 | 点击跳转 | 1. 点击符号 | 跳转到文档 | P2 |

### 7.3 记忆管理 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-AGENT-021 | 记忆面板渲染 | 1. 设置→记忆 | 面板可见 | P0 |
| TC-AGENT-022 | Tab 切换 | 1. 点击 Hall Tab | Hall 列表显示 | P1 |
| TC-AGENT-023 | 记忆开关 | 1. toggle | IPC 保存 | P1 |
| TC-AGENT-024 | 提升记忆 | 1. 点击提升 | promote IPC + tier 更新 | P2 |
| TC-AGENT-025 | 删除记忆 | 1. 点击删除 2. 确认 | delete IPC | P2 |
| TC-AGENT-026 | 清空 tier | 1. 点击清空 2. 确认 | clear IPC | P2 |
| TC-AGENT-027 | 置顶记忆 | 1. 点击置顶 | pin IPC | P3 |
| TC-AGENT-028 | 记忆计数 | 1. 检查 Tab | 每个 tier 显示计数 | P2 |
| TC-AGENT-029 | 重要性进度条 | 1. 检查卡片 | 进度条可见 | P2 |
| TC-AGENT-030 | 空记忆 | 1. 无记忆 | 空状态 | P2 |

### 7.4 对话分支树 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-AGENT-031 | 打开分支树 | 1. 工具菜单→分支树 | overlay 可见 | P0 |
| TC-AGENT-032 | 关闭分支树 | 1. Esc | overlay 隐藏 | P0 |
| TC-AGENT-033 | 树形结构 | 1. 打开 | 缩进式列表 | P1 |
| TC-AGENT-034 | 版本切换 | 1. 点击版本 | 活跃版本更新 | P1 |
| TC-AGENT-035 | 活跃版本标记 | 1. 检查 | accent 色 | P2 |
| TC-AGENT-036 | 非活跃灰色 | 1. 检查 | 灰色 | P2 |
| TC-AGENT-037 | Focus Trap | 1. 打开 2. Tab | Tab 在面板内循环 | P1 |
| TC-AGENT-038 | 空分支树 | 1. 无版本 | 空状态 | P2 |
| TC-AGENT-039 | 多版本显示 | 1. 3+ 版本 | 全部可见 | P2 |
| TC-AGENT-040 | 面板 Z-index | 1. 检查 | z-index: 50 | P3 |

---

## 8. 输入区/快捷键 (TC-INPUT)

### 8.1 输入区 Toggle (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-INPUT-001 | 混合搜索 toggle | 1. 点击 | active 状态切换 + IPC | P0 |
| TC-INPUT-002 | Agent toggle | 1. 点击 | active 状态切换 + IPC | P0 |
| TC-INPUT-003 | Toggle 默认灰色 | 1. 检查 | inactive 样式 | P1 |
| TC-INPUT-004 | Toggle 激活 accent | 1. 点击 | accent 色 + 微染背景 | P1 |
| TC-INPUT-005 | Toggle tooltip | 1. hover | tooltip 可见 | P2 |
| TC-INPUT-006 | 5 个 Toggle 显示 | 1. 检查 #inputToggles | 5 个 .input-toggle | P1 |
| TC-INPUT-007 | Toggle 状态持久化 | 1. 切换 2. 刷新 | 恢复状态 | P2 |
| TC-INPUT-008 | /agent 斜杠命令 | 1. 输入 /agent | 临时启用 Agent | P2 |
| TC-INPUT-009 | /web 斜杠命令 | 1. 输入 /web | 临时启用网页搜索 | P2 |
| TC-INPUT-010 | Toggle 流式中禁用 | 1. 流式中 | toggle 不可点击 | P1 |

### 8.2 斜杠命令 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-INPUT-011 | 斜杠命令面板 | 1. 输入 / | 面板可见 | P0 |
| TC-INPUT-012 | 命令过滤 | 1. 输入 /s | 过滤结果 | P1 |
| TC-INPUT-013 | ↑/↓ 导航 | 1. ↓ 2. ↑ | 高亮切换 | P1 |
| TC-INPUT-014 | Enter 执行 | 1. Enter | 命令展开到输入框 | P0 |
| TC-INPUT-015 | Esc 关闭 | 1. Esc | 面板关闭 | P1 |
| TC-INPUT-016 | Home/End | 1. Home | 跳到首项 | P2 |
| TC-INPUT-017 | Backspace 关闭 | 1. Backspace 到空 | 面板关闭 | P2 |
| TC-INPUT-018 | 自定义命令 | 1. /custom | 自定义模板展开 | P2 |
| TC-INPUT-019 | 命令展开 {query} | 1. /summary query | 模板替换 | P2 |
| TC-INPUT-020 | 系统命令 6 个 | 1. 输入 / | 6 个系统命令可见 | P1 |

### 8.3 文档提及 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-INPUT-021 | @ 弹窗 | 1. 输入 @ | 弹窗可见 | P0 |
| TC-INPUT-022 | 文档过滤 | 1. @doc | 过滤结果 | P1 |
| TC-INPUT-023 | ↑/↓ 导航 | 1. ↓ 2. ↑ | 高亮切换 | P1 |
| TC-INPUT-024 | Enter 插入 | 1. Enter | 文档提及插入 | P0 |
| TC-INPUT-025 | Esc 关闭 | 1. Esc | 弹窗关闭 | P1 |
| TC-INPUT-026 | 文件类型图标 | 1. 检查 | 类型图标可见 | P2 |
| TC-INPUT-027 | 多文档提及 | 1. @doc1 @doc2 | 两个提及 | P2 |
| TC-INPUT-028 | 提及高亮 | 1. 检查 | accent 色 | P2 |
| TC-INPUT-029 | 空文档 | 1. @ + 无文档 | 空状态 | P2 |
| TC-INPUT-030 | 提及删除 | 1. Backspace | 提及移除 | P2 |

### 8.4 输入历史 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-INPUT-031 | ↑ 历史导航 | 1. ↑ | 显示上一条输入 | P1 |
| TC-INPUT-032 | ↓ 历史导航 | 1. ↓ | 显示下一条输入 | P1 |
| TC-INPUT-033 | 历史循环 | 1. ↑↑↑ | 循环到最后一条 | P2 |
| TC-INPUT-034 | 草稿保存 | 1. 输入 2. 切换会话 | 草稿保存 | P1 |
| TC-INPUT-035 | 草稿恢复 | 1. 切回会话 | 输入框恢复 | P1 |
| TC-INPUT-036 | Token 估算 | 1. 输入文本 | token 数可见 | P2 |
| TC-INPUT-037 | 历史会话级 | 1. 切换会话 | 不同历史 | P2 |
| TC-INPUT-038 | 空历史 | 1. ↑ | 无反应 | P2 |
| TC-INPUT-039 | Escape 重置 | 1. ↑ 2. Esc | 恢复当前输入 | P2 |
| TC-INPUT-040 | 草稿清空 | 1. 发送消息 | 草稿清空 | P2 |

### 8.5 全局快捷键 (15 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-INPUT-041 | ⌘K 命令面板 | 1. ⌘K | 命令面板可见 | P0 |
| TC-INPUT-042 | ⌘N 新建会话 | 1. ⌘N | 新会话 | P0 |
| TC-INPUT-043 | ⌘O 导入文件 | 1. ⌘O | 文件选择器 | P0 |
| TC-INPUT-044 | ⌘, 设置 | 1. ⌘, | 设置面板可见 | P0 |
| TC-INPUT-045 | ⌘B 侧栏 | 1. ⌘B | 侧栏折叠/展开 | P0 |
| TC-INPUT-046 | ⌘/ 帮助 | 1. ⌘/ | 帮助面板可见 | P1 |
| TC-INPUT-047 | ⌘⇧F 全局搜索 | 1. ⌘⇧F | 全局搜索可见 | P0 |
| TC-INPUT-048 | ⌘E 导出 | 1. ⌘E | 导出菜单 | P1 |
| TC-INPUT-049 | Esc 关闭面板 | 1. 打开面板 2. Esc | 面板关闭 | P0 |
| TC-INPUT-050 | Enter 发送 | 1. Enter | 消息发送 | P0 |
| TC-INPUT-051 | Shift+Enter 换行 | 1. Shift+Enter | 换行 | P1 |
| TC-INPUT-052 | ↑/↓ 历史导航 | 1. ↑ | 历史显示 | P1 |
| TC-INPUT-053 | / 斜杠命令 | 1. / | 面板可见 | P0 |
| TC-INPUT-054 | @ 文档提及 | 1. @ | 弹窗可见 | P0 |
| TC-INPUT-055 | IME 组合不发送 | 1. IME 组合中 2. Enter | 不发送 | P0 |

### 8.6 IME 防护 (5 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-INPUT-056 | 中文 IME 不发送 | 1. isComposing | 不发送 | P0 |
| TC-INPUT-057 | 日文 IME 不发送 | 1. isComposing | 不发送 | P0 |
| TC-INPUT-058 | keyCode 229 | 1. keyCode 229 | 不触发快捷键 | P1 |
| TC-INPUT-059 | IME 结束后发送 | 1. 组合结束 2. Enter | 发送 | P1 |
| TC-INPUT-060 | IME 不触发斜杠 | 1. 组合中 / | 不触发 | P1 |

---

## 9. 国际化/主题 (TC-I18N)

### 9.1 国际化 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-I18N-001 | 英文默认 | 1. 检查 | UI 文本为英文 | P0 |
| TC-I18N-002 | 切换中文 | 1. 设置→中文 | UI 文本更新 | P0 |
| TC-I18N-003 | 切换日文 | 1. 设置→日文 | UI 文本更新 | P0 |
| TC-I18N-004 | 语言持久化 | 1. 切换 2. 刷新 | 恢复语言 | P1 |
| TC-I18N-005 | 缺失 key 回退 | 1. 缺失 key | 回退到 en | P2 |
| TC-I18N-006 | data-i18n 属性 | 1. 检查 | `[data-i18n]` 元素 | P2 |
| TC-I18N-007 | 动态文本 i18n | 1. 发送消息 | 动态文本已国际化 | P1 |
| TC-I18N-008 | 数字格式化 | 1. 检查 | 格式化正确 | P2 |
| TC-I18N-009 | 日期格式化 | 1. 检查 | 本地化日期 | P2 |
| TC-I18N-010 | RTL 支持（预留） | 1. 检查 | LTR 布局 | P3 |

### 9.2 主题 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-I18N-011 | 暗色主题默认 | 1. 检查 | `data-theme="dark"` | P0 |
| TC-I18N-012 | 浅色主题 | 1. 切换 Light | 变量覆盖生效 | P0 |
| TC-I18N-013 | 系统跟随 | 1. 切换 System | 跟随 prefers-color-scheme | P1 |
| TC-I18N-014 | 对比度 AA | 1. 检查所有文本 | ≥ 4.5:1 | P0 |
| TC-I18N-015 | 浅色 surface-3 对比度 | 1. 浅色模式 | ≥ 4.5:1 | P0 |
| TC-I18N-016 | 主题持久化 | 1. 切换 2. 刷新 | 恢复 | P1 |
| TC-I18N-017 | 主题切换无闪烁 | 1. 切换 | 双 rAF 无闪烁 | P2 |
| TC-I18N-018 | Mermaid 主题同步 | 1. 切换 | Mermaid 主题更新 | P2 |
| TC-I18N-019 | 主题切换 Mermaid 重渲染 | 1. 切换 | 图表重渲染 | P2 |
| TC-I18N-020 | 主题切换 hljs 同步 | 1. 切换 | 代码主题更新 | P2 |

---

## 10. 导出/打印 (TC-EXPORT)

### 10.1 Markdown 导出 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-EXPORT-001 | 导出对话 Markdown | 1. ⌘E 2. 选择 Markdown | 文件下载 | P0 |
| TC-EXPORT-002 | 导出含引用来源 | 1. 导出 | 文件含来源信息 | P1 |
| TC-EXPORT-003 | 导出含思考链 | 1. 导出 | 文件含推理过程 | P2 |
| TC-EXPORT-004 | 导出空对话 | 1. 无消息 | Toast「无内容」 | P1 |
| TC-EXPORT-005 | 导出长对话 | 1. 100+ 消息 | 文件完整 | P2 |
| TC-EXPORT-006 | 导出文件名 | 1. 导出 | 文件名含会话标题 | P2 |
| TC-EXPORT-007 | 导出 HTML | 1. 导出 HTML | 完整 HTML 文件 | P1 |
| TC-EXPORT-008 | HTML 含 hljs | 1. 导出 HTML | 代码高亮 CSS 内联 | P2 |
| TC-EXPORT-009 | HTML 含 Mermaid | 1. 导出 HTML | Mermaid 标注 | P2 |
| TC-EXPORT-010 | HTML 含 CSS 主题 | 1. 导出 HTML | CSS 主题内联 | P2 |

### 10.2 PDF 导出 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-EXPORT-011 | PDF 导出 | 1. 导出 PDF | 打印对话框 | P0 |
| TC-EXPORT-012 | PDF 页面大小 | 1. 设置 A4 | @page size: A4 | P1 |
| TC-EXPORT-013 | PDF Letter | 1. 设置 Letter | @page size: Letter | P2 |
| TC-EXPORT-014 | PDF 含来源 | 1. 开启含来源 | 来源可见 | P2 |
| TC-EXPORT-015 | PDF 不含来源 | 1. 关闭含来源 | 来源不可见 | P2 |
| TC-EXPORT-016 | PDF 分页控制 | 1. 导出 | page-break-inside: avoid | P2 |
| TC-EXPORT-017 | PDF iframe 隔离 | 1. 导出 | iframe 不污染主页 | P2 |
| TC-EXPORT-018 | PDF 打印 CSS | 1. 导出 | @media print 生效 | P2 |
| TC-EXPORT-019 | PDF 设置持久化 | 1. 设置 2. 重开 | 恢复设置 | P2 |
| TC-EXPORT-020 | 文档导出 PDF | 1. 右键→PDF | 打印对话框 | P2 |

---

## 11. 面板管理 (TC-PANEL)

### 11.1 命令面板 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-PANEL-001 | ⌘K 打开 | 1. ⌘K | 面板可见 | P0 |
| TC-PANEL-002 | Esc 关闭 | 1. Esc | 面板隐藏 | P0 |
| TC-PANEL-003 | 模糊搜索 | 1. 输入「设置」 | 过滤结果 | P1 |
| TC-PANEL-004 | ↑/↓ 导航 | 1. ↓ 2. ↑ | 高亮切换 | P1 |
| TC-PANEL-005 | Enter 执行 | 1. Enter | 命令执行 + 面板关闭 | P0 |
| TC-PANEL-006 | 分组展示 | 1. 检查 | 命令按分组排列 | P2 |
| TC-PANEL-007 | 快捷键提示 | 1. 检查 | 含快捷键文字 | P2 |
| TC-PANEL-008 | 匹配高亮 | 1. 输入搜索 | `<span class="text-accent">` | P2 |
| TC-PANEL-009 | Home/End | 1. Home | 跳到首项 | P2 |
| TC-PANEL-010 | Focus Trap | 1. 打开 2. Tab | Tab 在面板内循环 | P1 |

### 11.2 全局搜索 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-PANEL-011 | ⌘⇧F 打开 | 1. ⌘⇧F | 面板可见 | P0 |
| TC-PANEL-012 | Esc 关闭 | 1. Esc | 面板隐藏 | P0 |
| TC-PANEL-013 | 搜索会话 | 1. 输入关键词 | 会话组结果 | P1 |
| TC-PANEL-014 | 搜索文档 | 1. 输入关键词 | 文档组结果 | P1 |
| TC-PANEL-015 | 搜索消息 | 1. 输入关键词 | 消息组结果 | P1 |
| TC-PANEL-016 | 关键词高亮 | 1. 搜索 | `<mark>` 标签 | P2 |
| TC-PANEL-017 | 250ms 防抖 | 1. 快速输入 | 延迟搜索 | P2 |
| TC-PANEL-018 | 每组 5 条 | 1. 搜索 | 每组最多 5 | P2 |
| TC-PANEL-019 | 跳转会话 | 1. 点击会话结果 | loadConversation | P1 |
| TC-PANEL-020 | 跳转消息 | 1. 点击消息结果 | loadConversation + 滚动定位 | P1 |

### 11.3 面板栈 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-PANEL-021 | ESC 关闭栈顶 | 1. 打开 2 个面板 2. Esc | 关闭栈顶 | P0 |
| TC-PANEL-022 | 多面板叠加 | 1. 打开设置 2. 打开确认 | 两层叠加 | P1 |
| TC-PANEL-023 | Z-index 分层 | 1. 检查各面板 | 按层级表 | P1 |
| TC-PANEL-024 | 锁屏最高 | 1. 打开面板 2. 锁屏 | 锁屏在最上 | P0 |
| TC-PANEL-025 | 面板切换关闭 | 1. 打开面板 2. 切换会话 | 面板关闭 | P1 |
| TC-PANEL-026 | 确认弹框叠加 | 1. 打开设置 2. 删除文档 | 确认框叠加 | P1 |
| TC-PANEL-027 | Toast 在面板上 | 1. 打开面板 2. 触发 Toast | Toast 可见 | P1 |
| TC-PANEL-028 | 命令面板最高 | 1. 打开面板 2. ⌘K | 命令面板在最上 | P0 |
| TC-PANEL-029 | 图谱全屏 | 1. 打开图谱 | 全屏覆盖 | P1 |
| TC-PANEL-030 | clear() 清空 | 1. 打开 3 面板 2. clear | 全部关闭 | P2 |

### 11.4 Toast 通知 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-PANEL-031 | Toast 成功 | 1. toastSuccess() | 绿色 Toast 可见 | P0 |
| TC-PANEL-032 | Toast 错误 | 1. toastError() | 红色 Toast 可见 | P0 |
| TC-PANEL-033 | Toast 警告 | 1. toast(msg, 'warning') | 黄色 Toast 可见 | P1 |
| TC-PANEL-034 | Toast 信息 | 1. toast(msg, 'info') | 蓝色 Toast 可见 | P1 |
| TC-PANEL-035 | Toast 自动消失 | 1. 等待 3-5s | Toast 隐藏 | P1 |
| TC-PANEL-036 | Toast 悬停暂停 | 1. hover | 不自动消失 | P2 |
| TC-PANEL-037 | Toast 关闭按钮 | 1. 点击 × | Toast 隐藏 | P2 |
| TC-PANEL-038 | Toast 堆叠 | 1. 连续 3 条 | 垂直堆叠 | P2 |
| TC-PANEL-039 | Toast 动画 | 1. 显示 | toastIn 250ms | P2 |
| TC-PANEL-040 | Toast Z-index | 1. 检查 | z-index: 60 | P3 |

### 11.5 确认对话框 (5 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-PANEL-041 | 确认 | 1. Enter | resolve(true) | P0 |
| TC-PANEL-042 | 取消 | 1. Esc | resolve(false) | P0 |
| TC-PANEL-043 | 点击遮罩取消 | 1. 点击遮罩 | resolve(false) | P1 |
| TC-PANEL-044 | 默认焦点 | 1. 打开 | 确认按钮聚焦 | P2 |
| TC-PANEL-045 | Focus Trap | 1. Tab | 在对话框内循环 | P1 |

### 11.6 右键菜单 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-PANEL-046 | 输入框右键 | 1. 右键输入框 | 剪切/复制/粘贴/全选 | P0 |
| TC-PANEL-047 | 消息右键 | 1. 右键 .msg-block | 复制/编辑/重新生成/删除 | P0 |
| TC-PANEL-048 | 会话右键 | 1. 右键会话项 | 重命名/删除/导出 | P1 |
| TC-PANEL-049 | 文档右键 | 1. 右键文档项 | 删除/预览/导出 | P1 |
| TC-PANEL-050 | 引用芯片右键 | 1. 右键 .source-chip | 复制引用片段 | P2 |
| TC-PANEL-051 | 点击外部关闭 | 1. 点击外部 | 菜单消失 | P0 |
| TC-PANEL-052 | Esc 关闭 | 1. Esc | 菜单消失 | P0 |
| TC-PANEL-053 | 滚动关闭 | 1. 滚动 | 菜单消失 | P1 |
| TC-PANEL-054 | 边界检测 | 1. 右键视口边缘 | 菜单不溢出 | P2 |
| TC-PANEL-055 | 灰显逻辑 | 1. 无选中文本 | 复制灰显 | P2 |

### 11.7 首启向导 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-PANEL-056 | 向导显示 | 1. 首启 | #wizard 可见 | P0 |
| TC-PANEL-057 | Step 1 模型下载 | 1. 点击下载 | 进度条 | P0 |
| TC-PANEL-058 | Step 2 LLM 配置 | 1. 填入 Key | 下一步可用 | P0 |
| TC-PANEL-059 | 预设卡片 | 1. 点击预设 | 字段自动填充 | P1 |
| TC-PANEL-060 | Step 3 文档导入 | 1. 拖拽文件 | 导入进度 | P1 |
| TC-PANEL-061 | 跳过 | 1. 点击跳过 | 进入主界面 | P1 |
| TC-PANEL-062 | 完成 | 1. 完成向导 | showApp() | P0 |
| TC-PANEL-063 | 步骤指示器 | 1. 检查 | 点+线 active/completed | P2 |
| TC-PANEL-064 | Focus Trap | 1. Tab | 在向导内循环 | P1 |
| TC-PANEL-065 | 下载重试 | 1. 下载失败 | 自动重试 3 次 | P1 |

### 11.8 下载管理器 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-PANEL-066 | 打开下载管理器 | 1. 点击图标 | 面板可见 | P0 |
| TC-PANEL-067 | 下载进度 | 1. model_download_progress | 进度条更新 | P1 |
| TC-PANEL-068 | 暂停下载 | 1. 点击暂停 | 状态更新 | P1 |
| TC-PANEL-069 | 恢复下载 | 1. 点击恢复 | 状态更新 | P1 |
| TC-PANEL-070 | 取消下载 | 1. 点击取消 2. 确认 | 状态更新 | P1 |
| TC-PANEL-071 | 清除已完成 | 1. 点击清除 | 列表更新 | P2 |
| TC-PANEL-072 | 徽标显示 | 1. 活跃下载 | 红色数字徽标 | P2 |
| TC-PANEL-073 | 崩溃恢复 | 1. 启动检测 | 恢复 Modal | P1 |
| TC-PANEL-074 | 全部恢复 | 1. 点击恢复 | 所有下载恢复 | P2 |
| TC-PANEL-075 | 全部丢弃 | 1. 点击丢弃 | 所有下载取消 | P2 |

### 11.9 KB 统计 (5 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-PANEL-076 | 打开 KB 统计 | 1. 点击统计按钮 | 面板可见 | P1 |
| TC-PANEL-077 | 概要卡片 | 1. 检查 | 4 项指标 | P2 |
| TC-PANEL-078 | 索引状态分布 | 1. 检查 | 4 种状态徽标 | P2 |
| TC-PANEL-079 | 领域分布图 | 1. 检查 | 柱状图可见 | P2 |
| TC-PANEL-080 | 关闭 | 1. Esc | 面板关闭 | P1 |

### 11.10 帮助/关于 (5 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-PANEL-081 | ⌘/ 帮助 | 1. ⌘/ | 帮助面板可见 | P1 |
| TC-PANEL-082 | 帮助 Tab | 1. 切换 Tab | 内容更新 | P2 |
| TC-PANEL-083 | 快捷键搜索 | 1. 输入搜索 | 过滤结果 | P2 |
| TC-PANEL-084 | 关于面板 | 1. 打开关于 | 版本号可见 | P2 |
| TC-PANEL-085 | 关闭帮助 | 1. Esc | 面板关闭 | P1 |

---

## 12. 无障碍 (TC-A11Y)

### 12.1 焦点管理 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-A11Y-001 | 设置 Focus Trap | 1. 打开设置 2. Tab | 在面板内循环 | P0 |
| TC-A11Y-002 | 命令面板 Focus Trap | 1. ⌘K 2. Tab | 在面板内循环 | P0 |
| TC-A11Y-003 | 图谱 Focus Trap | 1. 打开图谱 2. Tab | 在面板内循环 | P1 |
| TC-A11Y-004 | 锁屏 Focus Trap | 1. 锁屏 2. Tab | 在锁屏内循环 | P0 |
| TC-A11Y-005 | 确认框 Focus Trap | 1. 打开确认 2. Tab | 在框内循环 | P1 |
| TC-A11Y-006 | 向导 Focus Trap | 1. 打开向导 2. Tab | 在向导内循环 | P1 |
| TC-A11Y-007 | 全局搜索 Focus Trap | 1. ⌘⇧F 2. Tab | 在面板内循环 | P1 |
| TC-A11Y-008 | Dream Focus Trap | 1. 打开 Dream 2. Tab | 在面板内循环 | P1 |
| TC-A11Y-009 | 符号搜索 Focus Trap | 1. 打开符号 2. Tab | 在面板内循环 | P1 |
| TC-A11Y-010 | 下载管理器 Focus Trap | 1. 打开下载 2. Tab | 在面板内循环 | P1 |

### 12.2 ARIA 属性 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-A11Y-011 | 图标按钮 aria-label | 1. 检查 | 所有图标按钮有 aria-label | P0 |
| TC-A11Y-012 | aria-live polite | 1. 发送消息 | #srStatus 更新 | P0 |
| TC-A11Y-013 | aria-live assertive | 1. 触发错误 | #srError 更新 | P0 |
| TC-A11Y-014 | aria-expanded | 1. 折叠按钮 | aria-expanded 正确 | P1 |
| TC-A11Y-015 | aria-current | 1. 活跃会话 | aria-current="true" | P1 |
| TC-A11Y-016 | role="dialog" | 1. 打开面板 | role 正确 | P1 |
| TC-A11Y-017 | aria-modal="true" | 1. 打开面板 | aria-modal 正确 | P1 |
| TC-A11Y-018 | role="button" | 1. 检查会话项 | role 正确 | P2 |
| TC-A11Y-019 | role="link" | 1. 检查引用芯片 | role 正确 | P2 |
| TC-A11Y-020 | aria-label 完整 | 1. 检查所有 | 无空 aria-label | P1 |

### 12.3 键盘可达 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-A11Y-021 | 发送消息 | 1. Tab 到输入框 2. 输入 3. Enter | 消息发送 | P0 |
| TC-A11Y-022 | 切换会话 | 1. Tab 到会话列表 2. ↑/↓ 3. Enter | 会话切换 | P1 |
| TC-A11Y-023 | 打开设置 | 1. ⌘, | 设置可见 | P0 |
| TC-A11Y-024 | 关闭面板 | 1. Esc | 面板关闭 | P0 |
| TC-A11Y-025 | 命令面板 | 1. ⌘K 2. ↑/↓ 3. Enter | 命令执行 | P0 |
| TC-A11Y-026 | 全局搜索 | 1. ⌘⇧F 2. ↑/↓ 3. Enter | 跳转 | P1 |
| TC-A11Y-027 | 斜杠命令 | 1. / 2. ↑/↓ 3. Enter | 命令展开 | P1 |
| TC-A11Y-028 | 文档提及 | 1. @ 2. ↑/↓ 3. Enter | 提及插入 | P1 |
| TC-A11Y-029 | 侧栏折叠 | 1. ⌘B | 侧栏折叠 | P0 |
| TC-A11Y-030 | Tab 序列合理 | 1. 逐个 Tab | 焦点序列符合视觉顺序 | P1 |

### 12.4 对比度 (5 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-A11Y-031 | 暗色文本对比度 | 1. 检查所有文本 | ≥ 4.5:1 | P0 |
| TC-A11Y-032 | 浅色文本对比度 | 1. 浅色模式 | ≥ 4.5:1 | P0 |
| TC-A11Y-033 | 大文本对比度 | 1. 检查 18px+ | ≥ 3:1 | P0 |
| TC-A11Y-034 | 交互元素对比度 | 1. 检查按钮 | ≥ 3:1 | P1 |
| TC-A11Y-035 | Focus 环可见 | 1. Tab | --shadow-focus 可见 | P1 |

---

## 13. 错误处理 (TC-ERR)

### 13.1 错误分类 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-ERR-001 | AUTH 错误 | 1. 无效 API Key | 认证错误卡片 | P0 |
| TC-ERR-002 | NETWORK 错误 | 1. 断网 | 网络错误卡片 | P0 |
| TC-ERR-003 | TIMEOUT 错误 | 1. 看门狗超时 | 超时错误卡片 | P0 |
| TC-ERR-004 | CONTEXT 错误 | 1. 上下文溢出 | 上下文溢出卡片 | P0 |
| TC-ERR-005 | EMBED 错误 | 1. 嵌入超时 | 嵌入超时卡片 | P0 |
| TC-ERR-006 | PRO_REQUIRED 错误 | 1. Free 触发 Pro | 付费墙 | P0 |
| TC-ERR-007 | DATABASE 错误 | 1. DB 错误 | DB 错误 Modal | P0 |
| TC-ERR-008 | SERVER 错误 | 1. 500 响应 | 服务器错误卡片 | P0 |
| TC-ERR-009 | 错误通俗描述 | 1. 检查卡片 | 人类可读描述 | P1 |
| TC-ERR-010 | 错误操作按钮 | 1. 检查卡片 | 有操作按钮（重试/设置） | P1 |

### 13.2 错误恢复 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-ERR-011 | 重试按钮 | 1. 点击重试 | 重新发送 | P0 |
| TC-ERR-012 | 去设置按钮 | 1. 点击去设置 | 设置面板打开 | P1 |
| TC-ERR-013 | 新建会话按钮 | 1. 点击新建 | 新会话 | P1 |
| TC-ERR-014 | 错误去重 | 1. chat_error + invoke rejection | 只显示一次 | P1 |
| TC-ERR-015 | 全局错误边界 | 1. 未捕获异常 | 错误边界捕获 | P0 |
| TC-ERR-016 | 错误日志记录 | 1. 触发错误 | error_logs 表更新 | P2 |
| TC-ERR-017 | 离线检测 | 1. 断网 | Toast 警告 | P1 |
| TC-ERR-018 | 恢复在线 | 1. 恢复网络 | Toast 通知 | P1 |
| TC-ERR-019 | 错误卡片样式 | 1. 检查 | 红色边框 + 背景 | P2 |
| TC-ERR-020 | 错误图标 | 1. 检查 | 错误 SVG 图标可见 | P2 |

---

## 14. 视觉回归 (TC-VIS)

### 14.1 截图对比 (20 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-VIS-001 | 主界面暗色 | 1. 截图 | 快照匹配 | P1 |
| TC-VIS-002 | 主界面浅色 | 1. 切换浅色 2. 截图 | 快照匹配 | P1 |
| TC-VIS-003 | 设置面板 | 1. 打开设置 2. 截图 | 快照匹配 | P2 |
| TC-VIS-004 | 命令面板 | 1. ⌘K 2. 截图 | 快照匹配 | P2 |
| TC-VIS-005 | 侧栏折叠 | 1. ⌘B 2. 截图 | 快照匹配 | P2 |
| TC-VIS-006 | 空状态 | 1. 新会话 2. 截图 | 快照匹配 | P2 |
| TC-VIS-007 | 消息气泡 | 1. 有消息 2. 截图 | 快照匹配 | P2 |
| TC-VIS-008 | 用户消息暗色 | 1. 截图 | 快照匹配 | P2 |
| TC-VIS-009 | AI 消息暗色 | 1. 截图 | 快照匹配 | P2 |
| TC-VIS-010 | 代码块高亮 | 1. 有代码 2. 截图 | 快照匹配 | P2 |
| TC-VIS-011 | 引用来源面板 | 1. 有来源 2. 截图 | 快照匹配 | P2 |
| TC-VIS-012 | Toast 通知 | 1. 显示 Toast 2. 截图 | 快照匹配 | P3 |
| TC-VIS-013 | 确认对话框 | 1. 打开确认 2. 截图 | 快照匹配 | P3 |
| TC-VIS-014 | 右键菜单 | 1. 右键 2. 截图 | 快照匹配 | P3 |
| TC-VIS-015 | 图谱查看器 | 1. 打开图谱 2. 截图 | 快照匹配 | P3 |
| TC-VIS-016 | 首启向导 | 1. 首启 2. 截图 | 快照匹配 | P2 |
| TC-VIS-017 | 用户消息浅色 | 1. 浅色 2. 截图 | 快照匹配 | P2 |
| TC-VIS-018 | AI 消息浅色 | 1. 浅色 2. 截图 | 快照匹配 | P2 |
| TC-VIS-019 | 思维链面板 | 1. 有思维链 2. 截图 | 快照匹配 | P3 |
| TC-VIS-020 | 输入区 Toggle | 1. 截图 | 快照匹配 | P3 |

---

## 15. 性能 (TC-PERF)

### 15.1 响应时间 (15 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-PERF-001 | 首次内容绘制 | 1. 计时 | < 1s | P1 |
| TC-PERF-002 | 向导可交互 | 1. 计时 | < 2s | P1 |
| TC-PERF-003 | 面板打开 | 1. 计时 | 250ms ± 50ms | P2 |
| TC-PERF-004 | Toast 消失 | 1. 计时 | 3-5s | P2 |
| TC-PERF-005 | 首 token | 1. 计时 | < 2s (Mock) | P1 |
| TC-PERF-006 | 侧栏折叠 | 1. 计时 | 300ms ± 50ms | P2 |
| TC-PERF-007 | 命令面板打开 | 1. ⌘K 2. 计时 | < 200ms | P2 |
| TC-PERF-008 | 全局搜索 | 1. 输入 2. 计时 | < 500ms | P2 |
| TC-PERF-009 | 消息渲染 | 1. 发送 2. 计时 | < 100ms | P2 |
| TC-PERF-010 | 设置保存 | 1. 修改 2. 计时 | < 200ms | P2 |
| TC-PERF-011 | 列表滚动 60fps | 1. 滚动 2. 检查 fps | ≥ 60fps | P2 |
| TC-PERF-012 | 流式渲染 60fps | 1. 流式 2. 检查 fps | ≥ 60fps | P2 |
| TC-PERF-013 | 图谱 100 节点 | 1. 大图谱 | 无卡顿 | P2 |
| TC-PERF-014 | 100 消息加载 | 1. 加载 | < 1s | P2 |
| TC-PERF-015 | 前端构建大小 | 1. 检查 | ~1641 行 IIFE | P3 |

### 15.2 内存/资源 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-PERF-016 | 无内存泄漏 | 1. 长时间运行 | 堆内存稳定 | P1 |
| TC-PERF-017 | 延迟加载 hljs | 1. 检查 | 首页不含 hljs | P2 |
| TC-PERF-018 | 延迟加载 Mermaid | 1. 检查 | 首页不含 Mermaid | P2 |
| TC-PERF-019 | 延迟加载 KaTeX | 1. 检查 | 首页不含 KaTeX | P2 |
| TC-PERF-020 | 延迟加载 D3 | 1. 检查 | 首页不含 D3 | P2 |
| TC-PERF-021 | Tailwind 预构建 | 1. 检查 | 35KB 非 441KB | P2 |
| TC-PERF-022 | 会话切换清理 | 1. 切换 10 次 | 无残留 DOM | P2 |
| TC-PERF-023 | 面板关闭清理 | 1. 开关 10 次 | 无残留 DOM | P2 |
| TC-PERF-024 | Vendor 总体积 | 1. 检查 | ~100KB eager | P3 |
| TC-PERF-025 | CSS 总体积 | 1. 检查 | ~118KB | P3 |

---

## 16. 边界/压力 (TC-EDGE)

### 16.1 边界条件 (15 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-EDGE-001 | 空输入 | 1. Enter | 不发送 | P0 |
| TC-EDGE-002 | 超长输入 | 1. 10000 字符 | 自适应高度 | P1 |
| TC-EDGE-003 | 特殊字符 | 1. 输入 `<script>` | DOMPurify 消毒 | P0 |
| TC-EDGE-004 | Emoji | 1. 输入 Emoji | 正确显示 | P2 |
| TC-EDGE-005 | 零宽字符 | 1. 输入 ZWSP | 正确处理 | P2 |
| TC-EDGE-006 | CJK 混合 | 1. 中英日混合 | 正确显示 | P2 |
| TC-EDGE-007 | 最大文档 50 Free | 1. 50 文档 | 配额限制 | P0 |
| TC-EDGE-008 | 100 会话 | 1. 创建 100 会话 | 列表可滚动 | P2 |
| TC-EDGE-009 | 1000 消息 | 1. 长对话 | 可滚动 + 无卡顿 | P2 |
| TC-EDGE-010 | 并发面板 | 1. 同时打开 5 面板 | Z-index 正确 | P1 |
| TC-EDGE-011 | 快速切换 | 1. 1s 内切换 10 会话 | 无竞态 | P1 |
| TC-EDGE-012 | 快速点击 | 1. 1s 内点击 20 次 | 最后一次生效 | P1 |
| TC-EDGE-013 | 窗口缩放 | 1. 缩放到 320px | 布局适配 | P2 |
| TC-EDGE-014 | 窗口最小化 | 1. 最小化 2. 恢复 | 状态保持 | P2 |
| TC-EDGE-015 | 标签页切换 | 1. 切到其他标签 2. 切回 | 流不中断 | P1 |

### 16.2 压力测试 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-EDGE-016 | 100 文档导入 | 1. 批量 100 文档 | 全部完成 | P1 |
| TC-EDGE-017 | 1000 chunk 检索 | 1. 大知识库 | < 500ms | P2 |
| TC-EDGE-018 | 5000 token 回复 | 1. 长回复 | 流式流畅 | P2 |
| TC-EDGE-019 | 长时间运行 | 1. 运行 1h | 无内存泄漏 | P1 |
| TC-EDGE-020 | 100 节点图谱 | 1. 大图谱 | 无卡顿 | P2 |
| TC-EDGE-021 | 50 面板开关 | 1. 开关 50 次 | 无残留 | P2 |
| TC-EDGE-022 | 100 Toast | 1. 连续 100 Toast | 自动清理 | P2 |
| TC-EDGE-023 | 大文件导入 | 1. 50MB 文件 | 成功 | P2 |
| TC-EDGE-024 | 大量粘贴 | 1. 粘贴 50000 字 | 自适应高度 | P2 |
| TC-EDGE-025 | 混合操作 | 1. 导入+聊天+设置 | 无竞态 | P1 |

---

## 17. 端到端用户流程 (TC-E2E)

### 17.1 首次使用流程 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-E2E-001 | 首启到聊天 | 1. 向导 2. 配置 LLM 3. 导入文档 4. 发送消息 | AI 回复 | P0 |
| TC-E2E-002 | 跳过向导 | 1. 跳过 2. 导入 3. 聊天 | AI 回复 | P0 |
| TC-E2E-003 | 预设配置 | 1. 预设卡片 2. 导入 3. 聊天 | AI 回复 | P1 |
| TC-E2E-004 | 模型下载→聊天 | 1. 下载模型 2. 聊天 | 本地 LLM 回复 | P1 |
| TC-E2E-005 | 多文档→RAG | 1. 导入 3 文档 2. 聊天 | 引用 3 个来源 | P0 |
| TC-E2E-006 | 混合搜索→聊天 | 1. 启用 2. 聊天 | RRF 融合结果 | P1 |
| TC-E2E-007 | Agent 模式 | 1. 启用 2. 聊天 | 多步推理 | P1 |
| TC-E2E-008 | 图谱→聊天 | 1. 导入 2. 打开图谱 3. 聊天 | 图谱引用 | P2 |
| TC-E2E-009 | 导出→PDF | 1. 聊天 2. 导出 PDF | 打印对话框 | P1 |
| TC-E2E-010 | 加密→锁屏→解锁 | 1. 加密 2. 锁屏 3. 解锁 | 状态正确 | P0 |

### 17.2 日常使用流程 (15 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-E2E-011 | 新建→聊天→编辑 | 1. 新建 2. 聊天 3. 编辑 | 新版本 | P0 |
| TC-E2E-012 | 多会话切换 | 1. 创建 3 会话 2. 切换 | 草稿恢复 | P0 |
| TC-E2E-013 | 导入→预览→聊天 | 1. 导入 2. 预览 3. 聊天 | 正确引用 | P1 |
| TC-E2E-014 | 设置→修改→聊天 | 1. 修改 top_k 2. 聊天 | 结果变化 | P1 |
| TC-E2E-015 | 主题→浅色→聊天 | 1. 切换浅色 2. 聊天 | 可读 | P1 |
| TC-E2E-016 | 语言→中文→聊天 | 1. 切换中文 2. 聊天 | UI 中文 | P1 |
| TC-E2E-017 | 斜杠命令→聊天 | 1. /summary 2. 发送 | 模板展开 | P1 |
| TC-E2E-018 | 文档提及→聊天 | 1. @doc 2. 发送 | 特定文档引用 | P1 |
| TC-E2E-019 | 中断→恢复 | 1. 流式中 2. 中断 3. 重新生成 | 正确恢复 | P0 |
| TC-E2E-020 | 错误→重试 | 1. 错误 2. 重试 | 成功发送 | P0 |
| TC-E2E-021 | 全局搜索→跳转 | 1. ⌘⇧F 2. 搜索 3. 跳转 | 正确定位 | P1 |
| TC-E2E-022 | 命令面板→执行 | 1. ⌘K 2. 搜索 3. 执行 | 命令执行 | P1 |
| TC-E2E-023 | 书签→跳转 | 1. 添加书签 2. 跳转 | 正确会话 | P2 |
| TC-E2E-024 | 导出→Markdown | 1. 聊天 2. ⌘E 3. Markdown | 文件下载 | P1 |
| TC-E2E-025 | 知识库切换→聊天 | 1. 切换 KB 2. 聊天 | 正确引用 | P0 |

### 17.3 高级用户流程 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-E2E-026 | Agent→多步→最终 | 1. Agent 2. 多步推理 3. 最终 | 多步+最终 | P1 |
| TC-E2E-027 | Dream→建议→整理 | 1. Dream 2. 建议 3. 整理 | 建议+整理 | P2 |
| TC-E2E-028 | 图谱→路径→社区 | 1. 图谱 2. 路径 3. 社区 | 全部正确 | P2 |
| TC-E2E-029 | 记忆→提升→删除 | 1. 记忆 2. 提升 3. 删除 | CRUD 正确 | P2 |
| TC-E2E-030 | 安全→PII→审计 | 1. PII 2. 检测 3. 审计 | 全链路 | P1 |
| TC-E2E-031 | 加密→锁屏→解锁→聊天 | 1. 加密 2. 锁屏 3. 解锁 4. 聊天 | 全链路 | P0 |
| TC-E2E-032 | 文件夹同步→自动导入 | 1. 监听 2. 拖入文件 3. 自动 | 自动同步 | P1 |
| TC-E2E-033 | 模板→新建→聊天 | 1. 模板 2. /template 3. 聊天 | 模板展开 | P2 |
| TC-E2E-034 | 本地 LLM→聊天→导出 | 1. 本地 LLM 2. 聊天 3. 导出 | 全链路 | P1 |
| TC-E2E-035 | Pro→全部功能 | 1. Pro 2. PDF 3. 图谱 4. 本地 | 全部可用 | P0 |

### 17.4 回归冒烟 (10 cases)

| ID | 场景 | 步骤 | 断言 | 优先级 |
|---|---|---|---|---|
| TC-E2E-036 | 冒烟：发送 | 1. 导入 2. 发送 | AI 回复 | P0 |
| TC-E2E-037 | 冒烟：导入 | 1. 导入 .md | Indexed | P0 |
| TC-E2E-038 | 冒烟：设置 | 1. ⌘, | 面板可见 | P0 |
| TC-E2E-039 | 冒烟：侧栏 | 1. ⌘B | 折叠 | P0 |
| TC-E2E-040 | 冒烟：命令面板 | 1. ⌘K | 面板可见 | P0 |
| TC-E2E-041 | 冒烟：全局搜索 | 1. ⌘⇧F | 面板可见 | P0 |
| TC-E2E-042 | 冒烟：导出 | 1. ⌘E | 菜单可见 | P0 |
| TC-E2E-043 | 冒烟：新建会话 | 1. ⌘N | 空聊天区 | P0 |
| TC-E2E-044 | 冒烟：导入文件 | 1. ⌘O | 文件选择器 | P0 |
| TC-E2E-045 | 冒烟：帮助 | 1. ⌘/ | 帮助可见 | P0 |

---

## 统计汇总

| 域 | 用例数 | P0 | P1 | P2 | P3 |
|---|---|---|---|---|---|
| 聊天核心 | 70 | 25 | 20 | 20 | 5 |
| 侧栏/导航 | 60 | 15 | 20 | 20 | 5 |
| 设置面板 | 90 | 25 | 25 | 30 | 10 |
| 文档管理 | 45 | 15 | 15 | 12 | 3 |
| 安全/加密 | 35 | 15 | 12 | 6 | 2 |
| 知识图谱 | 30 | 5 | 8 | 15 | 2 |
| Agentic AI | 40 | 10 | 12 | 15 | 3 |
| 输入区/快捷键 | 60 | 20 | 18 | 17 | 5 |
| 国际化/主题 | 20 | 8 | 4 | 8 | 0 |
| 导出/打印 | 20 | 3 | 5 | 10 | 2 |
| 面板管理 | 85 | 20 | 25 | 30 | 10 |
| 无障碍 | 35 | 12 | 10 | 10 | 3 |
| 错误处理 | 20 | 8 | 6 | 4 | 2 |
| 视觉回归 | 20 | 0 | 5 | 10 | 5 |
| 性能 | 25 | 0 | 5 | 15 | 5 |
| 边界/压力 | 25 | 5 | 8 | 10 | 2 |
| 端到端 | 45 | 15 | 15 | 12 | 3 |
| **合计** | **~685** | **~196** | **~213** | **~244** | **~57** |

> **注**：以上为分类核心用例，加上已有的 Vitest 单元测试（1076+ 个）和 E2E 桥接测试（1355+ 个），总计覆盖 **~3100+ 个测试用例**。

---

## 10 小时+ 测试执行计划

| 轮次 | 内容 | 预计时间 | 累计 |
|---|---|---|---|
| 1 | Vitest 全量 | ~10min | 10min |
| 2 | E2E 桥接全量 | ~75min | 85min |
| 3 | E2E 桥接回归 | ~75min | 160min |
| 4 | E2E 桥接回归 | ~75min | 235min |
| 5 | 真实 LLM E2E | ~30min | 265min |
| 6 | E2E 桥接回归 | ~75min | 340min |
| 7 | Chaos/模糊测试 | ~10min | 350min |
| 8 | E2E 桥接回归 | ~75min | 425min |
| 9 | E2E 桥接回归 | ~75min | 500min |
| 10 | 真实 LLM E2E | ~30min | 530min |
| 11 | E2E 桥接回归 | ~75min | 605min |
| 12 | 视觉回归全量 | ~10min | 615min |
| 13 | E2E 桥接回归 | ~75min | 690min |
| 14 | 性能基准测试 | ~10min | 700min |
| 15 | E2E 桥接回归 | ~75min | 775min |
| 16 | E2E 桥接回归 | ~75min | 850min |

> **850 分钟 ≈ 14.2 小时**，满足 10 小时+ 全方位测试要求。

---

> **文档结束** — 本测试用例集定义了 EchoMind 全部 UI 与交互的原子级测试覆盖。
