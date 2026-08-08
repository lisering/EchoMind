//! Layer 级流式预取（Phase 3 Session 21）。
//!
//! 在推理进行时后台预取下一层权重到 RAM，利用 `madvise(MADV_WILLNEED)`
//! 告知 OS 这些页面即将被访问，OS 会在后台异步从磁盘加载到 page cache。
//!
//! # 原理
//!
//! mmap 映射的 GGUF 文件采用按需分页（demand paging）：首次访问某页面时
//! 触发 page fault，OS 从磁盘加载到 RAM。这导致推理时每访问一层权重都
//! 产生 I/O 延迟。
//!
//! `madvise(MADV_WILLNEED)` 是一个非阻塞提示：调用后 OS 在后台异步预读
//! 指定地址范围的页面到 page cache。当下一次推理实际访问这些页面时，
//! page fault 命中 page cache，无需等待磁盘 I/O。
//!
//! # 使用方式
//!
//! 1. 从 `GgufFile` 提取所有张量的 mmap 偏移量
//! 2. 创建 `LayerPrefetcher`
//! 3. 在推理 layer N 时，调用 `prefetch_next(N+1)` 预取下一层
//! 4. 或调用 `prefetch_range(0, n)` 预取前 N 层
//! 5. 推理结束后调用 `stop()`
//!
//! # 平台支持
//!
//! - **Unix**（Linux/macOS）：使用 `libc::madvise(MADV_WILLNEED)`
//! - **Windows**：暂不支持（回退为 no-op），后续可使用 `PrefetchVirtualMemory`
//!
//! # 线程安全
//!
//! `LayerPrefetcher` 自动满足 `Send + Sync`：
//! - `SendPtr` 包装裸指针并实现 `Send + Sync`
//! - `active` / `stop_flag` 使用 `AtomicBool`
//! - 后台线程句柄通过 `Mutex` 保护
//!
//! # 参考
//!
//! - `madvise(2)` man page: <https://man7.org/linux/man-pages/man2/madvise.2.html>
//! - Linux page cache: <https://www.kernel.org/doc/html/latest/admin-guide/mm/concepts.html#page-cache>
//! - llama.cpp mmap: `ggml/src/ggml-alloc.c` — `ggml_mmap_madvise()`

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use memmap2::Mmap;

// ---------------------------------------------------------------------------
// SendPtr — 可跨线程传递的裸指针包装
// ---------------------------------------------------------------------------

/// 可跨线程安全传递的裸指针包装。
///
/// `*const u8` 默认不实现 `Send`，但 mmap 数据是只读的，
/// `madvise` 是只读提示操作，因此跨线程共享是安全的。
#[derive(Clone, Copy)]
struct SendPtr(*const u8);

// Safety: SendPtr 包装的裸指针指向 mmap 只读数据，
// madvise 是只读提示操作（不修改内存），跨线程访问安全。
unsafe impl Send for SendPtr {}
unsafe impl Sync for SendPtr {}

// ---------------------------------------------------------------------------
// LayerPrefetcher
// ---------------------------------------------------------------------------

/// Layer 级流式预取器。
///
/// 管理 mmap 区域的 layer 级偏移量，通过 `madvise(MADV_WILLNEED)`
/// 告知 OS 后台预取指定层的权重到 RAM。
///
/// # 生命周期
///
/// 1. `new()` — 从 `Mmap` 和 layer 偏移量创建预取器
/// 2. `prefetch_next()` / `prefetch_range()` — 预取指定层
/// 3. `stop()` — 停止预取（重置 active 状态）
/// 4. `is_active()` — 查询预取是否进行中
///
/// # 安全性
///
/// 预取器存储 mmap 的裸指针（通过 `SendPtr` 包装）。调用方必须确保
/// `Mmap` 的生命周期长于 `LayerPrefetcher`。通常 `GgufFile` 持有 `Mmap`，
/// 预取器在 `GgufFile` 存活期间使用。
///
/// `madvise` 是只读提示操作，不会修改 mmap 数据，因此多线程访问安全。
pub struct LayerPrefetcher {
    /// mmap 映射区域的起始地址（通过 SendPtr 包装，支持跨线程）。
    mmap_ptr: SendPtr,
    /// mmap 映射区域的总长度（字节，用于边界检查）。
    mmap_len: usize,
    /// Layer 偏移量列表：`(张量名, mmap 内绝对偏移, 字节大小)`。
    layer_offsets: Vec<(String, u64, u64)>,
    /// 预取器是否活跃（已发起预取且未停止）。
    active: Arc<AtomicBool>,
    /// 后台预取线程的停止信号。
    stop_flag: Arc<AtomicBool>,
    /// 后台预取线程句柄（`prefetch_range` 使用）。
    thread_handle: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
}

impl LayerPrefetcher {
    /// 创建预取器。
    ///
    /// # 参数
    ///
    /// - `mmap`：mmap 映射区域（调用方需确保生命周期长于预取器）
    /// - `layer_offsets`：Layer 偏移量列表，每项为 `(张量名, mmap 内绝对偏移, 字节大小)`
    ///
    /// # 返回
    ///
    /// 创建的 `LayerPrefetcher`，初始状态为非活跃（`is_active() == false`）。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// # use echomind_infra::layer_prefetch::LayerPrefetcher;
    /// # use memmap2::Mmap;
    /// # use std::fs::File;
    /// # let file = File::open("model.gguf").unwrap();
    /// # let mmap = unsafe { Mmap::map(&file) }.unwrap();
    /// let offsets = vec![
    ///     ("layer.0.weight".to_string(), 1024, 4096),
    ///     ("layer.1.weight".to_string(), 5120, 4096),
    /// ];
    /// let prefetcher = LayerPrefetcher::new(&mmap, offsets);
    /// assert!(!prefetcher.is_active());
    /// ```
    pub fn new(mmap: &Mmap, layer_offsets: Vec<(String, u64, u64)>) -> Self {
        Self {
            mmap_ptr: SendPtr(mmap.as_ptr()),
            mmap_len: mmap.len(),
            layer_offsets,
            active: Arc::new(AtomicBool::new(false)),
            stop_flag: Arc::new(AtomicBool::new(false)),
            thread_handle: Arc::new(Mutex::new(None)),
        }
    }

    /// 预取指定层权重到 RAM。
    ///
    /// 使用 `madvise(MADV_WILLNEED)` 告知 OS 这些页面即将被访问，
    /// OS 会在后台异步从磁盘加载到 page cache。
    ///
    /// 此方法是**非阻塞**的：`madvise` 调用本身只发送一个提示，
    /// 实际 I/O 由 OS 内核异步执行。调用后立即返回，不等待页面加载完成。
    ///
    /// # 参数
    ///
    /// - `layer_idx`：层索引（对应 `layer_offsets` 中的位置）
    ///
    /// # 安全性
    ///
    /// - 越界索引安全返回（不 panic）
    /// - `madvise` 失败不影响正确性，仅日志记录
    ///
    /// # 平台
    ///
    /// - Unix：调用 `libc::madvise(MADV_WILLNEED)`
    /// - Windows：no-op（后续支持 `PrefetchVirtualMemory`）
    pub fn prefetch_next(&self, layer_idx: usize) {
        // 边界检查：越界索引安全返回
        let Some((_, offset, size)) = self.layer_offsets.get(layer_idx) else {
            return;
        };
        self.do_madvise(*offset, *size);
        self.active.store(true, Ordering::SeqCst);
    }

    /// 预取连续 N 层。
    ///
    /// 在后台线程中依次对 `[start, start+count)` 范围内的每一层调用
    /// `madvise(MADV_WILLNEED)`。线程在每层之间短暂休眠（1ms），
    /// 避免一次性触发过多 I/O 请求导致系统卡顿。
    ///
    /// 如果已有后台线程在运行，会先停止旧线程再启动新线程。
    ///
    /// # 参数
    ///
    /// - `start`：起始层索引
    /// - `count`：预取层数
    ///
    /// # 安全性
    ///
    /// - `start` 超出范围时安全返回（不 panic）
    /// - `start + count` 超出范围时自动截断到实际层数
    /// - 后台线程通过 `stop_flag` 控制退出
    pub fn prefetch_range(&self, start: usize, count: usize) {
        // 停止已有后台线程
        self.stop_background();

        // 计算有效范围（自动截断）
        let end = start.saturating_add(count).min(self.layer_offsets.len());
        if start >= end {
            return;
        }

        // 收集需要预取的层偏移量
        let offsets: Vec<(u64, u64)> = self.layer_offsets[start..end]
            .iter()
            .map(|(_, off, sz)| (*off, *sz))
            .collect();

        // 准备后台线程所需数据
        let mmap_ptr = SendPtr(self.mmap_ptr.0);
        let mmap_len = self.mmap_len;
        let active = Arc::clone(&self.active);
        let stop_flag = Arc::clone(&self.stop_flag);

        active.store(true, Ordering::SeqCst);
        stop_flag.store(false, Ordering::SeqCst);

        // 启动后台预取线程
        let handle = std::thread::spawn(move || {
            for (offset, size) in offsets {
                if stop_flag.load(Ordering::SeqCst) {
                    break;
                }
                do_madvise_with_sendptr(mmap_ptr, mmap_len, offset, size);
                // 层间短暂休眠，避免 I/O 风暴
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            // 所有层预取完成后标记为非活跃
            if !stop_flag.load(Ordering::SeqCst) {
                active.store(false, Ordering::SeqCst);
            }
        });

        // 存储线程句柄
        if let Ok(mut guard) = self.thread_handle.lock() {
            *guard = Some(handle);
        }
    }

    /// 停止预取线程。
    ///
    /// 设置停止信号并等待后台线程退出。
    /// 调用后 `is_active()` 返回 `false`。
    ///
    /// 此方法是幂等的：多次调用不会 panic。
    /// 如果没有后台线程在运行，立即返回。
    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        self.active.store(false, Ordering::SeqCst);

        // 等待后台线程退出
        if let Ok(mut guard) = self.thread_handle.lock()
            && let Some(handle) = guard.take()
        {
            let _ = handle.join();
        }
    }

    /// 预取线程是否运行中。
    ///
    /// 返回 `true` 表示已发起预取且未调用 `stop()`。
    /// 注意：后台线程自然完成后也会返回 `false`。
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }

    /// 返回 layer 偏移量列表的引用。
    ///
    /// 用于调试和测试验证。
    pub fn layer_offsets(&self) -> &[(String, u64, u64)] {
        &self.layer_offsets
    }

    // ---- 内部方法 ----

    /// 对指定地址范围调用 madvise（实例方法，使用 self 的 mmap 指针）。
    fn do_madvise(&self, offset: u64, size: u64) {
        do_madvise_raw(self.mmap_ptr.0, self.mmap_len, offset, size);
    }

    /// 停止已有后台线程（内部方法）。
    ///
    /// 设置停止信号并等待线程退出。与 `stop()` 不同，不重置 `active` 标志
    /// （因为 `prefetch_range` 会重新设置）。
    fn stop_background(&self) {
        self.stop_flag.store(true, Ordering::SeqCst);

        if let Ok(mut guard) = self.thread_handle.lock()
            && let Some(handle) = guard.take()
        {
            let _ = handle.join();
        }
    }
}

impl Drop for LayerPrefetcher {
    fn drop(&mut self) {
        // 确保后台线程在预取器销毁前退出
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Ok(mut guard) = self.thread_handle.lock()
            && let Some(handle) = guard.take()
        {
            let _ = handle.join();
        }
    }
}

// ---------------------------------------------------------------------------
// 自由函数
// ---------------------------------------------------------------------------

/// 从 GgufFile 提取 layer 偏移量列表。
///
/// 遍历 GgufFile 的所有张量，计算每个张量在 mmap 中的绝对偏移量
/// （`data_offset + tensor.offset`）。
///
/// # 参数
///
/// - `gguf`：已打开的 GGUF 文件
///
/// # 返回
///
/// `Vec<(张量名, mmap 绝对偏移, 字节大小)>`，按文件中的张量顺序排列。
///
/// # 示例
///
/// ```no_run
/// # use echomind_infra::gguf_reader::GgufFile;
/// # use echomind_infra::layer_prefetch::layer_offsets_from_gguf;
/// # let gguf = GgufFile::open(std::path::Path::new("model.gguf")).unwrap();
/// let offsets = layer_offsets_from_gguf(&gguf);
/// println!("{} tensors", offsets.len());
/// ```
pub fn layer_offsets_from_gguf(gguf: &crate::gguf_reader::GgufFile) -> Vec<(String, u64, u64)> {
    let data_offset = gguf.data_offset();
    let mut offsets = Vec::new();
    for name in gguf.tensor_names() {
        if let Some(info) = gguf.tensor_info(name) {
            let abs_offset = data_offset.saturating_add(info.offset);
            offsets.push((name.to_string(), abs_offset, info.size));
        }
    }
    offsets
}

/// 对 mmap 中指定地址范围调用 `madvise(MADV_WILLNEED)`（接受 SendPtr 的包装函数）。
///
/// 用于后台线程中，避免闭包直接捕获 `*const u8`（不满足 `Send`）。
fn do_madvise_with_sendptr(ptr: SendPtr, mmap_len: usize, offset: u64, size: u64) {
    do_madvise_raw(ptr.0, mmap_len, offset, size);
}

/// 对 mmap 中指定地址范围调用 `madvise(MADV_WILLNEED)`（自由函数）。
///
/// # 安全性
///
/// - 偏移量 + 大小超出 mmap 范围时安全返回（不 panic）
/// - `madvise` 失败不影响正确性，仅记录调试日志
///
/// # 平台
///
/// - Unix：调用 `libc::madvise`
/// - Windows：no-op
fn do_madvise_raw(mmap_ptr: *const u8, mmap_len: usize, offset: u64, size: u64) {
    let offset_usize = offset as usize;
    let size_usize = size as usize;

    // 边界检查：防止越界访问
    if offset_usize.saturating_add(size_usize) > mmap_len {
        return;
    }
    if size_usize == 0 {
        return;
    }

    #[cfg(unix)]
    {
        // Safety: mmap_ptr + offset 在 mmap_len 范围内（已边界检查）。
        // madvise 是只读提示操作，不修改内存内容。
        let ptr = unsafe { mmap_ptr.add(offset_usize) };
        let ret = unsafe {
            libc::madvise(
                ptr as *mut libc::c_void,
                size_usize as libc::size_t,
                libc::MADV_WILLNEED,
            )
        };
        if ret != 0 {
            // madvise 失败不影响正确性（仅是提示），记录调试日志
            eprintln!(
                "[LayerPrefetcher] madvise failed: offset={offset_usize}, size={size_usize}, ret={ret}"
            );
        }
    }

    // Windows: no-op（后续可用 PrefetchVirtualMemory 实现）
    #[cfg(not(unix))]
    {
        let _ = (mmap_ptr, offset_usize, size_usize);
    }
}
