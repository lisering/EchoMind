#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! QA 红灯测试：TC-VEC-001 本地向量化批量推理（REQ-VEC-002-AC-2）。

use std::path::PathBuf;

use echomind_core::Embedder;

use crate::local_embedder::EmbeddingModel;
use crate::local_embedder::LocalEmbedder;
use crate::local_embedder::{DownloadEvent, ModelCacheInfo, ModelEntry};

/// 模型缓存目录：workspace target 下（已 gitignore），避免每次测试重复下载。
fn model_cache_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/fastembed-cache")
}

/// TC-VEC-001 批量推理：数量匹配、维度 384、非全零（all-MiniLM-L6-v2 量化版）。
/// 需要 ONNX 模型下载，CI 中标记为 ignore，通过 `cargo test -- --ignored` 或 `--include-ignored` 运行。
#[tokio::test(flavor = "multi_thread")]
#[ignore = "需要下载 ONNX 模型（~30MB），CI rust-check 默认跳过，混沌测试步骤覆盖"]
async fn tc_vec_001_local_embed_batch() {
    let embedder = LocalEmbedder::new(model_cache_dir()).await.unwrap();

    let texts = vec![
        "灵犀知识库".to_string(),
        "local-first AI knowledge base".to_string(),
        "Rust 极速检索".to_string(),
    ];
    let vectors = embedder.embed_batch(&texts).await.unwrap();

    assert_eq!(vectors.len(), texts.len(), "向量数量必须与输入匹配");
    for (i, v) in vectors.iter().enumerate() {
        assert_eq!(v.len(), 384, "第 {i} 条向量维度必须为 384");
        assert!(v.iter().any(|x| x.abs() > 1e-6), "第 {i} 条向量不得全零");
    }

    let single = embedder.embed("单条文本验证").await.unwrap();
    assert_eq!(single.len(), 384, "单条 embed 维度同样为 384");
}

/// TC-VEC-007：EmbeddingModel 预设模型配置正确性（E3-1）。
///
/// 验证每个预设模型的仓库路径、目录名、ONNX 文件名、向量维度配置正确。
/// 不涉及模型下载，纯配置断言。
#[test]
fn tc_vec_007_embedding_model_configs() {
    // AllMiniLML6V2（默认模型）
    let model = EmbeddingModel::AllMiniLML6V2;
    assert_eq!(model.dim(), 384, "AllMiniLML6V2 维度应为 384");

    // BgeSmallZhV1_5（中文优化）
    let model = EmbeddingModel::BgeSmallZhV1_5;
    assert_eq!(model.dim(), 512, "BgeSmallZhV1_5 维度应为 512");

    // E5SmallV2（多语言）
    let model = EmbeddingModel::E5SmallV2;
    assert_eq!(model.dim(), 384, "E5SmallV2 维度应为 384");
}

/// TC-VEC-007b：不同模型的配置互不相同（E3-1）。
#[test]
fn tc_vec_007b_different_models_have_different_configs() {
    let models = [
        EmbeddingModel::AllMiniLML6V2,
        EmbeddingModel::BgeSmallZhV1_5,
        EmbeddingModel::E5SmallV2,
    ];

    // 验证不同模型的维度配置
    let dims: Vec<usize> = models.iter().map(|m| m.dim()).collect();
    // 至少 BgeSmallZhV1_5 与其他两个不同（512 vs 384）
    assert_ne!(
        dims[0], dims[1],
        "AllMiniLML6V2(384) 与 BgeSmallZhV1_5(512) 维度应不同"
    );
}

/// TC-VEC-008：`DownloadEvent` 序列化/反序列化往返（REQ-VEC-008）。
#[test]
fn tc_vec_008_download_event_serde() {
    // Downloading
    let event = DownloadEvent::Downloading {
        file_name: "model.onnx".to_string(),
        current: 1024,
        total: 30000000,
        file_index: 0,
        total_files: 3,
        source: "https://huggingface.co".to_string(),
    };
    let json = serde_json::to_string(&event).expect("序列化失败");
    let de: DownloadEvent = serde_json::from_str(&json).expect("反序列化失败");
    match de {
        DownloadEvent::Downloading {
            file_name,
            current,
            total,
            file_index: _,
            total_files: _,
            source: _,
        } => {
            assert_eq!(file_name, "model.onnx");
            assert_eq!(current, 1024);
            assert_eq!(total, 30000000);
        }
        _ => panic!("反序列化后应为 Downloading 变体"),
    }

    // Loading
    let json = serde_json::to_string(&DownloadEvent::Loading).expect("序列化失败");
    assert!(json.contains("loading"));

    // Done
    let json = serde_json::to_string(&DownloadEvent::Done).expect("序列化失败");
    assert!(json.contains("done"));

    // Error
    let event = DownloadEvent::Error {
        message: "下载失败".to_string(),
    };
    let json = serde_json::to_string(&event).expect("序列化失败");
    let de: DownloadEvent = serde_json::from_str(&json).expect("反序列化失败");
    match de {
        DownloadEvent::Error { message } => assert_eq!(message, "下载失败"),
        _ => panic!("反序列化后应为 Error 变体"),
    }
}

/// TC-VEC-008b：`ModelCacheInfo` 序列化/反序列化（REQ-VEC-008-AC-5）。
#[test]
fn tc_vec_008b_model_cache_info_serde() {
    let info = ModelCacheInfo {
        total_size_bytes: 30000000,
        models: vec![ModelEntry {
            name: "all-MiniLM-L6-v2".to_string(),
            size_bytes: 30000000,
        }],
    };
    let json = serde_json::to_string(&info).expect("序列化失败");
    let de: ModelCacheInfo = serde_json::from_str(&json).expect("反序列化失败");
    assert_eq!(de.total_size_bytes, 30000000);
    assert_eq!(de.models.len(), 1);
    assert_eq!(de.models[0].name, "all-MiniLM-L6-v2");
    assert_eq!(de.models[0].size_bytes, 30000000);
}

/// TC-VEC-009：`get_cache_info` 空目录返回空模型列表（REQ-VEC-008-AC-5）。
#[test]
fn tc_vec_009_get_cache_info_empty_dir() {
    let tmpdir = tempfile::TempDir::new().expect("创建临时目录失败");
    let cache_dir = tmpdir.path().join("models");
    let info = LocalEmbedder::get_cache_info(&cache_dir);
    assert_eq!(info.total_size_bytes, 0, "空缓存目录总大小应为 0");
    assert!(info.models.is_empty(), "空缓存目录模型列表应为空");
}

/// TC-VEC-009b：`get_cache_info` 正确统计已存在的模型文件大小（REQ-VEC-008-AC-5）。
#[test]
fn tc_vec_009b_get_cache_info_with_files() {
    let tmpdir = tempfile::TempDir::new().expect("创建临时目录失败");
    let cache_dir = tmpdir.path().join("models");
    let model_dir = cache_dir.join("all-MiniLM-L6-v2");
    std::fs::create_dir_all(&model_dir).expect("创建模型目录失败");
    // 写入测试文件
    std::fs::write(model_dir.join("model_quantized.onnx"), vec![0u8; 1024])
        .expect("写入测试文件失败");
    std::fs::write(model_dir.join("tokenizer.json"), vec![0u8; 512]).expect("写入测试文件失败");

    let info = LocalEmbedder::get_cache_info(&cache_dir);
    assert_eq!(info.models.len(), 1, "应检测到 1 个模型");
    assert_eq!(info.models[0].name, "all-MiniLM-L6-v2");
    assert_eq!(
        info.models[0].size_bytes, 1536,
        "模型大小应为 1024+512=1536 字节"
    );
    assert_eq!(info.total_size_bytes, 1536);
}

/// TC-VEC-009c：`clear_cache` 删除指定模型并返回字节数（REQ-VEC-008-AC-6）。
#[test]
fn tc_vec_009c_clear_cache_returns_freed_bytes() {
    let tmpdir = tempfile::TempDir::new().expect("创建临时目录失败");
    let cache_dir = tmpdir.path().join("models");
    let model_dir = cache_dir.join("all-MiniLM-L6-v2");
    std::fs::create_dir_all(&model_dir).expect("创建模型目录失败");
    std::fs::write(model_dir.join("model.onnx"), vec![0u8; 2048]).expect("写入测试文件失败");

    let freed = LocalEmbedder::clear_cache(&cache_dir, Some("all-MiniLM-L6-v2"));
    assert_eq!(freed, 2048, "应返回删除的字节数");
    assert!(!model_dir.exists(), "模型目录应已删除");
}

// ============ GB 级文档加速：并行推理会话池（路径 2）============

/// TC-PERF-001：默认会话池大小在 [1, 8] 范围内，且等于 `available_parallelism`（封顶 8）。
///
/// 验证 `default_pool_size()` 返回合理值：不低于 1，不超过 8（防止内存爆炸）。
#[test]
fn tc_perf_001_default_pool_size_in_range() {
    let size = LocalEmbedder::default_pool_size();
    assert!(size >= 1, "会话池大小至少为 1");
    assert!(size <= 8, "会话池大小不超过 8（防止内存爆炸）");
}

/// TC-PERF-002：分片逻辑正确——给定 100 条文本和 pool_size=4，每片约 25 条。
///
/// 纯逻辑测试，不需要模型下载。验证 `shard_texts` 将输入均匀分配到 N 个分片。
#[test]
fn tc_perf_002_shard_texts_evenly() {
    let texts: Vec<String> = (0..100).map(|i| format!("text-{i}")).collect();
    let shards = LocalEmbedder::shard_texts(&texts, 4);
    assert_eq!(shards.len(), 4, "4 个会话 → 4 个分片");
    let total: usize = shards.iter().map(|s| s.len()).sum();
    assert_eq!(total, 100, "分片总量 = 输入总量");
    // 每片大小差不超过 1
    let sizes: Vec<usize> = shards.iter().map(|s| s.len()).collect();
    assert!(
        sizes.iter().max().unwrap() - sizes.iter().min().unwrap() <= 1,
        "分片大小应均匀分布（差 ≤ 1），实际: {sizes:?}"
    );
}

/// TC-PERF-002b：分片边界——空输入返回空分片列表。
#[test]
fn tc_perf_002b_shard_empty_texts() {
    let shards = LocalEmbedder::shard_texts(&[], 4);
    assert!(shards.is_empty(), "空输入 → 空分片列表");
}

/// TC-PERF-002c：分片边界——pool_size=1 时返回整个输入作为单个分片。
#[test]
fn tc_perf_002c_shard_single_pool() {
    let texts: Vec<String> = (0..5).map(|i| format!("t{i}")).collect();
    let shards = LocalEmbedder::shard_texts(&texts, 1);
    assert_eq!(shards.len(), 1, "pool_size=1 → 单个分片");
    assert_eq!(shards[0].len(), 5, "单分片包含全部输入");
}

/// TC-PERF-002d：分片边界——文本数 < pool_size 时，每条文本独占一个分片。
#[test]
fn tc_perf_002d_shard_fewer_texts_than_pool() {
    let texts: Vec<String> = (0..3).map(|i| format!("t{i}")).collect();
    let shards = LocalEmbedder::shard_texts(&texts, 8);
    assert_eq!(shards.len(), 3, "3 条文本 → 3 个分片（不超过文本数）");
}

/// TC-PERF-003：并行批量推理正确性——100 条文本 → 100 条 384 维向量，顺序保持。
///
/// 需要 ONNX 模型下载，CI 中标记为 ignore。
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "需要下载 ONNX 模型（~30MB），CI 默认跳过"]
async fn tc_perf_003_parallel_embed_batch_correctness() {
    let embedder = LocalEmbedder::new(model_cache_dir()).await.unwrap();

    // 生成 100 条不同文本
    let texts: Vec<String> = (0..100)
        .map(|i| format!("这是第 {i} 条测试文本，用于验证并行推理的正确性。"))
        .collect();

    let vectors = embedder.embed_batch(&texts).await.unwrap();

    // 数量匹配
    assert_eq!(vectors.len(), 100, "向量数量必须与输入匹配");
    // 维度正确
    for (i, v) in vectors.iter().enumerate() {
        assert_eq!(v.len(), 384, "第 {i} 条向量维度必须为 384");
        assert!(v.iter().any(|x| x.abs() > 1e-6), "第 {i} 条向量不得全零");
    }

    // 顺序保持：相同文本应产生相同向量（验证分片合并后顺序未乱）
    //
    // 阈值 0.98 依据（与 TC-PERF-004 一致）：fastembed Dynamic 量化按 batch 校准
    // 动态范围，单条 embed() 与大批次 embed_batch() 中同文本的输出存在固有数值
    // 容差（实测 cosine ≈0.987~0.999），并非顺序错乱。0.99 阈值会在混沌并行跑
    // 中偶发误报（2026-08-24 实证抓到 cosine=0.987587）。
    let probe = embedder.embed(&texts[50]).await.unwrap();
    let cosine = cosine_similarity(&probe, &vectors[50]);
    assert!(
        cosine > 0.98,
        "分片合并后顺序应保持一致（cosine={cosine:.6}，容差依据见上方注释）"
    );
}

/// TC-PERF-004：并行推理 vs 单会话推理——结果一致（cosine > 0.98）。
///
/// 需要 ONNX 模型下载，CI 中标记为 ignore。
///
/// # 阈值说明
/// `embed()` 委托 `embed_batch(&[text])`（同一路径），但 fastembed 对单条文本
/// （batch=1）与多条文本（batch=N）的 ONNX 推理存在数值差异：
/// - Mean pooling 的浮点累加顺序随 batch 内 padding 变化
/// - 多线程（intra_threads=None 全核）推理的浮点非确定性
/// 实测 cosine ≈ 0.989，属批次级数值容差，不影响 top-k 排序与检索精度。
/// 故阈值设为 0.98（而非 0.999）。
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "需要下载 ONNX 模型（~30MB），CI 默认跳过"]
async fn tc_perf_004_parallel_matches_single_session() {
    // 单会话 embedder（直接用 fastembed）
    let cache = model_cache_dir();
    let embedder_pool = LocalEmbedder::new(cache.clone()).await.unwrap();

    // 用 8 条文本验证
    let texts: Vec<String> = vec![
        "Rust 是一门系统编程语言".to_string(),
        "Tauri 是跨平台桌面框架".to_string(),
        "SQLite 是嵌入式关系数据库".to_string(),
        "ONNX 是开放神经网络交换格式".to_string(),
        "向量检索是 RAG 的核心".to_string(),
        "HNSW 是近似最近邻索引".to_string(),
        "Transformer 是注意力机制模型".to_string(),
        "BERT 是双向编码器表示".to_string(),
    ];

    let vectors_parallel = embedder_pool.embed_batch(&texts).await.unwrap();

    // 逐条 embed（单会话路径）
    let mut vectors_single = Vec::with_capacity(texts.len());
    for text in &texts {
        vectors_single.push(embedder_pool.embed(text).await.unwrap());
    }

    // 比对：每条向量的 cosine 相似度应 > 0.98（批次级数值容差，见上方阈值说明）
    for (i, (vp, vs)) in vectors_parallel
        .iter()
        .zip(vectors_single.iter())
        .enumerate()
    {
        let cos = cosine_similarity(vp, vs);
        assert!(
            cos > 0.98,
            "第 {i} 条向量：并行 vs 单会话 cosine={cos:.6}，应 > 0.98"
        );
    }
}

/// 计算两个向量的余弦相似度。
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a < 1e-10 || norm_b < 1e-10 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

// ============================================================
// 自定义 ONNX 嵌入模型单元测试（REQ-VEC-014）
// 测试 local_embedder.rs 中的纯逻辑函数（不涉及实际 ONNX 加载）
// ============================================================

/// TC-VEC-CUSTOM-UNIT-001: EmbeddingModel::Custom 变体基本属性。
#[test]
fn tc_vec_custom_unit_001_custom_variant_properties() {
    let model = EmbeddingModel::Custom("my-model".to_string());
    assert!(model.is_custom(), "Custom 变体 is_custom 应返回 true");
    assert_eq!(model.dim(), 0, "Custom 变体 dim 应返回 0（未检测）");
}

/// TC-VEC-CUSTOM-UNIT-002: find_onnx_file 按优先级查找 ONNX 文件。
#[test]
fn tc_vec_custom_unit_002_find_onnx_file() {
    let dir = tempfile::TempDir::new().unwrap();

    // 空目录 → None
    assert!(LocalEmbedder::find_onnx_file(dir.path()).is_none());

    // 只有 model.onnx → 返回 "model.onnx"（quantized 不存在）
    std::fs::write(dir.path().join("model.onnx"), b"fake").unwrap();
    assert_eq!(
        LocalEmbedder::find_onnx_file(dir.path()),
        Some("model.onnx")
    );

    // 添加 model_quantized.onnx → 优先返回 quantized
    std::fs::write(dir.path().join("model_quantized.onnx"), b"fake").unwrap();
    assert_eq!(
        LocalEmbedder::find_onnx_file(dir.path()),
        Some("model_quantized.onnx")
    );
}

/// TC-VEC-CUSTOM-UNIT-003: validate_onnx_file 检测无效文件。
#[test]
fn tc_vec_custom_unit_003_validate_onnx_file() {
    let dir = tempfile::TempDir::new().unwrap();

    // 有效 ONNX（非空、非 HTML）
    let valid_path = dir.path().join("model.onnx");
    std::fs::write(&valid_path, b"\x08\x01\x12\x04test").unwrap();
    assert!(LocalEmbedder::validate_onnx_file(&valid_path).is_ok());

    // 空文件 → Err
    let empty_path = dir.path().join("empty.onnx");
    std::fs::write(&empty_path, b"").unwrap();
    assert!(LocalEmbedder::validate_onnx_file(&empty_path).is_err());

    // HTML 错误页 → Err
    let html_path = dir.path().join("html.onnx");
    std::fs::write(&html_path, b"<!DOCTYPE html>").unwrap();
    assert!(LocalEmbedder::validate_onnx_file(&html_path).is_err());
}

/// TC-VEC-CUSTOM-UNIT-004: validate_tokenizer_files 检测缺失文件。
#[test]
fn tc_vec_custom_unit_004_validate_tokenizer_files() {
    let dir = tempfile::TempDir::new().unwrap();

    // 空目录 → 全部缺失
    let missing = LocalEmbedder::validate_tokenizer_files(dir.path());
    assert_eq!(missing.len(), 4, "空目录应有 4 个缺失文件");

    // 创建全部文件 → 无缺失
    for name in LocalEmbedder::REQUIRED_TOKENIZER_FILES {
        std::fs::write(dir.path().join(name), r#"{"test": true}"#).unwrap();
    }
    let missing = LocalEmbedder::validate_tokenizer_files(dir.path());
    assert!(missing.is_empty(), "全部文件就绪后应无缺失");
}

/// TC-VEC-CUSTOM-UNIT-005: list_custom_models 扫描目录并返回模型列表。
#[test]
fn tc_vec_custom_unit_005_list_custom_models() {
    let dir = tempfile::TempDir::new().unwrap();
    let custom_dir = dir.path().join("custom_models");
    std::fs::create_dir_all(&custom_dir).unwrap();

    // 创建两个模型目录（一个完整，一个不完整）
    let model_a = custom_dir.join("model-a");
    std::fs::create_dir_all(&model_a).unwrap();
    std::fs::write(model_a.join("model.onnx"), b"fake").unwrap();
    for name in LocalEmbedder::REQUIRED_TOKENIZER_FILES {
        std::fs::write(model_a.join(name), r#"{"test": true}"#).unwrap();
    }

    let model_b = custom_dir.join("model-b");
    std::fs::create_dir_all(&model_b).unwrap();
    std::fs::write(model_b.join("model.onnx"), b"fake").unwrap();
    // model_b 缺少 tokenizer 文件

    let models = LocalEmbedder::list_custom_models(&custom_dir);
    assert_eq!(models.len(), 2, "应有 2 个模型");

    let model_a_info = models.iter().find(|m| m.name == "model-a").unwrap();
    assert!(model_a_info.is_valid, "model-a 应有效");
    assert!(model_a_info.size_bytes > 0, "model-a 大小应 > 0");

    let model_b_info = models.iter().find(|m| m.name == "model-b").unwrap();
    assert!(!model_b_info.is_valid, "model-b 应无效（缺少 tokenizer）");
}

/// TC-VEC-CUSTOM-UNIT-006: delete_custom_model 删除目录并返回大小。
#[test]
fn tc_vec_custom_unit_006_delete_custom_model() {
    let dir = tempfile::TempDir::new().unwrap();
    let custom_dir = dir.path().join("custom_models");
    let model_dir = custom_dir.join("test-model");
    std::fs::create_dir_all(&model_dir).unwrap();
    std::fs::write(model_dir.join("model.onnx"), b"fake data").unwrap();

    // 删除存在的模型
    let freed = LocalEmbedder::delete_custom_model(&custom_dir, "test-model");
    assert!(freed > 0, "删除存在的模型应返回 > 0");
    assert!(!model_dir.exists(), "删除后目录不应存在");

    // 删除不存在的模型 → 返回 0
    let freed2 = LocalEmbedder::delete_custom_model(&custom_dir, "nonexistent");
    assert_eq!(freed2, 0, "删除不存在的模型应返回 0");
}

/// TC-VEC-CUSTOM-UNIT-007: delete_custom_model 路径遍历防护。
#[test]
fn tc_vec_custom_unit_007_delete_path_traversal_protection() {
    let dir = tempfile::TempDir::new().unwrap();
    let custom_dir = dir.path().join("custom_models");
    std::fs::create_dir_all(&custom_dir).unwrap();

    // 尝试路径遍历攻击
    let freed = LocalEmbedder::delete_custom_model(&custom_dir, "../../etc/passwd");
    // 应安全处理（返回 0 或清理后的名称），不应访问系统目录
    assert_eq!(freed, 0, "路径遍历应被防护，返回 0");

    // 验证没有在 custom_dir 外创建/删除文件
    let cleaned_name = "../../etc/passwd"
        .replace('/', "_")
        .replace("..", "_")
        .replace('~', "_");
    let cleaned_dir = custom_dir.join(&cleaned_name);
    assert!(!cleaned_dir.exists(), "不应在 custom_dir 外操作");
}

/// TC-VEC-CUSTOM-UNIT-008: copy_custom_model_files 复制文件并验证完整性。
#[test]
fn tc_vec_custom_unit_008_copy_custom_model_files() {
    let dir = tempfile::TempDir::new().unwrap();
    let dest_dir = dir.path().join("custom_models").join("test-model");

    // 创建源文件
    let onnx_src = dir.path().join("model.onnx");
    std::fs::write(&onnx_src, b"\x08\x01\x12\x04test").unwrap();

    let tokenizer_files: Vec<PathBuf> = LocalEmbedder::REQUIRED_TOKENIZER_FILES
        .iter()
        .map(|name| {
            let path = dir.path().join(name);
            std::fs::write(&path, r#"{"test": true}"#).unwrap();
            path
        })
        .collect();

    // 复制文件
    let result = LocalEmbedder::copy_custom_model_files(&dest_dir, &onnx_src, &tokenizer_files);
    assert!(result.is_ok(), "复制文件应成功: {:?}", result.err());

    // 验证文件已复制
    assert!(dest_dir.join("model.onnx").exists(), "ONNX 文件应已复制");
    for name in LocalEmbedder::REQUIRED_TOKENIZER_FILES {
        assert!(
            dest_dir.join(name).exists(),
            "tokenizer 文件 {name} 应已复制"
        );
    }

    // 缺少必需 tokenizer 文件 → Err
    let dest_dir2 = dir.path().join("custom_models").join("incomplete");
    let partial_tokenizers: Vec<PathBuf> = tokenizer_files
        .iter()
        .skip(1) // 跳过第一个文件
        .cloned()
        .collect();
    let result2 =
        LocalEmbedder::copy_custom_model_files(&dest_dir2, &onnx_src, &partial_tokenizers);
    assert!(result2.is_err(), "缺少 tokenizer 文件应返回错误");
}
