//! 真实文件端到端性能测试（REQ-NFR-002）。
//!
//! 使用真实文件（lisp-rs/README_zh.md, 8137 行）+ 真实 ONNX embedder，
//! 精确测量 RAG 管线每个阶段的延迟，定位用户报告的 20s 延迟。
//!
//! 运行方式：
//! ```bash
//! cargo test -p echomind-infra -- --ignored perf_real --nocapture
//! ```

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::time::Instant;

use tempfile::TempDir;

use crate::local_embedder::LocalEmbedder;
use crate::sqlite_storage::SqliteStorage;
use echomind_core::retriever::expand_neighbors;
use echomind_core::{
    Embedder, Loader, Retriever, Splitter, Storage, hybrid_retriever::HybridRetriever,
    loader::MarkdownLoader, splitter::TextSplitter,
};

/// 测试用真实文件路径。
const TEST_FILE: &str = "/Users/john/freesoft/lisp-rs/README_zh.md";

/// 获取 ONNX 模型缓存目录（复用应用数据目录，与 `cargo tauri dev` 一致）。
fn embedder_cache_dir() -> PathBuf {
    // macOS: ~/Library/Application Support/com.echomind.app/models
    // 与 AppState 中 data_dir.join("models") 一致
    if cfg!(target_os = "macos") {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        PathBuf::from(home).join("Library/Application Support/com.echomind.app/models")
    } else {
        PathBuf::from("/tmp/echomind-models")
    }
}

/// 真实文件端到端性能测试：精确测量从文件导入到查询检索的全链路延迟。
#[tokio::test]
#[ignore = "真实文件性能测试：手动运行 cargo test -p echomind-infra -- --ignored perf_real --nocapture"]
async fn perf_real_e2e_lisp_rs() {
    // 语料守卫（V3.1 阶段一）：测试依赖本机真实语料（lisp-rs README_zh），
    // CI/其他机器无此文件时静默跳过（避免 chaos --include-ignored 误报）。
    if !Path::new(TEST_FILE).exists() {
        eprintln!("⚠️ 跳过：语料不存在 {}（CI 环境预期行为）", TEST_FILE);
        return;
    }
    let sep = "=".repeat(80);
    let dash = "-".repeat(60);
    println!("\n{sep}");
    println!("🚀 真实文件端到端性能测试：lisp-rs/README_zh.md");
    println!("{sep}");

    // -----------------------------------------------------------------------
    // 阶段 0：初始化 ONNX embedder（首次运行会下载模型 ~30MB）
    // -----------------------------------------------------------------------
    let t0 = Instant::now();
    println!("\n📦 [阶段 0] 初始化 ONNX embedder...");
    let cache_dir = embedder_cache_dir();
    println!("   缓存目录: {}", cache_dir.display());

    let embedder = LocalEmbedder::new(cache_dir)
        .await
        .expect("ONNX embedder 初始化失败");
    let embedder_init_ms = t0.elapsed().as_millis();
    println!("   ✅ embedder 初始化完成: {}ms", embedder_init_ms);
    if embedder_init_ms > 3000 {
        println!("   ⚠️  embedder 初始化较慢（可能包含模型下载）");
    }

    // -----------------------------------------------------------------------
    // 阶段 1：文件加载（MarkdownLoader）
    // -----------------------------------------------------------------------
    let t1 = Instant::now();
    println!("\n📄 [阶段 1] 加载 Markdown 文件...");
    let file_path = PathBuf::from(TEST_FILE);
    if !file_path.exists() {
        panic!("测试文件不存在: {}", file_path.display());
    }
    let file_size = std::fs::metadata(&file_path).map(|m| m.len()).unwrap_or(0);
    println!(
        "   文件大小: {} bytes ({:.1} KB)",
        file_size,
        file_size as f64 / 1024.0
    );

    let loader = MarkdownLoader;
    let content = loader.load(TEST_FILE).await.expect("文件加载失败");
    let load_ms = t1.elapsed().as_millis();
    let char_count = content.chars().count();
    println!("   ✅ 文件加载完成: {}ms ({} chars)", load_ms, char_count);
    println!(
        "   吞吐量: {:.0} chars/ms",
        char_count as f64 / load_ms as f64
    );

    // -----------------------------------------------------------------------
    // 阶段 2：文本分块（TextSplitter）
    // -----------------------------------------------------------------------
    let t2 = Instant::now();
    println!("\n✂️  [阶段 2] 文本分块 (TextSplitter, 256 tokens / 32 overlap)...");
    let splitter = TextSplitter::new().expect("splitter 初始化失败");
    let chunk_texts = splitter.split(&content).await.expect("分块失败");
    let split_ms = t2.elapsed().as_millis();
    println!(
        "   ✅ 分块完成: {}ms ({} chunks)",
        split_ms,
        chunk_texts.len()
    );
    if !chunk_texts.is_empty() {
        let avg_chunk_len = chunk_texts.iter().map(|c| c.len()).sum::<usize>() / chunk_texts.len();
        println!("   平均 chunk 大小: {} chars", avg_chunk_len);
    }

    // -----------------------------------------------------------------------
    // 阶段 3：数据库初始化 + 文档写入
    // -----------------------------------------------------------------------
    let t3 = Instant::now();
    println!("\n💾 [阶段 3] 数据库初始化 + 文档写入...");
    let tmpdir = TempDir::new().expect("tempdir 创建失败");
    let db_path = tmpdir.path().join("perf_test.db");
    let storage = SqliteStorage::new(&db_path).expect("storage 初始化失败");

    use echomind_models::{Chunk, Document};
    let doc = Document::new("README_zh.md".to_string(), "perf-hash-test".to_string());
    storage.add_document(&doc).await.expect("add_document 失败");

    // 写入所有 chunks
    for (i, text) in chunk_texts.iter().enumerate() {
        let token_count = splitter.count_tokens(text).unwrap_or(0);
        let c = Chunk::new(doc.id.clone(), text.clone(), token_count, i);
        storage.add_chunk(&c).await.expect("add_chunk 失败");
    }
    let db_write_ms = t3.elapsed().as_millis();
    println!("   ✅ 文档 + chunks 写入完成: {}ms", db_write_ms);

    // -----------------------------------------------------------------------
    // 阶段 4：批量嵌入（embed_batch）— 预期主要瓶颈
    // -----------------------------------------------------------------------
    let t4 = Instant::now();
    println!(
        "\n🧠 [阶段 4] 批量嵌入 (embed_batch, {} chunks)...",
        chunk_texts.len()
    );
    let embeddings = embedder
        .embed_batch(&chunk_texts)
        .await
        .expect("embed_batch 失败");
    let embed_batch_ms = t4.elapsed().as_millis();
    let embed_dim = embeddings.first().map(|v| v.len()).unwrap_or(0);
    println!(
        "   ✅ 批量嵌入完成: {}ms ({} vectors, {}-dim)",
        embed_batch_ms,
        embeddings.len(),
        embed_dim
    );
    if !embeddings.is_empty() {
        let per_chunk_ms = embed_batch_ms as f64 / embeddings.len() as f64;
        println!("   每 chunk 嵌入耗时: {:.1}ms", per_chunk_ms);
        let throughput = embeddings.len() as f64 / (embed_batch_ms as f64 / 1000.0);
        println!("   吞吐量: {:.1} chunks/sec", throughput);
    }

    // -----------------------------------------------------------------------
    // 阶段 5：写入 embeddings 到数据库
    // -----------------------------------------------------------------------
    let t5 = Instant::now();
    println!("\n💾 [阶段 5] 写入 embeddings 到数据库...");
    // 需要重新查询 chunks 获取它们的 ID（因为 add_chunk 自动生成了 UUID）
    let stored_chunks = storage
        .list_chunks(&doc.id)
        .await
        .expect("list_chunks 失败");
    for (i, emb) in embeddings.iter().enumerate() {
        let chunk_id = stored_chunks
            .get(i)
            .map(|c| c.id.as_str())
            .unwrap_or_else(|| panic!("chunk {i} not found"));
        storage
            .add_embedding(chunk_id, emb)
            .await
            .expect("add_embedding 失败");
    }
    let emb_write_ms = t5.elapsed().as_millis();
    println!("   ✅ embeddings 写入完成: {}ms", emb_write_ms);

    // -----------------------------------------------------------------------
    // 阶段 6：查询嵌入（embed single query）
    // -----------------------------------------------------------------------
    let query = "请解释一下Lisp的原理";
    let t6 = Instant::now();
    println!("\n🔍 [阶段 6] 查询嵌入: \"{}\"", query);
    let query_vec = embedder.embed(query).await.expect("embed 失败");
    let query_embed_ms = t6.elapsed().as_millis();
    println!(
        "   ✅ 查询嵌入完成: {}ms ({}-dim)",
        query_embed_ms,
        query_vec.len()
    );

    // -----------------------------------------------------------------------
    // 阶段 7：向量检索（vector_search）
    // -----------------------------------------------------------------------
    let t7 = Instant::now();
    println!("\n🔎 [阶段 7] 向量检索 (top_k=5)...");
    let results = storage
        .vector_search(&query_vec, 5)
        .await
        .expect("vector_search 失败");
    let search_ms = t7.elapsed().as_millis();
    println!(
        "   ✅ 向量检索完成: {}ms ({} results)",
        search_ms,
        results.len()
    );
    for (i, r) in results.iter().enumerate().take(5) {
        let preview: String = r.chunk.content.chars().take(60).collect();
        println!(
            "   #{} score={:.4} chunk_id={} preview=\"{}...\"",
            i, r.score, r.chunk.id, preview
        );
    }

    // -----------------------------------------------------------------------
    // 阶段 8：Chunk 扩展（expand_neighbors）
    // -----------------------------------------------------------------------
    let t8 = Instant::now();
    println!("\n🔗 [阶段 8] Chunk 扩展 (expand_neighbors)...");
    let expanded = expand_neighbors(&storage, &results)
        .await
        .expect("expand_neighbors 失败");
    let expand_ms = t8.elapsed().as_millis();
    println!(
        "   ✅ Chunk 扩展完成: {}ms ({} → {} chunks)",
        expand_ms,
        results.len(),
        expanded.len()
    );

    // -----------------------------------------------------------------------
    // 阶段 9：全链路检索（HybridRetriever.retrieve）
    // -----------------------------------------------------------------------
    let t9 = Instant::now();
    println!("\n🔄 [阶段 9] 全链路检索 (HybridRetriever.retrieve)...");
    let mut retriever = HybridRetriever::new(embedder.clone(), storage.clone());
    retriever.set_hybrid_enabled(true); // 启用混合检索
    let retrieved = retriever.retrieve(query, 5).await.expect("retrieve 失败");
    let retrieve_ms = t9.elapsed().as_millis();
    println!(
        "   ✅ 全链路检索完成: {}ms ({} results)",
        retrieve_ms,
        retrieved.len()
    );
    for (i, r) in retrieved.iter().enumerate().take(5) {
        let preview: String = r.chunk.content.chars().take(60).collect();
        println!(
            "   #{} score={:.4} doc=\"{}\" preview=\"{}...\"",
            i, r.score, r.doc_name, preview
        );
    }

    // -----------------------------------------------------------------------
    // 汇总报告
    // -----------------------------------------------------------------------
    let total_ms = t0.elapsed().as_millis();
    println!("\n{sep}");
    println!("📊 性能汇总报告");
    println!("{sep}");
    println!("{:<40} {:>12} {:>8}", "阶段", "耗时 (ms)", "占比");
    println!("{dash}");
    println!(
        "{:<40} {:>12} {:>7.1}%",
        "阶段 0: embedder 初始化",
        embedder_init_ms,
        embedder_init_ms as f64 / total_ms as f64 * 100.0
    );
    println!(
        "{:<40} {:>12} {:>7.1}%",
        "阶段 1: 文件加载",
        load_ms,
        load_ms as f64 / total_ms as f64 * 100.0
    );
    println!(
        "{:<40} {:>12} {:>7.1}%",
        "阶段 2: 文本分块",
        split_ms,
        split_ms as f64 / total_ms as f64 * 100.0
    );
    println!(
        "{:<40} {:>12} {:>7.1}%",
        "阶段 3: 文档+chunks 写入",
        db_write_ms,
        db_write_ms as f64 / total_ms as f64 * 100.0
    );
    println!(
        "{:<40} {:>12} {:>7.1}%",
        "阶段 4: 批量嵌入 ⚡",
        embed_batch_ms,
        embed_batch_ms as f64 / total_ms as f64 * 100.0
    );
    println!(
        "{:<40} {:>12} {:>7.1}%",
        "阶段 5: embeddings 写入",
        emb_write_ms,
        emb_write_ms as f64 / total_ms as f64 * 100.0
    );
    println!(
        "{:<40} {:>12} {:>7.1}%",
        "阶段 6: 查询嵌入",
        query_embed_ms,
        query_embed_ms as f64 / total_ms as f64 * 100.0
    );
    println!(
        "{:<40} {:>12} {:>7.1}%",
        "阶段 7: 向量检索",
        search_ms,
        search_ms as f64 / total_ms as f64 * 100.0
    );
    println!(
        "{:<40} {:>12} {:>7.1}%",
        "阶段 8: Chunk 扩展",
        expand_ms,
        expand_ms as f64 / total_ms as f64 * 100.0
    );
    println!(
        "{:<40} {:>12} {:>7.1}%",
        "阶段 9: 全链路检索",
        retrieve_ms,
        retrieve_ms as f64 / total_ms as f64 * 100.0
    );
    println!("{dash}");
    println!("{:<40} {:>12} {:>7.1}%", "总计", total_ms, 100.0);
    println!();

    // 识别瓶颈
    let stages = [
        ("embedder 初始化", embedder_init_ms),
        ("文件加载", load_ms),
        ("文本分块", split_ms),
        ("文档+chunks 写入", db_write_ms),
        ("批量嵌入", embed_batch_ms),
        ("embeddings 写入", emb_write_ms),
        ("查询嵌入", query_embed_ms),
        ("向量检索", search_ms),
        ("Chunk 扩展", expand_ms),
        ("全链路检索", retrieve_ms),
    ];
    let bottleneck = stages
        .iter()
        .max_by_key(|(_, ms)| *ms)
        .expect("stages 非空");
    println!(
        "🏆 最大瓶颈: {} ({}ms, {:.1}%)",
        bottleneck.0,
        bottleneck.1,
        bottleneck.1 as f64 / total_ms as f64 * 100.0
    );

    // 查询阶段延迟分解（不含导入）
    let query_total = query_embed_ms + search_ms + expand_ms;
    println!("\n🔍 查询阶段延迟分解（不含导入）:");
    println!(
        "   查询嵌入: {}ms ({:.1}%)",
        query_embed_ms,
        query_embed_ms as f64 / query_total as f64 * 100.0
    );
    println!(
        "   向量检索: {}ms ({:.1}%)",
        search_ms,
        search_ms as f64 / query_total as f64 * 100.0
    );
    println!(
        "   Chunk 扩展: {}ms ({:.1}%)",
        expand_ms,
        expand_ms as f64 / query_total as f64 * 100.0
    );
    println!("   查询总计: {}ms", query_total);

    // chat 延迟估算（不含 LLM 首 token）
    let chat_latency_excluding_llm = embedder_init_ms + query_embed_ms + search_ms + expand_ms;
    println!("\n💬 chat 延迟估算（不含 LLM 首 token）:");
    println!("   embedder 初始化: {}ms", embedder_init_ms);
    println!("   查询嵌入: {}ms", query_embed_ms);
    println!("   向量检索: {}ms", search_ms);
    println!("   Chunk 扩展: {}ms", expand_ms);
    println!("   小计: {}ms", chat_latency_excluding_llm);
    println!(
        "   剩余 = LLM 首 token 延迟 ≈ 20s - {}ms = {}ms",
        chat_latency_excluding_llm,
        20000i64 - chat_latency_excluding_llm as i64
    );

    println!("\n{sep}\n");
}

/// 重复查询延迟测试：测量多次查询的平均延迟（排除首次 embedder 初始化开销）。
#[tokio::test]
#[ignore = "真实文件性能测试：手动运行 cargo test -p echomind-infra -- --ignored perf_real_query --nocapture"]
async fn perf_real_query_latency() {
    // 语料守卫（V3.1 阶段一）：测试依赖本机真实语料（lisp-rs README_zh），
    // CI/其他机器无此文件时静默跳过（避免 chaos --include-ignored 误报）。
    if !Path::new(TEST_FILE).exists() {
        eprintln!("⚠️ 跳过：语料不存在 {}（CI 环境预期行为）", TEST_FILE);
        return;
    }
    let sep = "=".repeat(80);
    println!("\n{sep}");
    println!("🔄 重复查询延迟测试（5 次查询平均）");
    println!("{sep}");

    // 初始化 embedder
    let cache_dir = embedder_cache_dir();
    let embedder = LocalEmbedder::new(cache_dir)
        .await
        .expect("ONNX embedder 初始化失败");

    // 初始化 storage + 导入文件
    let tmpdir = TempDir::new().expect("tempdir 创建失败");
    let db_path = tmpdir.path().join("perf_query.db");
    let storage = SqliteStorage::new(&db_path).expect("storage 初始化失败");

    let loader = MarkdownLoader;
    let content = loader.load(TEST_FILE).await.expect("文件加载失败");

    let splitter = TextSplitter::new().expect("splitter 初始化失败");
    let chunk_texts = splitter.split(&content).await.expect("分块失败");

    use echomind_models::{Chunk, Document};
    let doc = Document::new("README_zh.md".to_string(), "perf-hash-query".to_string());
    storage.add_document(&doc).await.expect("add_document 失败");

    for (i, text) in chunk_texts.iter().enumerate() {
        let token_count = splitter.count_tokens(text).unwrap_or(0);
        let c = Chunk::new(doc.id.clone(), text.clone(), token_count, i);
        storage.add_chunk(&c).await.expect("add_chunk 失败");
    }

    let embeddings = embedder
        .embed_batch(&chunk_texts)
        .await
        .expect("embed_batch 失败");

    let stored_chunks = storage
        .list_chunks(&doc.id)
        .await
        .expect("list_chunks 失败");
    for (i, emb) in embeddings.iter().enumerate() {
        let chunk_id = stored_chunks
            .get(i)
            .map(|c| c.id.as_str())
            .unwrap_or_else(|| panic!("chunk {i} not found"));
        storage
            .add_embedding(chunk_id, emb)
            .await
            .expect("add_embedding 失败");
    }

    // 多次查询测试
    let queries = [
        "请解释一下Lisp的原理",
        "递归是怎么实现的",
        "什么是尾调用优化",
        "Lisp的解释器是如何工作的",
        "环境是什么概念",
    ];

    let mut total_ms = 0u128;
    let mut embed_times = Vec::new();
    let mut search_times = Vec::new();

    for (i, query) in queries.iter().enumerate() {
        let t = Instant::now();
        let query_vec = embedder.embed(query).await.expect("embed 失败");
        let embed_ms = t.elapsed().as_millis();
        embed_times.push(embed_ms);

        let t2 = Instant::now();
        let results = storage
            .vector_search(&query_vec, 5)
            .await
            .expect("vector_search 失败");
        let search_ms = t2.elapsed().as_millis();
        search_times.push(search_ms);

        let total = t.elapsed().as_millis();
        total_ms += total;
        println!(
            "  查询 #{}: \"{}\" → embed={}ms, search={}ms, total={}ms ({} results)",
            i + 1,
            query,
            embed_ms,
            search_ms,
            total,
            results.len()
        );
    }

    println!("\n📊 查询延迟统计:");
    println!(
        "   embed 平均: {:.1}ms (min={}, max={})",
        embed_times.iter().sum::<u128>() as f64 / embed_times.len() as f64,
        embed_times.iter().min().unwrap(),
        embed_times.iter().max().unwrap()
    );
    println!(
        "   search 平均: {:.1}ms (min={}, max={})",
        search_times.iter().sum::<u128>() as f64 / search_times.len() as f64,
        search_times.iter().min().unwrap(),
        search_times.iter().max().unwrap()
    );
    println!("   总平均: {:.1}ms", total_ms as f64 / queries.len() as f64);
    println!();
}
