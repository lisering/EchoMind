# EchoMind 会话衔接提示词模板

> **用途**: 每次大功能开发跨越多个 Session 时，必须在当前 Session 结束时按此模板产出衔接提示词，确保下一 Session 无缝续接。
>
> **铁律十二配套文件**: 本文件是铁律十二（跨 Session 衔接）的标准模板。
>
> **使用方法**: 复制下方模板填入实际内容，粘贴到下一 Session 的第一条消息中。

---

## 模板

```
你正在继续 EchoMind 项目【功能名称】的开发。

## 上下文回溯

### 当前项目状态
请先阅读 `docs/sessions/CURRENT_STATE.md` 获取最新项目快照。

### 已完成的 Session
- Session N: 【一句话描述完成内容】
  - 修改文件：file1.rs / file2.rs / ...
  - 新增测试：X 个（全部通过）
  - 关键决策：【如有】
  - 归档记录：`docs/sessions/archive/YYYY-MM-DD-xxx.md`（如有）

### 当前 Session 编号
Session N+1

## 本 Session 目标
【一句话描述本 Session 要完成的目标】

## 必读文档（按优先级）
1. `docs/sessions/CURRENT_STATE.md` — 项目当前状态快照
2. `docs/SOFTWARE_ASSET.md` — 软件资产全景（架构、代码、数据流）
3. `docs/architecture/XXX_ATOMIC_PLAN.md` — 本 Session 章节（第 X-Y 行）
4. `crates/xxx/src/xxx.rs` — 已完成的参考代码
5. `.catpaw/rules/system-constraints.md` — 铁律规则
6. 【其他必读文件】

## 精确变更清单
| 文件 | 变更类型 | 内容 |
|---|---|---|
| path/to/file.rs | 新建/修改 | 描述 |
| ... | ... | ... |

## TDD 测试要求
- 先写红灯测试，再写绿灯实现
- 测试 ID 前缀：TC-XXX-
- 每个函数至少 2 个测试用例（正常 + 边界）
- 目标测试数：X 个

## 关键约束
- 【约束 1】
- 【约束 2】
- 铁律五：零 unwrap/panic/unimplemented
- 铁律七：文档实时同步
- 铁律十一：修改源代码后运行 sync-to-github.sh

## 验证命令
```bash
cargo test -p xxx -- yyy
cargo clippy -p xxx -- -D warnings
cargo fmt --check
```

## 完成后必须执行
1. 运行验证命令确认通过
2. 更新 `docs/sessions/CURRENT_STATE.md`（测试数、功能状态、待办列表）
3. 追加 `docs/sessions/LOG.md`（本次 session 摘要）
4. 如有大功能，创建 `docs/sessions/archive/` 归档记录
5. 按本模板产出下一 Session 衔接提示词
6. 运行 `./sync-to-github.sh` 同步到 GitHub 发布仓库
7. 如有源代码变更，同步更新 `docs/SOFTWARE_ASSET.md` 对应章节
8. 如有新增/修改 IPC 命令，更新 `docs/api/ipc-commands.md`
9. 如有新增/修改数据库表，更新 `docs/api/database-schema.md`
10. 如有新增/修改 trait/模型/AppState，更新 `docs/api/code-contracts.md`
```

---

## 使用示例

以下为「极致性能优化」项目中 Session 9 → Session 10 的实际衔接提示词：

```
你正在继续 EchoMind 极致性能优化 S10 的开发。

## 上下文回溯

### 当前项目状态
请先阅读 `docs/sessions/CURRENT_STATE.md` 获取最新项目快照。

### 已完成的 Session
- Session 1: 语义缓存金字塔完成
  - 新增 SqliteCache (L0/L1/L3 三级缓存) + 12 个测试
  - 4 个 IPC 命令 + 4 个设置键
- Session 2: Prompt 压缩完成
  - 新增 RuleBasedCompressor + OnnxCompressor + 11 个测试
  - 2 个 IPC 命令 + compression.ratio 设置键
- Session 3: CRAG + Self-RAG 完成
- Session 4: Proposition 级原子分割完成
- Session 5: ColBERT 多向量嵌入完成
- Session 6: RAPTOR 摘要树完成
- Session 7: 渐进式上下文注入完成
  - 新增 ProgressiveInjector + 14 个测试
  - rag.progressive_injection 设置键
- Session 8: Speculative RAG 完成
  - 新增 SpeculativeRagEngine<D,L> + 12 个测试
  - rag.speculative_enabled 设置键
- Session 9: 自进化检索记忆完成
  - 新增 RetrievalMemoryEngine + 10 个测试
  - 3 个 IPC 命令 + rag.retrieval_memory_enabled 设置键

### 当前 Session 编号
Session 10

## 本 Session 目标
文档同步 + 性能基准测试（TC-BENCH-001~005）

## 必读文档
1. `docs/sessions/CURRENT_STATE.md` — 项目当前状态
2. `docs/SOFTWARE_ASSET.md` — 软件资产全景
3. `docs/architecture/EXTREME_PERFORMANCE_ATOMIC_PLAN.md` — S10 章节
4. `.catpaw/rules/system-constraints.md` — 铁律规则

## 精确变更清单
| 文件 | 变更类型 | 内容 |
|---|---|---|
| `crates/core/src/benchmark_tests.rs` | 新建 | 5 个性能基准测试 |
| `crates/core/src/lib.rs` | 修改 | 注册 benchmark_tests 模块 |
| `docs/SRS_v1.0.md` | 修改 | 新增 §4.26 PERF 域 |
| `docs/api/ipc-commands.md` | 修改 | 命令数更新 |
| `docs/api/settings-reference.md` | 修改 | 新增设置键 |
| `docs/api/code-contracts.md` | 修改 | 新增性能优化契约 |
| `docs/api/frontend-modules.md` | 修改 | 新增前端函数 |
| `docs/architecture/adr/ADR-012-extreme-perf.md` | 新建 | ADR |
| `docs/architecture/EXTREME_PERFORMANCE_ATOMIC_PLAN.md` | 修改 | S10 完成标志 |
| `AGENTS.md` | 修改 | 命令数 + key files |
| `CLAUDE.md` | 修改 | key files |

## 验证命令
```bash
cargo test --features pro
cargo clippy --all-targets --features pro -- -D warnings
cargo fmt --check
./sync-to-github.sh
```

## 完成后必须执行
1. 运行验证命令确认通过
2. 更新 docs/sessions/CURRENT_STATE.md
3. 追加 docs/sessions/LOG.md
4. 创建 docs/sessions/archive/2026-08-04-extreme-perf-s10.md
5. 运行 ./sync-to-github.sh
```
