# EchoMind（灵犀）软件需求规格说明书 — SRS v1.0

> 本文档是产品需求的唯一真相源。所有需求原子化、可测试、可追溯。
> 规则约束见 `PROJECT_RULES.md`；测试映射与执行流程见 `docs/TEST_STRATEGY.md`。

| 项 | 内容 |
| --- | --- |
| 文档版本 | v5.9.0（v1.4 版本规划：5 条 Draft REQ 新增 + §6 v1.4 实施计划 + README Roadmap 更新） |
| 生效日期 | 2026-07-30 |
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
| DS | 设计系统规范（色板 / 字体 / 间距 / 图标 / 组件 / 动效 / 主题） |
| IX | 交互设计规范（手势 / 右键菜单 / 拖拽排序 / 反馈 / 过渡） |
| KB | 键盘快捷键与命令面板 |
| A11Y | 无障碍（ARIA / 焦点管理 / 对比度 / 屏幕阅读器） |
| NAV | 导航与信息架构（侧栏 / 面包屑 / 全局搜索） |
| WS | 工作空间与多知识库管理 |
| EXP | 数据导出与备份恢复 |
| ERR | 错误处理与降级策略 |
| OBS | 可观测性与诊断（日志 / 性能监控 / 诊断面板） |
| WIN | 窗口管理与系统集成 |
| I18N | 国际化与本地化 |
| HELP | 帮助与引导系统 |

### 2.2 验收标准编号
`REQ-<域>-<序号>-AC-<n>`。每条 AC 必须能被至少一个测试用例（`TC-*` / `E2E-*`）自动验证。

### 2.3 需求状态机
`Draft → Approved → Test-Red → Contract-Frozen → In-Dev → Done → Verified`，另设 `Deprecated`（须注明替代条目）。
状态流转与 `PROJECT_RULES.md` 体系二流水线一一对应，由 TPM 维护。

#### v1.0 发布分类决策记录（2026-08-01）

对 25 条 P1 Draft 需求进行 v1.0 发布范围审查，分类为三档：

| 分类 | 数量 | 说明 |
|---|---|---|
| v1.0 必须交付 | 10 条 | 用户可感知的核心体验缺失或安全/数据完整性阻塞项 |
| v1.0 力争交付 | 5 条 | 提升体验但非阻塞，已部分实现 |
| 延后到 v1.1 候选 | 9 条 | 后续版本候选，设计系统形式化 / 无障碍 / 诊断增强 |
| 已完成（无需操作） | 1 条 | REQ-SEC-001 实为 P0/Implemented |

- **v1.0 必须交付**（状态 Draft → Approved）：REQ-ERR-001、REQ-ERR-004、REQ-ERR-005、REQ-IX-005、REQ-IX-006、REQ-NFR-008、REQ-WIN-001、REQ-ING-013、REQ-KB-001、REQ-NFR-017
- **v1.0 力争交付**（状态 Draft → Approved）：REQ-RAG-012、REQ-IX-003、REQ-NAV-001、REQ-RAG-017、REQ-RAG-016
- **延后到 v1.1 候选**（v1.0.1 P1 Draft 收尾后全部升级为 Implemented 或 Deprecated）：REQ-DS-001→Implemented、REQ-DS-002→Implemented、REQ-DS-003→Implemented、REQ-DS-005→Implemented、REQ-A11Y-002→Implemented、REQ-A11Y-003→Implemented、REQ-OBS-001→Implemented(v1.0)、REQ-WIN-002→Implemented、REQ-NAV-003→Deprecated

详细实现计划见 `docs/v1.0-release-checklist.md`。

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

## 3. 用户旅程总览（v2.0）

### 3.1 核心旅程

1. **首次启动**：空状态引导 → 进入配置向导选择 Provider 预设 → 填写 BYOK（Provider / base_url / model / API Key，Key 本地加密保存）→ 连接测试通过 → 进入主界面。
2. **构建知识库**：拖拽文件入窗 → 全屏遮罩「数据仅本地处理」→ 解析 / 分块 / 向量化 → 列表徽标实时显示索引状态（待索引 / 索引中 / 已索引 / 失败）→ 进度条与微批次进度反馈 → 可中途取消。
3. **知识问答**：底部控制台输入问题 → 检索命中片段注入上下文（向量 + 关键词混合检索）→ 流式 Block 渲染（Markdown + 代码高亮 + Mermaid + KaTeX + Chart.js）→ 引用块可回溯来源文档与片段 → 回答语言跟随提问语言。
4. **触顶转化**：免费版导入第 51 个文件或导入 PDF 时 → 付费墙拦截并给出升级引导 → 输入 License 激活 Pro → 即刻解锁并继续被中断的操作。
5. **深度文档理解**：Pro 用户对结构化文档发起审计 → 全量扫描式矛盾检测 → 勘误报告流式渲染 → 引用位置可追溯。

### 3.2 辅助旅程

6. **知识库管理**：文档搜索 / 筛选 / 排序 → 批量删除 → 文档预览 → 失败重试 → 索引重建。
7. **会话管理**：历史会话列表 → 搜索定位 → 加载续聊 → 删除清理 → 对话导出。
8. **设置与配置**：LLM 配置修改 → 连接测试 → VLM 开关 → 混合检索开关 → 嵌入模型选择 → 模型缓存管理 → 授权管理 → 数据导出/备份。
9. **导航与搜索**：侧栏折叠展开 → 全局搜索 → 会话切换 → 文档跳转 → 引用定位 → 键盘快捷键操作。
10. **错误恢复**：操作失败 → 统一 toast 反馈 → 错误码与可读原因 → 重试 / 恢复引导 → 离线降级。
11. **帮助与引导**：首次使用引导 → 快捷键面板 → FAQ → 关于页面 → 检查更新。

---

## 4. 功能需求

### 4.1 ING — 文档导入与管理

#### REQ-ING-001 拖拽导入本地文件
- 优先级：P0 ｜ 状态：Done ｜ 依赖：无
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
- 优先级：P0 ｜ 状态：Implemented ｜ 依赖：REQ-ING-001
- 描述：v1.0 支持 `.md`、`.txt`（免费版）；`.pdf` 为 Pro 专属格式；其余格式一律拒绝。
- 验收标准：
  - AC-1：免费版导入 `.pdf` 被拦截，弹出付费墙升级引导（关联 REQ-LIC-003）。
  - AC-2：Pro 版导入 `.pdf` 可正常进入解析与索引流程。
  - AC-3：白名单外格式（如 `.exe`、`.docx`）被明确拒绝并提示当前支持格式列表。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-ING-003 文件选择器导入
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-ING-001
- 描述：除拖拽外，用户可通过「导入文件」按钮打开系统文件选择器导入，支持多选。
- 验收标准：
  - AC-1：选择器仅列出当前许可证允许的格式；多选确认后批量入库。
  - AC-2：导入结果与拖拽路径一致（同一入库与索引管线）。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-ING-004 文件指纹去重
- 优先级：P0 ｜ 状态：Implemented ｜ 依赖：REQ-ING-001
- 描述：以文件内容 MD5 作为指纹，防止同一内容重复入库（v1.0.2 修订：SHA-256 → MD5；指纹仅用于内容去重，非安全校验场景）。
- 验收标准：
  - AC-1：导入与库内已有文件内容完全相同的文件时，跳过导入并提示「该文件已存在」，不产生重复条目与重复向量。
  - AC-2：同名但内容不同的文件正常入库，互不影响。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-ING-005 删除文档与级联清理
- 优先级：P0 ｜ 状态：Done ｜ 依赖：REQ-ING-001、REQ-VEC-003
- 描述：用户可从知识库删除文档，其分块与向量数据经外键级联同步清理，数据目录中的文档副本一并删除。删除文档**不影响**会话与聊天记录。
- 验收标准：
- AC-1：删除后文档条目、全部 chunk 与向量从本地存储移除（前端经列表 × 按钮触发，E2E 断言 DOM 消失）。
- AC-2：删除后对该文档的检索不再返回任何结果；免费版文件计数同步释放。
- AC-3：删除文档**不删除** `conversations` 表和 `messages` 表中的任何记录——两者与 `documents` 表无外键关联，级联删除链仅覆盖 `documents → chunks → embeddings`。
- AC-4：删除文档后前端聊天区域**保持不变**——当前对话的消息、引用来源等 UI 状态不受文档删除操作影响。即使知识库被清空至 0 个文档，聊天区域也不得被清空或重置（仅禁用输入框）。
- AC-5：历史消息中引用了已删除文档的 `sources` JSON 仍保留原始数据（文档名、片段内容），作为历史记录的一部分；用户可正常查看历史引用内容，但这些引用无法再定位到知识库中的原文。
- 契约（TPM 冻结，2026-07-24）：Command `delete_document(id: String, state) -> Result<(), String>`；Command `get_documents(state) -> Result<Vec<Document>, String>`（列表读取，REQ-UI-006 共用）。
- 关联测试：E2E-UI-001（L3）、TC-ING-DEL-001（L2 集成测试：删除文档后会话/消息不受影响）

#### REQ-ING-006 导入进度与取消
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-ING-001
- 描述：批量导入时展示整体进度，并允许中途取消。
- 验收标准：
  - AC-1：进度以事件推送至前端（已完成数 / 总数 / 当前文件名）。
  - AC-2：取消后已完成部分保留，未开始部分不再执行，不产生半成品索引。
- 契约（2026-07-25 冻结）：Command `abort_import(state) -> Result<(), String>`；Event `import-progress` 负载 `ImportProgressPayload { completed, total, current_file, cancelled }`；`AppState.import_cancel: Arc<AtomicBool>` 全局取消标志。
- 关联测试：TC-ING-006（L2 集成测试）

#### REQ-ING-007 文档列表搜索与筛选
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-UI-006
- 描述：知识库列表上方提供搜索框，支持按文件名模糊搜索；支持按索引状态（全部 / 已索引 / 索引中 / 失败 / 待索引）筛选；支持按文件格式（.md / .txt / .pdf）筛选。搜索结果实时过滤，无需刷新。
- 验收标准：
  - AC-1：在搜索框输入关键词后，列表仅显示文件名包含该关键词的文档（大小写不敏感）。
  - AC-2：选择「失败」筛选后，列表仅显示 Failed 状态文档。
  - AC-3：搜索与筛选条件可组合使用，结果取交集。
  - AC-4：无匹配结果时显示「未找到匹配文档」空状态，非空白。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-ING-008 文档列表排序
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-UI-006
- 描述：知识库列表支持按导入时间（默认倒序）、文件名（A-Z / Z-A）、文件大小排序。排序状态在当前会话内保持。
- 验收标准：
  - AC-1：点击列头或排序按钮切换排序方式，列表即时重新排列。
  - AC-2：再次点击同一排序方式切换升序 / 降序。
  - AC-3：排序选择在应用运行期间持久（不要求跨重启持久化）。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-ING-009 批量删除文档
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-ING-005
- 描述：知识库列表支持多选模式（复选框或长按多选），选中多个文档后一键批量删除，删除前弹出确认对话框显示待删数量与释放配额。
- 验收标准：
  - AC-1：进入多选模式后，每个文档项前出现复选框；底部出现批量操作栏。
  - AC-2：批量删除前弹出确认对话框，显示「将删除 N 个文档」，确认后级联清理全部选中文档的 chunks 与向量。
  - AC-3：取消确认对话框不产生任何删除操作。
  - AC-4：批量删除完成后列表实时刷新，配额计数同步更新。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-ING-010 文档内容预览
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-UI-006
- 描述：点击文档列表中的文档名可展开内容预览面板（侧滑或模态），显示文档元数据（文件名、格式、大小、导入时间、chunk 数、索引状态）与原文前 500 字预览。Indexed 文档额外显示 chunk 列表，每个 chunk 可展开查看完整内容。
- 验收标准：
  - AC-1：点击文档名弹出预览面板，显示元数据信息。
  - AC-2：原文预览区显示前 500 字，超长部分以「…」截断。
  - AC-3：Indexed 文档的 chunk 列表按 sequence 排列，点击展开显示完整 chunk 文本。
  - AC-4：预览面板可关闭（Esc 或 × 按钮），关闭后焦点回到文档列表。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-ING-011 导入历史记录
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-DB-001
- 描述：记录每次导入操作的时间戳、文件名、格式、结果（成功 / 失败 / 跳过-重复）、失败原因（如有），存入 SQLite `import_log` 表。用户可在设置面板查看导入历史（最近 100 条），支持按结果筛选。
- 验收标准：
  - AC-1：每次导入操作后 `import_log` 表新增一条记录，含时间戳、文件名、格式、结果。
  - AC-2：导入历史面板按时间倒序显示记录，失败记录附带可读原因。
  - AC-3：支持按结果（成功 / 失败 / 跳过）筛选历史记录。
  - AC-4：历史记录上限 100 条，超过自动淘汰最旧记录。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-ING-012 文档内容更新（重新导入）
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-ING-001、REQ-ING-004
- 描述：当用户导入与库中已有文档同名但内容不同的文件时，提供「替换」选项：删除旧文档及其 chunks / 向量，导入新版本。不同于去重跳过（内容相同）和全新导入（名称不同），此场景为同名内容变更。
- 验收标准：
  - AC-1：导入同名不同内容的文件时，弹出「文件已存在但内容不同，是否替换？」确认对话框。
  - AC-2：确认替换后，旧文档及其 chunks / 向量级联删除，新文档进入完整导入管线。
  - AC-3：取消替换后跳过该文件，不影响同批次其他文件导入。
  - AC-4：替换后文档 ID 更新为新导入的 ID，引用该文档的历史会话中的引用可能失效（标注「来源已更新」）。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-ING-013 文件大小限制与警告
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-ING-001
- **v1.0 发布分类**：v1.0 必须交付（已实现，状态待验收）｜ 决策日期：2026-08-01
- 描述：导入前检查文件大小，单文件超过阈值（默认 100MB）时弹出警告确认，告知用户大文件导入耗时较长。单文件超过硬上限（500MB）时拒绝导入并返回明确错误。阈值可在设置中调整。
- 验收标准：
  - AC-1：导入 >100MB 文件时弹出警告对话框，显示文件大小与预估耗时，用户确认后继续。
  - AC-2：导入 >500MB 文件时直接拒绝，返回「文件过大，最大支持 500MB」错误。
  - AC-3：警告对话框可取消，取消后该文件不导入。
  - AC-4：批量导入时，对每个超限文件分别处理（警告 / 拒绝），不影响其他文件。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-ING-014 文件夹批量导入
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-ING-003
- 描述：文件选择器支持选择文件夹，递归扫描其中的受支持格式文件（.md / .txt / .pdf），批量导入。扫描时跳过隐藏文件（以 `.` 开头）和不受支持的格式，不报错。
- 验收标准：
  - AC-1：文件选择器可选择文件夹，选择后递归扫描受支持格式文件。
  - AC-2：隐藏文件（以 `.` 开头的文件/目录）被跳过，不导入。
  - AC-3：不受支持的格式被跳过，不报错。
  - AC-4：扫描完成后显示找到的文件数量，进入批量导入流程。
  - AC-5：空文件夹（无受支持文件）时提示「未找到可导入的文件」。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-ING-015 .docx 文档导入
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-ING-001、REQ-ING-002
- **v1.1 规划**：S44 ｜ 决策日期：2026-08-06 ｜ 实现日期：2026-08-06
- 描述：新增 `.docx` 文件格式导入支持。使用 `docx-rs` v0.4.22（纯 Rust）解析 Office Open XML 格式，提取段落文本 + 表格（转 Markdown 表格）+ 标题层级。图片以 `[图片]` 占位符替代，不提取二进制内容。段落边界以双换行保留，与 MarkdownLoader 一致。
- 验收标准：
  - AC-1：简单段落 .docx 导入后提取全部段落文本，段落间双换行分隔（TC-ING-DOCX-001）。
  - AC-2：含表格 .docx 导入后表格转换为 Markdown 表格语法（TC-ING-DOCX-002）。
  - AC-3：含图片 .docx 导入后图片替换为 `[图片]` 占位符，不崩溃（TC-ING-DOCX-003）。
  - AC-4：损坏 .docx 文件返回 `Err`，不 Panic（TC-ING-DOCX-004）。
- 契约：`DocxLoader` struct 实现 `Loader` trait ｜ `ALLOWED_EXTENSIONS` 新增 `"docx"` ｜ 依赖 `docx-rs = "0.4"` ｜ 关联测试：TC-ING-DOCX-001~004

#### REQ-ING-016 .html 文档导入
- 优先级：P1 ｜ 状态：Draft ｜ 依赖：REQ-ING-001、REQ-ING-002
- **v1.1 规划**：S45 ｜ 决策日期：2026-08-06
- 描述：新增 `.html` 文件格式导入支持。使用 `html2text` v0.17.1（纯 Rust）解析 HTML DOM，提取正文内容（去除 `<nav>`/`<header>`/`<footer>`/`<script>`/`<style>`），保留标题层级（`<h1>`~`<h6>` → `#`~`######`）、链接文本、列表结构。
- 验收标准：
  - AC-1：简单 HTML 导入后提取正文文本，去除 `<script>`/`<style>` 内容（TC-ING-HTML-001）。
  - AC-2：含标题层级 HTML 导入后 `<h1>`~`<h3>` 转换为 `#`~`###`（TC-ING-HTML-002）。
  - AC-3：含导航栏 HTML 导入后 `<nav>` 内容被去除（TC-ING-HTML-003）。
  - AC-4：损坏 HTML 文件容错解析，不崩溃（TC-ING-HTML-004）。
- 契约：`HtmlLoader` struct 实现 `Loader` trait ｜ `ALLOWED_EXTENSIONS` 新增 `"html"` ｜ 依赖 `html2text = "0.17"` ｜ 关联测试：TC-ING-HTML-001~004

#### REQ-ING-017 .pptx 演示文稿导入
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-ING-001、REQ-ING-002
- **v1.2 规划**：S50 ｜ 决策日期：2026-08-06 ｜ 实现日期：2026-08-06
- 描述：新增 `.pptx` 文件格式导入支持。使用 `pptx-to-md` v1.0.0（纯 Rust，MIT/Apache-2.0）将 PowerPoint 演示文稿转换为 Markdown 文本。提取每张幻灯片的标题（→ `#` 标题）、要点（→ `-` 列表）、备注，表格转换为 Markdown 表格，图片以 `[图片]` 占位符替代。每张幻灯片以 `---` 分隔。
- 验收标准：
  - AC-1：简单幻灯片 .pptx 导入后提取标题 + 要点文本（TC-ING-PPTX-001）。
  - AC-2：含表格 .pptx 导入后表格转换为 Markdown 表格语法（TC-ING-PPTX-002）。
  - AC-3：含图片 .pptx 导入后图片替换为 `[图片]` 占位符，不崩溃（TC-ING-PPTX-003）。
  - AC-4：损坏 .pptx 文件返回 `Err`，不 Panic（TC-ING-PPTX-004）。
- 契约：`PptxLoader` struct 实现 `Loader` trait ｜ `ALLOWED_EXTENSIONS` 新增 `"pptx"` ｜ 依赖 `pptx-to-md = "1.0"` ｜ 关联测试：TC-ING-PPTX-001~004

#### REQ-ING-018 .epub 电子书导入
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-ING-001、REQ-ING-002
- **v1.2 规划**：S51 ｜ 决策日期：2026-08-06 ｜ 实现日期：2026-08-07
- 描述：新增 `.epub` 文件格式导入支持。使用 `rbook` v0.7.10（纯 Rust，Apache-2.0，支持 EPUB 2/3）解析电子书。按 spine（阅读顺序）遍历章节，提取 XHTML 内容并通过已有 `html2text` crate 转为纯文本。每章以 `---` 分隔，HTML 标签（`<b>`/`<i>`/`<a>`/`<img>` 等）被剥离，纯文本保留。
- 验收标准：
  - AC-1：简单 EPUB 导入后提取章节文本，章节间分隔（TC-ING-EPUB-001）。
  - AC-2：多章节 EPUB 按 spine 顺序提取，标题层级保留（TC-ING-EPUB-002）。
  - AC-3：含 HTML 标签 EPUB 导入后标签被剥离（含 `<img>` 图片标签），纯文本保留，不崩溃（TC-ING-EPUB-003）。
  - AC-4：损坏 EPUB 文件返回 `Err`，不 Panic（TC-ING-EPUB-004）。
- 契约：`EpubLoader` struct 实现 `Loader` trait ｜ `ALLOWED_EXTENSIONS` 新增 `"epub"` ｜ 依赖 `rbook = { version = "0.7", default-features = false, features = ["threadsafe", "prelude"] }` ｜ 关联测试：TC-ING-EPUB-001~004

#### REQ-ING-021 .xlsx/.csv 电子表格导入
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-ING-001、REQ-ING-002
- **v1.4 规划**：S61 ｜ 决策日期：2026-08-07 ｜ 实现日期：2026-08-07
- 描述：新增 `.xlsx` / `.csv` 文件格式导入支持。使用 `calamine` v0.36（纯 Rust，MIT/Apache-2.0，支持 xlsx/xls/ods/xlsb）解析电子表格，使用 `csv` v1.4（纯 Rust，MIT/Unlicense）解析 CSV 文件。表格数据按行转换为 Markdown 表格格式（`| col1 | col2 |`），首行作为表头。多 Sheet 以 `---` 分隔，每个 Sheet 前添加 `## Sheet 名` 二级标题。CSV 自动检测分隔符（逗号/分号/Tab）。
- 验收标准：
  - AC-1：简单 .xlsx 导入后提取表头 + 数据行，转换为 Markdown 表格语法（TC-ING-XLSX-001）。
  - AC-2：多 Sheet .xlsx 导入后所有 Sheet 内容提取，Sheet 间分隔，Sheet 名保留（TC-ING-XLSX-002）。
  - AC-3：空 Sheet .xlsx 不崩溃，返回 Err（TC-ING-XLSX-003）。
  - AC-4：损坏 .xlsx 文件返回 Err，不 Panic（TC-ING-XLSX-004）。
  - AC-5：逗号分隔 .csv 导入后提取表头 + 数据行，转换为 Markdown 表格（TC-ING-CSV-001）。
  - AC-6：分号分隔 .csv 自动检测分隔符，正确解析（TC-ING-CSV-002）。
  - AC-7：含空行 .csv 跳过空行不崩溃（TC-ING-CSV-003）。
- 契约：`XlsxLoader` struct + `CsvLoader` struct 实现 `Loader` trait ｜ `ALLOWED_EXTENSIONS` 新增 `"xlsx"` + `"csv"`（13→15）｜ 依赖 `calamine = "0.36"` + `csv = "1.4"` ｜ 关联测试：TC-ING-XLSX-001~004 + TC-ING-CSV-001~003

### 4.2 VEC — 解析、分块与向量化索引

#### REQ-VEC-001 文档解析与分块
- 优先级：P0 ｜ 状态：Implemented ｜ 依赖：REQ-ING-001
- 描述：对入库文档进行文本提取与分块；v1.0 默认策略为固定窗口 512 tokens、重叠 64 tokens（参数集中于配置，可评审调整）。
- 验收标准：
  - AC-1：Markdown 按结构边界优先切分，代码块不被从中间截断。
  - AC-2：中英文混排文本分块后无乱码、无半个 UTF-8 字符。
  - AC-3：每个 chunk 携带元数据（文档 ID、序号、字符区间）。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-VEC-002 Embedding 执行通道（可插拔）
- 优先级：P0 ｜ 状态：Done ｜ 依赖：REQ-VEC-001
- 描述：Embedding 能力以端口 Trait 抽象，v1.0 提供本地内置模型（默认，数据不出机）与 BYOK 远程端点（可选，用户显式配置）两种适配器。本地引擎（Phase 3 冻结）：fastembed 加载 all-MiniLM-L6-v2 量化模型（384 维）；批量推理经 `spawn_blocking` 执行、推理线程数设为系统核心数；模型下载多源容错（先 `https://huggingface.co`，失败回退 `https://hf-mirror.com`），强制全量下载、禁用断点续传（避免镜像源 Content-Range 报错）。
- 验收标准：
  - AC-1：`crates/core` 中仅存在 Embedding 端口 Trait，无任何具体实现依赖（适配器位于 crates/infra）。
  - AC-2：批量推理返回向量数量与输入匹配、维度 384、非全零（TC-VEC-001）。
  - AC-3：官方源不可达时自动回退镜像源；双侧均失败时返回携带双侧原因的可读 Err。
- 契约：Embedder 端口契约不变；适配器 `echomind_infra::local_embedder::LocalEmbedder`。
- 关联测试：TC-VEC-001

> TPM 注（v1.0.3）：上游指令曾以「REQ-VEC-001」指代本地向量化引擎；因该编号已被「文档解析与分块」占用，本地引擎需求归口本条（REQ-VEC-002），避免编号冲突。

#### REQ-VEC-003 向量持久化与索引
- 优先级：P0 ｜ 状态：Done ｜ 依赖：REQ-VEC-002
- 描述：向量与 chunk 元数据持久化于本地 SQLite（embeddings 表，BLOB 存储）；导入管线已接通嵌入写入链路（分块入库 → embed_batch → add_embedding，失败置 Failed 并推送事件），随应用重启可复用。
- 验收标准：
  - AC-1：索引完成后重启应用，无需重建即可检索。
  - AC-2：向量库文件位于应用数据目录内，不污染用户文档原始位置。
- 契约：Storage 端口新增 `list_documents / delete_document / list_chunks`（REQ-ING-005 与嵌入接线共用）。
- 关联测试：TC-DB-001（旁证）、E2E-UI-001

#### REQ-VEC-004 索引状态机与事件推送
- 优先级：P0 ｜ 状态：Implemented ｜ 依赖：REQ-VEC-002、REQ-UI-006
- 描述：每个文档的索引生命周期为 `pending → processing → done / failed`，状态变更通过 Tauri Event 实时推送前端。
- 验收标准：
  - AC-1：每次状态迁移都发射事件，负载含文档 ID、新状态、失败原因（如有）。
  - AC-2：前端列表徽标与状态机严格一致，无「已完成但未刷新」情况。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-VEC-005 索引失败重试
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-VEC-004
- 描述：失败文档支持手动重试；瞬时网络错误自动退避重试（上限 3 次）。
- 验收标准：
  - AC-1：手动点击「重试」后文档重新进入 `pending`。
  - AC-2：自动重试达到上限后状态置为 `failed` 并给出原因。
- 契约（2026-07-25 冻结）：Command `retry_index(id: String, state) -> Result<(), String>`；`Storage::delete_chunks_by_doc` 清理旧分块；`MAX_RETRY_ATTEMPTS = 3` 指数退避（500ms→1s→2s）。
- 关联测试：TC-VEC-005（L2 集成测试）

#### REQ-VEC-006 章节感知分块（Section-Aware Chunking）
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-VEC-001
- 描述：当前 `SemanticSplitter` 按 `\n\n` 段落边界切分，不识别文档结构层级。对于学术论文、技术手册等结构化文档，跨段落的论证链（假设→推理→结论）可能被拆散到不同 chunk，导致检索时只命中局部、LLM 看不到完整论证上下文。本需求在现有语义分块前置一个**结构感知层**：识别 Markdown 标题层级（`#`/`##`/`###`），将「标题 + 其下所有正文」作为同一 section；section 内部再由 `SemanticSplitter` 切分；每个 chunk 内容前缀追加章节路径标记（如 `[第3章 > 3.2 实验设计]`），使 embedding 与 LLM 均能感知片段在文档中的结构位置。
- 验收标准：
  - AC-1：含 `# 引言`、`## 1.1 背景`、`# 方法` 三级标题的 Markdown 文档，分块后每个 chunk 内容以 `[章节路径]` 前缀开头，路径正确反映标题层级（TC-VEC-010）。
  - AC-2：标题与其下首段正文出现在同一 chunk，不因 token 窗口恰好满而分离（TC-VEC-010 旁证）。
  - AC-3：无标题的纯文本文档退化为现有 `SemanticSplitter` 行为，不产生空章节路径前缀（TC-VEC-010 旁证）。
  - AC-4：分块后的 chunk 内容仍为合法 UTF-8，章节路径前缀不破坏 `SemanticSplitter` 的中文句子/子句分割逻辑（TC-VEC-010 旁证）。
- 契约：TBD ｜ 关联测试：TBD
- 调研依据：LangChain `MarkdownHeaderTextSplitter`（按标题层级分块 + metadata）；LlamaIndex `SentenceSplitter`（段落优先策略）；Anthropic Contextual Retrieval 博文（chunk 上下文增强使检索失败率 ↓ 49%）。本需求为纯 `crates/core` 层算法改进，不引入新依赖。

#### REQ-VEC-008 模型下载管理
- 优先级：P2 ｜ 状态：Implemented ｜ 依赖：REQ-VEC-002
- 描述：当前 `LocalEmbedder` 的 ONNX 模型下载（~30MB）在首次使用 `chat` 命令时隐式触发，用户看到空白等待不知发生了什么（"懒加载 embedder" 固有问题）。本需求提供：(1) **显式初始化 IPC 命令** `init_embedder`——前端可在配置向导完成或设置面板中主动触发模型下载，通过 `model_download_progress` 事件推送进度（`downloading` / `loading` / `done` / `error`），每文件推送 `(file_name, current, total)` 进度；(2) **缓存管理** IPC 命令 `get_model_cache_info`（返回缓存目录大小 + 已安装模型列表）和 `clear_model_cache`（删除指定或全部模型文件）。进度推送采用 Tauri `app.emit()` 事件机制，与 `doc-status-changed` / `import-progress` 同模式。
- 验收标准：
  - AC-1：`init_embedder` 命令触发模型下载，推送 `model_download_progress` 事件含 `phase: "downloading"` + `file_name` + `current` + `total`（TC-VEC-008）。
  - AC-2：下载完成后推送 `phase: "loading"`（ONNX 会话构建），构建完成后推送 `phase: "done"`（TC-VEC-008 旁证）。
  - AC-3：下载失败时推送 `phase: "error"` + `message`，不 Panic（TC-VEC-008 旁证）。
  - AC-4：模型已缓存时 `init_embedder` 直接推送 `phase: "done"`，跳过下载（TC-VEC-008 旁证）。
  - AC-5：`get_model_cache_info` 返回缓存目录总大小（字节）+ 已安装模型列表（TC-VEC-009）。
  - AC-6：`clear_model_cache` 删除指定模型文件，返回删除的字节数（TC-VEC-009 旁证）。
- 契约：IPC `init_embedder(app, state) -> Result<(), String>` + `get_model_cache_info(state) -> Result<ModelCacheInfo, String>` + `clear_model_cache(model_name: Option<String>, state) -> Result<u64, String>` ｜ 事件 `model_download_progress` payload: `{phase, file_name?, current?, total?, message?}` ｜ 关联测试：TC-VEC-008、TC-VEC-009
- 调研依据：Tauri v2 `app.emit()` 事件机制（与 `doc-status-changed` / `import-progress` 同模式）；reqwest blocking response `.content_length()` 用于进度计算。本需求为跨层改动（infra + tauri-app + ui），无新 crate 依赖。

#### REQ-VEC-009 索引重建
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-VEC-003、REQ-ING-005
- 描述：用户可对已索引文档发起「重建索引」操作：删除该文档的全部 chunks 与向量，保留文档元数据与原始文件副本，重新执行解析 → 分块 → 嵌入 → 存储链路。适用于嵌入模型升级或分块策略变更后重新索引。重建期间文档状态标记为 Processing，重建完成后恢复 Indexed。
- 验收标准：
  - AC-1：点击「重建索引」后弹出确认对话框，提示「将删除现有索引并重新生成」。
  - AC-2：确认后文档状态变为 Processing，旧 chunks 与向量被清理。
  - AC-3：重建完成后状态恢复为 Indexed，新 chunks 数量与向量数一致。
  - AC-4：重建过程中可取消（复用 REQ-ING-006 取消机制），取消后保留已重建部分。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-VEC-010 知识库索引统计
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-DB-001
- 描述：设置面板或知识库面板顶部显示知识库统计概览：文档总数、总 chunk 数、总向量数、索引存储大小（向量 BLOB 字节数）、各状态文档分布（待索引 / 索引中 / 已索引 / 失败）。统计实时计算，不缓存。
- 验收标准：
  - AC-1：统计区显示文档总数、总 chunk 数、总向量数、存储大小。
  - AC-2：导入或删除文档后统计数字实时更新。
  - AC-3：存储大小以人类可读格式显示（KB / MB / GB）。
  - AC-4：各状态文档分布以徽标计数或进度条可视化展示。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-VEC-011 分块参数可视化配置
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-VEC-001
- 描述：设置面板提供分块参数配置区：分块大小（token 数，范围 128-2048，默认 256）、重叠大小（token 数，范围 0-256，默认 64）、大文件自动切换阈值（默认 1MB）。参数变更后新导入文档使用新参数；已索引文档不受影响（需手动重建索引，REQ-VEC-009）。配置值持久化到 settings 表。
- 验收标准：
  - AC-1：设置面板显示分块大小滑块（128-2048），默认值 256。
  - AC-2：修改分块参数后保存到 settings 表，重启后恢复。
  - AC-3：参数变更后新导入文档使用新参数分块，旧文档不受影响。
  - AC-4：参数范围校验：超出范围的值被拒绝并提示合法范围。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-VEC-012 嵌入模型运行时切换
- 优先级：P2 ｜ 状态：Implemented ｜ 依赖：REQ-VEC-002、REQ-VEC-009
- 描述：设置面板提供嵌入模型选择器（当前已实现 3 预设：all-MiniLM-L6-v2 / bge-small-zh-v1.5 / e5-small-v2），切换模型后提示用户「现有向量使用旧模型生成，建议重建索引以获得最佳检索效果」。切换不自动触发重建，由用户决定。
- 验收标准：
  - AC-1：设置面板显示嵌入模型下拉选择器，列出 3 个预设模型。
  - AC-2：切换模型后保存到 settings 表，后续新导入文档使用新模型嵌入。
  - AC-3：切换后弹出提示「现有索引使用旧模型，建议重建索引」并提供「重建全部」快捷入口。
  - AC-4：切换模型需重新下载模型文件（如未缓存），触发 model_download_progress 事件。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-VEC-013 领域画像自动分类
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-VEC-002、REQ-ING-001
- 描述：文档导入并完成向量化后，系统自动将其分类到 16 个预定义领域之一（technology/legal/medical/finance/science/education/business/engineering/literature/government/marketing/hr/design/data/security/general）。分类采用零 LLM 成本的嵌入质心方案——预计算每个领域的代表性关键词嵌入质心，文档取首 N 个 chunk 的嵌入均值与各领域质心做余弦相似度比较，取最高者。分类结果存储于 Document.domain 字段，前端文档列表显示领域标签徽章，支持按领域筛选。分类在后台异步执行，不阻塞导入完成事件。
- 验收标准：
  - AC-1：文档导入并完成向量化后，自动执行领域分类，Document.domain 字段被填充（TC-DOMAIN-001）。
  - AC-2：分类结果为 16 个预定义领域之一，非法值不被写入（TC-DOMAIN-002）。
  - AC-3：分类使用嵌入余弦相似度，不调用 LLM（零成本、离线可用）（TC-DOMAIN-003）。
  - AC-4：分类在后台异步执行，不阻塞导入完成事件（TC-DOMAIN-004）。
  - AC-5：嵌入失败时优雅降级，domain 设为 "general"，不中断导入流程（TC-DOMAIN-005）。
  - AC-6：前端文档列表显示领域标签徽章，点击徽章可按领域筛选文档（TC-DOMAIN-006）。
  - AC-7：`reclassify_document(doc_id)` 命令可手动重新分类指定文档（TC-DOMAIN-007）。
- 契约：`DomainClassifier` trait ｜ `EmbeddingDomainClassifier` 实现 ｜ `Document.domain: Option<String>` ｜ `Storage::update_document_domain` ｜ `reclassify_document` IPC 命令 ｜ 关联测试：TC-DOMAIN-001~007
- 调研依据：嵌入质心分类（Embedding Centroid Classification）——经典文本分类方法，利用预训练嵌入空间的语义聚类特性；零 LLM 成本、完全离线、推理快速（<10ms）。16 领域覆盖 SRS §5.1 全职业场景调研中的主要领域分类。

#### REQ-VEC-014 自定义 ONNX 嵌入模型上传（Pro）
- 优先级：P2 ｜ 状态：Implemented ｜ 依赖：REQ-VEC-002、REQ-VEC-012
- **v1.2 规划**：S52 ｜ 决策日期：2026-08-06
- 描述：Pro 用户可上传自己的 ONNX 嵌入模型（如 bge-large-zh-v1.5、e5-large-v2、领域专用微调模型），替代 3 个预设模型。上传文件包括 ONNX 模型文件（`model.onnx` 或 `model_quantized.onnx`）+ tokenizer 文件（`tokenizer.json` / `tokenizer_config.json` / `vocab.txt` 等）。后端通过 fastembed `UserDefinedEmbeddingModel` API 加载（已有基础设施），首次加载后检测嵌入维度。模型文件保存到 `{data_dir}/custom_models/{model_name}/`。Free 用户无法使用此功能（PRO_REQUIRED 错误）。
- 验收标准：
  - AC-1：上传有效 ONNX 模型后加载成功，维度检测正确（TC-VEC-CUSTOM-001 / TC-VEC-CUSTOM-UNIT-001~008）。
  - AC-2：切换到自定义模型后 `embed()` 使用自定义模型，维度匹配（TC-VEC-CUSTOM-002）。
  - AC-3：上传无效文件返回 `Err`，不崩溃（TC-VEC-CUSTOM-003）。
  - AC-4：删除自定义模型后文件删除，无法再切换（TC-VEC-CUSTOM-004）。
  - AC-5：Free 用户上传被拒绝，返回 `PRO_REQUIRED` 错误（TC-VEC-CUSTOM-005）。
  - AC-6：列出已上传的自定义模型列表（TC-VEC-CUSTOM-006）。
  - AC-7：路径遍历防护 — 模型名称中的 `/`、`\`、`..` 被替换为 `_`（TC-VEC-CUSTOM-007 / TC-VEC-CUSTOM-UNIT-007）。
- 契约：`EmbeddingModel::Custom(String)` 枚举变体 ｜ `LocalEmbedder::new_with_custom_model()` ｜ 3 个 Pro 门控 IPC 命令（`upload_custom_embedding_model` / `list_custom_models` / `delete_custom_model`）｜ 关联测试：TC-VEC-CUSTOM-001~005
- 调研依据：fastembed `UserDefinedEmbeddingModel` API（https://docs.rs/fastembed/）；参考 AnythingLLM / Open WebUI 自定义嵌入模型配置。

### 4.3 RAG — 检索增强对话

#### REQ-RAG-001 提问与流式回答
- 优先级：P0 ｜ 状态：Done ｜ 依赖：REQ-RAG-007、REQ-UI-002
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
- 优先级：P0 ｜ 状态：Implemented ｜ 依赖：REQ-RAG-001
- 描述：回答末尾附引用块，列出命中的来源文档与片段，可点击定位。
- 验收标准：
  - AC-1：每个引用含文档名、片段预览、chunk 序号；点击引用打开来源预览面板并高亮对应片段。
  - AC-2：引用列表与实际注入上下文的 chunk 完全一致，不多不少。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-RAG-003 检索策略与拒答
- 优先级：P0 ｜ 状态：Done ｜ 依赖：REQ-VEC-003
- 描述：VectorRetriever 接入 LocalEmbedder 与 SqliteStorage，默认 top-k（k=5）向量相似度检索；引入相关度分数阈值过滤（score < 0.35 直接丢弃），防止低质量上下文污染；过滤后上下文为空时，直接返回固定提示，**不调用 LLM**。
- 验收标准：
  - AC-1：低于阈值的检索结果全部被丢弃（TC-RAG-001）；相似度不足时回答明确告知「知识库中未找到相关内容」，不编造来源。
  - AC-2：空上下文时不发起任何 LLM 网络请求，直接返回固定警告流（TC-RAG-002）。
- 契约：Retriever 端口契约不变；编排器 `core::chat::ChatEngine`、检索适配器 `core::retriever::VectorRetriever`。
- 关联测试：TC-RAG-001、TC-RAG-002

> TPM 注（v1.0.4）：上游指令曾以「REQ-RAG-001」指代检索引擎；检索策略归口本条（REQ-RAG-003），REQ-RAG-001（流式回答）承载对话命令契约，避免编号冲突。

#### REQ-RAG-004 多轮上下文管理
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-RAG-001
- 描述：同一会话内携带对话历史；提供「清空上下文」操作。
- 验收标准：
  - AC-1：后续提问能引用前文信息（如代词指代）。
  - AC-2：清空后新提问不再携带任何历史消息。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-RAG-005 停止生成
- 优先级：P0 ｜ 状态：Done ｜ 依赖：REQ-RAG-001
- 描述：流式生成过程中用户可一键停止；后端为每个 conversation_id 维护一个 CancellationToken，abort_chat 触发后流循环立即 break，丢弃流即中止 HTTP 连接与事件推送；已生成部分内容保留并标记「已中断」，且照常落库（REQ-RAG-006）。
- 验收标准：
  - AC-1：abort 后事件流立即终止（TC-RAG-005），无 Panic，部分内容保留。
  - AC-2：停止后输入框恢复「空闲」态，可立即再次提问。
- 契约（TPM 冻结，2026-07-24）：Command `abort_chat(conversation_id: String, state) -> Result<(), String>`；中断事件语义复用已冻结契约：`chat_error("⏹ 生成已中断")` + `chat_done`（不新增事件类型）。
- 关联测试：TC-RAG-005

#### REQ-RAG-006 会话持久化与多轮记忆
- 优先级：P0 ｜ 状态：Done ｜ 依赖：REQ-RAG-001、REQ-DB-001
- 描述：新增 `conversations` 表（id, workspace_id, title, created_at）与 `messages` 表（id, conversation_id, role, content, sources，外键级联）；新建对话生成唯一 ID（占位标题「新会话」）；AI 回答完成（含被中断）后将用户问题与 AI 回复存入 messages；首轮问答后自动提取用户问题前 24 字符作为会话标题；前端左侧栏展示历史会话，点击加载历史消息并恢复多轮上下文；重启后完整可读可续聊。
- 验收标准：
  - AC-1：会话 CRUD 状态正确（TC-RAG-003：新建 → 列表 → 删除级联清理消息）。
  - AC-2：完成一次对话后 messages 表新增 user 与 assistant 两条记录，标题自动提取（TC-RAG-004）。
  - AC-3：重启应用后历史会话完整可读（含引用块 sources），续聊上下文包含该会话历史。
- 契约（TPM 冻结，2026-07-24）：Command `create_conversation(workspace_id: String) -> Result<String, String>`；`get_conversations(workspace_id: String) -> Result<Vec<Conversation>, String>`；`get_messages(conversation_id: String) -> Result<Vec<ChatMessage>, String>`；`delete_conversation(id: String) -> Result<(), String>`。chat 命令复用既有 `conversation_id` 参数（Phase 4 契约已含）。
- 关联测试：TC-RAG-003、TC-RAG-004

#### REQ-RAG-007 BYOK Provider 管理
- 优先级：P0 ｜ 状态：Implemented ｜ 依赖：REQ-SEC-001
- 描述：支持配置 OpenAI 兼容端点（含 DeepSeek 等）与 Anthropic 两类 Provider；每类可配置 base_url、model、API Key，并提供「连接测试」。
- 验收标准：
  - AC-1：保存配置前可执行连接测试，成功 / 失败均有明确反馈（失败不泄露 Key）。
  - AC-2：切换 Provider 后新会话立即生效，无需重启。
  - AC-3：未配置任何 Provider 时，提问入口给出引导而非报错。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-RAG-008 回答语言自适应
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-RAG-001、REQ-VIZ-004
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
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-RAG-003、REQ-VEC-001
- 描述：当前 `VectorRetriever` 检索 top-k chunk 后直接注入 LLM prompt，每个 chunk 是孤立片段。对于学术论文等深度文档，用户提问「检查 3.2 节实验设计的逻辑是否自洽」时，命中的 chunk 可能只是实验设计的一个段落，缺少前后文（如假设前提、实验参数定义），导致 LLM 无法做出完整判断。本需求在检索结果返回后、注入 prompt 前，增加一个**段落扩展层**：对每个命中 chunk，根据其 `sequence` 字段向前后各扩展 1 个相邻 chunk（同一文档内），合并为完整段落后注入 prompt。扩展仅影响注入 LLM 的上下文，不改变 `chat_sources` 事件推送的引用列表（引用列表仍只展示命中的核心 chunk，避免引用膨胀）。
- 验收标准：
  - AC-1：检索命中 chunk A（sequence=5）时，注入 LLM prompt 的上下文包含同一文档的 sequence=4 和 sequence=6 的 chunk 内容（TC-RAG-007）。
  - AC-2：命中 chunk 为文档首个 chunk（sequence=0）时，仅向后扩展，不产生越界（TC-RAG-007 旁证）。
  - AC-3：命中 chunk 为文档末尾 chunk 时，仅向前扩展，不产生越界（TC-RAG-007 旁证）。
  - AC-4：`chat_sources` 事件推送的引用列表只包含原始命中的 chunk，不包含扩展 chunk（TC-RAG-007 旁证）。
  - AC-5：扩展 chunk 与命中 chunk 来自不同文档时不合并（跨文档不扩展，TC-RAG-007 旁证）。
- 契约：§5.6 `VectorRetriever::expand_neighbors` ｜ 关联测试：TC-RAG-009
- 调研依据：LlamaIndex `SentenceWindowRetriever`（检索后扩展窗口）；LangChain `ParentDocumentRetriever`（子 chunk 检索→父文档返回）；Anthropic Contextual Retrieval 博文建议「检索后提供更完整的上下文窗口」。本需求为纯 `crates/core` 层逻辑改进，不引入新依赖，不改变 `Retriever` 端口签名。

#### REQ-RAG-010 混合检索（向量 + 关键词 RRF 融合）
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-RAG-003、REQ-VEC-003
- 描述：纯向量检索在精确匹配场景（API 名称、代码片段、专有名词）上召回不足。本需求在 `Retriever` 端口下新增 `HybridRetriever`，并行执行向量检索（语义匹配）与关键词检索（FTS5 BM25），通过 Reciprocal Rank Fusion (RRF) 算法融合两通道结果，重叠 chunk 获得更高排名。关键词检索使用 SQLite FTS5 `trigram` 分词器（支持中英文子串匹配），短查询（<3 字符）回退 SQL LIKE。混合检索默认启用，用户可通过 `set_hybrid_search` IPC 命令关闭（退化为纯向量检索）。
- 验收标准：
  - AC-1：向量检索与关键词检索的结果通过 RRF 融合，重叠 chunk 排名高于非重叠 chunk（TC-HYBRID-001）。
  - AC-2：空知识库时返回空结果，不返回 Err（TC-HYBRID-002）。
  - AC-3：向量通道未命中但关键词通道命中时，返回关键词检索结果（TC-HYBRID-003）。
  - AC-4：score < 0.35 的向量结果被阈值过滤，不参与 RRF 融合（TC-HYBRID-004）。
  - AC-5：混合检索结果同样进行 Chunk Expansion，扩展相邻 chunk（TC-HYBRID-005）。
- 契约：§5.7 `HybridRetriever` ｜ §5.6 `expand_neighbors`（共享函数） ｜ `Storage::keyword_search` ｜ 关联测试：TC-RRF-001~014、TC-HYBRID-001~005、TC-KEYWORD-001~006
- 调研依据：Cormack et al. 2009 RRF 论文（k=60 标准参数）；LangChain `EnsembleRetriever`（向量+BM25 RRF 融合）；rs-pro `GraphHybridRetriever`（已验证 RRF 实现）。FTS5 `trigram` 分词器（SQLite 3.34+）支持中英文子串匹配。

#### REQ-RAG-011 会话搜索
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-RAG-006
- 描述：左侧栏会话列表上方提供搜索框，支持按会话标题模糊搜索，实时过滤会话列表。搜索不区分大小写，支持中英文混合搜索。
- 验收标准：
  - AC-1：在搜索框输入关键词后，会话列表仅显示标题包含该关键词的会话。
  - AC-2：清空搜索框后恢复完整会话列表。
  - AC-3：无匹配会话时显示「未找到匹配会话」空状态。
  - AC-4：搜索在会话列表超过 20 条时自动显示搜索框，少于 20 条时可隐藏。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-RAG-012 消息复制与操作
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-UI-002
- **v1.0 发布分类**：v1.0 力争交付 ｜ 决策日期：2026-08-01
- 描述：每条消息 Block（user / assistant）悬停时显示操作按钮组：复制全文（Markdown 原文）、复制纯文本（去除 Markdown 语法）。Assistant 消息额外提供：重新生成（以相同问题重新发起 RAG 查询，生成新回答追加到对话末尾）。
- 验收标准：
  - AC-1：鼠标悬停消息 Block 时，操作按钮组淡入显示。
  - AC-2：点击「复制全文」将 Markdown 原文复制到系统剪贴板，显示「已复制」toast。
  - AC-3：点击「复制纯文本」将去除 Markdown 语法的纯文本复制到剪贴板。
  - AC-4：Assistant 消息的「重新生成」按钮以相同问题重新发起查询，新回答追加到对话末尾，不覆盖原回答。
  - AC-5：流式生成中操作按钮组不显示（避免干扰）。
- 契约：`showMsgActions(blockEl, rawMarkdown, role)` in `ui/src/chat.js` + `.msg-actions` CSS hover ｜ 关联测试：`e2e-tests/tests/msg-actions.spec.ts` E2E-RAG-012-AC1~AC5

#### REQ-RAG-013 单条消息删除
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-RAG-006
- 描述：用户可删除对话中的单条消息（user 或 assistant）。删除 user 消息时，对应的 assistant 回答一并删除（保持问答对完整性）。删除操作需确认。删除后从 `messages` 表移除，对话区即时更新。
- 验收标准：
  - AC-1：消息 Block 右键菜单或操作按钮提供「删除」选项。
  - AC-2：删除 user 消息时弹出确认，提示「将同时删除对应的 AI 回答」，确认后两条消息均删除。
  - AC-3：删除 assistant 消息时仅删除该条，不影响 user 消息。
  - AC-4：删除后 `messages` 表对应记录移除，对话区即时更新。
  - AC-5：删除最后一条问答后，若会话无剩余消息，会话标题保持不变。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-RAG-014 检索参数可配置
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-RAG-003
- 描述：设置面板提供 RAG 检索参数配置区：top-k 值（范围 1-20，默认 5）、相似度阈值（范围 0.0-1.0，默认 0.35）、Chunk Expansion 开关（默认开启）、扩展窗口大小（范围 0-3，默认 1）。参数变更后对新发起的查询生效，不影响进行中的对话。参数持久化到 settings 表。
- 验收标准：
  - AC-1：设置面板显示 top-k 滑块（1-20），默认值 5。
  - AC-2：设置面板显示相似度阈值滑块（0.0-1.0），默认值 0.35。
  - AC-3：修改参数后保存到 settings 表，重启后恢复。
  - AC-4：参数变更后新查询使用新参数，进行中的对话不受影响。
  - AC-5：参数范围校验：超出范围的值被拒绝。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-RAG-015 LLM 生成参数可配置
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-LLM-001
- 描述：设置面板提供 LLM 生成参数配置区：temperature（范围 0.0-2.0，默认 0.7）、max_tokens（范围 256-8192，默认 4096）、top_p（范围 0.0-1.0，默认 1.0）。参数经 OpenAI 兼容 API 传递。参数持久化到 settings 表，运行时生效（新查询使用新参数）。
- 验收标准：
  - AC-1：设置面板显示 temperature 滑块（0.0-2.0），默认 0.7。
  - AC-2：设置面板显示 max_tokens 输入框（256-8192），默认 4096。
  - AC-3：修改参数后保存到 settings 表，重启后恢复。
  - AC-4：新查询使用新参数，进行中的对话不受影响。
  - AC-5：参数范围校验：超出范围的值被拒绝并提示合法范围。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-RAG-016 引用块增强交互
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-RAG-002
- **v1.0 发布分类**：v1.0 力争交付 ｜ 决策日期：2026-08-01
- 描述：引用块（chat_sources）增强为可交互卡片：每条引用显示文档名、chunk 序号、相似度分数（百分比）、片段预览（前 100 字）。点击引用卡片打开文档预览面板（REQ-ING-010）并高亮对应 chunk。悬停引用卡片时对话区对应文本高亮（如 LLM 回答中引用了 [1] 标记，悬停引用 [1] 时高亮回答中的 [1]）。
- 验收标准：
  - AC-1：引用卡片显示文档名、chunk 序号、相似度分数（如 87%）、片段预览。
  - AC-2：点击引用卡片打开文档预览面板，自动定位到对应 chunk 并高亮。
  - AC-3：悬停引用卡片时，对话区对应引用标记高亮。
  - AC-4：引用卡片按相似度分数降序排列。
  - AC-5：无引用时（空上下文拒答）不显示引用区域。
- 契约：`renderSources(sources)` 排序后渲染 `.source-card` + `enhanceCitations(mdEl, sources)` 双向高亮 + `.cite-ref-highlighted` CSS ｜ 关联测试：E2E-SB

#### REQ-RAG-017 对话上下文长度管理
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-RAG-004
- **v1.0 发布分类**：v1.0 力争交付 ｜ 决策日期：2026-08-01
- 描述：多轮对话中，历史消息累积可能导致 LLM 上下文超限。系统自动管理上下文窗口：当历史消息 token 数超过阈值（默认 4096 tokens，可配置）时，采用「保留首轮 + 最近 N 轮」策略截断中间消息。截断时在对话区显示「部分历史已折叠」提示，用户可点击展开查看完整历史（但发送给 LLM 的是截断后的上下文）。
- 验收标准：
  - AC-1：历史消息 token 数超过阈值时自动截断中间消息，保留首轮与最近 N 轮。
  - AC-2：截断后对话区显示「部分历史已折叠」提示。
  - AC-3：点击提示可展开查看完整历史消息（仅前端展示，不影响发送给 LLM 的上下文）。
  - AC-4：截断阈值可在设置中配置（范围 2048-32768 tokens）。
  - AC-5：截断后发送给 LLM 的 messages 数组不含被截断的中间消息。
- 契约：`truncate_history()` in `crates/core/src/chat.rs` + `set_context_token_limit` IPC + `showCollapsedHint()` in `ui/src/chat.js` + `#contextTokenSlider` in `ui/src/settings.js` ｜ 关联测试：`integration.rs` TC-RAG-017-001~006

#### REQ-RAG-018 用户提问建议（空知识库与首次对话引导）
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-UI-005
- 描述：首次进入对话界面（空对话）时，在输入框上方显示 3-4 个建议提问卡片（如「这个知识库包含哪些主题？」「总结最重要的 3 个要点」），点击卡片自动填入输入框并发送。建议提问基于知识库中文档名动态生成（如知识库中有「实验设计.md」，建议提问「实验设计的核心方法是什么？」）。空知识库时不显示建议提问，仅显示导入引导。
- 验收标准：
  - AC-1：空对话界面显示 3-4 个建议提问卡片。
  - AC-2：点击建议卡片自动填入输入框并发送。
  - AC-3：建议提问基于知识库文档名动态生成，非固定文案。
  - AC-4：空知识库时不显示建议提问，仅显示导入引导。
  - AC-5：首次提问后建议卡片消失。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-RAG-019 消息列表懒加载与分页
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-RAG-006、REQ-DB-001
- 描述：加载历史会话时仅拉取最近 N 条消息（默认 30 条），而非全量加载。当会话消息总数超过已加载数量时，聊天区顶部显示「加载更早的 X 条消息」按钮，点击后分页拉取更早的消息并插入顶部，保持滚动位置不跳动。后端新增 `get_messages_paginated` IPC 命令，支持 `limit` / `offset` 参数（从最新消息向前偏移），返回 `PaginatedResult<ChatMessage>`（含 `items` 与 `total`）。Storage trait 新增 `list_messages_paginated` 与 `count_messages` 方法，SqliteStorage 使用 `ORDER BY rowid DESC LIMIT OFFSET` + 反转实现。此需求解决长会话（100+ 轮对话 = 200+ 条消息）全量加载导致的渲染阻塞问题。
- 验收标准：
  - AC-1：`get_messages_paginated(conversationId, limit=30, offset=0)` 返回最近 30 条消息（正序）与消息总数。
  - AC-2：会话消息超过 30 条时，聊天区顶部显示「加载更早的 X 条消息」按钮。
  - AC-3：点击「加载更多」后，更早的消息插入到顶部，滚动位置保持不变（不跳到底部）。
  - AC-4：`count_messages` 返回正确的消息总数（含未加载部分）。
  - AC-5：新建会话时消息分页状态重置（offset=0, total=0）。
  - AC-6：`list_messages_paginated` 返回的消息按写入顺序正序排列（最旧在前），即使 SQL 使用 DESC 查询。
- 契约：`get_messages_paginated(conversation_id: String, limit: Option<usize>, offset: Option<usize>) -> Result<PaginatedResult<ChatMessage>, String>`；Storage trait `list_messages_paginated` + `count_messages` ｜ 关联测试：TC-RAG-007

#### REQ-RAG-020 Cross-Encoder 重排序
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-RAG-010
- 描述：向量检索 + 关键词 RRF 融合后的候选结果按排名粗排，精确度有限。本需求在 RRF 融合后、Chunk Expansion 前插入 Cross-Encoder 精排阶段：对 top-N（N = top_k * 5）候选使用 bge-reranker-base 模型逐对 query-document 打分，按精确相关性重排，取 top-k 后再扩展相邻 chunk。Cross-Encoder 与双塔 embedding 模型不同——它将 query + document 拼接后送入 BERT，注意力机制可捕获细粒度语义交互，排序精度显著高于余弦相似度。重排序默认关闭（用户主动开启），通过 `set_rerank_enabled` IPC 命令切换，设置持久化至 `rag.rerank_enabled`。首次启用时下载 bge-reranker-base ONNX 模型（~280MB），后续从本地缓存加载。
- 验收标准：
  - AC-1：注入 reranker 后，检索结果按 Cross-Encoder 分数重排，而非原始 RRF 顺序（TC-RERANK-003）。
  - AC-2：reranker 返回全部候选后，仅取 top_k 注入 LLM prompt（TC-RERANK-004）。
  - AC-3：未注入 reranker 时行为与之前完全一致（向后兼容，TC-RERANK-005）。
  - AC-4：注入 reranker 但关闭混合检索时仍能正常工作（TC-RERANK-006）。
  - AC-5：空知识库时 reranker 不被调用（TC-RERANK-007）。
  - AC-6：reranker 分数替换原始 RRF 分数（TC-RERANK-008）。
  - AC-7：`set_rerank_enabled` 持久化至 settings 表，重启后恢复（TC-RAG-020）。
- 契约：`Reranker` trait（对象安全，`Pin<Box<Future>>`）｜ `NoReranker` 占位类型 ｜ `HybridRetriever::set_reranker(Option<Arc<dyn Reranker>>)` ｜ `set_rerank_enabled(enabled: bool) -> Result<(), String>` ｜ 关联测试：TC-RERANK-001~009、TC-RAG-020
- 调研依据：Anthropic Contextual Retrieval 博文（2024）——重排序可将检索失败率降低 49%；BAAI/bge-reranker-base——中英文双语 Cross-Encoder；fastembed `TextRerank`——纯 Rust ONNX 推理。

#### REQ-RAG-021 HyDE 查询改写
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-RAG-010
- 描述：用户查询通常是简短的自然语言问题，与知识库中的答案文档在嵌入空间中距离较远，导致向量检索召回率低。HyDE（Hypothetical Document Embeddings，论文 arXiv:2212.10496）通过让 LLM 先生成一段假设性答案文档，再用该文档的嵌入替代原始查询嵌入进行向量检索。因为假设性答案在语义上更接近知识库中的实际答案片段，检索质量显著提升。改写后的文本**仅用于向量检索**，关键词检索（FTS5 BM25）仍使用原始查询——因为关键词检索擅长精确匹配，改写后的文本可能丢失原始查询中的精确术语。HyDE 默认关闭（用户主动开启），通过 `set_hyde_enabled` IPC 命令切换，设置持久化至 `rag.hyde_enabled`。HyDE 无额外模型下载——复用用户已配置的 LLM 端点（BYOK），与 chat 命令使用相同的 `api_key` / `base_url` / `model`。LLM 调用使用非流式完成（`OpenAIProvider::complete`），超时 15 秒。如果 LLM 调用失败（网络错误、超时、HTTP 非 2xx、空响应），HyDE 优雅降级为原始查询，不中断检索管线。
- 验收标准：
  - AC-1：注入 rewriter 后，向量检索使用改写后的文本嵌入而非原始查询（TC-HYDE-002）。
  - AC-2：改写失败（LLM 错误、超时、空响应）时不中断检索，使用原始查询嵌入（TC-HYDE-003/004）。
  - AC-3：未注入 rewriter 时行为与之前完全一致（向后兼容，TC-HYDE-005）。
  - AC-4：注入 rewriter 但关闭混合检索时仍能正常工作（TC-HYDE-006）。
  - AC-5：HyDE + 重排序可同时启用，三阶段链路不冲突（TC-HYDE-007）。
  - AC-6：空知识库时 rewriter 被调用但不影响结果为空（TC-HYDE-008）。
  - AC-7：`set_hyde_enabled` 持久化至 settings 表，重启后恢复（TC-RAG-021）。
  - AC-8：NoRewriter 占位实现原样返回查询（TC-HYDE-001）。
- 契约：`QueryRewriter` trait（对象安全，`Pin<Box<Future>>`）｜ `NoRewriter` 占位类型 ｜ `HybridRetriever::set_rewriter(Option<Arc<dyn QueryRewriter>>)` ｜ `HydeRewriter`（infra 层，OpenAIProvider::complete 非流式完成）｜ `set_hyde_enabled(enabled: bool) -> Result<(), String>` ｜ 关联测试：TC-HYDE-001~009、TC-RAG-021
- 调研依据：论文 arXiv:2212.10496 "Precise Zero-Shot Dense Retrieval without Relevance Labels"——HyDE 可显著提升零样本密集检索的召回率；LangChain `HyDE` 实现——使用 LLM 生成假设性文档进行向量检索。

#### REQ-RAG-022 Agentic RAG 多步推理
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-RAG-001、REQ-RAG-003
- 描述：标准 RAG 检索→生成为单次管线，无法处理需要多步检索、信息综合、子问题分解的复杂查询。本需求新增 Agentic RAG 模式（ReAct 范式，论文 arXiv:2210.03629）：LLM 交替生成推理轨迹（Thought）和工具调用动作（Action），动作执行后返回观察结果（Observation），循环直至 LLM 认为信息充分并输出最终答案。可用工具包括 `search_kb`（知识库检索）和 `list_documents`（列出全部文档）。最大迭代次数 5 次（防止无限循环），超出后强制使用已有信息生成最终答案。中间推理步骤通过 `agent_step` 事件推送前端，展示思考过程。最终答案以 `chat_token` 流式输出。Agentic 模式默认关闭，通过 `set_agent_enabled` IPC 命令切换，设置持久化至 `rag.agent_enabled`。Agentic 模式与标准 RAG 共存——用户可在设置中切换，不影响标准对话流程。
- 验收标准：
  - AC-1：启用 Agentic 模式后，复杂查询触发多步检索（Thought→Action→Observation 循环），而非单次检索（TC-AGENT-001）。
  - AC-2：中间推理步骤通过 `agent_step` 事件推送前端，含 `thought`/`action`/`observation`/`answer` 类型（TC-AGENT-002）。
  - AC-3：最终答案以 `chat_token` 流式输出，与标准 RAG 一致（TC-AGENT-003）。
  - AC-4：最大迭代次数 5 次，超出后强制使用已有信息生成最终答案（TC-AGENT-004）。
  - AC-5：LLM 响应解析失败（非预期格式）时优雅降级为标准 RAG 单次检索（TC-AGENT-005）。
  - AC-6：Agentic 模式默认关闭，`set_agent_enabled` 持久化至 settings 表，重启后恢复（TC-AGENT-006）。
  - AC-7：引用来源（chat_sources）聚合全部检索步骤的命中结果（TC-AGENT-007）。
  - AC-8：支持取消（CancellationToken），取消后保留已生成内容（TC-AGENT-008）。
- 契约：`AgentEngine` 结构体 ｜ `AgentStep` 事件负载 ｜ `agent_step` Tauri 事件 ｜ `set_agent_enabled(enabled: bool) -> Result<(), String>` IPC 命令 ｜ `chat_agent` IPC 命令 ｜ 关联测试：TC-AGENT-001~008
- 调研依据：论文 arXiv:2210.03629 "ReAct: Synergizing Reasoning and Acting in Language Models"——LLM 交替生成推理轨迹与任务动作，通过外部工具获取信息，克服纯链式推理的幻觉和错误传播；LangGraph / CrewAI Agentic RAG 实现——多步推理+工具调用框架。

#### REQ-RAG-025 多代理协调模式（Coordinator Mode）
- 优先级：P2 ｜ 状态：Done ｜ 依赖：REQ-RAG-001、REQ-RAG-003、REQ-RAG-022
- 描述：借鉴 Claude Code 的 Coordinator Mode，将复杂查询分解为子查询，并行检索后综合分析，再流式生成最终答案。四阶段流水线：(1) Research——通过 LLM 将用户查询分解为 1-3 个子查询，每个子查询并行调用 `retriever.retrieve()`；(2) Synthesis——将所有研究发现拼接为文本，通过 LLM 生成综合分析报告；(3) Implementation——基于综合分析构建分段式 RAG prompt（利用 prompt caching），流式生成最终答案；(4) Verification——验证答案引用了所有相关来源（按 chunk.id 去重）。适用于「对比这三份合同的法律风险」等需要多维度检索的复杂研究查询。协调模式默认关闭，通过 `set_coordinator_mode` IPC 命令切换，设置持久化至 `rag.coordinator_enabled`。协调模式与标准 RAG 和 Agentic RAG 共存。
- 验收标准：
  - AC-1：启用协调模式后，复杂查询触发子查询分解 + 并行检索（TC-COORD-001/002）。
  - AC-2：中间阶段进度通过 `coordinator_phase` 事件推送前端（researching/synthesizing/generating）（TC-COORD-007）。
  - AC-3：综合分析阶段正确合并多源信息，综合摘要注入最终 RAG prompt（TC-COORD-003）。
  - AC-4：最终答案流包含所有来源引用，来源按 chunk.id 去重（TC-COORD-004）。
  - AC-5：子查询分解失败时降级为原始查询做单次检索（TC-COORD-005）。
  - AC-6：空知识库时返回 NoContext，不执行后续阶段（TC-COORD-006）。
  - AC-7：协调模式默认关闭，`set_coordinator_mode` 持久化至 settings 表（TC-COORD-001 集成测试）。
- 契约：`CoordinatorEngine` 结构体 ｜ `CoordinatorPhaseInfo` 事件信息 ｜ `coordinator_phase` Tauri 事件 ｜ `set_coordinator_mode(enabled: bool) -> Result<(), String>` IPC 命令 ｜ 关联测试：TC-COORD-001~007
- 调研依据：Claude Code Coordinator Mode——四阶段协调流程（Research→Synthesis→Implementation→Verification）；Anthropic "Building effective agents"——多代理协调模式最佳实践。
- 扩展（子代理舰队）：借鉴 Bamboo-agent 子代理 + mailbox 通信，新增 `SubAgent` / `SubAgentFleet` 模块（`crates/core/src/sub_agent.rs`）。启用后（`rag.sub_agent_enabled` = true），Research 阶段每个子查询由独立 SubAgent 处理（检索 + LLM 生成简要回答），通过 tokio mpsc mailbox 消息传递与主代理协调。子代理共享 `&R` / `&L` 不可变引用（无需 Clone/Arc），支持超时（`TimedOut`）和容错（`Failed` 不影响其他子代理）。`CoordinatorEngine::with_sub_agent_timeout(secs)` builder 方法启用。默认关闭，仅在 `rag.coordinator_enabled` 为 true 时生效。关联测试：TC-SUBAGENT-001~008。

#### REQ-RAG-026 消息编辑与版本分页
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-RAG-001、REQ-RAG-006
- 描述：用户可修改对话中已发出的问题（user 消息），修改后**就地**重新生成问答，不向列表追加新消息；同一问题的多个编辑版本通过**分页器**（`◀ 1/N ▶`）切换查看，分页器渲染在 assistant 答案消息下方（对齐 chat.deepseek.com 版本切换交互，不显示在用户气泡操作栏）。首次编辑时，原始问答原地升级为 version=1，编辑后内容成为 version=2，新答案流式生成期间不显示分页器，流完成后分页器出现（1/2）；后续每次编辑生成 max+1 新版本，旧版本始终保留、不覆盖、不删除、不设版本数量上限。切换分页器时问题文本与答案区（Markdown/来源/思考过程）就地替换为对应版本内容（思考面板先清空旧版本推理再加载目标版本）。活跃版本号与各版本问答内容持久化至 DB（`turn_active_versions` 表 + `messages` 表 `turn_group`/`version` 列），重启后恢复最后一次查看的版本与分页器状态，不产生重复渲染。
- 验收标准：
  - AC-1：点击问题文本或编辑按钮进入编辑模式，编辑交互与聊天输入框一致（Enter 发送、Shift+Enter 换行、Esc 取消、高度自适应）（TC-EDIT-DB-001）。
  - AC-2：编辑发送后不新增消息块，问题与答案就地替换为新的问答对（TC-EDIT-DB-001）。
  - AC-3：首次编辑（原始消息无 turn_group）时，原始问答升级为 version=1、新内容为 version=2，新答案流完成后方显示分页器 1/2（TC-EDIT-DB-004）。
  - AC-4：分页器渲染在 assistant 消息下方（.msg-actions 之前），切换时问题、答案、来源与思考过程就地替换为对应版本（TC-EDIT-DB-004、前端单元测试）。
  - AC-5：活跃版本号持久化（`turn_active_versions`），编辑后默认 active=最新版本；重启后恢复最后查看版本且无双问题气泡（TC-EDIT-DB-004）。
  - AC-6：从旧版本（如 v1）再次编辑，生成新版本（max+1），不覆盖旧版本；版本不设上限、不提供删除入口（TC-EDIT-DB-001）。
  - AC-7：无效的原始消息 id 时编辑失败且不产生部分数据（事务回滚）（TC-EDIT-DB-005）。
  - AC-8：多问答对场景下首次编辑仅升级目标问答对，不误升级相邻问答；promote 幂等可重试（TC-EDIT-DB-006）。
- 契约：`edit_user_message(conversation_id, turn_group, new_content, original_message_id: Option<String>) -> i32` IPC 命令 ｜ `Storage::promote_original_turn` trait 方法 ｜ `ChatMessage.id` 字段 ｜ `turn_active_versions` 表 ｜ 关联测试：TC-EDIT-DB-001~006
- 实现要点：首次编辑由前端在 `triggerEdit` 中生成新 turn_group 并传原始消息行 id（`get_messages` 返回的 `ChatMessage.id`）；后端 `promote_original_turn` 在事务中把原始 user 行及其后紧邻 assistant 行升级为 v1（幂等，assistant 定位限定同一问答对）；新内容 INSERT 为 v2；`edit_user_message_inner_full` 同时 UPSERT `turn_active_versions`。参考 chat.deepseek.com 消息编辑版本切换交互。
- 兼容性说明：本需求升级逻辑仅对「首次编辑」的新操作生效；改造前已编辑产生的旧数据（原始行未升级为 v1）不自动迁移，重载历史会话时可能出现同一问题重复渲染，属既有数据遗留问题（旧版行为），后续可提供一次性 DB 迁移脚本解决。

#### REQ-RAG-027 知识图谱实体关系检索
- 优先级：P2 ｜ 状态：Implemented ｜ 依赖：REQ-PERF-006（EntityExtractor）、REQ-VEC-003（Storage）
- 描述：借鉴 mnemo 知识图谱 RAG 架构，在现有实体抽取基础上新增实体间关系（知识图谱边）。导入文档时，`EntityExtractor::extract_relations()` 以纯规则模式匹配（零 LLM）抽取中英文 8 种关系类型（`defined_as` / `part_of` / `depends_on` / `uses` / `implements` / `extends` / `references` / `related_to`），写入 `entity_relations` 表。检索时可先匹配实体节点，再沿图边扩展到关联 chunk，显著提升关联查询的召回率。关系抽取典型耗时 < 2ms/chunk。
- 验收标准：
  - AC-1：英文 "X is defined as Y" 模式正确识别为 `defined_as` 关系，置信度 1.0（TC-RELATION-001）。
  - AC-2：中文 "X 依赖 Y" 模式正确识别为 `depends_on` 关系，置信度 1.0（TC-RELATION-002）。
  - AC-3：无关系的文本（单个实体或空文本）返回空 Vec（TC-RELATION-003）。
  - AC-4：一段包含多个关系的文本全部识别，关系类型 ≥ 2 种（TC-RELATION-004）。
  - AC-5：精确匹配置信度 1.0，模糊匹配置信度 0.7，同句共现兜底置信度 0.5（TC-RELATION-005）。
  - AC-6：同一 chunk 内相同三元组 `(subject, relation_type, object)` 只保留一个（TC-RELATION-006）。
  - AC-7：导入文档时在 `index_entities()` 后自动调用 `index_relations()`，关系写入 `entity_relations` 表。
  - AC-8：`entity_relations` 表通过 `ON DELETE CASCADE` 与 chunks 表关联，删除 chunk 自动清理关系。
  - AC-9：单跳图扩展——查询包含实体时，沿图边扩展到包含该实体关系的 chunk（TC-GRAPH-001）。
  - AC-10：多跳图扩展——BFS 遍历最大 2 跳，间接关联的 chunk 分数低于直接关联（距离衰减因子 0.5）（TC-GRAPH-002）。
  - AC-11：关系类型过滤——仅沿指定关系类型扩展，过滤无关关系（TC-GRAPH-003）。
  - AC-12：置信度加权排序——高置信度关系的 chunk 排在低置信度前面（TC-GRAPH-004）。
  - AC-13：空图降级——无关系时返回空 Vec，不报错，管线降级为三路 RRF（TC-GRAPH-005）。
  - AC-14：四路 RRF 融合——图扩展结果作为第四路检索通道与 Vector/BM25/Entity 融合（TC-GRAPH-006）。
  - AC-15：侧栏「知识图谱」按钮存在且可点击，点击后弹出全屏图谱 overlay 面板（TC-GRAPH-VIZ-001/002）。
  - AC-16：面板内渲染 D3 force-directed graph SVG，包含节点（圆形 + 标签）和边（曲线连接）（TC-GRAPH-VIZ-003）。
  - AC-17：节点数量与 `get_graph_data` 返回的去重实体数一致（TC-GRAPH-VIZ-004）。
  - AC-18：不同关系类型显示不同颜色边，图例展示关系类型颜色映射（TC-GRAPH-VIZ-005）。
  - AC-19：点击节点高亮该节点的所有关联边和关联节点，未关联元素暗淡（TC-GRAPH-VIZ-006）。
  - AC-20：拖拽节点后节点位置更新，滚轮缩放 + 画布平移可用（TC-GRAPH-VIZ-007）。
  - AC-21：空知识库时面板显示「暂无图谱数据」提示，SVG 隐藏（TC-GRAPH-VIZ-008）。
  - AC-22：ESC 键关闭图谱面板，Focus Trap 在面板打开时激活（TC-GRAPH-VIZ-009）。
  - AC-23：统计栏显示去重实体数和关系总数（TC-GRAPH-VIZ-010）。
  - AC-24：每个节点显示对应的实体类型图标（person/proper_noun/tech_term/identifier/date 五种内联 SVG 图标），不同类型显示不同图标（TC-GRAPH-VIZ-011）。
  - AC-25：图例区域显示关系类型过滤面板，包含每个关系类型的复选框，可勾选/取消（TC-GRAPH-VIZ-012）。
  - AC-26：取消勾选关系类型后，该类型的边和仅该类型连接的节点隐藏，可见边数量减少（TC-GRAPH-VIZ-013）。
  - AC-27：工具栏下方显示搜索输入框，可输入实体文本（TC-GRAPH-VIZ-014）。
  - AC-28：输入搜索文本并回车后，匹配的节点自动居中+缩放+高亮显示（TC-GRAPH-VIZ-015）。
  - AC-29：工具栏包含 SVG 导出和 PNG 导出两个按钮，可点击（TC-GRAPH-VIZ-016）。
  - AC-30：点击导出 SVG 按钮后通过 SVG 序列化→Blob→下载触发文件下载（TC-GRAPH-VIZ-017）。
  - AC-31：双击节点打开的详情面板中显示实体类型徽章（带颜色区分），并显示来源 chunk ID 预览（TC-GRAPH-VIZ-018）。
  - AC-32：布局切换器存在且包含 force/hierarchical/radial 三个选项，默认 force 激活（TC-GRAPH-VIZ-019）。
  - AC-33：切换到 hierarchical 布局后图谱重新排列为层次树形，按钮状态正确更新（TC-GRAPH-VIZ-020）。
  - AC-34：切换到 radial 布局后图谱重新排列为辐射环形，节点仍然可见（TC-GRAPH-VIZ-021）。
  - AC-35：路径分析面板存在且包含起点/终点选择器，选择器填充所有实体（TC-GRAPH-VIZ-022）。
  - AC-36：选择起点终点后点击查找路径，显示最短路径长度和经过节点（TC-GRAPH-VIZ-023）。
  - AC-37：路径上的节点高亮显示，非路径节点暗淡（TC-GRAPH-VIZ-024）。
  - AC-38：社区检测按钮存在且可点击（TC-GRAPH-VIZ-025）。
  - AC-39：点击社区检测后节点按社区着色，显示社区数量（TC-GRAPH-VIZ-026）。
  - AC-40：BFS 最短路径算法在后端 Rust 实现，Label Propagation 社区检测迭代直到收敛或最大 10 轮（TC-GRAPH-ANALYSIS-001~010）。
- 契约：`EntityRelation` 结构体 ｜ `GraphTriple` 结构体 ｜ `GraphStats` 结构体 ｜ `GraphPath` 结构体 ｜ `GraphCommunity` 结构体 ｜ `EntityExtractor::extract_relations(text, chunk_id) -> Vec<EntityRelation>` ｜ `GraphRetriever<S: Storage>` 图遍历检索器（BFS 最大 2 跳 + 置信度×距离衰减加权 + 关系类型过滤） ｜ `rrf_fuse_four_way()` 四路 RRF 融合函数（Vector+BM25+Entity+Graph） ｜ `GraphAnalyzer` 图分析引擎（shortest_path BFS + all_paths DFS + detect_communities Label Propagation + degree_centrality） ｜ `Storage::add_relation` / `add_relations_batch` / `get_relations_for_entity` / `get_relations_for_chunk` / `search_by_relation` / `get_chunk_by_id` / `list_all_relations` / `count_relations` / `get_entity_types` / `get_entity_graph` trait 方法 ｜ `entity_relations` SQLite 表 ｜ `rag.graph_retriever_enabled` 设置键 ｜ `set_graph_retriever_enabled` / `get_graph_data` / `get_entity_relations` / `get_graph_stats` / `get_entity_types` / `get_shortest_path` / `get_communities` / `get_graph_layout` IPC 命令 ｜ `ui/src/graph-viewer.js` D3 force-directed graph 模块（含实体类型图标 + 子图过滤 + 搜索定位 + SVG/PNG 导出 + 详情面板增强 + 布局切换 + 路径分析 + 社区检测） ｜ `ui/vendor/d3.min.js` D3.js v7 本地 vendor ｜ 关联测试：TC-RELATION-001~006 + TC-GRAPH-001~006 + TC-GRAPH-VIZ-001~026 + TC-GRAPH-ANALYSIS-001~010
- 调研依据：mnemo 项目（Rust 知识图谱 RAG）——petgraph DiGraph 实体-关系图 + 多阶段检索管线（chunk search → entity search → graph expansion → relation filtering → scoring）；Mem0 知识图谱增强——实体链接 + 多信号检索。

#### REQ-RAG-028 RAG 质量门控
- 优先级：P2 ｜ 状态：Implemented ｜ 依赖：REQ-RAG-003（检索端口）、REQ-RAG-001（对话编排）
- 描述：借鉴 StoryMoss 加权评分门控系统，在检索后评估结果质量。当前版本仅做评估 + 日志记录（PassThrough 策略），不阻断生成。三维加权评分：检索覆盖率（top-1 分数归一化）×0.4 + 来源多样性（不同文档占比）×0.3 + 分数方差（top-k 标准差）×0.3，低于阈值 0.6 时记录告警日志。降级策略（ExpandTopK / WarnUser）由 chat_inner 在外部处理。
- 验收标准：
  - AC-1：高分检索结果通过门控（TC-GATE-001：results=[0.85,0.80,0.75] → passed=true）。
  - AC-2：低分检索结果未通过门控（TC-GATE-002：results=[0.15,0.12,0.10] → passed=false）。
  - AC-3：多样性计算正确——3 个结果来自 3 个不同文档时 diversity=1.0，3 个结果来自 1 个文档时 diversity≈0.333（TC-GATE-003）。
  - AC-4：分数方差计算正确——[0.9,0.1,0.1] 高方差 >0.3，[0.5,0.5,0.5] 零方差 <0.001（TC-GATE-004）。
  - AC-5：加权总分公式验证——手动计算 coverage/diversity/variance，与 evaluate() 返回值误差 <0.01（TC-GATE-005）。
  - AC-6：空结果返回全零 + passed=false（TC-GATE-006）。
  - AC-7：自定义阈值生效——threshold=0.95 时原本通过的结果变为不通过（TC-GATE-007）。
  - AC-8：门控禁用时行为完全一致（向后兼容）。
  - AC-9：门控启用后低质量检索有日志输出（[GATE] 前缀）。
- 契约：`GateConfig` 结构体 ｜ `DegradationStrategy` 枚举 ｜ `GateScore` 结构体 ｜ `GATE_PASS_THRESHOLD` 常量 ｜ `evaluate(results, config) -> GateScore` 函数 ｜ `ChatEngine::with_quality_gate(config)` builder 方法 ｜ `ChatEngine::gate_score() -> Option<&GateScore>` ｜ `ChatOutcome::Answered { gate_score: Option<GateScore> }` ｜ `rag.quality_gate_enabled` 设置键 ｜ 关联测试：TC-GATE-001~007
- 调研依据：StoryMoss 项目 `gate.rs`——三维加权评分门控（code 0.4 + rule 0.3 + model 0.3），低于阈值 0.75 时触发重试或降级。

#### REQ-RAG-029 Agent 生命周期 Hooks 系统
- 优先级：P2 ｜ 状态：Implemented ｜ 依赖：REQ-RAG-022（AgentEngine）、REQ-RAG-025（CoordinatorEngine）
- 描述：借鉴 Bamboo-agent 生命周期 hooks 设计，在 `AgentEngine`（ReAct 循环）和 `CoordinatorEngine`（四阶段管线）的关键执行节点插入可扩展的 hook 点，允许插件式注入逻辑（如检索前自动查询改写、检索后结果过滤、生成前上下文增强），无需修改核心引擎源码。4 个 Hook 阶段：`BeforeRetrieval`（检索前，可修改查询文本）、`AfterRetrieval`（检索后，可修改/过滤检索结果）、`BeforeGeneration`（LLM 生成前，可修改 prompt 上下文）、`AfterGeneration`（LLM 生成后，记录用，流式输出不可修改）。hook 按注册顺序执行，前一个 hook 修改的 `HookContext` 传递给下一个。任一 hook 返回 `Err` 时中断后续 hook 执行（错误冒泡到引擎）。不注册 hook 时引擎行为完全一致（向后兼容，零开销）。
- 验收标准：
  - AC-1：注册 hook 后 `run_phase` 正确调用（TC-HOOK-001：`call_count == 1`）。
  - AC-2：多个同阶段 hook 按注册顺序执行（TC-HOOK-002：3 个 hook 全部执行，序号递增）。
  - AC-3：`enabled() == false` 的 hook 不执行（TC-HOOK-003：禁用 hook 被跳过，不 panic）。
  - AC-4：hook 可修改 `HookContext` 字段（TC-HOOK-004：`ctx.query` 被改写）。
  - AC-5：hook 返回 `Err` 时中断后续 hook（TC-HOOK-005：后续 hook `call_count == 0`）。
  - AC-6：不同阶段的 hook 互不干扰（TC-HOOK-006：`BeforeRetrieval` hook 不被 `AfterRetrieval` `run_phase` 调用）。
  - AC-7：`HookRegistry` 的 `is_empty` / `count` 正确（TC-HOOK-007：空注册表 `is_empty == true`，注册 3 个后 `count == 3`）。
  - AC-8：`AgentEngine` / `CoordinatorEngine` 未注册 hook 时行为完全一致（向后兼容）。
  - AC-9：`AgentHook` trait 的所有实现必须是 `Send + Sync`。
- 契约：`HookPhase` 枚举（BeforeRetrieval/AfterRetrieval/BeforeGeneration/AfterGeneration）｜ `HookContext` 结构体（conversation_id/query/current_phase/retrieval_results/output/metadata）｜ `AgentHook` trait（name/phase/execute/enabled，`Pin<Box<Future>>` 返回类型保证对象安全）｜ `HookRegistry` 结构体（new/register/run_phase/count/is_empty）｜ `AgentEngine::with_hooks(registry)` builder 方法 ｜ `CoordinatorEngine::with_hooks(registry)` builder 方法 ｜ `AppState.hook_registry: Option<Arc<HookRegistry>>` 字段 ｜ 关联测试：TC-HOOK-001~007
- 调研依据：Bamboo-agent 源码——Agent 执行关键节点（before_retrieval / after_retrieval / before_generation / after_generation）插入可扩展 hook 点，允许插件式注入逻辑。

#### REQ-RAG-030 DAG 工作流引擎
- 优先级：P2 ｜ 状态：Implemented ｜ 依赖：REQ-RAG-003（Retriever）、REQ-RAG-001（LLMProvider）
- 描述：借鉴 IfAI 的 DAG 工作流引擎设计，用户定义由节点（`WorkflowNode`）和边（`WorkflowEdge`）组成的有向无环图，引擎负责拓扑排序（Kahn's algorithm）、并行执行同层独立节点（`join_all`）、串行执行跨层依赖节点。支持 5 种节点类型：`Retrieval`（知识库检索）、`Generation`（LLM 生成，`{input}` 占位符替换）、`Condition`（条件分支，`contains:` / `length>N` / `nonempty` / `true` 表达式）、`Aggregate`（多输入聚合，`concat` / `summarize` / `best_of` 策略）、`Output`（工作流终点）。Condition 节点通过边的 `mapping` 字段（`"true"` / `"false"`）选择下游分支，未激活分支标记 `Skipped`。节点失败时下游传递闭包标记 `Skipped`（不崩溃）。工作流模板以 JSON 序列化存储在 settings 表（`workflow.{id}` 键 + `workflow.index` 索引），不新建数据库表。
- 验收标准：
  - AC-1：简单线性工作流（A→B→C）正确执行，3 个节点都 Completed，final_output 非空（TC-WF-001）。
  - AC-2：并行节点（A→{B,C}→D）正确并行执行，node_2 和 node_3 在同一层，node_4 聚合两者输出（TC-WF-002）。
  - AC-3：拓扑排序正确分层，3 层 DAG 返回 3 个执行层（TC-WF-003）。
  - AC-4：检测到环返回明确错误信息（TC-WF-004）。
  - AC-5：Condition 节点正确分支，`contains:法律` 匹配时 true 路径执行、false 路径跳过（TC-WF-005）。
  - AC-6：Aggregate 节点 concat 策略正确合并上游输出（TC-WF-006）。
  - AC-7：节点失败时下游标记 Skipped（不崩溃），传递闭包正确传播（TC-WF-007）。
  - AC-8：evaluate_condition 各种表达式正确评估（`contains:` / `length>N` / `length<N` / `nonempty` / `true` / 未知→false 安全降级）（TC-WF-008）。
  - AC-9：Generation 节点 `{input}` 占位符正确替换为上游输出（TC-WF-009）。
- 契约：`WorkflowEngine<R: Retriever, L: LLMProvider>` 结构体（new/run/parallel_layers/evaluate_condition/execute_node 方法）｜ `Workflow` 结构体（id/name/description/nodes/edges/created_at）｜ `WorkflowNode` 结构体（id/label/node_type）｜ `NodeType` 标签枚举（`#[serde(tag = "kind")]`，Retrieval/Generation/Condition/Aggregate/Output 变体）｜ `WorkflowEdge` 结构体（source/target/mapping）｜ `NodeStatus` 枚举（Pending/Running/Completed{output}/Failed{error}/Skipped）｜ `WorkflowResult` 结构体（node_results/final_output/duration_ms）｜ IPC 命令：`save_workflow_template` / `run_workflow` / `list_workflows` / `delete_workflow` ｜ 关联测试：TC-WF-001~009
- 调研依据：IfAI 项目源码——DAG 工作流引擎（WorkflowNode/WorkflowEdge 架构），拓扑排序 + 并行执行 + 条件分支 + 聚合节点。

#### REQ-RAG-031 代码符号引擎（tree-sitter 代码感知 RAG）
- 优先级：P2 ｜ 状态：Implemented ｜ 依赖：REQ-ING-002（文件格式白名单）、REQ-VEC-001（Chunk）
- 描述：借鉴 IfAI SymbolEngine，使用 tree-sitter 对代码文件（.rs/.ts/.tsx/.py/.go）进行 AST 级符号分析，建立「符号 → 位置 → chunk」三级映射，使代码查询能精确定位到函数/类定义而非模糊匹配整个文件。导入代码文件时按函数/类边界分块（而非 token 窗口），每个顶级函数/类/结构体为一个独立 chunk，超大函数内部按行分割。每块附带符号上下文前缀（如 `// Symbol: foo (Function)\n`）。符号索引存储在 `code_symbols` 表（name/kind/language/chunk_id/start_line/end_line/signature），支持精确匹配（`WHERE name = ?`）和模糊匹配（`WHERE name LIKE '%query%'`）。查询中包含函数/类名时优先精确匹配符号（score=1.0），不足时追加模糊匹配（score=0.8）。Free 模式代码文件按纯文本加载 + 语义分块（向后兼容）。Pro 专用功能（tree-sitter 依赖门控）。
- 验收标准：
  - AC-1：Rust 代码抽取函数符号，`hello_world` 函数被识别为 `Function` 类型，签名非空（TC-SYMBOL-001）。
  - AC-2：Rust 代码抽取 struct + impl 方法，`Foo` 被识别为 `Struct`，`bar` 被识别为 `Method`（TC-SYMBOL-002）。
  - AC-3：TypeScript 代码抽取 class + interface，`IFoo` 被识别为 `Interface`，`FooImpl` 被识别为 `Class`（TC-SYMBOL-003）。
  - AC-4：Python 代码抽取函数 + 类 + 方法，`hello` 被识别为 `Function`，`World` 被识别为 `Class`，`__init__` 被识别为 `Method`（TC-SYMBOL-004）。
  - AC-5：按函数边界分块，3 个函数被分为至少 3 个独立 chunk，每个 chunk 包含完整函数定义（TC-SYMBOL-005）。
  - AC-6：超大函数内部按行分割，`continued` 标记出现在后续 chunk 前缀中（TC-SYMBOL-006）。
  - AC-7：语言检测正确，.rs→rust、.ts/.tsx→typescript、.py→python、.go→go，非代码文件返回 None（TC-SYMBOL-007）。
  - AC-8：符号精确搜索通过 SQLite Storage，`search_by_symbol("hello_world", None)` 返回正确结果（TC-SYMBOL-008）。
  - AC-9：符号模糊搜索，`search_symbols_fuzzy("hello", 10)` 匹配 `hello_world` 和 `hello_rust`（TC-SYMBOL-009）。
  - AC-10：非代码文件不受影响，.md/.txt/.pdf 返回 None（TC-SYMBOL-010）。
  - AC-11：Go 代码抽取函数和 struct，`main` 被识别为 `Function`，`Foo` 被识别为 `Struct`（TC-SYMBOL-011）。
- 契约：`SymbolExtractor` trait（detect_language/extract_symbols/split_by_symbols）｜ `SymbolEngine` 结构体（infra，Pro 门控）｜ `CodeSymbol` 结构体（id/chunk_id/name/kind/language/start_line/end_line/signature）｜ `SymbolKind` 枚举（Function/Method/Class/Struct/Interface/Enum/Constant/Module）｜ `code_symbols` 数据库表（3 索引：idx_symbols_name/chunk/kind）｜ Storage trait 4 个方法（add_symbols/search_by_symbol/get_symbols_for_chunk/search_symbols_fuzzy）｜ IPC 命令：`search_symbols` / `get_symbols_for_chunk` / `rebuild_symbol_index`（Pro 门控）｜ 关联测试：TC-SYMBOL-001~011
- 调研依据：IfAI 项目 SymbolEngine——tree-sitter AST 级符号抽取，建立「符号 → 位置 → chunk」三级映射。

#### REQ-RAG-032 代码执行沙箱（Agent 代码验证）
- 优先级：P2 ｜ 状态：Implemented ｜ 依赖：REQ-RAG-022（Agentic RAG）、REQ-LIC-001（Pro 门控）
- 描述：借鉴 CodeForge 多语言执行器，在安全沙箱中执行代码片段（Python / JavaScript），返回 stdout / stderr / exit_code / duration / timed_out。Agent 在 ReAct 循环中可调用 `execute_code` 工具执行代码片段验证结果，提升回答可信度。安全限制：超时 10s（硬编码上限 30s）、内存 64MB（硬编码上限 256MB）、无网络访问（恒为 false）、临时目录隔离、环境变量清空。第一阶段 MVP 使用本地进程执行（python3/node），后续替换为 WASM 沙箱（wasmtime）。Pro 专用功能（tempfile 依赖门控）。
- 验收标准：
  - AC-1：MockExecutor 返回预定义结果，supported_languages 和 config 正确（TC-EXEC-001）。
  - AC-2：超时场景正确处理：进程被 kill，timed_out=true，duration_ms 记录（TC-EXEC-002）。
  - AC-3：NoExecutor 始终返回错误（Free 版本安全降级）（TC-EXEC-003）。
  - AC-4：stderr 输出被正确捕获（TC-EXEC-004）。
  - AC-5：Agent execute_code 工具正确调用 executor 并格式化结果（TC-EXEC-005）。
  - AC-6：Free 版本 Agent execute_code 返回 Pro 提示（优雅降级）（TC-EXEC-006）。
  - AC-7：CodeExecutorConfig 默认值正确（10s/64MB/false）（TC-EXEC-007）。
  - AC-8：CodeExecutorConfig 安全上限约束（30s/256MB/false）（TC-EXEC-008）。
  - AC-9：format_execution_result 正确格式化执行结果（TC-EXEC-009）。
  - AC-10：MockExecutor with_config 自定义配置正确（TC-EXEC-010）。
  - AC-11：stdin 参数正确传递给执行进程（TC-EXEC-011）。
- 契约：`CodeExecutor` trait（execute/supported_languages/config，`Pin<Box<Future>>` 返回类型保证对象安全）｜ `MockExecutor` 结构体（测试用）｜ `NoExecutor` 零大小类型（Free 版本占位）｜ `WasmExecutor` 结构体（infra，Pro 门控）｜ `ExecutionResult` 结构体（stdout/stderr/exit_code/duration_ms/timed_out）｜ `CodeExecutorConfig` 结构体（timeout_secs/memory_limit_mb/allow_network）｜ `format_execution_result()` 函数｜ `AgentEngine::with_executor()` builder 方法｜ `execute_code` Agent 工具（JSON 输入：`{"language":"python","code":"...","stdin":"..."}`）｜ IPC 命令：`execute_code_snippet`（Pro 门控）｜ 关联测试：TC-EXEC-001~011 + TC-EXEC-INT-001~008
- 调研依据：CodeForge 项目源码——多语言代码执行沙箱（安全限制 + 超时 + 内存限制 + 临时目录隔离）。

#### REQ-RAG-033 持久化记忆系统（IfAI 三层记忆 + Bamboo-agent 上下文压缩）
- 优先级：P2 ｜ 状态：Implemented ｜ 依赖：REQ-RAG-001（RAG 查询）、REQ-ING-002（AutoDream 后台整理）
- 描述：借鉴 IfAI 三层记忆架构（Wing/Hall/Room 空间隐喻）和 Bamboo-agent 上下文压缩，实现跨会话知识沉淀系统。对话中由 LLM 辅助提取关键事实存入 Wing 层（临时记忆），AutoDream 后台空闲时自动整合（importance ≥ 0.6 提升到 Hall，≥ 0.7 提升到 Room，< 0.3 遗忘），查询时自动检索相关记忆注入到 system prompt 前缀。用户可手动置顶记忆（Room 层，importance 1.0）、提升/删除/清空记忆。三层上限：Wing 50 条、Hall 100 条、Room 500 条。
- 验收标准：
- AC-1：从对话中提取记忆，LLM 返回关键事实列表，存入 Wing 层（TC-MEMORY-001）。
- AC-2：记忆提升 Wing → Hall → Room，每次 importance += 0.1（TC-MEMORY-002）。
- AC-3：记忆遗忘：importance < 0.3 且 access_count 低 → 删除（TC-MEMORY-003）。
- AC-4：检索相关记忆并按 importance DESC 排序（TC-MEMORY-004）。
- AC-5：记忆整合超限时自动提升/遗忘（TC-MEMORY-005）。
- AC-6：记忆注入到 system prompt [相关记忆] 块（TC-MEMORY-006）。
- AC-7：access_count 更新 + last_accessed 刷新（TC-MEMORY-007）。
- AC-8：用户手动置顶记忆 → Room 层 importance 1.0（TC-MEMORY-008）。
- AC-9：禁用记忆系统时 ChatEngine 行为不变（TC-MEMORY-009）。
- AC-10：MemoryEntry 序列化/反序列化正确（TC-MEMORY-010）。
- AC-11：空对话不提取记忆（TC-MEMORY-011）。
- AC-12：AutoDream Phase 4 记忆整合生成 DreamSuggestion（TC-MEMORY-012）。
- 契约：`MemoryStore<S: Storage>` 结构体（extract_from_conversation/promote/forget/retrieve_relevant/consolidate/pin_memory）｜ `MemoryRetriever` trait（retrieve_relevant_memories，对象安全 `Pin<Box<Future>>`）｜ `MemoryEntry` 结构体（id/tier/content/source/conversation_id/created_at/last_accessed/access_count/importance）｜ `MemoryTier` 枚举（Wing/Hall/Room）｜ `MemorySource` 枚举（UserStatement/AssistantAnswer/AutoExtracted/UserPinned）｜ `ConsolidationResult` 结构体（promoted/forgotten/remaining_wing/remaining_hall/remaining_room）｜ `memory_entries` 数据库表（9 列 + 2 索引）｜ Storage trait 6 个方法（add/get/update/delete/clear/search_memory_entries）｜ `ChatEngine::with_memory()` builder 方法｜ `DreamEngine::consolidate_memories()` Phase 4 ｜ IPC 命令：`set_memory_enabled` / `get_memories` / `pin_memory` / `promote_memory` / `delete_memory` / `clear_memories`（6 个）｜ settings `memory.enabled` ｜ 关联测试：TC-MEMORY-001~012
- 调研依据：IfAI 三层持久记忆系统（Wing/Hall/Room 空间隐喻）+ Bamboo-agent 上下文压缩（对话过长时自动提取关键信息）+ Mem0 (2026.04) 对话记忆提取与注入。

#### REQ-RAG-034 语音输入（Web Speech API）
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-RAG-001
- **v1.1 规划**：S47 ｜ 决策日期：2026-08-06 ｜ 完成日期：2026-08-06
- 描述：用户通过麦克风语音提问，实时转写填入聊天输入框。基于 Web Speech API（`SpeechRecognition` / `webkitSpeechRecognition`），浏览器原生支持，零新增依赖。语言跟随当前 UI 语言（`zh-CN` / `en-US`）。不支持 Web Speech API 的 Tauri WebView 自动隐藏入口。
- 验收标准：
- AC-1：输入框旁存在麦克风按钮（`#micBtn` 元素可见，TC-UI-VOICE-001）。✅
- AC-2：点击麦克风启动语音识别（`SpeechRecognition.start()` 被调用，TC-UI-VOICE-002）。✅
- AC-3：转写结果实时填入输入框（`#queryInput.value` 包含转写文本，TC-UI-VOICE-003）。✅
- AC-4：Web Speech API 不支持时麦克风按钮隐藏（优雅降级，TC-UI-VOICE-006）。✅
- 契约：`ui/src/voice-input.js` 麦克风按钮 + SpeechRecognition 逻辑 ｜ settings `voice.language`（跟随 UI 语言） ｜ 关联测试：TC-UI-VOICE-001~006

#### REQ-RAG-035 TTS 回答朗读
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-RAG-001
- **v1.1 规划**：S47 ｜ 决策日期：2026-08-06 ｜ 完成日期：2026-08-06
- 描述：用户点击「朗读」按钮，助手回答通过语音合成播放。基于 Web Speech API（`speechSynthesis`），浏览器原生支持。语速可配置（0.5~2.0），音色使用系统默认语音。朗读时按钮状态切换为「停止朗读」。
- 验收标准：
  - AC-1：每条 AI 消息旁存在朗读按钮（`.tts-btn` 元素可见，TC-UI-VOICE-004）。
  - AC-2：点击朗读启动语音合成（`speechSynthesis.speak()` 被调用，TC-UI-VOICE-005）。
  - AC-3：朗读中按钮变为「停止」状态，点击停止。
  - AC-4：语速可在设置面板配置（0.5~2.0）。
- 契约：`ui/src/chat.js` 朗读按钮 + speechSynthesis 逻辑 ｜ settings `voice.rate` ｜ 关联测试：TC-UI-VOICE-004~006

#### REQ-RAG-036 网页搜索集成
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-RAG-001
- **v1.1 规划**：S48 ｜ 决策日期：2026-08-06 ｜ **实现日期**：2026-08-06
- 描述：本地检索结果不足时自动搜索互联网，将搜索结果摘要作为 RAG context 补充。基于 DuckDuckGo Instant Answer API（零依赖，HTTPS GET，无需 API Key）。opt-in 默认关闭，搜索查询仅发送到搜索引擎不经过第三方 LLM，搜索结果标注 🌐 前缀与本地知识库来源区分。
- 验收标准：
  - AC-1：✅ 搜索结果格式正确（`SearchResult` 含 title/snippet/url/source，TC-RAG-SEARCH-001 + TC-RAG-SEARCH-006）。
  - AC-2：✅ 搜索结果注入 RAG prompt 时标注 🌐 前缀（`convert_to_retrieval_results` 中 `doc_name` 前缀 `🌐`，TC-RAG-SEARCH-006）。
  - AC-3：✅ 本地检索不足时（top-1 score < 0.3 阈值）自动触发搜索（TC-RAG-SEARCH-002）。
  - AC-4：✅ 搜索关闭时（`web_search_enabled == false`）不触发搜索（TC-RAG-SEARCH-001 + chat_inner 条件判断）。
  - AC-5：✅ 搜索失败优雅降级（网络错误 → 仅用本地结果，不崩溃，TC-RAG-SEARCH-004）。
  - AC-6：✅ DuckDuckGo API 请求格式正确（URL 编码查询，`format=json`，TC-DDG-001~003）。
- 契约：`WebSearchProvider` trait（对象安全 `Pin<Box<Future>>`）｜ `DuckDuckGoProvider` 实现（infra 层）｜ `SearchResult` struct ｜ IPC 命令 `set_web_search_enabled` / `web_search` ｜ settings `rag.web_search_enabled`（默认 false）｜ 关联测试：TC-RAG-SEARCH-001~006 + TC-DDG-001~003 + TC-WEB-SEARCH-001~005
- 安全考量：opt-in 默认关闭 ｜ 查询不经过第三方 LLM ｜ 搜索结果不持久化到数据库

### 4.4 UI — 界面与交互状态

> **规格说明**：本域需求采用「原子组件规格」格式——每条需求包含元素清单（DOM ID + 类型 + 尺寸 + 视觉规范 + 文案）、状态矩阵（每态下各元素显隐/可用/文案）、交互行为（点击/快捷键→调用什么）、后端契约（IPC 命令/事件）。前端单文件 SPA 位于 `ui/index.html`，全部元素 ID 以代码实现为准。

#### REQ-UI-001 全局视觉规范与主界面布局
- 优先级：P0 ｜ 状态：Done ｜ 依赖：无

**色板**（Tailwind `tailwind.config.theme.extend.colors`）：

| Token | 色值 | 用途 |
|---|---|---|
| `base` | `#161616` | 全局背景 `<body class="bg-base">` |
| `panel` | `#1C1C1E` | 侧栏 / 卡片 / 输入框背景 |
| `line` | `#2A2A2E` | 分隔线 / 边框 |
| `accent` | `#38BDF8` | 强调色（按钮 / 链接 / 高亮 / 激活态） |
| `ink` | `#0C1116` | 强调色按钮上的文字色 |
| `muted` | `#94a3b8` | 次要文字 / thinking 指示器 |

**主界面布局**（`#app`，`class="hidden h-full flex"`，启动后 `hidden` 移除）：

```
┌─────────────────────────────────────────────────────────┐
│ #sidebar (w-60 / w-14)        │  #chatArea (flex-1)     │
│ ┌───────────────────────────┐ │  ┌───────────────────┐  │
│ │ 品牌头 + #collapseBtn     │ │  │                   │  │
│ │ #newChatBtn "＋ 新对话"    │ │  │  消息 Block 列表   │  │
│ │ "会话历史" 标签            │ │  │  (user/assistant)  │  │
│ │ #convList (flex-1 滚动)    │ │  │                   │  │
│ │ "知识库" 标签 + #docCount  │ │  └───────────────────┘  │
│ │ #docList (max-h-56 滚动)   │ │  #importProgress       │
│ │ #proStatus + #settingsBtn  │ │  ┌───────────────────┐  │
│ └───────────────────────────┘ │  │ #inputBar          │  │
│                                │  │ #plusBtn #queryInput│  │
│                                │  │ #sendBtn #stopBtn  │  │
│                                │  │ #inputHint         │  │
│                                │  └───────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

**元素清单**：

| ID | 元素 | Tailwind 类 | 尺寸 |
|---|---|---|---|
| `#app` | 主容器 | `h-full flex flex-col` | 100%×100% |
| `#topBar` | 顶部工具栏 | `shrink-0 h-11 flex items-center gap-1 px-2 border-b` | 100%×44px（含 macOS traffic lights padding 78px） |
| `#sidebar` | 左侧栏 | `w-60 shrink-0 bg-panel border-r border-line flex flex-col transition-all duration-200` | 240px（展开）/ 0px（折叠，完全隐藏） |
| `#chatArea` | 对话区 | `flex-1 overflow-y-auto scrollbar-thin px-6 py-6` | 剩余宽度 |

**侧栏折叠交互**（REQ-NAV-009）：
- 点击 `#collapseBtn` → `#sidebar` 添加 `sidebar-hidden` class（width=0 完全隐藏），`#collapseBtn` 隐藏，`#expandBtn` 显示。
- 点击 `#expandBtn` → `#sidebar` 移除 `sidebar-hidden` class（恢复 w-60），`#expandBtn` 隐藏，`#collapseBtn` 显示。
- 顶部工具栏（`#topBar`）始终可见，包含知识库图标（`#kbBtn`）、设置图标（`#settingsBtn`）、展开/折叠按钮。

- 验收标准：
  - AC-1：`<body>` 计算样式 `background-color` = `#161616`（E2E 断言）。
  - AC-2：`#sidebar` 计算样式 `background-color` = `#1C1C1E`。
  - AC-3：`tailwind.config` 中定义全部 6 个色 token，无规范外硬编码色值（静态扫描断言）。
  - AC-4：`#app` 为 `flex` 布局，`#sidebar` 为 `shrink-0`，`#chatArea` 为 `flex-1`（E2E 断言）。
  - AC-5：点击 `#collapseBtn` 后 `#sidebar` 宽度从 240px 变为 0px（完全隐藏），`#expandBtn` 显示；点击 `#expandBtn` 后恢复 240px。
  - AC-6：`ui/index.html` 中不出现 `https://` 或 `http://` 外部资源引用（静态扫描断言，REQ-ARCH-007）。
  - AC-7：全部第三方库（Tailwind / marked / DOMPurify / highlight / Mermaid / KaTeX / Chart.js）位于 `ui/vendor/` 目录下。
- 契约：`tailwind.config` 色板定义 + `#app` flex 布局 + `#topBar` 顶部工具栏 + `#sidebar` 折叠逻辑（`toggleSidebar()` + `sidebar-hidden` class）。
- 关联测试：E2E-UI-001（L3）

#### REQ-UI-002 Block 式对话与安全流式渲染
- 优先级：P0 ｜ 状态：Done ｜ 依赖：REQ-RAG-001

**消息 Block 元素清单**：

| 元素 | 角色 | Tailwind 类 | 生成方式 |
|---|---|---|---|
| user Block | 用户消息 | `w-fit max-w-[85%] ml-auto mr-0`（纯文本右对齐，无气泡背景/边框，对齐 chat.deepseek.com） | `appendBlock('user')` |
| assistant Block | AI 消息 | `md-block max-w-[92%] bg-panel border border-line rounded-2xl rounded-tl-md px-5 py-4` | `appendBlock('assistant')` |
| `.thinking-indicator` | 思考指示器 | `display:flex; gap:8px; color:#94a3b8; font-size:13px` + 3 个脉冲圆点 | 流式开始时显示，首 token 到达时移除 |
| `.md` | Markdown 渲染区 | `font-size:14px; line-height:1.8; color:#cbd5e1` | 流式开始时 `hidden`，首 token 到达时移除 `hidden` |
| `.sources` | 引用来源芯片区 | `mt-3 flex flex-wrap gap-2` | `chat_sources` 事件到达时填充 |
| `.msg-actions` | 消息操作栏 | `display:flex; gap:6px; opacity:0` → hover 时 `opacity:1` | 流结束后 `showMsgActions()` 填充 |

**流式渲染状态矩阵**：

| 阶段 | `.thinking-indicator` | `.md` | `.sources` | `.msg-actions` | `#inputHint` |
|---|---|---|---|---|---|
| 发送后（等待首 token） | 可见·"正在检索知识库…" | hidden | 空 | 无 | "正在检索知识库…" |
| `chat_phase` 事件到达 | 可见·文案更新为 phase.message | hidden | 空 | 无 | phase.message |
| 首 `chat_token` 到达 | 移除 | 可见·开始渲染 | 空 | 无 | 保持上次文案 |
| `chat_sources` 到达 | 已移除 | 可见·持续更新 | 填充芯片 | 无 | 保持 |
| 流式进行中 | 已移除 | 可见·rAF 节流更新 | 可见 | 无 | 保持 |
| `chat_done` | 已移除 | 可见·最终渲染 + 富内容 | 可见 | 填充"复制全文"按钮 | 清空 |
| `chat_error`（中断） | 已移除 | 可见·追加"⏹ 已中断"标记 | 可见 | 无 | 错误文案 |

**引用来源芯片规格**：
- DOM：`<span class="text-[11px] px-2.5 py-1 rounded-full bg-accent/10 text-accent border border-accent/25">`
- 文案格式：`[序号] 文档名 · 相似度百分比%`（如 `[1] guide.md · 87%`）
- `title` 属性：chunk 内容前 200 字符（hover 提示）

**代码块增强规格**：
- 每个 `<pre>` 包裹为 `.code-block` 容器（`margin:12px 0; border-radius:10px; border:1px solid rgba(255,255,255,0.08); background:#0d1117`）
- 头部栏 `.code-header`：语言标签 `.code-lang`（uppercase, 11px, `#94a3b8`）+ 复制按钮 `.copy-btn`
- 复制按钮文案：默认"复制" → 点击后"已复制 ✓"（1.5s 后恢复）
- 复制行为：`navigator.clipboard.writeText(code.innerText)`
- highlight.js 语法高亮，`dataset.highlighted` 标记防重复

**渲染管线**：`currentRawMarkdown` → `marked.parse()` → `DOMPurify.sanitize()` → `innerHTML` → 代码块增强 → Mermaid 占位 → rAF 节流（`scheduleRender()`）

- 验收标准：
  - AC-1：user Block 右对齐（`justify-end`），assistant Block 左对齐，视觉区分明确。
  - AC-2：模型输出含 `<script>alert(1)</script>` 时被 DOMPurify 消毒，不执行、不弹窗。
  - AC-3：模型输出含代码块时，`.code-block` 容器存在，语言标签显示，复制按钮可点击且复制内容正确。
  - AC-4：流式期间半截 Markdown 语法（如未闭合 ```` ``` ````）不产生布局错乱（marked 容错）。
  - AC-5：`chat_sources` 到达后 `.sources` 区显示芯片，格式为 `[序号] 文档名 · 百分比%`。
  - AC-6：流结束后 `.msg-actions` 显示"复制全文"按钮，hover 时 `opacity:1`。
  - AC-7：rAF 节流生效——连续 `chat_token` 事件不触发多次 DOM 写入（`renderScheduled` 标志位防抖）。
- 契约：消费 `chat_token` / `chat_sources` / `chat_error` / `chat_done` / `chat_phase` 事件；`marked.parse()` + `DOMPurify.sanitize()` + `hljs.highlightElement()` 链路。
- 关联测试：E2E 待 CI 阶段

#### REQ-UI-003 控制台式输入框状态机
- 优先级：P0 ｜ 状态：Implemented ｜ 依赖：REQ-RAG-001

**元素清单**：

| ID | 元素类型 | Tailwind 类 | 尺寸 | 文案/图标 |
|---|---|---|---|---|
| `#inputBar` | 容器 | `bg-panel border border-line rounded-2xl px-4 py-3 flex items-end gap-3 transition-colors focus-within:border-accent/60` | 100%宽 × 自适应高 | — |
| `#plusBtn` | button | `shrink-0 w-9 h-9 rounded-xl bg-white/5 text-slate-300 hover:text-accent hover:bg-accent/10 transition-colors text-xl leading-none` | 36×36px | `＋` (xl)，`title="导入文件"` |
| `#queryInput` | textarea | `flex-1 bg-transparent resize-none outline-none text-sm leading-6 max-h-40 scrollbar-thin py-1.5` | 自适应宽 × 1~6行 | `placeholder="输入问题，Enter 发送 · Shift+Enter 换行"` |
| `#sendBtn` | button | `shrink-0 h-9 px-4 rounded-xl bg-accent text-sm font-medium hover:opacity-90 transition-opacity disabled:opacity-30 text-ink` | 高36px | "发送" |
| `#stopBtn` | button | `hidden shrink-0 h-9 px-4 rounded-xl bg-red-500/15 text-red-300 border border-red-400/40 text-sm hover:bg-red-500/25 transition-colors` | 高36px | "■ 停止"，`title="停止生成"` |
| `#inputHint` | div | `mt-2 px-1 text-[11px] text-slate-500 h-4` | 100%宽 × 16px | 状态提示文案（动态） |

**状态矩阵**（`setInputState(state, hint)` 函数驱动）：

| 状态 | `streaming` | `#queryInput` | `#sendBtn` | `#stopBtn` | `#inputHint` | 触发条件 |
|---|---|---|---|---|---|---|
| `idle` | false | 可用·`focus()` | 可见·可点击 | hidden | 空 | 启动 / `chat_done` / 错误恢复 |
| `streaming` | true | `disabled` | hidden | 可见·可点击 | hint 文案 | `send()` 调用后 |
| `error` | false | 可用·`focus()` | 可见·可点击 | hidden | 红色错误文案 | `chat_error` 非"已中断" |

**`#inputHint` 动态文案**：

| 事件 | 文案 |
|---|---|
| `send()` 调用后 | "正在检索知识库…" |
| `chat_phase` `{phase: "preparing"}` | "正在准备…" |
| `chat_phase` `{phase: "retrieving"}` | "正在检索知识库…" |
| `chat_phase` `{phase: "generating"}` | "正在生成回答…" |
| `chat_done` | 清空（""） |
| `chat_error`（非中断） | `sanitizeError(err)`（红色 `text-slate-500` 不变，内容为错误文案） |
| 审计模式 `audit_phase` | `p.message` |

**交互行为**：

| 触发 | 动作 | 条件 |
|---|---|---|
| 点击 `#plusBtn` | 调用 `tauriDialog.open({ multiple: true, filters: [{ name: '文档', extensions: ['md','txt','pdf'] }] })` → 选中后调用 `importPaths(paths)` | — |
| `#queryInput` 按 Enter（无 Shift） | `e.preventDefault()` → `send()` | `streaming === false` 且输入非空 |
| `#queryInput` 按 Shift+Enter | 插入换行（默认行为） | — |
| `#queryInput` `input` 事件 | `e.target.style.height = 'auto'` → `Math.min(scrollHeight, 160) + 'px'` | 自动扩展高度，上限 160px |
| 点击 `#sendBtn` | `send()` | `streaming === false` 且输入非空 |
| 点击 `#stopBtn` | 对话模式：`invoke('abort_chat', { conversationId })`；审计模式：`invoke('abort_audit', { docId })` | `auditingDocId` 判断模式 |

**`send()` 函数行为**：
1. 读取 `#queryInput.value.trim()`，空则 return。
2. 清空 `#queryInput` + 重置高度。
3. `appendUserBlock(query)` 追加用户消息。
4. `history.push({ role: 'user', content: query, sources: null })`。
5. `appendBlock('assistant')` 追加 AI 消息（含 thinking-indicator）。
6. `setInputState('streaming', '正在检索知识库…')`。
7. `invoke('chat', { query, history: history.slice(0,-1), conversationId })`。
8. IPC 失败 → `toastError(err)` + `finalizeStream(false)` + `setInputState('error', sanitizeError(err))`。

- 验收标准：
  - AC-1：`idle` 态下 `#sendBtn` 可见、`#stopBtn` hidden、`#queryInput` 可用。
  - AC-2：`streaming` 态下 `#queryInput.disabled = true`、`#sendBtn` hidden、`#stopBtn` 可见。
  - AC-3：`streaming` 态下按 Enter 不触发 `send()`（`streaming === true` 守卫）。
  - AC-4：`streaming` 态下点击 `#stopBtn` 调用 `abort_chat`（或 `abort_audit`），≤200ms 内流中断。
  - AC-5：`error` 态下 `#inputHint` 显示脱敏错误文案，`#queryInput` 可用且 `focus()`。
  - AC-6：`#queryInput` 输入超过 1 行时高度自动扩展，上限 160px（`max-h-40`）。
  - AC-7：点击 `#plusBtn` 弹出系统文件选择器，`filters` 限定 `md/txt/pdf`。
- 契约：`setInputState(state, hint)` 函数 + `send()` 函数 + `abort_chat` / `abort_audit` IPC 命令。
- 关联测试：TC-RAG-003（停止生成）、E2E-UI-001（L3）

#### REQ-UI-004 拖拽全屏遮罩
- 优先级：P0 ｜ 状态：Done ｜ 依赖：REQ-ING-001

**元素清单**：

| ID | 元素 | Tailwind 类 | 尺寸 | 默认状态 |
|---|---|---|---|---|
| `#dragOverlay` | 全屏遮罩 | `hidden fixed inset-0 z-50 bg-base/90 backdrop-blur-sm flex items-center justify-center pointer-events-none` | 100%×100% | hidden |
| 内部虚线框 | 视觉容器 | `border-2 border-dashed border-accent rounded-3xl w-[86%] h-[80%] flex flex-col items-center justify-center fade-in` | 86%×80% | — |
| 图标 | — | `text-5xl text-accent mb-4` | — | `⇩` |
| 标题 | — | `text-xl font-medium text-accent` | — | "数据仅本地处理" |
| 副标题 | — | `mt-2 text-sm text-slate-400` | — | "松开即可导入 · 支持 .md / .txt / .pdf（PDF 为 Pro 功能）" |

**状态矩阵**：

| 事件 | `#dragOverlay` | 说明 |
|---|---|---|
| 初始 | `hidden` | 遮罩不可见 |
| `tauri://drag-enter` | 移除 `hidden` | 遮罩显示，虚线框 fade-in 动画 |
| `tauri://drag-leave` | 添加 `hidden` | 遮罩消失，不产生导入 |
| `tauri://drag-drop` | 添加 `hidden` → `importPaths(e.payload.paths)` | 遮罩消失，触发导入 |

**交互行为**：
- `pointer-events-none`：遮罩不拦截鼠标事件，Tauri 原生拖拽事件直接处理。
- drop 后调用 `importPaths(paths)` → `invoke('import_files', { paths })`。

- 验收标准：
  - AC-1：拖拽文件进入窗口时 `#dragOverlay` 从 `hidden` 变为可见。
  - AC-2：遮罩文案为"数据仅本地处理"，强调色 `#38BDF8`。
  - AC-3：拖拽离开窗口（未释放）时遮罩消失，不触发导入。
  - AC-4：拖拽释放后遮罩消失，`importPaths` 被调用。
  - AC-5：遮罩 `pointer-events-none`，不拦截底层交互（由 Tauri 原生事件驱动）。
- 契约：监听 `tauri://drag-enter` / `tauri://drag-leave` / `tauri://drag-drop` 事件；drop 载荷 `paths` 传入 `importPaths()` → `import_files` IPC。
- 关联测试：E2E 待 CI 阶段

#### REQ-UI-005 空状态与全局错误反馈（Toast）
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：无

**Toast 通道元素清单**：

| ID | 元素 | Tailwind 类 | 位置 |
|---|---|---|---|
| `#toasts` | Toast 容器 | `fixed bottom-6 right-6 z-[60] space-y-2 w-80` | 右下角，z-index 60 |

**Toast 单条规格**（`toast(message, kind)` 函数动态创建）：

| kind | border 色 | text 色 | 场景 |
|---|---|---|---|
| `info`（默认） | `border-line` | `text-slate-300` | 一般提示 |
| `success` | `border-accent/50` | `text-accent` | 成功操作 |
| `error` | `border-red-400/40` | `text-red-300` | 错误提示 |

- 单条 Toast 类名：`bg-panel border {color} rounded-xl px-4 py-3 text-sm shadow-lg fade-in`
- 自动消失：`setTimeout(() => el.remove(), 4200)`（4.2 秒）
- `fade-in` 动画：`opacity 0→1` + `translateY(6px)→none`，250ms ease-out

**空状态引导规格**：

| 场景 | 触发条件 | 内容 |
|---|---|---|
| 空知识库（`#docList`） | `loadDocuments()` 返回空列表 | `text-[11px] text-slate-500`："知识库为空 / 点击 ＋ 或拖拽导入 / 支持 .md / .txt / .pdf" |
| 空对话（`#chatArea`） | `resetChatArea(title, desc)` | 居中布局：`◈`图标(text-5xl text-accent) + 标题(text-lg font-medium) + 描述(text-sm text-slate-500 max-w-sm) |
| 首次启动空对话 | `resetChatArea('向你的知识库提问', '回答仅基于本地数据与你自己配置的模型。点击左下角 ＋ 或直接把文件拖进来，开始构建知识库。')` | — |
| 新会话空对话 | `resetChatArea('新会话已开始', '上下文已清空，向你的知识库提问吧。')` | — |
| 无消息会话 | `resetChatArea('暂无消息', '开始提问吧。')` | — |

**错误脱敏规格**（`sanitizeError(err)` 函数）：
- 输入：`string | Error`，先 `String(err)` 转字符串。
- 过滤 API Key：`/sk-[a-zA-Z0-9]{8,}/g` → `'sk-****'`。
- 过滤 Windows 用户路径：`/\\Users\\[^\\]+?\\/g` → `'\\Users\\****\\'`。
- 过滤 Unix 用户路径：`/\/Users\/[^/]+?\//g` → `'/Users/****/'`。
- 输出：脱敏后的安全字符串。
- `toastError(err)` = `toast(sanitizeError(err), 'error')`。

- 验收标准：
  - AC-1：`#toasts` 容器位于右下角（`fixed bottom-6 right-6`），z-index 60。
  - AC-2：`toast('测试', 'success')` 生成的元素含 `border-accent/50 text-accent` 类。
  - AC-3：Toast 4.2 秒后自动从 DOM 移除。
  - AC-4：`sanitizeError('Error: sk-abcdefgh123456789')` 返回 `'Error: sk-****'`。
  - AC-5：`sanitizeError('Error: /Users/john/file.txt')` 返回 `'Error: /Users/****/file.txt'`。
  - AC-6：空知识库时 `#docList` 显示导入引导文案（含格式与配额说明）。
  - AC-7：空对话时 `#chatArea` 居中显示 `◈` 图标 + 标题 + 描述。
- 契约：`toast(message, kind)` / `toastError(err)` / `sanitizeError(err)` / `toastSuccess(message)` 函数 + `resetChatArea(title, desc)` 空状态渲染。
- 关联测试：E2E-UI-001-lite（L3 桥接层）

#### REQ-UI-006 知识库列表面板
- 优先级：P0 ｜ 状态：Done ｜ 依赖：REQ-ING-001、REQ-VEC-004

**侧栏「知识库」区块元素清单**（从上到下）：

| ID / 元素 | 角色 | Tailwind 类 / 规格 | 文案 |
|---|---|---|---|
| `.side-label` 标签行 | 区块标题 | `px-4 pt-4 pb-2 text-[11px] uppercase tracking-wider text-slate-500 flex items-center justify-between border-t border-line` | "知识库" |
| `#docCount` | 配额计数 | `text-accent` | 免费版：`"n/50"`；Pro 版：`"n"`（无上限） |
| `#docList` | 文档列表容器 | `max-h-56 overflow-y-auto scrollbar-thin px-2 space-y-1` | 最大高 224px，超出滚动 |

**文档列表项规格**（`loadDocuments()` 动态创建）：

```
┌─────────────────────────────────────────────────────┐
│ 📄 文件名（truncate）  [状态徽标] [×] [↻] [🔍]      │
│   ↑ .text-slate-300       ↑ 徽标     ↑ hover显示    │
└─────────────────────────────────────────────────────┘
```

- 列表项容器：`group flex items-center justify-between gap-2 px-3 py-2 rounded-xl text-xs hover:bg-white/5`
- 文件名：`truncate text-slate-300`，`title` = 完整 `file_path`，显示名经 `displayDocName()` 剥离 MD5 前缀
- 操作按钮区：`flex items-center gap-2 shrink-0`

**索引状态徽标规格**（`DOC_STATUS_STYLE` 映射）：

| DocStatus | 中文标签 | 文字色 | 边框色 | 附加按钮 |
|---|---|---|---|---|
| `Pending` | "待索引" | `text-slate-400` | `border-slate-500/40` | × 删除 |
| `Processing` | "索引中" | `text-amber-300` | `border-amber-400/40` | × 删除 |
| `Indexed` | "已索引" | `text-accent` | `border-accent/40` | × 删除 + 🔍 审计（Pro） |
| `Failed` | "失败" | `text-red-400` | `border-red-400/40` | × 删除 + ↻ 重试 |

- 徽标样式：`px-1.5 py-0.5 rounded border {color}`
- 删除按钮 `×`：`invisible group-hover:visible text-slate-500 hover:text-red-400`，点击 → `deleteDocument(docId, name)`
- 重试按钮 `↻`：仅 Failed 状态显示，`text-red-400 hover:text-red-300`，点击 → `retryDocument(docId, name)` → `invoke('retry_index', { id })`
- 审计按钮 `🔍`：仅 Pro + Indexed 显示，点击 → `auditDocument(docId, name)` → `invoke('audit_document', { docId, docName })`

**交互行为**：

| 触发 | 动作 | 条件 |
|---|---|---|
| `doc-status-changed` 事件 | `toast(message, status==='error'?'error':'info')` + `loadDocuments()` 刷新 | — |
| 点击 × | `deleteDocument(id, name)` → `invoke('delete_document', { id })` → toast + `loadDocuments()` | — |
| 点击 ↻ | `retryDocument(id, name)` → toast + `invoke('retry_index', { id })` | Failed 状态 |
| 点击 🔍 | `auditDocument(id, name)` → Pro 门控 → `invoke('audit_document', ...)` | Pro + Indexed |

- 验收标准：
  - AC-1：`loadDocuments()` 调用后 `#docList` 渲染全部文档项，`#docCount` 更新为 `n/50`（免费版）或 `n`（Pro）。
  - AC-2：文档状态徽标颜色与 `DOC_STATUS_STYLE` 映射一致（4 种状态 × 4 种颜色）。
  - AC-3：hover 文档项时删除按钮 `×` 从 `invisible` 变为 `visible`。
  - AC-4：点击 `×` 后调用 `delete_document`，成功后 toast + 列表刷新，该 DOM 元素消失。
  - AC-5：Failed 状态文档显示 `↻` 重试按钮，点击后调用 `retry_index`。
  - AC-6：Pro + Indexed 文档显示 `🔍` 审计按钮；非 Pro 不显示。
  - AC-7：`doc-status-changed` 事件到达时自动刷新列表 + toast 提示。
  - AC-8：空知识库时 `#docList` 显示导入引导文案。
- 契约：`get_documents` / `delete_document` / `retry_index` / `audit_document` IPC 命令 + `doc-status-changed` 事件。
- 关联测试：E2E-UI-001（L3）

#### REQ-UI-007 首次启动配置向导
- 优先级：P0 ｜ 状态：Done ｜ 依赖：REQ-UI-008、REQ-LLM-002

**向导元素清单**（`#wizard`，`class="hidden fixed inset-0 z-40 bg-base flex items-center justify-center"`）：

| ID | 元素 | Tailwind 类 | 文案/说明 |
|---|---|---|---|
| `#wizard` | 全屏遮罩 | `hidden fixed inset-0 z-40 bg-base flex items-center justify-center` | 默认 hidden，启动时若未配置 LLM 则显示 |
| 内部容器 | — | `w-full max-w-xl px-8 fade-in` | 最大宽 576px |
| 品牌标题 | — | `text-3xl font-semibold tracking-wide` | "灵犀 EchoMind"（EchoMind 为 accent 色） |
| 副标题 | — | `mt-3 text-sm text-slate-400` | "本地 AI 知识库 · 数据不出域 · BYOK 买断制 / 首次使用，请配置你的大模型接入" |
| `#presetCards` | 预设卡片容器 | `grid grid-cols-3 gap-3 mb-6` | 3 列等宽 |
| `#wizKey` | API Key 输入框 | `w-full bg-panel border border-line rounded-xl px-4 py-3 text-sm outline-none focus:border-accent transition-colors` | `type="password"`, `placeholder="sk-..."`, `autocomplete="off"` |
| `#keyOptional` | 可选提示 | `hidden text-accent` | "（本地端点可留空）" — Ollama 预设时显示 |
| `#wizUrl` | Base URL 输入框 | 同 `#wizKey` | `type="text"` |
| `#wizModel` | Model 输入框 | 同 `#wizKey` | `type="text"` |
| `#wizError` | 错误提示区 | `hidden mt-4 text-sm text-red-400 bg-red-400/10 border border-red-400/30 rounded-xl px-4 py-3` | 默认 hidden，验证失败时显示 |
| `#wizKeyLink` | 获取 Key 链接 | `text-sm text-accent hover:underline` | "去获取 API Key →" |
| `#wizStart` | 验证按钮 | `bg-accent font-medium text-sm rounded-xl px-6 py-3 hover:opacity-90 transition-opacity disabled:opacity-40 text-ink` | "验证并开始" |

**预设卡片规格**（`PRESETS` 对象 + `renderPresetCards()` 函数）：

| key | label | base_url | model | keyUrl | needKey | desc |
|---|---|---|---|---|---|---|
| `deepseek` | "DeepSeek" | `https://api.deepseek.com` | `deepseek-chat` | `https://platform.deepseek.com/api_keys` | true | "高性价比中文强模型" |
| `openai` | "OpenAI" | `https://api.openai.com` | `gpt-4o-mini` | `https://platform.openai.com/api-keys` | true | "GPT 系列官方端点" |
| `ollama` | "Ollama 本地" | `http://localhost:11434` | `llama3.1` | `https://ollama.com/library` | false | "完全离线的本地模型" |

- 选中态卡片：`border-accent bg-accent/10`，标题 `text-accent`
- 未选中态卡片：`border-line bg-panel hover:border-slate-500`
- 点击卡片 → `activePreset = key` → `applyPreset()` 填充 URL/Model → 重新渲染卡片

**交互行为**：

| 触发 | 动作 | 条件 |
|---|---|---|
| 点击预设卡片 | `activePreset = key` → `applyPreset()` → `renderPresetCards()` | — |
| `applyPreset()` | `#wizUrl.value = preset.base_url`；`#wizModel.value = preset.model`；`#keyOptional` toggle hidden（needKey=true 隐藏，false 显示）；`#wizError` 添加 hidden | — |
| 点击 `#wizKeyLink` | `tauriOpener.openUrl(preset.keyUrl)` → 失败则 `toast('请手动访问：' + url, 'info')` | — |
| 点击 `#wizStart` | `startWizard()`（见下方流程） | — |
| `#wizStart` 按钮文本变化 | 验证中："验证中…" + `disabled`；恢复："验证并开始" + `disabled=false` | — |

**`startWizard()` 验证流程**：
1. 读取 `#wizKey.value.trim()` / `#wizUrl.value.trim()` / `#wizModel.value.trim()`。
2. 前置校验：`preset.needKey && !api_key` → `#wizError` 显示"请填入 API Key（或选择 Ollama 本地预设）"。
3. 前置校验：`!base_url || !model` → `#wizError` 显示"Base URL 与 Model 不能为空"。
4. `#wizStart.disabled = true` + 文本改为"验证中…"。
5. `invoke('test_llm_connection', { apiKey, baseUrl, model })`。
6. 验证失败 → `#wizError` 显示 `String(err)` + 恢复按钮 → **不进入主界面**。
7. 验证成功 → `invoke('update_llm_config', { config: { api_key, base_url, model } })`。
8. 配置保存失败 → `#wizError` 显示"配置保存失败：" + err → 恢复按钮。
9. 配置保存成功 → `showApp()` + `toast('配置完成，开始构建你的知识库吧', 'success')` + `initConversations()`。

**启动流程**（IIFE）：
1. `invoke('get_settings')` → `settings.has_llm_config === true` → `isPro = invoke('get_pro_status')` → `updateProStatus()` → `showApp()` → `initConversations()`。
2. `settings.has_llm_config === false` → `showWizard()`。
3. `get_settings` 失败 → `showWizard()`（兜底）。

- 验收标准：
  - AC-1：`get_settings` 返回 `has_llm_config: false` 时启动即显示 `#wizard`，`#app` hidden。
  - AC-2：`get_settings` 返回 `has_llm_config: true` 时直接显示 `#app`，`#wizard` hidden。
  - AC-3：3 张预设卡片显示，DeepSeek 默认选中（`activePreset = 'deepseek'`）。
  - AC-4：选中 Ollama 预设后 `#keyOptional` 显示"（本地端点可留空）"。
  - AC-5：点击 `#wizStart` 且 API Key 为空（非 Ollama）→ `#wizError` 显示"请填入 API Key"。
  - AC-6：验证失败 → `#wizError` 显示错误 + 按钮恢复，不进入主界面。
  - AC-7：验证成功 → `showApp()` + toast + `initConversations()`。
  - AC-8：点击 `#wizKeyLink` 调用 `tauriOpener.openUrl(url)` 打开系统浏览器。
- 契约：`get_settings` / `test_llm_connection` / `update_llm_config` / `get_pro_status` IPC 命令 + `tauriOpener.openUrl()` 插件。
- 关联测试：TC-UI-001（持久化）、E2E 待 CI 阶段

#### REQ-UI-008 LLM 配置持久化
- 优先级：P0 ｜ 状态：Done ｜ 依赖：REQ-DB-001
- 描述：`update_llm_config` 命令将配置写入 SQLite settings 表（AES-256-GCM 加密，随机密钥存于数据目录 `secret.key`，权限 0600）；`get_settings` 返回配置存在性、base_url、model 与脱敏 Key（仅末四位），**严禁回传明文 Key**；启动时从 settings 表恢复运行态配置。
- 验收标准：
  - AC-1：update → 重建 AppState（模拟重启）→ get_settings 数据仍在且 Key 脱敏（TC-UI-001）。
  - AC-2：settings 表存储值不出现明文 Key（密文为 base64(nonce‖ciphertext)）。
  - AC-3：update 后运行态配置立即生效，无需重启。
- 契约（TPM 冻结，2026-07-24）：Command `update_llm_config(config: LlmConfig, state) -> Result<(), String>`；Command `get_settings(state) -> Result<SettingsPayload, String>`。
- 关联测试：TC-UI-001

#### REQ-UI-009 列表事件节流
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-UI-006、REQ-ING-006
- 描述：批量导入文件时 `doc-status-changed` 事件密集触发（每个文件索引完成发射一次），前端 `onDocStatusChanged` 回调直接调用 `loadDocuments()` 全量重渲染文档列表，导致事件风暴——50 个文件导入 = 50 次全量 DOM 重建。此需求在前端引入防抖机制：`onDocStatusChanged` 回调包装 `debounce(loadDocuments, 200)`，200ms 内的连续事件仅触发一次重渲染。防抖工具函数 `debounce(fn, delay)` 为通用工具，可用于其他密集事件场景。
- 验收标准：
  - AC-1：`debounce(fn, 200)` 在 200ms 内连续调用 5 次时，`fn` 仅执行 1 次（最后一次）。
  - AC-2：批量导入 10 个文件时，`loadDocuments` 调用次数 ≤ 2 次（而非 10 次）。
  - AC-3：防抖不延迟首次事件超过 200ms（用户感知到列表更新）。
  - AC-4：导入完成后 200ms 内列表最终刷新为正确状态。
- 契约：`debounce(fn: Function, delay: number) => Function` + `onDocStatusChanged: debounce(loadDocuments, 200)` ｜ 关联测试：TBD

#### REQ-UI-010 DeepSeek 风格 UI 对齐
- 优先级：P2 ｜ 状态：Done ｜ 依赖：REQ-UI-003、REQ-RAG-001
- 描述：借鉴 DeepSeek 聊天界面的设计风格，对 EchoMind 进行视觉与交互全面对齐。包括：用户消息纯文本右对齐（无气泡背景，对齐 chat.deepseek.com 2026-08 实测 DOM）、AI 消息移除背景（透明）、AI 免责声明、CJK 引号修复、焦点环改用 box-shadow、消息列表最大宽度 840px、可折叠思维链面板（思考内容 markdown 渲染）、版本切换器胶囊样式（`◀ 1 / N ▶` 带空格计数器）、输入区快速 Toggle（混合搜索）、新对话快捷键（⌘ J）、CSS 变量三层架构（static→alias→component）、主题切换过渡禁用。
- 验收标准：
  - AC-1：用户消息纯文本右对齐、无背景/边框（`getComputedStyle.backgroundColor` 透明、`borderTopWidth === '0px'`、右边缘贴近聊天区）
  - AC-2：AI 消息无背景色/边框（`getComputedStyle.backgroundColor === transparent`）
  - AC-3：AI 消息底部显示免责声明（`.ai-disclaimer` 元素可见且文本非空）
  - AC-4：思维链过程可折叠/展开（点击 `.thinking-panel-header` 切换 `.thinking-panel-content` 的 `hidden` 类）
  - AC-5：搜索来源可折叠/展开（点击 `.sources-toggle` 切换 `.sources-list` 的 `display` 属性）
  - AC-6：输入框旁显示快速 toggle（`#inputToggles` 容器含 `.input-toggle` 元素）
  - AC-7：新对话按钮显示快捷键提示（`#newChatBtn .shortcut-hint` 含 `⌘ J` 文本）
  - AC-8：⌘ J 全局快捷键触发新对话（按下后聊天区清空，空状态出现）
  - AC-9：CJK 引号使用 PingFangSC 渲染（`@font-face quote-cjk-patch` + `unicode-range: U+201C-201D`）
  - AC-10：焦点环使用 box-shadow 而非 outline（`:focus-visible` 的 `outline: none` + `box-shadow`）
  - AC-11：CSS 变量三层架构定义完成（`--dsw-alias-*` 语义别名层 + `--dsl-*` 组件变量层）
  - AC-12：主题切换过渡禁用规则定义（`body.change-theme *` 禁用 transition/animation）
- 契约：`set_hybrid_search(enabled: bool)` / `set_agent_enabled(enabled: bool)` IPC 命令；前端模块 `thinking-panel.js` / `input-toggles.js`。
- 关联测试：TC-DS-001~012（Vitest 单元测试）、TC-DS-013~020（Playwright E2E 测试）

#### REQ-UI-011 浅色主题切换
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-UI-001
- **v1.1 实现**：S46 ｜ 决策日期：2026-08-06 ｜ 完成日期：2026-08-06
- 描述：新增浅色主题支持，通过 CSS 变量覆盖实现零组件代码改动。`tokens.css` 新增 `[data-theme="light"]` 变量集（全部覆盖暗色变量），`main.js` 新增 `setTheme(theme)` 函数 + `localStorage` 持久化，设置面板新增「外观」区域（暗色/浅色/跟随系统三选一）。支持 `prefers-color-scheme` 自动跟随系统主题。后端持久化到 settings 表（`ui.theme` 键）。
- 验收标准：
  - AC-1：切换到浅色主题后 `document.documentElement.dataset.theme === 'light'`（TC-UI-LIGHT-001）。
  - AC-2：浅色主题下所有颜色变量生效（`getComputedStyle(body).backgroundColor` 为浅色，TC-UI-LIGHT-002）。
  - AC-3：主题选择持久化（刷新后保持，TC-UI-LIGHT-003）。
  - AC-4：「跟随系统」模式下根据 `prefers-color-scheme` 自动切换（TC-UI-LIGHT-004）。
  - AC-5：浅色主题下所有文字对比度 ≥ WCAG AA 4.5:1（TC-UI-LIGHT-005）。
- 契约：CSS 变量 `[data-theme="light"]` 覆盖（~40 个变量）｜ `setTheme(theme)` / `getTheme()` 函数 ｜ IPC 命令 `set_theme` / `get_theme` ｜ settings `ui.theme`（`dark` / `light` / `system`）｜ 关联测试：TC-UI-LIGHT-001~005

### 4.5 LIC — 授权与 Freemium 付费墙

#### REQ-LIC-001 License 离线校验（Ed25519）
- 优先级：P0 ｜ 状态：Done ｜ 依赖：无
- 描述：采用 Ed25519 公私钥签名体系。用户输入 License Key（格式：`base64(payload)-base64(signature)`），后端使用内置公钥验证签名。有效则解锁 Pro，无效返回明确错误。验证状态持久化到 SQLite settings 表（key=`license.is_pro`），支持离线启动恢复。License 验证逻辑位于 `crates/core/src/license.rs`（纯计算，无 I/O 依赖，经架构评审准入 core）。
- 验收标准：
  - AC-1：使用合法私钥签名的 License，`verify_license` 返回 `Ok(())`（TC-LIC-001）。
  - AC-2：篡改签名后的 License，`verify_license` 返回 `Err`（TC-LIC-002）。
  - AC-3：激活后重启应用，`is_pro` 状态从 SQLite 恢复为 `true`（TC-LIC-005）。
  - AC-4：伪造 / 篡改的 License 被拒绝，错误消息不泄露校验细节（安全官要求）。
- 契约（TPM 冻结，2026-07-25）：Command `activate_pro(license_key: String, state: State<'_, AppState>) -> Result<bool, String>`；Command `get_pro_status(state: State<'_, AppState>) -> bool`。License 格式：`base64(payload)-base64(signature)`，payload 为签名的原始字节（如 `EchoMind-Pro-v1`），signature 为 64 字节 Ed25519 签名。
- 关联测试：TC-LIC-001、TC-LIC-002、TC-LIC-005

#### REQ-LIC-002 is_pro 状态管理与门控
- 优先级：P0 ｜ 状态：Done ｜ 依赖：REQ-LIC-001、REQ-ING-001
- 描述：所有 Tauri Command（如 `import_files`）在执行前必须检查 `is_pro`。免费版硬性限制：最多 50 个文件，禁止导入专业文档格式（PDF/DOCX/PPTX/EPUB/XLSX/CSV）。免费版支持个人知识管理格式：md/txt/rs/ts/tsx/py/go/html/htm。`is_pro` 状态存储于 `AppState::is_pro: tokio::sync::RwLock<bool>`，启动时从 SQLite settings 表读取；`import_files` 从 AppState 读取 `is_pro`（不再信任前端参数），实现前后端双重拦截。导入拦截错误消息以结构化前缀返回：`LIMIT_REACHED:`（配额触顶）与 `PRO_REQUIRED:`（专业格式付费门）。
  
  **Alpha 阶段策略（ALPHA_ALL_FEATURES_FREE = true）**：早期版本功能不稳定，所有 Pro 功能暂时免费开放。`ConfigStore::load_is_pro()` 在 alpha 阶段自动返回 `true`，跳过所有限制。后期稳定后改为 `false` 即恢复门控，无需其他代码变更。
- 验收标准：
  - AC-1：免费版已有 50 个文件时，调用 `import_files` 返回 `LIMIT_REACHED` 错误（TC-LIC-003）。
  - AC-2：免费版导入专业文档格式（pdf/docx/pptx/epub/xlsx/csv）时，返回 `PRO_REQUIRED` 错误（TC-LIC-004）。
  - AC-3：Pro 版不受 50 文件限制且可导入所有格式。
  - AC-4：`get_pro_status` 返回当前授权状态（bool）。
- 契约：`import_files` 签名移除 `is_pro` 参数，改为从 `state.is_pro` 读取（契约变更经 Phase 8 TPM 授权）。
- 关联测试：TC-LIC-003、TC-LIC-004

#### REQ-LIC-003 付费墙 UI
- 优先级：P0 ｜ 状态：Done ｜ 依赖：REQ-LIC-001、REQ-LIC-002
- 描述：当免费用户触发限制时（`import_files` 返回 `LIMIT_REACHED` 或 `PRO_REQUIRED` 错误），自动弹出全屏 Modal 引导激活。Modal 包含 License 输入框和「激活」按钮；激活成功后关闭 Modal 并隐藏所有限制提示；侧栏底部显示当前授权状态（免费版 / Pro）。
- 验收标准：
  - AC-1：收到 `LIMIT_REACHED` 或 `PRO_REQUIRED` 错误时，付费墙 Modal 自动弹出（E2E 断言）。
  - AC-2：在 Modal 中输入有效 License 并点击激活后，Modal 关闭，`get_pro_status` 返回 `true`。
  - AC-3：激活成功后侧栏授权状态更新为「Pro 版」。
  - AC-4：关闭付费墙不产生任何副作用，原有数据不受影响。
- 契约：前端监听 `import_files` 错误前缀触发 Modal；调用 `activate_pro` 激活；调用 `get_pro_status` 刷新 UI。
- 关联测试：E2E-LIC-001（L3-lite）

#### REQ-LIC-004 License 停用与状态展示
- 优先级：P2 ｜ 状态：Implemented ｜ 依赖：REQ-LIC-001
- 描述：设置页展示当前授权状态（免费 / Pro），支持在本机停用 License 以便转移。
- 验收标准：
  - AC-1：停用后回落为免费版规则；已入库的 50+ 文件保留可读，但新导入按免费版拦截。
- 契约（2026-07-25 冻结）：Command `deactivate_pro(state) -> Result<(), String>`；设置面板 `settingsLicenseInfo` 渲染授权状态 + 停用按钮。
- 关联测试：TC-LIC-006（L2 集成测试）

#### REQ-LIC-005 License 签发 CLI 工具
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-LIC-001
- 描述：当前 `license.rs` 仅包含客户端验证逻辑（`verify_license`），`BUILTIN_PUBKEY` 为开发环境密钥。正式发布前需要：(1) 生成生产环境 Ed25519 密钥对；(2) 用私钥签发 License Key；(3) 将生产公钥替换到客户端。本需求提供一个独立的 CLI 工具 `license-issuer`，作为 `crates/license-issuer` 独立二进制 crate，不编入应用二进制。功能包括：`genkey` 子命令生成密钥对（输出 base64 编码的公钥 + 私钥，私钥写入文件权限 0600），`issue` 子命令用私钥签发 License Key（格式 `base64(payload)-base64(signature)`，与 `license.rs::parse_license` 兼容）。工具复用 `ed25519-dalek` 的 `SigningKey` + `VerifyingKey`，与客户端 `verify_license` 形成闭环。
- 验收标准：
  - AC-1：`genkey` 子命令输出 base64 编码的 32 字节公钥和 32 字节私钥 seed，私钥文件权限 0600（TC-LIC-007）。
  - AC-2：`issue` 子命令用私钥对 payload 签名，输出 `base64(payload)-base64(signature)` 格式 License Key，客户端 `verify_license_with_key` 验证通过（TC-LIC-008）。
  - AC-3：篡改 payload 后签名验证失败（TC-LIC-008 旁证）。
  - AC-4：私钥文件不存在或格式非法时返回明确错误，不 Panic（TC-LIC-007 旁证）。
  - AC-5：`issue` 子命令支持自定义 payload（如 `EchoMind-Pro-v1`、`EchoMind-Pro-v2`），不同 payload 生成不同 License Key（TC-LIC-008 旁证）。
- 契约：`crates/license-issuer` 独立二进制 crate，子命令 `genkey --output <dir>` + `issue --payload <text> --key <privkey-file>` ｜ 关联测试：TC-LIC-007、TC-LIC-008
- 调研依据：`ed25519-dalek` v2.1（已在 `crates/core` 依赖中）的 `SigningKey::from_bytes` + `sign` API；Ed25519 RFC 8032 标准签名流程；`license-management.md` §1 架构概述中定义的「License 签发服务端」角色。本需求为纯 Rust CLI 工具，零网络依赖，私钥仅存在于签发环境。

### 4.6 SEC — 安全与隐私

#### REQ-SEC-001 API Key 本地加密存储
- 优先级：P0 ｜ 状态：Implemented ｜ 依赖：无
- 描述：BYOK API Key 仅本地存储：优先使用 OS Keychain（macOS Keychain / Windows DPAPI / Linux Secret Service），降级为 AES-256-GCM 加密文件。
- 验收标准：
  - AC-1：数据目录内任何文件不出现明文 Key（静态扫描断言）。
  - AC-2：日志、错误信息、toast 中不出现 Key 明文（脱敏断言）。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-SEC-002 渲染层 XSS 防御
- 优先级：P0 ｜ 状态：Implemented ｜ 依赖：REQ-UI-002
- 描述：所有经 Markdown 渲染的内容必须过 DOMPurify 白名单消毒；禁止 `eval` 与未消毒 `innerHTML`。
- 验收标准：
  - AC-1：OWASP 常见注入向量集（脚本标签、事件属性、`javascript:` 协议等）在渲染输出中全部失效。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-SEC-003 网络出口白名单
- 优先级：P0 ｜ 状态：Implemented ｜ 依赖：REQ-RAG-007
- 描述：应用零遥测；网络请求仅允许发往用户显式配置的 LLM / Embedding 端点。
- 验收标准：
  - AC-1：默认配置下（未配置端点）应用不发起任何外网请求（集成测试断言）。
  - AC-2：全部网络调用集中经 `crates/infra` 的 HTTP 适配器，core 无网络代码。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-SEC-004 本地数据目录隔离
- 优先级：P1 ｜ 状态：Done ｜ 依赖：无
- 描述：全部应用数据（文档副本、向量库、会话、配置）集中于系统应用数据目录，文件权限 0700。
- 验收标准：
  - AC-1：数据目录权限检查通过；卸载流程可完全清除数据。
- 契约：`SqliteStorage::ensure_dir_0700` + `ensure_file_0600`（Unix `#[cfg(unix)]`，Windows 跳过）+ `lib.rs` 启动时设置 data_dir 0700 ｜ 关联测试：TC-SEC-004
- 实现细节：数据目录 0700、secret.key 0600、全部数据文件在 data_dir 内、卸载删除 data_dir 即完全清除

#### REQ-SEC-005 路径遍历防护
- 优先级：P0 ｜ 状态：Done ｜ 依赖：REQ-ING-001
- 描述：导入文件路径必须经过安全校验：拒绝含 `..` 上级引用的路径（防目录遍历攻击）、`canonicalize` 必须成功且目标为普通文件（拒绝目录、设备文件、符号链接指向系统路径）。校验在任何文件读取之前执行。
- 验收标准：
  - AC-1：含 `../` 的路径被拒绝并返回可读错误（TC-ING-001b）。
  - AC-2：不存在的路径被拒绝并返回「文件不存在或不可读」。
  - AC-3：目录路径被拒绝并返回「目标不是普通文件」。
  - AC-4：设备文件路径（如 `/dev/null`）被拒绝。
- 契约：`ImportService::sanitize_path` — `Component::ParentDir` 检测 + `canonicalize` + `is_file` ｜ 关联测试：TC-ING-001b

#### REQ-SEC-006 文件格式白名单
- 优先级：P0 ｜ 状态：Done ｜ 依赖：REQ-ING-001
- 描述：导入文件扩展名必须通过白名单校验（`md` / `txt` / `pdf`），非白名单格式直接拒绝。扩展名大小写不敏感（统一转小写比较）。无扩展名文件拒绝。
- 验收标准：
  - AC-1：`.md` / `.txt` / `.pdf` 扩展名通过校验（大小写不敏感）。
  - AC-2：`.docx` / `.pptx` / `.html` / `.exe` 等非白名单扩展名被拒绝并返回支持格式列表。
  - AC-3：无扩展名文件被拒绝。
- 契约：`ImportService::validate_extension` — `ALLOWED_EXTENSIONS` 常量 ｜ 关联测试：TC-ING-002

#### REQ-SEC-007 Panic 安全保证
- 优先级：P0 ｜ 状态：Done ｜ 依赖：REQ-ARCH-000
- 描述：生产代码严禁使用 `unwrap()` / `expect()` / `panic!()` / `unreachable!()` / `todo!()` / `unimplemented!()`。所有可能 panic 的操作必须以 `Result` 优雅处理。CI 通过 `clippy --deny` 强制执行。测试代码通过 `#![allow(...)]` 豁免。任何非法输入（损坏 PDF、非法 UTF-8、空文件）不得导致 Panic。
- 验收标准：
  - AC-1：`cargo clippy --all-targets -- -D warnings` 零警告，且 `unwrap_used` / `expect_used` / `panic` / `unreachable` / `todo` / `unimplemented` 全部 deny。
  - AC-2：伪造 PDF 字节流输入不 Panic，返回 `Result::Err`（TC-PDF-001）。
  - AC-3：非法 UTF-8 字节流通过 `String::from_utf8_lossy` 容错解码，不 Panic。
  - AC-4：空文件导入返回可读错误，不 Panic。
- 契约：`Cargo.toml` `[workspace.lints.clippy]` 全 deny + `#[cfg(test)] mod tests { #![allow(...)] }` 豁免 ｜ 关联测试：TC-PDF-001 + 全量 clippy CI 门禁

#### REQ-SEC-008 供应链安全审计
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-ARCH-000
- 描述：使用 `cargo-deny` 对全部依赖进行供应链安全审计，覆盖四个维度：(1) RustSec 安全公告数据库漏洞扫描（`advisories`）；(2) 许可证合规性检查（仅允许 MIT 兼容许可证，拒绝 GPL / AGPL 等传染性许可证）；(3) Crate 禁用与重复版本控制（`bans`）；(4) 来源限制（仅允许 crates.io 官方源，禁止 git 依赖）。CI 集成于 `e2e.yml` 的 supply-chain job。
- 验收标准：
  - AC-1：`cargo deny check` 通过，无已知漏洞（RustSec advisory）。
  - AC-2：全部依赖许可证在白名单内（MIT / Apache-2.0 / BSD / ISC / MPL-2.0 / Zlib 等）。
  - AC-3：无被禁用的 crate；重复版本仅产生 warn（非 deny）。
  - AC-4：全部 crate 来源为 crates.io 官方源（`unknown-registry = "deny"` / `unknown-git = "deny"`）。
  - AC-5：yanked crate 被 deny。
- 契约：`deny.toml` 四维配置 + `.github/workflows/e2e.yml` supply-chain job ｜ 关联测试：CI supply-chain job

#### REQ-SEC-009 LLM 请求超时与边界保护
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-LLM-001
- 描述：LLM HTTP 请求设置连接超时（30s）与整体超时（5min），防止网络异常导致无限等待。SSE 流式解析对不完整片段进行字节级缓冲，多字节字符边界切割不丢字。非 2xx 响应精准捕获 HTTP 状态码并透传真实错误体。
- 验收标准：
  - AC-1：连接超时 30s，整体超时 5min，超时后返回可读错误。
  - AC-2：SSE 字节流被任意切断（含多字节字符边界）时解析器不崩溃、不丢字（TC-LLM-001）。
  - AC-3：401 / 400 等非 2xx 响应返回携带状态码与响应体摘要的 Err。
- 契约：`OpenAIProvider` — `reqwest::Client` timeout 配置 + `stream_parse.rs` 字节缓冲 ｜ 关联测试：TC-LLM-001

#### REQ-SEC-010 错误信息脱敏
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-SEC-001
- 描述：所有面向用户的错误信息（toast、对话框、日志）不得泄露敏感信息：API Key（脱敏为 `****` + 末 4 位）、用户文件绝对路径（仅显示文件名）、LLM 响应体中的认证信息。后端错误经 `sanitizeError` 前端函数二次过滤后才展示。
- 验收标准：
  - AC-1：`get_settings` 返回 `api_key_masked`，不返回明文 Key。
  - AC-2：错误 toast 中不出现完整 API Key（前端 `sanitizeError` 过滤）。
  - AC-3：错误信息中文件路径仅显示文件名，不显示绝对路径。
  - AC-4：日志中不出现 API Key 明文（REQ-OBS-001 AC-4 交叉验证）。
- 契约：`commands.rs::mask_api_key` + 前端 `sanitizeError()` ｜ 关联测试：TC-UI-002 + TC-SEC-004

#### REQ-SEC-011 用户数据不出域
- 优先级：P0 ｜ 状态：Done ｜ 依赖：REQ-SEC-003
- 描述：用户文档内容、对话记录、向量数据严格存储于本地，不发送到任何远程服务器。仅以下数据允许网络传输：(1) 用户主动配置的 LLM 端点（RAG 查询 prompt + 用户问题）；(2) 用户主动配置的 VLM 端点（PDF 图片理解）；(3) 用户主动配置的 Embedding 模型下载（首次使用从 HuggingFace 下载 ONNX 模型）。所有网络传输均由用户显式配置触发，无后台静默上传。
- 验收标准：
  - AC-1：文档原文不出域（仅检索到的 chunk 内容经 RAG prompt 发往 LLM）。
  - AC-2：对话记录仅存本地 SQLite，不上传。
  - AC-3：向量数据仅存本地 SQLite BLOB，不上传。
  - AC-4：OCR 完全本地（ocrs），不联网。
  - AC-5：PDF 渲染完全本地（pdfium），不联网。
  - AC-6：无任何后台遥测 / 崩溃报告 / 使用统计上传。
- 契约：`crates/infra` HTTP 适配器仅连接用户配置端点 + `crates/infra/src/ocrs_engine.rs` 本地 OCR + `crates/infra/src/local_embedder.rs` 本地嵌入 ｜ 关联测试：TC-SEC-003（网络白名单断言）

#### REQ-SEC-012 FTS5 查询注入防护
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-RAG-010
- 描述：FTS5 关键词检索查询必须经过转义处理，防止 FTS5 操作符注入（`*` / `OR` / `AND` / `NOT` / `NEAR` / `"` 等）。每个查询 token 包裹在双引号中转义后再组装为 OR 查询。
- 验收标准：
  - AC-1：含 FTS5 操作符的查询（如 `test* OR 1=1`）被转义为字面量搜索，不触发操作符语义。
  - AC-2：含双引号的查询被正确转义，不破坏 SQL 查询结构。
  - AC-3：空查询返回空结果，不触发全表扫描。
- 契约：`SqliteStorage::build_fts5_or_query` — 每 token 双引号包裹 ｜ 关联测试：TC-KEYWORD-001~006

#### REQ-SEC-013 数据库加密（SQLCipher AES-256）
- 优先级：P0 ｜ 状态：Done ｜ 依赖：REQ-DB-001、REQ-SEC-001
- 描述：用户可为数据库设置加密密码，启用 SQLCipher 透明 AES-256-CBC 加密。密码经 Argon2id KDF（m=19456KB, t=2, p=1）派生 32 字节密钥，`SqliteStorage::new_encrypted(key)` 通过 `PRAGMA key` 开启加密。密码以 `SecureString`（zeroize on drop）在内存中处理，使用后立即清零。提供 PBKDF2 降级路径（Argon2id 不可用时）。加密后数据库状态机为 `Unencrypted → EncryptedUnlocked`。加密操作不可逆（解密需用户提供密码）。
- 验收标准：
  - AC-1：设置密码后数据库文件以加密形式存储，SQLite 工具无法直接读取明文。
  - AC-2：密码经 Argon2id 派生密钥，`SecureString` 使用后内存清零。
  - AC-3：加密后 `get_security_status` 返回 `encrypted_unlocked` 状态。
- 契约：`encrypt_database(password) -> ()`；`SecurityManager.encrypt_database()`；`SqliteStorage::new_encrypted(key)` ｜ 关联测试：TC-SEC-013

#### REQ-SEC-014 PII 检测与脱敏
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-SEC-011
- 描述：对用户输入文本进行 PII（Personally Identifiable Information）自动检测与脱敏。支持 8 种 PII 类型：邮箱（email）、手机号（phone）、身份证号（idcard）、银行卡号（bankcard）、IP 地址（ip）、社会安全号（ssn）、护照号（passport）、国际电话号码（intl_phone）。`PiiRedactor` 使用正则表达式扫描文本，匹配项替换为 `[REDACTED-TYPE]` 占位符。检测结果返回结构化列表（类型、匹配值、位置），供前端展示。PII 检测可在设置中启用/禁用。
- 验收标准：
  - AC-1：含邮箱的文本经 `detect_pii` 返回 `{ type: "email", value: "...", start, end }` 检测结果。
  - AC-2：`redact_text` 将邮箱替换为 `[REDACTED-EMAIL]`。
  - AC-3：8 种 PII 类型均有对应正则覆盖，无误报。
- 契约：`detect_pii(text) -> { detections, redacted }`；`PiiRedactor::detect()` / `PiiRedactor::redact_text()` ｜ 关联测试：TC-SEC-014

#### REQ-SEC-015 审计日志哈希链
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-DB-001
- 描述：安全相关操作（PII 检测、加密/解锁、文档删除等）记录到 SQLite `audit_logs` 表，形成不可篡改的哈希链。每条审计日志包含：操作类型（action）、目标（target）、时间戳（timestamp）、前一条哈希（prev_hash）、当前条目哈希（entry_hash）。`entry_hash = SHA256(prev_hash || entry_data)`，链式哈希确保任何篡改都会导致后续所有条目校验失败。`verify_audit_chain()` 重放哈希链检测篡改。审计日志可通过 `get_audit_logs` 查询、`clear_audit_logs` 清空。
- 验收标准：
  - AC-1：安全操作后 `audit_logs` 表新增一条记录，`entry_hash` 正确计算。
  - AC-2：篡改任意一条审计日志后 `verify_audit_chain()` 返回 `false`。
  - AC-3：`get_audit_logs(limit)` 按时间倒序返回最近 N 条审计记录。
- 契约：`get_audit_logs(limit) -> Vec<AuditEntry>`；`clear_audit_logs() -> ()`；`AuditLogger` trait ｜ 关联测试：TC-SEC-015

#### REQ-SEC-016 自动锁定
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-SEC-013
- 描述：加密数据库在用户空闲超过阈值后自动锁定，状态从 `EncryptedUnlocked` 转为 `Locked`。锁定后需重新输入密码解锁。自动锁定超时时间可配置（默认 300 秒），通过 `set_auto_lock_config(enabled, timeout_secs, lock_on_sleep)` 设置。`record_activity()` 重置空闲计时器。支持系统休眠时自动锁定（`lock_on_sleep` 选项）。锁定状态通过 `get_security_status` 查询，前端显示锁定遮罩。系统唤醒锁（`system_wake_lock`）在活跃对话期间阻止系统休眠。
- 验收标准：
  - AC-1：空闲超过阈值后 `get_security_status` 返回 `locked` 状态。
  - AC-2：`record_activity()` 调用后空闲计时器重置。
  - AC-3：`lock_app()` 手动锁定后状态变为 `locked`。
- 契约：`set_auto_lock_config(enabled, timeout_secs, lock_on_sleep) -> ()`；`lock_app() -> ()`；`record_activity() -> ()`；`SecurityManager.check_auto_lock()` ｜ 关联测试：TC-SEC-016

#### REQ-SEC-017 暴力破解防护
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-SEC-013
- 描述：连续 5 次解锁密码错误后启用指数退避锁定（30s → 60s → 120s → …），防止暴力破解。锁定期间 `unlock_app` 命令直接拒绝并返回剩余锁定秒数。`get_security_status` 返回 `remaining_attempts`（剩余尝试次数）和 `remaining_lock_seconds`（剩余锁定秒数）。成功解锁后重置计数器。
- 验收标准：
  - AC-1：5 次错误密码后第 6 次尝试被拒绝，返回 `remaining_lock_seconds > 0`。
  - AC-2：锁定期间过后可再次尝试解锁。
  - AC-3：正确密码解锁后 `remaining_attempts` 重置为 5。
- 契约：`unlock_app() -> ()`；`SecurityManager.unlock()`；`get_security_status` 含 `remaining_attempts` / `remaining_lock_seconds` 字段 ｜ 关联测试：TC-SEC-017

#### REQ-SEC-018 剪贴板自动清除
- 优先级：P2 ｜ 状态：Done ｜ 依赖：REQ-SEC-011
- 描述：用户复制敏感内容到系统剪贴板后，应用在可配置超时后自动清除剪贴板（默认 30 秒）。通过 `set_clipboard_config(enabled, clear_after_secs)` 配置。清除操作在后台异步执行，不影响用户其他操作。此功能防止敏感信息残留在剪贴板中被其他应用读取。
- 验收标准：
  - AC-1：启用后复制内容到剪贴板，超时后剪贴板被清空。
  - AC-2：`set_clipboard_config(false, 0)` 禁用后不再自动清除。
- 契约：`set_clipboard_config(enabled, clear_after_secs) -> ()`；`SecurityManager` 剪贴板清除后台任务 ｜ 关联测试：TC-SEC-018

#### REQ-SEC-019 紧急销毁（Panic Wipe）
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-SEC-013
- 描述：用户可预设一个「紧急销毁密码」。当用户在紧急情况下使用此密码尝试解锁时，应用不仅不解锁，反而销毁全部本地数据（数据库、文档副本、密钥文件），使数据不可恢复。紧急销毁密码通过 `set_panic_wipe_password(password)` 设置，`clear_panic_wipe_password()` 清除，`is_panic_wipe_enabled()` 查询状态。此功能为高风险场景（律师、记者等）提供「宁可销毁不可泄露」的最后防线。
- 验收标准：
  - AC-1：设置紧急销毁密码后，使用该密码解锁触发数据销毁。
  - AC-2：`is_panic_wipe_enabled` 返回 `true` 表示已设置紧急密码。
  - AC-3：使用正常密码解锁不受影响。
- 契约：`set_panic_wipe_password(password) -> ()`；`clear_panic_wipe_password() -> ()`；`is_panic_wipe_enabled() -> bool`；`SecurityManager.set_panic_wipe_password()` ｜ 关联测试：TC-SEC-019

#### REQ-SEC-020 密码强度评估
- 优先级：P2 ｜ 状态：Done ｜ 依赖：REQ-SEC-013
- 描述：用户设置加密密码或紧急销毁密码时，实时评估密码强度并给出改进建议。`PasswordStrengthChecker::check(password)` 返回强度等级（weak / fair / good / strong）与百分比，`PasswordStrengthChecker::suggestions(password)` 返回改进建议列表（如「增加大写字母」「增加数字」「增加特殊字符」「长度建议 ≥12 位」）。强度评估基于长度、字符种类（大小写字母/数字/特殊字符）、常见密码字典检测。前端在密码输入框旁实时显示强度指示器。
- 验收标准：
  - AC-1：`check_password_strength("123")` 返回 `level: "weak"` 与低百分比。
  - AC-2：`check_password_strength("Str0ng!Pass2024")` 返回 `level: "strong"` 与高百分比。
  - AC-3：弱密码返回至少一条改进建议。
- 契约：`check_password_strength(password) -> { level, percentage, suggestions }`；`PasswordStrengthChecker::check()` / `suggestions()` ｜ 关联测试：TC-SEC-020

### 4.7 ARCH — 架构与工程基建

#### REQ-ARCH-000 Workspace 底座与核心领域契约
- 优先级：P0 ｜ 状态：Done ｜ 依赖：无
- 描述：搭建 Cargo workspace（crates/models、crates/core、crates/tauri-app，resolver "3"）；models 定义数据契约（DocStatus / Document / Chunk / RetrievalResult / ChatMessage）；core 以原生 `async fn` in trait 定义六边形端口（Loader / Splitter / Embedder / Storage / LLMProvider / Retriever）；tauri-app 提供可编译的启动骨架。
- 验收标准：
  - AC-1：`cargo check` 全 workspace 通过且 0 警告。
  - AC-2：`crates/core` 不依赖 Tauri / reqwest / rusqlite 等任何具体实现（依赖树审计）。
  - AC-3：全部端口 Trait 满足 `Send + Sync` 约束，未使用 `async-trait` 宏。
- 契约：不涉及 IPC（纯 crate 层）｜ 关联测试：CI 构建门禁（cargo check / clippy / 依赖树审计）

#### REQ-ARCH-001 六边形架构依赖方向
- 优先级：P0 ｜ 状态：Done ｜ 依赖：REQ-ARCH-000
- 描述：严格遵循六边形（端口与适配器）架构，依赖方向始终向内：`crates/models → crates/core → crates/infra → crates/tauri-app`。`crates/core` 不得依赖 Tauri、reqwest、rusqlite、fastembed 或任何具体实现 crate。`crates/models` 仅定义数据契约，不包含业务逻辑或外部依赖。`crates/infra` 实现全部端口 Trait，是唯一允许依赖具体技术（rusqlite / fastembed / reqwest）的层。`crates/tauri-app` 负责组装与 IPC，持有 AppState。
- 验收标准：
  - AC-1：`cargo tree -p echomind-core` 输出中不含 `tauri` / `reqwest` / `rusqlite` / `fastembed` / `ort` 等适配器 crate。
  - AC-2：`crates/models` 的 `Cargo.toml` 依赖列表为空（或仅 `serde` 序列化）。
  - AC-3：`crates/infra` 实现全部 core 端口 Trait（Loader / Splitter / Embedder / Storage / LLMProvider / Retriever）。
  - AC-4：`crates/tauri-app` 不直接调用 infra 的具体类型，仅通过 core 端口 Trait 交互。
- 契约：`Cargo.toml` workspace member 依赖关系 + CI 依赖树审计脚本 ｜ 关联测试：CI 依赖审计门禁

#### REQ-ARCH-002 原生 async trait（无宏）
- 优先级：P0 ｜ 状态：Done ｜ 依赖：REQ-ARCH-000
- 描述：全部端口 Trait 使用 Rust Edition 2024 原生 `async fn` in trait，不使用 `#[async_trait]` 宏。`#![allow(async_fn_in_trait)]` 为刻意豁免——全部 Trait 有 `Send + Sync` supertrait 约束，端口仅在 workspace 内消费。Workspace `resolver = "3"`，Rust 工具链 ≥1.85。
- 验收标准：
  - AC-1：`crates/core` 源码中不出现 `#[async_trait]` 属性（grep 断言）。
  - AC-2：全部端口 Trait 有 `: Send + Sync` supertrait 约束。
  - AC-3：`Cargo.toml` 中 `edition = "2024"` 且 `resolver = "3"`。
  - AC-4：`rust-toolchain` 版本 ≥ 1.85。
- 契约：`crates/core/src/lib.rs` 端口定义 + `Cargo.toml` edition/resolver ｜ 关联测试：CI clippy + grep 门禁

#### REQ-ARCH-003 Workspace Lint 全覆盖
- 优先级：P0 ｜ 状态：Done ｜ 依赖：REQ-ARCH-000
- 描述：Workspace 级 clippy lint 配置（`[workspace.lints.clippy]`），全部危险操作 lint 设为 `deny`：`unwrap_used`、`expect_used`、`panic`、`unreachable`、`todo`、`unimplemented`。各 crate 通过 `[lints] workspace = true` 继承。`commands.rs` 因 `#[tauri::command]` 宏生成代码含 `unreachable!()`，使用 `#![allow(clippy::unreachable)]` 文件级豁免（手写代码仍禁止使用，代码审查强制）。测试模块通过 `#![allow(...)]` 豁免。
- 验收标准：
  - AC-1：`cargo clippy --all-targets -- -D warnings` 零警告零错误。
  - AC-2：全部 workspace member 的 `Cargo.toml` 含 `[lints] workspace = true`。
  - AC-3：`commands.rs` 文件级豁免仅含 `unreachable`，不含其他 lint 豁免。
  - AC-4：测试模块豁免通过 `#![allow(clippy::unwrap_used, clippy::expect_used)]` 局限在 `#[cfg(test)] mod tests` 块内。
- 契约：`Cargo.toml` `[workspace.lints.clippy]` + 各 crate `[lints] workspace = true` ｜ 关联测试：CI clippy 门禁

#### REQ-ARCH-004 Dev 编译优化
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-ARCH-000
- 描述：Workspace `[profile.dev.package."*"] opt-level = 3`，确保 dev 模式下全部依赖 crate 以 opt-level 3 编译，避免 ML 依赖（fastembed / ort / tokenizers）在 opt-level 0 下运行极慢。首次编译耗时较长（~5-10 min），增量编译正常。主 crate（workspace member）保持 opt-level 0 以获得编译速度。
- 验收标准：
  - AC-1：`Cargo.toml` 含 `[profile.dev.package."*"] opt-level = 3`。
  - AC-2：dev 模式下 fastembed 嵌入推理耗时合理（单次 384 维嵌入 < 100ms）。
  - AC-3：增量编译耗时 ≤ 30s（单个文件修改后 `cargo build` 增量时间）。
- 契约：`Cargo.toml` `[profile.dev.package."*"]` 配置 ｜ 关联测试：手动验证 + CI 编译时间监控

#### REQ-ARCH-005 错误传播策略
- 优先级：P0 ｜ 状态：Done ｜ 依赖：REQ-ARCH-000
- 描述：统一使用 `anyhow::Result<T>` 作为业务错误类型，`anyhow::Context` 为错误添加上下文链。IPC 命令层将 `anyhow::Error` 转换为 `String`（`map_err(|e| format!("{e:#}"))`）返回前端。前端通过错误消息前缀分类处理（REQ-ERR-001）。不使用自定义 Error enum（避免维护成本），`anyhow` 的 `#[derive]` 链式上下文足以诊断。`bail!` / `anyhow!` 宏用于快速构造错误。
- 验收标准：
  - AC-1：全部业务函数返回 `anyhow::Result<T>`，不返回 `Option` 或裸 panic。
  - AC-2：关键操作（文件 IO / DB / 网络）使用 `.with_context(|| ...)` 添加可读上下文。
  - AC-3：IPC 命令返回 `Result<T, String>`，错误消息为 `format!("{e:#}")` 格式（含上下文链）。
  - AC-4：前端错误消息含足够上下文供用户理解（如「写入数据目录失败: /path/to/file」而非仅「IO 错误」）。
- 契约：`anyhow` crate + `.with_context()` 约定 + `map_err(|e| format!("{e:#}"))` ｜ 关联测试：全量测试中的错误路径断言

#### REQ-ARCH-006 IPC 命令 Inner 模式
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-ARCH-000
- 描述：每个 Tauri IPC 命令（`#[tauri::command]`）配有对应的 `*_inner` 函数，接收 `&AppState` 而非 `State<'_, AppState>`。命令函数仅做参数解包与 inner 调用转发，inner 函数承载全部业务逻辑。集成测试通过 mock `AppState` 直接调用 inner 函数，无需 Tauri 运行时。新增命令时必须同步新增 inner 函数。
- 验收标准：
  - AC-1：全部 IPC 命令有对应的 `*_inner` 函数。
  - AC-2：`*_inner` 函数签名接收 `&AppState`（或 `&Handle` + `&AppState`），不接收 Tauri `State` wrapper。
  - AC-3：集成测试目录（`tests/integration.rs`）中的测试通过 inner 函数复用命令逻辑。
  - AC-4：命令函数体仅含参数转发（`xxx_inner(&app, &params, state.inner()).await`），不含业务逻辑。
- 契约：`commands.rs` 全部 `#[tauri::command]` + `*_inner` 配对约定 ｜ 关联测试：`tests/integration.rs` 全量集成测试

#### REQ-ARCH-007 前端零 CDN 依赖
- 优先级：P0 ｜ 状态：Done ｜ 依赖：REQ-UI-001
- 描述：前端全部第三方库本地 vendored 于 `ui/vendor/` 目录，禁止从 CDN 加载任何资源。已 vendored 的库：Tailwind CSS（本地构建）、marked.js（Markdown 渲染）、DOMPurify（XSS 消毒）、highlight.js（代码高亮）、Mermaid（图表渲染）、KaTeX（数学公式）、Chart.js（数据图表）。前端为单文件 SPA（`ui/index.html`），不使用框架（vanilla JS）。所有资源通过相对路径引用，不依赖网络。
- 验收标准：
  - AC-1：`ui/index.html` 中不出现 `https://` 或 `http://` 外部资源引用（静态扫描断言）。
  - AC-2：全部第三方库文件位于 `ui/vendor/` 目录下。
  - AC-3：断网状态下前端全部功能正常（加载、渲染、交互）。
  - AC-4：`ui/vendor/MANIFEST.json` 列出全部 vendored 库的名称、版本、来源、许可证。
- 契约：`ui/vendor/` 目录 + `ui/index.html` 相对路径引用 + `MANIFEST.json` 清单 ｜ 关联测试：静态扫描 + E2E 断网测试

#### REQ-ARCH-008 commands.rs 模块化拆分（SRP 违例修复）
- 优先级：P2 ｜ 状态：Implemented ｜ 依赖：REQ-ARCH-006
- 描述：`crates/tauri-app/src/commands.rs`（7706 行）严重违反 SOLID 单一职责原则（铁律十四），按功能域拆分为 `commands/` 目录下 14 个子模块 + `mod.rs`。每个子模块 ≤ 1600 行，通过 `pub use` 重导出保持 `lib.rs` 的 `generate_handler!` 不变。零功能变更，零新增依赖。
- 验收标准：
  - AC-1：`commands.rs` 单文件不再存在，替换为 `commands/mod.rs` + 子模块目录。
  - AC-2：每个子模块行数 ≤ 1600（`wc -l` 断言）。
  - AC-3：`lib.rs` 中 `generate_handler!` 注册的 IPC 命令数不变（152 → 152）。
  - AC-4：全部 `*_inner` 函数签名不变（集成测试不受影响）。
  - AC-5：`cargo test`（Free）全量通过，`cargo test --features pro` 全量通过。
  - AC-6：`cargo clippy --all-targets -- -D warnings`（Free + Pro）零警告。
- 子模块清单（14 个）：`chat.rs`（1305 行）、`features.rs`（1559 行）、`local_llm.rs`（755 行）、`import.rs`（651 行）、`conversation.rs`（552 行）、`settings.rs`（549 行）、`performance.rs`（515 行）、`mod.rs`（490 行，共享类型/常量/辅助函数）、`graph.rs`（301 行）、`sync.rs`（222 行）、`document.rs`（217 行）、`audit.rs`（184 行）、`security.rs`（151 行）、`agent.rs`（100 行）、`license.rs`（54 行）。
- 契约：`crates/tauri-app/src/commands/` 目录结构 + `mod.rs` pub use 重导出 ｜ 关联测试：TC-ARCH-SPLIT-001~005

#### REQ-RAG-042 Durable Prompt Admission（持久化提示接纳）
- 优先级：P2 ｜ 状态：Implemented ｜ 依赖：REQ-DB-001
- 描述：借鉴 OpenCode 的 Prompt Admission / Promotion 机制，用户消息先「接纳」到 `pending_inputs` 表（持久化但未加入消息历史），在安全边界点（如流式生成完成后）再「提升」为正式消息。支持两种投递模式：`steer`（优先中断当前生成）和 `queue`（排队等待）。`promoted_seq` 为 `None` 表示未提升；非 `None` 表示已提升为正式消息。删除会话时级联清理 pending_inputs。
- 验收标准：
  - AC-1：`admit_input` 接纳输入后 `pending_inputs` 表有记录，`promoted_seq` 为 NULL（TC-ADMIT-001）。
  - AC-2：`promote_input` 提升后 `promoted_seq` 非 NULL，不再出现在待处理列表（TC-ADMIT-002）。
  - AC-3：`steer` 模式优先于 `queue` 模式（steer 排在前面）（TC-ADMIT-003）。
  - AC-4：多条排队消息按 FIFO 顺序（按创建时间排序）（TC-ADMIT-004）。
  - AC-5：删除会话时级联删除 pending_inputs（TC-ADMIT-005）。
- 契约：`pending_inputs` 表（id/conversation_id/content/delivery/created_at/promoted_seq）+ Storage trait 3 方法（admit_input/promote_input/get_pending_inputs）+ 3 IPC 命令 ｜ 关联测试：TC-ADMIT-001~005

#### REQ-ARCH-009 Session Run Coordinator（会话运行协调器）
- 优先级：P2 ｜ 状态：Implemented ｜ 依赖：REQ-ARCH-000
- 描述：借鉴 OpenCode `run-coordinator.ts`，新建 `SessionCoordinator` 模块管理活跃会话的执行状态。同一 `conversation_id` 的多次执行请求自动排队（串行），不同会话的执行互不阻塞（并发）。支持 `wake`（合并多次唤醒为一次后续执行）和 `interrupt`（标记停止状态，正在执行的闭包可检查并优雅退出）。使用 `Arc<Mutex<HashMap>>` 管理 `CoordEntry`（done/pending_wake/stopping 三字段），`Notify` 实现等待通知，`Box::pin` 解决 async 递归调用。
- 验收标准：
  - AC-1：同一会话的两次 run 串行执行（最大并发=1）（TC-COORD-001）。
  - AC-2：不同会话的 run 并发执行（最大并发≥2）（TC-COORD-002）。
  - AC-3：多次 wake 合并为一次重执行（TC-COORD-003）。
  - AC-4：interrupt 标记 stopping 状态，完成后清除（TC-COORD-004）。
  - AC-5：执行完成后从活跃列表移除（TC-COORD-005）。
- 契约：`crates/core/src/session_coordinator.rs`（`SessionCoordinator` + `CoordEntry`）+ `AppState.session_coordinator` 字段 ｜ 关联测试：TC-COORD-001~005

#### REQ-RAG-043 Tool Output Bounding（工具输出有界截断）
- 优先级：P2 ｜ 状态：Implemented ｜ 依赖：REQ-RAG-022
- 描述：借鉴 OpenCode `tool-output-store.ts`，Agent 工具调用产生的超大输出进行有界截断，保留开头和结尾，中间插入截断标记。双限制：行数 500 行 + 字节数 64KB，任一超标即截断。截断标记格式 `[... 输出已截断，共 N 行 M 字节 ...]`。纯函数实现，零 I/O、零 async、零依赖。集成到 `AgentEngine` 的 `search_kb` 和 `execute_code` 工具输出路径。
- 验收标准：
  - AC-1：短输出（行数 ≤ 500 且字节数 ≤ 64KB）不截断，`truncated = false`，`text` 等于原始输出（TC-BOUND-001）。
  - AC-2：超行数限制（> 500 行）截断，`truncated = true`，输出包含首行和末行（TC-BOUND-002）。
  - AC-3：超字节限制（> 64KB）截断，`truncated = true`，输出字节数小于原始字节数（TC-BOUND-003）。
  - AC-4：截断标记包含原始行数和字节数信息，格式为 `[... 输出已截断，共 N 行 M 字节 ...]`（TC-BOUND-004）。
  - AC-5：空输出安全处理，`truncated = false`，`text = ""`（TC-BOUND-005）。
- 契约：`crates/core/src/tool_output.rs`（`BoundedOutput` 结构体 + `bound_tool_output()` 函数 + `MAX_TOOL_OUTPUT_LINES` / `MAX_TOOL_OUTPUT_BYTES` 常量）+ `AgentEngine.execute_tool_single()` / `execute_code_tool()` 集成 ｜ 关联测试：TC-BOUND-001~005

#### REQ-RAG-044 Session Todo 持久化（会话待办持久化）
- 优先级：P2 ｜ 状态：Implemented ｜ 依赖：REQ-DB-001
- 描述：借鉴 OpenCode `session/todo.ts`，将 Agent 的 Todo 列表持久化到 SQLite `session_todos` 表，支持跨会话恢复。每个 Todo 包含内容、状态（pending/in_progress/completed）、优先级（low/medium/high）、位置排序。删除会话时级联清理 Todo。提供 IPC 命令供前端展示 Todo 进度面板。
- 验收标准：
  - AC-1：创建 Todo 后 `session_todos` 表有记录，默认状态为 `pending`（TC-TODO-001）。
  - AC-2：更新 Todo 状态 `pending → in_progress → completed` 成功（TC-TODO-002）。
  - AC-3：Todo 按 `position` 字段升序排列（TC-TODO-003）。
  - AC-4：删除会话时级联删除关联 Todo（TC-TODO-004）。
  - AC-5：跨会话（重新打开会话）可恢复 Todo 列表（TC-TODO-005）。
- 契约：`session_todos` 表（id/conversation_id/content/status/priority/position/created_at + ON DELETE CASCADE）+ `SessionTodo` 模型 + `TodoStatus` / `TodoPriority` 枚举 + Storage trait 5 方法 + 5 IPC 命令 ｜ 关联测试：TC-TODO-001~005

#### REQ-ARCH-010 Skill 系统发现（技能发现）
- 优先级：P2 ｜ 状态：Implemented ｜ 依赖：REQ-ARCH-000
- 描述：借鉴 OpenCode `skill.ts`，实现 Markdown 文件驱动的技能发现。从本地目录扫描 `.md` 文件，解析 YAML frontmatter 中的 `name` / `description` / `slash` 字段，提取 Markdown 正文作为技能内容。纯函数 `parse_skill()` 不依赖 I/O，可独立单元测试。`discover_from_dir()` 使用 `tokio::fs` 异步扫描目录。无新依赖（复用 serde / serde_json / tokio）。
- 验收标准：
  - AC-1：包含 YAML frontmatter 的 Markdown 文件正确解析出 `name` / `description` / `content`（TC-SKILL-001）。
  - AC-2：`slash: true` 字段正确解析为 `slash = true`（TC-SKILL-002）。
  - AC-3：无 frontmatter 的 Markdown 文件返回 `None`（TC-SKILL-003）。
  - AC-4：frontmatter 缺少 `name` 字段时返回 `None`（TC-SKILL-004）。
  - AC-5：`discover_from_dir()` 扫描目录中多个 `.md` 文件，返回已解析的 Skill 列表（TC-SKILL-005）。
- 契约：`crates/core/src/skill.rs`（`Skill` 结构体 + `parse_skill()` 纯函数 + `discover_from_dir()` 异步函数）｜ 关联测试：TC-SKILL-001~005

#### REQ-ARCH-011 Permission 细粒度控制（权限规则引擎）
- 优先级：P2 ｜ 状态：Implemented ｜ 依赖：REQ-ARCH-000
- 描述：借鉴 OpenCode `permission.ts`，实现 Wildcard 匹配的 RBAC 权限规则引擎。支持 `Allow` / `Deny` / `Ask` 三种效果，`*` 全匹配和前缀通配符匹配，last-writer-wins 规则优先级（从后向前查找第一个匹配规则），默认拒绝。纯函数实现，零 I/O、零 async，可独立单元测试。为 Agent 工具调用提供权限拦截层。
- 验收标准：
  - AC-1：通配符 `*` 匹配所有 action 和 resource（TC-PERM-001）。
  - AC-2：前缀通配符 `agent.*` 匹配 `agent.search_kb` / `agent.execute_code`（TC-PERM-002）。
  - AC-3：last-writer-wins — 多条规则匹配时，最后一条规则的效果生效（TC-PERM-003）。
  - AC-4：无匹配规则时默认返回 `Deny`（TC-PERM-004）。
  - AC-5：`Ask` 效果正确返回，供前端弹出用户确认对话框（TC-PERM-005）。
- 契约：`crates/core/src/permission.rs`（`PermissionRule` + `PermissionEffect` + `Ruleset` + `evaluate()` + `wildcard_match()`）｜ 关联测试：TC-PERM-001~005

#### REQ-ARCH-012 多协议 LLM 适配（LLM 协议类型定义）
- 优先级：P2 ｜ 状态：Implemented ｜ 依赖：REQ-LLM-001
- 描述：借鉴 OpenCode `packages/llm/`，定义 LLM 协议类型枚举和协议感知 Provider trait。支持 OpenAI Chat Completions（现有）、Anthropic Messages API（原生 Prompt Caching）、OpenAI Responses API、Google Gemini 四种协议类型。`LlmProtocol` 枚举和 `ProtocolAwareProvider` trait 是纯数据类型定义，不涉及具体 API 调用实现。
- 验收标准：
  - AC-1：`LlmProtocol` 枚举包含 4 种协议变体（OpenAIChat / OpenAIResponses / AnthropicMessages / Gemini）（TC-PROTO-001）。
  - AC-2：`LlmProtocol::from_str()` 从字符串正确解析协议类型（TC-PROTO-002）。
  - AC-3：`LlmProtocol::as_str()` 返回协议字符串标识（TC-PROTO-003）。
  - AC-4：未知协议字符串返回 `None`（TC-PROTO-004）。
- 契约：`crates/core/src/llm_protocol.rs`（`LlmProtocol` 枚举 + `ProtocolAwareProvider` trait）｜ 关联测试：TC-PROTO-001~004

### 4.8 DB — 数据持久化

#### REQ-DB-001 SQLite 持久化存储
- 优先级：P0 ｜ 状态：Done ｜ 依赖：REQ-ARCH-000
- 描述：以 rusqlite（bundled）+ r2d2 连接池实现 Storage 端口，数据库文件位于 Tauri `app_data_dir`；启用 WAL 模式与外键级联；启动时执行崩溃恢复，将上次会话遗留的 Processing 僵尸文档置为 Failed。批量事务写入将随 Phase 4 向量写入路径以端口扩展交付（本期单条写在 WAL 下达标）。
- 验收标准：
  - AC-1：写入文档 / 分块 / 向量后销毁实例并重新实例化，数据完整可读（TC-DB-001）。
  - AC-2：手动置入 Processing 状态文档，调用 `cleanup_zombies` 后状态变为 Failed 且返回清理条数（TC-DB-002）。
  - AC-3：连接启用 WAL（journal_mode）与 foreign_keys 级联（删除文档级联清理 chunks 与 embeddings，REQ-ING-005 前置）。
- 契约（TPM 冻结，2026-07-24）：Storage Trait 契约不变，仅新增 `async fn cleanup_zombies(&self) -> anyhow::Result<usize>`；AppState 移除 InMemoryStorage，注入 SqliteStorage。
- 关联测试：TC-DB-001、TC-DB-002

#### REQ-DB-002 数据库 Schema 版本管理
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-DB-001
- 描述：SQLite 数据库使用 Schema 版本管理（`PRAGMA user_version`），启动时检查版本号并执行增量迁移。初始版本为 1，后续每次 schema 变更递增版本号并编写迁移 SQL。迁移在连接初始化时自动执行（`SqliteStorage::new`），迁移失败返回 Err 阻止启动。FTS5 虚拟表（`chunks_fts`）通过迁移脚本创建与回填。
- 验收标准：
  - AC-1：新建数据库 `PRAGMA user_version` 为当前最新版本号。
  - AC-2：旧版本数据库启动时自动执行迁移，迁移后 `user_version` 更新为最新。
  - AC-3：迁移失败时 `SqliteStorage::new` 返回 Err，应用不启动。
  - AC-4：FTS5 虚拟表在迁移中创建，旧数据自动回填。
- 契约：`SqliteStorage::run_migrations` — `PRAGMA user_version` 检查 + 增量 DDL ｜ 关联测试：TC-DB-001（重建验证）

#### REQ-DB-003 r2d2 连接池管理
- 优先级：P0 ｜ 状态：Done ｜ 依赖：REQ-DB-001
- 描述：使用 `r2d2` + `r2d2_sqlite` 连接池管理 SQLite 连接，池配置：最大连接数 1（SQLite 单写者模型，WAL 模式下并发读安全）、连接超时 30s、最小空闲 1。连接池在 `SqliteStorage::new` 时初始化，全部 DB 操作通过 `pool.get()` 获取连接。连接初始化时执行 `PRAGMA journal_mode=WAL` + `PRAGMA foreign_keys=ON` + `PRAGMA busy_timeout=5000`。
- 验收标准：
  - AC-1：`SqliteStorage` 内部持有 `r2d2::Pool<SqliteConnectionManager>`。
  - AC-2：连接池最大连接数为 1（SQLite 单写者约束）。
  - AC-3：每个连接初始化时执行 WAL + foreign_keys + busy_timeout PRAGMA。
  - AC-4：连接池耗尽时等待 30s 后返回 Err（而非立即 panic）。
- 契约：`SqliteStorage::new` — `r2d2::Pool::builder()` 配置 ｜ 关联测试：TC-DB-001（并发写入验证）

#### REQ-DB-004 异步数据库操作
- 优先级：P0 ｜ 状态：Done ｜ 依赖：REQ-DB-001
- 描述：全部数据库操作通过 `tokio::task::spawn_blocking` 在阻塞线程池执行，避免阻塞 async 运行时。`run_db` 辅助函数封装 `spawn_blocking` 模式：接收闭包，在阻塞线程执行，返回 `anyhow::Result<T>`。Storage Trait 的全部 `async fn` 方法内部通过 `run_db` 调用同步 rusqlite 代码。
- 验收标准：
  - AC-1：全部 Storage Trait 方法内部使用 `spawn_blocking`（源码审计）。
  - AC-2：数据库操作不阻塞 tokio async 运行时（长查询期间 UI 仍响应）。
  - AC-3：`run_db` 返回 `anyhow::Result<T>`，错误含上下文。
- 契约：`SqliteStorage` — `run_db<T>(f: impl FnOnce() -> Result<T> + Send + 'static)` ｜ 关联测试：TC-DB-001/002

#### REQ-DB-005 向量 BLOB 存储格式
- 优先级：P0 ｜ 状态：Done ｜ 依赖：REQ-DB-001
- 描述：向量数据以原始 `f32` 字节序列存储为 SQLite BLOB（`embeddings.vector BLOB`）。存储时 `vec.iter().flat_map(|f| f.to_le_bytes()).collect::<Vec<u8>>()`，读取时按 4 字节步长反序列化为 `Vec<f32>`。384 维向量占用 1536 字节存储。Cosine 相似度在 Rust 中计算（全表扫描加载后逐对计算），不在 SQLite 中计算。
- 验收标准：
  - AC-1：`add_embedding` 将 `&[f32]` 序列化为 little-endian BLOB 写入。
  - AC-2：`vector_search` 从 BLOB 反序列化为 `Vec<f32>`，cosine 相似度计算正确。
  - AC-3：384 维向量 BLOB 大小为 1536 字节（384 × 4）。
  - AC-4：空向量（dim=0）不导致 panic，返回 Err。
- 契约：`SqliteStorage::add_embedding` / `vector_search` — `f32::to_le_bytes` / `from_le_bytes` ｜ 关联测试：TC-DB-001 + TC-NFR-002

#### REQ-DB-006 FTS5 全文检索索引
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-DB-002、REQ-RAG-010
- 描述：SQLite FTS5 虚拟表 `chunks_fts` 以 trigram 分词器索引全部 chunk 文本，支持中英文子串匹配。FTS5 表通过外键关联 `chunks` 表，文档删除时级联清理 FTS5 索引。`keyword_search` 方法接受查询字符串，经 `build_fts5_or_query` 转义后执行 BM25 排序检索，返回 top-k 结果。
- 验收标准：
  - AC-1：`chunks_fts` 虚拟表存在且使用 trigram 分词器。
  - AC-2：添加 chunk 后 FTS5 索引自动更新。
  - AC-3：删除文档后 FTS5 索引级联清理（无孤儿索引）。
  - AC-4：`keyword_search` 返回 BM25 排序的 top-k 结果。
  - AC-5：FTS5 查询注入防护生效（REQ-SEC-012）。
- 契约：`SqliteStorage::keyword_search` + `build_fts5_or_query` + `chunks_fts` 虚拟表 ｜ 关联测试：TC-KEYWORD-001~009

#### REQ-DB-007 设置键值存储与加密
- 优先级：P0 ｜ 状态：Done ｜ 依赖：REQ-DB-001、REQ-SEC-001
- 描述：SQLite `settings` 表为键值存储（`key TEXT PRIMARY KEY, value TEXT`），存储全部应用配置（LLM config、VLM 开关、混合检索开关、License 状态、窗口位置等）。敏感值（API Key）使用 AES-256-GCM 加密后存储，密钥派生自数据目录下 `secret.key` 文件（首次启动随机生成）。`set_setting` / `get_setting` 为通用读写接口，`get_setting_decrypted` / `set_setting_encrypted` 为加密读写接口。
- 验收标准：
  - AC-1：`settings` 表存在且为 `key TEXT PRIMARY KEY, value TEXT` 结构。
  - AC-2：`set_setting` / `get_setting` 通用接口可读写任意键值。
  - AC-3：API Key 经 AES-256-GCM 加密后存储，`settings` 表中不出现明文 Key。
  - AC-4：`secret.key` 文件权限 0600（Unix），首次启动自动生成。
  - AC-5：加密设置项解密失败时返回 Err（密钥不匹配或数据损坏）。
- 契约：`SqliteStorage::set_setting` / `get_setting` + `EncryptedStore` — AES-256-GCM + `secret.key` ｜ 关联测试：TC-SEC-004 + TC-DB-001

### 4.9 PDF — PDF 文档解析

#### REQ-PDF-001 PDF 文档解析
- 优先级：P0 ｜ 状态：Done ｜ 依赖：REQ-ING-002
- 描述：基于 lopdf 实现 PdfLoader，逐页提取文本并以 `String::from_utf8_lossy` 容错解码；单页失败跳过并记录，整体失败优雅返回 Err，任何非法输入不得导致 Panic。
- 验收标准：
  - AC-1：伪造 PDF 字节流输入不 Panic，返回 Result（Err 时携带可读原因）（TC-PDF-001）。
  - AC-2：Pro 用户导入 PDF 后索引链路可完整走通（load → split → chunks 入库，打通 REQ-ING-002-AC-2）。
- 契约：复用 Loader 端口，无 IPC 变更。
- 关联测试：TC-PDF-001

### 4.10 LLM — 大模型接入

#### REQ-LLM-001 OpenAI 兼容流式对话
- 优先级：P0 ｜ 状态：Done ｜ 依赖：REQ-RAG-001
- 描述：基于 reqwest 调用 OpenAI 兼容端点（`POST {base_url}/v1/chat/completions`，stream=true）；SSE 流式解析（按 `\n\n` 切分事件，不完整片段字节级缓冲，`[DONE]` 结束）；精准捕获 HTTP 状态码并透传真实错误体（401/400 等）；支持多轮上下文（history 注入 messages）；base_url 规范化去尾斜杠（防双斜杠）；空 API Key 时不携带 Authorization 头（兼容本地 Ollama）。
- 验收标准：
  - AC-1：SSE 字节流被任意切断（含多字节字符边界）时解析器不崩溃、不丢字（TC-LLM-001）。
  - AC-2：非 2xx 响应返回携带状态码与响应体摘要的 Err，不 Panic。
  - AC-3：网络代码全部位于 infra，core 无 reqwest 依赖（依赖树审计）。
- 契约：LLMProvider 端口契约不变；适配器 `echomind_infra::openai_provider::OpenAIProvider`。
- 关联测试：TC-LLM-001

#### REQ-LLM-002 LLM 连接测试
- 优先级：P0 ｜ 状态：Done ｜ 依赖：REQ-LLM-001
- 描述：提供 test_llm_connection 命令，发送极简请求（max_tokens=1）验证 API Key 与 URL 有效性，返回可读结果。
- 验收标准：
  - AC-1：有效配置返回成功文案；无效 Key / URL / 模型返回真实 HTTP 错误信息。
- 契约（TPM 冻结，2026-07-24）：Command `test_llm_connection(api_key: String, base_url: String, model: String) -> Result<String, String>`。
- 关联测试：手测 + 设置界面 E2E（wiremock L2 用例随 CI 阶段补齐）

#### REQ-LLM-003 本地 LLM 推理引擎
- 优先级：P1 ｜ 状态：Implemented（Phase 2 完成） ｜ 依赖：REQ-LLM-001
- 描述：集成 mistral.rs v0.9.0 纯 Rust LLM 推理引擎，支持加载本地 GGUF 量化模型，在 CPU/GPU 上执行推理，通过 `LLMProvider` trait 与现有 RAG 管线无缝集成。用户可在设置中切换「远程 API」和「本地推理」两种模式。本地推理模式下零网络请求，查询内容不离开本机，满足高隐私场景（律师、研究人员）需求。mistral.rs 依赖通过 Pro feature 门控（`#[cfg(feature = "pro")]`），Free 版本不含本地推理依赖。
- Phase 2 增强：mistral.rs v0.8.1→v0.9.0 升级、GPU 加速（Metal/CUDA）、PagedAttention KV cache 管理、采样参数配置（temperature/top_p/top_k/max_tokens/penalty）、模型卸载/切换（OnceCell→RwLock<Option>）、流取消集成（CancellationToken）、模型预热（warm_up 后台加载消除首次对话卡顿）、extract_text 错误处理加固（match 全变体处理）。
- 验收标准：
  - AC-1：设置面板提供推理模式切换（远程 API / 本地推理），选择本地推理后 `chat` 命令使用 `LocalLlmEngine` 进行推理。
  - AC-2：本地推理支持 token 级流式输出，与远程 API 行为一致（`chat_token` 事件）。
  - AC-3：首次加载模型时显示加载状态，不阻塞 UI。
  - AC-4：`set_llm_mode` 命令切换 Remote/Local 模式，持久化到 settings 表 `llm.mode`。切换到 Local 模式后后台预热模型（S14）。
  - AC-5：模型加载失败时返回可读错误信息，不 Panic。`extract_text` 使用 match 全变体处理，防御性返回 None 而非 panic（S14）。
  - AC-6：Free 用户切换到 Local 模式时被拦截并提示升级 Pro。
  - AC-7（Phase 2）：`get_local_llm_device_kind` 命令返回当前推理设备类型（cpu/metal/cuda）。
  - AC-8（Phase 2）：`set_paged_attn` 命令配置 PagedAttention KV cache 管理（block_size/gpu_memory_ctx）。
  - AC-9（Phase 2）：`set_sampling_params` 命令配置采样参数（temperature/top_p/top_k/max_tokens/frequency_penalty/presence_penalty），运行时即时生效，无需重新加载模型。
  - AC-10（Phase 2）：`abort()` 方法中断推理流，已生成内容保留（CancellationToken 集成）。
  - AC-11（Phase 2）：`warm_up()` 方法后台预热模型，模式切换/模型选择后自动触发，消除首次对话卡顿。
- 契约：`LLMProvider` trait 不变；新增 `LocalLlmEngine` 适配器（`crates/infra/src/local_llm.rs`，Pro 门控）；新增 `LlmMode` 枚举（Remote/Local）、`LlmSamplingParams` 结构体；新增方法 `device_kind()` / `with_paged_attn()` / `sampling_params()` / `set_sampling_params()` / `unload()` / `abort()` / `warm_up()`。
- 关联测试：TC-LLM-LOCAL-001~008、TC-LLM-UPGRADE-001~003、TC-LLM-GPU-001~003、TC-LLM-PAGED-001~003、TC-LLM-SAMPLE-001~004、TC-LLM-UNLOAD-001~004、TC-LLM-ABORT-001~003、TC-LLM-WARM-001~003、TC-LLM-PERF-001~003

#### REQ-LLM-004 本地模型管理
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-LLM-003
- 描述：提供本地 GGUF 模型文件的管理功能：下载（从 HuggingFace）、列表、删除、选择。模型文件存储在 `{data_dir}/models/llm/` 目录下。下载时通过 `model_download_progress` 事件推送实时进度（已下载字节、总字节、速度），前端渲染进度条。支持从推荐模型列表一键下载（Qwen2.5 / Llama / Phi 系列）。
- 验收标准：
  - AC-1：`list_local_models` 命令返回已下载模型列表（文件名、大小、推断架构、参数量、量化格式）。
  - AC-2：`download_model` 命令从指定 URL 下载 GGUF 文件，通过 `model_download_progress` 事件推送进度。
  - AC-3：`delete_model` 命令删除指定模型文件，含路径穿越防护（拒绝 `..` / `/` / `\`）。
  - AC-4：`set_local_model` 命令设置当前选中的本地模型，持久化到 settings 表 `llm.local_model`。
  - AC-5：推荐模型列表（4 个模型：Qwen2.5-3B/7B、Llama-3.2-3B、Phi-3.5-mini），前端一键下载。
  - AC-6：前端设置面板渲染已下载模型列表和推荐模型列表，含下载进度条。
- 契约：`list_local_models() -> Vec<ModelInfo>` / `download_model(url: String, filename: String) -> ()` / `delete_model(filename: String) -> ()` / `set_llm_mode(mode: String) -> ()` / `get_llm_mode() -> String` / `set_local_model(filename: String) -> ()` / `get_recommended_models() -> Vec<RecommendedModel>`
- 关联测试：TC-MGR-001~020

#### REQ-LLM-005 GPU 加速推理（Phase 2）
- 优先级：P2 ｜ 状态：Implemented ｜ 依赖：REQ-LLM-003
- 描述：通过 mistral.rs v0.9.0 的 `best_device()` API 自动选择最优推理设备（CUDA → Metal → CPU），在支持的 GPU 上执行推理，显著提升 tokens/sec 并降低首 token 延迟。GPU 加速通过 Cargo feature flags（`metal` / `cuda` / `accelerate`）控制，编译时决定，无需运行时切换。`LocalLlmEngine::detect_device_kind()` 在 `new()` 时探测设备类型，前端通过 `get_local_llm_device_kind` 命令显示当前设备。
- 验收标准：
  - AC-1：`metal` feature 启用时（macOS），`best_device(false)` 返回 Metal 设备，`device_kind()` 返回 `"metal"`。
  - AC-2：`cuda` feature 启用时（NVIDIA），`best_device(false)` 返回 CUDA 设备，`device_kind()` 返回 `"cuda"`。
  - AC-3：无 GPU feature 时，`device_kind()` 固定返回 `"cpu"`，使用 `with_force_cpu()` 路径。
  - AC-4：`get_local_llm_device_kind` 命令返回当前设备类型字符串，前端用于展示。
  - AC-5：`best_device(true)` 始终返回 CPU（安全回退路径）。
- 契约：`LocalLlmEngine::device_kind() -> &str` / `LocalLlmEngine::detect_device_kind() -> String`（内部方法） / IPC `get_local_llm_device_kind() -> String`
- 关联测试：TC-LLM-GPU-001~003、TC-LLM-UPGRADE-003

#### REQ-LLM-006 采样参数配置（Phase 2）
- 优先级：P2 ｜ 状态：Implemented ｜ 依赖：REQ-LLM-003
- 描述：提供本地推理的采样参数运行时配置能力，包括 temperature、top_p、top_k、max_tokens、frequency_penalty、presence_penalty。参数通过 `set_sampling_params` IPC 命令持久化到 settings 表 `llm.sampling` 键（JSON 序列化），引擎加载时自动读取并应用。运行时修改参数即时生效，无需重新加载模型。各字段为 `Option`，`None` 表示使用引擎默认值（不向 RequestBuilder 注入该参数）。
- 验收标准：
  - AC-1：`set_sampling_params` 命令接受 `LlmSamplingParams` 结构体，参数范围校验（temperature 0.0~2.0、top_p 0.0~1.0、top_k 1~100、max_tokens >0、frequency_penalty -2.0~2.0、presence_penalty -2.0~2.0）。
  - AC-2：采样参数 JSON 序列化后持久化到 settings 表 `llm.sampling` 键。
  - AC-3：引擎加载时从 settings 表读取采样参数并应用到 `RequestBuilder`（mistral.rs v0.9.0 `set_sampler_*` API）。
  - AC-4：运行时 `set_sampling_params` 即时更新引擎内部采样参数，下次 `chat_stream` 生效。
  - AC-5：`None` 字段不向 `RequestBuilder` 注入参数（使用引擎默认值）。
  - AC-6：`SettingsPayload.llm_sampling` 字段通过 `get_settings` 命令返回当前采样参数。
- 契约：`LlmSamplingParams` 结构体（`crates/models/src/lib.rs`）/ `LocalLlmEngine::sampling_params() -> LlmSamplingParams` / `LocalLlmEngine::set_sampling_params(params)` / IPC `set_sampling_params(params: LlmSamplingParams) -> ()`
- 关联测试：TC-LLM-SAMPLE-001~004、TC-MODEL-LLM-005~008

#### REQ-LLM-007 自研量化推理内核（Phase 3）
- 优先级：P2 ｜ 状态：Implemented ｜ 依赖：REQ-LLM-003
- 描述：自研 GEMV（量化矩阵-向量乘法）内核，支持 Q4_0/Q4_K/Q8_0/Q8_K 四种 GGML 量化格式。包含 GGUF 文件解析器（mmap 零拷贝）、量化块结构、GEMV 内核（标量 + AVX2 预留）、CPU cache 友好权重重排（Row-Major → Tile-Major）和内核模式运行时切换。通过 `set_kernel_mode` / `get_kernel_mode` IPC 命令在 mistral.rs 内核与自研内核间切换，持久化到 settings 表 `llm.kernel_mode` 键。
- 验收标准：
  - AC-1：`GgufFile::open()` 通过 mmap 解析 GGUF v3 文件，提取元数据和张量信息（零拷贝）。
  - AC-2：`gemv_dispatch()` 支持 Q4_0/Q4_K/Q8_0/Q8_K 四种量化格式的矩阵-向量乘法。
  - AC-3：`repack_for_gemv()` 将权重从 Row-Major 重排为 Tile-Major 布局，`gemv_repacked_dispatch()` 使用重排权重执行 cache 友好 GEMV。
  - AC-4：`set_kernel_mode` / `get_kernel_mode` IPC 命令在 `"mistral"` / `"custom"` 间切换内核模式，持久化到 settings 表。
  - AC-5：自研内核与朴素 f32 参考实现的数值误差 < 2.0（Q8_0/Q4_K）。
- 契约：`GgufFile` / `GgmlDType` / `QuantBlock` / `gemv_dispatch` / `repack_for_gemv` / `gemv_repacked_dispatch` / `KernelMode` / IPC `set_kernel_mode(mode: String) -> ()` / `get_kernel_mode() -> String`
- 关联测试：TC-GGUF-001~014、TC-QUANT-001~014、TC-GEMV-001~014、TC-REPACK-001~006、TC-INTEG-001~006、TC-PHASE3-PERF-001~004

#### REQ-LLM-008 内存层次流式加载（Phase 3）
- 优先级：P2 ｜ 状态：Implemented ｜ 依赖：REQ-LLM-003
- 描述：在模型超过可用 RAM 时自动管理内存，通过 mmap 按需分页 + `madvise(MADV_WILLNEED)` 预取 + LRU 驱逐实现流式加载。Layer 级预取器在推理进行时后台预取下一层权重到 page cache，RAM 预算管理器跟踪内存使用量并在空间不足时 LRU 驱逐旧层。
- 验收标准：
  - AC-1：`LayerPrefetcher` 通过 `madvise(MADV_WILLNEED)` 预取指定层权重到 page cache（非阻塞）。
  - AC-2：`prefetch_range()` 在后台线程批量预取连续 N 层，层间 1ms 间隔避免 I/O 风暴。
  - AC-3：`MemoryBudget` 跟踪内存使用量，`allocate()` 分配预算（自动 LRU 驱逐），`release()` 释放，`touch()` 标记最近使用。
  - AC-4：`MemoryBudget::from_system(percentage)` 基于系统 RAM 百分比创建预算（使用 `libc::sysconf`，不依赖 sysinfo crate）。
  - AC-5：空间充足时 `evict_lru()` 不驱逐任何层（O(1) 快路径）。
- 契约：`LayerPrefetcher` / `layer_offsets_from_gguf` / `MemoryBudget` / `MemoryStats` / `get_system_memory`
- 关联测试：TC-PREFETCH-001~006、TC-MEM-001~013、TC-PHASE3-PERF-003

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
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-MM-003、REQ-MM-004
- 描述：当前 VLM 图片理解使用单一提示词（`VLM_IMAGE_PROMPT`），对所有图片内容一视同仁：表格→Markdown、甘特图→Mermaid、流程图→Mermaid、**其他图表→简洁文字描述**。对于学术论文中的科学数据图表（散点图、柱状图、折线图、热力图等），「简洁文字描述」仅输出趋势概括（如"显示正相关关系"），丢失原始数据点，用户无法通过 RAG 对话核对图表中的具体数值。本需求将 VLM 提示词升级为**分级理解策略**：按内容类型分四个精度等级处理——Level 1 表格（完整 Markdown 表格）、Level 2 流程图/甘特图（Mermaid 语法）、Level 3 数据图表（先判断图表类型，再提取坐标轴标签/数据系列名称/关键数值点以 CSV 格式输出，附带趋势描述）、Level 4 示意图/照片（简洁文字描述）。Level 3 是核心改进：从"描述图表"升级为"提取图表数据"，提取的 CSV 数据进入分块→嵌入→存储链路后，用户可通过 RAG 对话查询图表中的具体数值。因 VLM 对密集数据图表的提取精度有限（存在幻觉风险），提取结果须标注「以下数据由 AI 视觉提取，可能存在误差，请核对原文」。
- 验收标准：
  - AC-1：含数据图表（如柱状图）的 PDF 页面经 VLM 处理后，chunk 中包含 CSV 格式的数据点（坐标轴标签 + 至少 3 个数值点），而非仅文字趋势描述（TC-MM-007c）。
  - AC-2：含数学公式的 PDF 页面经 VLM 处理后，chunk 中包含 LaTeX 格式的公式文本（`$...$` 或 `$$...$$`），而非公式截图的 OCR 乱码（TC-MM-007）。
  - AC-3：VLM 提取的数据图表结果末尾标注「AI 视觉提取，可能存在误差，请核对原文」提示（TC-MM-007 旁证）。
  - AC-4：VLM 不可用（未配置或关闭）时退化为现有 OCR 行为，不崩溃（TC-MM-003c 旁证）。
  - AC-5：VLM 提取的 CSV 数据经分块和向量化后，可通过 RAG 对话检索到（如提问"图5的X轴标签是什么"能命中对应 chunk，TC-MM-007c 旁证）。
- 契约：§5.8 `vlm_prompt::VLM_TIERED_PROMPT` ｜ 关联测试：TC-MM-007/007a/007b/007c
- 调研依据：Salesforce AI Research CogAlign（VLM 图表理解增强，2025）；ChartHal（ICLR 2026 投稿，评估 VLM 在科学图表理解中的幻觉问题）；Marker（⭐37,816，PDF→Markdown 高精度转换，支持公式 LaTeX 还原）；Nougat（Meta，⭐10,050，学术 PDF 公式提取）。本需求不引入新依赖，将原 `loader.rs` 与 `import.rs` 中重复的 `VLM_IMAGE_PROMPT` 常量合并为集中式 `vlm_prompt::VLM_TIERED_PROMPT`（§5.8），升级为 4 级分级策略，纯 `crates/core` 层改动。Pro 版功能。

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
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-RAG-003、REQ-DB-001
- 描述：当前 `chat` 命令走 query → embedding → top-k 检索路径，无法胜任全文矛盾检测。本需求新增独立的审计命令 `audit_document(doc_id)`，走完全不同的数据路径：通过 `Storage.list_chunks(doc_id)` 获取文档全部 chunk（全量扫描），不走向量检索。审计流程分为三个阶段（对应 Decompose-Then-Verify 范式）：(1) **Decompose**——逐 chunk 提取原子声明；(2) **Verify**——对跨 chunk 的声明对做矛盾判定；(3) **Report**——聚合发现为结构化报告。审计结果以流式事件推送前端，渲染为 Block 形式的勘误报告。审计过程支持取消（CancellationToken，复用 REQ-RAG-005 的 abort 模式）。Pro 版功能。
- 验收标准：
  - AC-1：调用 `audit_document(doc_id)` 后，系统通过 `list_chunks` 获取该文档全部 chunk，而非走 `vector_search` top-k 路径（TC-AUDIT-001）。
  - AC-2：审计过程分三个阶段推送 `audit_phase` 事件（`extracting` / `comparing` / `reporting`），前端显示当前阶段（TC-AUDIT-001 旁证）。
  - AC-3：审计过程中可通过 `abort_audit` 取消，取消后已提取的声明和已发现的矛盾保留并输出部分报告（TC-AUDIT-001 旁证）。
  - AC-4：空文档（0 chunk）或不存在文档返回明确错误，不 Panic（TC-AUDIT-001 旁证）。
- 契约：§5.8 `AuditEngine::audit` + `commands::audit_document` ｜ 关联测试：TC-AUDIT-001/001b
- 调研依据：LangChain `MapReduceDocumentsChain`（全量扫描范式）；Google PAT（Inference Scaling 多轮推理）；FIND 基准（文档内部不一致性检测可行性验证）。本需求不引入新依赖，复用 `Storage` + `LLMProvider` + `Embedder` 端口。

#### REQ-AUDIT-002 原子声明提取（Decompose Phase）
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-AUDIT-001
- 描述：对文档的每个 chunk，调用 LLM 提取**可验证的原子声明**（Atomic Claims）。声明是文档中做出的可被事实检验的断言，包括但不限于：数值参数（"实验温度为 25°C"）、定义声明（"X 定义为 Y 的函数"）、因果声明（"A 导致 B"）、结论声明（"结果表明误差小于 5%"）。每个声明结构化为 `{claim_id, chunk_id, sequence, text, claim_type, entities}` 格式。提取使用专门设计的 Decompose Prompt，引导 LLM 逐句扫描并输出 JSON 数组。声明提取逐 chunk 独立执行（Map 阶段），可并行化。提取的声明暂存于内存（不持久化，审计结束后丢弃）。
- 验收标准：
  - AC-1：给定含数值参数的 chunk（如"实验在 25°C 下进行，误差小于 5%"），提取出的声明列表包含"温度为 25°C"和"误差小于 5%"两条原子声明（TC-AUDIT-002）。
  - AC-2：提取的声明携带来源信息（chunk_id 和 sequence），可追溯回原文位置（TC-AUDIT-002 旁证）。
  - AC-3：纯描述性文字（如"本章介绍实验设计"）不被提取为声明，只有可验证的断言才进入声明列表（TC-AUDIT-002 旁证）。
  - AC-4：LLM 返回非法 JSON 时优雅降级，跳过该 chunk 的声明提取并记录警告，不中断整个审计流程（TC-AUDIT-002 旁证）。
- 契约：§5.8 `AuditEngine::decompose` + `build_decompose_prompt` ｜ 关联测试：TC-AUDIT-002/002b
- 调研依据：Decompose-Then-Verify 范式（arXiv: 2505.12348 Reasoning-CV；arXiv: 2503.15354 Optimizing Decomposition——提出「原子性 atomicity」指标量化信息密度）；arXiv: 2510.22055（数值事实核查——数值型声明是矛盾高发区）；Google PAT（声明级验证目标记录）。本需求仅设计 Decompose Prompt + JSON 解析，纯 `crates/core` 层逻辑。

#### REQ-AUDIT-003 跨声明矛盾检测（Verify Phase）
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-AUDIT-002、REQ-VEC-002
- 描述：对提取的全部原子声明做跨 chunk 矛盾检测。直接两两比对复杂度为 \(O(n^2)\)（n = 声明数），对长文档不可行。本需求采用 **SIFiD 式预筛 + LLM 精判** 两阶段策略：(1) **预筛阶段**——用现有 `Embedder.embed()` 计算所有声明的向量，按 cosine 相似度找出"高相似但来自不同 chunk"的声明对（相似度高意味着讨论同一主题，更可能存在矛盾），过滤掉明显不相关的声明对，将 \(O(n^2)\) 降至 \(O(k)\)（k = 高风险声明对数）；(2) **精判阶段**——将预筛出的高风险声明对及其原文上下文（包含 chunk 的章节路径前缀，REQ-VEC-006）交给 LLM 判定是否矛盾，LLM 输出 `{pair_id, verdict: "contradiction" | "consistent" | "unverifiable", explanation, severity}`。判定为 `contradiction` 的声明对进入审计报告。
- 验收标准：
  - AC-1：给定两个来自不同 chunk 但讨论同一参数的矛盾声明（如"温度为 25°C" vs "温度为 20°C"），预筛阶段将其标记为高风险对，精判阶段判定为 `contradiction`（TC-AUDIT-003）。
  - AC-2：两个语义无关的声明（如"温度为 25°C" vs "使用 Rust 语言"）在预筛阶段被过滤，不进入精判（TC-AUDIT-003 旁证）。
  - AC-3：LLM 精判输出包含 verdict 字段（`contradiction` / `consistent` / `unverifiable`）和 explanation 字段（TC-AUDIT-003 旁证）。
  - AC-4：预筛阶段的 cosine 相似度计算复用现有 `Embedder.embed()` 端口，不引入新依赖（TC-AUDIT-003 旁证）。
- 契约：§5.8 `AuditEngine::verify` + `cosine_similarity` + `PREFILTER_THRESHOLD` ｜ 关联测试：TC-AUDIT-003/003b
- 调研依据：SIFiD（arXiv: 2403.07557——NLI/语义相似度过滤关键句子，缩小检测范围）；SummaC（arXiv: 2111.09525——句子级 NLI 分割 + 分数聚合）；MultiHop-RAG（arXiv: 2401.15391——标准 RAG 在多跳推理上表现不佳，需要跨 chunk 两两比对）。本需求复用 `Embedder` 端口做预筛，零新依赖。

#### REQ-AUDIT-004 结构化审计报告生成（Report Phase）
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-AUDIT-003、REQ-UI-002
- 描述：将 Verify 阶段判定为 `contradiction` 的声明对聚合成结构化审计报告，以流式 Block 渲染到对话区。报告格式为 Markdown，包含：(1) **摘要段**——审计文档名、chunk 总数、提取声明数、发现矛盾数、审计耗时；(2) **矛盾清单**——逐条列出矛盾，每条含序号、严重等级（`🔴 高` / `🟡 中` / `🟢 低`，由 LLM 在 Verify 阶段判定 severity）、矛盾位置（chunk 序号 + 章节路径前缀）、声明 A 文本、声明 B 文本、LLM 解释说明；(3) **免责声明**——"本报告由 AI 辅助生成，矛盾判定可能存在遗漏或误报，请人工核实"。报告以 `chat_token` 事件流式推送，复用现有 Markdown 渲染链路（marked.js → DOMPurify → highlight.js）。无矛盾时输出"未发现明显矛盾"。
- 验收标准：
  - AC-1：审计完成后对话区渲染为 Markdown 格式的勘误报告，包含摘要段、矛盾清单（如有）、免责声明三部分（TC-AUDIT-004）。
  - AC-2：每条矛盾包含严重等级标记（🔴/🟡/🟢）、两个矛盾声明的原文、来源 chunk 序号与章节路径（TC-AUDIT-004 旁证）。
  - AC-3：无矛盾时输出"未发现明显矛盾"文案，而非空报告（TC-AUDIT-004 旁证）。
  - AC-4：报告末尾包含免责声明"本报告由 AI 辅助生成，矛盾判定可能存在遗漏或误报，请人工核实"（TC-AUDIT-004 旁证）。
  - AC-5：报告内容经 DOMPurify 消毒，不含可执行脚本（REQ-SEC-002 安全审查，TC-AUDIT-004 旁证）。
- 契约：§5.8 `AuditReport` + `commands::report_to_markdown` ｜ 关联测试：TC-AUDIT-004/004b
- 调研依据：FIND 基准（arXiv: 2512.18601——GPT-5 仅恢复 64% 不一致性，需标注"辅助审计"定位）；Google PAT（四级 AI-人类协作分类法——Level 2: AI 辅助内容审查，人类决策）。本需求复用现有 `chat_token` 事件 + 前端 Markdown 渲染链路，零新依赖。

#### REQ-AUDIT-005 审计进度流式推送与取消
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-AUDIT-001、REQ-RAG-005
- 描述：审计过程可能耗时较长（N 个 chunk × 2 次 LLM 调用——声明提取 + 矛盾精判），需要为用户提供实时进度反馈和取消能力。进度推送采用与 `chat_phase` 相同的模式：新增 `audit_phase` 事件（`extracting` / `comparing` / `reporting`），每个阶段内推送 `audit_progress` 事件（`{current, total, message}`）。取消机制复用 REQ-RAG-005 的 `CancellationToken` 模式：`abort_audit` 命令触发 token 取消，审计循环检测到取消后 break，已提取的声明和已发现的矛盾保留并输出部分报告。取消事件语义复用 `chat_error("⏹ 审计已中断")` + `chat_done`。
- 验收标准：
  - AC-1：审计过程中前端实时显示当前阶段（提取声明中 / 比对中 / 生成报告中）和进度（如"正在提取声明: 3/12 chunks"）（TC-AUDIT-005）。
  - AC-2：用户点击取消后审计立即停止，已发现的部分矛盾输出为部分报告，不 Panic（TC-AUDIT-005 旁证）。
  - AC-3：取消后输入框恢复空闲态，可立即发起其他操作（TC-AUDIT-005 旁证）。
  - AC-4：审计完成后 `chat_done` 事件推送，前端 Block 进入完成态（TC-AUDIT-005 旁证）。
- 契约：§5.8 `AuditCancelFlag` + `commands::abort_audit` + `state.audit_cancels` ｜ 关联测试：TC-AUDIT-005
- 调研依据：REQ-RAG-005（abort_chat + CancellationToken 模式）；REQ-RAG-001（chat_phase 事件流式进度模式）；Google PAT（Inference Scaling——多轮迭代推理需要进度反馈）。本需求复用现有 CancellationToken + 事件推送架构，零新依赖。

---

### 4.14 DS — 设计系统规范

> **域背景**：当前 UI 需求仅定义了暗色背景 `#161616` 与强调色 `#38BDF8`，缺乏完整的设计系统规范。本域规格将每条设计规则绑定到 `ui/index.html` 中的实际 Tailwind 类与 CSS 值，确保设计与实现一致、可验证。

#### REQ-DS-001 完整色板体系
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-UI-001
- **v1.0 发布分类**：v1.0.1 Implemented（已实现 6 色自定义 token + Tailwind 原生色 + success/warning/danger/info 语义色，E2E 验收通过） ｜ 决策日期：2026-08-01

**已实现色板**（`tailwind.config.theme.extend.colors`）：

| Tailwind Token | 色值 | 语义角色 | 使用位置 |
|---|---|---|---|
| `base` | `#161616` | 背景色 | `<body class="bg-base">` / `#dragOverlay` / Modal 遮罩 |
| `panel` | `#1C1C1E` | 表面色 | `#sidebar` / `#inputBar` / Toast / Modal 容器 |
| `line` | `#2A2A2E` | 边框色 | 分隔线 / 输入框边框 / 卡片边框 |
| `accent` | `#38BDF8` | 强调色 | 按钮 / 链接 / 激活态 / 高亮 |
| `ink` | `#0C1116` | 按钮文字色 | accent 背景上的文字 |
| `muted` | `#94a3b8` | 次要文字色 | thinking 指示器 / 标签 |

**Tailwind 原生色引用**（未自定义，直接使用 Tailwind 色阶）：

| 色名 | 用途 | 典型类 |
|---|---|---|
| `slate-200` | 主要正文色 | `text-slate-200` |
| `slate-300` | 卡片正文色 | `text-slate-300` |
| `slate-400` | 次要说明色 | `text-slate-400` |
| `slate-500` | 标签 / 占位色 | `text-slate-500` |
| `slate-600` | Toggle 关闭态轨道 | `bg-slate-600` |
| `red-300` | 错误文字色 | `text-red-300` |
| `red-400` | 错误边框色 | `border-red-400/40` |
| `red-500` | 停止按钮背景 | `bg-red-500/15` |
| `amber-300` | 警告文字色 | `text-amber-300` |
| `amber-400` | 警告边框色 | `border-amber-400/30` |
| `white` | 透明叠加 | `bg-white/5` / `bg-white/3` |

**CSS 内联色值**（`.md` 排版体系中的硬编码色值，允许例外）：

| 色值 | 用途 | 位置 |
|---|---|---|
| `#cbd5e1` | `.md` 正文色 | `.md { color: #cbd5e1; }` |
| `#f1f5f9` | `.md` h1/h2 标题色 | `.md h1/h2 { color: #f1f5f9; }` |
| `#e2e8f0` | `.md` h3/h4 标题色 | `.md h3/h4 { color: #e2e8f0; }` |
| `#f8fafc` | `.md` strong 色色 | `.md strong { color: #f8fafc; }` |
| `#7dd3fc` | `.md` 行内代码色 | `.md code:not(pre code) { color: #7dd3fc; }` |
| `#0d1117` | 代码块背景色 | `.md .code-block { background: #0d1117; }` |
| `#38BDF8` | 圆点 / 引用边框 | `.md ul li::before` / `.md blockquote` |

**语义色**（已实现，`tailwind.config.theme.extend.colors` + `tokens.css` CSS 变量同步）：

| 语义角色 | 色值 | 对应场景 |
|---|---|---|
| success | `#4ADE80`（green-400） | 成功 Toast / 操作成功 |
| warning | `#FBBF24`（amber-400） | 警告 Toast / 配额接近上限 |
| danger | `#F87171`（red-400） | 错误 Toast / 确认对话框确认按钮 |
| info | `#60A5FA`（blue-400） | 信息 Toast / Callout-note |

- 验收标准：
  - AC-1：`tailwind.config.theme.extend.colors` 中定义 6 个自定义色 token（base / panel / line / accent / ink / muted）。
  - AC-2：`ui/index.html` 中 `tailwind.config` 之外不出现非 `.md` 排版体系的硬编码色值（静态扫描断言，`.md` 排版体系色值为允许例外）。
  - AC-3：`.md` 排版体系中的硬编码色值与上表一致。
  - AC-4：交互态色与默认态有明显视觉差异（如 `#sendBtn` hover opacity 1→0.9，差值 ≥ 10%）。
  - AC-5：语义色（success / warning / danger / info）在 Tailwind 配置中定义（已实现，TC-DS-012b 验证）。
- 契约：`tailwind.config.theme.extend.colors` 对象 + `.md` CSS 排版体系。
- 关联测试：TC-DS-007, TC-DS-008, TC-DS-012, TC-DS-012b

#### REQ-DS-002 字体层级体系
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-UI-001
- **v1.0 发布分类**：v1.0.1 Implemented（已实现完整字体层级体系，E2E getComputedStyle 验收通过） ｜ 决策日期：2026-08-01

**字体族**（`<style>` 中定义）：

| 用途 | 字体族 | CSS |
|---|---|---|
| 全局正文 | 系统字体栈 | `body { font-family: -apple-system, "SF Pro SC", "PingFang SC", "Segoe UI", sans-serif; }` |
| 代码（行内） | 等宽字体栈 | `.md code:not(pre code) { font-family: "SF Mono","Fira Code",monospace; }` |
| 代码（代码块） | 继承等宽 | `.md .code-block pre code { font-family: inherit; }` |

**字号层级**（实际使用值）：

| 层级 | 像素值 | Tailwind 类 | 使用位置 |
|---|---|---|---|
| xs | 11px | `text-[11px]` | 侧栏标签 / `#inputHint` / 徽标 / Chart 按钮文字 |
| sm | 12px | `text-xs` | 文档列表项 / 会话列表项 / `.copy-btn` / 代码语言标签 |
| base | 14px | `text-sm` | 正文 / 输入框 / 按钮 / `.md` 排版正文（14px） |
| lg | 16px | `text-base` | （预留，当前未直接使用） |
| xl | 18px | `text-lg` | 空状态标题 / Modal 标题 |
| 2xl | 20px | `text-xl` | 拖拽遮罩标题 / VLM 确认弹窗标题 |
| 3xl | 24px | `text-2xl` | （预留） |
| 4xl | 30px | `text-3xl` | 向导品牌标题 / 设置图标 |

**`.md` 排版字号体系**（相对于 `.md` 基准 14px 的 em 倍率）：

| 元素 | 字号 | CSS |
|---|---|---|
| `.md` 基准 | 14px | `.md { font-size: 14px; }` |
| `.md h1` | 1.45em (≈20.3px) | `.md h1 { font-size: 1.45em; }` |
| `.md h2` | 1.25em (≈17.5px) | `.md h2 { font-size: 1.25em; }` |
| `.md h3` | 1.12em (≈15.7px) | `.md h3 { font-size: 1.12em; }` |
| `.md h4` | 1.0em (14px) | `.md h4 { font-size: 1em; }` |
| `.md h5/h6` | 0.9em (≈12.6px) | `.md h5/h6 { font-size: 0.9em; }` |
| `.md code` | 0.88em | `.md code:not(pre code) { font-size: 0.88em; }` |
| `.md .code-block code` | 13px | `.md .code-block pre code { font-size: 13px; }` |

**行高**：

| 场景 | 行高 | CSS |
|---|---|---|
| `.md` 正文 | 1.8 | `.md { line-height: 1.8; }` |
| `#queryInput` | 1.5 (leading-6) | `class="leading-6"` |
| 代码块 | 1.65 | `.md .code-block pre code { line-height: 1.65; }` |

**字重**：

| 场景 | 字重 | CSS |
|---|---|---|
| 正文 | 400（默认） | — |
| 按钮 / 标签 | 500 | `font-medium` |
| 品牌标题 / 区块标签 | 600 | `font-semibold` |
| `.md h1/h2` / `strong` | 700 | `font-weight: 700` |

- 验收标准：
  - AC-1：`body` 计算样式 `font-family` 含 `-apple-system` 和 `"SF Pro SC"`。
  - AC-2：`.md` 计算样式 `font-size` = `14px`，`line-height` = `1.8`。
  - AC-3：`.md code:not(pre code)` 计算样式 `font-family` 含 `"SF Mono"` 或 `"Fira Code"`。
  - AC-4：`#inputHint` 计算样式 `font-size` = `11px`（`text-[11px]`）。
  - AC-5：`.md h1` 计算样式 `font-size` ≈ `20.3px`（1.45em × 14px），`font-weight` = `700`。
  - AC-6：中英文混排时行高一致（`.md { line-height: 1.8 }` 统一适用）。
- 契约：`body` CSS 规则 + `tailwind.config` + `.md` 排版 CSS。
- 关联测试：TC-DS-001, TC-DS-002, TC-DS-003, TC-DS-004, TC-DS-005, TC-DS-006

#### REQ-DS-003 间距网格系统
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-UI-001
- **v1.0 发布分类**：v1.0.1 Implemented（已实现 4px 基础网格 + Tailwind spacing scale，E2E padding 验收通过） ｜ 决策日期：2026-08-01

**4px 基础网格**（Tailwind spacing scale 对应）：

| Token | 像素值 | Tailwind 类 | 使用位置 |
|---|---|---|---|
| 1 | 4px | `p-1` / `gap-1` / `m-1` | 徽标 padding / 小间距 |
| 1.5 | 6px | `p-1.5` / `py-1.5` | `#queryInput` 垂直 padding / scrollbar 宽度 |
| 2 | 8px | `p-2` / `gap-2` / `px-2` | 徽标 padding / `.msg-actions` gap / 文档列表 `space-y-1` |
| 2.5 | 10px | `py-2.5` | `#newChatBtn` / 会话列表项 |
| 3 | 12px | `p-3` / `gap-3` / `px-3` | `#inputBar` padding / `.md` margin / 代码块 margin |
| 4 | 16px | `p-4` / `px-4` / `py-4` | `#sidebar` 内边距 / assistant Block padding / Modal 容器 |
| 5 | 20px | `px-5` / `pb-5` / `pt-5` | assistant Block `px-5` / 底部输入区 `pb-5` / 侧栏标签 `pt-5` |
| 6 | 24px | `px-6` / `py-6` | `#chatArea` padding / 底部输入区 `px-6` |
| 8 | 32px | — | （预留，当前通过组合实现） |
| 12 | 48px | `pt-12` | （预留） |

**关键容器间距实测**：

| 容器 | padding | Tailwind 类 |
|---|---|---|
| `#chatArea` | 24px | `px-6 py-6` |
| `#inputBar` | 16px 12px | `px-4 py-3` |
| `#sidebar` 品牌头 | 16px | `px-4 py-4` |
| assistant Block | 20px 16px | `px-5 py-4` |
| user Block | 16px | `px-4 py-3` |
| `#docList` 项间距 | 4px | `space-y-1` |
| `#convList` 项间距 | 4px | `space-y-1` |
| Modal 内部 | 32px | `px-8` |

- 验收标准：
  - AC-1：`#chatArea` 计算样式 `padding` = `24px`（`px-6 py-6`）。
  - AC-2：`#inputBar` 计算样式 `padding-left/right` = `16px`，`padding-top/bottom` = `12px`。
  - AC-3：`#docList` 内列表项间距 = `4px`（`space-y-1`）。
  - AC-4：Modal 内部 `padding-left/right` = `32px`（`px-8`）。
  - AC-5：所有 `margin` / `padding` / `gap` 值为 4 的倍数（静态扫描断言，允许 `.md` 排版中的 `10px`/`14px` 等排版微调值）。
- 契约：Tailwind spacing scale（1=4px, 2=8px, 3=12px, 4=16px, 5=20px, 6=24px, 8=32px）。
- 关联测试：TC-DS-009, TC-DS-010, TC-DS-011

#### REQ-DS-004 图标系统
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-UI-001

**当前图标方案**：使用 Unicode 字符 + Emoji，不使用 SVG 图标库。

| 图标 | 字符 | 使用位置 | 字号 |
|---|---|---|---|
| 导入文件 | `＋` | `#plusBtn` | `text-xl`（20px） |
| 折叠/展开 | `‹` / `›` | `#collapseBtn` | `text-lg`（18px） |
| 新对话 | `＋` | `#newChatBtn` | `text-sm`（14px，文字内） |
| 设置 | `⚙` | `#settingsBtn` | `text-base`（16px） |
| 停止 | `■` | `#stopBtn` | `text-sm`（14px，文字内） |
| 拖拽 | `⇩` | `#dragOverlay` | `text-5xl`（48px） |
| 品牌图标 | `◈` | 空状态 / Paywall | `text-5xl`（48px） / `text-4xl`（36px） |
| 删除 | `×` | 文档/会话列表项 | `text-sm`（14px，文字内） |
| 重试 | `↻` | Failed 文档项 | `text-xs`（12px，文字内） |
| 审计 | `🔍` | Indexed 文档项 | `text-xs`（12px，文字内） |
| 警告 | `⚠️` | VLM 确认弹窗 | `text-3xl`（30px） |
| 复制 | `📋` | 右键菜单（拟新增） | — |
| 删除（菜单） | `🗑` | 右键菜单（拟新增） | — |

**图标颜色规则**：
- 图标继承父元素 `currentColor`（Unicode 字符天然继承）。
- 交互态变化通过父元素 `hover:text-accent` 等类实现。

**待补充规格**（Draft，尚未实现）：
- SVG inline 图标系统替代 Unicode 字符（确保跨平台一致渲染）。
- 图标尺寸三档：sm 16px / md 20px / lg 24px。
- disabled 状态图标 `opacity: 0.4`。

- 验收标准：
  - AC-1：当前全部图标使用 Unicode 字符，无 SVG / CDN 图标库引用。
  - AC-2：`#plusBtn` 图标 `＋` 计算样式 `font-size` = `20px`（`text-xl`）。
  - AC-3：图标颜色继承父元素 `currentColor`（如 `#plusBtn` hover 时 `text-accent` → 图标变 accent 色）。
  - AC-4：`#dragOverlay` 图标 `⇩` 计算样式 `font-size` = `48px`（`text-5xl`），颜色 = `#38BDF8`。
  - AC-5：（Draft）待引入 SVG 图标系统后，尺寸限定 16px / 20px / 24px 三档。
- 契约：Unicode 字符 + Tailwind `text-{size}` 类 + `hover:text-{color}` 变体。
- 关联测试：TBD

#### REQ-DS-005 组件设计规范
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-DS-001、REQ-DS-002、REQ-DS-003
- **v1.0 发布分类**：v1.0.1 Implemented（已实现按钮/输入框/卡片/模态框/Toast 规格体系，E2E 验收通过） ｜ 决策日期：2026-08-01

**按钮变体规格**（实际实现）：

| 变体 | 示例元素 | Tailwind 类 | 高度 | 默认态 | hover 态 | disabled 态 |
|---|---|---|---|---|---|---|
| primary（accent） | `#sendBtn` / `#wizStart` / `#paywallActivate` / `#settingsClose` / `#vlmConfirmOk` | `bg-accent text-ink font-medium rounded-xl hover:opacity-90 transition-opacity` | 36px (`h-9`) | bg=#38BDF8, text=#0C1116 | opacity 0.9 | `disabled:opacity-30` |
| secondary（border） | `#paywallClose` / `#settingsEditLlm` / `#vlmConfirmCancel` / `#vlmConfirmCancel` | `border border-line rounded-xl text-slate-300 hover:bg-white/5 transition-colors` | 36-48px | bg=transparent, border=#2A2A2E | bg=rgba(255,255,255,0.05) | — |
| danger | `#stopBtn` / 确认对话框确认按钮 | `bg-red-500/15 text-red-300 border border-red-400/40 rounded-xl hover:bg-red-500/25 transition-colors` | 36px | bg=rgba(239,68,68,0.15) | bg=rgba(239,68,68,0.25) | — |
| ghost（icon） | `#plusBtn` / `#collapseBtn` / `#settingsBtn` | `bg-white/5 text-slate-300 hover:text-accent hover:bg-accent/10 transition-colors`（`#plusBtn`） | 36px (`w-9 h-9`) | bg=rgba(255,255,255,0.05) | text=#38BDF8, bg=rgba(56,189,248,0.1) | — |

**输入框规格**：

| 元素 | 高度 | 圆角 | 边框 | focus 态 |
|---|---|---|---|---|
| `#wizKey` / `#wizUrl` / `#wizModel` / `#licenseInput` | 48px (`py-3`) | `rounded-xl` (12px) | `border border-line` (1px #2A2A2E) | `focus:border-accent` (边框变 #38BDF8) |
| `#queryInput` | 自适应 (1~6行) | — | 无（`#inputBar` 容器有边框） | `#inputBar:focus-within:border-accent/60` |
| `#vlmToggle` | 24px (`h-6`) | `rounded-full` (9999px) | — | — |

**卡片规格**：

| 卡片类型 | 圆角 | 边框 | 内边距 | 背景 |
|---|---|---|---|---|
| assistant Block | `rounded-2xl` (16px) | `border border-line` (1px) | `px-5 py-4` (20px 16px) | `bg-panel` (#1C1C1E) |
| user Block | `rounded-2xl rounded-br-md` (16px/8px) | `border border-accent/30` | `px-4 py-3` (16px 12px) | `bg-accent/15` |
| 设置面板信息卡 | `rounded-xl` (12px) | `border border-line` | `px-4 py-3` | `bg-panel` |
| 导入进度条容器 | `rounded-xl` (12px) | `border border-line` | `px-4 py-2.5` | `bg-panel` |

**模态框规格**：

| 属性 | 值 | CSS |
|---|---|---|
| 遮罩层 | `bg-base/95 backdrop-blur-sm` | 背景色 rgba(22,22,22,0.95) + 模糊 |
| z-index | 50 | `z-50` |
| 容器最大宽 | 576px / 448px / 384px | `max-w-xl` / `max-w-md` / `max-w-sm` |
| 容器圆角 | 无（直接 `px-8` 内边距，无外圆角） | — |
| 动画 | `fade-in` | `opacity 0→1` + `translateY(6px)→none`，250ms |

**Toast 规格**（实际实现）：

| 属性 | 值 |
|---|---|
| 容器位置 | `fixed bottom-6 right-6`（右下角，距底/右各 24px） |
| z-index | 60 |
| 单条圆角 | `rounded-xl` (12px) |
| 单条内边距 | `px-4 py-3` (16px 12px) |
| 自动消失 | 4200ms（4.2 秒） |
| 动画 | `fade-in`（250ms ease-out） |
| 宽度 | `w-80`（320px） |

- 验收标准：
  - AC-1：`#sendBtn` 为 primary 变体，计算样式 `background-color` = `#38BDF8`，`color` = `#0C1116`。
  - AC-2：`#paywallClose` 为 secondary 变体，hover 时 `background-color` = `rgba(255,255,255,0.05)`。
  - AC-3：`#stopBtn` 为 danger 变体，计算样式 `background-color` = `rgba(239,68,68,0.15)`。
  - AC-4：`#wizKey` focus 时计算样式 `border-color` = `#38BDF8`（`focus:border-accent`）。
  - AC-5：assistant Block 计算样式 `border-radius` = `16px`（`rounded-2xl`），`background-color` = `#1C1C1E`。
  - AC-6：Toast 4200ms 后从 DOM 移除（`setTimeout(() => el.remove(), 4200)`）。
- 契约：Tailwind 组件类组合（`bg-accent` / `border-line` / `rounded-xl` / `rounded-2xl` 等）。
- 关联测试：TC-DS-007, TC-DS-008, TC-DS-012

#### REQ-DS-006 动效与过渡规范
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-UI-001

**过渡时长体系**（实际使用值）：

| Token | 时长 | CSS / Tailwind | 使用位置 |
|---|---|---|---|
| fast | 150ms | `transition-colors` / `transition-opacity` / `transition: all 0.15s` | 按钮 hover / `#sidebar` / `.copy-btn` / `.cite-ref` |
| normal | 200ms | `transition: opacity 0.2s` | `.msg-actions` 显隐 |
| normal | 250ms | `@keyframes fadeIn` `0.25s ease-out` | `fade-in` 类 / `#sidebar` `duration-200` |
| slow | 400ms | — | （预留，当前未使用） |

**`fade-in` 动画**（`<style>` 中定义）：
```css
.fade-in { animation: fadeIn 0.25s ease-out; }
@keyframes fadeIn { from { opacity: 0; transform: translateY(6px); } to { opacity: 1; transform: none; } }
```
- 用于：Modal 容器 / 拖拽遮罩虚线框 / 消息 Block / Toast / 空状态引导。

**`thinkingPulse` 动画**（思考指示器圆点）：
```css
@keyframes thinkingPulse { 0%, 80%, 100% { opacity: 0.3; transform: scale(0.8); } 40% { opacity: 1; transform: scale(1); } }
```
- 时长：1.4s `infinite ease-in-out`。
- 3 个圆点延迟：0s / 0.2s / 0.4s。

**`#sidebar` 折叠过渡**：
- `transition-all duration-200`：250ms 过渡（Tailwind `duration-200` = 200ms）。
- 宽度从 `w-60`（240px）→ `w-14`（56px）过渡。

**流式渲染 rAF 节流**：
- `scheduleRender()` 函数使用 `requestAnimationFrame` + `renderScheduled` 标志位防抖。
- 连续 `chat_token` 事件在同一帧内只触发一次 `renderMarkdown()` + `scrollToBottom()`。
- 目标帧率：60fps（16ms/帧）。

**Tailwind `transition-colors` 默认时长**：
- 150ms `cubic-bezier(0.4, 0, 0.2, 1)`（Tailwind 默认 `ease-in-out`）。

**待补充规格**（Draft，尚未实现）：
- 模态框打开动效：`scale(0.95)→scale(1)` + `opacity(0)→opacity(1)`，250ms（当前仅 `fade-in`）。
- Toast 滑入动效：`translateY(-100%)→translateY(0)`（当前仅 `fade-in` 从底部滑入）。
- `prefers-reduced-motion: reduce` 媒体查询支持。

- 验收标准：
  - AC-1：`#sendBtn` hover 时 `opacity` 过渡 150ms（`transition-opacity` = Tailwind 默认 150ms）。
  - AC-2：`.fade-in` 动画时长 250ms，缓动 `ease-out`，含 `opacity 0→1` + `translateY(6px)→none`。
  - AC-3：`#sidebar` 折叠时宽度过渡 200ms（`duration-200`）。
  - AC-4：`thinkingPulse` 动画时长 1.4s `infinite`，3 圆点延迟 0s/0.2s/0.4s。
  - AC-5：`scheduleRender()` 使用 `requestAnimationFrame`，连续 token 事件不触发多次 DOM 写入。
  - AC-6：（Draft）`prefers-reduced-motion: reduce` 时禁用 `fade-in` 和 `thinkingPulse` 动画。
- 契约：CSS `@keyframes` + Tailwind `transition-*` / `duration-*` 类 + `requestAnimationFrame` API。
- 关联测试：TBD

#### REQ-DS-007 圆角与阴影体系
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-DS-005

**圆角层级**（实际使用值）：

| Token | 像素值 | Tailwind 类 | 使用位置 |
|---|---|---|---|
| none | 0 | `rounded-none` | — |
| sm | 4px | `rounded` (4px) | `.copy-btn` / `.code-lang` / 文档徽标 |
| md | 8px | `rounded-lg` (8px) | 代码块内 `pre` / `.md details` |
| lg | 12px | `rounded-xl` (12px) | 按钮 / 输入框 / Toast / 信息卡片 / 导入进度条 |
| xl | 16px | `rounded-2xl` (16px) | assistant Block / user Block / `#inputBar` |
| 2xl | 24px | `rounded-3xl` (24px) | 拖拽遮罩虚线框 |
| full | 9999px | `rounded-full` | `#vlmToggle` / 引用芯片 / 进度条 / 圆点 |

**圆角组合**（不对称圆角）：

| 元素 | 圆角组合 | 效果 |
|---|---|---|
| user Block | 无（纯文本右对齐） | 无气泡背景/边框/圆角（对齐 chat.deepseek.com） |
| assistant Block | `rounded-2xl rounded-tl-md` | 16px 圆角 + 左上角 8px |

**阴影层级**（实际使用值）：

| Token | CSS | 使用位置 |
|---|---|---|
| sm | `shadow-lg`（Tailwind 内置，≈ `0 10px 15px -3px rgba(0,0,0,0.1)`） | Toast 单条 |
| md | `shadow-2xl`（Tailwind 内置，≈ `0 25px 50px -12px rgba(0,0,0,0.25)`） | 右键菜单（拟新增） |
| lg | — | （预留） |

**`.md` 排版圆角**（CSS 内联）：

| 元素 | 圆角 | CSS |
|---|---|---|
| `.md .code-block` | 10px | `border-radius: 10px` |
| `.md .code-header` | 无（继承容器） | — |
| `.md .table-wrap` | 10px | `border-radius: 10px` |
| `.md blockquote` | `0 8px 8px 0` | `border-radius: 0 8px 8px 0` |
| `.md .callout` | 10px | `border-radius: 10px` |
| `.md details` | 8px | `border-radius: 8px` |

- 验收标准：
  - AC-1：`#sendBtn` 计算样式 `border-radius` = `12px`（`rounded-xl`）。
  - AC-2：assistant Block 计算样式 `border-radius` = `16px`（`rounded-2xl`）。
  - AC-3：Toast 单条计算样式 `box-shadow` 含 `shadow-lg`（`0 10px 15px` 阴影）。
  - AC-4：`#vlmToggle` 计算样式 `border-radius` = `9999px`（`rounded-full`）。
  - AC-5：`.md .code-block` 计算样式 `border-radius` = `10px`（CSS 内联）。
  - AC-6：user Block 右下角 `border-bottom-right-radius` = `8px`（`rounded-br-md`），其余角 `16px`。
- 契约：Tailwind `rounded-*` 类 + `.md` CSS 内联 `border-radius`。
- 关联测试：TBD

---

### 4.15 IX — 交互设计规范

> **域背景**：当前交互需求散落在各功能域中（如拖拽遮罩、输入框三态），缺乏系统化的交互模式定义。本域规格将每条交互需求原子化为「触发元素 → 事件 → 视觉反馈（含 timing）→ 状态变更 / 后端调用」的四元组，确保交互行为无歧义、可测试。

#### REQ-IX-001 右键上下文菜单
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-UI-001

**菜单项规格**（按区域分组）：

**文档列表项右键菜单**：

| 菜单项 | 图标 | 快捷键提示 | 条件 | 动作 |
|---|---|---|---|---|
| 复制文件名 | 📋 | — | 始终 | `navigator.clipboard.writeText(docName)` + toast「已复制」 |
| 重建索引 | ↻ | — | Failed 状态 | `invoke('retry_index', { id })` |
| 审计 | 🔍 | — | Pro + Indexed | `invoke('audit_document', { docId, docName })` |
| 审计（灰显） | 🔍 | `Pro` | 非 Pro | 无动作，显示「Pro」标签 |
| 删除 | 🗑 | — | 始终 | 弹出确认对话框（REQ-IX-005） |

**会话列表项右键菜单**：

| 菜单项 | 图标 | 快捷键提示 | 动作 |
|---|---|---|---|
| 重命名 | ✏ | — | 进入行内编辑模式 |
| 删除 | 🗑 | — | `invoke('delete_conversation', { id })` |
| 导出 | 📤 | — | `exportConversation(id)` → Markdown 文件保存 |

**消息 Block 右键菜单**：

| 菜单项 | 图标 | 快捷键提示 | 条件 | 动作 |
|---|---|---|---|---|
| 复制全文 | 📋 | ⌘C | assistant Block | `navigator.clipboard.writeText(rawMarkdown)` |
| 复制纯文本 | 📋 | — | assistant Block | `navigator.clipboard.writeText(mdEl.innerText)` |
| 重新生成 | ↻ | — | assistant Block + idle 态 | 重新提交该消息对应的 user query |
| 删除 | 🗑 | — | 任意 Block | 从 DOM 移除 + 从 history 数组移除 |

**对话区空白处右键菜单**：

| 菜单项 | 图标 | 动作 |
|---|---|---|
| 全选 | — | 选中文本（浏览器默认行为） |
| 清空对话 | 🗑 | 弹出确认对话框 → 清空 history + resetChatArea |

**菜单容器规格**：
- DOM：动态创建 `<div>` 挂载到 `document.body`
- 类名：`fixed z-[70] bg-panel border border-line rounded-xl py-1.5 shadow-2xl min-w-[180px]`
- 菜单项：`px-4 py-2 text-sm text-slate-300 hover:bg-accent/10 cursor-pointer flex items-center justify-between gap-4`
- 灰显项：`text-slate-600 cursor-not-allowed`，右侧 `<span class="text-[10px] text-accent">Pro</span>`
- 分隔线：`<div class="h-px bg-line my-1"></div>`

**定位逻辑**：
- 默认：`left = event.clientX`，`top = event.clientY`
- 右边界溢出：`menuWidth + clientX > window.innerWidth` → `left = clientX - menuWidth`
- 下边界溢出：`menuHeight + clientY > window.innerHeight` → `top = clientY - menuHeight`

**关闭条件**：点击菜单项 / 点击菜单外区域 / 按 Esc 键 → 从 DOM 移除菜单容器

- 验收标准：
  - AC-1：右键文档列表项弹出菜单，含「复制文件名 / 重建索引 / 审计 / 删除」选项。
  - AC-2：右键会话列表项弹出菜单，含「重命名 / 删除 / 导出」选项。
  - AC-3：右键 assistant Block 弹出菜单，含「复制全文 / 复制纯文本 / 重新生成 / 删除」选项。
  - AC-4：菜单超出窗口右边界时 `left` 自动左偏，超出下边界时 `top` 自动上偏。
  - AC-5：点击菜单外区域或按 Esc 后菜单从 DOM 移除。
  - AC-6：非 Pro 用户右键 Indexed 文档时「审计」项灰显，右侧显示 `Pro` 标签。
- 契约：`document.addEventListener('contextmenu', ...)` + `retry_index` / `audit_document` / `delete_document` / `delete_conversation` IPC 命令。
- 关联测试：TBD

#### REQ-IX-002 会话列表拖拽排序
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-RAG-006

**触发元素**：`#convList` 内的会话列表项 `<div class="group flex items-center ...">`

**拖拽行为规格**：

| 阶段 | 事件 | 视觉反馈 | 数据操作 |
|---|---|---|---|
| 开始拖拽 | `dragstart` | 拖拽项 `opacity-50` + 半透明跟随光标 | 记录 `draggedId` |
| 拖拽经过 | `dragover` | 目标项上方/下方显示 2px 蓝色插入线（`border-t-2 border-accent`） | `e.preventDefault()` 允许放置 |
| 松开 | `drop` | 插入线消失，项移动到新位置 | `reorderConversations(draggedId, targetId)` → `invoke('reorder_conversations', { ... })` |
| 取消 | `dragend` | 拖拽项恢复 `opacity-100` | 清除 `draggedId` |

**插入线定位**：
- `dragover` 时计算鼠标 Y 坐标相对目标项中点：`offsetY < targetHeight / 2` → 上方插入线；否则下方插入线。
- 插入线 DOM：临时 `<div class="h-0.5 bg-accent mx-2 rounded-full">`，`dragleave` / `drop` 时移除。

**持久化**：
- SQLite `conversations` 表新增 `sort_order INTEGER DEFAULT 0` 字段。
- `get_conversations` 返回结果按 `sort_order ASC, created_at DESC` 排序。
- `reorder_conversations` IPC 命令批量更新 `sort_order`。

**默认排序**：`sort_order` 全部为 0 时按 `created_at DESC`（最新在最上方）。用户手动拖拽后 `sort_order` 被设置，切换为自定义顺序。

- 验收标准：
  - AC-1：长按会话项可拖拽，拖拽时项 `opacity-50` 并跟随光标。
  - AC-2：拖拽经过其他会话项时，目标位置显示 2px accent 色插入线。
  - AC-3：松开后顺序立即更新到 DOM。
  - AC-4：重启后 `get_conversations` 返回自定义顺序（`sort_order` 持久化）。
  - AC-5：拖拽排序不修改会话的 messages 内容。
- 契约：HTML5 Drag and Drop API + `reorder_conversations` IPC 命令（新增）+ `conversations.sort_order` 字段（新增）。
- 关联测试：TBD

#### REQ-IX-003 统一复制行为
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-UI-002
- **v1.0 发布分类**：v1.0 力争交付 ｜ 决策日期：2026-08-01

**复制行为矩阵**：

| 复制场景 | 触发 | 复制内容 | API | 反馈 |
|---|---|---|---|---|
| 代码块复制 | 点击 `.copy-btn` | `code.innerText`（代码原文，不含高亮标记） | `copyToClipboard()` | 按钮文案「复制」→「已复制 ✓」（1.5s 恢复） |
| 消息复制全文 | 点击 `.msg-action-btn`「复制全文」 | `rawMarkdown`（模型原始 Markdown 输出） | `copyToClipboard()` | toast「已复制」 |
| 消息复制纯文本 | 点击 `.msg-action-btn`「复制纯文本」 | `mdEl.innerText`（渲染后纯文本，去除 Markdown 标记） | `copyToClipboard()` | toast「已复制」 |
| 文档名复制 | 右键菜单「复制文件名」 | `docName`（完整文件名，不含路径） | `copyToClipboard()` | toast「已复制」 |
| 引用片段复制 | 点击 `.source-chip` | `chunk.content`（chunk 原文） | `copyToClipboard()` | toast「已复制」 |

**Toast 规格**：
- 成功：`toast('已复制', 'success')`（accent 色边框）
- 失败：`toast('复制失败', 'error')`（红色边框）

**错误处理**：
- `copyToClipboard()` 统一处理：安全上下文用 `navigator.clipboard.writeText()`，非安全上下文用 `execCommand('copy')` fallback。
- 复制失败时显示「复制失败」toast。

- 验收标准：
  - AC-1：点击代码块「复制」按钮后 `code.innerText` 在剪贴板，按钮文案变「已复制 ✓」1.5s 后恢复。
  - AC-2：点击「复制全文」后 `rawMarkdown` 在剪贴板，显示「已复制」success toast。
  - AC-3：右键「复制纯文本」后 `mdEl.innerText`（纯文本）在剪贴板。
  - AC-4：右键「复制文件名」后完整文件名在剪贴板。
  - AC-5：所有复制操作使用 `navigator.clipboard.writeText()`，非安全上下文时显示「复制失败」toast。
- 契约：`copyToClipboard(text)` in `ui/src/utils.js` + `toast(message, kind)` + `showMsgActions()` + `_showDocMenu()` / `_showSourceMenu()` in `ui/src/context-menu.js` ｜ 关联测试：`e2e-tests/tests/msg-actions.spec.ts` E2E-IX-003

#### REQ-IX-004 悬停反馈规范
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-DS-006

**悬停反馈矩阵**（按元素类型分组）：

| 元素 | 悬停效果 | CSS 实现 | 过渡时长 |
|---|---|---|---|
| `#newChatBtn` | 背景变深 | `hover:bg-accent/20` | `transition-colors`（默认 150ms） |
| `#collapseBtn` | 文字变色 | `hover:text-accent` | `transition-colors`（默认 150ms） |
| `#plusBtn` | 文字变色 + 背景变色 | `hover:text-accent hover:bg-accent/10` | `transition-colors`（默认 150ms） |
| `#sendBtn` | 透明度降低 | `hover:opacity-90` | `transition-opacity`（默认 150ms） |
| `#stopBtn` | 背景变深 | `hover:bg-red-500/25` | `transition-colors`（默认 150ms） |
| `#settingsBtn` | 文字变色 | `hover:text-accent` | 默认 |
| 文档列表项 | 背景微亮 | `hover:bg-white/5` | 默认 |
| 会话列表项（非激活） | 背景微亮 | `hover:bg-white/5` | 默认 |
| 文档删除按钮 `×` | 从 invisible 变 visible + 变红 | `group-hover:visible hover:text-red-400` | 默认 |
| `.msg-actions` | 从 opacity:0 变 opacity:1 | `.md-block:hover .msg-actions { opacity: 1 }` | `transition: opacity 0.2s` |
| `.copy-btn` | 文字变色 + 边框变色 + 背景变色 | `hover: color:#38BDF8; border-color:rgba(56,189,248,0.3); background:rgba(56,189,248,0.08)` | `transition: all 0.15s` |
| `.cite-ref`（引用标记） | 背景高亮 | `hover: background:rgba(56,189,248,0.15)` | `transition: background 0.15s` |
| 代码块表格行 | 背景高亮 | `.md tbody tr:hover { background:rgba(56,189,248,0.05) }` | 默认 |
| `.md a`（链接） | 底部边框出现 | `hover: border-bottom-color:#38BDF8` | `transition: border-color 0.15s` |
| `.md summary` | 背景微亮 | `hover: background:rgba(255,255,255,0.05)` | 默认 |

**Tooltip 规格**：
- 文档列表项：`title` 属性 = 完整 `file_path`，浏览器原生 tooltip。
- 会话列表项标题：`title` 属性 = 完整 `c.title`（因 `truncate` 截断）。
- 引用来源芯片：`title` 属性 = `chunk.content.slice(0, 200)`。

- 验收标准：
  - AC-1：文档列表项悬停时 `background-color` 变为 `rgba(255,255,255,0.05)`（`bg-white/5`）。
  - AC-2：`#sendBtn` 悬停时 `opacity` 从 1 变为 0.9，过渡 150ms。
  - AC-3：文档列表项 `title` 属性为完整文件路径，悬停 500ms 后显示浏览器原生 tooltip。
  - AC-4：会话标题被 `truncate` 截断时，`title` 属性为完整标题文本。
  - AC-5：`.msg-actions` 在 `.md-block` 悬停时从 `opacity:0` 过渡到 `opacity:1`，过渡 200ms。
  - AC-6：所有 `transition` 属性时长 ≤ 200ms（CSS 静态扫描断言）。
- 契约：Tailwind `hover:` 变体类 + CSS `transition` 属性 + HTML `title` 属性。
- 关联测试：TBD

#### REQ-IX-005 确认对话框统一模式
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-DS-005
- **v1.0 发布分类**：v1.0 必须交付（已实现 showConfirmDialog 统一组件，替代原生 confirm() + 零散确认弹框） ｜ 决策日期：2026-08-01

**触发场景矩阵**：

| 场景 | 标题 | 正文 | 确认按钮文案 | 确认动作 |
|---|---|---|---|---|
| 删除文档 | `删除「{docName}」？` | 文档及其全部索引数据将被删除。此操作不可撤销。 | 删除 | `invoke('delete_document', { id })` |
| 删除会话 | `删除会话「{title}」？` | 会话中的全部消息将被删除。此操作不可撤销。 | 删除 | `invoke('delete_conversation', { id })` |
| 清空对话 | 清空当前对话？ | 当前会话的全部消息将被清空。此操作不可撤销。 | 清空 | `history = []` + `resetChatArea()` |
| 重建索引 | 重建「{docName}」的索引？ | 将删除现有分块和向量，重新执行导入管线。此操作不可撤销。 | 重建 | `invoke('retry_index', { id })` |
| 停用 License | 停用 Pro 授权？ | 将回落为免费版（50 文件上限，PDF 禁用）。此操作可随时重新激活。 | 停用 | `invoke('deactivate_pro')` |
| 清理模型缓存 | 清理模型缓存？ | 已下载的嵌入模型将被删除，下次使用时需重新下载。 | 清理 | `invoke('clear_model_cache', { modelName: null })` |

**对话框元素规格**：

| 元素 | Tailwind 类 | 文案/说明 |
|---|---|---|
| 遮罩层 | `fixed inset-0 z-[55] bg-base/95 backdrop-blur-sm flex items-center justify-center` | 点击遮罩层 = 取消 |
| 容器 | `w-full max-w-sm px-8 fade-in` | 最大宽 384px |
| 图标 | `text-3xl mb-2 text-amber-400` | `⚠️`（危险操作） |
| 标题 | `text-lg font-semibold` | 动态（见矩阵） |
| 正文 | `text-sm text-slate-300 leading-relaxed mb-4` | 动态（见矩阵）+ 「此操作不可撤销」 |
| 取消按钮 | `flex-1 border border-line rounded-xl px-4 py-3 text-sm text-slate-300 hover:bg-white/5 transition-colors` | "取消"（左） |
| 确认按钮 | `flex-1 bg-red-500/15 text-red-300 border border-red-400/40 rounded-xl px-4 py-3 text-sm hover:bg-red-500/25 transition-colors disabled:opacity-30` | 动态文案（右），danger 变体 |

**交互行为**：

| 触发 | 动作 |
|---|---|
| 弹窗出现 | 确认按钮 `disabled = true`，500ms 后 `disabled = false`（防误触） |
| 点击取消按钮 | 关闭对话框，不执行操作 |
| 点击确认按钮 | 关闭对话框，执行对应动作 |
| 点击遮罩层 | 等同取消 |
| 按 Esc | 等同取消 |
| 按 Enter | 等同确认（仅 500ms 后生效） |

- 验收标准：
  - AC-1：删除文档操作弹出确认对话框，标题含文档名，正文含「此操作不可撤销」。
  - AC-2：确认按钮为 danger 变体（`bg-red-500/15 text-red-300 border-red-400/40`），取消按钮为 secondary 变体。
  - AC-3：Esc 键关闭对话框等同取消。
  - AC-4：确认按钮在弹窗出现后 500ms 内 `disabled`，500ms 后启用。
  - AC-5：点击遮罩层关闭对话框等同取消。
  - AC-6：清理模型缓存的确认对话框标题为「清理模型缓存？」。
- 契约：`showConfirmDialog({ title, body, confirmText, cancelText, danger })` 函数（ui/src/confirm-dialog.js）+ 各 IPC 命令。
- 关联测试：Vitest confirm-dialog.test.js（22 tests）+ E2E interaction-feedback.spec.ts E2E-UX-016~018

#### REQ-IX-006 输入框多行与自适应高度
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-UI-003
- **v1.0 发布分类**：v1.0 必须交付（已实现 input 事件自适应高度 + Shift+Enter 换行 + max-h-40 滚动条） ｜ 决策日期：2026-08-01

> **注意**：此需求与 REQ-UI-003 的输入框元素清单完全一致，此处仅规格化多行行为。

**元素**：`#queryInput`（`<textarea>`，`class="flex-1 bg-transparent resize-none outline-none text-sm leading-6 max-h-40 scrollbar-thin py-1.5"`）

**高度自适应规格**：

| 状态 | 触发 | `#queryInput` 高度 | 说明 |
|---|---|---|---|
| 空闲（1 行） | 初始 / 发送后 | `auto`（≈ 40px，1 行 × leading-6 + py-1.5×2） | 重置：`style.height = 'auto'` |
| 2~6 行 | `input` 事件 | `Math.min(scrollHeight, 160)` px | `max-h-40` = 160px ≈ 6 行 |
| >6 行 | 继续输入 | 固定 160px + 内部滚动条 | `scrollbar-thin` 类，`overflow-y: auto` |

**`input` 事件处理函数**（已有实现）：
```javascript
$('queryInput').addEventListener('input', (e) => {
  e.target.style.height = 'auto';                    // 先重置为 auto
  e.target.style.height = Math.min(e.target.scrollHeight, 160) + 'px';  // 再设为实际需要的高度
});
```

**键盘行为**：

| 按键 | 行为 | 条件 |
|---|---|---|
| Enter（无 Shift） | `e.preventDefault()` + `send()` | `streaming === false` 且输入非空 |
| Shift+Enter | 插入换行 `\n`（默认行为，不阻止） | — |
| ⌘+Enter / Ctrl+Enter | 同 Enter（发送） | 可选增强 |

- 验收标准：
  - AC-1：Shift+Enter 在 `#queryInput` 内插入换行，不触发 `send()`。
  - AC-2：Enter 发送消息（`streaming === false` 且输入非空时）。
  - AC-3：输入 2 行后 `#queryInput` 高度增长到 ≈ 64px（2 × 24px + 12px padding）。
  - AC-4：输入超过 6 行后 `#queryInput` 高度固定为 160px（`max-h-40`），内部出现滚动条。
  - AC-5：粘贴 5 行文本后高度自动适应到 5 行高度。
  - AC-6：`send()` 后 `#queryInput` 高度重置为 1 行（`style.height = 'auto'` + `value = ''`）。
- 契约：`#queryInput` `input` 事件监听器 + `send()` 函数中高度重置逻辑。
- 关联测试：TBD

#### REQ-IX-007 加载状态与骨架屏
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-DS-006

**骨架屏规格**：

**文档列表骨架**（`#docList` 加载中）：

| 元素 | Tailwind 类 | 尺寸 | 动画 |
|---|---|---|---|
| 骨架项容器 | `flex items-center gap-2 px-3 py-2` | 100%×40px | — |
| 文件名占位块 | `h-3 rounded bg-white/5` | 120px×12px | `animate-pulse` |
| 状态徽标占位块 | `h-3 w-10 rounded bg-white/5` | 40px×12px | `animate-pulse` |
| 骨架项数量 | — | 3~5 个 | — |

**会话列表骨架**（`#convList` 加载中）：
- 与文档列表骨架类似，占位块宽度 160px。

**延迟显示逻辑**：
```javascript
let skeletonTimer = null;
function showSkeleton(container, count = 4) {
  skeletonTimer = setTimeout(() => {
    // 200ms 后才插入骨架 DOM
    renderSkeletonItems(container, count);
  }, 200);
}
function hideSkeleton() {
  clearTimeout(skeletonTimer);
  // 移除骨架 DOM
}
```

**`animate-pulse` 动画规格**（Tailwind 内置）：
- `animation: pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite`
- `@keyframes pulse { 0%, 100% { opacity: 1 } 50% { opacity: 0.5 } }`

**淡入淡出过渡**：
- 骨架消失：`opacity 1→0`，200ms `ease-out` → DOM 移除
- 内容出现：`fade-in` 类（`opacity 0→1` + `translateY(6px)→none`，250ms `ease-out`）

**错误状态**：
- 加载失败时骨架替换为：`<div class="text-center py-4 text-sm text-red-400">加载失败</div>` + `<button class="text-xs text-accent hover:underline">重试</button>`

- 验收标准：
  - AC-1：文档列表加载期间 `#docList` 内显示 3~5 个骨架项（灰色块 + `animate-pulse`）。
  - AC-2：加载时间 < 200ms 时不显示骨架屏（`setTimeout` 延迟 200ms 守卫）。
  - AC-3：骨架动画为 `animate-pulse`（`pulse 2s infinite`），不阻塞主线程（CSS 动画）。
  - AC-4：加载完成后骨架项淡出（200ms），内容项 `fade-in`（250ms）。
  - AC-5：加载失败时显示「加载失败」文本 + 「重试」按钮。
- 契约：`showSkeleton(container, count)` / `hideSkeleton()` 函数（新增）+ Tailwind `animate-pulse` 类 + `setTimeout` 延迟守卫。
- 关联测试：TBD

---

### 4.16 KB — 键盘快捷键与命令面板

> **域背景**：桌面应用用户高度依赖键盘操作。当前仅有 Enter 发送和 Esc 取消等零散快捷键，缺乏系统化的快捷键体系。完整的快捷键体系是高效桌面应用的核心竞争力之一。

#### REQ-KB-001 全局快捷键体系
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-UI-001
- **v1.0 发布分类**：v1.0 必须交付（已实现 ActionRegistry + ⌘K/⌘N/⌘O/⌘,/Esc 快捷键，macOS Cmd / Windows Ctrl 自动适配） ｜ 决策日期：2026-08-01
- 描述：定义全局快捷键体系，覆盖高频操作：`Cmd/Ctrl+K` 打开命令面板、`Cmd/Ctrl+N` 新建会话、`Cmd/Ctrl+O` 导入文件、`Cmd/Ctrl+,` 打开设置、`Cmd/Ctrl+/` 显示快捷键面板、`Cmd/Ctrl+Shift+Delete` 清空当前对话、`Esc` 关闭弹窗/取消操作。快捷键在 macOS 使用 Cmd 键，Windows/Linux 使用 Ctrl 键。快捷键不与系统级快捷键冲突。
- 验收标准：
  - AC-1：`Cmd/Ctrl+K` 打开命令面板（REQ-KB-002）。
  - AC-2：`Cmd/Ctrl+N` 新建会话。
  - AC-3：`Cmd/Ctrl+O` 打开文件选择器导入文件。
  - AC-4：`Cmd/Ctrl+,` 打开设置面板。
  - AC-5：`Cmd/Ctrl+/` 显示快捷键帮助面板。
  - AC-6：`Esc` 关闭当前弹窗 / 取消操作 / 关闭命令面板。
  - AC-7：快捷键在 macOS 使用 Cmd，Windows/Linux 使用 Ctrl。
  - AC-8：输入框聚焦时，除 Enter / Shift+Enter / Esc 外，全局快捷键不触发（避免冲突）。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-KB-002 命令面板（Command Palette）
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-KB-001
- 描述：`Cmd/Ctrl+K` 打开全局命令面板（类似 VS Code / Raycast），提供模糊搜索快速执行命令：新建会话、导入文件、打开设置、切换混合检索、切换 VLM、激活 Pro、审计文档、导出对话、检查更新等。命令面板支持中文搜索。面板居中浮于主界面上方，半透明遮罩。
- 验收标准：
  - AC-1：`Cmd/Ctrl+K` 打开命令面板，面板居中显示。
  - AC-2：输入关键词后，命令列表实时模糊过滤。
  - AC-3：上下箭头选择命令，Enter 执行选中命令。
  - AC-4：Esc 关闭命令面板。
  - AC-5：命令面板支持中文搜索。
  - AC-6：面板打开时输入框自动聚焦，关闭后焦点回到之前元素。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-KB-003 对话区快捷键
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-RAG-001
- 描述：对话区专属快捷键：`Enter` 发送（输入非空时）、`Shift+Enter` 换行、`Cmd/Ctrl+Shift+Backspace` 清空当前对话（需确认）、`Cmd/Ctrl+Up/Down` 在历史消息间滚动、`Cmd/Ctrl+L` 聚焦输入框。生成中 `Esc` 或 `Cmd/Ctrl+.` 停止生成。
- 验收标准：
  - AC-1：`Enter` 在输入框非空时发送消息。
  - AC-2：`Shift+Enter` 在输入框内换行。
  - AC-3：`Cmd/Ctrl+Shift+Backspace` 弹出清空对话确认。
  - AC-4：`Cmd/Ctrl+L` 聚焦输入框。
  - AC-5：生成中 `Esc` 或 `Cmd/Ctrl+.` 触发停止生成（等同点击停止按钮）。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-KB-004 文档列表快捷键
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-UI-006
- 描述：文档列表聚焦时支持快捷键：`Up/Down` 在文档间导航、`Enter` 打开文档预览、`Delete` 删除选中文档（需确认）、`Cmd/Ctrl+A` 全选（多选模式下）。导航时列表自动滚动保持选中项可见。
- 验收标准：
  - AC-1：文档列表聚焦时 `Up/Down` 切换选中文档。
  - AC-2：`Enter` 打开选中文档的预览面板。
  - AC-3：`Delete` 弹出删除确认对话框。
  - AC-4：多选模式下 `Cmd/Ctrl+A` 全选文档。
  - AC-5：导航时列表自动滚动，选中项始终可见。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-KB-005 快捷键帮助面板
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-KB-001
- 描述：`Cmd/Ctrl+/` 打开快捷键帮助面板，以分组表格形式展示全部快捷键：全局快捷键、对话区快捷键、文档列表快捷键。每行显示快捷键组合与功能描述。面板可搜索过滤。
- 验收标准：
  - AC-1：`Cmd/Ctrl+/` 打开快捷键帮助面板。
  - AC-2：面板按分组（全局 / 对话 / 文档列表）展示快捷键。
  - AC-3：每行显示快捷键组合（如 `⌘K`）与功能描述。
  - AC-4：面板顶部有搜索框，支持模糊过滤。
  - AC-5：Esc 关闭面板。
- 契约：TBD ｜ 关联测试：TBD

---

### 4.17 A11Y — 无障碍

> **域背景**：无障碍是软件质量的重要维度，也是部分用户群体的基本需求。当前 UI 需求未涉及 ARIA 语义、焦点管理、对比度、屏幕阅读器支持等无障碍要求。

#### REQ-A11Y-001 ARIA 语义标注
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-UI-001
- 描述：所有交互元素具备正确的 ARIA 语义：按钮使用 `<button>` 或 `role="button"`、列表使用 `<ul>/<li>` 或 `role="list/listitem"`、对话框使用 `role="dialog"` + `aria-modal="true"`、进度条使用 `role="progressbar"` + `aria-valuenow/min/max`、Toast 使用 `role="alert"`。动态内容区域使用 `aria-live="polite"`（如流式回答区、进度文本）。
- 验收标准：
  - AC-1：全部可点击元素为 `<button>` 或 `role="button"`（E2E 断言）。
  - AC-2：模态框具有 `role="dialog"` + `aria-modal="true"` + `aria-labelledby`。
  - AC-3：进度条具有 `role="progressbar"` + `aria-valuenow` + `aria-valuemin` + `aria-valuemax`。
  - AC-4：Toast 具有 `role="alert"`。
  - AC-5：流式回答区具有 `aria-live="polite"`，token 推送时屏幕阅读器可感知。
  - AC-6：图标按钮具有 `aria-label` 描述功能。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-A11Y-002 焦点管理
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-UI-001
- **v1.0 发布分类**：v1.0.1 P1 Draft 收尾实现（Session 2，Focus Trap + 焦点恢复 + Tab 顺序优化） ｜ 完成日期：2026-08-02
- 描述：完整的键盘焦点管理：所有交互元素可通过 Tab 键到达、可见的 focus 环（`outline: 2px solid primary` + `outline-offset: 2px`）、模态框打开时焦点锁定在模态框内（focus trap）、模态框关闭后焦点回到触发元素、Tab 顺序符合视觉顺序（从左到右、从上到下）。
- 验收标准：
  - AC-1：全部交互元素可通过 Tab 键到达（E2E Tab 遍历断言）。
  - AC-2：聚焦元素显示 2px primary 色 focus 环 + 2px offset。
  - AC-3：模态框打开后 Tab 键仅在模态框内元素间循环（focus trap）。
  - AC-4：模态框关闭后焦点回到打开模态框的触发元素。
  - AC-5：Tab 顺序符合视觉顺序（左→右、上→下）。
  - AC-6：`tabindex="-1"` 不用于可交互元素（避免 Tab 跳过）。
- 契约：`ui/src/focus-trap.js` — `createFocusTrap(element)` ｜ 关联测试：TC-A11Y-002-001~006

#### REQ-A11Y-003 颜色对比度
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-DS-001
- **v1.0 发布分类**：v1.0.1 P1 Draft 收尾实现（Session 3，axe-core 全量扫描 + 色值修复） ｜ 完成日期：2026-08-02
- 描述：文本与背景色对比度满足 WCAG 2.1 AA 标准：正文文本（< 18px）对比度 ≥ 4.5:1、大文本（≥ 18px 或 ≥ 14px bold）对比度 ≥ 3:1、交互元素（按钮、链接）对比度 ≥ 3:1、禁用状态文本对比度 ≥ 2:1（可降低但需可辨识）。非文本元素（图标、边框）对比度 ≥ 3:1。
- 验收标准：
  - AC-1：正文文本与背景对比度 ≥ 4.5:1（自动化对比度检测断言）。
  - AC-2：大文本对比度 ≥ 3:1。
  - AC-3：按钮文本与按钮背景对比度 ≥ 4.5:1。
  - AC-4：禁用状态文本对比度 ≥ 2:1。
  - AC-5：图标与背景对比度 ≥ 3:1。
- 契约：`ui/styles/tokens.css` — `--text-quaternary: #8995A8`（WCAG AA 6.53:1 on surface-0）｜ 关联测试：TC-A11Y-003-001~004

#### REQ-A11Y-004 屏幕阅读器支持
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-A11Y-001
- 描述：关键交互流程支持屏幕阅读器（NVDA / VoiceOver）：文档状态变化时通过 `aria-live` 区域播报（如「文档导入完成」）、流式回答开始时播报「正在生成回答」、回答结束时播报「回答完成」、错误发生时播报错误摘要。纯装饰性元素使用 `aria-hidden="true"`。
- 验收标准：
  - AC-1：文档状态变化时 `aria-live` 区域更新，屏幕阅读器可播报。
  - AC-2：流式回答开始时播报「正在生成回答」。
  - AC-3：回答结束时播报「回答完成」。
  - AC-4：错误发生时播报错误摘要。
  - AC-5：装饰性图标使用 `aria-hidden="true"`，不被屏幕阅读器读取。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-A11Y-005 高对比度模式适配
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-DS-001
- 描述：尊重系统高对比度模式（`prefers-contrast: more`）：在高对比度模式下，边框加粗至 2px、文本色调整为更高对比度、禁用半透明效果。确保高对比度模式下所有文本与交互元素仍清晰可辨。
- 验收标准：
  - AC-1：系统启用高对比度模式时，UI 自动适配（边框加粗、对比度提升）。
  - AC-2：高对比度模式下全部文本对比度 ≥ 7:1（AAA 标准）。
  - AC-3：高对比度模式下禁用半透明遮罩，改用纯色边框区分层级。
- 契约：TBD ｜ 关联测试：TBD

---

### 4.18 NAV — 导航与信息架构

> **域背景**：当前应用导航为固定双栏布局（左栏会话/知识库 + 右栏对话区），缺乏全局搜索、面包屑导航、快速跳转等高效导航能力。随着功能和数据量增长，系统化的导航与信息架构成为必要。

#### REQ-NAV-001 侧栏折叠与展开
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-UI-001
- **v1.0 发布分类**：v1.0 力争交付 ｜ 决策日期：2026-08-01
- 描述：左侧栏支持折叠/展开切换：折叠后仅显示图标列（会话图标、知识库图标、设置图标），展开后显示完整列表。折叠状态持久化到 settings 表。折叠/展开有宽度过渡动画（250ms）。折叠状态下点击图标弹出浮层面板（类似 macOS Dock），显示对应完整列表。
- 验收标准：
  - AC-1：点击折叠按钮后侧栏宽度从完整宽度过渡到图标列宽度（250ms 动画）。
  - AC-2：折叠状态下显示会话 / 知识库 / 设置三个图标。
  - AC-3：折叠状态下点击图标弹出浮层面板显示完整列表。
  - AC-4：折叠状态持久化到 settings 表，重启后恢复。
  - AC-5：浮层面板点击外部关闭，点击列表项进入对应视图并展开侧栏。
- 契约：`toggleSidebar()` + `restoreSidebarState()` + `#sidebarIcons` + `#sidebarExpanded` + `#sidebarFlyout` + `get_sidebar_collapsed` / `set_sidebar_collapsed` IPC ｜ 关联测试：TC-NAV-001-001

#### REQ-NAV-002 全局搜索
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-RAG-011、REQ-ING-007
- 描述：提供全局搜索入口（侧栏顶部搜索图标或 `Cmd/Ctrl+Shift+F`），同时搜索会话标题、文档名、消息内容（已持久化的 assistant 回答）。搜索结果按类型分组（会话 / 文档 / 消息），每组显示前 5 条匹配。点击结果项跳转到对应位置（会话 → 加载会话、文档 → 打开预览、消息 → 加载会话并定位消息）。
- 验收标准：
  - AC-1：`Cmd/Ctrl+Shift+F` 打开全局搜索面板。
  - AC-2：搜索结果按类型分组（会话 / 文档 / 消息），每组最多显示 5 条。
  - AC-3：点击会话结果项加载该会话。
  - AC-4：点击文档结果项打开文档预览面板。
  - AC-5：点击消息结果项加载对应会话并滚动定位到该消息。
  - AC-6：搜索关键词在结果项中高亮。
  - AC-7：无结果时显示「未找到匹配内容」空状态。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-NAV-003 侧栏分区切换
- 优先级：P1 ｜ 状态：Deprecated ｜ 依赖：REQ-UI-001
- **v1.0 发布分类**：Deprecated（v1.0.1 P1 Draft 收尾，Session 4） ｜ 决策日期：2026-08-02
- **废弃说明**（2026-08-02）：当前设计决策为会话+知识库同屏显示（无需 Tab 切换），此需求已被 REQ-NAV-001 侧栏折叠与展开功能 + REQ-NAV-008 顶部工具栏与知识库弹框替代。同屏显示方案减少了用户切换成本，知识库弹框按需打开进一步优化空间利用率。
- 描述：左侧栏分为「会话」与「知识库」两个分区标签（Tab），点击切换显示对应列表。当前活跃分区高亮。分区选择持久化到 settings 表。会话分区显示会话列表 + 新建按钮；知识库分区显示文档列表 + 导入按钮 + 配额信息。
- 验收标准：
  - AC-1：侧栏顶部显示「会话」和「知识库」两个分区标签。
  - AC-2：点击标签切换显示对应列表，当前标签高亮。
  - AC-3：分区选择持久化到 settings 表，重启后恢复上次分区。
  - AC-4：「会话」分区顶部显示新建会话按钮。
  - AC-5：「知识库」分区底部显示配额用量（免费版 n/50）。
- 契约：已废弃 ｜ 替代方案：REQ-NAV-001（侧栏折叠与展开）+ REQ-NAV-008（顶部工具栏与知识库弹框）｜ 关联测试：无（已废弃）

#### REQ-NAV-004 面包屑与上下文指示
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-UI-001
- 描述：对话区顶部显示面包屑导航：`知识库名 > 会话标题`，点击知识库名可跳转到知识库分区，点击会话标题可编辑重命名。面包屑区域同时显示当前会话的消息数与创建时间。
- 验收标准：
  - AC-1：对话区顶部显示 `知识库名 > 会话标题` 面包屑。
  - AC-2：点击知识库名跳转到知识库分区。
  - AC-3：点击会话标题进入重命名编辑模式。
  - AC-4：面包屑右侧显示消息数与创建时间。
  - AC-5：空会话时面包屑显示「新会话」，消息数为 0。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-NAV-005 回到顶部与滚动定位
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-UI-002
- 描述：对话区滚动超过一屏后，右下角显示「回到顶部」浮动按钮（带向上箭头图标）。点击后平滑滚动到对话区顶部。流式生成中自动滚动到底部（现有行为），用户手动向上滚动后暂停自动滚动（避免抢占），显示「新消息」提示按钮，点击滚动到底部。
- 验收标准：
  - AC-1：对话区滚动超过一屏后右下角出现「回到顶部」按钮。
  - AC-2：点击「回到顶部」平滑滚动到顶部（400ms 动画）。
  - AC-3：流式生成中自动滚动到底部。
  - AC-4：用户手动向上滚动后暂停自动滚动。
  - AC-5：暂停自动滚动后显示「↓ 新消息」提示按钮，点击后滚动到底部并恢复自动滚动。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-NAV-006 会话列表搜索
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-UI-001、REQ-RAG-006
- 描述：会话历史列表上方提供搜索框（`#convSearchInput`），支持按会话标题模糊搜索。搜索结果实时过滤（客户端 `display:none` 切换），无需后端请求。搜索逻辑与文档列表搜索（`#docSearchInput`）保持一致的模式：`input` 事件触发，大小写不敏感，`data-conv-title` 属性匹配。此需求解决会话数量增长后用户无法快速定位历史会话的问题。
- 验收标准：
  - AC-1：会话列表上方显示搜索输入框 `#convSearchInput`，placeholder 为「搜索会话…」。
  - AC-2：输入关键词后，列表仅显示标题包含该关键词的会话（大小写不敏感）。
  - AC-3：清空搜索框后恢复显示全部会话。
  - AC-4：搜索不发起后端请求，纯客户端 DOM 过滤。
  - AC-5：会话列表项具有 `data-conv-title` 属性，值为完整会话标题。
- 契约：`#convSearchInput` `input` 事件监听器 + `data-conv-title` 属性 ｜ 关联测试：TBD

#### REQ-NAV-007 会话列表分页与时间分组
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-RAG-006、REQ-DB-001
- 描述：会话历史列表支持分页加载与时间分组展示。首次加载最近 50 条会话（`CONV_PAGE_SIZE = 50`），滚动到列表底部时自动加载下一页（无限滚动）。会话按创建时间分组，分组标签包括「今天」「昨天」「前天」「一周内」「一个月内」「YYYY年MM月」，每个分组的日期头在滚动时 sticky 固定到顶部（`position: sticky; top: 0`）。后端新增 `get_conversations_paginated(workspace_id, limit, offset) -> Result<PaginatedResult<Conversation>, String>` IPC 命令，返回 `PaginatedResult<Conversation>`（含 `items` 与 `total`）。此需求解决会话数量增长后全量加载导致的性能问题与查找效率问题。
- 验收标准：
  - AC-1：首次加载会话列表仅拉取最近 50 条，而非全量加载。
  - AC-2：滚动到列表底部 100px 范围内时自动加载下一页会话。
  - AC-3：会话按时间分组展示，分组标签为「今天」「昨天」「前天」「一周内」「一个月内」「YYYY年MM月」。
  - AC-4：滚动时当前分组的日期头固定在列表顶部（sticky），切换分组时日期头平滑替换。
  - AC-5：`get_conversations_paginated` 命令支持 `limit` / `offset` 参数，返回 `PaginatedResult<Conversation>`。
  - AC-6：全部会话加载完毕后（`convOffset >= convTotal`），不再发起分页请求。
- 契约：Command `get_conversations_paginated(workspace_id: String, limit: Option<usize>, offset: Option<usize>) -> Result<PaginatedResult<Conversation>, String>`；Storage trait `list_conversations_paginated` + `count_conversations` ｜ 关联测试：TC-NAV-007

#### REQ-NAV-008 顶部工具栏与知识库弹框
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-UI-001、REQ-ING-007
- 描述：原侧栏底部的设置按钮移至顶部工具栏（`#topBar`），位于 macOS traffic lights 右侧。知识库文档列表与搜索从侧栏内联区域改为弹框形式（`#kbModal`），通过顶部工具栏的知识库图标按钮（`#kbBtn`）打开。弹框包含文档搜索框、文档列表、配额计数、导入按钮。侧栏仅保留会话历史列表。此需求优化空间利用率，将低频使用的知识库管理功能从常驻侧栏移至按需弹框。
- 验收标准：
  - AC-1：顶部工具栏（`#topBar`）显示知识库图标（`#kbBtn`）和设置图标（`#settingsBtn`），位于 macOS traffic lights 右侧（`padding-left: 78px`）。
  - AC-2：点击知识库图标打开 `#kbModal` 弹框，弹框包含文档搜索框、文档列表、配额计数、导入按钮。
  - AC-3：弹框支持点击背景关闭、ESC 键关闭。
  - AC-4：侧栏不再包含知识库文档列表和搜索框，仅保留会话历史。
  - AC-5：弹框打开时自动加载文档列表并更新配额计数。
- 契约：`#topBar` + `#kbBtn` + `#kbModal` + `openKbModal()` / `closeKbModal()` / `initKbModal()` ｜ 关联测试：TC-NAV-008

#### REQ-NAV-009 侧栏完全折叠与展开
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-UI-001
- 描述：侧栏折叠行为从「收窄为图标列」改为「完全隐藏」（width=0）。折叠后顶部工具栏显示展开按钮（`#expandBtn`），点击展开按钮恢复侧栏显示，展开完成后展开按钮自动消失。折叠按钮（`#collapseBtn`）仅在侧栏展开时显示。此需求最大化聊天区可用宽度。
- 验收标准：
  - AC-1：点击折叠按钮后侧栏完全隐藏（`sidebar-hidden` class，width=0），不占任何空间。
  - AC-2：侧栏隐藏后顶部工具栏显示展开按钮（`#expandBtn`），折叠按钮（`#collapseBtn`）隐藏。
  - AC-3：点击展开按钮后侧栏恢复显示（width=240px），展开按钮隐藏，折叠按钮恢复显示。
  - AC-4：折叠/展开有 CSS 过渡动画（`transition-all duration-normal`）。
- 契约：`toggleSidebar()` + `#collapseBtn` + `#expandBtn` + `.sidebar-hidden` CSS class ｜ 关联测试：TC-NAV-009

---

### 4.19 WS — 工作空间与多知识库管理

> **域背景**：当前应用为单知识库模式（所有文档在同一库中），用户无法按主题/项目隔离文档。多知识库管理允许用户创建独立工作空间（如「论文库」「技术文档库」「个人笔记库」），各库独立检索、独立配额、独立会话。

#### REQ-WS-001 多知识库创建与切换
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-DB-001
- 描述：用户可创建多个知识库（工作空间），每个知识库有独立名称、独立文档集、独立会话列表。侧栏顶部显示知识库选择器（下拉或标签），切换后文档列表与会话列表同步切换。知识库数据存储在同一 SQLite 数据库中，通过 `workspace_id` 字段隔离。
- 验收标准：
  - AC-1：侧栏顶部显示知识库选择器，列出全部知识库。
  - AC-2：点击「新建知识库」弹出输入框，输入名称后创建新知识库。
  - AC-3：切换知识库后文档列表与会话列表同步更新为该库的数据。
  - AC-4：不同知识库的文档与会话相互隔离，互不影响。
  - AC-5：当前知识库选择持久化到 settings 表，重启后恢复。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-WS-002 知识库配额独立计算
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-WS-001、REQ-LIC-002
- 描述：免费版每个知识库独立计算 50 文件配额（非全局 50 文件）。切换知识库时配额显示同步更新。Pro 版不受配额限制。
- 验收标准：
  - AC-1：免费版每个知识库独立计算文件数，上限 50。
  - AC-2：切换知识库后配额显示更新为该库的用量。
  - AC-3：一个知识库满 50 文件不影响其他知识库导入。
  - AC-4：Pro 版所有知识库均不受配额限制。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-WS-003 知识库重命名与删除
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-WS-001
- 描述：用户可重命名知识库（双击名称或右键菜单）。删除知识库时级联删除该库全部文档、chunks、向量、会话、消息，删除前弹出确认对话框明确列出将删除的数据量。不允许删除最后一个知识库（至少保留一个）。
- 验收标准：
  - AC-1：双击知识库名称或右键菜单选择「重命名」进入编辑模式。
  - AC-2：重命名后即时更新侧栏显示。
  - AC-3：删除知识库前弹出确认对话框，显示「将删除 N 个文档、M 个会话」。
  - AC-4：确认删除后级联清理该库全部数据。
  - AC-5：只有一个知识库时删除按钮禁用，提示「至少保留一个知识库」。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-WS-004 文档跨知识库迁移
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-WS-001
- 描述：用户可将文档从一个知识库迁移到另一个知识库（右键菜单 > 迁移至 > 选择目标库）。迁移后文档的 chunks 与向量一并迁移，原知识库文件计数释放，目标知识库文件计数增加。迁移不重新解析或嵌入（复用现有索引数据）。
- 验收标准：
  - AC-1：右键文档选择「迁移至」弹出知识库选择列表。
  - AC-2：选择目标库后文档及其 chunks / 向量迁移到目标库。
  - AC-3：迁移后原库文件计数减少，目标库文件计数增加。
  - AC-4：迁移不重新解析或嵌入（索引数据复用）。
  - AC-5：目标库配额不足时拒绝迁移并提示。
- 契约：TBD ｜ 关联测试：TBD

---

### 4.20 EXP — 数据导出与备份恢复

> **域背景**：当前应用数据完全存储在本地 SQLite 中，用户无法导出对话记录或备份知识库。数据导出与备份恢复是数据安全的基本保障，也是用户迁移数据的必要能力。

#### REQ-EXP-001 对话导出为 Markdown
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-RAG-006
- 描述：用户可将单个会话导出为 Markdown 文件。导出内容包括：会话标题、创建时间、全部消息（user / assistant 按序排列）、每条 assistant 消息附带引用来源列表。导出文件以会话标题命名，通过系统保存对话框选择保存位置。
- 验收标准：
  - AC-1：会话列表项右键菜单或会话操作按钮提供「导出为 Markdown」选项。
  - AC-2：导出的 Markdown 文件包含会话标题、创建时间、全部消息。
  - AC-3：消息按 user / assistant 交替排列，角色标记清晰。
  - AC-4：Assistant 消息末尾附带引用来源列表（文档名 + chunk 序号）。
  - AC-5：导出文件名默认为会话标题，用户可在保存对话框中修改。
  - AC-6：代码块、Mermaid 语法、LaTeX 公式以原始 Markdown 语法保留。
- 契约：`export_conversation_to_markdown(conversation, messages) -> String`（`crates/core/src/export.rs`）；IPC：`export_conversation_markdown(conversation_id) -> (String, String)` + `save_text_file(path, content) -> ()`（`crates/tauri-app/src/commands.rs`）｜ 关联测试：TC-EXP-001~010（core）、TC-EXP-001~005（integration）

#### REQ-EXP-002 全量数据备份
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-DB-001
- 描述：设置面板提供「备份数据」功能，将应用数据目录中的全部内容（SQLite 数据库文件 + 文档副本 + secret.key + 模型缓存元信息）打包为 `.echomind-backup` 压缩文件（zip 格式）。备份文件包含版本号标记，便于后续恢复时校验兼容性。
- 验收标准：
  - AC-1：设置面板「数据管理」区显示「备份数据」按钮。
  - AC-2：点击后弹出系统保存对话框，选择位置后开始备份。
  - AC-3：备份文件为 `.echomind-backup` 格式（zip），包含全部应用数据。
  - AC-4：备份文件包含版本号标记（如 `backup_version: 1`）。
  - AC-5：备份过程中显示进度条，完成后显示「备份完成」toast。
  - AC-6：备份文件不含明文 API Key（Key 以加密形态备份）。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-EXP-003 全量数据恢复
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-EXP-002
- 描述：设置面板提供「恢复数据」功能，从 `.echomind-backup` 文件恢复应用数据。恢复前弹出确认对话框，明确提示「将覆盖现有全部数据」。恢复后应用自动重启以加载新数据。恢复前校验备份文件版本号与完整性。
- 验收标准：
  - AC-1：设置面板「数据管理」区显示「恢复数据」按钮。
  - AC-2：点击后弹出系统文件选择器，选择 `.echomind-backup` 文件。
  - AC-3：恢复前弹出确认对话框，提示「将覆盖现有全部数据，此操作不可撤销」。
  - AC-4：恢复前校验备份文件完整性（zip 结构 + 版本号），校验失败显示明确错误。
  - AC-5：恢复完成后应用自动重启。
  - AC-6：恢复后全部数据（文档、会话、配置）完整可用。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-EXP-004 文档原文导出
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-ING-005
- 描述：用户可从知识库导出文档的原始文件副本。文档列表项右键菜单提供「导出原文」选项，通过系统保存对话框选择保存位置。导出的文件与导入时完全一致（从数据目录复制原始副本）。
- 验收标准：
  - AC-1：文档列表项右键菜单提供「导出原文」选项。
  - AC-2：点击后弹出系统保存对话框，默认文件名为原文档名。
  - AC-3：导出的文件与导入时完全一致（字节级一致）。
  - AC-4：批量多选模式下可批量导出（保存到指定文件夹）。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-EXP-005 对话/文档导出为 PDF
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-EXP-001
- **v1.2 实现**：S49 ｜ 完成日期：2026-08-06
- **v1.2 规划**：S49 ｜ 决策日期：2026-08-06
- 描述：用户可将对话或知识库文档导出为 PDF 文件。导出通过 Tauri WebView 原生 `window.print()` 实现——前端构建打印专用 HTML 模板（复用 Markdown 渲染管线），添加 `@media print` CSS 规则（隐藏导航/侧栏/输入框，优化字体/间距/分页），调用 `window.print()` 触发浏览器打印对话框，用户选择「保存为 PDF」完成导出。零新增 Rust 依赖。
- 验收标准：
  - AC-1：对话操作菜单提供「导出为 PDF」选项（TC-EXP-PDF-001）。
  - AC-2：点击后触发 `window.print()`，打开浏览器打印对话框（TC-EXP-PDF-002）。
  - AC-3：打印内容包含对话全部消息文本（TC-EXP-PDF-003）。
  - AC-4：`@media print` CSS 隐藏侧栏/导航/输入框等 UI 元素（TC-EXP-PDF-004）。
  - AC-5：空对话导出显示提示，不崩溃（TC-EXP-PDF-005）。
- 契约：`exportConversationToPdf(conversationId, title)` / `exportDocumentToPdf(docId)`（`ui/src/export.js`，纯前端）｜ `@media print`（`ui/styles/print.css`）｜ 关联测试：TC-EXP-PDF-001~005
- 调研依据：Tauri v2 WebView（Wry）原生支持 `window.print()`；参考 AnythingLLM / Open WebUI 均使用浏览器打印对话框导出 PDF。

---

### 4.21 ERR — 错误处理与降级策略

> **域背景**：当前错误处理为各功能域内零散定义（如 toast 反馈、API Key 脱敏），缺乏系统化的错误分类、恢复策略与降级机制。本域规格将每条错误需求原子化为「错误类型 → 前缀 → 用户可见消息 → Toast 样式 → 恢复建议 → 后端契约」的六元组，确保错误处理无歧义、可测试。

#### REQ-ERR-001 统一错误分类体系
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-UI-005
- **v1.0 发布分类**：v1.0 必须交付（已实现全部 10 个错误前缀 + classify_llm_error 分类函数 + 前端 toastError 前缀感知） ｜ 决策日期：2026-08-01 ｜ 完成日期：2026-08-01

**错误前缀与消息模板**：

| 前缀 | 错误类型 | 用户可见消息模板 | 恢复建议 | Toast kind |
|---|---|---|---|---|
| `NETWORK:` | 网络连接错误（超时、连接拒绝、DNS 失败） | `NETWORK: 无法连接到 {endpoint}（{detail}）` | 请检查网络连接或端点地址 | `error`（红色） |
| `AUTH:` | API Key 无效/过期（401/403） | `AUTH: API Key 无效或已过期（HTTP {status}）` | 请检查 API Key 配置 | `error`（红色） |
| `QUOTA:` | 配额超限（免费版 50 文件） | `LIMIT_REACHED: 已达免费版上限（{current}/50）` | 升级到 Pro 版 | `error`（红色）→ 触发付费墙 |
| `PRO_REQUIRED:` | Pro 专属功能限制（PDF 导入） | `PRO_REQUIRED: {feature} 为 Pro 功能` | 升级到 Pro 版 | `error`（红色）→ 触发付费墙 |
| `PARSE:` | 文件解析失败 | `PARSE: 无法解析文件「{filename}」（{detail}）` | 请检查文件格式是否正确 | `error`（红色） |
| `EMBED:` | 嵌入失败 | `EMBED: 向量化失败（{detail}）` | 请重试或检查模型缓存 | `error`（红色） |
| `LLM:` | LLM 调用失败（非网络/非鉴权） | `LLM: 模型调用失败（{detail}）` | 请检查模型名称或稍后重试 | `error`（红色） |
| `STORAGE:` | 存储错误（SQLite 操作失败） | `STORAGE: 数据存储异常（{detail}）` | 请重启应用或检查磁盘空间 | `error`（红色） |
| `VALIDATION:` | 输入校验失败 | `VALIDATION: {field} {reason}` | 按提示修正输入 | `error`（红色） |
| `UNKNOWN:` | 未分类错误 | `UNKNOWN: 发生未知错误（{detail}）` | 请重启应用，如问题持续请联系支持 | `error`（红色） |

**Toast 与前端联动规格**：

| 错误前缀 | 前端处理 |
|---|---|
| `NETWORK:` | `toastError(err)` + `setInputState('error', sanitizeError(err))` |
| `AUTH:` | `toastError(err)` + `setInputState('error', sanitizeError(err))` |
| `LIMIT_REACHED:` / `PRO_REQUIRED:` | `showPaywall(reason)` — 弹出付费墙 Modal（`#paywall`） |
| `PARSE:` / `EMBED:` | `toast(message, 'error')` — `doc-status-changed` 事件含 `status: 'error'` |
| `LLM:` | `toastError(err)` + `finalizeStream(false)` + `setInputState('error', ...)` |
| `STORAGE:` | `toastError(err)` |
| `VALIDATION:` | `#wizError` 显示错误（向导内联）或 `toastError(err)` |
| `UNKNOWN:` | `toastError(err)` |

**前端 `sanitizeError()` 脱敏规则**（REQ-UI-005 已冻结）：
- API Key：`/sk-[a-zA-Z0-9]{8,}/g` → `'sk-****'`
- Windows 用户路径：`/\\Users\\[^\\]+?\\/g` → `'\\Users\\****\\'`
- Unix 用户路径：`/\/Users\/[^/]+?\//g` → `'/Users/****/'`

**已实现错误前缀**：
- 全部 10 个前缀已实现（`crates/core/src/errors.rs`）
- `classify_llm_error(err)` — 根据 OpenAIProvider 错误消息自动分类 NETWORK/AUTH/LLM
- `prefix_error(prefix, detail)` — 格式化 `{PREFIX}: {detail}`
- `has_error_prefix(err)` — 判断消息是否已携带前缀（避免重复包装）
- `chat_inner()` — LLM 调用错误经 `classify_llm_error` 分类；查询长度校验返回 `VALIDATION:`
- `chat` 命令 — 兜底包装无前缀错误为 `UNKNOWN:`
- `import_files_inner()` — 解析失败 `PARSE:` / 嵌入失败 `EMBED:` / 导入失败 `STORAGE:`
- `update_llm_config_inner()` — API Key 长度 + Base URL 格式校验返回 `VALIDATION:`
- 前端 `toastError()` — 解析前缀选择 toast kind + 用户友好消息（`ui/src/toast.js`）
- 前端 `chat_error` 事件监听 — `toastError(msg)` + `setInputState('error', ...)` 恢复输入框
- 前端 `doc-status-changed` 事件监听 — `toastError(p.message)` 解析 PARSE/EMBED 前缀

- 验收标准：
  - AC-1：后端 `import_files` 在配额超限时返回 `LIMIT_REACHED: {reason}`。
  - AC-2：后端 `import_files` 在 PDF 非Pro时返回 `PRO_REQUIRED: {reason}`。
  - AC-3：前端 `importPaths()` catch 块识别 `LIMIT_REACHED` / `PRO_REQUIRED` 前缀并弹出 `showPaywall(reason)`。
  - AC-4：其他错误经 `toastError(err)` 统一展示，含 `sanitizeError()` 脱敏。
  - AC-5：错误消息中不出现 API Key 明文（`sanitizeError` 过滤后为 `sk-****`）。
  - AC-6：（Draft）后端所有 IPC 命令返回的错误字符串以 `PREFIX:` 格式开头。
  - AC-7：（Draft）前端 `toast` 根据 prefix 选择合适的 kind（`QUOTA` → 触发付费墙而非普通 toast）。
- 契约：`sanitizeError(err)` 函数 + `toastError(err)` 函数 + `showPaywall(reason)` 函数 + `importPaths()` catch 块前缀匹配逻辑。
- 关联测试：TC-LIC-002（配额拒绝）、TC-LIC-003（PDF 付费墙）

#### REQ-ERR-002 网络错误自动重试
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-LLM-001、REQ-ERR-001

**重试策略规格**：

| 参数 | 值 | 说明 |
|---|---|---|
| 最大重试次数 | 3 | 第 1~3 次重试 |
| 退避策略 | 指数退避 | 1s → 2s → 4s |
| 重试触发条件 | 超时 / 连接拒绝 / 5xx | HTTP 500/502/503/504 |
| 不重试条件 | 4xx（除 429） | 401/403 → 直接返回 `AUTH:`；400 → 返回 `LLM:` |
| 429 处理 | 重试（Rate Limit） | 退避间隔加倍：2s → 4s → 8s |

**重试状态机**：

| 阶段 | 后端行为 | 前端行为 | 事件 |
|---|---|---|---|
| 首次请求失败 | 判断错误类型 | — | — |
| 可重试错误 | 等待 1s | `#inputHint` = "重连中…（1/3）" | `chat_phase` `{phase: "retrying", attempt: 1}` |
| 第 1 次重试失败 | 等待 2s | `#inputHint` = "重连中…（2/3）" | `chat_phase` `{phase: "retrying", attempt: 2}` |
| 第 2 次重试失败 | 等待 4s | `#inputHint` = "重连中…（3/3）" | `chat_phase` `{phase: "retrying", attempt: 3}` |
| 第 3 次重试成功 | 继续流式输出 | 恢复 `#inputHint` = "正在生成回答…" | `chat_token` 事件恢复 |
| 全部重试耗尽 | 返回 `NETWORK:` 错误 | `toastError(err)` + `setInputState('error', ...)` | `chat_error` 事件 |
| 不可重试错误（4xx） | 直接返回 `AUTH:` / `LLM:` | `toastError(err)` + `setInputState('error', ...)` | `chat_error` 事件 |

**当前实现状态**：
- 后端 `OpenAIProvider` 已有连接超时（30s）和请求超时（300s），但**无自动重试逻辑**（Draft）。
- 前端 `chat_phase` 事件已支持 `phase` 字段，但无 `retrying` phase（Draft）。

- 验收标准：
  - AC-1：（Draft）LLM 调用超时或 5xx 错误时自动重试，退避 1s→2s→4s。
  - AC-2：（Draft）重试期间 `#inputHint` 显示"重连中…（n/3）"。
  - AC-3：（Draft）重试成功后继续流式输出，`chat_token` 事件恢复。
  - AC-4：（Draft）重试 3 次仍失败后返回 `NETWORK:` 错误，`toastError` + `setInputState('error')`。
  - AC-5：（Draft）401/403 错误不重试，直接返回 `AUTH:` 错误。
- 契约：`OpenAIProvider.chat_stream()` 重试逻辑（Draft）+ `chat_phase` 事件新增 `retrying` phase（Draft）+ `#inputHint` 文案更新。
- 关联测试：TBD

#### REQ-ERR-003 离线模式降级
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-ERR-001

**离线模式 UI 元素规格**：

| 元素 | 位置 | Tailwind 类 | 可见条件 |
|---|---|---|---|
| 离线指示器 | `#proStatus` 旁（侧栏底部） | `text-amber-400 text-xs` + `⚠` 图标 | `offline === true` |
| `#inputHint` 离线文案 | 输入框下方 | `text-amber-400` | `offline === true` |
| `#sendBtn` | 输入框内 | `disabled:opacity-30` + `disabled` 属性 | `offline === true` |
| `#queryInput` placeholder | 输入框 | `placeholder="离线模式 — 请检查网络连接"` | `offline === true` |

**离线模式状态矩阵**：

| 功能 | 在线态 | 离线态 |
|---|---|---|
| 对话（`chat` IPC） | 可用 | 禁用，`#sendBtn` disabled |
| 文档导入（`import_files`） | 可用 | 可用（本地操作） |
| 文档删除（`delete_document`） | 可用 | 可用 |
| 会话历史查看 | 可用 | 可用 |
| 设置面板 | 可用 | 可用 |
| 模型下载（`init_embedder`） | 可用 | 禁用（需网络） |
| LLM 配置测试（`test_llm_connection`） | 可用 | 禁用 |

**离线检测机制**：
- 检测方式：`chat` IPC 调用失败且错误前缀为 `NETWORK:` 时进入离线模式。
- 退出方式：定期（每 30s）或用户操作时尝试 `test_llm_connection`，成功则退出离线模式。
- 退出事件：`chat_phase` `{phase: "online"}` → 恢复 `#sendBtn` enabled + `#inputHint` 清空 + `#queryInput` placeholder 恢复。

- 验收标准：
  - AC-1：（Draft）`chat` IPC 返回 `NETWORK:` 错误时 `#inputHint` 显示"离线模式 — 请检查网络连接"。
  - AC-2：（Draft）离线模式下 `#sendBtn` `disabled`，`#queryInput` placeholder 变为离线提示。
  - AC-3：（Draft）离线模式下文档导入/删除/会话历史/设置功能正常可用。
  - AC-4：（Draft）网络恢复后自动退出离线模式，`#sendBtn` 恢复 enabled。
  - AC-5：（Draft）侧栏 `#proStatus` 旁显示 `⚠` 离线指示器（amber 色）。
- 契约：（Draft）`offline` 全局变量 + `setOfflineMode(bool)` 函数 + `#sendBtn` disabled 控制 + `#inputHint` 文案切换 + `#queryInput` placeholder 切换。
- 关联测试：TBD

#### REQ-ERR-004 崩溃恢复与数据一致性
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-DB-001
- **v1.0 发布分类**：v1.0 必须交付（已实现 cleanup_zombies + persist_exchange + PRAGMA integrity_check + dbError Modal） ｜ 决策日期：2026-08-01 ｜ 完成日期：2026-08-01

**崩溃恢复流程**（启动时执行）：

| 步骤 | 操作 | 实现位置 | 当前状态 |
|---|---|---|---|
| 1 | `cleanup_zombies()` — 将 `Processing` 状态文档标记为 `Failed` | `SqliteStorage::cleanup_zombies()` | ✅ 已实现 |
| 2 | `PRAGMA integrity_check` — 检查 SQLite 数据库完整性 | `SqliteStorage::check_integrity()` | ✅ 已实现 |
| 3 | 完整性检查通过 → 正常启动 | `lib.rs setup` | ✅ 已实现 |
| 4 | 完整性检查失败 → 提示用户「数据库异常」 | `#dbError` Modal + `db-integrity-error` 事件 | ✅ 已实现 |
| 5 | 流式生成中断的会话保留已生成内容 | `persist_exchange()` 在 `chat_done` / `chat_error` 时调用 | ✅ 已实现 |

**`cleanup_zombies()` 规格**（已实现）：
- SQL：`UPDATE documents SET status = 'Failed' WHERE status = 'Processing'`
- 调用时机：`SqliteStorage::new()` 初始化时。
- 影响：所有 `Processing` 状态文档（崩溃时正在索引）变为 `Failed`，用户可点击 `↻` 重试。

**数据库完整性检查规格**（已实现）：
- SQL：`PRAGMA integrity_check`
- 返回值：`ok` → 正常启动；其他 → 损坏。
- 实现类型：`IntegrityCheckResult { Ok, Corrupted(String) }`
- 损坏处理：
  - 尝试 WAL 恢复：`PRAGMA wal_checkpoint(TRUNCATE)` 后重新检查。
  - 仍失败 → 通过 `db-integrity-error` 事件通知前端弹出 Modal（`#dbError`）：
    - 标题："数据库异常"
    - 正文："数据存储可能已损坏。建议从备份恢复或重置数据目录。"
    - 按钮：「打开数据目录」→ `open_data_dir` IPC → `tauri_plugin_opener.openPath()` / 「知道了」→ 关闭 Modal

**流式中断保留规格**（已实现）：
- `abort_chat` IPC → `CancellationToken.cancel()` → `forward_stream()` 循环中断。
- 中断后 `persist_exchange()` 保存已生成的部分内容到 `messages` 表。
- 前端在 `.md` 区追加"⏹ 已中断"标记。

- 验收标准：
  - AC-1：崩溃后重启时 `Processing` 状态文档被标记为 `Failed`（现有行为，`cleanup_zombies()`）。
  - AC-2：重启时执行 `PRAGMA integrity_check`，返回 `ok` 则正常启动。
  - AC-3：数据库损坏时弹出 Modal 提示"数据库异常"，含「打开数据目录」按钮。
  - AC-4：流式生成中断的会话保留已生成内容（`persist_exchange()` 在中断时调用）。
- 契约：`SqliteStorage::cleanup_zombies()` + `SqliteStorage::check_integrity()` + `IntegrityCheckResult` + `persist_exchange()` + `#dbError` Modal + `open_data_dir` IPC。
- 关联测试：TC-ERR-004（完整性检查）、TC-ERR-005（open_data_dir）、TC-DB-002（崩溃恢复）、TC-RAG-003（中断保留）

#### REQ-ERR-005 输入校验与防御
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-SEC-001
- **v1.0 发布分类**：v1.0 必须交付（已实现路径遍历防护 + 扩展名校验 + License 验证 + 向导前置校验 + API Key 长度/URL 格式/查询长度限制） ｜ 决策日期：2026-08-01 ｜ 完成日期：2026-08-01

**输入校验规则矩阵**：

| 输入字段 | 校验规则 | 限制值 | 失败行为 | 当前状态 |
|---|---|---|---|---|
| 文件路径 | 拒绝 `..` 路径遍历 | — | 返回 `VALIDATION: 路径包含非法字符` | ✅ 已实现（`sanitize_path()`） |
| 文件扩展名 | 白名单校验 | `.md` / `.txt` / `.pdf` | 返回 `VALIDATION: 不支持的文件格式` | ✅ 已实现（`validate_extension()`） |
| API Key | 长度限制 | ≤ 256 字符 | 返回 `VALIDATION: API Key 长度不能超过 256 字符` | ✅ 已实现（`update_llm_config_inner()`） |
| API Key | 空值校验 | 非空（非 Ollama 预设时） | 向导内联 `#wizError`："请填入 API Key" | ✅ 已实现（`startWizard()` 前置校验） |
| Base URL | 非空校验 | — | 向导内联 `#wizError`："Base URL 与 Model 不能为空" | ✅ 已实现 |
| Base URL | URL 格式校验 | 必须以 `http://` 或 `https://` 开头 | 返回 `VALIDATION: Base URL 格式不正确` | ✅ 已实现（`update_llm_config_inner()`） |
| Model 名称 | 长度限制 | ≤ 128 字符 | 返回 `VALIDATION: 模型名称长度不能超过 128 字符` | Draft |
| Model 名称 | 非空校验 | — | 向导内联 `#wizError`："Base URL 与 Model 不能为空" | ✅ 已实现 |
| 会话标题 | 长度限制 | ≤ 200 字符 | 截断 + 追加 `…` | Draft（`derive_title()` 取前 24 字符，更严格） |
| 对话查询 | 长度限制 | ≤ 8000 字符 | 返回 `VALIDATION: 问题过长，请缩减至 8000 字符以内` | ✅ 已实现（`chat_inner()`） |
| License Key | 格式校验 | Ed25519 签名验证 | 返回 `License 无效或已过期` | ✅ 已实现（`verify_license()`） |

**已实现校验函数**：
- `sanitize_path(path: &Path)` — `crates/core/src/import.rs`：拒绝含 `..` 的路径。
- `validate_extension(path: &Path)` — `crates/core/src/import.rs`：校验扩展名白名单。
- `derive_title(msg: &str)` — `crates/tauri-app/src/commands.rs`：取前 24 字符，超长追加 `…`。
- `verify_license(key: &str)` — `crates/core/src/license.rs`：Ed25519 离线签名验证。
- `startWizard()` 前置校验 — `ui/index.html`：API Key 非空（非 Ollama）+ Base URL/Model 非空。

- 验收标准：
  - AC-1：文件路径含 `..` 被拒绝（`sanitize_path()` 返回错误）。
  - AC-2：文件扩展名非 `.md` / `.txt` / `.pdf` 被拒绝（`validate_extension()` 返回错误）。
  - AC-3：向导中 API Key 为空（非 Ollama 预设）时 `#wizError` 显示"请填入 API Key"。
  - AC-4：向导中 Base URL 或 Model 为空时 `#wizError` 显示"Base URL 与 Model 不能为空"。
  - AC-5：会话标题超 24 字符时被 `derive_title()` 截断并追加 `…`。
  - AC-6：License Key 无效时返回"License 无效或已过期"（`verify_license()` 签名验证失败）。
  - AC-7：（Draft）API Key 超 256 字符被拒绝并提示长度限制。
  - AC-8：（Draft）Base URL 非 `http://` 或 `https://` 开头被拒绝并提示格式要求。
  - AC-9：（Draft）对话查询超 8000 字符被拒绝并提示"问题过长，请缩减"。
- 契约：`sanitize_path()` + `validate_extension()` + `derive_title()` + `verify_license()` + `startWizard()` 前置校验 + Draft 后端长度/格式校验。
- 关联测试：TC-SEC-001（路径遍历防护）、TC-LIC-001（License 验证）

---

### 4.22 OBS — 可观测性与诊断

> **域背景**：当前应用缺乏日志记录、性能监控与诊断能力。当用户遇到性能问题或异常行为时，无法提供有效诊断信息。系统化的可观测性是生产级桌面应用的基本要求。

#### REQ-OBS-001 本地日志系统
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-ARCH-000
- **v1.0 发布分类**：提前实现（结构化日志 + 轮转 + 级别控制） ｜ 决策日期：2026-08-01
- 描述：应用运行期间在本地写入结构化日志（JSON Lines 格式），存储于数据目录 `logs/` 下，按日期轮转（每日一个文件，保留最近 7 天）。日志级别：DEBUG / INFO / WARN / ERROR。日志字段：timestamp、level、module、message、context（可选 JSON）。默认级别 INFO，可在设置中调整为 DEBUG。日志不含 API Key 等敏感信息。
- 验收标准：
  - AC-1：✅ 应用运行期间在 `logs/` 目录下生成 JSON Lines 格式日志文件（`crates/infra/src/local_logger.rs` — `tracing-appender` 按日期轮转 + `fmt::layer().json()` 格式化）。
  - AC-2：✅ 日志按日期轮转，保留最近 7 天，旧日志自动删除（`cleanup_old_logs()` — 基于 `chrono` 计算过期时间，`std::fs::remove_file` 删除）。
  - AC-3：✅ 日志字段包含 timestamp、level、module、message（`tracing-subscriber` JSON 格式化层自动包含）。
  - AC-4：✅ 日志中不出现 API Key 明文（REQ-SEC-001 脱敏 — 日志仅记录模块名和操作描述，不记录用户数据或 API Key）。
  - AC-5：✅ 默认日志级别 INFO，可在设置中切换为 DEBUG（`set_log_level` IPC 命令 + `reload::Layer` 动态更新）。
  - AC-6：ERROR 级别日志同时触发系统通知（macOS Notification Center / Windows Toast）。（延后到 v1.1）
- 契约：`crates/infra/src/local_logger.rs` — `LocalLogger::init` / `set_level` / `read_recent_logs`；IPC: `get_log_level` / `set_log_level` / `export_logs` ｜ 关联测试：TC-OBS-001~009（`local_logger_tests.rs`）

#### REQ-OBS-002 性能指标采集
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-OBS-001
- 描述：应用在关键操作处采集性能指标并记录到日志：导入耗时（文件解析 / 分块 / 嵌入各阶段）、检索耗时（向量搜索 / 关键词搜索 / RRF 融合）、LLM 首 token 延迟、LLM 总生成耗时、冷启动耗时。指标以 INFO 级别日志记录，含操作类型、耗时(ms)、数据规模。
- 验收标准：
  - AC-1：文档导入完成后日志记录各阶段耗时（解析 / 分块 / 嵌入）。
  - AC-2：RAG 查询完成后日志记录检索耗时与首 token 延迟。
  - AC-3：冷启动完成后日志记录启动总耗时。
  - AC-4：性能指标日志含数据规模（如 chunk 数、向量维度）。
  - AC-5：性能指标不包含用户数据内容（仅记录元数据与耗时）。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-OBS-003 诊断信息导出
- 优先级：P2 ｜ 状态：Done ｜ 依赖：REQ-OBS-001
- 描述：设置面板提供「导出诊断信息」功能，将应用环境信息打包为 JSON 文件：应用版本、操作系统版本、CPU 核心数、内存、数据库大小、知识库规模（文档数 / chunk 数 / 嵌入维度）、最近 100 条日志、LLM 配置（脱敏，不含 API Key）。用户可将诊断信息提供给开发者排查问题。
- 验收标准：
  - AC-1：✅ 设置面板显示「导出诊断信息」按钮（`settings.js` — `exportDiagnostics()` + `#exportDiagnosticsBtn`）。
  - AC-2：✅ 导出文件包含应用版本、OS 版本、知识库规模、最近 100 条日志（`collect_diagnostics()` — 聚合 `serde_json::json!` 对象）。
  - AC-3：✅ 导出文件不含 API Key 明文（脱敏处理 — `mask_api_key()` 返回 `****` + 末四位）。
  - AC-4：✅ 导出文件不含用户文档内容或对话内容（仅记录元数据与系统信息）。
  - AC-5：✅ 导出文件通过系统保存对话框选择保存位置（复用 `save_text_file` IPC 命令）。
- 契约：`crates/infra/src/local_logger.rs` — `collect_diagnostics()`；IPC: `export_diagnostics` ｜ 关联测试：TC-OBS-005~006（`local_logger_tests.rs`）

---

### 4.23 WIN — 窗口管理与系统集成

> **域背景**：当前应用窗口配置为 Tauri 默认值，缺乏最小窗口尺寸、默认窗口尺寸、多显示器适配、系统托盘、开机自启等桌面应用标配的窗口管理能力。

#### REQ-WIN-001 窗口尺寸与约束
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-UI-001
- **v1.0 发布分类**：v1.0 必须交付（已实现 minWidth 960/minHeight 640/default 1280×800 + 窗口位置/尺寸/最大化持久化到 settings 表） ｜ 决策日期：2026-08-01 ｜ 完成日期：2026-08-01
- 描述：定义窗口尺寸约束：最小窗口尺寸 800×600（保证核心功能可用）、默认窗口尺寸 1200×800、最大窗口尺寸不限（跟随屏幕）。窗口位置与尺寸在关闭时保存，下次启动恢复。支持多显示器：窗口在上次使用的显示器上恢复。
- 验收标准：
  - AC-1：窗口不能缩小到 800×600 以下。
  - AC-2：首次启动窗口尺寸为 1200×800。
  - AC-3：窗口关闭时位置与尺寸保存到 settings 表，重启后恢复。
  - AC-4：多显示器环境下窗口在上次使用的显示器上恢复。
  - AC-5：窗口最大化状态持久化，重启后恢复最大化。
- 契约：`tauri.conf.json` (minWidth/minHeight/width/height) + `lib.rs` (`restore_window_state` / `save_window_state`) + settings 表 (`window.x` / `window.y` / `window.width` / `window.height` / `window.maximized`)。
- 关联测试：TC-WIN-001（状态持久化）、TC-WIN-002（默认值）

#### REQ-WIN-002 高 DPI 与 Retina 适配
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-UI-001
- **v1.0 发布分类**：v1.0.1 P1 Draft 收尾实现（Session 4，高 DPI E2E 验收通过） ｜ 完成日期：2026-08-02
- 描述：应用在高 DPI / Retina 屏幕下正确渲染：SVG 图标矢量缩放不模糊、字体清晰无锯齿、布局比例在不同 DPI 下一致。Tauri WebView 自动适配系统缩放比例。
- 验收标准：
  - AC-1：Retina 屏幕下 SVG 图标清晰无锯齿。（TC-WIN-002-001：deviceScaleFactor=2 下 SVG 图标 boundingBox 有效尺寸 + 可见 + viewBox 存在）
  - AC-2：Retina 屏幕下字体清晰，无像素化。（TC-WIN-002-002：deviceScaleFactor=2 下 getComputedStyle(body).fontFamily 含 -apple-system + CJK 字体）
  - AC-3：100% / 125% / 150% / 200% 系统缩放下布局比例一致，无溢出或截断。（TC-WIN-002-003：deviceScaleFactor=1.5 下 scrollWidth ≤ clientWidth + 核心区域可见）
  - AC-4：高 DPI 下 Mermaid 图表与 KaTeX 公式渲染清晰。（TC-WIN-002-004：deviceScaleFactor=2 下 .mermaid svg 存在 + .katex 存在 + 有效尺寸）
- 契约：Tauri WebView 自动适配 deviceScaleFactor ｜ Playwright `test.use({ deviceScaleFactor })` 模拟 ｜ 关联测试：TC-WIN-002-001~004

#### REQ-WIN-003 系统托盘集成
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-WIN-001
- 描述：应用在系统托盘显示图标（macOS 菜单栏 / Windows 系统托盘），右键菜单提供：显示/隐藏窗口、新建会话、退出。关闭窗口时最小化到托盘而非退出应用（可在设置中切换为直接退出）。托盘图标点击切换窗口显示/隐藏。
- 验收标准：
  - AC-1：系统托盘显示应用图标。
  - AC-2：右键托盘图标弹出菜单（显示/隐藏窗口、新建会话、退出）。
  - AC-3：关闭窗口时默认最小化到托盘（非退出）。
  - AC-4：设置中可切换「关闭窗口时退出」行为。
  - AC-5：点击托盘图标切换窗口显示/隐藏。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-WIN-004 开机自启
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-WIN-003
- 描述：设置面板提供「开机自启动」开关，开启后应用随系统启动（后台静默启动，不自动弹出窗口）。关闭后取消自启。自启状态通过 Tauri `autostart` 插件管理。
- 验收标准：
  - AC-1：设置面板显示「开机自启动」开关。
  - AC-2：开启后系统重启时应用自动启动（后台静默）。
  - AC-3：关闭后系统重启时应用不自动启动。
  - AC-4：自启状态下应用启动后不自动弹出窗口（仅托盘图标可见）。
  - AC-5：自启状态持久化，重启后保持。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-WIN-005 系统主题跟随
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-DS-001
- 描述：v1.0 仅暗色主题，但应用检测系统主题偏好（`prefers-color-scheme`）。当系统切换到浅色模式时，应用显示提示「浅色主题将在后续版本支持，当前为暗色主题」。尊重系统「减少动态效果」（`prefers-reduced-motion`）设置，已在 REQ-DS-006 定义。
- 验收标准：
  - AC-1：应用检测系统主题偏好（`prefers-color-scheme: light` 时显示提示）。
  - AC-2：系统切换到浅色模式时应用显示一次性提示 toast。
  - AC-3：尊重 `prefers-reduced-motion: reduce` 设置（REQ-DS-006 AC-6）。
  - AC-4：提示不频繁弹出（每次启动最多一次）。
- 契约：TBD ｜ 关联测试：TBD

---

### 4.24 I18N — 国际化与本地化

> **域背景**：当前应用界面文案为中文硬编码，缺乏国际化框架。虽然 v1.0 定位为中文优先，但预留国际化框架有助于后续扩展。

#### REQ-I18N-001 界面文案外部化与语言切换
- 优先级：P2 ｜ 状态：Done ｜ 依赖：REQ-UI-001
- 描述：将前端所有硬编码界面文案抽取为外部化字符串资源（JSON 格式），以键值对组织（如 `sidebar.new_chat` →「新建会话」）。支持简体中文（`zh-CN`）与英文（`en`）两种语言，默认语言为简体中文。字符串资源文件本地 vendored 于 `ui/locales/`（`en.json` + `zh-CN.json`，200+ key）。`ui/src/i18n.js` 模块提供 `t()` 函数（嵌套 key + `{placeholder}` 插值）+ `data-i18n` / `data-i18n-placeholder` / `data-i18n-title` / `data-i18n-aria-label` 属性系统 + `refreshI18nElements()` 自动刷新。设置面板提供语言切换下拉（English / 简体中文），语言偏好持久化到 settings 表 `ui.locale`，后端提供 `get_locale` / `set_locale` IPC 命令。
- 验收标准：
  - AC-1：前端所有界面文案通过字符串资源 key 引用，无硬编码中文（代码注释除外）。
  - AC-2：`ui/locales/zh-CN.json` 包含全部界面文案键值对。
  - AC-3：字符串资源 key 采用 `模块.功能` 命名规范（如 `sidebar.new_chat`）。
  - AC-4：缺少 key 时回退为 key 本身（不崩溃）。
  - AC-5：动态内容（LLM 输出、文档内容）不经 i18n 处理。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-I18N-002 日期时间本地化格式
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-I18N-001
- 描述：应用中显示的日期时间使用本地化格式：会话创建时间显示为「YYYY-MM-DD HH:mm」、导入历史时间显示为「相对时间」（如「3 分钟前」「昨天」「3 天前」，超过 7 天显示绝对时间）。时间格式遵循系统 locale 设置。
- 验收标准：
  - AC-1：会话创建时间显示为「YYYY-MM-DD HH:mm」格式。
  - AC-2：导入历史时间显示为相对时间（3 分钟内→「刚刚」、1 小时内→「N 分钟前」、24 小时内→「N 小时前」、7 天内→「N 天前」、超过 7 天→绝对时间）。
  - AC-3：时间格式遵循系统 locale（macOS / Windows / Linux 一致行为）。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-I18N-003 数字与文件大小本地化
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-I18N-001
- 描述：文件大小显示使用人类可读格式（B / KB / MB / GB），采用 1024 进制，保留 1 位小数。大数字（如 chunk 数、向量数）使用千分位分隔符。百分比显示保留整数。
- 验收标准：
  - AC-1：文件大小显示为「12.3 MB」格式（1024 进制，1 位小数）。
  - AC-2：大数字显示千分位分隔符（如「10,000 chunks」）。
  - AC-3：百分比显示整数（如「87%」）。
  - AC-4：< 1 KB 时显示字节数（如「512 B」）。
- 契约：TBD ｜ 关联测试：TBD

---

### 4.25 HELP — 帮助与引导系统

> **域背景**：当前应用有首次启动配置向导（REQ-UI-007），但缺乏持续性的帮助系统：快捷键参考、FAQ、关于页面、更新检查等。完善的帮助系统降低用户学习成本。

#### REQ-HELP-001 首次使用引导
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-UI-007
- 描述：首次进入主界面后（配置向导完成后），显示一次性交互式引导（3-4 步 Coach Marks）：第 1 步高亮侧栏介绍「会话与知识库」、第 2 步高亮导入按钮介绍「拖拽或点击导入文件」、第 3 步高亮输入框介绍「在此提问，AI 基于你的知识库回答」、第 4 步高亮设置按钮介绍「配置 LLM、VLM、检索参数」。引导可跳过，完成后不再显示（持久化标记）。
- 验收标准：
  - AC-1：首次进入主界面后自动显示交互式引导（Coach Marks）。
  - AC-2：引导共 3-4 步，每步高亮对应 UI 元素并显示说明文案。
  - AC-3：每步有「下一步」和「跳过」按钮。
  - AC-4：引导完成后持久化标记，重启后不再显示。
  - AC-5：设置面板提供「重新查看引导」入口。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-HELP-002 FAQ 与帮助文档
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-UI-001
- 描述：设置面板或帮助菜单提供「常见问题」入口，打开内嵌 FAQ 面板（Markdown 渲染），覆盖：如何配置 LLM、支持哪些文件格式、数据存储在哪里、如何备份恢复、免费版与 Pro 版区别、如何激活 License、嵌入模型是什么、隐私安全说明。FAQ 内容本地 vendored（`ui/faq.md`），不联网。
- 验收标准：
  - AC-1：设置面板或帮助菜单提供「常见问题」入口。
  - AC-2：FAQ 面板以 Markdown 渲染，支持滚动浏览。
  - AC-3：FAQ 覆盖至少 8 个常见问题。
  - AC-4：FAQ 内容本地 vendored，不联网。
  - AC-5：FAQ 面板可搜索过滤。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-HELP-003 关于页面
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-UI-001
- 描述：设置面板或帮助菜单提供「关于」入口，打开关于页面显示：应用名称与 Logo、版本号、构建日期、技术栈（Rust + Tauri v2）、开源许可声明、隐私政策摘要、联系支持方式。关于页面内容本地化，不联网。
- 验收标准：
  - AC-1：设置面板或帮助菜单提供「关于」入口。
  - AC-2：关于页面显示应用名称、Logo、版本号、构建日期。
  - AC-3：关于页面显示技术栈（Rust + Tauri v2）。
  - AC-4：关于页面显示隐私政策摘要与开源许可声明。
  - AC-5：关于页面内容本地化，不联网。
- 契约：TBD ｜ 关联测试：TBD

#### REQ-HELP-004 应用更新检查
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-SEC-003
- 描述：应用启动后（或手动触发）检查 GitHub Releases 是否有新版本。检查频率：每 24 小时最多一次（持久化上次检查时间）。发现新版本时在侧栏或顶栏显示更新提示徽标，点击查看更新详情与下载链接。更新检查请求仅发往 GitHub API（REQ-SEC-003 网络白名单扩展），不含任何用户数据。用户可在设置中关闭自动检查。
- 验收标准：
  - AC-1：应用启动后自动检查更新（每 24 小时最多一次）。
  - AC-2：发现新版本时侧栏或顶栏显示更新提示徽标。
  - AC-3：点击徽标显示更新详情（版本号、更新日志摘要、下载链接）。
  - AC-4：更新检查请求仅发往 GitHub API，不含用户数据。
  - AC-5：设置中可关闭自动更新检查。
  - AC-6：网络不可用时静默跳过检查，不报错。
- 契约：TBD ｜ 关联测试：TBD

---

### 4.23 SYNC — 文件监听与增量同步

> **域背景**：当前知识库文档需用户手动导入，外部文件夹中的文件更新后无法自动反映到知识库中。文件监听+增量同步允许用户链接一个本地文件夹，应用自动监听文件夹变化（新建/修改/删除），并增量同步到知识库——无需手动重新导入。适用于「笔记文件夹」「项目文档目录」等持续更新的场景。纯本地操作，零网络请求（符合「隐私不出域」承诺）。

#### REQ-SYNC-001 文件夹监听配置与管理
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-ING-001、REQ-DB-001
- 描述：用户可在设置面板中添加「监听文件夹」，将本地文件夹链接到知识库。每个监听文件夹记录路径、启用状态与最后同步时间。监听文件夹列表持久化到 settings 表（`sync.watched_folders`，JSON 数组）。添加文件夹后自动触发首次全量同步。可随时移除监听（仅停止监听，不删除已导入的文档）。
- 验收标准：
  - AC-1：设置面板显示「文件监听」区域，含「添加文件夹」按钮与已监听文件夹列表。
  - AC-2：点击「添加文件夹」弹出系统文件夹选择器，选择后文件夹加入监听列表并自动触发首次同步。
  - AC-3：每个监听文件夹条目显示路径、同步状态（空闲/同步中/错误）与最后同步时间。
  - AC-4：移除监听文件夹仅停止监听，不删除已导入的文档（级联清理仅由文件删除事件触发）。
  - AC-5：监听文件夹列表持久化到 settings 表，重启后自动恢复监听。
  - AC-6：非存在的文件夹路径被拒绝，提示「文件夹不存在」。
- 契约：IPC `add_watched_folder(path: String) -> Result<SyncResult, String>`；`remove_watched_folder(path: String) -> Result<(), String>`；`get_watched_folders() -> Result<Vec<WatchedFolderInfo>, String>`；Storage `set_setting("sync.watched_folders", json)` / `get_setting("sync.watched_folders")` ｜ 关联测试：TC-SYNC-001~003

#### REQ-SYNC-002 增量同步引擎
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-SYNC-001、REQ-ING-001、REQ-ING-004
- 描述：增量同步引擎对比监听文件夹中的文件与知识库中已导入的文档，执行三类操作：(1) **新增**——文件夹中存在但知识库中不存在的文件，自动导入+索引+向量化；(2) **更新**——文件内容变更（MD5 哈希不同），删除旧文档及其 chunks/向量，导入新版本；(3) **删除**——知识库中存在对应源文件路径的文档但文件夹中文件已被删除，级联清理文档及其 chunks/向量。同步引擎仅处理受支持格式（.md/.txt/.pdf），跳过隐藏文件（以 `.` 开头）。`Document` 模型新增 `original_path: Option<String>` 字段跟踪源文件路径（手动导入为 None，监听导入为源路径）。
- 验收标准：
  - AC-1：首次同步一个含 3 个 .md 文件的文件夹，全部导入成功，文档 `original_path` 指向源文件路径。
  - AC-2：修改文件夹中已导入文件的内容，触发同步后旧文档被删除、新文档导入（doc_id 变化，file_hash 变化）。
  - AC-3：删除文件夹中已导入的文件，触发同步后知识库中对应文档被级联删除。
  - AC-4：在文件夹中新建文件，触发同步后自动导入。
  - AC-5：隐藏文件（以 `.` 开头）被跳过，不导入。
  - AC-6：不受支持的格式（如 .docx）被跳过，不报错。
  - AC-7：同步过程中通过 `sync_progress` 事件推送进度（phase: scanning/importing/deleting/complete + counts）。
  - AC-8：同步是幂等的——多次同步无变更的文件夹不会产生重复导入或删除。
- 契约：`SyncService<S: Storage>`（`crates/core/src/sync.rs`）；`sync_folder(folder_path, is_pro) -> Result<SyncResult, Error>`；`SyncResult { added: usize, updated: usize, deleted: usize, skipped: usize, errors: Vec<String> }`；`Document.original_path: Option<String>`；Storage `find_document_by_original_path(path) -> Option<Document>` + `find_documents_by_original_path_prefix(prefix) -> Vec<Document>` ｜ 关联测试：TC-SYNC-004~010

#### REQ-SYNC-003 实时文件监听器
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-SYNC-001、REQ-SYNC-002
- 描述：使用 `notify` crate（跨平台文件系统通知库）实现实时文件监听。每个监听文件夹启动一个 `RecommendedWatcher`，通过 `notify-debouncer-full` 对事件去抖（500ms），避免文件写入过程中的大量中间事件触发多次同步。去抖后的事件触发增量同步引擎。监听器在后台 tokio task 中运行，不阻塞 UI 线程。应用退出时自动停止所有监听器。
- 验收标准：
  - AC-1：监听文件夹中新建文件后 ≤2s 触发增量同步，新文件被导入。
  - AC-2：监听文件夹中修改文件后 ≤2s 触发增量同步，旧文档被替换。
  - AC-3：监听文件夹中删除文件后 ≤2s 触发增量同步，对应文档被删除。
  - AC-4：快速连续修改同一文件（1s 内 5 次）仅触发一次同步（去抖生效）。
  - AC-5：应用退出后监听器停止，不再产生同步事件。
  - AC-6：监听器内部错误（如权限不足）不 Panic，通过 `sync_progress` 事件推送错误信息。
- 契约：`FileWatcher`（`crates/infra/src/file_watcher.rs`）；`notify = "8.2"` + `notify-debouncer-full = "0.7"`；AppState `file_watchers: Mutex<HashMap<String, FileWatcherHandle>>` ｜ 关联测试：TC-SYNC-011~013

---

### 4.26 PERF — 极致性能优化

> **域背景**：基于对 18 个论文/开源项目的系统调研（Anthropic Contextual Retrieval / Prompt Caching, LLMLingua-2, Self-RAG, CRAG, Dense X Retrieval, ColBERT, RAPTOR, vLLM, AutoRAG 等），对 RAG 管线进行 Token 消耗压缩、检索命中率提升、推理延迟优化三个维度的极致性能优化。目标：Token ↓50-75%，检索命中率 +20-30pp，延迟 ↓50-70%。详见 `docs/architecture/EXTREME_PERFORMANCE_ATOMIC_PLAN.md`。

#### REQ-PERF-001 语义缓存金字塔
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-RAG-001
- 描述：实现 L0 精确匹配 + L1 语义匹配 + L3 检索结果三级缓存，重复/相似查询直接返回缓存结果，0 LLM token 消耗。SqliteCache 共享 SqliteStorage 连接池，向量存储为 BLOB（f32 bytes），语义匹配全表扫描 + Rust 余弦相似度。缓存默认开启，高隐私场景可通过 `privacy_mode` 关闭。文档导入/删除时清空所有缓存。
- 验收标准：
  - AC-1：L0 精确匹配命中时返回缓存答案，不调用 LLM（TC-CACHE-003）
  - AC-2：L0 miss → L1 语义匹配命中（cosine ≥ 阈值）→ 返回缓存（TC-CACHE-004）
  - AC-3：TTL 过期后缓存自动失效（TC-CACHE-007）
  - AC-4：文档导入触发缓存清空（TC-CACHE-008）
  - AC-5：`privacy_mode=true` 时缓存禁用（TC-CACHE-009）
- 契约：`ResponseCache` trait（`crates/core/src/lib.rs`）+ `SqliteCache`（`crates/infra/src/sqlite_cache.rs`，3 张表）+ IPC `get_cache_stats` / `clear_cache` / `set_cache_settings` / `get_cache_settings` + settings `cache.enabled` / `cache.ttl_secs` / `cache.semantic_threshold` / `cache.privacy_mode` ｜ 关联测试：TC-CACHE-001~012, TC-CACHE-INT-001~007

#### REQ-PERF-002 Prompt 压缩
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-PERF-001
- 描述：LLMLingua-2 式检索片段 token 级压缩，两层策略——Free 版 `RuleBasedCompressor`（停用词去除 + Markdown 装饰去除 + 代码注释去除 + 句子级信息密度评分，零依赖）+ Pro 版 `OnnxCompressor`（嵌入余弦相似度评分，复用 LocalEmbedder）。ChatEngine 集成 `with_compressor()` builder，压缩比可配置（1.0=禁用 / 2.0=保守 / 3.0=平衡 / 5.0=激进）。压缩仅作用于检索片段，不压缩系统提示和用户查询。
- 验收标准：
  - AC-1：压缩比 2.0 时保留 ≥71% 内容（TC-COMP-001）
  - AC-2：停用词去除不破坏句子结构（TC-COMP-003）
  - AC-3：Markdown 装饰去除保留语义（TC-COMP-004）
  - AC-4：代码注释去除保留代码逻辑（TC-COMP-005）
  - AC-5：禁用时（ratio=1.0）行为完全一致（TC-COMP-010）
- 契约：`PromptCompressor` trait（`crates/core/src/lib.rs`）+ `RuleBasedCompressor`（`crates/core/src/prompt_compressor.rs`）+ `OnnxCompressor`（`crates/infra/src/onnx_compressor.rs`，Pro）+ IPC `set_compression_ratio` / `get_compression_ratio` + settings `compression.ratio` ｜ 关联测试：TC-COMP-001~010

#### REQ-PERF-003 CRAG 检索精炼
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-RAG-002
- 描述：基于 CRAG 论文（arXiv:2401.15884），对检索结果进行三级置信度评估（Correct/Ambiguous/Incorrect）+ decompose-then-recompose 句子级过滤。Correct 直接使用，Ambiguous 过滤低相关句子后使用，Incorrect 触发重新检索或回退。评估器使用规则匹配（零 LLM），避免额外推理开销。
- 验收标准：
  - AC-1：Correct 评估器正确识别高质量检索结果
  - AC-2：Ambiguous 评估器触发句子级过滤
  - AC-3：Incorrect 评估器触发回退路径
- 契约：`chat_inner` 三级精炼管线 ｜ 关联测试：TC-CRAG-001~010

#### REQ-PERF-004 Self-RAG 检索决策
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-RAG-001
- 描述：基于 Self-RAG 论文（arXiv:2310.11511），查询分类器决定是否触发检索（Simple/Analytical/Comparison/Factual）。简单问候/闲聊不触发检索，减少 20-30% token 消耗。分类器使用规则匹配（零 LLM）。
- 验收标准：
  - AC-1：Simple 查询（问候/闲聊）跳过检索
  - AC-2：Factual 查询触发标准检索
  - AC-3：Analytical 查询触发增强检索
- 契约：`classify_query_type()` 规则匹配函数 ｜ 关联测试：TC-SELFRAG-001~010

#### REQ-PERF-005 Contextual BM25
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-RAG-010
- 描述：基于 Anthropic Contextual Retrieval，FTS5 索引使用 `build_contextual_text()` 拼接文档名前缀，检索失败率 ↓49%。`add_chunk`/`add_chunks_batch` 写入上下文前缀，`rebuild_bm25_index` IPC 命令供用户手动触发索引重建。FTS5 回填使用上下文前缀。
- 验收标准：
  - AC-1：FTS5 索引使用上下文前缀文本
  - AC-2：`rebuild_bm25_index` 命令重建索引
  - AC-3：上下文前缀提升精确匹配命中率
- 契约：`build_contextual_text()` + `rebuild_bm25_index` IPC 命令 ｜ 关联测试：TC-CBM25-001~005

#### REQ-PERF-006 实体链接增强
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-RAG-010
- 描述：NER 实体抽取器（纯规则+正则，零 LLM/模型依赖），5 类实体（中文人名/英文专有名词/技术术语/数字标识符/日期）。`entities` 表 + `entity_search()` 精确匹配，三路 RRF 融合（Vector=1.0 + BM25=0.8 + Entity=0.6）。导入时自动抽取实体并索引。
- 验收标准：
  - AC-1：5 类实体正确抽取
  - AC-2：三路 RRF 融合结果优于双路
  - AC-3：导入时自动抽取并索引实体
- 契约：`EntityExtractor`（`crates/core/src/entity_extractor.rs`）+ `entities` 表 + `entity_search()` + 三路 RRF 融合 ｜ 关联测试：TC-ENT-001~008, TC-3RRF-001~004

#### REQ-PERF-007 Proposition 级原子分割
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-PERF-006
- 描述：基于 Dense X Retrieval 论文（arXiv:2312.06648），将 chunk 分解为自包含的原子事实（proposition），proposition 级检索精度显著优于 chunk 级。纯规则方案（零 LLM 调用）：句子分割（中英文标点）+ 代词消解（中文他/她/它 + 英文 he/she/it/they）+ 上下文补全（连词开头补全主语前缀）。
- 验收标准：
  - AC-1：句子正确分割
  - AC-2：代词消解使用前一句实体
  - AC-3：连词开头补全主语前缀
  - AC-4：proposition 级检索精度优于 chunk 级
- 契约：`PropositionSplitter`（`crates/core/src/proposition_splitter.rs`）+ `Proposition` 模型 + `propositions` 表 + `proposition_search_and_expand()` + `rebuild_proposition_index` IPC ｜ 关联测试：TC-PROP-001~008b

#### REQ-PERF-008 ColBERT 多向量嵌入（Pro feature）
- 优先级：P2 ｜ 状态：Implemented ｜ 依赖：REQ-VEC-002
- 描述：基于 ColBERT 论文（arXiv:2004.12832）Late Interaction 模型，为文本中每个 token 生成独立嵌入向量，检索时使用 MaxSim 交互计算。新增 `MultiVectorEmbedder` trait（core 层）+ `ColbertEmbedder` 适配器（infra 层，Pro 门控，复用 LocalEmbedder ONNX 引擎）。
- 验收标准：
  - AC-1：多向量嵌入正确生成
  - AC-2：MaxSim 计算结果正确
  - AC-3：多向量 BLOB 序列化/反序列化 roundtrip
- 契约：`MultiVectorEmbedder` trait + `ColbertEmbedder`（Pro）+ `maxsim()` / `cosine_sim()` + `multivec_to_bytes` / `multivec_from_bytes` ｜ 关联测试：TC-COL-001~008

#### REQ-PERF-009 RAPTOR 多级摘要树索引
- 优先级：P2 ｜ 状态：Implemented ｜ 依赖：REQ-RAG-002
- 描述：基于 RAPTOR 论文，将原始 chunks 组织为多级摘要树（Level 0 组摘要 → Level 1 主题摘要），提供从局部事实到全局主题的多层次检索能力。`SummaryTreeBuilder` 支持 LLM 摘要生成回调 + 增量构建。`classify_query_level()` 查询路由器使用规则匹配（零 LLM）。
- 验收标准：
  - AC-1：摘要树正确构建（Level 0 + Level 1）
  - AC-2：查询路由器正确分类查询级别
  - AC-3：摘要展开到原始 chunks
  - AC-4：摘要与 chunk 结果合并
- 契约：`SummaryTreeBuilder`（`crates/core/src/summary_tree.rs`）+ `SummaryNode` 模型 + `summary_nodes` 表 + `build_summary_tree` IPC + `set_embedder_model` IPC ｜ 关联测试：TC-RAP-001~008

#### REQ-PERF-010 渐进式上下文注入
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-PERF-002
- 描述：按需注入 chunk，LLM 上下文足够时停止追加。`ProgressiveInjector` 状态机（initial_count=2 可配置 + max_rounds=top_k 防无限追加）。`detect_insufficient_info()` 独立函数检测 LLM 输出中的 "信息不足" 关键词（15 中 + 12 英）。预期平均注入 chunk 数 5→2-3，token ↓40-60%。
- 验收标准：
  - AC-1：初始注入 2 个 chunk
  - AC-2：检测到 "信息不足" 关键词时追加
  - AC-3：max_rounds 防止无限追加
  - AC-4：禁用时注入全部 chunks（向后兼容）
- 契约：`ProgressiveInjector`（`crates/core/src/progressive_injector.rs`）+ `detect_insufficient_info()` + `ChatEngine.with_progressive()` builder + settings `rag.progressive_injection` ｜ 关联测试：TC-PROG-001~006b

#### REQ-PERF-011 Speculative RAG
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-PERF-010
- 描述：基于 vLLM Speculative Decoding 概念（arXiv:2309.06180），小模型快速生成草稿 → 大模型验证/修正。草稿用简化 prompt（仅 top-1 chunk），验证用完整 prompt + 草稿作为 prefix。`SpeculativeRagEngine<D, L>` 泛型引擎，三态结果（DraftAccepted / DraftCorrected / FallbackDirect）。基于归一化 Levenshtein 编辑距离计算相似度。预期首 token 延迟 ↓40-60%。
- 验收标准：
  - AC-1：草稿相似度 ≥ threshold → DraftAccepted
  - AC-2：草稿相似度 < threshold → FallbackDirect
  - AC-3：草稿长度不足 → FallbackDirect
  - AC-4：禁用时走标准 RAG 路径（向后兼容）
- 契约：`SpeculativeRagEngine<D, L>`（`crates/core/src/speculative_rag.rs`）+ `SpeculativeRagConfig` + `text_similarity()` + settings `rag.speculative_enabled` ｜ 关联测试：TC-SPEC-001~006c

#### REQ-PERF-012 自进化检索记忆
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-RAG-002
- 描述：借鉴 AutoRAG 2.0 自进化记忆系统，系统自动记录每种查询类型 × 检索方法的累计效果统计（hit_count/miss_count/avg_score），自适应选择命中率最高的检索策略。`QueryType` 四分类（Factual/Analytical/Code/Conversational，规则匹配零 LLM），`RetrievalMethod` 五方法（vector_only/hybrid/hybrid_rerank/proposition/colbert）。冷启动使用默认方法（Hybrid），随使用自动优化。不增加延迟（记忆查表 ~1ms）。
- 验收标准：
  - AC-1：查询分类正确（4 分类）
  - AC-2：冷启动返回默认方法（Hybrid）
  - AC-3：热启动选择命中率最高的方法
  - AC-4：avg_score 平局打破
  - AC-5：禁用时使用默认方法（向后兼容）
- 契约：`RetrievalMemoryEngine<S>`（`crates/core/src/retrieval_memory.rs`）+ `RetrievalMemoryStore` trait + `retrieval_memory` 表 + IPC `set_retrieval_memory_enabled` / `get_retrieval_memory_stats` / `reset_retrieval_memory` / `record_retrieval_feedback` + settings `rag.retrieval_memory_enabled` ｜ 关联测试：TC-MEM-001~006 + enum_roundtrip + default_for_each_type + record_updates_correctly + TC-FEEDBACK-001~007
- 扩展（v3.4.0）：**用户反馈自学习**——借鉴 StoryMoss 持续学习机制，通过观察用户行为（重新提问/编辑重发/点赞/点踩/继续新话题）自动蒸馏检索效果，无需显式标注。`FeedbackType` 五种反馈信号（RetryWithDifferentMethod/EditAndResend/Accepted/ThumbsUp/ThumbsDown），每种有不同 delta 权重（正信号提升 hit_rate，负信号降低，强信号幅度加倍）。`FeedbackSignal` 结构体（query/method/feedback/timestamp）。`record_feedback()` 方法根据 delta 调整 hit_count/miss_count/avg_score（clamp 0.0-1.0 边界保护）。前端 `retrieval-feedback.js` 模块隐式采集用户行为信号（Jaccard 词重叠率相似度检测）+ `feedback.js` 显式 👍/👎 按钮上报。禁用时静默跳过（向后兼容）。

---

## 5. 非功能需求（NFR）

#### REQ-NFR-001 冷启动性能
- 优先级：P1 ｜ 状态：Implemented
- 验收标准：AC-1：Apple Silicon 基准机上，从启动到主界面可交互 ≤ 2s（E2E 计时断言）。
- 契约：E2E `nfr.spec.ts` 冷启动计时测试（本地 ≤ 2s，CI ≤ 3s 容忍上限）。

#### REQ-NFR-002 检索性能
- 优先级：P1 ｜ 状态：Done
- 验收标准：AC-1：10,000 chunks 规模下，单次 top-k 检索 P95 ≤ 500ms（基准测试断言）。
- 契约：`SqliteStorage::vector_search` 全表扫描 + Rust cosine similarity + 流式 top-k 筛选 ｜ 关联测试：TC-NFR-002（`#[ignore]` 基准测试，手动运行 `cargo test -p echomind-infra -- --ignored tc_nfr_002`）
- 实测结果（Apple Silicon）：10k chunks / 384-dim / 20 queries，P95=178ms，avg=161ms。

#### REQ-NFR-003 平台覆盖
- 优先级：P1 ｜ 状态：Implemented
- 验收标准：AC-1：CI 产出 macOS（arm64 / x64）、Windows（x64）、Linux（x64）安装包，E2E 至少在 macOS 与 Windows 上执行。
- 契约：`.github/workflows/e2e.yml` build 矩阵（macOS arm64/x64 + Windows x64 + Linux x64）+ e2e-bridge 矩阵（ubuntu + macOS）。

#### REQ-NFR-004 资源占用
- 优先级：P2 ｜ 状态：Implemented
- 验收标准：AC-1：常规会话（知识库 ≤ 2,000 chunks）常驻内存 ≤ 500MB。
- 契约：`TC-NFR-004` `#[ignore]` 基准测试（2k chunks + 384 维向量存储与检索验证，手动运行 `cargo test -- --ignored tc_nfr_004`）。

#### REQ-NFR-005 HNSW 近似最近邻索引
- 优先级：P2 ｜ 状态：Implemented ｜ 依赖：REQ-VEC-003、REQ-NFR-002
- 描述：当前 `SqliteStorage::vector_search` 使用暴力全表扫描 + Rust cosine similarity，在 10k chunks 规模下 P95=178ms（REQ-NFR-002 已达标）。但对于 100k+ chunks 的大规模知识库，暴力扫描 O(n) 复杂度将成为瓶颈。本需求引入 HNSW（Hierarchical Navigable Small World）近似最近邻索引，将检索复杂度从 O(n) 降至 O(log n)。使用 `hnsw_rs` crate（纯 Rust 实现，基于 Malkov & Yashunin 2016/2018 论文，697k 下载），支持 Cosine 距离、多线程插入/搜索、序列化持久化。在 `crates/infra` 新建 `hnsw_index.rs` 模块，封装 `HnswIndex` struct，提供 `build`（批量构建）、`search`（查询 top-k）、`save`/`load`（文件持久化）接口。`SqliteStorage::vector_search` 增加 HNSW 索引路径：如果索引文件存在则用 HNSW 查询，否则回退暴力扫描（无缝降级）。索引构建通过新增 IPC 命令 `build_vector_index` 手动触发，设置键 `vec.hnsw_enabled` 控制是否启用。HNSW 参数：`max_nb_connection=32`、`max_layer=16`、`ef_construction=400`（论文推荐值）。
- 验收标准：
  - AC-1：`HnswIndex::build` 接受 `(chunk_id, vector)` 对列表，构建 HNSW 索引（TC-NFR-005）。
  - AC-2：`HnswIndex::search` 返回 top-k 最近邻 `(chunk_id, score)` 对，score 为 cosine 相似度（TC-NFR-005）。
  - AC-3：`HnswIndex::save` / `load` 持久化到文件，重启后可恢复索引（TC-NFR-005 旁证）。
  - AC-4：`vector_search` 在 HNSW 索引存在时使用 HNSW 查询，不存在时回退暴力扫描，结果一致（TC-NFR-005b）。
  - AC-5：HNSW 查询结果召回率 ≥ 90%（与暴力扫描 top-k 结果比对，TC-NFR-005b 旁证）。
  - AC-6：100k chunks 规模下 HNSW 查询 P95 ≤ 10ms（基准测试断言，TC-NFR-005c `#[ignore]`）。
- 契约：`crates/infra/src/hnsw_index.rs` `HnswIndex` + IPC `build_vector_index` + settings `vec.hnsw_enabled` ｜ 关联测试：TC-NFR-005/005b/005c
- 调研依据：Malkov & Yashunin "Efficient and Robust approximate nearest neighbours using Hierarchical Navigable Small World Graphs" (arXiv:1603.09320, 2016/2018)；`hnsw_rs` crate v0.3.4（纯 Rust，MIT/Apache-2.0，697k 下载，GitHub: jean-pierreBoth/hnswlib-rs）；ann-benchmarks 基准验证 recall ≥ 0.99（sift1m 1M points 128-dim）。本需求引入 1 个新 crate 依赖（`hnsw_rs`），属于 infra 适配器层，不影响 core 端口签名。

#### REQ-NFR-006 流式首 Token 延迟
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-RAG-001、REQ-LLM-001
- 描述：从用户发送提问到前端收到第一个 `chat_token` 事件的延迟须控制在可接受范围内。该延迟由四阶段构成：(1) 查询嵌入（本地 ONNX 推理）；(2) 向量检索 + 关键词检索 + RRF 融合；(3) RAG prompt 组装；(4) LLM 端点 HTTP 建连 + 首 SSE 事件到达。阶段 (1)~(3) 为本地操作，需 ≤ 500ms（10k chunks 规模）；阶段 (4) 取决于用户配置的 LLM 端点延迟，应用本身不引入额外延迟。`chat_phase` 事件（preparing → retrieving → generating）在首 token 前推送进度，消除用户感知的空白等待。
- 验收标准：
  - AC-1：本地操作阶段（嵌入 + 检索 + prompt 组装）延迟 ≤ 500ms（10k chunks 知识库，日志计时断言）。
  - AC-2：`chat_phase` 事件在首 token 前推送三个阶段进度（preparing / retrieving / generating）。
  - AC-3：首 token 到达后立即开始逐字渲染，无批量延迟（token-by-token 流式）。
  - AC-4：本地 Ollama（同机）端点下，从提问到首 token ≤ 3s（E2E 计时断言）。
  - AC-5：远程 LLM 端点下，连接超时 30s 内无响应返回可读错误（REQ-SEC-009）。
- 契约：`chat_inner` → `ChatEngine::chat` → `VectorRetriever::retrieve` + `OpenAIProvider::chat_stream` + `chat_phase` 事件 ｜ 关联测试：TC-NFR-006（本地阶段日志计时）

#### REQ-NFR-007 嵌入吞吐量
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-VEC-002、REQ-VEC-008
- 描述：文档导入的嵌入阶段须满足最小吞吐量要求，避免大规模知识库导入时长时间阻塞。并行推理会话池（`LocalEmbedder` 内部 N 个 `TextEmbedding` 实例）将 `embed_batch` 输入按会话数分片，在 N 个 `spawn_blocking` 任务中并行推理。微批次处理（`EMBED_BATCH_SIZE=64`）每批完成后发射 `embedding_progress` 事件，用户可实时观察进度。FTS5 索引在分块完成后立即可用（不依赖向量写入完成）。
- 验收标准：
  - AC-1：8 核 Apple Silicon 基准机上，384 维嵌入吞吐量 ≥ 200 chunks/s（ONNX 模型已下载缓存后）。
  - AC-2：会话池大小 = `available_parallelism().min(8)`，`pool_size=1` 时回退单会话路径。
  - AC-3：每 `EMBED_BATCH_SIZE`（64）条 chunk 完成后发射 `embedding_progress` 事件。
  - AC-4：分块完成后 FTS5 关键词检索立即可用（向量写入未完成时不阻塞 BM25 检索）。
  - AC-5：首次使用触发模型下载（~30MB），非首次不产生网络请求。
- 契约：`LocalEmbedder::embed_batch`（`shard_texts` 分片 + N×`spawn_blocking`）+ `commands.rs::embed_document_chunks`（`EMBED_BATCH_SIZE=64` 微批次）+ `embedding_progress` 事件 ｜ 关联测试：TC-PERF-001~008

#### REQ-NFR-008 UI 交互响应延迟
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-UI-001
- **v1.0 发布分类**：v1.0 必须交付（vanilla JS 无虚拟 DOM 开销 + spawn_blocking 异步 DB，架构已满足；需补 E2E 交互计时断言） ｜ 决策日期：2026-08-01
- 描述：用户界面交互操作须满足响应延迟要求，保证操作流畅感。所有用户可感知的交互（点击、输入、滚动、拖拽）须在 100ms 内给出视觉反馈。长耗时操作（导入、嵌入、LLM 流式）须立即显示加载状态或进度条，不阻塞 UI 线程。前端为单文件 SPA（vanilla JS），无虚拟 DOM diff 开销，DOM 操作直接操作元素。
- 验收标准：
  - AC-1：点击侧栏会话/文档列表项后 ≤ 100ms 高亮选中（E2E 计时断言）。
  - AC-2：输入框打字无明显延迟（每个字符渲染 ≤ 16ms，60fps）。
  - AC-3：拖拽文件到窗口时遮罩 ≤ 100ms 出现。
  - AC-4：长耗时操作（导入 / 嵌入 / LLM 流式）开始时立即显示加载状态（≤ 100ms）。
  - AC-5：滚动对话区域或文档列表时帧率 ≥ 30fps（无卡顿）。
  - AC-6：数据库操作不阻塞 UI 线程（全部经 `spawn_blocking`，长查询期间 UI 仍响应）。
- 契约：`ui/index.html` 事件处理 + `spawn_blocking` 数据库异步模式 ｜ 关联测试：E2E 交互计时测试

#### REQ-NFR-009 崩溃恢复可靠性
- 优先级：P0 ｜ 状态：Done ｜ 依赖：REQ-DB-001
- 描述：应用在异常退出（崩溃、强杀、断电）后重启时，须自动恢复到一致状态，不丢失已持久化数据，不残留中间状态。启动时执行 `cleanup_zombies()`，将上次会话遗留的 `Processing` 状态文档标记为 `Failed`，附带可读原因。SQLite WAL 模式保证已提交事务的持久性，崩溃时未提交事务自动回滚。外键级联保证引用完整性（删除文档时 chunks/embeddings/messages 级联清理）。
- 验收标准：
  - AC-1：应用崩溃后重启，`cleanup_zombies()` 将 Processing 文档标记为 Failed，返回清理条数（TC-DB-002）。
  - AC-2：崩溃前已成功写入的文档 / chunks / 向量在重启后完整可读（TC-DB-001）。
  - AC-3：崩溃时正在进行的嵌入写入不产生部分向量（INSERT OR REPLACE 原子性）。
  - AC-4：崩溃时正在进行的 LLM 流式对话，已生成的部分内容不丢失（persist_exchange 在中断时保存）。
  - AC-5：外键级联生效，无孤儿 chunks / embeddings / messages（TC-DB-001 AC-3）。
- 契约：`SqliteStorage::cleanup_zombies` + WAL `synchronous=NORMAL` + `PRAGMA foreign_keys=ON` + `ON DELETE CASCADE` ｜ 关联测试：TC-DB-001、TC-DB-002

#### REQ-NFR-010 流式生成中断容错
- 优先级：P0 ｜ 状态：Done ｜ 依赖：REQ-RAG-005、REQ-SEC-009
- 描述：LLM 流式生成过程中，无论是用户主动取消（`abort_chat`）、网络中断、还是 LLM 端点超时，已生成的部分内容须保留并持久化，不得丢失。`CancellationToken` 机制确保取消指令即时生效（`select! { biased; _ = abort_token.cancelled() => break }`），不等待当前 SSE 事件处理完成。中断后发射 `chat_done` 事件（含中断标记），前端显示「⏹ 生成已中断」并保留已渲染内容。
- 验收标准：
  - AC-1：用户点击停止按钮后 ≤ 200ms 内流式中断，已渲染内容保留（TC-RAG-003）。
  - AC-2：中断后 `persist_exchange` 保存用户问题 + 已生成的部分回答到 messages 表（TC-RAG-005）。
  - AC-3：网络中断或 LLM 端点超时（300s 兜底）后，已生成内容保留并持久化。
  - AC-4：中断后 `clear_abort` 清理 CancellationToken，后续对话不受影响（TC-RAG-005）。
  - AC-5：中断后前端显示「⏹ 生成已中断」标记，不显示错误 toast（区分主动取消与异常）。
- 契约：`AppState::abort_chat` / `clear_abort` + `forward_stream` `select! { biased }` + `persist_exchange` ｜ 关联测试：TC-RAG-003、TC-RAG-005

#### REQ-NFR-011 数据库并发写入安全
- 优先级：P0 ｜ 状态：Done ｜ 依赖：REQ-DB-003
- 描述：SQLite 为单写者模型，应用须保证并发场景下不产生 `database is locked` 错误。r2d2 连接池（`max_size=8`）管理连接生命周期，`PRAGMA busy_timeout=5000` 在写冲突时等待 5s 后重试。WAL 模式下读操作不阻塞写操作（并发读安全）。全部数据库操作经 `spawn_blocking` 在独立线程执行，不阻塞 tokio async 运行时。
- 验收标准：
  - AC-1：r2d2 连接池 `max_size=8`，连接初始化时执行 WAL + foreign_keys + busy_timeout PRAGMA。
  - AC-2：`busy_timeout=5000` 生效，写冲突时等待 5s 而非立即返回 `SQLITE_BUSY`。
  - AC-3：并发导入（多个 `import_files` 同时执行）不产生 `database is locked` 错误。
  - AC-4：WAL 模式下读操作（list_documents / vector_search）不阻塞写操作（add_document / add_chunk）。
  - AC-5：连接池耗尽时等待 30s 后返回 Err（而非 panic）。
- 契约：`SqliteStorage::new` — `r2d2::Pool::builder().max_size(8)` + `PRAGMA busy_timeout=5000` + WAL 模式 ｜ 关联测试：TC-DB-001（并发写入验证）

#### REQ-NFR-012 知识库容量可扩展性
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-LIC-002、REQ-NFR-002
- 描述：应用须支持不同规模的知识库，从个人笔记（数十文档）到企业文档库（万级文档 / 十万级 chunks）。免费版限 50 文档（`FREE_TIER_MAX_FILES=50`），Pro 版无上限。向量检索在 10k chunks 下 P95 ≤ 500ms（暴力扫描），100k+ chunks 通过 HNSW 索引降至 P95 ≤ 10ms（REQ-NFR-005）。FTS5 关键词检索利用 SQLite 内置 BM25 索引，百k级 chunks 下 P95 ≤ 100ms。大文件（>1MB）自动切换 1024 token 分块窗口，减少 chunk 总量 4x。
- 验收标准：
  - AC-1：免费版文档数 ≥ 50 时拒绝导入并提示配额限制。
  - AC-2：Pro 版无文档数上限，10k chunks 下向量检索 P95 ≤ 500ms（TC-NFR-002）。
  - AC-3：100k+ chunks 下 HNSW 检索 P95 ≤ 10ms（TC-NFR-005c）。
  - AC-4：>1MB 文件自动使用 1024 token 分块窗口（`auto_chunk_tokens`）。
  - AC-5：FTS5 关键词检索在 100k chunks 下 P95 ≤ 100ms（基准测试旁证）。
- 契约：`FREE_TIER_MAX_FILES=50` + `auto_chunk_tokens` + `HnswIndex::search` + FTS5 BM25 ｜ 关联测试：TC-NFR-002、TC-NFR-005c、TC-PERF-006

#### REQ-NFR-013 嵌入并发并行度
- 优先级：P1 ｜ 状态：Implemented ｜ 依赖：REQ-VEC-002
- 描述：嵌入推理须充分利用多核 CPU 并行能力。`LocalEmbedder` 维护 N 个独立 ONNX 会话（`Vec<Arc<Mutex<TextEmbedding>>>`），`embed_batch` 将输入按会话数分片（`shard_texts` 纯逻辑函数），在 N 个 `spawn_blocking` 中并行推理，结果按原始顺序合并。会话池大小默认为 `available_parallelism().min(8)`（`MAX_POOL_SIZE=8` 封顶防止内存爆炸，8×30MB≈240MB）。每个 ONNX 会话内部由 fastembed 按 `available_parallelism` 设置 intra-op 线程数。
- 验收标准：
  - AC-1：`default_pool_size()` 返回 `available_parallelism().min(8)`（TC-PERF-001）。
  - AC-2：`shard_texts` 将输入按会话数分片，结果按原始顺序合并（TC-PERF-002）。
  - AC-3：`pool_size=1` 时回退单会话路径，零额外开销（TC-PERF-003）。
  - AC-4：`MAX_POOL_SIZE=8` 封顶，`new_with_pool_size(100)` 实际创建 8 个会话（TC-PERF-004）。
  - AC-5：8 会话并行推理吞吐量 ≥ 单会话的 4x（8 核基准机，TC-PERF-005）。
- 契约：`LocalEmbedder` — `Vec<Arc<Mutex<TextEmbedding>>>` + `shard_texts` + `default_pool_size` + `MAX_POOL_SIZE=8` ｜ 关联测试：TC-PERF-001~005

#### REQ-NFR-014 代码质量门禁
- 优先级：P0 ｜ 状态：Done ｜ 依赖：REQ-ARCH-003
- 描述：CI 强制执行代码质量门禁，不合格代码不得合并。门禁包含三个维度：(1) Clippy 零警告零错误（`cargo clippy --all-targets -- -D warnings`，`unwrap_used` / `expect_used` / `panic` / `unreachable` / `todo` / `unimplemented` 全部 deny）；(2) 代码格式统一（`cargo fmt --check`）；(3) 供应链安全审计（`cargo deny check`，覆盖 RustSec 漏洞 / 许可证合规 / crate 来源 / yanked 检测）。测试代码通过 `#![allow(clippy::unwrap_used, clippy::expect_used)]` 局限在 `#[cfg(test)]` 块内豁免。
- 验收标准：
  - AC-1：`cargo clippy --all-targets -- -D warnings` 零警告零错误（CI 门禁）。
  - AC-2：`cargo fmt --check` 通过（CI 门禁）。
  - AC-3：`cargo deny check` 四维全部通过（CI supply-chain job）。
  - AC-4：全部 workspace member 的 `Cargo.toml` 含 `[lints] workspace = true`。
  - AC-5：`commands.rs` 文件级豁免仅含 `unreachable`（`#[tauri::command]` 宏生成代码），手写代码不使用。
- 契约：`Cargo.toml` `[workspace.lints.clippy]` + `deny.toml` 四维配置 + `.github/workflows/e2e.yml` CI 门禁 ｜ 关联测试：CI 全量门禁

#### REQ-NFR-015 测试金字塔覆盖率
- 优先级：P0 ｜ 状态：Done ｜ 依赖：REQ-ARCH-006
- 描述：应用须维持测试金字塔结构，确保代码质量可验证。三层测试金字塔：(1) L1 单元测试（`#[cfg(test)] mod *_tests`，同文件内，~136 个测试函数）覆盖 core / infra 层纯逻辑与端口契约；(2) L2 集成测试（`crates/tauri-app/tests/integration.rs`，~39 个测试函数）通过 `*_inner` 函数复用命令逻辑，mock AppState 验证端到端管线；(3) L3 E2E 测试（`e2e-tests/tests/*.spec.ts`，~141 个测试用例）通过 Playwright + tauri-stub.js 在浏览器中验证完整用户旅程。全部测试须在 CI 中通过（`#[ignore]` 基准测试除外）。
- 验收标准：
  - AC-1：`cargo test` 全量通过（0 failed，`#[ignore]` 测试除外）。
  - AC-2：L1 单元测试覆盖 core / infra 全部端口 Trait 实现。
  - AC-3：L2 集成测试通过 `*_inner` 函数复用命令逻辑，不依赖 Tauri 运行时。
  - AC-4：L3 E2E 测试覆盖核心用户旅程（导入 → 配置 → 对话 → 审计）。
  - AC-5：`#[ignore]` 基准测试（TC-NFR-002 / TC-NFR-004 / TC-NFR-005c）可在本地手动运行通过。
- 契约：`#[cfg(test)] mod *_tests` + `tests/integration.rs` + `e2e-tests/tests/*.spec.ts` + CI test job ｜ 关联测试：全量测试套件

#### REQ-NFR-016 文档实时同步
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-ARCH-000
- 描述：项目文档须与代码实现实时同步，作为开发、测试、验收的唯一依据。核心文档集：(1) `docs/SRS_v1.0.md`（需求规格）；(2) `docs/api/code-contracts.md`（端口与适配器契约）；(3) `docs/api/ipc-commands.md`（IPC 命令清单）；(4) `docs/api/settings-reference.md`（设置键值参考）；(5) `docs/api/frontend-modules.md`（前端模块说明）；(6) `docs/TEST_STRATEGY.md`（测试策略与追溯矩阵）；(7) `AGENTS.md` / `CLAUDE.md`（Agent 开发指南）。每次需求变更须同步更新 SRS 修订记录 + 相关文档章节，铁律要求「代码变更 = 文档变更」。
- 验收标准：
  - AC-1：SRS 每条 REQ 的「契约」字段与代码实现一致（TPM 审计）。
  - AC-2：SRS 每条 REQ 的「关联测试」字段与实际测试用例对应（QA 审计）。
  - AC-3：IPC 命令清单（`ipc-commands.md`）与 `lib.rs::run()` 注册的命令数量一致。
  - AC-4：设置键值参考（`settings-reference.md`）与 `set_setting` / `get_setting` 调用的 key 一致。
  - AC-5：SRS §7 修订记录每次变更追加版本条目（日期 + 摘要 + 批准人）。
- 契约：铁律七文档实时同步规则 + `.catpaw/rules/system-constraints.md` ｜ 关联测试：TPM / QA 人工审计

#### REQ-NFR-017 操作系统版本兼容性
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-NFR-003
- **v1.0 发布分类**：v1.0 必须交付（已实现 #[cfg(unix)] 平台条件 + Tauri v2 WebView 依赖 + CI 三平台构建矩阵 .github/workflows/build.yml） ｜ 决策日期：2026-08-01
- 描述：应用须在主流桌面操作系统上运行，CI 产出三平台安装包。最低操作系统版本：macOS 10.15（Catalina，Tauri v2 WebView 依赖）；Windows 10（x64，WebView2 预装或自动安装）；Linux（Ubuntu 20.04+，WebKit2GTK 4.1）。Tauri v2 使用系统 WebView（macOS WKWebView / Windows WebView2 / Linux WebKitGTK），不捆绑 Chromium。Rust `std::fs` 权限操作（`ensure_dir_0700` / `ensure_file_0600`）仅在 Unix 生效，Windows 跳过（`#[cfg(not(unix))]`）。
- 验收标准：
  - AC-1：CI 产出 macOS arm64 / x64、Windows x64、Linux x64 安装包。
  - AC-2：macOS 最低支持 10.15（Catalina），Windows 最低支持 Windows 10。
  - AC-3：Linux 依赖 WebKit2GTK 4.1（Ubuntu 20.04+ 默认提供）。
  - AC-4：文件权限操作（0700 / 0600）在 Unix 生效，Windows 不 panic（`#[cfg(not(unix))]` 跳过）。
  - AC-5：E2E 测试至少在 macOS 与 Ubuntu CI 环境执行通过。
- 契约：`.github/workflows/build.yml` 三平台构建矩阵 + `.github/workflows/e2e.yml` build 矩阵 + Tauri v2 WebView 依赖 + `#[cfg(unix)]` / `#[cfg(not(unix))]` 平台条件 ｜ 关联测试：CI 多平台构建 + E2E

#### REQ-NFR-018 开发工具链版本约束
- 优先级：P0 ｜ 状态：Done ｜ 依赖：REQ-ARCH-002
- 描述：项目使用 Rust Edition 2024（`edition = "2024"`，`resolver = "3"`），要求 Rust 工具链 ≥1.85。Edition 2024 支持原生 `async fn` in trait（不使用 `#[async_trait]` 宏），`#![allow(async_fn_in_trait)]` 为刻意豁免。Dev profile `[profile.dev.package."*"] opt-level = 3` 确保 ML 依赖（fastembed / ort / tokenizers）在 dev 模式下可用（opt-level=0 时推理极慢）。Tauri v2（`tauri = "2.0"`）为当前稳定版本。
- 验收标准：
  - AC-1：`Cargo.toml` 含 `edition = "2024"` 且 `resolver = "3"`（grep 断言）。
  - AC-2：Rust 工具链版本 ≥ 1.85（`rustc --version` 检查）。
  - AC-3：`crates/core` 源码不出现 `#[async_trait]` 属性（grep 断言）。
  - AC-4：`[profile.dev.package."*"] opt-level = 3` 存在且生效。
  - AC-5：dev 模式下单次 384 维嵌入推理 < 100ms（opt-level=3 生效验证）。
- 契约：`Cargo.toml` edition/resolver + `[profile.dev.package."*"]` + `rust-toolchain` ≥1.85 ｜ 关联测试：CI 构建门禁

#### REQ-NFR-019 构建产物体积
- 优先级：P2 ｜ 状态：Draft ｜ 依赖：REQ-ARCH-000
- 描述：应用安装包体积须控制在合理范围内，避免用户下载门槛过高。体积主要来源：Rust 静态链接二进制（含 fastembed / ort / tokenizers / rusqlite-bundled / reqwest / tokio）、ONNX 模型文件（首次使用下载，不打包进安装包）、前端本地 vendored 库（Tailwind / marked.js / DOMPurify / highlight.js / Mermaid / KaTeX / Chart.js）。Release profile 启用 LTO + strip 减小体积。
- 验收标准：
  - AC-1：macOS arm64 安装包（.dmg）≤ 50MB（不含 ONNX 模型）。
  - AC-2：Windows x64 安装包（.msi）≤ 60MB（不含 ONNX 模型）。
  - AC-3：ONNX 模型（~30MB）不打包进安装包，首次使用时从 HuggingFace 下载。
  - AC-4：前端 vendored 库总体积 ≤ 5MB（`ui/vendor/` 目录大小）。
  - AC-5：Release profile 启用 LTO（`lto = true`）+ strip（`strip = true`）。
- 契约：`Cargo.toml` `[profile.release]` LTO + strip + Tauri bundle 配置 ｜ 关联测试：CI 构建产物体积检查

#### REQ-NFR-020 增量编译性能
- 优先级：P1 ｜ 状态：Done ｜ 依赖：REQ-ARCH-004
- 描述：开发体验要求增量编译速度快，单个文件修改后 `cargo build` 增量时间合理。`[profile.dev.package."*"] opt-level = 3` 导致首次编译较慢（~5-10 min，全部依赖以 opt-level=3 编译），但增量编译仅重编译变更的 workspace member（opt-level=0）。`cargo tauri dev` 支持热重载，前端修改即时生效无需重编译 Rust。
- 验收标准：
  - AC-1：首次 `cargo build` 完成（~5-10 min，含全部依赖 opt-level=3 编译）。
  - AC-2：单个 Rust 文件修改后 `cargo build` 增量时间 ≤ 30s。
  - AC-3：`cargo tauri dev` 前端修改（`ui/index.html`）即时生效（无需 Rust 重编译）。
  - AC-4：`cargo check` 增量时间 ≤ 10s（快速类型检查）。
  - AC-5：`cargo clippy --all-targets` 增量时间 ≤ 60s（含测试编译）。
- 契约：`Cargo.toml` `[profile.dev.package."*"] opt-level = 3` + Tauri dev 热重载 ｜ 关联测试：手动计时验证

#### REQ-NFR-021 核心功能离线可用性
- 优先级：P0 ｜ 状态：Implemented ｜ 依赖：REQ-SEC-011
- 描述：应用的核心功能须在完全离线环境下可用（ONNX 模型已下载缓存后）。离线可用的功能：(1) 文档导入（md / txt / pdf 解析、分块、FTS5 索引）；(2) 本地嵌入（fastembed/ONNX 推理，首次需下载模型）；(3) 向量检索 + 关键词检索（本地 SQLite + Rust cosine similarity）；(4) 会话管理（创建 / 删除 / 历史加载）；(5) 审计（声明提取 + 矛盾检测，但需 LLM 端点）；(6) 设置管理。需联网的功能仅限：(1) LLM 对话（用户配置的端点）；(2) VLM 图片理解（用户配置的端点）；(3) ONNX 模型首次下载（HuggingFace）。前端全部资源本地 vendored，无 CDN 依赖。
- 验收标准：
  - AC-1：断网状态下文档导入全流程通过（解析 → 分块 → 嵌入 → 入库，模型已缓存）。
  - AC-2：断网状态下向量检索 + 关键词检索 + RRF 融合正常返回结果。
  - AC-3：断网状态下会话管理（创建 / 删除 / 历史加载 / 消息持久化）正常。
  - AC-4：断网状态下前端全部功能正常（加载 / 渲染 / 交互，无 CDN 依赖）。
  - AC-5：仅 LLM 对话 / VLM 图片理解 / 模型首次下载需要联网，其余功能离线可用。
- 契约：`crates/infra` 本地嵌入 + 本地 OCR + 本地 SQLite + `ui/vendor/` 零 CDN ｜ 关联测试：E2E 断网测试（REQ-ARCH-007 AC-3）

---

## 6. v1.0 范围外（Out of Scope）

以下功能不属于 v1.0 交付范围，列为后续版本候选或明确排除：

### 6.1 数据与同步
- **云同步与多端协同**：知识库与对话记录不提供云端同步，全部数据本地存储（REQ-SEC-011）。
- **多用户 / 权限体系**：桌面单用户应用，无多用户登录与权限管理。
- **团队 / 共享工作空间**：不支持多人共享知识库或协作标注（REQ-WS 工作空间为本地多知识库管理，非团队协作）。

### 6.2 平台与终端
- **移动端（iOS / Android）**：v1.0 仅支持桌面端（macOS / Windows / Linux，REQ-NFR-003）。
- **Web 版 / SaaS 部署**：本地优先架构，不提供 Web 端访问或云端托管。
- **浏览器扩展 / 网页剪藏**：不支持从浏览器一键剪藏网页内容到知识库。

### 6.3 文档格式
- **`.docx` / `.pptx` / `.epub` 导入**：v1.0 仅支持 `.md` / `.txt` / `.pdf`（REQ-SEC-006 白名单）。其中 `.docx` / `.html` 已在 v1.1 实现（REQ-ING-015 / REQ-ING-016，✅ Implemented），`.pptx` / `.epub` 已列入 v1.2 实施计划（REQ-ING-017 / REQ-ING-018，S50 / S51）。
- **音频 / 视频转录**：不支持音视频文件导入与自动转录。
- **代码仓库导入**：不支持 Git 仓库批量导入与代码语义索引。

### 6.4 UI 与主题
- **浅色主题**：v1.0 仅暗色主题（REQ-DS-007）。浅色主题已列入 v1.1 实施计划（REQ-UI-011，S46），通过 CSS 变量覆盖实现，无需组件代码改动。
- **自定义主题 / 皮肤**：不支持用户自定义色板或字体（v1.2+ 候选）。
- **多语言 UI**：v1.0 界面文案为中文硬编码，I18N 框架仅预留架构（REQ-I18N-001 Draft），不提供语言切换。

### 6.5 模型与 AI 能力
- **自定义 Embedding 模型上传**：已列入 v1.2 实施计划（REQ-VEC-014，S52，Pro 门控）。Pro 用户可上传自定义 ONNX 嵌入模型，Free 用户仅限 3 个预设模型（AllMiniLML6V2 / BgeSmallZhV1_5 / E5SmallV2，REQ-VEC-012）。
- **LLM 微调 / 本地训练**：不支持在本地知识库上微调 LLM。
- **多模态对话（图片 / 音频输入）**：VLM 仅用于 PDF 图片理解（导入阶段），不支持在对话中直接发送图片或音频。
> **v2.2.0 修正**：「Agent / 工具调用」与「自动文档分类 / 标签」原列于此处，已随 REQ-RAG-022（Agentic RAG）与 REQ-VEC-013（领域画像）在 v2.2.0 中实现，不再属于范围外（见 §6.7）。

### 6.6 高级功能
- **文档版本管理 / Diff 对比**：不支持文档多版本管理与差异对比。
- **知识图谱可视化**：不支持基于实体关系的知识图谱可视化（向量检索 + FTS5 关键词检索为唯一检索通道）。
- **网页搜索集成**：已列入 v1.1 实施计划（REQ-RAG-036，S48），DuckDuckGo Instant Answer API + 可选 SearXNG 自托管，opt-in 默认关闭。
- **语音输入 / TTS 朗读**：已列入 v1.1 实施计划（REQ-RAG-034 / REQ-RAG-035，S47），基于 Web Speech API（浏览器原生，零依赖）。
- **文档批注 / 高亮**：不支持在导入的文档上添加批注或高亮标记（v1.4+ 候选）。
- **批量导出对话 / 知识库为外部格式**：PDF 导出已在 v1.2 实现（REQ-EXP-005，✅ Implemented）。知识图谱导出（GraphML/JSON-LD）已在 v1.3 S54 实现（REQ-EXP-006，✅ Implemented）。Word 文件集导出仍为 v1.4+ 候选。
- **文档摘要自动生成**：已在 v1.3 S55 实现（REQ-ING-019，✅ Implemented），导入时自动调用 LLM 生成 200-300 字摘要，支持手动重新生成。
- **Markdown 笔记双向链接**：已完成 v1.3 S57 实施（REQ-ING-020，✅ Implemented），Obsidian 风格 `[[wiki-link]]` 双向链接 + 反向链接面板。
- **对话模板/快捷指令系统**：已列入 v1.3 实施计划（REQ-RAG-038，S56），用户自定义 slash command 模板。
- **对话分支/版本树**：已完成 v1.3 S58 实施（REQ-RAG-039，✅ Implemented），ChatGPT 风格可视化对话分支树。

### 6.7 已从范围外移除

> **v1.4.1 修正**：「OCR 与扫描件识别」原列于此处，但已随 MM 域（REQ-MM-001~005）在 v1.1.0~v1.3.1 中全部实现并验收通过（本地 ocrs OCR + BYOK VLM + 分级图表理解），不再属于范围外。
>
> **v2.2.0 修正**：「Agent / 工具调用」原列于 §6.5，已随 REQ-RAG-022（Agentic RAG ReAct 多步推理）在 v2.2.0 中实现，不再属于范围外。
>
> **v3.2.0 规划**：v1.1 功能规划启动——`.docx` 导入（REQ-ING-015，✅ Implemented）、`.html` 导入（REQ-ING-016，✅ Implemented）、浅色主题（REQ-UI-011）、语音输入（REQ-RAG-034）、TTS 朗读（REQ-RAG-035）、网页搜索集成（REQ-RAG-036）共 6 条新需求，原子计划见 `docs/architecture/V1.1_ATOMIC_PLAN.md`（S44-S48）。
>
> **v5.1.0 规划**：v1.2 功能规划启动——PDF 导出（REQ-EXP-005，S49）、`.pptx` 导入（REQ-ING-017，S50）、`.epub` 导入（REQ-ING-018，S51）、自定义 ONNX 嵌入模型上传（REQ-VEC-014，S52，Pro 门控）共 4 条新需求，原子计划见 `docs/architecture/V1.2_ATOMIC_PLAN.md`（S49-S53，5 个 Session，预计 5 天）。
>
> **v5.7.0 规划**：v1.3 功能规划启动——知识图谱导出（REQ-EXP-006，S54）、文档摘要自动生成（REQ-ING-019，S55）、对话模板/快捷指令系统（REQ-RAG-038，S56）、Markdown 笔记双向链接（REQ-ING-020，S57）、对话分支/版本树（REQ-RAG-039，S58）共 5 条新需求，原子计划见 `docs/architecture/V1.3_ATOMIC_PLAN.md`（S54-S59，6 个 Session，预计 8.5 天）。零新增依赖。
>
> **v5.9.0 规划**：v1.4 功能规划启动——.xlsx/.csv 文档导入（REQ-ING-021，S61，calamine + csv 纯 Rust crate）、对话全文搜索（REQ-RAG-040，S62，FTS5 全文索引）、Contextual Retrieval 上下文增强嵌入（REQ-RAG-041，S63，Anthropic 2024 论文，铁律十前沿技术）、文档标签系统（REQ-ING-022，S64，Document.tags + 前端标签管理）、commands.rs 模块化拆分（REQ-ARCH-001，S65，SRP 违例修复，铁律十四）共 5 条新需求，原子计划见 `docs/architecture/V1.4_ATOMIC_PLAN.md`（S61-S66，6 个 Session，预计 9 天）。新增依赖 calamine v0.26 + csv v1.3。IPC 命令 152→159（+7 Free）。

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
| v1.3.1 | 2026-07-25 | REQ-MM-005 分级图表理解状态 Draft→Approved + 落地：新建 `vlm_prompt` 模块集中定义 `VLM_TIERED_PROMPT` 4 级策略（Level 1 表格→MD / Level 2 流程图→Mermaid / Level 3 数据图表→CSV+趋势+误差标注 / Level 4 示意图→文字 + 数学公式→LaTeX）；`loader.rs` 与 `import.rs` 消除重复 `VLM_IMAGE_PROMPT` 常量，统一引用 `vlm_prompt::VLM_TIERED_PROMPT`；TC-MM-007/007a/007b/007c 全部绿灯 | TPM |
| v1.3.2 | 2026-07-25 | AUDIT 域开发绿灯实现完成 + 文档同步：(1) `crates/core/src/audit.rs` 实现 Decompose→Verify→Report 三阶段流水线（批量 LLM 声明提取 + embedding 预筛 cosine≥0.65 + LLM 精判 + 优雅降级）；(2) `crates/tauri-app/src/commands.rs` 新增 `audit_document` / `abort_audit` IPC 命令（17 个命令），`report_to_markdown` 渲染 Markdown 勘误报告；(3) `crates/tauri-app/src/state.rs` 新增 `audit_cancels` HashMap（`Arc<AtomicBool>` 取消机制）；(4) 9/9 测试全绿（TC-AUDIT-001~005b）；(5) REQ-AUDIT-001~005 状态 Draft→Approved + 契约/测试引用更新；REQ-RAG-009（Chunk Expansion）状态 Draft→Approved + 契约/测试引用更新 | DEV / TPM |
| v1.3.3 | 2026-07-25 | REQ-NFR-002 检索性能 Draft→Approved + 落地：新增 `tc_nfr_002_vector_search_p95_under_500ms` 基准测试（10k chunks / 384-dim / 20 queries / P95 断言 ≤ 500ms），标记 `#[ignore]` 避免拖慢 CI；实测 Apple Silicon P95=178ms，avg=161ms | DEV |
| v1.3.4 | 2026-07-25 | REQ-SEC-004 本地数据目录隔离 Draft→Approved + 落地：新增 `ensure_dir_0700` / `ensure_file_0600` 辅助函数（`#[cfg(unix)]` / `#[cfg(not(unix))]` 平台条件），`SqliteStorage::new` 创建数据目录后设 0700，`lib.rs` 启动时设 data_dir 0700；TC-SEC-004 验证目录权限 0700 + 密钥文件 0600 + 数据隔离 + 卸载清除 | DEV |
| v1.4.0 | 2026-07-25 | 需求状态全量同步（铁律七文档实时同步）：21 条 Contract-Frozen + 11 条 Approved → Done（契约已冻结 + L2 契约测试/E2E 测试通过）；12 条 Approved → Implemented（代码已实现，契约/测试 TBD 待冻结）；更新后状态分布：Done 41 / Implemented 19 / Draft 1；残留 Contract-Frozen/Approved = 0 | TPM |
| v1.4.1 | 2026-07-25 | §6 Out of Scope 修正：移除「OCR 与扫描件识别」（已随 REQ-MM-001~005 全部实现并验收）；`cargo fmt` 格式统一（commands.rs / state.rs / integration.rs 共 7 处差异修复） | TPM |
| v1.4.8 | 2026-07-25 | REQ-VEC-008 模型下载管理落地：(1) infra 层：`LocalEmbedder` 新增 `DownloadEvent` 枚举（Downloading/Loading/Done/Error）+ `DownloadProgressFn` 回调类型 + `new_with_progress` 方法（流式读取 64KB 缓冲区实时推送进度）+ `get_cache_info` / `clear_cache` 静态方法（递归目录大小统计 + 删除）；(2) tauri-app 层：新增 `init_embedder` / `get_model_cache_info` / `clear_model_cache` 3 个 IPC 命令（21→24 个命令）+ `model_download_progress` 事件推送（Tauri `app.emit`）+ `state.rs` 新增 `embedder_initialized()` + `init_embedder_with_progress()` 方法；(3) 前端：设置面板新增「模型缓存」区（大小显示 + 下载模型/清理缓存按钮）+ `model_download_progress` 事件监听 + toast 进度反馈 + `formatBytes` 工具函数；(4) 5/5 新增单元测试全绿（TC-VEC-008/008b/009/009b/009c），clippy 零警告 |
| v1.4.7 | 2026-07-25 | A1-1 论文审稿扩展落地：`crates/core/src/audit.rs` 新增 `AuditMode` 枚举（ConsistencyCheck/PaperReview）+ `ReviewDimension` 4 维度（Methodology/Statistics/Logic/Reproducibility）+ `PaperReviewReport` + `build_paper_review_prompt` 系统提示词；5/5 新增单元测试全绿（TC-AUDIT-006~010），clippy 零警告；调研依据 Google PAT 框架 + FIND 基准 |
| v1.4.6 | 2026-07-25 | E3-1 Embedding 模型选择落地：`LocalEmbedder` 重构硬编码常量为 `EmbeddingModel` 枚举（3 预设：AllMiniLML6V2/BgeSmallZhV1_5/E5SmallV2），新增 `new_with_model` 方法支持模型切换，向后兼容 `new` 委托默认模型；2/2 新增单元测试全绿（TC-VEC-007/007b 配置断言），clippy 零警告 |
| v1.4.5 | 2026-07-25 | F1-2 ANN 索引增量维护落地：`SqliteStorage` 新增 `load_all_embeddings`（HNSW 构建用全量向量加载）+ `get_chunks_by_ids`（HNSW 查询后按 ID 批量获取 chunk 详情）公开方法；2/2 新增单元测试全绿（TC-NFR-005f/005g），clippy 零警告；infra 总计 27/27+2 ignored 测试通过 |
| v1.4.4 | 2026-07-25 | REQ-NFR-005 HNSW 向量索引核心落地：新建 `crates/infra/src/hnsw_index.rs`，封装 `HnswIndex`（build + search + save/load JSON 持久化），引入 `hnsw_rs` v0.3.4 依赖（纯 Rust HNSW，Cosine 距离，Malkov & Yashunin 论文）；5/5 单元测试全绿（TC-NFR-005 系列：构建+搜索+召回率≥90%+空边界+持久化往返），clippy 零警告；参数 max_nb_connection=32/max_layer=16/ef_construction=400/ef_search=64（论文推荐值） |
| v1.5.1 | 2026-07-25 | FTS5 短语查询 Bug 修复：`keyword_search` 之前将整个查询包裹为 FTS5 精确短语匹配（`"trampoline 是什么？"`），导致文档中不存在该完整短语时返回 0 条——即使文档含 `trampoline` 关键词。修复：新建 `build_fts5_or_query` 辅助函数，将查询分词后逐词包裹为双引号短语用 `OR` 连接，任一 token 命中即可返回。分词策略：英文按空格分割保留 `_`/`-`；纯 CJK 无空格时拆为 3 字符滑窗（trigram 最小匹配单位）。安全：每 token 双引号转义防 FTS5 操作符注入（REQ-SEC-012）。3/3 新增回归测试全绿（TC-KEYWORD-007 混合语言整句、TC-KEYWORD-008 中文整句、TC-KEYWORD-009 注入防护），5/5 `build_fts5_or_query` 单元测试全绿，全量 190 passed 0 failed 5 ignored，clippy 零警告 | DEV / TPM |
| v1.5.0 | 2026-07-25 | GB 级文档导入加速（三条路径并行落地）：(1) **路径 2 并行推理会话池**：`LocalEmbedder` 从单 `Arc<Mutex<TextEmbedding>>` 重构为 `Vec<Arc<Mutex<TextEmbedding>>>` 会话池，`embed_batch` 将输入按会话数分片后在 N 个 `spawn_blocking` 中并行推理；新增 `default_pool_size()`/`new_with_pool_size()`/`shard_texts()`/`pool_size()`/`dim()` 方法；默认池大小 = `available_parallelism().min(8)`；(2) **路径 4 渐进式索引**：`embed_document_chunks` 重构为微批次处理（`EMBED_BATCH_SIZE=64`），每批完成后发射 `embedding_progress` 事件（`EmbeddingProgressPayload`）；前端新增事件监听，复用进度条渲染向量化进度；分块完成后 FTS5 立即可用（BM25 关键词检索不依赖向量）；(3) **路径 5 分块大小可配置**：`ImportService` 新增 `chunk_tokens` 字段 + `with_chunk_tokens()` 构造器 + `auto_chunk_tokens()` 自动选择（>1MB→1024 tokens，<1MB→256 tokens）；`SectionAwareSplitter` 新增 `new_with_config()`；新增 `DEFAULT_CHUNK_TOKENS`/`LARGE_FILE_CHUNK_TOKENS`/`LARGE_FILE_THRESHOLD` 常量。8/8 新增测试全绿（TC-PERF-001~008），全量 182 passed 0 failed 5 ignored，clippy 零警告 | DEV / TPM |
| v1.4.3 | 2026-07-25 | REQ-LIC-005 License 签发 CLI 工具落地：新建 `crates/license-issuer` 独立二进制 crate（genkey + issue 子命令），Ed25519 密钥对生成 + License Key 签发，与客户端 `verify_license_with_key` 闭环验证；10/10 单元测试全绿（TC-LIC-007/008 系列），clippy 零警告；私钥文件权限 0600，零网络依赖 |
| v1.4.2 | 2026-07-25 | REQ-RAG-010 混合检索落地：(1) 新建 `crates/core/src/hybrid_retriever.rs`——`HybridRetriever` 实现 `Retriever` 端口，向量检索 + 关键词检索 → RRF 融合（k=60，向量权重 1.0/关键词权重 0.8）→ Chunk Expansion；(2) `crates/infra/src/sqlite_storage.rs` 新增 FTS5 `chunks_fts` 虚拟表（trigram 分词器）+ `keyword_search` + `delete_chunks_by_doc` + 旧库迁移回填；(3) `crates/tauri-app/src/commands.rs` 新增 `set_hybrid_search` IPC 命令（21 个命令），`chat_inner` 按 `rag.hybrid_search` 设置构建 `HybridRetriever`；(4) `SettingsPayload` 新增 `hybrid_search: bool` 字段；(5) 测试全绿：TC-RRF-001~014（14 个 RRF 单元测试）+ TC-HYBRID-001~005（5 个混合检索集成测试）+ TC-KEYWORD-001~006（6 个 FTS5 关键词测试），clippy 零警告 | DEV / TPM |
| v2.0.0 | 2026-07-25 | **SRS 原子化重构 v2.0 — 全面覆盖功能/UI/交互/安全/架构/数据维度**：(1) **NFR 域深度扩展**（5→21 条）：新增 REQ-NFR-006~021 覆盖 7 个子域——性能（首 Token 延迟 / 嵌入吞吐量 / UI 交互延迟）、可靠性（崩溃恢复 / 流式中断容错 / 并发写入安全）、可扩展性（知识库容量 / 嵌入并行度）、可维护性（代码质量门禁 / 测试金字塔 / 文档同步）、兼容性（OS 版本 / 工具链版本）、构建部署（产物体积 / 增量编译）、离线能力（核心功能离线可用）；(2) **§6 范围外结构化**：从单行文本扩展为 7 个分类（数据同步 / 平台终端 / 文档格式 / UI 主题 / 模型 AI / 高级功能 / 已移除），覆盖 20+ 条明确排除项，每条标注关联 REQ 交叉引用；(3) 全部新需求均含可测试 AC + 契约引用 + 关联测试编号，状态标注基于代码实现实际状态 | TPM |
| v2.1.0 | 2026-07-25 | **UI/UX 原子化重构 — 四领域规格深度落地**：将 UI（§4.4）、IX（§4.15）、DS（§4.14）、ERR（§4.21）四个领域从模糊描述重构为无歧义、可测试的「原子组件规格」，每条需求绑定 DOM ID / Tailwind 类 / IPC 契约 / 状态矩阵 / 交互四元组：(1) **UI 域**——Block 式对话区（消息块 DOM 结构 + 流式 Markdown 渲染管线 marked→DOMPurify→highlight.js + rAF 节流）+ 控制台输入框（`#inputBar` 状态矩阵：idle/streaming/error 三态显隐与可用性 + 键盘快捷键 Ctrl+Enter 发送 / Esc 中断 + 字符计数）+ 配置向导四步流程原子化；(2) **DS 域**——语义化色板（primary/secondary/accent/success/warning/error/critical 全部绑定 Tailwind 类与 HEX 值）+ 4px 间距网格（4/8/12/16/24/32/48/64 token 表）+ 字体层级（display/h1~h4/body/caption 映射 Tailwind text-* + line-height + font-weight）+ 圆角阴影体系（rounded-sm/md/lg/full + shadow-sm/md/lg/xl 语义映射）+ 暗色模式语义 token；(3) **IX 域**——右键上下文菜单（文档列表 / 对话消息双场景 + 5+3 菜单项 + 禁用态规则）+ 拖拽排序（HTML5 Drag API + 拖拽幽灵视觉反馈 + 释放区高亮 + 排序持久化 IPC 调用）+ 复制行为（纯文本 vs Markdown 复制 + code block 一键复制 + 复制成功视觉反馈 Timing 200ms）+ 悬停反馈（hover transition 150ms ease + 列表项 hover 背景色 + 按钮 hover 阴影提升）+ 焦点可见性（focus-visible ring-2 + 键盘 Tab 顺序）；(4) **ERR 域**——统一错误分类六元组（错误类型 → 结构化前缀 `NETWORK:`/`AUTH:`/`QUOTA:`/`STORAGE:`/`EMBED:`/`LLM:`/`PARSE:` → 消息模板 → Toast 样式映射 → 恢复建议 → 是否可重试）+ `sanitizeError()` 脱敏规则（API Key `****`+last4 + 用户路径脱敏 + 堆栈信息过滤）+ 错误降级策略（网络断连重连 / LLM 超时降级 / 嵌入失败跳过 / 存储锁定等待）| TPM |
| v2.1.1 | 2026-07-26 | **测试资产同步补全**：(1) 新增 TC-NFR-006 L1 测试 3 个（`tc_nfr_006_prompt_assembly_under_500ms` / `tc_nfr_006b_chat_engine_local_pipeline_under_500ms` / `tc_nfr_006c_pipeline_stage_ordering`，覆盖 REQ-NFR-006 AC-1/AC-2 本地管线计时 + 三阶段顺序验证）；(2) 新增 E2E-OFFLINE-001~006 离线可用性 E2E 测试 6 个（`offline.spec.ts`，覆盖 REQ-NFR-021 AC-1~AC-5 断网下导入/会话/设置/零CDN/chat_phase 三阶段）；(3) `tauri-stub.js` 增强 chat mock 发射 `preparing` 阶段事件（与后端三阶段一致）；(4) TEST_STRATEGY v1.6.0 追溯矩阵补全 NFR-006~021 共 16 条 + OBS/WIN/I18N/HELP 四域规划条目；(5) 全量 193 passed 0 failed 5 ignored（Rust）+ 147 passed（E2E）+ clippy 零警告 + fmt 通过 | QA Lead / TPM |
| v2.2.0 | 2026-07-30 | **第二批 P1 剩余功能——Agentic RAG + 领域画像**：(1) 新增 REQ-VEC-013（领域画像自动分类）：16 领域预定义，嵌入质心分类方案（零 LLM 成本、离线可用），Document.domain 字段 + DomainClassifier trait + EmbeddingDomainClassifier 实现 + reclassify_document IPC 命令；(2) 新增 REQ-RAG-022（Agentic RAG 多步推理）：ReAct 范式（论文 arXiv:2210.03629），Thought→Action→Observation 循环，最大 5 次迭代，agent_step 事件推送思考过程，set_agent_enabled 运行时开关；(3) §6.5 Out of Scope 移除「Agent/工具调用」和「自动文档分类/标签」，§6.7 添加移除记录 | PM / TPM |
| v2.3.0 | 2026-07-30 | **文档同步：安全防御域 + 状态更新 + i18n + IPC 命令同步**：(1) REQ-VEC-013 状态 In-Dev→Done（领域画像 7 测试全绿 tc_domain_001~007）；(2) REQ-RAG-022 状态 In-Dev→Done（Agentic RAG 8 测试全绿 tc_agent_001~008，312 passed 0 failed 5 ignored clippy 零警告）；(3) REQ-I18N-001 状态 Draft→Done（i18n 基础设施 + 双语语言包 + 语言切换 UI + get_locale/set_locale IPC 命令，描述更新反映完整实现）；(4) 新增 SEC-013~020 八条安全防御需求：SEC-013 SQLCipher AES-256 加密 + SEC-014 PII 检测与脱敏（8 类型）+ SEC-015 审计日志哈希链 + SEC-016 自动锁定 + SEC-017 暴力破解防护 + SEC-018 剪贴板自动清除 + SEC-019 紧急销毁 Panic Wipe + SEC-020 密码强度评估；(5) IPC 命令总数同步 28→54（docs/api/ipc-commands.md 更新），新增 agent_step / chat_context_truncated 事件 | TPM |
| v2.4.0 | 2026-08-02 | **极致性能优化 Session 1-2**：(1) REQ-PERF-001（语义缓存金字塔）：L0 精确匹配 + L1 语义匹配 + L3 检索结果三级缓存，SqliteCache 共享 SqliteStorage 连接池，4 个 IPC 命令（get_cache_stats/clear_cache/set_cache_settings/get_cache_settings），17 个测试（12 单元 + 5 集成），SRS 状态 Implemented；(2) **新增 REQ-PERF-002（Prompt 压缩）**：LLMLingua-2 式检索片段 token 级压缩，两层策略——Free 版 RuleBasedCompressor（停用词去除 + Markdown 装饰去除 + 代码注释去除 + 句子级信息密度评分，零依赖）+ Pro 版 OnnxCompressor（嵌入余弦相似度评分，复用 LocalEmbedder），ChatEngine 集成 with_compressor() builder，2 个 IPC 命令（set_compression_ratio/get_compression_ratio），11 个测试 TC-COMP-001~010 + trait 验证，Free 546/Pro 808 passed 0 failed，clippy 零警告 | PM / TPM |
| v2.5.0 | 2026-08-03 | **极致性能优化 Session 3-4（Sprint A 完成 + Sprint B 开始）**：(1) **新增 REQ-PERF-003（CRAG 检索精炼）**：Correct/Ambiguous/Incorrect 三级置信度评估器 + decompose-then-recompose 句子级过滤 + Self-RAG 查询分类器（规则匹配，零 LLM），chat_inner 集成三级精炼管线，20 个测试通过；(2) **新增 REQ-PERF-004（Self-RAG 检索决策）**：查询分类决定是否触发检索（Simple/Analytical/Comparison/Factual），跳过不必要的检索减少 20-30% token 消耗；(3) **新增 REQ-PERF-005（Contextual BM25）**：FTS5 索引使用 `build_contextual_text()` 拼接文档名前缀（Anthropic Contextual Retrieval，检索失败率 ↓49%），`add_chunk`/`add_chunks_batch` 写入上下文前缀，`rebuild_bm25_index` IPC 命令供用户手动触发索引重建，FTS5 回填使用上下文前缀；(4) **新增 REQ-PERF-006（实体链接增强）**：NER 实体抽取器（纯规则+正则，零 LLM/模型依赖），5 类实体（中文人名/英文专有名词/技术术语/数字标识符/日期），entities 表 + entity_search() 精确匹配，三路 RRF 融合（Vector=1.0 + BM25=0.8 + Entity=0.6），导入时自动抽取实体并索引，12 个新测试（TC-ENT-001~008 + TC-3RRF-001~004）通过，全量 558 passed 0 failed，clippy 零警告 | PM / TPM |
| v2.6.0 | 2026-08-03 | **极致性能优化 Session 5（Proposition 级原子分割）**：**新增 REQ-PERF-007（Proposition 级原子分割）**：基于 Dense X Retrieval 论文（arXiv:2312.06648），将 chunk 分解为自包含的原子事实（proposition），proposition 级检索精度显著优于 chunk 级（预期命中率 +30-50%）。纯规则方案（零 LLM 调用）：(1) `PropositionSplitter` — 句子分割（中英文标点 + 段落）+ 代词消解（中文他/她/它 + 英文 he/she/it/they，使用前一句实体，排除「其他」等组合）+ 上下文补全（连词开头补全主语前缀，中文 10 个 + 英文 10 个连词）；(2) `Proposition` 数据模型（id/chunk_id/content/sequence）；(3) Storage trait 新增 5 个方法（add_propositions/add_proposition_embeddings/list_propositions_by_doc/proposition_search/rebuild_proposition_index）；(4) `propositions` 表（id/chunk_id/content/embedding BLOB/sequence + ON DELETE CASCADE）；(5) `proposition_search_and_expand()` 检索函数（proposition 向量检索 → chunk 去重 → expand_neighbors 相邻扩展）；(6) 导入管线 `index_propositions()` 覆盖常规 + Markdown mmap + Text mmap 三路径；(7) `rebuild_proposition_index` IPC 命令供旧库升级。10 个测试通过（TC-PROP-001~008b），全量 558 passed 0 failed 25 ignored，clippy 零警告 | PM / TPM |
| v2.7.0 | 2026-08-03 | **极致性能优化 Session 6（ColBERT 多向量嵌入 + RAPTOR 摘要树）**：(1) **新增 REQ-PERF-008（ColBERT 多向量嵌入, Pro feature）**：基于 ColBERT 论文（arXiv:2004.12832）Late Interaction 模型，为文本中每个 token 生成独立嵌入向量（而非整个 chunk 一个向量），检索时使用 MaxSim 交互计算（query 每个 token 在 document 所有 token 向量中找最大余弦相似度，求和）。新增 `MultiVectorEmbedder` trait（core 层）+ `ColbertEmbedder` 适配器（infra 层，Pro 门控，复用 LocalEmbedder ONNX 引擎）+ `maxsim()`/`cosine_sim()` 核心算法函数 + 多向量 BLOB 序列化（`multivec_to_bytes`/`multivec_from_bytes`）。6 个测试通过（TC-COL-001~008），精确匹配场景预期命中率 +20-30%。(2) **新增 REQ-PERF-009（RAPTOR 多级摘要树索引）**：基于 RAPTOR 论文（Recursive Abstractive Tree for Retrieval-Ordered Access），将原始 chunks 组织为多级摘要树（Level 0 组摘要 → Level 1 主题摘要），提供从局部事实到全局主题的多层次检索能力。新增 `SummaryTreeBuilder`（core 层，支持 LLM 摘要生成回调 + 增量构建）+ `SummaryNode` 数据模型 + `classify_query_level()` 查询路由器（规则匹配，零 LLM）+ `expand_summary_to_chunks()` 摘要展开 + `merge_summary_and_chunk_results()` 结果合并。Storage trait 新增 5 个方法（add_summary_nodes/update_summary_embedding/list_summary_nodes/summary_search/rebuild_summary_tree）。`summary_nodes` 表（id/doc_id/level/content/child_ids/embedding BLOB + ON DELETE CASCADE）。2 个 IPC 命令（build_summary_tree + set_embedder_model）。8 个测试通过（TC-RAP-001~008）。 | PM / TPM |
| v2.8.0 | 2026-08-04 | **极致性能优化 Session 7（渐进式上下文注入, Sprint C 开始）**：**新增 REQ-PERF-010（渐进式上下文注入）**：按需注入 chunk，LLM 上下文足够时停止追加。`ProgressiveInjector` 状态机（initial_count=2 可配置 + max_rounds=top_k 防无限追加 + `initial_indices()`/`needs_more_info()`/`can_append()`/`next_batch()` 方法）；`detect_insufficient_info()` 独立函数（15 个中文关键词 + 12 个英文关键词，大小写不敏感，供 `forward_stream` 调用）；`split_sources()` 分割辅助 + `ProgressiveStats` 统计（avg_injected/append_rate）；`ProgressiveInfo` 元数据（total_sources/injected_count/can_expand，由 `ChatOutcome::Answered` 携带）；`ChatEngine.with_progressive(initial_count)` builder；`chat_inner` 读取 `rag.progressive_injection` 设置启用渐进注入 + 日志输出。预期平均注入 chunk 数 5→2-3，token ↓40-60%。与 Prompt 压缩（S2）兼容：压缩全部 source 后仅注入初始子集。13 个测试通过（TC-PROG-001~006b）。 | PM / TPM |
| v2.9.0 | 2026-08-04 | **极致性能优化 Session 8（Speculative RAG）**：**新增 REQ-PERF-011（Speculative RAG）**：小模型快速生成草稿 → 大模型验证/修正，预期首 token 延迟 ↓40-60%。基于 vLLM Speculative Decoding 概念（arXiv:2309.06180）自研适配 RAG 场景：草稿用简化 prompt（仅 top-1 chunk），验证用完整 prompt + 草稿作为 prefix。`SpeculativeRagEngine<D, L>` 泛型引擎（D=草稿模型, L=验证模型，均实现 `LLMProvider`）；`SpeculativeRagConfig`（draft_top_k=1 / min_draft_length=20 / accept_threshold=0.85）；`SpeculativeOutcome` 三态枚举（DraftAccepted / DraftCorrected / FallbackDirect）；`text_similarity()` 基于归一化 Levenshtein 编辑距离的相似度计算；`SpeculativeStats` 统计（accept_rate/fallback_rate）；`chat_inner` 读取 `rag.speculative_enabled` 设置启用 Speculative RAG + 日志输出。与渐进注入（S7）兼容：草稿用 top-1 chunk（渐进注入的初始子集），验证用全部 chunks。12 个测试通过（TC-SPEC-001~006c）。Free 599/Pro 863 passed 0 failed，clippy 零警告。 | PM / TPM |
| v3.0.0 | 2026-08-04 | **极致性能优化 Session 9（自进化检索记忆）**：**新增 REQ-PERF-012（自进化检索记忆）**：借鉴 AutoRAG 2.0 自进化记忆系统，系统自动记录每种查询类型 × 检索方法的累计效果统计（hit_count/miss_count/avg_score），自适应选择命中率最高的检索策略，不增加延迟（记忆查表 ~1ms）。`QueryType` 四分类（Factual/Analytical/Code/Conversational，规则匹配零 LLM）；`RetrievalMethod` 五方法（vector_only/hybrid/hybrid_rerank/proposition/colbert）；`RetrievalMemoryEngine` 自适应引擎（`record_retrieval()` 记录效果 + `select_method()` 选择最佳方法，hit_rate 优先 + avg_score 平局打破）；`RetrievalMemoryStore` trait（对象安全，`get_memory`/`upsert_memory`/`list_memories`/`list_all_memories`/`clear_all_memories`）；`retrieval_memory` 表（query_type/method/hit_count/miss_count/avg_score + PRIMARY KEY(query_type, method)）；`chat_inner` 读取 `rag.retrieval_memory_enabled` 设置启用自适应检索 + 日志输出；3 个 IPC 命令（set_retrieval_memory_enabled/get_retrieval_memory_stats/reset_retrieval_memory）。冷启动使用默认方法（hybrid），随使用自动优化。10 个测试通过（TC-MEM-001~006 + enum_roundtrip + default_for_each_type + record_updates_correctly）。Free 612/Pro 881 passed 0 failed，clippy 零警告。 | PM / TPM |
| v3.1.0 | 2026-08-04 | **极致性能优化 Session 10（文档同步 + 性能基准对比, 最终 Session）**：(1) **新增 §4.26 PERF 域**：REQ-PERF-001~012 全部状态 Implemented，包含完整验收标准 + 契约 + 关联测试；(2) **新增 ADR-012**（`docs/architecture/adr/ADR-012-extreme-perf.md`）：极致性能优化架构决策记录，含 4 Sprint / 10 Session 完成状态 + 性能基准对比 + 风险与缓解 + 向后兼容性；(3) **新增 5 个性能基准测试**（`crates/core/src/benchmark_tests.rs`）：TC-BENCH-001 Token 消耗对比 + TC-BENCH-002 检索命中率对比 + TC-BENCH-003 首 token 延迟对比 + TC-BENCH-004 缓存命中率统计 + TC-BENCH-005 总延迟对比；(4) 全量文档同步：ipc-commands.md（14 个新命令）+ code-contracts.md（全部 trait/struct 签名）+ settings-reference.md（9 个新设置键）+ frontend-modules.md（前端函数）+ AGENTS.md + CLAUDE.md。极致性能优化全部完成：4 Sprint / 10 Session / 12 个 REQ-PERF / 14 个 IPC 命令 / 9 个设置键 / 3 张数据表 / 130+ 个测试。预期效果：Token ↓50-75%，检索命中率 +20-30pp，首 token 延迟 ↓50-60%，总延迟 ↓50-70%。 | PM / TPM |
| v3.2.0 | 2026-08-04 | **DeepSeek 风格 UI 对齐（4 Session）**：(1) **新增 REQ-UI-010**：借鉴 DeepSeek 聊天界面设计风格，视觉与交互全面对齐——用户消息气泡 22px 大圆角 + AI 消息透明背景 + AI 免责声明 + CJK 引号修复（PingFangSC + unicode-range）+ 焦点环 box-shadow + 消息列表 840px 最大宽度 + 可折叠思维链面板（thinking-panel.js）+ 输入区快速 Toggle（input-toggles.js）+ 新对话 ⌘ J 快捷键 + CSS 变量三层架构（static→alias→component）+ 主题切换过渡禁用；(2) 14 个单元测试（Vitest，TC-DS-001~012）+ 8 个 E2E 测试（Playwright，TC-DS-013~020）；(3) 4 个 Session 完成：S1 视觉基础对齐 + S2 思维链折叠 + S3 输入区 Toggle + S4 设计系统增强 | PM / TPM |
| v3.3.0 | 2026-08-05 | **RAG 质量门控系统**：**新增 REQ-RAG-028**：借鉴 StoryMoss 加权评分门控系统，检索后评估结果质量。三维加权评分：检索覆盖率（top-1 分数归一化）×0.4 + 来源多样性（不同文档占比）×0.3 + 分数方差（top-k 标准差）×0.3，低于阈值 0.6 时记录告警日志。`GateConfig` / `DegradationStrategy` / `GateScore` 结构体；`evaluate()` 评估函数；`ChatEngine::with_quality_gate()` builder 方法；`ChatOutcome::Answered` 新增 `gate_score` 字段；`SettingsPayload` 新增 `quality_gate_enabled` 字段；`AppState` 新增 `quality_gate_enabled` 字段；`rag.quality_gate_enabled` 设置键。当前版本 PassThrough 策略（仅评估+日志），降级策略（ExpandTopK/WarnUser）预留。7 个测试通过（TC-GATE-001~007）。Free/Pro clippy 零警告。 | PM / TPM |
| v3.4.0 | 2026-08-05 | **检索质量自学习引擎（REQ-PERF-012 扩展）**：借鉴 StoryMoss 持续学习机制，通过观察用户行为自动蒸馏检索效果，无需显式标注。`FeedbackType` 五种反馈信号（RetryWithDifferentMethod/EditAndResend/Accepted/ThumbsUp/ThumbsDown）+ `FeedbackSignal` 结构体 + `record_feedback()` 方法（delta 调整 hit_count/miss_count/avg_score，clamp 0.0-1.0 边界保护）。新增 IPC `record_retrieval_feedback` 命令。前端 `retrieval-feedback.js` 模块（隐式行为检测：Jaccard 词重叠率相似度 + 编辑重发检测 + 新话题检测）+ `feedback.js` 集成显式 👍/👎 上报。7 个测试通过（TC-FEEDBACK-001~007）。Free 653/Pro 888+1flaky passed，clippy 零警告，fmt 通过。 | PM / TPM |
| v3.5.0 | 2026-08-05 | **Agent 生命周期 Hooks 系统**：**新增 REQ-RAG-029**：借鉴 Bamboo-agent 生命周期 hooks 设计，在 `AgentEngine`（ReAct 循环）和 `CoordinatorEngine`（四阶段管线）的关键执行节点插入可扩展的 hook 点。4 个 Hook 阶段（BeforeRetrieval/AfterRetrieval/BeforeGeneration/AfterGeneration）；`HookPhase` 枚举 + `HookContext` 可变上下文（query/retrieval_results/output/metadata）+ `AgentHook` trait（`Pin<Box<Future>>` 返回类型保证对象安全，与 Reranker/QueryRewriter 一致）+ `HookRegistry` 注册表（按注册顺序执行，Err 中断冒泡）；`AgentEngine::with_hooks()` / `CoordinatorEngine::with_hooks()` builder 方法；`AppState.hook_registry` 字段。Agent 4 处调用点（BeforeRetrieval 改写查询 + AfterRetrieval 过滤结果 + BeforeGeneration 增强上下文 + AfterGeneration 记录），Coordinator 4 处调用点（Phase 1 前/后 + Phase 3 前/后）。不注册 hook 时零开销向后兼容。7 个测试通过（TC-HOOK-001~007）。Free/Pro clippy 零警告。 | PM / TPM |
| v3.6.0 | 2026-08-05 | **DAG 工作流引擎**：**新增 REQ-RAG-030**：借鉴 IfAI 的 DAG 工作流引擎（WorkflowNode/WorkflowEdge 架构），用户定义由节点和边组成的有向无环图，引擎负责拓扑排序（Kahn's algorithm）、并行执行同层独立节点（`join_all`）、串行执行跨层依赖节点。5 种节点类型：`Retrieval`（知识库检索）/ `Generation`（LLM 生成，`{input}` 占位符替换）/ `Condition`（条件分支，`contains:` / `length>N` / `nonempty` / `true` 表达式）/ `Aggregate`（多输入聚合，`concat` / `summarize` / `best_of` 策略）/ `Output`（工作流终点）。Condition 节点通过边 `mapping` 字段选择下游分支，未激活分支标记 `Skipped`。节点失败时下游传递闭包标记 `Skipped`（不崩溃）。`WorkflowEngine<R: Retriever, L: LLMProvider>` 结构体；`Workflow` / `WorkflowNode` / `NodeType`（`#[serde(tag = "kind")]` 标签枚举）/ `WorkflowEdge` / `NodeStatus` / `WorkflowResult` 数据模型（`crates/models`）；4 个 IPC 命令（`save_workflow_template` / `run_workflow` / `list_workflows` / `delete_workflow`）；工作流模板以 JSON 存储在 settings 表（`workflow.{id}` + `workflow.index`）。9 个测试通过（TC-WF-001~009）。Free/Pro clippy 零警告，fmt 通过。 | PM / TPM |
| v3.7.0 | 2026-08-06 | **代码符号引擎**：**新增 REQ-RAG-031**：借鉴 IfAI SymbolEngine，使用 tree-sitter 对代码文件（.rs/.ts/.tsx/.py/.go）进行 AST 级符号分析，建立「符号 → 位置 → chunk」三级映射。`SymbolExtractor` trait（core）+ `SymbolEngine` 实现（infra，Pro 门控）；`CodeSymbol` 结构体 + `SymbolKind` 枚举（8 种：Function/Method/Class/Struct/Interface/Enum/Constant/Module）；`code_symbols` 数据库表（3 索引）；Storage trait 4 个方法（add_symbols/search_by_symbol/get_symbols_for_chunk/search_symbols_fuzzy）；`ImportService.index_document_with_symbols()` 符号感知分块（按函数/类边界分块，超大函数内部按行分割）；`symbol_search()` 检索函数（精确匹配 score=1.0 + 模糊匹配 score=0.8）；3 个 IPC 命令（`search_symbols` / `get_symbols_for_chunk` / `rebuild_symbol_index`，Pro 门控）。Free 模式代码文件按纯文本加载（向后兼容）。11 个测试通过（TC-SYMBOL-001~011）。Free 555+Pro 372+11 symbol tests passed，clippy 零警告，fmt 通过。 | PM / TPM |
| v3.8.0 | 2026-08-06 | **代码执行沙箱**：**新增 REQ-RAG-032**：借鉴 CodeForge 多语言执行器，在安全沙箱中执行代码片段（Python / JavaScript）。`CodeExecutor` trait（execute/supported_languages/config，`Pin<Box<Future>>` 返回类型保证对象安全）+ `MockExecutor`（测试用）+ `NoExecutor`（Free 版本占位）+ `WasmExecutor`（infra，Pro 门控，本地进程执行 + 安全限制：超时 10s / 内存 64MB / 无网络 / 临时目录隔离 / 环境变量清空）；`ExecutionResult` 结构体（stdout/stderr/exit_code/duration_ms/timed_out）+ `CodeExecutorConfig` 结构体（timeout_secs/memory_limit_mb/allow_network）+ `format_execution_result()` 函数；`AgentEngine::with_executor()` builder 方法 + `execute_code` Agent 工具（JSON 输入：`{"language":"python","code":"...","stdin":"..."}`）；1 个 IPC 命令（`execute_code_snippet`，Pro 门控）+ 前端 `code-executor.js` 模块（代码执行面板 + 语言选择 + 结果展示 + Focus Trap）。19 个测试通过（TC-EXEC-001~011 + TC-EXEC-INT-001~008，5 个 ignored 需 python3/node）。Free/Pro clippy 零警告，fmt 通过。 | PM / TPM |
| v3.9.0 | 2026-08-06 | **持久化记忆系统**：**新增 REQ-RAG-033**：借鉴 IfAI 三层记忆（Wing/Hall/Room 空间隐喻）+ Bamboo-agent 上下文压缩，实现跨会话知识沉淀。`MemoryStore<S: Storage>` 结构体（extract_from_conversation LLM 辅助提取 / promote 层级提升 / forget 低分遗忘 / retrieve_relevant 查询注入 / consolidate 自动整合 / pin_memory 用户置顶）；`MemoryRetriever` trait（对象安全 `Pin<Box<Future>>`，ChatEngine 注入用）；`MemoryEntry` / `MemoryTier`（Wing/Hall/Room）/ `MemorySource`（4 种）/ `ConsolidationResult` 数据模型（models）；`memory_entries` 数据库表（9 列 + 2 索引）+ Storage trait 6 个方法；`ChatEngine::with_memory()` builder + system prompt [相关记忆] 块注入；`DreamEngine` Phase 4 记忆整合（Wing ≥0.6→Hall, Hall ≥0.7→Room, <0.3→遗忘, Room 超限→LRU 遗忘）；`AppState.memory_enabled` 字段 + `memory.enabled` 设置键。6 个 IPC 命令（set_memory_enabled / get_memories / pin_memory / promote_memory / delete_memory / clear_memories）。12 个测试通过（TC-MEMORY-001~012）。Free 432+125+121 passed，clippy 零警告，fmt 通过。 | PM / TPM |
| v4.0.0 | 2026-08-06 | **v1.1 版本规划 — 需求调研 + 原子计划制定 + SRS 扩展**：(1) **ROI 评估**：对 12 项 v1.0 范围外功能进行用户需求度 × 技术可行性 / 预计天数三维评分，选出 Top 5：(2) **新增 6 条 Draft 需求**：REQ-ING-015（.docx 导入，docx-rs v0.4.22）、REQ-ING-016（.html 导入，html2text v0.17.1）、REQ-UI-011（浅色主题，CSS 变量覆盖）、REQ-RAG-034（语音输入，Web Speech API）、REQ-RAG-035（TTS 朗读，speechSynthesis）、REQ-RAG-036（网页搜索集成，DuckDuckGo Instant Answer API）；(3) **原子计划**：`docs/architecture/V1.1_ATOMIC_PLAN.md`（S44-S48，5 个 Session，预计 4.5 天）；(4) **§6 更新**：将 .docx/.html/浅色主题/语音输入/TTS/网页搜索从「范围外」移至「v1.1 实施计划」；(5) **铁律六**：cargo search 确认 docx-rs 0.4.22（非任务描述 0.1）+ html2text 0.17.1（非 0.13）；(6) README v1.1 Roadmap 更新（中英文）。无源代码变更（纯规划文档）。 | PM / TPM |
| v4.1.0 | 2026-08-06 | **v1.1 S44 — .docx 文档导入实现（REQ-ING-015 Draft → Implemented）**：(1) **新增依赖** `docx-rs = "0.4"` (v0.4.22, default-features = false)；(2) **DocxLoader** 实现 `Loader` trait：`read_docx()` 解析 Office Open XML ZIP → 遍历 `Document.children` → 段落提取文本（Run/Hyperlink/Insert/MoveTo 递归）+ 图片 `[图片]` 占位符 + 表格转 Markdown 语法；(3) **import.rs** `ALLOWED_EXTENSIONS` 新增 `"docx"`（8→9）+ `index_inner` 新增 docx 分支 + `index_multimodal_inner` 新增 docx 分支；(4) **TDD 4 个测试** TC-ING-DOCX-001~004（简单段落/表格/标题层级/损坏文件）；(5) 前端文件对话框 + i18n + tauri-stub 同步更新；(6) 验证：core Free 436 + Pro 457 passed, clippy 零警告, fmt 通过, build-ui 成功, tsc 0 errors。 | Dev |
| v4.3.0 | 2026-08-06 | **v1.1 S46 — 浅色主题切换实现（REQ-UI-011 Draft → Implemented）**：(1) **tokens.css** 新增 `[data-theme="light"]` 全部颜色变量覆盖（~40 个变量：Surface/Border/Text/Accent/Semantic/Bg 别名/消息组件令牌）+ `[data-theme="system"]` @media (prefers-color-scheme: light) 自动跟随系统 + color-scheme: light + 浅色滚动条样式；(2) **main.js** 新增 `applyTheme(theme)` / `setTheme(theme)` / `initTheme()` 函数：双 rAF 过渡禁用防闪烁 + Mermaid 主题同步 + prefers-color-scheme 变化监听器（system 模式自动切换）+ 后端持久化降级 localStorage；(3) **settings.js** 新增 `initThemeSwitcher()` 函数 + 「外观」区域 UI（暗色/浅色/跟随系统三选一分段控件 + SVG 图标）；(4) **commands.rs** 新增 `set_theme` / `get_theme` IPC + `*_inner`（复用 get_locale/set_locale 模式，settings 表键 `ui.theme`）；(5) **lib.rs** 注册 2 个新 IPC 命令；(6) **i18n** 新增 theme_section/theme_dark/theme_light/theme_system 文案（中英文）；(7) **tauri-stub.js** 新增 set_theme/get_theme mock；(8) **theme.spec.ts** 5 个 E2E 测试 TC-UI-LIGHT-001~005。 | Dev |
| v4.4.0 | 2026-08-06 | **v1.1 S47 — 语音输入 + TTS 朗读实现（REQ-RAG-034/035 Draft → Implemented）**：(1) **新增模块** `ui/src/voice-input.js`：SpeechRecognition 麦克风转写（`initVoiceInput` / `stopRecording`）+ speechSynthesis TTS 朗读（`createTtsButton` / `stopAllTTS` / `getVoiceRate`），Web Speech API 不支持时优雅降级隐藏入口；(2) **index.html** 新增 `#micBtn` 麦克风按钮（#plusBtn 与 #sendBtn 之间）+ 设置面板「语音」区域（TTS 语速滑块 0.5~2.0）；(3) **chat-render.js** `showMsgActions()` 在 AI 消息操作栏中插入 `.tts-btn` 朗读按钮（重新生成按钮之前）；(4) **settings.js** 新增 `initVoiceSettings()` 函数（语速滑块 + localStorage `voice.rate` 持久化）；(5) **main.js** boot 中调用 `initVoiceInput()` + newChat 中调用 `stopAllTTS()`；(6) **components.css** 新增 `@keyframes micPulse` 脉冲动画 + `#micBtn.recording` 红色脉冲状态 + `.tts-btn.speaking` accent 高亮；(7) **dom-ext.d.ts** 新增 SpeechRecognition / SpeechSynthesis 类型声明（Window 可选属性）；(8) **i18n** 新增 voice.title/language/rate/start_recording/stop_recording/listen/stop_listening/not_supported/tts_not_supported/permission_denied 文案（中英文）；(9) **tauri-stub.js** 新增 Web Speech API mock（SpeechRecognition / speechSynthesis / SpeechSynthesisUtterance）；(10) **voice.spec.ts** 6 个 E2E 测试 TC-UI-VOICE-001~006。验证：tsc 0 errors, build-ui 成功（11134 行），6 E2E passed, clippy 零警告, fmt 通过。 | Dev |
| v4.5.0 | 2026-08-06 | **v1.1 S48 — 网页搜索集成实现（REQ-RAG-036 Draft → Implemented）**：(1) **models** 新增 `SearchResult` 结构体（title/url/snippet/source）；(2) **core/lib.rs** 新增 `WebSearchProvider` trait（`search()` 方法，`Pin<Box<Future>>` 对象安全，Send+Sync）+ `web_search` 模块；(3) **core/web_search.rs** 融合引擎：`should_search()`（阈值 0.3 判断）+ `convert_to_retrieval_results()`（🌐 前缀标注 Web 来源）+ `rrf_fuse_with_web()`（本地权重 1.0 / Web 权重 0.4 RRF 融合）+ `search_and_fuse()`（主入口，失败优雅降级）；(4) **core/web_search_tests.rs** 6 个 TDD 测试 TC-RAG-SEARCH-001~006；(5) **chat.rs** `ChatEngine` 新增 `web_search_provider: Option<Arc<dyn WebSearchProvider>>` + `with_web_search()` builder + `chat()` 中检索后阈值判断→搜索→融合；(6) **infra/duckduckgo_provider.rs** `DuckDuckGoProvider` 实现 `WebSearchProvider`：DuckDuckGo Instant Answer API（`https://api.duckduckgo.com`，连接超时 5s/请求超时 10s）+ 解析 `AbstractText` + `RelatedTopics`（含嵌套 Topics）+ HTTP 非 2xx 优雅降级 + 3 个测试 TC-DDG-001~003；(7) **tauri-app** `AppState.web_search_enabled` + `chat_inner` 集成（`get_settings_batch` 读取→条件判断→`DuckDuckGoProvider::new()`→`search_and_fuse()`）+ 2 个 IPC 命令（`set_web_search_enabled` / `web_search`）；(8) **前端** input-toggles.js 新增 `web_search` toggle（地球 SVG 图标）+ i18n 中英文文案 + tauri-stub.js mock + web-search.spec.ts 5 个 E2E 测试。验证：6 Rust + 3 infra 测试通过，clippy 零警告。 | Dev |
| v5.0.0 | 2026-08-06 | **v1.1.0 发布 — 版本号升级 + 全量回归验证**：(1) **版本号升级** 1.0.0 → 1.1.0（7 crate Cargo.toml + tauri.conf.json）；(2) **全量回归测试**：Rust Free 749 passed / Rust Pro 1034 passed / Clippy Free+Pro 零警告 / cargo fmt 通过 / Vitest 407 passed / TypeScript 0 errors / esbuild 11142 行 / E2E 999 passed；(3) **CHANGELOG_v1.1.md** 发布说明（5 项新功能 + 技术规格 + 向后兼容性）；(4) **v1.1 发布检查清单**（release-checklist 更新）；(5) v1.1 全部 5 个功能（S44-S48）实现完毕：.docx 导入 / .html 导入 / 浅色主题 / 语音输入+TTS / 网页搜索集成。 | PM / Dev |
| v5.2.0 | 2026-08-06 | **v1.2 S49 — PDF 导出实现（REQ-EXP-005 Draft → Implemented）**：(1) **新增模块** `ui/src/export.js`：`exportConversationToPdf(conversationId, title)` / `exportDocumentToPdf(docId)` / `buildPrintHtml(title, contentBlocks)` / `initExportButtons()` / `getPdfPageSize()` / `setPdfPageSize()` / `getPdfIncludeSources()` / `setPdfIncludeSources()`——复用 Markdown 渲染管线（marked.js + DOMPurify + highlight.js）构建打印 HTML → 隐藏 iframe 写入 → `iframe.contentWindow.print()` 触发浏览器打印对话框 → 用户选择「保存为 PDF」；(2) **新增样式** `ui/styles/print.css`：`@media print` 规则（隐藏 #sidebar/#inputBar/#settingsPanel 等非打印元素 + serif 字体 12pt + page-break-inside:avoid 分页控制）；(3) **index.html** 新增 `#exportPdfBtn` 按钮（侧栏工具栏）+ 设置面板 PDF 导出区域（页面大小 A4/Letter 选择 + 包含引用来源 toggle）；(4) **settings.js** 新增 `initPdfExportSettings()` 函数（localStorage 持久化 `export.pdf_page_size` / `export.pdf_include_sources`）；(5) **main.js** boot() 调用 `initExportButtons()`；(6) **i18n** 新增 export_pdf 文案（中英文 19 键）；(7) **tauri-stub.js** 新增 `window.print` mock + `get_document_chunks` mock；(8) **dom-ext.d.ts** 新增 Window 属性声明（`__printMockCalled` / `__lastPrintHtml` / `exportConversationToPdf` / `exportDocumentToPdf`）；(9) **export-pdf.spec.ts** 5 个 E2E 测试 TC-EXP-PDF-001~005。验证：tsc 0 errors, esbuild 11566 行, E2E 5/5 passed, clippy 零警告, fmt 通过。零新增 Rust 依赖，零新增 IPC 命令。 | Dev |
| v5.1.0 | 2026-08-06 | **v1.2 版本规划 — ROI 评估 + 技术调研 + 原子计划制定 + SRS 扩展**：(1) **ROI 评估**：对 9 项候选功能进行用户需求度 × 技术可行性 / 预计天数三维评分，选出 Top 4（ROI ≥ 10 的 3 项 + Pro 战略价值 1 项）；(2) **新增 4 条 Draft 需求**：REQ-EXP-005（PDF 导出，`window.print()` 零依赖）、REQ-ING-017（.pptx 导入，`pptx-to-md` v1.0.0）、REQ-ING-018（.epub 导入，`rbook` v0.7.10 Apache-2.0）、REQ-VEC-014（自定义 ONNX 嵌入模型上传，Pro 门控）；(3) **原子计划**：`docs/architecture/V1.2_ATOMIC_PLAN.md`（S49-S53，5 个 Session，预计 5 天）；(4) **§6 更新**：将 .pptx/.epub/自定义模型/PDF 导出从「范围外」移至「v1.2 实施计划」；(5) **铁律六**：cargo search 确认 pptx-to-md 1.0.0 + rbook 0.7.10（排除 epub crate GPL-3.0 不兼容）；(6) README v1.2 Roadmap 更新（中英文）。无源代码变更（纯规划文档）。 | PM / TPM |
| v5.3.0 | 2026-08-06 | **v1.2 S50 — .pptx 演示文稿导入实现（REQ-ING-017 Draft → Implemented）**：(1) **Cargo.toml** 新增 `pptx-to-md = "1.0"`（纯 Rust，MIT/Apache-2.0）；(2) **loader.rs** 新增 `PptxLoader` struct + `Loader` impl：`PptxContainer::open(path, config)` → `convert_to_md()` → 后处理压缩换行；配置策略 `extract_images(false)` + `include_speaker_notes(true)` + `include_slide_number_as_comment(false)` + `include_presentation_metadata(false)`；(3) **loader_tests.rs** 新增 TC-ING-PPTX-001~004 测试（使用 `tests/fixtures/test_{simple,table,image,corrupt}.pptx` fixture 文件，`env!("CARGO_MANIFEST_DIR")` 路径定位）；(4) **import.rs** `ALLOWED_EXTENSIONS` 11→12（新增 `"pptx"`）+ `index_inner()` 新增 `"pptx" =>` 分支 + `index_multimodal_inner()` 新增 `"pptx" =>` 分支；(5) **i18n** en.json + zh-CN.json 3 处格式列表加 `.pptx`（empty_state_formats / empty_formats / drag_hint）；(6) **tauri-stub.js** WHITELIST 新增 `'pptx'` + 错误消息加 `.pptx`；(7) **前端文件对话框** wizard.js / chat-render.js / import.js extensions 数组新增 `'pptx'`。验证：4 TDD 测试通过，clippy 零警告，fmt 通过。 | Dev |
| v5.4.0 | 2026-08-07 | **v1.2 S51 — .epub 电子书导入实现（REQ-ING-018 Draft → Implemented）**：(1) **Cargo.toml** 新增 `rbook = { version = "0.7", default-features = false, features = ["threadsafe", "prelude"] }`（纯 Rust，Apache-2.0，EPUB 2/3）；(2) **loader.rs** 新增 `EpubLoader` struct + `Loader` impl：`Epub::open(path)` → `epub.spine().iter()` 遍历 spine 条目 → `manifest_entry.read_str()` 读 XHTML → `html2text::from_read()` 转纯文本 → 章节间 `---` 分隔 + 压缩换行；(3) **loader_tests.rs** 新增 TC-ING-EPUB-001~004 测试（`tests/fixtures/test_{simple,multi,html,corrupt}.epub` fixture，Python stdlib zipfile 生成）；(4) **import.rs** `ALLOWED_EXTENSIONS` 12→13（新增 `"epub"`）+ `index_inner()` + `index_multimodal_inner()` 新增 `"epub" =>` 分支；(5) **i18n** en.json + zh-CN.json 3 处格式列表加 `.epub`；(6) **tauri-stub.js** WHITELIST + 错误消息加 `.epub`；(7) **前端文件对话框** wizard.js / chat-render.js / import.js extensions 数组新增 `'epub'`。验证：4 TDD 测试通过，clippy 零警告，fmt 通过，esbuild 11566 行，tsc 0 errors。 | Dev |
| v5.5.0 | 2026-08-07 | **v1.2 S52 — 自定义 ONNX 嵌入模型上传实现（REQ-VEC-014 Draft → Implemented，Pro 门控）**：(1) **local_embedder.rs** 新增 `EmbeddingModel::Custom(String)` 枚举变体 + `new_with_custom_model()` / `init_custom()` 加载用户 ONNX 模型（fastembed `UserDefinedEmbeddingModel`，首次推理检测维度）+ `find_onnx_file()` / `validate_onnx_file()` / `validate_tokenizer_files()` 验证 + `list_custom_models()` / `delete_custom_model()` / `copy_custom_model_files()` 管理 + `CustomModelInfo` 结构体 + `is_custom()` 方法 + `dim()` 返回 0 for Custom；(2) **model_store.rs** `parse_embedding_model()` 解析 `"custom:{name}"` 前缀 + `load_embedding_model()` / `init_embedder_with_progress()` 走 `new_with_custom_model()` 路径；(3) **state.rs** 新增 `custom_model_dir()` 方法；(4) **commands.rs** 新增 3 个 Pro 门控 IPC 命令 + `*_inner` 函数：`upload_custom_embedding_model` / `list_custom_models` / `delete_custom_model`（Free stub 返回 `PRO_REQUIRED`）+ 路径遍历防护（名称中的 `/\\..` 替换为 `_`）；(5) **lib.rs** 注册 3 命令（138→141）；(6) **前端** settings.js 新增 `loadCustomModels()` / `onUploadCustomModel()` / `switchToCustomModel()` / `deleteCustomModel()` + ipc.js 新增 `embedModelApi.uploadCustomModel/listCustomModels/deleteCustomModel` + index.html 新增自定义模型设置区域 + i18n 17 键（中英文）+ tauri-stub.js mock 3 命令 + types/tauri.d.ts 3 window 声明；(7) **测试** 7 集成测试 TC-VEC-CUSTOM-001~007 + 8 单元测试 TC-VEC-CUSTOM-UNIT-001~008。零新增 Rust 依赖（复用 fastembed）。 | Dev |
| v5.6.0 | 2026-08-07 | **v1.2.0 发布 — 版本号升级 + 全量回归验证**：(1) **版本号升级** 1.1.0 → 1.2.0（7 crate Cargo.toml + tauri.conf.json）；(2) **全量回归测试**：Rust Free 766 passed / Rust Pro 1057 passed / Clippy Free+Pro 零警告 / cargo fmt 通过 / Vitest 407 passed / TypeScript 0 errors / esbuild 11670 行 / E2E 950 passed 9 skipped；(3) **CHANGELOG_v1.2.md** 发布说明（4 项新功能 + 技术规格 + 向后兼容性）；(4) **Release build** `cargo build --release --features pro` exit_code 0；(5) v1.2 全部 4 个功能（S49-S52）实现完毕：PDF 导出 / .pptx 导入 / .epub 导入 / 自定义 ONNX 嵌入模型上传。新增依赖 pptx-to-md v1.0.0 + rbook v0.7.10。IPC 命令 138→141（+3 Pro 门控）。ALLOWED_EXTENSIONS 11→13（+pptx+epub）。 | PM / Dev |
| v5.7.0 | 2026-08-07 | **v1.3 版本规划 — ROI 评估 + 技术调研 + 原子计划制定 + SRS 扩展**：(1) **ROI 评估**：对 15 项候选功能进行用户需求度 × 技术可行性 / 预计天数三维评分，选出 Top 5（ROI ≥ 6.0）；(2) **新增 5 条 Draft 需求**：REQ-EXP-006（知识图谱导出 GraphML/JSON-LD）、REQ-ING-019（文档摘要自动生成）、REQ-RAG-038（对话模板/快捷指令系统）、REQ-ING-020（Markdown 笔记双向链接 Obsidian 风格）、REQ-RAG-039（对话分支/版本树 ChatGPT 风格）；(3) **原子计划**：`docs/architecture/V1.3_ATOMIC_PLAN.md`（S54-S59，6 个 Session，预计 8.5 天）；(4) **§6 更新**：将知识图谱导出/文档摘要/对话模板/双向链接/对话分支树从「范围外」移至「v1.3 实施计划」；(5) **零新增依赖**：全部 5 个功能使用已有 Rust crate（serde_json / regex / pulldown-cmark）+ 前端库（D3.js v7）；(6) README v1.3 Roadmap 更新（中英文）。无源代码变更（纯规划文档）。 | PM / TPM |
| v5.7.1 | 2026-08-07 | **v1.3 S54 — 知识图谱导出实现（REQ-EXP-006 Draft → Implemented）**：(1) **graph_export.rs** 新增模块：`export_graphml()` + `export_jsonld()` 纯函数（GraphML XML 手动拼接 + XML 特殊字符转义；JSON-LD 使用 serde_json 构建 @context + @graph 数组）；(2) **lib.rs** 注册 graph_export 模块 + graph_export_tests 测试模块；(3) **commands.rs** 新增 `export_graph` IPC 命令 + `*_inner` 函数（format 参数选择 graphml/jsonld，从 list_all_relations + get_entity_types 构建实体列表）；(4) **lib.rs** 注册 1 命令（141→142）；(5) **前端** graph-viewer.js 新增 GraphML + JSON-LD 导出按钮 + `exportGraphData()` 函数（invoke export_graph → Blob → downloadBlob）；(6) **i18n** 新增 export_graphml / export_jsonld 键（中英文）；(7) **tauri-stub.js** 新增 export_graph mock；(8) **测试** 3 单元测试 TC-EXP-GRAPH-001~003 + 2 E2E 测试 TC-EXP-GRAPH-004~005。零新增依赖。 | Dev |
| v5.7.2 | 2026-08-07 | **v1.3 S55 — 文档摘要自动生成实现（REQ-ING-019 Draft → Implemented）**：(1) **models/lib.rs** `Document` 新增 `summary: Option<String>` 字段（`#[serde(default)]` 向后兼容）；(2) **sqlite_storage.rs** `documents` 表新增 `summary TEXT` 列 + `ALTER TABLE` 迁移 + `DOC_COLS` 更新 + `row_to_document()` 读取 + `add_document()` 写入 + `update_document_summary()` 方法实现；(3) **core/lib.rs** `Storage` trait 新增 `update_document_summary()` 默认方法；(4) **core/import.rs** 新增 `generate_summary()` 方法（截断前 4000 字符 → LLM 摘要 prompt → `chat_stream()` 收集完整响应 → `update_document_summary()` 持久化）；(5) **commands.rs** 新增 `get_document_summary` / `regenerate_summary` 2 个 IPC 命令 + `*_inner` 函数 + 导入管线 `tokio::spawn` 异步摘要生成（不阻塞导入）；(6) **lib.rs** 注册 2 命令（142→144）；(7) **前端** main.js `loadDocuments()` 改为 wrapper 结构 + 📝 摘要展开/折叠按钮（懒加载 `get_document_summary`）+ `renderSummaryPanel()` 摘要面板（摘要文本 + 重新生成按钮）；(8) **i18n** 新增 8 键 summary_*（中英文）；(9) **tauri-stub.js** 新增 2 mock；(10) **测试** 5 TDD 测试 TC-ING-SUMMARY-001~005（MockStorage + SummaryMockLlm）。零新增依赖。 | Dev |
| v5.7.5 | 2026-08-07 | **v1.3 S58 — 对话分支/版本树实现（REQ-RAG-039 Draft → Implemented）**：(1) **models/lib.rs** 新增 `ConversationTreeNode` 结构体（node_id/conversation_id/parent_message_id/child_message_ids/active_child/created_at/turn_group/version/preview）+ `ConversationTree` 结构体（nodes/root_ids/active_path）+ `new()`/`is_root()`/`is_leaf()`/`empty()`/`find()`/`children_of()` 方法 + 4 单元测试（TC-MODEL-TREE-001~004，UTF-8 安全截断）；(2) **commands.rs** 新增 `get_conversation_tree` + `branch_from_message` 2 个 IPC 命令 + `*_inner` 函数（复用 `messages` 表 `turn_group`+`version` 列 + `turn_active_versions` 表，零新增数据库表）；`branch_from_message` 复用 `edit_user_message_inner_full` 逻辑；(3) **lib.rs** 注册 2 命令（148→150）；(4) **前端** turn-tree.js 新增 `openConversationTreePanel()`/`closeConversationTreePanel()`/`renderTreeNode()`/`switchToBranch()`/`isTreePanelOpen()` 函数 + ipc.js 新增 `convApi.getConversationTree`/`branchFromMessage` 封装 + main.js 新增 `#branchTreeBtn` 按钮绑定 + components.css 新增 `.conversation-tree-*` 样式；(5) **i18n** 新增 8 键 tree.*（中英文）；(6) **tauri-stub.js** 新增 `conversationTree` state + 2 mock；(7) **测试** 5 E2E 测试 TC-RAG-BRANCH-001~005。零新增 Rust 依赖。 | Dev |
| v5.8.0 | 2026-08-07 | **v1.3.0 发布 — 版本号升级 + 全量回归验证 + Bug 修复**：(1) **Bug 修复**：`DOC_COLS` 常量缺少 `summary` 列导致 `tc_db_001`/`tc_db_002` 测试失败（`Invalid column index: 8`），修复 `DOC_COLS` 添加 `summary` + `INSERT OR REPLACE` 添加 `?9` + `doc.summary` 参数；(2) **版本号升级** 1.2.0 → 1.3.0（7 crate Cargo.toml + tauri.conf.json）；(3) **全量回归测试**：Rust Free / Rust Pro / Clippy Free+Pro 零警告 / cargo fmt / Vitest / TypeScript 0 errors / esbuild / E2E；(4) **CHANGELOG_v1.3.md** 发布说明（5 项新功能 + Bug 修复 + 技术规格 + 向后兼容性）；(5) **Release build** `cargo build --release --features pro`；(6) v1.3 全部 5 个功能（S54-S58）实现完毕：知识图谱导出 / 文档摘要自动生成 / 自定义快捷指令模板 / Markdown 笔记双向链接 / 对话分支/版本树。IPC 命令 141→152（+11 Free）。零新增依赖。 | PM / Dev |
| v5.9.0 | 2026-08-07 | **v1.4 版本规划 — ROI 评估 + 技术调研 + 原子计划制定 + SRS 扩展**：(1) **ROI 评估**：对 12 项候选功能进行用户需求度 × 技术可行性 / 预计天数三维评分，选出 Top 5（ROI ≥ 5.0 + 铁律十/十四战略价值）；(2) **新增 5 条 Draft 需求**：REQ-ING-021（.xlsx/.csv 文档导入，calamine v0.26 + csv v1.3 纯 Rust）、REQ-RAG-040（对话全文搜索，FTS5 全文索引）、REQ-RAG-041（Contextual Retrieval 上下文增强嵌入，Anthropic 2024 论文，铁律十前沿技术）、REQ-ING-022（文档标签系统，Document.tags + JSON 存储）、REQ-ARCH-008（commands.rs 模块化拆分，7706 行 → 14 子模块，SRP 违例修复，铁律十四）；(3) **原子计划**：`docs/architecture/V1.4_ATOMIC_PLAN.md`（S61-S66，6 个 Session，预计 9 天）；(4) **§6 更新**：将 .xlsx/.csv 导入/对话搜索/Contextual Retrieval/标签系统/commands.rs 拆分从「范围外」移至「v1.4 实施计划」；(5) **新增依赖**：calamine v0.26（MIT/Apache-2.0，纯 Rust Excel 读取）+ csv v1.3（MIT/Unlicense，标准 CSV 库）；(6) **铁律八**：互联网搜索确认 calamine/csv 最新版本 + Anthropic Contextual Retrieval 论文 + SQLite FTS5 文档；(7) **铁律十**：Contextual Retrieval 从 🔄 待实现 变更为 v1.4 实施计划；Late Chunking 评估为 v1.5+ 候选（需长上下文嵌入模型）；MCP 评估为 v1.6+ 候选（Rust SDK 不成熟）；(8) README v1.4 Roadmap 更新（中英文）。无源代码变更（纯规划文档）。 | PM / TPM |
| v6.0.0 | 2026-08-07 | **commands.rs 模块化拆分完成（REQ-ARCH-008 Implemented）**：(1) **SRP 违例修复**：`commands.rs` 7706 行单文件 → `commands/mod.rs`（490 行）+ 14 个功能域子模块（chat/import/document/conversation/audit/license/settings/local_llm/security/agent/performance/graph/sync/features），最大子模块 features.rs 1559 行 ≤ 1600 行上限；(2) **零功能变更**：纯文件移动 + `pub use` 重导出，`lib.rs` 的 `generate_handler!` 152 个 IPC 命令注册不变，全部 `*_inner` 函数签名不变；(3) **共享层设计**：`mod.rs` 保留 LlmProvider enum + impl、共享常量（DEFAULT_TOP_K/TITLE_MAX_CHARS/EMBED_BATCH_SIZE 等）、共享辅助函数（emit_*/derive_title/mask_api_key/spawn_reasoning_forwarder 等），子模块通过 `use super::*` 访问；(4) **可见性修复**：5 个跨模块调用的私有函数提升为 `pub(crate)`（emit_agent_step/emit_coordinator_phase/classify_and_update_domain/generate_doc_summary_async/mask_api_key）；(5) **验证**：Free 编译 + Pro 编译通过；Free 测试全量通过（478 core + 136 infra + 28 models + 9 tauri-app + 集成测试）；Free Clippy + Pro Clippy 零警告；cargo fmt 通过。 | SDE |
| v6.1.0 | 2026-08-07 | **v1.4 S61 — .xlsx/.csv 电子表格导入实现（REQ-ING-021 Draft → Implemented）**：(1) **Cargo.toml** 新增 `calamine = "0.36"`（纯 Rust，MIT/Apache-2.0，支持 xlsx/xls/ods/xlsb）+ `csv = "1.4"`（纯 Rust，MIT/Unlicense）；(2) **loader.rs** 新增 `XlsxLoader` struct + `Loader` impl：`open_workbook_auto(path)` → 遍历 Sheet → `worksheet_range()` → `rows()` 转 `Vec<CalData>` → `rows_to_markdown_table()` Markdown 表格（首行表头 + 分隔行 + 数据行）+ `cell_to_string()` 类型转换（Int/Float/String/DateTime/Bool/Empty/Error）+ 多 Sheet 以 `---` 分隔 + `## Sheet 名` 二级标题；新增 `CsvLoader` struct + `Loader` impl：`std::fs::read()` → `detect_delimiter()`（逗号/分号/Tab 自动检测）→ `csv::ReaderBuilder` 解析 → Markdown 表格 + 空行过滤；(3) **loader_tests.rs** 新增 TC-ING-XLSX-001~004 + TC-ING-CSV-001~003 测试（7 tests，`tests/fixtures/test_{simple,multi_sheet,empty,corrupt}.xlsx` + `test_{simple,semicolon,empty_lines}.csv` fixture，Python stdlib zipfile 生成 .xlsx）；(4) **import.rs** `ALLOWED_EXTENSIONS` 13→15（新增 `"xlsx"` + `"csv"`）+ `index_inner()` + `index_text_mmap()` 新增 `"xlsx" =>` + `"csv" =>` 分支；(5) **i18n** en.json + zh-CN.json 3 处格式列表加 `.xlsx / .csv`（empty_state_formats / empty_formats / drag_hint）；(6) **tauri-stub.js** WHITELIST + 错误消息加 `xlsx`/`csv`；(7) **前端文件对话框** import.js extensions 数组新增 `'xlsx'`, `'csv'`；(8) **trace.rs** 修复 unused variable clippy 警告；(9) **token_budget.rs** 修复 CompactionInfo 导入缺失。验证：7 TDD 测试通过，core 498 + models 28 passed，clippy 零警告，fmt 通过，esbuild 12228 行，tsc 0 errors。 | Dev |
| v6.2.0 | 2026-08-07 | **v1.4 S62 — 对话全文搜索实现（REQ-RAG-040 Implemented）**：(1) **models/lib.rs** `MessageSearchResult` 结构体（message_id/conversation_id/conversation_title/role/content/score/created_at）；(2) **core/lib.rs** `Storage` trait 新增 `search_messages()` 默认空操作方法；(3) **sqlite_storage.rs** 新增 `SCHEMA_MESSAGES_FTS` FTS5 虚拟表（trigram 分词器，与 chunks_fts 一致）+ 3 触发器（INSERT/DELETE/UPDATE 自动同步 messages 表）+ `backfill_messages_fts_if_needed()` 迁移函数 + `search_messages()` 实现（短查询 <3 字符回退 LIKE、FTS5 BM25 排序、JOIN conversations 获取标题）+ 修复 `MessageSearchResult` import 缺失；(4) **conversation.rs** 新增 `search_conversations` IPC 命令 + `*_inner` 函数；(5) **mod.rs** 新增 `MessageSearchResult` 到 echomind_models import；(6) **lib.rs** 注册 1 命令（152→153）；(7) **前端 sidebar.js** 搜索弹框新增「会话/对话」模式切换按钮（`_searchMode` 变量、`_setSearchMode()` 切换、`_renderMessageSearchPage()` 对话搜索结果渲染含角色标签 Q/A + 会话标题 + 内容摘要）；(8) **index.html** 搜索弹框 header 新增模式切换 toggle 按钮；(9) **i18n** 新增 4 键 search_mode_title/search_mode_content/search_messages/search_messages_hint（中英文）；(10) **tauri-stub.js** 新增 search_conversations mock；(11) **测试** 4 TDD 存储层测试 TC-RAG-SEARCH-001~004（基本搜索/BM25排序/空查询安全/中文搜索）+ 5 E2E 测试 TC-RAG-SEARCH-005a~e。零新增依赖（复用 rusqlite FTS5）。 | Dev |
| v6.3.0 | 2026-08-07 | **v1.4 S63 — Contextual Retrieval 上下文增强嵌入实现（REQ-RAG-041 Implemented）**：(1) **state.rs** `AppState` 新增 `contextual_retrieval_enabled: bool` 字段（从 settings 表 `rag.contextual_retrieval` 读取，默认 true，仅当显式设为 "false" 时关闭）；(2) **commands/import.rs** `embed_document_chunks()` 嵌入管线条件化：`contextual_retrieval_enabled = true` 时使用 `build_contextual_text(doc_name, chunk_content)`（含文档名前缀），`false` 时使用纯 chunk content；函数提升为 `pub(crate)` 供 `performance.rs` 调用；(3) **commands/performance.rs** 新增 `set_contextual_retrieval` IPC（持久化到 settings 表 `rag.contextual_retrieval`）+ `rebuild_contextual_embeddings` IPC（遍历所有 Indexed 文档 → 逐文档 `embed_document_chunks` 重新嵌入 → 清空查询缓存+步骤缓存）；(4) **commands/settings.rs** `get_settings` 返回新增 `contextual_retrieval` 字段；(5) **models/lib.rs** `SettingsPayload` 新增 `contextual_retrieval: bool` 字段（`#[serde(default = "default_true")]`）+ `default_true()` 辅助函数；(6) **lib.rs** 注册 2 命令（153→155）；(7) **perf-settings.js** 新增 Contextual Retrieval toggle 开关 + 「重建上下文增强嵌入」按钮（复用 `rebuildButtons` 模式）；(8) **i18n** 新增 3 键 contextual_retrieval_toggle/contextual_retrieval_desc/rebuild_contextual_embeddings（中英文）；(9) **tauri-stub.js** 新增 2 mock + `contextualRetrieval` state + `get_settings` 返回含 `contextual_retrieval`；(10) **测试** 3 TDD 存储层测试 TC-RAG-CTX-001~003（上下文文本包含文档名/BM25 重建含上下文文本/设置持久化）。零新增依赖（复用 `build_contextual_text()` 已有函数）。 | Dev |
| v6.4.0 | 2026-08-07 | **v1.4 S64 — 文档标签系统实现（REQ-ING-022 Implemented）**：(1) **models/lib.rs** `Document` 新增 `tags: Vec<String>` 字段（`#[serde(default)]` 向后兼容）；(2) **sqlite_storage.rs** `documents` 表新增 `tags TEXT NOT NULL DEFAULT '[]'` 列 + `ALTER TABLE` 迁移 + `DOC_COLS` 更新 + `row_to_document()` 读取（JSON 反序列化）+ `add_document()` 写入（JSON 序列化）+ 4 个标签操作方法实现（`add_document_tag` / `remove_document_tag` / `list_all_tags` / `filter_documents_by_tag`）；(3) **core/lib.rs** `Storage` trait 新增 4 个默认空操作方法；(4) **commands/document.rs** 新增 4 个 IPC 命令 + `*_inner` 函数（add_document_tag / remove_document_tag / list_all_tags / filter_documents_by_tag）；(5) **lib.rs** 注册 4 命令（155→159）；(6) **前端 main.js** `loadDocuments()` 文档列表项新增标签徽章显示（点击移除）+ hover 标签输入框（Enter 添加）+ `data-doc-tags` 属性（JSON 数组，供客户端筛选）；(7) **前端 sidebar.js** `openKbModal()` 调用 `loadTagFilter()` 加载标签列表到 `#docTagFilter` 下拉框 + `filterDocuments()` 新增标签筛选逻辑（JSON 精确匹配）；(8) **index.html** KB Modal 新增 `#docTagFilter` select 元素（筛选区域第三行）；(9) **ipc.js** `docApi` 新增 4 方法封装；(10) **i18n** 新增 4 键 tag_input_placeholder / tag_remove_hint / filter_tag / filter_all_tags（中英文）；(11) **tauri-stub.js** 新增 4 mock；(12) **测试** 5 TDD 存储层测试 TC-ING-TAG-001~005（添加/移除/列出/筛选/幂等添加）。零新增依赖（复用 serde_json / rusqlite）。 | Dev |
| v6.5.0 | 2026-08-07 | **v1.4.0 发布 — 版本号升级 + 全量回归验证**：(1) **版本号升级** 1.3.0 → 1.4.0（7 crate Cargo.toml + tauri.conf.json）；(2) **全量回归测试**：Rust Free 808 passed / 0 failed / 24 ignored（+5 tag tests + 修复 tc_err_006 预存 bug：.xlsx 已支持但测试仍期望失败，改为 .bin）；(3) Clippy Free+Pro 零警告；(4) cargo fmt 通过；(5) TypeScript 0 errors / esbuild 12422 行；(6) Release build `cargo build --release --features pro` exit_code 0（3m59s）；(7) **CHANGELOG_v1.4.md** 发布说明（5 项新功能 + 技术规格 + 向后兼容性）；(8) sync-to-github.sh 8 files synced + git tag v1.4.0 创建。v1.4 全部 5 个功能（S61-S65）实现完毕：.xlsx/.csv 导入 / 对话全文搜索 / Contextual Retrieval / 文档标签系统 / commands.rs 模块化拆分。IPC 命令 152→159（+7 Free）。新增依赖 calamine v0.36 + csv v1.4。 | PM / Dev |
| v6.6.0 | 2026-08-08 | **B05 Durable Prompt Admission（持久化提示接纳）实现（REQ-RAG-042 Implemented）**：(1) **models/lib.rs** 新增 `PendingInput` 结构体（id/conversation_id/content/delivery/created_at/promoted_seq）+ `new()` 构造函数 + 4 单元测试 TC-MODEL-PENDING-001~004；(2) **core/lib.rs** `Storage` trait 新增 3 个默认空操作方法（`admit_input` / `promote_input` / `get_pending_inputs`）；(3) **sqlite_storage.rs** 新增 `pending_inputs` 表（6 列 + ON DELETE CASCADE + 2 索引 idx_pending_inputs_conv/promoted）+ KNOWN_TABLES 13→14 + 3 方法实现（admit INSERT + promote UPDATE 设置 promoted_seq + get_pending_inputs SELECT 排序 steer 优先 FIFO）；(4) **commands/chat.rs** 新增 3 个 IPC 命令 + `*_inner` 函数（admit_input / promote_input / get_pending_inputs）；(5) **commands/mod.rs** 新增 `PendingInput` 到 echomind_models import；(6) **lib.rs** 注册 3 命令（159→162）；(7) **tauri-stub.js** 新增 3 mock + `pendingInputs` state + reset() 清空。零新增依赖（复用 rusqlite/serde/uuid/chrono）。借鉴 OpenCode Prompt Admission / Promotion 机制。 | Dev |
| v6.7.0 | 2026-08-08 | **B06 Session Run Coordinator（会话运行协调器）实现（REQ-ARCH-009 Implemented）**：(1) **core/src/session_coordinator.rs** 新建模块：`SessionCoordinator` 结构体（`Arc<Mutex<HashMap>>` 管理 `CoordEntry`）+ `run()` 泛型方法（`Fn() -> Fut + Sync + Send`）+ `run_inner()` 内部方法（`Box::pin` 递归支持 wake 重执行）+ `wake()` 合并唤醒 + `interrupt()` 中断标记 + `is_stopping()` 状态查询 + `active_sessions()` 活跃列表；(2) **core/src/lib.rs** 注册 `session_coordinator` 模块；(3) **state.rs** `AppState` 新增 `session_coordinator: Arc<SessionCoordinator>` 字段 + 初始化；(4) **5 TDD 测试** TC-COORD-001~005（串行执行/跨会话并发/wake 合并/interrupt 标记/完成后移除）；(5) **设计关键点**：`Fn`（非 `FnOnce`）允许 wake 重执行、`Box::pin` 解决 async 递归、`Notify` 实现等待通知、`Send + Sync` 约束确保跨线程安全。零新增依赖（复用 tokio `Mutex` + `Notify`）。借鉴 OpenCode `run-coordinator.ts`。 | Dev |
| v6.8.0 | 2026-08-08 | **B07 Tool Output Bounding（工具输出有界截断）实现（REQ-RAG-043 Implemented）**：(1) **core/src/tool_output.rs** 新建模块：`BoundedOutput` 结构体（text/truncated/managed_path）+ `bound_tool_output()` 纯函数 + `MAX_TOOL_OUTPUT_LINES`(500) / `MAX_TOOL_OUTPUT_BYTES`(64KB) 常量 + `floor_char_boundary()` / `ceil_char_boundary()` UTF-8 安全辅助函数；(2) **设计**：双限制（行数+字节数）任一超标即截断，首尾保留（前 250 行+后 250 行），截断标记 `[... 输出已截断，共 N 行 M 字节 ...]`；行数不足以按行截断时按字节截断（UTF-8 安全边界）；(3) **core/src/lib.rs** 注册 `tool_output` 模块；(4) **agent.rs** 集成：`execute_tool_single()` search_kb 路径 + `execute_code_tool()` execute_code 路径均调用 `bound_tool_output()` 截断；(5) **5 TDD 测试** TC-BOUND-001~005（短输出不截断/超行截断/超字节截断/截断标记格式/空输出安全）。零新增依赖。借鉴 OpenCode `tool-output-store.ts`。 | Dev |
| v6.9.0 | 2026-08-08 | **B08 Session Todo 持久化（会话待办持久化）实现（REQ-RAG-044 Implemented）**：(1) **models/lib.rs** 新增 `SessionTodo` 结构体（id/conversation_id/content/status/priority/position/created_at）+ `TodoStatus` 枚举（Pending/InProgress/Completed）+ `TodoPriority` 枚举（Low/Medium/High）+ `as_str()` / `from_db_str()` 方法 + 4 单元测试 TC-MODEL-TODO-001~004；(2) **core/lib.rs** `Storage` trait 新增 5 个默认空操作方法（`add_session_todo` / `update_todo_status` / `get_session_todos` / `delete_session_todo` / `delete_session_todos`）；(3) **sqlite_storage.rs** 新增 `session_todos` 表（7 列 + ON DELETE CASCADE + 2 索引）+ KNOWN_TABLES 14→15 + 5 方法实现（INSERT/UPDATE/SELECT position ASC/DELETE/DELETE batch）；(4) **commands/chat.rs** 新增 5 IPC 命令 + `*_inner` 函数；(5) **commands/mod.rs** 新增 `SessionTodo` + `TodoStatus` import；(6) **lib.rs** 注册 5 命令（162→167）；(7) **tauri-stub.js** 新增 5 mock + `sessionTodos` state + reset() 清空。零新增依赖。借鉴 OpenCode `session/todo.ts`。 | Dev |
| v7.0.0 | 2026-08-08 | **B09 Skill 系统发现（技能发现）实现（REQ-ARCH-010 Implemented）+ 铁律一强化（代理仅限开发调研，软件本身禁止代理）**：(1) **core/src/skill.rs** 新建模块：`Skill` 结构体（name/description/slash/content/source_path）+ `parse_skill()` 纯函数（YAML frontmatter 解析，无 YAML crate 依赖）+ `extract_yaml_field()` 辅助函数 + `discover_from_dir()` 异步目录扫描函数（tokio::fs）；(2) **core/src/lib.rs** 注册 `skill` 模块；(3) **5 TDD 测试** TC-SKILL-001~005（有效解析/slash 字段/无 frontmatter 返回 None/缺 name 返回 None/目录扫描返回列表）+ 2 doctest；(4) **铁律一强化**：明确代理仅限开发调研，软件运行时禁止代理。6 处 `reqwest::Client::builder()` 全部加 `.no_proxy()`（openai_provider/openai_vision/duckduckgo_provider/ocrs_engine/model_manager/robust_downloader）。零新增依赖。借鉴 OpenCode `skill.ts`。 | Dev |
| v7.1.0 | 2026-08-08 | **B10 多协议 LLM 适配 + B11 Permission 细粒度控制实现（REQ-ARCH-011/012 Implemented）**：(1) **core/src/permission.rs** 新建模块：`PermissionEffect` 枚举（Allow/Deny/Ask）+ `PermissionRule` 结构体（action/resource/effect）+ `Ruleset` 类型别名 + `evaluate()` 纯函数（last-writer-wins 从后向前查找）+ `wildcard_match()` 通配符匹配函数（`*` 全匹配 + `prefix.*` 前缀匹配）；(2) **core/src/llm_protocol.rs** 新建模块：`LlmProtocol` 枚举（OpenAIChat/OpenAIResponses/AnthropicMessages/Gemini）+ `as_str()` / `parse_str()` 字符串映射 + `all()` 列表方法 + `ProtocolAwareProvider` trait（扩展 LLMProvider）；(3) **core/src/lib.rs** 注册 `permission` + `llm_protocol` 模块；(4) **5+5 TDD 测试** TC-PERM-001~005 + TC-PROTO-001~004 + 1 extra roundtrip + 4 doctest。零新增依赖。借鉴 OpenCode `permission.ts` + `packages/llm/`。B12-B14 评估为暂不实施（Snapshot/Revert 低优先级 / Plugin V2 当前不需要 / Effect 分层仅参考）。 | Dev |
| v7.2.0 | 2026-08-08 | **Q01 Scratch-Promote 两层记忆 + Consolidation 引擎实现（借鉴 QM scratch-promote + consolidation）**：(1) **models/lib.rs** 新增 `MemoryConsolidationAction` 枚举（Update/Delete/Add）+ `ScratchLogEntry` 结构体（id/date/content/created_at）+ `ScratchConsolidationResult`；(2) **core/lib.rs** Storage trait 新增 4 个 scratch 方法（add_scratch_log / get_scratch_logs / delete_scratch_log / cleanup_expired_scratch_logs）；(3) **sqlite_storage.rs** 新增 `scratch_logs` 表 + KNOWN_TABLES 16→17 + 4 方法实现；(4) **memory_store.rs** 新增 `write_scratch()` + `consolidate_scratch<L: LLMProvider>()` + `apply_consolidation_actions()` + `call_llm_for_consolidation()` + `parse_consolidation_output()` + `MEMORY_CONSOLIDATION_PROMPT` 常量；(5) **memory_store_tests.rs** MockStorage 新增 scratch_logs 字段 + 4 方法实现 + 7 TDD 测试 TC-SCRATCH-001~007；(6) **auto_dream.rs** DreamEngine Phase 5 `consolidate_scratch_layer()`；(7) **features.rs** 新增 2 IPC 命令 + lib.rs 注册（162→164）；(8) **tauri-stub.js** 新增 2 mock。借鉴 QM `memory/strategies/scratch-promote.ts` + `consolidation.ts`。 | Dev |
| v7.3.0 | 2026-08-08 | **Q02 Burst Buffer 延迟批量记忆提取 + Provenance 标记实现（借鉴 QM createBurstBuffer）**：(1) **models/lib.rs** 新增 `ProvenanceTag` 结构体（conversation_id/message_seq/captured_at/source_label）+ `new()` / `with_timestamp()` / `said_in_suffix()` 方法 + 4 单元测试 TC-MODEL-PROV-001~004；(2) **memory_store.rs** 新增 `BurstTurn` 结构体（user_msg/assistant_reply/provenance）+ `BurstBuffer` 结构体（pending/quiet_ms/max_turns/last_activity）+ `new()` / `with_config()` / `push()` / `should_flush()` / `pending_count()` / `is_empty()` / `drain()` / `flush()` 方法 + `flush_burst_turns()` 独立函数（聚合多轮 → LLM 提取 → scratch 写入 + provenance 后缀）；(3) **memory_store_tests.rs** 新增 10 TDD 测试 TC-BURST-001~010；(4) **state.rs** AppState 新增 `memory_burst_buffer: Arc<Mutex<BurstBuffer>>` 字段；(5) **features.rs** 新增 3 IPC 命令（push_burst_turn / flush_memory_burst_buffer / get_burst_buffer_status）+ `*_inner` 函数；(6) **lib.rs** 注册 3 命令（164→167）；(7) **tauri-stub.js** 新增 3 mock + `burstBuffer` state + reset() 清空。借鉴 QM `memory/strategies/per-turn.ts` 的 `createBurstBuffer(quietMs, maxTurns, flush)` + `ccCaptureToPersonal()` 的 `(said in #channel)` provenance 标记。 | Dev |
| v7.4.0 | 2026-08-08 | **Q03 双阈值压缩 + 后台异步压缩实现（借鉴 QM compaction.ts 双阈值机制）**：(1) **models/lib.rs** 新增 `CompactionKind` 枚举（Background/Synchronous）+ `as_str()` 方法 + `CompactionInfo` 新增 `compaction_kind: Option<CompactionKind>` 字段（`#[serde(default)]` 向后兼容）；(2) **compact/lib.rs** 新增 `CompactionTrigger` 枚举（None/Background/Synchronous，`#[derive(Default)]`）+ `DualThresholdConfig` 结构体（soft_threshold/hard_threshold/max_entries/max_tokens，默认 0.7/0.9/50/8192）+ `from_settings()` 方法 + `estimate_history_tokens()` 粗估函数 + `check_compaction_needed()` 方法（双阈值判断：entry_ratio vs token_ratio 取 max）+ `try_acquire_background_compaction()` / `release_background_compaction()` 去重函数 + `run_background_compaction()` 错误吞咽执行函数 + `serde` 依赖新增；(3) **compact/compact_tests.rs** 新增 12 TDD 测试 TC-COMPACT-DUAL-001~012（阈值判断/token_ratio 驱动/同步压缩/错误吞咽/去重/默认值/serde/from_settings/防除零/CompactionKind）；(4) **state.rs** AppState 新增 `compaction_pending: Arc<Mutex<HashSet<String>>>` 字段；(5) **commands/chat.rs** `chat_inner` 集成双阈值逻辑：Soft → `try_acquire` + `tokio::spawn` 后台压缩（创建新 provider，不借用当前）/ Hard → 同步 `compact()` / None → 跳过压缩返回原始历史。借鉴 QM `core/orchestrator/compaction.ts` 的 `COMPACT_SOFT_FRACTION`(70%) / `COMPACT_HARD_FRACTION`(90%) + `scheduleBackgroundCompaction()`。 | Dev |
| v7.5.0 | 2026-08-08 | **Q04 Token 级预算 + Entry Token 缓存 + Dangling Call 修复实现（借鉴 QM estimateEntryTokens LRU + healDanglingCalls）**：(1) **compact/lib.rs** 新增 `TokenEstimator` 结构体（LRU 缓存：`HashMap<u64, usize>` + `Vec<u64>` LRU 顺序 + `tokio::sync::RwLock` 并发安全）+ `new(max_cache)` / `estimate(content)` 异步方法（读锁查缓存 → 未命中 BPE 编码 → 写锁 LRU 驱逐 + 插入）+ `estimate_messages(messages)` 批量估算 + `recent_entry_count_within_budget(messages, budget)` 从末尾向前保留消息（至少 1 条）+ `cache_len()` 测试用 + `calculate_content_hash()` 辅助函数；(2) **compact/lib.rs** 新增 `heal_dangling_calls(history)` 函数（借鉴 QM `tape-fold.ts` healDanglingCalls）：检测 assistant 消息中 Action 模式（`Action:` / `Action N:`）且不含 Final Answer 且下一条消息无 Observation → 插入 `[interrupted]` 占位 system 消息；从后向前插入避免索引偏移；`has_action_pattern()` / `has_final_answer()` / `has_observation_pattern()` 辅助函数；(3) **compact/compact_tests.rs** 新增 6 TDD 测试 TC-TOKEN-CACHE-001~003（缓存命中/LRU 驱逐/recent_entry_count_within_budget）+ TC-HEAL-DANGLING-001~003（单个修复/多个修复/已配对不受影响）。零新增依赖。借鉴 QM `harness/context-compaction.ts` `estimateEntryTokens()` 50k LRU + `recentEntryCountWithinBudget()` + `harness/tape-fold.ts` `healDanglingCalls()`。 | Dev |
| v7.6.0 | 2026-08-08 | **Q05 安全态势分层 + Security-Tainted 条目实现（借鉴 QM SecurityPosture + securityTainted）**：(1) **core/src/security.rs** 新增 `SecurityPosture` 枚举（Dangerous/Auto/Strict，`#[derive(Default)]` Auto 为默认）+ `as_str()` / `parse_str()` 字符串映射 + `as_u8()` / `from_u8()` AtomicU8 无锁存储 + `ResolvedSecurityPolicy` 结构体（inbound_screening/tool_approvals）+ `compose_security_posture()` 组合函数（子 scope 只能收紧，不能放松父 scope）+ `resolve_security_policy()` 策略解析函数；(2) **core/src/lib.rs** Storage trait 新增 `set_entry_security_tainted()` / `get_entry_security_tainted()` 默认空操作方法 + 注册 `security_tests` 测试模块；(3) **infra/src/sqlite_storage.rs** messages 表新增 `security_tainted INTEGER NOT NULL DEFAULT 0` 列 + ALTER TABLE 迁移 + 2 方法实现（UPDATE/SELECT）；(4) **state.rs** AppState 新增 `security_posture: Arc<AtomicU8>` 字段 + `get_security_posture()` / `set_security_posture_value()` 便捷方法；(5) **commands/security.rs** 新增 2 IPC 命令 `set_security_posture` / `get_security_posture`；(6) **lib.rs** 注册 2 命令（167→169）；(7) **tauri-stub.js** 新增 2 mock + `securityPosture` state；(8) **core/src/security_tests.rs** 13 TDD 测试 TC-POSTURE-001~007 + 2 extra（Default/Policy 字段验证）+ filter_tainted_messages 模拟 QM `forModelContext()` 过滤逻辑。新增设置键 `security.posture`（默认 "auto"）。零新增依赖。借鉴 QM `security/security-posture.ts` + `harness/context-compaction.ts` `forModelContext()`。 | Dev |
| v7.7.0 | 2026-08-08 | **Q06 Shadow 安全筛查模式实现（借鉴 QM security-screen.ts）**：(1) **core/src/security.rs** 新增 `SecurityScreenVerdict` 结构体（decision/reason/unscreened，`#[derive(PartialEq, serde::Serialize, serde::Deserialize)]`）+ `SecurityScreener` trait（`classify() -> Pin<Box<dyn Future<Output = Option<SecurityScreenVerdict>> + Send>>`，对象安全）+ `Agreement` 枚举（Unavailable/Agree/Disagree）+ `ShadowScreenResult` 结构体（authoritative/shadow/agreement）+ `compute_agreement()` 内部函数（unscreened 裁决视为不可比较）+ `run_shadow_screen()` 并行筛查函数（`tokio::join!`）+ `run_shadow_screen_with_timeout()` 超时降级函数（超时 → `unscreened` 裁决）+ `ShadowScreenStats` 结构体（total/agree/disagree/unavailable + `disagree_rate()`）+ `ShadowScreenCollector` 线程安全统计收集器（`Arc<Mutex<ShadowScreenStats>>`，record/stats/reset）；(2) **core/src/security_tests.rs** 新增 10 TDD 测试 TC-SHADOW-001~006（agree/disagree/unavailable/shadow 不影响决策/超时降级/审计日志）+ 3 extra（collector reset/Arc 共享/serde roundtrip）；(3) **state.rs** AppState 新增 `shadow_screen_collector: Arc<ShadowScreenCollector>` 字段；(4) **commands/security.rs** 新增 2 IPC 命令 `get_security_screen_stats` / `reset_security_screen_stats`；(5) **lib.rs** 注册 2 命令（169→171）；(6) **tauri-stub.js** 新增 2 mock + `shadowScreenStats` state + reset() 清空。零新增依赖。借鉴 QM `core/orchestrator/security-screen.ts` `runShadowScreen()` + `security/security-screener.ts` `createSecurityScreenProxy()`。 | Dev |
| v7.8.0 | 2026-08-08 | **Q07 Idempotent Storage + Unified Periodic Task Abstraction 实现（借鉴 QM idempotency-store.ts + sweeper.ts）**：(1) **core/src/idempotency.rs** 新建模块：`IdempotencyStore` 幂等存储（LRU 驱逐 + TTL 失效 + 分布式安全 `once(key, f)` 执行）+ `Sweeper` 统一周期任务抽象（`schedule(interval, task)` 调度 + 自动 panic 捕获 + 优雅关闭）；(2) **core/src/idempotency_tests.rs** 9 个 TDD 测试（TC-IDEMP-001~005 含 `catch_unwind` 异常捕获测试 + TC-SWEEPER-001~004）；(3) **core/src/auto_dream.rs** 集成幂等性：`dream` 方法添加 "guard layer" 模式，使用 `idempotency_store.once("auto_dream_full_scan", ...)` 防止重复执行；(4) **core/src/sync.rs** 集成幂等性：`SyncService` 创建内部短时 `IdempotencyStore`（5分钟窗口），基于文件夹内容哈希值的 `once` 检查，防止相同内容文件夹的重复同步；(5) **core/src/lib.rs** 注册 idempotency 模块；(6) **tauri-app/src/commands/features.rs** 更新 `DreamEngine::new` 调用集成 `IdempotencyStore`；(7) 零新增依赖，复用 tokio/time/anyhow。借鉴 QM `idempotency-store.ts` `once()` 分布式锁语义 + `sweeper.ts` 周期任务调度器。 | Dev |
| v7.9.0 | 2026-08-08 | **Q08 预算追踪 + 速率限制实现（借鉴 QM BudgetTracker + RateLimiter）**：(1) **models/src/lib.rs** 新增 `BudgetStats` 结构体（daily_limit/spent_today/remaining）+ `RateLimitStats` 结构体（max_per_window/current_count/window_seconds）；(2) **core/src/budget.rs** 新建模块：`estimate_cost_usd()` 成本估算函数（input_tokens/usd_per_mtok）+ `BudgetTracker`（daily_limit/spent LRU 滑动窗口）+ `check()` 预算检查 + `record()` 记录使用 + `get_stats()` 获取统计 + `set_daily_limit()` 设置限制 + `RateLimiter`（max_per_window/window Duration/state HashMap）+ `check()` 请求检查 + `reset()` 重置计数 + `get_stats()` 获取速率统计；(3) **core/src/budget_tests.rs** 6 个 TDD 测试（TC-BUDGET-001~004 含滑动窗口重置+成本估算 + TC-RATELIMIT-001~002 含窗口限流）；(4) **core/src/lib.rs** 注册 budget 和 budget_tests 模块； (5) **infra/src/sqlite_storage.rs** 新增 budget_records 表（principal/timestamp/input_tokens/output_tokens/cost_usd/model_name）+ 索引 (idx_budget_records_principal/timestamp) + KNOWN_TABLES 18 表 + record_budget_usage() / get_budget_stats() / set_budget_limit() Storage trait 实现；(6) **state.rs** AppState 新增 `budget_tracker: Arc<BudgetTracker>` 字段 + new() 方法初始化（默认 unlimited）；(7) **commands/settings.rs** 新增 2 IPC 命令 `get_budget_stats()` / `set_budget_limit(daily_limit_usd)` + 内部实现函数；(8) **commands/chat.rs** record_token_usage_inner() 集成预算追踪：调用 LLM 后先检查预算限制 → 记录 budget_records 数据库 + 更新内存 BudgetTracker；(9) **lib.rs** 注册 2 命令（171→173）；(10) **tauri-stub.js** 新增 2 mock 命令 + budgetDailyLimit/budgetSpentToday state。新增设置键 `budget.daily_limit_usd`（默认 0 = unlimited）。零新增依赖。借鉴 QM `ratelimit/budget.ts` `trackSpent()` + `ratelimit/rate-limiter.ts` `isAllowed()`。 | Dev |
| v8.0.0 | 2026-08-08 | **Q09 KeyedQueue 并发序列化 + Durable-by-Default 审查实现（借鉴 QM createKeyedQueue + Durable by default）**：(1) **core/src/concurrency.rs** 新建模块：`KeyedQueue<K>` 泛型结构体（`tails: Arc<RwLock<HashMap<K, Arc<Mutex<()>>>>>`）+ `run(key, f)` 按 key 序列化执行（同 key 串行，不同 key 并行）+ `cleanup()` 清理空闲 tail 防 memory leak + `create_keyed_queue()` 便捷构造函数 + `Default` impl；(2) **core/src/concurrency_tests.rs** 新增 6 TDD 测试 TC-KEYED-001~006（同 key 串行执行/不同 key 并行执行/tail 清理无泄漏/panic 不影响后续同 key 操作/泛型返回值/多 key 并发各自独立串行）+ 3 内联单元测试；(3) **core/src/lib.rs** 注册 `concurrency` 模块；(4) **state.rs** AppState 新增 `persist_queue: KeyedQueue<String>` 字段 + 初始化；(5) **commands/chat.rs** `persist_exchange` 重构为 KeyedQueue 包装器：拆分为 `persist_exchange`（按 conversation_id 序列化）+ `persist_exchange_inner`（实际 SQLite 写入逻辑），4 个调用点自动受益；(6) **Durable-by-Default 审查**：全量审查 AppState 10+ 内存状态字段持久化状况——abort_tokens（会话级可丢弃 ✅）/ hybrid_search（settings 表 ✅）/ compression_ratio（settings 表 ✅）/ is_pro（settings 表 ✅）/ security_posture（settings 表 security.posture ✅）/ budget_tracker（budget_records 表 ✅）/ shadow_screen_collector（运行时聚合可丢弃 ✅）/ memory_burst_buffer（临时缓冲可丢弃 ✅）/ compaction_pending（崩溃后重新触发安全 ✅）/ session_coordinator（运行时状态可丢弃 ✅）。零新增依赖。借鉴 QM `util/async.ts` `createKeyedQueue<K>()` + `wiring.ts` AGENTS.md "Durable by default" 纪律。 | Dev |
| v8.1.0 | 2026-08-08 | **Q10 LLMProvider Trait 扩展 — 辅助模型方法实现（借鉴 QM HarnessModelUtilities）**：(1) **core/src/lib.rs** `LLMProvider` trait 新增 4 个可选默认方法：`generate_title(&self, transcript: &str) -> Result<Option<String>>`（默认 `Ok(None)`，借鉴 QM generateTitle）+ `one_shot(&self, system: &str, prompt: &str) -> Result<Option<String>>`（默认 `Ok(None)`，借鉴 QM oneShot）+ `judge(&self, system: &str, prompt: &str) -> Result<Option<String>>`（默认 `Ok(None)`，借鉴 QM judge）+ `context_token_budget(&self) -> Option<usize>`（默认 `None`，借鉴 QM contextTokenBudget）；所有方法有默认实现，现有 `LLMProvider` 实现者无需修改（向后兼容）；(2) **core/src/llm_aux_tests.rs** 新建测试文件：7 TDD 测试 TC-LLM-AUX-001~007（默认实现返回 None/ generate_title 返回标题/失败降级/one_shot 返回响应/context_token_budget 返回上限/judge 返回裁决/泛型分发验证）+ `MinimalStub` + `RichMock` 两个 mock provider；(3) **infra/src/openai_provider.rs** `OpenAIProvider` 实现 3 个方法覆盖：`generate_title`（复用 `complete()` 非流式调用 + 30 字截断）+ `one_shot`（委托 `complete()`）+ `judge`（委托 `complete()` + trim）；`context_token_budget` 使用默认实现（返回 None）；(4) **commands/mod.rs** `LlmProvider` enum 转发 4 个新方法到 Remote/Local 内部 provider；(5) **commands/chat.rs** `persist_exchange_inner` 集成 LLM 辅助标题生成：首轮对话先设置 `derive_title()` 快速标题 → Remote 模式下异步尝试 `generate_title()`（10s 超时 + answer 截断 200 字 + 失败/超时/None 静默降级保留 derive_title）；新增 `use echomind_core::LLMProvider` 导入。零新增依赖。借鉴 QM `harness/harness.ts` `HarnessModelUtilities` 接口（generateTitle/oneShot/judge/contextTokenBudget）。 | Dev |
| v8.2.0 | 2026-08-08 | **Q11 Harness Router — 动态后端切换实现（借鉴 QM HarnessRouter）**：(1) **core/src/llm_router.rs** 新建模块：`LlmChoice` 结构体（mode: LlmMode + model_id: String，借鉴 QM RuntimeChoice）+ `RouterVerdict` 枚举（Initial/SameMode/ModeChanged，借鉴 QM resetSession 信号）+ `RouterError` 枚举（ModeUnavailable）+ `LlmRouter` 结构体（fallback: Arc<RwLock<LlmChoice>> + last_mode: Arc<RwLock<HashMap<String, LlmMode>>> + available_modes: Arc<RwLock<HashSet<LlmMode>>>）+ `resolve()` 路由方法（fallback 解析 → 可用性校验 → 模式变更检测 → 记录 last_mode）+ `set_fallback()` / `set_available_modes()` / `last_mode_for()` / `clear()` / `fallback()` 方法 + `new_free_default()` / `new_pro_default()` 便捷构造 + `Default` impl；(2) **core/src/llm_router_tests.rs** 12 TDD 测试 TC-ROUTER-001~012（fallback 默认/请求 Remote/请求 Local/模式变更检测/Free 请求 Local 错误/多会话独立追踪/set_fallback 更新/clear 清除/set_available_modes 动态更新/Error Display/Default 是 Free/便捷构造函数）；(3) **models/src/lib.rs** `LlmMode` derive 新增 `Hash`（HashSet<LlmMode> 所需）；(4) **state.rs** AppState 新增 `llm_router: LlmRouter` 字段 + 初始化（根据 is_pro 选择 Free/Pro 可用模式 + 根据 llm_mode 设置 fallback）；(5) **commands/chat.rs** `chat_inner` 使用 `llm_router.resolve()` 替代直接 `get_llm_mode()` if/else 分支 + 模式变更日志；(6) **commands/local_llm.rs** `set_llm_mode_inner` 同步更新 router fallback（读取 llm.local_model 设置 model_id）；(7) **commands/license.rs** `activate_pro_inner` / `deactivate_pro_inner` 动态更新 router 可用模式集合。零新增依赖。借鉴 QM `harness/harness-router.ts` `createHarnessRouter(adapters, utility, resolve)` + `lastHarness` 追踪 + `resetSession` 信号。 | Dev |
| v8.3.0 | 2026-08-08 | **跨 Phase 可选依赖整合（S67+S68+S71）**：(1) **S67 Burst Buffer 用 Q10 one_shot 替代非流式 LLM 调用**：`memory_store.rs` 新增 `llm_complete()` 辅助函数——优先尝试 `LLMProvider::one_shot()`，`Ok(None)`（不支持）或 `Err`（API 错误）时降级为 `chat_stream + collect_stream`。`flush_burst_turns` / `extract_from_conversation` / `call_llm_for_consolidation` 三处统一使用 `llm_complete()`。5 个 TDD 测试 TC-BURST-011~015 覆盖 one_shot 优先/chat_stream 降级/向后兼容/extract 集成/consolidate 集成。(2) **S68 后台压缩用 Q07 Sweeper 统一管理**：`compact/lib.rs` 新增 `schedule_background_compaction()` 函数——合并 `try_acquire + run + release` 三步为单次调用，返回 `Pin<Box<dyn Future>>` 供调用方 spawn。`run_background_compaction` 日志从 `eprintln!` 升级为 `tracing::info!/warn!`（与 Sweeper 一致）。新增 `tracing` 依赖。4 个 TDD 测试 TC-SWEEPER-001~004 覆盖成功调度/去重/LLM 失败释放/自动释放。(3) **S71 Shadow 筛查用 Q10 judge 方法做安全判断**：`security.rs` 新增 `llm_classify()` 异步函数——调用 `LLMProvider::judge()` 获取裁决，解析 "allow"/"block" 为 `SecurityScreenVerdict`，可直接传递给 `run_shadow_screen()` 做 shadow 筛查。7 个 TDD 测试 TC-LLM-SCREEN-001~007 覆盖 allow/block/不支持/失败/agree/disagree/unavailable。 | Dev |
