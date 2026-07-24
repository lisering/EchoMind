# EchoMind（灵犀）软件需求规格说明书 — SRS v1.0

> 本文档是产品需求的唯一真相源。所有需求原子化、可测试、可追溯。
> 规则约束见 `PROJECT_RULES.md`；测试映射与执行流程见 `docs/TEST_STRATEGY.md`。

| 项 | 内容 |
| --- | --- |
| 文档版本 | v1.0.7（含知识库面板、文档删除与 L3 E2E 基建） |
| 生效日期 | 2026-07-24 |
| 维护者 | PM（状态流转由 TPM 统一登记） |

---

## 1. 引言

### 1.1 目的
定义 EchoMind v1.0 的功能与非功能需求，作为 QA 编写红灯测试、TPM 冻结契约、开发绿灯冲刺的唯一依据。

### 1.2 产品概述
EchoMind 是一款本地优先的 AI 知识库桌面应用。核心链路：用户导入本地文档 → 本地解析、分块与向量化 → 基于用户自带 API Key（BYOK）的大模型进行检索增强对话（RAG）。核心承诺：**Rust 极速、隐私不出域、BYOK 买断制**。

### 1.3 术语

| 术语 | 含义 |
| --- | --- |
| BYOK | Bring Your Own Key，用户自带 LLM / Embedding API Key |
| 知识库 | 已导入文档及其索引数据的集合 |
| Block | 对话界面中的单条消息渲染单元 |
| 端口（Port） | `crates/core` 中定义的 Trait 接口（六边形架构） |
| 触顶 | 达到免费版配额上限，触发付费墙 |

---

## 2. 编号与状态规则

### 2.1 需求编号
格式：`REQ-<域>-<三位序号>`。序号域内连续分配，废弃需求不复用序号。

| 域代码 | 领域 |
| --- | --- |
| ING | 文档导入与管理 |
| VEC | 解析、分块与向量化索引 |
| RAG | 检索增强对话 |
| UI | 界面与交互状态 |
| LIC | 授权与 Freemium 付费墙 |
| SEC | 安全与隐私 |
| NFR | 非功能需求 |
| ARCH | 架构与工程基建 |
| DB | 数据持久化 |
| PDF | PDF 文档解析 |
| LLM | 大模型接入 |

### 2.2 验收标准编号
`REQ-<域>-<序号>-AC-<n>`。每条 AC 必须能被至少一个测试用例（`TC-*` / `E2E-*`）自动验证。

### 2.3 需求状态机
`Draft → Approved → Test-Red → Contract-Frozen → In-Dev → Done → Verified`，另设 `Deprecated`（须注明替代条目）。
状态流转与 `PROJECT_RULES.md` 体系二流水线一一对应，由 TPM 维护。

### 2.4 优先级
P0 = v1.0 必须交付；P1 = v1.0 力争交付；P2 = 后续版本候选。

### 2.5 原子需求模板

```
### REQ-<域>-<序号> <标题>
- 优先级：P0/P1/P2 ｜ 状态：Draft ｜ 依赖：REQ-xxx-xxx 或 无
- 描述：<用户视角的一句话需求 + 必要细节>
- 验收标准：
  - AC-1：<可测试的判定语句>
- 契约（TPM 冻结）：TBD
- 关联测试（QA 登记）：TBD
```

---

## 3. 用户旅程总览（v1.0）

1. **首次启动**：空状态引导 → 进入设置配置 BYOK（Provider / base_url / model / API Key，Key 本地加密保存）→ 连接测试通过。
2. **构建知识库**：拖拽文件入窗 → 全屏遮罩「数据仅本地处理」→ 解析 / 分块 / 向量化 → 列表徽标实时显示索引状态（待索引 / 索引中 / 已索引 / 失败）。
3. **知识问答**：底部控制台输入问题 → 检索命中片段注入上下文 → 流式 Block 渲染（Markdown + 代码高亮）→ 引用块可回溯来源文档与片段。
4. **触顶转化**：免费版导入第 51 个文件或导入 PDF 时 → 付费墙拦截并给出升级引导 → 输入 License 激活 Pro → 即刻解锁并继续被中断的操作。

---

## 4. 功能需求

### 4.1 ING — 文档导入与管理

#### REQ-ING-001 拖拽导入本地文件
- 优先级：P0 ｜ 状态：Contract-Frozen ｜ 依赖：无
- 描述：用户可将受支持格式的文件拖拽至主窗口任意位置完成导入；拖拽进入窗口时全屏遮罩高亮，固定文案「数据仅本地处理」。
- 导入管线（PM 定义，Phase 2 冻结）：
  - 输入：文件路径数组。
  - 逻辑：格式白名单校验（.md / .txt / .pdf）→ 路径安全校验（拒绝 `..` 路径遍历，canonicalize 后必须存在且为普通文件）→ 内容指纹去重（MD5）→ 免费版限制（is_pro = false 时：导入后总数 > 50 或导入 PDF 均拦截并返回错误）→ 复制入本地数据目录。
  - 输出：成功入库并触发状态更新事件；返回成功入库的原始文件名列表。
- 验收标准：
  - AC-1：拖入受支持文件并释放后，文件被复制入本地数据目录，出现在知识库列表中，初始索引状态为「待索引」。
  - AC-2：拖拽进入窗口即显示遮罩；拖出未释放或按 Esc 取消时遮罩消失，且不产生任何导入动作。
  - AC-3：不支持的格式被拦截，以 toast 说明原因，文件不入库、不创建索引任务。
- 契约（TPM 冻结，2026-07-24）：
  - Command：`import_files(app: AppHandle, paths: Vec<String>, is_pro: bool, state: State<'_, AppState>) -> Result<Vec<String>, String>`。返回成功入库的原始文件名列表；格式非法 / 配额触顶 / 未授权 PDF / 路径遍历等错误经 `Err(可读消息)` 返回。（`app: AppHandle` 为事件发射所需，属冻结契约组成部分。）
  - Event：`doc-status-changed`，Payload `{ status: "indexing" | "done" | "error", message: String }`（对应 `models::DocStatusPayload`）。
- 关联测试：TC-ING-001 ~ TC-ING-007

#### REQ-ING-002 导入格式白名单
- 优先级：P0 ｜ 状态：Approved ｜ 依赖：REQ-ING-001
- 描述：v1.0 支持 `.md`、`.txt`（免费版）；`.pdf` 为 Pro 专属格式；其余格式一律拒绝。
- 验收标准：
  - AC-1：免费版导入 `.pdf` 被拦截，弹出付费墙升级引导（关联 REQ-LIC-003）。
  - AC-2：Pro 版导入 `.pdf` 可正常进入解析与索引流程。
  - AC-3：白名单外格式（如 `.exe`、`.docx`）被明确拒绝并提示当前支持格式列表。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-ING-003 文件选择器导入
- 优先级：P1 ｜ 状态：Approved ｜ 依赖：REQ-ING-001
- 描述：除拖拽外，用户可通过「导入文件」按钮打开系统文件选择器导入，支持多选。
- 验收标准：
  - AC-1：选择器仅列出当前许可证允许的格式；多选确认后批量入库。
  - AC-2：导入结果与拖拽路径一致（同一入库与索引管线）。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-ING-004 文件指纹去重
- 优先级：P0 ｜ 状态：Approved ｜ 依赖：REQ-ING-001
- 描述：以文件内容 MD5 作为指纹，防止同一内容重复入库（v1.0.2 修订：SHA-256 → MD5；指纹仅用于内容去重，非安全校验场景）。
- 验收标准：
  - AC-1：导入与库内已有文件内容完全相同的文件时，跳过导入并提示「该文件已存在」，不产生重复条目与重复向量。
  - AC-2：同名但内容不同的文件正常入库，互不影响。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-ING-005 删除文档与级联清理
- 优先级：P0 ｜ 状态：Contract-Frozen ｜ 依赖：REQ-ING-001、REQ-VEC-003
- 描述：用户可从知识库删除文档，其分块与向量数据经外键级联同步清理，数据目录中的文档副本一并删除。
- 验收标准：
  - AC-1：删除后文档条目、全部 chunk 与向量从本地存储移除（前端经列表 × 按钮触发，E2E 断言 DOM 消失）。
  - AC-2：删除后对该文档的检索不再返回任何结果；免费版文件计数同步释放。
- 契约（TPM 冻结，2026-07-24）：Command `delete_document(id: String, state) -> Result<(), String>`；Command `get_documents(state) -> Result<Vec<Document>, String>`（列表读取，REQ-UI-006 共用）。
- 关联测试：E2E-UI-001（L3）

#### REQ-ING-006 导入进度与取消
- 优先级：P1 ｜ 状态：Draft ｜ 依赖：REQ-ING-001
- 描述：批量导入时展示整体进度，并允许中途取消。
- 验收标准：
  - AC-1：进度以事件推送至前端（已完成数 / 总数 / 当前文件名）。
  - AC-2：取消后已完成部分保留，未开始部分不再执行，不产生半成品索引。
- 契约：TBD ｜ 关联测试：TBD

### 4.2 VEC — 解析、分块与向量化索引

#### REQ-VEC-001 文档解析与分块
- 优先级：P0 ｜ 状态：Approved ｜ 依赖：REQ-ING-001
- 描述：对入库文档进行文本提取与分块；v1.0 默认策略为固定窗口 512 tokens、重叠 64 tokens（参数集中于配置，可评审调整）。
- 验收标准：
  - AC-1：Markdown 按结构边界优先切分，代码块不被从中间截断。
  - AC-2：中英文混排文本分块后无乱码、无半个 UTF-8 字符。
  - AC-3：每个 chunk 携带元数据（文档 ID、序号、字符区间）。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-VEC-002 Embedding 执行通道（可插拔）
- 优先级：P0 ｜ 状态：Contract-Frozen ｜ 依赖：REQ-VEC-001
- 描述：Embedding 能力以端口 Trait 抽象，v1.0 提供本地内置模型（默认，数据不出机）与 BYOK 远程端点（可选，用户显式配置）两种适配器。本地引擎（Phase 3 冻结）：fastembed 加载 all-MiniLM-L6-v2 量化模型（384 维）；批量推理经 `spawn_blocking` 执行、推理线程数设为系统核心数；模型下载多源容错（先 `https://huggingface.co`，失败回退 `https://hf-mirror.com`），强制全量下载、禁用断点续传（避免镜像源 Content-Range 报错）。
- 验收标准：
  - AC-1：`crates/core` 中仅存在 Embedding 端口 Trait，无任何具体实现依赖（适配器位于 crates/infra）。
  - AC-2：批量推理返回向量数量与输入匹配、维度 384、非全零（TC-VEC-001）。
  - AC-3：官方源不可达时自动回退镜像源；双侧均失败时返回携带双侧原因的可读 Err。
- 契约：Embedder 端口契约不变；适配器 `echomind_infra::local_embedder::LocalEmbedder`。
- 关联测试：TC-VEC-001

> TPM 注（v1.0.3）：上游指令曾以「REQ-VEC-001」指代本地向量化引擎；因该编号已被「文档解析与分块」占用，本地引擎需求归口本条（REQ-VEC-002），避免编号冲突。

#### REQ-VEC-003 向量持久化与索引
- 优先级：P0 ｜ 状态：Contract-Frozen ｜ 依赖：REQ-VEC-002
- 描述：向量与 chunk 元数据持久化于本地 SQLite（embeddings 表，BLOB 存储）；导入管线已接通嵌入写入链路（分块入库 → embed_batch → add_embedding，失败置 Failed 并推送事件），随应用重启可复用。
- 验收标准：
  - AC-1：索引完成后重启应用，无需重建即可检索。
  - AC-2：向量库文件位于应用数据目录内，不污染用户文档原始位置。
- 契约：Storage 端口新增 `list_documents / delete_document / list_chunks`（REQ-ING-005 与嵌入接线共用）。
- 关联测试：TC-DB-001（旁证）、E2E-UI-001

#### REQ-VEC-004 索引状态机与事件推送
- 优先级：P0 ｜ 状态：Approved ｜ 依赖：REQ-VEC-002、REQ-UI-006
- 描述：每个文档的索引生命周期为 `pending → processing → done / failed`，状态变更通过 Tauri Event 实时推送前端。
- 验收标准：
  - AC-1：每次状态迁移都发射事件，负载含文档 ID、新状态、失败原因（如有）。
  - AC-2：前端列表徽标与状态机严格一致，无「已完成但未刷新」情况。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-VEC-005 索引失败重试
- 优先级：P1 ｜ 状态：Draft ｜ 依赖：REQ-VEC-004
- 描述：失败文档支持手动重试；瞬时网络错误自动退避重试（上限 3 次）。
- 验收标准：
  - AC-1：手动点击「重试」后文档重新进入 `pending`。
  - AC-2：自动重试达到上限后状态置为 `failed` 并给出原因。
- 契约：TBD ｜ 关联测试：TBD

### 4.3 RAG — 检索增强对话

#### REQ-RAG-001 提问与流式回答
- 优先级：P0 ｜ 状态：Contract-Frozen ｜ 依赖：REQ-RAG-007、REQ-UI-002
- 描述：用户在底部控制台输入问题，系统检索知识库并调用 BYOK 大模型，回答以 token 级流式渲染。
- 验收标准：
  - AC-1：网络正常时首 token 到达前端 ≤ 3s（以事件时间戳断言）。
  - AC-2：流式过程中 UI 不冻结，可随时滚动历史。
  - AC-3：回答完整结束后 Block 进入「完成」态，显示耗时与引用数。
- 契约（TPM 冻结，2026-07-24）：
  - Command：`chat(app: AppHandle, query: String, history: Vec<ChatMessage>, conversation_id: String, state: State<'_, AppState>) -> Result<(), String>`（`app: AppHandle` 为事件发射所需；conversation_id 为会话持久化预留，Phase 5 启用）。
  - Events：`chat_token(String)` 逐 token 推送；`chat_sources(Vec<RetrievalResult>)` 推送引用来源；`chat_error(String)` 推送中断或错误信息；`chat_done(())` 流结束信号。
- 关联测试：TC-RAG-002、TC-LLM-001（L2 契约测试随 CI 阶段补齐）

#### REQ-RAG-002 引用来源回溯
- 优先级：P0 ｜ 状态：Approved ｜ 依赖：REQ-RAG-001
- 描述：回答末尾附引用块，列出命中的来源文档与片段，可点击定位。
- 验收标准：
  - AC-1：每个引用含文档名、片段预览、chunk 序号；点击引用打开来源预览面板并高亮对应片段。
  - AC-2：引用列表与实际注入上下文的 chunk 完全一致，不多不少。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-RAG-003 检索策略与拒答
- 优先级：P0 ｜ 状态：Contract-Frozen ｜ 依赖：REQ-VEC-003
- 描述：VectorRetriever 接入 LocalEmbedder 与 SqliteStorage，默认 top-k（k=5）向量相似度检索；引入相关度分数阈值过滤（score < 0.5 直接丢弃），防止低质量上下文污染；过滤后上下文为空时，直接返回固定提示，**不调用 LLM**。
- 验收标准：
  - AC-1：低于阈值的检索结果全部被丢弃（TC-RAG-001）；相似度不足时回答明确告知「知识库中未找到相关内容」，不编造来源。
  - AC-2：空上下文时不发起任何 LLM 网络请求，直接返回固定警告流（TC-RAG-002）。
- 契约：Retriever 端口契约不变；编排器 `core::chat::ChatEngine`、检索适配器 `core::retriever::VectorRetriever`。
- 关联测试：TC-RAG-001、TC-RAG-002

> TPM 注（v1.0.4）：上游指令曾以「REQ-RAG-001」指代检索引擎；检索策略归口本条（REQ-RAG-003），REQ-RAG-001（流式回答）承载对话命令契约，避免编号冲突。

#### REQ-RAG-004 多轮上下文管理
- 优先级：P1 ｜ 状态：Approved ｜ 依赖：REQ-RAG-001
- 描述：同一会话内携带对话历史；提供「清空上下文」操作。
- 验收标准：
  - AC-1：后续提问能引用前文信息（如代词指代）。
  - AC-2：清空后新提问不再携带任何历史消息。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-RAG-005 停止生成
- 优先级：P0 ｜ 状态：Contract-Frozen ｜ 依赖：REQ-RAG-001
- 描述：流式生成过程中用户可一键停止；后端为每个 conversation_id 维护一个 CancellationToken，abort_chat 触发后流循环立即 break，丢弃流即中止 HTTP 连接与事件推送；已生成部分内容保留并标记「已中断」，且照常落库（REQ-RAG-006）。
- 验收标准：
  - AC-1：abort 后事件流立即终止（TC-RAG-005），无 Panic，部分内容保留。
  - AC-2：停止后输入框恢复「空闲」态，可立即再次提问。
- 契约（TPM 冻结，2026-07-24）：Command `abort_chat(conversation_id: String, state) -> Result<(), String>`；中断事件语义复用已冻结契约：`chat_error("⏹ 生成已中断")` + `chat_done`（不新增事件类型）。
- 关联测试：TC-RAG-005

#### REQ-RAG-006 会话持久化与多轮记忆
- 优先级：P0 ｜ 状态：Contract-Frozen ｜ 依赖：REQ-RAG-001、REQ-DB-001
- 描述：新增 `conversations` 表（id, workspace_id, title, created_at）与 `messages` 表（id, conversation_id, role, content, sources，外键级联）；新建对话生成唯一 ID（占位标题「新会话」）；AI 回答完成（含被中断）后将用户问题与 AI 回复存入 messages；首轮问答后自动提取用户问题前 24 字符作为会话标题；前端左侧栏展示历史会话，点击加载历史消息并恢复多轮上下文；重启后完整可读可续聊。
- 验收标准：
  - AC-1：会话 CRUD 状态正确（TC-RAG-003：新建 → 列表 → 删除级联清理消息）。
  - AC-2：完成一次对话后 messages 表新增 user 与 assistant 两条记录，标题自动提取（TC-RAG-004）。
  - AC-3：重启应用后历史会话完整可读（含引用块 sources），续聊上下文包含该会话历史。
- 契约（TPM 冻结，2026-07-24）：Command `create_conversation(workspace_id: String) -> Result<String, String>`；`get_conversations(workspace_id: String) -> Result<Vec<Conversation>, String>`；`get_messages(conversation_id: String) -> Result<Vec<ChatMessage>, String>`；`delete_conversation(id: String) -> Result<(), String>`。chat 命令复用既有 `conversation_id` 参数（Phase 4 契约已含）。
- 关联测试：TC-RAG-003、TC-RAG-004

#### REQ-RAG-007 BYOK Provider 管理
- 优先级：P0 ｜ 状态：Approved ｜ 依赖：REQ-SEC-001
- 描述：支持配置 OpenAI 兼容端点（含 DeepSeek 等）与 Anthropic 两类 Provider；每类可配置 base_url、model、API Key，并提供「连接测试」。
- 验收标准：
  - AC-1：保存配置前可执行连接测试，成功 / 失败均有明确反馈（失败不泄露 Key）。
  - AC-2：切换 Provider 后新会话立即生效，无需重启。
  - AC-3：未配置任何 Provider 时，提问入口给出引导而非报错。
- 契约：TBD ｜ 关联测试：TBD

### 4.4 UI — 界面与交互状态

#### REQ-UI-001 全局视觉规范与主界面布局
- 优先级：P0 ｜ 状态：Contract-Frozen ｜ 依赖：无
- 描述：v1.0 仅暗黑主题；背景 `#161616`，强调色 `#38BDF8`；色值统一由 Tailwind 配置输出。布局：左侧极简折叠栏（会话历史），右侧主对话区，底部宽大圆角控制台式输入框（左下角 + 号导入文件）。前端第三方库（marked / DOMPurify / highlight.js / Tailwind）一律本地化 vendored 于 `ui/vendor/`，**禁止运行时 CDN**（体系三与 REQ-SEC-003 红线）。
- 验收标准：
  - AC-1：全部页面背景 / 强调色与规范值一致（E2E 计算样式断言）。
  - AC-2：不存在规范外的硬编码色值（样式静态检查）。
  - AC-3：界面无任何外联 CDN 请求（静态扫描 `ui/` 无 http(s):// script/link 引用）。
- 契约：前端仅经已冻结命令与事件通信（REQ-ING-001 / REQ-RAG-001 / REQ-LLM-002 / REQ-UI-008）。
- 关联测试：E2E 待 CI 阶段

#### REQ-UI-002 Block 式对话与安全流式渲染
- 优先级：P0 ｜ 状态：Contract-Frozen ｜ 依赖：REQ-RAG-001
- 描述：消息以 Block 呈现；监听 `chat_token` 事件维护 `currentRawMarkdown`，流式 Markdown 必须经 marked.js → DOMPurify → highlight.js 链路渲染；代码块带复制按钮；流式输出时平滑滚动到底部（rAF 节流，零阻塞）。
- 验收标准：
  - AC-1：模型输出包含代码块时语法高亮正确，且可复制。
  - AC-2：模型输出含 `<script>`、事件属性等注入载荷时被完全消毒，不执行、不弹窗（安全用例）。
  - AC-3：流式期间 Markdown 半截语法（如未闭合代码围栏）不产生布局错乱。
- 契约：消费 REQ-RAG-001 冻结的 `chat_token` / `chat_sources` / `chat_error` / `chat_done` 事件。
- 关联测试：E2E 待 CI 阶段

#### REQ-UI-003 控制台式输入框状态
- 优先级：P0 ｜ 状态：Approved ｜ 依赖：REQ-RAG-001
- 描述：底部输入框具备「空闲 / 生成中 / 错误」三态；生成中禁用发送并显示停止按钮。
- 验收标准：
  - AC-1：生成中发送行为被阻止（Enter 与按钮均无效），停止按钮可见可用。
  - AC-2：后端报错时输入框进入错误态并展示可读错误摘要，不暴露 API Key 等敏感信息。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-UI-004 拖拽全屏遮罩
- 优先级：P0 ｜ 状态：Contract-Frozen ｜ 依赖：REQ-ING-001
- 描述：文件拖拽进入窗口时，全屏遮罩高亮显示「数据仅本地处理」。
- 验收标准：
  - AC-1：遮罩文案与强调色（`#38BDF8`）符合规范；拖拽离开即消失，无误判残留。
  - AC-2：遮罩出现时页面其余交互被遮罩层接管，避免误触。
- 契约：监听 Tauri 原生拖拽事件 `tauri://drag-enter` / `tauri://drag-leave` / `tauri://drag-drop`；drop 载荷 `paths` 传入 `import_files`。
- 关联测试：E2E 待 CI 阶段

#### REQ-UI-005 空状态与全局错误反馈
- 优先级：P1 ｜ 状态：Draft ｜ 依赖：无
- 描述：首次启动空知识库、空对话均有引导性空状态；全局错误经统一 toast 通道呈现。
- 验收标准：
  - AC-1：空知识库时主界面展示导入引导（含格式与配额说明）。
  - AC-2：任意后端错误经统一格式 toast 展示，含错误码，不泄露敏感信息。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-UI-006 知识库列表面板
- 优先级：P0 ｜ 状态：Contract-Frozen ｜ 依赖：REQ-ING-001、REQ-VEC-004
- 描述：侧栏「知识库」区块列出全部已导入文档：文件名、索引状态徽标（Pending/Processing/Indexed/Failed）、删除 × 按钮；免费版显示配额用量（n/50），导入与删除后实时刷新。
- 验收标准：
  - AC-1：索引状态徽标与 REQ-VEC-004 状态机实时一致。
  - AC-2：免费版配额用量实时刷新；删除后 DOM 元素即时消失（E2E 断言）。
- 契约：`get_documents` / `delete_document` 命令；`doc-status-changed` 事件驱动刷新。
- 关联测试：E2E-UI-001（L3）

#### REQ-UI-007 首次启动配置向导
- 优先级：P0 ｜ 状态：Contract-Frozen ｜ 依赖：REQ-UI-008、REQ-LLM-002
- 描述：未配置 LLM 时全屏遮罩向导；提供 DeepSeek / OpenAI / Ollama 预设卡片（自动填充 base_url 与 model，均可编辑以兼容自建端点）；「去获取 API Key」按钮经 opener 插件跳转系统浏览器；「验证并开始」调用 test_llm_connection，成功后 update_llm_config 并进入主界面。
- 验收标准：
  - AC-1：get_settings 返回未配置时启动即显示向导，已配置直接进入主界面。
  - AC-2：验证失败在向导内联展示真实错误（HTTP 状态码 + 摘要），不得进入主界面。
  - AC-3：Ollama 预设允许空 API Key（本地端点）。
- 契约：`get_settings` / `test_llm_connection` / `update_llm_config` 命令；`opener` 插件跳转外链。
- 关联测试：E2E 待 CI 阶段（持久化由 TC-UI-001 覆盖）

#### REQ-UI-008 LLM 配置持久化
- 优先级：P0 ｜ 状态：Contract-Frozen ｜ 依赖：REQ-DB-001
- 描述：update_llm_config 命令将配置写入 SQLite settings 表（AES-256-GCM 加密，随机密钥存于数据目录 `secret.key`，权限 0600）；get_settings 返回配置存在性、base_url、model 与脱敏 Key（仅末四位），**严禁回传明文 Key**；启动时从 settings 表恢复运行态配置（替换 Phase 4 的 llm_config.json 手配方案）。
- 验收标准：
  - AC-1：update → 重建 AppState（模拟重启）→ get_settings 数据仍在且 Key 脱敏（TC-UI-001）。
  - AC-2：settings 表存储值不出现明文 Key（密文为 base64(nonce‖ciphertext)）。
  - AC-3：update 后运行态配置立即生效，无需重启。
- 契约（TPM 冻结，2026-07-24）：Command `update_llm_config(config: LlmConfig, state) -> Result<(), String>`；Command `get_settings(state) -> Result<SettingsPayload, String>`。
- 关联测试：TC-UI-001

### 4.5 LIC — 授权与 Freemium 付费墙

#### REQ-LIC-001 免费版配额规则
- 优先级：P0 ｜ 状态：Approved ｜ 依赖：REQ-ING-001
- 描述：免费版上限 50 个文件且禁用 PDF；配额计数本地持久化，删除文件后释放。
- 验收标准：
  - AC-1：库内达 50 文件后，任何新导入被拦截并触发付费墙（REQ-LIC-003）。
  - AC-2：批量导入时若部分文件导致超限，仅允许导入至满额，其余拦截并逐文件说明。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-LIC-002 License 激活与离线校验
- 优先级：P0 ｜ 状态：Approved ｜ 依赖：无
- 描述：用户输入 License Key 激活 Pro；采用非对称签名（Ed25519，公钥内置）本地校验，全程离线可用。
- 验收标准：
  - AC-1：有效 License 断网环境下可激活，激活状态持久化，重启后仍为 Pro。
  - AC-2：伪造 / 篡改的 License 被拒绝，且不泄露校验细节。
  - AC-3：激活记录具备防回滚保护（无法通过简单删除文件降级绕过，安全评审确认）。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-LIC-003 触顶拦截与升级引导
- 优先级：P0 ｜ 状态：Approved ｜ 依赖：REQ-LIC-001
- 描述：配额触顶或免费版导入 PDF 时，展示付费墙：说明限制、Pro 权益、License 输入入口与购买链接。
- 验收标准：
  - AC-1：付费墙内可直接输入 License；激活成功后自动继续被中断的导入。
  - AC-2：关闭付费墙不产生任何副作用，原有数据不受影响。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-LIC-004 License 停用与状态展示
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-LIC-002
- 描述：设置页展示当前授权状态（免费 / Pro），支持在本机停用 License 以便转移。
- 验收标准：
  - AC-1：停用后回落为免费版规则；已入库的 50+ 文件保留可读，但新导入按免费版拦截。
- 契约：TBD ｜ 关联测试：TBD

### 4.6 SEC — 安全与隐私

#### REQ-SEC-001 API Key 本地加密存储
- 优先级：P0 ｜ 状态：Approved ｜ 依赖：无
- 描述：BYOK API Key 仅本地存储：优先使用 OS Keychain（macOS Keychain / Windows DPAPI / Linux Secret Service），降级为 AES-256-GCM 加密文件。
- 验收标准：
  - AC-1：数据目录内任何文件不出现明文 Key（静态扫描断言）。
  - AC-2：日志、错误信息、toast 中不出现 Key 明文（脱敏断言）。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-SEC-002 渲染层 XSS 防御
- 优先级：P0 ｜ 状态：Approved ｜ 依赖：REQ-UI-002
- 描述：所有经 Markdown 渲染的内容必须过 DOMPurify 白名单消毒；禁止 `eval` 与未消毒 `innerHTML`。
- 验收标准：
  - AC-1：OWASP 常见注入向量集（脚本标签、事件属性、`javascript:` 协议等）在渲染输出中全部失效。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-SEC-003 网络出口白名单
- 优先级：P0 ｜ 状态：Approved ｜ 依赖：REQ-RAG-007
- 描述：应用零遥测；网络请求仅允许发往用户显式配置的 LLM / Embedding 端点。
- 验收标准：
  - AC-1：默认配置下（未配置端点）应用不发起任何外网请求（集成测试断言）。
  - AC-2：全部网络调用集中经 `crates/infra` 的 HTTP 适配器，core 无网络代码。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-SEC-004 本地数据目录隔离
- 优先级：P1 ｜ 状态：Draft ｜ 依赖：无
- 描述：全部应用数据（文档副本、向量库、会话、配置）集中于系统应用数据目录，文件权限 0700。
- 验收标准：
  - AC-1：数据目录权限检查通过；卸载流程可完全清除数据。
- 契约：TBD ｜ 关联测试：TBD

### 4.7 ARCH — 架构与工程基建

#### REQ-ARCH-000 Workspace 底座与核心领域契约
- 优先级：P0 ｜ 状态：Done ｜ 依赖：无
- 描述：搭建 Cargo workspace（crates/models、crates/core、crates/tauri-app，resolver "3"）；models 定义数据契约（DocStatus / Document / Chunk / RetrievalResult / ChatMessage）；core 以原生 `async fn` in trait 定义六边形端口（Loader / Splitter / Embedder / Storage / LLMProvider / Retriever）；tauri-app 提供可编译的启动骨架。
- 验收标准：
  - AC-1：`cargo check` 全 workspace 通过且 0 警告。
  - AC-2：`crates/core` 不依赖 Tauri / reqwest / rusqlite 等任何具体实现（依赖树审计）。
  - AC-3：全部端口 Trait 满足 `Send + Sync` 约束，未使用 `async-trait` 宏。
- 契约：不涉及 IPC（纯 crate 层）｜ 关联测试：CI 构建门禁（cargo check / clippy / 依赖树审计）

### 4.8 DB — 数据持久化

#### REQ-DB-001 SQLite 持久化存储
- 优先级：P0 ｜ 状态：Contract-Frozen ｜ 依赖：REQ-ARCH-000
- 描述：以 rusqlite（bundled）+ r2d2 连接池实现 Storage 端口，数据库文件位于 Tauri `app_data_dir`；启用 WAL 模式与外键级联；启动时执行崩溃恢复，将上次会话遗留的 Processing 僵尸文档置为 Failed。批量事务写入将随 Phase 4 向量写入路径以端口扩展交付（本期单条写在 WAL 下达标）。
- 验收标准：
  - AC-1：写入文档 / 分块 / 向量后销毁实例并重新实例化，数据完整可读（TC-DB-001）。
  - AC-2：手动置入 Processing 状态文档，调用 `cleanup_zombies` 后状态变为 Failed 且返回清理条数（TC-DB-002）。
  - AC-3：连接启用 WAL（journal_mode）与 foreign_keys 级联（删除文档级联清理 chunks 与 embeddings，REQ-ING-005 前置）。
- 契约（TPM 冻结，2026-07-24）：Storage Trait 契约不变，仅新增 `async fn cleanup_zombies(&self) -> anyhow::Result<usize>`；AppState 移除 InMemoryStorage，注入 SqliteStorage。
- 关联测试：TC-DB-001、TC-DB-002

### 4.9 PDF — PDF 文档解析

#### REQ-PDF-001 PDF 文档解析
- 优先级：P0 ｜ 状态：Contract-Frozen ｜ 依赖：REQ-ING-002
- 描述：基于 lopdf 实现 PdfLoader，逐页提取文本并以 `String::from_utf8_lossy` 容错解码；单页失败跳过并记录，整体失败优雅返回 Err，任何非法输入不得导致 Panic。
- 验收标准：
  - AC-1：伪造 PDF 字节流输入不 Panic，返回 Result（Err 时携带可读原因）（TC-PDF-001）。
  - AC-2：Pro 用户导入 PDF 后索引链路可完整走通（load → split → chunks 入库，打通 REQ-ING-002-AC-2）。
- 契约：复用 Loader 端口，无 IPC 变更。
- 关联测试：TC-PDF-001

### 4.10 LLM — 大模型接入

#### REQ-LLM-001 OpenAI 兼容流式对话
- 优先级：P0 ｜ 状态：Contract-Frozen ｜ 依赖：REQ-RAG-001
- 描述：基于 reqwest 调用 OpenAI 兼容端点（`POST {base_url}/v1/chat/completions`，stream=true）；SSE 流式解析（按 `\n\n` 切分事件，不完整片段字节级缓冲，`[DONE]` 结束）；精准捕获 HTTP 状态码并透传真实错误体（401/400 等）；支持多轮上下文（history 注入 messages）；base_url 规范化去尾斜杠（防双斜杠）；空 API Key 时不携带 Authorization 头（兼容本地 Ollama）。
- 验收标准：
  - AC-1：SSE 字节流被任意切断（含多字节字符边界）时解析器不崩溃、不丢字（TC-LLM-001）。
  - AC-2：非 2xx 响应返回携带状态码与响应体摘要的 Err，不 Panic。
  - AC-3：网络代码全部位于 infra，core 无 reqwest 依赖（依赖树审计）。
- 契约：LLMProvider 端口契约不变；适配器 `echomind_infra::openai_provider::OpenAIProvider`。
- 关联测试：TC-LLM-001

#### REQ-LLM-002 LLM 连接测试
- 优先级：P0 ｜ 状态：Contract-Frozen ｜ 依赖：REQ-LLM-001
- 描述：提供 test_llm_connection 命令，发送极简请求（max_tokens=1）验证 API Key 与 URL 有效性，返回可读结果。
- 验收标准：
  - AC-1：有效配置返回成功文案；无效 Key / URL / 模型返回真实 HTTP 错误信息。
- 契约（TPM 冻结，2026-07-24）：Command `test_llm_connection(api_key: String, base_url: String, model: String) -> Result<String, String>`。
- 关联测试：手测 + 设置界面 E2E（wiremock L2 用例随 CI 阶段补齐）

---

## 5. 非功能需求（NFR）

#### REQ-NFR-001 冷启动性能
- 优先级：P1 ｜ 状态：Draft
- 验收标准：AC-1：Apple Silicon 基准机上，从启动到主界面可交互 ≤ 2s（E2E 计时断言）。

#### REQ-NFR-002 检索性能
- 优先级：P1 ｜ 状态：Draft
- 验收标准：AC-1：10,000 chunks 规模下，单次 top-k 检索 P95 ≤ 500ms（基准测试断言）。

#### REQ-NFR-003 平台覆盖
- 优先级：P1 ｜ 状态：Draft
- 验收标准：AC-1：CI 产出 macOS（arm64 / x64）、Windows（x64）、Linux（x64）安装包，E2E 至少在 macOS 与 Windows 上执行。

#### REQ-NFR-004 资源占用
- 优先级：P2 ｜ 状态：Draft
- 验收标准：AC-1：常规会话（知识库 ≤ 2,000 chunks）常驻内存 ≤ 500MB。

---

## 6. v1.0 范围外（Out of Scope）

云同步与多端协同、多用户 / 权限体系、移动端、OCR 与扫描件识别、网页剪藏、`.docx` / `.pptx` 导入（v1.1 候选）、浅色主题。

---

## 7. 追溯与修订

- 每条 REQ 的「契约」字段由 TPM 在体系二阶段 3 冻结；「关联测试」字段由 QA 在阶段 2 登记。
- REQ↔TC 全量追溯矩阵维护于 `docs/TEST_STRATEGY.md` 附录。

| 版本 | 日期 | 修订摘要 | 批准 |
| --- | --- | --- | --- |
| v1.0.0 | 2026-07-24 | 骨架与 v1.0 核心需求首批定义（ING 6 / VEC 5 / RAG 7 / UI 6 / LIC 4 / SEC 4 / NFR 4，共 36 条） | PM / TPM |
| v1.0.1 | 2026-07-24 | 新增 ARCH 域与 REQ-ARCH-000（Phase 1 工程底座），需求总数 36 → 37 | TPM |
| v1.0.2 | 2026-07-24 | REQ-ING-001 导入管线细化、契约冻结；REQ-ING-004 指纹算法修订为 MD5；登记 TC-ING-001~007 | PM / TPM |
| v1.0.3 | 2026-07-24 | 新增 DB / PDF 域；REQ-DB-001（SQLite 持久化）、REQ-PDF-001（lopdf 解析）、REQ-VEC-002（fastembed 本地引擎）契约冻结；新增 crates/infra 适配器层；登记 TC-DB-001/002、TC-PDF-001、TC-VEC-001 | PM / TPM |
| v1.0.4 | 2026-07-24 | 新增 LLM 域；REQ-RAG-001 对话命令契约冻结（chat + 四事件）；REQ-RAG-003 检索引擎细化（阈值过滤 / 空上下文拒答）；新增 REQ-LLM-001（OpenAI 兼容流式）、REQ-LLM-002（连接测试）；登记 TC-RAG-001/002、TC-LLM-001 | PM / TPM |
| v1.0.5 | 2026-07-24 | REQ-UI-001/002/003/004 契约冻结；新增 REQ-UI-007（配置向导）、REQ-UI-008（settings 加密持久化）；裁决前端库本地化（禁 CDN）；REQ-RAG-007 部分交付；登记 TC-UI-001/002 | PM / TPM |
| v1.0.6 | 2026-07-24 | REQ-RAG-005（停止生成，abort_chat + CancellationToken）、REQ-RAG-006（会话持久化，conversations/messages 表）契约冻结；中断事件语义复用 chat_error/chat_done；登记 TC-RAG-003/004/005 | PM / TPM |
| v1.0.7 | 2026-07-24 | 纪律纠偏：L3 E2E 基建（WebdriverIO+tauri-driver 进 CI，Playwright 桥接层补本机）；REQ-ING-005 / REQ-UI-006 契约冻结；嵌入写入链路接通（embed_batch → add_embedding）；登记 E2E-UI-001 | PM / TPM |
