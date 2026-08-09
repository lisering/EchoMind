#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! LayerPrefetcher 单元测试（Phase 3 Session 21）。
//!
//! 测试覆盖：预取器创建、单层预取、范围预取、越界安全、停止、页面驻留检查。
//! TC-PREFETCH-006 标记为 `#[ignore]`，需手动运行验证 madvise 效果。

use super::layer_prefetch::*;

/// 创建测试用 mmap（从临时文件映射）。
///
/// `Mmap` 只能从 `File` 创建，这里使用临时文件。
/// 在 Unix 上，mmap 持有文件引用，即使 `NamedTempFile` 被 drop，mmap 仍有效。
fn create_test_mmap(size: usize) -> memmap2::Mmap {
    use std::io::Write;
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    let data = vec![0u8; size];
    tmp.write_all(&data).unwrap();
    tmp.flush().unwrap();
    // 注意：返回 Mmap，临时文件在 NamedTempFile drop 时删除
    // 但 Mmap 仍持有文件映射，只要 Mmap 存活就有效（Unix semantics）
    unsafe { memmap2::Mmap::map(tmp.as_file()) }.unwrap()
}

/// TC-PREFETCH-001：预取器创建成功。
///
/// 从 mmap 和 layer_offsets 创建 `LayerPrefetcher`，
/// 验证初始状态为非活跃、layer_offsets 正确存储。
#[test]
fn test_prefetcher_creation() {
    let mmap = create_test_mmap(4096);
    let offsets = vec![
        ("layer.0.weight".to_string(), 0, 1024),
        ("layer.1.weight".to_string(), 1024, 1024),
        ("layer.2.weight".to_string(), 2048, 1024),
    ];
    let prefetcher = LayerPrefetcher::new(&mmap, offsets.clone());

    assert!(!prefetcher.is_active(), "新建的预取器应处于非活跃状态");
    assert_eq!(
        prefetcher.layer_offsets().len(),
        3,
        "layer_offsets 应有 3 项"
    );
    assert_eq!(
        prefetcher.layer_offsets()[0].0,
        "layer.0.weight",
        "第一层名称应匹配"
    );
}

/// TC-PREFETCH-002：prefetch_next 不 panic。
///
/// 对有效层索引调用 `prefetch_next`，验证不 panic 且
/// `is_active()` 变为 `true`。
#[test]
fn test_prefetch_next_no_panic() {
    let mmap = create_test_mmap(4096);
    let offsets = vec![
        ("layer.0.weight".to_string(), 0, 1024),
        ("layer.1.weight".to_string(), 1024, 1024),
    ];
    let prefetcher = LayerPrefetcher::new(&mmap, offsets);

    // 预取第 0 层（有效索引）
    prefetcher.prefetch_next(0);
    assert!(
        prefetcher.is_active(),
        "prefetch_next 后 is_active 应为 true"
    );

    // 预取第 1 层（有效索引）
    prefetcher.prefetch_next(1);
    assert!(prefetcher.is_active());
}

/// TC-PREFETCH-003：prefetch_range 不 panic。
///
/// 对有效范围调用 `prefetch_range`，验证不 panic。
/// 等待后台线程完成后验证状态。
#[test]
fn test_prefetch_range_no_panic() {
    let mmap = create_test_mmap(8192);
    let offsets: Vec<(String, u64, u64)> = (0..4)
        .map(|i| (format!("layer.{i}.weight"), (i * 2048) as u64, 2048))
        .collect();
    let prefetcher = LayerPrefetcher::new(&mmap, offsets);

    // 预取第 0~2 层（3 层）
    prefetcher.prefetch_range(0, 3);
    assert!(
        prefetcher.is_active(),
        "prefetch_range 后 is_active 应为 true"
    );

    // 等待后台线程完成
    std::thread::sleep(std::time::Duration::from_millis(50));
}

/// TC-PREFETCH-004：越界预取安全返回（不 panic）。
///
/// 对超出范围的层索引调用 `prefetch_next` 和 `prefetch_range`，
/// 验证不 panic。
#[test]
fn test_prefetch_out_of_bounds_safe() {
    let mmap = create_test_mmap(1024);
    let offsets = vec![("layer.0.weight".to_string(), 0, 512)];
    let prefetcher = LayerPrefetcher::new(&mmap, offsets);

    // 越界索引：layer_offsets 只有 1 项，请求索引 5
    prefetcher.prefetch_next(5);
    // 不 panic，且 is_active 仍为 false（未实际预取）
    assert!(!prefetcher.is_active(), "越界 prefetch_next 不应激活预取器");

    // 越界范围：start 超出范围
    prefetcher.prefetch_range(10, 5);
    // 不 panic

    // 部分越界范围：start 有效但 start+count 超出范围
    prefetcher.prefetch_range(0, 100);
    // 不 panic，自动截断到实际层数
}

/// TC-PREFETCH-005：stop 后 is_active 返回 false。
///
/// 先调用 `prefetch_range` 激活预取器，再调用 `stop()`，
/// 验证 `is_active()` 返回 `false`。
#[test]
fn test_prefetch_stop() {
    let mmap = create_test_mmap(4096);
    let offsets = vec![
        ("layer.0.weight".to_string(), 0, 1024),
        ("layer.1.weight".to_string(), 1024, 1024),
    ];
    let prefetcher = LayerPrefetcher::new(&mmap, offsets);

    // 激活预取器
    prefetcher.prefetch_range(0, 2);
    assert!(prefetcher.is_active(), "prefetch_range 后应活跃");

    // 停止预取器
    prefetcher.stop();
    assert!(!prefetcher.is_active(), "stop 后 is_active 应为 false");

    // 再次 stop（幂等性）
    prefetcher.stop();
    assert!(!prefetcher.is_active());
}

/// TC-PREFETCH-006：预取后页面在 RAM 中（mincore 检查）。
///
/// 验证 `madvise(MADV_WILLNEED)` 后页面被加载到 page cache。
/// 使用 `mincore` 系统调用检查页面驻留状态。
///
/// **注意**：此测试依赖真实文件 I/O 和 OS page cache 行为，
/// 标记为 `#[ignore]`，手动运行：
///
/// ```bash
/// cargo test -p echomind-infra --features pro -- test_prefetch_pages_loaded --ignored --nocapture
/// ```
#[test]
#[ignore]
fn test_prefetch_pages_loaded() {
    use std::io::Write;

    // 创建临时文件并写入数据
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    let page_size = get_page_size();
    let data_size = page_size * 4; // 4 页
    let data = vec![0xABu8; data_size];
    tmp.write_all(&data).unwrap();
    tmp.flush().unwrap();

    // mmap 映射文件
    let mmap = unsafe { memmap2::Mmap::map(tmp.as_file()) }.unwrap();

    // 创建预取器（偏移量覆盖整个文件）
    let offsets = vec![("data".to_string(), 0, data_size as u64)];
    let prefetcher = LayerPrefetcher::new(&mmap, offsets);

    // 预取前：检查页面是否在 RAM 中（应该不在，因为是新 mmap）
    // 注意：某些 OS 可能已经预读了部分页面，所以这里不做严格断言

    // 执行预取
    prefetcher.prefetch_next(0);

    // 等待 OS 后台加载页面
    std::thread::sleep(std::time::Duration::from_millis(100));

    // 预取后：检查页面是否在 RAM 中
    #[cfg(unix)]
    {
        let pages_in_ram = check_pages_in_ram(mmap.as_ptr(), data_size, page_size);
        eprintln!(
            "[TC-PREFETCH-006] pages_in_ram = {}/{}",
            pages_in_ram,
            data_size / page_size
        );
        // 至少部分页面应该在 RAM 中
        assert!(pages_in_ram > 0, "预取后应至少有 1 个页面在 RAM 中");
    }

    prefetcher.stop();
}

// ---- 测试辅助函数 ----

/// 获取系统页大小。
#[cfg(unix)]
fn get_page_size() -> usize {
    unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize }
}

#[cfg(not(unix))]
#[allow(dead_code)]
fn get_page_size() -> usize {
    4096
}

/// 使用 mincore 检查 mmap 区域中有多少页面驻留在 RAM 中。
#[cfg(unix)]
fn check_pages_in_ram(ptr: *const u8, size: usize, page_size: usize) -> usize {
    let n_pages = size.div_ceil(page_size);
    let mut vec = vec![0u8; n_pages];

    // Safety: mincore 要求 addr 是页对齐的。mmap 返回的地址总是页对齐的。
    let ret = unsafe {
        libc::mincore(
            ptr as *mut libc::c_void,
            size as libc::size_t,
            vec.as_mut_ptr() as *mut libc::c_char,
        )
    };

    if ret != 0 {
        return 0;
    }

    vec.iter().filter(|&&b| (b & 1) != 0).count()
}
