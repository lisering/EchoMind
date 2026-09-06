# EchoMind Session 上下文管理中心

> **本目录是 EchoMind 项目跨会话上下文管理的唯一权威位置。**
>
> 任何 AI 会话（无论哪个工具、哪个模型）在开始 EchoMind 工作前，**必须先读取 `CURRENT_STATE.md`** 获取最新上下文。
>
> **创建日期**: 2026-08-04 | **最后更新**: 2026-08-04

---

## 目录结构

```
docs/sessions/
├── README.md              ← 本文件（索引 + 使用指南）
├── CURRENT_STATE.md       ← 当前项目状态快照（≤100 行，每次 session 结束时必须更新）
├── HISTORY.md             ← 历史详情归档（功能矩阵、版本规划、审计报告，从 CURRENT_STATE 拆分）
├── HANDOFF_TEMPLATE.md    ← 会话衔接提示词模板（复制使用）
├── LOG.md                 ← 会话时间线日志（按日期记录所有 session 摘要）
└── archive/               ← 历史会话详情归档
    └── YYYY-MM-DD-<功能名>.md
```

---

## 使用方法

### 场景 1: 新会话开始

1. **读取 `CURRENT_STATE.md`** — 获取项目当前完整状态（测试数、已完成功能、待办任务、关键文件等）
2. **读取 `LOG.md`** — 快速浏览历史会话时间线，了解前序工作
3. **如需特定功能上下文** — 在 `archive/` 中查找对应历史记录
4. **如需衔接提示词** — 复制 `HANDOFF_TEMPLATE.md` 模板填入实际内容

### 场景 2: 会话进行中

- 遇到需要追溯的设计决策，查 `archive/` 或 `docs/architecture/adr/`
- 遇到需要确认接口契约，查 `docs/api/` 或 `docs/SOFTWARE_ASSET.md`
- 遇到需要确认铁律规则，查 `.catpaw/rules/system-constraints.md`

### 场景 3: 会话结束

1. **更新 `CURRENT_STATE.md`** — 刷新测试数、已完成/待办列表、关键变更
2. **追加 `LOG.md`** — 在时间线末尾添加本次 session 摘要
3. **创建 `archive/` 归档** — 将本次 session 的详细记录归档（可选，仅大型功能开发需要）
4. **生成衔接提示词** — 按 `HANDOFF_TEMPLATE.md` 模板为下一 session 生成衔接提示词
5. **更新 `docs/SOFTWARE_ASSET.md`** — 如有源代码变更，同步更新软件资产文档

---

## 文件职责

| 文件 | 职责 | 更新频率 |
|---|---|---|
| `README.md` | 索引与使用指南 | 结构变更时 |
| `CURRENT_STATE.md` | 项目状态快照（≤100 行核心指标） | **每次 session 结束时必须更新** |
| `HISTORY.md` | 历史详情（功能矩阵/版本规划/审计报告） | 从 CURRENT_STATE 拆分，大版本时追加 |
| `HANDOFF_TEMPLATE.md` | 衔接提示词模板 | 模板格式变更时 |
| `LOG.md` | 会话时间线 | **每次 session 结束时追加** |
| `archive/*.md` | 历史详情 | 大型功能 session 结束时创建 |

---

## 与其他文档的关系

| 文档 | 与本目录的关系 |
|---|---|
| `docs/SOFTWARE_ASSET.md` | 软件资产全景（架构、代码、数据流），本目录引用其内容 |
| `docs/SRS_v1.0.md` | 需求规格，本目录的待办任务引用其 REQ ID |
| `docs/api/` | 接口契约文档，本目录引用其内容 |
| `docs/architecture/adr/` | 架构决策记录，本目录引用其 ADR 编号 |
| `.catpaw/rules/system-constraints.md` | 铁律规则，所有 session 必须遵守 |
| `AGENTS.md` / `CLAUDE.md` | AI Agent 指南，本目录是其补充 |

---

## 会话索引（按时间倒序）

> 完整时间线见 `LOG.md`，详细记录见 `archive/`。

| 日期 | Session | 功能 | 状态 |
|---|---|---|---|
| 2026-08-04 | 软件资产 + Session 管理 | 全量源码阅读 + SOFTWARE_ASSET.md + Session 体系 | ✅ |
| 2026-08-04 | 极致性能 S10 | 文档同步 + 基准测试 | ✅ |
| 2026-08-04 | 极致性能 S9 | 检索记忆自进化 | ✅ |
| 2026-08-04 | 极致性能 S8 | Speculative RAG | ✅ |
| 2026-08-04 | 极致性能 S7 | 渐进式上下文注入 | ✅ |
| 2026-08-04 | 极致性能 S4-S6 | Proposition/ColBERT/RAPTOR | ✅ |
| 2026-08-04 | 极致性能 S1-S3 | 语义缓存/Prompt 压缩/CRAG | ✅ |
| 2026-08-04 | 聊天韧性修复 | embedder 超时 + 看门狗 | ✅ |
| 2026-08-04 | v1.0.1 P1 收尾 S1-S4 | 无障碍 + 焦点 + 对比度 + 高DPI | ✅ |
| 2026-08-04 | DeepSeek UI S1-S5 | 视觉对齐 + 思维链 + Toggle + 设计系统 + E2E | ✅ |
| 2026-08-03 | Phase 4 S25 | KV Cache 跨会话复用 | ✅ |
| 2026-08-01 | Phase 3 S16-S23 | 自研量化内核（8 session） | ✅ |
| 2026-07-31 | Phase 2 S8-S15 | 本地 LLM GPU/PagedAttention/采样（8 session） | ✅ |
| 2026-07-29 | Phase 1 S1-S7 | 本地 LLM 数据模型/管理器/引擎/IPC/前端/文档（7 session） | ✅ |
| 2026-07-28 | Free/Pro 条件编译 | `#[cfg(feature = "pro")]` 拆分 | ✅ |
| 2026-07-29 | Logo + i18n + 菜单 | 品牌 + 国际化基础设施 | ✅ |

---

## 铁律十二（跨 Session 衔接）

> 完整定义见 `.catpaw/rules/system-constraints.md` 铁律十二。

**未完成的功能必须在 Session 结束时按 `HANDOFF_TEMPLATE.md` 模板产出衔接提示词。**

衔接提示词必须包含：
1. 上下文回溯（已完成 Session 列表）
2. 当前 Session 编号
3. 本 Session 目标
4. 必读文档（按优先级）
5. 精确变更清单
6. TDD 测试要求
7. 关键约束
8. 验证命令
9. 完成后必须执行的操作
