# VM pooling / scratch reuse on the lookup hot path

**Status:** proposed · **Area:** lookup hot path / VM lifecycle

## Finding

A fresh `VM` is allocated and dropped per statement. Every `execute_prepared`
(`src/prepared.rs:87`) builds a new VM via `VM::init` (`src/vm.rs:329`) and wraps
it in `Rows` (`src/vm.rs:1128`). `VM::init` allocates per call:

- `cursors` — `Vec::with_capacity(program.cursors)` + `resize_with(..., Cursor::new)`
- `counters: vec![0; program.counters]`
- `aggs: vec![Value::Null; program.aggs]`
- `stack: vec![]` (grows on first push)
- `params.clone()` — a deep clone of the bound `Vec<Value>` (+ named map)
- several Rc/Arc bumps: `storage`, `catalog_generation`, `program`, `session_txn`

The whole VM and its vecs are dropped when `Rows` is dropped at end of query. For
a point lookup the vecs are small, but these are several allocations on a path
whose total is ~956 allocs/op and whose target is sub-microsecond.

## Impact

N-independent fixed per-op overhead. Pooling/reusing the VM scratch state across
executions removes these allocations from *every* lookup, regardless of result
size. It compounds with the cached-btree-handle and plan-cache findings
(sibling plans `01` and `02`): once the plan is cached and the btree handle is
reused, the residual per-op allocation is dominated by VM setup and `params`.

## Brainstorming (options & techniques, with tradeoffs)

- **(a) Per-connection scratch arena.** Keep `stack`, `cursors`, `counters`,
  `aggs` as long-lived `Vec`s owned by `MonaDB`. On each execute, `truncate(0)`
  / `clear()` then `resize_with` to the program's required sizes — reusing
  capacity instead of reallocating. *Pro:* removes the bulk of the allocs, no
  API change. *Con:* `Cursor::new` slots must be re-initialized each run; needs
  a clean borrow story (see reentrancy below).

- **(b) Object-pool of VMs on the handle.** Park whole idle `VM`s (or their
  scratch) on `MonaDB`, hand one out at execute, return it at `Halt`/drop.
  *Pro:* amortizes everything including the struct itself. *Con:* MonaDB is
  single-threaded and not `Send` across the env in this design, so the pool is
  trivially a `Vec<VmScratch>` — but lifetime plumbing (returning at `Halt`) is
  the tricky part.

- **(c) Avoid `params.clone()`.** Pass params by shared reference or `Rc<Params>`
  into the VM instead of deep-cloning the bound vec. `LoadParam` only reads, so
  shared ownership is sufficient. *Pro:* removes a per-op clone whose cost scales
  with bound-param count. *Con:* `Params` lifetime must outlive `Rows`, or be
  `Rc`-wrapped; mild API ripple.

- **(d) Lazily allocate aggs/counters.** Point lookups have zero aggs/counters,
  so `vec![...; 0]` is already cheap — but the `cursors` vec is not. Prefer
  reusing the cursors vec (option a) over micro-optimizing the empty ones.

## Implementation sketch (code locations, approach, risks)

Start with **(a) + (c)** as the minimal, low-risk pair.

1. Introduce a `VmScratch { stack, cursors, counters, aggs }` owned by `MonaDB`.
   `execute_prepared` (`src/prepared.rs:63`) hands it to `VM::init`, which
   `clear()`s + `resize_with`s each vec to `program.{cursors,counters,aggs}`
   rather than allocating. Cursors must be reset to `Cursor::new` state per run.
2. Change `params: Params` to `params: Rc<Params>` (or `&Params` tied to the
   `Rows` lifetime) to drop the `params.clone()` at `src/prepared.rs:92`.

**Reentrancy.** A `Rows` iterator (`src/vm.rs:1128`) borrows the VM until
exhausted, so the scratch can't return to the pool until `Halt` or drop. Two
viable shapes: (i) move scratch *into* the VM at execute and move it *back out*
on `Halt`/`Drop` (option b mechanics, fits the existing move-based `Rows`);
(ii) keep scratch on the handle behind a `RefCell` and have `Rows` borrow it,
which conflicts with `&mut self` execute ergonomics. Shape (i) is cleaner.

**Correctness with `defer_commit` / `session_txn`.** `Halt` (`src/vm.rs:604`)
clears `cursors`, and either returns the write txn to `session_txn` (deferred)
or commits it. Scratch return must happen *after* the txn is settled at `Halt`,
and also on the error/early-drop path — so wire it into both `Halt` and `Drop`
for `VM`/`Rows`, never only the happy path. The `txn`/`session_txn` Rc/Arc
handles are not part of the reusable scratch and stay per-execute.

**Risks.** Stale cursor state leaking across executions (must fully reset);
double-return of scratch on a `Halt`-then-`Drop` sequence; holding a pooled VM
across a CREATE/DROP that changed `catalog_generation` (already guarded at
`src/prepared.rs:69`, but pooled scratch sizing must re-read the new program).

## References

- `src/prepared.rs:63` — `execute_prepared`; `:87` `VM::init` call; `:92` `params.clone()`
- `src/vm.rs:289` — `VM` struct fields; `:329` `VM::init` allocations
- `src/vm.rs:604` — `Vop::Halt` (cursor clear, defer_commit/session_txn settle)
- `src/vm.rs:1128` — `Rows` iterator wrapping the VM
- Related: `docs/plans/01-*` (cached btree handle), `docs/plans/02-*` (plan cache)
