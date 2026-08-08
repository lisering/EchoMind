# Zed 架构模式借鉴规划

> **状态**：In-Progress ｜ **创建日期**：2026-07-31
>
> **来源**：基于 [Zed 源码](https://github.com/zed-industries/zed)（241 crates）深度分析

---

## 1. 规划总则

### 1.1 借鉴原则

1. **不改变核心架构** — 六边形（Ports & Adapters）+ Tauri + tokio 三大基石不变
2. **内部组织优化** — 在现有 crate 结构内提升状态管理、配置系统、测试框架质量
3. **向后兼容** — 所有改动不破坏现有 IPC 命令签名和前端 API
4. **TDD 驱动** — 每个模式先写红灯测试，再绿灯实现
5. **Pro 门控不变** — `#[cfg(feature = "pro")]` 条件编译策略不受影响

### 1.2 四个模式总览

| # | 模式 | 借鉴自 | 落地范围 | 优先级 | 预计 Session |
|---|---|---|---|---|---|
| 1 | Store 模式 | Zed `buffer_store`/`git_store`/`lsp_store` | `crates/tauri-app/src/state.rs` | P0 | S1 |
| 2 | 类型安全 Settings | Zed `settings_macros`/`RegisterSetting` | `crates/models` + `crates/tauri-app` | P1 | S2 |
| 3 | `#[perf]` 基准测试 | Zed `util_macros::perf`/`cargo perf-test` | `crates/infra` + workspace | P2 | S3 |
| 4 | Action 前端模式 | Zed `gpui::Action`/`actions!` 宏 | `ui/src/keyboard.js` + `ui/src/command-palette.js` | P2 | S4 |

---

## 2. Session 1：Store 模式 — 拆分 AppState

### 2.1 问题分析

当前 `AppState`（530 行）是一个巨型结构体，持有 15+ 个字段：
- 存储相关：`storage`, `data_dir`
- 安全相关：`security`, `clipboard_guard`
- 向量相关：`embedder`, `reranker`
- 多模态相关（Pro）：`page_renderer`, `ocr_engine`, `vision_provider`
- 会话中断：`abort_tokens`, `audit_cancels`, `import_cancel`
- 文件监听：`file_watchers`
- LLM 相关：`llm_config`, `is_pro`, `llm_mode`, `model_manager`, `local_llm`

**痛点**：所有状态共用同一结构体，无法独立测试；锁粒度过粗；职责不清晰。

### 2.2 目标架构

```
AppState
├── doc_store: DocumentStore       // 文档管理 + 导入取消 + 文件监听
├── chat_store: ChatStore           // 会话中断令牌 + 审计取消
├── security_store: SecurityStore   // SecurityManager + ClipboardGuard
├── model_store: ModelStore         // embedder + reranker + local_llm + model_manager
├── config_store: ConfigStore       // llm_config + is_pro + llm_mode
├── storage: SqliteStorage          // 原样保留（所有 Store 共享引用）
└── data_dir: PathBuf               // 原样保留
```

### 2.3 实施步骤

1. 创建 `crates/tauri-app/src/stores/mod.rs` 模块
2. 逐步迁移字段到各 Store，每个 Store 持有 `Arc<SqliteStorage>` 共享引用
3. `AppState` 保留便捷访问方法（委托到各 Store），确保 `commands.rs` 无需大改
4. 更新 `AppState::new()` 初始化逻辑

### 2.4 验收标准

- [x] `AppState` 字段数从 15+ 降到 7
- [x] 每个 Store 可独立单元测试
- [x] `cargo test` 全量通过（Free + Pro）
- [x] `cargo clippy --all-targets -- -D warnings` 零警告
- [x] IPC 命令签名不变（向后兼容）

---

## 3. Session 2：类型安全 Settings 系统

### 3.1 问题分析

当前 settings 是扁平 KV：`storage.set_setting("llm.mode", "local")` — 无类型安全，字符串拼错无法编译期发现。

### 3.2 目标架构

在 `crates/models` 定义 `SettingsPayload`（已有），新增 `SettingsRegistry` trait + derive 宏在 tauri-app 层。

### 3.3 实施步骤

1. 在 `crates/models` 定义 `SettingDefinition` 枚举（类型/key/默认值/描述）
2. 在 `crates/core` 定义 `SettingsRegistry` trait
3. 在 `crates/tauri-app` 实现 `SettingsRegistry` for `AppState`
4. 将所有 `set_setting("key", "value")` 调用替换为类型安全方法
5. 生成 settings schema JSON 供前端动态渲染

### 3.4 验收标准

- [x] 所有 settings key 在 `SettingsRegistry` 中集中定义
- [x] settings 读写有类型检查（编译期发现 key 拼错）
- [x] 前端可获取 settings schema JSON
- [x] `cargo test` 全量通过

---

## 4. Session 3：#[perf] 基准测试框架

### 4.1 问题分析

当前性能测试是 `#[ignore]` + 手动计时，缺乏系统化框架。

### 4.2 目标架构

创建 `crates/infra/src/perf.rs` 模块，提供：
- `PerfReport` 结构体（操作名/耗时/吞吐量/内存）
- `PerfRunner` 测试运行器
- `#[perf_test]` 属性宏（标注基准测试）

### 4.3 实施步骤

1. 创建 `crates/infra/src/perf.rs` — `PerfReport` + `PerfRunner`
2. 创建测试模块 `crates/infra/src/perf_tests.rs`
3. 编写 3 个基准测试（embedding/embedding_batch/vector_search）
4. 添加 `cargo perf-test` 别名到 `.cargo/config.toml`

### 4.4 验收标准

- [x] `PerfReport` 包含操作名/耗时/吞吐量
- [x] 3+ 个基准测试通过
- [x] `cargo perf-test` 命令可用

---

## 5. Session 4：Action 前端模式

### 5.1 问题分析

当前 `keyboard.js` 用 `if (mod && e.key === 'k')` 硬编码快捷键，难以扩展、无类型安全。

### 5.2 目标架构

创建 `ui/src/action.js` — 类型安全 Action 系统：
- `Action` 类（id/keybinding/handler/description）
- `ActionRegistry` — 注册表 + 快捷键 → Action 调度
- `ActionContext` — 执行上下文（当前状态、焦点元素等）

### 5.3 实施步骤

1. 创建 `ui/src/action.js` — Action 系统
2. 重构 `ui/src/keyboard.js` — 委托给 ActionRegistry
3. 统一命令面板和快捷键的 Action 定义
4. 更新 `build-ui.mjs` 模块顺序

### 5.4 验收标准

- [x] 所有快捷键通过 ActionRegistry 注册
- [x] `keyboard.js` 不再包含 `if (mod && e.key === ...)` 硬编码
- [x] 命令面板和快捷键共用同一 Action 注册表
- [x] 前端测试通过
