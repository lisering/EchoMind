# 供应链安全管理

> **状态**：活跃 | **最后审阅**：2026-07-25
>
> 本文定义 EchoMind 项目的供应链安全策略，包括依赖更新、漏洞扫描、许可证合规和来源验证。

---

## 1. 概述

供应链安全是软件安全的基础——如果你的依赖被投毒或包含漏洞，你的应用也会被影响。EchoMind 采用多层防御策略：

```
依赖更新（Dependabot）
    ↓ 自动创建 PR
漏洞扫描（cargo-audit + npm audit）
    ↓ CI 门禁
许可证合规（cargo-deny）
    ↓ CI 门禁
来源验证（cargo-deny sources）
    ↓ CI 门禁
```

---

## 2. 工具链

| 工具 | 范围 | 作用 | 配置文件 |
|---|---|---|---|
| **Dependabot** | Rust + npm + GitHub Actions | 自动创建依赖更新 PR | `.github/dependabot.yml` |
| **cargo-audit** | Rust | 扫描 RustSec 安全公告数据库 | — |
| **cargo-deny** | Rust | 许可证合规 + crate 来源 + 重复版本 + 禁用检查 | `deny.toml` |
| **npm audit** | npm | npm 依赖安全扫描 | — |

---

## 3. Dependabot 配置

**配置文件**：`.github/dependabot.yml`

### 3.1 监控范围

| 生态系统 | 目录 | 频率 | 分组策略 |
|---|---|---|---|
| Rust (cargo) | `/` (workspace root) | 每周一 | 所有 Rust 依赖合并为 1 个 PR |
| npm (E2E) | `/e2e-tests` | 每周一 | 独立 PR |
| GitHub Actions | `/` | 每月 | 独立 PR |

### 3.2 审查流程

1. Dependabot 自动创建 PR
2. CI 自动运行（包括供应链安全扫描 + 测试）
3. CI 通过后，人工审查变更内容
4. 合并（Squash merge）

---

## 4. cargo-deny 配置

**配置文件**：`deny.toml`

### 4.1 许可证白名单

只允许以下许可证（与 MIT 兼容）：

| 许可证 | 常见 crate |
|---|---|
| MIT | tokio, serde, anyhow, ... |
| Apache-2.0 | rusqlite, ... |
| BSD-2-Clause / BSD-3-Clause | 部分传递依赖 |
| ISC | 部分加密库 |
| MPL-2.0 | 部分 Mozilla 库 |
| Zlib | 部分 C 绑定 |
| Unicode-DFS-2016 / Unicode-3.0 | unicode 相关 |

**禁止的许可证**：GPL-2.0, GPL-3.0, AGPL-3.0, LGPL（copyleft 许可证与 MIT 项目不兼容）

### 4.2 来源限制

- 只允许从 `crates.io` 获取依赖
- 禁止从任意 git 仓库获取（除非在 `allow-git` 中明确列出）
- 防止供应链投毒

### 4.3 漏洞处理策略

| 情况 | 处理方式 |
|---|---|
| 已知漏洞 (RustSec) | CI 立即失败，必须修复或明确忽略 |
| 暂时无法修复 | 在 `deny.toml` 的 `ignore` 中列出 RUSTSEC ID + 理由 |
| 已弃用 crate | `unmaintained = "workspace"` 标记 |
| Yanked crate | `yanked = "deny"` 立即失败 |

---

## 5. CI 集成

### 5.1 日常 CI（每次 push/PR）

```
supply-chain job（与测试并行）
  ├── cargo audit          → RustSec 漏洞扫描
  ├── cargo deny check     → 许可证 + 来源 + 重复版本
  └── npm audit (e2e)      → E2E 依赖漏洞
```

### 5.2 发布 CI（tag 推送时）

```
supply-chain job（必须通过）
  ├── cargo audit
  └── cargo deny check
       ↓ 通过后
build-and-release job 才会执行
```

---

## 6. 本地运行

### 6.1 安装工具

```bash
# 安装 cargo-audit
cargo install cargo-audit --locked

# 安装 cargo-deny
cargo install cargo-deny --locked
```

### 6.2 运行检查

```bash
# 漏洞扫描
cargo audit

# 许可证 + 来源 + 重复版本检查
cargo deny check

# 单独检查某个维度
cargo deny check advisories    # 仅漏洞
cargo deny check licenses      # 仅许可证
cargo deny check bans          # 仅禁用/重复
cargo deny check sources       # 仅来源

# 更新 advisory 数据库
cargo audit update
```

### 6.3 npm 检查

```bash
# E2E 依赖
cd e2e-tests && npm audit
```

---

## 7. 漏洞响应流程

### 7.1 评估与修复

| 严重程度 | 响应时间 | 处理方式 |
|---|---|---|
| Critical (CVSS ≥ 9.0) | 24 小时内 | 立即升级依赖，发布 hotfix |
| High (7.0-8.9) | 3 天内 | 升级依赖，下一个 patch release |
| Medium (4.0-6.9) | 1 周内 | 升级依赖，下一个 minor release |
| Low (< 4.0) | 下一个版本 | 升级依赖，常规节奏 |

### 7.2 无法立即修复时

在 `deny.toml` 中添加忽略条目：

```toml
[advisories]
ignore = [
    { id = "RUSTSEC-2024-XXXX", reason = "上游尚未发布修复版本，当前漏洞不影响本项目因为..." },
]
```

**要求**：
- 必须填写理由（说明为什么不影响本项目）
- 必须定期复查（至少每月检查一次）
- 忽略条目数量不应超过 5 个

---

## 8. 检查清单（发布前）

- [ ] `cargo audit` 无漏洞
- [ ] `cargo deny check` 无错误
- [ ] `npm audit` 无 moderate 以上漏洞
- [ ] Dependabot PR 已处理（合并或关闭）
- [ ] `deny.toml` 中无过期忽略条目

---

## 9. 相关文件

| 文件 | 用途 |
|---|---|
| `.github/dependabot.yml` | Dependabot 自动依赖更新配置 |
| `deny.toml` | cargo-deny 许可证 + 安全 + 来源配置 |
| `.github/workflows/e2e.yml` | CI 流水线 |
