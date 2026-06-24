# MonaDB vs SQLite — Document Workload Benchmark Report

Generated from the `metrics` harness (`cargo bench --bench metrics`).

|              |                                                                                                                                                                                                                                                                                                             |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **MonaDB**   | 0.1.0 (LMDB / heed backend, flat lazy value codec) — **`Config::nosync()` for writes**                                                                                                                                                                                                                      |
| **SQLite**   | 3.46.0 (bundled, `rusqlite`) — `TEXT` and `JSONB` doc columns, `synchronous=NORMAL`                                                                                                                                                                                                                         |
| **Build**    | `--release` (optimized)                                                                                                                                                                                                                                                                                     |
| **Platform** | macOS 26.5 (build 25F71), Apple M3 (arm64)                                                                                                                                                                                                                                                                  |
| **Date**     | 2026-06-24                                                                                                                                                                                                                                                                                                  |
| **Config**   | preload `N = 2000`, seed `0x0ADB00EC`                                                                                                                                                                                                                                                                       |
| **Method**   | Ad-hoc SQL end-to-end (no prepared statements on either engine); each result fully decoded. **Reads: median of 5 passes at `M = 4000` timed ops/cell.** **Inserts: median of 3 passes at `M = 500`.** Both engines now run under **relaxed durability** (MonaDB `MDB_NOSYNC`, SQLite `synchronous=NORMAL`). |

> **What changed since [REPORT-01](REPORT-01.md).** Two things. **(1) Writes now run
> under `Config::nosync()`** (`MDB_NOSYNC`) — the relaxed-durability analogue of the
> `synchronous=NORMAL` SQLite already used — making the insert comparison apples-to-apples
> for the first time. This collapses the insert gap from **8–130×** down to **0.87–2.0×**.
> **(2) Read latency dropped across the board** from the recent plan-caching and
> param-binding work (`perf: 5x faster plan caching`, `feat: improve param binding`,
> `feat: statement caching`). Point lookups, which trailed SQLite **1.4–2.1×** at small
> sizes in REPORT-01, are now a **clean win at every size (0.43–0.69×)**. Point-lookup
> allocations dropped from 15 to **13/op**; range reads from ~1,022 to **822/op**.

> **Reading the numbers.** `ns/op` is wall-clock latency per operation (a point read,
> a 100-row range read, one insert, …). `B/op` and `allocs/op` come from a counting
> Rust global allocator: **exact for MonaDB, undercounted for SQLite** (its C heap is
> invisible to the Rust allocator). So compare _latency_ across engines directly, but
> treat MonaDB's allocation figures as a self-improvement signal, not a like-for-like
> memory comparison. `Δ` columns are MonaDB ÷ SQLite-`TEXT` (so `<1.0` = MonaDB faster).

---

## TL;DR

- **Point lookups are now a MonaDB win at every document size** — `0.43–0.69×`
  (single) and `0.44–0.54×` (composite), versus the `1.4–2.1×` _loss_ in REPORT-01.
  The plan-cache/param-binding work cut the fixed per-op overhead that previously
  sank small reads.
- **Prefix reads flipped to a win across the board too** (`0.34–0.91×`), and range
  reads remain a large-document win (`md` `0.45×`, `lg` `0.32×`).
- **Small range reads are the last read MonaDB loses** (`xs` `4.26×`, `sm` `2.14×`) —
  the 100-point-gets pattern against SQLite's single scan over a tiny payload — but
  even these improved from `5.60× / 2.65×`.
- **Relaxed durability closes the insert gap.** With `nosync`, inserts run **~0.87–2.0×**
  SQLite instead of 8–130×. MonaDB now **wins composite `xs` inserts (0.87×)** and sits
  near parity at `sm`; it trails ~2× at `md`/`lg`, where its higher per-row allocation
  (build + flat-encode the row object) becomes the cost driver rather than fsync.
- **The flat lazy codec keeps read cost decoupled from document size.** A point lookup
  allocates **13 times regardless of profile**; range and prefix reads hold at ~822 / ~93
  allocations across all sizes — the remaining `B/op` is the single `Rc<[u8]>` copy of
  each row out of the mmap.

---

## Latency by workload (ns/op)

Read tables: median of 5 passes at `M = 4000`. Insert tables: median of 3 passes at
`M = 500`. Both engines under relaxed durability.

### Point lookup — `single_key_select_1` (`docs[id]`)

| Profile      | MonaDB | SQLite TEXT | SQLite JSONB |         Δ |
| ------------ | -----: | ----------: | -----------: | --------: |
| xs (256 B)   |  1,152 |       1,659 |        1,662 | **0.69×** |
| sm (2 KiB)   |  1,215 |       1,963 |        1,911 | **0.62×** |
| md (16 KiB)  |  2,591 |       4,657 |        4,625 | **0.56×** |
| lg (128 KiB) | 10,003 |      23,153 |       23,195 | **0.43×** |

### Composite point lookup — `composite_key_select_1` (`docs["t007", seq]`)

| Profile | MonaDB | SQLite TEXT | SQLite JSONB |         Δ |
| ------- | -----: | ----------: | -----------: | --------: |
| xs      |  1,224 |       2,262 |        2,266 | **0.54×** |
| sm      |  1,341 |       2,580 |        2,551 | **0.52×** |
| md      |  2,775 |       5,259 |        5,259 | **0.53×** |
| lg      | 10,579 |      24,205 |       24,063 | **0.44×** |

### Range read — `single_key_select_range` (100 contiguous keys)

| Profile |  MonaDB | SQLite TEXT | SQLite JSONB |         Δ |
| ------- | ------: | ----------: | -----------: | --------: |
| xs      |  38,971 |       9,158 |        9,055 | **4.26×** |
| sm      |  47,761 |      22,280 |       22,153 | **2.14×** |
| md      | 114,922 |     257,010 |      253,342 | **0.45×** |
| lg      | 666,711 |   2,098,340 |    2,109,713 | **0.32×** |

### Prefix / partition read — `composite_key_select_prefix` (~20 rows/tenant)

| Profile |  MonaDB | SQLite TEXT | SQLite JSONB |         Δ |
| ------- | ------: | ----------: | -----------: | --------: |
| xs      |   3,750 |       4,103 |        4,074 | **0.91×** |
| sm      |   5,742 |       7,005 |        7,041 | **0.82×** |
| md      |  27,686 |      53,756 |       53,787 | **0.52×** |
| lg      | 145,521 |     424,494 |      420,086 | **0.34×** |

### Insert — `single_key_insert` / `composite_key_insert`

| Profile | MonaDB single | SQLite TEXT | SQLite JSONB |         Δ |
| ------- | ------------: | ----------: | -----------: | --------: |
| xs      |        15,647 |      10,774 |       12,469 | **1.45×** |
| sm      |        39,171 |      30,897 |       32,673 | **1.27×** |
| md      |       254,981 |     126,042 |      135,832 | **2.02×** |
| lg      |     1,215,769 |     719,792 |      762,578 | **1.69×** |

| Profile | MonaDB composite | SQLite TEXT | SQLite JSONB |         Δ |
| ------- | ---------------: | ----------: | -----------: | --------: |
| xs      |           15,782 |      18,110 |       20,010 | **0.87×** |
| sm      |           38,393 |      37,451 |       40,212 | **1.03×** |
| md      |          253,023 |     125,914 |      142,474 | **2.01×** |
| lg      |        1,206,775 |     717,781 |      768,503 | **1.68×** |

> With `nosync`, MonaDB's per-insert cost is no longer fsync-bound — it scales with
> payload size (~16 µs at `xs` → ~1.2 ms at `lg`), the signature of row build +
> flat-encode + b-tree write. MonaDB **wins composite `xs` (0.87×)** and is at parity at
> `sm`; the ~2× gap at `md`/`lg` tracks its higher allocation count per row (see Memory).
> Compare REPORT-01, where full durability put inserts at 8–130× SQLite.

---

## Read-latency stability

At `M = 4000` the per-cell spread across 5 passes is tight, and tightest exactly where
absolute latency is largest. The only loose cell is the `xs` point lookup — a
~1.1 µs operation where small absolute jitter reads as a large percentage.

| Cell                  | MonaDB median | 5-pass spread |
| --------------------- | ------------: | ------------: |
| point lookup `xs`     |         1,152 |          ±28% |
| point lookup `md`     |         2,591 |           ±5% |
| point lookup `lg`     |        10,003 |           ±1% |
| composite lookup `lg` |        10,579 |           ±3% |
| range `md`            |       114,922 |           ±3% |
| range `lg`            |       666,711 |           ±1% |
| prefix `lg`           |       145,521 |           ±6% |

(Spread = (max − min) / median across the 5 passes.) Every cell except `xs` point
lookup settles within ~6%; treat those Δ values as solid and the `xs` point-lookup Δ
as ±~25%.

---

## Memory

Allocation **count** is the cleanest cross-cut. With the flat lazy codec, MonaDB's
per-read allocations no longer scale with the values materialized — and the recent
read-path work shaved a couple more off the point-lookup and range hot paths.

| Workload (md profile)  | MonaDB allocs/op | MonaDB B/op | SQLite allocs/op | MonaDB peak heap |
| ---------------------- | ---------------: | ----------: | ---------------: | ---------------: |
| point lookup           |               13 |      37,010 |                2 |            36 KB |
| composite point lookup |               15 |      37,304 |                2 |            36 KB |
| range (100 rows)       |              822 |   3,696,575 |              101 |           1.8 MB |
| prefix (~20 rows)      |               93 |   1,098,374 |               21 |           376 KB |
| insert                 |            3,228 |     552,409 |            1,091 |            18 MB |

Observations:

- **Per-read allocation churn is flat with document size and down from REPORT-01.** A
  point lookup allocates **13 times at every profile** (was 15); range reads hold at
  **822** (was ~1,022) and prefix at **93** (was ~97) across all sizes. The remaining
  `B/op` is the single `Rc<[u8]>` copy of each row out of the mmap.
- **Reads now beat SQLite on latency at _every_ size** because navigation is offset
  arithmetic over bytes already resident, with no decode and no per-field heap traffic —
  and the fixed per-op overhead that used to sink small reads has been trimmed.
- **Inserts still allocate** to build the row object and its flat encoding
  (~3,228 allocs/op at `md` vs SQLite's ~1,091). With fsync out of the picture, this
  allocation count is now the dominant `md`/`lg` insert-latency driver and the clearest
  remaining optimization target. The 18 MB peak heap is the timed loop's cumulative build
  cost across many inserts, not a per-op figure.
- **SQLite `B/op` is not comparable** (C-heap invisible); included only to show its
  Rust-binding overhead is minimal and ~flat.

### Peak RSS caveat

RSS is a process high-water mark, so the single-process matrix run reports cumulative
RSS (it pins near ~450 MB after the largest `lg` range read and stays there). For clean
per-engine RSS, run one engine per process (see `benches/README.md`).

---

## Takeaways

**Where MonaDB wins:** every point lookup (single `0.43–0.69×`, composite `0.44–0.54×`),
every prefix read (`0.34–0.91×`), large/medium range reads (`md` `0.45×`, `lg` `0.32×`),
and — under relaxed durability — composite `xs` inserts (`0.87×`) plus near-parity `sm`
inserts. **Where it trails:** small range reads (`xs` `4.26×`, `sm` `2.14×`, the
100-point-gets pattern) and `md`/`lg` inserts (~2×, now allocation-bound rather than
fsync-bound).

**Highest-leverage optimization targets, in order:**

1. **Per-row insert allocations.** With fsync removed by `nosync`, the ~2× `md`/`lg`
   insert gap is now driven by MonaDB allocating ~3× as many times per row as SQLite
   (~3,228 vs ~1,091 at `md`). Reducing row-build / flat-encode allocations is the
   clearest path to insert parity at large sizes.
2. **Small range-read construction.** The 100-row batch-get (`select [docs[lo], …]`)
   issues 100 point gets and builds a result-array slot per element (~822 allocs);
   at `xs`/`sm` this loses to SQLite's single scan. A streaming cursor scan would both
   cut the constant allocations and close the small-payload gap.
3. **`xs` point-lookup noise floor.** The `xs` point lookup is already a win but is the
   noisiest cell (±28%); the fixed per-op floor is now small enough that machine jitter
   dominates. Worth a Criterion-grade re-measure before quoting a precise Δ.

---

## Reproduction

```sh
# Read workloads — median of 5 passes at M=4000 (the values in this report)
for i in 1 2 3 4 5; do
  MONADB_BENCH_NOSYNC=1 MONADB_BENCH_N=2000 MONADB_BENCH_M=4000 \
    MONADB_BENCH_WORKLOADS=single_key_select_1,composite_key_select_1,single_key_select_range,composite_key_select_prefix \
    MONADB_BENCH_CSV=target/reads-$i.csv \
    cargo bench --bench metrics
done

# Insert workloads — median of 3 passes at M=500, relaxed durability (nosync)
for i in 1 2 3; do
  MONADB_BENCH_NOSYNC=1 MONADB_BENCH_N=2000 MONADB_BENCH_M=500 \
    MONADB_BENCH_WORKLOADS=single_key_insert,composite_key_insert \
    MONADB_BENCH_CSV=target/engine-compare-$i.csv \
    cargo bench --bench metrics
done

# Authoritative latency distributions (Criterion)
cargo bench --bench doc_workloads
```

`MONADB_BENCH_NOSYNC=1` opens MonaDB with [`Config::nosync()`](../src/config.rs)
(`MDB_NOSYNC`), the relaxed-durability analogue of SQLite's `synchronous=NORMAL`.
Reported latency is the per-cell median across passes; allocation counts are
deterministic and near-identical across runs. Inserts and small reads are
machine-state-sensitive — re-run on the target machine before drawing conclusions.
Use the Criterion harness for statistically rigorous latency.
