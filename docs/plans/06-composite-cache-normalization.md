# Composite-key lookups under-cache: string key literals aren't parameterized

**Status:** proposed · **Area:** composite-key lookups / plan cache normalization

## Finding

`MonaDB::normalize` (`src/lib.rs:255`) builds the plan-cache key by a single
pure-token-stream pass: it parameterizes **numeric** literals in expression
position into `?` and copies everything else — including **string** literals —
verbatim. This is intentional. Its doc comment notes that a string in a `FROM`
source is lowered to a file scan based on the *literal* at parse time (via
`looks_like_file`), which a `?` would silently defeat, and that `LIMIT` operands
parse as compile-time counts (not expressions) so a `?` there fails to parse.
Both constraints push the allow-list to "numbers only."

The fallout lands on keyed lookups. A composite point lookup
`select docs["t007", 42];` normalizes to the template
`select docs["t007", ?];` — the tenant string `"t007"` stays embedded. So each
distinct tenant string is its **own** template: with 100 tenants you get up to
100 distinct cache entries (and at `PLAN_CACHE_CAP = 256` they thrash against
every other live shape). `docs["t007", 42]` and `docs["t999", 7]` *should*
share one plan — the compiled program is identical save for the key bytes — but
they do not.

This matches a previously documented finding: composite reads under-cache
because `normalize` keeps strings verbatim → ~100 templates; the scoped fix is
AST-based, position-aware normalization.

## Impact

Composite-key point lookups are MonaDB's weakest read workload relative to
SQLite (REPORT composite xs/sm **1.77–1.86×** slower). Empirical measurement
this investigation (xs profile, single-sample harness; alloc counts are
deterministic):

| workload                  | allocs/op | vs single-key |
|---------------------------|-----------|---------------|
| single-key point lookup   | ~956      | 1.0×          |
| composite point lookup    | ~4360     | ~4.5×         |

The ~3400 extra allocs/op is roughly **one full extra parse + bind + compile per
op** — the signature of a cache *miss* and recompile on (nearly) every call.
Composite lookups also run ~1.8× slower than single-key and ~1.8× slower than
SQLite, tracking the alloc multiplier. The recompilation churn — not the
composite key encoding itself — is the likely dominant cause of the gap.

## Brainstorming (options & techniques, with tradeoffs)

**(a) AST-/position-aware normalization (parameterize string KEY literals).**
Parameterize string literals that sit in **subscript/key position**
(`table[... here ...]`) while still leaving `FROM`-source strings and `LIMIT`
operands literal. Then `docs["t007", 42]` and `docs["t999", 7]` collapse to one
template `docs[?, ?]` and share a plan. The catch: the current pass is a pure
token stream with no nesting context, so it can't tell a key-position string
from a `FROM` string. Two ways to get the context:
- *Light structural tracking in the token loop* — count `[`/`]` depth (subscript
  depth) and the `from` keyword state, parameterizing string literals only when
  inside a subscript and outside a `FROM` source. Cheap, no full parse, but it
  reintroduces ad-hoc parsing into `normalize` and is fragile around nested
  subscripts / strings used as object keys.
- *Normalize after a light parse* — parse once, walk the IR, and template from
  `Expr::Get` key args (which the binder already isolates). Robust and exact, but
  pays a parse on the normalization path — the very cost `normalize` was written
  to avoid. Could be gated so it only runs when a `[` is present.

**(b) Invert the allow-list.** Parameterize string literals *everywhere except*
the few syntactic positions that must stay literal (`FROM` source, `LIMIT`).
Same context problem as (a) but framed as a deny-list; correctness still hinges
on reliably detecting those positions in a token stream. Broader blast radius
(every string literal, not just key-position ones) for marginal extra hit rate.

**(c) Plan-cache capacity / key changes.** Orthogonal mitigation: raise
`PLAN_CACHE_CAP` or give composite shapes a separate/larger bucket so 100
tenant-specialized templates don't evict each other or unrelated shapes. Doesn't
fix the root cause (you still compile 100 near-identical plans), but caps the
thrash. Cheapest to ship; weakest fix.

**(d) Cut per-key encoding cost (independent win).** `encode_str`
(`src/schema.rs:30`) and `encode_key_tuple` (`src/schema.rs:71`) each allocate a
fresh `Vec` per key. Encoding into a reused scratch buffer removes those allocs
on every lookup regardless of cache behavior. This shaves the *floor* even once
(a) lands — but it is a small slice next to the ~3400-alloc recompile, so it is a
follow-up, not the headline.

**Tradeoff — parse vs token stream.** `normalize` deliberately avoids a full
parse for speed; (a)-structural and (b) preserve that but re-grow an informal
parser inside the lexer loop. (a)-light-parse is the correct, maintainable shape
but moves a parse onto the hot normalization path. The pragmatic middle:
token-stream subscript-depth tracking, gated to only engage when a `[` appears,
so single-key and aggregate queries pay nothing.

**Correctness constraints (must hold for any option):** `FROM`-source strings
must stay literal (file-scan lowering via `looks_like_file`); `LIMIT` operands
must stay literal (compile-time counts, `?` won't parse); explicit
`?`/`$N`/`$name` still forces the uncached fallback (`src/lib.rs:276`); and any
newly-parameterized string must round-trip to an identical `Expr::Lit` so the
extracted value binds back correctly.

## Implementation sketch (code locations, approach, risks)

Primary file: `src/lib.rs` — `normalize` (`src/lib.rs:255`–`289`) and the
`run_cached` hit/miss path it feeds. Supporting: `src/compiler.rs`
(`cc_expr_get` `src/compiler.rs:1369`, `emit_key_tuple` `src/compiler.rs:1389`,
`all_literal_keys` `src/compiler.rs:1405`) and `src/schema.rs` for (d).

Suggested staging:
1. **(a), token-stream variant first.** Add `subscript_depth` and a `from`-state
   flag to the `normalize` loop; parameterize `Token::Str` only when
   `subscript_depth > 0` and not in a `FROM` source, pushing the decoded string
   `Value` and emitting `?`. Gate the whole branch behind "the SQL contains `[`"
   so non-keyed queries are untouched. Verify the existing
   `query_with_caches_by_sql`-style cache tests plus a new one asserting
   `docs["t007",42]` and `docs["t999",7]` hit the same entry.
2. **Confirm the compiler already handles it.** `emit_key_tuple` /
   `all_literal_keys` already split literal-vs-parameter keys: with the string
   now arriving as a bound `?` param, `all_literal_keys` returns `None` and the
   key encodes at RUN time via `EncodeKeyTuple` — exactly the parameterized
   keyed-get path. No compiler change should be needed; the type-mismatch error
   that compile-time encoding raised (`t["a"]` on an int key) now surfaces at
   run time instead, which is acceptable and consistent with numeric params.
3. **(d) as a separate measured change** in `src/schema.rs`.

Risks: the subscript-depth heuristic must not misclassify a string used as an
object key or a string literal that legitimately must stay literal; nested
subscripts and `FROM` sources containing brackets are the edge cases to test.
Moving key-type validation from compile time to run time (step 2) is a small
behavior shift — keep a test that `docs["a"]` on an int key still errors,
just later. The `[`-gate keeps the common path zero-cost.

## References

- `src/lib.rs:255` — `MonaDB::normalize` (numeric-only, string-verbatim pass)
- `src/lib.rs:247` — doc note: `FROM`-source strings → file scan (`looks_like_file`)
- `src/lib.rs:259` — `LIMIT`-operand literal handling
- `src/lib.rs:276` — explicit `?`/`$N`/`$name` → uncached fallback
- `src/compiler.rs:1369` — `cc_expr_get` (point vs range keyed access)
- `src/compiler.rs:1389` — `emit_key_tuple` (compile-time vs run-time key encode)
- `src/compiler.rs:1405` — `all_literal_keys` (literal-key detection)
- `src/schema.rs:30` — `encode_str` (per-key `Vec` alloc)
- `src/schema.rs:71` — `encode_key_tuple` (per-key `Vec` alloc)
- Related: `02-plan-cache-get-overhead.md` (N-independent cache-path overhead),
  `03-prepared-lookup-skip-normalize.md` (skipping normalize on the prepared path),
  prior memory finding *composite cache finding* (AST-based position-aware fix).
