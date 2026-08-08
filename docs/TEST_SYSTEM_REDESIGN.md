# EchoMind 测试体系全面重构方案 — TEST_SYSTEM_REDESIGN v1.0

> **文档定位**：本文件是测试体系全面审计与重构的唯一真相源。基于对现有测试资产的深度审计，设计能完全替代人工测试的自动化测试系统。
> 需求来源见 `docs/SRS_v1.0.md`；测试策略见 `docs/TEST_STRATEGY.md`；测试覆盖矩阵见 `docs/E2E_COVERAGE.md`。

| 项 | 内容 |
| --- | --- |
| 文档版本 | v1.0 |
| 生效日期 | 2026-07-29 |
| 维护者 | QA Lead |

---

## 一、现有测试体系全面审计

### 1.1 测试资产统计

| 层级 | 文件数 | 总行数 | 用例数（约） | 状态 |
| --- | --- | --- | --- | --- |
| L1 单元测试 (core) | 10 | 2,757 | ~98 (Free) / ~136 (Pro) | ✅ 覆盖良好 |
| L1 单元测试 (infra) | 2 | 1,084 | ~32 (Free) / ~47 (Pro) | ✅ 覆盖良好 |
| L2 集成测试 | 1 | 2,146 | ~25 (Free) / ~38 (Pro) | ⚠️ 有缺口 |
| L3-lite 桥接 E2E | 23 spec + 2 infra | 6,650 | ~307 | ⚠️ 21 个失败 |
| **合计** | **38** | **12,637** | **~450 (Free) / ~520 (Pro)** | — |

### 1.2 审计发现的关键问题

#### P0 — 体系性缺陷

| 编号 | 问题 | 影响 | 根因 |
| --- | --- | --- | --- |
| AUDIT-P0-001 | **无 Mock 与真实后端契约一致性验证** | Mock 行为可能偏离真实实现，桥接测试通过但真实使用失败 | `tauri-stub.js` 手写 mock 逻辑，无与 `commands.rs` 的自动契约对齐机制 |
| AUDIT-P0-002 | **无属性测试（Property-Based Testing）** | 仅示例测试，边界值覆盖靠人工枚举，遗漏组合爆炸场景 | 未引入 `proptest` / `quickcheck`，分块器/检索器/编码器等纯函数未做随机输入验证 |
| AUDIT-P0-003 | **无变异测试（Mutation Testing）** | 无法量化测试集有效性——测试通过不代表测试质量高 | 未引入 `cargo-mutants`，无法发现"虚假绿色"测试 |
| AUDIT-P0-004 | **无并发/竞态条件测试** | SQLite WAL 并发、CancellationToken 竞态、导入取消竞态未验证 | 仅单元测试原子操作，无 `loom` / `tokio::test::pause` 等并发测试工具 |
| AUDIT-P0-005 | **无崩溃恢复仿真测试** | Processing→Failed 僵尸清理仅有单元测试，无进程级崩溃仿真 | 无 kill -9 进程后重启验证、无断电仿真、无 WAL 损坏恢复测试 |

#### P1 — 覆盖率缺口

| 编号 | 问题 | 影响范围 | 补充方向 |
| --- | --- | --- | --- |
| AUDIT-P1-001 | **无性能回归 CI 门禁** | NFR-002/006/008/012 等性能需求仅 `#[ignore]` 手动跑 | 引入 nightly 性能基准 CI + 历史趋势对比 |
| AUDIT-P1-002 | **无可视化回归测试** | UI 变更无法自动检测像素级偏移 | 引入 Playwright screenshot comparison |
| AUDIT-P1-003 | **无安全模糊测试（Fuzzing）** | 路径遍历、XSS、FTS5 注入仅测试已知向量 | 引入 `cargo-fuzz` + OWASP ZAP 扫描 |
| AUDIT-P1-004 | **无无障碍自动化测试** | WCAG 对比度/ARIA/键盘导航/屏幕阅读器无自动化断言 | 引入 `axe-core` Playwright 集成 |
| AUDIT-P1-005 | **无覆盖率报告 CI** | core ≥ 90% 行覆盖率仅纸面承诺，CI 未执行 `tarpaulin` | 引入 `cargo-tarpaulin` + Codecov/Coveralls |
| AUDIT-P1-006 | **无状态机模型测试** | 文档状态机、输入框状态机、会话状态机无形式化验证 | 引入状态机模型 + 状态转换覆盖准则 |
| AUDIT-P1-007 | **无 Mock→Real 契约漂移检测** | tauri-stub.js 533 行手写逻辑 vs commands.rs 真实实现无自动校验 | 引入 IPC 契约快照测试（JSON schema 对齐） |
| AUDIT-P1-008 | **Draft 需求域零测试覆盖** | DS/IX/KB/A11Y/NAV/WS/EXP/ERR/OBS/WIN/I18N/HELP 共 12 个域 | Draft→Approved 后立即编写红灯测试 |

#### P2 — 测试质量问题

| 编号 | 问题 | 影响测试数 | 修复方向 |
| --- | --- | --- | --- |
| AUDIT-P2-001 | 21 个失败测试未修复 | 21/307 (6.8%) | 逐个分析失败根因，区分产品 Bug vs 测试 Bug |
| AUDIT-P2-002 | 测试时序竞争 | ~5 个 | 引入确定性等待（`waitFor` 替代 `waitForTimeout`） |
| AUDIT-P2-003 | Mock 耦合场景 | tauri-stub.js | 提取为数据驱动 mock（配置文件而非硬编码） |
| AUDIT-P2-004 | 无测试数据工厂 | 全部 E2E | 引入 Builder 模式 + Factory 函数 |
| AUDIT-P2-005 | 无 Flaky 检测机制 | 全部 E2E | 引入 retry + quarantine 策略 |
| AUDIT-P2-006 | 无测试报告仪表盘 | 全部 | 引入 Allure / HTML 报告 |
| AUDIT-P2-007 | "手动验证" 仍存在于 NFR | NFR-017/019/020 | 全部替换为自动化断言 |

### 1.3 SRS 需求域覆盖审计

| 域 | REQ 数 | 已实现 | 已测试 | 测试缺口 |
| --- | --- | --- | --- | --- |
| ING | 14 | 6 | 6 | 文件夹导入/大小限制/历史记录/内容更新/预览 8 个 Draft 未测 |
| VEC | 12 | 6 | 5 | 索引重建/统计/参数配置/模型切换 6 个 Draft 未测 |
| RAG | 18 | 8 | 8 | 消息操作/删除/引用增强/上下文管理/建议提问 10 个 Draft 未测 |
| UI | 8 | 8 | 8 | ✅ 覆盖完整 |
| LIC | 5 | 5 | 5 | ✅ 覆盖完整 |
| SEC | 12 | 12 | 10 | FTS5 注入防护/错误脱敏细节 2 个未充分覆盖 |
| NFR | 21 | 12 | 10 | OS 兼容/构建体积/增量编译 3 个仍手动 |
| ARCH | 6 | 6 | 6 | ✅ 覆盖完整 |
| DB | 1 | 1 | 1 | ✅ 覆盖完整 |
| PDF | 1 | 1 | 1 | ✅ 覆盖完整 |
| LLM | 2 | 2 | 1 | LLM 连接测试 L2 契约测试缺口 |
| VIZ | 4 | 4 | 4 | ✅ 覆盖完整 |
| MM | 5 | 5 | 5 | ✅ 覆盖完整 |
| AUDIT | 5 | 5 | 5 | ✅ 覆盖完整 |
| DS | 7 | 0 | 0 | 全部 Draft，全部未测 |
| IX | 7 | 2 | 2 | 5 个 Draft 未测 |
| KB | 5 | 0 | 0 | 全部 Draft，全部未测 |
| A11Y | 5 | 0 | 0 | 全部 Draft，全部未测 |
| NAV | 5 | 1 | 1 | 4 个 Draft 未测 |
| WS | 4 | 0 | 0 | 全部 Draft，全部未测 |
| EXP | 4 | 0 | 0 | 全部 Draft，全部未测 |
| ERR | 5 | 2 | 0 | 3 个 Draft 未测，2 个已实现未测 |
| OBS | 3 | 0 | 0 | 全部 Draft，全部未测 |
| WIN | 5 | 0 | 0 | 全部 Draft，全部未测 |
| I18N | 3 | 0 | 0 | 全部 Draft，全部未测 |
| HELP | 4 | 0 | 0 | 全部 Draft，全部未测 |
| **合计** | **~168** | **~84** | **~77** | **~91 个需求无自动化测试覆盖（54%）** |

---

## 二、先进测试系统架构设计

### 2.1 设计原则

1. **完全自动化**：零人工干预，从测试编写→执行→报告→门禁全自动
2. **分层隔离**：每层只测自己的职责，不跨层依赖
3. **确定性优先**：消除随机性、时序竞争、外部依赖
4. **快速反馈**：L1 < 5s，L2 < 30s，L3-lite < 60s，全量 < 10min
5. **可观测**：每次运行产出结构化报告、覆盖率、性能趋势
6. **自愈能力**：Flaky 自动隔离 + 3 次重试 + 自动报告
7. **防回归**：变异测试量化测试质量 + 性能趋势防退化
8. **发散覆盖**：属性测试自动生成边界组合 + 模糊测试探索未知路径

### 2.2 十层测试金字塔

```
┌─────────────────────────────────────────────────────────────────┐
│  L9  变异测试 (cargo-mutants)          — 测试质量量化           │
├─────────────────────────────────────────────────────────────────┤
│  L8  可视化回归 (Screenshot Diff)      — 像素级 UI 一致性      │
├─────────────────────────────────────────────────────────────────┤
│  L7  无障碍测试 (axe-core + ARIA)      — WCAG 2.1 AA 合规       │
├─────────────────────────────────────────────────────────────────┤
│  L6  安全测试 (Fuzzing + ZAP)          — 模糊测试 + 渗透扫描   │
├─────────────────────────────────────────────────────────────────┤
│  L5  混沌/容错测试 (Fault Injection)    — 崩溃恢复 + 网络故障   │
├─────────────────────────────────────────────────────────────────┤
│  L4  性能回归 (Benchmark CI)           — P95/吞吐量/内存趋势   │
├─────────────────────────────────────────────────────────────────┤
│  L3  真实 GUI E2E (tauri-driver)       — Linux/Windows 真实   │
├─────────────────────────────────────────────────────────────────┤
│  L3-lite  桥接 E2E (Playwright+Mock)   — 全平台 UI 行为断言    │
├─────────────────────────────────────────────────────────────────┤
│  L2.5  契约一致性测试 (Mock vs Real)    — IPC 契约漂移检测      │
├─────────────────────────────────────────────────────────────────┤
│  L2  集成测试 (mock_app + wiremock)    — 真实 SQLite + Mock LLM│
├─────────────────────────────────────────────────────────────────┤
│  L1.5  属性测试 (proptest)             — 随机输入 + 不变量验证 │
├─────────────────────────────────────────────────────────────────┤
│  L1  单元测试 (cargo test)             — 领域逻辑 + 纯函数     │
├─────────────────────────────────────────────────────────────────┤
│  L0  静态分析 (clippy/fmt/deny/grep)  — 编译时 + 规则检查      │
└─────────────────────────────────────────────────────────────────┘
```

### 2.3 各层详细规格

#### L0 — 静态分析层

| 检查项 | 工具 | CI 集成 | 门禁级别 |
| --- | --- | --- | --- |
| 代码格式 | `cargo fmt --check` | 每个 PR | deny |
| Lint 检查 | `cargo clippy --all-targets -- -D warnings` | 每个 PR | deny |
| 供应链安全 | `cargo deny check` | 每个 PR + 每日 | deny |
| 依赖方向审计 | `cargo tree -p echomind-core` (grep 断言) | 每个 PR | deny |
| async-trait 禁用 | `grep -r '#\[async_trait\]' crates/core/src` | 每个 PR | deny |
| CDN 审计 | `grep -r 'http\(s\)://' ui/index.html` | 每个 PR | deny |
| 硬编码色值审计 | 静态扫描 `ui/index.html` | 每个 PR | warn |
| unwrap 禁用 | `clippy::unwrap_used = "deny"` | 每个 PR | deny |

#### L1 — 单元测试层

**范围**：`crates/core`（领域逻辑）+ `crates/infra`（适配器内部逻辑）

**新增测试目录结构**：
```
crates/core/src/
├── chat_tests.rs              — 现有（增强）
├── chat_property_tests.rs     — 新增：proptest 随机输入
├── import_tests.rs            — 现有（增强）
├── import_concurrency_tests.rs — 新增：tokio 并发导入
├── retriever_tests.rs         — 现有（增强）
├── retriever_property_tests.rs — 新增：proptest 检索不变量
├── splitter_tests.rs          — 现有（增强）
├── splitter_property_tests.rs — 新增：proptest 分块不变量
├── stream_parse_tests.rs      — 现有（增强）
├── stream_parse_fuzz_tests.rs — 新增：模糊测试 SSE 解析
├── license_tests.rs           — 现有（增强）
├── license_fuzz_tests.rs      — 新增：模糊测试 License 解析
├── state_machine_tests.rs     — 新增：文档/会话/输入状态机
├── ... (现有其他测试文件)
```

**覆盖率门禁**：
- `crates/core` 行覆盖率 ≥ 95%（从 90% 提升）
- `crates/infra` 行覆盖率 ≥ 85%
- 每个公开函数至少 1 个测试
- 每个错误分支至少 1 个测试
- 每个边界值至少 3 个测试（min/normal/max）

#### L1.5 — 属性测试层（新增）

**工具**：`proptest` crate

**测试策略**：
- 为纯函数生成随机输入，验证不变量（Invariant）
- 自动发现边界值组合（人工枚举遗漏的场景）
- 每个属性测试运行 ≥ 256 个随机用例

**关键属性测试清单**：

| 编号 | 被测函数 | 不变量 | 输入域 |
| --- | --- | --- | --- |
| PROP-SPLIT-001 | `Splitter::split()` | chunks 拼接后内容 == 原文（无损） | 任意 UTF-8 文本 |
| PROP-SPLIT-002 | `Splitter::split()` | 每个 chunk 不含半个 UTF-8 字符 | 含 CJK/emoji/组合字符文本 |
| PROP-SPLIT-003 | `Splitter::split()` | 空 Vec 或每个 chunk 非空 | 任意文本 |
| PROP-EMBED-001 | `Embedder::embed()` | 返回向量维度 == 384 | 任意非空文本 |
| PROP-EMBED-002 | `Embedder::embed()` | 返回向量不含 NaN/Inf | 任意文本 |
| PROP-SEARCH-001 | `vector_search` | top-k 结果数 ≤ k | 任意查询向量 + k |
| PROP-SEARCH-002 | `cosine_similarity` | 结果 ∈ [-1, 1] | 任意两个向量 |
| PROP-SEARCH-003 | `cosine_similarity` | 自相似度 == 1.0 | 任意非零向量 |
| PROP-LICENSE-001 | `verify_license()` | 合法签名通过，篡改签名拒绝 | 任意 payload + 正确/错误签名 |
| PROP-STREAM-001 | `parse_sse()` | 完整 chunk 拼接后 == 原始 SSE 内容 | 任意 SSE 字节流（含切割点） |
| PROP-IMPORT-001 | `ImportService::import_one()` | 格式非法返回 Err，不 Panic | 任意文件路径 |
| PROP-FTS5-001 | `build_fts5_or_query()` | 输出不含未转义 FTS5 操作符 | 任意用户输入 |

#### L2 — 集成测试层（增强）

**范围**：`crates/tauri-app/tests/integration.rs` + 新增并发/恢复测试

**新增测试类别**：

| 编号 | 测试名 | 验证内容 |
| --- | --- | --- |
| TC-CONC-001 | 并发导入 50 个文件 | 无死锁、无数据丢失、状态一致 |
| TC-CONC-002 | 并发 chat + 导入 | 流式不被阻塞、状态不串 |
| TC-CONC-003 | 并发 delete + retry | 级联清理无竞态 |
| TC-CONC-004 | 并发 chat + abort | CancellationToken 无竞态 |
| TC-RECOV-001 | 进程崩溃后重启恢复 | Processing→Failed 僵尸清理 |
| TC-RECOV-002 | SQLite WAL 恢复 | 崩溃后数据完整性 |
| TC-RECOV-003 | 断网后离线功能 | 本地功能不受影响 |
| TC-RECOV-004 | 磁盘满错误处理 | 优雅降级不 Panic |
| TC-CONTRACT-001 | IPC 返回值 JSON schema | 所有命令返回值结构一致 |
| TC-CONTRACT-002 | Event payload JSON schema | 所有事件负载结构一致 |

#### L2.5 — 契约一致性测试（新增）

**目的**：确保 `tauri-stub.js` Mock 行为与 `commands.rs` 真实实现保持一致

**策略**：
1. 从 `commands.rs` 提取所有 IPC 命令签名 + 返回类型 → 生成 JSON Schema
2. `tauri-stub.js` 每个 mock 返回值通过 schema 校验
3. CI 自动运行 schema drift detection
4. 契约变更时强制更新 mock

```
tauri-stub.js mock 返回值
    ↓ JSON Schema 校验
commands.rs 真实返回类型 → serde_json::Value → Schema
    ↓ 不匹配 → CI 失败
```

#### L3-lite — 桥接 E2E（增强）

**增强方向**：
1. **数据驱动测试**：从 YAML/JSON 配置生成测试用例，消除硬编码
2. **Page Object 模式**：提取 UI 交互为可复用对象
3. **测试数据工厂**：Builder 模式生成测试文档/会话/配置
4. **Flaky 隔离**：自动重试 3 次，连续失败标记为 Flaky 并隔离
5. **截图回归**：关键页面截图对比
6. **无障碍扫描**：`axe-core` 自动 ARIA/对比度检查

#### L3 — 真实 GUI E2E（新增）

**工具**：WebdriverIO + tauri-driver

**平台**：Linux (ubuntu-latest) + Windows (windows-latest)

**测试范围**：
- 真实 IPC 调用（非 Mock）
- 真实 SQLite 读写
- 真真 ONNX 模型推理
- Mock LLM（wiremock 流式假端点）

#### L4 — 性能回归测试（新增）

**CI 流水线**：nightly-perf

| 基准 | 指标 | 阈值 | 工具 |
| --- | --- | --- | --- |
| 检索 P95 | 10k chunks, 384-dim | ≤ 500ms | `#[bench]` + criterion |
| HNSW P95 | 100k+ chunks | ≤ 10ms | `#[ignore]` 基准 |
| 首 Token 延迟 | 本地管线 | ≤ 500ms | 计时断言 |
| 冷启动 | 启动→可交互 | ≤ 2s | Playwright navigation timing |
| 内存峰值 | 导入/聊天/审计 | ≤ 512MB | `sysinfo` 采集 |
| 嵌入吞吐 | batch=100 | ≥ 50 vec/s | 计时断言 |
| UI 响应 | 点击→渲染 | ≤ 100ms | Playwright timing |
| 滚动帧率 | 流式渲染 | ≥ 30fps | Performance Observer API |

**退化检测**：
- 每次运行记录指标 → 与历史趋势对比
- 退化 > 10% → CI 失败
- 自动生成性能趋势图

#### L5 — 混沌/容错测试（新增）

**测试矩阵**：

| 编号 | 故障注入 | 验证 |
| --- | --- | --- |
| CHAOS-001 | 进程 kill -9 后重启 | Processing→Failed 恢复 |
| CHAOS-002 | SQLite WAL 截断 | 数据库恢复或明确错误 |
| CHAOS-003 | 网络断开（LLM 流式中） | 优雅中断 + 部分内容保留 |
| CHAOS-004 | 磁盘满（导入中） | 优雅错误 + 无 Panic |
| CHAOS-005 | OOM 仿真（限制内存） | 优雅降级 |
| CHAOS-006 | 模型下载中断（50% 后断网） | 全量重下（禁断点续传） |
| CHAOS-007 | SQLite 锁定（外部进程持锁） | busy_timeout 重试 → 超时错误 |
| CHAOS-008 | 时钟回拨（License 时效） | 不影响离线验证 |
| CHAOS-009 | 文件权限变更（运行中改为 000） | 优雅错误 |
| CHAOS-010 | 编码地狱（GBK/Shift-JIS/无效字节） | from_utf8_lossy 容错 |

#### L6 — 安全模糊测试（新增）

**工具**：`cargo-fuzz` + OWASP ZAP

| 编号 | 模糊目标 | 不变量 |
| --- | --- | --- |
| FUZZ-001 | 文件路径输入 | 不 Panic + 拒绝 `..` |
| FUZZ-002 | License Key 输入 | 不 Panic + 无信息泄露 |
| FUZZ-003 | SSE 字节流 | 解析器不崩溃 + 不丢字 |
| FUZZ-004 | Markdown 内容（XSS） | DOMPurify 消毒所有注入 |
| FUZZ-005 | FTS5 查询输入 | 无操作符注入 |
| FUZZ-006 | PDF 字节流 | 不 Panic + 返回 Err |
| FUZZ-007 | Unicode 文本 | 不 Panic + 合法 UTF-8 输出 |

#### L7 — 无障碍测试（新增）

**工具**：`@axe-core/playwright`

| 编号 | 检查项 | WCAG 标准 |
| --- | --- | --- |
| A11Y-001 | 所有交互元素 ARIA 语义 | 4.1.2 Name, Role, Value |
| A11Y-002 | 颜色对比度 ≥ 4.5:1 (正文) / 3:1 (大文本) | 1.4.3 Contrast (Minimum) |
| A11Y-003 | 键盘可达性（Tab 遍历） | 2.1.1 Keyboard |
| A11Y-004 | 焦点可见（focus 环） | 2.4.7 Focus Visible |
| A11Y-005 | 模态框 focus trap | 2.4.3 Focus Order |
| A11Y-006 | aria-live 流式内容 | 4.1.3 Status Messages |
| A11Y-007 | `prefers-reduced-motion` 支持 | 2.3.3 Animation from Interactions |
| A11Y-008 | 表单标签关联 | 3.3.2 Labels or Instructions |
| A11Y-009 | 页面标题/面包屑 | 2.4.2 Page Titled |
| A11Y-010 | 错误识别与建议 | 3.3.1 Error Identification |

#### L8 — 可视化回归测试（新增）

**工具**：Playwright `screenshot()` + `toHaveScreenshot()`

**截图基线**：
- 主界面（空状态）
- 主界面（含文档+会话）
- 配置向导（3 预设）
- 设置面板
- 付费墙
- 流式渲染中
- 审计报告
- 侧栏折叠
- 暗色主题
- 各错误状态

**退化检测**：像素差异 > 2% → CI 失败（需人工审查后更新基线）

#### L9 — 变异测试（新增）

**工具**：`cargo-mutants`

**策略**：
- 对 `crates/core` 全部公开函数注入变异
- 变异类型：`+` → `-`、`==` → `!=`、`if true` → `if false`、删除语句、返回值替换
- 每个变异必须被至少 1 个测试捕获（杀死）
- 变异存活率（Mutation Score）≥ 95%
- CI 流水线：weekly（全量）+ PR（变更文件）

---

## 三、发散性测试场景全覆盖矩阵

### 3.1 导入管线（ING/VEC/SEC）— 100 个场景

#### 正常路径（20）

| # | 场景 | 输入 | 预期 |
| --- | --- | --- | --- |
| 1 | 导入单 .md | test.md (1KB) | Indexed |
| 2 | 导入单 .txt | test.txt (1KB) | Indexed |
| 3 | 导入单 .pdf (Pro) | test.pdf (10KB) | Indexed |
| 4 | 批量导入 10 个 | 10×.md | 全部 Indexed |
| 5 | 拖拽导入 | drag-drop | 遮罩→列表更新 |
| 6 | 文件选择器导入 | dialog open | 列表更新 |
| 7 | 大 Markdown | 10MB .md | Indexed |
| 8 | 中文内容 .md | 中文 Markdown | Indexed + 无乱码 |
| 9 | 英文内容 .md | English Markdown | Indexed |
| 10 | 中英混排 .md | 中英混合 | Indexed + 无乱码 |
| 11 | 含代码块 .md | ```rust\nfn main() {} | Indexed + 代码完整 |
| 12 | 含表格 .md | Markdown 表格 | Indexed |
| 13 | 含 mermaid .md | ```mermaid\nflowchart | Indexed |
| 14 | 含 LaTeX .md | $E=mc^2$ | Indexed |
| 15 | 含 HTML .md | `<div>html</div>` | Indexed |
| 16 | 深层标题 .md | #→##→###→#### | Indexed + 章节感知 |
| 17 | YAML front matter .md | `---\ntitle: x\n---` | Indexed |
| 18 | 多行列表 .md | 100 项列表 | Indexed |
| 19 | 嵌套引用 .md | `> > > 深嵌套` | Indexed |
| 20 | 图片 base64 .md | `![alt](data:image/png;base64,...)` | Indexed |

#### 边界值（30）

| # | 场景 | 输入 | 预期 |
| --- | --- | --- | --- |
| 21 | 空文件 .md | 0 字节 | Err "文件为空" |
| 22 | 空文件 .txt | 0 字节 | Err "文件为空" |
| 23 | 仅空白 .md | " \n\t\r\n " | Err 或空 chunks |
| 24 | 单字符 .md | "A" | Indexed (1 chunk) |
| 25 | 最大路径 | OS 限制长度路径 | Indexed |
| 26 | Unicode 文件名 | 中文.md / 日本語.txt | Indexed |
| 27 | 空格文件名 | "my file.md" | Indexed |
| 28 | 特殊字符名 | `!@#$%^&().md` | Indexed |
| 29 | 大文件 .md | 100MB | Indexed (警告) |
| 30 | 超大 .txt | 500MB+ | Err "文件过大" |
| 31 | \r\n 行尾 | Windows 换行 | Indexed |
| 32 | \r 行尾 | 旧 Mac 换行 | Indexed |
| 33 | \n 行尾 | Unix 换行 | Indexed |
| 34 | 混合行尾 | \r\n + \r + \n | Indexed |
| 35 | BOM 文件 | UTF-8 BOM + 内容 | Indexed (BOM 去除) |
| 36 | 混合编码 | UTF-8 + GBK | from_utf8_lossy 容错 |
| 37 | 非法 UTF-8 | 0xFF 0xFE | from_utf8_lossy |
| 38 | 仅代码块 | ```\ncode\n``` | Indexed |
| 39 | 深嵌套列表 | 10 层缩进 | Indexed |
| 40 | 超长行 | 单行 100KB | Indexed (分块切分) |
| 41 | 无换行 .txt | 单行 1MB | Indexed (分块切分) |
| 42 | 百万行 .txt | 100万行 | Indexed (性能验证) |
| 43 | .MD 大写扩展 | file.MD | Indexed (大小写不敏感) |
| 44 | .Md 混合扩展 | file.Md | Indexed |
| 45 | 零页 PDF | 0 页 PDF | Err 或空内容 |
| 46 | 千页 PDF | 1000 页 | Indexed (性能) |
| 47 | 扫描 PDF | 仅图片无文本 | OCR 提取 (Pro) |
| 48 | 表格 PDF | 复杂表格 | VLM 提取 (Pro) |
| 49 | 公式 PDF | 数学公式 | VLM 提取 (Pro) |
| 50 | 加密 PDF | 密码保护 | Err "无法解密" |

#### 错误/异常路径（25）

| # | 场景 | 输入 | 预期 |
| --- | --- | --- | --- |
| 51 | 不存在文件 | /nonexistent.md | Err "文件不存在" |
| 52 | 目录路径 | /tmp/ | Err "不是文件" |
| 53 | 符号链接 | symlink → real.md | Indexed (canonicalize) |
| 54 | 设备文件 | /dev/null | Err "不是普通文件" |
| 55 | 路径遍历 | ../../etc/passwd | Err "拒绝路径遍历" |
| 56 | Null 字节路径 | "file\x00.md" | Err |
| 57 | .exe 格式 | malware.exe | Err "不支持的格式" |
| 58 | .docx 格式 | doc.docx | Err "不支持的格式" |
| 59 | .html 格式 | page.html | Err "不支持的格式" |
| 60 | 无扩展名 | filename | Err "不支持的格式" |
| 61 | Free 版 PDF | test.pdf (is_pro=false) | Err "PRO_REQUIRED" |
| 62 | Free 版第 51 文件 | 51st file | Err "LIMIT_REACHED" |
| 63 | 重复内容导入 | same content | 跳过 + "已存在" |
| 64 | 同名不同内容 | same name, diff content | Indexed |
| 65 | 文件被锁定 | flock by other process | Err 或等待 |
| 66 | 网络驱动器 | SMB/NFS path | Indexed 或 Err |
| 67 | 磁盘满 | disk full | Err "磁盘空间不足" |
| 68 | 只读数据目录 | chmod 0500 data_dir | Err |
| 69 | SQLite 锁定 | external lock | busy_timeout 重试 |
| 70 | 模型下载失败 | HF + mirror 不可达 | Err "下载失败" |
| 71 | 模型未初始化 | no init_embedder | chat 触发懒加载 |
| 72 | 损坏 SQLite | corrupt db file | Err "数据库损坏" |
| 73 | 缺失数据目录 | deleted data_dir | 自动重建 |
| 74 | 权限不足 | chmod 0000 source | Err "权限不足" |
| 75 | 伪造 PDF | random bytes named .pdf | Err "非 PDF 格式" |

#### 并发/竞态（15）

| # | 场景 | 验证 |
| --- | --- | --- |
| 76 | 100 文件并发导入 | 无死锁、无丢失、状态一致 |
| 77 | 导入 + 聊天并行 | 不互相阻塞 |
| 78 | 导入 + 审计并行 | 不互相阻塞 |
| 79 | 导入 + 删除并行 | 级联清理无竞态 |
| 80 | 导入 + 重试并行 | 同文档不冲突 |
| 81 | 同文件双导入 | 去重，仅 1 个入库 |
| 82 | 导入 + 关闭应用 | 已完成部分保留 |
| 83 | 导入 + 设置更新 | 不互相干扰 |
| 84 | 同文档并发重试 | 仅 1 次成功 |
| 85 | 同文档并发删除+重试 | 删除优先或明确错误 |
| 86 | 同文档并发删除+审计 | 删除优先或明确错误 |
| 87 | 同文档并发删除+聊天 | 删除后聊天返回空结果 |
| 88 | 并发导入相同内容 | 去重，仅 1 个入库 |
| 89 | 并发聊天不同会话 | 各自独立流式 |
| 90 | 并发 abort 不同会话 | 各自独立中断 |

#### 状态机（10）

| # | 转换 | 验证 |
| --- | --- | --- |
| 91 | Pending→Processing→Indexed | 状态正确推进 |
| 92 | Pending→Processing→Failed | 错误状态正确 |
| 93 | Failed→Pending→Processing→Indexed | 重试成功 |
| 94 | Indexed→Deleted | 级联清理 |
| 95 | Processing→Failed (崩溃) | 重启后恢复 |
| 96 | Indexed→Processing→Indexed | 重建索引 |
| 97 | 并发状态转换 | 无非法状态 |
| 98 | 重启后状态恢复 | settings 表持久化 |
| 99 | 无效转换拒绝 | 不允许 Indexed→Pending |
| 100 | 全状态循环 | import→index→delete→re-import |

### 3.2 聊天/RAG 管线（RAG）— 80 个场景

#### 正常路径（15）

| # | 场景 | 验证 |
| --- | --- | --- |
| 101 | 单问单答 + 来源 | sources 非空 + token 流式 |
| 102 | 多轮对话（3 轮） | 上下文携带历史 |
| 103 | 英文提问 | 回答含英文 |
| 104 | 中文提问 | 回答含中文 |
| 105 | 混合语言提问 | 语言跟随 |
| 106 | 代码相关问题 | 代码块渲染 |
| 107 | 数学相关问题 | LaTeX 渲染 |
| 108 | 流程图问题 | Mermaid 渲染 |
| 109 | 数据对比问题 | Markdown 表格 |
| 110 | 空检索结果 | 固定拒答提示 |
| 111 | 部分检索结果 | 阈值过滤 |
| 112 | 混合检索（向量+关键词） | RRF 融合 |
| 113 | Chunk Expansion | 相邻 chunk 扩展 |
| 114 | 上下文嵌入 | 文档名前缀 |
| 115 | 引用块回溯 | 文档名+分数+预览 |

#### 边界值（20）

| # | 场景 | 输入 | 预期 |
| --- | --- | --- | --- |
| 116 | 空白提问 | "   " | 不发送 |
| 117 | 超长提问 | 10000 字 | 正常处理 |
| 118 | 仅 Emoji | "🚀🎉" | 正常处理 |
| 119 | HTML 标签 | `<b>test</b>` | 安全渲染 |
| 120 | Script 标签 | `<script>alert(1)` | DOMPurify 消毒 |
| 121 | SQL 注入 | `'; DROP TABLE--` | FTS5 转义 |
| 122 | 路径遍历 | `../../etc/passwd` | 安全处理 |
| 123 | Markdown 注入 | `[click](javascript:alert(1))` | 消毒 |
| 124 | Mermaid 语法 | ```mermaid\nflowchart | 渲染图表 |
| 125 | LaTeX 语法 | `$E=mc^2$` | 渲染公式 |
| 126 | 全大写 | "HELLO WORLD" | 正常处理 |
| 127 | 重复词 | "test test test" | 正常处理 |
| 128 | CJK Unicode | "中文测试" | 正常处理 |
| 129 | RTL 文本 | "مرحبا" | 正常处理 |
| 130 | 组合字符 | é (e + ´) | 正常处理 |
| 131 | 零宽字符 | "te\u200Bst" | 正常处理 |
| 132 | 二进制数据 | \x00\x01\x02 | 安全处理 |
| 133 | Base64 内容 | "dGVzdA==" | 正常处理 |
| 134 | 特殊标点 | "¿Qué? «test» — 中文" | 正常处理 |
| 135 | 控制字符 | \x01\x02\x03 | 安全处理 |

#### 错误/异常路径（20）

| # | 场景 | 验证 |
| --- | --- | --- |
| 136 | LLM 不可达 | Err "无法连接" |
| 137 | 401 无效 Key | Err "AUTH: API Key 无效" |
| 138 | 400 错误请求 | Err 含状态码 |
| 139 | 429 限流 | Err "请求过频" |
| 140 | 500 服务器错误 | Err 含状态码 |
| 141 | 非 JSON 响应 | Err "解析失败" |
| 142 | 畸形 SSE | 不崩溃 + 不丢字 |
| 143 | 连接超时 30s | Err "连接超时" |
| 144 | 整体超时 5min | Err "请求超时" |
| 145 | 空响应 | Err "空响应" |
| 146 | 超长响应 | 正常流式渲染 |
| 147 | 非法 UTF-8 | from_utf8_lossy |
| 148 | 流中断 mid-token | 部分内容保留 |
| 149 | 多字节字符切割 | 不丢字 |
| 150 | XSS 响应 | DOMPurify 消毒 |
| 151 | 模型未加载 | 触发懒加载 |
| 152 | 模型下载中 | 阻塞或进度提示 |
| 153 | SQLite 锁定 | busy_timeout 重试 |
| 154 | 空知识库 | Err "知识库为空" |
| 155 | 全部 Failed 文档 | Err 或空结果 |

#### 并发/状态（15）

| # | 场景 | 验证 |
| --- | --- | --- |
| 156 | 多会话并行聊天 | 各自独立流式 |
| 157 | 聊天 + 导入并行 | 不互相阻塞 |
| 158 | 聊天 + 删除并行 | 删除后结果调整 |
| 159 | abort + 立即新聊天 | 无残留状态 |
| 160 | 流式中切换会话 | 前一个保留部分内容 |
| 161 | 流式中删除会话 | 优雅处理 |
| 162 | Idle→Streaming→Done | 状态正确 |
| 163 | Idle→Streaming→Aborted | 中断+保留 |
| 164 | Idle→Streaming→Error | 错误恢复 |
| 165 | Streaming 中 Enter | 不触发新 send |
| 166 | 连续快速发送 | 仅首个生效 |
| 167 | abort ≤200ms | 立即中断 |
| 168 | 中断后输入框恢复 | 可立即提问 |
| 169 | 中断后内容持久化 | 部分内容入库 |
| 170 | 多轮上下文截断 | 保留首轮+最近N轮 |

#### 富内容渲染（10）

| # | 场景 | 验证 |
| --- | --- | --- |
| 171 | Mermaid 流程图 | SVG 渲染 |
| 172 | Mermaid 甘特图 | SVG 渲染 |
| 173 | Mermaid 序列图 | SVG 渲染 |
| 174 | Mermaid 语法错误 | 优雅提示 |
| 175 | KaTeX 行内公式 | 上标渲染 |
| 176 | KaTeX 块级公式 | 居中渲染 |
| 177 | KaTeX 化学方程式 | mhchem 渲染 |
| 178 | Chart.js 表格→图表 | 切换按钮 + 图表 |
| 179 | 代码块语法高亮 | highlight.js |
| 180 | 半截 Markdown | 不布局错乱 |

### 3.3 审计管线（AUDIT）— 30 个场景

| # | 场景 | 验证 |
| --- | --- | --- |
| 181 | 有矛盾文档审计 | 矛盾清单 + 报告 |
| 182 | 无矛盾文档审计 | "未发现明显矛盾" |
| 183 | 混合内容文档审计 | 数值/因果/定义声明 |
| 184 | 大文档审计 (100+ chunks) | 进度反馈 + 性能 |
| 185 | 小文档审计 (1 chunk) | 无矛盾报告 |
| 186 | 英文文档审计 | 英文声明提取 |
| 187 | 中文文档审计 | 中文声明提取 |
| 188 | Pro 许可审计 | 正常执行 |
| 189 | 取消审计 | 部分报告保留 |
| 190 | 审计不存在文档 | Err |
| 191 | 审计空文档 (0 chunks) | Err |
| 192 | 审计 Failed 文档 | Err 或提示 |
| 193 | 审计 Processing 文档 | Err 或等待 |
| 194 | 无 Pro 许可审计 | Err "Pro 功能" |
| 195 | 无 LLM 配置审计 | Err "未配置 LLM" |
| 196 | LLM 返回错误 | 优雅降级 |
| 197 | 模型未加载 | 触发懒加载 |
| 198 | SQLite 锁定 | busy_timeout 重试 |
| 199 | LLM JSON 解析失败 | 跳过该 chunk |
| 200 | 全部声明语义无关 | 无矛盾报告 |
| 201 | 声明跨语言 | 语义相似度计算 |
| 202 | 审计三阶段事件 | extracting/comparing/reporting |
| 203 | 审计进度实时显示 | current/total |
| 204 | abort_audit 取消 | 立即停止 |
| 205 | 取消后输入框恢复 | 可立即操作 |
| 206 | 审计完成 chat_done | Block 完成态 |
| 207 | 报告 DOMPurify 消毒 | 无 XSS |
| 208 | 报告含免责声明 | "AI 辅助生成" |
| 209 | 报告含严重等级 | 🔴/🟡/🟢 |
| 210 | 报告含来源追溯 | chunk 序号+章节路径 |

### 3.4 UI/UX/无障碍（UI/DS/IX/A11Y）— 60 个场景

#### 视觉设计（15）

| # | 场景 | 验证 |
| --- | --- | --- |
| 211 | 6 色板 token | getComputedStyle 断言 |
| 212 | 字号层级 | body 14px / .md 14px / h1 20.3px |
| 213 | 行高 | .md 1.8 / input 1.5 / code 1.65 |
| 214 | 字重 | 400/500/600/700 |
| 215 | 间距网格 | 4px 基础网格 |
| 216 | 圆角 | 4/8/12/16/24/9999px |
| 217 | 阴影 | shadow-lg / shadow-2xl |
| 218 | fade-in 动画 | 250ms ease-out |
| 219 | thinkingPulse 动画 | 1.4s infinite |
| 220 | sidebar 过渡 | 200ms |
| 221 | transition-colors | 150ms |
| 222 | 图标 Unicode | ＋/‹/›/⚙/■/⇩/◈/×/↻/🔍 |
| 223 | 暗色主题 | 背景透明+文字清晰 |
| 224 | 高对比度模式 | prefers-contrast: more |
| 225 | reduced-motion | prefers-reduced-motion: reduce |

#### 交互行为（20）

| # | 场景 | 验证 |
| --- | --- | --- |
| 226 | 拖拽进入 | 遮罩显示 |
| 227 | 拖拽离开 | 遮罩消失+不导入 |
| 228 | 拖拽释放 | 遮罩消失+导入 |
| 229 | Tab 遍历 | 从左到右、从上到下 |
| 230 | Enter 发送 | 非空时 send() |
| 231 | Shift+Enter | 换行不发送 |
| 232 | Esc 关闭 Modal | 优先级 VLM>Paywall>Settings |
| 233 | Esc 取消操作 | abort_chat/abort_audit |
| 234 | Cmd/Ctrl 快捷键 | N/O/,/// |
| 235 | 右键菜单 | 弹出+边界偏移+Esc 关闭 |
| 236 | hover 反馈 | 背景色/opacity 变化 |
| 237 | Modal 焦点管理 | 打开聚焦+关闭恢复 |
| 238 | focus trap | Modal 内 Tab 循环 |
| 239 | 自动滚动 | 流式生成中滚到底部 |
| 240 | 手动滚动暂停 | 用户上滚→暂停自动 |
| 241 | 新消息提示 | "↓ 新消息"按钮 |
| 242 | 侧栏折叠 | 240px→56px |
| 243 | 输入框自适应 | 1→6 行→160px 上限 |
| 244 | 复制按钮 | "复制"→"已复制 ✓"→1.5s 恢复 |
| 245 | 确认框 500ms 防误触 | disabled 500ms |

#### 无障碍（15）

| # | 场景 | 验证 |
| --- | --- | --- |
| 246 | ARIA button/role | 全部可点击元素 |
| 247 | ARIA dialog | Modal role+aria-modal |
| 248 | ARIA progressbar | 进度条 role+valuenow |
| 249 | ARIA alert | Toast role="alert" |
| 250 | aria-live 流式 | aria-live="polite" |
| 251 | aria-label 图标 | 有 aria-label |
| 252 | 纯键盘操作 | 无鼠标完成全流程 |
| 253 | 对比度正文 ≥4.5:1 | 自动化检测 |
| 254 | 对比度大文本 ≥3:1 | 自动化检测 |
| 255 | 对比度按钮 ≥4.5:1 | 自动化检测 |
| 256 | focus 可见 | 2px outline + 2px offset |
| 257 | Tab 顺序 | 视觉顺序一致 |
| 258 | 装饰元素 aria-hidden | aria-hidden="true" |
| 259 | 屏幕阅读器播报 | aria-live 区域更新 |
| 260 | disabled 对比度 ≥2:1 | 自动化检测 |

#### 状态/反馈（10）

| # | 场景 | 验证 |
| --- | --- | --- |
| 261 | 空知识库引导 | 文案+格式说明 |
| 262 | 空对话引导 | ◈ 图标+标题+描述 |
| 263 | Toast 4.2s 消失 | setTimeout |
| 264 | Toast 堆叠 | space-y-2 |
| 265 | Toast 类型 | info/success/error |
| 266 | 错误脱敏 | sk-**** + 路径脱敏 |
| 267 | 进度条显示 | 导入进度 |
| 268 | 骨架屏 | animate-pulse |
| 269 | 确认对话框 | 标题+不可撤销+确认/取消 |
| 270 | 思考指示器 | 脉冲动画+chat_phase 文案 |

### 3.5 安全/隐私（SEC）— 30 个场景

| # | 场景 | 验证 |
| --- | --- | --- |
| 271 | 路径遍历 `../` | 拒绝 |
| 272 | 路径遍历 `..\\` (Win) | 拒绝 |
| 273 | Null 字节注入 | 拒绝 |
| 274 | 符号链接攻击 | canonicalize |
| 275 | 设备文件 | 拒绝 |
| 276 | Script 注入 | DOMPurify |
| 277 | onerror 注入 | DOMPurify |
| 278 | javascript: 协议 | DOMPurify |
| 279 | Iframe 注入 | DOMPurify |
| 280 | SVG 脚本 | DOMPurify |
| 281 | Mermaid XSS | securityLevel: strict |
| 282 | KaTeX XSS | DOMPurify 后渲染 |
| 283 | Data URI 注入 | DOMPurify |
| 284 | CSS 注入 | 白名单 |
| 285 | FTS5 操作符注入 | 双引号转义 |
| 286 | API Key 不在日志 | grep 断言 |
| 287 | API Key 不在错误 | sanitizeError |
| 288 | API Key 加密存储 | base64(nonce‖ciphertext) |
| 289 | 文件路径不泄露 | 仅显示文件名 |
| 290 | 无遥测 | 网络监控 |
| 291 | 无外部请求 | 仅用户配置端点 |
| 292 | 数据目录 0700 | 权限检查 |
| 293 | 密钥文件 0600 | 权限检查 |
| 294 | cargo deny 无漏洞 | advisories |
| 295 | 许可证合规 | MIT 兼容 |
| 296 | 无 git 依赖 | unknown-git = deny |
| 297 | 无 yanked crate | yanked = deny |
| 298 | 用户数据不出域 | 网络 白名单 |
| 299 | OCR 完全本地 | 零网络请求 |
| 300 | PDF 渲染完全本地 | 零网络请求 |

### 3.6 性能/可扩展性（NFR）— 25 个场景

| # | 场景 | 验证 |
| --- | --- | --- |
| 301 | 冷启动 ≤2s | navigation timing |
| 302 | 首 token ≤3s (网络) | 事件时间戳 |
| 303 | 首 token ≤500ms (本地) | 计时 |
| 304 | 点击响应 ≤100ms | Playwright timing |
| 305 | 输入响应 ≤16ms/字符 | timing |
| 306 | 拖拽遮罩 ≤100ms | timing |
| 307 | 滚动 ≥30fps | Performance Observer |
| 308 | DB 不阻塞 UI | spawn_blocking |
| 309 | 10k chunks P95 ≤500ms | 基准测试 |
| 310 | 100k+ HNSW P95 ≤10ms | 基准测试 |
| 311 | 嵌入吞吐 ≥50 vec/s | 计时 |
| 312 | 并行嵌入 ≥4x 单会话 | 计时 |
| 313 | 内存峰值 ≤512MB | sysinfo |
| 314 | 二进制 ≤50MB (macOS) | 文件大小 |
| 315 | ui/vendor ≤5MB | 目录大小 |
| 316 | 增量编译 ≤30s | 计时 |
| 317 | cargo check ≤10s | 计时 |
| 318 | cargo clippy ≤60s | 计时 |
| 319 | 连接池 max_size=8 | 配置断言 |
| 320 | busy_timeout=5000ms | 配置断言 |
| 321 | WAL 模式 | PRAGMA journal_mode |
| 322 | ONNX 模型不打包 | 二进制大小 |
| 323 | rAF 节流 | renderScheduled 标志 |
| 324 | 级联删除无孤儿 | 外键约束 |
| 325 | 长会话 100+ 消息 | 性能不退化 |

### 3.7 崩溃恢复/容错（NFR）— 15 个场景

| # | 场景 | 验证 |
| --- | --- | --- |
| 326 | 导入中崩溃 | Processing→Failed 恢复 |
| 327 | 嵌入中崩溃 | 部分 vector 保留 |
| 328 | 聊天中崩溃 | 部分内容保留 |
| 329 | 审计中崩溃 | 部分报告保留 |
| 330 | 删除中崩溃 | 级联清理完成 |
| 331 | 重试中崩溃 | 状态恢复 |
| 332 | SQLite WAL 恢复 | 数据完整性 |
| 333 | SQLite 损坏 | 明确错误 |
| 334 | 断电仿真 | 数据一致性 |
| 335 | OOM 仿真 | 优雅降级 |
| 336 | 磁盘满 | 优雅错误 |
| 337 | 模型下载中断 | 全量重下 |
| 338 | LLM 流中断 | 部分保留 |
| 339 | 网络断开 | 离线模式 |
| 340 | SQLite 锁定 | busy_timeout 重试 |

### 3.8 跨平台/国际化（NFR-017/I18N）— 15 个场景

| # | 场景 | 验证 |
| --- | --- | --- |
| 341 | macOS arm64 | 编译+运行 |
| 342 | macOS x64 | 编译+运行 |
| 343 | Windows x64 | 编译+运行 |
| 344 | Linux x64 | 编译+运行 |
| 345 | Unix 路径处理 | `/` 分隔符 |
| 346 | Windows 路径处理 | `\` 分隔符 |
| 347 | Unix 权限 | 0700/0600 |
| 348 | Windows ACL | 跳过权限 |
| 349 | 中文 UI | 文案正确 |
| 350 | 英文 UI | 文案正确 |
| 351 | 混合语言 | 正确处理 |
| 352 | 日期本地化 | YYYY-MM-DD HH:mm |
| 353 | 文件大小格式 | B/KB/MB |
| 354 | 千分位分隔 | 1,234 |
| 355 | 缺失翻译回退 | key 回退 |

### 3.9 数据完整性（DB/FTS5/向量）— 15 个场景

| # | 场景 | 验证 |
| --- | --- | --- |
| 356 | 向量 BLOB 编解码 | f32 bytes ↔ Vec<f32> |
| 357 | 余弦相似度计算 | [-1, 1] 范围 |
| 358 | 零向量处理 | 不 Panic |
| 359 | NaN/Inf 向量 | 不 Panic |
| 360 | 维度不匹配 | Err |
| 361 | FTS5 trigram | 中英文子串匹配 |
| 362 | 短查询 LIKE 回退 | <3 字符 |
| 363 | FTS5 删除同步 | 文档删除后不命中 |
| 364 | 外键级联 | ON DELETE CASCADE |
| 365 | 事务回滚 | 错误后数据一致 |
| 366 | 索引状态机 | Pending→Processing→Indexed |
| 367 | settings 持久化 | 重启后恢复 |
| 368 | 会话+消息级联 | 删会话→删消息 |
| 369 | 文档+chunk+向量级联 | 删文档→删全部 |
| 370 | 空向量存储 | 空 Vec 处理 |

### 3.10 License/配额（LIC）— 15 个场景

| # | 场景 | 验证 |
| --- | --- | --- |
| 371 | 合法 License 激活 | is_pro=true |
| 372 | 篡改签名拒绝 | Err |
| 373 | 篡改 payload 拒绝 | Err |
| 374 | 重启后恢复 | is_pro 持久化 |
| 375 | 停用 License | is_pro=false |
| 376 | 停用后重新激活 | is_pro=true |
| 377 | 空 License Key | Err |
| 378 | 非 base64 Key | Err |
| 379 | 超长 Key | Err |
| 380 | Free 49 文件 | 成功 |
| 381 | Free 50 文件 | 成功 |
| 382 | Free 51 文件 | LIMIT_REACHED |
| 383 | Free PDF | PRO_REQUIRED |
| 384 | 删除后配额释放 | count 减少 |
| 385 | 并发配额检查 | 无竞态 |

---

## 四、测试基础设施设计

### 4.1 测试数据工厂

```rust
// crates/core/src/test_data/factory.rs
pub struct DocumentBuilder {
    name: String,
    content: String,
    format: FileFormat,
    size: usize,
    encoding: Encoding,
    // ...
}

impl DocumentBuilder {
    pub fn new() -> Self { /* defaults */ }
    pub fn with_name(mut self, name: &str) -> Self { self.name = name.into(); self }
    pub fn with_content(mut self, content: &str) -> Self { self.content = content.into(); self }
    pub fn md() -> Self { Self::new().with_format(FileFormat::Markdown) }
    pub fn txt() -> Self { Self::new().with_format(FileFormat::Text) }
    pub fn pdf() -> Self { Self::new().with_format(FileFormat::Pdf) }
    pub fn empty() -> Self { Self::new().with_content("") }
    pub fn large(size_mb: usize) -> Self { /* generate large content */ }
    pub fn unicode() -> Self { /* CJK + emoji + RTL */ }
    pub fn malicious() -> Self { /* XSS + path traversal */ }
    pub fn build(self) -> TempFile { /* write to temp file */ }
}
```

### 4.2 E2E Page Object 模式

```javascript
// e2e-tests/pages/SidebarPage.js
class SidebarPage {
  constructor(page) { this.page = page; }
  async collapse() { await this.page.locator('#collapseBtn').click(); }
  async expand() { await this.page.locator('#collapseBtn').click(); }
  async getWidth() { return await this.page.locator('#sidebar').evaluate(el => el.offsetWidth); }
  async newConversation() { await this.page.locator('#newChatBtn').click(); }
  async openSettings() { await this.page.locator('#settingsBtn').click(); }
  // ...
}

// e2e-tests/pages/ChatPage.js
class ChatPage {
  constructor(page) { this.page = page; }
  async send(message) { /* fill + click send */ }
  async abort() { await this.page.locator('#stopBtn').click(); }
  async waitForStream() { /* wait for sendBtn visible */ }
  async getSources() { /* extract sources */ }
  async getLastMessage() { /* extract last assistant block */ }
  // ...
}
```

### 4.3 Mock 契约校验

```javascript
// e2e-tests/contract/schema-validator.js
import { schemaFromZod } from 'zod-to-json-schema';

// 从 commands.rs 提取的 IPC 返回类型 schema
const IPC_SCHEMAS = {
  get_settings: { has_llm_config: 'boolean', base_url: 'string', model: 'string', api_key_masked: 'string' },
  get_documents: [{ id: 'string', file_name: 'string', file_path: 'string', file_hash: 'string', status: 'string', created_at: 'string' }],
  // ... 全部 24 命令
};

// 每个 mock 返回值运行时校验
export function validateMockReturn(cmd, value) {
  const schema = IPC_SCHEMAS[cmd];
  if (!schema) throw new Error(`No schema for command: ${cmd}`);
  // Deep validate value against schema
}
```

### 4.4 CI 流水线设计

```yaml
# .github/workflows/comprehensive-test.yml
jobs:
  # L0: 静态分析（每个 PR）
  static-analysis:
    steps:
      - cargo fmt --check
      - cargo clippy --all-targets -- -D warnings
      - cargo deny check
      - ./scripts/audit-deps.sh
      - ./scripts/audit-cdn.sh
      - ./scripts/audit-colors.sh

  # L1+L1.5: 单元+属性测试（每个 PR）
  unit-tests:
    strategy:
      matrix:
        features: ['', '--features echomind-tauri-app/pro']
    steps:
      - cargo test
      - cargo test --features echomind-tauri-app/pro
      - cargo test --doc

  # L1 覆盖率（每个 PR）
  coverage:
    steps:
      - cargo tarpaulin --out lcov --workspace --exclude echomind-tauri-app
      - bash <(curl -s https://codecov.io/bash)

  # L2: 集成测试（每个 PR）
  integration-tests:
    steps:
      - cargo test --test integration

  # L3-lite: 桥接 E2E（每个 PR）
  e2e-bridge:
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    steps:
      - cd e2e-tests && npm ci
      - npx playwright test

  # L4: 性能回归（nightly）
  performance:
    schedule: '0 2 * * *'
    steps:
      - cargo test -- --ignored --test perf
      - ./scripts/perf-trend.sh

  # L5: 混沌测试（nightly）
  chaos:
    schedule: '0 3 * * *'
    steps:
      - cargo test --test chaos -- --test-threads=1

  # L6: 安全模糊测试（weekly）
  security-fuzz:
    schedule: '0 4 * * 1'
    steps:
      - cargo fuzz run --all-features

  # L9: 变异测试（weekly）
  mutation:
    schedule: '0 5 * * 1'
    steps:
      - cargo mutants --workspace --exclude echomind-tauri-app
```

### 4.5 测试报告仪表盘

- **Allure Report**：测试执行结果 + 趋势 + Flaky 检测
- **Codecov**：代码覆盖率 + 行覆盖趋势
- **Cargo Mutants Dashboard**：变异存活率趋势
- **Performance Dashboard**：P95/吞吐量/内存趋势图
- **Lighthouse CI**：性能评分 + 可访问性评分 + 最佳实践评分

---

## 五、实施路线图

### Phase 1：修复 + 增强（1-2 周）

1. 修复 21 个失败测试
2. 修复测试时序竞争（`waitFor` 替代 `waitForTimeout`）
3. 引入 Page Object 模式重构 E2E
4. 引入测试数据工厂
5. 增强 tauri-stub.js 为数据驱动 mock
6. 添加 IPC 契约 schema 校验

### Phase 2：属性测试 + 覆盖率（2-3 周）

1. 引入 `proptest`，编写 12 个关键属性测试
2. 引入 `cargo-tarpaulin` CI 集成
3. 达到 core ≥ 95% 行覆盖率
4. 补充并发测试（`tokio::test` + `loom`）
5. 补充崩溃恢复测试

### Phase 3：高级测试（3-4 周）

1. 引入 `cargo-mutants` 变异测试
2. 引入 `@axe-core/playwright` 无障碍测试
3. 引入 Playwright screenshot 可视化回归
4. 引入 `cargo-fuzz` 安全模糊测试
5. 引入 nightly 性能基准 CI
6. 引入 Lighthouse CI

### Phase 4：Draft 需求覆盖（持续）

1. DS 域 7 个 Draft → 实现 + 红灯测试
2. IX 域 5 个 Draft → 实现 + 红灯测试
3. KB 域 5 个 Draft → 实现 + 红灯测试
4. A11Y 域 5 个 Draft → 实现 + 红灯测试
5. NAV/WS/EXP/ERR/OBS/WIN/I18N/HELP → 逐步实现 + 测试

---

## 六、质量门禁汇总

| 门禁 | 检查项 | 阈值 | 执行时机 |
| --- | --- | --- | --- |
| 格式 | `cargo fmt --check` | 0 差异 | 每个 PR |
| Lint | `cargo clippy -- -D warnings` | 0 警告 | 每个 PR |
| 供应链 | `cargo deny check` | 0 漏洞 | 每个 PR + 每日 |
| 单元测试 | `cargo test` | 0 failed | 每个 PR |
| 覆盖率 | `cargo tarpaulin` | core ≥ 95%, infra ≥ 85% | 每个 PR |
| 集成测试 | `cargo test --test integration` | 0 failed | 每个 PR |
| E2E 桥接 | Playwright | 0 failed | 每个 PR |
| 契约一致性 | schema 校验 | 0 drift | 每个 PR |
| 无障碍 | axe-core | 0 critical | 每个 PR |
| 可视化回归 | screenshot diff | < 2% 差异 | 每个 PR |
| 性能回归 | nightly benchmark | < 10% 退化 | 每日 |
| 变异测试 | cargo-mutants | 存活率 ≤ 5% | 每周 |
| 模糊测试 | cargo-fuzz | 0 crash | 每周 |
| 混沌测试 | fault injection | 0 Panic | 每日 |
| Lighthouse | 性能/无障碍/最佳实践 | ≥ 90 分 | 每日 |

---

## 七、修订记录

| 版本 | 日期 | 修订摘要 | 批准 |
| --- | --- | --- | --- |
| v1.0 | 2026-07-29 | 创世版：全面审计现有测试体系 + 十层金字塔设计 + 385 场景发散矩阵 + 实施路线图 | QA Lead |
