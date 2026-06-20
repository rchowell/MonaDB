//! Document-oriented workload benchmarks: MonaDB vs SQLite (TEXT / JSONB).
//!
//! This is the Criterion *timing* harness. For time + memory data collection,
//! see the sibling `metrics` bench (`benches/metrics.rs`).

mod config;
mod fixtures;
mod monadb;
mod report;
mod sqlite;
mod store;
mod workloads;

use std::time::Duration;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};

use config::{BenchConfig, Engine, Profile, Workload};
use fixtures::assert_profile_sizes;
use report::{format_cardinality, print_environment};
use sqlite::SqliteBench;
use store::open_store;
use workloads::{generate_plan, preload, run_insert, run_read};

/// Seed offset for untimed warmup plans so they differ from the timed plan.
const WARMUP_SEED_OFFSET: u64 = 0x9E37_79B9;

fn register_benchmarks(c: &mut Criterion, cfg: &BenchConfig) {
    assert_profile_sizes();

    let mut group = c.benchmark_group("doc_workloads");
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(10);

    for workload in &cfg.workloads {
        for profile in &cfg.profiles {
            if workload.is_read() {
                for cardinality in &cfg.cardinalities {
                    for engine in &cfg.engines {
                        register_read(&mut group, cfg, *workload, *profile, *cardinality, *engine);
                    }
                }
            } else {
                for engine in &cfg.engines {
                    register_insert(&mut group, cfg, *workload, *profile, *engine);
                }
            }
        }
    }

    group.finish();
}

fn register_read(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    cfg: &BenchConfig,
    workload: Workload,
    profile: Profile,
    cardinality: usize,
    engine: Engine,
) {
    let id = format!(
        "{}/{}/{}/{}",
        workload.label(),
        profile.label(),
        format_cardinality(cardinality),
        engine.label()
    );

    group.bench_function(id, |b| {
        b.iter_batched(
            || {
                let mut store = open_store(engine, workload);
                preload(&mut *store, workload, profile, cardinality);
                let warm = generate_plan(
                    workload,
                    cardinality,
                    cfg.warmup_lookups,
                    cfg.seed.wrapping_add(WARMUP_SEED_OFFSET),
                    cfg.range_width,
                );
                run_read(&mut *store, workload, &warm);
                let plan = generate_plan(workload, cardinality, cfg.m, cfg.seed, cfg.range_width);
                (store, plan)
            },
            |(mut store, plan)| {
                let _ = run_read(&mut *store, workload, &plan);
            },
            BatchSize::LargeInput,
        );
    });
}

fn register_insert(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    cfg: &BenchConfig,
    workload: Workload,
    profile: Profile,
    engine: Engine,
) {
    let id = format!(
        "{}/{}/empty/{}",
        workload.label(),
        profile.label(),
        engine.label()
    );
    let m = cfg.m;
    let base = cfg.n as i64;

    group.bench_function(id, |b| {
        b.iter_batched(
            || open_store(engine, workload),
            |mut store| run_insert(&mut *store, workload, profile, base, m),
            BatchSize::LargeInput,
        );
    });
}

fn doc_workloads(c: &mut Criterion) {
    print_environment(&SqliteBench::version());
    let cfg = BenchConfig::from_env();
    register_benchmarks(c, &cfg);
}

criterion_group! {
    name = benches;
    config = Criterion::default();
    targets = doc_workloads
}
criterion_main!(benches);
