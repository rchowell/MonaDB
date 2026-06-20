//! Process peak resident-set-size via `getrusage`.
//!
//! Unlike the counting allocator, RSS captures the whole process — including
//! LMDB's faulted mmap pages and SQLite's C heap — so it is the fair signal for
//! cross-engine memory comparison. It is a monotonic high-water mark for the
//! life of the process, so clean per-engine numbers require one engine per
//! process (see the metrics README).

/// Returns the process peak resident set size in bytes (0 if unavailable).
pub fn peak_rss_bytes() -> u64 {
    // SAFETY: `getrusage` only writes into the zeroed `rusage` we provide.
    unsafe {
        let mut usage: libc::rusage = std::mem::zeroed();
        if libc::getrusage(libc::RUSAGE_SELF, &mut usage) != 0 {
            return 0;
        }
        let maxrss = usage.ru_maxrss as u64;
        // macOS reports bytes; Linux/BSD report kibibytes.
        if cfg!(target_os = "macos") {
            maxrss
        } else {
            maxrss * 1024
        }
    }
}
