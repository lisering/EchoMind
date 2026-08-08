# 文档注释规范

> **状态**：活跃 | **最后审阅**：2026-07-25
>
> 对应 PROJECT_RULES.md 体系三 §3.0 铁律三。

---

## 1. Rust 文档注释

### 1.1 模块级注释（`//!`）

每个 `.rs` 文件顶部必须有 `//!` 模块注释，说明：
- 模块职责（这个模块做什么）
- 架构约束（依赖方向、铁律引用）

```rust
//! EchoMind 六边形架构核心层（对应 REQ-ARCH-000）。
//! 铁律（PROJECT_RULES.md 体系三）：本 crate 只定义端口 Trait 与领域逻辑，
//! 严禁依赖 Tauri、reqwest、rusqlite 等任何具体实现。
```

### 1.2 公开项注释（`///`）

所有 `pub` 项（struct、enum、trait、fn、const）必须有 `///` 文档注释，说明：
- 用途
- 参数（`# Arguments`）
- 返回值（`# Returns`）
- 错误条件（`# Errors`）
- 约束与前置条件

```rust
/// 文档加载端口：从文件路径提取纯文本（对应 REQ-ING-001/002）。
pub trait Loader: Send + Sync {
    /// 读取并解析指定文件，返回纯文本内容；不支持的格式或 I/O 错误返回 Err。
    ///
    /// # Arguments
    /// * `file_path` - 文件绝对路径或相对路径
    ///
    /// # Errors
    /// - 文件不存在或无读取权限
    /// - 不支持的文件格式
    async fn load(&self, file_path: &str) -> anyhow::Result<String>;
}
```

### 1.3 行内注释（`//`）

复杂逻辑用行内 `//` 注释说明**「为什么」**而非「是什么」：

```rust
// 铁律五例外：#[tauri::command] 宏展开的代码内部使用 unreachable!()，
// Clippy 无法区分宏生成代码与手写代码，此处仅豁免 unreachable lint。
#![allow(clippy::unreachable)]
```

### 1.4 TODO 标记

任何「暂时这样写，以后再改」的代码必须标记 `// TODO(req-xxx): 说明` 并关联需求编号：

```rust
// TODO(req-vec-002): 当前批量大小固定为 256，后续应支持配置化
const EMBED_BATCH_SIZE: usize = 256;
```

---

## 2. 前端 JavaScript 注释

### 2.1 JSDoc 注释

所有函数必须有 JSDoc 注释（`/** ... */`），标注 `@param`、`@returns`、`@throws`：

```javascript
/**
 * 发送对话请求并处理流式响应。
 * @param {string} query - 用户输入的问题
 * @param {string} conversationId - 当前会话 ID
 * @param {Array} history - 历史消息列表
 * @returns {Promise<void>} 流式响应完成后 resolve
 * @throws {Error} 当 IPC 调用失败或流被中断时 reject
 */
async function sendChat(query, conversationId, history) { ... }
```

### 2.2 状态机注释

复杂状态机和事件流必须有注释说明状态迁移与数据流向：

```javascript
// 文档状态流转：Pending → Processing → Indexed / Failed
// 事件来源：doc-status-changed (Tauri Event)
// UI 映射：列表项徽标颜色 + 进度条
```

### 2.3 严禁无注释魔法函数

所有函数必须有明确注释。如果函数名不自解释，注释中必须说明用途。

---

## 3. 注释与代码同步

- 修改代码时必须同步更新注释
- 过时注释比没有注释更危险
- Code review 时注释与代码不一致视为 **review 阻塞项**
- 删除代码时同步删除其注释

---

## 4. `cargo doc` 生成

- `cargo doc` 生成的文档必须可读且自解释
- 所有 `pub` 项在生成的 HTML 文档中不应出现无注释的条目
- 模块级 `//!` 注释出现在模块文档页面顶部
