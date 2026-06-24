# Batched Preload + Cardinality Sweep Knob

**Status:** ready to implement · **Area:** benchmark harness / scaling-measurement enabler
**Scope:** `benches/` only — no engine (`src/`) changes.
**Prereq for:** empirically measuring point/composite lookup scaling at 100k–1M
(plans 01–08 reason about large-N analytically because today we cannot measure it).

---

## 1. Background (read this first — assumes no prior context)

MonaDB is benchmarked against SQLite by a metrics harness (`cargo bench --bench
metrics`, entry `benches/metrics.rs`) and a Criterion harness (`doc_workloads`).
Both dispatch through one engine-agnostic trait, `DocStore` (`benches/store.rs`),
implemented by `MonaDbBench` (`benches/monadb.rs`) and `SqliteBench`
(`benches/sqlite.rs`). A run is a matrix of *workload × profile × cardinality ×
engine* (`benches/config.rs::BenchConfig`).

For a **read** workload, `measure()` (`benches/metrics.rs:96`) first **preloads**
`cardinality` documents (untimed), runs an untimed warmup, then times `M`
lookups. Preload is `workloads::preload` (`benches/workloads.rs:62`), which loops
calling `store.insert(spec)` once per row.

Two harness limitations block a lookup-scaling study:

### Problem A — preload commits one durable transaction per row

`MonaDbBench::insert` (`benches/monadb.rs:40`) calls `self.db.execute(&sql)` per
row. Each `execute` opens a write txn and **commits it with an fsync**. The
REPORT measures MonaDB single inserts at **~5.4 ms** under sustained pressure, so
preload time grows linearly:

| N (cardinality) | preload ≈ N × ~5 ms |
|-----------------|---------------------|
| 1,000           | ~5 s                |
| 10,000          | ~50 s               |
| 100,000         | ~8 min              |
| 1,000,000       | ~80+ min            |

This makes the cells the investigation wants (1 / 1,000 / 100,000 / 1,000,000)
infeasible. The root cause (per-row fsync) is the *same* phenomenon as MonaDB's
#1 weakness, the insert gap — but the **preload is untimed setup**, so batching it
changes only setup wall-clock, never a measured lookup number (preload runs
*before* `alloc::reset()` and the `Instant::now()` at `benches/metrics.rs:129`).

### Problem B — no multi-cardinality sweep knob

`BenchConfig` carries `cardinalities: Vec<usize>` (default `[10_000, 100_000]`,
`benches/config.rs:199`), but `from_env` only honors `MONADB_BENCH_N`, which
**overwrites** the vec with a single value (`config.rs:216-220`). There is no way
to sweep several cardinalities in one invocation, so a scaling curve requires
several manual runs with hand-merged CSVs.

---

## 2. Goals / Non-goals

**Goals**
1. Make preloading 100k–1M rows take seconds, not minutes — without changing any
   timed measurement or weakening what the lookup benchmark reports.
2. Let one harness invocation sweep an explicit list of cardinalities
   (`MONADB_BENCH_CARDINALITIES=1,1000,100000,1000000`).
3. Keep the MonaDB/SQLite comparison fair (batch both engines symmetrically).
4. Define honest semantics for composite cardinalities below `TENANTS`.

**Non-goals**
- No engine/bulk-insert API change (that is the insert-gap work; see plan 08's
  sibling note and §8 below).
- No change to timed read/insert loops or to what gets measured.
- Not trying to make MonaDB *inserts* fast — only the untimed preload.

---

## 3. Constraints & prerequisites (IMPORTANT — read before running 1M)

**The LMDB map size is a hard ceiling.** `LMDB_MMAP_SIZE` is a compile-time const
of **1 GiB** (`src/storage.rs:19`), passed to `.map_size()` (`storage.rs:47`); it
is **not** configurable through `Config` (`src/config.rs` exposes only `nosync`).
A write that would exceed the mapped size fails with `MDB_MAP_FULL`. Approximate
on-disk footprint is `N × profile.target_bytes` plus btree overhead:

| profile (bytes) | 100k     | 1,000,000           |
|-----------------|----------|---------------------|
| xs (256 B)      | ~26 MB   | **~256 MB** ✅       |
| sm (2 KiB)      | ~205 MB  | **~2 GB** ❌ > 1 GiB |
| md (16 KiB)     | ~1.6 GB ❌ | ~16 GB ❌            |
| lg (128 KiB)    | ~13 GB ❌ | ~128 GB ❌           |

**Implication:** the 1,000,000 sweep is only feasible at the **`xs`** profile
under today's 1 GiB map. To measure larger profiles at high N you must first
raise `LMDB_MMAP_SIZE` (or make it `Config`-tunable and have the bench request a
bigger map). Treat that as a *separate, optional* follow-up; this plan targets
the `xs` scaling curve, which is exactly where the lookup gap with SQLite is
widest and most interesting. **Document this ceiling in the run instructions** so
a future reader does not burn time on a sweep that is structurally impossible.

Map sizing is virtual address reservation, not eager allocation, so a generous
map (e.g. 16–64 GiB) is cheap on 64-bit — a reasonable future default if we want
high-N at larger profiles.

---

## 4. Design overview

Two independent changes:

1. **Bulk-preload bracketing.** Add optional begin/end hooks to `DocStore` with
   default no-ops. `preload` brackets its insert loop with them. `MonaDbBench`
   implements them as a (chunked) session transaction via `begin;`/`commit;`;
   `SqliteBench` implements them as a (chunked) SQL transaction. Non-batched
   stores inherit the no-op defaults and stay correct.

2. **Cardinality list knob.** Add `MONADB_BENCH_CARDINALITIES` parsed via the
   existing `parse_list` helper, with documented precedence over
   `MONADB_BENCH_N`.

Chunking (commit every `CHUNK` rows, then re-open the txn) is the key robustness
detail: it keeps fsync count at `N/CHUNK` (orders of magnitude below `N`) while
bounding the dirty-page set / RSS so a 1M preload does not balloon memory or
strain the map within a single never-committing txn.

---

## 5. Detailed changes

### 5.1 `DocStore` trait — `benches/store.rs`

Add two defaulted hooks (keep the trait object-safe; `open_store` returns
`Box<dyn DocStore>`):

```rust
pub trait DocStore {
    // ... existing methods unchanged ...

    /// Begins a bulk-insert region: subsequent `insert`s should batch into one
    /// (chunked) transaction instead of committing per row. Default: no-op.
    fn begin_bulk(&mut self) {}

    /// Notes that `inserted` rows have been written since `begin_bulk`, giving
    /// the store a chance to flush a chunk. Default: no-op.
    fn bulk_checkpoint(&mut self, _inserted: usize) {}

    /// Ends the bulk-insert region, committing any open batch. Default: no-op.
    fn end_bulk(&mut self) {}
}
```

> Design choice: three thin hooks (begin / periodic checkpoint / end) rather than
> a single `preload_batch(specs)` method. This keeps `preload`'s existing
> per-row `DocSpec` generation loop intact (it already maps offsets →
> `DocSpec::single/composite`) and leaves chunk policy to the driver, not each
> adapter. The default no-ops mean any future store is correct without
> implementing them.

### 5.2 Driver — `benches/workloads.rs::preload` (`:62`)

Bracket the loop and checkpoint each row. Pick `CHUNK` (e.g. `10_000`) as a
`const` here so both engines share one policy:

```rust
pub const PRELOAD_CHUNK: usize = 10_000;

pub fn preload(store: &mut dyn DocStore, workload: Workload, profile: Profile, cardinality: usize) {
    store.begin_bulk();
    let mut written = 0usize;
    if workload.is_composite() {
        let per_tenant = cardinality / TENANTS;       // see §6 edge case
        for tenant in 0..TENANTS as i64 {
            for seq in 0..per_tenant as i64 {
                store.insert(&DocSpec::composite(profile, tenant, seq));
                written += 1;
                store.bulk_checkpoint(written);
            }
        }
    } else {
        for id in 0..cardinality as i64 {
            store.insert(&DocSpec::single(profile, id));
            written += 1;
            store.bulk_checkpoint(written);
        }
    }
    store.end_bulk();
}
```

(`bulk_checkpoint` is called every row but only acts every `PRELOAD_CHUNK`; the
counter compare is negligible against the insert. Alternatively pass `CHUNK` into
the adapter and let it self-count — either is fine.)

### 5.3 MonaDB adapter — `benches/monadb.rs`

Implement the hooks against the existing session-transaction API
(`src/lib.rs:329-345`, `begin;`/`commit;`). Track a row counter so checkpoints
commit-and-reopen:

```rust
// add a field: bulk_open: bool, and reuse PRELOAD_CHUNK
fn begin_bulk(&mut self) {
    self.db.execute("begin;").expect("begin bulk");
    self.bulk_open = true;
}
fn bulk_checkpoint(&mut self, inserted: usize) {
    if self.bulk_open && inserted % PRELOAD_CHUNK == 0 {
        self.db.execute("commit;").expect("commit chunk");
        self.db.execute("begin;").expect("reopen bulk");
    }
}
fn end_bulk(&mut self) {
    if self.bulk_open {
        self.db.execute("commit;").expect("commit bulk");
        self.bulk_open = false;
    }
}
```

While a session txn is active, `insert`'s `execute` buffers into it
(`defer_commit` path, `src/vm.rs` `Transaction`/`Halt`) instead of committing per
row — that is the whole win. No change to `MonaDbBench::insert` itself.

### 5.4 SQLite adapter — `benches/sqlite.rs` (fairness)

Mirror the batch so both engines amortize identically:

```rust
fn begin_bulk(&mut self)            { self.conn.execute_batch("BEGIN;").expect("begin"); self.bulk_open = true; }
fn bulk_checkpoint(&mut self, n: usize) {
    if self.bulk_open && n % PRELOAD_CHUNK == 0 {
        self.conn.execute_batch("COMMIT; BEGIN;").expect("chunk");
    }
}
fn end_bulk(&mut self)              { if self.bulk_open { self.conn.execute_batch("COMMIT;").expect("commit"); self.bulk_open = false; } }
```

SQLite already runs `journal_mode=WAL, synchronous=NORMAL` (`apply_pragmas`,
`sqlite.rs:87`), so its per-row preload is cheap and batching mostly removes
statement overhead — but keeping the shape symmetric keeps the comparison honest.

> **Aside (fairness, not this plan):** the MonaDB adapter opens with full
> durability (`MonaDB::open`, default `Config`) while SQLite runs
> `synchronous=NORMAL`. For an apples-to-apples *insert* comparison one could open
> MonaDB with `Config::default().nosync()` (`src/config.rs:18`, the `MDB_NOSYNC`
> analog of `synchronous=NORMAL`). That also happens to speed preload, but it is a
> durability-semantics decision for the REPORT, tracked separately from this
> harness enabler.

### 5.5 Config knob — `benches/config.rs::from_env` (`:209`)

Add a branch using the existing `parse_list` (`:245`), with precedence:

```rust
// after the MONADB_BENCH_N branch
if let Ok(v) = env::var("MONADB_BENCH_CARDINALITIES") {
    let cards: Vec<usize> = v.split(',').filter_map(|s| s.trim().parse().ok()).collect();
    if !cards.is_empty() {
        cfg.cardinalities = cards;   // explicit list wins over MONADB_BENCH_N
    }
}
```

Precedence rule to document: **`MONADB_BENCH_CARDINALITIES` (list) overrides
`MONADB_BENCH_N` (single)** for the read-cardinality sweep. `MONADB_BENCH_N`
still sets `cfg.n` (the insert-workload base/preload count for non-swept
workloads), so leave that assignment intact.

### 5.6 No timing-path change

`measure()` keeps preload before `alloc::reset()`/`Instant::now()`
(`benches/metrics.rs:106-130`); Criterion `doc_workloads` reads the same
`BenchConfig`, so the new knob and batching flow through both harnesses with no
further edits.

---

## 6. Edge cases

- **Composite cardinality < `TENANTS` (=100).** `per_tenant = cardinality /
  TENANTS` is `0` for `cardinality ∈ {1, …, 99}` → empty table → every composite
  lookup is a **miss**. Decision: **keep the integer-division math, do not clamp,
  and label the cell.** Clamping (`per_tenant.max(1)`) would silently insert 100
  rows for "N=1" and mislabel cardinality. Instead, in the report/CSV note that
  *composite cells with cardinality < 100 measure the all-miss path*. Practically,
  run the composite sweep at `100,1000,100000,1000000` and the single-key sweep at
  `1,1000,100000,1000000`. (Optional nicety: have the metrics table append an
  `(all-miss)` marker when `is_composite && cardinality < TENANTS`.)
- **Chunk boundary vs final commit.** `end_bulk` must commit whatever is open even
  if the last chunk was partial; the `% CHUNK == 0` checkpoint plus an
  unconditional `end_bulk` commit covers both. Guard with the `bulk_open` flag so
  a double-commit can't happen.
- **Mid-preload failure.** The adapters `.expect(...)`; a panic aborts the
  process and the `TempDir` cleans up — acceptable for a bench. If we later make
  preload fallible, ensure a dangling `begin;` is rolled back (`rollback;` /
  `ROLLBACK;`) so the env isn't left mid-transaction.
- **`MONADB_BENCH_N` + `MONADB_BENCH_CARDINALITIES` both set.** List wins for the
  sweep (documented); `cfg.n` still comes from `N`.

---

## 7. Validation

**Correctness (no behavior drift):**
1. `cargo bench --bench metrics` with defaults: confirm the table/CSV match the
   prior shape (same workloads/profiles/engines), only faster preload.
2. Spot-check a small cell against a non-batched build: the **timed** `ns/op`,
   `B/op`, and `allocs/op` for `single_key_select_1` should be unchanged within
   noise (batching touches only untimed setup). This is the key regression guard.
3. Smoke test (`benches/main.rs` smoke path / `composite_key_for_offset`) still
   reads back a known inserted row.

**The scaling sweep (the payoff):**
```sh
# Single-key point lookup scaling, xs profile (the only profile that fits 1M
# under the 1 GiB map — see §3). MonaDB vs SQLite-text, point lookups only.
MONADB_BENCH_CARDINALITIES=1,1000,100000,1000000 \
MONADB_BENCH_PROFILES=xs \
MONADB_BENCH_ENGINES=monadb,sqlite_text \
MONADB_BENCH_WORKLOADS=single_key_select_1 \
MONADB_BENCH_M=2000 \
MONADB_BENCH_CSV=target/scale-single.csv \
  cargo bench --bench metrics

# Composite point lookup scaling (cardinality >= TENANTS=100; see §6).
MONADB_BENCH_CARDINALITIES=100,1000,100000,1000000 \
MONADB_BENCH_PROFILES=xs \
MONADB_BENCH_ENGINES=monadb,sqlite_text \
MONADB_BENCH_WORKLOADS=composite_key_select_1 \
MONADB_BENCH_M=2000 \
MONADB_BENCH_CSV=target/scale-composite.csv \
  cargo bench --bench metrics
```
Expected wall-clock after batching: each preload step is seconds even at 1M (xs),
versus ~80 min before. Use median-of-3 (per REPORT method) before drawing
latency conclusions — the metrics harness takes a single sample per cell, so the
deterministic `allocs/op` column is the trustworthy scaling signal; wrap the
runs in the REPORT's 3-pass median for latency.

**Success criteria:** the 1M (xs) single-key cell completes; `allocs/op` stays
flat across cardinality (confirming the analytical "fixed overhead, N-independent"
claim that motivated plans 01–05); the latency curve for both engines is
observable and comparable.

---

## 8. Risks & mitigations

| Risk | Mitigation |
|---|---|
| Single giant txn spikes RSS / strains the 1 GiB map at high N | **Chunked commit** (`PRELOAD_CHUNK`, §5.2) bounds dirty pages; commit count is `N/CHUNK`. |
| 1M sweep at `sm`/`md`/`lg` fails with `MDB_MAP_FULL` | Documented in §3; restrict 1M to `xs`, or raise `LMDB_MMAP_SIZE` first (follow-up). |
| Batching only one engine skews the comparison | Mirror BEGIN/COMMIT in `SqliteBench` (§5.4). |
| Default no-op hooks break a non-batched store | Defaults are pure no-ops; only the two adapters override. |
| Dangling `begin;` after a mid-preload error | `bulk_open` guard + (future) `rollback;`; today `.expect` aborts the process and `TempDir` cleans up. |
| "N=1" composite silently means 100 rows if clamped | Don't clamp; label all-miss cells (§6). |

---

## 9. Out of scope / follow-ups

- **Engine bulk-insert API** (multi-row insert / `bulk_load`) — would help
  production inserts *and* preload, but it is the insert-gap optimization
  (engine change), not a harness fix.
- **Config-tunable / larger default `LMDB_MMAP_SIZE`** — required to run high-N at
  `sm`/`md`/`lg`; cheap on 64-bit (virtual reservation). Natural companion if we
  want a full profile × cardinality scaling surface.
- **Median-of-N built into the metrics harness** — currently single-sample;
  REPORT medians three external passes. Orthogonal to this plan.

---

## 10. References

- `benches/workloads.rs:62` — `preload` (loop to bracket); `:14,64` — `TENANTS`, composite divide
- `benches/monadb.rs:40` — `MonaDbBench::insert` → `db.execute` (per-row commit today)
- `benches/sqlite.rs:61,87` — `SqliteBench::insert`, `apply_pragmas` (WAL/NORMAL)
- `benches/store.rs:17,38` — `DocStore` trait (add hooks), `open_store`
- `benches/config.rs:163` — `BenchConfig`; `:199` default cardinalities; `:209` `from_env`; `:216` `MONADB_BENCH_N`; `:245` `parse_list`
- `benches/metrics.rs:96,106-130` — `measure`; preload precedes `reset()`/timing (untimed)
- `src/lib.rs:329-353` — `begin_transaction` / `commit_transaction` (session txn used by §5.3)
- `src/storage.rs:19,47` — `LMDB_MMAP_SIZE` (1 GiB map ceiling, §3)
- `src/config.rs:18` — `Config::nosync` (`MDB_NOSYNC`; durability aside, §5.4)
- `benches/REPORT.md` — ~5.4 ms single-insert median; fsync-dominated inserts (#1 weakness)
- Related plans: 01–05 (the fixed-overhead lookup findings this sweep would validate), 08 sibling note (engine bulk-insert is the out-of-scope twin of this harness fix)
