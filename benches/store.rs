//! Engine-agnostic document store interface.
//!
//! Both the MonaDB and SQLite adapters implement [`DocStore`], so workload
//! drivers, the Criterion harness, the metrics harness, and the smoke test all
//! dispatch through one trait instead of per-engine match arms.

use super::config::{Engine, Workload};
use super::fixtures::DocSpec;
use super::monadb::MonaDbBench;
use super::sqlite::SqliteBench;

/// A keyed document store under benchmark.
///
/// Read methods return the number of rows they consumed so callers can assert
/// the work happened and the optimizer cannot elide it. Each method fully
/// materializes (decodes) the documents it touches for cross-engine fairness.
pub trait DocStore {
    /// Creates the `docs` table for the given workload's key shape.
    fn create_table(&mut self, workload: Workload);

    /// Inserts one document using ad-hoc SQL.
    fn insert(&mut self, spec: &DocSpec);

    /// Point lookup by integer key; returns rows consumed (0 or 1).
    fn select_single(&mut self, id: i64) -> usize;

    /// Point lookup by composite key; returns rows consumed (0 or 1).
    fn select_composite(&mut self, tenant: &str, seq: i64) -> usize;

    /// Range read over integer keys `[lo, hi)`; returns rows consumed.
    fn select_single_range(&mut self, lo: i64, hi: i64) -> usize;

    /// Prefix read of all documents for one tenant; returns rows consumed.
    fn select_composite_prefix(&mut self, tenant: &str) -> usize;
}

/// Opens a fresh store for `engine` and creates the workload's table.
pub fn open_store(engine: Engine, workload: Workload) -> Box<dyn DocStore> {
    let mut store: Box<dyn DocStore> = match engine {
        Engine::MonaDb => Box::new(MonaDbBench::open()),
        Engine::Sqlite => Box::new(SqliteBench::open()),
    };
    store.create_table(workload);
    store
}
