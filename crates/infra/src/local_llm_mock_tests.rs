#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

//! LocalLlmEngine 状态机 Mock 测试（Phase 3 CI 自动化正确性测试）。
//!
//! 本模块测试 `LocalLlmEngine` 的状态机转换，不依赖真实 GGUF 模型文件：
//! 1. KernelMode 解析与切换（`set_kernel_mode` / `with_kernel_mode` / `kernel_mode`）
//! 2. CustomGemv 模式 `load_custom_weights` 使用合成 GGUF 文件
//! 3. CustomGemv 模式 `warm_up` 使用合成 GGUF 文件
//! 4. KV cache 保存/恢复基本流程
//! 5. 状态机综合转换测试
//!
//! 所有测试在 CI 中自动运行（非 `#[ignore]`），使用合成 GGUF fixture。

use super::local_llm::*;
use echomind_models::LlmSamplingParams;

// 从 synthetic_gguf_tests 导入共享 fixture 生成器
use super::synthetic_gguf_tests::{
    GgufVersion, TensorSpec, create_synthetic_gguf, make_q8_0_bytes, write_temp_file,
};
use crate::gguf_reader::GgmlDType;
use crate::quant_blocks::QK8_0;

// ===========================================================================
// 辅助函数
// ===========================================================================

/// 创建包含有效 `token_embd.weight` Q8_0 张量的合成 GGUF 临时文件。
///
/// 用于 CustomGemv 模式测试，`load_custom_weights` 会打开此文件并执行
/// `repack_for_gemv` 重排 `token_embd.weight` 张量。
fn create_synthetic_model_file() -> tempfile::NamedTempFile {
    let n = 4;
    let k = QK8_0; // 32
    let weight_data: Vec<f32> = (0..n * k)
        .map(|i| ((i * 7 + 3) % 17) as f32 - 8.0)
        .collect();
    let tensor_bytes = make_q8_0_bytes(&weight_data, n, k);

    let tensors = vec![TensorSpec {
        name: "token_embd.weight".to_string(),
        dims: vec![n as u64, k as u64],
        dtype: GgmlDType::Q8_0,
        data: tensor_bytes,
    }];
    let gguf_data = create_synthetic_gguf(GgufVersion::V3, &tensors);
    write_temp_file(&gguf_data)
}

/// 创建一个假模型文件（内容为无效数据，仅用于 `LocalLlmEngine::new` 的文件存在检查）。
fn create_fake_model_file() -> tempfile::NamedTempFile {
    let mut tmp = tempfile::NamedTempFile::new().expect("创建临时文件失败");
    std::io::Write::write_all(&mut tmp, b"fake gguf content").expect("写入失败");
    tmp
}

// ===========================================================================
// 步骤 5a：KernelMode 解析与状态切换
// ===========================================================================

// ---- TC-LLM-MOCK-001 ~ 005: KernelMode 基础 ----

/// TC-LLM-MOCK-001：`KernelMode::default()` 返回 `MistralRs`。
#[test]
fn test_mock_kernel_mode_default() {
    let mode = KernelMode::default();
    assert_eq!(mode, KernelMode::MistralRs, "默认应为 MistralRs");
}

/// TC-LLM-MOCK-002：`KernelMode::from_str("mistral")` 返回 Ok(MistralRs)。
#[test]
fn test_mock_kernel_mode_from_str_mistral() {
    let result = KernelMode::from_str("mistral");
    assert!(result.is_ok(), "应解析成功");
    assert_eq!(result.unwrap(), KernelMode::MistralRs);
}

/// TC-LLM-MOCK-003：`KernelMode::from_str("custom")` 返回 Ok(CustomGemv)。
#[test]
fn test_mock_kernel_mode_from_str_custom() {
    let result = KernelMode::from_str("custom");
    assert!(result.is_ok(), "应解析成功");
    assert_eq!(result.unwrap(), KernelMode::CustomGemv);
}

/// TC-LLM-MOCK-004：`KernelMode::from_str("invalid")` 返回 Err。
#[test]
fn test_mock_kernel_mode_from_str_invalid() {
    let result = KernelMode::from_str("invalid");
    assert!(result.is_err(), "无效字符串应返回错误");
}

/// TC-LLM-MOCK-005：`KernelMode::as_str()` 返回正确字符串。
#[test]
fn test_mock_kernel_mode_as_str() {
    assert_eq!(KernelMode::MistralRs.as_str(), "mistral");
    assert_eq!(KernelMode::CustomGemv.as_str(), "custom");
}

// ---- TC-LLM-MOCK-006 ~ 009: 引擎 KernelMode 读写 ----

/// TC-LLM-MOCK-006：新引擎 `kernel_mode()` 默认为 MistralRs。
#[tokio::test]
async fn test_mock_engine_default_kernel_mode() {
    let tmp = create_fake_model_file();
    let engine =
        LocalLlmEngine::new(tmp.path().to_path_buf(), Quantization::Auto).expect("创建引擎失败");
    let mode = engine.kernel_mode().await;
    assert_eq!(mode, KernelMode::MistralRs, "默认应为 MistralRs");
}

/// TC-LLM-MOCK-007：`set_kernel_mode` 后 `kernel_mode()` 返回新值。
#[tokio::test]
async fn test_mock_set_kernel_mode() {
    let tmp = create_fake_model_file();
    let engine =
        LocalLlmEngine::new(tmp.path().to_path_buf(), Quantization::Auto).expect("创建引擎失败");

    engine.set_kernel_mode(KernelMode::CustomGemv).await;
    let mode = engine.kernel_mode().await;
    assert_eq!(mode, KernelMode::CustomGemv, "应为 CustomGemv");

    engine.set_kernel_mode(KernelMode::MistralRs).await;
    let mode = engine.kernel_mode().await;
    assert_eq!(mode, KernelMode::MistralRs, "应切回 MistralRs");
}

/// TC-LLM-MOCK-008：`with_kernel_mode` builder 模式设置正确。
#[tokio::test]
async fn test_mock_with_kernel_mode_builder() {
    let tmp = create_fake_model_file();
    let engine =
        LocalLlmEngine::new(tmp.path().to_path_buf(), Quantization::Auto).expect("创建引擎失败");
    let engine = engine.with_kernel_mode(KernelMode::CustomGemv);

    let mode = engine.kernel_mode().await;
    assert_eq!(mode, KernelMode::CustomGemv, "builder 应设置 CustomGemv");
}

/// TC-LLM-MOCK-009：状态转换 MistralRs → CustomGemv → MistralRs 循环正确。
#[tokio::test]
async fn test_mock_kernel_mode_state_transition_cycle() {
    let tmp = create_fake_model_file();
    let engine =
        LocalLlmEngine::new(tmp.path().to_path_buf(), Quantization::Auto).expect("创建引擎失败");

    // 初始：MistralRs
    assert_eq!(engine.kernel_mode().await, KernelMode::MistralRs);

    // 切换到 CustomGemv
    engine.set_kernel_mode(KernelMode::CustomGemv).await;
    assert_eq!(engine.kernel_mode().await, KernelMode::CustomGemv);

    // 切换到 CustomGemv（重复设置，幂等）
    engine.set_kernel_mode(KernelMode::CustomGemv).await;
    assert_eq!(engine.kernel_mode().await, KernelMode::CustomGemv);

    // 切换回 MistralRs
    engine.set_kernel_mode(KernelMode::MistralRs).await;
    assert_eq!(engine.kernel_mode().await, KernelMode::MistralRs);
}

// ===========================================================================
// 步骤 5b：CustomGemv 模式 load_custom_weights 使用合成 GGUF
// ===========================================================================

/// TC-LLM-MOCK-010：CustomGemv 模式 `load_custom_weights` 合成 GGUF 成功。
///
/// 创建包含有效 `token_embd.weight` Q8_0 张量的合成 GGUF 文件，
/// 切换到 CustomGemv 模式后调用 `load_custom_weights`，应成功返回 Ok(())。
#[tokio::test]
async fn test_mock_load_custom_weights_synthetic_gguf() {
    let model_file = create_synthetic_model_file();
    let engine = LocalLlmEngine::new(model_file.path().to_path_buf(), Quantization::Auto)
        .expect("创建引擎失败");

    engine.set_kernel_mode(KernelMode::CustomGemv).await;

    // 加载自研内核权重（GGUF 解析 + repack_for_gemv）
    let result = engine.load_custom_weights().await;
    assert!(
        result.is_ok(),
        "load_custom_weights 应成功: {:?}",
        result.err()
    );

    // 模型本身未加载到 mistral.rs（is_loaded 仅检查 mistral.rs runner）
    assert!(!engine.is_loaded(), "mistral.rs 模型不应被加载");
}

/// TC-LLM-MOCK-011：CustomGemv 模式 `warm_up` 合成 GGUF 成功。
///
/// `warm_up` 在 CustomGemv 模式下路由到 `load_custom_weights`，
/// 使用合成 GGUF 文件应成功返回 Ok(())。
#[tokio::test]
async fn test_mock_warm_up_custom_gemv_synthetic() {
    let model_file = create_synthetic_model_file();
    let engine = LocalLlmEngine::new(model_file.path().to_path_buf(), Quantization::Auto)
        .expect("创建引擎失败");

    engine.set_kernel_mode(KernelMode::CustomGemv).await;

    let result = engine.warm_up().await;
    assert!(
        result.is_ok(),
        "warm_up (CustomGemv) 应成功: {:?}",
        result.err()
    );
}

/// TC-LLM-MOCK-012：`load_custom_weights` 幂等性（多次调用不报错）。
#[tokio::test]
async fn test_mock_load_custom_weights_idempotent() {
    let model_file = create_synthetic_model_file();
    let engine = LocalLlmEngine::new(model_file.path().to_path_buf(), Quantization::Auto)
        .expect("创建引擎失败");

    engine.set_kernel_mode(KernelMode::CustomGemv).await;

    // 第一次调用
    let r1 = engine.load_custom_weights().await;
    assert!(
        r1.is_ok(),
        "第一次 load_custom_weights 应成功: {:?}",
        r1.err()
    );

    // 第二次调用（快路径：已加载）
    let r2 = engine.load_custom_weights().await;
    assert!(
        r2.is_ok(),
        "第二次 load_custom_weights 应成功（幂等）: {:?}",
        r2.err()
    );

    // 第三次调用
    let r3 = engine.load_custom_weights().await;
    assert!(
        r3.is_ok(),
        "第三次 load_custom_weights 应成功（幂等）: {:?}",
        r3.err()
    );
}

/// TC-LLM-MOCK-013：`warm_up` 幂等性（CustomGemv 模式多次调用）。
#[tokio::test]
async fn test_mock_warm_up_custom_gemv_idempotent() {
    let model_file = create_synthetic_model_file();
    let engine = LocalLlmEngine::new(model_file.path().to_path_buf(), Quantization::Auto)
        .expect("创建引擎失败");

    engine.set_kernel_mode(KernelMode::CustomGemv).await;

    let _ = engine.warm_up().await;
    let r2 = engine.warm_up().await;
    assert!(r2.is_ok(), "第二次 warm_up 应成功（幂等）: {:?}", r2.err());
}

// ===========================================================================
// 步骤 5c：KV cache 保存/恢复基本流程
// ===========================================================================

/// TC-LLM-MOCK-014：KV cache save → restore 基本流程。
///
/// 使用合成模型文件，保存 KV cache 到临时目录，
/// 然后从同一目录恢复，应返回 Ok(true)（缓存命中，模型名匹配）。
#[tokio::test]
async fn test_mock_kv_cache_save_restore() {
    let model_file = create_synthetic_model_file();
    let engine = LocalLlmEngine::new(model_file.path().to_path_buf(), Quantization::Auto)
        .expect("创建引擎失败");

    let cache_dir = tempfile::tempdir().expect("创建缓存目录失败");
    let conv_id = "test-conv-001";

    // 保存
    let save_result = engine.save_kv_cache(conv_id, cache_dir.path()).await;
    assert!(
        save_result.is_ok(),
        "save_kv_cache 应成功: {:?}",
        save_result.err()
    );

    // 验证文件存在
    let file_path = cache_dir.path().join(format!("{conv_id}.emkv"));
    assert!(file_path.exists(), "KV cache 文件应存在");

    // 恢复
    let restore_result = engine.restore_kv_cache(conv_id, cache_dir.path()).await;
    assert!(
        restore_result.is_ok(),
        "restore_kv_cache 应成功: {:?}",
        restore_result.err()
    );
    assert!(
        restore_result.unwrap(),
        "应返回 true（缓存命中，模型名匹配）"
    );
}

/// TC-LLM-MOCK-015：KV cache restore 在文件不存在时返回 Ok(false)。
#[tokio::test]
async fn test_mock_kv_cache_restore_miss() {
    let model_file = create_synthetic_model_file();
    let engine = LocalLlmEngine::new(model_file.path().to_path_buf(), Quantization::Auto)
        .expect("创建引擎失败");

    let cache_dir = tempfile::tempdir().expect("创建缓存目录失败");

    // 文件不存在 → cache miss
    let result = engine
        .restore_kv_cache("nonexistent", cache_dir.path())
        .await;
    assert!(result.is_ok(), "应返回 Ok");
    assert!(!result.unwrap(), "应返回 false（cache miss）");
}

/// TC-LLM-MOCK-016：KV cache restore 模型名不匹配返回 Ok(false)。
#[tokio::test]
async fn test_mock_kv_cache_restore_model_mismatch() {
    // 第一个引擎保存 KV cache
    let model_file_1 = create_synthetic_model_file();
    let engine1 = LocalLlmEngine::new(model_file_1.path().to_path_buf(), Quantization::Auto)
        .expect("创建引擎1失败");
    let cache_dir = tempfile::tempdir().expect("创建缓存目录失败");
    let conv_id = "conv-001";

    engine1
        .save_kv_cache(conv_id, cache_dir.path())
        .await
        .expect("保存应成功");

    // 第二个引擎使用不同的模型文件（文件名不同）
    let model_file_2 = create_fake_model_file();
    let engine2 = LocalLlmEngine::new(model_file_2.path().to_path_buf(), Quantization::Auto)
        .expect("创建引擎2失败");

    // 模型名不同 → 返回 false
    let result = engine2.restore_kv_cache(conv_id, cache_dir.path()).await;
    assert!(result.is_ok(), "应返回 Ok");
    assert!(!result.unwrap(), "模型名不匹配应返回 false");
}

/// TC-LLM-MOCK-017：KV cache save 幂等性（多次保存同一会话）。
#[tokio::test]
async fn test_mock_kv_cache_save_idempotent() {
    let model_file = create_synthetic_model_file();
    let engine = LocalLlmEngine::new(model_file.path().to_path_buf(), Quantization::Auto)
        .expect("创建引擎失败");

    let cache_dir = tempfile::tempdir().expect("创建缓存目录失败");
    let conv_id = "idempotent-conv";

    // 多次保存
    engine
        .save_kv_cache(conv_id, cache_dir.path())
        .await
        .expect("第一次保存应成功");
    engine
        .save_kv_cache(conv_id, cache_dir.path())
        .await
        .expect("第二次保存应成功");
    engine
        .save_kv_cache(conv_id, cache_dir.path())
        .await
        .expect("第三次保存应成功");

    // 恢复应成功
    let result = engine.restore_kv_cache(conv_id, cache_dir.path()).await;
    assert!(result.is_ok() && result.unwrap(), "恢复应成功");
}

/// TC-LLM-MOCK-018：KV cache conversation_id 含特殊字符时不报错。
///
/// `sanitize_conversation_id` 将路径分隔符等特殊字符替换为 `_`，
/// 确保不发生路径遍历攻击。
#[tokio::test]
async fn test_mock_kv_cache_special_conversation_id() {
    let model_file = create_synthetic_model_file();
    let engine = LocalLlmEngine::new(model_file.path().to_path_buf(), Quantization::Auto)
        .expect("创建引擎失败");

    let cache_dir = tempfile::tempdir().expect("创建缓存目录失败");

    // 包含路径分隔符和特殊字符的 conversation_id
    let conv_id = "../malicious/../../path";
    let result = engine.save_kv_cache(conv_id, cache_dir.path()).await;
    assert!(
        result.is_ok(),
        "含特殊字符的 save 应成功: {:?}",
        result.err()
    );

    // 验证文件名被清理（不应在 cache_dir 之外创建文件）
    let safe_name = conv_id
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    let file_path = cache_dir.path().join(format!("{safe_name}.emkv"));
    assert!(file_path.exists(), "清理后的文件应存在");

    // 恢复也应成功
    let restore = engine.restore_kv_cache(conv_id, cache_dir.path()).await;
    assert!(restore.is_ok() && restore.unwrap(), "恢复应成功");
}

// ===========================================================================
// 步骤 5d：状态机综合转换测试
// ===========================================================================

/// TC-LLM-MOCK-019：综合状态转换：创建 → set_kernel → set_sampling → abort → unload。
///
/// 验证多个状态转换按顺序执行时不 panic，且各方法返回正确状态。
#[tokio::test]
async fn test_mock_comprehensive_state_machine() {
    let model_file = create_synthetic_model_file();
    let engine = LocalLlmEngine::new(model_file.path().to_path_buf(), Quantization::Auto)
        .expect("创建引擎失败");

    // 1. 初始状态
    assert!(!engine.is_loaded(), "初始应未加载");

    // 2. 设置采样参数
    engine
        .set_sampling_params(LlmSamplingParams {
            temperature: Some(0.7),
            top_p: Some(0.9),
            ..Default::default()
        })
        .await;
    let params = engine.sampling_params().await;
    assert_eq!(params.temperature, Some(0.7));
    assert_eq!(params.top_p, Some(0.9));

    // 3. 切换内核模式
    engine.set_kernel_mode(KernelMode::CustomGemv).await;
    assert_eq!(engine.kernel_mode().await, KernelMode::CustomGemv);

    // 4. abort（未启动 chat 时调用应安全）
    engine.abort().await;
    let token = engine.current_cancel_token().await;
    assert!(token.is_cancelled(), "abort 后 token 应取消");

    // 5. unload（未加载模型时调用应安全）
    engine.unload().await;
    assert!(!engine.is_loaded(), "unload 后应未加载");

    // 6. 切换回 MistralRs
    engine.set_kernel_mode(KernelMode::MistralRs).await;
    assert_eq!(engine.kernel_mode().await, KernelMode::MistralRs);

    // 7. 重新设置采样参数
    engine
        .set_sampling_params(LlmSamplingParams::default())
        .await;
    let params = engine.sampling_params().await;
    assert!(params.temperature.is_none(), "重置后 temperature 应为 None");
}

/// TC-LLM-MOCK-020：CustomGemv → warm_up 成功 → 切换 MistralRs 路径验证。
///
/// 验证内核模式切换后 `warm_up` 路由到不同路径：
/// - CustomGemv → warm_up → load_custom_weights（合成 GGUF，成功）
/// - 切换回 MistralRs → 验证 kernel_mode 已切换（不调用 warm_up，因为合成 GGUF
///   缺少 mistral.rs 所需的完整模型元数据，会导致 panic 而非 Err）
#[tokio::test]
async fn test_mock_warm_up_mode_switch() {
    let model_file = create_synthetic_model_file();
    let engine = LocalLlmEngine::new(model_file.path().to_path_buf(), Quantization::Auto)
        .expect("创建引擎失败");

    // CustomGemv 模式 warm_up 应成功（合成 GGUF 有效，GgufFile::open + repack_for_gemv）
    engine.set_kernel_mode(KernelMode::CustomGemv).await;
    let result = engine.warm_up().await;
    assert!(
        result.is_ok(),
        "CustomGemv warm_up 应成功: {:?}",
        result.err()
    );

    // 切换回 MistralRs 模式，验证 kernel_mode 已正确切换
    engine.set_kernel_mode(KernelMode::MistralRs).await;
    assert_eq!(
        engine.kernel_mode().await,
        KernelMode::MistralRs,
        "切换后 kernel_mode 应为 MistralRs"
    );
}

/// TC-LLM-MOCK-021：KV cache save 后不同 conversation_id 恢复返回 false。
#[tokio::test]
async fn test_mock_kv_cache_different_conversation_id() {
    let model_file = create_synthetic_model_file();
    let engine = LocalLlmEngine::new(model_file.path().to_path_buf(), Quantization::Auto)
        .expect("创建引擎失败");

    let cache_dir = tempfile::tempdir().expect("创建缓存目录失败");

    // 保存 conv-A
    engine
        .save_kv_cache("conv-A", cache_dir.path())
        .await
        .expect("保存 conv-A 应成功");

    // 恢复 conv-B（不存在）应返回 false
    let result = engine.restore_kv_cache("conv-B", cache_dir.path()).await;
    assert!(result.is_ok(), "应返回 Ok");
    assert!(!result.unwrap(), "不存在的会话应返回 false（cache miss）");

    // 恢复 conv-A（存在）应返回 true
    let result = engine.restore_kv_cache("conv-A", cache_dir.path()).await;
    assert!(result.is_ok() && result.unwrap(), "存在的会话应返回 true");
}

/// TC-LLM-MOCK-022：多次 abort 后的 cancel token 状态一致性。
#[tokio::test]
async fn test_mock_abort_state_consistency() {
    let model_file = create_synthetic_model_file();
    let engine = LocalLlmEngine::new(model_file.path().to_path_buf(), Quantization::Auto)
        .expect("创建引擎失败");

    // 初始未取消
    let token = engine.current_cancel_token().await;
    assert!(!token.is_cancelled(), "初始不应取消");

    // abort
    engine.abort().await;
    let token = engine.current_cancel_token().await;
    assert!(token.is_cancelled(), "abort 后应取消");

    // 再次 abort（幂等）
    engine.abort().await;
    let token = engine.current_cancel_token().await;
    assert!(token.is_cancelled(), "再次 abort 后仍应取消");
}
