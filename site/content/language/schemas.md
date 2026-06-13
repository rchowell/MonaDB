+++
title = "Schemas"
description = "Table declarations, key columns, composite keys, and keyed storage for ordered scans and lookups."
weight = 4
+++

A table's schema is declared with `create table`. The declaration names the table and, optionally, an ordered list of **key columns**. Key columns define how rows are indexed in the underlying B+ tree: the physical key is an order-preserving encoding of those columns, so full-table scans, prefix scans, and point lookups all respect the same sort order.

```
create table <name> [(<key> int|string, …)];
drop table <name>;
clear table <name>;
```

Key columns are the only schema MonaDB enforces at insert time. Payload fields beyond the declared keys are stored as part of the row object but are not part of the index definition.

## Key columns

Each key column is a name followed by a type keyword. Only `int` (integer key component) and `string` (string key component) are accepted:

```
create table users (id int);
create table pages (slug string);
create table events (tenant string, user string, ts int);
```

The parser recognises additional type keywords (`bool`, `float`, `number`, `object`, `array`, `any`), but using any of them in a key-column position is rejected at compile time.

When key columns are declared, every insert must supply each key field with the correct type. Missing or mistyped keys are schema errors. Extra payload fields are allowed — the row object round-trips whole, keyed by the declared columns:

```
create table t (x int);
insert into t ({ x: 1, note: 'ok' });   -- ok
insert into t ({ note: 'no key' });     -- schema error
insert into t ({ x: 'a' });             -- schema error
```

Re-inserting the same key overwrites the previous row (last write wins).

## Keyless tables

A table declared without key columns accepts any JSON object (or scalar) on insert. No field-level schema checking is performed:

```
create table t;
create table t ();     -- equivalent
```

Keyless tables assign each row a surrogate id. Rows return in **insertion order**, not sorted by any field in the payload. This is appropriate for append-only logs, scratch buffers, and tables where order does not matter.

Keyed tables, by contrast, sort rows by their encoded composite key. Inserts arrive in any order; scans and lookups see key order.

## Composite keys

Multiple key columns form a **composite key** evaluated left to right, like a tuple. The first column is the primary sort key; ties break on the second column, then the third, and so on:

```
create table c (a string, b int);
insert into c ({a: "x", b: 2}, {a: "x", b: 1}, {a: "y", b: 9});
select * from c;
-- → {a: "x", b: 1}, {a: "x", b: 2}, {a: "y", b: 9}
```

Integers sort numerically (negative values sort before zero and positives). Strings sort lexicographically in UTF-8 byte order. A shorter string sorts before any longer string that shares its prefix — `"ab"` sorts before `"abc"`.

Choose column order deliberately. `(tenant string, ts int)` groups all rows for one tenant contiguously and sorts them by timestamp within that group. `(ts int, tenant string)` would sort primarily by time across all tenants. The declaration order is the storage order.

## Physical ordering and ranged reads

MonaDB encodes each key column into a byte string so that lexicographic byte order matches logical sort order. Concatenating those components in declaration order yields the physical key stored in LMDB.

That encoding is what makes **ranged reads** efficient. Every prefix of the composite key corresponds to a contiguous slice of the B+ tree:

| Leading key prefix | What sits contiguously in storage |
|--------------------|-----------------------------------|
| _(none — full table)_ | All rows in the table |
| `tenant = "acme"` | All rows whose first key column is `"acme"` |
| `tenant = "acme", user = "alice"` | All rows for that tenant–user pair |
| Full key `(tenant, user, ts)` | At most one row (point lookup) |

A `select * from events` over a keyed table walks rows in key order without an explicit `order by`. When the query's `where` clause matches leading key columns with equalities or range comparisons, the engine can bound the cursor to the matching byte range instead of scanning the whole table.

Design keys so the access patterns you care about align with leading prefixes. Equality filters on early columns narrow the range; range filters on the next column walk a sub-sequence in sort order; trailing columns that are not constrained still sort within each prefix group.

## Point and prefix lookup

When the receiver of `[…]` resolves to a catalog table with declared key columns, subscripting is a **keyed lookup**, not ordinary path navigation into a row object. See [Expressions](@/language/expressions.md) for the full syntax rules; the behaviour follows directly from the key layout above.

```
create table t (id int);
select t[1];              -- whole row, or null if absent
select t[1].v;            -- field of the looked-up row

create table c (a string, b int);
select c["x", 7];         -- full composite key → one row or null
select c["x"];            -- leading-prefix key → matching rows as an array, in key order
```

A **full key** with the same arity as the table's key columns returns the one matching row, or `null` when no row exists. A **partial key** — fewer subscript elements than key columns — returns the sub-sequence of matching rows as an array, sorted by the remaining key components. A partial key that matches nothing yields an empty array.

Partial-key lookup is the query-language face of prefix ranging: `c["x"]` is equivalent in spirit to scanning every row whose first key column equals `"x"`, without writing a `where` clause. Nested paths compose — `c["x"][0].v` indexes into the prefix result.

On a row binding (not a table name), multi-element subscripts are not supported. Keyless tables cannot be indexed by key.

## Choosing keyed vs keyless

| | Keyless | Keyed |
|---|---------|-------|
| Insert constraints | Any JSON value | Key fields required, typed |
| Row order on scan | Insertion order | Key order |
| Point lookup by key | Not supported | `table[key]` |
| Prefix / range scans | Full table scan | Contiguous B+ tree ranges |
| Overwrite semantics | Each insert adds a row | Same key replaces the row |

Use a keyless table when you need a flexible document bag or strict append order. Declare key columns when rows have a natural identifier, when you need idempotent upserts by key, or when queries repeatedly filter or sort on the same leading fields — the storage layout will match those access patterns.

See the [keys examples](/examples/keys/) for insert validation and sort-order cases, and [keyed lookup examples](/examples/get/) for point and prefix access patterns.
