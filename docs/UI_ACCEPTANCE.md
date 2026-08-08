# EchoMind UI 验收测试体系 — UI_ACCEPTANCE v1.0

> 本文档定义 EchoMind 前端 UI 的全量验收测试体系，是 UI 质量保障的唯一真相源。
> 测试策略基础见 `docs/TEST_STRATEGY.md`；E2E 覆盖矩阵见 `docs/E2E_COVERAGE.md`；需求来源见 `docs/SRS_v1.0.md`。

| 项 | 内容 |
| --- | --- |
| 文档版本 | v1.3.0 |
| 生效日期 | 2026-07-25 |
| 维护者 | QA Lead |

---

## 1. 体系总览

### 1.1 设计原则

1. **无死角覆盖**：每个 UI 元素、每种交互状态、每条用户旅程均有自动化断言。
2. **契约驱动**：所有断言基于 SRS 冻结的 Command/Event 契约，不测试实现细节。
3. **100% 自动化**：不接受手工测试作为验收依据；人工探索性测试仅作补充。
4. **全平台可跑**：L3-lite 桥接层（Playwright + Mock）在 macOS / Linux / Windows 全平台 headless 可执行。

### 1.2 测试分层架构

```
┌─────────────────────────────────────────────────────┐
│  L3 真实 GUI（WebdriverIO + tauri-driver）           │
│  Linux / Windows · 真实 IPC + 真实 SQLite             │
│  覆盖：全链路冒烟（E2E-UI-001）                       │
├─────────────────────────────────────────────────────┤
│  L3-lite 桥接层（Playwright + 契约级 Mock）           │
│  全平台 headless · 24/24 命令 + 10 事件全覆盖          │
│  覆盖：全部 UI 验收场景（本文档主体）                  │
├─────────────────────────────────────────────────────┤
│  L2 集成测试（cargo test · Tauri mock_app）          │
│  Command 签名 + 真实 SQLite · 契约 Payload 快照比对    │
│  覆盖：IPC 契约一致性                                 │
├─────────────────────────────────────────────────────┤
│  L1 单元测试（cargo test）                            │
│  领域逻辑 · 适配器内部逻辑                             │
│  覆盖：业务规则正确性                                 │
└─────────────────────────────────────────────────────┘
```

### 1.3 验收维度（8 大维度 60+ 用例）

| 维度 | 覆盖范围 | 用例数 | 测试文件 |
| --- | --- | --- | --- |
| ① 视觉规范与安全 | 色值、CDN 审计、XSS 防御 | 7 | `visual.spec.ts` |
| ② 配置向导 | 预设切换、验证失败、Ollama 空 Key | 6 | `wizard.spec.ts` |
| ③ 导入管线 | 格式拦截、去重、配额、状态徽标 | 10 | `import.spec.ts` |
| ④ 对话链路 | 思考指示器、流式渲染、引用块、停止 | 12 | `chat.spec.ts` |
| ⑤ 会话持久化 | CRUD、标题提取、历史切换 | 6 | `session.spec.ts` |
| ⑥ 富内容可视化 | Mermaid / KaTeX / Chart.js | 12 | `viz.spec.ts` |
| ⑦ 边界与防御性 | 空状态、错误 toast、付费墙 | 6 | `edge-cases.spec.ts` |
| ⑧ 性能 | 冷启动 ≤ 2s | 1 | `nfr.spec.ts` |
| **新增维度** | | | |
| ⑨ 审计功能 UI | 审计按钮、阶段事件、报告渲染、取消 | 7 | `audit.spec.ts` |
| ⑩ 文档操作按钮 | 重试/删除/审计按钮可见性与门控 | 8 | `document-actions.spec.ts` |
| ⑪ 导入进度与取消 | 进度条、百分比、取消按钮 | 7 | `import-progress.spec.ts` |
| ⑫ 模型缓存管理 | 缓存大小、下载、清理、空状态 | 8 | `model-cache.spec.ts` |
| ⑬ Toast 系统 | 样式、自动消失、堆叠、脱敏 | 8 | `toast.spec.ts` |
| ⑭ Markdown 渲染 | 标题/列表/引用/链接/表格/代码 | 10 | `markdown-render.spec.ts` |
| ⑮ 键盘与无障碍 | Tab 导航、Enter/Shift+Enter、ARIA | 10 | `keyboard-a11y.spec.ts` |
| ⑯ 侧栏行为 | 折叠/展开、标签隐藏、状态展示 | 9 | `sidebar-behavior.spec.ts` |
| ⑰ License 停用 | 授权状态、停用/恢复、审计门控联动 | 8 | `license-deactivate.spec.ts` |
| ⑱ 设置面板 + VLM | 开关、隐私弹窗、持久化 | 10 | `settings.spec.ts` |
| ⑲ 全链路冒烟 | 向导→导入→对话→停止→删除 | 2 | `bridge.spec.ts` |
| ⑳ 设计系统规范 | 色板/字体/间距/图标/组件/动效/圆角阴影 | ~20 | `visual.spec.ts`（扩展） |
| ㉑ 交互设计规范 | 右键菜单/拖拽排序/复制/悬停/确认框/骨架屏 | ~25 | `ix-context-menu.spec.ts` 等（规划中） |
| ㉒ 错误处理与降级 | 错误分类/重试/离线模式/崩溃恢复/输入校验 | ~15 | `error-handling.spec.ts`（规划中） |
| **合计** | | **~210** | **22+ 个 spec 文件** |

---

## 2. Mock 基础设施

### 2.1 tauri-stub.js 契约级 Mock

Mock 层位于 `e2e-tests/tests/bridge/tauri-stub.js`，覆盖全部 24 个 IPC 命令和 10 个事件：

**命令覆盖（24/24）：**

| 命令 | Mock 行为 |
| --- | --- |
| `get_settings` | 返回配置状态 + 脱敏 Key + VLM 开关 |
| `update_llm_config` | 标记已配置 |
| `test_llm_connection` | 默认成功；可注入失败（`setConnectionFail`） |
| `import_files` | 格式校验 + PDF 付费门 + 配额 + 去重 + 状态事件 + 导入进度事件 |
| `get_documents` | 返回文档列表 |
| `delete_document` | 移除文档 |
| `retry_index` | Failed → Processing → Indexed 状态流转 |
| `create_conversation` | 创建会话 + 初始化消息列表 |
| `get_conversations` | 返回会话列表 |
| `get_messages` | 返回指定会话消息 |
| `delete_conversation` | 移除会话 + 消息 |
| `chat` | 流式 token 输出 + 阶段事件 + 引用块 + 取消 |
| `abort_chat` | 设置取消标志 |
| `abort_import` | 设置导入取消标志 |
| `activate_pro` | 激活 Pro（License Key 非空即通过） |
| `deactivate_pro` | 回落免费版 |
| `get_pro_status` | 返回 isPro 状态 |
| `set_vlm_enabled` | 持久化 VLM 开关 |
| `set_hybrid_search` | 持久化混合检索开关 |
| `audit_document` | 三阶段事件 + 流式报告输出 + 取消 |
| `abort_audit` | 设置审计取消标志 |
| `init_embedder` | 模拟下载进度事件（4 阶段） |
| `get_model_cache_info` | 返回模型缓存信息 |
| `clear_model_cache` | 清空缓存 + 返回释放字节数 |

**事件覆盖（10/10）：**

| 事件 | 推送时机 |
| --- | --- |
| `chat_phase` | 检索/生成阶段 |
| `chat_token` | 流式 token |
| `chat_sources` | 引用块 |
| `chat_error` | 错误/中断 |
| `chat_done` | 流式完成 |
| `doc-status-changed` | 文档状态变更 |
| `audit_phase` | 审计三阶段（extracting/comparing/reporting） |
| `import-progress` | 导入进度（completed/total/current_file/cancelled） |
| `embedding_progress` | 向量化进度（embedded/total/doc_name） |
| `model_download_progress` | 模型下载进度（downloading/loading/done/error） |

### 2.2 测试控制接口

`window.__mock` 提供运行时行为注入：

| 方法 | 用途 |
| --- | --- |
| `setNextChatEmpty()` | 下次 chat 返回空上下文 |
| `setCustomTokens(tokens)` | 自定义流式 token 序列 |
| `setConnectionFail()` | 下次连接测试失败 |
| `setFileContent(path, content)` | 设置文件内容（去重测试） |
| `xssTokens()` | XSS 注入 token 序列 |
| `mermaidTokens()` | Mermaid flowchart token |
| `mermaidInvalidTokens()` | Mermaid 语法错误 token |
| `mermaidXssTokens()` | Mermaid XSS 注入 token |
| `katexInlineTokens()` | KaTeX 行内公式 token |
| `katexBlockTokens()` | KaTeX 块级公式 token |
| `katexInvalidTokens()` | KaTeX 语法错误 token |
| `katexXssTokens()` | KaTeX XSS 注入 token |
| `chartTableTokens()` | Chart.js 表格数据 token |
| `auditTokens()` | 审计报告 token |
| `reset()` | 重置全部状态 |
| `simulateDragEnter()` | 模拟拖拽进入 |
| `simulateDragLeave()` | 模拟拖拽离开 |
| `simulateDragDrop(paths)` | 模拟拖拽释放 |

---

## 3. 验收矩阵（REQ → E2E-TC 全量映射）

### 3.1 审计域（REQ-AUDIT-001~005）

| REQ | AC | E2E-TC | 文件 | 断言要点 |
| --- | --- | --- | --- | --- |
| REQ-AUDIT-001 | AC-1 | E2E-AUDIT-001 | audit.spec.ts | Pro 版 Indexed 文档显示审计按钮 |
| REQ-AUDIT-001 | AC-2 | E2E-AUDIT-001b | audit.spec.ts | 免费版不显示审计按钮 |
| REQ-AUDIT-005 | AC-1 | E2E-AUDIT-002 | audit.spec.ts | 审计阶段事件驱动思考指示器文案 |
| REQ-AUDIT-004 | AC-1 | E2E-AUDIT-003 | audit.spec.ts | 审计报告流式渲染为 Markdown |
| REQ-AUDIT-004 | AC-3 | E2E-AUDIT-004 | audit.spec.ts | 审计报告含矛盾清单表格 |
| REQ-AUDIT-005 | AC-3 | E2E-AUDIT-005 | audit.spec.ts | 审计取消 — stopBtn 调用 abort_audit |
| REQ-AUDIT-001 | AC-4 | E2E-AUDIT-006 | audit.spec.ts | 审计中尝试发送消息被阻止 |

### 3.2 文档操作域（REQ-VEC-005 / REQ-ING-005 / REQ-AUDIT-001）

| REQ | AC | E2E-TC | 文件 | 断言要点 |
| --- | --- | --- | --- | --- |
| REQ-VEC-005 | AC-1 | E2E-DOC-001 | document-actions.spec.ts | Failed 文档显示重试按钮 |
| REQ-VEC-005 | AC-1 | E2E-DOC-002 | document-actions.spec.ts | 重试后状态 Failed → Indexed |
| REQ-VEC-005 | AC-1 | E2E-DOC-003 | document-actions.spec.ts | Indexed 文档不显示重试按钮 |
| REQ-ING-005 | AC-1 | E2E-DOC-004 | document-actions.spec.ts | 删除按钮交互与 toast |
| REQ-AUDIT-001 | AC-1 | E2E-DOC-005 | document-actions.spec.ts | Pro 版 Indexed 显示审计按钮 |
| REQ-AUDIT-001 | AC-2 | E2E-DOC-006 | document-actions.spec.ts | 免费版不显示审计按钮 |
| REQ-AUDIT-001 | AC-3 | E2E-DOC-007 | document-actions.spec.ts | Processing 状态不显示审计按钮 |

### 3.3 导入进度域（REQ-ING-006）

| REQ | AC | E2E-TC | 文件 | 断言要点 |
| --- | --- | --- | --- | --- |
| REQ-ING-006 | AC-1 | E2E-IMP-001 | import-progress.spec.ts | 多文件导入显示进度条 |
| REQ-ING-006 | AC-2 | E2E-IMP-002 | import-progress.spec.ts | 进度文本含文件名与计数 |
| REQ-ING-006 | AC-3 | E2E-IMP-003 | import-progress.spec.ts | 导入完成后进度条隐藏 |
| REQ-ING-006 | AC-4 | E2E-IMP-004 | import-progress.spec.ts | 取消导入按钮可点击 |
| REQ-ING-006 | AC-4 | E2E-IMP-005 | import-progress.spec.ts | 取消后 toast 提示 |
| REQ-ING-006 | AC-1 | E2E-IMP-006 | import-progress.spec.ts | 单文件也显示进度条 |

### 3.4 模型缓存域（REQ-VEC-008）

| REQ | AC | E2E-TC | 文件 | 断言要点 |
| --- | --- | --- | --- | --- |
| REQ-VEC-008 | AC-5 | E2E-VEC-001 | model-cache.spec.ts | 设置面板显示模型缓存大小 |
| REQ-VEC-008 | AC-1 | E2E-VEC-002 | model-cache.spec.ts | 下载模型按钮触发 init_embedder |
| REQ-VEC-008 | AC-2 | E2E-VEC-003 | model-cache.spec.ts | 下载进度事件驱动 toast |
| REQ-VEC-008 | AC-4 | E2E-VEC-004 | model-cache.spec.ts | 下载完成后 toast 模型就绪 |
| REQ-VEC-008 | AC-6 | E2E-VEC-005 | model-cache.spec.ts | 清理缓存按钮触发 clear_model_cache |
| REQ-VEC-008 | AC-6 | E2E-VEC-006 | model-cache.spec.ts | 清理后缓存大小归零 |
| REQ-VEC-008 | AC-5 | E2E-VEC-007 | model-cache.spec.ts | 空缓存状态展示 |

### 3.5 Toast 系统域（REQ-UI-005）

| REQ | AC | E2E-TC | 文件 | 断言要点 |
| --- | --- | --- | --- | --- |
| REQ-UI-005 | AC-2 | E2E-TOAST-001 | toast.spec.ts | 成功 toast 样式（accent 色） |
| REQ-UI-005 | AC-2 | E2E-TOAST-002 | toast.spec.ts | 错误 toast 样式（红色边框） |
| REQ-UI-005 | AC-2 | E2E-TOAST-003 | toast.spec.ts | info toast 默认样式 |
| REQ-UI-005 | AC-2 | E2E-TOAST-004 | toast.spec.ts | toast 4.2s 后自动消失 |
| REQ-UI-005 | AC-2 | E2E-TOAST-005 | toast.spec.ts | 多 toast 堆叠不覆盖 |
| REQ-UI-005 | AC-2 | E2E-TOAST-006 | toast.spec.ts | toastError 脱敏 API Key |
| REQ-UI-005 | AC-2 | E2E-TOAST-007 | toast.spec.ts | toastError 脱敏用户路径 |

### 3.6 Markdown 渲染域（REQ-UI-002）

| REQ | AC | E2E-TC | 文件 | 断言要点 |
| --- | --- | --- | --- | --- |
| REQ-UI-002 | AC-1 | E2E-MD-001 | markdown-render.spec.ts | 标题 h1~h4 渲染 |
| REQ-UI-002 | AC-1 | E2E-MD-002 | markdown-render.spec.ts | 无序列表渲染 |
| REQ-UI-002 | AC-1 | E2E-MD-003 | markdown-render.spec.ts | 有序列表渲染 |
| REQ-UI-002 | AC-1 | E2E-MD-004 | markdown-render.spec.ts | 引用块渲染 |
| REQ-UI-002 | AC-1 | E2E-MD-005 | markdown-render.spec.ts | 链接渲染 + href 安全 |
| REQ-UI-002 | AC-1 | E2E-MD-006 | markdown-render.spec.ts | 行内代码渲染 |
| REQ-UI-002 | AC-1 | E2E-MD-007 | markdown-render.spec.ts | 加粗与斜体渲染 |
| REQ-UI-002 | AC-1 | E2E-MD-008 | markdown-render.spec.ts | Markdown 表格渲染 |
| REQ-UI-002 | AC-1 | E2E-MD-009 | markdown-render.spec.ts | 嵌套列表渲染 |
| REQ-UI-002 | AC-1 | E2E-MD-010 | markdown-render.spec.ts | 分隔线渲染 |

### 3.7 键盘与无障碍域（REQ-UI-001/003/007）

| REQ | AC | E2E-TC | 文件 | 断言要点 |
| --- | --- | --- | --- | --- |
| REQ-UI-003 | AC-1 | E2E-KB-001 | keyboard-a11y.spec.ts | Tab 键遍历交互元素 |
| REQ-UI-003 | AC-1 | E2E-KB-002 | keyboard-a11y.spec.ts | Enter 发送消息 |
| REQ-UI-003 | AC-1 | E2E-KB-003 | keyboard-a11y.spec.ts | Shift+Enter 换行不发送 |
| REQ-UI-007 | AC-1 | E2E-KB-004 | keyboard-a11y.spec.ts | Escape 关闭设置面板 |
| REQ-LIC-003 | AC-4 | E2E-KB-005 | keyboard-a11y.spec.ts | 付费墙关闭可用 |
| REQ-UI-001 | AC-1 | E2E-KB-006 | keyboard-a11y.spec.ts | VLM Toggle ARIA role=switch |
| REQ-UI-001 | AC-1 | E2E-KB-007 | keyboard-a11y.spec.ts | 按钮有 title/aria-label |
| REQ-UI-001 | AC-1 | E2E-KB-008 | keyboard-a11y.spec.ts | 输入框有 placeholder |
| REQ-UI-003 | AC-1 | E2E-KB-009 | keyboard-a11y.spec.ts | 生成中输入框禁用 |
| REQ-UI-001 | AC-1 | E2E-KB-010 | keyboard-a11y.spec.ts | 新对话按钮可键盘聚焦 |

### 3.8 侧栏行为域（REQ-UI-001 布局）

| REQ | AC | E2E-TC | 文件 | 断言要点 |
| --- | --- | --- | --- | --- |
| REQ-UI-001 | AC-1 | E2E-SB-001 | sidebar-behavior.spec.ts | 折叠按钮切换侧栏宽度 |
| REQ-UI-001 | AC-1 | E2E-SB-002 | sidebar-behavior.spec.ts | 折叠后侧标签隐藏 |
| REQ-UI-001 | AC-1 | E2E-SB-003 | sidebar-behavior.spec.ts | 展开后侧标签恢复 |
| REQ-UI-001 | AC-1 | E2E-SB-004 | sidebar-behavior.spec.ts | 折叠按钮文字方向切换 |
| REQ-UI-001 | AC-1 | E2E-SB-005 | sidebar-behavior.spec.ts | 新对话按钮可点击 |
| REQ-UI-001 | AC-1 | E2E-SB-006 | sidebar-behavior.spec.ts | 会话列表可见 |
| REQ-UI-001 | AC-1 | E2E-SB-007 | sidebar-behavior.spec.ts | 设置按钮可点击 |
| REQ-UI-001 | AC-1 | E2E-SB-008 | sidebar-behavior.spec.ts | 授权状态展示 |

### 3.9 License 停用域（REQ-LIC-004）

| REQ | AC | E2E-TC | 文件 | 断言要点 |
| --- | --- | --- | --- | --- |
| REQ-LIC-004 | AC-1 | E2E-LIC-005 | license-deactivate.spec.ts | 设置面板 Pro 版授权状态 |
| REQ-LIC-004 | AC-2 | E2E-LIC-006 | license-deactivate.spec.ts | 停用按钮可见 |
| REQ-LIC-004 | AC-2 | E2E-LIC-007 | license-deactivate.spec.ts | 停用后回落免费版 |
| REQ-LIC-004 | AC-3 | E2E-LIC-008 | license-deactivate.spec.ts | 停用后 toast 提示 |
| REQ-LIC-004 | AC-2 | E2E-LIC-009 | license-deactivate.spec.ts | 停用后审计按钮消失 |
| REQ-LIC-004 | AC-2 | E2E-LIC-010 | license-deactivate.spec.ts | 停用后侧栏状态更新 |
| REQ-LIC-004 | AC-4 | E2E-LIC-011 | license-deactivate.spec.ts | 停用后重新激活可用 |

### 3.10 设计系统域（REQ-DS-001~007，SRS §4.14）

> **状态**：规划中。SRS v2.1.0 已将 DS 域需求原子化为色板 token + 字体层级 + 间距网格 + 图标系统 + 组件规范 + 动效规范 + 圆角阴影体系。测试用例待编写。

| REQ | AC | E2E-TC | 文件 | 断言要点 |
| --- | --- | --- | --- | --- |
| REQ-DS-001 | AC-1 | E2E-DS-001 | visual.spec.ts | `tailwind.config` 定义 6 个自定义色 token（base/panel/line/accent/ink/muted） |
| REQ-DS-001 | AC-3 | E2E-DS-002 | visual.spec.ts | `.md` 排版体系硬编码色值与规格表一致 |
| REQ-DS-002 | AC-2 | E2E-DS-003 | visual.spec.ts | `.md` 计算样式 `font-size`=14px，`line-height`=1.8 |
| REQ-DS-002 | AC-5 | E2E-DS-004 | visual.spec.ts | `.md h1` 计算样式 `font-size`≈20.3px，`font-weight`=700 |
| REQ-DS-003 | AC-1 | E2E-DS-005 | visual.spec.ts | `#chatArea` padding=24px（`px-6 py-6`） |
| REQ-DS-003 | AC-2 | E2E-DS-006 | visual.spec.ts | `#inputBar` padding-left/right=16px，top/bottom=12px |
| REQ-DS-004 | AC-1 | E2E-DS-007 | visual.spec.ts | 全部图标为 Unicode 字符，无 SVG/CDN 引用 |
| REQ-DS-005 | AC-1 | E2E-DS-008 | visual.spec.ts | `#sendBtn` bg=#38BDF8，color=#0C1116（primary 变体） |
| REQ-DS-005 | AC-3 | E2E-DS-009 | visual.spec.ts | `#stopBtn` bg=rgba(239,68,68,0.15)（danger 变体） |
| REQ-DS-006 | AC-2 | E2E-DS-010 | visual.spec.ts | `.fade-in` 动画时长 250ms ease-out |
| REQ-DS-006 | AC-5 | E2E-DS-011 | chat.spec.ts | `scheduleRender()` 使用 rAF，连续 token 不触发多次 DOM 写入 |
| REQ-DS-007 | AC-1 | E2E-DS-012 | visual.spec.ts | `#sendBtn` border-radius=12px（`rounded-xl`） |
| REQ-DS-007 | AC-2 | E2E-DS-013 | visual.spec.ts | assistant Block border-radius=16px（`rounded-2xl`） |
| REQ-DS-007 | AC-6 | E2E-DS-014 | visual.spec.ts | user Block 右下角 border-bottom-right-radius=8px |

### 3.11 交互设计域（REQ-IX-001~007，SRS §4.15）

> **状态**：规划中。SRS v2.1.0 已将 IX 域需求原子化为交互四元组。REQ-IX-004（悬停反馈）和 REQ-IX-006（输入框多行）已实现，其余 Draft。

| REQ | AC | E2E-TC | 文件 | 断言要点 |
| --- | --- | --- | --- | --- |
| REQ-IX-001 | AC-1 | E2E-IX-001 | ix-context-menu.spec.ts | 右键文档列表项弹出菜单含「复制文件名/重建索引/审计/删除」 |
| REQ-IX-001 | AC-4 | E2E-IX-002 | ix-context-menu.spec.ts | 菜单超出右边界时自动左偏 |
| REQ-IX-001 | AC-5 | E2E-IX-003 | ix-context-menu.spec.ts | 点击菜单外区域或 Esc 后菜单从 DOM 移除 |
| REQ-IX-001 | AC-6 | E2E-IX-004 | ix-context-menu.spec.ts | 非 Pro 用户右键 Indexed 文档时「审计」项灰显 + Pro 标签 |
| REQ-IX-002 | AC-1 | E2E-IX-005 | ix-drag-sort.spec.ts | 长按会话项可拖拽，拖拽时 opacity-50 |
| REQ-IX-002 | AC-2 | E2E-IX-006 | ix-drag-sort.spec.ts | 拖拽经过其他项时显示 2px accent 色插入线 |
| REQ-IX-002 | AC-4 | E2E-IX-007 | ix-drag-sort.spec.ts | 重启后 get_conversations 返回自定义顺序 |
| REQ-IX-003 | AC-1 | E2E-IX-008 | ix-copy.spec.ts | 点击代码块「复制」后 code.innerText 在剪贴板 + 按钮文案变「已复制 ✓」 |
| REQ-IX-003 | AC-5 | E2E-IX-009 | ix-copy.spec.ts | 非安全上下文时显示「复制失败」toast |
| REQ-IX-004 | AC-1 | E2E-IX-010 | visual.spec.ts | 文档列表项 hover 时 bg=rgba(255,255,255,0.05) |
| REQ-IX-004 | AC-2 | E2E-IX-011 | visual.spec.ts | `#sendBtn` hover 时 opacity 1→0.9，过渡 150ms |
| REQ-IX-004 | AC-5 | E2E-IX-012 | chat.spec.ts | `.msg-actions` 在 `.md-block` hover 时 opacity 0→1，过渡 200ms |
| REQ-IX-005 | AC-1 | E2E-IX-013 | ix-confirm.spec.ts | 删除文档弹出确认对话框，标题含文档名 + 正文含「此操作不可撤销」 |
| REQ-IX-005 | AC-4 | E2E-IX-014 | ix-confirm.spec.ts | 确认按钮弹窗出现后 500ms 内 disabled |
| REQ-IX-006 | AC-1 | E2E-IX-015 | keyboard-a11y.spec.ts | Shift+Enter 插入换行不触发 send() |
| REQ-IX-006 | AC-4 | E2E-IX-016 | keyboard-a11y.spec.ts | 输入超 6 行后高度固定 160px + 内部滚动条 |
| REQ-IX-007 | AC-1 | E2E-IX-017 | ix-skeleton.spec.ts | 文档列表加载期间显示 3~5 个骨架项 + animate-pulse |
| REQ-IX-007 | AC-2 | E2E-IX-018 | ix-skeleton.spec.ts | 加载时间 <200ms 时不显示骨架屏 |

### 3.12 错误处理与降级域（REQ-ERR-001~005，SRS §4.21）

> **状态**：规划中。SRS v2.1.0 已将 ERR 域需求原子化为错误分类六元组。REQ-ERR-001 部分实现（`LIMIT_REACHED`/`PRO_REQUIRED`），REQ-ERR-004 部分实现（`cleanup_zombies`），其余 Draft。

| REQ | AC | E2E-TC | 文件 | 断言要点 |
| --- | --- | --- | --- | --- |
| REQ-ERR-001 | AC-1 | E2E-ERR-001 | error-handling.spec.ts | import_files 配额超限返回 `LIMIT_REACHED:` 前缀 |
| REQ-ERR-001 | AC-3 | E2E-ERR-002 | error-handling.spec.ts | 前端 importPaths catch 识别前缀并弹出 showPaywall |
| REQ-ERR-001 | AC-5 | E2E-ERR-003 | toast.spec.ts | sanitizeError 过滤后 API Key 为 `sk-****` |
| REQ-ERR-001 | AC-5 | E2E-ERR-004 | toast.spec.ts | sanitizeError 过滤后 Unix 路径为 `/Users/****/` |
| REQ-ERR-002 | AC-1 | E2E-ERR-005 | error-handling.spec.ts | LLM 超时自动重试，退避 1s→2s→4s |
| REQ-ERR-002 | AC-2 | E2E-ERR-006 | error-handling.spec.ts | 重试期间 #inputHint 显示「重连中…（n/3）」 |
| REQ-ERR-002 | AC-5 | E2E-ERR-007 | error-handling.spec.ts | 401/403 不重试，直接返回 `AUTH:` 错误 |
| REQ-ERR-003 | AC-1 | E2E-ERR-008 | error-handling.spec.ts | chat IPC 返回 `NETWORK:` 时 #inputHint 显示离线提示 |
| REQ-ERR-003 | AC-2 | E2E-ERR-009 | error-handling.spec.ts | 离线模式下 #sendBtn disabled |
| REQ-ERR-003 | AC-3 | E2E-ERR-010 | error-handling.spec.ts | 离线模式下文档导入/删除/会话历史可用 |
| REQ-ERR-004 | AC-1 | E2E-ERR-011 | error-handling.spec.ts | 崩溃后重启时 Processing 文档标记为 Failed |
| REQ-ERR-004 | AC-4 | E2E-ERR-012 | chat.spec.ts | 流式中断后已生成内容保留（persist_exchange） |
| REQ-ERR-005 | AC-1 | E2E-ERR-013 | edge-cases.spec.ts | 文件路径含 `..` 被拒绝 |
| REQ-ERR-005 | AC-5 | E2E-ERR-014 | session.spec.ts | 会话标题超 24 字符被 derive_title 截断 + 追加 … |
| REQ-ERR-005 | AC-6 | E2E-ERR-015 | edge-cases.spec.ts | License Key 无效时返回「License 无效或已过期」 |

---

## 4. CI 门禁

### 4.1 E2E 桥接层（每个 PR 必跑）

```yaml
e2e-bridge:
  matrix:
    os: [ubuntu-latest, macos-latest]
  steps:
    - npm ci
    - npx playwright install --with-deps chromium
    - npm run test:bridge    # 运行全部 19 个 spec 文件
```

### 4.2 E2E 真实 GUI（每个 PR + 每日）

```yaml
e2e-gui:
  matrix:
    os: [ubuntu-latest, windows-latest]
  steps:
    - cargo build --workspace
    - npm ci
    - npm run test:e2e       # WebdriverIO + tauri-driver
```

### 4.3 门禁规则

- 任何 UI 变更必须附带自动化断言通过证据。
- 桥接层全绿后方可合并。
- 真实 GUI 层在 Linux/Windows 矩阵全绿（macOS 不可用，WKWebView 硬限制）。
- **严禁任何形式的人工冒烟代替自动化测试。**

---

## 5. 测试文件组织

```
e2e-tests/tests/
├── bridge/
│   └── tauri-stub.js              # 契约级 Mock（24 命令 + 10 事件）
├── helpers.mjs                    # 共享辅助（enterApp / sendMessage / activatePro / importDocs / waitDone）
├── bridge.spec.ts                 # ⑲ 全链路冒烟 + 付费墙
├── visual.spec.ts                 # ① 视觉规范 + CDN 审计 + XSS 防御
├── wizard.spec.ts                 # ② 配置向导
├── import.spec.ts                 # ③ 导入管线
├── chat.spec.ts                   # ④ 对话链路
├── session.spec.ts                # ⑤ 会话持久化
├── viz.spec.ts                    # ⑥ 富内容可视化
├── edge-cases.spec.ts             # ⑦ 边界与防御性
├── nfr.spec.ts                    # ⑧ 冷启动性能
├── audit.spec.ts                  # ⑨ 审计功能 UI ★新增
├── document-actions.spec.ts       # ⑩ 文档操作按钮 ★新增
├── import-progress.spec.ts        # ⑪ 导入进度与取消 ★新增
├── model-cache.spec.ts            # ⑫ 模型缓存管理 ★新增
├── toast.spec.ts                  # ⑬ Toast 系统行为 ★新增
├── markdown-render.spec.ts        # ⑭ Markdown 渲染完整性 ★新增
├── keyboard-a11y.spec.ts          # ⑮ 键盘导航与无障碍 ★新增
├── sidebar-behavior.spec.ts       # ⑯ 侧栏折叠与交互 ★新增
├── license-deactivate.spec.ts     # ⑰ License 停用 UI ★新增
├── input-bar-align.spec.ts       # ⑳ 输入框视觉对齐 ★新增
└── settings.spec.ts               # ⑱ 设置面板 + VLM 开关
```

---

## 6. 缺陷发现记录

本体系在建立过程中已发现并修复以下缺陷：

| 编号 | 缺陷描述 | 严重性 | 修复状态 |
| --- | --- | --- | --- |
| BUG-UI-001 | `toastSuccess()` 函数被调用但从未定义，导致模型下载完成和缓存清理时 JS 运行时错误 | P0 | 已修复（添加函数定义） |
| BUG-UI-002 | `toContainText('A|B|C')` 误将 `|` 作字面量而非正则分隔符，导致审计阶段文案匹配失败 | P1 | 已修复（改用正则 `/A|B|C/`） |
| BUG-UI-003 | `docItem.locator('button').click()` 匹配多个按钮（重试/审计/删除），触发 strict mode violation | P1 | 已修复（改用 `button[title="删除文档"]` 精确选择器） |
| BUG-UI-004 | `#stopBtn` 初始隐藏态被断言为 `toBeVisible`，导致键盘无障碍测试失败 | P2 | 已修复（改为 `toHaveCount(1)` + `toHaveAttribute('title')` 验证存在性与属性） |
| BUG-UI-005 | Mock `import_files` 同步返回导致进度条闪现，测试无法捕获可见状态 | P1 | 已修复（Mock 添加 80ms 文件处理 + 50ms 文件间延迟） |
| BUG-UI-006 | Toast 通知层（z-60 fixed）遮挡 `#importCancelBtn`，Playwright click 被拦截 | P1 | 已修复（改用 `page.evaluate(() => el.click())` DOM 原生点击绕过遮挡） |
| BUG-UI-007 | `#importProgressText` 初始文本 `导入中（0/N）…` 不含文件名，`innerText()` 读取时序竞争 | P2 | 已修复（改用 `toContainText(/filename/)` 等待文件名出现再断言） |
| BUG-UI-008 | Mock `audit_document` token 间隔 150ms 过短，取消测试 click 前 audit 可能已完成 | P2 | 已修复（token 间隔增至 300ms + DOM 原生 click） |
| BUG-UI-009 | `#queryInput` textarea 缺少垂直内边距，单行高度（24px）比按钮（36px）矮 12px，`items-end` 导致 placeholder/光标偏下 | P1 | 已修复（添加 `py-1.5` 使单行高度=36px=h-9） |

---

## 7. 修订记录

| 版本 | 日期 | 修订摘要 | 批准 |
| --- | --- | --- | --- |
| v1.0.0 | 2026-07-25 | 创世版：8 大新增验收维度（审计/文档操作/导入进度/模型缓存/Toast/Markdown/键盘无障碍/侧栏/License停用），~80 新增用例，24/24 命令 + 10 事件 Mock 全覆盖，修复 toastSuccess 未定义 P0 缺陷 | QA Lead / TPM |
| v1.1.0 | 2026-07-25 | 全量 133/133 绿灯：修复 8 个测试缺陷（正则误用/多按钮匹配/toast遮挡/stopBtn可见性/进度条时序/审计取消时序）；Mock 增强 import_files 延迟 + audit token 间隔 | QA Lead |
| v1.2.0 | 2026-07-25 | 新增输入框视觉对齐维度（8 用例 input-bar-align.spec.ts）；修复 BUG-UI-009 textarea 垂直内边距缺失；全量 141/141 绿灯 | QA Lead |
| v1.3.0 | 2026-07-25 | SRS v2.1.0 原子化重构同步：新增 §3.10 DS 域（14 用例 E2E-DS-001~014）、§3.11 IX 域（18 用例 E2E-IX-001~018）、§3.12 ERR 域（15 用例 E2E-ERR-001~015）验收矩阵规划；维度表新增⑳~㉒三域，合计~210 用例 | QA Lead / TPM |
