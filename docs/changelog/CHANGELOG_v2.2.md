# EchoMind v2.2.0 — 全方位性能优化

> **发布日期**: 2026-08-18
> **版本号**: 2.2.0
> **基线**: v2.1.0 安全加固版
> **战略方向**: 全链路性能分析 → 6 个性能 REQ 实施 → +28 测试

---

## 概述

v2.2.0 是 v2.1 安全加固版发布后的性能优化增量发布。本次发布聚焦一个方向：

**全方位性能优化** — 全链路性能分析产出 9 个热点（H1-H9），实施 6 个 REQ 全部 Implemented，+28 测试，零新增依赖。

---

## 量化改进

| 指标 | v2.1.0 | v2.2.0 | 改善 |
|---|---|---|---|
| Rust 单元+集成测试 (Free) | 1,442 | 1,540 | +6.8% (+98) |
| Rust 单元+集成测试 (Pro) | 1,733 | 1,831 | +5.7% (+98) |
| HNSW 索引持久化 | 不落盘 | 二进制加密持久化 + 懒加载 | 二次启动首检索 O(n) → O(1) 加载 |
| 向量缓存正确性 | LRU 5000 截断 bug | Arc 快照零深拷贝 + 自动预算 | 正确性修复 + 零拷贝 |
| 余弦相似度计算 | 3×乘加/dim | 归一化点积 1×乘加/dim | ~3× FLOPs 降低 |
| 检索管线 | 图扩展串行 | tokio::join! 并行 | 延迟降低 |
| BM25 检索 | 原始 chunk 文本 | Contextual BM25 | 失败率 ↓17.6%（Anthropic） |
| 语义缓存阈值 | 固定 0.92 | [0.85, 0.97] 动态校准 | 命中率自适应优化 |

---

## 1. HNSW 索引持久化与懒加载 (REQ-PERF-015, P0)

**问题**: HNSW 索引不落盘，每次启动/文档变更后首次检索全量重建 O(n log n)。

**实现**: `crates/infra/src/hnsw_index.rs` + `crates/infra/src/storage/crypto.rs` + `crates/infra/src/sqlite_storage.rs`

| 变更 | 说明 |
|---|---|
| 二进制持久化格式 | 魔数 + 版本号 + 维度校验 + 压缩向量数据 |
| 懒加载 | `SqliteStorage::new()/new_encrypted()` 启动不加载；首次检索时加载磁盘索引 |
| 崩溃一致性 | 磁盘索引缺失/损坏/维度不匹配 → 回退全量构建 → 自动落盘 |
| 加密落盘 | Pro 加密模式下，索引文件使用 AES-256 加密 |
| dirty 标记 | 写入/删除 embedding 后 `mark_hnsw_dirty()`，下次检索重建 |

**测试**: TC-PERSIST-001~008（save/load 往返 / 魔数校验 / 维度不匹配回退 / 损坏文件回退 / dirty 重建 / 空索引 / 大索引性能护栏 / 与全量扫描一致性）

## 2. 向量缓存正确性修复 + 内存路径加速 (REQ-PERF-016, P0)

**问题**: LRU 向量缓存 `max_vectors=5000` 硬编码，>5000 向量时后段 chunk 被静默排除出检索（正确性 bug）+ 每次查询 `to_vec()` 全量克隆。

**实现**: `crates/infra/src/sqlite_storage.rs`

| 变更 | 说明 |
|---|---|
| LRU 截断 bug 修复 | 缓存容量按内存预算动态计算（`max_vectors = max(5000, 知识库向量总数)`） |
| BinaryHeap top-k | 全量扫描路径 top-k 使用 BinaryHeap（O(n log k)），替代全排序 O(n log n) |
| Arc 快照零拷贝 | 缓存命中路径零全量克隆：读锁内直接计算（`Arc<RwLock>` 快照语义） |
| 可配置 | `set_max_vectors()` 参数化 + 设置键 `vec.max_cached_vectors`（默认 0 = 自动预算） |
| 正确性保证 | >5000 向量的知识库，检索结果与全量扫描语义一致 |

**测试**: TC-VEC-CACHE-001~006（LRU 容量自动扩展 / 超预算检索完整性 / BinaryHeap 排序正确性 / 缓存命中零克隆 / set_max_vectors 生效 / >5000 向量完整检索）

## 3. 余弦相似度归一化预计算 (REQ-PERF-017, P0/P1)

**问题**: `cosine_similarity` 每次比较重算 query_norm × candidate_norm（3 次乘加/dim），无归一化预计算。

**实现**: `crates/infra/src/sqlite_storage.rs`

| 变更 | 说明 |
|---|---|
| 归一化点积 | 向量在入库内存缓存时归一化一次，检索时仅计算点积（3×乘加/dim → 1×乘加/dim，~3× FLOPs 降低） |
| 向后兼容 | 原始 DB 存储格式不变（归一化仅作用于内存缓存副本） |
| 零向量防御 | 归一化后全零向量得分 0.0 |
| HNSW 不受影响 | hnsw_rs 内部 DistCosine 自带归一化 |

**测试**: TC-COS-001~004（归一化点积与余弦等价 / 零向量 / 非单位向量查询等价性 / 基准护栏 10k×384 ≤ 20ms）

## 4. 检索管线并行化 (REQ-PERF-018, P1)

**问题**: 图扩展（`GraphRetriever.expand`）在主检索完成后串行追加。

**实现**: `crates/tauri-app/src/commands/chat.rs`

| 变更 | 说明 |
|---|---|
| tokio::join! 并行 | 图扩展与标准检索并行执行 |
| merge_graph_results | 纯函数合并逻辑（分数取 max，截断 top_k×2） |
| 降级语义不变 | 图扩展失败降级与串行模式一致 |
| 确定性保证 | 并行与串行结果一致（同分数同排序） |

**测试**: TC-PARALLEL-001~003（并行结果等于串行结果 / 图扩展失败降级 / 空结果不崩溃）

## 5. Contextual BM25 (REQ-PERF-019, P1)

**问题**: FTS5 索引用原始 chunk 文本，未用上下文前缀（Anthropic: 失败率 ↓17.6%）。

**实现**: `crates/infra/src/storage/vectors.rs`

| 变更 | 说明 |
|---|---|
| 上下文前缀 | `add_chunk` 时 FTS5 索引内容使用 `build_contextual_text(doc_name, chunk_content)` |
| 重建索引 | `rebuild_bm25_index` IPC 同步改为使用上下文前缀文本 |
| 向后兼容 | 新库自动生效，旧库需重建 |
| 查询语义不变 | `keyword_search` 查询语义不变（仅索引内容变化） |

**测试**: TC-CTX-BM25-001~004（FTS5 存储上下文文本 / 上下文前缀命中检索 / 重建索引包含前缀 / 无文档名时不破坏）

## 6. 语义缓存阈值动态校准 (REQ-PERF-020, P2)

**问题**: 语义缓存阈值固定 0.92，无命中率反馈校准。

**实现**: `crates/infra/src/sqlite_cache.rs` + `crates/tauri-app/src/commands/performance.rs`

| 变更 | 说明 |
|---|---|
| 命中率统计 | `SqliteCache` 增加 hit/miss 计数 + 阈值历史 |
| 自适应阈值 | 阈值在 [0.85, 0.97] 区间内按命中质量自适应 |
| 反馈信号 | 误命中（用户反馈无来源引用）上调阈值，零命中下调 |
| 默认行为不变 | 0.92 起始值，统计清零不影响缓存内容 |

**测试**: TC-CALIB-001~003（初始阈值 / 命中率统计累加 / 阈值钳位区间）

## 7. 启动与首次检索延迟 (REQ-NFR-022, P0 配套)

**实现**: `crates/infra/src/sqlite_storage.rs` + `crates/infra/src/hnsw_index.rs`

| 变更 | 说明 |
|---|---|
| 启动零阻塞 | 应用启动不触发 HNSW 构建/加载（懒加载） |
| 二次启动加速 | 10k chunks 知识库二次启动后首次检索（索引加载路径）≤ 全量构建路径的 50% |
| 无新阻塞点 | 不引入新的启动阻塞点 |

**测试**: TC-BOOT-001~002（启动无 HNSW 构建 / 二次启动首次检索延迟护栏）

---

## 归档项

| 热点 | 结论 |
|---|---|
| H8 嵌入池分片 | ✅ 实测否决 — 单会话最优 1-2s vs 分片 39-66s，线程爆炸 |
| H9 前端重资源 | ✅ 已达标 — mermaid/d3/chart/katex/hljs 全懒加载 + esbuild minify -56% |

---

## 向后兼容性

| 变更 | 兼容性 | 说明 |
|---|---|---|
| 版本号升级 | ✅ 纯版本号 | 无 API 变更 |
| HNSW 持久化 | ✅ 向后兼容 | 磁盘索引缺失时回退全量构建 |
| 向量缓存修复 | ✅ 正确性修复 | LRU 截断 bug 修复，检索结果更准确 |
| 归一化点积 | ✅ 向后兼容 | DB 存储格式不变，仅内存缓存副本归一化 |
| 检索并行化 | ✅ 向后兼容 | 并行与串行结果一致 |
| Contextual BM25 | ✅ 向后兼容 | 新库自动生效，旧库需重建 |
| 缓存阈值校准 | ✅ 向后兼容 | 默认行为不变 |

---

## 验证结果

| 检查项 | 结果 |
|---|---|
| Rust Free 测试 | **1,540 passed**, 0 failed, 32 ignored ✅ |
| Rust Pro 测试 | **1,831 passed**, 0 failed, 49 ignored ✅ |
| Clippy Free | 零警告 ✅ |
| Clippy Pro | 零警告 ✅ |
| cargo fmt | 通过 ✅ |
| TypeScript | **0 errors** ✅ |
| esbuild | 1,626 行 ✅ |
| Vitest | **1,076 passed**, 62 files, 0 failed ✅ |
| Release build | exit_code 0 ✅ |

---

## 新增测试清单 (+28)

| 测试组 | 数量 | 覆盖 |
|---|---|---|
| TC-PERSIST-001~008 | 8 | HNSW 二进制持久化 / 懒加载 / 崩溃一致性 |
| TC-VEC-CACHE-001~006 | 6 | LRU 截断修复 / BinaryHeap / Arc 零拷贝 |
| TC-COS-001~004 | 4 | 归一化点积 / 零向量 / 等价性 / 基准护栏 |
| TC-PARALLEL-001~003 | 3 | 并行结果一致性 / 降级 / 空结果 |
| TC-CALIB-001~003 | 3 | 缓存阈值动态校准 |
| TC-BOOT-001~002 | 2 | 启动零阻塞 / 二次启动延迟 |
| TC-CTX-BM25-001~004 | 4 | Contextual BM25 索引 |
| **合计** | **+28** | 零新增依赖 |

---

> **发布人**: AI Agent (CatPaw)
> **发布依据**: 全链路性能分析 → 6 个性能 REQ 实施 → +28 测试
