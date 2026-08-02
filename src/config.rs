//! LMDB open-time configuration.

use heed::EnvFlags;

/// LMDB open-time configuration for [`crate::MonaDB::open_with_config`].
#[derive(Debug, Clone, Default)]
pub struct Config {
    nosync: bool,
}

impl Config {
    /// Skips fsync on commit (`MDB_NOSYNC`).
    ///
    /// Roughly analogous to SQLite `PRAGMA synchronous = NORMAL` — lower commit
    /// latency, weaker crash durability, but never a corrupt database: without
    /// `MDB_WRITEMAP` a commit still reaches the OS page cache in order, so it
    /// keeps atomicity, consistency, and isolation and gives up only durability.
    /// A *process* crash therefore loses nothing; an OS crash or power loss may
    /// lose the last committed transactions.
    ///
    /// The default (sync) path is comparatively strict: on macOS LMDB flushes
    /// with `fcntl(F_FULLFSYNC)`, a true barrier, where SQLite uses the cheaper
    /// `fsync` unless `PRAGMA fullfsync` is set.
    #[must_use]
    pub fn nosync(mut self) -> Self {
        self.nosync = true;
        self
    }

    /// Returns the LMDB env flags implied by this configuration.
    pub(crate) fn env_flags(&self) -> EnvFlags {
        if self.nosync {
            EnvFlags::NO_SYNC
        } else {
            EnvFlags::empty()
        }
    }
}
