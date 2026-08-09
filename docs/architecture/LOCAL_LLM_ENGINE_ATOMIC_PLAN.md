# EchoMind 纯 Rust 本地 LLM 引擎 — 原子级实施规划

> **状态**：Active ｜ **创建日期**：2026-07-30 ｜ **父文档**：`LOCAL_LLM_ENGINE_PLAN.md`
>
> 本文档将 Phase 1（MVP）拆分为 **7 个 Session**，每个 Session 包含：
> - 精确到文件、函数、行号的变更清单
> - TDD 测试用例（先红灯后绿灯）
> - Session 完成验证命令
> - 下一 Session 衔接提示词
>
> **铁律**：每个 Session 结束时必须产出衔接提示词，确保下一 Session 无缝续接。

---

## 全局依赖关系图

```
Session 1 (Models) ──→ Session 2 (ModelManager) ──→ Session 3 (LocalLlmEngine)
                                                        │
                                                        ↓
                  Session 4 (AppState) ←────────────────┘
                        │
                        ↓
                  Session 5 (IPC Commands)
                        │
                        ↓
                  Session 6 (Frontend)
                        │
                        ↓
                  Session 7 (Docs & Sync)
```

---

## Session 1：数据模型层

**目标**：在 `crates/models` 中定义本地 LLM 相关的数据结构。

**预计耗时**：0.5 天

### 1.1 文件变更清单

#### 文件 1：`crates/models/src/lib.rs`

**变更类型**：修改（新增结构体）

**新增内容**（在 `SettingsPayload` 结构体之后追加）：

```rust
/// LLM 推理模式（REQ-LLM-003）。
///
/// - `Remote`：BYOK 远程 API（现有行为，OpenAI 兼容 SSE 流式）
/// - `Local`：本地推理（mistral.rs，GGUF 模型，零网络请求）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LlmMode {
    /// 远程 API（BYOK 模式）
    Remote,
    /// 本地推理（mistral.rs）
    Local,
}

impl Default for LlmMode {
    fn default() -> Self {
        LlmMode::Remote
    }
}

/// 本地模型元信息（REQ-LLM-004）。
///
/// 由 `ModelManager::list_models()` 返回，前端用于渲染已下载模型列表。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// 文件名（如 `qwen2.5-7b-instruct-q4_k_m.gguf`）
    pub filename: String,
    /// 完整路径
    pub path: String,
    /// 文件大小（字节）
    pub size_bytes: u64,
    /// 推断的模型架构（从文件名解析，如 `qwen2.5` / `llama3.2` / `phi3.5`）
    pub architecture: String,
    /// 推断的参数规模（如 `7B` / `3B` / `3.8B`）
    pub param_size: String,
    /// 量化格式（如 `Q4_K_M` / `Q5_K_M` / `Q8_0`）
    pub quantization: String,
}

/// 模型下载进度事件载荷（REQ-LLM-004 AC-2）。
///
/// 通过 `model_download_progress` Tauri 事件推送到前端。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDownloadProgressPayload {
    /// 正在下载的模型文件名
    pub filename: String,
    /// 已下载字节数
    pub downloaded: u64,
    /// 文件总大小（字节）；未知时为 0
    pub total: u64,
    /// 下载速度（字节/秒）
    pub speed: u64,
}

/// 模型加载状态事件载荷（REQ-LLM-003 AC-4）。
///
/// 通过 `model_load_progress` Tauri 事件推送到前端。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelLoadStatusPayload {
    /// 正在加载的模型文件名
    pub model: String,
    /// 加载状态：`loading` / `ready` / `error`
    pub status: String,
    /// 错误信息（仅 `status == "error"` 时有值）
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub error: Option<String>,
}
```

**修改 `SettingsPayload`**（新增两个字段）：

```rust
// 在 context_token_limit 字段之后追加：
    /// LLM 推理模式（REQ-LLM-003；默认 "remote"，可选 "local"）
    #[serde(default)]
    pub llm_mode: String,
    /// 当前选中的本地模型文件名（REQ-LLM-003；空串表示未选择）
    #[serde(default)]
    pub local_model: String,
```

### 1.2 TDD 测试用例

#### 文件：`crates/models/src/lib.rs`（底部 `#[cfg(test)]` 模块）

```rust
#[cfg(test)]
mod llm_model_tests {
    use super::*;

    /// TC-MODEL-LLM-001：LlmMode 默认值为 Remote
    #[test]
    fn test_llm_mode_default() {
        assert_eq!(LlmMode::default(), LlmMode::Remote);
    }

    /// TC-MODEL-LLM-002：LlmMode 序列化/反序列化往返一致
    #[test]
    fn test_llm_mode_serde() {
        let mode = LlmMode::Local;
        let json = serde_json::to_string(&mode).expect("serialize");
        let deserialized: LlmMode = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(mode, deserialized);
    }

    /// TC-MODEL-LLM-003：ModelInfo 序列化包含所有字段
    #[test]
    fn test_model_info_serialize() {
        let info = ModelInfo {
            filename: "qwen2.5-7b-instruct-q4_k_m.gguf".to_string(),
            path: "/data/models/llm/qwen2.5-7b-instruct-q4_k_m.gguf".to_string(),
            size_bytes: 4400000000,
            architecture: "qwen2.5".to_string(),
            param_size: "7B".to_string(),
            quantization: "Q4_K_M".to_string(),
        };
        let json = serde_json::to_string(&info).expect("serialize");
        assert!(json.contains("qwen2.5-7b-instruct-q4_k_m.gguf"));
        assert!(json.contains("Q4_K_M"));
    }

    /// TC-MODEL-LLM-004：SettingsPayload 新增字段默认值正确
    #[test]
    fn test_settings_payload_llm_mode_default() {
        let json = r#"{
            "has_llm_config": false,
            "base_url": "",
            "model": "",
            "api_key_masked": "",
            "vlm_enabled": false,
            "hybrid_search": true,
            "rerank_enabled": false,
            "hyde_enabled": false,
            "agent_enabled": false,
            "embedding_model": "all-MiniLM-L6-v2",
            "context_token_limit": 4096
        }"#;
        let payload: SettingsPayload = serde_json::from_str(json).expect("deserialize");
        assert_eq!(payload.llm_mode, "");
        assert_eq!(payload.local_model, "");
    }
}
```

### 1.3 验证命令

```bash
cargo test -p echomind-models -- llm_model_tests
cargo clippy -p echomind-models -- -D warnings
cargo fmt --check -p echomind-models
```

### 1.4 Session 1 完成标志

- [ ] `LlmMode` / `ModelInfo` / `ModelDownloadProgressPayload` / `ModelLoadStatusPayload` 定义完成
- [ ] `SettingsPayload` 新增 `llm_mode` / `local_model` 字段
- [ ] 4 个测试全部通过
- [ ] clippy 零警告

### 1.5 Session 1 → Session 2 衔接提示词

```
你正在继续 EchoMind 纯 Rust 本地 LLM 引擎的开发。

## 上下文
- 上一 Session 已完成数据模型层（crates/models）：
  - 新增 `LlmMode` 枚举（Remote/Local）
  - 新增 `ModelInfo` 结构体（filename/path/size_bytes/architecture/param_size/quantization）
  - 新增 `ModelDownloadProgressPayload` 和 `ModelLoadStatusPayload`
  - `SettingsPayload` 新增 `llm_mode` 和 `local_model` 字段
  - 全部测试通过，clippy 零警告

## 本 Session 目标
实现 `ModelManager`——本地 GGUF 模型文件管理器（列表/删除/路径/下载）。

## 必读文档
1. `docs/architecture/LOCAL_LLM_ENGINE_ATOMIC_PLAN.md` — 本规划文档（Session 2 章节）
2. `docs/architecture/LOCAL_LLM_ENGINE_PLAN.md` — 总体规划
3. `.catpaw/rules/system-constraints.md` — 铁律规则
4. `crates/models/src/lib.rs` — 已完成的数据模型

## TDD 纪律
先写测试（红灯），再写实现（绿灯）。每个函数至少 2 个测试用例。

## 完成后
1. 运行验证命令确认通过
2. 产出下一 Session 衔接提示词
3. 运行 `./sync-to-github.sh` 同步
```

---

## Session 2：ModelManager — 模型文件管理

**目标**：实现 `crates/infra/src/model_manager.rs`，提供本地 GGUF 模型的列表、删除、路径获取、下载功能。

**预计耗时**：2 天

### 2.1 文件变更清单

#### 文件 1：`crates/infra/src/lib.rs`（修改）

**变更**：新增模块声明

```rust
/// 本地 LLM 模型文件管理器（REQ-LLM-004）：下载、删除、列表本地 GGUF 模型。
pub mod model_manager;
```

#### 文件 2：`crates/infra/src/model_manager.rs`（新建）

**完整函数签名清单**：

| 函数 | 签名 | 职责 |
|---|---|---|
| `ModelManager::new` | `pub fn new(data_dir: &Path) -> anyhow::Result<Self>` | 创建管理器，确保 `models/llm/` 目录存在 |
| `ModelManager::models_dir` | `pub fn models_dir(&self) -> &Path` | 返回模型存储目录路径 |
| `ModelManager::list_models` | `pub fn list_models(&self) -> anyhow::Result<Vec<ModelInfo>>` | 扫描目录，列出所有 `.gguf` 文件 |
| `ModelManager::delete_model` | `pub fn delete_model(&self, filename: &str) -> anyhow::Result<()>` | 删除指定模型文件（含路径穿越防护） |
| `ModelManager::model_path` | `pub fn model_path(&self, filename: &str) -> anyhow::Result<PathBuf>` | 获取模型文件完整路径（含路径穿越防护） |
| `ModelManager::download_model` | `pub async fn download_model(&self, url: &str, filename: &str, progress: DownloadProgressFn) -> anyhow::Result<()>` | 从 URL 下载 GGUF 文件，支持进度回调 |
| `ModelManager::parse_model_info` | `fn parse_model_info(path: &Path) -> Option<ModelInfo>` | 从文件路径解析模型元信息 |
| `ModelManager::sanitize_filename` | `fn sanitize_filename(filename: &str) -> anyhow::Result<String>` | 安全校验文件名（防目录穿越） |
| `ModelManager::parse_architecture` | `fn parse_architecture(filename: &str) -> String` | 从文件名推断模型架构 |
| `ModelManager::parse_param_size` | `fn parse_param_size(filename: &str) -> String` | 从文件名推断参数规模 |
| `ModelManager::parse_quantization` | `fn parse_quantization(filename: &str) -> String` | 从文件名推断量化格式 |

**类型定义**：

```rust
/// 下载进度回调类型。
/// 参数：(已下载字节, 总字节, 速度字节/秒)
pub type DownloadProgressFn = Box<dyn Fn(u64, u64, u64) + Send + Sync>;
```

**推荐模型预设**（内置常量）：

```rust
/// 推荐模型列表（REQ-LLM-004 AC-6）。
///
/// 用户可从设置面板一键下载这些模型。
/// 来源：HuggingFace GGUF 仓库（通过代理下载）。
pub const RECOMMENDED_MODELS: &[RecommendedModel] = &[
    RecommendedModel {
        name: "Qwen2.5-3B-Instruct",
        architecture: "qwen2.5",
        param_size: "3B",
        quantization: "Q4_K_M",
        size_gb: 2.0,
        url: "https://huggingface.co/Qwen/Qwen2.5-3B-Instruct-GGUF/resolve/main/qwen2.5-3b-instruct-q4_k_m.gguf",
        description: "低配设备推荐，中文能力优秀",
    },
    RecommendedModel {
        name: "Llama-3.2-3B-Instruct",
        architecture: "llama3.2",
        param_size: "3B",
        quantization: "Q4_K_M",
        size_gb: 2.0,
        url: "https://huggingface.co/meta-llama/Llama-3.2-3B-Instruct-GGUF/resolve/main/llama-3.2-3b-instruct-q4_k_m.gguf",
        description: "英文场景推荐",
    },
    RecommendedModel {
        name: "Phi-3.5-mini-instruct",
        architecture: "phi3.5",
        param_size: "3.8B",
        quantization: "Q4_K_M",
        size_gb: 2.2,
        url: "https://huggingface.co/microsoft/Phi-3.5-mini-instruct-gguf/resolve/main/Phi-3.5-mini-instruct-q4_k_m.gguf",
        description: "推理能力强",
    },
    RecommendedModel {
        name: "Qwen2.5-7B-Instruct",
        architecture: "qwen2.5",
        param_size: "7B",
        quantization: "Q4_K_M",
        size_gb: 4.1,
        url: "https://huggingface.co/Qwen/Qwen2.5-7B-Instruct-GGUF/resolve/main/qwen2.5-7b-instruct-q4_k_m.gguf",
        description: "平衡质量与速度",
    },
];

/// 推荐模型信息。
#[derive(Debug, Clone, Serialize)]
pub struct RecommendedModel {
    pub name: &'static str,
    pub architecture: &'static str,
    pub param_size: &'static str,
    pub quantization: &'static str,
    pub size_gb: f64,
    pub url: &'static str,
    pub description: &'static str,
}
```

### 2.2 TDD 测试用例

#### 文件：`crates/infra/src/model_manager_tests.rs`（新建）

| 测试 ID | 测试名 | 验证内容 | 对应 AC |
|---|---|---|---|
| TC-MGR-001 | `test_new_creates_models_dir` | `new()` 创建 `models/llm/` 目录 | — |
| TC-MGR-002 | `test_list_models_empty_dir` | 空目录返回空 Vec | AC-1 |
| TC-MGR-003 | `test_list_models_finds_gguf` | 目录中有 `.gguf` 文件时正确列出 | AC-1 |
| TC-MGR-004 | `test_list_models_ignores_non_gguf` | 非 `.gguf` 文件被忽略 | AC-1 |
| TC-MGR-005 | `test_list_models_parses_metadata` | 正确解析文件名中的架构/参数量/量化格式 | AC-1 |
| TC-MGR-006 | `test_delete_model_removes_file` | 删除已存在的模型文件 | AC-3 |
| TC-MGR-007 | `test_delete_model_nonexistent_fails` | 删除不存在的文件返回 Err | AC-3 |
| TC-MGR-008 | `test_delete_model_path_traversal_blocked` | `../../etc/passwd` 被拒绝 | AC-3 |
| TC-MGR-009 | `test_model_path_returns_correct_path` | 返回正确的完整路径 | — |
| TC-MGR-010 | `test_model_path_traversal_blocked` | 路径穿越被拒绝 | — |
| TC-MGR-011 | `test_sanitize_filename_rejects_slash` | 含 `/` 的文件名被拒绝 | — |
| TC-MGR-012 | `test_sanitize_filename_rejects_dotdot` | 含 `..` 的文件名被拒绝 | — |
| TC-MGR-013 | `test_sanitize_filename_accepts_valid` | 正常文件名通过 | — |
| TC-MGR-014 | `test_parse_architecture_qwen` | `qwen2.5-7b-...` → `qwen2.5` | — |
| TC-MGR-015 | `test_parse_architecture_llama` | `llama-3.2-3b-...` → `llama3.2` | — |
| TC-MGR-016 | `test_parse_param_size` | `...7b...` → `7B` | — |
| TC-MGR-017 | `test_parse_quantization_q4km` | `...q4_k_m...` → `Q4_K_M` | — |
| TC-MGR-018 | `test_parse_quantization_q8` | `...q8_0...` → `Q8_0` | — |
| TC-MGR-019 | `test_recommended_models_not_empty` | `RECOMMENDED_MODELS` 非空 | AC-6 |
| TC-MGR-020 | `test_recommended_models_urls_valid` | 所有 URL 以 `https://` 开头 | AC-6 |

**注册测试模块**（`crates/infra/src/lib.rs`）：

```rust
#[cfg(test)]
mod model_manager_tests;
```

### 2.3 安全要点

1. **路径穿越防护**：`sanitize_filename()` 必须拒绝包含 `/`、`\`、`..` 的文件名
2. **下载 URL 校验**：仅允许 `https://` 前缀的 URL
3. **文件大小校验**：下载完成后校验文件大小是否与 Content-Length 匹配
4. **临时文件**：下载时先写入 `.tmp` 文件，完成后原子重命名

### 2.4 验证命令

```bash
cargo test -p echomind-infra -- model_manager_tests
cargo clippy -p echomind-infra -- -D warnings
cargo fmt --check -p echomind-infra
```

### 2.5 Session 2 → Session 3 衔接提示词

```
你正在继续 EchoMind 纯 Rust 本地 LLM 引擎的开发。

## 上下文
- Session 1 完成：数据模型层（crates/models）已定义 LlmMode/ModelInfo/ModelDownloadProgressPayload/ModelLoadStatusPayload，SettingsPayload 新增 llm_mode/local_model 字段
- Session 2 完成：ModelManager（crates/infra/src/model_manager.rs）已实现：
  - list_models() — 扫描 .gguf 文件，解析架构/参数量/量化格式
  - delete_model() — 安全删除（路径穿越防护）
  - model_path() — 获取模型完整路径
  - download_model() — 从 HuggingFace 下载，进度回调
  - RECOMMENDED_MODELS 常量 — 4 个推荐模型
  - 20 个测试全部通过，clippy 零警告

## 本 Session 目标
实现 `LocalLlmEngine`——基于 mistral.rs 的本地 LLM 推理适配器，实现 `LLMProvider` trait。

## 必读文档
1. `docs/architecture/LOCAL_LLM_ENGINE_ATOMIC_PLAN.md` — Session 3 章节
2. `crates/core/src/lib.rs` — LLMProvider trait 定义（第 293-301 行）
3. `crates/infra/src/openai_provider.rs` — 参考实现（OpenAIProvider）
4. `crates/infra/src/model_manager.rs` — 已完成的模型管理器
5. mistral.rs 官方文档：https://ericlbuehler.github.io/mistral.rs/

## 关键约束
- mistral.rs 依赖通过 Pro feature 门控：`#[cfg(feature = "pro")]`
- 模型加载在 spawn_blocking 中执行，不阻塞 async runtime
- chat_stream 返回 BoxStream<Result<String>>，与 OpenAIProvider 完全一致
- 铁律五：零 unwrap/panic/unimplemented

## TDD 纪律
先写测试（红灯），再写实现（绿灯）。

## 完成后
1. 运行验证命令确认通过
2. 产出下一 Session 衔接提示词
3. 运行 `./sync-to-github.sh` 同步
```

---

## Session 3：LocalLlmEngine — 本地推理适配器

**目标**：实现 `crates/infra/src/local_llm.rs`，基于 mistral.rs Rust SDK 实现 `LLMProvider` trait。

**预计耗时**：3 天

### 3.1 前置调研（Session 开始时必须执行）

```bash
# 1. 查询 mistral.rs 最新版本和 API
export https_proxy=http://127.0.0.1:7890 http_proxy=http://127.0.0.1:7890 all_proxy=socks5://127.0.0.1:7890
curl -sL "https://crates.io/api/v1/crates/mistralrs" | python3 -m json.tool | head -30

# 2. 获取 mistral.rs Rust SDK 文档
curl -sL "https://raw.githubusercontent.com/EricLBuehler/mistral.rs/master/docs/quickstart/rust.md"

# 3. 查看 mistral.rs SDK 示例代码
curl -sL "https://api.github.com/repos/EricLBuehler/mistral.rs/contents/mistralrs/examples"

unset https_proxy http_proxy all_proxy
```

### 3.2 文件变更清单

#### 文件 1：`crates/infra/Cargo.toml`（修改）

**变更**：新增 mistralrs 可选依赖

```toml
[features]
pro = [
    "dep:pdfium-render", "dep:ocrs", "dep:rten", "dep:image", "dep:hnsw_rs",
    "dep:mistralrs",  # 新增
]

# 本地 LLM 推理引擎（REQ-LLM-003）
mistralrs = { version = "<查询到的最新版本>", optional = true }
```

#### 文件 2：`crates/infra/src/lib.rs`（修改）

**变更**：新增模块声明（Pro 门控）

```rust
/// 本地 LLM 推理引擎（REQ-LLM-003）：基于 mistral.rs 的纯 Rust 本地推理。
#[cfg(feature = "pro")]
pub mod local_llm;
```

#### 文件 3：`crates/infra/src/local_llm.rs`（新建）

**完整函数签名清单**：

| 函数 | 签名 | 职责 |
|---|---|---|
| `Quantization::from_str` | `pub fn from_str(s: &str) -> Self` | 从字符串解析量化格式 |
| `Quantization::as_str` | `pub fn as_str(&self) -> &'static str` | 转为字符串表示 |
| `LocalLlmEngine::new` | `pub fn new(model_path: PathBuf, quantization: Quantization) -> anyhow::Result<Self>` | 创建引擎实例 |
| `LocalLlmEngine::runner` | `async fn runner(&self) -> anyhow::Result<&MistralRS>` | 懒加载模型（spawn_blocking） |
| `LocalLlmEngine::is_loaded` | `pub fn is_loaded(&self) -> bool` | 检查模型是否已加载 |
| `LocalLlmEngine::unload` | `pub fn unload(&self)` | 卸载模型（释放内存） |
| `LLMProvider::chat_stream` | `async fn chat_stream(&self, system_prompt: &str, history: &[ChatMessage], query: &str) -> Result<BoxStream<Result<String>>>` | 流式推理（trait 实现） |
| `build_mistralrs_request` | `fn build_request(system_prompt: &str, history: &[ChatMessage], query: &str) -> Request` | 构建 mistral.rs 请求 |
| `stream_to_boxstream` | `fn stream_to_boxstream(stream: impl Stream<Item = Result<String>>) -> BoxStream<Result<String>>` | 适配流类型 |

**结构体设计**：

```rust
/// 量化格式枚举。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quantization {
    Q4K_M,
    Q5K_M,
    Q8_0,
    Auto,
}

/// 本地 LLM 推理引擎。
pub struct LocalLlmEngine {
    model_path: PathBuf,
    quantization: Quantization,
    runner: tokio::sync::OnceCell<MistralRS>,
}
```

### 3.3 TDD 测试用例

#### 文件：`crates/infra/src/local_llm_tests.rs`（新建，Pro 门控）

| 测试 ID | 测试名 | 验证内容 | 对应 AC |
|---|---|---|---|
| TC-LLM-LOCAL-001 | `test_quantization_from_str` | 字符串解析量化格式 | — |
| TC-LLM-LOCAL-002 | `test_quantization_as_str` | 量化格式转字符串 | — |
| TC-LLM-LOCAL-003 | `test_new_nonexistent_file_fails` | 不存在的模型文件返回 Err | AC-7 |
| TC-LLM-LOCAL-004 | `test_new_valid_path_succeeds` | 有效路径创建成功 | — |
| TC-LLM-LOCAL-005 | `test_is_loaded_false_before_init` | 初始状态未加载 | — |
| TC-LLM-LOCAL-006 | `test_build_request_system_prompt` | 请求包含 system prompt | — |
| TC-LLM-LOCAL-007 | `test_build_request_history` | 请求包含历史消息 | — |
| TC-LLM-LOCAL-008 | `test_build_request_query` | 请求包含用户查询 | — |

**注意**：真实模型推理测试（TC-LLM-002/003/004）需要下载 GGUF 模型，标记为 `#[ignore]`，手动运行：

```bash
# 下载测试模型（约 2GB）
export https_proxy=http://127.0.0.1:7890
curl -L -o /tmp/test-model.gguf "https://huggingface.co/Qwen/Qwen2.5-3B-Instruct-GGUF/resolve/main/qwen2.5-3b-instruct-q4_k_m.gguf"

# 运行推理测试
ECHOMIND_TEST_MODEL=/tmp/test-model.gguf cargo test -p echomind-infra --features pro -- local_llm_tests --ignored
```

### 3.4 关键实现细节

1. **模型加载**：
   - 使用 `tokio::task::spawn_blocking` 包装 mistral.rs 初始化
   - `OnceCell` 确保只加载一次
   - 加载完成后 emit `model_load_progress` 事件

2. **流式适配**：
   - mistral.rs 返回 `impl Stream<Item = Result<String>>`
   - 需适配为 `BoxStream<'static, Result<String>>`
   - 使用 `futures::stream::BoxStream` + `StreamExt::boxed()`

3. **请求构建**：
   - system_prompt → `Request::new_message("system", system_prompt)`
   - history → 逐条转换为 `Request::new_message(role, content)`
   - query → `Request::new_message("user", query)`

### 3.5 验证命令

```bash
# Free 模式编译检查
cargo build -p echomind-infra

# Pro 模式编译检查（需要下载 mistral.rs 依赖）
cargo build -p echomind-infra --features pro

# 单元测试（不含真实模型推理）
cargo test -p echomind-infra --features pro -- local_llm_tests

# Clippy
cargo clippy -p echomind-infra --features pro -- -D warnings
```

### 3.6 Session 3 → Session 4 衔接提示词

```
你正在继续 EchoMind 纯 Rust 本地 LLM 引擎的开发。

## 上下文
- Session 1 完成：数据模型层定义
- Session 2 完成：ModelManager 模型文件管理
- Session 3 完成：LocalLlmEngine 适配器
  - crates/infra/src/local_llm.rs 已实现 LLMProvider trait
  - crates/infra/Cargo.toml 已添加 mistralrs Pro 依赖
  - 8 个单元测试通过（真实推理测试标记 #[ignore]）
  - clippy 零警告

## 本 Session 目标
修改 AppState，集成 LlmMode 切换和 LocalLlmEngine 懒加载，修改 chat_inner 支持本地/远程 provider 选择。

## 必读文档
1. `docs/architecture/LOCAL_LLM_ENGINE_ATOMIC_PLAN.md` — Session 4 章节
2. `crates/tauri-app/src/state.rs` — AppState 当前结构
3. `crates/tauri-app/src/commands.rs` — chat_inner 函数（第 552-784 行）
4. `crates/infra/src/local_llm.rs` — 已完成的 LocalLlmEngine

## 关键修改点
1. state.rs: 新增 llm_mode / local_llm / model_manager 字段
2. state.rs: 新增 local_llm() / set_llm_mode() / get_llm_mode() 方法
3. commands.rs: chat_inner() 中根据 llm_mode 选择 provider
4. commands.rs: get_settings_inner() 返回 llm_mode / local_model

## TDD 纪律
先写测试（红灯），再写实现（绿灯）。

## 完成后
1. 运行验证命令确认通过
2. 产出下一 Session 衔接提示词
3. 运行 `./sync-to-github.sh` 同步
```

---

## Session 4：AppState 集成 ✅ 已完成

**目标**：修改 `AppState` 集成本地 LLM 引擎，修改 `chat_inner` 支持 provider 切换。

**预计耗时**：1.5 天

### 4.1 文件变更清单

#### 文件 1：`crates/tauri-app/src/state.rs`（修改）

**新增字段**：

```rust
pub struct AppState {
    // ... 现有字段保持不变 ...

    /// LLM 推理模式（REQ-LLM-003；Remote=BYOK API，Local=mistral.rs 本地推理）
    llm_mode: tokio::sync::RwLock<LlmMode>,
    /// 本地 LLM 引擎（Pro 门控，懒加载）
    #[cfg(feature = "pro")]
    local_llm: tokio::sync::RwLock<Option<LocalLlmEngine>>,
    /// 模型管理器
    model_manager: ModelManager,
}
```

**新增方法**：

| 方法 | 签名 | 职责 |
|---|---|---|
| `AppState::load_llm_mode` | `async fn load_llm_mode(storage: &SqliteStorage) -> LlmMode` | 从 settings 表恢复模式 |
| `AppState::get_llm_mode` | `pub async fn get_llm_mode(&self) -> LlmMode` | 获取当前模式 |
| `AppState::set_llm_mode` | `pub async fn set_llm_mode(&self, mode: LlmMode) -> anyhow::Result<()>` | 切换模式 + 持久化 |
| `AppState::local_llm` | `pub async fn local_llm(&self) -> anyhow::Result<&LocalLlmEngine>` | 懒加载本地引擎（Pro） |
| `AppState::unload_local_llm` | `pub async fn unload_local_llm(&self)` | 卸载本地引擎 |
| `AppState::model_manager` | `pub fn model_manager(&self) -> &ModelManager` | 获取模型管理器 |

**`AppState::new()` 修改**：

```rust
// 在现有初始化逻辑中追加：
let llm_mode = Self::load_llm_mode(&storage).await;
let model_manager = ModelManager::new(&data_dir)?;

Ok(Self {
    // ... 现有字段 ...
    llm_mode: tokio::sync::RwLock::new(llm_mode),
    #[cfg(feature = "pro")]
    local_llm: tokio::sync::RwLock::new(None),
    model_manager,
})
```

#### 文件 2：`crates/tauri-app/src/commands.rs`（修改）

**修改 `chat_inner`** — 在创建 provider 处分支：

```rust
// 现有代码（第 642-647 行）：
// let provider = OpenAIProvider::new(
//     llm_config.api_key.clone(),
//     llm_config.base_url.clone(),
//     llm_config.model.clone(),
// )
// .map_err(|e| format!("{e:#}"))?;

// 替换为：
let llm_mode = state.get_llm_mode().await;
let provider: Box<dyn LLMProvider> = match llm_mode {
    LlmMode::Remote => {
        let p = OpenAIProvider::new(
            llm_config.api_key.clone(),
            llm_config.base_url.clone(),
            llm_config.model.clone(),
        )
        .map_err(|e| format!("{e:#}"))?;
        Box::new(p)
    }
    LlmMode::Local => {
        #[cfg(feature = "pro")]
        {
            let engine = state
                .local_llm()
                .await
                .map_err(|e| format!("本地推理引擎不可用: {e:#}"))?;
            Box::new(engine.clone())
        }
        #[cfg(not(feature = "pro"))]
        {
            return Err("本地推理是 Pro 版功能，请使用 Pro 版本".to_string());
        }
    }
};
```

**修改 `get_settings_inner`** — 新增 `llm_mode` 和 `local_model` 返回：

```rust
// 读取 llm_mode 和 local_model 设置
let llm_mode = state
    .storage
    .get_setting("llm.mode")
    .await
    .map_err(|e| format!("{e:#}"))?
    .unwrap_or_else(|| "remote".to_string());
let local_model = state
    .storage
    .get_setting("llm.local_model")
    .await
    .map_err(|e| format!("{e:#}"))?
    .unwrap_or_default();

// 在 SettingsPayload 构造中追加：
SettingsPayload {
    // ... 现有字段 ...
    llm_mode,
    local_model,
}
```

### 4.2 TDD 测试用例

#### 文件：`crates/tauri-app/tests/integration.rs`（修改，新增测试模块）

| 测试 ID | 测试名 | 验证内容 |
|---|---|---|
| TC-STATE-LLM-001 | `test_llm_mode_default_remote` | 默认模式为 Remote |
| TC-STATE-LLM-002 | `test_set_llm_mode_persists` | set_llm_mode 持久化到 settings 表 |
| TC-STATE-LLM-003 | `test_get_llm_mode_returns_persisted` | 重启后恢复模式 |
| TC-STATE-LLM-004 | `test_model_manager_initializes` | AppState 初始化时 model_manager 就绪 |

### 4.3 验证命令

```bash
# Free 模式
cargo test -p tauri-app -- state_llm
cargo clippy -p tauri-app -- -D warnings

# Pro 模式
cargo test -p tauri-app --features pro -- state_llm
cargo clippy -p tauri-app --features pro -- -D warnings
```

### 4.4 Session 4 → Session 5 衔接提示词

```
你正在继续 EchoMind 纯 Rust 本地 LLM 引擎的开发。

## 上下文
- Session 1-3 完成：数据模型 / ModelManager / LocalLlmEngine
- Session 4 完成：AppState 集成
  - state.rs: 新增 llm_mode / local_llm(Pro) / model_manager 字段
  - state.rs: 新增 get_llm_mode / set_llm_mode / local_llm / model_manager 方法
  - commands.rs: chat_inner 根据 llm_mode 选择 Remote/Local provider
  - commands.rs: get_settings_inner 返回 llm_mode / local_model
  - 4 个集成测试通过，Free/Pro 双版本 clippy 零警告

## 本 Session 目标
实现 6 个 IPC 命令：list_local_models / download_model / delete_model / set_llm_mode / get_llm_mode / abort_download，并在 lib.rs 注册。

## 必读文档
1. `docs/architecture/LOCAL_LLM_ENGINE_ATOMIC_PLAN.md` — Session 5 章节
2. `crates/tauri-app/src/commands.rs` — 现有命令模式（参考 set_hybrid_search 等）
3. `crates/tauri-app/src/lib.rs` — invoke_handler 注册（第 39-97 行）
4. `crates/infra/src/model_manager.rs` — ModelManager API

## 命令模式
每个命令遵循 *_inner 模式：
- #[tauri::command] pub async fn xxx(...) -> Result<T, String>
- pub async fn xxx_inner(...) -> Result<T, String>（供测试复用）

## 完成后
1. 运行验证命令确认通过
2. 产出下一 Session 衔接提示词
3. 运行 `./sync-to-github.sh` 同步
```

---

## Session 5：IPC 命令 ✅ 已完成

**目标**：实现 6 个新 IPC 命令并注册。

**预计耗时**：2 天

### 5.1 文件变更清单

#### 文件 1：`crates/tauri-app/src/commands.rs`（修改）

**新增命令**（在 `set_context_token_limit` 之后）：

| 命令 | 参数 | 返回 | inner 函数 |
|---|---|---|---|
| `list_local_models` | `state: State<'_, AppState>` | `Result<Vec<ModelInfo>, String>` | `list_local_models_inner(state: &AppState)` |
| `get_recommended_models` | — | `Result<Vec<RecommendedModel>, String>` | — |
| `download_model` | `app: AppHandle, url: String, filename: String, state: State<'_, AppState>` | `Result<(), String>` | `download_model_inner(app, url, filename, state)` |
| `delete_model` | `filename: String, state: State<'_, AppState>` | `Result<(), String>` | `delete_model_inner(filename, state)` |
| `set_llm_mode` | `mode: String, state: State<'_, AppState>` | `Result<(), String>` | `set_llm_mode_inner(mode, state)` |
| `get_llm_mode` | `state: State<'_, AppState>` | `Result<String, String>` | `get_llm_mode_inner(state)` |
| `abort_download` | `filename: String, state: State<'_, AppState>` | `Result<(), String>` | `abort_download_inner(filename, state)` |

**每个命令的实现模式**（以 `set_llm_mode` 为例）：

```rust
/// 切换 LLM 推理模式（REQ-LLM-003 AC-1）。
///
/// 写入 settings 表 `llm.mode`，下次 chat 命令即时生效。
/// - `"remote"`：BYOK 远程 API（OpenAI 兼容）
/// - `"local"`：本地推理（mistral.rs，Pro 功能）
#[tauri::command]
pub async fn set_llm_mode(
    mode: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    set_llm_mode_inner(&mode, state.inner()).await
}

/// 模式切换逻辑（命令与集成测试复用）。
pub async fn set_llm_mode_inner(mode: &str, state: &AppState) -> Result<(), String> {
    match mode {
        "remote" | "local" => {
            state
                .storage
                .set_setting("llm.mode", mode)
                .await
                .map_err(|e| format!("{e:#}"))?;
            let llm_mode = if mode == "local" { LlmMode::Local } else { LlmMode::Remote };
            *state.llm_mode.write().await = llm_mode;
            Ok(())
        }
        _ => Err(format!("无效的 LLM 模式: {mode}（可选: remote / local）")),
    }
}
```

**`download_model` 特殊处理**：
- 在 `spawn` 中执行下载，通过 `app.emit("model_download_progress", ...)` 推送进度
- 下载完成后 emit `model_load_progress` (status: "ready")
- 支持取消（`abort_download` 设置 `AtomicBool` 标志）

#### 文件 2：`crates/tauri-app/src/lib.rs`（修改）

**注册新命令**（在 `invoke_handler` 中追加）：

```rust
// 本地 LLM 推理（REQ-LLM-003/004）
commands::list_local_models,
commands::get_recommended_models,
commands::download_model,
commands::delete_model,
commands::set_llm_mode,
commands::get_llm_mode,
commands::abort_download,
```

### 5.2 TDD 测试用例

| 测试 ID | 测试名 | 验证内容 | 对应 AC |
|---|---|---|---|
| TC-IPC-LLM-001 | `test_list_local_models_empty` | 无模型时返回空 Vec | AC-1 |
| TC-IPC-LLM-002 | `test_list_local_models_after_download` | 下载后列表包含模型 | AC-1 |
| TC-IPC-LLM-003 | `test_delete_model_success` | 删除模型成功 | AC-3 |
| TC-IPC-LLM-004 | `test_delete_model_path_traversal` | 路径穿越被拒绝 | AC-3 |
| TC-IPC-LLM-005 | `test_set_llm_mode_remote` | 设置 remote 模式 | AC-4 |
| TC-IPC-LLM-006 | `test_set_llm_mode_local` | 设置 local 模式 | AC-4 |
| TC-IPC-LLM-007 | `test_set_llm_mode_invalid` | 无效模式返回 Err | AC-4 |
| TC-IPC-LLM-008 | `test_get_llm_mode_default` | 默认返回 remote | — |
| TC-IPC-LLM-009 | `test_get_recommended_models` | 返回推荐模型列表 | AC-6 |

### 5.3 验证命令

```bash
cargo test -p tauri-app -- ipc_llm
cargo clippy -p tauri-app -- -D warnings
cargo clippy -p tauri-app --features pro -- -D warnings
```

### 5.4 Session 5 → Session 6 衔接提示词

```
你正在继续 EchoMind 纯 Rust 本地 LLM 引擎的开发。

## 上下文
- Session 1-4 完成：数据模型 / ModelManager / LocalLlmEngine / AppState 集成
- Session 5 完成：6 个 IPC 命令
  - list_local_models / get_recommended_models / download_model / delete_model / set_llm_mode / get_llm_mode / abort_download
  - lib.rs invoke_handler 已注册全部新命令
  - 9 个集成测试通过，Free/Pro 双版本 clippy 零警告

## 本 Session 目标
前端集成：设置面板新增「本地模型」区域，模型下载进度条，模式切换 UI。

## 必读文档
1. `docs/architecture/LOCAL_LLM_ENGINE_ATOMIC_PLAN.md` — Session 6 章节
2. `ui/src/settings.js` — 设置面板逻辑
3. `ui/src/i18n.js` — 国际化系统
4. `ui/locales/zh-CN.json` / `ui/locales/en.json` — 语言包

## 关键前端变更
1. settings.js: 新增本地模型设置区域渲染逻辑
2. settings.js: 模型列表渲染 + 下载/删除/选择操作
3. i18n: 新增 llm.local.* 系列翻译键
4. 监听 model_download_progress / model_load_progress 事件

## 完成后
1. 运行 `./sync-to-github.sh` 同步
2. 产出下一 Session 衔接提示词
```

---

## Session 6：前端集成

**目标**：设置面板新增本地模型管理 UI。

**预计耗时**：2 天

### 6.1 文件变更清单

#### 文件 1：`ui/src/settings.js`（修改）

**新增函数**：

| 函数 | 职责 |
|---|---|
| `renderLocalModelSection()` | 渲染本地模型设置区域 HTML |
| `loadLocalModels()` | 调用 `list_local_models` 加载已下载模型 |
| `loadRecommendedModels()` | 调用 `get_recommended_models` 加载推荐模型 |
| `downloadModel(url, filename)` | 调用 `download_model`，监听进度事件 |
| `deleteModel(filename)` | 调用 `delete_model` 删除模型 |
| `selectLocalModel(filename)` | 选中模型，调用 `set_llm_mode("local")` |
| `switchLlmMode(mode)` | 切换 Remote/Local 模式 |
| `handleDownloadProgress(event)` | 处理 `model_download_progress` 事件 |
| `handleModelLoadStatus(event)` | 处理 `model_load_progress` 事件 |

#### 文件 2：`ui/locales/zh-CN.json`（修改）

新增键：

```json
{
  "llm.local.title": "本地模型",
  "llm.local.mode": "推理模式",
  "llm.local.mode.remote": "远程 API（BYOK）",
  "llm.local.mode.local": "本地推理",
  "llm.local.downloaded": "已下载模型",
  "llm.local.recommended": "推荐模型",
  "llm.local.download": "下载",
  "llm.local.delete": "删除",
  "llm.local.use": "使用",
  "llm.local.downloading": "下载中…",
  "llm.local.download_complete": "下载完成",
  "llm.local.download_failed": "下载失败",
  "llm.local.loading": "模型加载中…",
  "llm.local.ready": "模型已就绪",
  "llm.local.no_models": "尚未下载任何本地模型",
  "llm.local.pro_required": "本地推理是 Pro 版功能"
}
```

#### 文件 3：`ui/locales/en.json`（修改）

对应英文翻译。

#### 文件 4：`ui/src/state.js`（修改）

新增事件监听注册：

```javascript
// 在现有事件监听后追加
window.__TAURI__.event.listen('model_download_progress', handleDownloadProgress);
window.__TAURI__.event.listen('model_load_progress', handleModelLoadStatus);
```

### 6.2 UI 设计规格

```
┌─ LLM 设置 ──────────────────────────────────┐
│                                             │
│  推理模式                                    │
│  ○ 远程 API（BYOK）                          │
│  ● 本地模型                                  │
│                                             │
│  ┌─ 已下载模型 ─────────────────────────┐   │
│  │ ✓ Qwen2.5-7B-Instruct  Q4_K_M  4.1GB │   │
│  │   [使用] [删除]                       │   │
│  │ ✓ Llama-3.2-3B-Instruct Q4_K_M  2.0GB│   │
│  │   [使用] [删除]                       │   │
│  └──────────────────────────────────────┘   │
│                                             │
│  [下载模型]                                  │
│  ┌─ 推荐模型 ───────────────────────────┐   │
│  │ Qwen2.5-3B-Instruct  Q4_K_M  2.0GB   │   │
│  │ 中文优秀，低配推荐    [下载]          │   │
│  │ Llama-3.2-3B         Q4_K_M  2.0GB   │   │
│  │ 英文场景推荐          [下载]          │   │
│  │ Phi-3.5-mini         Q4_K_M  2.2GB   │   │
│  │ 推理能力强            [下载]          │   │
│  │ Qwen2.5-7B           Q4_K_M  4.1GB   │   │
│  │ 质量速度平衡          [下载]          │   │
│  └──────────────────────────────────────┘   │
│                                             │
│  下载进度: ████████░░ 80% (3.2/4.1GB)       │
└─────────────────────────────────────────────┘
```

### 6.3 验证命令

```bash
# 前端构建检查
node scripts/build-ui.mjs

# E2E 测试（如果编写了）
cd e2e-tests && ECHOMIND_E2E_TEST=1 npx playwright test --grep "local model"
```

### 6.4 Session 6 → Session 7 衔接提示词

```
你正在继续 EchoMind 纯 Rust 本地 LLM 引擎的开发。

## 上下文
- Session 1-5 完成：后端全栈（数据模型 / ModelManager / LocalLlmEngine / AppState / IPC 命令）
- Session 6 完成：前端集成
  - settings.js: 本地模型设置区域
  - i18n: zh-CN.json / en.json 新增 llm.local.* 键
  - 事件监听: model_download_progress / model_load_progress

## 本 Session 目标
文档同步 + GitHub 发布同步，确保所有文档与代码一致。

## 必须更新的文档清单
1. docs/SRS_v1.0.md — 新增 REQ-LLM-003 / REQ-LLM-004
2. docs/api/ipc-commands.md — 新增 7 个命令
3. docs/api/code-contracts.md — 新增 LocalLlmEngine / ModelManager / ModelInfo 等
4. docs/api/settings-reference.md — 新增 llm.mode / llm.local_model / llm.local_quantization 键
5. docs/api/frontend-modules.md — 新增 settings.js 中的本地模型函数
6. docs/architecture/adr/ADR-011-local-llm-engine.md — 架构决策记录
7. AGENTS.md / CLAUDE.md — 更新架构描述

## 完成后
1. 运行 `./sync-to-github.sh` 同步
2. 运行 `cargo test` 确认全部通过
3. 运行 `cargo clippy --all-targets -- -D warnings` 确认零警告
4. 更新 memory（如果使用 update_memory 工具）
```

---

## Session 7：文档同步 & 发布

**目标**：所有文档与代码同步，GitHub 发布。

**预计耗时**：1 天

### 7.1 文档更新清单

| 文档 | 变更内容 |
|---|---|
| `docs/SRS_v1.0.md` | 新增 REQ-LLM-003（本地 LLM 推理引擎）+ REQ-LLM-004（本地模型管理），含完整 AC |
| `docs/api/ipc-commands.md` | 新增 7 个命令的签名、参数、返回值 |
| `docs/api/code-contracts.md` | 新增 `LocalLlmEngine` / `ModelManager` / `ModelInfo` / `LlmMode` / `Quantization` |
| `docs/api/settings-reference.md` | 新增 `llm.mode` / `llm.local_model` / `llm.local_quantization` |
| `docs/api/frontend-modules.md` | 新增 settings.js 中的 `renderLocalModelSection` 等函数 |
| `docs/architecture/adr/ADR-011-local-llm-engine.md` | 新建 ADR |
| `AGENTS.md` | 更新架构表，新增 local_llm / model_manager 模块 |
| `CLAUDE.md` | 更新 crate 列表和关键文件 |

### 7.2 最终验证

```bash
# 全量测试
cargo test
cargo test --features pro

# Clippy
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --features pro -- -D warnings

# 格式检查
cargo fmt --check

# 同步到 GitHub
./sync-to-github.sh
```

### 7.3 Session 7 完成标志

- [ ] 全部文档更新完成
- [ ] `cargo test` 全部通过（Free + Pro）
- [ ] `cargo clippy` 零警告（Free + Pro）
- [ ] `./sync-to-github.sh` 执行成功
- [ ] 前端构建成功
- [ ] ADR-011 创建完成

---

## 附录 A：完整文件变更矩阵

| 文件路径 | Session | 变更类型 | 说明 |
|---|---|---|---|
| `crates/models/src/lib.rs` | S1 | 修改 | 新增 LlmMode/ModelInfo/Payloads + SettingsPayload 字段 |
| `crates/infra/src/lib.rs` | S2, S3 | 修改 | 注册 model_manager + local_llm 模块 |
| `crates/infra/src/model_manager.rs` | S2 | 新建 | ModelManager 实现 |
| `crates/infra/src/model_manager_tests.rs` | S2 | 新建 | 20 个测试 |
| `crates/infra/Cargo.toml` | S3 | 修改 | 新增 mistralrs Pro 依赖 |
| `crates/infra/src/local_llm.rs` | S3 | 新建 | LocalLlmEngine 实现 |
| `crates/infra/src/local_llm_tests.rs` | S3 | 新建 | 8 个测试 |
| `crates/tauri-app/src/state.rs` | S4 | 修改 | 新增 llm_mode/local_llm/model_manager |
| `crates/tauri-app/src/commands.rs` | S4, S5 | 修改 | chat_inner 分支 + 7 个新命令 |
| `crates/tauri-app/src/lib.rs` | S5 | 修改 | 注册 7 个新命令 |
| `crates/tauri-app/tests/integration.rs` | S4, S5 | 修改 | 新增测试模块 |
| `ui/src/settings.js` | S6 | 修改 | 本地模型 UI |
| `ui/src/state.js` | S6 | 修改 | 事件监听注册 |
| `ui/locales/zh-CN.json` | S6 | 修改 | 新增翻译键 |
| `ui/locales/en.json` | S6 | 修改 | 新增翻译键 |
| `docs/SRS_v1.0.md` | S7 | 修改 | 新增 REQ-LLM-003/004 |
| `docs/api/ipc-commands.md` | S7 | 修改 | 新增 7 个命令 |
| `docs/api/code-contracts.md` | S7 | 修改 | 新增契约 |
| `docs/api/settings-reference.md` | S7 | 修改 | 新增设置键 |
| `docs/api/frontend-modules.md` | S7 | 修改 | 新增前端函数 |
| `docs/architecture/adr/ADR-011-local-llm-engine.md` | S7 | 新建 | ADR |
| `AGENTS.md` | S7 | 修改 | 架构更新 |
| `CLAUDE.md` | S7 | 修改 | 架构更新 |

---

## 附录 B：Settings 键完整定义

| 键 | 类型 | 默认值 | Session | 说明 |
|---|---|---|---|---|
| `llm.mode` | String | `"remote"` | S4 | 推理模式：`remote` / `local` |
| `llm.local_model` | String | `""` | S4 | 当前选中的本地模型文件名 |
| `llm.local_quantization` | String | `"q4_k_m"` | S3 | 量化格式 |

---

## 附录 C：Tauri 事件完整定义

| 事件 | 载荷类型 | Session | 说明 |
|---|---|---|---|
| `model_download_progress` | `ModelDownloadProgressPayload` | S5 | 模型下载进度（每秒推送） |
| `model_load_progress` | `ModelLoadStatusPayload` | S3 | 模型加载状态变更 |

---

## 附录 D：IPC 命令完整定义

| 命令 | 参数 | 返回 | Session |
|---|---|---|---|
| `list_local_models` | — | `Vec<ModelInfo>` | S5 |
| `get_recommended_models` | — | `Vec<RecommendedModel>` | S5 |
| `download_model` | `url: String, filename: String` | `()` | S5 |
| `delete_model` | `filename: String` | `()` | S5 |
| `set_llm_mode` | `mode: String` | `()` | S5 |
| `get_llm_mode` | — | `String` | S5 |
| `abort_download` | `filename: String` | `()` | S5 |
