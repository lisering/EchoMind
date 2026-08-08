# 🔒 EchoMind 铁律规则（跨会话持久化）

> 本文件为绝对强制规则，每次新建对话都必须严格遵守，不可跳过、不可遗忘。
> 完整规则定义见 `PROJECT_RULES.md` 体系三 §3.0。

---

## 铁律一：网络代理设置（仅限开发调研，软件本身禁止代理）

**⚠️ 核心原则：代理仅用于 AI Agent 的开发调研活动（搜索资料、下载依赖、抓取文档）。EchoMind 软件本身运行时（LLM API 调用、模型下载、任何用户面向的网络请求）绝对禁止使用代理。**

### ✅ 允许使用代理的场景（开发调研活动）

当 AI Agent 进行以下开发活动遇到无法访问的资源时，可以使用代理：

```bash
export https_proxy=http://127.0.0.1:7890
export http_proxy=http://127.0.0.1:7890
export all_proxy=socks5://127.0.0.1:7890
```

- `cargo build` / `cargo update` 下载 crates 时
- `npm install` 下载 E2E 依赖时
- HuggingFace 模型下载（fastembed ONNX 模型，**仅限开发环境**）
- `web_fetch` / `curl` 抓取外部文档时
- 技术选型调研（GitHub API / crates.io 搜索）
- 任何 AI Agent 执行的开发相关网络命令

### 🚫 禁止使用代理的场景（EchoMind 软件运行时）

**EchoMind 软件本身的网络请求绝对不能走代理。** 软件面向终端用户，用户不一定有代理环境，软件必须能在无代理环境下正常工作：

- **LLM API 调用**（`OpenAIProvider` 的 `reqwest` 请求）：用户配置的 `base_url` 必须直连，不能受 `https_proxy` / `http_proxy` 环境变量影响
- **模型下载**（`ModelManager` 下载 GGUF 模型）：用户从 HuggingFace 下载模型时直连
- **fastembed ONNX 模型下载**（运行时首次加载）：用户使用软件时触发的模型下载直连
- **Web 搜索**（`DuckDuckGoProvider`）：用户使用 Web 搜索功能时直连
- **任何其他用户面向的网络请求**

### 规则

1. **开发调研时**：如果命令可能涉及网络访问，默认在命令前加 `export https_proxy=http://127.0.0.1:7890 http_proxy=http://127.0.0.1:7890 all_proxy=socks5://127.0.0.1:7890 &&`
2. **首次执行网络命令失败时**（超时/连接拒绝），立即重试并加上代理
3. **代理地址固定为** `127.0.0.1:7890`，除非用户明确通知更改
4. **用完后必须立即 unset，防止代理泄露或影响其他任务：**
   ```bash
   unset https_proxy http_proxy all_proxy
   ```
5. **软件代码中绝对不能读取或使用代理环境变量**：`reqwest::Client::builder()` 不设置 `.proxy()`，不读取 `https_proxy` / `http_proxy` 环境变量。如果 `reqwest` 默认读取了环境变量（reqwest 默认行为），必须在代码中显式禁用：
   ```rust
   reqwest::Client::builder()
       .no_proxy() // 🚫 禁止代理，确保软件直连
       .build()
   ```

---

## 铁律二：TDD 驱动开发

1. **所有功能必须 TDD 驱动**：先写红灯测试，再写实现代码使其变绿，严禁「先实现后补测试」。
2. **用例必须有效且有意义**：禁止写永远通过的空断言（如 `assert!(true)`）；每个用例必须明确对应一条 AC（验收标准），断言聚焦行为而非实现细节。
3. **根据开发进度持续调整**：随着需求演进和代码重构，测试用例必须同步更新。废弃的用例及时删除，新增的分支及时覆盖。测试代码与生产代码同等重要，接受同等质量的 review。
4. **测试左移纪律**（体系二阶段 2）：QA 在开发写代码之前介入编写测试骨架，此时测试必须失败（红灯），失败原因必须是「未实现」而非「测试本身错误」。

---

## 铁律三：代码注释规范

1. **Rust**：充分发挥 Rust 强大的文档注释系统。所有 `pub` 项必须有 `///` 文档注释，说明用途、参数、返回值、错误条件与约束。模块级 `//!` 注释说明模块职责与架构约束。复杂逻辑用行内 `//` 注释说明「为什么」而非「是什么」。`cargo doc` 生成的文档必须可读且自解释。
2. **前端 JavaScript**：所有函数必须有 JSDoc 注释（`/** ... */`），标注 `@param`、`@returns`、`@throws`。复杂状态机和事件流必须有注释说明状态迁移与数据流向。严禁「无注释魔法函数」。
3. **注释与代码同步**：修改代码时必须同步更新注释。过时注释比没有注释更危险。Code review 时注释与代码不一致视为 review 阻塞项。
4. **杜绝屎山**：每个函数单一职责，命名自解释。超过 50 行的函数必须拆分。重复代码必须抽取。任何「暂时这样写，以后再改」的代码必须在注释中标记 `// TODO(req-xxx): 说明` 并关联需求编号。

---

## 铁律四：真实数据测试

1. **测试必须覆盖真实世界语料**：禁止仅用 toy data（如 `b"hello"`）作为唯一测试输入。L2 及以上测试必须包含真实公开语料（GitHub README、arXiv 论文 PDF 等）和极端边缘数据（混合编码、恶意 XSS、空文件、深套目录），确保系统在野蛮场景下不崩溃。
2. **混沌测试常态化**：每次大版本迭代必须执行混沌测试集（`chaos_tests` 模块），覆盖抗压、容错、安全、格式欺骗、海量并发等野蛮场景。混沌测试用例与功能测试同等重要，纳入 CI 门禁。
3. **真实 LLM 联调测试**：需要真实 LLM 的集成测试通过环境变量注入凭据，严禁硬编码 API Key。环境变量配置见 `docs/TEST_STRATEGY.md` §5。
   - API Key → 环境变量 `ANTHROPIC_AUTH_TOKEN`
   - Base URL → 环境变量 `BASE_URL`
   - Model → 环境变量 `CLAUDE_CODE_SUBAGENT_MODEL`
4. **语料生成自动化**：测试 fixtures 由 `tests/fixtures/generate_corpus.py` 自动生成，确保可复现。网络获取失败时使用本地 fallback，保证 CI 离线可跑。

---

## 铁律五：生产级代码质量（零 unwrap / 零 panic / 零裸溢出）

1. **零 unwrap / 零 expect**：生产代码严禁出现 `.unwrap()`、`.expect()`、`panic!()`、`unreachable!()`、`todo!()`、`unimplemented!()`。所有可能失败的操作必须经 `?` 传播或 `match` 显式处理，错误类型用 `thiserror`（领域层）或 `anyhow`（组装层）定义。CI 以 deny lint 强制。
2. **零裸整数溢出**：涉及用户输入或外部数据的算术运算（文件大小、配额计数、分块偏移等）必须使用 `checked_*` / `saturating_*` 系列方法或显式范围校验，严禁依赖默认 wrapping 行为。
3. **零 unwrap 链式调用**：禁止 `.map(|x| x.unwrap())`、`if let Some(x) = opt.unwrap()` 等变体绕过 lint。`Option::unwrap` 与 `Result::unwrap` 一视同仁，均以 `ok_or` / `context` / `map_err` 替代。
4. **错误信息安全**：错误消息严禁包含 API Key、文件路径中的用户名、数据库连接串等敏感信息。`anyhow::Context` 的描述须聚焦「做了什么」而非「内部细节」。
5. **防御性编程**：对外部输入（文件内容、网络响应、Tauri 参数）执行长度截断、类型校验与字符边界检查后再使用。`String::from_utf8` 失败时用 `from_utf8_lossy` 容错，不得 panic。
6. **CI 门禁自动化**：每次 PR 必须通过 `cargo clippy --all-targets -- -D warnings`（含上述全部 deny lint），任何违反铁律五的代码不得合入主干。新增 crate 时须在 `Cargo.toml` 继承 workspace lints（`[lints] workspace = true`）。

---

## 铁律六：最新版本依赖原则

**所有用到的技术和依赖必须使用最新稳定版本。** 面对新版本时，必须去官网查阅 API 文档和迁移指南，不能凭记忆假设 API 用法。

核心技术栈版本要求：

| 技术 | 最低版本 | 官方文档 |
|---|---|---|
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

1. **每次开始新功能开发前**，运行 `cargo update --dry-run` 和 `npm outdated` 检查是否有新版本
2. **遇到 API 破坏性变更**时，必须查阅官方迁移指南（Migration Guide），不能凭旧记忆编写代码
3. **新增依赖**时，必须在 `crates.io` 或 `npm` 确认最新版本号，不能手动猜测版本
4. **构建失败**时，首先考虑是否是依赖版本过旧导致的不兼容
5. **技术选型**时，优先选择官方维护活跃、文档完善的最新方案

版本检查命令：

```bash
# Rust 依赖更新检查
cargo update --dry-run

# npm 依赖更新检查
cd e2e-tests && npm outdated

# 查看当前 Cargo.toml 中各依赖版本
grep -A1 "\[dependencies\]" crates/*/Cargo.toml
```

---

## 铁律七：文档实时同步（大模型开发友好度保障）

**代码变更必须同步更新文档，文档与代码的脱节是项目质量的最大威胁。**

同步规则：

| 代码变更类型 | 必须同步更新的文档 | 时限 |
|---|---|---|
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
6. `.catpaw/rules/iron-rules.md` — 铁律规则（本文件）

---

## 铁律八：开发决策必须互联网搜索（杜绝闭门造车）

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

1. **每次技术选型前**：在 GitHub API / crates.io 搜索同类项目，对比 stars、活跃度、最近更新
2. **每次竞品分析时**：搜索同类开源项目，读取 README 了解最新功能定位
3. **每次 API 使用前**：查阅最新官方文档，不凭记忆假设 API 用法（铁律六也要求此点）
4. **每次架构决策时**：搜索行业最佳实践和最新趋势验证方案
5. **搜索结果必须引用来源**：在文档和决策中标注信息来源（URL / 项目名 / 调研日期）
6. **DuckDuckGo 直接搜索会遇到 CAPTCHA**：优先使用 GitHub API 间接搜索或直接访问已知 URL

---

## 铁律九：Rust 生态优先原则

**所有功能实现必须优先使用 Rust 原生生态（crates.io / GitHub Rust 项目），除非经过充分调研确认 Rust 生态无可用方案。**

### 核心原则

EchoMind 是纯 Rust + Tauri 桌面应用，所有核心能力（文档解析、向量化、推理、检索、加密）必须在 Rust 生态内闭环。引入非 Rust 依赖（Python 脚本、C 库、外部进程）需要经过严格论证。

### Rust 生态能力清单（已验证可用）

| 能力 | Rust Crate / 项目 | 说明 |
|---|---|---|
| 文本分块 | `tiktoken-rs` | OpenAI cl100k_base BPE 编码器 |
| 语义分块 | 自研 `SemanticSplitter` | 段落→句子→子句递归，支持中文标点 |
| Markdown 解析 | `pulldown-cmark` | CommonMark 标准解析 |
| PDF 解析 | `lopdf` | 纯 Rust PDF 文本提取 |
| Word 解析 | `zip` + XML | .docx 是 ZIP 格式，解压后提取 XML 文本 |
| Excel 解析 | `calamine` | .xlsx 表格数据读取 |
| OCR | `ocrs` + `rten` | 纯 Rust PaddleOCR PP-OCRv4 |
| VLM | `ort` + `tokenizers` | ONNX Runtime 视觉语言模型 |
| 文本嵌入 | `fastembed` | ONNX 模型本地推理 |
| 重排序 | `fastembed::TextRerank` | bge-reranker-base Cross-Encoder |
| 向量存储 | `rusqlite` + `r2d2` | SQLite + 连接池 |
| 加密 | `rusqlite/bundled-sqlcipher` | AES-256-CBC 透明加密 |
| HTTP | `reqwest` | OpenAI 兼容 SSE 流式 |
| 并发 | `tokio` + `async-channel` | 异步运行时 + 有界通道背压 |

### 决策流程

1. **需求出现** → 先在 crates.io / GitHub 搜索 Rust 实现
2. **找到候选** → 检查 stars、活跃度、最近更新、文档质量
3. **多方案对比** → 优先选择纯 Rust 实现（无 C 依赖），其次选择有 Rust binding 的成熟 C 库
4. **Rust 无方案** → 记录调研过程，方可考虑非 Rust 替代（如调用外部 CLI）
5. **新增依赖** → 在 `Cargo.toml` 中记录用途，更新铁律六的版本表

### 参考项目

- **`/Users/john/rs-pro`** — EchoMind 的成熟版本，包含 `SemanticSplitter`、`CrossEncoderReranker`、`GraphHybridRetriever`、`HyDERewriter` 等全部 Rust 实现，是移植首选来源

---

## 铁律十：最前沿技术追踪（持续升华）

**开发决策不能止步于「能用」，必须追踪并吸收业界最前沿的技术方案，持续提升产品竞争力。**

### 必须追踪的前沿技术领域

| 领域 | 前沿技术 | 来源 |
|---|---|---|
| 文档分块 | Semantic Chunking / Late Chunking / Contextual Retrieval | Anthropic / Jina AI / LangChain |
| 检索增强 | HyDE / Multi-Query / RRF Fusion / Reranking | LangChain / LlamaIndex / Cohere |
| Agentic RAG | ReAct 多步推理 / 工具调用 / 自主决策 | LangGraph / CrewAI |
| 知识图谱 | 实体抽取 / 多跳查询 / 图谱增强检索 | LlamaIndex KG / Microsoft GraphRAG |
| Embedding 模型 | bge-m3 / multilingual-e5 / Nomic Embed | HuggingFace SOTA |
| 本地推理 | ONNX Runtime / Candle / rten | fastembed / HuggingFace |
| 多模态 | CLIP / Florence-2 / LLaVA | OpenAI / Microsoft |

### 追踪方法

1. **GitHub Trending**：每周浏览 [github.com/trending?since=weekly&language=rust](https://github.com/trending?since=weekly&language=rust)
2. **HuggingFace SOTA**：关注 [huggingface.co/models](https://huggingface.co/models) 排行榜
3. **关键论文库**：
   - Anthropic Engineering Blog — Contextual Retrieval 等
   - arXiv cs.CL / cs.IR — RAG / Embedding 前沿论文
   - Jina AI Blog — Late Chunking 等
4. **竞品监控**：定期搜索 GitHub RAG 项目（LangChain / LlamaIndex / AnythingLLM / Dify），读取最新 release notes

### 融入流程

1. **调研阶段**：每次新功能开发前，搜索该领域最新论文和开源实现
2. **评估阶段**：评估前沿技术是否适合本地优先、纯 Rust 架构
3. **移植阶段**：优先从 `rs-pro` 移植已验证的 Rust 实现，不足部分自研
4. **验证阶段**：用真实文档测试，对比改进前后的检索准确率

### 已吸收的前沿技术（来源记录）

| 技术 | 来源 | 移植状态 |
|---|---|---|
| Semantic Chunking | LangChain `MarkdownHeaderTextSplitter` / LlamaIndex `SentenceSplitter` | ✅ `rs-pro` 已实现 `SemanticSplitter` |
| Cross-Encoder Reranking | Anthropic Contextual Retrieval / BGE | ✅ `rs-pro` 已实现 `CrossEncoderReranker` |
| RRF Hybrid Retrieval | LangChain / LlamaIndex | ✅ `rs-pro` 已实现 `GraphHybridRetriever` |
| HyDE Query Rewriting | LangChain / 论文 arXiv:2212.10496 | ✅ `rs-pro` 已实现 `HyDERewriter` |
| Contextual Retrieval | Anthropic Engineering Blog (2024) | 🔄 待实现 |
| Late Chunking | Jina AI Blog (2024) | 🔄 待评估（需长上下文 embedding 模型） |

---

## 铁律十一：GitHub 发布同步规则（双仓库工作流）

**⚠️ 绝对铁律：根目录下的 `EchoMind/EchoMind/` 只放推到 GitHub 去的文件——任何和打包编译无关的文件都不要放到这里。开发在项目根目录进行，每次源代码修改后必须立即同步到发布仓库，否则 GitHub 上的代码就是过时的。**

### 工作流架构

```
项目根目录 /Users/john/freesoft/EchoMind/          ← 开发工作区（含 docs/ tests/ e2e-tests/ 等开发资产）
  └── EchoMind/EchoMind/                            ← GitHub 发布仓库（仅含打包编译必需文件）
        └── git remote → github.com/xxx/EchoMind    ← 推送到 GitHub
```

- **开发**：在项目根目录正常编码、测试、运行 `cargo tauri dev`
- **发布**：`EchoMind/EchoMind/` 只包含打包编译必需的文件，不含开发资产
- **同步**：根目录源代码变更后，**必须立即**运行 `./sync-to-github.sh` 同步到 `EchoMind/EchoMind/`

### 同步时机（强制）

以下场景必须在完成修改后**立即**执行同步：
1. 修改了任何 `.rs` / `.html` / `.css` / `.js` / `.json` / `.toml` 源文件后
2. 新增或删除了源文件后
3. 修改了 `Cargo.toml` / `Cargo.lock` / `tauri.conf.json` 等配置文件后
4. 修改了 `ui/` 下的任何文件（`index.html`、`styles/`、`vendor/`）后
5. **每次对话结束前**，如果有源代码变更，必须运行同步脚本

### 同步规则

1. **任何源代码文件（`.rs`、`.html`、`.json`、`.toml`、`.css`、`.js`）修改后**，必须同步到 `EchoMind/EchoMind/` 下的对应路径
2. **新增源文件**必须同步到发布仓库并在 `EchoMind/EchoMind/` 中 `git add`
3. **删除源文件**必须在发布仓库中同步删除（`git rm`）
4. **`Cargo.toml` / `Cargo.lock` / `tauri.conf.json` 等配置文件**修改后必须同步
5. **`ui/vendor/` 目录**新增或更新前端库时必须同步
6. **`ui/styles/` 目录**修改后必须同步（CSS 文件被 `index.html` 引用）
7. **不同步的文件**：`docs/`、`tests/fixtures/`、`e2e-tests/`、`.catpaw/`、`.githooks/`、`PROJECT_RULES.md`、`ui/test-*.html`、`ui/tauri-stub.js`、`ui/src/` 等开发专属资产

### 同步方法

使用项目根目录下的 `sync-to-github.sh` 脚本一键同步：

```bash
# 在项目根目录执行
./sync-to-github.sh
```

脚本会自动将所有打包编译必需的文件从根目录同步到 `EchoMind/EchoMind/`，然后显示待提交状态。

### 提交前检查清单（每次推 GitHub 前）

1. [ ] 运行 `./sync-to-github.sh` 同步最新源代码
2. [ ] 在 `EchoMind/EchoMind/` 中运行 `cargo build` 确认编译通过
3. [ ] 检查 `git diff` 确认无敏感信息（API Key、本地路径、密钥等）
4. [ ] 检查无 `unwrap()` / `expect()` / `panic!()` 泄漏（铁律五）
5. [ ] `git add -A && git commit -m "desc" && git push`

---

## 铁律十二：Session 上下文全自动管理（保证开发连续性）

**⚠️ 绝对铁律：Session 上下文管理是 AI Agent 的自动行为，不需要人工介入。Agent 在每次会话开始时自动读取上下文，在每次会话结束时自动更新状态。`docs/sessions/` 是跨会话上下文管理的唯一权威目录。**

### Session 管理目录结构

```
docs/sessions/
├── README.md              ← 索引 + 使用指南
├── CURRENT_STATE.md       ← 当前项目状态快照（Agent 自动更新）
├── HANDOFF_TEMPLATE.md    ← 衔接提示词模板
├── LOG.md                 ← 会话时间线日志（Agent 自动追加）
└── archive/               ← 历史会话详情归档
```

### Agent 自动行为：会话开始时（强制）

**AI Agent 在开始任何 EchoMind 开发工作前，必须自动执行以下步骤，不需要用户提醒：**

1. **读取 `docs/sessions/CURRENT_STATE.md`** — 获取最新项目快照（测试数、已完成功能、待办任务、关键约束）
2. **浏览 `docs/sessions/LOG.md`** — 快速了解前序 session 时间线
3. **如有特定功能上下文需求** — 在 `docs/sessions/archive/` 中查找对应历史记录
4. **确认铁律规则** — 读取 `.catpaw/rules/iron-rules.md` 确认当前约束

**这是 Agent 的第一优先级行为，优先于任何开发任务。**

### Agent 自动行为：会话结束时（强制）

**AI Agent 在完成工作准备结束会话前，必须自动执行以下步骤，不需要用户提醒：**

1. **更新 `docs/sessions/CURRENT_STATE.md`** — 刷新测试数、已完成/待办列表、关键变更
2. **追加 `docs/sessions/LOG.md`** — 在时间线末尾添加本次 session 摘要（日期、功能、测试数、文件数、关键产出）
3. **创建 `docs/sessions/archive/` 归档** — 大型功能开发时创建详细归档记录（小型修改可跳过）
4. **如有源代码变更** — 同步更新 `docs/SOFTWARE_ASSET.md` 对应章节
5. **如有未完成功能** — 按 `docs/sessions/HANDOFF_TEMPLATE.md` 模板生成衔接提示词
6. **运行 `./sync-to-github.sh`** — 如有源代码变更（铁律十一）

**Agent 不得在未完成上述步骤的情况下结束会话。**

### 适用场景

以下场景**必须**产出衔接提示词（写入 `docs/sessions/HANDOFF_TEMPLATE.md` 模板）：
1. 功能开发未完成，需要下一 Session 继续
2. 分阶段实施的大型功能（如本地 LLM 引擎、多模态 PDF 等）
3. 调研规划类任务，实施阶段需要新 Session 承接
4. 任何涉及 3 个以上文件修改且无法在单 Session 完成的任务

### 衔接提示词模板

完整模板见 `docs/sessions/HANDOFF_TEMPLATE.md`。核心要素：

```
你正在继续 EchoMind 项目【功能名称】的开发。

## 上下文回溯
- Session 1: 【完成内容】（修改文件 / 测试数 / 关键决策）
- Session 2: 【完成内容】
- 当前: Session N

## 本 Session 目标
【一句话目标】

## 必读文档（按优先级）
1. docs/sessions/CURRENT_STATE.md — 项目当前状态
2. docs/SOFTWARE_ASSET.md — 软件资产全景
3. 原子规划文档 — 本 Session 章节
4. 已完成的参考代码
5. 铁律规则

## 精确变更清单
| 文件 | 变更类型 | 内容 |

## TDD 测试要求
- 测试 ID 前缀：TC-XXX-
- 先红灯后绿灯

## 验证命令
## 完成后必须执行
1. 更新 docs/sessions/CURRENT_STATE.md
2. 追加 docs/sessions/LOG.md
3. 产出下一 Session 衔接提示词
4. ./sync-to-github.sh
```

### 衔接规则

1. **每个 Session 结束时**：Agent 自动更新 `CURRENT_STATE.md` + 追加 `LOG.md`
2. **衔接提示词存储**：写入 `docs/sessions/HANDOFF_TEMPLATE.md` 或原子规划文档对应章节
3. **下一 Session 启动**：Agent 自动读取 `CURRENT_STATE.md` 恢复上下文，无需用户提供衔接提示词
4. **上下文回溯**：衔接提示词必须列出所有已完成 Session 的摘要（修改了哪些文件、多少测试通过、关键决策）
5. **精确变更清单**：下一 Session 要修改的每个文件、每个函数、每个测试必须列出
6. **验证命令**：必须给出下一 Session 完成后的验证命令（cargo test / cargo clippy）
7. **依赖关系**：如果 Session 之间有依赖，必须在规划文档中画出依赖关系图

### 原子级规划文档要求

对于跨 Session 的大型功能，必须产出原子级规划文档（如 `docs/architecture/XXX_ATOMIC_PLAN.md`），包含：

1. **全局依赖关系图**：Session 之间的依赖关系
2. **每个 Session 的精确变更清单**：文件路径、函数签名、变更类型
3. **每个 Session 的 TDD 测试用例**：测试 ID、测试名、验证内容、对应 AC
4. **每个 Session 的验证命令**：cargo test / clippy / fmt
5. **每个 Session 的完成标志**：checkbox 列表
6. **每个 Session 的衔接提示词**：按模板填写

### 衔接提示词输出格式（铁律：单代码块一次拷贝）

**⚠️ 绝对铁律：衔接提示词必须放在一个完整的 Markdown 代码块（``` ... ```）中输出，用户只需点击一次「复制」按钮即可获取完整提示词内容。禁止将提示词拆分为多个代码块、禁止在代码块外散落提示词片段。**

格式要求：
1. **单个 ` ``` ` 代码块**：整个衔接提示词（从「你正在继续…」到最后的验证命令）必须在**同一个**三反引号代码块中
2. **自包含**：提示词内包含下一 Session 所需的全部信息（上下文回溯、目标、变更清单、TDD 要求、验证命令），用户不需要再查阅其他文件即可开始工作
3. **直接可用**：用户复制后直接粘贴到新对话即可启动下一 Session，无需任何手动拼接
4. **同时写入文件**：提示词除在对话中输出外，还必须同步写入原子规划文档对应 Session 的衔接小节（如 `docs/architecture/XXX_ATOMIC_PLAN.md` 的 `### S58→S59 衔接` 小节）

### Agent 自动检查清单（每次 Session 结束前，Agent 自动执行）

1. [ ] `docs/sessions/CURRENT_STATE.md` 是否已更新？（测试数、功能状态、待办列表）
2. [ ] `docs/sessions/LOG.md` 是否已追加本次 session 记录？
3. [ ] 如有源代码变更，`docs/SOFTWARE_ASSET.md` 是否已同步更新？
4. [ ] 功能是否已完成？如未完成 → 是否已产出衔接提示词？
5. [ ] 衔接提示词是否包含上下文回溯（所有已完成 Session 摘要）？
6. [ ] 衔接提示词是否包含精确变更清单（文件/函数/测试）？
7. [ ] 衔接提示词是否以**单个代码块**输出（一次拷贝即可获取完整内容）？
8. [ ] 原子规划文档是否已更新（标记已完成 Session）？
9. [ ] `./sync-to-github.sh` 是否已执行（如有源代码变更）？

> **以上步骤由 AI Agent 自动执行，无需用户提醒。Agent 不得在未完成上述步骤的情况下结束会话。**

---

## 铁律十三：前端样式必须使用 Tailwind 工具类 + CSS 变量（禁止冗余自定义 CSS）

**⚠️ 绝对铁律：项目已集成 Tailwind CSS（本地 JIT 构建 `vendor/tailwindcss.js`），并在 `tailwind.config` 中定义了全部设计 Token（颜色 / 字体 / 圆角 / 阴影 / 动效）。前端样式必须优先使用 Tailwind 工具类，不得编写可用 Tailwind 替代的自定义 CSS。**

### 样式编写规则

1. **优先使用 Tailwind 工具类**：在 JS 中创建 DOM 元素时，直接在 `className` 中写 Tailwind 工具类（如 `flex items-center gap-2 px-4 py-3 rounded-lg`），不要写自定义 CSS 类再在 `components.css` 中定义样式
2. **设计 Token 通过 Tailwind config 使用**：颜色用 `bg-surface-2`、`text-text-primary`、`border-line`、`text-accent` 等（定义在 `tailwind.config.theme.extend.colors`），不要硬编码 `#1C1C20` 或 `var(--surface-2)`
3. **CSS 变量作为 Token 单一来源**：`ui/styles/tokens.css` 中的 `:root` CSS 变量是设计 Token 的单一来源，`tailwind.config` 映射这些变量。需要新增 Token 时，先加 CSS 变量，再在 Tailwind config 中映射
4. **仅以下场景允许自定义 CSS**：
   - `@keyframes` 动画定义（Tailwind 的 `animate-*` 无法覆盖复杂关键帧）
   - 伪类/伪元素选择器（`:hover`、`::before`、`::selection` 等 Tailwind 无法表达的）
   - 父状态选择器（如 `.editing .child`，Tailwind 的 `group:` 无法表达任意父类名）
   - `user-select`、`-webkit-` 前缀等浏览器特定属性
   - 第三方库样式覆盖（`marked.js`、`highlight.js`、`KaTeX` 等）
   - CSS 变量定义（`tokens.css`）
5. **禁止在 `components.css` / `typography.css` 中编写 Tailwind 能替代的规则**：如 `display: flex; gap: 8px; padding: 16px; border-radius: 12px;` 这类声明应直接用 `flex gap-2 p-4 rounded-md` 工具类
6. **JS 中动态创建元素的样式**：必须在 `className` 字符串中写 Tailwind 工具类，不要创建自定义 class 名再去 CSS 文件中定义

### 违规示例与修正

```javascript
// ❌ 违规：自定义 CSS 类 + components.css 中写样式
el.className = 'msg-edit-actions-below';
// components.css: .msg-edit-actions-below { display: flex; gap: 0; justify-content: flex-end; margin-top: 0; }

// ✅ 正确：直接用 Tailwind 工具类
el.className = 'flex gap-0 justify-end mt-0';
```

```javascript
// ❌ 违规：硬编码颜色
el.className = 'bg-[#1C1C20] text-[#F8FAFC]';

// ✅ 正确：用 Tailwind config 中定义的 Token
el.className = 'bg-surface-2 text-text-primary';
```

### 检查清单（每次前端修改前）

1. [ ] 要写的样式能否用 Tailwind 工具类表达？→ 能则用工具类，不写自定义 CSS
2. [ ] 颜色/字体/圆角是否用了 `tailwind.config` 中定义的 Token？→ 不硬编码十六进制
3. [ ] 是否在 `components.css` 中写了 `display: flex` / `padding` / `margin` / `border-radius` 等 Tailwind 能替代的声明？→ 移除，改用工具类
4. [ ] 仅在铁律十三 §4 列举的场景下编写自定义 CSS

---

## 铁律十四：SOLID 原则（面向对象设计五大原则）

**⚠️ 绝对铁律：EchoMind 采用六边形架构（Hexagonal Architecture），其核心思想与 SOLID 原则高度一致。所有新增代码必须自觉遵循 SOLID 五大原则，在 code review 时以 SOLID 违例视为阻塞项。**

### SRP：单一职责原则（Single Responsibility Principle）

**一个类/模块/函数只应有一个变更原因。**

1. **Rust crate 划分**：`crates/models`（领域契约）、`crates/prompt`（提示词构建）、`crates/compact`（上下文压缩）、`crates/core`（业务逻辑 + Port traits）、`crates/infra`（适配器实现）、`crates/tauri-app`（组装层）—— 每个 crate 有且只有一个职责
2. **文件级**：`chat.rs`（RAG 查询编排）、`agent.rs`（ReAct 多步推理）、`retriever.rs`（向量检索）、`splitter.rs`（文本分块）—— 每个文件只负责一件事
3. **函数级**：铁律三已要求「超过 50 行的函数必须拆分」；每个函数只做一件事，命名自解释
4. **检查**：如果一个类/函数同时处理「数据持久化」和「业务逻辑」或「UI 渲染」，则违反 SRP，必须拆分

### OCP：开闭原则（Open-Closed Principle）

**对扩展开放，对修改封闭。新增功能应通过新增代码实现，而非修改已有代码。**

1. **Port-Trait 扩展模式**：`crates/core/src/lib.rs` 定义 Port trait（`Loader`/`Embedder`/`Storage`/`LLMProvider`/`Retriever` 等），新增格式支持只需新增 trait 实现，不修改已有实现
2. **Pro 门控**：`#[cfg(feature = "pro")]` 条件编译实现 OCP —— Pro 功能通过新增模块实现，不修改 Free 代码路径
3. **前端模块化**：`ui/src/*.js` ES modules 互不依赖，新增 UI 面板只需新增 JS 文件 + 在 `main.js` 注册
4. **检查**：如果新增一个文档格式需要修改 `loader.rs` 中已有的 `MarkdownLoader` 代码，则违反 OCP；正确做法是新增 `XxxLoader` 实现 `Loader` trait

### LSP：里氏替换原则（Liskov Substitution Principle）

**子类型必须能完全替换其基类型而不破坏程序正确性。**

1. **Trait 实现**：所有 trait 实现者必须遵守 trait 契约（参数约束、返回值语义、错误条件）。例如 `Storage` trait 的 `vector_search()` 必须返回按相似度降序排列的结果，任何实现（`SqliteStorage`/`MockStorage`）都不能违反此约定
2. **`NoXxx` 占位实现**：`NoCompressor`（压缩禁用时）、`NoVlm`（VLM 禁用时）、`NoExecutor`（代码执行禁用时）必须与真实实现行为一致（返回空/默认值），保证调用方无感知切换
3. **默认实现**：Storage trait 方法有默认空操作实现（`default {}` / `default { vec![] }`），实现者可选择性覆写，但覆写后行为必须与默认语义一致
4. **检查**：如果某个 trait 实现在特定条件下 panic 或返回不符合契约的结果，则违反 LSP

### ISP：接口隔离原则（Interface Segregation Principle）

**客户端不应被迫依赖它不使用的方法。**

1. **Trait 细粒度拆分**：不定义一个巨大的 `Everything` trait，而是按职责拆分为 16 个 Port trait（`Loader`/`Splitter`/`Embedder`/`Storage`/`LLMProvider`/`Retriever`/`Reranker`/`QueryRewriter`/`PromptCompressor`/`ResponseCache`/`PageRenderer`/`OcrEngine`/`VisionLanguageModel`/`MultiVectorEmbedder`/`DomainClassifier`/`CodeExecutor`）
2. **可选能力分离**：`MultiVectorEmbedder` 与 `Embedder` 分离（ColBERT 多向量 vs 标准单向量），实现者只需实现自己需要的能力
3. **Pro trait 分离**：`PageRenderer`/`OcrEngine`/`VisionLanguageModel`/`CodeExecutor` 是 Pro 专属 trait，Free 用户不需要这些能力，通过 `#[cfg(feature = "pro")]` 隔离
4. **检查**：如果一个 trait 有 20+ 方法且大多数实现者只用到其中 3-5 个，则违反 ISP；应拆分为多个细粒度 trait

### DIP：依赖倒置原则（Dependency Inversion Principle）

**高层模块不应依赖低层模块，二者都应依赖抽象。**

1. **Port-Adapter 架构**：`crates/core` 定义 Port trait（抽象），`crates/infra` 实现 Adapter（具体），`crates/tauri-app` 在组装层注入具体实现。`core` 永远不 import `infra` 或 `tauri-app`
2. **依赖方向铁律**：`crates/tauri-app → crates/core → crates/models`，严禁反向依赖。`crates/infra → crates/core`（实现 Port trait），`crates/core` 绝不依赖 `infra`
3. **泛型注入**：`ChatEngine<R, L>` / `AgentEngine<R, L>` / `SpeculativeRagEngine<D, L>` 通过泛型参数接收 trait 实现，而非硬编码具体类型
4. **`*_inner` 测试模式**：IPC 命令的 `*_inner` 函数接收 `&AppState` 而非 `State<'_, AppState>`，允许集成测试注入 mock runtime
5. **检查**：如果 `crates/core` 中出现 `use echomind_infra::` 或 `use rusqlite::` 或 `use reqwest::`，则严重违反 DIP

### SOLID 违例检查清单（每次 code review / PR 提交前）

1. [ ] **SRP**：是否有类/函数承担多个职责？→ 拆分
2. [ ] **OCP**：新增功能是否修改了已有代码？→ 改为新增实现
3. [ ] **LSP**：trait 实现是否遵守 trait 契约？→ 修正不一致行为
4. [ ] **ISP**：是否有 trait 过于臃肿？→ 拆分为细粒度 trait
5. [ ] **DIP**：`core` 是否依赖了 `infra` 或具体实现？→ 通过 trait 抽象解耦

---

## 铁律十五：Spec-driven Development（规格驱动开发）

**⚠️ 绝对铁律：先写规格，再写代码。任何功能实现之前，必须先在 `docs/SRS_v1.0.md` 或原子规划文档中定义完整的规格（REQ + AC + TC），经确认后方可进入实现阶段。规格是开发的唯一依据，实现偏离规格时必须先更新规格。**

### 核心原则

规格驱动开发是 TDD（铁律二）的上游纪律。TDD 要求「先写测试再写代码」，但测试从何而来？Spec-driven development 回答这个问题：**测试从规格的验收标准（AC）派生**。规格定义「做什么」，AC 定义「怎样算做对」，TC 验证「是否做对了」。

```
规格（REQ + AC）→ 测试用例（TC）→ 实现 → 验证
     ↑                                      |
     └────────── 偏离时回写规格 ──────────────┘
```

### 规格必须包含的要素

每条 REQ（需求）规格必须包含以下要素，缺一不可：

| 要素 | 说明 | 示例 |
|---|---|---|
| **REQ-ID** | 唯一标识符，格式 `REQ-<域>-<序号>` | `REQ-RAG-043` |
| **优先级** | P0（核心）/ P1（重要）/ P2（增强） | `P2` |
| **状态** | Draft → Implemented → Done / Deprecated | `Draft` |
| **依赖** | 前置 REQ-ID | `REQ-ARCH-000` |
| **描述** | 功能行为的精确叙述（做什么、不做什么、边界条件） | 一段文字 |
| **验收标准（AC）** | 可验证的断言，每条 AC 对应至少一个 TC | `AC-1: ...` |
| **契约** | 涉及的 trait 方法 / 数据模型 / IPC 命令 / 数据库表 / 前端模块 | 契约清单 |
| **关联测试（TC）** | 验证各 AC 的测试 ID 列表 | `TC-BOUND-001~005` |

### 开发流程（强制顺序）

1. **需求分析**：理解需求背景，明确「为什么做」和「做什么」
2. **规格编写**（红灯前置）：在 `docs/SRS_v1.0.md` 新增 REQ 条目（或原子规划文档新增 Session），包含完整 AC + 契约 + TC 列表
3. **规格确认**：确认 AC 覆盖全部行为路径（正常 / 边界 / 错误），TC 与 AC 一一对应
4. **测试编写**（TDD 红灯）：根据 TC 编写测试代码，此时测试必须失败（因未实现）
5. **实现编码**（TDD 绿灯）：编写生产代码使测试通过
6. **规格回写**：实现过程中如有偏离（如发现 AC 不完整、契约需调整），**必须先更新规格再继续实现**
7. **文档同步**（铁律七）：实现完成后同步更新 SRS 状态 + code-contracts + ipc-commands 等 API 文档
8. **验证闭环**：运行全部测试 + clippy + fmt，确认 TC 全部通过且对应 AC

### 规格质量标准

1. **AC 可验证**：每条 AC 必须能用断言验证（不能是「系统应该好用」这种模糊描述）
2. **AC 覆盖完整**：至少覆盖正常路径、边界条件（空输入/超大输入/非法输入）、错误路径
3. **TC 一一对应**：每条 AC 至少有一个 TC 验证，TC-ID 必须在规格中列出
4. **契约精确**：trait 方法签名、数据模型字段、IPC 命令参数/返回值必须明确
5. **不描述实现细节**：规格描述「做什么」而非「怎么做」，实现方案不锁死在规格中

### 与 TDD（铁律二）的关系

| 维度 | 铁律二 TDD | 铁律十五 Spec-driven |
|---|---|---|
| 层次 | 代码级 | 需求级 |
| 产出 | 测试代码 | REQ + AC + TC 规格文档 |
| 顺序 | 测试 → 实现 | 规格 → 测试 → 实现 |
| 关系 | TDD 是 Spec-driven 的执行阶段 | Spec-driven 是 TDD 的上游纪律 |

**两者互补**：没有规格的 TDD 是「无方向地写测试」，没有 TDD 的 Spec-driven 是「有方向但不执行」。EchoMind 要求两者同时遵守。

### 违规示例

```text
❌ 违规：直接写代码，没有先定义 REQ + AC
   "我觉得需要个截断功能，直接写个函数吧"

✅ 正确：先写规格
   REQ-RAG-043 Tool Output Bounding
   AC-1: 短输出不截断 (TC-BOUND-001)
   AC-2: 超行数限制截断保留首尾 (TC-BOUND-002)
   ...
   → 先写 TC 测试（红灯）→ 再写实现（绿灯）
```

### 检查清单（每次功能开发前）

1. [ ] 是否已在 SRS 或原子规划文档中定义了 REQ + AC？
2. [ ] AC 是否覆盖正常/边界/错误三种路径？
3. [ ] 每个 AC 是否有对应的 TC-ID？
4. [ ] 契约（trait/模型/IPC/DB）是否已明确定义？
5. [ ] 测试代码是否从 AC 派生（而非从实现反推）？
6. [ ] 实现偏离时是否回写了规格？
