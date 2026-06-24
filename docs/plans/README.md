# Plans

Hi, this is actually Conner (not Claude). This folder has Claude-assisted
implementation plans that I want to save but not yet implement.

I am constantly measuring MonaDB against SQLite (which is very fast) to
identify where I can improve on MonaDB's performance. Claude is excellent
at working backwards (#amzn) from benchmarks to identify where to improve.

## Point & composite-key lookup investigation (2026-06-24)

Findings from working backwards from `benches/REPORT.md` on the point/composite
lookup path. The per-op cost is dominated by **N-independent fixed overhead**
(allocations measured flat across cardinality 1→10k), which is why MonaDB trails
SQLite most on small documents/small N and wins on large documents. Each doc has
a brainstorming section and an implementation sketch.

Lookup hot path (helps most at small N, where the gap is widest):

- [01 — Cache LMDB btree handles](01-cache-btree-handles.md) — `Vop::Open` re-resolves the named sub-DB (String alloc + dbi lookup) every op.
- [02 — Cheap `PlanCache::get`](02-plan-cache-get-overhead.md) — per hit: clone the whole `PreparedStatement`, O(cap=256) LRU scan, extra String alloc, SipHash.
- [03 — Prepared lookup / skip `normalize()`](03-prepared-lookup-skip-normalize.md) — every ad-hoc query re-lexes + rebuilds a template just to compute the cache key.
- [04 — VM pooling / scratch reuse](04-vm-pooling.md) — a fresh VM (cursors/counters/aggs vecs + `params.clone()`) is allocated and dropped per statement.
- [05 — Read snapshots](05-read-snapshots.md) — every read begins+commits its own LMDB read txn; extend the write session-txn machinery to reads.

Composite keys:

- [06 — Composite cache normalization](06-composite-cache-normalization.md) — string key literals stay verbatim → one plan per tenant; measured ~4360 vs ~956 allocs/op (recompilation churn).

Range/prefix reads (scales with N):

- [07 — Stream range/prefix results](07-stream-range-prefix-results.md) — `GetRange` eagerly materializes a `Value::Array` of every match (~N/tenants rows per prefix read).
- [08 — Borrow rows from the mmap](08-borrow-rows-from-mmap.md) — *research:* `from_storage` copies each row into an `Rc<[u8]>`; the dominant read `B/op`.

Measurement enabler:

- [09 — Batched preload + cardinality knob](09-batched-preload-cardinality-knob.md) — preload commits one txn per row (~5 ms), so 1M rows ≈ 80 min; blocks measuring lookup scaling at 100k–1M.