//! Benchmark configuration parsed from environment variables.

use std::env;

/// Document payload size tier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Profile {
    /// ~256 B flat scalars.
    Xs,
    /// ~2 KiB with metadata and tags.
    Sm,
    /// ~16 KiB with line items.
    Md,
    /// ~128 KiB with padded content and audit log.
    Lg,
}

impl Profile {
    /// Returns the target encoded JSON byte length for this profile.
    pub const fn target_bytes(self) -> usize {
        match self {
            Self::Xs => 256,
            Self::Sm => 2 * 1024,
            Self::Md => 16 * 1024,
            Self::Lg => 128 * 1024,
        }
    }

    /// Parses a profile name (`xs`, `sm`, `md`, `lg`).
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "xs" => Some(Self::Xs),
            "sm" => Some(Self::Sm),
            "md" => Some(Self::Md),
            "lg" => Some(Self::Lg),
            _ => None,
        }
    }

    /// Returns the short label for reports and Criterion IDs.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Xs => "xs",
            Self::Sm => "sm",
            Self::Md => "md",
            Self::Lg => "lg",
        }
    }
}

/// Storage engine under test.
///
/// Both engines always run through prepared statements with bound parameters,
/// and SQLite stores documents as its native JSONB binary type, so the harness
/// compares each engine's parse-free / normalize-free steady-state hot path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Engine {
    MonaDb,
    Sqlite,
}

impl Engine {
    /// Parses an engine name.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "monadb" => Some(Self::MonaDb),
            "sqlite" => Some(Self::Sqlite),
            _ => None,
        }
    }

    /// Returns the short label for reports and Criterion IDs.
    pub const fn label(self) -> &'static str {
        match self {
            Self::MonaDb => "monadb",
            Self::Sqlite => "sqlite",
        }
    }
}

/// Benchmark workload identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Workload {
    /// Point lookup by integer key (`docs[id]`).
    SingleKeySelect1,
    /// Range read over a contiguous integer key span.
    SingleKeySelectRange,
    SingleKeyInsert,
    /// Point lookup by full composite key (`docs[tenant, seq]`).
    CompositeKeySelect1,
    /// Prefix / partition read — all documents for one tenant.
    CompositeKeySelectPrefix,
    CompositeKeyInsert,
}

impl Workload {
    /// Parses a workload name (with or without trailing `_1`).
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "single_key_select_1" | "single_key_select" => Some(Self::SingleKeySelect1),
            "single_key_select_range" | "single_key_range" => Some(Self::SingleKeySelectRange),
            "single_key_insert" => Some(Self::SingleKeyInsert),
            "composite_key_select_1" | "composite_key_select" => Some(Self::CompositeKeySelect1),
            "composite_key_select_prefix" | "composite_key_prefix" => {
                Some(Self::CompositeKeySelectPrefix)
            }
            "composite_key_insert" => Some(Self::CompositeKeyInsert),
            _ => None,
        }
    }

    /// Returns the canonical label for reports and Criterion IDs.
    pub const fn label(self) -> &'static str {
        match self {
            Self::SingleKeySelect1 => "single_key_select_1",
            Self::SingleKeySelectRange => "single_key_select_range",
            Self::SingleKeyInsert => "single_key_insert",
            Self::CompositeKeySelect1 => "composite_key_select_1",
            Self::CompositeKeySelectPrefix => "composite_key_select_prefix",
            Self::CompositeKeyInsert => "composite_key_insert",
        }
    }

    /// Returns whether this workload uses composite keys.
    pub const fn is_composite(self) -> bool {
        matches!(
            self,
            Self::CompositeKeySelect1
                | Self::CompositeKeySelectPrefix
                | Self::CompositeKeyInsert
        )
    }

    /// Returns whether this workload is a read (select) benchmark.
    pub const fn is_read(self) -> bool {
        matches!(
            self,
            Self::SingleKeySelect1
                | Self::SingleKeySelectRange
                | Self::CompositeKeySelect1
                | Self::CompositeKeySelectPrefix
        )
    }
}

/// Runtime benchmark knobs.
#[derive(Clone, Debug)]
pub struct BenchConfig {
    /// Timed iterations per Criterion sample.
    pub m: usize,
    /// Preload row count for select workloads.
    pub n: usize,
    /// Document profiles to run.
    pub profiles: Vec<Profile>,
    /// Workloads to run.
    pub workloads: Vec<Workload>,
    /// Engines to run.
    pub engines: Vec<Engine>,
    /// Cardinalities for select workloads.
    pub cardinalities: Vec<usize>,
    /// RNG seed for lookup key selection.
    pub seed: u64,
    /// Warmup lookups after preload (not timed).
    pub warmup_lookups: usize,
    /// Row span for single-key range reads.
    pub range_width: usize,
}

impl Default for BenchConfig {
    fn default() -> Self {
        Self {
            m: 10_000,
            n: 10_000,
            profiles: vec![Profile::Xs, Profile::Sm, Profile::Md, Profile::Lg],
            workloads: vec![
                Workload::SingleKeySelect1,
                Workload::SingleKeySelectRange,
                Workload::SingleKeyInsert,
                Workload::CompositeKeySelect1,
                Workload::CompositeKeySelectPrefix,
                Workload::CompositeKeyInsert,
            ],
            engines: vec![Engine::MonaDb, Engine::Sqlite],
            cardinalities: vec![10_000, 100_000],
            seed: 0x0ADB_00EC,
            warmup_lookups: 100,
            range_width: 100,
        }
    }
}

impl BenchConfig {
    /// Loads configuration from environment variables with defaults.
    pub fn from_env() -> Self {
        let mut cfg = Self::default();
        if let Ok(v) = env::var("MONADB_BENCH_M") {
            if let Ok(n) = v.parse() {
                cfg.m = n;
            }
        }
        if let Ok(v) = env::var("MONADB_BENCH_N") {
            if let Ok(n) = v.parse() {
                cfg.n = n;
                cfg.cardinalities = vec![n];
            }
        }
        if let Ok(v) = env::var("MONADB_BENCH_PROFILES") {
            cfg.profiles = parse_list(&v, Profile::parse);
        }
        if let Ok(v) = env::var("MONADB_BENCH_WORKLOADS") {
            cfg.workloads = parse_list(&v, Workload::parse);
        }
        if let Ok(v) = env::var("MONADB_BENCH_ENGINES") {
            cfg.engines = parse_list(&v, Engine::parse);
        }
        if let Ok(v) = env::var("MONADB_BENCH_SEED") {
            if let Ok(seed) = v.parse() {
                cfg.seed = seed;
            }
        }
        if let Ok(v) = env::var("MONADB_BENCH_RANGE") {
            if let Ok(width) = v.parse::<usize>() {
                cfg.range_width = width.max(1);
            }
        }
        cfg
    }
}

fn parse_list<T, F>(raw: &str, parse: F) -> Vec<T>
where
    F: Fn(&str) -> Option<T>,
{
    raw.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter_map(parse)
        .collect()
}
