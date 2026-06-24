# Document-Oriented Performance Benchmarks

Compare MonaDB to SQLite (TEXT JSON and JSONB) on embedded document workloads. Each timed iteration runs **ad-hoc SQL end-to-end** — no prepared statements on either engine — so document size affects parse, serialize, and I/O together.

Two harnesses share one set of workload definitions:

- **`doc_workloads`** — Criterion, the authoritative **latency** harness (statistical, HTML reports).
- **`metrics`** — a single-pass **time + memory** harness that emits a CSV/table for tracking and cross-engine comparison.

Both are driven through the `DocStore` trait (`store.rs`); adding an engine means implementing that trait once, and adding a workload means one `Workload` variant plus its SQL renderers in `fixtures.rs`.

## Access patterns

| Pattern | Workload | MonaDB SQL | SQLite SQL |
|---------|----------|------------|------------|
| **Key-value get** (single) | `single_key_select_1` | `select docs[id];` | `SELECT doc FROM docs WHERE id = …` |
| **Key-value get** (composite) | `composite_key_select_1` | `select docs["t007", seq];` | `WHERE tenant = … AND seq = …` |
| **Range read** (integer key span) | `single_key_select_range` | `select [docs[lo], …, docs[hi-1]];` (batch get) | `WHERE id >= lo AND id < hi ORDER BY id` |
| **Prefix / partition read** | `composite_key_select_prefix` | `select docs["t007"];` (GetRange array) | `WHERE tenant = 't007' ORDER BY seq` |
| **Write** (single key) | `single_key_insert` | inline object insert | `INSERT … VALUES (id, doc)` |
| **Write** (composite key) | `composite_key_insert` | inline object insert | composite PK insert |

Range reads fetch a contiguous span of `MONADB_BENCH_RANGE` documents (default **100**) per query. Prefix reads return all documents for one of **100** tenant partitions (`N / 100` rows at preload cardinality `N`).

MonaDB expresses ranged/prefix reads through its keyed-table index syntax (`docs[key…]`), which maps to LMDB point gets and prefix scans. SQLite uses primary-key range predicates and returns one row per document.

## Workloads

| Workload | Description |
|----------|-------------|
| `single_key_select_1` | Point lookup by integer key |
| `single_key_select_range` | Range read over `[lo, hi)` integer keys |
| `single_key_insert` | Autocommit insert of M fresh documents |
| `composite_key_select_1` | Point lookup by `(tenant, seq)` |
| `composite_key_select_prefix` | Prefix read — all docs for one tenant |
| `composite_key_insert` | Autocommit composite-key inserts |

Crossed with document profiles:

| Profile | Target JSON size | Shape |
|---------|------------------|-------|
| `xs` | ~256 B | Flat scalars |
| `sm` | ~2 KiB | Metadata + tags |
| `md` | ~16 KiB | 20 line items |
| `lg` | ~128 KiB | Padded content + audit log |

And three engines: `monadb`, `sqlite_text`, `sqlite_jsonb`.

## Run

```sh
# Full matrix (slow — dozens of Criterion cases)
cargo bench --bench doc_workloads

# Filter by Criterion ID substring
cargo bench --bench doc_workloads -- single_key_select_1/md/10k/monadb

# Environment filters
MONADB_BENCH_PROFILES=xs,sm \
MONADB_BENCH_WORKLOADS=single_key_select_1,single_key_insert \
MONADB_BENCH_ENGINES=monadb,sqlite_jsonb \
MONADB_BENCH_N=10000 \
MONADB_BENCH_M=1000 \
  cargo bench --bench doc_workloads
```

### Environment variables

| Variable | Default | Meaning |
|----------|---------|---------|
| `MONADB_BENCH_M` | `10000` | Timed operations per Criterion sample |
| `MONADB_BENCH_N` | `10000` | Preload cardinality for select workloads; insert key offset base |
| `MONADB_BENCH_PROFILES` | `xs,sm,md,lg` | Comma-separated profile list |
| `MONADB_BENCH_WORKLOADS` | all six | Comma-separated workload filter |
| `MONADB_BENCH_ENGINES` | all three | Comma-separated engine filter |
| `MONADB_BENCH_RANGE` | `100` | Row span for `single_key_select_range` |
| `MONADB_BENCH_SEED` | `0x0ADB00EC` | RNG seed for lookup key selection |
| `MONADB_BENCH_CSV` | `target/bench-metrics.csv` | Output path for the `metrics` harness CSV |

## Fairness controls

1. **Fresh database per sample** — Criterion `iter_batched` setup opens a new temp DB, runs DDL, and preloads (select workloads only).
2. **Warmup** — 100 random lookups after preload, outside the timed loop.
3. **Full result consumption** — MonaDB rows are decoded/re-encoded; SQLite `doc` bytes are read fully.
4. **SQLite pragmas** — `WAL`, `synchronous=NORMAL`, `cache_size=-64000` (64 MiB).
5. **MonaDB** — default LMDB settings (`map_size` 1 GiB, full durability). Use
   [`Config::nosync()`](../src/config.rs) with
   [`MonaDB::open_with_config`](../src/lib.rs) for `MDB_NOSYNC` (SQLite
   `synchronous=NORMAL` analogue). Explicit `begin;` / `commit;` batch multiple
   statements under one fsync.

## Insert breakdown harness

Isolates fsync vs SQL overhead:

```sh
cargo bench --bench insert_breakdown
MONADB_BENCH_M=100 MONADB_BENCH_PROFILE=md cargo bench --bench insert_breakdown
```

| Mode | What it measures |
|------|------------------|
| `autocommit` | One `execute()` per row (baseline) |
| `explicit_txn` | `begin;` + N inserts + `commit;` |
| `multi_value` | One `insert into t ({…}, …, {…});` |
| `prepared_param` | `insert into t ($1)` with bound params |
| `relaxed_autocommit` | Autocommit with `Config::nosync()` |

## Metrics harness (time + memory)

```sh
# Single-pass matrix; writes target/bench-metrics.csv + prints a table
cargo bench --bench metrics

# Same env filters as doc_workloads
MONADB_BENCH_PROFILES=xs,md MONADB_BENCH_ENGINES=monadb cargo bench --bench metrics
```

Per matrix cell it reports:

| Column | Meaning |
|--------|---------|
| `ns_per_op` | Wall-clock nanoseconds per operation |
| `bytes_alloc_per_op` | Rust heap bytes allocated per operation |
| `allocs_per_op` | Allocation count per operation |
| `peak_heap_bytes` | Peak live-heap growth during the timed loop |
| `peak_rss_bytes` | Process peak resident set size |

**Memory caveats:**

- The allocation columns come from a counting `#[global_allocator]` that only sees
  **Rust** allocations. MonaDB's numbers are exact and are the actionable signal for
  reducing its memory use. SQLite's bundled-C heap is invisible to it, so SQLite's
  allocation columns undercount — **do not** compare allocation bytes across engines.
- For a fair **cross-engine** memory comparison use `peak_rss_bytes`. Because RSS is a
  process high-water mark, a single in-process matrix run reports cumulative RSS. For
  clean per-engine RSS, run one engine per process:

  ```sh
  for e in monadb sqlite_text sqlite_jsonb; do
    MONADB_BENCH_ENGINES=$e MONADB_BENCH_CSV=target/rss-$e.csv \
      cargo bench --bench metrics
  done
  ```

  The allocation columns are reset per cell and stay accurate in a single run.

## Smoke test (CI-friendly)

```sh
cargo test --test bench_smoke
```

Runs 10 operations per workload × `xs` profile × all three engines (including range and prefix reads).

## Reading results

Criterion prints HTML reports under `target/criterion/`. Benchmark IDs follow:

```
{workload}/{profile}/{cardinality}/{engine}
```

Examples:

- `single_key_select_1/md/100k/monadb`
- `single_key_select_range/md/10k/monadb`
- `composite_key_select_prefix/md/100k/sqlite_text`
- `single_key_insert/lg/empty/sqlite_jsonb`

**Headline chart:** median latency vs encoded JSON bytes across profiles — that is the embedded document database story.

**Footnote:** Production SQLite apps typically use prepared statements. This suite measures ad-hoc SQL honestly; MonaDB would fare relatively better on a future "hot path" comparison with statement caching.

For tracking time + memory over releases, use the `metrics` harness and its CSV (see [Metrics harness](#metrics-harness-time--memory)). Criterion's report remains the authoritative latency view.
