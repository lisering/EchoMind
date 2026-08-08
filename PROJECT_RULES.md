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

### 3.0 工程纪律铁律（v1.6.0 新增铁律十五 Spec-driven Development，完整规则见 `.catpaw/rules/iron-rules.md`）

#### 铁律一：网络代理规范（仅限开发调研，软件本身禁止代理）

**⚠️ 核心原则：代理仅用于 AI Agent 的开发调研活动（搜索资料、下载依赖、抓取文档）。EchoMind 软件本身运行时（LLM API 调用、模型下载、Web 搜索等用户面向的网络请求）绝对禁止使用代理。**

**✅ 允许使用代理的场景（开发调研）**：
- `cargo build` / `cargo update` 下载 crates
- `npm install` 下载 E2E 依赖
- HuggingFace 模型下载（仅限开发环境）
- `web_fetch` / `curl` 抓取外部文档
- 技术选型调研（GitHub API / crates.io 搜索）

开发环境中若遇无法访问的地址（如 crates.io、huggingface.co 等），允许使用本地代理：

```bash
export https_proxy=http://127.0.0.1:7890 http_proxy=http://127.0.0.1:7890 all_proxy=socks5://127.0.0.1:7890
```

**用完后必须立即 unset，防止代理泄露或影响其他任务：**

```bash
unset https_proxy http_proxy all_proxy
```

**🚫 禁止使用代理的场景（软件运行时）**：
- LLM API 调用（`OpenAIProvider` 的 reqwest 请求）
- 模型下载（`ModelManager` 下载 GGUF 模型）
- fastembed ONNX 模型下载（运行时首次加载）
- Web 搜索（`DuckDuckGoProvider`）
- 任何其他用户面向的网络请求

**代码强制要求**：所有 `reqwest::Client::builder()` 必须加 `.no_proxy()` 确保软件直连，不受 `https_proxy` / `http_proxy` 环境变量影响。

#### 铁律二：TDD 驱动开发

1. **所有功能必须 TDD 驱动**：先写红灯测试，再写实现代码使其变绿，严禁「先实现后补测试」。
2. **用例必须有效且有意义**：禁止写永远通过的空断言（如 `assert!(true)`）；每个用例必须明确对应一条 AC（验收标准），断言聚焦行为而非实现细节。
3. **根据开发进度持续调整**：随着需求演进和代码重构，测试用例必须同步更新。废弃的用例及时删除，新增的分支及时覆盖。测试代码与生产代码同等重要，接受同等质量的 review。
4. **测试左移纪律**（体系二阶段 2）：QA 在开发写代码之前介入编写测试骨架，此时测试必须失败（红灯），失败原因必须是「未实现」而非「测试本身错误」。

#### 铁律四：真实数据测试

1. **测试必须覆盖真实世界语料**：禁止仅用 toy data（如 `b"hello"`）作为唯一测试输入。L2 及以上测试必须包含真实公开语料（GitHub README、arXiv 论文 PDF 等）和极端边缘数据（混合编码、恶意 XSS、空文件、深套目录），确保系统在野蛮场景下不崩溃。
2. **混沌测试常态化**：每次大版本迭代必须执行混沌测试集（`chaos_tests` 模块），覆盖抗压、容错、安全、格式欺骗、海量并发等野蛮场景。混沌测试用例与功能测试同等重要，纳入 CI 门禁。
3. **真实 LLM 联调测试**：需要真实 LLM 的集成测试通过环境变量注入凭据，严禁硬编码 API Key。环境变量配置见 `docs/TEST_STRATEGY.md` §5。测试中使用的 LLM 配置如下：
   - API Key → 环境变量 `ANTHROPIC_AUTH_TOKEN`
   - Base URL → 环境变量 `BASE_URL`
   - Model → 环境变量 `CLAUDE_CODE_SUBAGENT_MODEL`
4. **语料生成自动化**：测试 fixtures 由 `tests/fixtures/generate_corpus.py` 自动生成，确保可复现。网络获取失败时使用本地 fallback，保证 CI 离线可跑。

#### 铁律三：代码注释规范

1. **Rust**：充分发挥 Rust 强大的文档注释系统。所有 `pub` 项必须有 `///` 文档注释，说明用途、参数、返回值、错误条件与约束。模块级 `//!` 注释说明模块职责与架构约束。复杂逻辑用行内 `//` 注释说明「为什么」而非「是什么」。`cargo doc` 生成的文档必须可读且自解释。
2. **前端 JavaScript**：所有函数必须有 JSDoc 注释（`/** ... */`），标注 `@param`、`@returns`、`@throws`。复杂状态机和事件流必须有注释说明状态迁移与数据流向。严禁「无注释魔法函数」。
3. **注释与代码同步**：修改代码时必须同步更新注释。过时注释比没有注释更危险。Code review 时注释与代码不一致视为 review 阻塞项。
4. **杜绝屎山**：每个函数单一职责，命名自解释。超过 50 行的函数必须拆分。重复代码必须抽取。任何「暂时这样写，以后再改」的代码必须在注释中标记 `// TODO(req-xxx): 说明` 并关联需求编号。

#### 铁律五：生产级代码质量（零 unwrap / 零 panic / 零裸溢出）

> 本铁律将 §3.2.1 的 unwrap 禁令升格为独立铁律，并扩展至 panic 与整数溢出，适用于全部 `crates/` 生产代码（测试代码豁免）。

1. **零 unwrap / 零 expect**：生产代码严禁出现 `.unwrap()`、`.expect()`、`panic!()`、`unreachable!()`、`todo!()`、`unimplemented!()`。所有可能失败的操作必须经 `?` 传播或 `match` 显式处理，错误类型用 `thiserror`（领域层）或 `anyhow`（组装层）定义。CI 以 `clippy::unwrap_used = "deny"`、`clippy::expect_used = "deny"`、`clippy::panic = "deny"`、`clippy::unreachable = "deny"`、`clippy::todo = "deny"`、`clippy::unimplemented = "deny"` 强制。
2. **零裸整数溢出**：涉及用户输入或外部数据的算术运算（文件大小、配额计数、分块偏移等）必须使用 `checked_*` / `saturating_*` 系列方法或显式范围校验，严禁依赖默认 wrapping 行为。`clippy::arithmetic_side_effects = "deny"` 在高危 crate 启用。
3. **零 unwrap 链式调用**：禁止 `.map(|x| x.unwrap())`、`if let Some(x) = opt.unwrap()` 等变体绕过 lint。`Option::unwrap` 与 `Result::unwrap` 一视同仁，均以 `ok_or` / `context` / `map_err` 替代。
4. **错误信息安全**：错误消息严禁包含 API Key、文件路径中的用户名、数据库连接串等敏感信息。`anyhow::Context` 的描述须聚焦「做了什么」而非「内部细节」。
5. **防御性编程**：对外部输入（文件内容、网络响应、Tauri 参数）执行长度截断、类型校验与字符边界检查后再使用。`String::from_utf8` 失败时用 `from_utf8_lossy` 容错，不得 panic。
6. **CI 门禁自动化**：每次 PR 必须通过 `cargo clippy --all-targets -- -D warnings`（含上述全部 deny lint），任何违反铁律五的代码不得合入主干。新增 crate 时须在 `Cargo.toml` 继承 workspace lints（`[lints] workspace = true`）。

#### 铁律六：最新版本依赖原则

**所有用到的技术和依赖必须使用最新稳定版本。** 面对新版本时，必须去官网查阅 API 文档和迁移指南，不能凭记忆假设 API 用法。

核心技术栈版本要求：

| 技术 | 最低版本 | 官方文档 |
| --- | --- | --- |
| Rust Edition 2024 | Rust 1.85+ | https://www.rust-lang.org/ |
| Tauri | v2.x | https://v2.tauri.app/ |
| fastembed | 最新 crates.io 版本 | https://crates.io/crates/fastembed |
| rusqlite | 最新 crates.io 版本 | https://crates.io/crates/rusqlite |
| reqwest | 最新 crates.io 版本 | https://crates.io/crates/reqwest |
| tokio | 最新 crates.io 版本 | https://crates.io/crates/tokio |
| pulldown-cmark | 最新 crates.io 版本 | https://crates.io/crates/pulldown-cmark |
| thiserror | 最新 crates.io 版本 | https://crates.io/crates/thiserror |
| Playwright | 最新 npm 版本 | https://playwright.dev/ |
| Tailwind CSS | v4+ | https://tailwindcss.com/ |

版本更新规则：

1. **每次开始新功能开发前**，运行 `cargo update --dry-run` 和 `npm outdated` 检查是否有新版本。
2. **遇到 API 破坏性变更**时，必须查阅官方迁移指南（Migration Guide），不能凭旧记忆编写代码。
3. **新增依赖**时，必须在 `crates.io` 或 `npm` 确认最新版本号，不能手动猜测版本。
4. **构建失败**时，首先考虑是否是依赖版本过旧导致的不兼容。
5. **技术选型**时，优先选择官方维护活跃、文档完善的最新方案。

#### 铁律七：文档实时同步（大模型开发友好度保障）

**代码变更必须同步更新文档，文档与代码的脱节是项目质量的最大威胁。**

同步规则：

| 代码变更类型 | 必须同步更新的文档 | 时限 |
| --- | --- | --- |
| 新增/修改 IPC 命令 | `docs/api/ipc-commands.md` | 同一 PR 内 |
| 新增/修改数据库表 | `docs/api/database-schema.md` | 同一 PR 内 |
| 新增/修改 trait 方法 | `docs/api/code-contracts.md` | 同一 PR 内 |
| 新增/修改数据模型 | `docs/api/code-contracts.md` | 同一 PR 内 |
| 新增/修改 AppState 字段 | `docs/api/code-contracts.md` | 同一 PR 内 |
| 新增前端 JS 文件 | `docs/api/frontend-modules.md` | 同一 PR 内 |
| 新增 settings 键 | `docs/api/settings-reference.md` | 同一 PR 内 |
| 新增 Tauri 事件 | `docs/api/settings-reference.md` | 同一 PR 内 |
| 功能实现/补齐 | `docs/SRS_v1.0.md` 对应 REQ 状态 | 同一 PR 内 |
| 新增架构决策 | `docs/architecture/adr/` 新建 ADR | 同一 PR 内 |

检查清单（每次 PR 提交前）：

1. [ ] 是否新增了 IPC 命令？→ 更新 `ipc-commands.md` 并在 `invoke_handler` 注册
2. [ ] 是否修改了数据库结构？→ 更新 `database-schema.md`
3. [ ] 是否新增了 trait 方法或数据模型？→ 更新 `code-contracts.md`
4. [ ] 是否新增了前端文件？→ 更新 `frontend-modules.md`
5. [ ] 是否新增了设置键或事件？→ 更新 `settings-reference.md`
6. [ ] 功能状态是否变化？→ 更新 SRS 对应 REQ 状态
7. [ ] 是否有重大架构决策？→ 新建 ADR

大模型开发文档优先级：

AI Agent 在开发前必须先查阅以下文档（按优先级）：

1. `PROJECT_RULES.md` — 找回纪律约束（最高优先级）
2. `docs/api/code-contracts.md` — 了解 trait 签名和数据模型（**第一技术参考**）
3. `docs/api/ipc-commands.md` — 确认命令参数和返回值
4. `docs/api/database-schema.md` — 确认表名/列名
5. `AGENTS.md` — 架构速查和已知陷阱
6. `.catpaw/rules/iron-rules.md` — 铁律规则（跨会话持久化）

#### 铁律八：开发决策必须互联网搜索（杜绝闭门造车）

**在进行产品决策、技术选型、架构设计、头脑风暴等开发活动时，必须先通过互联网搜索获取最新最权威的资料，不能凭记忆做判断。**

适用场景：

- 技术选型（选择 crate / npm 包 / 框架 / API）
- 竞品分析（了解同类产品的最新功能和架构）
- 架构设计（查找最新最佳实践和设计模式）
- 版本兼容性确认（API 是否有破坏性变更）
- 安全漏洞检查（依赖是否有已知 CVE）
- 产品决策（行业趋势、用户需求验证）

搜索方法（按可靠性排序）：

1. **`curl` + 代理**（最可靠）：
   ```bash
   export https_proxy=http://127.0.0.1:7890 http_proxy=http://127.0.0.1:7890 all_proxy=socks5://127.0.0.1:7890
   curl -sL "https://api.github.com/search/repositories?q=关键词&sort=stars&order=desc&per_page=10"
   curl -sL "https://crates.io/api/v1/crates?q=关键词&sort=downloads"
   curl -sL "https://raw.githubusercontent.com/用户/仓库/分支/文件路径"
   ```
2. **`web_fetch` 工具**（部分网站可用，不稳定）
3. **浏览器工具**（Google/DuckDuckGo 搜索可能超时或遇到 CAPTCHA）

搜索规则：

1. **每次技术选型前**：在 GitHub API / crates.io 搜索同类项目，对比 stars、活跃度、最近更新。
2. **每次竞品分析时**：搜索同类开源项目，读取 README 了解最新功能定位。
3. **每次 API 使用前**：查阅最新官方文档，不凭记忆假设 API 用法（铁律六也要求此点）。
4. **每次架构决策时**：搜索行业最佳实践和最新趋势验证方案。
5. **搜索结果必须引用来源**：在文档和决策中标注信息来源（URL / 项目名 / 调研日期）。
6. **DuckDuckGo 直接搜索会遇到 CAPTCHA**：优先使用 GitHub API 间接搜索或直接访问已知 URL。

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
| v1.5.0 | 2026-08-04 | 新增铁律九~十四：Rust 生态优先、最前沿技术追踪、GitHub 发布同步、Session 上下文全自动管理、Tailwind 样式规范、SOLID 原则（详见 `.catpaw/rules/iron-rules.md`） | TPM / Chief Architect |
| v1.6.0 | 2026-08-08 | 新增铁律十五：Spec-driven Development 规格驱动开发（先写规格 REQ+AC+TC 再写代码，测试从 AC 派生） | TPM / Chief Architect |
