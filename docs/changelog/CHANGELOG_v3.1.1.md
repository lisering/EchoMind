# CHANGELOG v3.1.1 — 代码清理 + 文档修复 + 依赖简化

> 发布日期：2026-09-05

本版本为维护性发布，聚焦代码库卫生、文档准确性和依赖瘦身。清理了已移除模块的残留引用、僵尸命令、死代码，以及文档中的过时数据。

---

## 🔧 修复

- **IPC 命令重复注册修复**：`rebuild_contextual_embeddings` 和 `rebuild_all_embeddings` 两处命令在 `generate_handler!` 中重复注册，导致编译警告。移除冗余注册项。

## 🧹 代码清理

- **删除 `crates/context` 死代码**：移除约 1,776 行未使用的 context crate 代码，该 crate 已从 workspace members 中移除。
- **清理 14 个未注册的僵尸命令实现**：这些命令存在于 `crates/tauri-app/src/commands/` 中但未在 `lib.rs` 的 `generate_handler!` 中注册，属于历史遗留死代码。

## 📚 文档修复

- **`docs/SOFTWARE_ASSET.md`**：修复 12+ 个已移除模块仍在文档中列出的问题；修复端口 Trait 列表约 30% 名单错误。
- **`docs/SRS_v1.0.md`**：修复 REQ-LLM-004~008 未标记 Deprecated 的问题。
- **`docs/sessions/CURRENT_STATE.md`**：修复 IPC 命令数偏高问题（233/212 → 181/171），与实际注册数对齐。
- **`PROJECT_RULES.md`**：修复 Rust 版本号格式和目录名不一致问题。

## 📦 依赖简化

- 从 `Cargo.toml` workspace members 中移除 `crates/context`，减少编译范围。

## ✅ 回归验证

| 项 | 结果 |
|---|---|
| Rust Free | 通过 |
| Clippy | 零警告 |
| cargo fmt | 通过 |
