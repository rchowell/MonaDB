# MonaDB vs SQLite — Document Workload Benchmark Report

Generated from the `metrics` harness (`cargo bench --bench metrics`).

| | |
|---|---|
| **MonaDB** | 0.1.0 (LMDB / heed backend, flat lazy value codec) |
| **SQLite** | 3.46.0 (bundled, `rusqlite`) — `TEXT` and `JSONB` doc columns |
| **Build** | `--release` (optimized) |
| **Platform** | macOS (darwin 26.5), Apple M3 (arm64) |
| **Date** | 2026-06-24 |
| **Config** | preload `N = 2000`, timed ops `M = 500`, seed `0x0ADB00EC` |
| **Method** | Ad-hoc SQL end-to-end (no prepared statements on either engine); each result fully decoded. **Latency = median of 3 back-to-back matrix runs** to damp single-sample noise. |

> **Reading the numbers.** `ns/op` is wall-clock latency per operation (a point read,
> a 100-row range read, one insert, …). `B/op` and `allocs/op` come from a counting
> Rust global allocator: **exact for MonaDB, undercounted for SQLite** (its C heap is
> invisible to the Rust allocator). So compare *latency* across engines directly, but
> treat MonaDB's allocation figures as a self-improvement signal, not a like-for-like
> memory comparison. `Δ` columns are MonaDB ÷ SQLite-`TEXT` (so `<1.0` = MonaDB faster).

> **What changed since the prior report (2026-06-21).** (1) **Reads improved
> further** — single-key lookups at `xs`/`sm` dropped to ~2.5–4.7 µs (was ~3.4–5.6 µs),
> and MonaDB now **wins at `md`/`lg`** on point reads (0.61–0.89×). Large-document
> range reads remain a clear win (0.30× at `lg`); prefix reads improved at `xs`/`md`
> (0.68–0.76×). (2) **Insert medians shifted** — MonaDB's per-insert fsync cost
> measured ~3.1–5.2 ms (was ~5.3–6.8 ms) while SQLite's small inserts also dropped on
> this machine, so the *ratio* at `xs`/`sm` widened even as absolute MonaDB insert
> latency fell. Run 3 of 3 showed heavy tail latency on both engines (disk/thermal
> state); medians damp that. (3) **`Config::nosync()`** is now available for
> relaxed durability (`MDB_NOSYNC`); these numbers still use default full durability.

---

## TL;DR

- **Reads are competitive or faster than SQLite on most profiles.** Point lookups are
  within ~1.2–1.7× at `xs`/`sm`, **faster at `md`/`lg`** (0.61–0.89×); 100-row range
  reads are **2.9× slower at `sm`** but **2.1× faster at `md`** (0.48×) and **3.3×
  faster at `lg`** (0.30×); prefix reads are **faster at `xs`/`md`/`lg`** (0.27–0.76×).
- **The flat lazy codec keeps read cost decoupled from document size.** A point lookup
  allocates **18 times regardless of profile**; range and prefix reads allocate a
  constant ~1,025 / ~99 times across all sizes.
- **Inserts remain MonaDB's dominant gap** — ~3.1–6.4 ms median, roughly flat across
  document size (≈2.7–182× SQLite depending on profile) — the signature of
  **committing a durable write transaction (fsync) per `execute`**. Use
  `Config::nosync()` or batched transactions to close this gap.

---

## Latency by workload (ns/op, median of 3 runs)

### Point lookup — `single_key_select_1` (`docs[id]`)

| Profile | MonaDB | SQLite TEXT | SQLite JSONB | Δ |
|---|--:|--:|--:|--:|
| xs (256 B) | 2,506 | 2,107 | 1,904 | **1.19×** |
| sm (2 KiB) | 4,718 | 2,744 | 2,658 | **1.72×** |
| md (16 KiB) | 4,788 | 5,410 | 11,560 | **0.89×** |
| lg (128 KiB) | 42,224 | 68,812 | 163,307 | **0.61×** |

### Composite point lookup — `composite_key_select_1` (`docs["t007", seq]`)

| Profile | MonaDB | SQLite TEXT | SQLite JSONB | Δ |
|---|--:|--:|--:|--:|
| xs | 4,356 | 3,069 | 2,699 | **1.42×** |
| sm | 4,169 | 3,942 | 3,427 | **1.06×** |
| md | 14,375 | 12,231 | 9,326 | **1.18×** |
| lg | 26,124 | 44,588 | 257,406 | **0.59×** |

### Range read — `single_key_select_range` (100 contiguous keys)

| Profile | MonaDB | SQLite TEXT | SQLite JSONB | Δ |
|---|--:|--:|--:|--:|
| xs | 84,760 | 14,784 | 14,382 | **5.73×** |
| sm | 88,976 | 30,692 | 31,139 | **2.90×** |
| md | 384,890 | 806,585 | 1,045,218 | **0.48×** |
| lg | 1,155,529 | 3,851,282 | 3,559,016 | **0.30×** |

### Prefix / partition read — `composite_key_select_prefix` (~20 rows/tenant)

| Profile | MonaDB | SQLite TEXT | SQLite JSONB | Δ |
|---|--:|--:|--:|--:|
| xs | 5,523 | 7,248 | 5,911 | **0.76×** |
| sm | 12,123 | 11,279 | 11,572 | **1.07×** |
| md | 54,827 | 80,768 | 86,399 | **0.68×** |
| lg | 311,724 | 1,146,248 | 988,314 | **0.27×** |

### Insert — `single_key_insert` / `composite_key_insert`

| Profile | MonaDB single | SQLite TEXT | SQLite JSONB | Δ |
|---|--:|--:|--:|--:|
| xs | 3,077,321 | 16,873 | 16,943 | **182.4×** |
| sm | 3,060,154 | 41,536 | 41,820 | **73.7×** |
| md | 3,818,574 | 178,310 | 170,729 | **21.4×** |
| lg | 5,190,111 | 1,022,044 | 1,094,306 | **5.1×** |

| Profile | MonaDB composite | SQLite TEXT | SQLite JSONB | Δ |
|---|--:|--:|--:|--:|
| xs | 4,299,449 | 33,520 | 36,959 | **128.3×** |
| sm | 3,618,144 | 77,770 | 95,246 | **46.5×** |
| md | 4,000,130 | 212,987 | 193,697 | **18.8×** |
| lg | 6,362,871 | 2,349,947 | 1,571,129 | **2.7×** |

> MonaDB's insert latency is roughly **flat at ~3.1–6.4 ms** until the `lg` payload
> rivals the fixed cost — the signature of a per-insert durable commit. SQLite uses
> `synchronous=NORMAL` in this harness; MonaDB uses full durability (`Config::default()`).
> A batched-transaction path or `Config::nosync()` would close most of the small-insert
> gap.

---

## Memory

Allocation **count** is the cleanest cross-cut. With the flat lazy codec, MonaDB's
per-read allocations no longer scale with the values materialized.

| Workload (md profile) | MonaDB allocs/op | MonaDB B/op | SQLite allocs/op | MonaDB peak heap |
|---|--:|--:|--:|--:|
| point lookup | 18 | 37,082 | 2 | 36 KB |
| range (100 rows) | 1,025 | 3,702,492 | 101 | 1.9 MB |
| prefix (~20 rows) | 99 | 1,098,637 | 21 | 386 KB |
| insert | 3,701 | 654,622 | 1,091 | 23 MB |

Observations:

- **Per-read allocation churn is flat with document size.** A point lookup allocates
  **18 times at every profile** (`xs`→`lg`). Range and prefix reads hold at ~1,025 and
  ~99 allocations across all sizes. The remaining `B/op` is the single `Rc<[u8]>`
  copy of each row out of the mmap.
- **Reads beat SQLite on latency once the payload is large** because navigation is
  offset arithmetic over bytes already resident, with no decode and no per-field heap
  traffic — the gap that remains at `xs`/`sm` is fixed per-op overhead.
- **Inserts still allocate** to build the row object and its flat encoding
  (~3,701 allocs/op at `md`), but latency is dominated by the per-`execute` fsync, not
  allocation. The 23 MB peak heap is the timed loop's cumulative build cost across 500
  inserts, not a per-op figure.
- **SQLite `B/op` is not comparable** (C-heap invisible); included only to show its
  Rust-binding overhead is minimal and ~flat.

### Peak RSS caveat

RSS is a process high-water mark, so the single-process matrix run reports cumulative
RSS (it pins near ~485 MB after the largest `lg` range read and stays there). For clean
per-engine RSS, run one engine per process (see `benches/README.md`).

---

## Takeaways

**Where MonaDB now wins:** large-document (`lg`) reads — range (0.30×) and prefix
(0.27×) reads run much faster than SQLite, and single-key point lookups are faster too
(0.61×). **Where it's competitive:** medium (`md`) reads (~0.5–0.9× on most patterns).
**Where it still trails:** small-document range reads (`xs`/`sm`, ~3–6×), where batch-get
setup cost dominates a tiny payload, and — by a wide margin — **inserts** (~3–182× under
full durability).

**Highest-leverage optimization targets, in order:**

1. **Relaxed durability or batched/bulk insert.** The ~3.1–6.4 ms flat insert cost is
   per-`execute` transaction durability (one fsync each). `Config::nosync()` (LMDB
   `MDB_NOSYNC`, SQLite `synchronous=NORMAL` analogue), multi-row transactions, or a
   bulk-load API would cut small-insert latency by 1–2 orders of magnitude.
2. **Trim fixed per-op read overhead for small documents.** At `xs`/`sm` the gap on
   range reads is setup cost (100 point gets vs one SQLite scan), not allocation.
3. **Range-read result construction.** The 100-row batch-get (`select [docs[lo], …]`)
   still constructs an offset key and result-array slot per element; a streaming cursor
   scan would shave the remaining constant ~10 allocations/row.

---

## Reproduction

```sh
# One matrix pass (this report medianed three such passes)
MONADB_BENCH_N=2000 MONADB_BENCH_M=500 \
  MONADB_BENCH_CSV=target/engine-compare.csv \
  cargo bench --bench metrics

# Repeat with -2 and -3 suffixes for median stability
MONADB_BENCH_CSV=target/engine-compare-2.csv cargo bench --bench metrics
MONADB_BENCH_CSV=target/engine-compare-3.csv cargo bench --bench metrics

# Authoritative latency distributions (Criterion)
cargo bench --bench doc_workloads
```

Raw data: `target/engine-compare{,-2,-3}.csv` (72 cells each = 6 workloads × 4 profiles
× 3 engines). Reported latency is the per-cell median across the three runs; allocation
counts are deterministic and identical across runs. Inserts are fsync-dominated and
machine/disk-state-sensitive — re-run on the target machine before drawing conclusions.
Use the Criterion harness for statistically rigorous latency.
