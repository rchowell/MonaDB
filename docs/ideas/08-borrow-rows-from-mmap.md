# Borrow rows from the LMDB mmap instead of copying into `Rc<[u8]>`

**Status:** research / exploratory · **Area:** read path / zero-copy

## Finding

Every row read copies the row out of the LMDB memory-mapped page into an owned
`Rc<[u8]>`. The seam is `Value::from_storage` (src/value.rs ~547):

```rust
let buf: Rc<[u8]> = Rc::from(bytes);   // one alloc + memcpy of the whole row
Ok(Value::Raw(RawValue { buf, at: 0, end }))
```

`bytes` here is a `&[u8]` pointing directly into the LMDB mmap. The `Rc::from`
allocates and copies the full row out into an owned buffer; the resulting lazy
`Value::Raw` then navigates allocation-free thereafter (the doc comment on
`from_storage` says as much — "a single allocation — one copy of the row out of
the LMDB mmap"). This copy fires on:

- the point-lookup path: `Vop::Get` (src/vm.rs ~805) → `Cursor::get`
  (src/cursor.rs ~158) → `from_storage`;
- per-row in range/prefix reads: `Vop::GetRange` (src/vm.rs ~811) and table
  scans, where `TableScan::next` first `to_vec()`s key+val out of the iterator
  (src/cursor.rs ~241) and `TableScan::load` then `from_storage`s the val — so a
  range read pays the copy (and an extra `Vec`) once **per row** (see sibling
  plan 07).

It is the dominant remaining `B/op` for reads: REPORT.md (~line 129) attributes
the residual bytes to "the single `Rc<[u8]>` copy of each row out of the mmap".

**Why it exists.** LMDB mmap bytes are only valid while the read transaction is
live. Copying to an owned `Rc` makes the resulting `Value` safe to outlive the
txn — `Value` is currently `'static`, and the `RoTxn` is itself
`unsafe { transmute }`-erased to `'static` and kept alive by an `Arc<Env>` clone
(src/transaction.rs ~64-76). So the copy is the price of decoupling `Value`'s
lifetime from the txn's.

But within a single query the `RoTxn` is held by the VM for the entire `Rows`
iteration and only commits at `Vop::Halt`. For the common "produce the row,
consume it, finish" pattern the row never needs to outlive the txn — the copy is
**conservative**, not required. The benchmark's `drain_one` even re-encodes the
`Raw` straight back to bytes (benches/monadb.rs) — a bench artifact, but it
illustrates that the row frequently is read and dropped without ever escaping.

## Impact

One alloc + a memcpy of the entire row per point lookup; for range reads the
copy count (and a redundant `Vec` per row in `TableScan`) scales with N. For
large documents this is real, measurable `B/op` — though MonaDB still wins at the
`lg` size because navigation into `Raw` stays lazy/allocation-free regardless.
Eliminating the copy on the borrow-safe path would push the read `B/op` down
toward SQLite's.

## Brainstorming (options & techniques, with honest difficulty/tradeoffs)

**(a) Borrowed `Raw` variant — lifetime-parameterized `Value`, copy-on-escape.**
Add a `Value::Raw` backing that holds `&'txn [u8]` into the live mmap, and
promote to an owned `Rc<[u8]>` only when the value must outlive the txn. This is
the textbook zero-copy answer and the biggest win. It is also the hardest: today
`Value` is `'static`. Threading a `'txn` lifetime through `Value` ripples into
the stack, cursors, the agg bank, sorter, subquery sinks, and every API that
returns a `Value` — and the txn lifetime is *already* unsafe-erased to `'static`
(src/transaction.rs), so the borrow would be borrowing from a `'static` lie. You
would either re-introduce a real lifetime (large surgery, fights the
self-referential VM design plan 05 leans on) or keep the erasure and make the
borrow itself `unsafe`, pushing the safety obligation onto every escape site.
High risk, high reward; not a quick win.

**(b) `Rc`-shared page handle / zero-copy slice that keeps the txn+env alive.**
Instead of `Rc<[u8]>` owning a *copy*, have `RawValue` hold an `Rc` to a
txn/env-guard plus an `(at, end)` offset range into the mmap — i.e. an owned-ish
handle whose `Deref` points back into the page. Cloning is a refcount bump (same
as today's `Rc<[u8]>`), and no per-row heap copy happens. This keeps `Value`
`'static` (no lifetime ripple) at the cost of: every live `Value::Raw` now pins
the whole read txn (and a reader-table slot) open until dropped, and the mmap
slice must be wrapped so `RawValue`'s existing `buf: Rc<[u8]>` / offset logic
still works (likely a small enum: `Owned(Rc<[u8]>)` vs `Mapped(Rc<TxnGuard>,
range)`). Medium difficulty, medium payoff, and it composes well with plan 05
(shared read snapshots) — the snapshot becomes the thing the page handle pins.

**(c) Consume-then-drop fast path only — a `get_into` / visitor read API.**
Don't materialize a `Value` at all on the hot path. Add an API that reads fields
directly from the mmap bytes (the flat codec already supports lazy navigation)
and writes them into the caller's sink — projection, comparison, encode — without
ever constructing an owned `Raw`. Scope it to the known "read row, project/emit,
discard" pattern. Lower blast radius than (a) because it doesn't touch the
`Value` type; the cost is a parallel read path that must mirror flat-codec
navigation and only covers the cases that opt in. Medium difficulty, partial
coverage.

**(d) Leave point lookups; focus zero-copy on range reads.** The single-row
copy on `Vop::Get` is one alloc; the range path copies per row *and* does the
redundant `TableScan` `Vec`. The cheapest honest win is to kill the per-row
double-copy in scans (plan 07 territory) and leave point lookups as-is. Low risk,
narrower scope, leaves the point-lookup `B/op` on the table.

## Implementation sketch (what would have to change; risks; why it's hard)

Most promising near-term: **option (b)** layered on plan 05.

1. Generalize `RawValue.buf` from `Rc<[u8]>` to a small enum — `Owned(Rc<[u8]>)`
   for today's copy/`materialized` path, and `Mapped { guard: Rc<TxnGuard>,
   range }` for the borrowed path. `at`/`end` offset arithmetic stays identical;
   only the byte-slice accessor branches.
2. Introduce a `TxnGuard` (`Rc`-shared) that owns the `RoTxn` + `Arc<Env>`
   keep-alive, so any `Value::Raw` cloned out of a scan keeps the snapshot live
   for exactly as long as it is referenced. This is the natural home for plan
   05's shared read snapshot.
3. Change `from_storage` (and `Cursor::get` / `TableScan::load`) to construct the
   `Mapped` variant from the cursor's current mmap slice + guard, instead of
   `Rc::from(bytes)`.
4. `materialized()` and the CoW mutators (`own`, `push`, `spread`) already
   collapse `Raw` → owned tree; ensure a `Mapped` raw materializes (real copy)
   the moment it is mutated or must outlive the guard.

**Risks / why it's genuinely hard:**

- **Aliasing the mmap.** Multiple `Value::Raw` handles (and concurrent cursors)
  would alias the same read-only pages. Read-only aliasing is sound, but the
  borrow must be expressed without re-introducing a `&'txn` lifetime that fights
  the `'static` erasure in src/transaction.rs.
- **Txn lifetime / page stability.** LMDB guarantees page stability only within
  the txn. A `Mapped` value that outlives its `TxnGuard` is use-after-free; the
  guard's refcount is what makes that safe, but every escape (returning a row
  across a commit, stashing it in a long-lived cache) must go through
  `materialized()` first. Getting that contract exhaustively right is the crux.
- **Pinned snapshots.** Holding `Mapped` values keeps reader-table slots and the
  snapshot open longer, which can grow the LMDB freelist / delay reclamation
  under a long-running consumer. A long-lived `Value` would now retain a whole
  read snapshot — a footgun worth a doc note.
- Option (a) is strictly more invasive than (b) for marginal extra benefit; defer
  it unless profiling shows the `TxnGuard` refcount itself is hot.

## References

- src/value.rs ~547 `Value::from_storage` (the copy); ~533 `encode`; ~558
  `materialized`; ~520 `own` / `RawValue`
- src/cursor.rs ~158 `Cursor::get`; ~241 `TableScan::next` (redundant `to_vec`);
  ~245 `TableScan::load`
- src/vm.rs ~805 `Vop::Get`; ~811 `Vop::GetRange`
- src/transaction.rs ~64-76 `Transaction::read` lifetime erasure (`transmute` to
  `'static`, `Arc<Env>` keep-alive)
- benches/REPORT.md ~129 (residual `B/op` = the per-row `Rc<[u8]>` copy)
- Related plans: 05 (read snapshots — supplies the `TxnGuard`/shared snapshot),
  07 (range-read per-row copy — the N-scaling sibling of this finding)
