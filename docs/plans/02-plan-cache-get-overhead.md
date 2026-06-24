# PlanCache::get overhead on the point-lookup hot path

**Status:** proposed · **Area:** lookup hot path / plan cache

## Finding

Every ad-hoc query goes through the auto-parameterizing plan cache
(`PlanCache` in `src/lib.rs:77`). `MonaDB::query` normalizes the SQL to a
`?`-templated key and calls `run_cached` (`src/lib.rs:210`), which on a HIT
calls `PlanCache::get(key)` (`src/lib.rs:92`). That hit does more work than its
size suggests:

1. **`self.plans.get(key)?.clone()`** — clones the whole `PreparedStatement`
   (`src/prepared.rs:21`). The `program: Rc<Program>` clone is a cheap refcount
   bump, but `sql: String` and `required_params: Vec<Param>` are **deep-cloned**
   → ~2 heap allocations every hit. (The clone exists only so the cache borrow
   can be released before executing under `&mut self` — the caller needs a shared
   handle, not an owned copy.)
2. **`self.detach_order(key)`** (`src/lib.rs:120`) — `order.iter().position(...)`
   then `remove(pos)`: an **O(cap) linear scan** of the `VecDeque` doing up to
   `cap = PLAN_CACHE_CAP = 256` (`src/lib.rs:70`) String comparisons per lookup,
   to find and unlink the touched key for LRU recency.
3. **`self.order.push_back(key.to_owned())`** — another String allocation to
   re-own the key at the back of the order queue.
4. The default std `HashMap` hashes the String key with **SipHash**, which is
   robust but slow for short keys like SQL templates — and the key is hashed
   twice on a hit-then-touch path (`get`, and again if `detach_order`/insert run).

## Impact

Point and composite-key lookups (`t[1]`, `c['x', 7]`) are the latency target
where MonaDB is benchmarked against SQLite. Their cost is **N-independent fixed
overhead** — the actual `btree.get` is a tiny fraction of the per-call work, so
anything on the dispatch path is proportionally large. The cache hit currently
adds, per lookup:

- ~4–5 heap allocations (`sql` clone, `required_params` clone, key re-own,
  plus `Params`/key churn upstream),
- an O(256) linear scan of the order `VecDeque`,
- one or two SipHash passes over the key.

Measured baseline for a single-key lookup is ~956 allocs/op total; trimming the
cache path removes several of those and eliminates the linear scan entirely.
At sub-microsecond targets this is a meaningful slice of the budget.

## Brainstorming (options & techniques, with tradeoffs)

**(a) Store `Rc<PreparedStatement>` in the map.** `get` becomes a single
refcount bump — no `sql`/`required_params` clone. `execute_prepared` only ever
takes `&PreparedStatement`, so callers genuinely need a *shared handle*, not an
owned copy. Removes 2 allocs/hit. Cost: `prepare` wraps its result in `Rc` once;
`insert` and the `query_with` clone-to-cache both get cheaper. Lowest-risk, highest-payoff change.

**(b) Replace the VecDeque LRU with O(1) recency.** Options:
- *`lru` crate* — a `HashMap` + intrusive doubly-linked list; `get`/`put` are
  O(1) amortized, no scan, no per-touch String alloc. Least code, well-tested.
- *Intrusive linked LRU hand-rolled* — same shape, no dependency, more code.
- *`HashMap<String, (Rc<plan>, u64 tick)> + monotonic counter`* — touch just
  bumps the entry's tick (O(1), no alloc); eviction scans for the min tick only
  *when over cap* (rare), not on every hit. Simple, moves all cost off the hot path.
- *Approximate eviction (CLOCK / 2-random)* — a reference bit or sampling two
  keys and dropping the older. O(1), no ordering structure, no per-hit alloc.

**(c) Swap the hasher.** Use `ahash` or `FxHash`/`rustc-hash` for the map.
Short-string keys hash far faster than SipHash; collision-DoS resistance is moot
for a process-local plan cache. Drop-in via `HashMap<_, _, FxBuildHasher>`.

**(d) Avoid re-owning the key on touch.** Whatever LRU structure is chosen,
the recency update should not allocate a new `String` — bump a counter, flip a
bit, or relink an existing node instead of `push_back(key.to_owned())`.

**Tradeoff — strict vs approximate LRU.** Strict LRU's exact-recency ordering
is rarely worth its bookkeeping for a *plan cache*: the working set of hot query
shapes is small and stable, so any reasonable policy (LRU-ish, CLOCK, 2-random)
keeps the same handful of plans resident. An approximate policy with O(1)
alloc-free touches is the better fit; pay for strict ordering only if a
benchmark shows eviction quality actually matters at `cap = 256`.

## Implementation sketch (code locations, approach, risks)

Primary file: `src/lib.rs` (the `PlanCache` impl, `src/lib.rs:84`–`125`, plus
`run_cached` at `src/lib.rs:210`). Supporting: `src/prepared.rs` for (a).

Suggested staging:
1. **(a) first, in isolation.** Change `plans: HashMap<String, Rc<PreparedStatement>>`;
   have `prepare` return / wrap an `Rc`, and `get` return `Option<Rc<PreparedStatement>>`.
   `execute_prepared(&stmt, ...)` already borrows, so callers just `&*stmt`.
   Removes the deep clone with no policy change. Verify against the existing
   `query_with_caches_by_sql` test (`src/lib.rs:556`).
2. **(b) + (d) together.** Either pull in `lru` and replace `plans`/`order`
   wholesale, or keep the `HashMap` and add a monotonic-tick value to drop the
   `order` VecDeque and `detach_order` entirely. Keep `remove` (catalog-stale
   eviction, `src/lib.rs:114`) working.
3. **(c)** as a one-line hasher swap, measured separately.

Risks: `order`/`plans` lockstep invariant (`src/lib.rs:101`) must be preserved
by any new structure; the stale-plan eviction path in `run_cached`
(`src/lib.rs:219`) must still remove cleanly. The `Rc<PreparedStatement>` change
touches the public-ish `prepare`/`execute_prepared` seam — keep the external
signatures borrow-based to avoid churn. Adding `ahash`/`lru`/`rustc-hash` adds a
dependency; the tick-counter and hand-rolled options avoid that.

## References

- `src/lib.rs:70` — `PLAN_CACHE_CAP = 256`
- `src/lib.rs:77` — `struct PlanCache { plans, order, cap }`
- `src/lib.rs:92` — `PlanCache::get` (clone + detach + push_back)
- `src/lib.rs:120` — `detach_order` (O(cap) linear scan)
- `src/lib.rs:210` — `run_cached` (hit/miss + stale eviction)
- `src/prepared.rs:21` — `PreparedStatement` fields (`sql`, `program: Rc`, `required_params`)
- Related: keyed-table get (`project_get`), flat value encoding (`project_flat_value_encoding`) — the other N-independent slices of point-lookup latency.
