#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! 首次聊天延迟性能基准测试（预热 vs 无预热）。
//!
//! 测试场景：
//! - 冷启动后首次聊天（无预热）：ONNX 模型需在首次聊天时加载
//! - 预热后首次聊天：ONNX 模型已在启动时预加载
//!
//! 测量指标：
//! - 首次聊天响应时间（从发送查询到收到首个 token）
//! - 预热耗时（ONNX 模型加载时间）
//! - 内存占用（预热前后对比）
//!
//! 运行方式：
//! ```bash
//! cargo test -p echomind-tauri-app --test chat_latency_bench -- --ignored --nocapture
//! ```

use std::time::{Duration, Instant};

use echomind_core::{Embedder, Storage};
use echomind_infra::local_embedder::{EmbeddingModel, LocalEmbedder, MirrorSource};
use echomind_infra::sqlite_storage::SqliteStorage;
use echomind_models::{Chunk, Document};

/// 测试用嵌入模型（默认 bge-small-en-v1.5，约 30MB）
const TEST_MODEL: EmbeddingModel = EmbeddingModel::BgeSmallEnV1_5;

/// 预热迭代次数（用于测量预热稳定性）
const WARMUP_ITERATIONS: usize = 3;

/// 聊天延迟测量迭代次数
const CHAT_BENCH_ITERATIONS: usize = 5;

// ---------------------------------------------------------------------------
// 辅助函数
// ---------------------------------------------------------------------------

/// 获取当前进程内存占用（字节）。
///
/// macOS: 通过 `mach_task_self` 获取物理内存使用
/// Linux: 读取 /proc/self/status
/// 其他平台: 返回 0（不支持）
fn get_memory_usage_bytes() -> u64 {
    #[cfg(target_os = "macos")]
    {
        use std::mem::MaybeUninit;
        unsafe extern "C" {
            fn mach_task_self() -> u32;
            fn task_info(
                task: u32,
                flavor: u32,
                info: *mut task_vm_info_data_t,
                count: *mut u32,
            ) -> u32;
        }

        #[repr(C)]
        struct task_vm_info_data_t {
            virtual_size: u64,
            region_size: u32,
            pages_resident: u64, // 实际物理内存使用
                                 // ... 其他字段省略
        }

        const TASK_VM_INFO: u32 = 22;
        const TASK_VM_INFO_COUNT: u32 = 4; // 只取前 4 个字段

        unsafe {
            let mut info: MaybeUninit<task_vm_info_data_t> = MaybeUninit::uninit();
            let mut count = TASK_VM_INFO_COUNT;
            let result = task_info(
                mach_task_self(),
                TASK_VM_INFO,
                info.as_mut_ptr(),
                &mut count,
            );
            if result == 0 {
                info.assume_init().pages_resident * 4096 // 页大小 4KB
            } else {
                0
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        use std::fs;
        if let Ok(status) = fs::read_to_string("/proc/self/status") {
            for line in status.lines() {
                if let Some(vmrss) = line.strip_prefix("VmRSS:") {
                    if let Some(kb) = vmrss.trim().strip_suffix(" kB") {
                        if let Ok(kb_val) = kb.parse::<u64>() {
                            return kb_val * 1024;
                        }
                    }
                }
            }
        }
        0
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        0
    }
}

/// 格式化字节数为人类可读格式。
fn format_bytes(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.2} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.0} B", bytes)
    }
}

/// 生成测试查询文本。
fn gen_test_query(seed: usize) -> String {
    format!("Benchmark test query for performance measurement iteration {seed}")
}

/// 创建测试数据库并插入模拟数据。
async fn setup_test_database(
    dir: &std::path::Path,
    num_chunks: usize,
) -> (SqliteStorage, Document) {
    let db_path = dir.join("test.db");
    let storage = SqliteStorage::new(&db_path).unwrap();

    // 创建测试文档
    let doc = Document::new("bench_test.md".to_string(), "bench-hash".to_string());
    storage.add_document(&doc).await.unwrap();

    // 批量插入 chunks 和 embeddings
    let mut chunks = Vec::with_capacity(num_chunks);
    let mut embeddings = Vec::with_capacity(num_chunks);

    for i in 0..num_chunks {
        let chunk = Chunk::new(
            doc.id.clone(),
            format!("Benchmark test content for chunk {i}"),
            10,
            i,
        );
        // 生成伪随机向量用于测试
        let vec = gen_test_vector(i);
        chunks.push(chunk.clone());
        embeddings.push((chunk.id, vec));
    }

    storage.add_chunks_batch(&chunks).await.unwrap();
    storage.add_embeddings_batch(&embeddings).await.unwrap();

    (storage, doc)
}

/// 生成测试用的伪随机向量（用于模拟数据库中的 embedding）。
fn gen_test_vector(seed: usize) -> Vec<f32> {
    let mut state = (seed as u64)
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    let mut vec = Vec::with_capacity(384);
    for _ in 0..384 {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let val = ((state >> 33) as i32 as f32) / (i32::MAX as f32);
        vec.push(val);
    }
    let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        vec.iter().map(|x| x / norm).collect()
    } else {
        vec
    }
}

// ---------------------------------------------------------------------------
// 场景 1：无预热 - 首次聊天延迟（ONNX 模型懒加载）
// ---------------------------------------------------------------------------

/// 测量无预热时的首次聊天延迟。
///
/// 模拟冷启动场景：ONNX 模型未加载，首次聊天时触发模型下载/加载。
/// 测量从 embedder 初始化到首次 embedding 完成的耗时。
#[tokio::test]
#[ignore = "性能基准：手动运行 cargo test -- --ignored bench_no_prewarm --nocapture"]
async fn bench_no_prewarm() {
    let dir = tempfile::tempdir().unwrap();
    let (_storage, _doc) = setup_test_database(dir.path(), 100).await;

    let mut latencies = Vec::with_capacity(CHAT_BENCH_ITERATIONS);
    let memory_before = get_memory_usage_bytes();

    println!("\n=== 场景 1：无预热 - 首次聊天延迟 ===");
    println!("模型: {:?}", TEST_MODEL);
    println!("数据规模: 100 chunks\n");

    for i in 0..CHAT_BENCH_ITERATIONS {
        // 每次迭代重新创建 embedder（模拟冷启动）
        let start = Instant::now();

        // 1. 初始化 embedder（首次加载 ONNX 模型）
        let cache_dir = dir.path().join("models").join(format!("iter_{i}"));
        let embedder = LocalEmbedder::new_with_mirror(
            cache_dir,
            TEST_MODEL.clone(),
            None,
            LocalEmbedder::default_pool_size(),
            MirrorSource::Auto,
        )
        .await
        .unwrap();

        let init_time = start.elapsed();

        // 2. 执行 embedding（模拟聊天中的查询向量化）
        let query = gen_test_query(1000 + i);
        let embed_start = Instant::now();
        let _embedding = embedder.embed(&query).await.unwrap();
        let embed_time = embed_start.elapsed();

        let total_time = start.elapsed();
        latencies.push(total_time.as_millis() as u64);

        println!(
            "迭代 {}: 初始化={}ms, Embedding={}ms, 总计={}ms",
            i + 1,
            init_time.as_millis(),
            embed_time.as_millis(),
            total_time.as_millis()
        );
    }

    let memory_after = get_memory_usage_bytes();
    let avg_latency = latencies.iter().sum::<u64>() as f64 / latencies.len() as f64;
    let max_latency = *latencies.iter().max().unwrap();
    let min_latency = *latencies.iter().min().unwrap();

    println!("\n--- 结果汇总 ---");
    println!("平均延迟: {:.1} ms", avg_latency);
    println!("最小延迟: {} ms", min_latency);
    println!("最大延迟: {} ms", max_latency);
    println!(
        "内存增长: {}",
        format_bytes(memory_after.saturating_sub(memory_before))
    );
    println!("========================================\n");

    // 基本断言（验证测量有效性）
    assert!(avg_latency > 0.0, "平均延迟应大于 0");
    assert!(latencies.len() == CHAT_BENCH_ITERATIONS, "应完成所有迭代");
}

// ---------------------------------------------------------------------------
// 场景 2：有预热 - 首次聊天延迟
// ---------------------------------------------------------------------------

/// 测量预热后的首次聊天延迟。
///
/// 模拟预热场景：ONNX 模型已在启动时加载，聊天时直接使用已加载的模型。
#[tokio::test]
#[ignore = "性能基准：手动运行 cargo test -- --ignored bench_with_prewarm --nocapture"]
async fn bench_with_prewarm() {
    let dir = tempfile::tempdir().unwrap();
    let (_storage, _doc) = setup_test_database(dir.path(), 100).await;

    let cache_dir = dir.path().join("models");

    println!("\n=== 场景 2：有预热 - 首次聊天延迟 ===");
    println!("模型: {:?}", TEST_MODEL);
    println!("预热次数: {}\n", WARMUP_ITERATIONS);

    // 预热阶段：加载 ONNX 模型
    let prewarm_start = Instant::now();
    let embedder = LocalEmbedder::new_with_mirror(
        cache_dir.clone(),
        TEST_MODEL.clone(),
        None,
        LocalEmbedder::default_pool_size(),
        MirrorSource::Auto,
    )
    .await
    .unwrap();
    let prewarm_time = prewarm_start.elapsed();

    // 预热 embedding 操作（确保 ONNX 会话完全初始化）
    for i in 0..WARMUP_ITERATIONS {
        let query = gen_test_query(2000 + i);
        let _ = embedder.embed(&query).await.unwrap();
    }
    let total_prewarm_time = prewarm_start.elapsed();

    println!("预热耗时:");
    println!("  模型加载: {} ms", prewarm_time.as_millis());
    println!("  包含预热迭代: {} ms", total_prewarm_time.as_millis());

    // 测量预热后的聊天延迟
    let mut latencies = Vec::with_capacity(CHAT_BENCH_ITERATIONS);
    let _memory_before = get_memory_usage_bytes();

    println!("\n测量预热后聊天延迟:\n");

    for i in 0..CHAT_BENCH_ITERATIONS {
        let start = Instant::now();

        // 执行 embedding（模拟聊天中的查询向量化）
        let query = gen_test_query(3000 + i);
        let _embedding = embedder.embed(&query).await.unwrap();

        let elapsed = start.elapsed();
        latencies.push(elapsed.as_millis() as u64);

        println!("迭代 {}: {} ms", i + 1, elapsed.as_millis());
    }

    let memory_after = get_memory_usage_bytes();
    let avg_latency = latencies.iter().sum::<u64>() as f64 / latencies.len() as f64;
    let max_latency = *latencies.iter().max().unwrap();
    let min_latency = *latencies.iter().min().unwrap();

    println!("\n--- 结果汇总 ---");
    println!("预热耗时: {} ms", total_prewarm_time.as_millis());
    println!("平均延迟: {:.1} ms", avg_latency);
    println!("最小延迟: {} ms", min_latency);
    println!("最大延迟: {} ms", max_latency);
    println!("内存占用: {}", format_bytes(memory_after));
    println!("========================================\n");

    // 基本断言
    assert!(avg_latency > 0.0, "平均延迟应大于 0");
    assert!(latencies.len() == CHAT_BENCH_ITERATIONS, "应完成所有迭代");
}

// ---------------------------------------------------------------------------
// 场景 3：预热耗时测量
// ---------------------------------------------------------------------------

/// 详细测量预热各阶段耗时。
///
/// 分解 ONNX 模型加载过程：
/// 1. 模型文件读取
/// 2. ONNX 会话构建
/// 3. 首次推理验证
#[tokio::test]
#[ignore = "性能基准：手动运行 cargo test -- --ignored bench_prewarm_breakdown --nocapture"]
async fn bench_prewarm_breakdown() {
    let dir = tempfile::tempdir().unwrap();

    println!("\n=== 场景 3：预热耗时分解 ===");
    println!("模型: {:?}\n", TEST_MODEL);

    // 测量不同规模下的预热耗时
    let chunk_counts = [0, 100, 500, 1000];

    for &chunk_count in &chunk_counts {
        let test_dir = dir.path().join(format!("chunks_{chunk_count}"));
        std::fs::create_dir_all(&test_dir).unwrap();

        let start = Instant::now();

        // 加载模型
        let cache_dir = test_dir.join("models");
        let embedder = LocalEmbedder::new_with_mirror(
            cache_dir,
            TEST_MODEL.clone(),
            None,
            LocalEmbedder::default_pool_size(),
            MirrorSource::Auto,
        )
        .await
        .unwrap();

        let load_time = start.elapsed();

        // 首次 embedding（验证模型可用）
        let query = gen_test_query(999);
        let verify_start = Instant::now();
        let _ = embedder.embed(&query).await.unwrap();
        let verify_time = verify_start.elapsed();

        let total_time = start.elapsed();

        println!("数据规模: {} chunks", chunk_count);
        println!("  模型加载: {} ms", load_time.as_millis());
        println!("  验证推理: {} ms", verify_time.as_millis());
        println!("  总预热:   {} ms\n", total_time.as_millis());
    }

    println!("========================================\n");
}

// ---------------------------------------------------------------------------
// 场景 4：内存占用对比
// ---------------------------------------------------------------------------

/// 对比预热前后的内存占用。
///
/// 测量：
/// 1. 空进程内存基线
/// 2. 加载 ONNX 模型后内存
/// 3. 执行多次 embedding 后内存
/// 4. 内存释放（drop embedder）
#[tokio::test]
#[ignore = "性能基准：手动运行 cargo test -- --ignored bench_memory_usage --nocapture"]
async fn bench_memory_usage() {
    let dir = tempfile::tempdir().unwrap();

    println!("\n=== 场景 4：内存占用对比 ===");
    println!("模型: {:?}\n", TEST_MODEL);

    // 基线内存
    let baseline_memory = get_memory_usage_bytes();
    println!("基线内存: {}", format_bytes(baseline_memory));

    // 加载模型
    let cache_dir = dir.path().join("models");
    let embedder = LocalEmbedder::new_with_mirror(
        cache_dir,
        TEST_MODEL.clone(),
        None,
        LocalEmbedder::default_pool_size(),
        MirrorSource::Auto,
    )
    .await
    .unwrap();

    let after_load_memory = get_memory_usage_bytes();
    println!(
        "加载模型后: {} (+{})",
        format_bytes(after_load_memory),
        format_bytes(after_load_memory.saturating_sub(baseline_memory))
    );

    // 执行多次 embedding
    for i in 0..10 {
        let query = gen_test_query(i);
        let _ = embedder.embed(&query).await.unwrap();
    }

    let after_embed_memory = get_memory_usage_bytes();
    println!(
        "10次 embedding 后: {} (+{})",
        format_bytes(after_embed_memory),
        format_bytes(after_embed_memory.saturating_sub(baseline_memory))
    );

    // 释放 embedder
    drop(embedder);

    // 等待一段时间让系统回收内存
    tokio::time::sleep(Duration::from_millis(100)).await;

    let after_drop_memory = get_memory_usage_bytes();
    println!(
        "释放模型后: {} (-{})",
        format_bytes(after_drop_memory),
        format_bytes(after_embed_memory.saturating_sub(after_drop_memory))
    );

    println!("\n--- 内存分析 ---");
    println!(
        "ONNX 模型内存开销: ~{}",
        format_bytes(after_load_memory.saturating_sub(baseline_memory))
    );
    println!(
        "Embedding 缓存开销: ~{}",
        format_bytes(after_embed_memory.saturating_sub(after_load_memory))
    );
    println!("========================================\n");
}

// ---------------------------------------------------------------------------
// 场景 5：综合对比（无预热 vs 有预热）
// ---------------------------------------------------------------------------

/// 综合对比无预热和有预热的性能差异。
///
/// 输出性能提升百分比和用户体验影响分析。
#[tokio::test]
#[ignore = "性能基准：手动运行 cargo test -- --ignored bench_comparison --nocapture"]
async fn bench_comparison() {
    let dir = tempfile::tempdir().unwrap();
    let (_storage, _doc) = setup_test_database(dir.path(), 100).await;

    println!("\n=== 综合对比：无预热 vs 有预热 ===\n");

    // ---- 无预热场景 ----
    let mut no_prewarm_latencies = Vec::with_capacity(CHAT_BENCH_ITERATIONS);
    for i in 0..CHAT_BENCH_ITERATIONS {
        let cache_dir = dir.path().join("no_prewarm").join(format!("iter_{i}"));
        let start = Instant::now();
        let embedder = LocalEmbedder::new_with_mirror(
            cache_dir,
            TEST_MODEL.clone(),
            None,
            LocalEmbedder::default_pool_size(),
            MirrorSource::Auto,
        )
        .await
        .unwrap();
        let query = gen_test_query(1000 + i);
        let _ = embedder.embed(&query).await.unwrap();
        no_prewarm_latencies.push(start.elapsed().as_millis() as u64);
    }

    // ---- 有预热场景 ----
    let cache_dir = dir.path().join("with_prewarm").join("models");
    let prewarm_start = Instant::now();
    let embedder = LocalEmbedder::new_with_mirror(
        cache_dir,
        TEST_MODEL.clone(),
        None,
        LocalEmbedder::default_pool_size(),
        MirrorSource::Auto,
    )
    .await
    .unwrap();
    let prewarm_time = prewarm_start.elapsed();

    // 预热 embedding
    for i in 0..WARMUP_ITERATIONS {
        let query = gen_test_query(2000 + i);
        let _ = embedder.embed(&query).await.unwrap();
    }

    let mut with_prewarm_latencies = Vec::with_capacity(CHAT_BENCH_ITERATIONS);
    for i in 0..CHAT_BENCH_ITERATIONS {
        let query = gen_test_query(3000 + i);
        let start = Instant::now();
        let _ = embedder.embed(&query).await.unwrap();
        with_prewarm_latencies.push(start.elapsed().as_millis() as u64);
    }

    // ---- 计算统计 ----
    let no_prewarm_avg =
        no_prewarm_latencies.iter().sum::<u64>() as f64 / no_prewarm_latencies.len() as f64;
    let with_prewarm_avg =
        with_prewarm_latencies.iter().sum::<u64>() as f64 / with_prewarm_latencies.len() as f64;
    let speedup = no_prewarm_avg / with_prewarm_avg.max(1.0);
    let improvement_pct = ((no_prewarm_avg - with_prewarm_avg) / no_prewarm_avg) * 100.0;

    println!("--- 性能对比 ---");
    println!("无预热平均延迟: {:.1} ms", no_prewarm_avg);
    println!("有预热平均延迟: {:.1} ms", with_prewarm_avg);
    println!("预热耗时: {} ms", prewarm_time.as_millis());
    println!("性能提升: {:.1}%", improvement_pct);
    println!("加速比: {:.2}x\n", speedup);

    println!("--- 用户体验影响 ---");
    if no_prewarm_avg > 5000.0 {
        println!("⚠️  无预热首次聊天延迟超过 5 秒，用户体验较差");
        println!("   建议：启动时预热 ONNX 模型，避免首次聊天阻塞");
    } else if no_prewarm_avg > 2000.0 {
        println!("⚡ 无预热首次聊天延迟 2-5 秒，用户可感知延迟");
        println!("   建议：考虑后台预热或增量加载");
    } else {
        println!("✅ 无预热首次聊天延迟 < 2 秒，用户体验可接受");
    }

    if with_prewarm_avg < 100.0 {
        println!("✅ 有预热聊天延迟 < 100ms，响应流畅");
    } else if with_prewarm_avg < 500.0 {
        println!("⚡ 有预热聊天延迟 100-500ms，基本流畅");
    } else {
        println!("⚠️  有预热聊天延迟 > 500ms，可能需要进一步优化");
    }

    println!("\n--- 优化建议 ---");
    println!("1. 启动时预热：在应用启动时加载 ONNX 模型，避免首次聊天阻塞");
    println!("2. 后台预热：使用异步任务在后台预加载模型，显示加载进度");
    println!("3. 增量加载：模型文件分块加载，优先加载核心层");
    println!("4. 模型缓存：将加载后的模型实例缓存在内存中，避免重复加载");

    println!("\n========================================\n");

    // 基本断言
    assert!(no_prewarm_avg > 0.0, "无预热延迟应大于 0");
    assert!(with_prewarm_avg > 0.0, "有预热延迟应大于 0");
    assert!(speedup > 1.0, "有预热应比无预热快");
}

// ---------------------------------------------------------------------------
// 场景 6：并发聊天延迟
// ---------------------------------------------------------------------------

/// 测量并发聊天场景下的延迟表现。
///
/// 模拟多用户同时发起聊天，测量平均延迟和 P95 延迟。
#[tokio::test]
#[ignore = "性能基准：手动运行 cargo test -- --ignored bench_concurrent聊天 --nocapture"]
async fn bench_concurrent聊天() {
    let dir = tempfile::tempdir().unwrap();

    println!("\n=== 场景 6：并发聊天延迟 ===\n");

    let cache_dir = dir.path().join("models");
    let embedder = LocalEmbedder::new_with_mirror(
        cache_dir,
        TEST_MODEL.clone(),
        None,
        LocalEmbedder::default_pool_size(),
        MirrorSource::Auto,
    )
    .await
    .unwrap();

    // 预热
    for i in 0..WARMUP_ITERATIONS {
        let query = gen_test_query(i);
        let _ = embedder.embed(&query).await.unwrap();
    }

    // 并发测试
    let concurrency_levels = [1, 2, 4, 8];
    for &concurrency in &concurrency_levels {
        let mut latencies = Vec::with_capacity(concurrency * 5);
        let start = Instant::now();

        let mut handles = Vec::with_capacity(concurrency);
        for i in 0..concurrency {
            let embedder_clone = embedder.clone();
            let handle = tokio::spawn(async move {
                let mut local_latencies = Vec::new();
                for j in 0..5 {
                    let query = gen_test_query(i * 100 + j);
                    let start = Instant::now();
                    let _ = embedder_clone.embed(&query).await.unwrap();
                    local_latencies.push(start.elapsed().as_millis() as u64);
                }
                local_latencies
            });
            handles.push(handle);
        }

        for handle in handles {
            let local_latencies = handle.await.unwrap();
            latencies.extend(local_latencies);
        }

        let total_time = start.elapsed();
        let avg = latencies.iter().sum::<u64>() as f64 / latencies.len() as f64;
        let mut sorted = latencies.clone();
        sorted.sort_unstable();
        let p95_idx = (sorted.len() as f64 * 0.95).ceil() as usize;
        let p95 = sorted[p95_idx.min(sorted.len() - 1)];

        println!(
            "并发 {}: 平均={:.1}ms, P95={}ms, 总时间={}ms",
            concurrency,
            avg,
            p95,
            total_time.as_millis()
        );
    }

    println!("\n========================================\n");
}
