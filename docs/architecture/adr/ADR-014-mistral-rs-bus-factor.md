# ADR-014: mistral.rs bus factor=1 风险评估与缓解策略

> **状态**：✅ 已采纳（持续监控）
> **日期**：2026-08-16
> **决策者**：Chief Architect
> **来源**：R3 复盘报告 P1-4（`docs/sessions/archive/retrospective_v2_round2_R3.md`）

## 背景

EchoMind 的本地 LLM 推理（Pro 功能）依赖 `mistral.rs` crate（v0.9.0）。R3 复盘发现：

- **mistral.rs 的 bus factor = 1**：主要由 EricLBuehler 一人维护
- **仓库活跃度**：GitHub star ~2.8k，但核心开发者仅 1 人
- **风险**：如果核心开发者停止维护，EchoMind 的本地 LLM 功能将面临停滞
- **影响范围**：`crates/infra/src/local_llm.rs` 直接依赖 mistral.rs API

## 决策

采用三层缓解策略：

### 1. 持续监控（被动）

- 定期检查 mistral.rs 仓库活跃度（月度）
- 关注 mistral.rs 版本发布频率和 issue 响应时间
- 记录 mistral.rs API 变化对 EchoMind 的影响

### 2. 自研 GEMV 内核（主动 Plan B，Phase 3 已部分实现）

EchoMind 已在 Phase 3 实现了自研 GEMV 内核基础设施：

| 模块 | 文件 | 状态 |
|---|---|---|
| GGUF 文件解析器 | `crates/infra/src/gguf_reader.rs` | ✅ 完成（mmap 零拷贝，V1/V2/V3） |
| 量化块结构 + 反量化 | `crates/infra/src/quant_blocks.rs` | ✅ 完成（Q4_0/Q4K/Q8_0/Q8K） |
| GEMV 内核 | `crates/infra/src/gemv_kernel.rs` | ✅ 完成（4 种量化格式，标量+AVX2 预留） |
| 权重重排 | `crates/infra/src/weight_repack.rs` | ✅ 完成（Row-Major → Tile-Major） |
| 层级流式预取 | `crates/infra/src/layer_prefetch.rs` | ✅ 完成（madvise + 后台线程） |
| RAM 预算管理 | `crates/infra/src/memory_budget.rs` | ✅ 完成（LRU 驱逐） |
| KV Cache 序列化 | `crates/infra/src/kv_cache.rs` | ✅ 完成（磁盘格式） |
| 完整 transformer 前向传播 | — | ⏳ 未实现（attention/RMSNorm/SwiGLU/RoPE） |

**评估**：如果 mistral.rs 停止维护，EchoMind 可在自研 GEMV 基础设施上完成 transformer 前向传播实现。预计工作量：2-3 Sessions（attention + RMSNorm + SwiGLU + RoPE + 集成测试）。

### 3. 评估 candle 替代方案

- **candle**（Hugging Face）：纯 Rust ML 框架，bus factor 更高（HF 团队维护）
- **优势**：生态成熟，支持多种模型架构，社区活跃
- **劣势**：GGUF 支持不如 mistral.rs 完善，需要额外适配
- **评估结论**：作为 Plan C 备选，当 mistral.rs 停止维护且自研 GEMV 无法满足需求时启用

## 后果

- 正面：三层策略确保即使 mistral.rs 停止维护，EchoMind 本地 LLM 功能不受致命影响
- 正面：自研 GEMV 基础设施已就绪，仅需补全 transformer 前向传播
- 正面：KernelMode 枚举支持运行时切换（MistralRs ↔ CustomGemv）
- 负面：完整 transformer 前向传播未实现，当前 CustomGemv 模式不可用
- 中性：candle 评估为 Plan C，暂不实施

## 监控指标

| 指标 | 当前值 | 告警阈值 | 频率 |
|---|---|---|---|
| mistral.rs GitHub stars | ~2800 | < 2500（可能表示生态衰退） | 月度 |
| 最近 commit 日期 | 2026-08 | > 60 天无 commit | 月度 |
| 开放 issue 数 | ~150 | > 300（可能表示维护力下降） | 月度 |
| EchoMind API 兼容性 | v0.9.0 | major 版本升级需适配 | 版本发布时 |
