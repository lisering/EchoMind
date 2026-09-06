# EchoMind AI Code Review 集成方案

> **版本**: v1.0.0  
> **日期**: 2026-09-06  
> **状态**: 已实施

---

## 概览

本方案为 EchoMind 项目引入**多层次 AI 代码审查体系**，结合确定性静态分析与大模型语义理解，实现：

| 层级 | 工具 | 特点 | 作用 |
|------|------|------|------|
| **L1 - 快速门禁** | rust-doctor | 秒级、确定性、可量化评分 | CI 质量门禁，阻断低质量代码 |
| **L2 - 深度语义** | Claude Code Action | 理解业务逻辑、架构合规 | 发现静态分析遗漏的问题 |
| **L3 - 补充视角** | rs-guard (可选) | 多 LLM 提供商、轻量级 | 额外审查视角，按需启用 |

---

## 架构设计

```
┌─────────────────────────────────────────────────────────────────┐
│                        PR Opened / Sync                         │
└──────────────────────────┬──────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────────┐
│                      ci.yml: lint job                           │
│              (fmt + clippy --deny-warnings)                     │
└──────────────────────────┬──────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────────┐
│                 ci.yml: ai-review job (L1)                      │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ rust-doctor --diff main --fail-below 85 --fail-on error │   │
│  │ • 700+ clippy lints (zero unwrap/panic enforced)         │   │
│  │ • 19 custom AST rules (security/async/arch/performance)  │   │
│  │ • cargo-audit (CVE) + cargo-deny (supply chain)          │   │
│  │ • cargo-geiger (unsafe) + cargo-machete (unused deps)    │   │
│  │ • Unified 0-100 health score                             │   │
│  │ • SARIF → GitHub Code Scanning                           │   │
│  └─────────────────────────────────────────────────────────┘   │
└──────────────────────────┬──────────────────────────────────────┘
                           │ (on failure: block PR)
                           │ (on success: trigger L2)
                           ▼
┌─────────────────────────────────────────────────────────────────┐
│              ai-review.yml: claude-code-review (L2)             │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ anthropics/claude-code-action@v1                        │   │
│  │ • Deep semantic analysis                                │   │
│  │ • Architecture compliance (hexagonal, DIP)              │   │
│  │ • Security: SQLi, XSS, path traversal, secrets          │   │
│  │ • Iron Rules (铁律) validation                          │   │
│  │ • Output: JSON array → inline PR comments               │   │
│  └─────────────────────────────────────────────────────────┘   │
└──────────────────────────┬──────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────────┐
│              ai-review.yml: summary job                         │
│  • Aggregate rust-doctor score + Claude findings                │
│  • Post summary comment on PR                                   │
└─────────────────────────────────────────────────────────────────┘
```

---

## 配置文件

### 1. rust-doctor.toml
**位置**: 项目根目录  
**用途**: 配置 rust-doctor 扫描规则、评分权重、输出格式

关键配置：
```toml
[general]
mode = "diff"           # PR 模式仅扫描变更
fail-below = 85         # 质量门禁阈值
fail-on = "error"       # 失败级别

[clippy]
pedantic = true         # 启用 pedantic 级别
severity-overrides = [
    { lint = "unwrap_used", severity = "error" },
    { lint = "await_holding_lock", severity = "error" },
    # ... 更多覆盖
]

[custom-rules]
# 19 个自定义规则：错误处理、性能、安全、异步、架构、框架
dependency-direction = { enabled = true, severity = "error" }
no-std-mutex-across-await = { enabled = true, severity = "error" }
tauri-command-error-handling = { enabled = true, severity = "error" }

[scoring]
security = 30
reliability = 25
maintainability = 20
performance = 15
dependencies = 10
```

### 2. deny.toml
**位置**: 项目根目录  
**用途**: 供应链安全（RustSec 漏洞、许可证、禁用 crate、来源限制）

### 3. .github/workflows/ci.yml
**集成点**: `ai-review` job（需 `lint` 通过后运行）

### 4. .github/workflows/ai-review.yml
**独立工作流**: 支持 `pull_request`、`issue_comment` (`@claude`)、`workflow_dispatch`

---

## 触发机制

| 触发方式 | 场景 | 运行内容 |
|---------|------|----------|
| PR opened/synced | 常规开发 | ci.yml → lint → ai-review (L1) → 触发 ai-review.yml (L2) |
| PR comment `@claude` | 请求深度审查 | ai-review.yml → claude-code-review |
| 手动派发 | 指定 PR 复查 | workflow_dispatch with pr_number |
| Label `ai-review-deep` | 额外视角 | ai-review.yml → rs-guard (L3) |

---

## 质量门禁标准

### rust-doctor (L1) - 必须通过
| 维度 | 权重 | 关键检查项 |
|------|------|------------|
| **Security** | 30% | CVE、硬编码密钥、SQL 注入、路径遍历、unsafe 审计 |
| **Reliability** | 25% | unwrap/expect/panic、Result 处理、空 catch、整数溢出 |
| **Maintainability** | 20% | 架构依赖方向、命名、文档、循环复杂度 |
| **Performance** | 15% | 循环分配、不必要 clone、大类型按值传递、async 阻塞 |
| **Dependencies** | 10% | 许可证合规、未使用依赖、重复版本、来源白名单 |

**通过标准**: 总分 ≥ 85，且无 `error` 级别发现

### Claude Code Review (L2) - 必须审查
- 输出为 JSON 结构化问题列表
- 自动发布为 PR 行内评论
- 严重级：CRITICAL/HIGH/MEDIUM/LOW
- 重点：铁律合规、业务逻辑、架构设计

---

## 本地开发验证

### 安装工具
```bash
# rust-doctor (推荐：预编译二进制)
curl -L -o rust-doctor https://github.com/ArthurDEV44/rust-doctor/releases/latest/download/rust-doctor-x86_64-unknown-linux-gnu
chmod +x rust-doctor
sudo mv rust-doctor /usr/local/bin/

# 或通过 cargo
cargo install rust-doctor

# 安装外部依赖
rust-doctor --install-deps
```

### 本地运行
```bash
# 完整扫描（开发时）
rust-doctor --verbose

# 仅评分（快速检查）
rust-doctor --score

# 仅变更文件（模拟 PR）
rust-doctor --diff main --fail-below 85 --fail-on error

# 输出 SARIF（用于 IDE 集成）
rust-doctor --sarif target/rust-doctor.sarif

# JSON 输出（脚本解析）
rust-doctor --json target/rust-doctor.json
```

### Pre-commit 集成 (可选)
```bash
# .pre-commit-config.yaml
- repo: local
  hooks:
    - id: rust-doctor
      name: rust-doctor (fast)
      entry: rust-doctor --diff HEAD --score --fail-below 85
      language: system
      types: [rust]
      stages: [commit]
```

---

## GitHub Secrets 配置

在仓库 Settings → Secrets and variables → Actions 中添加：

| Secret | 必需 | 用途 | 来源 |
|--------|------|------|------|
| `ANTHROPIC_API_KEY` | ✅ | Claude Code Action | https://console.anthropic.com/ |
| `DEEPSEEK_API_KEY` | ⭕ | rs-guard (DeepSeek) | https://platform.deepseek.com/ |
| `OPENAI_API_KEY` | ⭕ | rs-guard (OpenAI) | https://platform.openai.com/ |

---

## 故障排查

### rust-doctor 评分过低
1. 查看 `target/rust-doctor.json` 详细诊断
2. 检查 GitHub Code Scanning 标签页的 SARIF 结果
3. 常见问题：
   - `unwrap_used` → 改用 `?` 或 `ok_or()` 
   - `dependency-direction` → 检查 crate 依赖方向
   - `no-std-mutex-across-await` → 改用 `tokio::sync::Mutex`

### Claude 审查未触发
1. 确认 `ANTHROPIC_API_KEY` 已配置
2. 检查 workflow 权限：`pull-requests: write`, `issues: write`
3. 查看 Actions 日志中的 claude-code-action 步骤

### SARIF 未上传
1. 确认 `security-events: write` 权限
2. 检查 `target/rust-doctor.sarif` 是否生成
3. 验证 `github/codeql-action/upload-sarif@v3` 步骤

---

## 扩展建议

### 1. 增加 CodeQL (安全增强)
```yaml
# .github/workflows/codeql.yml
- uses: github/codeql-action/init@v3
  with:
    languages: rust
- uses: github/codeql-action/analyze@v3
```

### 2. 集成 SonarQube (企业级)
- 使用 `SonarSource/sonarqube-scan-action`
- 配置 `sonar-project.properties` with Rust plugin

### 3. 自定义规则开发
- rust-doctor 支持通过 syn 编写自定义 AST 规则
- 参考 `crates/rust-doctor/src/rules/` 实现项目特有规则

### 4. 评分趋势追踪
- 将 rust-doctor `--score` 输出记录到时序数据库
- 在 Grafana/Datadog 看板展示质量趋势

---

## 维护清单

- [ ] 每月更新 rust-doctor 版本（检查新规则、性能改进）
- [ ] 每季度审查评分权重（根据项目演进调整）
- [ ] 每半年评估 AI 审查效果（误报率、漏报率、开发者满意度）
- [ ] 同步更新 `.catpaw/rules/system-constraints.md` 中的铁律引用

---

## 相关文档

- [rust-doctor 官方文档](https://github.com/ArthurDEV44/rust-doctor)
- [Claude Code Action](https://github.com/anthropics/claude-code-action)
- [rs-guard](https://github.com/nebulaideas/rs-guard)
- [EchoMind 铁律规则](.catpaw/rules/system-constraints.md)
- [项目宪法](PROJECT_RULES.md)