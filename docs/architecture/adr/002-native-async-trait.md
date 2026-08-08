# ADR-002: 原生 async fn in trait 而非 `#[async_trait]` 宏

> **状态**：✅ 已采纳
> **日期**：2026-07-24
> **决策者**：Chief Architect

## 背景

EchoMind 的端口 Trait（Loader, Splitter, Embedder, Storage, LLMProvider, Retriever）需要异步方法。Rust Edition 2024 原生支持 `async fn` in trait，但有一个已知 lint 警告：`async_fn_in_trait`（pub trait 无法约束返回 Future 的 auto traits）。

## 决策

使用原生 `async fn` in trait（Edition 2024），禁用 `#[async_trait]` 宏。通过以下方式处理 lint：

```rust
#![allow(async_fn_in_trait)]
```

所有 trait 添加 `Send + Sync` supertrait 约束，确保实现者可跨线程使用。端口仅供 workspace 内部消费，不对外发布。

## 考虑的替代方案

| 方案 | 优点 | 缺点 | 为什么没选 |
|---|---|---|---|
| `#[async_trait]` 宏 | 无 lint 警告 | proc-macro 编译开销；返回 `Pin<Box<dyn Future>>` 堆分配 | 性能开销不必要 |
| 手写 `Pin<Box<dyn Future>>` | 完全控制 | 代码冗长，每个方法手写返回类型 | 维护成本高 |
| `trait_variant` crate | 编译器支持 | 额外依赖 | 过度工程 |

## 后果

- 正面：零 proc-macro 开销，编译更快
- 正面：代码更自然（`async fn` 直接写）
- 正面：Edition 2024 原生支持，是未来方向
- 负面：`#![allow(async_fn_in_trait)]` 需要在每个 trait 文件添加
- 中性：如果未来需要对外发布 trait，需重新评估（pub trait 的 auto trait 约束问题）
