# Caching Plans

Iterations...

1. No cached plans
2. Naive cached plans
3. Parameterized cached plans
4. LRU HashMap + Doubly-Linked List
5. LRU FxHashMap + ticker (5x faster lookup)
6. Open btrees once (VM-loop optimization)
7. Dumb string->plan cache

## Results

These results show cache get is 5x faster (200ns -> 40ns) with no scan or string allocations, only an Rc bump.

```
cache          fastest    │ median
├─ get_old     207 ns     │ 208 ns     ← clone + O(256) scan + String alloc
├─ get_new      40 ns     │  40 ns     ← Rc bump + u64 write (5× faster)
├─ put_old      10 µs     │  13 µs     ← detach scan + insert + evict
╰─ put_new      11 µs     │  13 µs     ← insert + evict (same — eviction cost dominates)
```

I was able to shave ~250ns off point lookups by caching btree opens which LMDB only requires
to be done once. The hot path now references btrees by a slot index instead of opening a new
handle which is idempotent on alredy-opened btress thus unecessary.

```
point_lookup median 1.416 µs → 1.165 µs prepared.
```

Next step was to perform normalization during the lex phase; skipping both parsing and
an additional normalization pass which had string allocations. This got me ~25% speedup
and removes a string allocation.

```
Normalization — String template vs lex u64 hash+extract (cargo bench --bench normalize)

┌────────────────────────────┬───────────┬──────────┬───────┐
│            SQL             │ normalize │ lex_hash │   Δ   │
├────────────────────────────┼───────────┼──────────┼───────┤
│ select t[123];             │ 150.5 ns  │ 131.0 ns │ 0.87× │
├────────────────────────────┼───────────┼──────────┼───────┤
│ select docs["t042", 5700]; │ 231.2 ns  │ 179.1 ns │ 0.77× │
├────────────────────────────┼───────────┼──────────┼───────┤
│ select * from t limit 10;  │ 179.1 ns  │ 124.5 ns │ 0.70× │
└────────────────────────────┴───────────┴──────────┴───────┘
```

I also perform semantic hashing during the lex pass rather than hashing the string
template during lookup. This changes the cache key from a `String` to u64 which 
surprisingly didn't change lookup performance, but improved inserts by ~27% (-4us/4000ns)
since it removes a string clone.

```
Cache key — String vs u64 (cargo bench --bench cache)

┌─────┬──────────┬──────────────────────────────────────────────────────────┐
│     │  String  │                           u64                            │
├─────┼──────────┼──────────────────────────────────────────────────────────┤
│ get │ 40.55 ns │ 40.55 ns (both at timer floor — u64 not slower)          │
├─────┼──────────┼──────────────────────────────────────────────────────────┤
│ put │ 14.12 µs │ 10.31 µs (~27% faster — no String clone on insert/evict) │
└─────┴──────────┴──────────────────────────────────────────────────────────┘
```

These changes improved non-prepared statements by ~15% (150ns) because we removed
the normalization pass, dropped an extra string alloc, and now hash a little u64
instead of the query string template (>10 bytes with a memcmp).


```
End-to-end benchmarks of before (normalization pass) to after (lex normalization)

  ┌────────────────────────┬─────────┬─────────┬──────────────────────────────────────────────────────┐
  │         metric         │ before  │  after  │                         note                         │
  ├────────────────────────┼─────────┼─────────┼──────────────────────────────────────────────────────┤
  │ point_lookup fastest   │ 915 ns  │ 791 ns  │ ~124 ns (~14%); back-to-back A/B, same machine state │
  ├────────────────────────┼─────────┼─────────┼──────────────────────────────────────────────────────┤
  │ point_lookup median    │ ~960 ns │ 834 ns  │ ~130 ns (~13%); branch stable ×3, main 958–1124 ns   │
  ├────────────────────────┼─────────┼─────────┼──────────────────────────────────────────────────────┤
  │ query_with_hit fastest │ ~665 ns │ ~665 ns │ within noise (tiny SQL, key cost already negligible) │
  ├────────────────────────┼─────────┼─────────┼──────────────────────────────────────────────────────┤
  │ adhoc allocs/op        │ 12.0    │ 11.0    │ −1: String template key removed                      │
  ├────────────────────────┼─────────┼─────────┼──────────────────────────────────────────────────────┤
  │ prepared allocs/op     │ 9.0     │ 9.0     │ unchanged (hash_str keys raw bytes, no lex)          │
  └────────────────────────┴─────────┴─────────┴──────────────────────────────────────────────────────┘
```

I thought I was clever by removing the normalization pass and doing the same work
during lexing -- one less pass right? Turns out a semantic hash is an ~80ns penalty
at best. Without templates and semantic hash, adhoc queries are way faster. The actual
clever work I learned from this was automatically preparing statements and using
them for customer using the python API. Best UX, customer never knows what a "prepared"
statement is, yet they get to benefit from them on every query.

```
strategy            derive    hit (derive+get)   miss     (keying work)
1  lex → u64        101 ns     82 ns             750 ns   (lexes twice on a miss)
2  lex+parse → u64  786 ns    708 ns             651 ns   (parse on every lookup)
3  raw string        ~0 ns*     7 ns             651 ns   (no lex, no parse)
```

Turns out doing raw-string lookup is 100x faster than a semantic hash on the AST
and 10x faster than a semantic lex hash. In the spirit of keeping things simple,
the advice is to "prepare" your statement to make it fast.
