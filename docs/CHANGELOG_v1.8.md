# EchoMind v1.8.0 变更日志

> **发布日期**：2026-08-11 | **版本**：v1.8.0（基于 v1.7.0）

---

## 新增功能

### B09 Skill 系统集成（REQ-ARCH-010 v1.8）

用户可在 `{data_dir}/skills/` 目录放置 Markdown 文件（YAML frontmatter `slash: true`），
自动出现在 `/` 斜杠命令面板。选中后 Skill 内容作为上下文注入对话。

**新增 IPC 命令**：
- `discover_skills` — 扫描 `{data_dir}/skills/*.md`，返回 `slash: true` 的 Skill 列表

**前端变更**：
- `slash-commands.js` — 新增 `loadSkills()` 函数，启动时加载 Skill 并合并到斜杠命令面板
- Skill 指令显示 `📦` 角标以区分系统指令（无角标）和自定义模板（`⚡` 角标）
- `main.js` — 启动时调用 `loadSkills()`（非阻塞，失败静默降级）

**降级策略**：
- 目录不存在 → 返回空列表（不报错）
- 文件解析失败 → 跳过该文件
- IO 错误 → 返回空列表 + 日志告警

**Skill 文件格式**：
```markdown
---
name: code-review
description: 代码审查技能
slash: true
---
你是一个专业的代码审查专家。请分析以下代码的问题：
```

### TDD 测试

| 测试 ID | 描述 | 结果 |
|---|---|---|
| TC-SKILL-INTEGRATE-001 | `discover_skills` 返回 `slash: true` 的 Skill 列表 | ✅ |
| TC-SKILL-INTEGRATE-002 | 目录不存在时返回空列表（静默降级） | ✅ |
| TC-SKILL-INTEGRATE-003 | `slash: false` 的 Skill 不出现在结果中 | ✅ |

---

## 评估文档

### F5-F8 远期项目评估（V1.8_ATOMIC_PLAN.md）

对 v1.7 第 5 节列出的 4 项远期项目进行详细评估，结论：全部推迟到 v2.0。

| 项 | ROI | 实现状态 | 阻碍原因 | 结论 |
|---|---|---|---|---|
| F5 Scored Selector | 4.2 | ✅ 12 测试 | 需多 provider 架构 | ⏸ v2.0 |
| F6 SystemContext+Epoch | 4.1 | ✅ 16 测试 | 需 chat flow 重构 | ⏸ v2.0 |
| F7 PromptCache | 4.0 | ✅ 27 测试 | 需 mistral.rs API 变更 | ⏸ v2.0 |
| F8 CachePolicy | 3.7 | ✅ 6 测试 | 需 Anthropic API 支持 | ⏸ v2.0 |

> 全部四项基础设施已就绪（TDD 测试通过），但集成均需架构级变更，不适合 v1.8 小版本。

---

## 验证结果

| 指标 | v1.7.0 | v1.8.0 | 变化 |
|---|---|---|---|
| Rust Free 测试 | 1242 passed | **1245 passed** | +3 新 TDD 测试 |
| Clippy | 零警告 | 零警告 | — |
| cargo fmt | 通过 | 通过 | — |
| TypeScript | 0 errors | 0 errors | — |
| esbuild 行数 | 16614 | **16643** | +29 行 |
| IPC 命令数 | 197 Free | **198 Free** | +1 (`discover_skills`) |
| 版本号 | 1.7.0 | **1.8.0** | — |

---

## 变更文件清单

### 新增文件
- `docs/architecture/V1.8_ATOMIC_PLAN.md` — v1.8 原子规划（含 F5-F8 评估）
- `docs/CHANGELOG_v1.8.md` — 本文件

### 修改文件
- `crates/tauri-app/src/commands/features.rs` — 新增 `discover_skills` IPC 命令 + 3 TDD 测试
- `crates/tauri-app/src/commands/mod.rs` — 新增 `Skill` 类型导入
- `crates/tauri-app/src/lib.rs` — 注册 `discover_skills` 到 `generate_handler!`
- `crates/tauri-app/tauri.conf.json` — 版本号 1.7.0 → 1.8.0
- `ui/src/slash-commands.js` — 新增 `loadSkills()` + Skill 合并到命令面板 + `📦` 角标
- `ui/src/main.js` — 启动时调用 `loadSkills()`
- `ui/locales/zh-CN.json` — 新增 `slash_skill` / `slash_skill_desc` i18n 键
- `ui/locales/en.json` — 新增 `slash_skill` / `slash_skill_desc` i18n 键
- `e2e-tests/tests/bridge/tauri-stub.js` — 新增 `discover_skills` mock + `skills` state
- `docs/sessions/CURRENT_STATE.md` — 更新项目状态快照
