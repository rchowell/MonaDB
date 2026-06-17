+++
title = "Python"
description = "Python API — connections, cursors, and dict-like tables."
template = "docs/page.html"
weight = 3
+++

MonaDB exposes a DuckDB-style SQL cursor and dict-like table handles on top of a compiled Rust engine.

## connect

```python
import monadb

db = monadb.connect()           # in-memory
db = monadb.connect("app.db")   # file-backed
db = monadb.connect("app.db", read_only=True)
```

`connect` returns a `Connection`. Use it as a context manager to close automatically.

## Connection

| Method | Description |
| --- | --- |
| `execute(sql, parameters=None)` | Run SQL; buffer rows; return `self` for chaining |
| `sql(query, parameters=None)` | Alias of `execute` |
| `fetchone()` | Next buffered row, or `None` |
| `fetchmany(size=1)` | Up to `size` rows |
| `fetchall()` | All remaining rows |
| `close()` | Close the database |
| `table(name, keys=None)` | `Table` handle for `name` |

Parameters bind to `?`, `$N`, or `$name` placeholders in SQL:

```python
db.execute("select $greeting;", {"greeting": "hello"})
```

## Table

`db.table("table")` returns a dict-like handle:

| Method | Description |
| --- | --- |
| `create(**columns)` | `create table` with key columns (`id=int`, `ts=int`, …) |
| `insert(rows)` | Insert one dict or an iterable of dicts |
| `get(key, …)` | Keyed lookup |
| `delete(key, …)` | Delete by key |
| `table[key]` | Same as `get` |
| `del table[key]` | Same as `delete` |
| `len(table)` | Row count |
| `for row in table` | Iterate all rows |

```python
users = db.table("users")
users.create(id=int)
users.insert({"id": 1, "name": "Ada"})
users[1]          # {'id': 1, 'name': 'Ada'}
users.delete(id=1)
```

## Module shortcuts

`monadb.execute`, `fetchone`, `fetchmany`, and `fetchall` run against a shared in-memory connection for quick experiments.

## Errors

`monadb.Error` is raised for SQL errors, closed connections, and type mismatches.
