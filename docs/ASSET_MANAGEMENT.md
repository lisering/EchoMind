# EchoMind（灵犀）资产管理协议 — ASSET_MANAGEMENT v1.0

> 本文档记录 Git 规范、上下文重建协议与变更审查制度，保障「任何会话、任何时刻」都能无损恢复工程现场。
> 规则上位文件：`PROJECT_RULES.md` 体系四。

| 项 | 内容 |
| --- | --- |
| 文档版本 | v1.2.0 |
| 生效日期 | 2026-07-25 |
| 维护者 | TPM（Git 规范）+ DevOps（目录与密钥基线） |

---

## 1. 唯一真相源清单

| 文件 | 职责 | 变更时机 |
| --- | --- | --- |
| `PROJECT_RULES.md` | 四大纪律体系（最高约束） | 仅经附则修订程序 |
| `docs/SRS_v1.0.md` | 全部原子需求与验收标准 | 任何需求增删、状态流转、契约冻结 |
| `docs/TEST_STRATEGY.md` | 测试分层、门禁、追溯矩阵 | 测试策略调整、用例登记 |
| `docs/ASSET_MANAGEMENT.md` | 本文件：Git 与上下文协议 | 流程调整 |

**文档先行铁律**：代码变更之前，必须先更新上表对应文档。文档与代码不一致时，以文档为准修改代码。

---

## 2. Git 规范

### 2.1 分支模型
- `main`：受保护主干，任何时刻保持可构建、可发布；禁止直接推送。
- 功能分支：`feat/req-<域>-<序号>-<短描述>`（如 `feat/req-ing-001-drag-import`）。
- 修复分支：`fix/<简述>`；基建分支：`chore/<简述>`。
- 合并方式：PR + Squash Merge；合并前必须通过 `docs/TEST_STRATEGY.md` 第 7 节全部 CI 门禁。

### 2.2 Conventional Commits
格式：`<type>(<scope>): <简述>`，其中 `scope` 关联 REQ 时使用小写需求编号。

| type | 用途 | 示例 |
| --- | --- | --- |
| feat | 新功能（必须关联 REQ） | `feat(req-ing-001): 支持拖拽导入 md/txt` |
| fix | 缺陷修复 | `fix(req-rag-005): 停止生成后输入框未复位` |
| test | 测试先行 / 测试补全 | `test(req-lic-001): 免费版 50 文件配额红灯用例` |
| docs | 文档变更 | `docs(srs): 冻结 REQ-ING-001 导入契约` |
| refactor | 不改变行为的重构 | `refactor(core): 抽取分块器端口 Trait` |
| perf | 性能优化 | `perf(req-nfr-002): 向量检索加 HNSW 索引` |
| build / ci | 构建与流水线 | `ci: 增加 Windows E2E 矩阵` |
| chore | 杂项（依赖升级等） | `chore: 升级 tauri 至 2.x.y` |

规则：**每完成一个原子任务提交一次**；一个提交只做一件事；禁止把多个 REQ 揉进同一提交。

### 2.3 提交前门禁（本地必查）
```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo check
cargo test
```
四项全绿方可提交；禁止 `--no-verify` 跳过 hooks。

### 2.4 Git Hooks 自动化（v1.1.0 新增）

项目内置 `.githooks/` 目录，提供提交前自动化检查：

| Hook | 文件 | 检查内容 |
| --- | --- | --- |
| pre-commit | `.githooks/pre-commit` | 合并冲突标记检测、TODO/FIXME 提醒、敏感数据（API Key 密钥模式）拦截 |
| commit-msg | `.githooks/commit-msg` | Conventional Commits 格式校验（`type(scope): description` 正则匹配）、超长标题警告 |

**安装命令**（克隆后执行一次）：
```bash
git config core.hooksPath .githooks
```

验证安装：
```bash
git config --get core.hooksPath  # 应输出 .githooks
```

跳过 hooks（仅紧急情况）：`git commit --no-verify`，但必须在 PR 描述中说明原因。

---

## 3. 上下文重建协议（新会话 SOP）

AI 引擎或任何协作者开启新会话时，**严禁凭记忆写代码**，必须按序执行：

1. `git log --oneline -20` —— 定位最近的工程进度。
2. `git status` —— 确认是否存在未提交的在途变更。
3. 通读 `PROJECT_RULES.md` —— 找回纪律约束。
4. 通读 `docs/SRS_v1.0.md` —— 找回需求状态机与各 REQ 契约 / 测试登记。
5. 通读 `docs/TEST_STRATEGY.md` 与本文件 —— 找回门禁与流程。
6. 读取任务涉及源文件的**当前真实内容**后，方可开始设计与编码。

完成以上六步前，不允许产出任何代码修改。

---

## 4. 变更审查协议

1. 每次修改完成后，向开发者输出**变更摘要**，包含：变更动机（关联 REQ）、文件清单、行为变化、风险与回滚方式。
2. 建议以 hunk diff 形式逐块审查（`git diff` / 评审工具），AI 引擎应主动提供。
3. 摘要模板：

```
## 变更摘要
- 关联：REQ-XXX-XXX / TC-XXX-XXX
- 动机：<一句话>
- 文件：<新增/修改/删除 清单>
- 行为变化：<之前 → 之后>
- 风险：<兼容性、性能、安全>
- 验证：<已执行的测试与门禁>
```

---

## 5. 仓库目录结构（Phase 1 落地基线）

```
EchoMind/
├─ PROJECT_RULES.md            # 项目宪法
├─ docs/
│  ├─ SRS_v1.0.md              # 需求规格说明书
│  ├─ TEST_STRATEGY.md         # 测试策略
│  └─ ASSET_MANAGEMENT.md      # 本文件
├─ crates/
│  ├─ core/                    # 六边形内核：领域模型 + 端口 Trait，零具体实现依赖
│  └─ infra/                   # 适配器：fs / sqlite-vec / http / crypto / keychain
├─ src-tauri/                  # 组装层：Tauri commands、events、DI、错误码映射
├─ ui/                         # 纯 HTML / JS / Tailwind 前端（本地打包三件套库）
├─ e2e/                        # Playwright + tauri-driver 测试
├─ tests/fixtures/             # 测试样例文档（小型、匿名化）
├─ .github/workflows/          # pr-check / nightly-e2e / release
└─ README.md                   # 中英双语（DevOps 维护）
```

目录职责与 `PROJECT_RULES.md` 体系三一一对应；新增顶层目录须经架构评审。

---

## 6. 敏感资产与 .gitignore 基线

### 6.1 严禁入库
真实 API Key、License 私钥（仅构建期环境变量注入）、用户文档样本、本地数据目录、`.env`、`*.log`、签名证书与公证凭据。

### 6.2 .gitignore 基线（Phase 1 生成）
`target/`、`node_modules/`、`dist/`、`.env*`、`*.log`、`fixtures/` 之外的用户数据、OS 杂物（`.DS_Store`）等。

### 6.3 事故响应
一旦发现敏感信息入库：立即吊销对应凭据 → 重写历史（`git filter-repo`）→ 全员强推同步 → 复盘记录于本文件修订表。

---

## 7. 版本与变更日志

- 语义化版本：`v<major>.<minor>.<patch>`；v1.0.0 为首个商业发布版。
- 每个 release tag 对应 `CHANGELOG.md` 一节（Keep a Changelog 格式，中英双语条目），由 DevOps 汇总、PM 审核。
- SRS 版本号随需求基线调整递增，与代码 tag 的对应关系在 CHANGELOG 中登记。

---

| 版本 | 日期 | 修订摘要 | 批准 |
| --- | --- | --- | --- |
| v1.0.0 | 2026-07-24 | 创世版：Git 规范、上下文重建 SOP、变更审查与目录基线确立 | TPM / DevOps |
| v1.1.0 | 2026-07-25 | 新增 §2.4 Git Hooks 自动化（pre-commit + commit-msg，Conventional Commits 强制校验） | TPM / DevOps |
| v1.1.1 | 2026-07-25 | VIZ Phase 1：新增 ui/vendor/mermaid.min.js（v11.16.0）+ ui/vendor/MANIFEST.json（SRI 资产清单） | TPM / DevOps |
| v1.1.2 | 2026-07-25 | VIZ Phase 2：新增 ui/vendor/katex/（KaTeX v0.18.1：JS 272KB + CSS 25KB + mhchem 34KB + 20 woff2 字体 296KB ≈ 627KB）；MANIFEST.json 已更新 SRI 哈希 | TPM / DevOps |
| v1.2.0 | 2026-07-25 | SRS v2.1.0 原子化重构同步：唯一真相源清单无变更（SRS 仍为唯一需求规格源）；6 个软件资产文档同步更新——frontend-modules（补全已实现函数 + DS/IX/ERR 交叉引用）、UI_ACCEPTANCE（新增 §3.10~3.12 三域验收矩阵 47 用例）、E2E_COVERAGE（新增 §2.8~2.10 三域追溯矩阵）、TEST_STRATEGY（追溯矩阵新增 DS/IX/ERR 条目）、code-contracts（新增 §7 前端原子契约 + 错误前缀系统）、ASSET_MANAGEMENT（本条修订记录） | TPM |
