# EchoMind E2E 全量测试覆盖矩阵 — E2E_COVERAGE v1.0

> 本文档是 E2E 测试覆盖的唯一真相源，逐条映射 SRS 需求（REQ）到 E2E 测试用例（E2E-TC）。
> 测试策略基础见 `docs/TEST_STRATEGY.md`；需求来源见 `docs/SRS_v1.0.md`。

| 项 | 内容 |
| --- | --- |
| 文档版本 | v2.3.0 |
| 生效日期 | 2026-07-25 |
| 维护者 | QA Lead |

---

## 1. 覆盖范围与原则

### 1.1 覆盖目标
- **100% P0 用户旅程覆盖**：SRS 第 3 节定义的全部 P0 旅程必须有自动化 E2E 断言。
- **100% AC 覆盖**：每条 P0 需求的验收标准至少对应 1 个 E2E 断言。
- **死角清零**：视觉规范、安全防御、状态流转、边界条件全部纳入。

### 1.2 测试分层
| 层级 | 工具 | 范围 | 平台 |
| --- | --- | --- | --- |
| L3-lite 桥接 | Playwright + Mock `__TAURI__` | UI 行为断言（全量） | 全平台 headless |
| L3 真实 GUI | WebdriverIO + tauri-driver | 真实 IPC + 真实 SQLite | Linux/Windows |
| L2 集成 | `cargo test`（Tauri mock_app） | Command 签名 + 真实 SQLite | 全平台 |

### 1.3 桥接测试文件组织
```
e2e-tests/tests/
├── bridge/
│   └── tauri-stub.js          # 契约级 Mock（24 命令 + 10 事件全覆盖）
├── helpers.mjs                # 共享辅助（enterApp / sendMessage / activatePro / importDocs）
├── bridge.spec.ts             # 全链路冒烟 + 付费墙
├── visual.spec.ts             # 视觉规范 + CDN 审计 + XSS 防御
├── wizard.spec.ts             # 配置向导全场景
├── import.spec.ts             # 导入管线全场景
├── chat.spec.ts               # 对话链路全场景（含思考指示器）
├── session.spec.ts            # 会话持久化全场景
├── settings.spec.ts           # 设置面板 + VLM 开关 + 隐私弹窗
├── edge-cases.spec.ts         # 边界与防御性测试
├── viz.spec.ts                # 富内容可视化（Mermaid/KaTeX/Chart.js）
├── nfr.spec.ts                # 冷启动性能
├── audit.spec.ts              # 审计功能 UI ★新增
├── document-actions.spec.ts   # 文档操作按钮（重试/删除/审计门控）★新增
├── import-progress.spec.ts    # 导入进度与取消 ★新增
├── model-cache.spec.ts        # 模型缓存管理 ★新增
├── toast.spec.ts              # Toast 系统行为 ★新增
├── markdown-render.spec.ts    # Markdown 渲染完整性 ★新增
├── keyboard-a11y.spec.ts      # 键盘导航与无障碍 ★新增
├── sidebar-behavior.spec.ts   # 侧栏折叠与交互 ★新增
├── license-deactivate.spec.ts # License 停用 UI ★新增
├── input-bar-align.spec.ts   # 输入框视觉对齐 ★新增
└── settings.spec.ts           # 设置面板 + VLM 开关
```

---

## 2. REQ → E2E 追溯矩阵

### 2.1 UI 域

| REQ | AC | E2E-TC | 文件 | 断言要点 | 状态 |
| --- | --- | --- | --- | --- | --- |
| REQ-UI-001 | AC-1 | E2E-UI-002 | visual.spec.ts | 背景 `#161616`、强调色 `#38BDF8` 计算样式断言 | 新增 |
| REQ-UI-001 | AC-2 | E2E-UI-003 | visual.spec.ts | `ui/` 内无规范外硬编码色值（静态扫描） | 新增 |
| REQ-UI-001 | AC-3 | E2E-UI-004 | visual.spec.ts | `ui/` 无 `http(s)://` script/link 引用（CDN 审计） | 新增 |
| REQ-UI-002 | AC-1 | E2E-UI-005 | visual.spec.ts | 代码块语法高亮 + 复制按钮可见可点击 | 新增 |
| REQ-UI-002 | AC-2 | E2E-SEC-001 | visual.spec.ts | `<script>` / `onerror` / `javascript:` 注入载荷被 DOMPurify 消毒 | 新增 |
| REQ-UI-002 | AC-3 | E2E-UI-006 | chat.spec.ts | 流式半截代码围栏不产生布局错乱 | 新增 |
| REQ-UI-003 | AC-1 | E2E-UI-007 | chat.spec.ts | 生成中 Enter 与发送按钮均无效；停止按钮可见 | 新增 |
| REQ-UI-003 | AC-2 | E2E-UI-008 | chat.spec.ts | 后端报错时输入框进入错误态，不泄露 Key | 新增 |
| REQ-UI-004 | AC-1 | E2E-UI-009 | visual.spec.ts | 拖拽遮罩文案「数据仅本地处理」+ 强调色 | 新增 |
| REQ-UI-004 | AC-2 | E2E-UI-010 | visual.spec.ts | 遮罩出现时其余交互被遮罩层接管 | 新增 |
| REQ-UI-005 | AC-1 | E2E-UI-011 | edge-cases.spec.ts | 空知识库时主界面展示导入引导 | 新增 |
| REQ-UI-005 | AC-2 | E2E-UI-012 | edge-cases.spec.ts | 后端错误经 toast 展示，不含敏感信息 | 新增 |
| REQ-UI-006 | AC-1 | E2E-UI-013 | import.spec.ts | 索引状态徽标与事件驱动一致 | 新增 |
| REQ-UI-006 | AC-2 | E2E-UI-014 | import.spec.ts | 免费版配额 `n/50` 实时刷新 | 新增 |
| REQ-UI-007 | AC-1 | E2E-UI-015 | wizard.spec.ts | 未配置显示向导；已配置直接进入主界面 | 新增 |
| REQ-UI-007 | AC-2 | E2E-UI-016 | wizard.spec.ts | 验证失败内联展示错误，不进入主界面 | 新增 |
| REQ-UI-007 | AC-3 | E2E-UI-017 | wizard.spec.ts | Ollama 预设允许空 API Key | 新增 |
| REQ-UI-008 | AC-1 | TC-UI-001 | L2 集成 | 配置持久化跨重启 | 已有 |
| REQ-UI-008 | AC-3 | E2E-UI-018 | wizard.spec.ts | 配置后运行态立即生效 | 新增 |

### 2.2 ING 域

| REQ | AC | E2E-TC | 文件 | 断言要点 | 状态 |
| --- | --- | --- | --- | --- | --- |
| REQ-ING-001 | AC-1 | E2E-UI-001 | bridge.spec.ts | 拖入文件后出现在列表，初始状态「待索引」 | 已有 |
| REQ-ING-001 | AC-2 | E2E-ING-001 | import.spec.ts | 拖入即显示遮罩；拖出未释放遮罩消失 | 新增 |
| REQ-ING-001 | AC-3 | E2E-ING-002 | import.spec.ts | 不支持格式被拦截，toast 说明原因 | 新增 |
| REQ-ING-002 | AC-1 | E2E-LIC-001 | bridge.spec.ts | 免费版 PDF 弹付费墙 | 已有 |
| REQ-ING-002 | AC-2 | E2E-ING-003 | import.spec.ts | Pro 版导入 PDF 正常入库 | 新增 |
| REQ-ING-002 | AC-3 | E2E-ING-004 | import.spec.ts | `.exe`/`.docx` 被拒绝并提示支持格式 | 新增 |
| REQ-ING-003 | AC-1 | E2E-ING-005 | import.spec.ts | 文件选择器多选后批量入库 | 新增 |
| REQ-ING-004 | AC-1 | E2E-ING-006 | import.spec.ts | 相同内容重复导入跳过，提示已存在 | 新增 |
| REQ-ING-004 | AC-2 | E2E-ING-007 | import.spec.ts | 同名不同内容正常入库 | 新增 |
| REQ-ING-005 | AC-1 | E2E-UI-001 | bridge.spec.ts | 删除后 DOM 消失 | 已有 |
| REQ-ING-005 | AC-2 | E2E-ING-008 | import.spec.ts | 删除后配额计数释放 | 新增 |

### 2.3 RAG 域

| REQ | AC | E2E-TC | 文件 | 断言要点 | 状态 |
| --- | --- | --- | --- | --- | --- |
| REQ-RAG-001 | AC-1 | E2E-RAG-001 | chat.spec.ts | 首个 token 到达前端有可见反馈（思考指示器） | 新增 |
| REQ-RAG-001 | AC-2 | E2E-RAG-002 | chat.spec.ts | 流式过程中 UI 不冻结，可滚动 | 新增 |
| REQ-RAG-001 | AC-3 | E2E-RAG-003 | chat.spec.ts | 回答结束后 Block 进入完成态 | 新增 |
| REQ-RAG-002 | AC-1 | E2E-RAG-004 | chat.spec.ts | 引用块含文档名、片段预览、分数 | 新增 |
| REQ-RAG-002 | AC-2 | E2E-RAG-005 | chat.spec.ts | 引用列表与注入上下文的 chunk 一致 | 新增 |
| REQ-RAG-003 | AC-1 | E2E-RAG-006 | chat.spec.ts | 低分结果不出现在引用中 | 新增 |
| REQ-RAG-003 | AC-2 | E2E-RAG-007 | chat.spec.ts | 空上下文返回固定提示，不调用 LLM | 新增 |
| REQ-RAG-004 | AC-1 | E2E-RAG-008 | session.spec.ts | 多轮上下文携带历史 | 新增 |
| REQ-RAG-004 | AC-2 | E2E-RAG-009 | session.spec.ts | 新会话不携带旧历史 | 新增 |
| REQ-RAG-005 | AC-1 | E2E-UI-001 | bridge.spec.ts | 停止后中断标记出现 | 已有 |
| REQ-RAG-005 | AC-2 | E2E-RAG-010 | chat.spec.ts | 停止后输入框恢复空闲态 | 已有 |
| REQ-RAG-006 | AC-1 | E2E-RAG-011 | session.spec.ts | 会话 CRUD 状态正确 | 新增 |
| REQ-RAG-006 | AC-2 | E2E-RAG-012 | session.spec.ts | 首轮问答后标题自动提取 | 新增 |
| REQ-RAG-006 | AC-3 | E2E-RAG-013 | session.spec.ts | 切换会话加载历史消息 | 新增 |

### 2.4 LIC 域

| REQ | AC | E2E-TC | 文件 | 断言要点 | 状态 |
| --- | --- | --- | --- | --- | --- |
| REQ-LIC-001 | AC-1 | TC-LIC-001 | L1 单元 | 合法签名验证通过 | 已有 |
| REQ-LIC-001 | AC-2 | TC-LIC-002 | L1 单元 | 篡改签名拒绝 | 已有 |
| REQ-LIC-002 | AC-1 | TC-LIC-003 | L2 集成 | 50 文件配额拦截 | 已有 |
| REQ-LIC-002 | AC-2 | TC-LIC-004 | L2 集成 | PDF 付费门拦截 | 已有 |
| REQ-LIC-002 | AC-3 | E2E-LIC-002 | import.spec.ts | Pro 版不受 50 限制且可导入 PDF | 新增 |
| REQ-LIC-002 | AC-4 | E2E-LIC-003 | import.spec.ts | get_pro_status 返回当前状态 | 新增 |
| REQ-LIC-003 | AC-1 | E2E-LIC-001 | bridge.spec.ts | 付费墙 Modal 自动弹出 | 已有 |
| REQ-LIC-003 | AC-2 | E2E-LIC-001 | bridge.spec.ts | 激活后 Modal 关闭 | 已有 |
| REQ-LIC-003 | AC-3 | E2E-LIC-001 | bridge.spec.ts | 侧栏状态更新为 Pro | 已有 |
| REQ-LIC-003 | AC-4 | E2E-LIC-004 | edge-cases.spec.ts | 关闭付费墙无副作用 | 新增 |

### 2.5 SEC 域

| REQ | AC | E2E-TC | 文件 | 断言要点 | 状态 |
| --- | --- | --- | --- | --- | --- |
| REQ-SEC-002 | AC-1 | E2E-SEC-001 | visual.spec.ts | OWASP 注入向量集全部失效 | 新增 |
| REQ-SEC-003 | AC-1 | E2E-UI-004 | visual.spec.ts | 默认配置无外网请求 | 新增 |
| REQ-SEC-003 | AC-2 | E2E-SEC-002 | visual.spec.ts | core 无网络代码（依赖树审计） | 新增 |

### 2.6 思考过程（REQ-RAG-001 扩展）

| AC | E2E-TC | 文件 | 断言要点 | 状态 |
| --- | --- | --- | --- | --- |
| AC-1 | E2E-RAG-001 | chat.spec.ts | 发送后立即出现思考指示器（脉冲动画） | 新增 |
| AC-2 | E2E-RAG-014 | chat.spec.ts | chat_phase 事件更新指示器文案 | 新增 |
| AC-3 | E2E-RAG-015 | chat.spec.ts | 首 token 到达后指示器消失 | 新增 |

### 2.7 MM 域（多模态 VLM 前端接线）

| REQ | AC | E2E-TC | 文件 | 断言要点 | 状态 |
| --- | --- | --- | --- | --- | --- |
| REQ-MM-003 | 前端接线 | E2E-MM-001 | settings.spec.ts | 设置面板打开，LLM 配置展示（端点/模型/脱敏 Key） | 新增 |
| REQ-MM-003 | 前端接线 | E2E-MM-002 | settings.spec.ts | 设置面板「完成」按钮关闭 | 新增 |
| REQ-MM-003 | 隐私边界 | E2E-MM-003 | settings.spec.ts | VLM 开关初始关闭，隐私提示隐藏 | 新增 |
| REQ-MM-003 | 隐私边界 | E2E-MM-004 | settings.spec.ts | 开启 VLM 弹出隐私确认弹窗（含 BYOK 文案） | 新增 |
| REQ-MM-003 | 隐私边界 | E2E-MM-005 | settings.spec.ts | 取消确认 → 开关保持关闭，set_vlm_enabled 未调用 | 新增 |
| REQ-MM-003 | AC-1/AC-4 | E2E-MM-006 | settings.spec.ts | 确认开启 → 开关 on + 隐私提示可见 + set_vlm_enabled(true) + toast | 新增 |
| REQ-MM-003 | 前端接线 | E2E-MM-007 | settings.spec.ts | 关闭 VLM 直接生效（无确认弹窗）+ set_vlm_enabled(false) + toast | 新增 |
| REQ-MM-003 | 前端接线 | E2E-MM-008 | settings.spec.ts | VLM 状态跨设置面板开关持久化恢复（E2E 版 TC-MM-005） | 新增 |
| REQ-MM-003 | 前端接线 | E2E-MM-009 | settings.spec.ts | 「修改 LLM 配置」入口关闭设置面板并打开向导 | 新增 |

### 2.8 DS 域（设计系统规范，SRS §4.14）

> **状态**：规划中。SRS v2.1.0 已将 DS 域原子化为色板 token + 字体层级 + 间距网格 + 图标系统 + 组件规范 + 动效规范 + 圆角阴影体系。

| REQ | AC | E2E-TC | 文件 | 断言要点 | 状态 |
| --- | --- | --- | --- | --- | --- |
| REQ-DS-001 | AC-1 | E2E-DS-001 | visual.spec.ts | `tailwind.config` 定义 6 个自定义色 token | 规划 |
| REQ-DS-001 | AC-3 | E2E-DS-002 | visual.spec.ts | `.md` 排版硬编码色值与规格表一致 | 规划 |
| REQ-DS-002 | AC-2 | E2E-DS-003 | visual.spec.ts | `.md` font-size=14px，line-height=1.8 | 规划 |
| REQ-DS-002 | AC-5 | E2E-DS-004 | visual.spec.ts | `.md h1` font-size≈20.3px，font-weight=700 | 规划 |
| REQ-DS-003 | AC-1 | E2E-DS-005 | visual.spec.ts | `#chatArea` padding=24px | 规划 |
| REQ-DS-003 | AC-2 | E2E-DS-006 | visual.spec.ts | `#inputBar` padding 16px/12px | 规划 |
| REQ-DS-004 | AC-1 | E2E-DS-007 | visual.spec.ts | 全部图标为 Unicode 字符 | 规划 |
| REQ-DS-005 | AC-1 | E2E-DS-008 | visual.spec.ts | `#sendBtn` primary 变体色值 | 规划 |
| REQ-DS-005 | AC-3 | E2E-DS-009 | visual.spec.ts | `#stopBtn` danger 变体色值 | 规划 |
| REQ-DS-006 | AC-2 | E2E-DS-010 | visual.spec.ts | `.fade-in` 250ms ease-out | 规划 |
| REQ-DS-006 | AC-5 | E2E-DS-011 | chat.spec.ts | rAF 节流防抖生效 | 规划 |
| REQ-DS-007 | AC-1 | E2E-DS-012 | visual.spec.ts | `#sendBtn` border-radius=12px | 规划 |
| REQ-DS-007 | AC-2 | E2E-DS-013 | visual.spec.ts | assistant Block border-radius=16px | 规划 |
| REQ-DS-007 | AC-6 | E2E-DS-014 | visual.spec.ts | user Block 不对称圆角 | 规划 |

### 2.9 IX 域（交互设计规范，SRS §4.15）

> **状态**：规划中。REQ-IX-004 和 REQ-IX-006 已实现，其余 Draft。

| REQ | AC | E2E-TC | 文件 | 断言要点 | 状态 |
| --- | --- | --- | --- | --- | --- |
| REQ-IX-001 | AC-1 | E2E-IX-001 | ix-context-menu.spec.ts | 右键文档列表项弹出菜单 | 规划 |
| REQ-IX-001 | AC-4 | E2E-IX-002 | ix-context-menu.spec.ts | 菜单边界自动偏移 | 规划 |
| REQ-IX-001 | AC-5 | E2E-IX-003 | ix-context-menu.spec.ts | 点击外部/Esc 关闭菜单 | 规划 |
| REQ-IX-001 | AC-6 | E2E-IX-004 | ix-context-menu.spec.ts | 非 Pro 审计项灰显 + Pro 标签 | 规划 |
| REQ-IX-002 | AC-1 | E2E-IX-005 | ix-drag-sort.spec.ts | 会话项拖拽 opacity-50 | 规划 |
| REQ-IX-002 | AC-2 | E2E-IX-006 | ix-drag-sort.spec.ts | 插入线显示 | 规划 |
| REQ-IX-002 | AC-4 | E2E-IX-007 | ix-drag-sort.spec.ts | sort_order 持久化 | 规划 |
| REQ-IX-003 | AC-1 | E2E-IX-008 | ix-copy.spec.ts | 代码块复制 + 按钮文案变化 | 规划 |
| REQ-IX-003 | AC-5 | E2E-IX-009 | ix-copy.spec.ts | 非安全上下文复制失败 toast | 规划 |
| REQ-IX-004 | AC-1 | E2E-IX-010 | visual.spec.ts | 文档列表项 hover 背景色 | 规划 |
| REQ-IX-004 | AC-2 | E2E-IX-011 | visual.spec.ts | `#sendBtn` hover opacity 过渡 | 规划 |
| REQ-IX-004 | AC-5 | E2E-IX-012 | chat.spec.ts | `.msg-actions` hover opacity 过渡 | 规划 |
| REQ-IX-005 | AC-1 | E2E-IX-013 | ix-confirm.spec.ts | 确认对话框标题 + 不可撤销提示 | 规划 |
| REQ-IX-005 | AC-4 | E2E-IX-014 | ix-confirm.spec.ts | 500ms 防误触 disabled | 规划 |
| REQ-IX-006 | AC-1 | E2E-IX-015 | keyboard-a11y.spec.ts | Shift+Enter 换行不发送 | 规划 |
| REQ-IX-006 | AC-4 | E2E-IX-016 | keyboard-a11y.spec.ts | 超 6 行高度固定 + 滚动条 | 规划 |
| REQ-IX-007 | AC-1 | E2E-IX-017 | ix-skeleton.spec.ts | 骨架项 + animate-pulse | 规划 |
| REQ-IX-007 | AC-2 | E2E-IX-018 | ix-skeleton.spec.ts | <200ms 不显示骨架 | 规划 |

### 2.10 ERR 域（错误处理与降级，SRS §4.21）

> **状态**：规划中。REQ-ERR-001 部分实现，REQ-ERR-004 部分实现，其余 Draft。

| REQ | AC | E2E-TC | 文件 | 断言要点 | 状态 |
| --- | --- | --- | --- | --- | --- |
| REQ-ERR-001 | AC-1 | E2E-ERR-001 | error-handling.spec.ts | 配额超限返回 `LIMIT_REACHED:` 前缀 | 规划 |
| REQ-ERR-001 | AC-3 | E2E-ERR-002 | error-handling.spec.ts | 前端识别前缀弹出 showPaywall | 规划 |
| REQ-ERR-001 | AC-5 | E2E-ERR-003 | toast.spec.ts | sanitizeError API Key 脱敏 | 规划 |
| REQ-ERR-001 | AC-5 | E2E-ERR-004 | toast.spec.ts | sanitizeError 路径脱敏 | 规划 |
| REQ-ERR-002 | AC-1 | E2E-ERR-005 | error-handling.spec.ts | LLM 超时自动重试 1s→2s→4s | 规划 |
| REQ-ERR-002 | AC-2 | E2E-ERR-006 | error-handling.spec.ts | 重试期间 #inputHint 文案 | 规划 |
| REQ-ERR-002 | AC-5 | E2E-ERR-007 | error-handling.spec.ts | 4xx 不重试直接返回 AUTH | 规划 |
| REQ-ERR-003 | AC-1 | E2E-ERR-008 | error-handling.spec.ts | NETWORK 错误触发离线提示 | 规划 |
| REQ-ERR-003 | AC-2 | E2E-ERR-009 | error-handling.spec.ts | 离线 #sendBtn disabled | 规划 |
| REQ-ERR-003 | AC-3 | E2E-ERR-010 | error-handling.spec.ts | 离线本地功能可用 | 规划 |
| REQ-ERR-004 | AC-1 | E2E-ERR-011 | error-handling.spec.ts | 崩溃恢复 Processing→Failed | 规划 |
| REQ-ERR-004 | AC-4 | E2E-ERR-012 | chat.spec.ts | 中断保留已生成内容 | 规划 |
| REQ-ERR-005 | AC-1 | E2E-ERR-013 | edge-cases.spec.ts | 路径遍历 `..` 拒绝 | 规划 |
| REQ-ERR-005 | AC-5 | E2E-ERR-014 | session.spec.ts | 标题截断 + … | 规划 |
| REQ-ERR-005 | AC-6 | E2E-ERR-015 | edge-cases.spec.ts | License Key 无效返回错误 | 规划 |

---

## 3. Mock 能力清单（24 命令 + 10 事件全覆盖）

tauri-stub.js 覆盖全部 24 个 IPC 命令和 10 个事件，支撑全量测试：

| 能力 | 用途 | 影响测试 |
| --- | --- | --- |
| 文档内容哈希去重 | REQ-ING-004 去重测试 | E2E-ING-006/007 |
| 配额计数器 | REQ-LIC-002 配额测试 | E2E-LIC-002/003 |
| chat_phase 事件 | 思考指示器测试 | E2E-RAG-001/014/015 |
| 空上下文模式 | REQ-RAG-003 空上下文 | E2E-RAG-007 |
| 消息持久化 | REQ-RAG-006 会话测试 | E2E-RAG-008~013 |
| XSS token 注入 | REQ-SEC-002 安全测试 | E2E-SEC-001 |
| 连接测试失败模拟 | REQ-UI-007 验证失败 | E2E-UI-016 |
| 拖拽事件模拟 | REQ-UI-004 遮罩测试 | E2E-UI-009/010 |
| retry_index 状态流转 | REQ-VEC-005 重试测试 | E2E-DOC-001~003 |
| deactivate_pro | REQ-LIC-004 停用测试 | E2E-LIC-005~011 |
| audit_document 三阶段事件 | REQ-AUDIT-005 审计测试 | E2E-AUDIT-001~006 |
| abort_audit 取消 | REQ-AUDIT-005 取消测试 | E2E-AUDIT-005 |
| init_embedder 下载进度 | REQ-VEC-008 模型缓存 | E2E-VEC-001~007 |
| get_model_cache_info | REQ-VEC-008 缓存信息 | E2E-VEC-001/006/007 |
| clear_model_cache | REQ-VEC-008 清理缓存 | E2E-VEC-005/006 |
| abort_import 取消 | REQ-ING-006 导入取消 | E2E-IMP-004/005 |
| import-progress 事件 | REQ-ING-006 进度条 | E2E-IMP-001~006 |
| set_hybrid_search | REQ-RAG-010 混合检索 | 预留 |

---

## 4. 执行计划

| 阶段 | 内容 | 预计用例数 |
| --- | --- | --- |
| 1 | 增强 tauri-stub.js mock（24 命令 + 10 事件） | — |
| 2 | visual.spec.ts（视觉 + CDN + XSS） | ~7 |
| 3 | wizard.spec.ts（配置向导） | ~6 |
| 4 | import.spec.ts（导入管线） | ~10 |
| 5 | chat.spec.ts（对话链路） | ~12 |
| 6 | session.spec.ts（会话持久化） | ~6 |
| 7 | edge-cases.spec.ts（边界防御） | ~6 |
| 8 | viz.spec.ts（富内容可视化） | ~12 |
| 9 | nfr.spec.ts（冷启动性能） | ~1 |
| 10 | settings.spec.ts（设置面板 + VLM） | ~10 |
| 11 | audit.spec.ts（审计功能 UI）★ | ~7 |
| 12 | document-actions.spec.ts（文档操作按钮）★ | ~8 |
| 13 | import-progress.spec.ts（导入进度与取消）★ | ~7 |
| 14 | model-cache.spec.ts（模型缓存管理）★ | ~8 |
| 15 | toast.spec.ts（Toast 系统行为）★ | ~8 |
| 16 | markdown-render.spec.ts（Markdown 渲染完整性）★ | ~10 |
| 17 | keyboard-a11y.spec.ts（键盘导航与无障碍）★ | ~10 |
| 18 | sidebar-behavior.spec.ts（侧栏折叠与交互）★ | ~9 |
| 19 | license-deactivate.spec.ts（License 停用 UI）★ | ~8 |
| **合计** | | **~150** |

---

## 5. 修订记录

| 版本 | 日期 | 修订摘要 | 批准 |
| --- | --- | --- | --- |
| v1.0.0 | 2026-07-25 | 创世版：全量 REQ→E2E 追溯矩阵、Mock 增强清单、执行计划 | QA Lead / TPM |
| v1.1.0 | 2026-07-25 | MM Phase 2 前端接线 E2E：新增 §2.7 MM 域 E2E-MM-001~009（设置面板 + VLM 开关 + 隐私弹窗，9/9 绿灯） | QA Lead / TPM |
| v2.0.0 | 2026-07-25 | UI 验收体系全面升级：Mock 覆盖 24/24 命令 + 10 事件；新增 9 个 spec 文件（审计/文档操作/导入进度/模型缓存/Toast/Markdown渲染/键盘无障碍/侧栏行为/License停用），~80 新增用例；修复 toastSuccess 未定义 P0 缺陷 | QA Lead / TPM |
| v2.1.0 | 2026-07-25 | 全量 133/133 绿灯：修复 10 个测试缺陷（toContainText 正则误用、strict mode 多按钮匹配、toast 遮挡 click 拦截、#stopBtn 可见性断言、import-progress 时序竞争、mock 延迟不足）；Mock 增强 import_files 文件间延迟 + audit token 间隔加大 | QA Lead |
| v2.2.0 | 2026-07-25 | 新增输入框视觉对齐维度（input-bar-align.spec.ts 8 用例）；修复 BUG-UI-009 textarea py-1.5 垂直内边距；全量 141/141 绿灯 | QA Lead |
| v2.3.0 | 2026-07-25 | SRS v2.1.0 原子化重构同步：新增 §2.8 DS 域（14 用例 E2E-DS-001~014）、§2.9 IX 域（18 用例 E2E-IX-001~018）、§2.10 ERR 域（15 用例 E2E-ERR-001~015）REQ→E2E 追溯矩阵；全部状态标注为「规划」（Draft 需求待实现后编写测试） | QA Lead / TPM |
