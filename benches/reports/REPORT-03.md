# MonaDB vs SQLite — Document Workload Benchmark Report

Generated from the `metrics` harness (`cargo bench --bench metrics`).

|              |                                                                                                                                                                                                                                                                                                                                          |
| ------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **MonaDB**   | 0.1.0 (LMDB / heed backend, flat lazy value codec) — **`Config::nosync()` for writes**                                                                                                                                                                                                                                                   |
| **SQLite**   | 3.46.0 (bundled, `rusqlite`) — **JSONB** `doc BLOB` column, `synchronous=NORMAL`                                                                                                                                                                                                                                                         |
| **Build**    | `--release` (optimized)                                                                                                                                                                                                                                                                                                                  |
| **Platform** | macOS 26.5 (build 25F71), Apple M3 (arm64)                                                                                                                                                                                                                                                                                               |
| **Date**     | 2026-06-24                                                                                                                                                                                                                                                                                                                               |
| **Config**   | preload `N = 2000`, seed `0x0ADB00EC`                                                                                                                                                                                                                                                                                                    |
| **Method**   | **Prepared statements with bound parameters on both engines** — the parse-free steady-state hot path. Each result fully decoded. **Reads: median of 5 passes at `M = 4000` timed ops/cell.** **Inserts: median of 3 passes at `M = 500`.** Both engines under **relaxed durability** (MonaDB `MDB_NOSYNC`, SQLite `synchronous=NORMAL`). |

> **What changed since [REPORT-02](REPORT-02.md): the methodology, not just the build.**
> This is the first **apples-to-apples** comparison. Three changes:
> **(1) Both engines now use prepared statements** with bound parameters — MonaDB via
> `prepare_cached` (no `normalize`), SQLite via connection-cached `prepare_cached`.
> REPORT-02 ran MonaDB ad-hoc `query` (paying `normalize` per call) against SQLite
> `query_row` (re-parsed per call). **(2) The engine matrix collapsed to two** —
> `monadb` and `sqlite` — dropping the separate SQLite TEXT/JSONB split.
> **(3) SQLite stores documents as its native JSONB binary type** (`jsonb(?)` into a
> `BLOB` column), not TEXT JSON.
>
> The honest effects: SQLite's **reads got faster** once prepared (its `xs` point lookup
> dropped ~40%, from 1,659 ns ad-hoc TEXT in REPORT-02 to 990 ns prepared JSONB), which
> **narrows MonaDB's small-document read win and flips small prefix reads to a loss**. MonaDB's **inserts got dramatically faster** — the
> prepared object-param path skips re-parsing a large object-literal SQL string — while
> SQLite's inserts got slower (`jsonb()` transcodes JSON text to binary per row). See
> the **Honesty notes** at the end for the one input-side asymmetry this introduces.

> **Reading the numbers.** `ns/op` is wall-clock latency per operation (a point read, a
> 100-row range read, one insert, …). `B/op` and `allocs/op` come from a counting Rust
> global allocator: **exact for MonaDB, undercounted for SQLite** (its C heap is invisible
> to the Rust allocator). So compare _latency_ across engines directly, but treat MonaDB's
> allocation figures as a self-improvement signal, not a like-for-like memory comparison.
> `Δ` columns are MonaDB ÷ SQLite (so `<1.0` = MonaDB faster).

---

## TL;DR

- **Point lookups stay a MonaDB win at every size, but the margin is honest now.**
  `0.95–0.52×` (single) and `0.82–0.48×` (composite). At `xs` the single-key lookup is
  effectively **parity** (`0.95×`, and noisy — see stability), because SQLite's prepared
  point lookup is ~990 ns. The win grows with document size (`lg` `0.52×`/`0.48×`), where
  MonaDB's flat lazy codec avoids re-serializing the payload.
- **Range reads: large-document win, small-document loss** — `md` `0.50×`, `lg` `0.35×`,
  but `xs` `3.74×` and `sm` `2.47×`. MonaDB's contiguous span is 100 point gets vs
  SQLite's single index scan; that overhead only amortizes once payloads are large.
- **Prefix reads flipped at small sizes.** Now `xs` `1.35×` / `sm` `1.23×` (a **loss**,
  vs REPORT-02's win) but still `md` `0.60×` / `lg` `0.39×`. SQLite's prepared prefix scan
  over a tiny payload is hard to beat; the win returns once per-row decode dominates.
- **Inserts are now a MonaDB win across the board** — single `0.45–0.79×`, composite
  `0.47–0.64×`. Two drivers, **one of them an input-side asymmetry** (see Honesty notes):
  MonaDB binds a pre-built object value (skipping document parsing), while SQLite's
  `jsonb(?)` parses JSON text into binary on every row.
- **The flat lazy codec keeps read cost decoupled from document size**, and the prepared
  path shaved allocations further: a point lookup now allocates **9/op** (down from 13 —
  no `normalize`), and inserts dropped from ~3,228 to **~1,414 allocs/op** (no giant
  object-literal SQL to lex).

---

## Latency by workload (ns/op)

Read tables: median of 5 passes at `M = 4000`. Insert tables: median of 3 passes at
`M = 500`. Both engines prepared, under relaxed durability.

### Point lookup — `single_key_select_1` (`docs[?]`)

| Profile      | MonaDB | SQLite |         Δ |
| ------------ | -----: | -----: | --------: |
| xs (256 B)   |    943 |    990 | **0.95×** |
| sm (2 KiB)   |  1,021 |  1,187 | **0.86×** |
| md (16 KiB)  |  2,414 |  3,415 | **0.71×** |
| lg (128 KiB) | 10,422 | 19,943 | **0.52×** |

### Composite point lookup — `composite_key_select_1` (`docs[?, ?]`)

| Profile      | MonaDB | SQLite |         Δ |
| ------------ | -----: | -----: | --------: |
| xs (256 B)   |  1,004 |  1,229 | **0.82×** |
| sm (2 KiB)   |  1,121 |  1,442 | **0.78×** |
| md (16 KiB)  |  2,519 |  3,831 | **0.66×** |
| lg (128 KiB) |  9,609 | 20,225 | **0.48×** |

### Range read — `single_key_select_range` (100 contiguous keys)

| Profile      |  MonaDB |    SQLite |         Δ |
| ------------ | ------: | --------: | --------: |
| xs (256 B)   |  26,718 |     7,135 | **3.74×** |
| sm (2 KiB)   |  36,015 |    14,559 | **2.47×** |
| md (16 KiB)  | 111,214 |   221,087 | **0.50×** |
| lg (128 KiB) | 647,756 | 1,849,973 | **0.35×** |

### Prefix / partition read — `composite_key_select_prefix` (~20 rows/tenant)

| Profile      |  MonaDB |  SQLite |         Δ |
| ------------ | ------: | ------: | --------: |
| xs (256 B)   |   3,646 |   2,709 | **1.35×** |
| sm (2 KiB)   |   5,507 |   4,482 | **1.23×** |
| md (16 KiB)  |  27,324 |  45,356 | **0.60×** |
| lg (128 KiB) | 147,100 | 379,824 | **0.39×** |

### Insert — `single_key_insert` (`insert into docs ($1)` vs `VALUES (?, jsonb(?))`)

| Profile      |  MonaDB |  SQLite |         Δ |
| ------------ | ------: | ------: | --------: |
| xs (256 B)   |   7,758 |   9,810 | **0.79×** |
| sm (2 KiB)   |  13,605 |  30,258 | **0.45×** |
| md (16 KiB)  |  74,164 | 109,630 | **0.68×** |
| lg (128 KiB) | 299,698 | 627,151 | **0.48×** |

### Insert — `composite_key_insert`

| Profile      |  MonaDB |  SQLite |         Δ |
| ------------ | ------: | ------: | --------: |
| xs (256 B)   |   8,115 |  17,317 | **0.47×** |
| sm (2 KiB)   |  14,468 |  27,939 | **0.52×** |
| md (16 KiB)  |  74,022 | 115,375 | **0.64×** |
| lg (128 KiB) | 300,705 | 646,339 | **0.47×** |

> The insert win is real but read it with the **Honesty notes** below: MonaDB's
> prepared path receives the document already as a structured object value and skips
> document parsing entirely, whereas SQLite receives JSON **text** and `jsonb()` parses
> it into binary on every row. Both are each engine's idiomatic prepared insert, but the
> input forms differ — so the insert Δ is not strictly like-for-like on the encode side.

---

## Read-latency stability

At `M = 4000` the per-cell spread across 5 passes is tight, and tightest where absolute
latency is largest. The loose cell is the `xs` single point lookup — a sub-microsecond
operation where small absolute jitter reads as a large percentage.

| Cell                  | MonaDB median | 5-pass spread |
| --------------------- | ------------: | ------------: |
| point lookup `xs`     |           943 |          ±58% |
| point lookup `md`     |         2,414 |           ±8% |
| point lookup `lg`     |        10,422 |           ±9% |
| composite lookup `xs` |         1,004 |           ±6% |
| composite lookup `lg` |         9,609 |           ±4% |
| range `md`            |       111,214 |           ±3% |
| range `lg`            |       647,756 |           ±3% |
| prefix `lg`           |       147,100 |          ±11% |

(Spread = (max − min) / median across the 5 passes.) Most cells settle within ~10%.
**Treat the `xs` single point-lookup Δ (`0.95×`) as parity ±~25%**, not a measured win.

---

## Memory

Allocation **count** is the cleanest cross-cut. With the flat lazy codec, MonaDB's
per-read allocations don't scale with the values materialized; the prepared path removes
the `normalize` allocations that REPORT-02 paid on every ad-hoc call.

| Workload (md profile)  | MonaDB allocs/op | MonaDB B/op | SQLite allocs/op | MonaDB peak heap |
| ---------------------- | ---------------: | ----------: | ---------------: | ---------------: |
| point lookup           |                9 |      36,862 |                2 |            36 KB |
| composite point lookup |               12 |      36,990 |                2 |            36 KB |
| range (100 rows)       |              519 |   3,683,622 |              101 |           1.8 MB |
| prefix (~20 rows)      |               96 |   1,098,213 |               21 |           376 KB |
| single insert          |            1,414 |     183,267 |            1,088 |            58 KB |
| composite insert       |            1,455 |     185,786 |            1,125 |            58 KB |

Observations:

- **The prepared read path is leaner than REPORT-02's ad-hoc path.** Point-lookup
  allocations fell `13 → 9` and range `822 → 519`, because `prepare_cached` skips the
  per-call lex + template `String` + value `Vec` that `normalize` built. Read `B/op` is
  dominated by the single `Rc<[u8]>` copy of each row out of the mmap, so it tracks
  document size (point/composite ≈ one `md` document; range ≈ 100 of them).
- **Insert allocations and peak heap dropped sharply** (`~3,228 → ~1,414` allocs/op;
  peak heap `18 MB → 58 KB` at `md`) because the prepared insert binds a pre-built object
  value instead of building and lexing a 16 KiB object-literal SQL string per row.
- SQLite's allocation columns undercount (C heap invisible to the Rust allocator). They
  are shown only to scale MonaDB's numbers, not for a cross-engine memory verdict; for
  that, run one engine per process and compare `peak_rss_bytes`.

---

## Honesty notes (methodology asymmetries)

1. **Insert input form differs by engine.** MonaDB's prepared insert (`insert into docs
($1)`) binds a fully-constructed object `Value` (built in-process from the fixture via
   `Value::from_json`), so it pays **no document-text parsing**. SQLite's prepared insert
   binds the document as a JSON **text** string and `jsonb(?)` transcodes it to binary
   JSONB on every row. Both are the idiomatic prepared-insert for each engine, but this
   means part of MonaDB's insert win is "MonaDB received the document already structured."
   A workload whose source data is JSON text (and where MonaDB would also have to parse it)
   would narrow this gap. **Do not read the insert Δ as a pure storage-engine result.**
2. **`xs` single point lookup is at the noise floor** (~0.95 µs, ±58% across passes).
   Treat it as parity, not a measured win.
3. **Range reads compare different shapes.** MonaDB expresses a contiguous span as 100
   point gets (`select [docs[?], …]`) because it has no single-key range subscript;
   SQLite uses one `WHERE id BETWEEN` index scan. This is honest to each engine's surface
   but is why MonaDB loses small ranges and wins large ones (per-row decode dominates).
4. **Peak RSS is process-cumulative** in a single matrix run and is therefore omitted from
   the per-cell tables; allocation counts and peak heap are reset per cell and are exact
   for MonaDB.

---

For tracking time + memory over releases, use the `metrics` harness and its CSV. Criterion's
`doc_workloads` report remains the authoritative latency view; the medians above come from
repeated `metrics` passes for stability.
