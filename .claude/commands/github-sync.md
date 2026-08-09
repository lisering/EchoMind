---
description: GitHub 发布同步（铁律十一：双仓库工作流）
---

将源代码变更同步到 GitHub 发布仓库（铁律十一强制）。

执行命令：`./sync-to-github.sh`

同步规则：
- **同步的文件**：源代码（`.rs` / `.js` / `.html` / `.css` / `.json` / `.toml`）、`Cargo.lock`、前端资源
- **不同步的文件**：`docs/`、`e2e-tests/`、`.catpaw/`、`PROJECT_RULES.md`、`.githooks/`、`scripts/`（开发专属资产）

参数: $ARGUMENTS（可选）
- 无参数：同步但不推送
- "push"：同步并推送到远程 `cd EchoMind/EchoMind && git push origin main`
- "status"：查看发布仓库当前状态 `cd EchoMind/EchoMind && git status --short && git log --oneline -5`

完成后报告：
- 同步的文件数量
- 新增 / 修改 / 删除行数
- 发布仓库当前分支和最新 commit
