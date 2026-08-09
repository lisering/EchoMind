---
description: Session 开始 — 读取上下文，了解 EchoMind 项目当前状态
---

新 session 开始时执行以下步骤（铁律十二强制）：

1. 读取 `docs/sessions/CURRENT_STATE.md` — 获取最新项目快照（测试数、已完成功能、待办任务、关键约束）
2. 读取 `.catpaw/rules/system-constraints.md` — 确认当前铁律约束（21 条）
3. 浏览 `docs/sessions/LOG.md` — 快速了解前序 session 时间线
4. 运行 `cargo test 2>&1 | tail -5` 确认测试基线（Free 版）
5. 运行 `git status --short` 查看当前工作区状态

读取完成后，用中文简要总结：
- 当前项目版本号和测试数量（Free / Pro / E2E）
- 上次 session 完成了什么
- 当前待办任务（P0/P1/P2）
- 当前 Git 状态
- 关键约束提醒（铁律五零 unwrap、铁律十一 GitHub 同步、铁律十二 Session 管理）

如果 `CURRENT_STATE.md` 不存在或过时，分析项目当前状态并更新。
