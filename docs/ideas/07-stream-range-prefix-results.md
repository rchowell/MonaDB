# Stream range/prefix read results instead of eager `Value::Array`

**Status:** proposed · **Area:** range/prefix reads / scaling with N

## Finding

`Vop::GetRange { csr }` (`src/vm.rs:811`) materializes the *entire* match set into
one `Value::array()` before the consumer sees a single row:

```rust
let mut arr = Value::array();
let mut more = cursor.scan(txn, Some(&prefix))?;
while more {
    arr.push(cursor.load()?);   // copies row bytes out of the mmap
    more = cursor.next()?;      // .to_vec()s the (key,val) bytes
}
self.push(arr);
```

Two copies happen per row:

1. `TableScan::next` (`src/cursor.rs:240`) caches the row as owned bytes —
   `self.iter.next()?.map(|(k, v)| (k.to_vec(), v.to_vec()))` — copying the
   (key,val) slices out of the LMDB mmap into fresh `Vec`s.
2. `cursor.load()` → `TableScan::load` (`src/cursor.rs:245`) → `Value::from_storage`
   wraps the value bytes in a fresh `Rc<[u8]>` (`Value::Raw`), a second copy.

So a prefix/range read copies and holds the full result set in memory up front.
This is the lowering for partial-key prefix lookups (`docs["t007"]`) and batch
gets — compiler `cc_expr_get`/`emit_get_range` (`src/compiler.rs:1369`) emit
`Vop::Get` for a full key (point lookup) and `Vop::GetRange` for a leading prefix.

## Impact (scales with N)

This is the read workload whose cost **grows with the table.** A prefix read
returns ~N/tenants rows, and benchmark composite data is spread across
`TENANTS = 100` partitions (`benches/workloads.rs:14`, `preload` at line 62).
So at cardinality 1,000,000 one prefix read materializes ~10,000 rows — ~10k
allocations plus one large array — *per query*, up front, before the consumer
can short-circuit.

`benches/REPORT.md` already quantifies this (md profile memory table):

| Workload (md)     | MonaDB allocs/op | MonaDB B/op |
|-------------------|------------------|-------------|
| range (100 rows)  | 1,025            | 3,702,492   |
| prefix (~20 rows) | 99               | 1,098,637   |

Both scale with row count, and REPORT takeaway #3 ("Range-read result
construction") already flags streaming the result via a cursor.

Crucially, MonaDB *already wins* on large-document range/prefix reads thanks to
the lazy flat codec (REPORT: `lg` range 0.30×, prefix down to 0.27×). Decode is
not the problem — the remaining inefficiency is the eager array construction and
the per-row double copy, not value decoding.

## Brainstorming (options & tradeoffs)

**(a) Lazy cursor-backed sequence result (the big lever).**
Instead of collecting into a `Value::Array`, leave the cursor positioned and let
rows stream to the consumer one at a time — `Yield` per row (`src/vm.rs:582`)
rather than push-one-array. This caps memory at one row and enables
short-circuiting: a `LIMIT`, `EXISTS`, or first-match consumer stops the scan
early instead of paying for all N rows.
- *Interaction with the Sink model* (`src/compiler.rs`, `Sink::Yield` vs
  `Sink::Collect`): a range read in *yield* position should drive the pull loop
  directly (like a `Scan`/`Next` loop), while a range read in *collect* position
  (subquery array, nested expression) still needs an array. So this is really
  "compile `GetRange` to a scan loop whose sink is the enclosing sink," reusing
  the existing `Scan { prefix: ScanPrefix::Stack }` + `Next` machinery
  (`src/vm.rs:823`, `:841`) rather than a dedicated `GetRange` opcode.
- *Interaction with the `Rows` pull iterator* (`src/vm.rs:1128`): already
  pull-based — each `Rows::next` resumes the VM to the next `Yield`. A streaming
  prefix read fits this model with no new surface; the array is the anomaly.
- *Tradeoff:* a range read used as a value (e.g. assigned, passed to a function,
  `len()`-ed) genuinely needs a materialized array — keep a collect path for
  those. The win is restricting eager materialization to genuine value position.

**(b) Kill the double copy in `TableScan` (orthogonal, smaller, safe).**
`next` does `.to_vec()` then `load` does an `Rc<[u8]>` copy. Options: borrow the
value slice directly from the live read txn mmap where lifetimes permit (avoid
caching owned bytes), or at minimum copy *once* (cache only what `load` actually
needs, or build the `Value::Raw` directly from the iterator slice without the
intermediate `Vec`). Halves the per-row allocation on every scan, not just range
reads. Independent of (a) and worth doing regardless.

**(c) Reserve array capacity when bounded.** When the result count is known or
bounded (batch get of K literal keys), `Value::array()` could pre-size to avoid
reallocation churn during `push`. Cheap, but only helps the eager path.

**(d) Keep eager but pre-sized for bounded batch gets.** The 100-row batch-get
(`select [docs[lo], …]`) is genuinely bounded; for it, eager + pre-sized (option
c) is fine and simpler than streaming. Streaming (a) targets the *unbounded*
prefix read where N/tenants grows.

Recommended split: ship (b) as a standalone copy-reduction (helps every scan),
then (a) as the scaling fix for prefix reads in yield position, keeping (c)/(d)
for the bounded batch-get path.

## Implementation sketch

- **Copy reduction (b):** rework `TableScan` (`src/cursor.rs:226`) so `load`
  produces `Value::Raw` from the iterator's borrowed slice without first
  `.to_vec()`-ing in `next`. The iterators are already `'static`-erased over the
  env (`src/cursor.rs:65`), so the borrow can flow into `from_storage`/`Value::Raw`
  with one copy instead of two. Risk: `current()` returns `&[u8]` and is used by
  `cc_delete`'s key capture — keep a key-bytes path even if the value path stops
  caching.
- **Streaming (a):** in `cc_expr_get` (`src/compiler.rs:1369`), when the get is a
  prefix in *yield* sink position, lower it to the existing `Scan`/`Next` loop
  (reuse `emit_key_tuple` for the prefix, `ScanPrefix::Stack`) with the enclosing
  sink driving each row, instead of `emit_get_range`. Keep `emit_get_range`/
  `Vop::GetRange` for collect position (subquery/value). Risk: detecting "yield
  position" cleanly — this is the `Sink` field the compiler already threads.
- **Lifetime/txn caveat (the load-bearing constraint):** streamed rows borrow the
  mmap, valid only while the `RoTxn` lives. The VM already holds the txn across
  pulls (committed at `Halt`, `src/vm.rs:604`, after iteration), and `Rows`
  (`src/vm.rs:1128`) keeps the VM alive across `next` calls — so the txn naturally
  outlives a streamed result. No change to txn lifetime is needed; the eager array
  exists today *only* to decouple the result from the cursor, and streaming
  removes that need precisely because the txn already stays open. The one hazard:
  do not `close()` the cursor or commit before the consumer has pulled the last
  row — the scan loop must own the cursor for the duration, exactly as the current
  `Scan`/`Next` loops do.

## References

- `src/vm.rs:811` — `Vop::GetRange` eager array construction (the finding)
- `src/vm.rs:582` — `Vop::Yield` (per-row stream primitive)
- `src/vm.rs:823`, `:841` — `Vop::Scan` / `Vop::Next` (existing scan loop to reuse)
- `src/vm.rs:1128` — `Rows` pull iterator (already streaming-shaped)
- `src/vm.rs:604` — `Vop::Halt` commits the txn after iteration
- `src/cursor.rs:240` — `TableScan::next` `.to_vec()` copy #1
- `src/cursor.rs:245` — `TableScan::load` → `Value::from_storage` copy #2
- `src/cursor.rs:54`, `:199` — `Cursor::scan` (prefix iterator) and `ValueIter`
- `src/compiler.rs:1369` — `cc_expr_get` / `emit_get_range` lowering
- `benches/workloads.rs:14`, `:62` — `TENANTS = 100`, `preload` partition spread
- `benches/REPORT.md` — memory table + takeaway #3 (streaming the range result)
- Related: `docs/plans/04-vm-pooling.md` (N-independent per-op overhead, complementary)
