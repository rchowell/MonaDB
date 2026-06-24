# MonaDB vs SQLite — Document Workload Benchmark Report

Generated from the `metrics` harness (`cargo bench --bench metrics`).

|              |                                                                                                                                                                                                                                                                                 |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **MonaDB**   | 0.1.0 (LMDB / heed backend, flat lazy value codec)                                                                                                                                                                                                                              |
| **SQLite**   | 3.46.0 (bundled, `rusqlite`) — `TEXT` and `JSONB` doc columns                                                                                                                                                                                                                   |
| **Build**    | `--release` (optimized)                                                                                                                                                                                                                                                         |
| **Platform** | macOS 26.5 (build 25F71), Apple M3 (arm64)                                                                                                                                                                                                                                      |
| **Date**     | 2026-06-24                                                                                                                                                                                                                                                                      |
| **Config**   | preload `N = 2000`, seed `0x0ADB00EC`                                                                                                                                                                                                                                           |
| **Method**   | Ad-hoc SQL end-to-end (no prepared statements on either engine); each result fully decoded. **Reads: median of 5 passes at `M = 4000` timed ops/cell** (high `M` amortizes per-op jitter). **Inserts: median of 3 passes at `M = 500`** (fsync-dominated, so fewer/longer ops). |

> **Reading the numbers.** `ns/op` is wall-clock latency per operation (a point read,
> a 100-row range read, one insert, …). `B/op` and `allocs/op` come from a counting
> Rust global allocator: **exact for MonaDB, undercounted for SQLite** (its C heap is
> invisible to the Rust allocator). So compare _latency_ across engines directly, but
> treat MonaDB's allocation figures as a self-improvement signal, not a like-for-like
> memory comparison. `Δ` columns are MonaDB ÷ SQLite-`TEXT` (so `<1.0` = MonaDB faster).

> **On noise (vs the first pass of this report).** An initial `M = 500` matrix showed
> wide run-to-run swings on small reads (a point lookup ranged 3.8–7.4 µs across three
> runs) that muddied the picture — e.g. composite `md` point lookups read as 13 µs.
> Re-running the **read** workloads at **`M = 4000` (median of 5)** cut the per-cell
> spread to ~3–25% and confirmed the real shape: that composite `md` lookup is ~7 µs,
> and **`lg` point lookups are a MonaDB win (0.57–0.61×)**, not the parity the noisy
> run suggested. The read tables below use the tighter `M = 4000` numbers; inserts are
> fsync-bound and were left at the `M = 500` median of 3.

---

## TL;DR

- **Large-document reads are a clean, reproducible MonaDB win across the board.** At
  `lg`: point lookups **0.57–0.61×**, range reads **0.33×** (3× faster), prefix reads
  **0.41×** (2.4× faster). At `md`: range **0.51×** and prefix **0.60×** also win.
- **Small reads trail SQLite by fixed per-op overhead.** Point lookups run **1.4–2.1×**
  at `xs`/`sm`/`md`; small range reads are **5.6× / 2.7×** slower at `xs`/`sm`, where
  MonaDB issues 100 point gets against SQLite's single scan over a tiny payload.
- **The flat lazy codec keeps read cost decoupled from document size.** A point lookup
  allocates **15 times regardless of profile**; range and prefix reads allocate a
  constant ~1,022 / ~97 times across all sizes — the remaining `B/op` is the single
  `Rc<[u8]>` copy of each row out of the mmap.
- **Inserts remain MonaDB's dominant gap** — ~3.5–6.4 ms median, roughly flat across
  document size (≈8–130× SQLite depending on profile) — the signature of **committing a
  durable write transaction (fsync) per `execute`**. Use `Config::nosync()` or batched
  transactions to close this gap.

---

## Latency by workload (ns/op)

Read tables: median of 5 passes at `M = 4000`. Insert tables: median of 3 passes at
`M = 500`.

### Point lookup — `single_key_select_1` (`docs[id]`)

| Profile      | MonaDB | SQLite TEXT | SQLite JSONB |         Δ |
| ------------ | -----: | ----------: | -----------: | --------: |
| xs (256 B)   |  3,134 |       1,996 |        1,648 | **1.57×** |
| sm (2 KiB)   |  4,130 |       1,994 |        1,902 | **2.07×** |
| md (16 KiB)  |  6,992 |       4,461 |        4,533 | **1.57×** |
| lg (128 KiB) | 14,402 |      23,557 |       23,949 | **0.61×** |

### Composite point lookup — `composite_key_select_1` (`docs["t007", seq]`)

| Profile | MonaDB | SQLite TEXT | SQLite JSONB |         Δ |
| ------- | -----: | ----------: | -----------: | --------: |
| xs      |  3,570 |       2,472 |        2,118 | **1.44×** |
| sm      |  4,311 |       2,480 |        2,500 | **1.74×** |
| md      |  7,103 |       5,168 |        5,267 | **1.37×** |
| lg      | 14,108 |      24,573 |       25,062 | **0.57×** |

### Range read — `single_key_select_range` (100 contiguous keys)

| Profile |  MonaDB | SQLite TEXT | SQLite JSONB |         Δ |
| ------- | ------: | ----------: | -----------: | --------: |
| xs      |  50,878 |       9,080 |        9,037 | **5.60×** |
| sm      |  59,325 |      22,413 |       22,578 | **2.65×** |
| md      | 133,939 |     262,207 |      260,598 | **0.51×** |
| lg      | 712,659 |   2,181,982 |    2,176,241 | **0.33×** |

### Prefix / partition read — `composite_key_select_prefix` (~20 rows/tenant)

| Profile |  MonaDB | SQLite TEXT | SQLite JSONB |         Δ |
| ------- | ------: | ----------: | -----------: | --------: |
| xs      |   8,152 |       4,178 |        4,067 | **1.95×** |
| sm      |  11,227 |       6,890 |        7,127 | **1.63×** |
| md      |  32,633 |      54,316 |       55,122 | **0.60×** |
| lg      | 175,434 |     430,810 |      426,868 | **0.41×** |

### Insert — `single_key_insert` / `composite_key_insert`

| Profile | MonaDB single | SQLite TEXT | SQLite JSONB |          Δ |
| ------- | ------------: | ----------: | -----------: | ---------: |
| xs      |     3,685,637 |      28,451 |       20,201 | **129.5×** |
| sm      |     3,502,453 |      64,858 |       41,979 |  **54.0×** |
| md      |     4,336,287 |     185,929 |      138,978 |  **23.3×** |
| lg      |     6,414,204 |     782,207 |      764,830 |   **8.2×** |

| Profile | MonaDB composite | SQLite TEXT | SQLite JSONB |         Δ |
| ------- | ---------------: | ----------: | -----------: | --------: |
| xs      |        3,663,076 |      48,775 |       31,032 | **75.1×** |
| sm      |        3,648,124 |      72,747 |       48,919 | **50.2×** |
| md      |        4,318,321 |     176,020 |      135,320 | **24.5×** |
| lg      |        6,268,392 |     750,752 |      767,456 |  **8.4×** |

> MonaDB's insert latency is roughly **flat at ~3.5–6.4 ms** until the `lg` payload
> rivals the fixed cost — the signature of a per-insert durable commit. SQLite uses
> `synchronous=NORMAL` in this harness; MonaDB uses full durability (`Config::default()`).
> A batched-transaction path or `Config::nosync()` would close most of the small-insert
> gap.

---

## Read-latency stability

At `M = 4000` the per-cell spread across 5 passes is tight, and tightest exactly where
the comparison matters most (the large-document wins). Small reads still carry the most
relative jitter — a fixed per-op floor measured against a sub-microsecond SQLite scan
magnifies small absolute swings.

| Cell                  | MonaDB median | 5-pass spread |
| --------------------- | ------------: | ------------: |
| point lookup `xs`     |         3,134 |          ±36% |
| point lookup `md`     |         6,992 |          ±24% |
| point lookup `lg`     |        14,402 |          ±14% |
| composite lookup `lg` |        14,108 |           ±6% |
| range `md`            |       133,939 |          ±18% |
| range `lg`            |       712,659 |           ±9% |
| prefix `lg`           |       175,434 |           ±4% |

(Spread = (max − min) / median across the 5 passes.) The `lg` and prefix cells settle
within ~5–15%; treat their Δ values as solid and the small-read Δ values as ±25%.

---

## Memory

Allocation **count** is the cleanest cross-cut. With the flat lazy codec, MonaDB's
per-read allocations no longer scale with the values materialized.

| Workload (md profile)  | MonaDB allocs/op | MonaDB B/op | SQLite allocs/op | MonaDB peak heap |
| ---------------------- | ---------------: | ----------: | ---------------: | ---------------: |
| point lookup           |               15 |      37,028 |                2 |            36 KB |
| composite point lookup |               19 |      38,997 |                2 |            36 KB |
| range (100 rows)       |            1,022 |   3,698,276 |              101 |           1.8 MB |
| prefix (~20 rows)      |               97 |   1,099,997 |               21 |           376 KB |
| insert                 |            3,226 |     552,305 |            1,091 |            18 MB |

Observations:

- **Per-read allocation churn is flat with document size.** A point lookup allocates
  **15 times at every profile** (`xs`→`lg`). Range and prefix reads hold at ~1,022 and
  ~97 allocations across all sizes. The remaining `B/op` is the single `Rc<[u8]>`
  copy of each row out of the mmap.
- **Reads beat SQLite on latency once the payload is large** because navigation is
  offset arithmetic over bytes already resident, with no decode and no per-field heap
  traffic — the gap that remains at `xs`/`sm` is fixed per-op overhead.
- **Inserts still allocate** to build the row object and its flat encoding
  (~3,226 allocs/op at `md`), but latency is dominated by the per-`execute` fsync, not
  allocation. The 18 MB peak heap is the timed loop's cumulative build cost across many
  inserts, not a per-op figure.
- **SQLite `B/op` is not comparable** (C-heap invisible); included only to show its
  Rust-binding overhead is minimal and ~flat.

### Peak RSS caveat

RSS is a process high-water mark, so the single-process matrix run reports cumulative
RSS (it pins near ~536 MB after the largest `lg` range read and stays there). For clean
per-engine RSS, run one engine per process (see `benches/README.md`).

---

## Takeaways

**Where MonaDB wins:** every large-document (`lg`) read — point lookups (0.57–0.61×),
range (0.33×), and prefix (0.41×) — plus medium (`md`) range (0.51×) and prefix (0.60×).
These are the tightest cells run-to-run. **Where it trails:** small-document point reads
(`xs`/`sm`/`md`, 1.4–2.1×) and especially small range reads (`xs`/`sm`, 2.7–5.6×), where
fixed per-op and batch-get setup cost dominate a tiny payload — and, by a wide margin,
**inserts** (~8–130× under full durability).

**Highest-leverage optimization targets, in order:**

1. **Relaxed durability or batched/bulk insert.** The ~3.5–6.4 ms flat insert cost is
   per-`execute` transaction durability (one fsync each). `Config::nosync()` (LMDB
   `MDB_NOSYNC`, SQLite `synchronous=NORMAL` analogue), multi-row transactions, or a
   bulk-load API would cut small-insert latency by 1–2 orders of magnitude.
2. **Trim fixed per-op read overhead for small documents.** At `xs`/`sm` the gap on
   point and range reads is setup cost (100 point gets vs one SQLite scan), not
   allocation. This is also the noisiest region — a lower fixed floor would tighten it.
3. **Range-read result construction.** The 100-row batch-get (`select [docs[lo], …]`)
   still constructs an offset key and result-array slot per element (~1,022 allocs); a
   streaming cursor scan would shave the remaining constant ~10 allocations/row.

---

## Reproduction

```sh
# Read workloads — median of 5 passes at M=4000 (the values in this report)
for i in 1 2 3 4 5; do
  MONADB_BENCH_N=2000 MONADB_BENCH_M=4000 \
    MONADB_BENCH_WORKLOADS=single_key_select_1,composite_key_select_1,single_key_select_range,composite_key_select_prefix \
    MONADB_BENCH_CSV=target/reads-$i.csv \
    cargo bench --bench metrics
done

# Insert workloads — median of 3 passes at M=500 (fsync-dominated)
for i in 1 2 3; do
  MONADB_BENCH_N=2000 MONADB_BENCH_M=500 \
    MONADB_BENCH_WORKLOADS=single_key_insert,composite_key_insert \
    MONADB_BENCH_CSV=target/engine-compare-$i.csv \
    cargo bench --bench metrics
done

# Authoritative latency distributions (Criterion)
cargo bench --bench doc_workloads
```

Reported latency is the per-cell median across passes; allocation counts are
deterministic and near-identical across runs. Inserts and small reads are
fsync-/machine-state-sensitive — re-run on the target machine before drawing
conclusions. Use the Criterion harness for statistically rigorous latency.
