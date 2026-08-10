# DS4 (DwarfStar) 借鉴原子计划

> 源码调研：https://github.com/antirez/ds4  
> 本地路径：`/Users/john/code/ds4-main`  
> 调研日期：2026-08-10

## 1. ds4 项目概述

**DwarfStar** 是 antirez (Redis 作者) 开发的 DeepSeek V4 Flash 专用推理引擎，纯 C 实现：

- 不依赖 GGML，自研推理路径（ds4.c 80万行）
- Metal/CUDA/ROCm 三后端 + SSD 流式加载
- 分布式管道并行 + 张量并行（RDMA/TCP）
- 内置编码 Agent（KV cache 即 session 状态）
- OpenAI/Anthropic/Responses API 三协议兼容服务器
- 原生 DSML 工具调用（非 DSML 转换）

## 2. 借鉴分析（7 项）

### 高优先级（3 项）

| ID | 借鉴项 | ds4 源码位置 | EchoMind 目标位置 | ROI |
|---|---|---|---|---|
| DS-01 | Prompt Prefix 磁盘缓存 + LRU 驱逐 | `ds4_kvstore.c` | `crates/infra/src/prompt_cache.rs` | 20.0 |
| DS-02 | 上下文压缩：摘要 + 原文尾部保留 | `misc/COMPACT.md` | `crates/compact/src/lib.rs` | 15.0 |
| DS-03 | Session Strip 操作 | `ds4_agent.c` `/strip` | `crates/core/src/session.rs` | 8.0 |

### 中优先级（2 项）

| ID | 借鉴项 | ds4 源码位置 | EchoMind 目标位置 | ROI |
|---|---|---|---|---|
| DS-04 | 本地 LLM 功率管理 | `ds4.c --power N` | `crates/infra/src/local_llm.rs` | 6.0 |
| DS-05 | Live Tool Call 续接 | `misc/RESPONSE_API.md` | `crates/core/src/agent.rs` | 4.0 |

### 低优先级（2 项，仅参考）

| ID | 借鉴项 | 说明 |
|---|---|---|
| DS-06 | Radix Tree 精确重放 | ds4 用 rax.c 做 tool ID → DSML 映射；EchoMind 用 HashMap 即可 |
| DS-07 | SSD 流式自动缓存计划 | EchoMind 已有 memory_budget.rs，模式类似 |

## 3. 实施计划

### DS-01: Prompt Prefix 磁盘缓存 + LRU 驱逐

**借鉴设计** (ds4_kvstore.c):
- SHA1 文本前缀匹配（非 token 序列匹配，因为 BPE 可能在边界处分词不同）
- LRU 驱逐评分：`score = (effective_hits + 1) * tokens / file_size * exp2(-elapsed/6h)`
- 4 种保存时机：cold（首次长 prompt 稳定后）/ continued（定期）/ evict（替换时）/ shutdown
- 边界裁剪：trim 32 tokens, align 2048 tokens（防 BPE 重分词）
- 原子写入：.tmp → rename
- 预算管理 + 1% headroom

**EchoMind 适配**:
- mistral.rs v0.9.0 API 不暴露原始 KV 张量，因此缓存 token IDs + 元数据（非 KV payload）
- 缓存命中时跳过重新 tokenize + 通知调用方可复用前缀
- 文件格式：KPC header (32 bytes) + JSON payload (token_ids + metadata)
- 纯 Rust 实现，零外部依赖（SHA1 用 `sha1` crate）

**文件清单**:
- `crates/infra/src/prompt_cache.rs` — 新建
- `crates/infra/src/prompt_cache_tests.rs` — TDD 测试
- `crates/infra/src/lib.rs` — 注册模块
- `crates/infra/Cargo.toml` — 新增 sha1 依赖

### DS-02: 上下文压缩增强 — 原文尾部保留

**借鉴设计** (misc/COMPACT.md):
- 软触发：上下文使用 ≥85% 或剩余 <8192 tokens
- 硬触发：工具结果放不下时
- 压缩策略：
  1. 记录当前 transcript 长度为 BOTTOM
  2. LLM 生成旧状态摘要（目标/文件/命令/决策/下一步）
  3. 从 BOTTOM 向前扫描，取最近 10% 上下文（上限 50K tokens）原文
  4. 尾部对齐到 user message 边界
  5. 重建：system prompt + 摘要 + 原文尾部
- 摘要合约：不发明事实，描述可重新加载的大数据

**EchoMind 适配**:
- 在现有 compact crate 的 `CompactionEngine` 上增强
- 新增 `compact_with_verbatim_tail()` 方法
- `CompactionConfig` 新增 `tail_percentage`（默认 10%）和 `max_tail_tokens`（默认 50000）
- 复用现有 `STRUCTURED_SUMMARY_TEMPLATE`（5 章节结构化模板）
- 尾部对齐：扫描 `ChatMessage` 列表，找最后一个 `role == "user"` 的消息边界

**文件清单**:
- `crates/compact/src/lib.rs` — 增强
- `crates/compact/src/compact_tests.rs` — 新增 TDD 测试

## 4. 实施状态

### DS-01: ✅ 已完成

**实现内容**:
- `crates/infra/src/prompt_cache.rs` — Prompt Prefix 磁盘缓存（KPC 文件格式 + SHA1 匹配 + LRU 驱逐 + 边界裁剪/对齐 + 原子写入 + 预算管理）
- `crates/infra/src/prompt_cache_tests.rs` — 27 个 TDD 测试（SHA1 正确性 / 缓存打开关闭 / 存储+加载 / 最小 token 过滤 / 边界裁剪+对齐 / 持续保存间隔 / LRU 驱逐评分 / 驱逐逻辑 / 模型名称过滤 / ctx_size 过滤 / CacheReason 序列化 / 重复存储 / 原子写入 / 持久化跨实例 / 未找到返回 None）
- `crates/infra/src/lib.rs` — 注册 `prompt_cache` 模块 + `prompt_cache_tests` 测试模块
- `crates/infra/Cargo.toml` — 新增 `sha1` 依赖 + `tempfile` dev-dependency

**关键设计**:
- EchoMind 适配：mistral.rs 不暴露 KV 张量，缓存 token IDs + 元数据（非 KV payload）
- `find_text_prefix` / `find_exact` 使用 SHA1 完整匹配（非前缀 SHA1）
- LRU 驱逐评分公式：`(effective_hits + 1) * tokens / file_size * anchor_factor * exp2(-elapsed/6h)`
- 边界裁剪 + 对齐：trim 32 tokens, align 2048 tokens（防 BPE 重分词）
- 原子写入：.tmp → rename
- `is_none_or` 替换为 `is_none() || is_some_and()` 确保兼容性

### DS-02: ✅ 已完成

**实现内容**:
- `crates/compact/src/lib.rs` — 新增 `VerbatimTailConfig` 结构体 + `needs_verbatim_tail_compaction()` + `compact_with_verbatim_tail()` + `align_to_user_boundary()`
- `crates/compact/src/compact_tests.rs` — 14 个 TDD 测试（TC-DS02-001~014）

**关键设计**:
- 双触发机制：软触发（使用率 ≥ 85%）+ 硬触发（剩余 < 8192 tokens）
- 尾部保留比例 10%（上限 50000 tokens）
- 尾部对齐到 user message 边界（`role == "user"`）
- LLM 失败降级为截断提示（不返回 Err）
- 复用现有 `STRUCTURED_SUMMARY_TEMPLATE`（5 章节结构化模板）
- 压缩后历史 = [摘要 system 消息] + [原文尾部消息]

## 5. 验证标准

- [x] DS-01: prompt_cache 27 个 TDD 测试通过
- [x] DS-02: compact 新增 14 个 TDD 测试通过
- [x] `cargo test -p echomind-infra` prompt_cache 全量通过
- [x] `cargo test -p echomind-compact` 全量通过（65 passed）
- [x] `cargo clippy -p echomind-infra -p echomind-compact --all-targets -- -D warnings` 零警告
- [x] `cargo fmt` 通过
- [x] sync-to-github.sh 同步（5 files, 443 insertions, 123 deletions）
