# PROJECT_RULES — EchoMind（灵犀）项目宪法

> 本文件是 EchoMind 项目的最高约束，与 `docs/SRS_v1.0.md`、`docs/TEST_STRATEGY.md` 共同构成「唯一真相源」。
> 任何代码、设计、流程与本文件冲突时，一律以本文件为准并返工。

| 项 | 内容 |
| --- | --- |
| 产品 | EchoMind（灵犀）—— 商业级本地 AI 知识库桌面应用 |
| 价值主张 | Rust 极速 · 隐私不出域 · BYOK 买断制 |
| 文档版本 | v1.4.0 |
| 生效日期 | 2026-07-25 |
| 维护者 | TPM（修订须走附则流程） |

---

## 0. 技术栈基线（锁定）

非经 Chief Architect 发起、TPM 批准的架构评审，任何人不得变更以下选型。

| 层 | 选型 | 硬性约束 |
| --- | --- | --- |
| 语言 | Rust 1.97.1（Edition 2024） | 使用原生 `async fn` in trait，**禁用 `async-trait` 宏** |
| 桌面框架 | Tauri v2 | 前后端仅经 `#[tauri::command]` 与 `Event` 通信，禁止旁路 |
| 前端 | 纯原生 HTML / JS / Tailwind CSS | **禁止** React / Vue / Svelte / jQuery 等任何框架与运行时依赖 |
| 流式渲染 | marked.js + DOMPurify + highlight.js | 仅这三个前端库，本地打包引入，**禁止 CDN** |
| E2E | Playwright（+ tauri-driver） | 覆盖全部 P0 用户旅程 |

---

## 体系一：全明星团队角色与职责（The Dream Team）

AI 引擎在每次任务中按角色视角协同工作。各角色**严禁越位**：只交付本角色产物，不代写他人产物。

| 角色 | 职责 | 标准交付物 | 禁区 |
| --- | --- | --- | --- |
| TPM 技术项目经理 / 总指挥 | 需求拆解、跨部门接口契约对齐、工作流编排、门禁审查 | 任务拆解单、契约登记、验收结论 | **不写任何业务代码** |
| PM 产品经理 | 原子化需求（SRS）、用户旅程设计、验收标准 | `docs/SRS_v1.0.md` 中的 REQ 条目 | 不定义技术实现方案 |
| Chief Architect 首席架构师 | 守护 Rust 六边形架构、异步安全、性能调优 | 端口 Trait 设计、依赖方向审查结论 | 不允许 core 泄漏任何具体实现 |
| UI/UX Expert 设计专家 | 交互对标 Warp / DeepSeek；极简暗黑主题；微交互与状态反馈 | 视觉与交互规范、状态流转定义 | 不引入规范外色值与组件风格 |
| Frontend Dev 前端专家 | 纯 HTML/JS/Tailwind 实现；复杂前端状态管理；防抖节流；流畅动画 | `ui/` 前端代码 | 契约冻结前**禁止编写任何 `invoke` 调用** |
| Backend Dev 后端专家 | Rust 核心逻辑与 Tauri IPC 通信实现 | `crates/`、`src-tauri/` 代码 | 禁止绕过端口 Trait 直接访问具体实现 |
| Security Officer 安全官 | 审查所有 I/O；确保数据加密；防御 XSS；API Key 仅本地存储 | 安全审查结论、威胁清单 | 对明文密钥 / 未消毒 HTML 拥有**一票否决权** |
| QA Lead 测试主管 | 100% 自动化测试；TDD 先行；E2E 覆盖状态流转 | 红灯测试用例、REQ↔TC 追溯矩阵 | 不允许无测试的需求进入开发阶段 |
| Growth Strategist 增长策略师 | Freemium 付费墙逻辑、License 校验、转化率优化 | 配额与升级引导需求、License 校验方案 | 任何增长手段不得损害「隐私不出域」承诺 |
| DevOps & Tech Writer 运维与文档 | GitHub Actions 全平台打包；Docker 隔离测试；中英双语 README | CI/CD 流水线、`README.md`（中英） | 不允许手工打包发布 |

---

## 体系二：跨部门协同工作流（测试左移 · 契约驱动）

任何功能开发必须由 TPM 拆解为工作流后，严格按以下流水线执行，**严禁跳阶段、严禁越位**。

### 阶段 1 — PM（需求定义）
- 在 `docs/SRS_v1.0.md` 中定义原子需求（`REQ-<域>-<序号>`）与可执行的验收标准（AC）。
- 出口门禁：每条 AC 可测试、无歧义、不含技术实现方案。

### 阶段 2 — QA（前置测试，红灯）
- 在开发写代码**之前**，QA 根据 AC 编写测试用例骨架（单元 / 契约 / E2E）。
- 此时测试**必须失败（红灯）**；QA 将断言逻辑同步全队。
- 出口门禁：每个 AC 至少对应一条红灯测试，并登记 `TC-<域>-<序号>` 追溯关系。

### 阶段 3 — TPM（契约对齐）
- 拉齐前后端：在对应 REQ 条目内冻结 Tauri Command 签名与 Event Payload 结构（名称、参数、返回值、事件负载、错误码）。
- **契约冻结前：前端严禁编写任何 `invoke` 调用，后端严禁开始实现。**
- 出口门禁：契约字段完整，无 TBD。

### 阶段 4 — 开发（绿灯冲刺）
- Chief Architect / Frontend Dev / Backend Dev 开始实现，**唯一目标是把 QA 的红灯测试变绿**。
- 严禁偏离测试断言自由发挥；发现断言不合理时，退回 QA 阶段修订测试，而非绕开测试。
- 出口门禁：相关测试全绿，且 `cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo check` 全部通过。

### 阶段 5 — 闭环
- 测试全绿后，TPM 审查契约一致性与文档同步情况，将 REQ 状态置为 `Done`。
- 执行一次 Conventional Commit，任务关闭。

---

## 体系三：架构纯洁性与代码铁律

### 3.0 工程纪律铁律（完整规则见 `.catpaw/rules/system-constraints.md`）

> **铁律一 ~ 铁律二十一** 已统一整合到 `.catpaw/rules/system-constraints.md`，本节仅保留 EchoMind 项目特定的架构补充约束。
> 以下为铁律编号速查（详见 `.catpaw/rules/system-constraints.md`）：

| 铁律 | 主题 |
|---|---|
| 一 | 网络代理设置（仅限开发调研，软件本身禁止代理） |
| 二 | TDD 驱动开发 |
| 三 | 代码注释规范 |
| 四 | 真实数据测试 |
| 五 | 生产级代码质量（零 unwrap / 零 panic / 零裸溢出） |
| 六 | 最新版本依赖原则 |
| 七 | 文档实时同步 |
| 八 | 开发决策必须互联网搜索 |
| 九 | Rust 生态优先原则 |
| 十 | 最前沿技术追踪 |
| 十一 | GitHub 发布同步规则（双仓库工作流） |
| 十二 | Session 上下文全自动管理 |
| 十三 | 前端样式必须使用 Tailwind 工具类 + CSS 变量 |
| 十四 | SOLID 架构原则 |
| 十五 | Spec-driven Development（规格驱动开发） |
| 十六 | AI 生成代码审查与验证 |
| 十七 | 结构化日志与可观测性 |
| 十八 | LLM 应用安全防护 |
| 十九 | 错误处理与弹性设计 |
| 二十 | AI Agent 行为规范 |
| 二十一 | RAG 安全防护（向量与嵌入弱点） |

#### EchoMind 特定补充约束

**网络代理 — 软件运行时禁止代理的代码位置**（铁律一项目特定落实点）：
- fastembed ONNX 模型下载（运行时首次加载）
- Web 搜索（`DuckDuckGoProvider`）
- 任何其他用户面向的网络请求

**代码强制要求**：所有 `reqwest::Client::builder()` 必须加 `.no_proxy()` 确保软件直连，不受 `https_proxy` / `http_proxy` 环境变量影响。

> **铁律二 ~ 铁律八**（TDD、代码注释、真实数据测试、生产级代码质量、最新版本依赖、文档实时同步、开发决策互联网搜索）的完整内容见 `.catpaw/rules/system-constraints.md`。
> **铁律十六 ~ 铁律二十一**（AI 代码审查、结构化日志、LLM 安全、错误弹性、Agent 行为规范、RAG 安全防护）为 v3.0~v3.1 新增，来源：2026 年最新业界实践调研（含 OWASP GenAI LLM Top 10 2026、OpenSSF Scorecard、cargo-miri 等）。

### 3.1 六边形架构
1. 核心逻辑位于 `crates/core`：**绝对禁止依赖** Tauri、reqwest、rusqlite 等任何具体实现；对外只暴露领域模型与端口 Trait（原生 `async fn` in trait）。
2. 适配器位于 `crates/infra`（文件系统、向量库、HTTP、加密等），负责实现 core 定义的端口。
3. `src-tauri` 仅做组装：依赖注入、Command / Event 声明、错误码映射，不含业务规则。
4. 依赖方向单向向内：`src-tauri` → `crates/infra` → `crates/core`，**严禁反向依赖**。

### 3.2 异步绝对安全
1. 生产代码**严禁 `unwrap()` / `expect()`**；领域错误用 `thiserror` 定义，应用组装层可用 `anyhow`。以 `clippy::unwrap_used` / `clippy::expect_used = deny` 强制（测试代码除外）。
2. **严禁跨 `.await` 持有 `std::sync::Mutex`**；确需共享可变状态时使用 `tokio::sync::Mutex`，并将临界区缩到最小。
3. CPU 密集型任务（解析、分块、哈希、向量计算）必须经 `tokio::task::spawn_blocking` 或专用线程池执行，严禁阻塞 async executor。
4. UI 零阻塞：前端所有 `invoke` 异步化；长任务必须提供进度事件与取消机制。

### 3.3 UI/UX 铁律
1. 深邃灰黑背景 `#161616`，科技青蓝强调色 `#38BDF8`；全部色值以 Tailwind 配置为唯一来源。
2. Block 式对话设计；底部控制台式输入框。
3. 流式 Markdown 渲染**必须**走 marked.js → DOMPurify（白名单消毒）→ highlight.js 链路；禁止任何 `innerHTML` 直插未消毒内容。
4. 所有状态（空闲 / 索引中 / 生成中 / 错误 / 配额触顶）必须有明确的视觉反馈与微交互。

### 3.4 商业化与安全红线
1. 免费版：**上限 50 个文件、禁用 PDF**；Pro 版通过 License 解锁，License 本地离线校验。
2. 拖拽文件时全屏高亮遮罩，固定文案「数据仅本地处理」。
3. API Key **仅本地存储**（OS Keychain 优先，降级为加密文件）；严禁明文、严禁入库、严禁出现在日志与错误信息中。
4. 隐私不出域：零遥测；网络出口仅限用户自行配置的 LLM / Embedding 端点。

---

## 体系四：资产与上下文同步协议

1. **唯一真相源**：`PROJECT_RULES.md`（规则）、`docs/SRS_v1.0.md`（需求）、`docs/TEST_STRATEGY.md`（测试策略）。任何代码变更，必须先更新对应文档条目。
2. **Git 同步**：每完成一个原子任务执行一次 Conventional Commit（如 `feat(req-ing-001): 支持拖拽导入 md/txt`）；提交前 `cargo check` 必须全绿。细则见 `docs/ASSET_MANAGEMENT.md`。
3. **上下文重建**：新会话必须先执行 `git log --oneline -20` 并通读三件套与 `docs/ASSET_MANAGEMENT.md`，再读取当前文件真实内容。**严禁凭「记忆」写代码。**
4. **变更审查**：每次修改后向开发者提供变更摘要，建议以 hunk diff 形式逐块审查。

---

## 附则：规则修订程序

1. 任何角色可提出修订建议；由 TPM 发起评审，Chief Architect 与 QA Lead 会签后生效。
2. 修订生效后文档版本号 +0.1.0，并在下表登记。

| 版本 | 日期 | 修订摘要 | 批准 |
| --- | --- | --- | --- |
| v1.0.0 | 2026-07-24 | 创世版：四大纪律体系确立 | TPM |
| v1.1.0 | 2026-07-25 | 新增体系三 3.0 三条铁律：网络代理规范、TDD 驱动开发、代码注释规范 | TPM / Chief Architect |
| v1.2.0 | 2026-07-25 | 新增铁律四：真实数据测试（真实语料、混沌测试、LLM 环境变量、语料自动化） | TPM / QA Lead |
| v1.3.0 | 2026-07-25 | 新增铁律五：生产级代码质量（零 unwrap / 零 panic / 零裸溢出，CI deny lint 全覆盖） | TPM / Chief Architect |
| v1.4.0 | 2026-07-25 | 新增铁律六/七/八：最新版本依赖原则、文档实时同步操作清单、开发决策必须互联网搜索 | TPM / Chief Architect |
| v1.5.0 | 2026-08-04 | 新增铁律九~十四：Rust 生态优先、最前沿技术追踪、GitHub 发布同步、Session 上下文全自动管理、Tailwind 样式规范、SOLID 原则 | TPM / Chief Architect |
| v1.6.0 | 2026-08-08 | 新增铁律十五：Spec-driven Development 规格驱动开发（先写规格 REQ+AC+TC 再写代码，测试从 AC 派生） | TPM / Chief Architect |
| v1.7.0 | 2026-08-08 | 铁律规则统一整合到 `.catpaw/rules/system-constraints.md`，删除 `.catpaw/rules/iron-rules.md`，本节改为速查表+项目特定补充 | TPM / Chief Architect |
| v1.8.0 | 2026-08-08 | 铁律扩展至 20 条：新增铁律十六~二十（AI 代码审查 / 结构化日志 / LLM 安全 / 错误弹性 / Agent 行为规范），增强铁律二/五/七。来源：vibe-coding-ai-rules / SoloDawn / OWASP / agent-best-practices 等业界最新实践 | TPM / Chief Architect |
| v1.9.0 | 2026-08-08 | 铁律扩展至 21 条：3 轮深度调研后新增铁律二十一 RAG 安全防护，增强铁律二/五/六/十七/十八。来源：OWASP GenAI LLM Top 10 2026 / cargo-miri / OpenSSF Scorecard / cargo-fuzz | TPM / Chief Architect |
| v2.0.0 | 2026-08-08 | 铁律二新增「Vibe Coding 全自动化测试体系」——分层测试门禁（L0~L4）+ AI Agent 行为规则 + 测试工具链清单（10 工具）。来源：template-agenticide-vibecoding / vibe-coding-ai-rules / cargo-nextest / cargo-mutants / insta / cargo-llvm-cov / wiremock | TPM / Chief Architect |
