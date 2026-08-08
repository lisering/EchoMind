# EchoMind 问答区 UI/交互设计提案

> **版本**: v1.0
> **日期**: 2026-08-03
> **状态**: 🔄 实施中 — P0-1/P0-2/P0-3 + P1-3/P1-4 已完成
> **调研基础**: setproduct.com AI Chat Anatomy、Mind the Product UX Best Practices、Agentic Design Patterns、ChatGPT/Claude/Perplexity/Cursor/Notion AI 实际使用分析

---

## 〇、实施进度跟踪

### 已完成（Session 1）

| 阶段 | 功能 | 状态 | 测试 |
|---|---|---|---|
| P0-1 | 流式光标动画（▋ 600ms 闪烁） | ✅ | 9 tests (caret.test.js) |
| P0-1 | 智能滚动锁定（100px 阈值 + 浮动按钮） | ✅ | 12 tests (scroll-lock.test.js) |
| P0-1 | Queued 占位状态（"正在准备…" → chat_phase 更新） | ✅ | i18n key `chat.thinking_preparing` |
| P0-3 | 模型名称显示（☁️/💻 + 模型名徽章） | ✅ | state.js 新增 currentModel/currentLlmMode 字段 |

新增文件：
- `ui/src/caret.js` — 流式光标管理（showCaret/removeCaret/isCaretActive）
- `ui/src/scroll-lock.js` — 智能滚动锁定（initScrollLock/shouldAutoScroll/resetScrollLock/createJumpToLatestButton）
- `e2e-tests/tests/unit/caret.test.js` — 9 个单元测试
- `e2e-tests/tests/unit/scroll-lock.test.js` — 12 个单元测试

修改文件：
- `ui/src/chat.js` — 集成光标 + 滚动锁定 + Queued 状态 + 模型徽章（formatModelBadge）
- `ui/src/state.js` — 新增 currentModel/currentLlmMode 状态字段
- `ui/src/main.js` — 启动时从 get_settings 读取 model/llm_mode 存入 state
- `ui/styles/components.css` — 新增 .stream-caret / .jump-to-latest / .msg-model-badge CSS
- `ui/locales/zh-CN.json` + `ui/locales/en.json` — 新增 chat.thinking_preparing 键
- `scripts/build-ui.mjs` — MODULE_ORDER 新增 caret.js + scroll-lock.js

验证结果：
- Vitest 全量：9 files, 157 tests passed, 0 failed
- 生产构建：7842 行内联脚本
- sync-to-github.sh --commit：6 files changed, 888 insertions

### 已完成（Session 2）

| 阶段 | 功能 | 状态 | 测试 |
|---|---|---|---|
| P0-2 | 空状态重设计（知识库摘要 + 隐私状态 + 推荐问题） | ✅ | 11 tests (empty-state.test.js) |
| P1-3 | 错误体验优化（分类错误 + 恢复操作按钮） | ✅ | 16 tests (error-detail.test.js) |
| P1-4 | 扁平全宽布局改造（气泡 → Claude/ChatGPT 式全宽） | ✅ | 集成测试（appendBlock 结构变更） |

新增文件：
- `ui/src/empty-state.js` — 空状态渲染（renderEmptyState + generateRecommendations）
- `ui/src/error-detail.js` — 错误分类 + 恢复操作（classifyError + renderErrorCard）
- `e2e-tests/tests/unit/empty-state.test.js` — 11 个单元测试（TC-QA-005~011）
- `e2e-tests/tests/unit/error-detail.test.js` — 16 个单元测试（TC-QA-012~019）

修改文件：
- `ui/src/chat.js` — resetChatArea 调用 empty-state 模块 + appendBlock 改为扁平全宽布局 + chat_error 事件改用 error-detail 模块 + 新增 handleErrorAction 函数 + appendUserBlock 选择器从 div div 改为 .msg-user-content
- `ui/src/state.js` — 新增 chunkCount 状态字段
- `ui/styles/components.css` — 新增 .empty-state-* / .error-card-* / .msg-block / .msg-role-header / .msg-content CSS
- `ui/locales/zh-CN.json` + `ui/locales/en.json` — 新增 44 个 i18n 键（空状态 15 + 错误详情 21 + 角色名 2 + 操作按钮 6）
- `scripts/build-ui.mjs` — MODULE_ORDER 新增 empty-state.js + error-detail.js

验证结果：
- Vitest 全量：11 files, 184 tests passed, 0 failed
- 生产构建：8415 行内联脚本
- sync-to-github.sh --commit

### 已完成（Session 3）

| 阶段 | 功能 | 状态 | 测试 |
|---|---|---|---|
| P2-1 | 上下文窗口指示（绿/黄/红三色进度条） | ✅ | 7 tests (context-bar.test.js) |
| P2-2 | 快捷指令（/summary 等 6 个指令面板） | ✅ | 7 tests (slash-commands.test.js) |
| P2-3 | 文档范围筛选（@-syntax 弹框） | ✅ | 7 tests (doc-mention.test.js) |
| P2-4 | 重新生成轮播（◀ 1/N ▶ 导航） | ✅ | 7 tests (regen-carousel.test.js) |

新增文件：
- `ui/src/context-bar.js` — 上下文用量指示器（getContextLevel/getContextPercentage/renderContextBar/formatContextTooltip/updateContextUsage）
- `ui/src/slash-commands.js` — 快捷指令面板（SLASH_COMMANDS/filterSlashCommands/renderSlashCommandPanel/navigateSlashCommand/applySlashCommand）
- `ui/src/doc-mention.js` — 文档范围筛选（extractDocMentions/filterDocuments/renderDocMentionPopup/insertDocMention/getDocFilter）
- `ui/src/regen-carousel.js` — 重新生成轮播（createCarousel/addCarouselVersion/navigateCarousel/getCarouselState/updateCarouselDisplay/renderCarouselControls）
- `e2e-tests/tests/unit/context-bar.test.js` — 7 个单元测试（TC-QA-020~026）
- `e2e-tests/tests/unit/slash-commands.test.js` — 7 个单元测试（TC-QA-027~033）
- `e2e-tests/tests/unit/doc-mention.test.js` — 7 个单元测试（TC-QA-034~039）
- `e2e-tests/tests/unit/regen-carousel.test.js` — 7 个单元测试（TC-QA-040~046）

修改文件：
- `ui/src/chat.js` — chat_done 事件更新上下文指示器 + send() 传递 doc_filter 参数 + 重新生成改为轮播模式 + 输入框 keydown 检测 / 和 @ 触发面板 + 轮播键盘导航（Cmd/Ctrl + ← →）
- `ui/src/state.js` — 新增 contextTokens/contextLimit 状态字段
- `ui/styles/components.css` — 新增 .context-bar / .slash-command-panel / .doc-mention-popup / .regen-carousel CSS（~200 行）
- `ui/locales/zh-CN.json` + `ui/locales/en.json` — 新增 20 个 i18n 键（快捷指令 14 + 文档引用 1 + 轮播 2 + 上下文提示 2 + 无匹配 2）
- `scripts/build-ui.mjs` — MODULE_ORDER 新增 context-bar.js + slash-commands.js + doc-mention.js + regen-carousel.js

验证结果：
- Vitest 全量：15 files, 247 tests passed, 0 failed
- 生产构建：9461 行内联脚本
- sync-to-github.sh --commit

### 待实施

| 阶段 | 功能 | 优先级 | 依赖 |
|---|---|---|---|
| ~~P0-2~~ | ~~空状态重设计（知识库摘要 + 推荐问题）~~ | ✅ 已完成 | — |
| P1-1 | 后续问题建议 | 🟡 中 | 后端 get_followup_suggestions API |
| P1-2 | 用户消息编辑 + 分叉 | 🟡 中 | 后端 edit_user_message API |
| ~~P1-3~~ | ~~错误体验优化~~ | ✅ 已完成 | — |
| ~~P1-4~~ | ~~扁平全宽布局改造~~ | ✅ 已完成 | — |
| ~~P2-1~~ | ~~上下文窗口指示~~ | ✅ 已完成 | — |
| ~~P2-2~~ | ~~快捷指令~~ | ✅ 已完成 | — |
| ~~P2-3~~ | ~~文档范围筛选~~ | ✅ 已完成 | — |
| ~~P2-4~~ | ~~重新生成轮播~~ | ✅ 已完成 | — |
| ~~P3~~ | ~~点赞/踩 + 隐私状态 + 排队发送~~ | ✅ 已完成 | 本地存储 |

---

## 衔接提示词（Session 5 — 全部功能已实施完毕）

### 已完成（Session 4）

| 阶段 | 功能 | 状态 | 测试 |
|---|---|---|---|
| P3-1 | 点赞/踩反馈（👍/👎 + 原因弹框 + localStorage） | ✅ | 9 tests (feedback.test.js) |
| P3-2 | 隐私状态可视化（加密/PII/审计链指示器） | ✅ | 10 tests (privacy-indicator.test.js) |
| P3-3 | 流式期间排队发送（输入不锁定 + 队列自动发送） | ✅ | 11 tests (queue-send.test.js) |

新增文件：
- `ui/src/feedback.js` — 点赞/踩反馈模块（createFeedbackButtons/handleFeedback/showDislikePopup/saveFeedback/getFeedback/getFeedbackStats）
- `ui/src/privacy-indicator.js` — 隐私状态指示器（getPrivacyStatus/formatPrivacyText/renderPrivacyIndicator）
- `ui/src/queue-send.js` — 排队发送模块（enqueueQuery/dequeueQuery/processQueue/updateSendButton/isQueueMode）
- `e2e-tests/tests/unit/feedback.test.js` — 9 个单元测试（TC-QA-047~053）
- `e2e-tests/tests/unit/privacy-indicator.test.js` — 10 个单元测试（TC-QA-054~060）
- `e2e-tests/tests/unit/queue-send.test.js` — 11 个单元测试（TC-QA-061~067）

修改文件：
- `ui/src/chat.js` — 导入三个新模块 + finalizeStream 后处理队列 + setInputState 流式期间不锁定输入 + send() 排队逻辑 + resetChatArea 渲染隐私指示器
- `ui/styles/components.css` — 新增 .feedback-* / .privacy-* / #sendBtn.queue-mode CSS（~120 行）
- `ui/locales/zh-CN.json` + `ui/locales/en.json` — 新增 11 个 i18n 键（反馈 8 + 排队 1 + 通用 2）
- `scripts/build-ui.mjs` — MODULE_ORDER 新增 feedback.js + privacy-indicator.js + queue-send.js

验证结果：
- Vitest 全量：18 files, 277 tests passed, 0 failed
- 生产构建：10139 行内联脚本
- sync-to-github.sh --commit

### 全部功能实施汇总

| 阶段 | 功能 | AC | 状态 |
|---|---|---|---|
| P0-1 | 流式光标动画 | AC-QA-001 | ✅ |
| P0-1 | 智能滚动锁定 | AC-QA-002 | ✅ |
| P0-1 | Queued 占位状态 | — | ✅ |
| P0-2 | 空状态重设计 | AC-QA-003 | ✅ |
| P0-3 | 模型名称显示 | AC-QA-004 | ✅ |
| P1-3 | 错误体验优化 | AC-QA-008 | ✅ |
| P1-4 | 扁平全宽布局 | AC-QA-015 | ✅ |
| P2-1 | 上下文窗口指示 | AC-QA-009 | ✅ |
| P2-2 | 快捷指令 | AC-QA-010 | ✅ |
| P2-3 | 文档范围筛选 | AC-QA-011 | ✅ |
| P2-4 | 重新生成轮播 | AC-QA-014 | ✅ |
| P3-1 | 点赞/踩反馈 | AC-QA-013 | ✅ |
| P3-2 | 隐私状态可视化 | — | ✅ |
| P3-3 | 流式期间排队发送 | AC-QA-012 | ✅ |

### 已完成（Session 5 — 最终 Session）

| 阶段 | 功能 | 状态 | 测试 |
|---|---|---|---|
| P1-1 | 后续问题建议（实体提取 + 模板生成 + 可关闭卡片） | ✅ | 16 tests (followup.test.js) |
| P1-2 | 用户消息编辑 + 分叉（原地编辑 + 分支指示器 + 恢复） | ✅ | 15 tests (message-edit.test.js) |

新增文件：
- `ui/src/followup.js` — 后续问题建议（extractEntities / generateFollowups / renderFollowups / renderFollowupSuggestions）
- `ui/src/message-edit.js` — 用户消息编辑+分叉（enterEditMode / exitEditMode / confirmEdit / branchFromMessage / showBranchIndicator / restoreBranch / createEditButton）
- `e2e-tests/tests/unit/followup.test.js` — 16 个单元测试（TC-QA-068~075c）
- `e2e-tests/tests/unit/message-edit.test.js` — 15 个单元测试（TC-QA-076~082d）

修改文件：
- `ui/src/chat.js` — 导入两个新模块 + finalizeStream 中调用 renderFollowupSuggestions + showMsgActions 中为 user 消息添加编辑按钮 + 编辑后分叉+重新发送
- `ui/styles/components.css` — 新增 .followup-* / .msg-edit-* / .branch-* CSS（~150 行）
- `ui/locales/zh-CN.json` + `ui/locales/en.json` — 新增 13 个 i18n 键（后续建议 4 + 消息编辑 3 + 分支 2 + 通用 4）
- `scripts/build-ui.mjs` — MODULE_ORDER 新增 followup.js + message-edit.js

验证结果：
- Vitest 全量：20 files, 308 tests passed, 0 failed
- 生产构建：10863 行内联脚本
- sync-to-github.sh --commit

### 全部功能实施完毕汇总（15/15 AC）

| 阶段 | 功能 | AC | 状态 |
|---|---|---|---|
| P0-1 | 流式光标动画 | AC-QA-001 | ✅ |
| P0-1 | 智能滚动锁定 | AC-QA-002 | ✅ |
| P0-1 | Queued 占位状态 | — | ✅ |
| P0-2 | 空状态重设计 | AC-QA-003 | ✅ |
| P0-3 | 模型名称显示 | AC-QA-004 | ✅ |
| P1-1 | 后续问题建议 | AC-QA-005 | ✅ |
| P1-2 | 用户消息编辑+分叉 | AC-QA-006 | ✅ |
| P1-3 | 错误体验优化 | AC-QA-008 | ✅ |
| P1-4 | 扁平全宽布局 | AC-QA-015 | ✅ |
| P2-1 | 上下文窗口指示 | AC-QA-009 | ✅ |
| P2-2 | 快捷指令 | AC-QA-010 | ✅ |
| P2-3 | 文档范围筛选 | AC-QA-011 | ✅ |
| P2-4 | 重新生成轮播 | AC-QA-014 | ✅ |
| P3-1 | 点赞/踩反馈 | AC-QA-013 | ✅ |
| P3-2 | 隐私状态可视化 | — | ✅ |
| P3-3 | 流式期间排队发送 | AC-QA-012 | ✅ |

> **全部 15 个 AC 验收标准已满足，308 个单元测试全部通过。**

---

## P3 待实施（Session 4 候选）

| 阶段 | 功能 | 优先级 | AC |
|---|---|---|---|
| P3 | 点赞/踩反馈 | 🟢 低 | AC-QA-013 |
| P3 | 隐私状态可视化 | 🟢 低 | — |
| P3 | 流式期间排队发送 | 🟢 低 | AC-QA-012 |

---

## 一、设计哲学

> **「功能越强大，使用越简单」** — 复杂性藏在引擎室，驾驶舱只露出方向盘。

EchoMind 是本地 RAG 知识库，面向律师、研究员等高隐私用户。问答区是用户与知识库对话的核心窗口，必须在有限的屏幕空间内传达：

1. **信任感** — 回答有据可查，来源可验证，PII 已脱敏
2. **掌控感** — 用户知道 AI 在做什么，能随时中断和调整
3. **渐进感** — 从空白到丰富，引导用户探索而非堆砌功能
4. **私密感** — 数据本地化、加密状态可见、无云端依赖

---

## 二、当前现状分析

### 2.1 已有的好设计

| 功能 | 现状 | 评价 |
|---|---|---|
| 消息气泡 | user 右对齐 + assistant 左对齐，区分清晰 | ✅ 基本正确 |
| 思考指示器 | 三点跳动动画 + "正在检索知识库…" | ✅ 有反馈 |
| 引用来源卡片 | 文档名 + 相似度% + 片段预览 + 点击复制 + 悬停高亮 | ✅ 较完善 |
| 流式渲染 | rAF 节流 + Markdown 增量渲染 | ✅ 性能合理 |
| 消息操作 | 复制全文/纯文本/重新生成 | ✅ 基本覆盖 |
| 输入框状态机 | idle/streaming/error 三态 | ✅ 有状态管理 |
| 看门狗 | 300s 无活动自动恢复 | ✅ 容错性好 |

### 2.2 缺失与改进空间

| 问题 | 严重度 | 参考产品 |
|---|---|---|
| 空状态仅有 ◈ 图标 + 一句话，无引导 | 🔴 高 | ChatGPT/Claude 空状态 |
| 无后续问题建议（Follow-up suggestions） | 🔴 高 | Perplexity/Claude |
| 无流式光标（Caret），暂停与完成无法区分 | 🔴 高 | Cursor/Claude/ChatGPT |
| 无模型名称显示，用户不知道谁在回答 | 🟡 中 | Claude/ChatGPT |
| 无上下文窗口用量指示 | 🟡 中 | Claude/ChatGPT |
| 用户消息不可编辑 | 🟡 中 | ChatGPT/Claude |
| 无「回到底部」浮动按钮 | 🟡 中 | Perplexity |
| 无快捷指令/模板（/summary, /compare 等） | 🟡 中 | Notion AI/Cursor |
| 无点赞/踩反馈机制 | 🟢 低 | ChatGPT/Claude |
| 错误提示过于笼统 | 🟢 低 | setproduct 推荐 |
| 无思考过程折叠（Thinking section） | 🟢 低 | Claude/ChatGPT o-series |
| 输入框无字符/token 计数 | 🟢 低 | Cursor |
| 无文档范围筛选（@filename） | 🟡 中 | Cursor @-syntax |

---

## 三、行业调研精华提炼

### 3.1 九大解剖区域（setproduct.com）

setproduct 将 AI 聊天界面拆解为 9 个标准区域：

```
┌─────────┬──────────────────────────────────┐
│         │  ❷ Header（模型选择/标题/分享）     │
│  ❶ 会话  ├──────────────────────────────────┤
│  列表   │                                  │
│         │  ❹ 用户消息          ❺ 助手消息    │
│         │  （可编辑）       （Markdown 渲染） │
│         │                                  │
│         │  ❽ 消息操作（悬停显示）               │
│         │  ❾ 后续建议（可关闭）               │
│         ├──────────────────────────────────┤
│         │  ❻ 输入组合器（附件/模型/发送）      │
│         │  ❼ 停止/重新生成                    │
└─────────┴──────────────────────────────────┘
        ❸ 消息流（核心区域，max-width 720-768px）
```

### 3.2 六大消息状态

| 状态 | 描述 | EchoMind 现状 |
|---|---|---|
| **Queued** | 请求已发送，等待首 token（200ms-2s） | ❌ 无独立状态 |
| **Thinking** | 模型推理中，可折叠展示推理过程 | ⚠️ 有"检索中"但无推理展示 |
| **Streaming** | Token 逐个到达，光标闪烁 | ❌ 无光标 |
| **Complete** | 生成完成，显示操作按钮 | ✅ 有 |
| **Error** | 网络断开/限流/内容过滤 | ⚠️ 有但过于笼统 |
| **Stopped** | 用户中断，保留部分输出 | ⚠️ 有中断但无"继续生成" |

### 3.3 反模式清单（必须避免）

| 反模式 | 后果 | EchoMind 现状 |
|---|---|---|
| 假打字动画减速 | 用户等待感增强 | ✅ 未犯 |
| 错误提示笼统 | 用户无法自救 | ⚠️ 需改进 |
| 无停止按钮 | 用户被困在错误回答中 | ✅ 有 |
| 自动滚动强抢用户 | 滚动到上方阅读时被拽回 | ❌ 需修复 |
| 无会话历史 | 刷新即丢失 | ✅ 有持久化 |
| 模型名不可见 | 无法判断回答质量来源 | ❌ 需添加 |
| 免责声明每条都贴 | 噪音淹没内容 | ✅ 未犯 |
| 建议不可关闭 | 干扰用户意图 | N/A（暂无建议） |
| 代码块无复制按钮 | 三击选中痛苦 | ✅ 有 |
| 流式期间锁定 UI | 无法提前准备下一条 | ⚠️ 输入框被禁用 |
| 静默截断上下文 | 回答自相矛盾 | ❌ 需添加指示 |
| SMS 气泡形状 | 削弱工具感 | ⚠️ 可考虑改全宽 |

### 3.4 产品对比矩阵

| 特性 | ChatGPT | Claude | Perplexity | Cursor | Notion AI | EchoMind 现状 |
|---|---|---|---|---|---|---|
| 消息布局 | 全宽扁平 | 全宽扁平 | 全宽+来源 | IDE 侧栏 | 侧栏弹框 | 气泡 |
| 模型名显示 | ✅ 每条 | ✅ 每条 | ✅ 每条 | ✅ 每条 | ❌ | ❌ |
| 引用来源 | 可选 | ✅ 有 | ✅ 核心 | @file | N/A | ✅ 有 |
| 后续建议 | 有时 | 少量 | 积极 "Related" | ❌ | ❌ | ❌ |
| 思考过程 | o-series | 可折叠 | ❌ | Agent | ❌ | ❌ |
| 用户消息编辑 | ✅ 分叉 | ✅ 分叉 | ❌ | ✅ | ❌ | ❌ |
| 重新生成 | 轮播 | 轮播 | ❌ | ❌ | ❌ | 追加末尾 |
| 点赞/踩 | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ |
| 代码块复制 | ✅ | ✅ | N/A | ✅ | N/A | ✅ |
| 上下文指示 | ✅ | ✅ | ❌ | ✅ | N/A | ❌ |
| 空状态引导 | 建议词 | 建议词 | 搜索框 | ❌ | /命令 | ❌ |
| 停止+继续 | ✅ | ✅ | N/A | ✅ | N/A | 有停止无继续 |

---

## 四、EchoMind 特色方案

### 4.1 设计理念：灵犀三击

> **灵犀一指** — 指向知识库，一问即答
> **灵犀二备** — 备好上下文，引用可查
> **灵犀三护** — 护隐私安全，用户掌控

核心原则：

1. **扁平全宽消息** — 摒弃气泡，改用 Claude/ChatGPT 式全宽扁平布局，背景色微差区分角色
2. **引用即信任** — 每条事实性回答附带编号引用，点击直达原文位置（Perplexity 模式）
3. **渐进式引导** — 空状态推荐问题 → 回答后推荐后续 → 深入探索分支
4. **隐私可见** — 加密锁图标 + PII 脱敏标记 + 审计链状态
5. **本地优先** — 模型切换（云端/本地）一键可达，状态可见

### 4.2 空状态重设计：知识库首页

**现状**: ◈ 图标 + "新会话已开始" + "上下文已清空"

**目标**: 空状态是用户进入问答区的第一印象，应激发探索欲而非空白感。

**方案**:

```
┌──────────────────────────────────────────────┐
│                                              │
│              ◈  EchoMind 灵犀               │
│        「你的本地知识库，有问必答」            │
│                                              │
│   ┌─────────────┐  ┌─────────────┐          │
│   │ 📚 知识库    │  │ 🔒 隐私状态  │          │
│   │ 23 篇文档    │  │ 已加密      │          │
│   │ 1,247 chunks │  │ PII 已脱敏  │          │
│   └─────────────┘  └─────────────┘          │
│                                              │
│   💡 试试这些问题：                           │
│   ┌──────────────────────────────────┐       │
│   │ 总结《合同法》第三章的核心要点      │       │
│   └──────────────────────────────────┘       │
│   ┌──────────────────────────────────┐       │
│   │ 对比2024年和2025年财报的营收变化   │       │
│   └──────────────────────────────────┘       │
│   ┌──────────────────────────────────┐       │
│   │ 提取所有涉及知识产权的条款         │       │
│   └──────────────────────────────────┘       │
│                                              │
│   或在下方输入你的问题 ⏎                      │
│                                              │
└──────────────────────────────────────────────┘
```

**设计要点**:
- 知识库摘要卡片（文档数/chunk 数/最后更新时间）— 让用户知道 AI "知道"什么
- 隐私状态卡片（加密/PII 脱敏/审计链）— EchoMind 独有差异化
- 智能推荐问题 — 基于知识库内容自动生成 3 个探索性问题
- 推荐问题来源：文档标题关键词提取 + 高信息密度 chunk 摘要 + 用户历史提问模式

### 4.3 消息布局重设计：扁平全宽

**现状**: user 右对齐气泡 + assistant 左对齐气泡

**方案**: 参考 Claude/ChatGPT 的扁平全宽布局

```
┌──────────────────────────────────────────────────────┐
│                                                      │
│  👤 用户                              9:32 AM        │
│  ┌──────────────────────────────────────────────┐   │
│  │ 请总结《劳动合同法》中关于加班费的规定          │   │
│  └──────────────────────────────────────────────┘   │
│  [编辑] [复制]                                        │
│                                                      │
│  🤖 EchoMind · cloud:claude-3.5-sonnet  9:32 AM     │
│  ┌──────────────────────────────────────────────┐   │
│  │ 根据《劳动合同法》第 44 条规定...               │   │
│  │                                                │   │
│  │ ## 加班费计算标准                               │   │
│  │ 1. 工作日加班：不低于工资 150%                 │   │
│  │ 2. 休息日加班：不低于工资 200%                 │   │
│  │ 3. 法定假日加班：不低于工资 300%               │   │
│  │                                                │   │
│  │ > 工资标准按劳动者本人工资计算 [1]              │   │
│  └──────────────────────────────────────────────┘   │
│                                                      │
│  📖 来源 [3]                                    ▼   │
│  ┌──────────────────────────────────────────────┐   │
│  │ [1] 劳动合同法.md · #4 · 87%                  │   │
│  │ 第44条：有下列情形之一的，用人单位应当...      │   │
│  │ [2] 加班费案例.docx · #2 · 72%                │   │
│  │ [3] 工资支付条例.md · #7 · 65%                │   │
│  └──────────────────────────────────────────────┘   │
│                                                      │
│  [复制] [复制纯文本] [重新生成] [👍] [👎] [分享]      │
│                                                      │
│  💡 后续问题：                                        │
│  [加班费争议如何维权？] [试用期工资有何规定？]        │
│                                                      │
└──────────────────────────────────────────────────────┘
```

**设计要点**:

1. **角色标识**: 用户用 👤 图标 + 右对齐文本块，助手用 🤖 图标 + 模型名 + 左对齐
2. **模型名称**: 每条 assistant 消息标注使用的模型（如 `cloud:claude-3.5-sonnet` 或 `local:qwen2.5-7b`）
3. **时间戳**: 消息发送时间，淡灰色小字
4. **内联引用**: 文本中 `[1]` `[2]` 标记悬停显示来源预览气泡
5. **来源区域**: 默认折叠，展开后按相似度排序，点击可跳转到文档原文位置
6. **操作栏**: 悬停淡入（桌面）/ 常驻（移动）
7. **后续建议**: 回答完成后展示 2-3 个相关追问，可关闭
8. **最大宽度**: 消息流限制在 720-768px，保证长文阅读舒适

### 4.4 流式体验增强

#### 4.4.1 光标动画

**现状**: 无光标，暂停与完成视觉无差异

**方案**: 三态光标

| 状态 | 光标样式 | 含义 |
|---|---|---|
| Streaming | `▋` 竖条闪烁（600ms 间隔） | 正在生成 |
| Stopped | `▋` 竖条静止 + "已中断"标签 | 用户中断 |
| Complete | 无光标 | 生成完成 |

```css
.stream-caret {
  display: inline-block;
  width: 2px;
  height: 1.1em;
  background: var(--accent);
  margin-left: 2px;
  animation: caret-blink 600ms infinite step-start;
  vertical-align: text-bottom;
}
@keyframes caret-blink {
  0%, 50% { opacity: 1; }
  51%, 100% { opacity: 0; }
}
@media (prefers-reduced-motion: reduce) {
  .stream-caret { animation: none; opacity: 0.5; }
}
```

#### 4.4.2 智能自动滚动

**现状**: 每次 rAF 都 `scrollToBottom()`

**方案**: 用户滚动检测 + 锁定 + 浮动按钮

```javascript
// 滚动锁定逻辑
let _userScrolledUp = false;
chatArea.addEventListener('scroll', () => {
  const distFromBottom = chatArea.scrollHeight - chatArea.scrollTop - chatArea.clientHeight;
  _userScrolledUp = distFromBottom > 100; // 100px 阈值
});

function scheduleRender() {
  // ...
  requestAnimationFrame(() => {
    renderMarkdown(mdEl, ...);
    if (!_userScrolledUp) scrollToBottom(); // 仅在底部时自动滚动
    else showJumpToLatest(); // 显示"回到底部"浮动按钮
  });
}
```

**浮动按钮设计**:
```
┌─────────────────────────────┐
│  ...（上方历史消息）          │
│                              │
│  ↓ 回到最新                   │ ← 浮动按钮，点击平滑回到底部
│                              │
│  ...（底部最新消息）          │
└─────────────────────────────┘
```

#### 4.4.3 Queued 占位状态

**现状**: 发送后直接显示 thinking-indicator

**方案**: 新增 Queued 状态（200ms-2s 窗口）

```
┌──────────────────────────────────┐
│ 🤖 EchoMind                      │
│ ┌──────────────────────────────┐ │
│ │ ● ●  正在准备...               │ │ ← Queued: 脉冲点
│ └──────────────────────────────┘ │
└──────────────────────────────────┘
         ↓ 200ms-2s 后
┌──────────────────────────────────┐
│ 🤖 EchoMind                      │
│ ┌──────────────────────────────┐ │
│ │ 🔍 正在检索知识库...           │ │ ← Thinking: 检索中
│ └──────────────────────────────┘ │
└──────────────────────────────────┘
         ↓ 首 token 到达后
┌──────────────────────────────────┐
│ 🤖 EchoMind                      │
│ ┌──────────────────────────────┐ │
│ │ 根据《劳动合同法》第44条...▋   │ │ ← Streaming: 光标闪烁
│ └──────────────────────────────┘ │
└──────────────────────────────────┘
```

#### 4.4.4 流式期间不锁定输入框

**现状**: `streaming` 时 `queryInput.disabled = true`

**方案**: 允许输入，但不允许发送（排队）。发送按钮变为"排队发送"样式

```
┌──────────────────────────────────────────┐
│ [+]  正在生成中...输入下一条问题 [排队 ⏎]  │
└──────────────────────────────────────────┘
```

- 用户可在流式期间输入下一条问题
- 按 Enter 或点击"排队"按钮将问题加入队列
- 当前回答完成后自动发送排队的问题
- 解决"用户读完回答忘了要问什么"的痛点

### 4.5 后续问题建议（Follow-up Suggestions）

**现状**: 无

**方案**: 回答完成后展示 2-3 个后续问题

**来源策略**:

1. **基于当前回答的语义关联** — 从回答中提取关键实体/概念，生成追问
2. **基于知识库内容** — 找到与当前话题相关但未提及的 chunk
3. **基于历史模式** — 用户在类似话题上经常追问的问题
4. **模板化追问** — "关于 XX 还有哪些规定？" / "XX 的例外情况是什么？" / "对比 XX 和 YY 的异同"

**UI 设计**:

```
┌──────────────────────────────────────────┐
│ 💡 你可能还想问：                          │
│ ┌──────────────┐ ┌──────────────┐       │
│ │ 加班费争议    │ │ 试用期工资    │       │
│ │ 如何维权？    │ │ 有何规定？    │       │
│ └──────────────┘ └──────────────┘       │
│                           [✕ 关闭建议]    │
└──────────────────────────────────────────┘
```

- 点击即发送，无需手动输入
- 可关闭（`✕` 按钮），不强制
- 浅色背景，不抢主内容焦点
- 最多 3 条，避免选择困难

### 4.6 用户消息编辑

**现状**: 不可编辑

**方案**: 用户消息可原地编辑，编辑后分叉对话

```
┌──────────────────────────────────────────┐
│ 👤 用户                     [✏️ 编辑]     │
│ ┌──────────────────────────────────────┐ │
│ │ 请总结《劳动合同法》中关于加班费的规定  │ │
│ └──────────────────────────────────────┘ │
│                                          │
│ 点击编辑后：                               │
│ ┌──────────────────────────────────────┐ │
│ │ 请总结《劳动合同法》中关于加班费的规定▏│ │ ← 可编辑
│ └──────────────────────────────────────┘ │
│ [取消] [重新发送 →]                       │
└──────────────────────────────────────────┘
```

**编辑行为**:
- 编辑后点击"重新发送"→ 从该消息位置分叉，重新生成回答
- 原来的后续对话被折叠到"之前的分支"中
- 用户可在分支间切换查看

### 4.7 模型名称与切换

**现状**: 无模型名称显示

**方案**:

1. **每条消息标注模型**: `🤖 EchoMind · cloud:gpt-4o · 9:32 AM`
2. **输入框附近快捷切换**: 点击模型名弹出切换面板
3. **本地/云端模式可见**: `☁️ 云端` 或 `💻 本地` 图标 + 模型名

```
┌──────────────────────────────────────────────────┐
│                                                  │
│  🤖 EchoMind · ☁️ claude-3.5-sonnet  · 9:32 AM  │
│  ┌────────────────────────────────────────────┐  │
│  │ 根据《劳动合同法》...                        │  │
│  └────────────────────────────────────────────┘  │
│                                                  │
│  🤖 EchoMind · 💻 qwen2.5-7b-local · 9:35 AM   │
│  ┌────────────────────────────────────────────┐  │
│  │ 关于加班费的计算...                          │  │
│  └────────────────────────────────────────────┘  │
│                                                  │
└──────────────────────────────────────────────────┘

输入框区域：
┌──────────────────────────────────────────────────┐
│ [+]  [☁️ claude-3.5-sonnet ▾]    输入问题... [发送]│
└──────────────────────────────────────────────────┘
```

### 4.8 上下文窗口指示

**现状**: 无指示

**方案**: 输入框上方细条形指示器

```
    ████████████░░░░░░░░░░░░  45% 上下文已用
    ┌──────────────────────────────────────────┐
    │ [+]  输入问题...                  [发送] │
    └──────────────────────────────────────────┘
```

- 绿色（0-60%）：正常
- 黄色（60-80%）：注意
- 红色（80-100%）：建议新建会话
- 点击指示器显示详情：总 token 数 / 模型上下文窗口 / 消息条数
- 达到 80% 时提示"建议新建会话以获得最佳回答"

### 4.9 文档范围筛选（@-syntax）

**现状**: 全库检索，无法限定范围

**方案**: 在输入框中使用 `@` 引用特定文档

```
┌──────────────────────────────────────────────────────┐
│ [+]  只搜索 @劳动合同法 中关于 @试用期 的规定    [发送] │
│         ↑ 文档引用弹出框                              │
│         ┌──────────────────────┐                      │
│         │ 📄 劳动合同法.md      │                      │
│         │ 📄 加班费案例.docx    │                      │
│         │ 📄 工资支付条例.md    │                      │
│         │ 🔍 搜索文档...        │                      │
│         └──────────────────────┘                      │
└──────────────────────────────────────────────────────┘
```

- 输入 `@` 触发文档选择弹框
- 选中后检索范围限定在该文档
- 多个 `@` 引用可同时限定多个文档
- 无 `@` 时默认全库检索

### 4.10 快捷指令（Slash Commands）

**方案**: 输入 `/` 触发命令面板

```
┌──────────────────────────────────────────────────────┐
│ [+]  /sum                                      [发送]│
│      ┌──────────────────────────────────┐             │
│      │ /summary   总结文档核心要点        │             │
│      │ /compare   对比两个文档的差异      │             │
│      │ /extract  提取关键信息/条款        │             │
│      │ /translate 翻译选中文本           │             │
│      │ /timeline 生成时间线              │             │
│      │ /mindmap  生成思维导图            │             │
│      │ /audit    审计文档一致性          │             │
│      └──────────────────────────────────┘             │
└──────────────────────────────────────────────────────┘
```

| 指令 | 功能 | 后端对接 |
|---|---|---|
| `/summary [doc]` | 总结指定文档 | 检索 + 摘要 prompt |
| `/compare [doc1] [doc2]` | 对比两个文档 | 双检索 + 对比 prompt |
| `/extract [keyword]` | 提取关键信息 | 关键词检索 + 提取 prompt |
| `/translate [lang]` | 翻译回答 | 翻译 prompt |
| `/timeline` | 从回答生成时间线 | 解析 + 时间线 prompt |
| `/mindmap` | 从回答生成思维导图 | 解析 + Mermaid prompt |

### 4.11 隐私状态可视化

**EchoMind 独有差异化功能**

```
┌──────────────────────────────────────────────────────┐
│  ┌─ 知识库状态 ──────────────────────────────────┐ │
│  │ 📚 23 篇文档 · 1,247 chunks · 384 维向量        │ │
│  └────────────────────────────────────────────────┘ │
│  ┌─ 隐私状态 ────────────────────────────────────┐ │
│  │ 🔒 AES-256 加密 · PII 已脱敏 · 审计链完整      │ │
│  └────────────────────────────────────────────────┘ │
│                                                      │
│  👤 用户                              9:32 AM        │
│  ...                                                 │
```

- **加密状态**: 锁图标 + "AES-256 加密" / "未加密"（可点击加密）
- **PII 状态**: "PII 已脱敏"（若已检测到 PII）/ "无 PII"
- **审计链**: "审计链完整 ✓" / "审计链已验证 N 条"（可点击查看审计日志）
- 在空状态和每条回答的来源区域旁显示

### 4.12 错误体验优化

**现状**: 错误提示笼统，恢复操作不明确

**方案**: 分类错误 + 明确恢复操作

```
┌──────────────────────────────────────────────────────┐
│ 🤖 EchoMind · 9:32 AM                                │
│ ┌──────────────────────────────────────────────────┐ │
│ │ ⚠️ 网络连接中断                                    │ │
│ │                                                    │ │
│ │ 原因：无法连接到 API 服务器                         │ │
│ │ 建议：检查网络连接后重试                            │ │
│ │                                                    │ │
│ │ [🔄 重试]  [⚙️ 检查设置]  [💾 本地模式]             │ │
│ └──────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────┘
```

| 错误类型 | 标题 | 原因说明 | 恢复操作 |
|---|---|---|---|
| 网络中断 | ⚠️ 网络连接中断 | 无法连接到 API 服务器 | [重试] [检查设置] [本地模式] |
| API Key 无效 | 🔑 认证失败 | API Key 无效或已过期 | [打开设置] [切换本地模式] |
| 限流 | ⏱️ 请求过快 | API 调用频率超限 | [等待 30s] [切换模型] |
| 上下文超限 | 📏 上下文过长 | 对话历史超出模型窗口 | [新建会话] [压缩历史] |
| 知识库为空 | 📚 知识库为空 | 请先导入文档 | [导入文件] |
| 模型加载失败 | 💻 本地模型错误 | GGUF 文件加载失败 | [检查模型] [切换云端] |

### 4.13 重新生成轮播

**现状**: 重新生成追加到末尾

**方案**: 改为轮播模式（ChatGPT/Claude 方式）

```
┌──────────────────────────────────────────────────────┐
│ 🤖 EchoMind · claude-3.5-sonnet · 9:32 AM           │
│ ┌──────────────────────────────────────────────────┐ │
│ │ 根据《劳动合同法》第 44 条...                      │ │
│ │ ...（回答内容）                                   │ │
│ └──────────────────────────────────────────────────┘ │
│                                                      │
│  ◀ 1/2 ▶   [🔄 重新生成]                             │
│                                                      │
│ 点击 ◀ ▶ 可在多个生成结果间切换                        │
│ [复制] [复制纯文本] [👍] [👎]                         │
└──────────────────────────────────────────────────────┘
```

- 多次重新生成的回答放入轮播
- `1/2` 表示第 1 个共 2 个版本
- 左右箭头或键盘 ← → 切换
- 每个版本独立保留来源和操作

### 4.14 点赞/踩反馈

```
┌──────────────────────────────────────────────────────┐
│ [复制] [复制纯文本] [重新生成] [👍 1] [👎 0] [分享]   │
└──────────────────────────────────────────────────────┘
```

- 点击 👍 标记"有帮助"
- 点击 👎 弹出小框："哪里有问题？" → 选项：信息不准确 / 不完整 / 格式问题 / 其他
- 反馈数据本地存储，用于未来检索优化（反馈检索记忆引擎）
- 不发送到云端，保护隐私

---

## 五、交互细节规范

### 5.1 键盘快捷键

| 快捷键 | 功能 | 现状 |
|---|---|---|
| `Enter` | 发送 | ✅ 已有 |
| `Shift+Enter` | 换行 | ✅ 已有 |
| `Esc` | 取消编辑/关闭弹框 | ✅ 已有 |
| `Cmd/Ctrl+K` | 命令面板 | ✅ 已有 |
| `Cmd/Ctrl+Enter` | 强制发送（替代 Enter） | 🆕 新增 |
| `↑` (空输入框时) | 编辑上一条用户消息 | 🆕 新增 |
| `Cmd/Ctrl+←` | 轮播上一个生成版本 | 🆕 新增 |
| `Cmd/Ctrl+→` | 轮播下一个生成版本 | 🆕 新增 |
| `/` (空输入框时) | 触发快捷指令 | 🆕 新增 |
| `@` | 触发文档引用 | 🆕 新增 |

### 5.2 滚动行为

| 场景 | 行为 |
|---|---|
| 用户在底部 + 新 token 到达 | 自动滚动到底部 |
| 用户滚向上方 + 新 token 到达 | 锁定位置，显示"回到底部"按钮 |
| 用户点击"回到底部" | 平滑滚动到底部 |
| 加载历史会话 | 滚动到底部 |
| 新建会话 | 清空 + 滚动到底部 |

### 5.3 消息操作栏显隐

| 平台 | 触发方式 | 消失方式 |
|---|---|---|
| 桌面 | 鼠标悬停消息块 | 鼠标离开 |
| 移动 | 长按消息块 | 点击其他区域 |

### 5.4 动画规范

| 动画 | 时长 | 缓动 |
|---|---|---|
| 消息淡入 | 300ms | ease-out |
| 操作栏淡入 | 150ms | ease-out |
| 来源卡片展开 | 200ms | cubic-bezier(0.4,0,0.2,1) |
| 输入框高度变化 | 100ms | ease-out |
| 光标闪烁 | 600ms | step-start |
| 轮播切换 | 200ms | ease-in-out |
| "回到底部"按钮 | 150ms | ease-out |

所有动画在 `prefers-reduced-motion: reduce` 时禁用。

### 5.5 阅读宽度

| 区域 | 最大宽度 | 理由 |
|---|---|---|
| 消息流容器 | 768px | WCAG 2.2 推荐 65-80 字符/行 |
| 代码块 | 748px | 留 20px padding |
| 来源卡片 | 768px | 与消息流对齐 |
| 后续建议 | 768px | 与消息流对齐 |
| 输入框 | 768px | 与消息流对齐 |

---

## 六、技术实现路径

### 6.1 分阶段实施

| 阶段 | 功能 | 优先级 | 工作量 | 依赖 |
|---|---|---|---|---|
| **P0** | 光标动画 + 智能滚动 + Queued 状态 | 🔴 高 | 小 | 无 |
| **P0** | 空状态重设计（知识库摘要 + 推荐问题） | 🔴 高 | 中 | 后端 API |
| **P0** | 模型名称显示 | 🔴 高 | 小 | 后端已有数据 |
| **P1** | 后续问题建议 | 🟡 中 | 中 | LLM 生成 |
| **P1** | 用户消息编辑 + 分叉 | 🟡 中 | 中 | 后端 API |
| **P1** | 错误体验优化（分类 + 恢复操作） | 🟡 中 | 小 | 无 |
| **P1** | 扁平全宽布局改造 | 🟡 中 | 中 | 无 |
| **P2** | 上下文窗口指示 | 🟡 中 | 小 | 后端 token 计数 |
| **P2** | 快捷指令（/summary 等） | 🟡 中 | 中 | 后端 prompt 模板 |
| **P2** | 文档范围筛选（@-syntax） | 🟡 中 | 中 | 后端检索过滤 |
| **P2** | 重新生成轮播 | 🟢 低 | 中 | 前端状态管理 |
| **P3** | 点赞/踩反馈 | 🟢 低 | 小 | 本地存储 |
| **P3** | 隐私状态可视化 | 🟢 低 | 小 | 后端已有数据 |
| **P3** | 流式期间排队发送 | 🟢 低 | 小 | 前端队列 |

### 6.2 后端 API 新增

| API | 功能 |
|---|---|
| `get_kb_summary()` | 知识库摘要（文档数/chunk数/向量数/最后更新） |
| `get_followup_suggestions(conversation_id, message_id)` | 基于当前对话生成后续问题建议 |
| `edit_user_message(message_id, new_content)` | 编辑用户消息 + 分叉 |
| `get_context_usage(conversation_id)` | 上下文窗口使用量（token 数 / 模型窗口大小） |
| `feedback_message(message_id, is_helpful, reason?)` | 消息反馈（本地存储） |

### 6.3 前端模块新增

| 模块 | 职责 |
|---|---|
| `ui/src/caret.js` | 流式光标管理 |
| `ui/src/scroll-lock.js` | 智能滚动锁定 + "回到底部"按钮 |
| `ui/src/empty-state.js` | 空状态渲染（知识库摘要 + 推荐问题） |
| `ui/src/followup.js` | 后续问题建议渲染 |
| `ui/src/message-edit.js` | 用户消息编辑 + 分叉 |
| `ui/src/slash-commands.js` | 快捷指令面板 |
| `ui/src/doc-mention.js` | @-syntax 文档引用弹框 |
| `ui/src/context-bar.js` | 上下文窗口指示器 |
| `ui/src/model-badge.js` | 模型名称显示与切换 |
| `ui/src/error-detail.js` | 分类错误 + 恢复操作 |
| `ui/src/regen-carousel.js` | 重新生成轮播 |
| `ui/src/feedback.js` | 点赞/踩反馈 |

---

## 七、竞品差异化总结

| 差异化点 | EchoMind 特色 | 竞品参考 |
|---|---|---|
| **隐私状态可视化** | 加密锁 + PII 脱敏标记 + 审计链状态 | 无竞品有此功能 |
| **本地/云端模型一键切换** | 输入框旁模型切换器，状态可见 | ChatGPT 有但不显示在每条消息 |
| **知识库摘要** | 空状态展示文档数/chunk数/向量维度 | 无竞品有此功能 |
| **文档范围筛选** | @-syntax 引用特定文档 | Cursor 有 @file 但用途不同 |
| **PII 脱敏标记** | 回答中脱敏内容用 `[REDACTED]` 标记 | 无竞品有此功能 |
| **审计链状态** | 回答旁显示"审计链完整 ✓" | 无竞品有此功能 |
| **本地模型 warm-up** | 切换到本地模式时后台预热 | 无竞品有此功能 |
| **混合搜索可见** | 输入框显示"向量+关键词"状态 | Perplexity 有 Focus 但不同 |

---

## 八、验收标准

| AC ID | 验收条件 |
|---|---|
| AC-QA-001 | 流式生成时末尾有闪烁光标，完成后光标消失 |
| AC-QA-002 | 用户滚动到上方时自动滚动锁定，显示"回到底部"浮动按钮 |
| AC-QA-003 | 空状态显示知识库摘要（文档数/chunk 数）+ 3 个推荐问题 |
| AC-QA-004 | 每条 assistant 消息显示模型名称（云端/本地 + 模型名） |
| AC-QA-005 | 回答完成后显示 2-3 个后续问题建议，可关闭 |
| AC-QA-006 | 用户消息可点击编辑，编辑后重新发送并分叉 |
| AC-QA-007 | 重新生成的回答以轮播方式展示，可左右切换 |
| AC-QA-008 | 错误提示分类显示（网络/认证/限流/上下文超限）+ 恢复操作按钮 |
| AC-QA-009 | 输入框中 `/` 触发快捷指令面板 |
| AC-QA-010 | 输入框中 `@` 触发文档选择弹框 |
| AC-QA-011 | 上下文窗口使用量以进度条显示（绿/黄/红三色） |
| AC-QA-012 | 流式期间输入框可用，可排队下一条问题 |
| AC-QA-013 | 消息操作栏包含点赞/踩按钮 |
| AC-QA-014 | 所有动画在 `prefers-reduced-motion: reduce` 时禁用 |
| AC-QA-015 | 消息流最大宽度 768px，代码块有复制按钮 |

---

## 九、设计参考来源

| 来源 | URL | 关键贡献 |
|---|---|---|
| setproduct.com | https://www.setproduct.com/blog/ai-chat-interface-ui-design | 九大解剖区域 + 六大消息状态 + 反模式清单 + 发布前检查清单 |
| Mind the Product | https://www.mindtheproduct.com/deep-dive-ux-best-practices-for-ai-chatbots/ | 九大 UX 最佳实践（有效性/效率平衡/尊重安全/上下文感知/期望管理/UI 协调/动态反馈/精准部署/性能评估/信任透明） |
| Agentic Design | https://agentic-design.ai/patterns/ui-ux-patterns/conversational-interface-patterns | 对话式界面模式（多模态/渐进式展示/信心可视化/混合主动性/Agent 状态） |
| ChatGPT | 实际使用 | 模型选择器/轮播重新生成/Artifacts 侧栏/代码块复制 |
| Claude.ai | 实际使用 | 全宽扁平布局/Projects/Artifacts/思考折叠/Claude 模型标注 |
| Perplexity | 实际使用 | 编号引用/Related 建议/Focus 模式/来源卡片/自动滚动锁定 |
| Cursor | 实际使用 | IDE 侧栏/@-syntax/Inline diff/Apply 按钮/Agent 模式 |
| Notion AI | 实际使用 | 内联动作/文档作用域/轻量侧栏/命令式输入 |

---

> **下一步**: 评审通过后，按 P0→P1→P2→P3 优先级分阶段实施。每个阶段遵循 TDD（先写测试再实现），确保 `cargo clippy --all-targets -- -D warnings` 零警告，`cargo fmt --check` 通过。
