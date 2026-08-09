# EchoMind Session 衔接提示词模板

> **用途**：每次大功能开发跨越多个 Session 时，必须在当前 Session 结束时按此模板产出衔接提示词，确保下一 Session 无缝续接。
>
> **铁律**：本文件是铁律十二的配套模板，使用时复制下方模板填入实际内容。

---

## 模板

```
你正在继续 EchoMind 项目【功能名称】的开发。

## 上下文回溯

### 已完成的 Session
- Session N: 【一句话描述完成内容】
  - 修改文件：file1.rs / file2.rs / ...
  - 新增测试：X 个（全部通过）
  - 关键决策：【如有】

### 当前 Session 编号
Session N+1

## 本 Session 目标
【一句话描述本 Session 要完成的目标】

## 必读文档（按优先级）
1. `docs/architecture/XXX_ATOMIC_PLAN.md` — 本 Session 章节（第 X-Y 行）
2. `crates/xxx/src/xxx.rs` — 已完成的参考代码
3. `.catpaw/rules/system-constraints.md` — 铁律规则
4. 【其他必读文件】

## 精确变更清单
| 文件 | 变更类型 | 内容 |
|---|---|---|
| path/to/file.rs | 新建/修改 | 描述 |
| ... | ... | ... |

## TDD 测试要求
- 先写红灯测试，再写绿灯实现
- 测试 ID 前缀：TC-XXX-
- 每个函数至少 2 个测试用例（正常 + 边界）

## 关键约束
- 【约束 1】
- 【约束 2】

## 验证命令
```bash
cargo test -p xxx -- yyy
cargo clippy -p xxx -- -D warnings
```

## 完成后必须执行
1. 运行验证命令确认通过
2. 按本模板产出下一 Session 衔接提示词
3. 运行 `./sync-to-github.sh` 同步到 GitHub 发布仓库
4. 如有源代码变更，确认 clippy 零警告
```

---

## 使用示例

以下为「纯 Rust 本地 LLM 引擎」项目中 Session 2 → Session 3 的实际衔接提示词：

```
你正在继续 EchoMind 纯 Rust 本地 LLM 引擎的开发。

## 上下文回溯

### 已完成的 Session
- Session 1: 数据模型层（crates/models）定义完成
  - 修改文件：crates/models/src/lib.rs
  - 新增：LlmMode 枚举、ModelInfo 结构体、ModelDownloadProgressPayload、ModelLoadStatusPayload
  - SettingsPayload 新增 llm_mode / local_model 字段
  - 测试：4 个（TC-MODEL-LLM-001~004，全部通过）

- Session 2: ModelManager 模型文件管理完成
  - 新建文件：crates/infra/src/model_manager.rs
  - 新建文件：crates/infra/src/model_manager_tests.rs
  - 修改文件：crates/infra/src/lib.rs（注册模块）
  - 实现：list_models / delete_model / model_path / download_model + RECOMMENDED_MODELS
  - 测试：20 个（TC-MGR-001~020，全部通过）
  - clippy 零警告

### 当前 Session 编号
Session 3

## 本 Session 目标
实现 LocalLlmEngine 适配器，基于 mistral.rs Rust SDK 实现 LLMProvider trait。

## 必读文档
1. docs/architecture/LOCAL_LLM_ENGINE_ATOMIC_PLAN.md — Session 3 章节
2. crates/core/src/lib.rs — LLMProvider trait 定义（第 293-301 行）
3. crates/infra/src/openai_provider.rs — 参考实现
4. crates/infra/src/model_manager.rs — 已完成的模型管理器

## 精确变更清单
| 文件 | 变更类型 | 内容 |
|---|---|---|
| crates/infra/Cargo.toml | 修改 | 新增 mistralrs Pro 可选依赖 |
| crates/infra/src/lib.rs | 修改 | 新增 #[cfg(feature="pro")] pub mod local_llm |
| crates/infra/src/local_llm.rs | 新建 | LocalLlmEngine + Quantization + LLMProvider impl |
| crates/infra/src/local_llm_tests.rs | 新建 | 8 个测试（TC-LLM-LOCAL-001~008） |

## TDD 测试要求
- TC-LLM-LOCAL-001: test_quantization_from_str
- TC-LLM-LOCAL-002: test_quantization_as_str
- TC-LLM-LOCAL-003: test_new_nonexistent_file_fails
- TC-LLM-LOCAL-004: test_new_valid_path_succeeds
- TC-LLM-LOCAL-005: test_is_loaded_false_before_init
- TC-LLM-LOCAL-006: test_build_request_system_prompt
- TC-LLM-LOCAL-007: test_build_request_history
- TC-LLM-LOCAL-008: test_build_request_query

## 关键约束
- mistral.rs 依赖通过 Pro feature 门控
- 模型加载在 spawn_blocking 中执行
- chat_stream 返回 BoxStream<Result<String>>
- 铁律五：零 unwrap/panic/unimplemented
- 先查询 mistral.rs 最新版本和 API 文档（铁律六/八）

## 验证命令
cargo test -p echomind-infra --features pro -- local_llm_tests
cargo clippy -p echomind-infra --features pro -- -D warnings

## 完成后必须执行
1. 运行验证命令确认通过
2. 按本模板产出 Session 3 → Session 4 衔接提示词
3. 运行 ./sync-to-github.sh 同步
```
