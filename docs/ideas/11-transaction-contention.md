# Transaction contention: a held result can deadlock the connection

**Status:** partially addressed · **Area:** transactions / lazy results / prepare path

> **Update.** The transaction-simplification pass closed **W2** and narrowed
> **W3**. `guard_idle` (formerly `guard_statement_in_progress`) now runs in
> `compile_plan` and `execute_plan` rather than `route_session`, so `prepare`,
> `Statement::query`, and `PyConnection::prepare` are covered — a statement
> issued while a prior result holds the session txn errors instead of opening a
> second write txn. `resolve_tables` also returns early when a program opens no
> table, so `select 1;` and transaction control no longer take the writer lock.
> **W1, W4, and W3-for-table-statements remain live** — see below.
>
> **Correction (same session).** The claim above was initially overstated: a
> build-time guard alone cannot close W2, because `Vop::Transaction` runs
> *lazily* on the first `Rows::next()`. Two results built before either is
> stepped both pass `guard_idle`, and `ConnState::lend()` originally collapsed
> `Closed` and `Lent` into `None` — so the second silently opened its own
> transaction (wrong snapshot for a read, writer-lock deadlock for a write).
> `lend()` now returns `Result<Option<Transaction>>` and errors on `Lent`, which
> is what actually closes W2. Pinned by
> `second_statement_cannot_steal_a_lent_session_txn` (src/lib.rs). The general
> lesson for W1 below: **any guard that runs before the VM does cannot bound a
> lazily-executed statement.**

## Finding

Write-transaction contention in MonaDB fails by **hanging**, not by erroring, and
one reachable path bypasses the guard that was written to prevent exactly this.

### There is no busy error

`heed::Env::write_txn()` is `RwTxn::new(self)` with no guard
(heed-0.22.1/src/envs/env.rs:383) — straight to `mdb_txn_begin`, which takes the
writer lock. On macOS that lock is a **System V semaphore**, not a pthread mutex:
mdb.c:166-169 selects `MDB_USE_SYSV_SEM` for `__APPLE__` (the `posix-sem` cargo
feature is off by default, lmdb-master-sys/build.rs:142). Acquisition is
`semop(sem, {-1, SEM_UNDO})` in a retry-on-`EINTR` loop (mdb.c:435-449).

SysV semaphores have no owner and no recursion. A second write-txn acquisition
from the *same thread that already holds one* does not return `EDEADLK` — it
blocks forever. LMDB has no lock timeout; MonaDB has no busy handler.

### The window is narrower than it looks

`emit_yield` is reachable only through `Sink::Yield` (src/compiler.rs:1949), and
`Sink` is `Insert` for CTAS (:301) and `Collect` for subqueries (:1493).
`cc_insert` (:480) and `cc_delete` (:520) emit no `Yield` at all. So an autocommit
write statement runs `Init → Transaction → body → Halt` inside a **single**
`next()` call — the txn opens and commits without ever returning to the caller.
A *successfully progressing* write statement never holds an `RwTxn` across the
API boundary.

### The paths that do reach it

**W1 — mid-statement error with the result retained (autocommit). STILL LIVE.**
Not covered by the guard: `guard_idle` inspects the *session*, which is `Closed`
in autocommit, so it passes and the next statement opens a second write txn. The
retained transaction is a `VmTxn::Owned`, invisible to connection state.
`Vop::Transaction` has opened the `RwTxn`; an error propagates out of `next()`
before `Halt`; `Rows` stays `Active` still owning the txn. Any later statement
needing a write txn hangs.

```rust
let mut r = db.query(r#"insert into t ({"z": 9});"#)?;  // no txn yet
let _ = r.next();                     // Err — RwTxn now open, retained in r
db.execute(r#"insert into t ({"id": 1});"#)?;           // hangs forever
```

Not reachable through `execute()`, which is `query()?.finish()` — `finish(mut self)`
consumes the `Rows`, so the txn aborts on drop. Reachable through `query()` plus
a manual `next()`, and through `Statement::query()`.

**W2 — `prepare` bypassed the in-progress guard. FIXED.**
The guard lived in `route_session`, which only `query`/`query_with` call, so
`MonaDB::prepare` → `compile_plan` → `resolve_tables` went nowhere near it;
finding the session slot empty because a held `Rows` had borrowed the txn out,
it opened a **second** write txn and hung. It now calls `ConnState::guard_idle`
at the top of both `compile_plan` and `execute_plan`, and the `Lent` state makes
"a statement holds it" explicit rather than inferred from an empty slot.
Regression: `statement_in_progress_is_refused_on_every_entry_path` (src/lib.rs).
The sequence that used to hang:

```rust
db.begin_transaction()?;
db.execute(r#"insert into t ({"id": 1});"#)?;
let mut rows = db.query("select * from t;")?;
rows.next()?;                       // session txn moves into rows' VM; slot empty
let stmt = db.prepare("select * from u;")?;   // was: hang. now: Error::Transaction
```

It compiled at all because `Rows` carries no lifetime tie to the connection —
still true, so the guard is a runtime check, not a type-level one. Option (c)
below (a `Rows<'conn>` lifetime) remains the only way to make it a compile error.

**W3 — compiling a table statement takes the global writer lock. NARROWED.**
Programs that open no table (`select 1;`, `begin;`/`commit;`/`rollback;`, a pure
file scan) now return early from `resolve_tables` instead of opening and
committing a write txn. Statements that *do* touch a table still take the lock
at compile time, so the cross-process case below is unchanged:
`resolve_tables` must resolve handles in a *committed write* txn, because LMDB
only registers a named dbi into the shared env on commit (doc comment at
src/statement.rs:126-134; regression test `reopen_queries_user_table`). So process
A compiling `select * from t;` blocks behind process B's long write — a read
blocked by a write, which LMDB's MVCC otherwise never does.

**W4 — two `MonaDB` handles on one path in one process. STILL LIVE.** Each `open`
builds its own `Env`; LMDB forbids opening an environment twice in a process.
Nothing guards it; src/python.rs:172 acknowledges the hazard in a comment only.

## Impact

- W2 needs no error and no API misuse beyond holding a result, and it fires on a
  read-only statement. It is the one a real user hits.
- W1 needs a failed write plus a retained result — plausible in a REPL or a
  notebook, where results linger in a variable.
- W3 is bounded to *compilation*: a plan-cache hit skips `compile_plan` entirely
  (src/lib.rs:178-185), so steady-state reads are unaffected. It still means a
  cold SELECT can stall behind an unrelated writer.
- A hang is strictly worse than an error for an embedded database: no timeout, no
  diagnostic, and in a single-threaded connection no way to recover.

Non-hanging costs in the same area: a retained partially-consumed SELECT `Rows`
holds a read snapshot, so pages freed after it cannot be reclaimed and the file
grows (see [05-read-snapshots.md](05-read-snapshots.md) for the same long-reader
hazard from the other direction). And `max_readers` is unset (LMDB default 126)
with a `WithoutTls` env, so leaking >126 results yields `MDB_READERS_FULL`,
stringified into `Error::Storage` (src/error.rs:141) with the cause erased.

## Brainstorming (options & techniques, with tradeoffs)

**(a) Extend the guard to the `prepare` family.** Call
`guard_statement_in_progress` at the top of `prepare`, `prepare_cached`, and
`cached_plan`. Smallest possible delta, closes W2 exactly, and turns the hang
into the error the guard already has wording for. Tradeoff: it rejects a
`prepare` that would have been harmless outside a session, and it does nothing
for W1.

**(b) Make `resolve_tables` not need a write txn.** The write txn exists only so
LMDB registers the dbi. If handles were resolved once at open (or lazily under
the *session* txn whenever one exists, erroring rather than opening a second),
the prepare path would stop taking the writer lock at all — closing W2 and W3
together. Tradeoff: needs a different answer to the reopen problem that
`reopen_queries_user_table` pins.

**(c) Tie `Rows` to the connection with a lifetime.** `Rows<'conn>` would make
W1 and W2 compile errors instead of runtime hangs — the borrow checker enforcing
what the guard checks dynamically. Much the strongest option and the most
invasive: it changes a public type, and the `'static` transmute in
src/transaction.rs exists precisely to avoid this lifetime.

**(d) A single write-txn sentinel on the connection.** An `Rc<Cell<bool>>` set
whenever any write txn is outstanding, checked before every `write_txn()` call,
erroring instead of blocking. Catches W1, W2, and any future path in one place,
and is cheap. Tradeoff: it is a lock on top of a lock, and it must be released on
every exit path or it wedges the connection — the same discipline `Drop for VM`
already implements for the session slot.

**(e) Detect W4 at open.** A process-wide registry of open env paths, erroring on
a second `MonaDB::open` of the same absolutized path. Independent of the others
and cheap; LMDB's own rule makes it unambiguously correct.

Cross-cutting: whatever the mechanism, the *observable* outcome should be an
`Error::Transaction`, not a hang. That is the SQLite contract (`SQLITE_BUSY` plus
a busy handler) and matches the wording `guard_statement_in_progress` already
uses.

## Implementation sketch

Step 2 below is **done** (see the update at the top). What remains for W1:

1. Add a `writer_held: Rc<Cell<bool>>` to `ConnState`, set whenever any write txn
   is outstanding — including a `VmTxn::Owned` one — and cleared on
   commit/abort/drop. Every write-txn open (src/statement.rs `resolve_tables`,
   `ConnState::begin`, `Vop::Transaction`) checks it first and returns
   `Error::Transaction` rather than blocking. This is the only one of the options
   that catches W1, because W1's transaction is owned by the VM, not the session.
2. ~~Call the in-progress guard from the `prepare` family.~~ Done: `guard_idle`
   now runs in `compile_plan` and `execute_plan`.
3. Add a regression test for W1. It must be written with a timeout — a plain
   `#[test]` that reproduces the bug hangs the suite rather than failing it,
   which is why it is still uncovered.

Risks / caveats:

- **Clearing the sentinel on every path.** The session txn moves between the slot
  and the VM and back; `Drop for VM` is the only thing that reliably runs on every
  exit. The sentinel has to be cleared there too, or an abandoned `Rows` wedges
  the connection permanently — trading a hang for a softer hang.
- **W3 is not addressed by either.** Erroring instead of blocking is wrong for
  genuine cross-process contention, where waiting is the correct behaviour. Only
  (b) actually removes the writer-lock acquisition from the read path.
- **Tests that hang.** Any regression test here needs a watchdog thread or a
  subprocess; `cargo test` has no per-test timeout.

## References

- src/statement.rs — `resolve_tables`, the committed-write-txn resolve (now with
  an empty-`oids` early return); `compile_plan`/`execute_plan` call `guard_idle`
- src/session.rs — `ConnState`, the `Session` state machine, `guard_idle`
- src/vm.rs — `Drop for VM` and `VmTxn`, the always-restore contract
- src/compiler.rs — `Sink` dispatch; why writes never `Yield`
- heed-0.22.1/src/envs/env.rs:383 — `write_txn` has no guard
- lmdb-master-sys-0.2.6/…/mdb.c:166-169, :435-449 — SysV semaphore on macOS
- [05-read-snapshots.md](05-read-snapshots.md) — the long-reader half of this story
