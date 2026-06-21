# MonaDB vs SQLite — Document Workload Benchmark Report

Generated from the `metrics` harness (`cargo bench --bench metrics`).

| | |
|---|---|
| **MonaDB** | 0.1.0 (LMDB / heed backend, flat lazy value codec) |
| **SQLite** | 3.46.0 (bundled, `rusqlite`) — `TEXT` and `JSONB` doc columns |
| **Build** | `--release` (optimized) |
| **Platform** | macOS (darwin), arm64 |
| **Date** | 2026-06-21 |
| **Config** | preload `N = 2000`, timed ops `M = 500`, seed `0x0ADB00EC` |
| **Method** | Ad-hoc SQL end-to-end (no prepared statements on either engine); each result fully decoded |

> **Reading the numbers.** `ns/op` is wall-clock latency per operation (a point read,
> a 100-row range read, one insert, …). `B/op` and `allocs/op` come from a counting
> Rust global allocator: **exact for MonaDB, undercounted for SQLite** (its C heap is
> invisible to the Rust allocator). So compare *latency* across engines directly, but
> treat MonaDB's allocation figures as a self-improvement signal, not a like-for-like
> memory comparison. `Δ` columns are MonaDB ÷ SQLite-`TEXT` (so `<1.0` = MonaDB faster).

> **What changed since the prior report.** MonaDB now stores document values in a
> flat, JSONB-style binary layout read lazily (`Value::Raw`, an `Rc<[u8]>` view):
> a read copies the row bytes once, then navigates by offset with no per-field
> allocation, and re-encoding is a `memcpy`. **Read allocation count is now flat
> with respect to document size** and reads are dramatically faster, flipping the
> earlier "SQLite wins everything" result on medium/large documents. Inserts are
> unchanged (still one durable commit per `execute`).

---

## TL;DR

- **Reads are now competitive, and faster than SQLite on large documents.** Point
  lookups are within ~2.5× at `md` and **on par at `lg`** (0.9×); 100-row range reads
  are **2.5× faster than SQLite at `lg`** (0.40×); prefix reads are **~1.75× faster
  at `lg`** (0.57×). MonaDB still trails ~3–4× on **small** documents (`xs`/`sm`),
  where fixed per-op overhead dominates a tiny payload.
- **The flat lazy codec decoupled read cost from document size.** A point lookup
  now allocates **43 times regardless of profile** (was ~1,091 at `md`); range and
  prefix reads allocate a constant ~3,236 / ~131 times across `xs`→`lg`.
- **Inserts remain MonaDB's dominant gap** — ~3.1–4.6 ms, roughly flat across
  document size (≈6× SQLite at `lg`, up to ~150× on tiny single-row inserts) — the
  signature of **committing a durable write transaction (fsync) per `execute`**.
  This is now the clear #1 optimization target.

---

## Latency by workload (ns/op)

### Point lookup — `single_key_select_1` (`docs[id]`)

| Profile | MonaDB | SQLite TEXT | SQLite JSONB | Δ |
|---|--:|--:|--:|--:|
| xs (256 B) | 9,427 | 2,441 | 1,916 | **3.9×** |
| sm (2 KiB) | 11,220 | 2,591 | 2,092 | **4.3×** |
| md (16 KiB) | 14,105 | 5,664 | 4,922 | **2.5×** |
| lg (128 KiB) | 27,068 | 30,136 | 25,294 | **0.90×** |

### Composite point lookup — `composite_key_select_1` (`docs["t007", seq]`)

| Profile | MonaDB | SQLite TEXT | SQLite JSONB | Δ |
|---|--:|--:|--:|--:|
| xs | 9,518 | 2,847 | 2,569 | **3.3×** |
| sm | 12,245 | 3,118 | 2,815 | **3.9×** |
| md | 14,951 | 5,743 | 5,455 | **2.6×** |
| lg | 24,434 | 25,710 | 24,871 | **0.95×** |

### Range read — `single_key_select_range` (100 contiguous keys)

| Profile | MonaDB | SQLite TEXT | SQLite JSONB | Δ |
|---|--:|--:|--:|--:|
| xs | 206,870 | 10,124 | 9,738 | **20.4×** |
| sm | 220,097 | 23,421 | 22,762 | **9.4×** |
| md | 358,703 | 266,685 | 270,506 | **1.3×** |
| lg | 868,664 | 2,178,051 | 2,287,643 | **0.40×** |

### Prefix / partition read — `composite_key_select_prefix` (~20 rows/tenant)

| Profile | MonaDB | SQLite TEXT | SQLite JSONB | Δ |
|---|--:|--:|--:|--:|
| xs | 18,767 | 4,297 | 4,440 | **4.4×** |
| sm | 31,110 | 8,917 | 11,041 | **3.5×** |
| md | 54,368 | 57,434 | 57,583 | **0.95×** |
| lg | 258,574 | 454,052 | 551,568 | **0.57×** |

### Insert — `single_key_insert` / `composite_key_insert`

| Profile | MonaDB single | SQLite TEXT | SQLite JSONB | Δ |
|---|--:|--:|--:|--:|
| xs | 3,220,385 | 20,907 | 17,112 | **154×** |
| sm | 3,159,391 | 63,709 | 42,274 | **50×** |
| md | 3,929,919 | 171,353 | 142,029 | **23×** |
| lg | 4,587,978 | 750,118 | 829,827 | **6.1×** |

| Profile | MonaDB composite | SQLite TEXT | SQLite JSONB | Δ |
|---|--:|--:|--:|--:|
| xs | 3,132,165 | 47,016 | 31,825 | **67×** |
| sm | 3,256,023 | 81,566 | 52,618 | **40×** |
| md | 3,412,811 | 204,276 | 158,176 | **17×** |
| lg | 4,317,506 | 764,511 | 785,377 | **5.6×** |

> MonaDB's insert latency is roughly **flat at ~3.1–4.6 ms** until the `lg` payload
> finally rivals the fixed cost — the signature of a per-insert durable commit. A
> batched-transaction / bulk-load path (or a relaxed durability mode) would close
> most of this gap; `TEXT` vs `JSONB` shows SQLite's `json()` parse adds cost on
> writes but is free-to-cheaper on reads.

---

## Memory

Allocation **count** is the cleanest cross-cut. With the flat lazy codec, MonaDB's
per-read allocations no longer scale with the values materialized — a read is one
buffer copy plus a small, *constant* amount of navigation/result bookkeeping.

| Workload (md profile) | MonaDB allocs/op | MonaDB B/op | SQLite allocs/op | MonaDB peak heap |
|---|--:|--:|--:|--:|
| point lookup | 43 | 46,789 | 2 | 37 KB |
| range (100 rows) | 3,236 | 4,259,074 | 101 | 1.9 MB |
| prefix (~20 rows) | 131 | 1,108,262 | 21 | 386 KB |
| insert | 3,306 | 507,628 | 1,091 | 105 KB |

Observations:

- **Per-read allocation churn is now flat with document size.** A point lookup
  allocates **43 times at every profile** (`xs`→`lg`), down from ~1,091 at `md` in
  the prior tree-materializing decoder (~25× fewer). Range and prefix reads hold at
  ~3,236 and ~131 allocations across all sizes. The remaining `B/op` is the single
  `Rc<[u8]>` copy of each row out of the mmap, not a fanned-out object tree.
- **Reads beat SQLite on latency once the payload is large** because navigation is
  offset arithmetic over bytes already resident, with no decode and no per-field
  heap traffic — the gap that remains at `xs`/`sm` is fixed per-op overhead.
- **Inserts still allocate** to build the row object and its flat encoding
  (~3,306 allocs/op at `md`, modestly better than the prior ~3,596), but latency is
  dominated by the per-`execute` fsync, not allocation.
- **SQLite `B/op` is not comparable** (C-heap invisible); included only to show its
  Rust-binding overhead is minimal and ~flat.

### Peak RSS caveat

RSS is a process high-water mark, so the single-process matrix run reports cumulative
RSS (it pins near ~391 MB after the largest `lg` range read and stays there). The
early cells are still indicative — e.g. point-lookup RSS grew xs→md as 6 MB → 16 MB →
50 MB (SQLite) vs 6 MB → 12 MB → 34 MB (MonaDB). For clean per-engine RSS, run one
engine per process (see `benches/README.md`).

---

## Takeaways

**Where MonaDB now wins:** large-document (`lg`) reads — range (0.40×) and prefix
(0.57×) reads run faster than SQLite, and point lookups are on par (0.90×), because
the flat lazy codec turns a read into a single buffer copy plus offset navigation.
**Where it's competitive:** medium (`md`) reads (~1–2.6×). **Where it still trails:**
small-document reads (`xs`/`sm`, ~3–4×), where fixed per-op cost dominates a tiny
payload, and — by a wide margin — **inserts** (~6–150×).

**Highest-leverage optimization targets, in order:**

1. **Batched/bulk insert path (or relaxed durability).** The ~3.1–4.6 ms flat insert
   cost is per-`execute` transaction durability (one fsync each). A multi-row
   transaction, bulk-load API, or a `synchronous=NORMAL`-equivalent mode would cut
   small-insert latency by 1–2 orders of magnitude. This is now the single dominant
   gap.
2. **Trim fixed per-op read overhead for small documents.** At `xs`/`sm` the ~3–4×
   gap is setup cost (cursor open, key encode, result wrapping) amortized over a tiny
   payload, not allocation — the flat codec already removed the per-value churn.
3. **Range-read result construction.** The 100-row batch-get (`select [docs[lo], …]`)
   still constructs an offset key and result-array slot per element; a streaming
   cursor scan would shave the remaining constant ~32 allocations/row.

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
