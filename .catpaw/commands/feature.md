---
description: TDD 功能开发流程（铁律二 + 铁律十五：Spec-driven + TDD）
---

按 Spec-driven Development 流程开发新功能（铁律十五 + 铁律二）。

参数: $ARGUMENTS（功能描述或 REQ-ID，如 "REQ-RAG-037 记忆自动提取管道"）

## 开发流程（强制顺序）

### 1. 需求分析
理解需求背景，明确「为什么做」和「做什么」。读取 `docs/SRS_v1.0.md` 中对应 REQ 条目。

### 2. 规格编写（红灯前置）
在 `docs/SRS_v1.0.md` 新增/更新 REQ 条目，包含：
- REQ-ID（格式 `REQ-<域>-<序号>`）
- 优先级（P0/P1/P2）
- 描述
- 验收标准（AC，每条可测试）
- 契约（trait 方法 / 数据模型 / IPC 命令 / 数据库表）
- 关联测试（TC）

### 3. 测试编写（TDD 红灯）
根据 AC 编写测试代码，此时测试必须失败（红灯）。
- Rust: `#[cfg(test)] mod *_tests`（与实现同文件）
- 前端: `e2e-tests/tests/unit/` 下的 Vitest 测试

### 4. 实现编码（TDD 绿灯）
编写生产代码使测试通过。遵循铁律五（零 unwrap/panic）。

### 5. 验证闭环
- `cargo test -p <crate> -- <测试名>` 确认测试通过
- `cargo clippy --all-targets -- -D warnings` 零警告
- `cargo fmt --check` 格式一致

### 6. 文档同步（铁律七）
- 更新 `docs/api/` 对应文档
- 更新 `docs/SOFTWARE_ASSET.md`
- 更新 `docs/sessions/CURRENT_STATE.md`
- 如有架构决策，新建 ADR

### 7. GitHub 同步（铁律十一）
`./sync-to-github.sh`
