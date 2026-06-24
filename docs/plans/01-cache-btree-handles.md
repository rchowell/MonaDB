# Cache LMDB btree handles instead of re-resolving on every Open

**Status:** proposed · **Area:** lookup hot path / storage

## Finding

Every read and write re-opens the table's btree handle from scratch. `Vop::Open { csr, tbl }`
(`src/vm.rs` ~786) calls `self.storage.open_btree(txn, *tbl)`. `Storage::open_btree`
(`src/storage.rs` ~74) calls `self.env.open_database(rtxn, Some(&hex(oid)))`, where
`hex(oid)` (`src/storage.rs` ~92) is `format!("{oid:08x}")` — a fresh `String` allocation —
and `env.open_database` performs an LMDB named-database (dbi) lookup against the main DB.
This runs on *every* point lookup, range read, insert, and delete (also via `Vop::OpenOid`,
`src/vm.rs` ~793).

This is pure rediscovery of a stable fact. heed `Database`/`BTree` handles
(`pub type BTree = Database<Bytes, Bytes>;`, `src/storage.rs:22`) are `Copy` and stable for
the life of the env — designed to be opened once and reused. The codebase already relies on
this *within* a statement: `Open` resolves the handle once, stows it in the cursor's
`Source::Btree { btree, .. }` (`src/cursor.rs` ~46), and scan/insert/delete all reuse that
copy. We just don't carry the handle *across* statements.

SQLite avoids this entirely: a prepared statement already holds the table's rootpage, so a
warm point lookup does no name resolution at all.

## Impact

A `String` alloc plus a dbi resolution per op, sitting on the hot lookup path where the
actual `btree.get` is a small fraction of total cost. Point-lookup allocation count is
N-independent fixed overhead (~956 allocs/op single, measured flat across cardinality
1→10k); this `hex` String is one avoidable slice of that fixed cost.

The relative win is largest at small N, exactly where fixed overhead dominates and MonaDB
trails SQLite most (REPORT: xs/sm point lookups 1.2–1.9× slower). At large N the btree
descent dominates and this matters less in relative terms — but it's never negative.

## Brainstorming (options & techniques, with tradeoffs)

**(a) Cache `oid → BTree` in a map owned by `MonaDB` (or `Storage`).**
Lazy-populate on first `Open`, look up on subsequent ones. `BTree` is `Copy`, so the cache
stores values, not references. Cheap, central, easy to invalidate by clearing on DROP.
Tradeoff: the cache lives behind the VM, so `Open` still does a `HashMap` probe (hash of a
`u32`) — cheaper than a String alloc + dbi walk, but not free. A tiny `Vec<(u32, BTree)>`
beats a `HashMap` for the handful of hot tables.

**(b) Store the handle in the catalog entry itself.**
The catalog already maps names→oids and is consulted at bind time. Hang the `BTree` off the
catalog row so resolution is a field read. Natural invalidation: DROP removes the row, so the
handle vanishes with it. Tradeoff: couples storage handles into the catalog (a binder-time
structure) and means the catalog must be loaded/threaded into the VM's `Open` path; ordering
of "create btree" vs "catalog row populated" needs care.

**(c) Thread the handle into the compiled program / `PreparedStatement` so `Open` is an array
index.** Closest to SQLite's model: resolve once at prepare time, bake the `BTree` (or a slot
index into a per-statement handle table) into the `Open` op. A warm cached plan then does zero
resolution. Tradeoff: prepares are cached and reused across executions (`run_cached`,
`src/lib.rs` ~210), so a baked handle must be invalidated exactly when the plan is — which is
already wired through `catalog_generation` (stale-plan detection, `src/prepared.rs:69`). This
is the highest-payoff but most invasive option.

**Thread-safety / lifetime constraints (all options):** a `BTree` handle must not outlive its
`Env`. `Storage` holds `Arc<Env<WithoutTls>>` (`src/storage.rs:28`), so any cache co-located
with or downstream of `Storage` is fine as long as it's dropped no later than the env. Don't
leak handles into anything that can outlive the `MonaDB`/`Storage`.

## Implementation sketch (code locations, approach, risks, invalidation)

Recommended staging: ship **(a)** first (smallest, self-contained), measure, then consider
**(c)** if prepare-time baking pays for the extra wiring.

- Add a handle cache to `Storage` (or `MonaDB`): e.g. `RefCell<Vec<(u32, BTree)>>` or
  `RefCell<HashMap<u32, BTree>>`. `Storage` is `Clone` and `Arc`-shares the env, so wrap the
  cache in the same shared cell if multiple `Storage` clones must agree.
- Introduce `Storage::btree(&self, txn, oid) -> Result<BTree>`: probe the cache; on miss call
  the existing `open_database` path and insert. Keep `hex` only on the cold miss path.
- Repoint `Vop::Open` and `Vop::OpenOid` (`src/vm.rs` ~786/793) at the cached accessor.
- **Invalidation on DROP:** `cc_drop` (`src/compiler.rs:445`) emits `Clear { oid }`; DROP must
  also evict the cached handle for that oid. Tie eviction to the same signal that bumps
  `catalog_generation` (`src/lib.rs` ~135/369) — DROP/CREATE already drive plan-cache
  staleness via `Error::StalePreparedStatement` (`src/lib.rs:219`, `src/prepared.rs:69`).
  Simplest correct rule: on any catalog-generation bump, clear the whole handle cache (the
  handful of tables re-warm on next `Open`).
- **CREATE:** `create_btree` (`src/storage.rs:67`) can pre-seed the cache with the new handle,
  or leave it to lazily populate on first `Open`.

**Risks:**
- A handle cached against a dropped-then-recreated oid must not survive the recreate. The
  generation-bump-clears-everything rule covers this; a per-oid eviction must fire on *both*
  DROP and the matching CREATE.
- Stale handle after DROP without eviction would read/write a cleared or reused dbi — must be
  prevented by invalidation, not by hope.
- If option (c) is pursued, the baked handle in a cached plan is only safe because
  `catalog_generation` already forces a re-prepare on membership change; do not bypass that
  check.

## References

- `src/vm.rs` ~786 — `Vop::Open`; ~793 — `Vop::OpenOid` (the hot re-open path)
- `src/storage.rs:22` — `pub type BTree = Database<Bytes, Bytes>;` (Copy, env-lifetime)
- `src/storage.rs:67` — `create_btree`; ~74 — `open_btree`; ~92 — `hex(oid)` String alloc
- `src/storage.rs:28` — `Storage` holds `Arc<Env<WithoutTls>>`
- `src/cursor.rs` ~46 — `Source::Btree { btree, .. }` (existing intra-statement handle reuse)
- `src/lib.rs` ~135/369 — `catalog_generation`; ~210 — `run_cached`; ~219 — stale-plan evict
- `src/prepared.rs:69` — `catalog_generation` staleness check
- `src/compiler.rs:445` — `cc_drop` (DROP path; invalidation hook point)
- Related: REPORT (xs/sm point lookups 1.2–1.9× slower); the flat-value/fixed-overhead
  allocation finding (~956 allocs/op, N-independent).
