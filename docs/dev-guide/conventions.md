# 编码规范

> **状态**：活跃 | **最后审阅**：2026-07-25

---

## 1. Rust 编码规范

### 1.1 Edition 与工具链

- Edition 2024（`Cargo.toml` 中 `edition = "2024"`）
- 需要 Rust ≥1.85（stable 或 nightly）
- **clippy 必须通过**：`cargo clippy --all-targets -- -D warnings` 无新增警告

### 1.2 代码风格

- 遵循 `rustfmt` 默认配置
- 公开类型必须有文档注释 `///`
- 公开函数必须有文档注释 + `# Examples`（如适用）
- 使用 `#[must_use]` 标注返回值不可忽略的函数

### 1.3 错误处理（铁律五）

- 使用 `Result<T, E>` 而非 `Option` 或 panic
- 领域错误使用 `thiserror::Error` derive
- 组装层使用 `anyhow::Context`
- **生产代码严禁** `unwrap()` / `expect()` / `panic!()` / `unreachable!()` / `todo!()` / `unimplemented!()`
- Workspace CI 强制 deny lint：`unwrap_used`, `expect_used`, `panic`, `unreachable`, `todo`, `unimplemented`
- 测试代码豁免（`#[cfg(test)]` 模块内允许）
- `commands.rs` 中 `#![allow(clippy::unreachable)]` 是 `#[tauri::command]` 宏豁免，手写代码仍严禁

### 1.4 异步约定

- 使用原生 `async fn` in trait（Edition 2024），**禁用 `#[async_trait]` 宏**
- `#![allow(async_fn_in_trait)]` 是经架构评审的刻意豁免（Send + Sync supertrait 约束）
- 数据库操作使用 `tokio::task::spawn_blocking`
- **严禁跨 `.await` 持有 `std::sync::Mutex`**；使用 `tokio::sync::Mutex`

### 1.5 存储操作

- 所有 SQLite 操作通过 `tokio::task::spawn_blocking` 包装
- 向量存储为 BLOB（f32 字节）
- 余弦相似度在 Rust 中计算（全量加载后）

### 1.6 测试约定

- 单元测试与实现同文件（`#[cfg(test)] mod *_tests`）
- 集成测试在 `crates/tauri-app/tests/integration.rs`
- `*_inner` 函数模式：IPC 命令逻辑提取为 `*_inner(&AppState)` 供测试复用
- 命名：`test_<行为>_<条件>_<预期结果>`

## 2. 前端编码规范

### 2.1 架构约定

- 单文件 SPA：`ui/index.html`
- **禁止 CDN**：marked.js、DOMPurify、highlight.js、Tailwind CSS 全部本地打包于 `ui/vendor/`
- **禁止框架**：React / Vue / Svelte / jQuery 等一律禁用
- 纯原生 HTML / JS / Tailwind CSS

### 2.2 安全约定

- 用户输入必须经过 DOMPurify 净化
- 不使用 `innerHTML` 直接拼接用户输入
- 流式 Markdown 渲染必须走 marked.js → DOMPurify → highlight.js 链路
- 文件名经过净化防止路径遍历

### 2.3 注释规范（铁律三）

- 所有函数必须有 JSDoc 注释（`/** ... */`），标注 `@param`、`@returns`、`@throws`
- 复杂状态机和事件流必须有注释说明状态迁移与数据流向
- 严禁「无注释魔法函数」

## 3. Git 提交规范

### 3.1 分支策略

- `main`：受保护主干，任何时刻保持可构建、可发布
- 功能分支：`feat/req-<域>-<序号>-<短描述>`
- 修复分支：`fix/<简述>`
- 合并方式：PR + Squash Merge

### 3.2 提交信息格式

```
<type>(<scope>): <description>

类型：feat | fix | docs | refactor | test | chore | perf | ci | build | revert
范围：关联 REQ 时使用小写需求编号（如 req-ing-001）
```

示例：
```
feat(req-ing-001): 支持拖拽导入 md/txt
fix(req-rag-005): 停止生成后输入框未复位
test(req-lic-001): 免费版 50 文件配额红灯用例
docs(srs): 冻结 REQ-ING-001 导入契约
```

规则：**每完成一个原子任务提交一次**；一个提交只做一件事。

### 3.3 Git Hooks 自动化

项目内置 `.githooks/` 目录：
- `pre-commit`：冲突标记检测、TODO 提醒、敏感数据拦截
- `commit-msg`：Conventional Commits 格式校验

安装：`git config core.hooksPath .githooks`

## 4. CI/CD 门禁

| 检查 | 命令 | 必须通过 |
|---|---|---|
| 编译检查 | `cargo check` | ✅ |
| Clippy（含 deny lint） | `cargo clippy --all-targets -- -D warnings` | ✅ |
| 格式检查 | `cargo fmt --check` | ✅ |
| Rust 测试 | `cargo test` | ✅ |
| E2E 测试 | `cd e2e-tests && ECHOMIND_E2E_TEST=1 npx playwright test` | ✅ |
