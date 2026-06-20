# MonaDB vs SQLite — Document Workload Benchmark Report

Generated from the `metrics` harness (`cargo bench --bench metrics`).

| | |
|---|---|
| **MonaDB** | 0.1.0 (LMDB / heed backend) |
| **SQLite** | 3.46.0 (bundled, `rusqlite`) — `TEXT` and `JSONB` doc columns |
| **Build** | `--release` (optimized) |
| **Platform** | macOS (darwin), arm64 |
| **Date** | 2026-06-19 |
| **Config** | preload `N = 2000`, timed ops `M = 500`, seed `0x0ADB00EC` |
| **Method** | Ad-hoc SQL end-to-end (no prepared statements on either engine); each result fully decoded |

> **Reading the numbers.** `ns/op` is wall-clock latency per operation (a point read,
> a 100-row range read, one insert, …). `B/op` and `allocs/op` come from a counting
> Rust global allocator: **exact for MonaDB, undercounted for SQLite** (its C heap is
> invisible to the Rust allocator). So compare *latency* across engines directly, but
> treat MonaDB's allocation figures as a self-improvement signal, not a like-for-like
> memory comparison. `Δ` columns are MonaDB ÷ SQLite-`TEXT`.

---

## TL;DR

- **SQLite is faster on every workload at this scale**, by ~7× on point lookups up to
  ~100× on small single-row inserts. The gap **shrinks as documents grow** (to ~6–8×)
  because large-payload serialization cost dominates and converges across engines.
- **Inserts are MonaDB's biggest gap.** MonaDB spends ~3.7–5 ms per insert almost
  regardless of document size — consistent with **committing a write transaction
  (fsync) per `execute`**. SQLite (WAL + `synchronous=NORMAL`) is 1–2 orders of
  magnitude cheaper on small docs.
- **MonaDB's allocation count scales with rows/values touched**; SQLite stays near
  constant (≈2 allocations per point read vs MonaDB's 84–2,800+). This is the clearest,
  most actionable optimization target.
- **Range/prefix reads amplify both effects** — MonaDB's batch-get array construction
  allocates heavily (up to ~105 MB of churn for a 100-row `lg` range read).

---

## Latency by workload (ns/op)

### Point lookup — `single_key_select_1` (`docs[id]`)

| Profile | MonaDB | SQLite TEXT | SQLite JSONB | Δ |
|---|--:|--:|--:|--:|
| xs (256 B) | 15,400 | 2,143 | 1,738 | **7.2×** |
| sm (2 KiB) | 22,453 | 2,076 | 1,914 | **10.8×** |
| md (16 KiB) | 90,473 | 4,516 | 4,589 | **20.0×** |
| lg (128 KiB) | 178,736 | 23,552 | 23,585 | **7.6×** |

### Composite point lookup — `composite_key_select_1` (`docs["t007", seq]`)

| Profile | MonaDB | SQLite TEXT | SQLite JSONB | Δ |
|---|--:|--:|--:|--:|
| xs | 18,838 | 2,749 | 2,158 | **6.9×** |
| sm | 28,725 | 2,761 | 2,489 | **10.4×** |
| md | 87,163 | 5,126 | 5,391 | **17.0×** |
| lg | 179,059 | 23,601 | 25,076 | **7.6×** |

### Range read — `single_key_select_range` (100 contiguous keys)

| Profile | MonaDB | SQLite TEXT | SQLite JSONB | Δ |
|---|--:|--:|--:|--:|
| xs | 361,048 | 9,194 | 9,646 | **39.3×** |
| sm | 704,962 | 22,201 | 22,369 | **31.8×** |
| md | 5,086,700 | 261,545 | 260,724 | **19.4×** |
| lg | 16,839,432 | 2,101,046 | 2,141,139 | **8.0×** |

### Prefix / partition read — `composite_key_select_prefix` (~20 rows/tenant)

| Profile | MonaDB | SQLite TEXT | SQLite JSONB | Δ |
|---|--:|--:|--:|--:|
| xs | 78,542 | 4,155 | 3,928 | **18.9×** |
| sm | 156,249 | 6,824 | 6,725 | **22.9×** |
| md | 964,690 | 54,043 | 53,432 | **17.9×** |
| lg | 3,372,940 | 419,536 | 421,926 | **8.0×** |

### Insert — `single_key_insert` / `composite_key_insert`

| Profile | MonaDB single | SQLite TEXT | SQLite JSONB | Δ |
|---|--:|--:|--:|--:|
| xs | 3,735,104 | 37,284 | 22,344 | **100×** |
| sm | 3,623,364 | 65,602 | 42,017 | **55×** |
| md | 3,972,083 | 188,283 | 134,535 | **21×** |
| lg | 4,741,833 | 727,995 | 766,344 | **6.5×** |

| Profile | MonaDB composite | SQLite TEXT | SQLite JSONB | Δ |
|---|--:|--:|--:|--:|
| xs | 3,868,180 | 49,219 | 32,965 | **79×** |
| sm | 3,674,055 | 83,008 | 51,542 | **44×** |
| md | 4,298,223 | 187,950 | 135,773 | **23×** |
| lg | 4,971,766 | 725,914 | 763,282 | **6.8×** |

> MonaDB's insert latency is roughly **flat at ~3.7–5 ms** until the `lg` payload
> finally rivals the fixed cost — the signature of a per-insert durable commit. A
> batched-transaction / bulk-load path would close most of this gap; `TEXT` vs `JSONB`
> shows SQLite's `json()` parse adds cost on writes but is free-to-cheaper on reads.

---

## Memory

Allocation **count** is the cleanest cross-cut: SQLite's Rust surface is ~constant
per operation while MonaDB allocates proportionally to the values it materializes.

| Workload (md profile) | MonaDB allocs/op | MonaDB B/op | SQLite allocs/op | MonaDB peak heap |
|---|--:|--:|--:|--:|
| point lookup | 1,091 | 203,158 | 2 | 125 KB |
| range (100 rows) | 108,036 | 19,901,777 | 101 | 3.3 MB |
| prefix (~20 rows) | 21,233 | 4,217,869 | 21 | 745 KB |
| insert | 3,596 | 600,648 | 1,091 | 168 KB |

Observations:

- **Per-value allocation churn dominates MonaDB's reads.** A 100-row `md` range read
  allocates ~108K times (~1,080 allocations *per returned row*) and churns ~20 MB.
  SQLite issues ~101 allocations for the same query. Reducing per-`Value`/per-row
  allocations (arena reuse, borrowed decode, fewer `Rc` heap nodes) is the highest-leverage
  memory win.
- **MonaDB peak heap tracks payload size** (KB for points, MB for ranges) — expected,
  but the multiplier over the raw document bytes is large for multi-row reads.
- **SQLite `B/op` is not comparable** (C-heap invisible); included only to show its
  Rust-binding overhead is minimal and ~flat.

### Peak RSS caveat

RSS is a process high-water mark, so the single-process matrix run reports cumulative
RSS (it pins near ~407 MB after the largest `lg` range read and stays there). The early
cells are still indicative — e.g. point-lookup RSS grew xs→md as 6 MB → 16 MB → 49 MB
(SQLite) vs 6 MB → 12 MB → 33 MB (MonaDB). For clean per-engine RSS, run one engine per
process (see `benches/README.md`).

---

## Takeaways

**Where MonaDB is closest:** large-document (`lg`) reads — ~7–8× — where serialization
cost dominates and the engines converge. **Where it's furthest:** small-document inserts
(~80–100×) and small-row-count range reads (~30–40×).

**Highest-leverage optimization targets, in order:**

1. **Batched/bulk insert path.** The ~3.7–5 ms flat insert cost is per-`execute`
   transaction durability. A multi-row transaction or bulk-load API would likely cut
   small-insert latency by 1–2 orders of magnitude.
2. **Cut per-`Value` allocations on reads.** Allocation count scales ~linearly with
   rows×fields. Arena/borrowed decoding and fewer `Rc` nodes would help both latency and
   memory, most visibly on range/prefix reads.
3. **Range-read materialization.** The `select [docs[lo], …]` batch-get builds a large
   array expression; a streaming cursor scan would avoid constructing (and allocating)
   the whole result array.

---

## Reproduction

```sh
# Full matrix used for this report
MONADB_BENCH_N=2000 MONADB_BENCH_M=500 \
  MONADB_BENCH_CSV=target/engine-compare.csv \
  cargo bench --bench metrics

# Authoritative latency distributions (Criterion)
cargo bench --bench doc_workloads
```

Raw data: `target/engine-compare.csv` (72 cells = 6 workloads × 4 profiles × 3 engines).
All figures are single-sample; use the Criterion harness for statistically rigorous
latency. Numbers are hardware-, build-, and scale-dependent — re-run on the target
machine before drawing conclusions.
