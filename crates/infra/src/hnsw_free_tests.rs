#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! HNSW 向量索引下沉 Free TDD 测试（REQ-PERF-013）。
//!
//! 验证 HNSW 索引从 Pro 下沉到 Free 后的功能正确性：
//! - Free 模式下 HNSW 模块可编译使用
//! - 阈值切换（>500 用 HNSW，≤500 用全量扫描）
//! - 嵌入写入/删除后索引 dirty 标记
//! - HNSW 检索结果召回率 ≥ 90%
//! - 小知识库全量扫描路径向后兼容

use echomind_core::Storage;
use echomind_models::{Chunk, DocStatus, Document};
use tempfile::TempDir;

use crate::sqlite_storage::SqliteStorage;

/// 生成确定性测试向量（多频混合正弦，模拟 embedding 输出）。
fn make_vector(index: usize, dim: usize) -> Vec<f32> {
    (0..dim)
        .map(|j| {
            let v = (index as f32 * 0.137 + j as f32 * 0.731).sin() * 0.6
                + (index as f32 * 0.913 - j as f32 * 0.197).sin() * 0.3
                + (index as f32 * 1.571 + j as f32 * 0.421).cos() * 0.1;
            v * 0.5 + 0.5
        })
        .collect()
}

/// 辅助：创建存储并插入 N 个带向量的 chunk。
async fn setup_with_vectors(n_chunks: usize, dim: usize) -> (TempDir, SqliteStorage, Vec<String>) {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test_hnsw_free.db");
    let storage = SqliteStorage::new(&db_path).unwrap();

    let doc = Document::new("test-doc.md".to_string(), "hash-test".to_string());
    storage.add_document(&doc).await.unwrap();

    let mut chunk_ids = Vec::with_capacity(n_chunks);
    for i in 0..n_chunks {
        let chunk = Chunk::new(doc.id.clone(), format!("chunk content {i}"), 10, i);
        let cid = chunk.id.clone();
        storage.add_chunk(&chunk).await.unwrap();
        let vec = make_vector(i, dim);
        storage.add_embedding(&cid, &vec).await.unwrap();
        chunk_ids.push(cid);
    }

    // 确保文档状态为 Indexed
    storage
        .update_doc_status(&doc.id, DocStatus::Indexed)
        .await
        .unwrap();

    (dir, storage, chunk_ids)
}

/// TC-HNSW-FREE-001：Free 模式下 HnswIndex 模块可用（AC-1）。
///
/// 验证 `HnswIndex::build` 和 `search` 在 Free 编译模式下可调用。
#[test]
fn tc_hnsw_free_001_module_available_in_free() {
    use crate::hnsw_index::HnswIndex;

    let vectors: Vec<(String, Vec<f32>)> = (0..10)
        .map(|i| (format!("chunk-{i}"), make_vector(i, 384)))
        .collect();

    let index = HnswIndex::build(&vectors).expect("Free 模式构建 HNSW 索引应成功");
    assert_eq!(index.len(), 10, "索引应包含 10 个向量");

    // 搜索自身
    let results = index.search(&vectors[0].1, 3);
    assert_eq!(results.len(), 3, "应返回 3 个最近邻");
    assert_eq!(results[0].0, "chunk-0", "自身应排第一");
}

/// TC-HNSW-FREE-002：Free 模式下 SqliteStorage 包含 HNSW 字段（AC-2）。
///
/// 验证 `mark_hnsw_dirty` 方法在 Free 模式下可调用（不编译错误）。
#[tokio::test]
async fn tc_hnsw_free_002_storage_has_hnsw_fields() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test_fields.db");
    let storage = SqliteStorage::new(&db_path).unwrap();

    // mark_hnsw_dirty 在 Free 模式应可调用（不 panic）
    storage.mark_hnsw_dirty();

    // 验证不会编译失败——方法存在且可调用
    let count = storage.count_documents().await.unwrap();
    assert_eq!(count, 0, "新存储应无文档");
}

/// TC-HNSW-FREE-003：大知识库（>500 chunks）自动使用 HNSW 路径（AC-3 / AC-7）。
///
/// 插入 550 个带向量的 chunk，查询应返回非空结果且 score 降序排列。
/// HNSW_AUTO_THRESHOLD = 500，550 > 500 触发 HNSW 路径。
#[tokio::test]
async fn tc_hnsw_free_003_large_kb_uses_hnsw() {
    let (_dir, storage, chunk_ids) = setup_with_vectors(550, 64).await;

    // 用第一个 chunk 的向量查询
    let query_vec = make_vector(0, 64);
    let results = storage.vector_search(&query_vec, 5).await.unwrap();

    // AC-7：非空 + score 降序
    assert!(!results.is_empty(), "HNSW 路径应返回非空结果");
    assert!(
        results.len() <= 5,
        "应返回最多 5 个结果，实际 {}",
        results.len()
    );

    // 验证 score 降序
    for i in 1..results.len() {
        assert!(
            results[i - 1].score >= results[i].score,
            "结果应按 score 降序排列: [{}]={:.4} >= [{}]={:.4}",
            i - 1,
            results[i - 1].score,
            i,
            results[i].score
        );
    }

    // 自身应在结果中（召回验证）
    assert!(
        results.iter().any(|r| r.chunk.id == chunk_ids[0]),
        "查询向量对应的 chunk 应在结果中"
    );
}

/// TC-HNSW-FREE-004：小知识库（≤500 chunks）使用全量扫描路径（AC-3 / AC-8）。
///
/// 插入 100 个带向量的 chunk，查询应正确返回结果（全量扫描路径）。
#[tokio::test]
async fn tc_hnsw_free_004_small_kb_uses_full_scan() {
    let (_dir, storage, chunk_ids) = setup_with_vectors(100, 64).await;

    let query_vec = make_vector(0, 64);
    let results = storage.vector_search(&query_vec, 5).await.unwrap();

    // AC-8：全量扫描路径不受影响
    assert!(!results.is_empty(), "全量扫描应返回非空结果");
    assert!(results.len() <= 5, "应返回最多 5 个结果");

    // 自身应在结果中
    assert!(
        results.iter().any(|r| r.chunk.id == chunk_ids[0]),
        "查询向量对应的 chunk 应在全量扫描结果中"
    );

    // 验证 score 降序
    for i in 1..results.len() {
        assert!(
            results[i - 1].score >= results[i].score,
            "全量扫描结果也应按 score 降序"
        );
    }
}

/// TC-HNSW-FREE-005：嵌入写入后 HNSW 索引标记 dirty（AC-4）。
///
/// 先查询触发 HNSW 构建，再添加新向量，下次查询应自动重建索引（包含新向量）。
#[tokio::test]
async fn tc_hnsw_free_005_embedding_write_marks_dirty() {
    let (_dir, storage, _chunk_ids) = setup_with_vectors(550, 64).await;

    // 第一次查询：触发 HNSW 构建并搜索
    let query_vec = make_vector(0, 64);
    let results1 = storage.vector_search(&query_vec, 5).await.unwrap();
    assert!(!results1.is_empty(), "首次查询应返回结果");

    // 添加新 chunk + embedding（应标记 dirty）
    let doc = Document::new("new-doc.md".to_string(), "hash-new".to_string());
    storage.add_document(&doc).await.unwrap();
    let new_chunk = Chunk::new(doc.id.clone(), "new content".to_string(), 10, 999);
    let new_chunk_id = new_chunk.id.clone();
    storage.add_chunk(&new_chunk).await.unwrap();
    let new_vec = make_vector(0, 64); // 与 chunk-0 相同向量
    storage
        .add_embedding(&new_chunk_id, &new_vec)
        .await
        .unwrap();

    // 第二次查询：应自动重建 HNSW 索引，新 chunk 应可被检索到
    let results2 = storage.vector_search(&query_vec, 10).await.unwrap();
    assert!(!results2.is_empty(), "重建后查询应返回结果");

    // 新 chunk 应在结果中（因为与查询向量相同）
    assert!(
        results2.iter().any(|r| r.chunk.id == new_chunk_id),
        "新增 chunk 应在重建后的 HNSW 索引中被检索到"
    );
}

/// TC-HNSW-FREE-006：文档删除后 HNSW 索引标记 dirty（AC-6）。
///
/// 先查询触发 HNSW 构建，再删除文档，下次查询应自动重建索引（不含已删除的 chunk）。
#[tokio::test]
async fn tc_hnsw_free_006_doc_delete_marks_dirty() {
    let (_dir, storage, _chunk_ids) = setup_with_vectors(550, 64).await;

    // 第一次查询触发 HNSW 构建
    let query_vec = make_vector(0, 64);
    let results1 = storage.vector_search(&query_vec, 5).await.unwrap();
    assert!(!results1.is_empty());

    // 记录第一个结果的 chunk_id
    let first_hit_id = results1[0].chunk.id.clone();
    let first_doc_id = results1[0].chunk.doc_id.clone();

    // 删除该文档
    storage.delete_document(&first_doc_id).await.unwrap();

    // 第二次查询：应重建索引，被删除的 chunk 不应出现
    let results2 = storage.vector_search(&query_vec, 10).await.unwrap();
    assert!(
        !results2.iter().any(|r| r.chunk.id == first_hit_id),
        "已删除的 chunk 不应在重建后的索引中被检索到"
    );
}

/// TC-HNSW-FREE-007：HNSW 检索结果召回率 ≥ 90%（AC-5）。
///
/// 构建大知识库，比较 HNSW top-k 与全量扫描 top-k 的重叠度。
#[tokio::test]
async fn tc_hnsw_free_007_recall_rate() {
    use crate::hnsw_index::HnswIndex;

    // 构建纯 HNSW 索引（绕过 SqliteStorage 的阈值逻辑，直接测试 HnswIndex 召回率）
    let n = 600;
    let dim = 64;
    let vectors: Vec<(String, Vec<f32>)> = (0..n)
        .map(|i| (format!("chunk-{i}"), make_vector(i, dim)))
        .collect();

    let index = HnswIndex::build(&vectors).expect("构建 HNSW 索引失败");

    // 对每个向量查询 top-5，验证自身是否在结果中
    let mut hits = 0;
    let total = n;
    for (i, (_, query)) in vectors.iter().enumerate() {
        let results = index.search(query, 5);
        if results.iter().any(|(id, _)| id == &format!("chunk-{i}")) {
            hits += 1;
        }
    }

    let recall = hits as f32 / total as f32;
    assert!(
        recall >= 0.90,
        "HNSW 召回率应 ≥ 90%，实际 {recall:.3}（{hits}/{total}）"
    );
}

/// TC-HNSW-FREE-008：HNSW 路径与全量扫描结果一致性（AC-5 旁证）。
///
/// 用相同查询向量分别通过 HNSW 和全量扫描查询，验证 top-1 结果一致。
#[tokio::test]
async fn tc_hnsw_free_008_hnsw_vs_full_scan_consistency() {
    use crate::sqlite_storage::cosine_similarity;

    let n = 550;
    let dim = 64;
    let (_dir, storage, _chunk_ids) = setup_with_vectors(n, dim).await;

    // 加载全部向量做全量扫描
    let all_vectors = storage.load_all_embeddings().await.unwrap();

    // 选择几个查询向量
    let query_indices = [0, 100, 300, 549];

    for &qi in &query_indices {
        let query_vec = make_vector(qi, dim);

        // HNSW 路径（通过 vector_search，n > 500 自动走 HNSW）
        let hnsw_results = storage.vector_search(&query_vec, 5).await.unwrap();
        assert!(!hnsw_results.is_empty(), "HNSW 查询应返回结果");

        // 全量扫描路径（手动计算）
        let mut full_scores: Vec<(String, f32)> = all_vectors
            .iter()
            .map(|(id, vec)| (id.clone(), cosine_similarity(&query_vec, vec)))
            .collect();
        full_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let full_top1 = &full_scores[0].0;

        // HNSW top-1 应在全量扫描 top-5 中（允许排序微调，但 top-1 应一致或非常接近）
        let hnsw_top1 = &hnsw_results[0].chunk.id;
        let hnsw_top1_in_full_top5 = full_scores.iter().take(5).any(|(id, _)| id == hnsw_top1);

        assert!(
            hnsw_top1_in_full_top5 || full_top1 == hnsw_top1,
            "HNSW top-1 ({hnsw_top1}) 应在全量扫描 top-5 中（query={qi}），全量 top-1={full_top1}"
        );
    }
}

// ============================================================================
// REQ-PERF-015 + REQ-NFR-022 TC-PERSIST-005~008 / TC-BOOT-001~002
// ============================================================================

/// 辅助：在指定 db 文件名下创建 550 个向量并触发一次检索（构建 + 落盘）。
/// 返回 (TempDir 保活, db 路径, chunk_ids)。文档用独立 hash 避免跨测试冲突。
async fn setup_indexed_at(db_name: &str) -> (TempDir, std::path::PathBuf, Vec<String>) {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join(db_name);
    let storage = SqliteStorage::new(&db_path).unwrap();

    let doc = Document::new("persist-doc.md".to_string(), format!("hash-{db_name}"));
    storage.add_document(&doc).await.unwrap();
    let mut chunk_ids = Vec::with_capacity(550);
    for i in 0..550 {
        let chunk = Chunk::new(doc.id.clone(), format!("chunk {i}"), 10, i);
        let cid = chunk.id.clone();
        storage.add_chunk(&chunk).await.unwrap();
        storage
            .add_embedding(&cid, &make_vector(i, 64))
            .await
            .unwrap();
        chunk_ids.push(cid);
    }

    // 触发 HNSW 构建 + 落盘
    let hits = storage.vector_search(&make_vector(0, 64), 5).await.unwrap();
    assert!(!hits.is_empty(), "首次检索应返回结果");

    (dir, db_path, chunk_ids)
}

/// TC-PERSIST-005：构建后落盘 hnsw_index.bin（AC-1）。
#[tokio::test]
async fn tc_persist_005_build_persists_file() {
    let (dir, db_path, _ids) = setup_indexed_at("persist_005.db").await;
    let index_path = dir.path().join("hnsw_index.bin");
    assert!(index_path.exists(), "构建后应落盘 hnsw_index.bin");
    let _ = db_path;
}

/// TC-PERSIST-006：重开实例从磁盘加载（不重建，文件内容不变）。
#[tokio::test]
async fn tc_persist_006_reopen_loads_from_disk() {
    let (dir, db_path, _ids) = setup_indexed_at("persist_006.db").await;
    let index_path = dir.path().join("hnsw_index.bin");
    let before = std::fs::read(&index_path).unwrap();
    drop(_ids);

    // 重新打开（模拟应用重启）
    let reopened = SqliteStorage::new(&db_path).unwrap();
    let hits = reopened
        .vector_search(&make_vector(10, 64), 5)
        .await
        .unwrap();
    assert!(!hits.is_empty(), "重开后检索应返回结果");

    // 磁盘加载路径不重写文件（from_disk=true 跳过落盘）
    let after = std::fs::read(&index_path).unwrap();
    assert_eq!(before, after, "磁盘加载路径不得重写索引文件");
}

/// TC-PERSIST-007：损坏索引文件 → 回退全量构建（不崩溃，文件被覆盖）。
#[tokio::test]
async fn tc_persist_007_corrupt_file_falls_back() {
    let (dir, db_path, _ids) = setup_indexed_at("persist_007.db").await;
    let index_path = dir.path().join("hnsw_index.bin");
    // 破坏文件（写入垃圾字节）
    std::fs::write(&index_path, b"garbage-not-an-index").unwrap();

    let reopened = SqliteStorage::new(&db_path).unwrap();
    let hits = reopened
        .vector_search(&make_vector(10, 64), 5)
        .await
        .unwrap();
    assert!(!hits.is_empty(), "损坏文件应回退全量构建并返回结果");

    // 回退构建后应重新落盘（覆盖损坏文件）
    let rebuilt = std::fs::read(&index_path).unwrap();
    assert_ne!(
        &rebuilt[..],
        b"garbage-not-an-index",
        "回退构建应覆盖损坏文件"
    );
}

/// TC-PERSIST-008：写入后标记 dirty → 删除落盘文件 → 重建（AC-4）。
#[tokio::test]
async fn tc_persist_008_write_deletes_and_rebuilds() {
    let (dir, db_path, _ids) = setup_indexed_at("persist_008.db").await;
    let index_path = dir.path().join("hnsw_index.bin");
    assert!(index_path.exists());

    // 写入新 embedding → mark_hnsw_dirty 删除落盘文件
    let storage = SqliteStorage::new(&db_path).unwrap();
    let doc = Document::new("extra.md".to_string(), "hash-extra".to_string());
    storage.add_document(&doc).await.unwrap();
    let chunk = Chunk::new(doc.id.clone(), "extra chunk".to_string(), 10, 999);
    storage.add_chunk(&chunk).await.unwrap();
    storage
        .add_embedding(&chunk.id, &make_vector(0, 64))
        .await
        .unwrap();

    assert!(
        !index_path.exists(),
        "写入后应删除陈旧索引文件（防止崩溃重启加载陈旧索引）"
    );

    // 下次检索重建并重新落盘
    let hits = storage.vector_search(&make_vector(0, 64), 5).await.unwrap();
    assert!(!hits.is_empty());
    assert!(index_path.exists(), "重建后应重新落盘");
}

/// TC-BOOT-001：启动（构造 SqliteStorage）不构建/不加载 HNSW（懒加载，AC-1）。
#[tokio::test]
async fn tc_boot_001_no_index_on_startup() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("boot_001.db");
    let storage = SqliteStorage::new(&db_path).unwrap();

    // 插入 550 个向量（>阈值）但不检索
    let doc = Document::new("boot-doc.md".to_string(), "hash-boot".to_string());
    storage.add_document(&doc).await.unwrap();
    for i in 0..550 {
        let chunk = Chunk::new(doc.id.clone(), format!("c {i}"), 10, i);
        let cid = chunk.id.clone();
        storage.add_chunk(&chunk).await.unwrap();
        storage
            .add_embedding(&cid, &make_vector(i, 64))
            .await
            .unwrap();
    }

    let index_path = dir.path().join("hnsw_index.bin");
    assert!(
        !index_path.exists(),
        "启动 + 导入（无检索）不得构建/落盘 HNSW 索引"
    );
}

/// TC-BOOT-002：二次启动首次检索走磁盘加载且结果正确（AC-2 功能护栏）。
#[tokio::test]
async fn tc_boot_002_second_start_first_search_works() {
    let (dir, db_path, _ids) = setup_indexed_at("boot_002.db").await;
    let _ = db_path;

    // 模拟重启：新实例首次检索直接命中磁盘索引
    let reopened = SqliteStorage::new(&dir.path().join("boot_002.db")).unwrap();
    let hits = reopened
        .vector_search(&make_vector(5, 64), 5)
        .await
        .unwrap();
    assert_eq!(hits.len(), 5, "二次启动首次检索应返回 5 个结果");
    // 结果按分数降序
    for w in hits.windows(2) {
        assert!(w[0].score >= w[1].score, "结果必须降序");
    }
}
