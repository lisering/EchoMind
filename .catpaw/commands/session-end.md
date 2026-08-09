---
description: Session 结束 — 更新状态 + 同步 GitHub（铁律十一/十二强制）
---

⚠️ Session 结束必做流程（铁律十一 + 铁律十二），依次执行以下步骤：

## 步骤 1: 更新 docs/sessions/CURRENT_STATE.md

更新项目状态快照，刷新：
- 测试数量（Free / Pro / E2E / Vitest）
- 已完成功能矩阵
- 待办任务列表
- 版本号
- IPC 命令数
- 当前 Git 状态

## 步骤 2: 追加 docs/sessions/LOG.md

在时间线末尾添加本次 session 摘要：
- Session 日期
- 完成的功能 / 修复的 Bug
- 新增测试数量
- 变更的文件列表

## 步骤 3: 生成衔接提示词

如有未完成功能，按 `docs/sessions/HANDOFF_TEMPLATE.md` 模板生成衔接提示词。
**衔接提示词必须放在一个完整的 Markdown 代码块中输出，用户只需点击一次「复制」按钮即可获取完整内容。**

## 步骤 4: 同步到 GitHub

执行命令：`./sync-to-github.sh`

同步脚本将源代码同步到发布仓库 `EchoMind/EchoMind/`。

## 完成后报告

- 更新的 `CURRENT_STATE.md` 行数
- 追加的 `LOG.md` 行数
- 同步到发布仓库的文件数
- 如需推送到远程：`cd EchoMind/EchoMind && git push origin main`

参数: $ARGUMENTS（可选，如果为 "push" 则同步后自动推送到远程）
