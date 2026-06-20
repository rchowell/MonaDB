//! Workload drivers: data preload and timed read/insert loops.
//!
//! Every driver dispatches through [`DocStore`], so adding an engine means
//! implementing the trait once — no per-engine branches live here.

use super::config::{Profile, Workload};
use super::fixtures::{
    DocSpec, random_composite_keys, random_single_keys, random_single_ranges, random_tenant_labels,
    tenant_label,
};
use super::store::DocStore;

/// Number of tenant partitions composite-key data is spread across.
pub const TENANTS: usize = 100;

/// Query parameters generated for one read-benchmark sample.
#[derive(Default)]
pub struct ReadQueryPlan {
    /// Point-lookup keys for single-key tables.
    pub single_keys: Vec<i64>,
    /// Point-lookup keys for composite-key tables.
    pub composite_keys: Vec<(String, i64)>,
    /// Half-open integer key ranges `[lo, hi)`.
    pub ranges: Vec<(i64, i64)>,
    /// Tenant partition labels for prefix reads.
    pub tenants: Vec<String>,
}

/// Generates a `count`-operation query plan for one read workload.
///
/// Only the field this workload reads is populated; the rest stay empty.
pub fn generate_plan(
    workload: Workload,
    cardinality: usize,
    count: usize,
    seed: u64,
    range_width: usize,
) -> ReadQueryPlan {
    let n = cardinality.max(1);
    match workload {
        Workload::SingleKeySelect1 => ReadQueryPlan {
            single_keys: random_single_keys(count, n, seed),
            ..ReadQueryPlan::default()
        },
        Workload::SingleKeySelectRange => ReadQueryPlan {
            ranges: random_single_ranges(count, n, range_width, seed),
            ..ReadQueryPlan::default()
        },
        Workload::CompositeKeySelect1 => ReadQueryPlan {
            composite_keys: random_composite_keys(count, n, seed),
            ..ReadQueryPlan::default()
        },
        Workload::CompositeKeySelectPrefix => ReadQueryPlan {
            tenants: random_tenant_labels(count, seed),
            ..ReadQueryPlan::default()
        },
        _ => ReadQueryPlan::default(),
    }
}

/// Preloads `cardinality` documents for a read workload.
pub fn preload(store: &mut dyn DocStore, workload: Workload, profile: Profile, cardinality: usize) {
    if workload.is_composite() {
        let per_tenant = cardinality / TENANTS;
        for tenant in 0..TENANTS as i64 {
            for seq in 0..per_tenant as i64 {
                store.insert(&DocSpec::composite(profile, tenant, seq));
            }
        }
    } else {
        for id in 0..cardinality as i64 {
            store.insert(&DocSpec::single(profile, id));
        }
    }
}

/// Runs one read workload over its plan; returns total rows consumed.
pub fn run_read(store: &mut dyn DocStore, workload: Workload, plan: &ReadQueryPlan) -> usize {
    let mut consumed = 0usize;
    match workload {
        Workload::SingleKeySelect1 => {
            for &key in &plan.single_keys {
                consumed += store.select_single(key);
            }
        }
        Workload::SingleKeySelectRange => {
            for &(lo, hi) in &plan.ranges {
                consumed += store.select_single_range(lo, hi);
            }
        }
        Workload::CompositeKeySelect1 => {
            for (tenant, seq) in &plan.composite_keys {
                consumed += store.select_composite(tenant, *seq);
            }
        }
        Workload::CompositeKeySelectPrefix => {
            for tenant in &plan.tenants {
                consumed += store.select_composite_prefix(tenant);
            }
        }
        _ => panic!("not a read workload"),
    }
    consumed
}

/// Runs `m` fresh inserts for an insert workload, keyed from `base`.
pub fn run_insert(
    store: &mut dyn DocStore,
    workload: Workload,
    profile: Profile,
    base: i64,
    m: usize,
) {
    match workload {
        Workload::SingleKeyInsert => {
            for i in 0..m {
                store.insert(&DocSpec::single(profile, base + i as i64));
            }
        }
        Workload::CompositeKeyInsert => {
            for i in 0..m {
                let (tenant_index, seq) = composite_offset(base + i as i64);
                store.insert(&DocSpec::composite(profile, tenant_index, seq));
            }
        }
        _ => panic!("not an insert workload"),
    }
}

/// Maps a linear offset to a `(tenant_index, seq)` composite coordinate.
fn composite_offset(offset: i64) -> (i64, i64) {
    (offset.rem_euclid(TENANTS as i64), offset / TENANTS as i64)
}

/// Returns the `(tenant_label, seq)` a linear offset inserts at.
///
/// Used by the smoke test to verify a known insert; unused by the harnesses.
#[allow(dead_code)]
pub fn composite_key_for_offset(offset: i64) -> (String, i64) {
    let (tenant_index, seq) = composite_offset(offset);
    (tenant_label(tenant_index), seq)
}
