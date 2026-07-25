# EchoMind（灵犀）软件需求规格说明书 — SRS v1.0

> 本文档是产品需求的唯一真相源。所有需求原子化、可测试、可追溯。
> 规则约束见 `PROJECT_RULES.md`；测试映射与执行流程见 `docs/TEST_STRATEGY.md`。

| 项 | 内容 |
| --- | --- |
| 文档版本 | v1.3.0（文档一致性审计与矛盾检测：Decompose-Then-Verify 全量扫描式勘误） |
| 生效日期 | 2026-07-25 |
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
| VIZ | 富内容可视化渲染（图表/公式/数据可视化） |
| MM | 多模态内容理解（PDF 图片/表格/图表识别） |
| AUDIT | 文档一致性审计与矛盾检测（全量扫描式勘误） |

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

#### REQ-VEC-006 章节感知分块（Section-Aware Chunking）
- 优先级：P1 ｜ 状态：Approved ｜ 依赖：REQ-VEC-001
- 描述：当前 `SemanticSplitter` 按 `\n\n` 段落边界切分，不识别文档结构层级。对于学术论文、技术手册等结构化文档，跨段落的论证链（假设→推理→结论）可能被拆散到不同 chunk，导致检索时只命中局部、LLM 看不到完整论证上下文。本需求在现有语义分块前置一个**结构感知层**：识别 Markdown 标题层级（`#`/`##`/`###`），将「标题 + 其下所有正文」作为同一 section；section 内部再由 `SemanticSplitter` 切分；每个 chunk 内容前缀追加章节路径标记（如 `[第3章 > 3.2 实验设计]`），使 embedding 与 LLM 均能感知片段在文档中的结构位置。
- 验收标准：
  - AC-1：含 `# 引言`、`## 1.1 背景`、`# 方法` 三级标题的 Markdown 文档，分块后每个 chunk 内容以 `[章节路径]` 前缀开头，路径正确反映标题层级（TC-VEC-010）。
  - AC-2：标题与其下首段正文出现在同一 chunk，不因 token 窗口恰好满而分离（TC-VEC-010 旁证）。
  - AC-3：无标题的纯文本文档退化为现有 `SemanticSplitter` 行为，不产生空章节路径前缀（TC-VEC-010 旁证）。
  - AC-4：分块后的 chunk 内容仍为合法 UTF-8，章节路径前缀不破坏 `SemanticSplitter` 的中文句子/子句分割逻辑（TC-VEC-010 旁证）。
- 契约：TBD ｜ 关联测试：TBD
- 调研依据：LangChain `MarkdownHeaderTextSplitter`（按标题层级分块 + metadata）；LlamaIndex `SentenceSplitter`（段落优先策略）；Anthropic Contextual Retrieval 博文（chunk 上下文增强使检索失败率 ↓ 49%）。本需求为纯 `crates/core` 层算法改进，不引入新依赖。

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
- 描述：VectorRetriever 接入 LocalEmbedder 与 SqliteStorage，默认 top-k（k=5）向量相似度检索；引入相关度分数阈值过滤（score < 0.35 直接丢弃），防止低质量上下文污染；过滤后上下文为空时，直接返回固定提示，**不调用 LLM**。
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

#### REQ-RAG-008 回答语言自适应
- 优先级：P1 ｜ 状态：Approved ｜ 依赖：REQ-RAG-001、REQ-VIZ-004
- 描述：RAG 系统提示词追加语言跟随引导指令，使 LLM 以用户提问的语言回答。默认策略为 `auto`（跟随提问语言），用户无需任何配置即可获得语言匹配的回答。引用知识库中的代码、API 名称、专有术语时保留原文，不翻译。本需求仅修改系统提示词组装逻辑（`build_rag_prompt`），不改变检索、嵌入或流式解析链路。
- 验收标准：
  - AC-1：`build_rag_prompt` 输出的系统提示词包含语言跟随引导指令（TC-RAG-006）。
  - AC-2：引导指令明确要求代码/术语/API 名称保留原文不翻译（TC-RAG-006）。
  - AC-3：引导指令与现有图表引导（REQ-VIZ-004）共存，互不冲突（TC-RAG-006）。
  - AC-4：引导指令使用条件式语言（"请使用…"），不强制所有回答必须使用某一语言。
- 契约：`build_rag_prompt(sources: &[RetrievalResult]) -> String` 签名不变，仅在返回值末尾追加语言引导段。无新增 IPC 命令、无新增设置键、无架构变更。
- 关联测试：TC-RAG-006
- 调研依据：AnythingLLM Issue #97 官方维护者建议「best place to force this instruction is in the system prompt」；Dify 平台将语言策略交由开发者通过 Prompt Engineering 定义；业界无产品使用本地语言检测库（whatlang/lingua），均依赖 LLM 自身语言识别能力。

#### REQ-RAG-009 检索后段落扩展（Chunk Expansion）
- 优先级：P1 ｜ 状态：Draft ｜ 依赖：REQ-RAG-003、REQ-VEC-001
- 描述：当前 `VectorRetriever` 检索 top-k chunk 后直接注入 LLM prompt，每个 chunk 是孤立片段。对于学术论文等深度文档，用户提问「检查 3.2 节实验设计的逻辑是否自洽」时，命中的 chunk 可能只是实验设计的一个段落，缺少前后文（如假设前提、实验参数定义），导致 LLM 无法做出完整判断。本需求在检索结果返回后、注入 prompt 前，增加一个**段落扩展层**：对每个命中 chunk，根据其 `sequence` 字段向前后各扩展 1 个相邻 chunk（同一文档内），合并为完整段落后注入 prompt。扩展仅影响注入 LLM 的上下文，不改变 `chat_sources` 事件推送的引用列表（引用列表仍只展示命中的核心 chunk，避免引用膨胀）。
- 验收标准：
  - AC-1：检索命中 chunk A（sequence=5）时，注入 LLM prompt 的上下文包含同一文档的 sequence=4 和 sequence=6 的 chunk 内容（TC-RAG-007）。
  - AC-2：命中 chunk 为文档首个 chunk（sequence=0）时，仅向后扩展，不产生越界（TC-RAG-007 旁证）。
  - AC-3：命中 chunk 为文档末尾 chunk 时，仅向前扩展，不产生越界（TC-RAG-007 旁证）。
  - AC-4：`chat_sources` 事件推送的引用列表只包含原始命中的 chunk，不包含扩展 chunk（TC-RAG-007 旁证）。
  - AC-5：扩展 chunk 与命中 chunk 来自不同文档时不合并（跨文档不扩展，TC-RAG-007 旁证）。
- 契约：TBD ｜ 关联测试：TBD
- 调研依据：LlamaIndex `SentenceWindowRetriever`（检索后扩展窗口）；LangChain `ParentDocumentRetriever`（子 chunk 检索→父文档返回）；Anthropic Contextual Retrieval 博文建议「检索后提供更完整的上下文窗口」。本需求为纯 `crates/core` 层逻辑改进，不引入新依赖，不改变 `Retriever` 端口签名。

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

#### REQ-LIC-001 License 离线校验（Ed25519）
- 优先级：P0 ｜ 状态：Contract-Frozen ｜ 依赖：无
- 描述：采用 Ed25519 公私钥签名体系。用户输入 License Key（格式：`base64(payload)-base64(signature)`），后端使用内置公钥验证签名。有效则解锁 Pro，无效返回明确错误。验证状态持久化到 SQLite settings 表（key=`license.is_pro`），支持离线启动恢复。License 验证逻辑位于 `crates/core/src/license.rs`（纯计算，无 I/O 依赖，经架构评审准入 core）。
- 验收标准：
  - AC-1：使用合法私钥签名的 License，`verify_license` 返回 `Ok(())`（TC-LIC-001）。
  - AC-2：篡改签名后的 License，`verify_license` 返回 `Err`（TC-LIC-002）。
  - AC-3：激活后重启应用，`is_pro` 状态从 SQLite 恢复为 `true`（TC-LIC-005）。
  - AC-4：伪造 / 篡改的 License 被拒绝，错误消息不泄露校验细节（安全官要求）。
- 契约（TPM 冻结，2026-07-25）：Command `activate_pro(license_key: String, state: State<'_, AppState>) -> Result<bool, String>`；Command `get_pro_status(state: State<'_, AppState>) -> bool`。License 格式：`base64(payload)-base64(signature)`，payload 为签名的原始字节（如 `EchoMind-Pro-v1`），signature 为 64 字节 Ed25519 签名。
- 关联测试：TC-LIC-001、TC-LIC-002、TC-LIC-005

#### REQ-LIC-002 is_pro 状态管理与门控
- 优先级：P0 ｜ 状态：Contract-Frozen ｜ 依赖：REQ-LIC-001、REQ-ING-001
- 描述：所有 Tauri Command（如 `import_files`）在执行前必须检查 `is_pro`。免费版硬性限制：最多 50 个文件，禁止导入 PDF。`is_pro` 状态存储于 `AppState::is_pro: tokio::sync::RwLock<bool>`，启动时从 SQLite settings 表读取；`import_files` 从 AppState 读取 `is_pro`（不再信任前端参数），实现前后端双重拦截。导入拦截错误消息以结构化前缀返回：`LIMIT_REACHED:`（配额触顶）与 `PRO_REQUIRED:`（PDF 付费门）。
- 验收标准：
  - AC-1：免费版已有 50 个文件时，调用 `import_files` 返回 `LIMIT_REACHED` 错误（TC-LIC-003）。
  - AC-2：免费版导入 PDF 时，返回 `PRO_REQUIRED` 错误（TC-LIC-004）。
  - AC-3：Pro 版不受 50 文件限制且可导入 PDF。
  - AC-4：`get_pro_status` 返回当前授权状态（bool）。
- 契约：`import_files` 签名移除 `is_pro` 参数，改为从 `state.is_pro` 读取（契约变更经 Phase 8 TPM 授权）。
- 关联测试：TC-LIC-003、TC-LIC-004

#### REQ-LIC-003 付费墙 UI
- 优先级：P0 ｜ 状态：Contract-Frozen ｜ 依赖：REQ-LIC-001、REQ-LIC-002
- 描述：当免费用户触发限制时（`import_files` 返回 `LIMIT_REACHED` 或 `PRO_REQUIRED` 错误），自动弹出全屏 Modal 引导激活。Modal 包含 License 输入框和「激活」按钮；激活成功后关闭 Modal 并隐藏所有限制提示；侧栏底部显示当前授权状态（免费版 / Pro）。
- 验收标准：
  - AC-1：收到 `LIMIT_REACHED` 或 `PRO_REQUIRED` 错误时，付费墙 Modal 自动弹出（E2E 断言）。
  - AC-2：在 Modal 中输入有效 License 并点击激活后，Modal 关闭，`get_pro_status` 返回 `true`。
  - AC-3：激活成功后侧栏授权状态更新为「Pro 版」。
  - AC-4：关闭付费墙不产生任何副作用，原有数据不受影响。
- 契约：前端监听 `import_files` 错误前缀触发 Modal；调用 `activate_pro` 激活；调用 `get_pro_status` 刷新 UI。
- 关联测试：E2E-LIC-001（L3-lite）

#### REQ-LIC-004 License 停用与状态展示
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-LIC-001
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

### 4.11 VIZ — 富内容可视化渲染

> **域背景**：当前答案区仅支持 Markdown 基础语法 + 代码高亮。LLM 天然擅长生成结构化文本语法（Mermaid / LaTeX / CSV），一次集成即可覆盖 12+ 职业场景的 10+ 种图表类型，是本产品核心价值差异化所在。
>
> **技术调研来源**（铁律八互联网搜索，2026-07-25）：
> - Mermaid.js v11.16.0（⭐89,399）— 文本→10+种图表
> - Chart.js v4.5.1 — 数据可视化图表
> - KaTeX — LaTeX 数学公式渲染
> - 全职业场景调研覆盖 12 个领域，Mermaid+Chart.js+KaTeX 组合覆盖 ~65% 场景

#### REQ-VIZ-001 Mermaid 图表渲染引擎
- 优先级：P0 ｜ 状态：Done ｜ 依赖：REQ-UI-002
- 描述：在答案区支持 Mermaid 语法代码块（```mermaid），流式结束后渲染为可视化图表。支持图表类型：流程图（flowchart）、甘特图（gantt）、序列图（sequenceDiagram）、类图（classDiagram）、状态图（stateDiagram-v2）、ER 图（erDiagram）、饼图（pie）、思维导图（mindmap）、时间线（timeline）、用户旅程（journey）、C4 架构图（C4Context）、Git 分支图（gitGraph）。Mermaid.js 库本地 vendored 于 `ui/vendor/`，暗色主题（theme: 'dark'），背景对齐 #161616。流式渲染时序：流式过程中 ```mermaid 代码块显示为源码 + 「图表渲染中…」占位提示；chat_done 事件后调用 mermaid.run() 一次性渲染所有图表为 SVG。图表支持点击放大查看。
- 验收标准：
  - AC-1：LLM 输出 ```mermaid flowchart 语法时，流结束后渲染为可读流程图 SVG，非纯文本代码块（TC-VIZ-001）。
  - AC-2：流式过程中 mermaid 代码块不产生布局错乱，占位提示可见（TC-VIZ-002）。
  - AC-3：Mermaid 语法错误时显示错误提示而非崩溃，不阻塞后续渲染（TC-VIZ-003）。
  - AC-4：渲染出的 SVG 不含可执行的 <script> 标签（安全审查，TC-VIZ-006）。
  - AC-5：暗色主题下图表背景透明，文字与连线在 #161616 背景上清晰可读（TC-VIZ-004）。
- 契约：TBD（待 TPM 冻结） ｜ 关联测试：TC-VIZ-001~004、TC-VIZ-006

#### REQ-VIZ-002 KaTeX 数学公式渲染
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-UI-002
- 描述：在答案区支持 LaTeX 数学公式渲染。行内公式以 `$...$` 包裹，块级公式以 `$$...$$` 包裹。使用 KaTeX 库（本地 vendored），支持 mhchem 插件（化学方程式）。公式在流式过程中实时渲染（KaTeX 渲染速度 ≤5ms/公式）。暗色主题适配：公式文字色 `#E2E8F0`（slate-200），背景透明。
- 验收标准：
  - AC-1：LLM 输出 `$E=mc^2$` 时渲染为上标公式，非原始 LaTeX 源码（TC-VIZ-005）。
  - AC-2：块级公式 `$$\sum_{i=1}^{n} x_i$$` 居中渲染，带独立边距（TC-VIZ-005b）。
  - AC-3：化学方程式 `$\ce{H2O}$` 经 mhchem 插件正确渲染（TC-VIZ-005）。
  - AC-4：半截公式语法（流式中）不产生布局错乱（TC-VIZ-002 旁证）。
  - AC-5：KaTeX 语法错误时显示优雅错误提示，不崩溃（TC-VIZ-005c）。
  - AC-6：KaTeX 渲染输出不含 XSS 载荷（TC-VIZ-005d，REQ-SEC-002）。
- 契约：已冻结（ADR-009 §3.1 前端渲染链路） ｜ 关联测试：TC-VIZ-005、TC-VIZ-005b、TC-VIZ-005c、TC-VIZ-005d

#### REQ-VIZ-003 数据图表渲染（Chart.js）
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-UI-002
- 描述：在答案区支持数据可视化图表。采用「表格数据自动转图表」策略：检测 Markdown 表格数据，在表格上方显示「📊 切换图表视图」按钮，用户可选择柱状图/折线图/饼图。Chart.js 库本地 vendored。暗色主题适配：坐标轴文字色 `#94a3b8`，网格线 `rgba(255,255,255,0.08)`，强调色 `#38BDF8`。
- 验收标准：
  - AC-1：Markdown 表格旁出现「切换图表」按钮，点击后表格转换为对应图表（TC-VIZ-007）。
  - AC-2：柱状图/折线图/饼图三种类型可切换（TC-VIZ-007b）。
  - AC-3：图表数据与原表格数据一致（TC-VIZ-007）。
- 契约：已冻结（ADR-009 §3.3 前端表格→图表转换链路） ｜ 关联测试：TC-VIZ-007、TC-VIZ-007b

#### REQ-VIZ-004 RAG Prompt 图表引导增强
- 优先级：P0 ｜ 状态：Done ｜ 依赖：REQ-VIZ-001、REQ-RAG-003
- 描述：在 ChatEngine 的 build_rag_prompt() 中增加图表引导指令，引导 LLM 根据问题类型自动选择合适的可视化方式。指令内容：「当回答涉及流程、架构、关系、时间线时，使用 ```mermaid 代码块输出图表语法；当涉及数学公式时，使用 $...$ 或 $$...$$ 语法；当涉及数据对比时，优先使用 Markdown 表格。图表语法必须完整闭合。」此增强不改变 RAG 检索逻辑，仅修改系统提示词。
- 验收标准：
  - AC-1：提问「画一个用户登录流程图」时，LLM 回答包含 ```mermaid 代码块（TC-VIZ-008）。
  - AC-2：提问「推导 E=mc²」时，LLM 回答包含 LaTeX 公式语法（TC-VIZ-008b）。
  - AC-3：不涉及图表的问题回答不受 Prompt 引导影响，不强制输出图表语法（TC-VIZ-008c）。
- 契约：TBD ｜ 关联测试：TC-VIZ-008、TC-VIZ-008b、TC-VIZ-008c

---

### 4.12 MM — 多模态内容理解

> **域背景**：当前 PDF 导入仅提取纯文本（lopdf 文本层），图片中的文字、表格、甘特图等信息完全丢失。对于扫描版 PDF、含图表的技术文档、医学影像报告等场景，检索能力严重不足。
>
> **技术调研来源**（铁律八互联网搜索，2026-07-25）：
> - Tesseract OCR：开源 OCR 引擎（C 库 + Rust binding）
> - ColPali：2024-2025 前沿视觉文档检索（无 OCR，直接页面级视觉嵌入）
> - Jina CLIP v2：多模态嵌入模型（文本+图像统一向量空间）
> - BYOK VLM：用户配置的 Vision LLM（GPT-4o / Claude 3.5 Sonnet）经 OpenAI 兼容 Vision API
> - fastembed-rs v5.17.3：当前不支持 CLIP 视觉嵌入（Rust 生态无成熟方案）
> - PaddleOCR PP-Structure：表格识别主流方案（但需 Python 运行时，与纯 Rust 架构不兼容）

#### REQ-MM-001 PDF 图片提取与页面渲染
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-ING-002
- 描述：当前 PdfLoader 基于 lopdf 仅提取文本层。本需求扩展为：检测 PDF 页面是否含图片/XObject，将含图片的页面渲染为位图（PNG/JPEG），供后续 OCR 或 VLM 处理。渲染引擎优先选择 Rust 可用的 PDF 渲染库（pdfium 绑定或 mupdf 绑定），经架构评审确定。渲染分辨率建议 150-200 DPI（平衡清晰度与内存）。渲染为 CPU 密集任务，经 spawn_blocking 执行。Pro 版功能（免费版 PDF 导入本就被禁止，REQ-LIC-002）。
- 验收标准：
  - AC-1：含图片的 PDF 页面被渲染为位图，位图尺寸与页面比例一致（TC-MM-001）。
  - AC-2：纯文本 PDF 页面不触发渲染（跳过优化，TC-MM-001 旁证）。
  - AC-3：渲染过程不阻塞 async executor（spawn_blocking，TC-MM-001 旁证）。
- 契约：`PageRenderer` 端口（§1.7）+ `PdfiumRenderer` 适配器（§4.4） ｜ 关联测试：TC-MM-001

#### REQ-MM-002 本地 OCR 文字识别（ocrs）
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-MM-001
- 描述：对渲染出的页面位图执行本地 OCR 识别，提取图片中的文字内容。OCR 引擎使用 ocrs（纯 Rust PaddleOCR PP-OCRv4 + rten 推理引擎，ADR-010 架构评审确定），完全本地运行，零网络请求（符合「隐私不出域」承诺）。OCR 结果作为附加文本追加到该页的文本内容中，统一进入分块→嵌入→存储链路。支持中英文识别（PP-OCRv4 原生支持）。OCR 为 CPU 密集任务，经 spawn_blocking 执行。Pro 版功能。
- 验收标准：
  - AC-1：含印刷文字的图片经 OCR 提取后，文字内容出现在对应 chunk 中（TC-MM-002）。
  - AC-2：OCR 过程无网络请求（网络监控断言，TC-MM-002 旁证）。
  - AC-3：OCR 失败（如无语言包）时优雅降级，保留页面文本层内容，不崩溃（TC-MM-002 旁证）。
- 契约：`OcrEngine` 端口（§1.8）+ `OcrsEngine` 适配器（§4.5） ｜ 关联测试：TC-MM-002

#### REQ-MM-003 BYOK VLM 图片理解（表格/图表/甘特图）
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-MM-001、REQ-LLM-001
- 描述：OCR 对表格、甘特图等结构化内容的识别效果有限。本需求提供 VLM（Vision Language Model）增强路径：将渲染出的页面位图发送到用户配置的 Vision LLM（经 OpenAI 兼容 Vision API），获取结构化文本描述（表格转 Markdown 表格、甘特图转 Mermaid 语法、流程图转 Mermaid flowchart）。VLM 返回的描述文本追加到该页文本内容中，进入分块→嵌入→存储链路。**隐私边界**：图片仅发送到用户自行配置的 LLM 端点（BYOK），符合「隐私不出域」——数据出口由用户控制。前端需明确提示「图片将发送到配置的 LLM 端点」。用户可在设置中关闭 VLM 增强（仅用 OCR）。Pro 版功能。
- 验收标准：
  - AC-1：含表格的图片经 VLM 处理后，表格内容以 Markdown 表格形式出现在 chunk 中（TC-MM-003）。
  - AC-2：含甘特图的图片经 VLM 处理后，甘特图内容以 Mermaid gantt 语法出现在 chunk 中（TC-MM-003b）。
  - AC-3：VLM 不可用（未配置 Vision LLM）时优雅降级为纯 OCR，不崩溃（TC-MM-003c）。
  - AC-4：网络请求仅发往用户配置的 base_url（网络监控断言，安全官要求）。
- 契约：`VisionLanguageModel` 端口（§1.9）+ `OpenAIVisionProvider` 适配器（§4.6）+ `NoVlm` 占位实现（§1.9） ｜ 关联测试：TC-MM-003、TC-MM-003b、TC-MM-003c

#### REQ-MM-004 多模态内容导入管线编排
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-MM-001、REQ-MM-002、REQ-MM-003
- 描述：将上述三个能力编排为统一的 PDF 多模态导入管线。PdfLoader 扩展为：文本层提取 → 图片页面检测 → 页面渲染 → OCR（本地）→ VLM 增强（可选，BYOK）→ 合并全部文本 → SemanticSplitter 分块 → 嵌入 → 存储。管线状态机扩展：Processing 状态增加子阶段「text_extracting / image_rendering / ocr / vlm_enhancing」以提供更精细的进度反馈。前端 doc-status-changed 事件增加子阶段进度文案。Pro 版功能。
- 验收标准：
  - AC-1：含图文混排的 PDF 导入后，文本和图片中的文字均可被检索（TC-MM-004）。
  - AC-2：导入进度展示子阶段（文本提取→图片渲染→OCR→VLM 增强），非黑盒等待（TC-MM-004 旁证）。
  - AC-3：用户在设置中关闭 VLM 增强后，管线自动跳过 VLM 阶段，仅用 OCR（TC-MM-004 旁证）。
- 契约：`MultimodalPdfLoader`（§5.5）+ `ImportService.index_document_multimodal`（§5.4）+ `DocStatusPayload.sub_phase` ｜ 关联测试：TC-MM-004

#### REQ-MM-005 分级图表理解（Tiered Chart Understanding）
- 优先级：P1 ｜ 状态：Draft ｜ 依赖：REQ-MM-003、REQ-MM-004
- 描述：当前 VLM 图片理解使用单一提示词（`VLM_IMAGE_PROMPT`），对所有图片内容一视同仁：表格→Markdown、甘特图→Mermaid、流程图→Mermaid、**其他图表→简洁文字描述**。对于学术论文中的科学数据图表（散点图、柱状图、折线图、热力图等），「简洁文字描述」仅输出趋势概括（如"显示正相关关系"），丢失原始数据点，用户无法通过 RAG 对话核对图表中的具体数值。本需求将 VLM 提示词升级为**分级理解策略**：按内容类型分四个精度等级处理——Level 1 表格（完整 Markdown 表格）、Level 2 流程图/甘特图（Mermaid 语法）、Level 3 数据图表（先判断图表类型，再提取坐标轴标签/数据系列名称/关键数值点以 CSV 格式输出，附带趋势描述）、Level 4 示意图/照片（简洁文字描述）。Level 3 是核心改进：从"描述图表"升级为"提取图表数据"，提取的 CSV 数据进入分块→嵌入→存储链路后，用户可通过 RAG 对话查询图表中的具体数值。因 VLM 对密集数据图表的提取精度有限（存在幻觉风险），提取结果须标注「以下数据由 AI 视觉提取，可能存在误差，请核对原文」。
- 验收标准：
  - AC-1：含数据图表（如柱状图）的 PDF 页面经 VLM 处理后，chunk 中包含 CSV 格式的数据点（坐标轴标签 + 至少 3 个数值点），而非仅文字趋势描述（TC-MM-005）。
  - AC-2：含数学公式的 PDF 页面经 VLM 处理后，chunk 中包含 LaTeX 格式的公式文本（`$...$` 或 `$$...$$`），而非公式截图的 OCR 乱码（TC-MM-005b）。
  - AC-3：VLM 提取的数据图表结果末尾标注「AI 视觉提取，可能存在误差，请核对原文」提示（TC-MM-005 旁证）。
  - AC-4：VLM 不可用（未配置或关闭）时退化为现有 OCR 行为，不崩溃（TC-MM-005 旁证）。
  - AC-5：VLM 提取的 CSV 数据经分块和向量化后，可通过 RAG 对话检索到（如提问"图5的X轴标签是什么"能命中对应 chunk，TC-MM-005 旁证）。
- 契约：TBD ｜ 关联测试：TBD
- 调研依据：Salesforce AI Research CogAlign（VLM 图表理解增强，2025）；ChartHal（ICLR 2026 投稿，评估 VLM 在科学图表理解中的幻觉问题）；Marker（⭐37,816，PDF→Markdown 高精度转换，支持公式 LaTeX 还原）；Nougat（Meta，⭐10,050，学术 PDF 公式提取）。本需求不引入新依赖，仅升级 `VLM_IMAGE_PROMPT` 提示词与 `VisionLanguageModel::describe_image` 调用逻辑，纯 `crates/core` 层改动。Pro 版功能。

---

### 4.13 AUDIT — 文档一致性审计与矛盾检测

> **域背景**：标准 RAG 是「问题驱动」的单向检索（query → top-k），无法胜任「全文矛盾检测」这类需要两两比对的任务。学术文献中第 3 页的"25°C"与第 18 页的"20°C"可能因分块后被分到不同 chunk，向量检索只能找到与问题语义相似的片段，不会遍历全文做跨章节一致性检查。矛盾检测需要「双重上下文」——AI 必须同时持有两段可能矛盾的文本才能判定，而 RAG 每次只召回几个相关 chunk，无法保证矛盾的两段话被同时召回。
>
> **技术调研来源**（铁律八互联网搜索，2026-07-25）：
> - **FIND 基准**（arXiv: 2512.18601, 2025.12）——首个文档内部不一致性检测基准；GPT-5 仅恢复 64% 人工插入的不一致性，但在 50 篇 arXiv 论文上发现了 136 条作者自己遗漏的真实不一致性；证明 LLM 具备矛盾发现能力但远非完美，应定位为「辅助审计」而非「自动纠错」。
> - **Decompose-Then-Verify 范式**（arXiv: 2505.12348 / 2503.15354 / 2602.21857）——事实核查领域的共识方法论：复杂声明 → 分解为原子子声明 → 逐个验证 → 聚合判定；8B 模型经 RL 优化后可达 71.75% macro-F1。
> - **Google PAT**（arXiv: 2606.28277, 2026.06）——Agentic AI 学术审稿框架，已在 STOC/ICML 顶会试点；核心为 Inference Scaling（多轮迭代推理），比 zero-shot recall 提升 34%；提出 AI-人类协作四级分类法。
> - **SIFiD**（arXiv: 2403.07557）——用 NLI/语义相似度过滤文档关键句子，缩小检测范围，降低 LLM 调用次数。
> - **SummaC**（arXiv: 2111.09525）——句子级 NLI 分割 + 分数聚合 = 文档级一致性检测；过去失败原因是输入粒度不匹配（NLI 句子级 vs 检测文档级）。
> - **LangChain MapReduce/Refine Chain**——全量扫描式文档处理框架：Map 阶段逐块提取声明，Reduce 阶段合并比对；Refine 阶段逐块传入逐步积累发现。
> - **Rust 生态**：`rust-bert`（本地 BERT/NLI 推理）、`sqlite-knowledge-graph`（SQLite 轻量知识图谱）、`semantic-diff`（语义差异比较）等 crate 提供构建块，但无成熟「文档矛盾检测器」——EchoMind 的差异化机会。
>
> **设计原则**：
> 1. **全量扫描而非问题驱动**——审计模式遍历文档全部 chunk，不依赖向量检索的 top-k 召回。
> 2. **Decompose-Then-Verify 流水线**——先提取原子声明，再交叉验证，最后聚合报告。
> 3. **零新依赖**——复用现有 `Storage.list_chunks()` + `LLMProvider.chat_stream()` + `Embedder.embed()` 端口，纯 `crates/core` 层新增逻辑。
> 4. **辅助审计定位**——不期望 100% 召回率（FIND 证明 GPT-5 也只达 64%），定位为「AI 辅助发现，人类最终裁决」。

#### REQ-AUDIT-001 文档全量审计命令（Full-Scan Audit Mode）
- 优先级：P1 ｜ 状态：Draft ｜ 依赖：REQ-RAG-003、REQ-DB-001
- 描述：当前 `chat` 命令走 query → embedding → top-k 检索路径，无法胜任全文矛盾检测。本需求新增独立的审计命令 `audit_document(doc_id)`，走完全不同的数据路径：通过 `Storage.list_chunks(doc_id)` 获取文档全部 chunk（全量扫描），不走向量检索。审计流程分为三个阶段（对应 Decompose-Then-Verify 范式）：(1) **Decompose**——逐 chunk 提取原子声明；(2) **Verify**——对跨 chunk 的声明对做矛盾判定；(3) **Report**——聚合发现为结构化报告。审计结果以流式事件推送前端，渲染为 Block 形式的勘误报告。审计过程支持取消（CancellationToken，复用 REQ-RAG-005 的 abort 模式）。Pro 版功能。
- 验收标准：
  - AC-1：调用 `audit_document(doc_id)` 后，系统通过 `list_chunks` 获取该文档全部 chunk，而非走 `vector_search` top-k 路径（TC-AUDIT-001）。
  - AC-2：审计过程分三个阶段推送 `audit_phase` 事件（`extracting` / `comparing` / `reporting`），前端显示当前阶段（TC-AUDIT-001 旁证）。
  - AC-3：审计过程中可通过 `abort_audit` 取消，取消后已提取的声明和已发现的矛盾保留并输出部分报告（TC-AUDIT-001 旁证）。
  - AC-4：空文档（0 chunk）或不存在文档返回明确错误，不 Panic（TC-AUDIT-001 旁证）。
- 契约：TBD ｜ 关联测试：TBD
- 调研依据：LangChain `MapReduceDocumentsChain`（全量扫描范式）；Google PAT（Inference Scaling 多轮推理）；FIND 基准（文档内部不一致性检测可行性验证）。本需求不引入新依赖，复用 `Storage` + `LLMProvider` + `Embedder` 端口。

#### REQ-AUDIT-002 原子声明提取（Decompose Phase）
- 优先级：P1 ｜ 状态：Draft ｜ 依赖：REQ-AUDIT-001
- 描述：对文档的每个 chunk，调用 LLM 提取**可验证的原子声明**（Atomic Claims）。声明是文档中做出的可被事实检验的断言，包括但不限于：数值参数（"实验温度为 25°C"）、定义声明（"X 定义为 Y 的函数"）、因果声明（"A 导致 B"）、结论声明（"结果表明误差小于 5%"）。每个声明结构化为 `{claim_id, chunk_id, sequence, text, claim_type, entities}` 格式。提取使用专门设计的 Decompose Prompt，引导 LLM 逐句扫描并输出 JSON 数组。声明提取逐 chunk 独立执行（Map 阶段），可并行化。提取的声明暂存于内存（不持久化，审计结束后丢弃）。
- 验收标准：
  - AC-1：给定含数值参数的 chunk（如"实验在 25°C 下进行，误差小于 5%"），提取出的声明列表包含"温度为 25°C"和"误差小于 5%"两条原子声明（TC-AUDIT-002）。
  - AC-2：提取的声明携带来源信息（chunk_id 和 sequence），可追溯回原文位置（TC-AUDIT-002 旁证）。
  - AC-3：纯描述性文字（如"本章介绍实验设计"）不被提取为声明，只有可验证的断言才进入声明列表（TC-AUDIT-002 旁证）。
  - AC-4：LLM 返回非法 JSON 时优雅降级，跳过该 chunk 的声明提取并记录警告，不中断整个审计流程（TC-AUDIT-002 旁证）。
- 契约：TBD ｜ 关联测试：TBD
- 调研依据：Decompose-Then-Verify 范式（arXiv: 2505.12348 Reasoning-CV；arXiv: 2503.15354 Optimizing Decomposition——提出「原子性 atomicity」指标量化信息密度）；arXiv: 2510.22055（数值事实核查——数值型声明是矛盾高发区）；Google PAT（声明级验证目标记录）。本需求仅设计 Decompose Prompt + JSON 解析，纯 `crates/core` 层逻辑。

#### REQ-AUDIT-003 跨声明矛盾检测（Verify Phase）
- 优先级：P1 ｜ 状态：Draft ｜ 依赖：REQ-AUDIT-002、REQ-VEC-002
- 描述：对提取的全部原子声明做跨 chunk 矛盾检测。直接两两比对复杂度为 \(O(n^2)\)（n = 声明数），对长文档不可行。本需求采用 **SIFiD 式预筛 + LLM 精判** 两阶段策略：(1) **预筛阶段**——用现有 `Embedder.embed()` 计算所有声明的向量，按 cosine 相似度找出"高相似但来自不同 chunk"的声明对（相似度高意味着讨论同一主题，更可能存在矛盾），过滤掉明显不相关的声明对，将 \(O(n^2)\) 降至 \(O(k)\)（k = 高风险声明对数）；(2) **精判阶段**——将预筛出的高风险声明对及其原文上下文（包含 chunk 的章节路径前缀，REQ-VEC-006）交给 LLM 判定是否矛盾，LLM 输出 `{pair_id, verdict: "contradiction" | "consistent" | "unverifiable", explanation, severity}`。判定为 `contradiction` 的声明对进入审计报告。
- 验收标准：
  - AC-1：给定两个来自不同 chunk 但讨论同一参数的矛盾声明（如"温度为 25°C" vs "温度为 20°C"），预筛阶段将其标记为高风险对，精判阶段判定为 `contradiction`（TC-AUDIT-003）。
  - AC-2：两个语义无关的声明（如"温度为 25°C" vs "使用 Rust 语言"）在预筛阶段被过滤，不进入精判（TC-AUDIT-003 旁证）。
  - AC-3：LLM 精判输出包含 verdict 字段（`contradiction` / `consistent` / `unverifiable`）和 explanation 字段（TC-AUDIT-003 旁证）。
  - AC-4：预筛阶段的 cosine 相似度计算复用现有 `Embedder.embed()` 端口，不引入新依赖（TC-AUDIT-003 旁证）。
- 契约：TBD ｜ 关联测试：TBD
- 调研依据：SIFiD（arXiv: 2403.07557——NLI/语义相似度过滤关键句子，缩小检测范围）；SummaC（arXiv: 2111.09525——句子级 NLI 分割 + 分数聚合）；MultiHop-RAG（arXiv: 2401.15391——标准 RAG 在多跳推理上表现不佳，需要跨 chunk 两两比对）。本需求复用 `Embedder` 端口做预筛，零新依赖。

#### REQ-AUDIT-004 结构化审计报告生成（Report Phase）
- 优先级：P1 ｜ 状态：Draft ｜ 依赖：REQ-AUDIT-003、REQ-UI-002
- 描述：将 Verify 阶段判定为 `contradiction` 的声明对聚合成结构化审计报告，以流式 Block 渲染到对话区。报告格式为 Markdown，包含：(1) **摘要段**——审计文档名、chunk 总数、提取声明数、发现矛盾数、审计耗时；(2) **矛盾清单**——逐条列出矛盾，每条含序号、严重等级（`🔴 高` / `🟡 中` / `🟢 低`，由 LLM 在 Verify 阶段判定 severity）、矛盾位置（chunk 序号 + 章节路径前缀）、声明 A 文本、声明 B 文本、LLM 解释说明；(3) **免责声明**——"本报告由 AI 辅助生成，矛盾判定可能存在遗漏或误报，请人工核实"。报告以 `chat_token` 事件流式推送，复用现有 Markdown 渲染链路（marked.js → DOMPurify → highlight.js）。无矛盾时输出"未发现明显矛盾"。
- 验收标准：
  - AC-1：审计完成后对话区渲染为 Markdown 格式的勘误报告，包含摘要段、矛盾清单（如有）、免责声明三部分（TC-AUDIT-004）。
  - AC-2：每条矛盾包含严重等级标记（🔴/🟡/🟢）、两个矛盾声明的原文、来源 chunk 序号与章节路径（TC-AUDIT-004 旁证）。
  - AC-3：无矛盾时输出"未发现明显矛盾"文案，而非空报告（TC-AUDIT-004 旁证）。
  - AC-4：报告末尾包含免责声明"本报告由 AI 辅助生成，矛盾判定可能存在遗漏或误报，请人工核实"（TC-AUDIT-004 旁证）。
  - AC-5：报告内容经 DOMPurify 消毒，不含可执行脚本（REQ-SEC-002 安全审查，TC-AUDIT-004 旁证）。
- 契约：TBD ｜ 关联测试：TBD
- 调研依据：FIND 基准（arXiv: 2512.18601——GPT-5 仅恢复 64% 不一致性，需标注"辅助审计"定位）；Google PAT（四级 AI-人类协作分类法——Level 2: AI 辅助内容审查，人类决策）。本需求复用现有 `chat_token` 事件 + 前端 Markdown 渲染链路，零新依赖。

#### REQ-AUDIT-005 审计进度流式推送与取消
- 优先级：P1 ｜ 状态：Draft ｜ 依赖：REQ-AUDIT-001、REQ-RAG-005
- 描述：审计过程可能耗时较长（N 个 chunk × 2 次 LLM 调用——声明提取 + 矛盾精判），需要为用户提供实时进度反馈和取消能力。进度推送采用与 `chat_phase` 相同的模式：新增 `audit_phase` 事件（`extracting` / `comparing` / `reporting`），每个阶段内推送 `audit_progress` 事件（`{current, total, message}`）。取消机制复用 REQ-RAG-005 的 `CancellationToken` 模式：`abort_audit` 命令触发 token 取消，审计循环检测到取消后 break，已提取的声明和已发现的矛盾保留并输出部分报告。取消事件语义复用 `chat_error("⏹ 审计已中断")` + `chat_done`。
- 验收标准：
  - AC-1：审计过程中前端实时显示当前阶段（提取声明中 / 比对中 / 生成报告中）和进度（如"正在提取声明: 3/12 chunks"）（TC-AUDIT-005）。
  - AC-2：用户点击取消后审计立即停止，已发现的部分矛盾输出为部分报告，不 Panic（TC-AUDIT-005 旁证）。
  - AC-3：取消后输入框恢复空闲态，可立即发起其他操作（TC-AUDIT-005 旁证）。
  - AC-4：审计完成后 `chat_done` 事件推送，前端 Block 进入完成态（TC-AUDIT-005 旁证）。
- 契约：TBD ｜ 关联测试：TBD
- 调研依据：REQ-RAG-005（abort_chat + CancellationToken 模式）；REQ-RAG-001（chat_phase 事件流式进度模式）；Google PAT（Inference Scaling——多轮迭代推理需要进度反馈）。本需求复用现有 CancellationToken + 事件推送架构，零新依赖。

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
| v1.0.8 | 2026-07-25 | Phase 8 商业化冲刺：REQ-LIC-001（Ed25519 License 离线校验）、REQ-LIC-002（is_pro 状态管理与门控）、REQ-LIC-003（付费墙 UI）契约冻结；import_files 签名变更（移除 is_pro 参数，改读 AppState）；登记 TC-LIC-001~005、E2E-LIC-001 | PM / TPM |
| v1.0.9 | 2026-07-25 | 新增 VIZ 域：REQ-VIZ-001（Mermaid 图表引擎）、REQ-VIZ-002（KaTeX 数学公式）、REQ-VIZ-003（Chart.js 数据图表）、REQ-VIZ-004（RAG Prompt 图表引导）；全职业场景调研覆盖 12 个领域 | PM / TPM |
| v1.1.0 | 2026-07-25 | 新增 MM 域：REQ-MM-001（PDF 图片提取）、REQ-MM-002（本地 OCR）、REQ-MM-003（BYOK VLM 图片理解）、REQ-MM-004（多模态导入管线）；调研 ColPali/Jina CLIP/Tesseract/PaddleOCR | PM / TPM |
| v1.1.1 | 2026-07-25 | VIZ Phase 1 实施：REQ-VIZ-001（Mermaid 图表引擎）状态→In-Dev；REQ-VIZ-004（RAG Prompt 图表引导）状态→In-Dev；ADR-009 状态→Approved (Phase 1)；TC-VIZ-008/008b/008c 绿灯 | PM / TPM |
| v1.1.2 | 2026-07-25 | VIZ Phase 1+2 验收完成：REQ-VIZ-001/004→Done、REQ-VIZ-002→Done（ADR-009 Phase 1+2 已实施验证）；MM 域登记：ADR-010 Approved（Phase 1 架构评审）；文档同步：code-contracts 补充 SemanticSplitter、frontend-modules/settings-reference 创建、session-handoff 修订 | TPM |
| v1.1.3 | 2026-07-25 | MM Phase 3 L2 契约测试验收：REQ-MM-001/002/004→Done（TC-MM-001/002/004 全部绿灯）；L2 测试使用 lopdf 编程生成含图片 XObject 的测试 PDF + Mock PageRenderer/OcrEngine 验证端口契约 | TPM |
| v1.1.4 | 2026-07-25 | MM Phase 2 L2 契约测试验收：REQ-MM-003→Done（TC-MM-003/003b/003c 全部绿灯）；VisionLanguageModel 端口 + NoVlm 占位 + OpenAIVisionProvider 适配器 + ImportService.index_document_multimodal_with_vlm + MultimodalPdfLoader 三泛型扩展 | TPM |
| v1.1.5 | 2026-07-25 | 新增 REQ-RAG-008（回答语言自适应）：`build_rag_prompt` 追加语言跟随引导指令（auto 策略，代码/术语保留原文）；调研依据 AnythingLLM Issue #97 + Dify 官方文档；登记 TC-RAG-006 | PM / TPM |
| v1.2.0 | 2026-07-25 | 深度文档理解 Phase 1 需求起草（教授文章勘误场景）：新增 REQ-VEC-006（章节感知分块，标题层级→章节路径前缀）、REQ-RAG-009（检索后段落扩展，sequence ±1 相邻 chunk 合并注入）、REQ-MM-005（分级图表理解，VLM 提示词四级策略：表格→MD/流程图→Mermaid/数据图表→CSV+趋势/公式→LaTeX）；调研依据 LangChain MarkdownHeaderTextSplitter + LlamaIndex SentenceWindowRetriever + Anthropic Contextual Retrieval + Marker/Nougat/ChartHal；三条需求均为纯 crates/core 层改动，零新依赖 | PM / TPM |
| v1.2.1 | 2026-07-25 | REQ-VEC-006 章节感知分块状态 Draft→Approved + 落地（SectionAwareSplitter pulldown-cmark 标题层级解析 → section 切分 → 委托 SemanticSplitter → 章节路径前缀；ImportService.index_inner .md 接入；TC-VEC-010 绿灯） | TPM |
| v1.3.0 | 2026-07-25 | 新增 AUDIT 域（文档一致性审计与矛盾检测）：REQ-AUDIT-001（全量审计命令，list_chunks 全扫描非 top-k 检索）、REQ-AUDIT-002（原子声明提取，Decompose Phase）、REQ-AUDIT-003（跨声明矛盾检测，SIFiD 式 embedding 预筛 + LLM 精判两阶段）、REQ-AUDIT-004（结构化审计报告，Markdown 勘误报告 + 免责声明）、REQ-AUDIT-005（审计进度流式推送与取消，复用 CancellationToken）；调研依据 FIND 基准(arXiv:2512.18601) + Decompose-Then-Verify 范式 + Google PAT(arXiv:2606.28277) + SIFiD(arXiv:2403.07557) + SummaC(arXiv:2111.09525) + LangChain MapReduce/Refine + Rust 生态(rust-bert/sqlite-knowledge-graph/semantic-diff)；五条需求均为纯 crates/core 层改动，零新依赖，复用 Storage + LLMProvider + Embedder 端口 | PM / TPM |
