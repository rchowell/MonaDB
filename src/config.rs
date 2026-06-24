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
    /// latency, weaker crash durability. A process crash may lose the last
    /// committed transactions; an OS crash can still lose more.
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
