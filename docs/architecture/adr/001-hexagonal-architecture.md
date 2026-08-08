# ADR-001: 采用四层六边形架构（Ports & Adapters）

> **状态**：✅ 已采纳
> **日期**：2026-07-24
> **决策者**：Chief Architect / TPM

## 背景

EchoMind 需要一个可测试、可维护的架构，支持：
- 业务逻辑独立于外部依赖（SQLite、ONNX、HTTP）
- 端口 Trait 可被 fake/mock 替换用于测试
- 依赖方向严格单向，防止循环依赖
- 未来可替换适配器（如 SQLite → PostgreSQL、fastembed → 其他模型）

## 决策

采用四层六边形架构：

```
crates/models  →  crates/core  →  crates/infra  →  crates/tauri-app
  (contracts)     (ports+logic)   (adapters)       (assembly)
```

- `crates/models`：纯数据结构，无业务逻辑
- `crates/core`：端口 Trait 定义 + 业务逻辑实现，**严禁依赖任何具体实现**
- `crates/infra`：适配器，实现 core 定义的端口（SQLite、fastembed、OpenAI）
- `crates/tauri-app`：组装层，依赖注入、IPC 命令、错误码映射

## 考虑的替代方案

| 方案 | 优点 | 缺点 | 为什么没选 |
|---|---|---|---|
| 三层（models→core→tauri-app） | 结构更简单 | 适配器混入 core，违反端口隔离 | core 会泄漏具体实现 |
| MVC 分层 | 业界熟悉 | 不适合桌面应用 + 异步端口 | 无法隔离外部依赖 |
| Clean Architecture | 类似六边形 | 过度抽象，EchoMind 规模不需要 | 增加不必要的复杂度 |

## 后果

- 正面：业务逻辑可独立测试（不需要 SQLite/ONNX/HTTP）
- 正面：适配器可替换（如 SQLite → PostgreSQL 只需新增 infra 适配器）
- 正面：依赖方向清晰，CI 可用 `cargo deny` 检查依赖方向
- 负面：四层 crate 增加项目结构复杂度
- 中性：新增功能时需要跨多层 crate 编辑
