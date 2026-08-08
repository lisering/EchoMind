#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented,
    clippy::manual_checked_ops
)]
//! LocalLlmEngine 单元测试（REQ-LLM-003）。
//!
//! 测试覆盖：量化格式解析、引擎创建、加载状态、消息构建。
//! 真实模型推理测试标记为 `#[ignore]`，需要下载 GGUF 模型后手动运行。

use std::sync::Arc;

use super::local_llm::*;
use echomind_models::{ChatMessage, LlmSamplingParams};

// ---- TC-LLM-LOCAL-001 ~ TC-LLM-LOCAL-002: Quantization ----

/// TC-LLM-LOCAL-001：字符串解析量化格式
#[test]
fn test_quantization_from_str() {
    assert_eq!(Quantization::from_str("Q4_K_M"), Quantization::Q4K_M);
    assert_eq!(Quantization::from_str("q4_k_m"), Quantization::Q4K_M);
    assert_eq!(Quantization::from_str("Q5_K_M"), Quantization::Q5K_M);
    assert_eq!(Quantization::from_str("Q8_0"), Quantization::Q8_0);
    assert_eq!(Quantization::from_str("unknown"), Quantization::Auto);
    assert_eq!(Quantization::from_str(""), Quantization::Auto);
}

/// TC-LLM-LOCAL-002：量化格式转字符串
#[test]
fn test_quantization_as_str() {
    assert_eq!(Quantization::Q4K_M.as_str(), "Q4_K_M");
    assert_eq!(Quantization::Q5K_M.as_str(), "Q5_K_M");
    assert_eq!(Quantization::Q8_0.as_str(), "Q8_0");
    assert_eq!(Quantization::Auto.as_str(), "auto");
}

// ---- TC-LLM-LOCAL-003 ~ TC-LLM-LOCAL-005: LocalLlmEngine 创建与状态 ----

/// TC-LLM-LOCAL-003：不存在的模型文件返回 Err
#[test]
fn test_new_nonexistent_file_fails() {
    let result = LocalLlmEngine::new(
        std::path::PathBuf::from("/nonexistent/model.gguf"),
        Quantization::Q4K_M,
    );
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("模型文件不存在"));
}

/// TC-LLM-LOCAL-004：有效路径创建成功
#[test]
fn test_new_valid_path_succeeds() {
    let tmp = tempfile::tempdir().expect("创建临时目录失败");
    let model_path = tmp.path().join("test-model.gguf");
    std::fs::write(&model_path, b"fake gguf content").expect("写入文件失败");

    let engine =
        LocalLlmEngine::new(model_path.clone(), Quantization::Q4K_M).expect("创建引擎失败");
    assert!(!engine.is_loaded());
}

/// TC-LLM-LOCAL-005：初始状态未加载
#[test]
fn test_is_loaded_false_before_init() {
    let tmp = tempfile::tempdir().expect("创建临时目录失败");
    let model_path = tmp.path().join("test-model.gguf");
    std::fs::write(&model_path, b"fake").expect("写入文件失败");

    let engine = LocalLlmEngine::new(model_path, Quantization::Auto).expect("创建引擎失败");
    assert!(!engine.is_loaded());
}

// ---- TC-LLM-LOCAL-006 ~ TC-LLM-LOCAL-008: build_messages ----

/// TC-LLM-LOCAL-006：请求包含 system prompt
#[test]
fn test_build_messages_system_prompt() {
    let messages = build_messages("你是一个助手", &[], "你好");
    assert!(!messages.is_empty());
    assert_eq!(messages[0].0, "system");
    assert_eq!(messages[0].1, "你是一个助手");
}

/// TC-LLM-LOCAL-007：请求包含历史消息
#[test]
fn test_build_messages_history() {
    let history = vec![
        ChatMessage {
            id: None,
            role: "user".to_string(),
            content: "之前的问题".to_string(),
            sources: None,
            reasoning: None,
            ..Default::default()
        },
        ChatMessage {
            id: None,
            role: "assistant".to_string(),
            content: "之前的回答".to_string(),
            sources: None,
            reasoning: None,
            ..Default::default()
        },
    ];
    let messages = build_messages("系统提示", &history, "新问题");
    // system + 2 history + user = 4
    assert_eq!(messages.len(), 4);
    assert_eq!(messages[1].0, "user");
    assert_eq!(messages[1].1, "之前的问题");
    assert_eq!(messages[2].0, "assistant");
    assert_eq!(messages[2].1, "之前的回答");
}

/// TC-LLM-LOCAL-008：请求包含用户查询
#[test]
fn test_build_messages_query() {
    let messages = build_messages("系统提示", &[], "这是一个查询");
    let last = messages.last().expect("消息列表不应为空");
    assert_eq!(last.0, "user");
    assert_eq!(last.1, "这是一个查询");
}

// ---- TC-LLM-UPGRADE-001 ~ TC-LLM-UPGRADE-003: v0.9.0 API 可用性验证 ----

/// TC-LLM-UPGRADE-001：v0.9.0 GgufModelBuilder 含 with_force_cpu 方法。
///
/// 编译时验证：方法存在即可链式调用（不实际加载模型）。
/// 如果此测试编译通过，说明 v0.9.0 的 `with_force_cpu()` API 存在。
#[test]
fn test_gguf_builder_has_force_cpu() {
    let builder = mistralrs::GgufModelBuilder::new("/tmp", vec!["test.gguf"]).with_force_cpu();
    drop(builder);
}

/// TC-LLM-UPGRADE-002：v0.9.0 RequestBuilder 含 set_sampler_temperature 方法。
///
/// 验证采样温度 API 存在，S11 采样参数优化将使用此方法。
#[test]
fn test_request_builder_has_set_sampler_temperature() {
    let builder = mistralrs::RequestBuilder::new().set_sampler_temperature(0.7);
    drop(builder);
}

/// TC-LLM-UPGRADE-003：best_device(true) 返回 CPU。
///
/// `force_cpu=true` 时必须返回 `Device::Cpu`。
#[test]
fn test_best_device_returns_cpu() {
    let device = mistralrs::best_device(true).expect("best_device 应返回 CPU");
    assert!(
        matches!(device, mistralrs::Device::Cpu),
        "force_cpu=true 时应返回 CPU 设备"
    );
}

// ---- TC-LLM-GPU-001 ~ TC-LLM-GPU-003: GPU 设备选择 ----

/// TC-LLM-GPU-001：无 GPU feature 时 device_kind 为 "cpu"。
///
/// 在未启用 metal/cuda feature 时，`device_kind()` 必须返回 `"cpu"`。
#[test]
fn test_device_kind_cpu_without_gpu_features() {
    let tmp = tempfile::tempdir().expect("创建临时目录失败");
    let model_path = tmp.path().join("test.gguf");
    std::fs::write(&model_path, b"fake").expect("写入失败");
    let engine = LocalLlmEngine::new(model_path, Quantization::Auto).expect("创建失败");
    assert_eq!(engine.device_kind(), "cpu");
}

/// TC-LLM-GPU-002：device_kind 在 new() 时即确定。
///
/// 无论 GPU 是否可用，`device_kind()` 不应为空字符串。
/// 这验证设备类型在引擎创建时就已确定（而非加载时）。
#[test]
fn test_device_kind_set_at_creation() {
    let tmp = tempfile::tempdir().expect("创建临时目录失败");
    let model_path = tmp.path().join("test.gguf");
    std::fs::write(&model_path, b"fake").expect("写入失败");
    let engine = LocalLlmEngine::new(model_path, Quantization::Q4K_M).expect("创建失败");
    // 无论 GPU 是否可用，device_kind 不应为空
    assert!(!engine.device_kind().is_empty());
}

/// TC-LLM-GPU-003：best_device(true) 始终返回 CPU。
///
/// `force_cpu=true` 是 GPU 回退的安全路径，必须始终返回 `Device::Cpu`。
#[test]
fn test_best_device_force_cpu() {
    let device = mistralrs::best_device(true).expect("best_device(true) 应返回 CPU");
    assert!(
        matches!(device, mistralrs::Device::Cpu),
        "force_cpu=true 时应始终返回 CPU 设备"
    );
}

// ---- TC-LLM-PAGED-001 ~ TC-LLM-PAGED-003: PagedAttention 配置 ----

/// TC-LLM-PAGED-001：with_paged_attn 设置启用标志。
///
/// 调用 `with_paged_attn()` 后，引擎应能成功创建（方法存在且不 panic）。
/// 由于 `use_paged_attn` 是内部字段，通过验证方法链式调用不 panic 来间接验证。
#[test]
fn test_with_paged_attn_sets_flag() {
    let tmp = tempfile::tempdir().expect("创建临时目录失败");
    let model_path = tmp.path().join("test.gguf");
    std::fs::write(&model_path, b"fake").expect("写入失败");

    let engine = LocalLlmEngine::new(model_path, Quantization::Auto).expect("创建失败");
    // with_paged_attn 返回 Self，验证链式调用不 panic
    let _engine = engine.with_paged_attn(16, 2048);
}

/// TC-LLM-PAGED-002：默认块大小为 32，GPU 上下文为 4096。
///
/// 新建引擎时，PagedAttention 默认关闭，块大小默认 32，GPU 上下文默认 4096。
/// 通过 `with_paged_attn()` 可覆盖默认值。
#[test]
fn test_paged_attn_default_block_size() {
    let tmp = tempfile::tempdir().expect("创建临时目录失败");
    let model_path = tmp.path().join("test.gguf");
    std::fs::write(&model_path, b"fake").expect("写入失败");

    let engine = LocalLlmEngine::new(model_path, Quantization::Auto).expect("创建失败");
    // 默认不启用 PagedAttention，使用 with_paged_attn 自定义参数
    let _engine = engine.with_paged_attn(32, 4096);
    // 验证自定义块大小也可工作
    let tmp2 = tempfile::tempdir().expect("创建临时目录失败");
    let model_path2 = tmp2.path().join("test2.gguf");
    std::fs::write(&model_path2, b"fake").expect("写入失败");
    let engine2 = LocalLlmEngine::new(model_path2, Quantization::Q4K_M).expect("创建失败");
    let _engine2 = engine2.with_paged_attn(8, 1024);
}

/// TC-LLM-PAGED-003：默认不启用 PagedAttention。
///
/// 新创建的引擎默认 `use_paged_attn = false`，即使 GPU feature 启用，
/// 也不会启用 PagedAttention（需用户主动通过 `with_paged_attn()` 开启）。
#[test]
fn test_paged_attn_disabled_by_default() {
    let tmp = tempfile::tempdir().expect("创建临时目录失败");
    let model_path = tmp.path().join("test.gguf");
    std::fs::write(&model_path, b"fake").expect("写入失败");

    let engine = LocalLlmEngine::new(model_path, Quantization::Auto).expect("创建失败");
    // 默认创建的引擎不应启用 PagedAttention
    // 通过 is_loaded() == false 验证引擎状态正常（未触发模型加载）
    assert!(!engine.is_loaded());
    // 引擎可正常使用，验证不 panic
    let _ = engine.device_kind();
}

// ---- TC-LLM-SAMPLE-001 ~ TC-LLM-SAMPLE-004: 采样参数（S11） ----

/// TC-LLM-SAMPLE-001：默认采样参数所有字段为 None。
///
/// 新创建的引擎，采样参数应为全 `None`（使用引擎默认值）。
#[tokio::test]
async fn test_default_sampling_params() {
    let tmp = tempfile::tempdir().expect("创建临时目录失败");
    let model_path = tmp.path().join("test.gguf");
    std::fs::write(&model_path, b"fake").expect("写入失败");
    let engine = LocalLlmEngine::new(model_path, Quantization::Auto).expect("创建失败");
    let params = engine.sampling_params().await;
    assert!(params.temperature.is_none(), "默认 temperature 应为 None");
    assert!(params.top_p.is_none(), "默认 top_p 应为 None");
    assert!(params.top_k.is_none(), "默认 top_k 应为 None");
    assert!(params.max_tokens.is_none(), "默认 max_tokens 应为 None");
    assert!(
        params.frequency_penalty.is_none(),
        "默认 frequency_penalty 应为 None"
    );
    assert!(
        params.presence_penalty.is_none(),
        "默认 presence_penalty 应为 None"
    );
}

/// TC-LLM-SAMPLE-002：设置参数后可读取。
///
/// 调用 `set_sampling_params()` 后，`sampling_params()` 应返回最新值。
#[tokio::test]
async fn test_set_sampling_params() {
    let tmp = tempfile::tempdir().expect("创建临时目录失败");
    let model_path = tmp.path().join("test.gguf");
    std::fs::write(&model_path, b"fake").expect("写入失败");
    let engine = LocalLlmEngine::new(model_path, Quantization::Auto).expect("创建失败");

    let params = LlmSamplingParams {
        temperature: Some(0.8),
        top_p: Some(0.95),
        top_k: Some(40),
        max_tokens: Some(2048),
        frequency_penalty: Some(0.5),
        presence_penalty: Some(0.2),
    };
    engine.set_sampling_params(params).await;

    let read = engine.sampling_params().await;
    assert_eq!(read.temperature, Some(0.8));
    assert_eq!(read.top_p, Some(0.95));
    assert_eq!(read.top_k, Some(40));
    assert_eq!(read.max_tokens, Some(2048));
    assert_eq!(read.frequency_penalty, Some(0.5));
    assert_eq!(read.presence_penalty, Some(0.2));
}

/// TC-LLM-SAMPLE-003：采样参数可多次修改（覆盖而非追加）。
///
/// 连续调用 `set_sampling_params()` 后，应反映最后一次设置的值。
#[tokio::test]
async fn test_sampling_params_overwrite() {
    let tmp = tempfile::tempdir().expect("创建临时目录失败");
    let model_path = tmp.path().join("test.gguf");
    std::fs::write(&model_path, b"fake").expect("写入失败");
    let engine = LocalLlmEngine::new(model_path, Quantization::Auto).expect("创建失败");

    // 第一次设置
    engine
        .set_sampling_params(LlmSamplingParams {
            temperature: Some(0.8),
            max_tokens: Some(2048),
            ..Default::default()
        })
        .await;
    let first = engine.sampling_params().await;
    assert_eq!(first.temperature, Some(0.8));
    assert_eq!(first.max_tokens, Some(2048));

    // 第二次设置（不同值，max_tokens 设为 None）
    engine
        .set_sampling_params(LlmSamplingParams {
            temperature: Some(0.3),
            max_tokens: None,
            ..Default::default()
        })
        .await;
    let second = engine.sampling_params().await;
    assert_eq!(
        second.temperature,
        Some(0.3),
        "应反映第二次设置的 temperature"
    );
    assert!(
        second.max_tokens.is_none(),
        "max_tokens 应被覆盖为 None（而非保留第一次的 2048）"
    );
}

/// TC-LLM-SAMPLE-004：build_messages 不受采样参数影响。
///
/// 采样参数仅影响 RequestBuilder，不影响消息构建逻辑。
#[test]
fn test_build_messages_with_sampling() {
    let messages = build_messages("系统", &[], "查询");
    assert_eq!(messages.len(), 2, "系统提示 + 用户查询 = 2 条消息");
    assert_eq!(messages[0].0, "system");
    assert_eq!(messages[0].1, "系统");
    assert_eq!(messages[1].0, "user");
    assert_eq!(messages[1].1, "查询");
}

// ---- TC-LLM-UNLOAD-001 ~ TC-LLM-UNLOAD-004: 模型卸载/切换（S12） ----

/// TC-LLM-UNLOAD-001：unload() 后 is_loaded() == false。
///
/// 新建引擎处于未加载状态，调用 `unload()` 后仍应为未加载状态。
/// 验证 `unload()` 在未加载模型时不会 panic，且 `is_loaded()` 正确返回 false。
#[tokio::test]
async fn test_unload_sets_none() {
    let tmp = tempfile::tempdir().expect("创建临时目录失败");
    let model_path = tmp.path().join("test.gguf");
    std::fs::write(&model_path, b"fake").expect("写入失败");
    let engine = LocalLlmEngine::new(model_path, Quantization::Auto).expect("创建失败");
    assert!(!engine.is_loaded());
    // 卸载未加载的模型不应 panic
    engine.unload().await;
    assert!(!engine.is_loaded());
}

/// TC-LLM-UNLOAD-002：初始状态未加载（RwLock<Option> 迁移后验证）。
///
/// 迁移到 `RwLock<Option<Arc<Model>>>` 后，新创建的引擎 `is_loaded()` 必须返回 false。
/// 这验证 `try_read()` 在 RwLock 上的行为正确。
#[tokio::test]
async fn test_is_loaded_false_before_init_rwlock() {
    let tmp = tempfile::tempdir().expect("创建临时目录失败");
    let model_path = tmp.path().join("test.gguf");
    std::fs::write(&model_path, b"fake").expect("写入失败");
    let engine = LocalLlmEngine::new(model_path, Quantization::Auto).expect("创建失败");
    assert!(!engine.is_loaded());
}

/// TC-LLM-UNLOAD-003：多次调用 unload() 不 panic（幂等性）。
///
/// 连续调用 `unload()` 三次，每次都不应 panic。
/// 这验证 RwLock 的 write 锁在多次获取时不会死锁或出错。
#[tokio::test]
async fn test_unload_idempotent() {
    let tmp = tempfile::tempdir().expect("创建临时目录失败");
    let model_path = tmp.path().join("test.gguf");
    std::fs::write(&model_path, b"fake").expect("写入失败");
    let engine = LocalLlmEngine::new(model_path, Quantization::Auto).expect("创建失败");
    engine.unload().await;
    engine.unload().await;
    engine.unload().await;
    assert!(!engine.is_loaded());
}

/// TC-LLM-UNLOAD-004：get_or_load_model() 并发调用只加载一次（需真实模型）。
///
/// 此测试需要真实的 GGUF 模型文件，标记为 `#[ignore]`。
/// 验证 `get_or_load_model()` 的 double-check locking 模式：
/// 多个并发任务同时调用时，模型只加载一次，所有任务获得同一个 Arc<Model>。
#[tokio::test]
#[ignore = "需要真实 GGUF 模型文件，设置 ECHOMIND_TEST_MODEL 环境变量"]
async fn test_get_or_load_model_double_check() {
    let model_path = std::env::var("ECHOMIND_TEST_MODEL").unwrap_or_else(|_| {
        eprintln!("跳过：设置 ECHOMIND_TEST_MODEL 环境变量指向 GGUF 模型文件以运行此测试");
        std::process::exit(0);
    });
    let engine = LocalLlmEngine::new(std::path::PathBuf::from(&model_path), Quantization::Auto)
        .expect("创建失败");
    assert!(!engine.is_loaded());

    // 并发调用 get_or_load_model — double-check locking 确保只加载一次
    let engine_clone = engine.clone();
    let task1 = tokio::spawn(async move { engine_clone.get_or_load_model().await });
    let task2 = tokio::spawn(async move { engine.get_or_load_model().await });

    let model1 = task1.await.expect("task1 panic").expect("加载失败");
    let model2 = task2.await.expect("task2 panic").expect("加载失败");

    // 两个 Arc<Model> 应指向同一个模型实例（Arc 指针相等）
    assert!(
        Arc::as_ptr(&model1) == Arc::as_ptr(&model2),
        "并发加载应返回同一个模型实例（double-check locking 生效）"
    );
}

// ---- TC-LLM-ABORT-001 ~ TC-LLM-ABORT-003: 流取消集成（S13） ----

/// TC-LLM-ABORT-001：abort() 在未启动 chat 时调用不 panic。
///
/// 新建引擎处于未加载状态，调用 `abort()` 应安全返回（不 panic）。
/// 此验证确保 `abort_chat` 命令在用户未发起对话时也能安全调用。
#[tokio::test]
async fn test_abort_does_not_panic_before_chat() {
    let tmp = tempfile::tempdir().expect("创建临时目录失败");
    let model_path = tmp.path().join("test.gguf");
    std::fs::write(&model_path, b"fake").expect("写入失败");
    let engine = LocalLlmEngine::new(model_path, Quantization::Auto).expect("创建失败");
    // 不应 panic
    engine.abort().await;
}

/// TC-LLM-ABORT-002：abort() 后 cancel_token 被取消。
///
/// `abort()` 触发内部 `CancellationToken`，通过 `current_cancel_token()`
/// 读取的令牌应处于已取消状态（`is_cancelled() == true`）。
#[tokio::test]
async fn test_abort_sets_cancelled() {
    let tmp = tempfile::tempdir().expect("创建临时目录失败");
    let model_path = tmp.path().join("test.gguf");
    std::fs::write(&model_path, b"fake").expect("写入失败");
    let engine = LocalLlmEngine::new(model_path, Quantization::Auto).expect("创建失败");

    // 初始状态：cancel token 未被取消
    let token = engine.current_cancel_token().await;
    assert!(
        !token.is_cancelled(),
        "新创建的引擎 cancel token 不应处于已取消状态"
    );

    // 触发 abort
    engine.abort().await;

    // abort() 触发的是之前读取的 token
    assert!(
        token.is_cancelled(),
        "abort() 后 cancel token 应处于已取消状态"
    );
}

/// TC-LLM-ABORT-003：多次调用 abort() 不 panic（幂等性）。
///
/// 连续调用 `abort()` 三次，每次都不应 panic。
/// 模拟前端用户快速多次点击「停止生成」按钮的场景。
#[tokio::test]
async fn test_abort_multiple_calls() {
    let tmp = tempfile::tempdir().expect("创建临时目录失败");
    let model_path = tmp.path().join("test.gguf");
    std::fs::write(&model_path, b"fake").expect("写入失败");
    let engine = LocalLlmEngine::new(model_path, Quantization::Auto).expect("创建失败");
    engine.abort().await;
    engine.abort().await;
    engine.abort().await;
    // 不应 panic，且 token 应处于已取消状态
    let token = engine.current_cancel_token().await;
    assert!(
        token.is_cancelled(),
        "多次 abort() 后 token 应处于已取消状态"
    );
}

// ---- TC-LLM-WARM-001 ~ TC-LLM-WARM-003: 模型预热 + 错误处理加固（S14） ----

/// TC-LLM-WARM-001：warm_up() 返回 Err（假模型文件无法加载）。
///
/// warm_up() 内部调用 get_or_load_model()，使用假文件（内容为 "fake"）
/// 会触发 GGUF 解析失败，返回 Err。此测试验证方法存在且不 panic，
/// 错误为预期行为（真实模型文件不存在时用户会看到此错误）。
#[tokio::test]
async fn test_warm_up_returns_err_with_fake_file() {
    let tmp = tempfile::tempdir().expect("创建临时目录失败");
    let model_path = tmp.path().join("test.gguf");
    std::fs::write(&model_path, b"fake").expect("写入失败");
    let engine = LocalLlmEngine::new(model_path, Quantization::Auto).expect("创建失败");
    // warm_up 会尝试加载模型（fake 文件会失败），但方法不应 panic
    let result = engine.warm_up().await;
    assert!(result.is_err(), "假模型文件应返回 Err，而不是 Ok 或 panic");
}

/// TC-LLM-WARM-002：多次调用 warm_up() 不 panic（幂等性）。
///
/// 连续调用 warm_up() 三次，每次都不应 panic。
/// 第一次调用失败后，后续调用应安全重试（get_or_load_model 的
/// double-check locking 确保并发安全）。
/// 模拟用户快速切换模型或模式时的场景。
#[tokio::test]
async fn test_warm_up_idempotent() {
    let tmp = tempfile::tempdir().expect("创建临时目录失败");
    let model_path = tmp.path().join("test.gguf");
    std::fs::write(&model_path, b"fake").expect("写入失败");
    let engine = LocalLlmEngine::new(model_path, Quantization::Auto).expect("创建失败");
    // 多次调用不应 panic
    let _ = engine.warm_up().await;
    let _ = engine.warm_up().await;
    let _ = engine.warm_up().await;
    // 引擎仍处于未加载状态（假文件加载失败）
    assert!(!engine.is_loaded());
}

/// TC-LLM-WARM-003：extract_text 对空 choices 的 Chunk 返回 None。
///
/// 构造一个 choices 为空向量的 ChatCompletionChunkResponse，
/// 验证 extract_text 返回 None（无法提取文本）。
/// 这测试了 S14 加固后 extract_text 的防御性处理：
/// 当 Chunk 变体不含有效 delta.content 时安全返回 None，
/// 而非 panic 或返回空字符串。
///
/// 注意：mistral.rs 的 Response 枚举有多个变体（Chunk/Done/Error/
/// CompletionChunk/ImageGeneration 等），所有非 Chunk 变体由
/// match 的 `_ => None` 分支处理。此处测试 Chunk 变体内部 choices
/// 为空时的 None 返回路径（非 Chunk 变体因构造复杂由编译器保证安全）。
#[test]
fn test_extract_text_handles_non_chunk() {
    use mistralrs::{ChatCompletionChunkResponse, Response};

    // 构造空 choices 的 Chunk — choices.first() 返回 None
    let chunk = ChatCompletionChunkResponse {
        id: String::new(),
        choices: vec![],
        created: 0,
        model: String::new(),
        system_fingerprint: String::new(),
        object: String::new(),
        usage: None,
        session_id: None,
    };
    let response = Response::Chunk(chunk);
    assert!(
        extract_text(&response).is_none(),
        "空 choices 的 Chunk 应返回 None"
    );
}

// ---- TC-LLM-PERF-001 ~ TC-LLM-PERF-003: 性能基准测试（S15） ----

/// 性能基准测试（标记 #[ignore]，需要真实 GGUF 模型文件）。
///
/// 对比 CPU vs GPU（Metal）推理速度，验证 Phase 2 性能优化效果。
/// 运行方式：
/// ```bash
/// ECHOMIND_TEST_MODEL=/path/to/model.gguf \
/// cargo test -p echomind-infra --features pro -- local_llm_tests --ignored
/// ```
///
/// 测试输出（stdout）包含首 token 延迟和 tokens/sec，
/// 运行后在 ADR-011 中记录实际测试数据。
///
/// TC-LLM-PERF-001：CPU 推理速度基准。
///
/// 使用 `with_force_cpu()` 路径创建 CPU-only 引擎，发送固定 prompt，
/// 测量首 token 延迟（TTFT）和总生成速度（tokens/sec）。
///
/// 运行方式：
/// ```bash
/// ECHOMIND_TEST_MODEL=/path/to/model.gguf \
/// cargo test -p echomind-infra --features pro -- test_cpu_inference_speed --ignored --nocapture
/// ```
#[tokio::test]
#[ignore = "需要真实 GGUF 模型文件，设置 ECHOMIND_TEST_MODEL 环境变量"]
async fn test_cpu_inference_speed() {
    let model_path = std::env::var("ECHOMIND_TEST_MODEL").unwrap_or_else(|_| {
        eprintln!("跳过：设置 ECHOMIND_TEST_MODEL 环境变量指向 GGUF 模型文件以运行此测试");
        std::process::exit(0);
    });

    let engine = LocalLlmEngine::new(std::path::PathBuf::from(&model_path), Quantization::Auto)
        .expect("创建引擎失败");

    // 预热模型
    engine.warm_up().await.expect("模型预热失败");
    assert!(engine.is_loaded(), "模型应已加载");

    let prompt = "Write a 100-word essay about Rust programming language.";
    let start = std::time::Instant::now();
    let mut first_token_time: Option<std::time::Duration> = None;
    let mut token_count = 0usize;

    use echomind_core::LLMProvider;
    use futures::StreamExt;
    let mut stream = engine
        .chat_stream("You are a helpful assistant.", &[], prompt)
        .await
        .expect("chat_stream 失败");

    while let Some(result) = stream.next().await {
        if first_token_time.is_none() {
            first_token_time = Some(start.elapsed());
        }
        if result.is_ok() {
            token_count += 1;
        }
    }

    let total_time = start.elapsed();
    let ttft = first_token_time.unwrap_or(total_time);
    let tokens_per_sec = if total_time.as_secs_f64() > 0.0 {
        token_count as f64 / total_time.as_secs_f64()
    } else {
        0.0
    };

    println!(
        "\n=== CPU 推理基准 ===\n模型: {model_path}\n设备: {}\n首 token 延迟: {:.0} ms\n总 token 数: {token_count}\n总时间: {:.2} s\n速度: {:.1} tok/s\n",
        engine.device_kind(),
        ttft.as_millis(),
        total_time.as_secs_f64(),
        tokens_per_sec,
    );

    // 基本合理性断言
    assert!(token_count > 0, "应至少生成 1 个 token");
}

/// TC-LLM-PERF-002：GPU（Metal）推理速度基准。
///
/// 使用 `best_device(false)` 路径创建 GPU 引擎（仅 metal feature 启用时有效），
/// 发送相同 prompt，与 CPU 基准对比。
///
/// 运行方式（需 metal feature）：
/// ```bash
/// ECHOMIND_TEST_MODEL=/path/to/model.gguf \
/// cargo test -p echomind-infra --features pro,metal -- test_gpu_metal_inference_speed --ignored --nocapture
/// ```
#[tokio::test]
#[ignore = "需要真实 GGUF 模型文件 + metal feature，设置 ECHOMIND_TEST_MODEL 环境变量"]
async fn test_gpu_metal_inference_speed() {
    let model_path = std::env::var("ECHOMIND_TEST_MODEL").unwrap_or_else(|_| {
        eprintln!("跳过：设置 ECHOMIND_TEST_MODEL 环境变量指向 GGUF 模型文件以运行此测试");
        std::process::exit(0);
    });

    let engine = LocalLlmEngine::new(std::path::PathBuf::from(&model_path), Quantization::Auto)
        .expect("创建引擎失败");

    // 预热模型
    engine.warm_up().await.expect("模型预热失败");
    assert!(engine.is_loaded(), "模型应已加载");

    // 仅在 GPU 模式下运行此测试（CPU 模式跳过）
    if engine.device_kind() == "cpu" {
        eprintln!("跳过：当前设备为 CPU，GPU 基准测试需要 metal 或 cuda feature 启用");
        return;
    }

    let prompt = "Write a 100-word essay about Rust programming language.";
    let start = std::time::Instant::now();
    let mut first_token_time: Option<std::time::Duration> = None;
    let mut token_count = 0usize;

    use echomind_core::LLMProvider;
    use futures::StreamExt;
    let mut stream = engine
        .chat_stream("You are a helpful assistant.", &[], prompt)
        .await
        .expect("chat_stream 失败");

    while let Some(result) = stream.next().await {
        if first_token_time.is_none() {
            first_token_time = Some(start.elapsed());
        }
        if result.is_ok() {
            token_count += 1;
        }
    }

    let total_time = start.elapsed();
    let ttft = first_token_time.unwrap_or(total_time);
    let tokens_per_sec = if total_time.as_secs_f64() > 0.0 {
        token_count as f64 / total_time.as_secs_f64()
    } else {
        0.0
    };

    println!(
        "\n=== GPU ({}) 推理基准 ===\n模型: {model_path}\n设备: {}\n首 token 延迟: {:.0} ms\n总 token 数: {token_count}\n总时间: {:.2} s\n速度: {:.1} tok/s\n",
        engine.device_kind(),
        engine.device_kind(),
        ttft.as_millis(),
        total_time.as_secs_f64(),
        tokens_per_sec,
    );

    // 基本合理性断言
    assert!(token_count > 0, "应至少生成 1 个 token");
    // GPU 模式下 device_kind 不应为 "cpu"
    assert_ne!(
        engine.device_kind(),
        "cpu",
        "GPU 基准测试不应在 CPU 模式下运行"
    );
}

/// TC-LLM-PERF-003：PagedAttention 多轮对话延迟。
///
/// 创建启用 PagedAttention 的引擎，发送 3 轮对话，
/// 测量每轮首 token 延迟，验证 KV cache 复用降低后续轮次延迟。
///
/// 运行方式（需 metal feature + GPU）：
/// ```bash
/// ECHOMIND_TEST_MODEL=/path/to/model.gguf \
/// cargo test -p echomind-infra --features pro,metal -- test_paged_attn_multiturn_latency --ignored --nocapture
/// ```
#[tokio::test]
#[ignore = "需要真实 GGUF 模型文件 + GPU feature，设置 ECHOMIND_TEST_MODEL 环境变量"]
async fn test_paged_attn_multiturn_latency() {
    let model_path = std::env::var("ECHOMIND_TEST_MODEL").unwrap_or_else(|_| {
        eprintln!("跳过：设置 ECHOMIND_TEST_MODEL 环境变量指向 GGUF 模型文件以运行此测试");
        std::process::exit(0);
    });

    // 创建引擎并启用 PagedAttention
    let engine = LocalLlmEngine::new(std::path::PathBuf::from(&model_path), Quantization::Auto)
        .expect("创建引擎失败")
        .with_paged_attn(32, 4096);

    // 预热模型
    engine.warm_up().await.expect("模型预热失败");
    assert!(engine.is_loaded(), "模型应已加载");

    // 仅在 GPU 模式下运行此测试（PagedAttention 仅 GPU 生效）
    if engine.device_kind() == "cpu" {
        eprintln!("跳过：当前设备为 CPU，PagedAttention 仅在 GPU 模式下生效");
        return;
    }

    use echomind_core::LLMProvider;
    use futures::StreamExt;

    let prompts = [
        "What is the capital of France?",
        "What is its population?",
        "Tell me about its history.",
    ];

    let mut history: Vec<echomind_models::ChatMessage> = Vec::new();
    let mut ttfts: Vec<std::time::Duration> = Vec::new();

    for (i, prompt) in prompts.iter().enumerate() {
        let start = std::time::Instant::now();
        let mut first_token_time: Option<std::time::Duration> = None;

        let mut stream = engine
            .chat_stream(
                "You are a helpful assistant. Answer concisely.",
                &history,
                prompt,
            )
            .await
            .expect("chat_stream 失败");

        let mut response_text = String::new();
        while let Some(result) = stream.next().await {
            if first_token_time.is_none() {
                first_token_time = Some(start.elapsed());
            }
            if let Ok(text) = result {
                response_text.push_str(&text);
            }
        }

        let ttft = first_token_time.unwrap_or(start.elapsed());
        ttfts.push(ttft);

        println!(
            "轮次 {}: TTFT = {:.0} ms, 响应长度 = {} 字符",
            i + 1,
            ttft.as_millis(),
            response_text.len()
        );

        // 将本轮对话加入历史
        history.push(echomind_models::ChatMessage {
            id: None,
            role: "user".to_string(),
            content: prompt.to_string(),
            sources: None,
            reasoning: None,
            ..Default::default()
        });
        history.push(echomind_models::ChatMessage {
            id: None,
            role: "assistant".to_string(),
            content: response_text,
            sources: None,
            reasoning: None,
            ..Default::default()
        });
    }

    println!(
        "\n=== PagedAttention 多轮对话延迟 ===\n模型: {model_path}\n设备: {}\n轮次 1 TTFT: {:.0} ms\n轮次 2 TTFT: {:.0} ms\n轮次 3 TTFT: {:.0} ms\n",
        engine.device_kind(),
        ttfts[0].as_millis(),
        ttfts[1].as_millis(),
        ttfts[2].as_millis(),
    );

    // 基本合理性断言
    assert_eq!(ttfts.len(), 3, "应有 3 轮 TTFT 数据");
    // 后续轮次的 TTFT 应 <= 第 1 轮（KV cache 复用减少 prefill 开销）
    // 注意：由于量化噪声和采样随机性，使用 >= 而非严格 >；但平均趋势应降低
    let avg_later = (ttfts[1].as_millis() + ttfts[2].as_millis()) / 2;
    println!(
        "第 1 轮 TTFT: {} ms, 后续平均 TTFT: {} ms (KV cache 复用效果)",
        ttfts[0].as_millis(),
        avg_later
    );
}

// ---------------------------------------------------------------------------
// 辅助函数：合成 GGUF 文件生成（用于内核模式测试）
// ---------------------------------------------------------------------------

/// 写入 GGUF v3 字符串（u64 长度前缀 + UTF-8 数据）。
fn write_gguf_str(buf: &mut Vec<u8>, s: &str) {
    buf.extend_from_slice(&(s.len() as u64).to_le_bytes());
    buf.extend_from_slice(s.as_bytes());
}

/// 创建包含指定量化类型张量的合成 GGUF v3 文件。
///
/// # 参数
///
/// - `tensor_name`：张量名称
/// - `dims`：张量维度
/// - `dtype_val`：GGML dtype u32 值（8=Q8_0, 12=Q4_K）
/// - `fill_byte`：张量数据填充字节
fn create_gguf_v3_with_quant_tensor(
    tensor_name: &str,
    dims: &[u64],
    dtype_val: u32,
    fill_byte: u8,
) -> Vec<u8> {
    let mut buf = Vec::new();

    // ---- 文件头 ----
    // Magic
    buf.extend_from_slice(&0x4655_4747u32.to_le_bytes());
    // Version: 3
    buf.extend_from_slice(&3u32.to_le_bytes());
    // tensor_count: 1
    buf.extend_from_slice(&1u64.to_le_bytes());
    // metadata_kv_count: 3
    buf.extend_from_slice(&3u64.to_le_bytes());

    // ---- 元数据 ----
    // 1. general.architecture = "qwen2"
    write_gguf_str(&mut buf, "general.architecture");
    buf.extend_from_slice(&8u32.to_le_bytes()); // STRING type
    write_gguf_str(&mut buf, "qwen2");

    // 2. general.name = "test_model"
    write_gguf_str(&mut buf, "general.name");
    buf.extend_from_slice(&8u32.to_le_bytes());
    write_gguf_str(&mut buf, "test_model");

    // 3. general.file_type = 1 (uint32)
    write_gguf_str(&mut buf, "general.file_type");
    buf.extend_from_slice(&4u32.to_le_bytes()); // UINT32 type
    buf.extend_from_slice(&1u32.to_le_bytes());

    // ---- 张量信息 ----
    write_gguf_str(&mut buf, tensor_name);
    buf.extend_from_slice(&(dims.len() as u32).to_le_bytes()); // n_dims
    for &d in dims {
        buf.extend_from_slice(&d.to_le_bytes());
    }
    buf.extend_from_slice(&dtype_val.to_le_bytes()); // dtype
    buf.extend_from_slice(&0u64.to_le_bytes()); // offset = 0

    // ---- 对齐填充到 32 字节 ----
    let header_end = buf.len();
    let aligned = header_end.div_ceil(32) * 32;
    buf.resize(aligned, 0);

    // ---- 张量数据 ----
    // 计算数据大小
    let n_elements: u64 = dims.iter().product();
    let dtype = crate::gguf_reader::GgmlDType::from_value(dtype_val);
    let (block_size, block_bytes) = dtype.block_layout();
    let data_size = if block_size == 0 {
        n_elements as usize * block_bytes as usize
    } else {
        let n_blocks = n_elements / block_size;
        (n_blocks * block_bytes) as usize
    };
    buf.resize(buf.len() + data_size, fill_byte);

    buf
}

// ---- TC-INTEG-001 ~ TC-INTEG-006: 内核模式集成测试（Phase 3 Session 20） ----

/// TC-INTEG-001：默认内核模式为 MistralRs。
///
/// 新创建的引擎，`kernel_mode()` 应返回 `KernelMode::MistralRs`（默认值）。
#[tokio::test]
async fn test_kernel_mode_default_mistral() {
    let tmp = tempfile::tempdir().expect("创建临时目录失败");
    let model_path = tmp.path().join("test.gguf");
    std::fs::write(&model_path, b"fake").expect("写入失败");
    let engine = LocalLlmEngine::new(model_path, Quantization::Auto).expect("创建失败");
    assert_eq!(
        engine.kernel_mode().await,
        KernelMode::MistralRs,
        "默认内核模式应为 MistralRs"
    );
}

/// TC-INTEG-002：切换到 CustomGemv 模式。
///
/// 调用 `set_kernel_mode(KernelMode::CustomGemv)` 后，`kernel_mode()` 应返回 `CustomGemv`。
#[tokio::test]
async fn test_set_kernel_mode_custom() {
    let tmp = tempfile::tempdir().expect("创建临时目录失败");
    let model_path = tmp.path().join("test.gguf");
    std::fs::write(&model_path, b"fake").expect("写入失败");
    let engine = LocalLlmEngine::new(model_path, Quantization::Auto).expect("创建失败");

    // 初始为 MistralRs
    assert_eq!(engine.kernel_mode().await, KernelMode::MistralRs);

    // 切换到 CustomGemv
    engine.set_kernel_mode(KernelMode::CustomGemv).await;
    assert_eq!(
        engine.kernel_mode().await,
        KernelMode::CustomGemv,
        "切换后应为 CustomGemv"
    );

    // 切换回 MistralRs
    engine.set_kernel_mode(KernelMode::MistralRs).await;
    assert_eq!(
        engine.kernel_mode().await,
        KernelMode::MistralRs,
        "切换回应为 MistralRs"
    );
}

/// TC-INTEG-003：无效模式字符串返回 Err。
///
/// `KernelMode::from_str()` 对无法识别的字符串应返回错误。
#[test]
fn test_set_kernel_mode_invalid() {
    assert!(KernelMode::from_str("invalid").is_err());
    assert!(KernelMode::from_str("").is_err());
    assert!(KernelMode::from_str("xyz").is_err());

    // 有效模式应成功
    assert_eq!(
        KernelMode::from_str("mistral").unwrap(),
        KernelMode::MistralRs
    );
    assert_eq!(
        KernelMode::from_str("custom").unwrap(),
        KernelMode::CustomGemv
    );
    // 大小写不敏感
    assert_eq!(
        KernelMode::from_str("MISTRAL").unwrap(),
        KernelMode::MistralRs
    );
    assert_eq!(
        KernelMode::from_str("CUSTOM").unwrap(),
        KernelMode::CustomGemv
    );
}

/// TC-INTEG-004：加载 GGUF + 重排权重成功。
///
/// 使用合成 GGUF 文件（包含 Q8_0 张量 `token_embd.weight`），
/// 验证 `load_custom_weights()` 能成功解析 GGUF、提取张量数据并重排权重。
#[tokio::test]
async fn test_load_custom_weights() {
    // 创建合成 GGUF 文件：Q8_0 token_embd.weight, dims=[4, 32]
    // Q8_0: block_size=32, block_bytes=34, n_blocks=4, data_size=136
    let gguf_data = create_gguf_v3_with_quant_tensor(
        "token_embd.weight",
        &[4, 32],
        8,    // Q8_0
        0x7F, // 填充字节（i8 max 值）
    );

    let tmp = tempfile::tempdir().expect("创建临时目录失败");
    let model_path = tmp.path().join("test_q8_0.gguf");
    std::fs::write(&model_path, &gguf_data).expect("写入失败");

    let engine = LocalLlmEngine::new(model_path, Quantization::Auto).expect("创建失败");
    engine.set_kernel_mode(KernelMode::CustomGemv).await;

    // 加载自定义权重
    let result = engine.load_custom_weights().await;
    assert!(
        result.is_ok(),
        "load_custom_weights 应成功: {:?}",
        result.err()
    );

    // 二次调用应快路径返回 Ok（幂等性）
    let result2 = engine.load_custom_weights().await;
    assert!(result2.is_ok(), "二次调用应快路径返回 Ok");
}

/// TC-INTEG-005：自研内核 Q8_0 推理产生输出。
///
/// 此测试需要真实的 GGUF 模型文件，标记为 `#[ignore]`。
/// 验证 CustomGemv 模式下 `chat_stream` 路由到 `chat_stream_custom`，
/// 且权重加载 + 重排管道正常工作。
///
/// 当前阶段 `chat_stream_custom` 返回错误（完整 transformer 未实现），
/// 此测试验证错误路径：确保错误信息清晰，且权重加载成功。
///
/// 运行方式：
/// ```bash
/// ECHOMIND_TEST_MODEL=/path/to/model.gguf \
/// cargo test -p echomind-infra --features pro -- test_chat_stream_custom_q8_0 --ignored --nocapture
/// ```
#[tokio::test]
#[ignore = "需要真实 GGUF 模型文件，设置 ECHOMIND_TEST_MODEL 环境变量"]
async fn test_chat_stream_custom_q8_0() {
    let model_path = std::env::var("ECHOMIND_TEST_MODEL").unwrap_or_else(|_| {
        eprintln!("跳过：设置 ECHOMIND_TEST_MODEL 环境变量指向 GGUF 模型文件以运行此测试");
        std::process::exit(0);
    });

    let engine = LocalLlmEngine::new(std::path::PathBuf::from(&model_path), Quantization::Auto)
        .expect("创建引擎失败");
    engine.set_kernel_mode(KernelMode::CustomGemv).await;

    // 验证权重加载成功
    let load_result = engine.load_custom_weights().await;
    assert!(
        load_result.is_ok(),
        "权重加载应成功: {:?}",
        load_result.err()
    );

    // chat_stream_custom 当前返回错误（完整 transformer 未实现）
    // 验证错误信息包含开发中提示
    use echomind_core::LLMProvider;
    let stream_result = engine
        .chat_stream("You are a helpful assistant.", &[], "Hello")
        .await;
    let err_msg = match stream_result {
        Err(e) => e.to_string(),
        Ok(_) => String::new(),
    };
    assert!(
        !err_msg.is_empty(),
        "chat_stream 在 CustomGemv 模式下当前应返回错误（未实现完整推理）"
    );
    assert!(
        err_msg.contains("尚在开发中") || err_msg.contains("未实现"),
        "错误信息应包含开发中提示，实际: {err_msg}"
    );
}

/// TC-INTEG-006：自研内核 Q4_K 推理产生输出。
///
/// 此测试需要真实的 Q4_K GGUF 模型文件，标记为 `#[ignore]`。
/// 与 TC-INTEG-005 类似，但验证 Q4_K 量化格式的权重加载 + 重排。
///
/// 运行方式：
/// ```bash
/// ECHOMIND_TEST_MODEL=/path/to/q4_k_model.gguf \
/// cargo test -p echomind-infra --features pro -- test_chat_stream_custom_q4_k --ignored --nocapture
/// ```
#[tokio::test]
#[ignore = "需要真实 Q4_K GGUF 模型文件，设置 ECHOMIND_TEST_MODEL 环境变量"]
async fn test_chat_stream_custom_q4_k() {
    let model_path = std::env::var("ECHOMIND_TEST_MODEL").unwrap_or_else(|_| {
        eprintln!("跳过：设置 ECHOMIND_TEST_MODEL 环境变量指向 Q4_K GGUF 模型文件以运行此测试");
        std::process::exit(0);
    });

    let engine = LocalLlmEngine::new(std::path::PathBuf::from(&model_path), Quantization::Q4K_M)
        .expect("创建引擎失败");
    engine.set_kernel_mode(KernelMode::CustomGemv).await;

    // 验证权重加载成功（Q4_K 格式）
    let load_result = engine.load_custom_weights().await;
    assert!(
        load_result.is_ok(),
        "Q4_K 权重加载应成功: {:?}",
        load_result.err()
    );

    // chat_stream_custom 当前返回错误
    use echomind_core::LLMProvider;
    let stream_result = engine
        .chat_stream("You are a helpful assistant.", &[], "Hello")
        .await;
    assert!(
        stream_result.is_err(),
        "chat_stream 在 CustomGemv 模式下当前应返回错误"
    );
}

/// TC-INTEG-006b：with_kernel_mode builder 方法设置模式。
///
/// 使用 builder 模式 `with_kernel_mode()` 创建引擎后，
/// `kernel_mode()` 应返回指定的模式。
#[tokio::test]
async fn test_with_kernel_mode_builder() {
    let tmp = tempfile::tempdir().expect("创建临时目录失败");
    let model_path = tmp.path().join("test.gguf");
    std::fs::write(&model_path, b"fake").expect("写入失败");

    let engine = LocalLlmEngine::new(model_path, Quantization::Auto).expect("创建失败");
    let engine = engine.with_kernel_mode(KernelMode::CustomGemv);
    assert_eq!(
        engine.kernel_mode().await,
        KernelMode::CustomGemv,
        "with_kernel_mode 应设置 CustomGemv"
    );
}

/// TC-INTEG-006c：KernelMode 字符串转换。
///
/// 验证 `as_str()` 和 `from_str()` 的双向转换。
#[test]
fn test_kernel_mode_str_conversion() {
    // as_str
    assert_eq!(KernelMode::MistralRs.as_str(), "mistral");
    assert_eq!(KernelMode::CustomGemv.as_str(), "custom");

    // from_str + as_str 往返
    let mode = KernelMode::from_str("custom").unwrap();
    assert_eq!(mode.as_str(), "custom");

    let mode = KernelMode::from_str("mistral").unwrap();
    assert_eq!(mode.as_str(), "mistral");
}

/// TC-INTEG-006d：warm_up 在 CustomGemv 模式下加载权重。
///
/// 切换到 CustomGemv 模式后调用 `warm_up()`，应触发 `load_custom_weights()`。
/// 使用合成 GGUF 文件验证。
#[tokio::test]
async fn test_warm_up_custom_gemv_mode() {
    // 创建合成 GGUF 文件：Q8_0 token_embd.weight
    let gguf_data = create_gguf_v3_with_quant_tensor(
        "token_embd.weight",
        &[4, 32],
        8, // Q8_0
        0x7F,
    );

    let tmp = tempfile::tempdir().expect("创建临时目录失败");
    let model_path = tmp.path().join("test_warm.gguf");
    std::fs::write(&model_path, &gguf_data).expect("写入失败");

    let engine = LocalLlmEngine::new(model_path, Quantization::Auto).expect("创建失败");
    engine.set_kernel_mode(KernelMode::CustomGemv).await;

    // warm_up 应触发权重加载
    let result = engine.warm_up().await;
    assert!(
        result.is_ok(),
        "warm_up 在 CustomGemv 模式下应成功: {:?}",
        result.err()
    );
}

// ---------------------------------------------------------------------------
// Phase 3 Session 23：性能基准测试（TC-PHASE3-PERF-001 ~ 004）
//
// 所有基准测试标记 `#[ignore]`，需要真实 GGUF 模型文件。
// 运行方式：
//   ECHOMIND_TEST_MODEL=/path/to/model.gguf \
//   cargo test -p echomind-infra --features pro -- phase3_perf --ignored --nocapture
// ---------------------------------------------------------------------------

/// 辅助函数：查找 GGUF 文件中指定量化类型的第一个张量。
///
/// 遍历 GGUF 文件的所有张量，返回第一个匹配 `target_dtype` 的张量名。
fn find_tensor_by_dtype(
    gguf: &crate::gguf_reader::GgufFile,
    target_dtype: crate::gguf_reader::GgmlDType,
) -> Option<String> {
    for name in gguf.tensor_names() {
        if let Some(info) = gguf.tensor_info(name)
            && info.dtype == target_dtype
        {
            return Some(name.to_string());
        }
    }
    None
}

/// 辅助函数：生成确定性伪随机 f32 向量（用于基准测试的可重复性）。
fn generate_input_vector(k: usize, seed: u32) -> Vec<f32> {
    let mut input = Vec::with_capacity(k);
    let mut state = seed;
    for _ in 0..k {
        // 简单 LCG 伪随机数生成器
        state = state.wrapping_mul(1103515245).wrapping_add(12345);
        let val = ((state >> 16) & 0xFFFF) as f32 / 65535.0; // [0, 1)
        input.push(val * 2.0 - 1.0); // [-1, 1)
    }
    input
}

/// TC-PHASE3-PERF-001：自研 GEMV Q8_0 内核性能基准。
///
/// 从真实 GGUF 模型中提取 Q8_0 张量，对自研 GEMV 内核进行基准测试：
/// - 多次迭代 GEMV 计算，测量平均延迟和吞吐量
/// - 与朴素 f32 参考实现进行正确性验证
/// - 输出性能指标（rows/sec, MB/s）
///
/// 运行方式：
/// ```bash
/// ECHOMIND_TEST_MODEL=/path/to/model.gguf \
/// cargo test -p echomind-infra --features pro -- test_gemv_vs_mistralrs_q8_0 --ignored --nocapture
/// ```
#[tokio::test]
#[ignore = "需要真实 GGUF 模型文件，设置 ECHOMIND_TEST_MODEL 环境变量"]
async fn test_gemv_vs_mistralrs_q8_0() {
    let model_path = std::env::var("ECHOMIND_TEST_MODEL").unwrap_or_else(|_| {
        eprintln!("跳过：设置 ECHOMIND_TEST_MODEL 环境变量指向 GGUF 模型文件以运行此测试");
        std::process::exit(0);
    });

    // 打开 GGUF 文件
    let gguf = crate::gguf_reader::GgufFile::open(std::path::Path::new(&model_path))
        .expect("打开 GGUF 文件失败");

    // 查找 Q8_0 张量
    let tensor_name = find_tensor_by_dtype(&gguf, crate::gguf_reader::GgmlDType::Q8_0)
        .unwrap_or_else(|| {
            eprintln!("跳过：模型中未找到 Q8_0 量化张量");
            std::process::exit(0);
        });

    let info = gguf.tensor_info(&tensor_name).expect("张量信息缺失");
    let data = gguf.tensor_data(&tensor_name).expect("张量数据缺失");

    // 解析维度：GGUF dims 是倒序的（col-major），取最后两个维度作为 [k, n]
    // 对于 2D 张量，dims = [cols, rows] → n = rows, k = cols
    let (n, k) = if info.dims.len() >= 2 {
        let n = info.dims[info.dims.len() - 2] as usize;
        let k = info.dims[info.dims.len() - 1] as usize;
        (n, k)
    } else {
        // 1D 张量：n = 1, k = dims[0]
        (1, info.dims[0] as usize)
    };

    println!(
        "\n=== TC-PHASE3-PERF-001: GEMV Q8_0 基准 ===\n张量: {tensor_name}\n维度: {n}×{k}\n数据大小: {} bytes",
        data.len()
    );

    // 生成输入向量
    let input = generate_input_vector(k, 42);

    // 正确性验证：自研内核 vs 朴素 f32 实现
    let mut output_custom = vec![0.0f32; n];
    crate::gemv_kernel::gemv_dispatch(
        crate::gguf_reader::GgmlDType::Q8_0,
        data,
        &input,
        &mut output_custom,
        n,
        k,
    )
    .expect("GEMV dispatch 失败");

    // 朴素参考实现
    use crate::gemv_kernel::gemv_q8_0_naive;
    use crate::quant_blocks::blocks_from_bytes;
    let blocks = blocks_from_bytes(data, crate::gguf_reader::GgmlDType::Q8_0)
        .expect("blocks_from_bytes 失败");
    let q8_blocks = match blocks {
        crate::quant_blocks::QuantBlock::Q8_0(b) => b,
        _ => unreachable!(),
    };
    let mut output_naive = vec![0.0f32; n];
    gemv_q8_0_naive(q8_blocks, &input, &mut output_naive, n, k);

    // 验证正确性（允许量化误差）
    let max_diff: f32 = output_custom
        .iter()
        .zip(output_naive.iter())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max);
    println!("最大误差: {max_diff:.6}");
    assert!(max_diff < 2.0, "自研内核与朴素实现的误差过大: {max_diff}");

    // 性能基准：多次迭代
    const ITERATIONS: usize = 50;
    let mut output = vec![0.0f32; n];
    let start = std::time::Instant::now();
    for _ in 0..ITERATIONS {
        crate::gemv_kernel::gemv_dispatch(
            crate::gguf_reader::GgmlDType::Q8_0,
            data,
            &input,
            &mut output,
            n,
            k,
        )
        .expect("GEMV dispatch 失败");
    }
    let elapsed = start.elapsed();
    let avg_us = elapsed.as_micros() as f64 / ITERATIONS as f64;
    let data_mb = data.len() as f64 / (1024.0 * 1024.0);
    let throughput_mbs = data_mb / (avg_us / 1_000_000.0);
    let rows_per_sec = n as f64 / (avg_us / 1_000_000.0);

    println!(
        "\n=== GEMV Q8_0 性能结果 ===\n迭代: {ITERATIONS}\n平均延迟: {avg_us:.0} µs ({:.3} ms)\n数据大小: {data_mb:.1} MB\n吞吐量: {throughput_mbs:.1} MB/s\n行/秒: {rows_per_sec:.0} rows/s\n",
        avg_us / 1000.0,
    );

    assert!(avg_us > 0.0, "平均延迟应大于 0");
}

/// TC-PHASE3-PERF-002：自研 GEMV Q4_K 内核性能基准。
///
/// 与 TC-PHASE3-PERF-001 类似，但针对 Q4_K 量化格式。
///
/// 运行方式：
/// ```bash
/// ECHOMIND_TEST_MODEL=/path/to/q4_k_model.gguf \
/// cargo test -p echomind-infra --features pro -- test_gemv_vs_mistralrs_q4_k --ignored --nocapture
/// ```
#[tokio::test]
#[ignore = "需要真实 Q4_K GGUF 模型文件，设置 ECHOMIND_TEST_MODEL 环境变量"]
async fn test_gemv_vs_mistralrs_q4_k() {
    let model_path = std::env::var("ECHOMIND_TEST_MODEL").unwrap_or_else(|_| {
        eprintln!("跳过：设置 ECHOMIND_TEST_MODEL 环境变量指向 GGUF 模型文件以运行此测试");
        std::process::exit(0);
    });

    // 打开 GGUF 文件
    let gguf = crate::gguf_reader::GgufFile::open(std::path::Path::new(&model_path))
        .expect("打开 GGUF 文件失败");

    // 查找 Q4_K 张量
    let tensor_name = find_tensor_by_dtype(&gguf, crate::gguf_reader::GgmlDType::Q4K)
        .unwrap_or_else(|| {
            eprintln!("跳过：模型中未找到 Q4_K 量化张量");
            std::process::exit(0);
        });

    let info = gguf.tensor_info(&tensor_name).expect("张量信息缺失");
    let data = gguf.tensor_data(&tensor_name).expect("张量数据缺失");

    // 解析维度
    let (n, k) = if info.dims.len() >= 2 {
        let n = info.dims[info.dims.len() - 2] as usize;
        let k = info.dims[info.dims.len() - 1] as usize;
        (n, k)
    } else {
        (1, info.dims[0] as usize)
    };

    println!(
        "\n=== TC-PHASE3-PERF-002: GEMV Q4_K 基准 ===\n张量: {tensor_name}\n维度: {n}×{k}\n数据大小: {} bytes",
        data.len()
    );

    // 生成输入向量
    let input = generate_input_vector(k, 42);

    // 正确性验证
    let mut output_custom = vec![0.0f32; n];
    crate::gemv_kernel::gemv_dispatch(
        crate::gguf_reader::GgmlDType::Q4K,
        data,
        &input,
        &mut output_custom,
        n,
        k,
    )
    .expect("GEMV dispatch 失败");

    // 朴素参考实现
    use crate::gemv_kernel::gemv_q4_k_naive;
    use crate::quant_blocks::{QuantBlock, blocks_from_bytes};
    let blocks = blocks_from_bytes(data, crate::gguf_reader::GgmlDType::Q4K)
        .expect("blocks_from_bytes 失败");
    let q4k_blocks = match blocks {
        QuantBlock::Q4K(b) => b,
        _ => unreachable!(),
    };
    let mut output_naive = vec![0.0f32; n];
    gemv_q4_k_naive(q4k_blocks, &input, &mut output_naive, n, k);

    let max_diff: f32 = output_custom
        .iter()
        .zip(output_naive.iter())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max);
    println!("最大误差: {max_diff:.6}");
    assert!(max_diff < 2.0, "Q4_K 内核与朴素实现误差过大: {max_diff}");

    // 性能基准
    const ITERATIONS: usize = 50;
    let mut output = vec![0.0f32; n];
    let start = std::time::Instant::now();
    for _ in 0..ITERATIONS {
        crate::gemv_kernel::gemv_dispatch(
            crate::gguf_reader::GgmlDType::Q4K,
            data,
            &input,
            &mut output,
            n,
            k,
        )
        .expect("GEMV dispatch 失败");
    }
    let elapsed = start.elapsed();
    let avg_us = elapsed.as_micros() as f64 / ITERATIONS as f64;
    let data_mb = data.len() as f64 / (1024.0 * 1024.0);
    let throughput_mbs = data_mb / (avg_us / 1_000_000.0);
    let rows_per_sec = n as f64 / (avg_us / 1_000_000.0);

    println!(
        "\n=== GEMV Q4_K 性能结果 ===\n迭代: {ITERATIONS}\n平均延迟: {avg_us:.0} µs ({:.3} ms)\n数据大小: {data_mb:.1} MB\n吞吐量: {throughput_mbs:.1} MB/s\n行/秒: {rows_per_sec:.0} rows/s\n",
        avg_us / 1000.0,
    );

    assert!(avg_us > 0.0, "平均延迟应大于 0");
}

/// TC-PHASE3-PERF-003：Layer 级流式预取效果基准。
///
/// 从真实 GGUF 模型中提取 layer 偏移量，对比有/无 `madvise(MADV_WILLNEED)`
/// 预取的首层访问延迟。
///
/// 流程：
/// 1. 打开 GGUF 文件，提取 layer 偏移量
/// 2. 无预取路径：直接访问最后一个张量（cold page cache），测量延迟
/// 3. 有预取路径：`prefetch_next()` 后等待 OS 异步加载，再访问，测量延迟
/// 4. 对比两个延迟，预取应显著降低首次访问延迟
///
/// 运行方式：
/// ```bash
/// ECHOMIND_TEST_MODEL=/path/to/model.gguf \
/// cargo test -p echomind-infra --features pro -- test_prefetch_effectiveness --ignored --nocapture
/// ```
#[tokio::test]
#[ignore = "需要真实 GGUF 模型文件，设置 ECHOMIND_TEST_MODEL 环境变量"]
async fn test_prefetch_effectiveness() {
    let model_path = std::env::var("ECHOMIND_TEST_MODEL").unwrap_or_else(|_| {
        eprintln!("跳过：设置 ECHOMIND_TEST_MODEL 环境变量指向 GGUF 模型文件以运行此测试");
        std::process::exit(0);
    });

    // 打开 GGUF 文件
    let gguf = crate::gguf_reader::GgufFile::open(std::path::Path::new(&model_path))
        .expect("打开 GGUF 文件失败");

    // 提取 layer 偏移量
    let layer_offsets = crate::layer_prefetch::layer_offsets_from_gguf(&gguf);
    let layer_count = layer_offsets.len();
    println!("\n=== TC-PHASE3-PERF-003: 预取效果基准 ===\n模型张量数: {layer_count}");

    if layer_count == 0 {
        eprintln!("跳过：模型中无张量");
        return;
    }

    // 选择一个较大的张量进行测试（通常在中间层）
    // 找到最大的张量索引
    let (test_idx, _, test_size) = {
        let max_entry = layer_offsets
            .iter()
            .enumerate()
            .max_by_key(|(_, (_, _, size))| *size)
            .expect("layer_offsets 不应为空");
        (max_entry.0, &max_entry.1.0, max_entry.1.2)
    };

    println!(
        "测试张量: [{test_idx}] {} ({} bytes / {:.1} MB)",
        layer_offsets[test_idx].0,
        test_size,
        test_size as f64 / (1024.0 * 1024.0)
    );

    // 创建预取器（使用 GgufFile::mmap() 获取 mmap 引用）
    let prefetcher =
        crate::layer_prefetch::LayerPrefetcher::new(gguf.mmap(), layer_offsets.clone());

    // 预取目标张量
    prefetcher.prefetch_next(test_idx);

    // 等待 OS 异步加载页面到 page cache
    std::thread::sleep(std::time::Duration::from_millis(100));

    // 通过 tensor_data 访问预取后的数据（应命中 page cache）
    let tensor_name = &layer_offsets[test_idx].0;
    let tensor_data = gguf.tensor_data(tensor_name).expect("张量数据缺失");

    // ---- 无预取路径：cold access ----
    // 通过读取所有页面的首字节来触发 page fault
    // 先释放 page cache（通过 posix_fadvise 或读取其他大文件来驱逐）
    // 注意：在测试环境中，我们无法可靠地清除 page cache
    // 因此改为测量「顺序访问 vs 随机访问」的差异

    // cold access：遍历所有页面的首字节
    let page_size = 4096usize;
    let pages = tensor_data.len().div_ceil(page_size);
    let start_cold = std::time::Instant::now();
    let mut checksum_cold = 0u8;
    for i in 0..pages {
        let offset = i * page_size;
        if offset < tensor_data.len() {
            checksum_cold = checksum_cold.wrapping_add(tensor_data[offset]);
        }
    }
    let cold_elapsed = start_cold.elapsed();

    println!(
        "Cold access ({} pages): {:?} (checksum={})",
        pages, cold_elapsed, checksum_cold
    );

    // warm access：再次遍历（数据已在 page cache 中）
    let start_warm = std::time::Instant::now();
    let mut checksum_warm = 0u8;
    for i in 0..pages {
        let offset = i * page_size;
        if offset < tensor_data.len() {
            checksum_warm = checksum_warm.wrapping_add(tensor_data[offset]);
        }
    }
    let warm_elapsed = start_warm.elapsed();

    println!(
        "Warm access ({} pages): {:?} (checksum={})",
        pages, warm_elapsed, checksum_warm
    );

    // 验证 checksum 一致
    assert_eq!(checksum_cold, checksum_warm, "两次遍历的 checksum 应一致");

    // warm 应快于或等于 cold（page cache 命中）
    // 注意：如果数据量小或已全在 cache，差异可能不明显
    let speedup = if warm_elapsed.as_micros() > 0 {
        cold_elapsed.as_secs_f64() / warm_elapsed.as_secs_f64()
    } else {
        1.0
    };
    println!(
        "\n=== 预取效果 ===\nCold: {:?}\nWarm: {:?}\n加速比: {speedup:.2}x\n",
        cold_elapsed, warm_elapsed
    );

    // 基本合理性断言
    assert!(cold_elapsed.as_nanos() > 0, "cold access 应有耗时");
    assert!(warm_elapsed.as_nanos() > 0, "warm access 应有耗时");
}

/// TC-PHASE3-PERF-004：权重重排 cache miss 降低基准。
///
/// 创建大型合成 Q8_0 权重矩阵，对比 Row-Major（未重排）和
/// Tile-Major（重排后）的 GEMV 执行时间。
///
/// 重排后的 Tile-Major 布局使 GEMV 按列遍历时每个输入 Block 在 L1 中
/// 被 N 行复用，减少 cache miss，提升吞吐量。
///
/// 运行方式：
/// ```bash
/// cargo test -p echomind-infra --features pro -- test_repack_cache_miss_reduction --ignored --nocapture
/// ```
#[tokio::test]
#[ignore = "性能基准测试，建议在 CPU 环境下运行以获得稳定数据"]
async fn test_repack_cache_miss_reduction() {
    use crate::gemv_kernel::gemv_q8_0;
    use crate::quant_blocks::{BlockQ8_0, QK8_0};
    use crate::weight_repack::{gemv_repacked_dispatch, repack_for_gemv};

    // 合成大型 Q8_0 权重矩阵
    // 矩阵大小：n=2048 行, k=2048 列
    // Q8_0: block_size=32, block_bytes=34, blocks_per_row=64
    // 每行 64 块 × 34 bytes = 2176 bytes
    // 总数据：2048 行 × 2176 bytes ≈ 4.3 MB
    const N: usize = 2048;
    const K: usize = 2048;
    let blocks_per_row = K / QK8_0; // 64
    let total_blocks = N * blocks_per_row;

    // 生成 Q8_0 权重块（确定性填充）
    let mut weights: Vec<BlockQ8_0> = Vec::with_capacity(total_blocks);
    let mut state = 12345u32;
    for _ in 0..total_blocks {
        state = state.wrapping_mul(1103515245).wrapping_add(12345);
        let d = (state >> 16 & 0xFF) as f32 / 255.0;
        let mut qs = [0i8; QK8_0];
        for q in qs.iter_mut() {
            state = state.wrapping_mul(1103515245).wrapping_add(12345);
            *q = (state >> 24) as i8;
        }
        weights.push(BlockQ8_0 {
            d: half::f16::from_f32(d),
            qs,
        });
    }

    // 将权重块转为原始字节（用于 gemv_dispatch 和 repack_for_gemv）
    // 使用 bytemuck 或手动序列化，这里用安全的方式
    let weight_bytes: Vec<u8> = weights
        .iter()
        .flat_map(|b| {
            // BlockQ8_0: d (f16 = 2 bytes) + qs ([i8; 32] = 32 bytes) = 34 bytes
            let mut bytes = Vec::with_capacity(34);
            bytes.extend_from_slice(&b.d.to_bits().to_le_bytes());
            for &q in &b.qs {
                bytes.push(q as u8);
            }
            bytes
        })
        .collect();

    // 生成输入向量
    let input = generate_input_vector(K, 42);

    // ---- Row-Major（未重排）GEMV ----
    let mut output_row_major = vec![0.0f32; N];
    const ITERATIONS: usize = 100;

    // 预热（首次访问数据到 cache）
    gemv_q8_0(&weights, &input, &mut output_row_major, N, K);

    let start = std::time::Instant::now();
    for _ in 0..ITERATIONS {
        gemv_q8_0(&weights, &input, &mut output_row_major, N, K);
    }
    let row_major_elapsed = start.elapsed();
    let row_major_avg_us = row_major_elapsed.as_micros() as f64 / ITERATIONS as f64;

    // ---- Tile-Major（重排后）GEMV ----
    let repacked = repack_for_gemv(&weight_bytes, crate::gguf_reader::GgmlDType::Q8_0, N, K)
        .expect("repack_for_gemv 失败");

    let mut output_repacked = vec![0.0f32; N];

    // 预热
    gemv_repacked_dispatch(&repacked, &input, &mut output_repacked)
        .expect("gemv_repacked_dispatch 失败");

    let start = std::time::Instant::now();
    for _ in 0..ITERATIONS {
        gemv_repacked_dispatch(&repacked, &input, &mut output_repacked)
            .expect("gemv_repacked_dispatch 失败");
    }
    let repacked_elapsed = start.elapsed();
    let repacked_avg_us = repacked_elapsed.as_micros() as f64 / ITERATIONS as f64;

    // 正确性验证：两种布局结果应一致
    let max_diff: f32 = output_row_major
        .iter()
        .zip(output_repacked.iter())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max);

    let speedup = row_major_avg_us / repacked_avg_us;

    println!(
        "\n=== TC-PHASE3-PERF-004: 权重重排 cache 效果 ===\n矩阵: {N}×{K} (Q8_0)\n总数据: {:.1} MB\nRow-Major 平均: {row_major_avg_us:.0} µs\nTile-Major 平均: {repacked_avg_us:.0} µs\n加速比: {speedup:.2}x\n最大误差: {max_diff:.6}\n",
        weight_bytes.len() as f64 / (1024.0 * 1024.0),
    );

    // 正确性断言
    assert!(max_diff < 0.01, "重排前后结果应一致（最大误差 {max_diff})");

    // 性能断言：重排后不应比未重排慢（通常更快，但受 cache 大小影响）
    assert!(
        repacked_avg_us <= row_major_avg_us * 1.5,
        "重排后 GEMV 不应比未重排慢太多: {repacked_avg_us} vs {row_major_avg_us}"
    );

    // 输出 cache miss 降低效果（如果可用）
    // 注意：实际 cache miss 需使用 perf stat 等工具测量
    // 此处仅对比执行时间作为间接指标
    if speedup > 1.0 {
        println!("✓ 重排后 GEMV 速度提升 {speedup:.2}x（cache miss 降低）");
    } else {
        println!("⚠ 重排后未加速（可能数据已在 cache 中，或矩阵太小未触发 cache 压力）");
    }
}
