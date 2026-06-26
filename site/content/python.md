+++
title = "Python"
description = "Python API — connections, tables, rows handles, and the SQL cursor."
template = "docs/page.html"
weight = 3
+++

MonaDB exposes a DuckDB-style SQL cursor and dict-like table handles on top of a compiled Rust engine. Each `Table` offers two peer surfaces over the same LMDB-backed storage: a **dict-like** primary-key API and a **SQL-like** predicate API. Query results are lazy, reusable [`Rows`](#rows) handles.

## connect

```python
import monadb

db = monadb.connect()           # in-memory
db = monadb.connect("app.db")   # file-backed
db = monadb.connect("app.db", read_only=True)
db = monadb.connect("app.db", config={"nosync": True})
```

`connect` returns a `Connection`. Use it as a context manager to close automatically. The `config` keyword accepts open-time settings (see `monadb.ConnectConfig`); `read_only` is a separate connection-level flag.

## Connection

| Method / property               | Description                                                 |
| ------------------------------- | ----------------------------------------------------------- |
| `execute(sql, parameters=None)` | Run SQL; buffer rows; return `self` for chaining            |
| `sql(query, parameters=None)`   | Alias of `execute`                                          |
| `prepare(sql)`                  | Parse and cache `sql`; return a `Statement`                 |
| `fetchone()`                    | Next buffered row, or `None`                                |
| `fetchmany(size=1)`             | Up to `size` rows                                           |
| `fetchall()`                    | All remaining rows                                          |
| `description`                   | DBAPI-style column metadata from the last result, or `None` |
| `close()`                       | Close the database                                          |
| `table(name, schema=None)`      | `Table` handle for `name`                                   |

Parameters bind to `?`, `$N`, or `$name` placeholders in SQL. Pass a list or tuple for positional binding and a dict for named binding:

```python
db.execute("select ?;", [1])
db.execute("select $greeting;", {"greeting": "hello"})
```

## Statement

`Connection.prepare` returns a statement handle for repeated execution. The parse and compile work happens once; each call to `execute` only binds parameters and runs the cached plan.

| Method / property          | Description                                                 |
| -------------------------- | ----------------------------------------------------------- |
| `execute(parameters=None)` | Run the prepared statement; return `self` for chaining      |
| `fetchone()`               | Next buffered row, or `None`                                |
| `fetchmany(size=1)`        | Up to `size` rows                                           |
| `fetchall()`               | All remaining rows                                          |
| `sql`                      | Original SQL text passed to `prepare`                       |
| `description`              | DBAPI-style column metadata from the last result, or `None` |

```python
stmt = db.prepare("select t[?];")
row = stmt.execute([1]).fetchone()
```

## Table

```python
items = db.table("items", {"x": int})
```

`schema` is an open-schema type hint: declared fields are key columns (`int` or `str`) with type enforcement; undeclared fields pass through freely. When the table does not yet exist, it is created from the schema.

`Table` subclasses `collections.abc.MutableMapping`. Implementing the five abstract methods yields the full dict protocol (`keys`, `values`, `items`, `get`, `__contains__`, `update`, `pop`, …) via the mixin, with `values`, `items`, and `__contains__` overridden for single-scan performance.

### Dict-like surface

```python
items[1] = {"x": 1, "y": 2}   # upsert at caller-supplied key
items[1]                      # point lookup; KeyError if absent
items[1:4]                    # range scan → Rows
del items[1]                  # KeyError if absent

for k in items:               # yields keys
    ...
len(items)                    # COUNT(*)
1 in items                    # point lookup
```

Slices are **range queries**, not positional indices. `items[1:4]` scans keys in `[1, 4)`. `items[4:1:-1]` is a reverse range scan; other `step` values raise `NotImplementedError`.

### SQL-like surface

```python
items.select(predicate=None, params=None) -> Rows
items.insert(doc) -> key                         # DuplicateKeyError on collision
items.update(patch, predicate=None, params=None) -> int
items.delete(predicate=None, params=None) -> int
items.count(predicate=None, params=None) -> int
```

```python
items.select("x > ?", [3])
items.insert({"x": 9, "y": 0})
items.update({"y": 99}, "x > ?", [3])
items.delete("x < ?", [0])
items.count("x > ?", [3])
```

`predicate` is a SQL `WHERE` fragment (column names may be bare when they appear in the schema). Pass `predicate=None` explicitly for whole-table `update` and `delete`.

`insert` and `__setitem__` are distinct: `insert` derives the key and returns it; `__setitem__` is an upsert at a caller-supplied key.

`update`'s `patch` is either a dict (shallow-merged into each match) or a callable `doc -> doc`.

## Rows

`select` and key-slice access return a `Rows` handle: a lazy, iterable query that has not necessarily executed yet. It holds SQL and bound params, re-executing on each materialization.

```python
for item in items.select("x > ?", [3]):
    process(item)

rows  = items.select("x > ?", [3]).fetchall()
first = items.select("x > ?", [3]).fetchone()
n     = len(items.select("x > ?", [3]))
```

Lazy iteration holds a read transaction open until the iterator is exhausted or dropped. `fetchall` opens and closes its transaction within the call.

## Module shortcuts

`monadb.execute`, `fetchone`, `fetchmany`, and `fetchall` run against a shared in-memory connection for quick experiments.

## Errors

| Exception                    | When                                      |
| ---------------------------- | ----------------------------------------- |
| `monadb.Error`               | SQL errors, closed connections, type bugs |
| `monadb.DuplicateKeyError`   | Keyed `insert` when the key already exists |
