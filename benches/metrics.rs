//! Time + memory metrics harness for document workloads.
//!
//! Complements the Criterion `doc_workloads` bench: where Criterion gives
//! rigorous latency statistics, this harness runs each matrix cell once and
//! records latency *and* memory (heap allocations via a counting global
//! allocator, plus process peak RSS), emitting a CSV and a stdout table for
//! tracking MonaDB-vs-SQLite over time.
//!
//! ```sh
//! cargo bench --bench metrics
//! MONADB_BENCH_PROFILES=xs,md MONADB_BENCH_ENGINES=monadb cargo bench --bench metrics
//! ```
//!
//! It honors the same `MONADB_BENCH_*` env knobs as `doc_workloads`.
//! `MONADB_BENCH_CSV` overrides the output path (default `target/bench-metrics.csv`).

mod alloc;
mod config;
mod fixtures;
mod monadb;
mod report;
mod rss;
mod sqlite;
mod store;
mod workloads;

use std::fs::{self, File};
use std::hint::black_box;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use config::{BenchConfig, Engine, Profile, Workload};
use report::{format_cardinality, print_environment};
use sqlite::SqliteBench;
use store::open_store;
use workloads::{generate_plan, preload, run_insert, run_read};

/// Install the counting allocator so heap traffic can be attributed per cell.
#[global_allocator]
static ALLOC: alloc::Counting = alloc::Counting::new();

/// Seed offset for untimed warmup plans (kept distinct from the timed plan).
const WARMUP_SEED_OFFSET: u64 = 0x9E37_79B9;

/// One measured matrix cell.
struct MetricRow {
    workload: &'static str,
    profile: &'static str,
    cardinality: String,
    engine: &'static str,
    ns_per_op: f64,
    bytes_alloc_per_op: f64,
    allocs_per_op: f64,
    peak_heap_bytes: usize,
    peak_rss_bytes: u64,
}

fn main() {
    fixtures::assert_profile_sizes();
    print_environment(&SqliteBench::version());
    let cfg = BenchConfig::from_env();

    let mut rows = Vec::new();
    for &workload in &cfg.workloads {
        for &profile in &cfg.profiles {
            let cardinalities: Vec<usize> = if workload.is_read() {
                cfg.cardinalities.clone()
            } else {
                vec![0]
            };
            for &cardinality in &cardinalities {
                for &engine in &cfg.engines {
                    let row = measure(&cfg, workload, profile, cardinality, engine);
                    println!(
                        "  {:>28} {:>3} {:>6} {:<13} {:>10.0} ns/op  {:>9.0} B/op  rss {} MiB",
                        row.workload,
                        row.profile,
                        row.cardinality,
                        row.engine,
                        row.ns_per_op,
                        row.bytes_alloc_per_op,
                        row.peak_rss_bytes / (1024 * 1024),
                    );
                    rows.push(row);
                }
            }
        }
    }

    write_csv(&rows);
    write_table(&rows);
}

/// Runs one matrix cell once and captures time + memory metrics.
fn measure(
    cfg: &BenchConfig,
    workload: Workload,
    profile: Profile,
    cardinality: usize,
    engine: Engine,
) -> MetricRow {
    let mut store = open_store(engine, workload);

    // Untimed setup: preload + warmup for read workloads.
    let plan = if workload.is_read() {
        preload(&mut *store, workload, profile, cardinality);
        let warm = generate_plan(
            workload,
            cardinality,
            cfg.warmup_lookups,
            cfg.seed.wrapping_add(WARMUP_SEED_OFFSET),
            cfg.range_width,
        );
        run_read(&mut *store, workload, &warm);
        // Generate the timed plan before reset() so key generation isn't counted.
        Some(generate_plan(
            workload,
            cardinality,
            cfg.m,
            cfg.seed,
            cfg.range_width,
        ))
    } else {
        None
    };

    let m = cfg.m;
    alloc::reset();
    let start = Instant::now();
    let consumed = match &plan {
        Some(plan) => run_read(&mut *store, workload, plan),
        None => {
            run_insert(&mut *store, workload, profile, cfg.n as i64, m);
            m
        }
    };
    let elapsed = start.elapsed();
    let stats = alloc::snapshot();
    black_box(consumed);

    let ops = m.max(1) as f64;
    MetricRow {
        workload: workload.label(),
        profile: profile.label(),
        cardinality: format_cardinality(cardinality),
        engine: engine.label(),
        ns_per_op: elapsed.as_nanos() as f64 / ops,
        bytes_alloc_per_op: stats.total_bytes as f64 / ops,
        allocs_per_op: stats.alloc_count as f64 / ops,
        peak_heap_bytes: stats.peak_bytes,
        peak_rss_bytes: rss::peak_rss_bytes(),
    }
}

/// Output CSV path (`MONADB_BENCH_CSV` or `target/bench-metrics.csv`).
fn csv_path() -> PathBuf {
    std::env::var_os("MONADB_BENCH_CSV")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/bench-metrics.csv"))
}

/// Writes the metric rows as CSV, creating parent directories as needed.
fn write_csv(rows: &[MetricRow]) {
    let path = csv_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let mut file = File::create(&path).expect("create csv");
    writeln!(
        file,
        "workload,profile,cardinality,engine,ns_per_op,bytes_alloc_per_op,allocs_per_op,peak_heap_bytes,peak_rss_bytes"
    )
    .expect("csv header");
    for r in rows {
        writeln!(
            file,
            "{},{},{},{},{:.0},{:.0},{:.2},{},{}",
            r.workload,
            r.profile,
            r.cardinality,
            r.engine,
            r.ns_per_op,
            r.bytes_alloc_per_op,
            r.allocs_per_op,
            r.peak_heap_bytes,
            r.peak_rss_bytes,
        )
        .expect("csv row");
    }
    println!("\nwrote {} rows -> {}", rows.len(), path.display());
}

/// Prints an aligned summary table to stdout.
fn write_table(rows: &[MetricRow]) {
    println!(
        "\n{:<28} {:<4} {:<6} {:<13} {:>12} {:>12} {:>10} {:>10}",
        "workload", "prof", "card", "engine", "ns/op", "B/op", "peak_heap", "peak_rss"
    );
    for r in rows {
        println!(
            "{:<28} {:<4} {:<6} {:<13} {:>12.0} {:>12.0} {:>10} {:>10}",
            r.workload,
            r.profile,
            r.cardinality,
            r.engine,
            r.ns_per_op,
            r.bytes_alloc_per_op,
            r.peak_heap_bytes,
            r.peak_rss_bytes,
        );
    }
}
