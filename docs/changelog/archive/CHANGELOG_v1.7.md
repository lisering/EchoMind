# EchoMind v1.7.0 CHANGELOG

> **发布日期**：2026-08-11 | **版本**：v1.7.0 | **上一版本**：v1.6.0

---

## 新增功能（4 项）

### S4: Q06/S71 Shadow Screen 生产集成（P0，ROI 7.3）

- **功能**：`security_posture == Strict` 时，对用户 query 执行 LLM 安全分类（`llm_classify`），与权威决策对比并记录 agree/disagree 统计
- **集成点**：`chat_inner` 中 provider 创建后调用 `execute_shadow_screen()`
- **安全隔离**：Shadow 筛查不影响对话流，仅收集统计
- **降级策略**：LLM 不可用 / 超时 5s → 降级为 Unscreened
- **TDD 测试**：TC-SHADOW-INTEGRATE-001~003（3 个测试）
- **关键文件**：`crates/core/src/security.rs`（`execute_shadow_screen` 函数）、`crates/tauri-app/src/commands/chat.rs`（集成点）

### S5: B06 SessionCoordinator 生产集成（P1，ROI 5.9）

- **功能**：用 `session_coordinator.run()` 包装 `chat_inner` 调用，实现会话级串行化
- **集成点**：`chat()` 命令中用 `coord.run(conversation_id, || async { ... })` 包装
- **行为**：同一 conversation_id 的请求串行化（防止并发写入消息历史损坏），不同 conversation_id 并发执行互不阻塞
- **TDD 测试**：TC-COORD-INTEGRATE-001~003（3 个测试）
- **关键文件**：`crates/tauri-app/src/commands/chat.rs`（`chat()` 函数）

### S6: 向量缓存 LRU 驱逐（P1，ROI 5.7）

- **功能**：`SqliteStorage::vector_cache` 从全量加载改为 `LruVectorCache` 带容量限制的 LRU 缓存
- **配置**：`max_vectors` 配置项（默认 5000），超限时驱逐最久未访问的向量
- **API**：新增 `set_max_vectors()` / `max_vectors()` 方法
- **搜索后 touch**：top-k 结果更新 LRU 访问顺序，保持热点向量在缓存中
- **TDD 测试**：TC-LRU-001~003 + 2 额外测试（5 个测试）
- **关键文件**：`crates/infra/src/sqlite_storage.rs`（`LruVectorCache` struct + `vector_search` 更新）

### S7: 启动并行化（P1，ROI 5.3）

- **功能**：`AppState::new()` 中 `tokio::join!` 并行化 ConfigStore 初始化 + settings 批量读取
- **优化**：10+ 次串行 `get_setting` DB 查询 → 1 次 `get_settings_batch` 批量查询
- **Tracing**：添加 `app_state_init` span + 启动耗时日志
- **TDD 测试**：TC-BOOT-001~002（2 个测试）
- **关键文件**：`crates/tauri-app/src/state.rs`（`AppState::new()` 重构）

---

## 测试基线

| 指标 | v1.6.0 | v1.7.0 | 变化 |
|---|---|---|---|
| Rust 测试 (Free) | 1228 passed | **1242 passed** | +14 |
| Clippy | 零警告 | 零警告 | — |
| cargo fmt | 通过 | 通过 | — |
| TypeScript | 0 errors | 0 errors | — |
| 新增测试 | — | 13 TDD 测试 | +13 |
| 版本号 | 1.6.0 | 1.7.0 | — |

---

## 技术约束遵守

- ✅ TDD（铁律二）：13 个新测试先写红灯再实现
- ✅ 零 unwrap/expect/panic（workspace deny lint）
- ✅ forbid unsafe
- ✅ 文档同步（铁律七）：CHANGELOG + 版本号更新
- ✅ GitHub 同步（铁律十一）：`sync-to-github.sh --commit`

---

## 不纳入 v1.7 的项目

| 项 | 理由 | 远期计划 |
|---|---|---|
| F5 OM3 Scored Selector | 当前单 provider 场景无需求 | 多 provider 路由时纳入 |
| F6 B01/B02 SystemContext+Epoch | 需 chat 启动流程重构 | v2.0 架构重设计 |
| F7 DS-01 PromptCache 集成 | 需本地 LLM 调用链重构 | mistral.rs API 暴露后 |
| F8 B04 CachePolicy 集成 | 需 LLM 调用链重构 | v2.0 架构重设计 |
