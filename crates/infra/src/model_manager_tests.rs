//! ModelManager 单元测试（REQ-LLM-004）。
//!
//! 测试覆盖：目录创建、模型列表、删除、路径获取、文件名安全校验、
//! 元信息解析、推荐模型常量。

#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::model_manager::*;
use std::io::Write;
use tempfile::TempDir;

/// 辅助：创建临时 ModelManager
fn make_manager() -> (TempDir, ModelManager) {
    let tmp = tempfile::tempdir().expect("创建临时目录失败");
    let mgr = ModelManager::new(tmp.path()).expect("创建 ModelManager 失败");
    (tmp, mgr)
}

/// 辅助：在模型目录中创建假 GGUF 文件
fn create_fake_gguf(mgr: &ModelManager, filename: &str, content: &[u8]) {
    let path = mgr.models_dir().join(filename);
    let mut file = std::fs::File::create(&path).expect("创建文件失败");
    file.write_all(content).expect("写入文件失败");
}

// ---- TC-MGR-001 ~ TC-MGR-005: list_models ----

/// TC-MGR-001：`new()` 创建 `models/llm/` 目录
#[test]
fn test_new_creates_models_dir() {
    let tmp = tempfile::tempdir().expect("创建临时目录失败");
    let data_dir = tmp.path();
    let expected = data_dir.join("models").join("llm");

    assert!(!expected.exists());
    let _mgr = ModelManager::new(data_dir).expect("创建 ModelManager 失败");
    assert!(expected.exists());
    assert!(expected.is_dir());
}

/// TC-MGR-002：空目录返回空 Vec
#[test]
fn test_list_models_empty_dir() {
    let (_tmp, mgr) = make_manager();
    let models = mgr.list_models().expect("list_models 失败");
    assert!(models.is_empty());
}

/// TC-MGR-003：目录中有 `.gguf` 文件时正确列出
#[test]
fn test_list_models_finds_gguf() {
    let (_tmp, mgr) = make_manager();
    create_fake_gguf(&mgr, "qwen2.5-7b-instruct-q4_k_m.gguf", b"fake model");
    create_fake_gguf(&mgr, "llama-3.2-3b-instruct-q4_k_m.gguf", b"fake model");

    let models = mgr.list_models().expect("list_models 失败");
    assert_eq!(models.len(), 2);
}

/// TC-MGR-004：非 `.gguf` 文件被忽略
#[test]
fn test_list_models_ignores_non_gguf() {
    let (_tmp, mgr) = make_manager();
    create_fake_gguf(&mgr, "model.gguf", b"fake");
    create_fake_gguf(&mgr, "readme.txt", b"not a model");
    create_fake_gguf(&mgr, "config.json", b"{}");

    let models = mgr.list_models().expect("list_models 失败");
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].filename, "model.gguf");
}

/// TC-MGR-005：正确解析文件名中的架构/参数量/量化格式
#[test]
fn test_list_models_parses_metadata() {
    let (_tmp, mgr) = make_manager();
    create_fake_gguf(&mgr, "qwen2.5-7b-instruct-q4_k_m.gguf", b"fake model");

    let models = mgr.list_models().expect("list_models 失败");
    assert_eq!(models.len(), 1);
    let info = &models[0];
    assert_eq!(info.filename, "qwen2.5-7b-instruct-q4_k_m.gguf");
    assert_eq!(info.architecture, "qwen2.5");
    assert_eq!(info.param_size, "7B");
    assert_eq!(info.quantization, "Q4_K_M");
    assert_eq!(info.size_bytes, 10); // "fake model" = 10 bytes
}

// ---- TC-MGR-006 ~ TC-MGR-008: delete_model ----

/// TC-MGR-006：删除已存在的模型文件
#[test]
fn test_delete_model_removes_file() {
    let (_tmp, mgr) = make_manager();
    create_fake_gguf(&mgr, "test-model.gguf", b"data");

    mgr.delete_model("test-model.gguf").expect("删除失败");
    assert!(!mgr.models_dir().join("test-model.gguf").exists());
}

/// TC-MGR-007：删除不存在的文件返回 Err
#[test]
fn test_delete_model_nonexistent_fails() {
    let (_tmp, mgr) = make_manager();
    let result = mgr.delete_model("nonexistent.gguf");
    assert!(result.is_err());
}

/// TC-MGR-008：`../../etc/passwd` 被拒绝
#[test]
fn test_delete_model_path_traversal_blocked() {
    let (_tmp, mgr) = make_manager();
    let result = mgr.delete_model("../../etc/passwd");
    assert!(result.is_err());
}

// ---- TC-MGR-009 ~ TC-MGR-010: model_path ----

/// TC-MGR-009：返回正确的完整路径
#[test]
fn test_model_path_returns_correct_path() {
    let (_tmp, mgr) = make_manager();
    let path = mgr.model_path("test.gguf").expect("model_path 失败");
    assert_eq!(path, mgr.models_dir().join("test.gguf"));
}

/// TC-MGR-010：路径穿越被拒绝
#[test]
fn test_model_path_traversal_blocked() {
    let (_tmp, mgr) = make_manager();
    assert!(mgr.model_path("../../etc/passwd").is_err());
    assert!(mgr.model_path("subdir/file.gguf").is_err());
}

// ---- TC-MGR-011 ~ TC-MGR-013: sanitize_filename ----

/// TC-MGR-011：含 `/` 的文件名被拒绝
#[test]
fn test_sanitize_filename_rejects_slash() {
    assert!(ModelManager::sanitize_filename("path/to/file.gguf").is_err());
    assert!(ModelManager::sanitize_filename(r"back\slash.gguf").is_err());
}

/// TC-MGR-012：含 `..` 的文件名被拒绝
#[test]
fn test_sanitize_filename_rejects_dotdot() {
    assert!(ModelManager::sanitize_filename("../file.gguf").is_err());
    assert!(ModelManager::sanitize_filename("file..gguf").is_err());
    assert!(ModelManager::sanitize_filename("..hidden.gguf").is_err());
}

/// TC-MGR-013：正常文件名通过
#[test]
fn test_sanitize_filename_accepts_valid() {
    assert_eq!(
        ModelManager::sanitize_filename("qwen2.5-7b-instruct-q4_k_m.gguf").unwrap(),
        "qwen2.5-7b-instruct-q4_k_m.gguf"
    );
    assert_eq!(
        ModelManager::sanitize_filename("model.gguf").unwrap(),
        "model.gguf"
    );
    assert!(ModelManager::sanitize_filename("").is_err());
}

// ---- TC-MGR-014 ~ TC-MGR-018: parse 辅助函数 ----

/// TC-MGR-014：`qwen2.5-7b-...` → `qwen2.5`
#[test]
fn test_parse_architecture_qwen() {
    assert_eq!(
        ModelManager::parse_architecture("qwen2.5-7b-instruct-q4_k_m.gguf"),
        "qwen2.5"
    );
    assert_eq!(
        ModelManager::parse_architecture("Qwen2.5-3B-Instruct-Q5_K_M.gguf"),
        "qwen2.5"
    );
}

/// TC-MGR-015：`llama-3.2-3b-...` → `llama3.2`
#[test]
fn test_parse_architecture_llama() {
    assert_eq!(
        ModelManager::parse_architecture("llama-3.2-3b-instruct-q4_k_m.gguf"),
        "llama3.2"
    );
    assert_eq!(
        ModelManager::parse_architecture("Llama-3.2-1B-Instruct-Q8_0.gguf"),
        "llama3.2"
    );
}

/// TC-MGR-016：`...7b...` → `7B`
#[test]
fn test_parse_param_size() {
    assert_eq!(
        ModelManager::parse_param_size("qwen2.5-7b-instruct-q4_k_m.gguf"),
        "7B"
    );
    assert_eq!(
        ModelManager::parse_param_size("llama-3.2-3b-instruct-q4_k_m.gguf"),
        "3B"
    );
    assert_eq!(
        ModelManager::parse_param_size("Phi-3.5-mini-instruct-3.8b-q4_k_m.gguf"),
        "3.8B"
    );
    assert_eq!(
        ModelManager::parse_param_size("unknown-model.gguf"),
        "unknown"
    );
}

/// TC-MGR-017：`...q4_k_m...` → `Q4_K_M`
#[test]
fn test_parse_quantization_q4km() {
    assert_eq!(
        ModelManager::parse_quantization("qwen2.5-7b-instruct-q4_k_m.gguf"),
        "Q4_K_M"
    );
    assert_eq!(
        ModelManager::parse_quantization("model-Q4_K_M.gguf"),
        "Q4_K_M"
    );
}

/// TC-MGR-018：`...q8_0...` → `Q8_0`
#[test]
fn test_parse_quantization_q8() {
    assert_eq!(
        ModelManager::parse_quantization("llama-3.2-3b-instruct-q8_0.gguf"),
        "Q8_0"
    );
    assert_eq!(ModelManager::parse_quantization("model.f16.gguf"), "F16");
    assert_eq!(
        ModelManager::parse_quantization("unknown-model.gguf"),
        "unknown"
    );
}

// ---- TC-MGR-019 ~ TC-MGR-020: RECOMMENDED_MODELS ----

/// TC-MGR-019：`RECOMMENDED_MODELS` 非空
#[test]
fn test_recommended_models_not_empty() {
    assert!(!RECOMMENDED_MODELS.is_empty());
    assert!(RECOMMENDED_MODELS.len() >= 3);
}

/// TC-MGR-020：所有 URL 以 `https://` 开头
#[test]
fn test_recommended_models_urls_valid() {
    for model in RECOMMENDED_MODELS {
        assert!(
            model.url.starts_with("https://"),
            "模型 {} 的 URL 不是 HTTPS: {}",
            model.name,
            model.url
        );
        assert!(!model.name.is_empty(), "模型名为空");
        assert!(!model.url.is_empty(), "模型 URL 为空");
        assert!(model.size_gb > 0.0, "模型 {} 大小为 0", model.name);
    }
}
