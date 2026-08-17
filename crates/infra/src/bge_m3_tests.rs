//! REQ-VEC-015 bge-m3 多语言嵌入模型 TDD 测试。
//!
//! 测试 bge-m3 模型配置（维度、仓库路径、文件列表、缓存管理）。
//! TC-VEC-M3-001~007 覆盖 SRS REQ-VEC-015 全部 AC。

// 测试代码允许 unwrap/expect（铁律五仅约束生产代码）
#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::local_embedder::{EmbeddingModel, LocalEmbedder};

/// TC-VEC-M3-001：BgeM3 变体存在且 dim() 返回 1024。
#[test]
fn tc_vec_m3_001_dim_returns_1024() {
    let model = EmbeddingModel::BgeM3;
    assert_eq!(model.dim(), 1024, "bge-m3 维度必须为 1024");
}

/// TC-VEC-M3-002：repo() 和 dir_name() 返回正确路径。
#[test]
fn tc_vec_m3_002_repo_and_dir_name() {
    let model = EmbeddingModel::BgeM3;
    assert_eq!(model.repo(), "BAAI/bge-m3", "repo 必须为 BAAI/bge-m3");
    assert_eq!(model.dir_name(), "bge-m3", "dir_name 必须为 bge-m3");
}

/// TC-VEC-M3-003：onnx_file() 和 onnx_repo_path() 返回正确路径。
#[test]
fn tc_vec_m3_003_onnx_paths() {
    let model = EmbeddingModel::BgeM3;
    assert_eq!(
        model.onnx_file(),
        "model.onnx",
        "onnx_file 必须为 model.onnx"
    );
    assert_eq!(
        model.onnx_repo_path(),
        "onnx/model.onnx",
        "onnx_repo_path 必须为 onnx/model.onnx"
    );
}

/// TC-VEC-M3-004：files() 返回 5 个必需文件。
#[test]
fn tc_vec_m3_004_files_count() {
    let model = EmbeddingModel::BgeM3;
    let files = model.files();
    assert_eq!(files.len(), 5, "必须有 5 个文件");
    // 第一个文件是 ONNX 模型
    assert!(files[0].1.ends_with(".onnx"), "第一个文件必须是 ONNX");
    // 检查 tokenizer 文件存在
    let names: Vec<&str> = files.iter().map(|(_, n)| *n).collect();
    assert!(names.contains(&"tokenizer.json"), "必须包含 tokenizer.json");
    assert!(names.contains(&"config.json"), "必须包含 config.json");
    assert!(
        names.contains(&"special_tokens_map.json"),
        "必须包含 special_tokens_map.json"
    );
    assert!(
        names.contains(&"tokenizer_config.json"),
        "必须包含 tokenizer_config.json"
    );
}

/// TC-VEC-M3-005：get_cache_info 包含 bge-m3 模型目录扫描。
///
/// 创建临时目录，在其中创建 bge-m3 子目录，验证 get_cache_info 返回该模型。
#[test]
fn tc_vec_m3_005_get_cache_info_includes_bge_m3() {
    let dir = tempfile::tempdir().unwrap();
    let cache_dir = dir.path();

    // 创建 bge-m3 模型目录并放入一个假文件
    let model_dir = cache_dir.join("bge-m3");
    std::fs::create_dir_all(&model_dir).unwrap();
    std::fs::write(model_dir.join("model.onnx"), b"fake onnx content").unwrap();

    let info = LocalEmbedder::get_cache_info(cache_dir);
    assert!(
        info.models.iter().any(|m| m.name == "bge-m3"),
        "get_cache_info 必须包含 bge-m3 模型"
    );
}

/// TC-VEC-M3-006：clear_cache 包含 bge-m3 模型目录清理。
#[test]
fn tc_vec_m3_006_clear_cache_includes_bge_m3() {
    let dir = tempfile::tempdir().unwrap();
    let cache_dir = dir.path();

    // 创建 bge-m3 模型目录
    let model_dir = cache_dir.join("bge-m3");
    std::fs::create_dir_all(&model_dir).unwrap();
    std::fs::write(model_dir.join("model.onnx"), b"fake onnx content").unwrap();

    // 验证目录存在
    assert!(model_dir.exists(), "清理前 bge-m3 目录必须存在");

    // 清理全部
    let freed = LocalEmbedder::clear_cache(cache_dir, None);
    assert!(freed > 0, "清理后必须返回释放的字节数");
    assert!(!model_dir.exists(), "清理后 bge-m3 目录必须不存在");
}

/// TC-VEC-M3-007：is_custom() 对 BgeM3 返回 false。
#[test]
fn tc_vec_m3_007_is_custom_false() {
    let model = EmbeddingModel::BgeM3;
    assert!(!model.is_custom(), "BgeM3 不是自定义模型");
}
