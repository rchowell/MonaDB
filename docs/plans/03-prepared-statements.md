# Prepared lookup path: skip per-query `normalize()`

**Status:** partially implemented · **Area:** lookup hot path / parse & plan cache

## Progress

The first-class prepared API — option **(d)** — has **landed**:

- `MonaDB::prepare` / `MonaDB::prepare_cached` return a `Statement<'db>` handle
  (`src/prepared.rs:69,75`); `Statement::query` / `Statement::execute` take only
  bound params (`src/prepared.rs:56,62`). The plan is `Rc`-shared
  (`StatementPlan`, `src/prepared.rs:19`), so `prepare_cached` reuses one
  allocation across calls.
- Parameter binding goes through `Params` / `IntoParams` (`src/params.rs`):
  positional, named, and the `?`/`$N`/`$name` placeholder lowering. `query_with`
  / `execute_with` (`src/lib.rs:292,311`) run parameterized SQL cached by *raw*
  SQL (no `normalize`).
- The Python surface mirrors it: `Connection.prepare` → `Statement` with
  `query`/`execute`/`sql` (`monadb/connection.py:11,86`, `src/python.rs:271`).
  Covered by `monadb/tests/test_prepare.py` and `tests/prepare_api.rs`.

**Still open:** the perf work for the ad-hoc `query` path — **(b)** cheaper
`normalize`, **(c)** a raw-SQL fast tier — and the flagship comparison
benchmark **(a)**. None of these has been started. The core finding below (fixed
per-op `normalize` overhead on cache hits) is therefore *avoidable* today by
migrating to `prepare` / `prepare_cached` / `query_with`, but **not yet fixed**
for callers who stay on `db.query`.

## Finding

`MonaDB::query` (`src/lib.rs:177`) routes every ad-hoc SQL string through
`MonaDB::normalize` (`src/lib.rs:250`) to build an auto-parameterized cache key
before consulting the plan cache via `run_cached` (`src/lib.rs:200`).

`normalize` runs the logos `SqlLexer` over the **entire** SQL string on every
call, allocates a fresh template `String` (`with_capacity(sql.len())`) and a
`Vec<Value>`, copies inter-token text verbatim, and replaces each numeric
literal with `?`. Only then is the template hashed and looked up.

So even on a guaranteed cache **hit** — the same point-lookup shape fired in a
loop — each call pays a full lex pass, a template `String` build, a `Vec`
build, and a hash, purely to compute the cache key. The `btree.get` it guards
is a small fraction of total latency. A first-class prepared path already
sidesteps all of this: `prepare` + `Statement::query` (`src/prepared.rs`)
compile once and reuse, and `query_with` (`src/lib.rs:292`) caches by *raw* SQL
(no `normalize`). The benchmark deliberately drives ad-hoc `db.query`
(`benches/monadb.rs:54-62`, via `drain_one`) to model "ad-hoc SQL", but a real
hot point-lookup workload would use a prepared statement — exactly as SQLite
callers do.

## Impact

The lex + template + hash cost is **N-independent fixed per-op overhead**: it
does not scale with document size, so it dominates precisely where the document
is small and the actual read is cheapest. That is where MonaDB trails SQLite
most: point lookups at `xs`/`sm` run 1.19×/1.72× (single) and 1.42×/1.06×
(composite) in `benches/REPORT.md:57-67`. The REPORT already attributes the
residual `xs`/`sm` gap to "fixed per-op overhead" (`REPORT.md:133`). For a known
repeated shape, that overhead is wasted work on the critical path.

## Brainstorming (options & techniques, with tradeoffs)

**(a) Lean on / promote the prepared API for hot paths.** `prepare` once, then
`Statement::query` with bound key params — zero `normalize`, one cached
program. This is the honest apples-to-apples baseline, since SQLite point
lookups go through prepared statements too. **Still open:** the benchmark only
drives ad-hoc `db.query`; adding a prepared variant (`prepare` in
`create_table`/`open`, key bound via `Params::positional` in `Statement::query`)
would show the ceiling and isolate `normalize` from `btree.get`. Tradeoff:
doesn't speed up callers who *stay* on ad-hoc `query`.

**(b) Make `normalize` cheaper.** A fast pre-scan that detects "no numeric
literals to extract" and bails to using the raw SQL as the cache key — skipping
the template `String` build and the `Vec` entirely (most lookups that bind keys
as literals still have numbers, so pair this with scratch-buffer reuse and
dropping the second allocation when `vals` is empty). Tradeoff: still a full lex
pass per call; helps constant factors, not asymptotics.

**(c) Tiny direct cache keyed by raw SQL.** For *exactly*-repeated statement
text (the dominant case in a tight loop), a small first-level map from raw SQL
hash → plan, checked before `normalize`. A hit skips lex+template+`Vec`
entirely. Tradeoff: a second cache tier to keep coherent with the
template-keyed cache and catalog-generation invalidation; raw-SQL keys don't
collapse differing-literal shapes, so it complements rather than replaces (b).

**(d) Surface `prepare` in the embedded/Python API. ✅ Done.** `MonaDB::prepare`
/ `prepare_cached` return a `Statement<'db>` handle (`src/prepared.rs`), and the
Python `Connection.prepare` → `Statement` mirror (`monadb/connection.py`,
`src/python.rs`) lets callers opt into zero-`normalize` execution explicitly. The
`StatementPlan` is `Clone` + `Rc`-shared. The remaining item from this option is
documentation: `site/content/python.md` has no `prepare` / `Statement` section
yet.

**Correctness constraints (from `normalize`'s doc comment, `src/lib.rs:233-249`).**
Any cheaper key must preserve current semantics: **string literals are NOT
parameterized** (a string in a `FROM` source lowers to a file scan on the
literal at parse time — `looks_like_file`), and **`LIMIT` operands are NOT
parameterized** (they parse as compile-time counts, so a `?` there fails to
parse). An explicit `?`/`$N`/`$name` placeholder, or a lex error, must still bail
to the direct uncached path. Option (c)'s raw-SQL tier is automatically safe
here (it preserves the literal text); (b)'s pre-scan must keep the same bail
conditions.

## Implementation sketch (code locations, approach, risks)

- **(a) — not yet done:** add a prepared variant to `benches/monadb.rs`
  `select_single` / `select_composite` (call `prepare` in `create_table`/`open`,
  store the `Statement`/`StatementPlan`, bind the key via `Params::positional` in
  `Statement::query`). No engine change — measures the `normalize`-free ceiling;
  then refresh `benches/REPORT.md` with the prepared-vs-ad-hoc comparison.
- **(b) — not yet done:** rework `MonaDB::normalize` (`src/lib.rs:250`) — single
  lex pass that records whether any numeric (non-`LIMIT`) token was seen; if none,
  return the raw `sql` as the key with an empty `Vec` and skip the `String` build.
  Keep the existing bail set (`Question`/`NumberedParam`/`NamedParam`/lex-error).
  Reuse a thread-local/`self`-owned scratch `String` to avoid the per-call
  allocation.
- **(c) — not yet done:** add a raw-SQL-keyed fast tier consulted at the top of
  `query` (`src/lib.rs:177`) before `normalize`, reusing the existing
  catalog-generation staleness eviction in `run_cached` (`src/lib.rs:214`).
- **(d) — done.** See Progress. Remaining: a `prepare` / `Statement` section in
  `site/content/python.md`.

**Risks.** (b) and (c) touch the cache-key contract — the falling-back behaviour
in `query` (`src/lib.rs:187-194`) and the `StalePreparedStatement` eviction must
stay intact; add coverage that a literal-in-non-expr-position still falls back,
and that string/`LIMIT` literals are never parameterized. (a) carries no engine
risk but expands the bench surface.

## References

- `src/lib.rs:177` — `MonaDB::query` (ad-hoc entry, calls `normalize`)
- `src/lib.rs:200` — `run_cached` (plan-cache hit/miss + stale eviction)
- `src/lib.rs:233-284` — `normalize` (lex + template + `Vec` build; doc comment
  states the string-literal / `LIMIT` non-parameterization constraints)
- `src/lib.rs:292` — `query_with` (caches by raw SQL, no `normalize`); `311` —
  `execute_with`
- `src/prepared.rs:69,75` — `prepare`, `prepare_cached`; `:56,62` —
  `Statement::query` / `Statement::execute`; `:19` — `StatementPlan` (`Rc`-shared)
- `src/params.rs` — `Params` / `IntoParams` (positional, named, placeholder
  binding)
- `monadb/connection.py:11,86` · `src/python.rs:271` — Python `Connection.prepare`
  → `Statement`; tests in `monadb/tests/test_prepare.py`, `tests/prepare_api.rs`
- `benches/monadb.rs:54-62` — ad-hoc `db.query` point/composite lookups (no
  prepared variant yet — option (a))
- `benches/REPORT.md:57-67,133` — `xs`/`sm` point-lookup ratios; fixed per-op
  overhead attribution
- `site/content/python.md` — Python guide (no `prepare` / `Statement` section yet)
- Related: `docs/plans/` (this directory) holds saved-but-not-yet-implemented
  perf plans
