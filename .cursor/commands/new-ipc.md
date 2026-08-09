---
description: 新增 IPC 命令（EchoMind 标准模式）
---

按 EchoMind 项目标准模式新增 Tauri IPC 命令。

参数: $ARGUMENTS（命令名和功能描述，如 "get_memory_stats 获取记忆统计"）

## 标准开发流程

### 1. 定义命令（commands/ 目录下对应模块文件）
在 `crates/tauri-app/src/commands/` 下找到对应模块文件（如 `features.rs`、`chat.rs`、`performance.rs` 等）。

```rust
/// 命令描述（铁律三：必须文档注释）。
#[tauri::command]
pub async fn <command_name>(
    <params>: <types>,
    state: State<'_, AppState>,
) -> Result<ReturnType, String> {
    <command_name>_inner(<params>, state.inner()).await
}

/// 命令逻辑（命令与集成测试复用，铁律十四 DIP）。
pub async fn <command_name>_inner(
    <params>: <types>,
    state: &AppState,
) -> Result<ReturnType, String> {
    // 实现...
}
```

### 2. 注册命令
在 `crates/tauri-app/src/lib.rs` 的 `invoke_handler!` 中注册命令名。

### 3. 更新 IPC 文档
更新 `docs/api/ipc-commands.md`，新增命令签名。

### 4. 编写测试
在 `crates/tauri-app/tests/integration.rs` 中新增集成测试。

### 5. 前端 mock
在 `e2e-tests/tests/bridge/tauri-stub.js` 中新增 mock。

### 6. 同步
- `docs/api/ipc-commands.md` 更新命令数
- `docs/sessions/CURRENT_STATE.md` 更新 IPC 命令总数
- `./sync-to-github.sh` 同步到发布仓库
