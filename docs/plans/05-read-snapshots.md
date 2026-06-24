# Read snapshots: sharing one LMDB read transaction across many lookups

**Status:** proposed · **Area:** lookup hot path / transactions

## Finding

Every read query begins and commits its own LMDB read transaction. There is no
way to share a read snapshot across multiple lookups.

The lifecycle is per-statement:

- `Vop::Transaction { txm }` (src/vm.rs ~589) calls `Transaction::new(&self.storage, Read)`
  when `self.txn.is_none()`, which goes through `Transaction::read` →
  `storage.env.read_txn()` (src/transaction.rs ~64). That acquires a slot in
  LMDB's reader lock table and sets up a snapshot. The env is opened
  `read_txn_without_tls()` / `WithoutTls` (src/storage.rs ~46), so reader slots
  are not thread-cached.
- `Vop::Halt` (src/vm.rs ~604) commits the txn (a read commit is a no-op release
  that frees the reader slot).

So a burst of N point lookups begins and commits N read transactions. LMDB read
txns are cheap but not free: reader-table slot management plus per-txn snapshot
setup, paid once per op with no amortization. SQLite, by contrast, amortizes
across a warm page cache and reuses its implicit transaction between statements.

MonaDB already has session-transaction machinery — but it is **write-only**.
`begin;`/`commit;`/`rollback;` (src/lib.rs ~331–354) hold a
`session_txn: Rc<RefCell<Option<Transaction>>>` (src/lib.rs ~144) across
statements. The VM cooperates only when `defer_commit` is set *and* the mode is
`Write`: it `take()`s the session txn at `Vop::Transaction` and returns it at
`Vop::Halt` instead of committing (src/vm.rs ~591–613). Read-only statements
always fall through to the per-op `Transaction::new` path.

## Impact

- N-independent fixed per-op overhead on every read.
- Worst where the SQLite gap is already widest: bursts of small point/composite
  lookups at small documents and small N (REPORT xs / sm sizes), where snapshot
  setup is a large fraction of the work done.
- Read-heavy batch workloads — e.g. resolving many keys for a join or a fan-out
  lookup — repeatedly tear down and rebuild a snapshot that could be held once.

## Brainstorming (options & techniques, with tradeoffs)

**(a) Extend the session-txn mechanism to read-only snapshots.**
A `begin read;` (or an auto-snapshot mode) opens one `RoTxn`, stores it in
`session_txn`, and reuses it across many lookups; `commit;`/`rollback;`/close
drops it. The VM's `Vop::Transaction` would `take()` the session read txn the
same way it already does for writes, and `Vop::Halt` would return it instead of
committing. Smallest delta — reuses the exact borrow/return seam already in the
VM. Tradeoff: stale reads (the snapshot does not see writes committed after it
opened) and the long-reader hazard below.

**(b) Connection-level "current read txn", reused when no write is active.**
Implicitly keep one `RoTxn` on the `MonaDB` handle and reuse it for read
statements, refreshing (commit + reopen) on demand to advance the snapshot —
e.g. lazily after a local write, or on an explicit `refresh`. Best ergonomics
(no API change for callers) but the implicit refresh policy is the whole design;
get it wrong and you either pin the free list forever or lose snapshot
consistency surprisingly.

**(c) Explicit snapshot handles in the embedded / Python API.**
A `snapshot()` context that means "read many keys at one consistent point" —
`with db.snapshot() as s: s.get(...)`. Makes both the perf win and the isolation
guarantee explicit and scoped; the handle's lifetime bounds the long reader.
More API surface, but the clearest semantics.

**(d) Reuse the read txn within a single multi-statement script.**
Narrowest scope: one `RoTxn` for the duration of one submitted batch/script,
committed when the batch finishes. No user-visible API, bounded lifetime (no
long-reader risk across an idle connection), but only helps when lookups arrive
batched in one call.

Cross-cutting: snapshot consistency is a *feature*, not only a perf trick — a
held read txn gives repeatable reads across many lookups. Options (a)/(c) make
that guarantee user-visible; (b)/(d) make it implicit.

## Implementation sketch (code locations, approach, risks, isolation/long-reader caveats)

Approach, favoring (a) as the minimal path that lights up the existing seam:

1. Add a read variant of the session begin. `MonaDB::begin_transaction`
   (src/lib.rs ~331) currently hardcodes `Transaction::write`; introduce a mode
   so a read begin stores `Transaction::read(&self.storage)` in `session_txn`.
2. Teach the VM's `defer_commit` path to cover reads. Today `Vop::Transaction`
   (src/vm.rs ~591) only `take()`s the session txn for `Write`; allow it to take
   a session **read** txn too. At `Vop::Halt` (src/vm.rs ~606) the return branch
   already distinguishes by `as_rw().is_ok()` — a read txn falls into the `else`
   and would be committed (releasing it); to *hold* it across statements, return
   any session-owned txn (read or write) rather than committing.
3. A statement that needs a write while a read session is held must error or
   transparently upgrade — decide the policy (simplest: reject writes while a
   read snapshot is open, mirroring "transaction already active").

Risks / caveats:

- **Long-reader / free-list pinning.** LMDB cannot reclaim pages freed after the
  oldest live read txn's snapshot. A long-held `RoTxn` with concurrent writers
  bloats the data file (the classic LMDB long-reader problem). Snapshots must be
  explicitly scoped and closed — never tie one to an idle connection's lifetime.
  This argues for explicit `begin read;` / `snapshot()` scoping (a/c/d) over a
  never-refreshed implicit txn (b).
- **Reader-slot exhaustion.** Each held snapshot occupies a reader-table slot;
  with `WithoutTls` (src/storage.rs ~46) slots are per-txn, so leaking snapshots
  leaks slots up to the max-readers bound.
- **Lifetime erasure.** `Transaction::read`/`write` `transmute` the heed txn to
  `'static` (src/transaction.rs ~64–91), sound only because each `Transaction`
  carries a `Storage` clone (`Arc<Env>` keep-alive) dropped last (the module
  header, src/transaction.rs ~1–8). A session-held read txn already satisfies
  this — it owns its `Storage` clone — so holding it longer is safe as long as it
  is still dropped/committed before teardown.
- **Staleness.** Reused snapshots will not observe later commits; document this
  as the intended isolation semantics and provide an explicit refresh path
  (commit + reopen) for callers that want to advance.

## References

- src/vm.rs:589 — `Vop::Transaction` (per-op `Transaction::new`, session take)
- src/vm.rs:604 — `Vop::Halt` (commit vs. return-to-session under `defer_commit`)
- src/transaction.rs:56 — `Transaction::new`; :64 `read`; :70 `'static` transmute; :1–8 keep-alive contract
- src/storage.rs:38 — env opened `read_txn_without_tls()` / `WithoutTls`
- src/lib.rs:144 — `session_txn: Rc<RefCell<Option<Transaction>>>`
- src/lib.rs:331–354 — `begin`/`commit`/`rollback` (write-only session today)
- benches/REPORT.md — xs/sm point & composite lookup gap vs SQLite (motivation)
