# 贡献指南

> 感谢你对 EchoMind 的关注！本文档帮助你快速参与项目贡献。

---

## 🚀 快速上手

1. **Fork & Clone**
   ```bash
   git clone https://github.com/<your-username>/EchoMind.git
   cd EchoMind
   ```

2. **安装依赖**
   ```bash
   # Rust ≥1.85 (edition 2024)
   rustc --version

   # Node.js ≥18 (E2E 测试)
   node --version
   ```

3. **安装 Git Hooks**
   ```bash
   git config core.hooksPath .githooks
   ```

4. **首次构建**
   ```bash
   cargo build  # 首次约 5-10 分钟（ML 依赖 opt-level=3）
   ```

5. **运行开发模式**
   ```bash
   cargo tauri dev
   ```

6. **运行测试**
   ```bash
   cargo test                           # Rust 测试
   cargo clippy --all-targets -- -D warnings  # Lint（含铁律五 deny lint）
   cargo fmt --check                    # 格式检查
   cd e2e-tests && ECHOMIND_E2E_TEST=1 npx playwright test  # E2E 测试
   bash scripts/doc-check.sh            # 文档质量检查（版本一致/死链/过时/格式）
   ```

更多细节见 [开发者入门指南](docs/dev-guide/getting-started.md)。

---

## 🌿 分支策略

```
main (Stable)
  ├── tag v1.0.0 ──→ GitHub Release
  ├── feature/* (功能分支，从 main 拉)
  ├── fix/* (修复分支，从 main 拉)
  └── chore/* (基建分支，从 main 拉)
```

- 功能开发：从 `main` 拉 `feature/req-<域>-<序号>-<短描述>` 分支
- Bug 修复：从 `main` 拉 `fix/<问题描述>` 分支
- 基建变更：从 `main` 拉 `chore/<简述>` 分支

---

## 📝 提交规范

### 提交信息格式

```
<type>(<scope>): <description>

类型：feat | fix | docs | refactor | test | chore | perf | ci | build | revert
范围：关联 REQ 时使用小写需求编号
```

**示例：**
```
feat(req-ing-001): 支持拖拽导入 md/txt
fix(req-rag-005): 停止生成后输入框未复位
docs(srs): 冻结 REQ-ING-001 导入契约
test(req-lic-001): 免费版 50 文件配额红灯用例
chore: 升级 tauri 至 2.x.y
```

> Git Hooks 会自动校验提交信息格式。跳过校验（`--no-verify`）需在 PR 中说明原因。

### PR 流程

1. 从 `main` 拉取功能分支
2. 提交 PR 到 `main`
3. CI 必须通过：
   - `cargo fmt --check`
   - `cargo clippy --all-targets -- -D warnings`
   - `cargo check`
   - `cargo test`
   - `cd e2e-tests && ECHOMIND_E2E_TEST=1 npx playwright test`
4. 至少 1 个 review approval
5. Squash merge

---

## 🧪 测试要求

### Rust

- 单元测试与实现同文件（`#[cfg(test)] mod *_tests`）
- `*_inner` 函数模式：IPC 命令逻辑提取为 `*_inner(&AppState)` 供集成测试复用
- 命名：`test_<行为>_<条件>_<预期结果>`
- **铁律五**：生产代码严禁 `unwrap()` / `expect()` / `panic!()`（测试代码豁免）

### E2E

- `ECHOMIND_E2E_TEST=1` 启用 Mock 模式（零网络、零 API 费用）
- `ECHOMIND_DATA_DIR` 重定向数据目录（隔离测试数据）
- 测试在 headless Chromium 中运行

### 混沌测试（铁律四）

- `tests/fixtures/generate_corpus.py` 自动生成混沌测试语料
- 覆盖：超长文档、复杂 PDF、混合编码、恶意 XSS、空文件、深套目录
- L2 及以上测试必须包含真实公开语料和极端边缘数据

详见 [测试指南](docs/dev-guide/testing.md)。

---

## 📐 编码规范

- Rust edition 2024，clippy 必须通过（含 deny lint）
- 公开类型必须有 `///` 文档注释（铁律三）
- 前端：DOMPurify 净化所有用户输入，禁止 CDN
- 详见 [编码规范](docs/dev-guide/conventions.md)

---

## 📖 文档同步（铁律七）

代码变更必须同步更新对应文档：

| 变更类型 | 必须更新 | 时限 |
|---|---|---|
| 新增/修改 IPC 命令 | `docs/api/ipc-commands.md` | 同一 PR |
| 新增/修改数据库表 | `docs/api/database-schema.md` | 同一 PR |
| 新增/修改 Trait 或数据模型 | `docs/api/code-contracts.md` | 同一 PR |
| 功能状态变化 | `docs/SRS_v1.0.md` 对应 REQ 状态 | 同一 PR |

### 文档质量自动化检查

本地运行：

```bash
bash scripts/doc-check.sh
```

CI 会在以下条件自动触发检查：
- Push 到 `main` 且修改了 `*.md` 或 `tauri.conf.json`
- PR 目标为 `main` 且涉及文档或配置文件变更
- 手动触发 (`workflow_dispatch`)

检查项：

| 检查项 | 说明 |
|---|---|
| 版本号一致性 | `tauri.conf.json` 版本与 AGENTS.md、CHANGELOG.md、SOFTWARE_ASSET.md、CURRENT_STATE.md 一致 |
| 内部死链 | Markdown 文件中 `](path)` 引用的本地文件必须存在 |
| 文档过时 | 关键文档超过 90 天未更新产生警告 |
| Markdown 格式 | 行尾空格、连续空行等格式问题 |

---

## 🐛 报告问题

### Bug 报告

请包含：
- 操作系统和版本
- EchoMind 版本（设置面板 → 关于）
- 复现步骤
- 期望行为 vs 实际行为
- 错误日志（如有）

### 功能请求

请描述：
- 使用场景（你是谁，想做什么）
- 期望的行为
- 为什么现有功能无法满足

---

## 📜 许可证

本项目使用 MIT 许可证。贡献的代码将在此许可证下发布。
