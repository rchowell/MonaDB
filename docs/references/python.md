# MonaDB Python Embedded API

## Overview

The MonaDB Python API exposes a single embedded table abstraction with two
complementary access surfaces over the same LMDB-backed storage:

- A **dict-like surface** for point and range access on the primary key.
- A **SQL-like surface** for predicate-driven reads and writes.

The two surfaces are not layered conveniences over one another — they are peers.
Dict operations are always point or range operations on the primary key. SQL
operations express arbitrary predicates over document contents. Users reach for
whichever fits the access pattern they have.

There is no embedded DSL. Predicates are SQL string fragments with positional
`?` parameters. MonaDB already speaks a SQL dialect; the Python API is a thin
window into it, not a second query language built in Python.

---

## Connection and Table Handles

```python
conn = monadb.connect()
items = conn.table("items", {"x": int})
```

`conn.table(name, schema)` returns a `Table`. The schema is an open-schema type
hint: declared fields carry type/NOT NULL enforcement; undeclared fields pass
through freely.

---

## Table

`Table` subclasses `collections.abc.MutableMapping[K, V]`. Implementing the five
abstract methods yields the full dict protocol (`keys`, `values`, `items`,
`get`, `__contains__`, `update`, `pop`, ...) via the mixin.

### Dict-like surface

```python
items[1] = {"x": 1, "y": 2}   # __setitem__  — upsert at caller-supplied key
items[1]                      # __getitem__  — point lookup, KeyError if absent
items[1:4]                    # __getitem__  — range scan, returns Rows
del items[1]                  # __delitem__  — KeyError if absent

for k in items:               # __iter__     — yields keys
    ...
items.keys()                  # keys
items.values()                # values  (overridden: single cursor scan)
items.items()                 # (key, value) pairs  (overridden: single scan)
len(items)                    # __len__      — COUNT(*)
1 in items                    # __contains__ (overridden: point lookup)
```

**Slices are range queries, not positional.** `items[1:4]` means keys in
`[1, 4)`, mapping to an LMDB range scan over the sort key. This is a documented
extension of the mapping contract — `dict` rejects slices; `Table` accepts them.
`__getitem__` inspects its argument: a `slice` returns a `Rows` handle; anything
else is a point lookup. The `step` field selects scan direction
(`items[4:1:-1]` is a reverse range scan); other step values raise
`NotImplementedError`.

**Mixin overrides for performance.** The default mixin implements `values()`,
`items()`, and `__contains__` in terms of repeated `__getitem__` (N point
lookups). `Table` overrides all three to run a single cursor scan.

### SQL-like surface

```python
items.select(predicate=None, params=None) -> Rows
items.insert(doc) -> K                          # generated key, DuplicateKeyError on collision
items.update(patch, predicate=None, params=None) -> int   # rows affected
items.delete(predicate=None, params=None) -> int          # rows deleted
items.count(predicate=None, params=None) -> int
```

```python
items.select("x > ?", [3])                 # SELECT * FROM items WHERE x > 3
items.insert({"x": 9, "y": 0})             # returns generated key
items.update({"y": 99}, "x > ?", [3])      # patch merge into matching docs
items.delete("x < ?", [0])                 # returns count deleted
items.count("x > ?", [3])
```

`predicate` is a SQL `WHERE` fragment; `params` binds its `?` placeholders.
A `None` predicate means "all rows" and must be passed explicitly for
`update`/`delete` (no silent whole-table mutation).

`insert` and `__setitem__` are distinct operations, deliberately not unified:

- `__setitem__` is an **upsert** — the caller supplies the key.
- `insert` is for **generated keys** — MonaDB shreds/derives the key and
  returns it.

`update`'s `patch` is either a dict (shallow-merged into each matching document)
or a callable `V -> V` (full document transform).

---

## Rows

`select` and key-slice access return a `Rows` handle: a lazy, iterable query
that has not necessarily executed yet. It holds the SQL string and bound params —
not an open cursor — so it is **reusable**: each materialization re-executes
against LMDB. Iterating it twice yields consistent results rather than a silent
empty second pass.

```python
class Rows:
    def __init__(self, conn, sql: str, params: list): ...

    def __iter__(self) -> Iterator[dict]: ...   # lazy; one row at a time
    def __len__(self) -> int: ...               # COUNT(*) over the query
    def fetchone(self) -> dict | None: ...
    def fetchall(self) -> list[dict]: ...
```

The surface is intentionally minimal: iteration, length, and the two DB-API
fetch verbs. No chaining, no materialization-format methods, no DSL.

```python
for item in items.select("x > ?", [3]):   # lazy iteration
    process(item)

rows  = items.select("x > ?", [3]).fetchall()   # materialize all
first = items.select("x > ?", [3]).fetchone()    # first or None
n     = len(items.select("x > ?", [3]))          # count without fetching rows
```

**Naming.** `Rows`, not `Result`. The object represents a _query_ that has not
necessarily run, not a _snapshot_ of fetched data. The distinction is
load-bearing: `Rows` is a deferred computation; a result would imply the work is
already done.

**Transaction lifetime.** Lazy iteration holds a read transaction open until the
iterator is exhausted or dropped. Long-lived iterators pin an LMDB snapshot and
prevent page reclamation; callers iterating large scans partially should not
retain the iterator. `fetchall` opens and closes its transaction within the call.

---

## Design Principles

1. **Two peer surfaces, one store.** Dict = primary-key point/range. SQL =
   predicate over contents. Neither wraps the other.
2. **No DSL.** Predicates are parameterized SQL string fragments. The query
   language is MonaDB's, surfaced directly.
3. **`MutableMapping` for the dict protocol.** Five abstract methods; the rest
   via mixin, with `values`/`items`/`__contains__` overridden for single-scan
   performance.
4. **Slices are ranges.** A documented extension of the mapping contract.
5. **`insert` ≠ `__setitem__`.** Generated key vs. caller-supplied key are
   different operations with different names.
6. **`Rows` is lazy and reusable.** Holds SQL + params, re-executes per
   materialization. Minimal surface: `__iter__`, `__len__`, `fetchone`,
   `fetchall`.
7. **Explicit whole-table mutation.** `update`/`delete` require an explicit
   `None` predicate to touch all rows.
