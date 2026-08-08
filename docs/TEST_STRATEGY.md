# EchoMind（灵犀）测试策略 — TEST_STRATEGY v1.0

> 本文档定义测试金字塔分工、测试左移执行流程与质量门禁，是 QA 工作的唯一真相源。
> 需求来源见 `docs/SRS_v1.0.md`；流程约束见 `PROJECT_RULES.md` 体系二。

| 项 | 内容 |
| --- | --- |
| 文档版本 | v1.6.0 |
| 生效日期 | 2026-07-26 |
| 维护者 | QA Lead |

---

## 1. 质量目标与原则

1. **测试左移**：QA 在开发写代码之前介入，先写红灯测试，开发唯一目标是让红灯变绿。
2. **契约驱动**：前后端联调不靠口头约定，一切断言以 TPM 冻结的 Command / Event 契约为准。
3. **100% 自动化**：不接受手工测试用例作为交付依据；所有验收标准（AC）必须有自动化断言。
4. **真实隔离**：测试不触碰真实用户数据目录、不使用真实 API Key、不依赖外网。

---

## 2. 测试金字塔与分工

| 层级 | 范围 | 工具 | Owner | 覆盖率门禁 | 执行时机 |
| --- | --- | --- | --- | --- | --- |
| L1 单元测试 | `crates/core` 领域逻辑（分块、配额、License 校验、检索排序等）；`crates/infra` 适配器内部逻辑 | `cargo test`（推荐 nextest）、`mockall`（dev-dependency） | Backend Dev 编写，QA Lead 审查断言 | core 行覆盖率 ≥ 90%；每个 P0 领域规则 100% 有用例 | 每次提交前本地 + CI 必跑 |
| L2 契约 / 集成测试 | `src-tauri` 全部 `#[tauri::command]` 签名与 `Event` Payload；SQLite / 向量库真实读写；HTTP 适配器 | `tauri::test`（mock app）、`wiremock`（LLM 端点打桩）、临时目录 | QA Lead 主导编写（红灯），Backend Dev 配合变绿 | 契约测试覆盖 100% Command 与 Event；每条 AC ≥ 1 用例 | 每次提交前本地 + CI 必跑 |
| L3 E2E 测试（真实 GUI） | 完整用户旅程与 UI 状态流转（向导→导入→索引→提问→流式渲染→停止→删除） | **WebdriverIO + tauri-driver**（Playwright 仅讲 CDP，无法连接 WebDriver 协议的 tauri-driver） | QA Lead | 覆盖 100% P0 用户旅程（SRS 第 3 节） | CI 强制门禁（ubuntu-latest + windows-latest 矩阵）；**macOS 不可用**（WKWebView 无 WebDriver 实现，上游硬限制） |
| L3-lite 桥接测试 | 与 L3 同内容的 UI 行为断言（本机含 macOS 可跑） | Playwright + 契约级 Mock `__TAURI__` 注入 | QA Lead | 与 L3 断言逐条对应 | 每个 PR 必跑（全平台 headless） |

### 2.1 各层纪律
- **L1**：`crates/core` 的测试不引入 Tauri、reqwest、rusqlite；端口 Trait 用手写 fake 或 mockall 打桩。测试代码允许 `unwrap()`，生产代码严禁。
- **L2**：每个 Command 至少验证「正常路径 + 参数校验失败 + 领域错误映射为错误码」三类断言；Event Payload 用 serde JSON 快照比对，防止契约静默漂移。
- **L3**：E2E 只走真实 UI 操作路径（拖放、点击、键盘），不调用内部 API 抄近路；断言聚焦「状态流转」而非实现细节。LLM 一律连接 wiremock 打桩的流式假端点。

---

## 3. 测试左移执行流程（与体系二对齐）

1. **PM 定需求**：SRS 中 REQ 条目状态置为 `Approved`，AC 齐备。
2. **QA 写红灯**：QA 将 AC 逐条翻译为自动化断言骨架，运行确认**全部失败（红灯）**；失败原因必须是「未实现」而非「测试本身错误」。
3. **断言同步**：QA 在全队同步断言逻辑；同时在 SRS 对应 REQ 的「关联测试」字段登记用例编号。
4. **TPM 冻结契约**：契约字段填齐后，QA 校验 L2 契约测试与冻结契约逐字一致。
5. **开发变绿灯**：开发实现直至相关用例全绿；禁止修改测试断言——断言有异议时退回 QA 阶段修订并重新走红灯。
6. **门禁确认**：CI 全绿 + QA Lead 复核追溯矩阵，REQ 状态置 `Done`。

---

## 4. 用例编号与追溯

### 4.1 编号规则
- 单元 / 集成用例：`TC-<域>-<三位序号>`（域代码与 SRS 一致）。
- E2E 用例：`E2E-<域>-<三位序号>`。
- 性能基准用例：`BENCH-<域>-<三位序号>`。

### 4.2 追溯矩阵
- 每条 AC 必须映射 ≥ 1 个用例；矩阵作为 SRS 附录由 QA 维护，每次 REQ 状态变更同步刷新。
- 提交信息中的 REQ 编号（如 `feat(req-ing-001): ...`）是代码↔需求↔测试三方追溯的锚点。

---

## 5. 测试数据与环境

### 5.1 基础约定

| 项 | 约定 |
| --- | --- |
| fixtures | `tests/fixtures/` 存放测试语料；野蛮场景 fixtures 由 `generate_corpus.py` 自动生成 |
| 数据目录 | 测试一律使用临时目录（`tempfile`），用例结束自动销毁 |
| LLM / Embedding | L1 用 fake Trait 实现；L2/L3 用 wiremock 流式假端点（固定 token 序列与延迟）；**真实 LLM 联调测试**见 §5.2 |
| 密钥 | 测试中只允许使用形如 `sk-test-*` 的假 Key；发现真实 Key 入库视为安全事故 |
| 并发 | 测试必须可并行执行（默认 `cargo test` 多线程），禁止用例间共享可变状态 |

### 5.2 真实 LLM 联调测试环境变量

> **铁律**（PROJECT_RULES.md 体系三 §3.0 铁律四）：需要真实 LLM 的集成测试通过环境变量注入凭据，严禁硬编码。

| 环境变量 | 用途 | 示例 |
| --- | --- |
| `ANTHROPIC_AUTH_TOKEN` | LLM API Key | `sk-ant-xxxx` |
| `BASE_URL` | LLM API Base URL | `https://api.anthropic.com` |
| `CLAUDE_CODE_SUBAGENT_MODEL` | 模型名称 | `claude-sonnet-4-20250514` |

**使用规范**：
1. 测试代码通过 `std::env::var()` 读取，缺失时 `skip` 而非 `fail`。
2. 严禁将环境变量值写入日志、断言消息或 Git 提交。
3. CI 中通过 GitHub Secrets 注入；本地开发通过 shell `export` 临时设置。
4. 测试结束后无需 `unset`（环境变量本身不构成安全风险，与代理不同）。

### 5.3 混沌测试语料（Phase 8.5）

| 文件 | 类型 | 场景 | 生成方式 |
| --- | --- | --- | --- |
| `large_real.md` | 大型真实 Markdown | 抗压测试（超长文档分块） | GitHub Raw 拉取 |
| `complex_paper.pdf` | 复杂学术论文 PDF | PDF 解析容错（图表/公式/多栏） | arXiv 下载 |
| `mixed_encoding.txt` | 混合编码文本 | 编码地狱容错（UTF-8/GBK/非法字节） | 脚本生成 |
| `malicious.md` | 恶意 Markdown | XSS 安全测试（script/img onerror） | 脚本生成 |
| `fake.pdf` | 格式欺骗文件 | PdfLoader 错误处理 | 脚本生成 |
| `empty.md` | 空文件 | 空内容边界处理 | 脚本生成 |
| `deep_nested/` | 深套目录（50 层） | 海量并发遍历 | 脚本生成 |

**生成命令**：`python3 tests/fixtures/generate_corpus.py`

---

## 6. 命令与工具链（基线）

```bash
cargo fmt --check                  # 格式门禁
cargo clippy --all-targets -- -D warnings   # 静态检查（含 unwrap_used / expect_used = deny）
cargo check                        # 提交前必过
cargo test                         # L1 + L2（或 cargo nextest run）
cargo tarpaulin --out Html         # 覆盖率报告（core ≥ 90%）
cd e2e-tests && npm run test:e2e   # L3 真实 GUI（Linux/Windows；macOS 不可用）
cd e2e-tests && npm run test:bridge # L3-lite 桥接层（Playwright，全平台）
```

Clippy 强制配置（写入 workspace 根 `Cargo.toml` 或 `clippy.toml`，Phase 1 落地）：`unwrap_used = "deny"`、`expect_used = "deny"`、`pedantic` 级别逐 crate 评审开启。

---

## 7. CI 质量门禁（GitHub Actions，Phase 1 搭建）

| 流水线 | 触发 | 内容 | 合并门禁 |
| --- | --- | --- | --- |
| pr-check | 每个 PR | fmt + clippy + check + L1/L2 全量 | 必须全绿 |
| e2e-gui | 每个 PR + 每日 | L3 真实 GUI（ubuntu + windows 矩阵，tauri-driver + WebdriverIO，Mock LLM SSE） | 必须全绿 |
| e2e-bridge | 每个 PR | L3-lite 桥接层（Playwright headless，全平台） | 必须全绿 |
| nightly-e2e | 每日定时 | L3 全量 + 性能基准 | 红灯时当日冻结新功能合并 |
| release | tag | 全平台打包 + E2E 冒烟 + 安装包校验 | 必须全绿 |

> 纪律条款（v1.0.6 起生效）：任何 UI 变更必须附带自动化断言通过证据；**严禁任何形式的人工冒烟代替自动化测试**。人工探索性测试仅作补充，不构成验收依据。

任何门禁失败时不允许 `git commit` 合入主干；禁止跳过 hooks 与 CI 检查。

## 7.5 UI 验收测试体系（v1.4.0 新增）

> 详细规范见 `docs/UI_ACCEPTANCE.md`。

### 7.5.1 体系范围

UI 验收测试体系覆盖 **8 大基础维度 + 9 大新增维度 = 17 个验收维度**，~150 个自动化用例，分布在 19 个 Playwright spec 文件中。

### 7.5.2 Mock 覆盖率

- **IPC 命令**：24/24 全覆盖（`tauri-stub.js`）
- **事件**：10/10 全覆盖（chat_phase / chat_token / chat_sources / chat_error / chat_done / doc-status-changed / audit_phase / import-progress / embedding_progress / model_download_progress）

### 7.5.3 新增验收维度

| 维度 | spec 文件 | 用例数 | 覆盖 REQ |
| --- | --- | --- | --- |
| 审计功能 UI | audit.spec.ts | 7 | REQ-AUDIT-001/004/005 |
| 文档操作按钮 | document-actions.spec.ts | 8 | REQ-VEC-005/ING-005/AUDIT-001 |
| 导入进度与取消 | import-progress.spec.ts | 7 | REQ-ING-006 |
| 模型缓存管理 | model-cache.spec.ts | 8 | REQ-VEC-008 |
| Toast 系统行为 | toast.spec.ts | 8 | REQ-UI-005 |
| Markdown 渲染完整性 | markdown-render.spec.ts | 10 | REQ-UI-002 |
| 键盘导航与无障碍 | keyboard-a11y.spec.ts | 10 | REQ-UI-001/003/007 |
| 侧栏折叠与交互 | sidebar-behavior.spec.ts | 9 | REQ-UI-001 |
| License 停用 UI | license-deactivate.spec.ts | 8 | REQ-LIC-004 |

### 7.5.4 缺陷发现

体系建立过程中已发现并修复 8 个缺陷：

| 编号 | 严重性 | 描述 |
| --- | --- | --- |
| BUG-UI-001 | P0 | `toastSuccess()` 被调用但未定义，导致 JS 运行时错误 |
| BUG-UI-002 | P1 | `toContainText('A|B|C')` 误将 `|` 作字面量，审计阶段文案匹配失败 |
| BUG-UI-003 | P1 | `locator('button').click()` 多按钮匹配触发 strict mode violation |
| BUG-UI-004 | P2 | `#stopBtn` 初始隐藏态被断言为 `toBeVisible` |
| BUG-UI-005 | P1 | Mock `import_files` 同步返回导致进度条闪现 |
| BUG-UI-006 | P1 | Toast 通知层遮挡 `#importCancelBtn`，Playwright click 被拦截 |
| BUG-UI-007 | P2 | `#importProgressText` 初始文本不含文件名，`innerText()` 时序竞争 |
| BUG-UI-008 | P2 | Mock `audit_document` token 间隔过短，取消测试时序不可靠 |
| BUG-UI-009 | P1 | `#queryInput` textarea 缺少 `py-1.5` 垂直内边距，单行高度（24px）比按钮（36px）矮 12px，`items-end` 导致 placeholder/光标偏下 |

全量 141/141 测试绿灯。

---

一个原子任务被 TPM 关闭前，必须同时满足：

1. SRS 中对应 REQ 状态为 `Done`，契约与实现逐字一致；
2. 该 REQ 全部 AC 对应的自动化用例绿灯，且能在 CI 复现；
3. `fmt / clippy / check` 三门禁通过；
4. 变更摘要已按 `docs/ASSET_MANAGEMENT.md` 模板输出，并以 Conventional Commit 入库。

---

## 9. 附录：REQ↔TC 追溯矩阵（骨架）

> 首个功能切片（REQ-ING-001 等）进入 QA 阶段后，由 QA Lead 在此登记全量矩阵。

| REQ | AC | 用例编号 | 层级 | 状态 |
| --- | --- | --- | --- | --- |
| REQ-ARCH-000 | AC-1/2/3 | CI-BUILD（cargo check + clippy + 依赖树审计） | CI | 绿灯 |
| REQ-ING-001 / REQ-ING-002 | AC-3 / AC-3 | TC-ING-001（.jpg 格式白名单拦截） | L1 | 红灯→绿灯 |
| REQ-ING-002 | AC-1 | TC-ING-004（免费版 PDF 付费门） | L1 | 红灯→绿灯 |
| REQ-ING-004 | AC-1 | TC-ING-002（MD5 指纹去重跳过） | L1 | 红灯→绿灯 |
| REQ-LIC-001 | AC-1 | TC-ING-003（免费版第 51 文件拦截） | L1 | 红灯→绿灯 |
| REQ-ING-001（加载链路） | AC-1 旁证 | TC-ING-005 / TC-ING-006（Markdown / Text 加载器） | L1 | 红灯→绿灯 |
| REQ-VEC-001 | AC-2 / AC-3（前置） | TC-ING-007（token 窗口分块器） | L1 | 红灯→绿灯 |
| REQ-DB-001 | AC-1 | TC-DB-001（SQLite 跨实例持久化） | L1 | 红灯→绿灯 |
| REQ-DB-001 | AC-2 | TC-DB-002（Processing 僵尸崩溃恢复） | L1 | 红灯→绿灯 |
| REQ-PDF-001 | AC-1 | TC-PDF-001（伪造 PDF 字节流不 Panic） | L1 | 红灯→绿灯 |
| REQ-VEC-002 | AC-2 | TC-VEC-001（fastembed 批量推理维度/数量） | L1 | 红灯→绿灯 |
| REQ-RAG-003 | AC-1 | TC-RAG-001（低分检索结果阈值过滤） | L1 | 红灯→绿灯 |
| REQ-RAG-003 | AC-1 / AC-2 | TC-RAG-002（空上下文不调用 LLM 直接固定提示） | L1 | 红灯→绿灯 |
| REQ-LLM-001 | AC-1 | TC-LLM-001（SSE 粘包/拆包缓冲解析） | L1 | 红灯→绿灯 |
| REQ-UI-008 | AC-1 / AC-3 | TC-UI-001（配置持久化跨重启 + Key 脱敏） | L2 | 红灯→绿灯 |
| REQ-RAG-003 | AC-2 | TC-UI-002（空知识库 chat 明确拦截） | L2 | 红灯→绿灯 |
| REQ-RAG-006 | AC-1 | TC-RAG-003（会话 CRUD 与级联删除） | L2 | 红灯→绿灯 |
| REQ-RAG-006 | AC-2 | TC-RAG-004（消息持久化双记录 + 标题提取） | L2 | 红灯→绿灯 |
| REQ-RAG-005 | AC-1 | TC-RAG-005（CancellationToken 流中断无 Panic） | L2 | 红灯→绿灯 |
| REQ-LIC-001 | AC-1 | TC-LIC-001（合法签名 License 验证通过） | L1 | 红灯→绿灯 |
| REQ-LIC-001 | AC-2 | TC-LIC-002（篡改签名 License 验证拒绝） | L1 | 红灯→绿灯 |
| REQ-LIC-002 | AC-1 | TC-LIC-003（免费版 50 文件配额拦截） | L2 | 红灯→绿灯 |
| REQ-LIC-002 | AC-2 | TC-LIC-004（免费版 PDF 付费门拦截） | L2 | 红灯→绿灯 |
| REQ-LIC-001 | AC-3 | TC-LIC-005（激活后重启 is_pro 持久化恢复） | L2 | 红灯→绿灯 |
| REQ-LIC-003 | AC-1/AC-2/AC-3 | E2E-LIC-001（付费墙弹出→激活→状态更新） | L3-lite | CI 门禁 |
| REQ-CHAOS-001 | AC-1 | TC-CHAOS-001（大型真实 Markdown 抗压导入） | L2 | 红灯→绿灯 |
| REQ-CHAOS-002 | AC-1 | TC-CHAOS-002（复杂学术论文 PDF 文本提取） | L2 | 红灯→绿灯 |
| REQ-CHAOS-003 | AC-1 | TC-CHAOS-003（混合编码 TXT from_utf8_lossy 容错） | L2 | 红灯→绿灯 |
| REQ-CHAOS-004 | AC-1 | TC-CHAOS-004（恶意 Markdown XSS 标签剥离） | L2 | 红灯→绿灯 |
| REQ-CHAOS-005 | AC-1 | TC-CHAOS-005（空文件零垃圾 Chunk） | L2 | 红灯→绿灯 |
| REQ-CHAOS-006 | AC-1 | TC-CHAOS-006（格式欺骗文件优雅失败） | L2 | 红灯→绿灯 |
| REQ-CHAOS-007 | AC-1 | TC-CHAOS-007（50 层深套目录批量导入） | L2 | 红灯→绿灯 |
| REQ-UI-006 / REQ-ING-005 | AC-1/AC-2 / AC-1 | E2E-UI-001（六断言全链路冒烟：向导→配置→导入→列表→流式→删除） | L3 | CI 门禁 |
| REQ-RAG-001 / REQ-RAG-005 | AC-3 / AC-1 | E2E-UI-001（流式增长 + 代码块复制 + 停止中断断言） | L3 | CI 门禁 |
| REQ-VIZ-001 | AC-1 | TC-VIZ-001（Mermaid flowchart 渲染为 SVG） | L3-lite | 绿灯 |
| REQ-VIZ-001 | AC-2 | TC-VIZ-002（流式期间 mermaid 代码块占位提示） | L3-lite | 绿灯 |
| REQ-VIZ-001 | AC-3 | TC-VIZ-003（Mermaid 语法错误优雅提示） | L3-lite | 绿灯 |
| REQ-VIZ-001 | AC-5 | TC-VIZ-004（暗色主题图表可读性） | L3-lite | 绿灯 |
| REQ-VIZ-002 | AC-1/AC-3 | TC-VIZ-005（行内 LaTeX 公式渲染 + mhchem 化学方程式） | L3-lite | 绿灯 |
| REQ-VIZ-002 | AC-2 | TC-VIZ-005b（块级 LaTeX 公式居中渲染） | L3-lite | 绿灯 |
| REQ-VIZ-002 | — | TC-VIZ-005c（KaTeX 语法错误优雅提示） | L3-lite | 绿灯 |
| REQ-VIZ-002/REQ-SEC-002 | — | TC-VIZ-005d（KaTeX 渲染不含 XSS 载荷） | L3-lite | 绿灯 |
| REQ-VIZ-001/REQ-SEC-002 | AC-4 | TC-VIZ-006（渲染 SVG 不含 <script> 标签） | L3-lite | 绿灯 |
| REQ-VIZ-003 | AC-1/AC-3 | TC-VIZ-007（Markdown 表格→图表切换 + 数据一致） | L3-lite | 绿灯 |
| REQ-VIZ-003 | AC-2 | TC-VIZ-007b（柱/折/饼三种图表类型可切换） | L3-lite | 绿灯 |
| REQ-VIZ-004 | AC-1 | TC-VIZ-008（提问流程图→LLM 输出 mermaid 语法） | L2 | 红灯→绿灯 |
| REQ-VIZ-004 | AC-2 | TC-VIZ-008b（提问公式→LLM 输出 LaTeX 语法） | L2 | 红灯→绿灯 |
| REQ-VIZ-004 | AC-3 | TC-VIZ-008c（普通问题不强制图表语法） | L2 | 红灯→绿灯 |
| REQ-MM-001 | AC-1/AC-2/AC-3 | TC-MM-001（PDF 图片页面渲染为位图） | L2 | 红灯→绿灯 |
| REQ-MM-002 | AC-1/AC-2/AC-3 | TC-MM-002（本地 OCR 提取图片文字） | L2 | 红灯→绿灯 |
| REQ-MM-003 | AC-1 | TC-MM-003（VLM 表格→Markdown 表格） | L2 | 绿灯 |
| REQ-MM-003 | AC-2 | TC-MM-003b（VLM 甘特图→Mermaid gantt） | L2 | 绿灯 |
| REQ-MM-003 | AC-3/AC-4 | TC-MM-003c（VLM 不可用时降级为 OCR） | L2 | 绿灯 |
| REQ-MM-004 | AC-1/AC-2/AC-3 | TC-MM-004（多模态导入管线图文混排可检索） | L2 | 红灯→绿灯 |
| REQ-MM-003 | 前端接线 AC-1/AC-2/AC-3/AC-4 | TC-MM-005（VLM 开关持久化与重启恢复） | L2 | 绿灯 |
| REQ-MM-003 | 前端接线 AC-1/AC-2/AC-3/AC-4 | TC-MM-006（vision_provider 运行时开关响应） | L2 | 绿灯 |
| REQ-MM-005 | AC-1/AC-3 / 4 级分级策略框架 | TC-MM-007（VLM_TIERED_PROMPT 含 Level 1~4 全部等级标记 + 表格/Markdown/Mermaid/数据图表关键词） | L1 | 绿灯 |
| REQ-MM-005 | AC-1 / Level 3 CSV 提取 | TC-MM-007a（prompt 含坐标轴/数据系列/CSV/数值/趋势提取引导） | L1 | 绿灯 |
| REQ-MM-005 | AC-3 / 误差标注 | TC-MM-007b（prompt 含「AI 视觉提取」/「可能存在误差」/「请核对原文」三段标注） | L1 | 绿灯 |
| REQ-MM-005 | AC-2 / LaTeX 公式提取 | TC-MM-007（prompt 含公式/LaTeX/$ 定界符引导） | L1 | 绿灯 |
| REQ-MM-005 | AC-1/AC-3/AC-5 / CSV 端到端管线 | TC-MM-007c（mock VLM 返回 CSV 数据 → chunks 含坐标轴+3 数据点+误差标注 + prompt 含 Level 3/CSV/LaTeX 验证） | L2 | 绿灯 |
| REQ-MM-003 | 前端接线 | E2E-MM-001~009（设置面板 + VLM 开关 + 隐私弹窗 9 场景） | L3-lite | 绿灯 |
| REQ-VEC-001 | AC-3 / 段落感知 / 代码块完整性 | TC-VEC-002（SemanticSplitter 端到端分块入库：sequence 连续 + 标题正文同 chunk + 代码块内容完整 + token_count 合理） | L1 | 绿灯 |
| REQ-RAG-003 | AC-1 / Phase 4 Chunk Expansion | TC-RAG-009（检索命中后扩展前后各 1 个相邻 chunk，补全跨 chunk 上下文，去重 + 按 sequence 排序） | L1 | 绿灯 |
| REQ-VEC-002 | AC-2 / Phase 3 ContextualEmbedding | TC-RAG-010（build_contextual_text 拼接文档名前缀，使嵌入向量包含文档上下文信息） | L1 | 绿灯 |
| REQ-VEC-006 | AC-1/2/3/4 | TC-VEC-010（SectionAwareSplitter 章节感知分块：标题层级 → section 切分 → 章节路径前缀，无标题退化） | L1 | 绿灯 |
| REQ-AUDIT-001 | AC-1/AC-2/AC-3/AC-4 | TC-AUDIT-001/001b（全量审计 list_chunks 非 vector_search + audit_phase 事件 + 取消部分报告 + 空文档 NoChunks）；TC-AUDIT-006（免费版审计门控）；TC-AUDIT-007（Pro 未配置 LLM 拦截）；TC-AUDIT-009（Pro Indexed 文档审计前置数据验证）；TC-AUDIT-010（空文档边界处理） | L1+L2 | 绿灯 |
| REQ-AUDIT-002 | AC-1/AC-2/AC-3/AC-4 | TC-AUDIT-002/002b（原子声明提取 JSON + chunk_id/sequence 来源追溯 + 非声明过滤 + JSON 解析失败降级） | L1 | 绿灯 |
| REQ-AUDIT-003 | AC-1/AC-2/AC-3/AC-4 | TC-AUDIT-003/003b（SIFiD 预筛 cosine≥0.65 + LLM 精判 contradiction/consistent/unverifiable + 语义无关过滤） | L1 | 绿灯 |
| REQ-AUDIT-004 | AC-1/AC-2/AC-3/AC-4/AC-5 | TC-AUDIT-004/004b（report_to_markdown 摘要+矛盾清单+免责声明 + 空矛盾文案 + DOMPurify 安全） | L1 | 绿灯 |
| REQ-AUDIT-005 | AC-1/AC-2/AC-3/AC-4 | TC-AUDIT-005（audit_phase 进度 + abort_audit 取消 + 部分报告保留 + chat_done 完成）；TC-AUDIT-008（abort_audit 设置 AtomicBool 标志 + clear_audit_cancel 清理语义） | L1+L2 | 绿灯 |
| REQ-NFR-002 | AC-1 | TC-NFR-002（10k chunks / 384-dim / 20 queries / P95 断言 ≤ 500ms，`#[ignore]` 基准测试） | L1 | 绿灯 |
| REQ-SEC-004 | AC-1 | TC-SEC-004（数据目录权限 0700 + 密钥文件 0600 + 数据隔离 + 卸载清除） | L1 | 绿灯 |
| REQ-RAG-010 | AC-1~AC-5 / RRF 融合 + 混合检索 | TC-RRF-001~014（RRF 算法单元测试：空列表/单通道/重叠 chunk 去重/top-k 限制/权重/字段保留）；TC-HYBRID-001~005（混合检索集成测试：向量+关键词融合/空库/关键词单通道命中/阈值过滤/Chunk Expansion）；TC-KEYWORD-001~006（FTS5 关键词测试：匹配/空查询/top-k/删除文档同步/短查询 LIKE 回退） | L1 | 绿灯 |
| REQ-NFR-006 | AC-1/AC-2/AC-3 / 首 Token 延迟 | TC-NFR-006（build_rag_prompt 20 条 sources ≤ 500ms + ChatEngine.chat 本地管线 ≤ 500ms + chat_phase 三阶段事件顺序验证）；E2E-RAG-014（chat_phase 事件更新指示器文案——已有绿灯用例旁证 AC-2） | L1+L3-lite | 绿灯 |
| REQ-NFR-007 | AC-1~AC-5 / 嵌入吞吐量 | TC-PERF-001~008（会话池大小/分片/并行推理/微批次进度事件/分块大小自动选择——已有绿灯用例） | L1+L2 | 绿灯 |
| REQ-NFR-008 | AC-1~AC-6 / UI 交互响应延迟 | E2E-NFR-008（点击列表项 ≤ 100ms 高亮 + 输入框 ≤ 16ms/字符 + 拖拽遮罩 ≤ 100ms + 长耗时操作 ≤ 100ms 加载态 + 滚动 ≥ 30fps + DB 不阻塞 UI） | L3-lite | 规划 |
| REQ-NFR-009 | AC-1~AC-5 / 崩溃恢复可靠性 | TC-DB-001（已提交数据完整可读 + 外键级联无孤儿）；TC-DB-002（Processing→Failed 僵尸清理）；TC-RAG-005（中断 persist_exchange 保留部分内容） | L1+L2 | 绿灯 |
| REQ-NFR-010 | AC-1~AC-5 / 流式生成中断容错 | TC-RAG-003（停止按钮 ≤ 200ms 中断 + 内容保留）；TC-RAG-005（中断 persist_exchange + clear_abort 清理） | L1+L2 | 绿灯 |
| REQ-NFR-011 | AC-1~AC-5 / 数据库并发写入安全 | TC-DB-001（r2d2 连接池 max_size=8 + busy_timeout=5000 + WAL 模式并发读写不阻塞） | L1+L2 | 绿灯 |
| REQ-NFR-012 | AC-1~AC-5 / 知识库容量可扩展性 | TC-NFR-002（10k chunks P95 ≤ 500ms）；TC-NFR-005c（100k+ HNSW P95 ≤ 10ms `#[ignore]`）；TC-PERF-006（FTS5 分块后立即可用 + auto_chunk_tokens >1MB 切 1024 窗口） | L1+L2 | 绿灯 |
| REQ-NFR-013 | AC-1~AC-5 / 嵌入并发并行度 | TC-PERF-001~005（default_pool_size / shard_texts / 并行 vs 单会话一致性 / MAX_POOL_SIZE=8 封顶 / 并行吞吐 ≥ 4x） | L1 | 绿灯 |
| REQ-NFR-014 | AC-1~AC-5 / 代码质量门禁 | CI-BUILD（cargo clippy --all-targets -D warnings + cargo fmt --check + cargo deny check 四维 + workspace.lints 继承） | CI | 绿灯 |
| REQ-NFR-015 | AC-1~AC-5 / 测试金字塔覆盖率 | CI-TEST（cargo test 全量通过 0 failed + L1 单元 ~136 + L2 集成 ~39 + L3 E2E ~141 + `#[ignore]` 基准手动可跑） | CI | 绿灯 |
| REQ-NFR-016 | AC-1~AC-5 / 文档实时同步 | TPM/QA 人工审计（SRS 契约字段与代码一致 + 关联测试与实际用例对应 + IPC 命令数一致 + 设置键值一致 + 修订记录追加） | 人工审计 | 绿灯 |
| REQ-NFR-017 | AC-1~AC-5 / OS 版本兼容性 | CI-BUILD（e2e.yml build 矩阵 macOS arm64/x64 + Windows x64 + Linux x64 + `#[cfg(unix)]`/`#[cfg(not(unix))]` 平台条件） | CI | 规划 |
| REQ-NFR-018 | AC-1~AC-5 / 开发工具链版本约束 | CI-BUILD（Cargo.toml edition=2024 + resolver=3 + `#[async_trait]` grep 断言 + `[profile.dev.package."*"] opt-level=3` + Rust ≥1.85） | CI | 绿灯 |
| REQ-NFR-019 | AC-1~AC-5 / 构建产物体积 | CI-BUILD（macOS .dmg ≤ 50MB + Windows .msi ≤ 60MB + ONNX 模型不打包 + ui/vendor/ ≤ 5MB + release LTO+strip） | CI | 规划 |
| REQ-NFR-020 | AC-1~AC-5 / 增量编译性能 | 手动计时验证（首次 cargo build ~5-10min + 增量 ≤ 30s + cargo check ≤ 10s + cargo clippy ≤ 60s + tauri dev 前端热重载） | 手动验证 | 绿灯 |
| REQ-NFR-021 | AC-1~AC-5 / 核心功能离线可用性 | E2E-OFFLINE-001~005（前端零 CDN + 零外部网络请求 + 断网下导入/会话/设置正常 + 仅 LLM/VLM/模型下载需联网）；E2E-UI-004（已有 CDN 审计旁证 AC-4） | L3-lite | 绿灯 |
| REQ-DS-001~007 | AC-1~AC-7 / 色板/字体/间距/图标/组件/动效/圆角阴影 | E2E-DS-001~014（设计系统原子规格 E2E 断言：tailwind.config 色值断言 + `.md` 计算样式 + padding/间距实测 + 按钮变体色值 + fade-in 动画时长 + border-radius 实测） | L3-lite | 规划 |
| REQ-IX-001~007 | AC-1~AC-7 / 右键菜单/拖拽排序/复制/悬停/确认框/骨架屏 | E2E-IX-001~018（交互设计原子规格 E2E 断言：右键菜单弹出/边界偏移/Esc 关闭/拖拽 opacity/插入线/复制 clipboard/hover 背景色/确认框 500ms 防误触/骨架屏 animate-pulse） | L3-lite | 规划 |
| REQ-ERR-001~005 | AC-1~AC-5 / 错误分类/重试/离线/崩溃恢复/输入校验 | E2E-ERR-001~015（错误处理原子规格 E2E 断言：LIMIT_REACHED 前缀/showPaywall/sanitizeError 脱敏/LLM 重试退避/离线模式 disabled/崩溃恢复 Processing→Failed/路径遍历拒绝/标题截断） | L3-lite | 规划 |
| REQ-OBS-001~003 | AC-1~AC-6/AC-1~AC-5/AC-1~AC-5 / 本地日志/性能指标采集/诊断信息导出 | E2E-OBS-001~010（可观测性原子规格 E2E 断言：JSON Lines 日志格式 + 日期轮转 + 日志字段 + 敏感信息过滤 + 性能指标记录 + 诊断信息导出按钮 + 导出文件脱敏） | L3-lite | 规划 |
| REQ-WIN-001~005 | AC-1~AC-5 / 窗口尺寸/高 DPI/系统托盘/开机自启/系统主题跟随 | E2E-WIN-001~010（窗口管理原子规格 E2E 断言：最小窗口 800×600 + 默认 1200×800 + 窗口位置持久化 + Retina 渲染清晰 + 系统托盘菜单 + 开机自启开关 + prefers-color-scheme 检测） | L3-lite | 规划 |
| REQ-I18N-001~003 | AC-1~AC-5/AC-1~AC-3/AC-1~AC-4 / 文案外部化/日期本地化/数字本地化 | E2E-I18N-001~008（国际化原子规格 E2E 断言：zh-CN.json 键值对 + 字符串 key 引用 + 缺失 key 回退 + 日期 YYYY-MM-DD HH:mm + 相对时间 + 文件大小 B/KB/MB + 千分位分隔符） | L3-lite | 规划 |
| REQ-HELP-001~004 | AC-1~AC-5/AC-1~AC-5/AC-1~AC-5/AC-1~AC-6 / 首次引导/FAQ/关于页面/更新检查 | E2E-HELP-001~010（帮助系统原子规格 E2E 断言：Coach Marks 引导 + 跳过/完成持久化 + FAQ 面板 Markdown 渲染 + 关于页面版本号/技术栈 + 更新检查徽标 + 24h 频率限制） | L3-lite | 规划 |
| （其余条目待 QA 阶段登记） | | | | |

| 版本 | 日期 | 修订摘要 | 批准 |
| --- | --- | --- | --- |
| v1.0.0 | 2026-07-24 | 创世版：金字塔分工、左移流程、门禁与 DoD 确立 | QA Lead / TPM |
| v1.0.1 | 2026-07-25 | Phase 8：登记 TC-LIC-001~005（License 校验与配额门控）、E2E-LIC-001（付费墙 UI） | QA Lead / TPM |
| v1.1.0 | 2026-07-25 | Phase 8.5：新增 §5.2 真实 LLM 环境变量、§5.3 混沌测试语料、TC-CHAOS-001~007 追溯矩阵 | QA Lead / TPM |
| v1.2.0 | 2026-07-25 | Phase 9：新增 VIZ 域 TC-VIZ-001~008c 追溯矩阵（Mermaid/KaTeX/Chart.js 富内容渲染） | QA Lead / TPM |
| v1.2.1 | 2026-07-25 | VIZ Phase 1 实施：TC-VIZ-008/008b/008c 绿灯（build_rag_prompt 图表引导）；TC-VIZ-001~006 E2E 用例已编写待浏览器验证 | QA Lead / TPM |
| v1.2.2 | 2026-07-25 | VIZ Phase 1 E2E 验证：TC-VIZ-001~006 全部绿灯（5/5 passed）。VIZ Phase 2 实施：TC-VIZ-005/005b/005c/005d 全部绿灯（KaTeX 0.18.1 + mhchem，4/4 passed） | QA Lead / TPM |
| v1.2.3 | 2026-07-25 | MM Phase 3 L2 契约测试验收：TC-MM-001/002/004 全部绿灯（lopdf 编程生成测试 PDF + Mock 端口验证） | QA Lead / TPM |
| v1.2.4 | 2026-07-25 | MM Phase 2 L2 契约测试验收：TC-MM-003/003b/003c 全部绿灯（VisionLanguageModel 端口 + OpenAIVisionProvider 适配器 + NoVlm 占位 + Mock VLM 验证表格→MD/甘特→Mermaid/降级） | TPM |
| v1.2.5 | 2026-07-25 | MM Phase 2 前端接线 L2 测试：TC-MM-005/006 绿灯（set_vlm_enabled 持久化 + vision_provider 运行时开关 + OnceCell 缓存移除验证） | TPM |
| v1.2.6 | 2026-07-25 | MM Phase 2 前端接线 E2E：E2E-MM-001~009 全部绿灯（设置面板打开/关闭 + VLM 开关初始状态 + 隐私确认弹窗弹出/取消/确认 + VLM 关闭直接生效 + 状态持久化恢复 + LLM 配置修改入口） | QA Lead / TPM |
| v1.2.7 | 2026-07-25 | SemanticSplitter L1 集成测试：TC-VEC-002 绿灯（端到端分块入库契约：真实 Markdown 语料 → import → index → 验证 sequence 连续 + 标题正文同 chunk + 代码块内容完整 + token_count 合理）；MockStorage 扩展 chunks 捕获能力 | TPM |
| v1.2.8 | 2026-07-25 | Phase 4 Chunk Expansion：TC-RAG-009 绿灯（VectorRetriever.retrieve 命中后扩展前后各 1 个相邻 chunk，补全跨 chunk 上下文，去重 + 按 sequence 排序）；MockStorage 扩展 all_chunks 字段 | TPM |
| v1.2.9 | 2026-07-25 | Phase 3 ContextualEmbedding：TC-RAG-010 绿灯（build_contextual_text 拼接文档名前缀，零 LLM 成本规则方案）；commands.rs embed_document_chunks 接入 | TPM |
| v1.3.0 | 2026-07-25 | REQ-VEC-006 章节感知分块：TC-VEC-010 绿灯（SectionAwareSplitter pulldown-cmark 标题层级解析 → section 切分 → 委托 SemanticSplitter → 章节路径前缀）；ImportService.index_inner .md 接入 SectionAwareSplitter | TPM |
| v1.3.1 | 2026-07-25 | REQ-MM-005 分级图表理解：TC-MM-007/007a/007b/007c 全部绿灯（vlm_prompt 模块 VLM_TIERED_PROMPT 4 级策略 + loader/import 消除重复 + mock VLM CSV 端到端管线） | TPM |
| v1.3.2 | 2026-07-25 | AUDIT 域 + NFR-002 + SEC-004：TC-AUDIT-001~005b 全部绿灯（9/9 审计单元测试）；TC-AUDIT-006~010 全部绿灯（5/5 审计集成测试——Pro 门控 + LLM 配置检查 + 取消机制 + Indexed 文档前置验证 + 空文档边界）；TC-NFR-002 绿灯（10k chunks P95=178ms ≤ 500ms）；TC-SEC-004 绿灯（数据目录 0700 + 密钥 0600 + 隔离 + 卸载清除） | QA Lead / TPM |
| v1.3.3 | 2026-07-25 | REQ-RAG-010 混合检索：TC-RRF-001~014 全部绿灯（14/14 RRF 融合算法单元测试——空列表/单通道/重叠去重/top-k/权重/字段保留）；TC-HYBRID-001~005 全部绿灯（5/5 混合检索集成测试——向量+关键词融合/空库/关键词单通道/阈值过滤/Chunk Expansion）；TC-KEYWORD-001~006 全部绿灯（6/6 FTS5 关键词检索测试——匹配/空查询/top-k/删除同步/短查询 LIKE 回退） | QA Lead / TPM |
| v1.4.0 | 2026-07-25 | UI 验收体系全面升级：新增 §7.5 UI 验收测试体系（17 维度 ~150 用例 19 spec 文件）；Mock 覆盖 24/24 命令 + 10 事件；修复 toastSuccess 未定义 P0 缺陷（BUG-UI-001） | QA Lead / TPM |
| v1.4.1 | 2026-07-25 | 全量 133/133 绿灯：修复 7 个测试缺陷（BUG-UI-002~008：正则误用/多按钮匹配/toast遮挡/stopBtn可见性/进度条时序/审计取消时序）；Mock 增强 import_files 延迟 + audit token 间隔 | QA Lead |
| v1.4.2 | 2026-07-25 | 新增输入框视觉对齐维度（input-bar-align.spec.ts 8 用例）；修复 BUG-UI-009 textarea py-1.5 垂直内边距；全量 141/141 绿灯 | QA Lead |
| v1.5.0 | 2026-07-25 | SRS v2.1.0 原子化重构同步：追溯矩阵新增 DS 域（E2E-DS-001~014）、IX 域（E2E-IX-001~018）、ERR 域（E2E-ERR-001~015）三域 47 条 E2E 用例规划条目；全部状态标注为「规划」（Draft 需求待实现后编写测试） | QA Lead / TPM |
| v1.6.0 | 2026-07-26 | SRS v2.0/v2.1 同步补全：(1) 追溯矩阵新增 NFR-006~021 共 16 条条目（含 TC-NFR-006 新增 L1 绿灯 + NFR-021 离线 E2E 绿灯 + NFR-007/009/010/011/012/013 已有用例映射 + NFR-014~016/018/020 CI/人工审计绿灯 + NFR-008/017/019 规划）；(2) 追溯矩阵新增 OBS/WIN/I18N/HELP 四域规划条目（E2E-OBS-001~010 / E2E-WIN-001~010 / E2E-I18N-001~008 / E2E-HELP-001~010）；(3) 新增 TC-NFR-006 L1 测试（build_rag_prompt 计时 + ChatEngine 本地管线计时 + 三阶段事件验证）；(4) 新增 E2E-OFFLINE-001~005 离线可用性 E2E 测试（零 CDN + 零外部网络请求 + 断网下导入/会话/设置正常）；(5) tauri-stub.js 增强 chat_phase emitting preparing 阶段 | QA Lead / TPM |