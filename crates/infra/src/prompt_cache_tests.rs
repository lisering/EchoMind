//! Prompt Prefix 磁盘缓存 TDD 测试（DS-01：借鉴 ds4 `ds4_kvstore.c`）。

use crate::prompt_cache::*;

use tempfile::TempDir;

/// 辅助：创建临时缓存目录。
fn make_cache() -> (TempDir, PromptCache) {
    let dir = TempDir::new().unwrap();
    let cache = PromptCache::open(dir.path(), PromptCacheConfig::default()).unwrap();
    (dir, cache)
}

/// 辅助：创建小型配置（测试用）。
fn make_small_config() -> PromptCacheConfig {
    PromptCacheConfig {
        budget_bytes: 1024 * 1024, // 1 MB
        min_tokens: 10,
        cold_max_tokens: 100,
        continued_interval_tokens: 20,
        boundary_trim_tokens: 2,
        boundary_align_tokens: 5,
    }
}

/// 辅助：生成假 token IDs。
fn fake_tokens(n: usize) -> Vec<u32> {
    (0..n as u32).collect()
}

/// 辅助：生成假 prompt 文本（每 token 1 字符）。
fn fake_prompt(n: usize) -> String {
    (0..n).map(|_| 'a').collect()
}

// ============================================================
// DS-01-T01: SHA1 计算正确性
// ============================================================

#[test]
fn test_sha1_hex_correctness() {
    let sha = PromptCache::sha1_hex("hello");
    assert_eq!(sha.len(), 40);
    assert_eq!(sha, "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d");
}

#[test]
fn test_sha1_hex_empty_string() {
    let sha = PromptCache::sha1_hex("");
    assert_eq!(sha, "da39a3ee5e6b4b0d3255bfef95601890afd80709");
}

#[test]
fn test_sha1_hex_unicode() {
    let sha = PromptCache::sha1_hex("你好世界");
    assert_eq!(sha.len(), 40);
    // 确保是有效 hex
    assert!(sha.chars().all(|c| c.is_ascii_hexdigit()));
}

// ============================================================
// DS-01-T02: 缓存打开 + 关闭
// ============================================================

#[test]
fn test_open_creates_directory() {
    let dir = TempDir::new().unwrap();
    let cache_dir = dir.path().join("subdir").join("cache");
    let _cache = PromptCache::open(&cache_dir, PromptCacheConfig::default()).unwrap();
    assert!(cache_dir.exists());
}

#[test]
fn test_open_empty_directory() {
    let (_dir, cache) = make_cache();
    assert!(cache.is_empty());
}

// ============================================================
// DS-01-T03: 存储 + 精确加载
// ============================================================

#[test]
fn test_store_and_load_exact() {
    let (_dir, mut cache) = make_cache();
    let prompt = fake_prompt(100);
    let tokens = fake_tokens(100);

    cache
        .store(&prompt, &tokens, "test-model", 4096, CacheReason::Cold)
        .unwrap();

    assert_eq!(cache.len(), 1);

    // 精确查找
    let entry = cache.find_exact(&prompt, "test-model", 4096).unwrap();
    assert_eq!(entry.tokens, 100);
    assert_eq!(entry.model_name, "test-model");
    assert_eq!(entry.ctx_size, 4096);
    assert_eq!(entry.reason, CacheReason::Cold);

    // 加载 token IDs
    let result = cache.load_tokens(entry).unwrap();
    assert_eq!(result.token_ids, tokens);
    assert_eq!(result.tokens, 100);
}

// ============================================================
// DS-01-T04: 最小 token 数过滤
// ============================================================

#[test]
fn test_store_below_min_tokens_skipped() {
    let dir = TempDir::new().unwrap();
    let config = PromptCacheConfig {
        min_tokens: 50,
        ..PromptCacheConfig::default()
    };
    let mut cache = PromptCache::open(dir.path(), config).unwrap();

    let prompt = fake_prompt(30);
    let tokens = fake_tokens(30);

    cache
        .store(&prompt, &tokens, "test-model", 4096, CacheReason::Cold)
        .unwrap();

    // 应被跳过（30 < 50）
    assert_eq!(cache.len(), 0);
}

// ============================================================
// DS-01-T05: 边界裁剪 + 对齐
// ============================================================

#[test]
fn test_compute_store_len_no_trim_when_small() {
    let config = make_small_config();
    let dir = TempDir::new().unwrap();
    let cache = PromptCache::open(dir.path(), config).unwrap();

    // 12 tokens, min=10, trim=2, align=5
    // 12 > 10+2=12? no, equal, so no trim
    // 实际上 12 > 12 is false, so no trim → return 12
    let len = cache.compute_store_len(12);
    assert_eq!(len, 12);
}

#[test]
fn test_compute_store_len_with_trim_and_align() {
    let config = make_small_config();
    let dir = TempDir::new().unwrap();
    let cache = PromptCache::open(dir.path(), config).unwrap();

    // 20 tokens, min=10, trim=2, align=5
    // 20 > 10+2=12? yes
    // stable = 20-2 = 18
    // aligned = 18 - (18 % 5) = 18 - 3 = 15
    // 15 >= 10? yes → return 15
    let len = cache.compute_store_len(20);
    assert_eq!(len, 15);
}

#[test]
fn test_compute_store_len_trim_below_min_returns_full() {
    let config = PromptCacheConfig {
        min_tokens: 100,
        boundary_trim_tokens: 32,
        boundary_align_tokens: 2048,
        ..PromptCacheConfig::default()
    };
    let dir = TempDir::new().unwrap();
    let cache = PromptCache::open(dir.path(), config).unwrap();

    // 120 tokens, min=100, trim=32, align=2048
    // 120 > 100+32=132? no → return 120
    let len = cache.compute_store_len(120);
    assert_eq!(len, 120);
}

// ============================================================
// DS-01-T06: 持续保存间隔
// ============================================================

#[test]
fn test_continued_store_target_at_interval() {
    let dir = TempDir::new().unwrap();
    let config = PromptCacheConfig {
        min_tokens: 10,
        continued_interval_tokens: 100,
        ..PromptCacheConfig::default()
    };
    let mut cache = PromptCache::open(dir.path(), config).unwrap();

    // 200 tokens, interval=100, 200 % 100 == 0, 200 > 0 (last_store)
    let target = cache.continued_store_target(200);
    assert_eq!(target, Some(200));
}

#[test]
fn test_continued_store_target_not_at_interval() {
    let dir = TempDir::new().unwrap();
    let config = PromptCacheConfig {
        min_tokens: 10,
        continued_interval_tokens: 100,
        ..PromptCacheConfig::default()
    };
    let cache = PromptCache::open(dir.path(), config).unwrap();

    // 150 tokens, 150 % 100 != 0
    let target = cache.continued_store_target(150);
    assert_eq!(target, None);
}

#[test]
fn test_continued_store_target_below_min() {
    let dir = TempDir::new().unwrap();
    let config = PromptCacheConfig {
        min_tokens: 100,
        continued_interval_tokens: 50,
        ..PromptCacheConfig::default()
    };
    let cache = PromptCache::open(dir.path(), config).unwrap();

    // 50 tokens, min=100
    let target = cache.continued_store_target(50);
    assert_eq!(target, None);
}

#[test]
fn test_continued_store_target_already_stored() {
    let dir = TempDir::new().unwrap();
    let config = PromptCacheConfig {
        min_tokens: 10,
        continued_interval_tokens: 100,
        ..PromptCacheConfig::default()
    };
    let mut cache = PromptCache::open(dir.path(), config).unwrap();

    // 先存储 200
    let prompt = fake_prompt(200);
    let tokens = fake_tokens(200);
    cache
        .store(&prompt, &tokens, "test-model", 4096, CacheReason::Continued)
        .unwrap();

    // 再次检查 200，应该返回 None（已存储）
    let target = cache.continued_store_target(200);
    assert_eq!(target, None);
}

// ============================================================
// DS-01-T07: LRU 驱逐评分
// ============================================================

#[test]
fn test_eviction_score_zero_file_size() {
    let entry = CacheEntry {
        sha: "abc".to_string(),
        path: PathBuf::new(),
        model_name: "test".to_string(),
        tokens: 100,
        ctx_size: 4096,
        reason: CacheReason::Cold,
        hits: 10,
        created_at: 1000,
        last_used: 2000,
        file_size: 0,
    };
    assert_eq!(eviction_score(&entry, 3000), 0.0);
}

#[test]
fn test_eviction_score_basic() {
    let entry = CacheEntry {
        sha: "abc".to_string(),
        path: PathBuf::new(),
        model_name: "test".to_string(),
        tokens: 1000,
        ctx_size: 4096,
        reason: CacheReason::Continued,
        hits: 5,
        created_at: 1000,
        last_used: 2000,
        file_size: 10000,
    };
    // now=2000, elapsed=0, effective_hits=5
    // score = (5+1) * 1000 / 10000 = 0.6
    let score = eviction_score(&entry, 2000);
    assert!((score - 0.6).abs() < 0.001);
}

#[test]
fn test_eviction_score_anchor_doubled() {
    let entry = CacheEntry {
        sha: "abc".to_string(),
        path: PathBuf::new(),
        model_name: "test".to_string(),
        tokens: 1000,
        ctx_size: 4096,
        reason: CacheReason::Cold, // anchor
        hits: 5,
        created_at: 1000,
        last_used: 2000,
        file_size: 10000,
    };
    // score = 0.6 * 2.0 = 1.2
    let score = eviction_score(&entry, 2000);
    assert!((score - 1.2).abs() < 0.001);
}

#[test]
fn test_eviction_score_decay_over_time() {
    let entry = CacheEntry {
        sha: "abc".to_string(),
        path: PathBuf::new(),
        model_name: "test".to_string(),
        tokens: 1000,
        ctx_size: 4096,
        reason: CacheReason::Continued,
        hits: 10,
        created_at: 1000,
        last_used: 1000,
        file_size: 10000,
    };
    // now = 1000 + 6h = 1000 + 21600
    // elapsed = 21600 = half_life
    // effective_hits = 10 * 2^(-1) = 5
    // score = (5+1) * 1000 / 10000 = 0.6
    let now = 1000 + (HIT_HALF_LIFE_SECONDS as u64);
    let score = eviction_score(&entry, now);
    assert!((score - 0.6).abs() < 0.01);
}

// ============================================================
// DS-01-T08: 驱逐逻辑
// ============================================================

#[test]
fn test_evict_removes_lowest_score() {
    let dir = TempDir::new().unwrap();
    let config = PromptCacheConfig {
        budget_bytes: 500, // very small budget
        min_tokens: 5,
        cold_max_tokens: 1000,
        continued_interval_tokens: 1000,
        boundary_trim_tokens: 0,
        boundary_align_tokens: 0,
    };
    let mut cache = PromptCache::open(dir.path(), config).unwrap();

    // 存储两个条目
    let prompt1 = fake_prompt(20);
    let tokens1 = fake_tokens(20);
    cache
        .store(&prompt1, &tokens1, "model", 4096, CacheReason::Cold)
        .unwrap();

    let prompt2 = fake_prompt(30);
    let tokens2 = fake_tokens(30);
    cache
        .store(&prompt2, &tokens2, "model", 4096, CacheReason::Cold)
        .unwrap();

    assert_eq!(cache.len(), 2);

    // 触发驱逐（需要额外 500 字节，但预算只有 500）
    cache.evict(500).unwrap();

    // 至少驱逐一个
    assert!(cache.len() <= 1);
}

// ============================================================
// DS-01-T09: 模型名称不匹配
// ============================================================

#[test]
fn test_find_exact_wrong_model() {
    let (_dir, mut cache) = make_cache();
    let prompt = fake_prompt(100);
    let tokens = fake_tokens(100);

    cache
        .store(&prompt, &tokens, "model-a", 4096, CacheReason::Cold)
        .unwrap();

    // 用不同模型名查找
    let result = cache.find_exact(&prompt, "model-b", 4096);
    assert!(result.is_none());
}

// ============================================================
// DS-01-T10: ctx_size 过滤
// ============================================================

#[test]
fn test_find_exact_ctx_size_too_small() {
    let (_dir, mut cache) = make_cache();
    let prompt = fake_prompt(100);
    let tokens = fake_tokens(100);

    // 存储时 ctx_size=8192
    cache
        .store(&prompt, &tokens, "test-model", 8192, CacheReason::Cold)
        .unwrap();

    // 用更小的 ctx_size 查找（缓存条目的 ctx_size > 请求的 ctx_size）
    let result = cache.find_exact(&prompt, "test-model", 4096);
    assert!(result.is_none());
}

// ============================================================
// DS-01-T11: CacheReason 序列化
// ============================================================

#[test]
fn test_cache_reason_to_byte_roundtrip() {
    for reason in [
        CacheReason::Unknown,
        CacheReason::Cold,
        CacheReason::Continued,
        CacheReason::Evict,
        CacheReason::Shutdown,
    ] {
        let byte = reason.to_byte();
        let restored = CacheReason::from_byte(byte);
        assert_eq!(reason, restored);
    }
}

#[test]
fn test_cache_reason_is_anchor() {
    assert!(!CacheReason::Unknown.is_anchor());
    assert!(CacheReason::Cold.is_anchor());
    assert!(!CacheReason::Continued.is_anchor());
    assert!(CacheReason::Evict.is_anchor());
    assert!(CacheReason::Shutdown.is_anchor());
}

// ============================================================
// DS-01-T12: 重复存储不创建新文件
// ============================================================

#[test]
fn test_store_duplicate_skips() {
    let (_dir, mut cache) = make_cache();
    let prompt = fake_prompt(100);
    let tokens = fake_tokens(100);

    cache
        .store(&prompt, &tokens, "test-model", 4096, CacheReason::Cold)
        .unwrap();
    assert_eq!(cache.len(), 1);

    // 再次存储相同内容
    cache
        .store(&prompt, &tokens, "test-model", 4096, CacheReason::Cold)
        .unwrap();
    assert_eq!(cache.len(), 1);
}

// ============================================================
// DS-01-T13: 原子写入（无残留 .tmp 文件）
// ============================================================

#[test]
fn test_store_atomic_no_tmp_left() {
    let (dir, mut cache) = make_cache();
    let prompt = fake_prompt(100);
    let tokens = fake_tokens(100);

    cache
        .store(&prompt, &tokens, "test-model", 4096, CacheReason::Cold)
        .unwrap();

    // 检查无 .tmp 文件残留
    for entry in fs::read_dir(dir.path()).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        assert!(!name_str.contains(".tmp"), "found tmp file: {name_str}");
    }
}

// ============================================================
// DS-01-T14: 持久化跨实例
// ============================================================

#[test]
fn test_persistence_across_instances() {
    let dir = TempDir::new().unwrap();

    // 第一个实例存储
    let prompt = fake_prompt(100);
    let tokens = fake_tokens(100);
    {
        let mut cache = PromptCache::open(dir.path(), PromptCacheConfig::default()).unwrap();
        cache
            .store(&prompt, &tokens, "test-model", 4096, CacheReason::Cold)
            .unwrap();
    }

    // 第二个实例加载
    {
        let cache = PromptCache::open(dir.path(), PromptCacheConfig::default()).unwrap();
        assert_eq!(cache.len(), 1);

        let entry = cache.find_exact(&prompt, "test-model", 4096).unwrap();
        let result = cache.load_tokens(entry).unwrap();
        assert_eq!(result.token_ids, tokens);
    }
}

// ============================================================
// DS-01-T15: find_exact 不存在时返回 None
// ============================================================

#[test]
fn test_find_exact_not_found() {
    let (_dir, cache) = make_cache();
    let result = cache.find_exact("nonexistent", "model", 4096);
    assert!(result.is_none());
}
