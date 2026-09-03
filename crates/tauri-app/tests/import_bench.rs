#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! 真实文件导入性能基准测试。
//!
//! 使用 `/Users/john/freesoft/lisp-rs/README_zh.md` 作为测试文件，
//! 分阶段计时定位导入瓶颈。

use std::time::Instant;

use echomind_core::import::{ImportOutcome, ImportService};
use echomind_core::{Embedder, Splitter, Storage};
use echomind_models::Chunk;
use echomind_tauri_app::state::AppState;

/// 测试文件路径（可用环境变量 `ECHOMIND_BENCH_FILE` 覆盖；默认为本机开发语料）
const TEST_FILE: &str = match std::option_env!("ECHOMIND_BENCH_FILE") {
    Some(p) => p,
    None => "/Users/john/freesoft/lisp-rs/README_zh.md",
};

/// 真实文件导入分阶段计时基准测试。
///
/// 默认忽略（`#[ignore]`）：依赖本机语料路径与 ONNX 模型下载（网络），
/// 离线 / 其他机器上必然失败。手动运行：
/// `cargo test -p echomind-tauri-app --test import_bench -- --ignored`
///
/// 分阶段测量：
/// 1. 文件读取 + MD5
/// 2. SectionAwareSplitter 分块
/// 3. add_chunks_batch（SQLite 写入）
/// 4. index_entities（实体抽取）
/// 5. index_relations（关系抽取）
/// 6. Wiki-link 解析
/// 7. index_wiki_links（Wiki 链接解析）
/// 8. embedder 初始化（ONNX 模型加载，首次）
/// 9. embed_batch + add_embeddings_batch（向量推理 + 写入）
#[tokio::test]
#[ignore = "基准测试：需真实语料与网络（ONNX 模型），仅本地手动运行"]
async fn bench_real_import_phases() {
    // 确保测试文件存在
    if !std::path::Path::new(TEST_FILE).exists() {
        eprintln!("⚠️ 测试文件不存在: {TEST_FILE}，跳过基准测试");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let storage = &state.storage;

    // ===== 阶段 0: 文件复制 + add_document =====
    let t0 = Instant::now();
    let service = ImportService::new(storage.clone(), dir.path().to_path_buf());
    let outcome = service.import_one(TEST_FILE, true).await.unwrap();
    let doc = match outcome {
        ImportOutcome::Imported(d) => d,
        ImportOutcome::SkippedDuplicate(_) => {
            panic!("测试文件应首次导入");
        }
        ImportOutcome::NameConflict { .. } => {
            panic!("测试文件应首次导入，不应同名冲突");
        }
    };
    let t_import = t0.elapsed();

    // ===== 阶段 1: 文件读取 =====
    let t1 = Instant::now();
    let raw_text = tokio::fs::read_to_string(&doc.file_path).await.unwrap();
    let t_read = t1.elapsed();
    let file_size = raw_text.len();
    eprintln!("📊 文件大小: {} bytes ({} KB)", file_size, file_size / 1024);

    // ===== 阶段 2: SectionAwareSplitter 分块 =====
    let t2 = Instant::now();
    let splitter =
        echomind_core::section_aware_splitter::SectionAwareSplitter::default_config().unwrap();
    let pieces = splitter.split(&raw_text).await.unwrap();
    let t_split = t2.elapsed();
    let chunk_count = pieces.len();
    eprintln!("📊 分块数量: {chunk_count}");

    // 构建 Chunk 对象
    let chunks: Vec<Chunk> = pieces
        .into_iter()
        .enumerate()
        .map(|(sequence, content)| {
            let token_count = splitter.count_tokens(&content).unwrap_or(0);
            Chunk::new(doc.id.clone(), content, token_count, sequence)
        })
        .collect();

    // ===== 阶段 3: add_chunks_batch =====
    let t3 = Instant::now();
    storage.add_chunks_batch(&chunks).await.unwrap();
    let t_chunks = t3.elapsed();

    // ===== 阶段 4: index_entities =====
    let t4 = Instant::now();
    let entity_chunks = chunks.chunks(5000);
    for batch in entity_chunks {
        let entities: Vec<(String, String, String)> = batch
            .iter()
            .flat_map(|chunk| {
                echomind_core::entity_extractor::EntityExtractor::extract_with_chunk_id(
                    &chunk.content,
                    &chunk.id,
                )
            })
            .collect();
        if !entities.is_empty() {
            storage.add_entities(&entities).await.unwrap();
        }
    }
    let t_entities = t4.elapsed();

    // ===== 阶段 5: index_relations =====
    let t5 = Instant::now();
    for batch in chunks.chunks(5000) {
        let relations: Vec<echomind_models::EntityRelation> = batch
            .iter()
            .flat_map(|chunk| {
                echomind_core::entity_extractor::EntityExtractor::extract_relations(
                    &chunk.content,
                    &chunk.id,
                )
            })
            .collect();
        if !relations.is_empty() {
            storage.add_relations_batch(&relations).await.unwrap();
        }
    }
    let t_relations = t5.elapsed();

    // ===== 阶段 7: index_wiki_links =====
    let t7 = Instant::now();
    for batch in chunks.chunks(5000) {
        let links: Vec<echomind_models::WikiLink> = batch
            .iter()
            .flat_map(|chunk| {
                echomind_core::wiki_link_parser::parse_wiki_links(
                    &chunk.content,
                    &doc.id,
                    &chunk.id,
                )
            })
            .collect();
        if !links.is_empty() {
            storage.add_wiki_links(&links).await.unwrap();
        }
    }
    let t_wiki = t7.elapsed();

    // ===== 阶段 8: embedder 初始化（ONNX 模型加载） =====
    let t8 = Instant::now();
    let embedder = state.embedder().await.unwrap();
    let t_embedder_init = t8.elapsed();

    // ===== 阶段 9: 向量推理 + 写入 =====
    let t9 = Instant::now();
    let stored_chunks = storage.list_chunks(&doc.id).await.unwrap();
    let t_list_chunks = t9.elapsed();

    let t10 = Instant::now();
    let total = stored_chunks.len();
    let mut embedded = 0usize;
    for batch in stored_chunks.chunks(64) {
        let texts: Vec<String> = batch.iter().map(|c| c.content.clone()).collect();
        let vectors = embedder.embed_batch(&texts).await.unwrap();
        let embeddings: Vec<(String, Vec<f32>)> = batch
            .iter()
            .zip(vectors)
            .map(|(c, v)| (c.id.clone(), v))
            .collect();
        storage.add_embeddings_batch(&embeddings).await.unwrap();
        embedded += batch.len();
    }
    let t_embed = t10.elapsed();

    // ===== 汇总报告 =====
    let total_time = t_import
        + t_read
        + t_split
        + t_chunks
        + t_entities
        + t_relations
        + t_wiki
        + t_embedder_init
        + t_list_chunks
        + t_embed;

    eprintln!("\n========== 导入性能基准报告 ==========");
    eprintln!("文件: {TEST_FILE}");
    eprintln!("大小: {} bytes ({} KB)", file_size, file_size / 1024);
    eprintln!("分块: {chunk_count} chunks");
    eprintln!("----------------------------------------");
    eprintln!(
        "阶段 0: 文件复制 + add_document : {:>8.1} ms",
        t_import.as_secs_f64() * 1000.0
    );
    eprintln!(
        "阶段 1: 文件读取                : {:>8.1} ms",
        t_read.as_secs_f64() * 1000.0
    );
    eprintln!(
        "阶段 2: SectionAware 分块       : {:>8.1} ms",
        t_split.as_secs_f64() * 1000.0
    );
    eprintln!(
        "阶段 3: add_chunks_batch        : {:>8.1} ms",
        t_chunks.as_secs_f64() * 1000.0
    );
    eprintln!(
        "阶段 4: 实体抽取 + 索引         : {:>8.1} ms",
        t_entities.as_secs_f64() * 1000.0
    );
    eprintln!(
        "阶段 5: 关系抽取 + 索引         : {:>8.1} ms",
        t_relations.as_secs_f64() * 1000.0
    );
    eprintln!(
        "阶段 6: Wiki-link 解析 + 索引   : {:>8.1} ms",
        t_wiki.as_secs_f64() * 1000.0
    );
    eprintln!(
        "阶段 8: Embedder 初始化 (ONNX)  : {:>8.1} ms",
        t_embedder_init.as_secs_f64() * 1000.0
    );
    eprintln!(
        "阶段 9a: list_chunks (DB查询)   : {:>8.1} ms",
        t_list_chunks.as_secs_f64() * 1000.0
    );
    eprintln!(
        "阶段 9b: embed_batch + 写入     : {:>8.1} ms",
        t_embed.as_secs_f64() * 1000.0
    );
    eprintln!("----------------------------------------");
    eprintln!(
        "总计: {:.1} ms ({:.2}s)",
        total_time.as_secs_f64() * 1000.0,
        total_time.as_secs_f64()
    );
    eprintln!("========================================\n");

    // 基本断言（非性能断言，只验证正确性）
    assert!(chunk_count > 0, "应产生至少 1 个 chunk");
    assert_eq!(embedded, total, "全部 chunks 应完成嵌入");
}
