# EchoMind（灵犀）测试策略 — TEST_STRATEGY v1.0

> 本文档定义测试金字塔分工、测试左移执行流程与质量门禁，是 QA 工作的唯一真相源。
> 需求来源见 `docs/SRS_v1.0.md`；流程约束见 `PROJECT_RULES.md` 体系二。

| 项 | 内容 |
| --- | --- |
| 文档版本 | v1.0.0 |
| 生效日期 | 2026-07-24 |
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
| L3 E2E 测试 | 完整用户旅程与 UI 状态流转（导入→索引→提问→流式渲染→引用回溯；付费墙；License 激活） | Playwright + tauri-driver | QA Lead | 覆盖 100% P0 用户旅程（SRS 第 3 节） | CI 每日全量（macOS / Windows），发布前必跑 |

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

| 项 | 约定 |
| --- | --- |
| fixtures | `tests/fixtures/` 存放小型样例文档（md / txt / pdf），单文件 ≤ 100KB，禁止放入任何真实用户文档 |
| 数据目录 | 测试一律使用临时目录（`tempfile`），用例结束自动销毁 |
| LLM / Embedding | 一律打桩：L1 用 fake Trait 实现；L2/L3 用 wiremock 流式假端点（固定 token 序列与延迟） |
| 密钥 | 测试中只允许使用形如 `sk-test-*` 的假 Key；发现真实 Key 入库视为安全事故 |
| 并发 | 测试必须可并行执行（默认 `cargo test` 多线程），禁止用例间共享可变状态 |

---

## 6. 命令与工具链（基线）

```bash
cargo fmt --check                  # 格式门禁
cargo clippy --all-targets -- -D warnings   # 静态检查（含 unwrap_used / expect_used = deny）
cargo check                        # 提交前必过
cargo test                         # L1 + L2（或 cargo nextest run）
cargo tarpaulin --out Html         # 覆盖率报告（core ≥ 90%）
npx playwright test                # L3 E2E（需 tauri-driver 环境）
```

Clippy 强制配置（写入 workspace 根 `Cargo.toml` 或 `clippy.toml`，Phase 1 落地）：`unwrap_used = "deny"`、`expect_used = "deny"`、`pedantic` 级别逐 crate 评审开启。

---

## 7. CI 质量门禁（GitHub Actions，Phase 1 搭建）

| 流水线 | 触发 | 内容 | 合并门禁 |
| --- | --- | --- | --- |
| pr-check | 每个 PR | fmt + clippy + check + L1/L2 全量 | 必须全绿 |
| nightly-e2e | 每日定时 | L3 全量（macOS / Windows 矩阵）+ 性能基准 | 红灯时当日冻结新功能合并 |
| release | tag | 全平台打包 + E2E 冒烟 + 安装包校验 | 必须全绿 |

任何门禁失败时不允许 `git commit` 合入主干；禁止跳过 hooks 与 CI 检查。

---

## 8. 完成定义（Definition of Done）

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
| （其余条目待 QA 阶段登记） | | | | |

| 版本 | 日期 | 修订摘要 | 批准 |
| --- | --- | --- | --- |
| v1.0.0 | 2026-07-24 | 创世版：金字塔分工、左移流程、门禁与 DoD 确立 | QA Lead / TPM |
