# Prepared lookup path: skip per-query `normalize()`

**Status:** proposed · **Area:** lookup hot path / parse & plan cache

## Finding

`MonaDB::query` (`src/lib.rs:190`) routes every ad-hoc SQL string through
`MonaDB::normalize` (`src/lib.rs:255`) to build an auto-parameterized cache key
before consulting the plan cache via `run_cached` (`src/lib.rs:210`).

`normalize` runs the logos `SqlLexer` over the **entire** SQL string on every
call, allocates a fresh template `String` (`with_capacity(sql.len())`) and a
`Vec<Value>`, copies inter-token text verbatim, and replaces each numeric
literal with `?`. Only then is the template hashed and looked up.

So even on a guaranteed cache **hit** — the same point-lookup shape fired in a
loop — each call pays a full lex pass, a template `String` build, a `Vec`
build, and a hash, purely to compute the cache key. The `btree.get` it guards
is a small fraction of total latency. A first-class prepared path already
sidesteps all of this: `prepare` + `execute_prepared` (`src/prepared.rs`)
compile once and reuse, and `query_with` (`src/lib.rs:297`) caches by *raw* SQL
(no `normalize`). The benchmark deliberately drives ad-hoc `db.query`
(`benches/monadb.rs:46-53`) to model "ad-hoc SQL", but a real hot point-lookup
workload would use a prepared statement — exactly as SQLite callers do.

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
`execute_prepared` with bound key params — zero `normalize`, one cached
program. This is the honest apples-to-apples baseline, since SQLite point
lookups go through prepared statements too. Cheap to act on: the benchmark could
add an optional prepared variant (`prepare` + `execute_prepared`, key passed as
a bound param) to show the ceiling and isolate `normalize` from `btree.get`.
Tradeoff: doesn't speed up callers who *stay* on ad-hoc `query`.

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

**(d) Surface `prepare` in the embedded/Python API.** A `prepare`-returning
handle so callers opt into zero-`normalize` execution explicitly (the
`PreparedStatement` is already `Clone` + `Rc`-shared). Tradeoff: new public
surface to design and keep stable across backends.

**Correctness constraints (from `normalize`'s doc comment, `src/lib.rs:238-254`).**
Any cheaper key must preserve current semantics: **string literals are NOT
parameterized** (a string in a `FROM` source lowers to a file scan on the
literal at parse time — `looks_like_file`), and **`LIMIT` operands are NOT
parameterized** (they parse as compile-time counts, so a `?` there fails to
parse). An explicit `?`/`$N`/`$name` placeholder, or a lex error, must still bail
to the direct uncached path. Option (c)'s raw-SQL tier is automatically safe
here (it preserves the literal text); (b)'s pre-scan must keep the same bail
conditions.

## Implementation sketch (code locations, approach, risks)

- **(a)/(d):** add a prepared variant to `benches/monadb.rs` `select_single` /
  `select_composite` (call `prepare` in `create_table`/`open`, store the
  `PreparedStatement`, bind the key via `Params::positional` in
  `execute_prepared`). No engine change — measures the `normalize`-free ceiling.
- **(b):** rework `MonaDB::normalize` (`src/lib.rs:255`) — single lex pass that
  records whether any numeric (non-`LIMIT`) token was seen; if none, return the
  raw `sql` as the key with an empty `Vec` and skip the `String` build. Keep the
  existing bail set (`Question`/`NumberedParam`/`NamedParam`/lex-error). Reuse a
  thread-local/`self`-owned scratch `String` to avoid the per-call allocation.
- **(c):** add a raw-SQL-keyed fast tier consulted at the top of `query`
  (`src/lib.rs:190`) before `normalize`, reusing the existing
  catalog-generation staleness eviction in `run_cached` (`src/lib.rs:219`).

**Risks.** (b) and (c) touch the cache-key contract — the falling-back behaviour
in `query` (`src/lib.rs:197-204`) and the `StalePreparedStatement` eviction must
stay intact; add coverage that a literal-in-non-expr-position still falls back,
and that string/`LIMIT` literals are never parameterized. (a)/(d) carry no
engine risk but expand the bench/API surface.

## References

- `src/lib.rs:190` — `MonaDB::query` (ad-hoc entry, calls `normalize`)
- `src/lib.rs:210` — `run_cached` (plan-cache hit/miss + stale eviction)
- `src/lib.rs:238-289` — `normalize` (lex + template + `Vec` build; doc comment
  states the string-literal / `LIMIT` non-parameterization constraints)
- `src/lib.rs:297` — `query_with` (caches by raw SQL, no `normalize`)
- `src/prepared.rs` — `prepare`, `execute_prepared`, `PreparedStatement`
- `benches/monadb.rs:46-53` — ad-hoc `db.query` point/composite lookups
- `benches/REPORT.md:57-67,133` — `xs`/`sm` point-lookup ratios; fixed per-op
  overhead attribution
- Related: `docs/plans/` (this directory) holds saved-but-not-yet-implemented
  perf plans
