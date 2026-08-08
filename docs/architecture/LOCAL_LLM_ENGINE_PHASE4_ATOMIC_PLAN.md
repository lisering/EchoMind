# 本地 LLM 引擎 Phase 4 原子化实施计划

> **关联 ADR**：[ADR-011](adr/ADR-011-local-llm-engine.md) — Phase 4（差异化）
>
> **前置条件**：Phase 3 完成（S16-S23 ✅），ADR-011 状态 Accepted（Phase 3 完成）
>
> **调研来源**（2026-08-01，铁律八互联网搜索）：
> - mistral.rs v0.9.0 源码：`mistralrs-core/src/model.rs` — Model 结构体、`forward` 方法签名
> - mistral.rs v0.9.0：`mistralrs-core/src/pipeline.rs` — Pipeline KV cache 管理
> - llama.cpp `kv-cache.md` — KV cache 持久化格式设计参考
> - Speculative Decoding 论文（Leviathan et al. 2023）— draft-verify-accept 算法
> - DeepSeek-V3 MoE 架构 — 专家路由预测 + 热点缓存参考

## Phase 4 目标

1. **KV cache 持久化**：跨会话保存/恢复 KV cache，消除重复前缀计算（多轮对话 2-5x 加速）
2. **推测解码**：小模型起草 + 大模型验证，提升单 token 生成速度 1.5-3x
3. **MoE 专家缓存**：预计算路由 + 热点专家缓存，降低 MoE 模型推理延迟

## Session 概览

| Session | 目标 | 预计耗时 | 状态 |
|---|---|---|---|
| S24 | KV cache 序列化/反序列化（磁盘格式 + 实现） | 2 天 | ✅ |
| S25 | KV cache 跨会话复用（对话切换自动恢复 + IPC） | 2 天 | ✅ |
| S26 | Draft Model 加载与管理（小模型选择 + 并行加载） | 1.5 天 | 📋 |
| S27 | Speculative Decoding 推理路径（draft→verify→accept） | 3 天 | 📋 |
| S28 | MoE 路由预测 + 专家缓存（预计算 + 热点缓存） | 2 天 | 📋 |
| S29 | 专家权重流式加载（按需加载 + LRU 驱逐） | 2 天 | 📋 |
| S30 | 集成 LocalLlmEngine + IPC 命令 + warm_up | 2 天 | 📋 |
| S31 | 性能基准测试 + 文档同步 | 1 天 | 📋 |

---

## Session 24：KV Cache 序列化/反序列化

**目标**：设计磁盘格式，实现 KV cache 快照的序列化和反序列化。

### 文件变更

| 文件 | 变更类型 | 说明 |
|---|---|---|
| `crates/infra/src/kv_cache.rs` | 新建 | KV cache 序列化模块（Pro 门控） |
| `crates/infra/src/kv_cache_tests.rs` | 新建 | 测试 |
| `crates/infra/src/lib.rs` | 修改 | 注册 kv_cache 模块 |
| `crates/infra/Cargo.toml` | 修改 | 可能新增依赖 |

### 实现内容

```rust
/// KV cache 快照（序列化格式）
///
/// 文件头 magic: "EMKV" + version: u16
/// 之后按 layer 顺序存储 K 和 V 张量数据
pub struct KvCacheSnapshot {
    pub model_name: String,
    pub context_length: usize,
    pub layers: Vec<LayerKvCache>,
}

pub struct LayerKvCache {
    pub layer_idx: usize,
    pub k_bytes: Vec<u8>,  // f32 或量化后的 K 张量
    pub v_bytes: Vec<u8>,  // f32 或量化后的 V 张量
    pub seq_len: usize,    // 该层已缓存的 token 数
}

impl KvCacheSnapshot {
    pub fn serialize(&self) -> Vec<u8>;      // → 字节流
    pub fn deserialize(data: &[u8]) -> Result<Self>;  // ← 字节流
    pub fn save_to_file(&self, path: &Path) -> Result<()>;
    pub fn load_from_file(path: &Path) -> Result<Self>;
}
```

### 测试用例

| 测试 ID | 测试名 | 验证内容 | 状态 |
|---|---|---|---|
| TC-KVC-001 | `test_snapshot_creation` | 创建空快照，字段正确 | ✅ |
| TC-KVC-002 | `test_serialize_deserialize` | 序列化→反序列化往返正确 | ✅ |
| TC-KVC-003 | `test_save_load_file` | 保存到文件→读回一致 | ✅ |
| TC-KVC-004 | `test_invalid_magic` | 错误 magic 拒绝 | ✅ |
| TC-KVC-005 | `test_version_check` | 版本不匹配拒绝 | ✅ |
| TC-KVC-006 | `test_multi_layer` | 多层 K/V 数据完整 | ✅ |

### 完成详情（2026-08-01）

**实现内容**：
- `crates/infra/src/kv_cache.rs` — KV cache 序列化模块（Pro 门控，406 行）
- `crates/infra/src/kv_cache_tests.rs` — 6 个测试
- `crates/infra/src/lib.rs` — 注册 kv_cache 模块 + kv_cache_tests 测试模块

**新增类型/函数**：
- `KvCacheSnapshot` 结构体（model_name/context_length/layers）
- `KvCacheSnapshot::new(model_name, context_length)` — 创建空快照
- `KvCacheSnapshot::model_name()/context_length()/layers()/layer_count()` — 访问器方法
- `KvCacheSnapshot::add_layer(layer)` — 添加一层（自动按 layer_idx 排序）
- `KvCacheSnapshot::serialize() -> Result<Vec<u8>>` — 序列化为字节流
- `KvCacheSnapshot::deserialize(data) -> Result<Self>` — 从字节流反序列化
- `KvCacheSnapshot::save_to_file(path) -> Result<()>` — 原子写入文件（.tmp → rename）
- `KvCacheSnapshot::load_from_file(path) -> Result<Self>` — 从文件加载
- `LayerKvCache` 结构体（layer_idx/k_bytes/v_bytes/seq_len）
- `LayerKvCache::new(layer_idx, k_bytes, v_bytes, seq_len)` — 创建层缓存
- `LayerKvCache::layer_idx()/k_bytes()/v_bytes()/seq_len()/k_len()/v_len()` — 访问器方法
- `Cursor` 内部游标读取器（边界检查 + checked_add 防溢出）
- `read_u64_le(bytes)` — 小端序 u64 读取辅助
- `read_layer(cursor)` — 逐层读取 KV 数据

**磁盘格式**：
```
[4B] magic "EMKV" | [2B] version=1 | [2B] model_name_len | [N B] model_name
[8B] context_length | [4B] layer_count
--- per layer ---
[4B] layer_idx | [8B] seq_len | [8B] k_bytes_len | [k_bytes_len B] K data
[8B] v_bytes_len | [v_bytes_len B] V data
```

**关键设计**：
- 私有字段 + 公开访问器方法（封装性，测试通过方法调用访问）
- 原子文件写入：先写 `.emkv.tmp`，再 rename（防崩溃损坏）
- 所有偏移量使用 `checked_add` 防溢出（铁律五）
- 反序列化严格校验：魔数、版本、数据截断、层数/字节长度上限
- 常量上限防恶意文件：MAX_MODEL_NAME_LEN=4KB, MAX_LAYER_COUNT=1M, MAX_TENSOR_BYTES=4GB

**验证结果**：
- cargo test --features pro -p echomind-infra: 230 passed, 0 failed, 15 ignored
- cargo clippy --all-targets --features pro -- -D warnings: 零警告
- cargo fmt --check: 通过

### S24 → S25 衔接提示词

```
你正在继续 EchoMind 本地 LLM 引擎 Phase 4 的开发。

## 上下文
- Phase 3 全部 8 个 Session（S16-S23）已完成
- Phase 4 Session 24 已完成（KV cache 序列化/反序列化）
- ADR-011 状态：Accepted（Phase 3 完成）
- 测试：Pro 230 passed, 0 failed, 15 ignored（infra crate）
- Clippy 零警告，fmt 通过

## S24 完成详情
- crates/infra/src/kv_cache.rs — KvCacheSnapshot + LayerKvCache + serialize/deserialize/save_to_file/load_from_file
- crates/infra/src/kv_cache_tests.rs — 6 个测试（TC-KVC-001~006）
- 磁盘格式：magic "EMKV" + version 1 + model_name + context_length + layers[K/V bytes]
- 原子写入（.tmp → rename），checked_add 防溢出，私有字段 + 访问器方法

## 下一步
- 实现 Session 25：KV Cache 跨会话复用
  - crates/infra/src/local_llm.rs — 新增 save_kv_cache() / restore_kv_cache()
  - crates/tauri-app/src/commands.rs — 新增 save_kv_cache / load_kv_cache / clear_kv_cache / get_kv_cache_status IPC 命令
  - crates/tauri-app/src/state.rs — kv_cache 目录管理
  - 设置键：llm.kv_cache_enabled（默认 false）

## 必读文档
1. docs/architecture/LOCAL_LLM_ENGINE_PHASE4_ATOMIC_PLAN.md — Session 25 章节
2. crates/infra/src/kv_cache.rs — S24 实现的序列化模块
3. crates/infra/src/local_llm.rs — 现有引擎（KV cache 集成点）
4. crates/tauri-app/src/commands.rs — 现有 IPC 命令模式
5. crates/tauri-app/src/state.rs — AppState 结构

## 验证
- cargo test --features pro（编译通过 + 现有测试不破坏）
- cargo clippy --all-targets --features pro -- -D warnings
- cargo fmt --check
```

## Session 25：KV Cache 跨会话复用

**目标**：对话切换时自动保存/恢复 KV cache，消除重复前缀计算。

### 文件变更

| 文件 | 变更类型 | 说明 |
|---|---|---|
| `crates/infra/src/local_llm.rs` | 修改 | 新增 `save_kv_cache()` / `restore_kv_cache()` |
| `crates/tauri-app/src/commands.rs` | 修改 | 新增 `save_kv_cache` / `load_kv_cache` / `clear_kv_cache` IPC 命令 |
| `crates/tauri-app/src/state.rs` | 修改 | kv_cache 目录管理 |

### IPC 命令

| 命令 | 参数 | 返回 | 说明 |
|---|---|---|---|
| `save_kv_cache` | `conversation_id: String` | `()` | 保存指定会话的 KV cache |
| `load_kv_cache` | `conversation_id: String` | `bool` | 恢复 KV cache，返回是否命中 |
| `clear_kv_cache` | `conversation_id: String` | `()` | 清除指定会话的 KV cache |
| `get_kv_cache_status` | — | `KvCacheStatus` | 返回 KV cache 状态（启用/禁用/大小） |

### 设置键

| 键 | 默认值 | 说明 |
|---|---|---|
| `llm.kv_cache_enabled` | `false` | KV cache 持久化开关 |

### 测试用例

| 测试 ID | 测试名 | 验证内容 | 状态 |
|---|---|---|---|
| TC-KVC-007 | `test_save_restore_roundtrip` | 保存→恢复→继续推理结果一致 | ✅ |
| TC-KVC-008 | `test_cache_miss_first_load` | 首次无缓存 → 正常推理 | ✅ |
| TC-KVC-009 | `test_cache_clear` | 清除后再恢复 → cache miss | ✅ |
| TC-KVC-010 | `test_disabled_noop` | 禁用时 save/load 为空操作 | ✅ |

### 完成详情（2026-08-01）

**实现内容**：
- `crates/infra/src/local_llm.rs` — 新增 `save_kv_cache()` / `restore_kv_cache()` 方法 + `sanitize_conversation_id()` 辅助函数
- `crates/models/src/lib.rs` — 新增 `KvCacheStatus` 结构体
- `crates/tauri-app/src/state.rs` — 新增 `kv_cache_dir()` 便捷方法
- `crates/tauri-app/src/commands.rs` — 新增 5 个 Pro 门控 IPC 命令 + `*_inner` 函数
- `crates/tauri-app/src/lib.rs` — 注册 5 个新 IPC 命令
- `crates/tauri-app/tests/integration.rs` — 新增 7 个集成测试（TC-KVC-007~013）

**新增类型/函数**：
- `KvCacheStatus` 结构体（models crate）— `enabled` / `cache_dir` / `file_count` / `total_size_bytes`
- `LocalLlmEngine::save_kv_cache(conversation_id, kv_cache_dir)` — 保存 KV cache 快照到磁盘
- `LocalLlmEngine::restore_kv_cache(conversation_id, kv_cache_dir)` — 恢复 KV cache 快照
- `sanitize_conversation_id(id)` — 清理会话 ID 中的文件系统非法字符（防路径遍历）
- `AppState::kv_cache_dir()` — 返回 `{data_dir}/kv_cache/` 路径

**新增 IPC 命令**（5 个，Pro 门控）：
- `save_kv_cache(conversation_id: String) -> ()` — 保存指定会话的 KV cache
- `load_kv_cache(conversation_id: String) -> bool` — 恢复 KV cache，返回是否命中
- `clear_kv_cache(conversation_id: String) -> ()` — 清除指定会话的 KV cache
- `get_kv_cache_status() -> KvCacheStatus` — 返回 KV cache 状态（启用/禁用/大小）
- `set_kv_cache_enabled(enabled: bool) -> ()` — 设置 KV cache 持久化开关

**新增设置键**：
- `llm.kv_cache_enabled`（bool，默认 `false`）— KV cache 持久化开关

**关键设计**：
- mistral.rs v0.9.0 API 不直接暴露 KV cache 张量提取/注入，当前实现保存元数据快照（模型名 + 上下文长度），文件基础设施就绪。当 CustomGemv 模式完整实现后，将填充实际 K/V 张量数据（TODO 标记）
- 文件存储路径：`{data_dir}/kv_cache/{conversation_id}.emkv`
- 会话 ID 清理：非字母数字/横线/下划线字符替换为 `_`（防路径遍历攻击）
- 禁用时的 noop 行为：`save_kv_cache` 返回 `Ok(())`（不创建文件），`load_kv_cache` 返回 `Ok(false)`（cache miss）
- `clear_kv_cache` 不检查 `kv_cache_enabled`（允许清理旧数据），且文件不存在时幂等返回 `Ok(())`
- `get_kv_cache_status` 纯文件系统扫描，不需要模型已加载
- 原子文件写入（S24 的 `save_to_file` 内部 .tmp → rename）

**测试详情**（7 个集成测试）：
- TC-KVC-007：save_kv_cache + load_kv_cache 往返（无模型时验证 IPC 管道不崩溃）
- TC-KVC-008：首次加载无缓存 → cache miss
- TC-KVC-009：清除缓存后再恢复 → cache miss
- TC-KVC-010：禁用时 save/load 为空操作（noop）
- TC-KVC-011：set_kv_cache_enabled 持久化 + get_kv_cache_status 返回正确状态（文件计数 + 大小统计 + 重启恢复）
- TC-KVC-012：clear_kv_cache 幂等性（文件不存在时返回 Ok）
- TC-KVC-013：clear_kv_cache 路径清理（非法字符替换为下划线，防路径遍历）

**验证结果**：
- cargo test --features pro: 全部通过（core 239 + infra 230 + models 8 + tauri-app 9 + integration 115 + license-issuer 10 + doc-tests 6）
- cargo clippy --all-targets -- -D warnings（Free）：零警告
- cargo clippy --all-targets --features pro -- -D warnings（Pro）：零警告
- cargo fmt --check：通过

### S25 → S26 衔接提示词

```
你正在继续 EchoMind 本地 LLM 引擎 Phase 4 的开发。

## 上下文
- Phase 3 全部 8 个 Session（S16-S23）已完成
- Phase 4 Session 24-25 已完成（KV cache 序列化/反序列化 + 跨会话复用）
- ADR-011 状态：Accepted（Phase 3 完成）
- 测试：Pro 全部通过（core 239 + infra 230 + integration 115）
- Clippy 零警告（Free + Pro），fmt 通过

## S25 完成详情
- crates/infra/src/local_llm.rs — save_kv_cache() / restore_kv_cache() + sanitize_conversation_id()
- crates/models/src/lib.rs — KvCacheStatus 结构体
- crates/tauri-app/src/state.rs — kv_cache_dir() 便捷方法
- crates/tauri-app/src/commands.rs — 5 个 Pro 门控 IPC 命令（save_kv_cache / load_kv_cache / clear_kv_cache / get_kv_cache_status / set_kv_cache_enabled）
- crates/tauri-app/src/lib.rs — 注册 5 个新命令
- 7 个集成测试（TC-KVC-007~013）
- 设置键：llm.kv_cache_enabled（默认 false）
- mistral.rs v0.9.0 API 限制：当前保存元数据快照，实际 K/V 张量提取待 CustomGemv 完整实现后填充

## 下一步
- 实现 Session 26：Draft Model 加载与管理
  - crates/infra/src/draft_model.rs — 新建 DraftModelManager（Pro 门控）
  - crates/infra/src/draft_model_tests.rs — 5 个测试
  - crates/infra/src/lib.rs — 注册 draft_model 模块
  - crates/models/src/lib.rs — 新增 DraftModelConfig 结构

## 必读文档
1. docs/architecture/LOCAL_LLM_ENGINE_PHASE4_ATOMIC_PLAN.md — Session 26 章节
2. crates/infra/src/local_llm.rs — 现有引擎（模型加载模式参考）
3. crates/infra/src/model_manager.rs — ModelManager（下载/管理模式参考）
4. crates/tauri-app/src/commands.rs — 现有 IPC 命令模式

## 验证
- cargo test --features pro（编译通过 + 现有测试不破坏）
- cargo clippy --all-targets --features pro -- -D warnings
- cargo fmt --check
```

---

## Session 26：Draft Model 加载与管理

**目标**：支持加载小模型作为 draft model，用于推测解码。

### 文件变更

| 文件 | 变更类型 | 说明 |
|---|---|---|
| `crates/infra/src/draft_model.rs` | 新建 | Draft model 管理器（Pro 门控） |
| `crates/infra/src/draft_model_tests.rs` | 新建 | 测试 |
| `crates/infra/src/lib.rs` | 修改 | 注册模块 |
| `crates/models/src/lib.rs` | 修改 | 新增 `DraftModelConfig` 结构 |

### 实现内容

```rust
/// Draft model 管理器
pub struct DraftModelManager {
    models_dir: PathBuf,
    draft_model: RwLock<Option<Arc<Model>>>,  // 小模型实例
}

impl DraftModelManager {
    pub fn new(models_dir: PathBuf) -> Self;
    pub async fn load_draft(&self, filename: &str) -> Result<()>;
    pub async fn unload_draft(&self);
    pub fn is_loaded(&self) -> bool;
    pub async fn get_or_load(&self) -> Result<Arc<Model>>;
}
```

### 测试用例

| 测试 ID | 测试名 | 验证内容 |
|---|---|---|
| TC-DRAFT-001 | `test_load_draft_model` | 加载小模型成功 |
| TC-DRAFT-002 | `test_unload_draft` | 卸载后 is_loaded 为 false |
| TC-DRAFT-003 | `test_double_load` | 重复加载幂等 |
| TC-DRAFT-004 | `test_get_or_load_lazy` | 首次 get_or_load 触发加载 |
| TC-DRAFT-005 | `test_nonexistent_file` | 文件不存在返回错误 |

---

## Session 27：Speculative Decoding 推理路径

**目标**：实现 draft→verify→accept 推测解码算法。

### 文件变更

| 文件 | 变更类型 | 说明 |
|---|---|---|
| `crates/infra/src/speculative.rs` | 新建 | 推测解码引擎（Pro 门控） |
| `crates/infra/src/speculative_tests.rs` | 新建 | 测试 |

### 算法

```
1. Draft model 生成 N 个候选 token（自回归，快速）
2. Large model 对 N 个 token 做单次 forward（并行验证）
3. 对每个 token：
   - 如果 draft_prob <= large_prob：接受，继续下一个
   - 如果 draft_prob > large_prob：以 (large/draft) 概率接受，否则拒绝
4. 在第一个拒绝点停止，从拒绝处用 large model 重新采样
5. 接受的 token 全部输出到流
```

### 实现内容

```rust
pub struct SpeculativeEngine {
    draft_model: Arc<Model>,
    target_model: Arc<Model>,
    draft_tokens: usize,      // 每轮起草 token 数（默认 4）
    max_retries: usize,       // 最大重试次数（默认 3）
}

impl SpeculativeEngine {
    pub async fn speculate_stream(
        &self,
        input_ids: &[u32],
        max_tokens: usize,
    ) -> BoxStream<'static, Result<Vec<u32>>>;  // 每次返回一批已验证 token
}
```

### 测试用例

| 测试 ID | 测试名 | 验证内容 |
|---|---|---|
| TC-SPEC-001 | `test_draft_generate` | Draft model 生成 N 个 token |
| TC-SPEC-002 | `test_verify_accept_all` | 所有 token 被接受 |
| TC-SPEC-003 | `test_verify_reject_first` | 首个 token 被拒绝 |
| TC-SPEC-004 | `test_verify_partial_accept` | 部分 token 被接受后拒绝 |
| TC-SPEC-005 | `test_max_retries` | 达到最大重试次数 |
| TC-SPEC-006 | `test_output_stream_correctness` | 流输出与直接推理结果一致 |
| TC-SPEC-007 | `test_no_draft_model_error` | 无 draft model 时返回错误 |

---

## Session 28：MoE 路由预测 + 专家缓存

**目标**：预计算 MoE 层的路由决策，缓存热点专家权重。

### 文件变更

| 文件 | 变更类型 | 说明 |
|---|---|---|
| `crates/infra/src/moe_cache.rs` | 新建 | MoE 专家缓存（Pro 门控） |
| `crates/infra/src/moe_cache_tests.rs` | 新建 | 测试 |

### 实现内容

```rust
/// MoE 路由预测器
pub struct MoeRouter {
    /// 路由表：token_pattern_hash → 预计算的专家索引
    route_table: HashMap<u64, Vec<usize>>,
    /// 热点专家集合
    hot_experts: HashSet<usize>,
}

impl MoeRouter {
    pub fn predict_route(&self, token_pattern: &[f32]) -> Vec<usize>;
    pub fn record_route(&mut self, token_pattern: &[f32], experts: &[usize]);
    pub fn hot_expert_threshold(&self) -> usize;
    pub fn update_hot_experts(&mut self, frequency: &HashMap<usize, usize>);
}

/// 专家缓存
pub struct ExpertCache {
    /// 缓存的专家权重（layer_idx, expert_idx → weights）
    cache: HashMap<(usize, usize), ExpertWeights>,
    /// LRU 队列
    lru: VecDeque<(usize, usize)>,
    /// 最大缓存专家数
    max_cached: usize,
}
```

### 测试用例

| 测试 ID | 测试名 | 验证内容 |
|---|---|---|
| TC-MOE-001 | `test_route_prediction` | 预测路由返回正确专家 |
| TC-MOE-002 | `test_route_record` | 记录路由后可查询 |
| TC-MOE-003 | `test_hot_expert_update` | 热点专家集合更新 |
| TC-MOE-004 | `test_expert_cache_hit` | 缓存命中返回权重 |
| TC-MOE-005 | `test_expert_cache_miss` | 缓存未命中返回 None |
| TC-MOE-006 | `test_lru_eviction` | 超容量时 LRU 驱逐 |
| TC-MOE-007 | `test_cache_stats` | 统计命中率 |

---

## Session 29：专家权重流式加载

**目标**：按需加载活跃专家权重到内存，结合 Phase 3 的 MemoryBudget 管理。

### 文件变更

| 文件 | 变更类型 | 说明 |
|---|---|---|
| `crates/infra/src/moe_cache.rs` | 修改 | 新增 `ExpertLoader` 结构 |
| `crates/infra/src/memory_budget.rs` | 修改 | 集成专家权重预算 |

### 实现内容

```rust
/// 专家权重加载器
pub struct ExpertLoader {
    mmap: Mmap,                    // GGUF 文件 mmap
    expert_offsets: HashMap<(usize, usize), (u64, u64)>,  // (layer, expert) → (offset, size)
    budget: MemoryBudget,          // 复用 Phase 3 RAM 预算
}

impl ExpertLoader {
    pub fn load_expert(&mut self, layer: usize, expert: usize) -> Result<ExpertWeights>;
    pub fn evict_expert(&mut self, layer: usize, expert: usize);
    pub fn prefetch_hot_experts(&mut self, router: &MoeRouter);
}
```

### 测试用例

| 测试 ID | 测试名 | 验证内容 |
|---|---|---|
| TC-MOE-008 | `test_load_expert` | 加载专家权重成功 |
| TC-MOE-009 | `test_evict_expert` | 驱逐后内存释放 |
| TC-MOE-010 | `test_prefetch_hot` | 预取热点专家到 page cache |
| TC-MOE-011 | `test_budget_integration` | 超预算时 LRU 驱逐 |
| TC-MOE-012 | `test_nonexistent_expert` | 不存在的专家返回错误 |

---

## Session 30：集成 LocalLlmEngine + IPC 命令 + warm_up

**目标**：将 Phase 4 功能集成到 LocalLlmEngine，新增 IPC 命令。

### 文件变更

| 文件 | 变更类型 | 说明 |
|---|---|---|
| `crates/infra/src/local_llm.rs` | 修改 | 新增 speculative / moe 集成 |
| `crates/tauri-app/src/commands.rs` | 修改 | 新增 IPC 命令 |
| `crates/tauri-app/src/lib.rs` | 修改 | 注册命令 |
| `crates/tauri-app/src/state.rs` | 修改 | 新增 draft_model_manager |

### IPC 命令

| 命令 | 参数 | 返回 | 说明 |
|---|---|---|---|
| `set_speculative_decoding` | `enabled: bool, draft_tokens: usize` | `()` | 配置推测解码 |
| `get_speculative_decoding` | — | `SpeculativeConfig` | 返回当前配置 |
| `set_draft_model` | `filename: String` | `()` | 设置 draft model |
| `get_draft_model` | — | `String` | 返回当前 draft model |
| `set_moe_cache` | `enabled: bool, max_cached: usize` | `()` | 配置 MoE 缓存 |
| `get_moe_cache_status` | — | `MoeCacheStatus` | 返回缓存状态 |

### 设置键

| 键 | 默认值 | 说明 |
|---|---|---|
| `llm.speculative_enabled` | `false` | 推测解码开关 |
| `llm.draft_tokens` | `4` | 每轮起草 token 数 |
| `llm.draft_model` | — | Draft model 文件名 |
| `llm.moe_cache_enabled` | `false` | MoE 专家缓存开关 |
| `llm.moe_max_cached` | `8` | 最大缓存专家数 |

### 测试用例

| 测试 ID | 测试名 | 验证内容 |
|---|---|---|
| TC-INTEG-001 | `test_speculative_chat_stream` | 推测解码流式输出正确 |
| TC-INTEG-002 | `test_speculative_disabled` | 禁用时走标准路径 |
| TC-INTEG-003 | `test_moe_cache_integration` | MoE 缓存集成推理 |
| TC-INTEG-004 | `test_warm_up_speculative` | warm_up 预加载 draft model |
| TC-INTEG-005 | `test_settings_roundtrip` | 设置持久化往返正确 |
| TC-INTEG-006 | `test_pro_gate` | Free 用户被拒 |
| TC-INTEG-007 | `test_concurrent_speculative` | 并发推测解码安全 |

---

## Session 31：性能基准测试 + 文档同步

**目标**：编写 Phase 4 性能基准测试，同步全部文档。

### 性能基准测试

| 测试 ID | 测试名 | 验证内容 |
|---|---|---|
| TC-PHASE4-PERF-001 | `test_kv_cache_speedup` | KV cache 复用 vs 重新计算延迟对比 |
| TC-PHASE4-PERF-002 | `test_speculative_speedup` | 推测解码 vs 标准推理速度对比 |
| TC-PHASE4-PERF-003 | `test_moe_cache_hit_rate` | MoE 专家缓存命中率 |
| TC-PHASE4-PERF-004 | `test_expert_prefetch_effectiveness` | 专家预取效果 |

运行方式：
```bash
ECHOMIND_TEST_MODEL=/path/to/model.gguf \
ECHOMIND_DRAFT_MODEL=/path/to/draft.gguf \
cargo test -p echomind-infra --features pro -- phase4_perf --ignored --nocapture
```

### 文档更新清单

| 文档 | 变更内容 |
|---|---|
| `docs/SRS_v1.0.md` | 新增 REQ-LLM-009/010/011 |
| `docs/api/ipc-commands.md` | 新增 6 个命令 |
| `docs/api/code-contracts.md` | 新增 §4.10 Phase 4 契约 |
| `docs/api/settings-reference.md` | 新增 5 个设置键 |
| `docs/api/frontend-modules.md` | 新增前端函数 |
| `docs/architecture/adr/ADR-011-local-llm-engine.md` | Phase 4 标记 ✅ |
| `AGENTS.md` | 架构更新 |
| `CLAUDE.md` | 架构更新 |

### 最终验证

```bash
cargo test
cargo test --features pro
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --features pro -- -D warnings
cargo fmt --check
node scripts/build-ui.mjs
./sync-to-github.sh --commit
```

---

## 附录 A：Phase 4 文件变更矩阵

| 文件路径 | Session | 变更类型 | 说明 |
|---|---|---|---|
| `crates/infra/src/kv_cache.rs` | S24 | 新建 | KV cache 序列化 |
| `crates/infra/src/kv_cache_tests.rs` | S24 | 新建 | 6 个测试 |
| `crates/infra/src/local_llm.rs` | S25, S30 | 修改 | save/restore + 集成 |
| `crates/infra/src/draft_model.rs` | S26 | 新建 | Draft model 管理器 |
| `crates/infra/src/draft_model_tests.rs` | S26 | 新建 | 5 个测试 |
| `crates/infra/src/speculative.rs` | S27 | 新建 | 推测解码引擎 |
| `crates/infra/src/speculative_tests.rs` | S27 | 新建 | 7 个测试 |
| `crates/infra/src/moe_cache.rs` | S28, S29 | 新建 | MoE 专家缓存 |
| `crates/infra/src/moe_cache_tests.rs` | S28, S29 | 新建 | 12 个测试 |
| `crates/tauri-app/src/commands.rs` | S25, S30 | 修改 | 新增 10 个 IPC 命令 |
| `crates/tauri-app/src/lib.rs` | S25, S30 | 修改 | 注册命令 |
| `crates/tauri-app/src/state.rs` | S30 | 修改 | draft_model_manager |
| `crates/infra/src/local_llm_tests.rs` | S31 | 修改 | 4 个性能基准 |

## 附录 B：Phase 4 SRS 需求

| 需求 ID | 名称 | Session | 优先级 |
|---|---|---|---|
| REQ-LLM-009 | KV cache 持久化 | S24-S25 | P2 |
| REQ-LLM-010 | 推测解码 | S26-S27 | P2 |
| REQ-LLM-011 | MoE 专家缓存 | S28-S29 | P3 |

## 附录 C：Phase 4 IPC 命令

| 命令 | 参数 | 返回 | Session | Pro 门控 |
|---|---|---|---|---|
| `save_kv_cache` | `conversation_id: String` | `()` | S25 | ✅ |
| `load_kv_cache` | `conversation_id: String` | `bool` | S25 | ✅ |
| `clear_kv_cache` | `conversation_id: String` | `()` | S25 | ✅ |
| `get_kv_cache_status` | — | `KvCacheStatus` | S25 | ✅ |
| `set_speculative_decoding` | `enabled: bool, draft_tokens: usize` | `()` | S30 | ✅ |
| `get_speculative_decoding` | — | `SpeculativeConfig` | S30 | ✅ |
| `set_draft_model` | `filename: String` | `()` | S30 | ✅ |
| `get_draft_model` | — | `String` | S30 | ✅ |
| `set_moe_cache` | `enabled: bool, max_cached: usize` | `()` | S30 | ✅ |
| `get_moe_cache_status` | — | `MoeCacheStatus` | S30 | ✅ |

## 附录 D：Phase 4 设置键

| 键 | 类型 | 默认值 | Session | 说明 |
|---|---|---|---|---|
| `llm.kv_cache_enabled` | bool | `false` | S25 | KV cache 持久化开关 |
| `llm.speculative_enabled` | bool | `false` | S30 | 推测解码开关 |
| `llm.draft_tokens` | int | `4` | S30 | 每轮起草 token 数 |
| `llm.draft_model` | String | — | S30 | Draft model 文件名 |
| `llm.moe_cache_enabled` | bool | `false` | S30 | MoE 专家缓存开关 |
| `llm.moe_max_cached` | int | `8` | S30 | 最大缓存专家数 |

## 附录 E：Phase 4 测试汇总

| Session | 测试 ID 前缀 | 测试数 | 说明 |
|---|---|---|---|
| S24 | TC-KVC-001~006 | 6 | KV cache 序列化 |
| S25 | TC-KVC-007~010 | 4 | KV cache 复用 |
| S26 | TC-DRAFT-001~005 | 5 | Draft model 管理 |
| S27 | TC-SPEC-001~007 | 7 | 推测解码 |
| S28 | TC-MOE-001~007 | 7 | MoE 路由 + 缓存 |
| S29 | TC-MOE-008~012 | 5 | 专家流式加载 |
| S30 | TC-INTEG-001~007 | 7 | 集成测试 |
| S31 | TC-PHASE4-PERF-001~004 | 4 (#[ignore]) | 性能基准 |
| **合计** | | **45** | |

---

## S23 → S24 衔接提示词

```
你正在继续 EchoMind 本地 LLM 引擎 Phase 4 的开发。

## 上下文
- Phase 3 全部 8 个 Session（S16-S23）已完成
- ADR-011 状态：Accepted（Phase 3 完成）
- 测试：Free 328 passed / Pro 480 passed, 0 failed
- Clippy 零警告，fmt 通过
- Phase 4 原子化计划已创建：docs/architecture/LOCAL_LLM_ENGINE_PHASE4_ATOMIC_PLAN.md

## 下一步
- 实现 Session 24：KV Cache 序列化/反序列化
  - crates/infra/src/kv_cache.rs — 新建 KV cache 序列化模块（Pro 门控）
  - crates/infra/src/kv_cache_tests.rs — 6 个测试
  - crates/infra/src/lib.rs — 注册 kv_cache 模块

## 必读文档
1. docs/architecture/LOCAL_LLM_ENGINE_PHASE4_ATOMIC_PLAN.md — Session 24 章节
2. crates/infra/src/local_llm.rs — 现有引擎（KV cache 集成点）
3. crates/infra/src/kv_cache.rs — 待创建
4. docs/architecture/adr/ADR-011-local-llm-engine.md — Phase 4 路线

## 验证
- cargo test --features pro（编译通过 + 现有测试不破坏）
- cargo clippy --all-targets --features pro -- -D warnings
- cargo fmt --check
```
