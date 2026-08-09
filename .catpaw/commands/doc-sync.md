---
description: 文档同步检查（铁律七：文档实时同步）
---

检查代码变更后文档是否同步更新（铁律七强制）。

执行以下检查：

1. **IPC 命令变更** → 检查 `docs/api/ipc-commands.md` 是否已更新
2. **数据库表变更** → 检查 `docs/api/database-schema.md` 是否已更新
3. **Trait 方法变更** → 检查 `docs/api/code-contracts.md` 是否已更新
4. **数据模型变更** → 检查 `docs/api/code-contracts.md` 是否已更新
5. **前端文件变更** → 检查 `docs/api/frontend-modules.md` 是否已更新
6. **配置键变更** → 检查 `docs/api/settings-reference.md` 是否已更新
7. **REQ 状态变更** → 检查 `docs/SRS_v1.0.md` 对应 REQ 状态是否已更新
8. **架构决策** → 检查是否需要新建 ADR（`docs/architecture/adr/`）

运行 `git diff --name-only HEAD~1` 获取变更文件列表，根据变更类型判断需要同步的文档。

输出检查结果：
- ✓ 已同步的文档
- ✗ 未同步的文档（列出需要更新的文件和建议内容）

参数: $ARGUMENTS（可选，如果为 "fix" 则自动更新缺失的文档）
