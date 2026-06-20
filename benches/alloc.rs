//! A counting global allocator for the metrics harness.
//!
//! [`Counting`] wraps the system allocator and tallies allocation bytes, count,
//! and the live-heap high-water mark in process-global atomics. Install it as
//! the `#[global_allocator]` of the metrics binary, then bracket a workload with
//! [`reset`] / [`snapshot`] to attribute heap traffic to that workload.
//!
//! Caveat: only Rust-side allocations are visible. SQLite's bundled C heap
//! bypasses this allocator, so allocation figures are exact for MonaDB but
//! undercount SQLite — use peak RSS (see `rss.rs`) for cross-engine memory.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

/// Total bytes allocated since the last [`reset`].
static TOTAL_BYTES: AtomicUsize = AtomicUsize::new(0);
/// Number of allocations since the last [`reset`].
static ALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);
/// Live bytes (allocated minus freed) across the whole process.
static CURRENT: AtomicUsize = AtomicUsize::new(0);
/// Live-bytes baseline captured at the last [`reset`].
static BASELINE: AtomicUsize = AtomicUsize::new(0);
/// Live-bytes high-water mark since the last [`reset`].
static PEAK: AtomicUsize = AtomicUsize::new(0);

/// A `System`-backed allocator that records allocation statistics.
pub struct Counting;

impl Counting {
    /// Returns a new counting allocator (usable in a `static` initializer).
    pub const fn new() -> Self {
        Counting
    }
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forwarding an unchanged layout to the system allocator.
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            ALLOC_COUNT.fetch_add(1, Relaxed);
            TOTAL_BYTES.fetch_add(layout.size(), Relaxed);
            let live = CURRENT.fetch_add(layout.size(), Relaxed) + layout.size();
            PEAK.fetch_max(live, Relaxed);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr`/`layout` came from this allocator's `alloc`.
        unsafe { System.dealloc(ptr, layout) };
        CURRENT.fetch_sub(layout.size(), Relaxed);
    }
}

/// Allocation statistics over one measurement window.
#[derive(Clone, Copy, Debug)]
pub struct Stats {
    /// Total bytes allocated during the window.
    pub total_bytes: usize,
    /// Number of allocations during the window.
    pub alloc_count: usize,
    /// Peak live-heap growth over the window's baseline, in bytes.
    pub peak_bytes: usize,
}

/// Starts a new measurement window, baselining peak to current live heap.
pub fn reset() {
    let live = CURRENT.load(Relaxed);
    BASELINE.store(live, Relaxed);
    PEAK.store(live, Relaxed);
    TOTAL_BYTES.store(0, Relaxed);
    ALLOC_COUNT.store(0, Relaxed);
}

/// Returns the statistics accumulated since the last [`reset`].
pub fn snapshot() -> Stats {
    Stats {
        total_bytes: TOTAL_BYTES.load(Relaxed),
        alloc_count: ALLOC_COUNT.load(Relaxed),
        peak_bytes: PEAK.load(Relaxed).saturating_sub(BASELINE.load(Relaxed)),
    }
}
