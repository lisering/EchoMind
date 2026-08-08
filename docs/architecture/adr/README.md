# 架构决策记录（ADR）

> **状态**：活跃 | **最后审阅**：2026-08-01（补充 ADR-009/010/011 索引）

ADR（Architecture Decision Records）记录 EchoMind 的重大架构决策——为什么这么选、考虑了什么替代方案、有什么后果。

---

## ADR 索引

| 编号 | 标题 | 状态 | 日期 |
|---|---|---|---|
| [ADR-001](001-hexagonal-architecture.md) | 采用四层六边形架构（Ports & Adapters） | ✅ 已采纳 | 2026-07-24 |
| [ADR-002](002-native-async-trait.md) | 原生 async fn in trait 而非 `#[async_trait]` 宏 | ✅ 已采纳 | 2026-07-24 |
| [ADR-003](003-no-cdn-frontend.md) | 前端禁止 CDN，本地打包三件套 | ✅ 已采纳 | 2026-07-24 |
| [ADR-009](ADR-009-rich-content-rendering.md) | 富内容可视化渲染引擎（Mermaid/KaTeX/Chart.js） | ✅ 已采纳 | 2026-07-25 |
| [ADR-010](ADR-010-multimodal-content-understanding.md) | 多模态内容理解引擎（PDF 图片/表格/图表识别） | ✅ 已采纳 | 2026-07-25 |
| [ADR-011](ADR-011-local-llm-engine.md) | 本地 LLM 推理引擎技术选型（mistral.rs v0.9.0） | ✅ 已采纳 | 2026-07-30 |

---

## ADR 模板

新建 ADR 时复制以下结构：

```markdown
# ADR-XXX: [标题]

> **状态**：提议 | 已采纳 | 已废弃 | 已替代
> **日期**：YYYY-MM-DD
> **决策者**：[姓名/角色]

## 背景

[为什么需要做这个决策？有什么约束？]

## 决策

[选择了什么？]

## 考虑的替代方案

| 方案 | 优点 | 缺点 | 为什么没选 |
|---|---|---|---|
| 方案A | ... | ... | ... |

## 后果

- 正面：[带来了什么好处]
- 负面：[有什么代价]
- 中性：[需要注意的事项]
```

## 规则

1. ADR 编号连续递增，不回收
2. 已采纳的 ADR 不修改，如需变更新建 ADR 并标记前序 ADR 为"已替代"
3. 每个重大架构选择（新依赖、设计模式变更、安全方案选择）都应记录
