//! Fast correctness gate for the document-workload benchmark adapters.
//!
//! Exercises every workload against all engines at tiny scale so CI catches
//! broken SQL renderers or adapters without running the full Criterion matrix.

mod benches;

use benches::config::{Engine, Profile, Workload};
use benches::store::open_store;
use benches::workloads::{composite_key_for_offset, generate_plan, preload, run_insert, run_read};

const PRELOAD: usize = 100;
const OPS: usize = 10;
const SEED: u64 = 7;
const RANGE_WIDTH: usize = 10;
const ENGINES: [Engine; 3] = [Engine::MonaDb, Engine::SqliteText, Engine::SqliteJsonb];

const READ_WORKLOADS: [Workload; 4] = [
    Workload::SingleKeySelect1,
    Workload::SingleKeySelectRange,
    Workload::CompositeKeySelect1,
    Workload::CompositeKeySelectPrefix,
];

#[test]
fn profile_fixture_sizes() {
    benches::fixtures::assert_profile_sizes();
}

#[test]
fn smoke_reads_all_engines() {
    for workload in READ_WORKLOADS {
        for engine in ENGINES {
            let mut store = open_store(engine, workload);
            preload(&mut *store, workload, Profile::Xs, PRELOAD);
            let plan = generate_plan(workload, PRELOAD, OPS, SEED, RANGE_WIDTH);
            let consumed = run_read(&mut *store, workload, &plan);
            assert!(
                consumed > 0,
                "{} / {:?} consumed no rows",
                workload.label(),
                engine
            );
        }
    }
}

#[test]
fn smoke_inserts_all_engines() {
    for workload in [Workload::SingleKeyInsert, Workload::CompositeKeyInsert] {
        for engine in ENGINES {
            let mut store = open_store(engine, workload);
            run_insert(&mut *store, workload, Profile::Xs, 10_000, OPS);
            match workload {
                Workload::SingleKeyInsert => {
                    assert_eq!(store.select_single(10_000), 1, "{engine:?} single insert");
                }
                Workload::CompositeKeyInsert => {
                    let (tenant, seq) = composite_key_for_offset(10_000);
                    assert_eq!(
                        store.select_composite(&tenant, seq),
                        1,
                        "{engine:?} composite insert"
                    );
                }
                _ => unreachable!(),
            }
        }
    }
}
