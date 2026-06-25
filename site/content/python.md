+++
title = "Python"
description = "Python API — connections, cursors, prepared statements, and dict-like tables."
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
| `table(name, keys=None)`        | `Table` handle for `name`                                   |

Parameters bind to `?`, `$N`, or `$name` placeholders in SQL. Pass a list or tuple for positional binding and a dict for named binding:

```python
db.execute("select ?;", [1])
db.execute("select $greeting;", {"greeting": "hello"})
```

## Statement

`Connection.prepare` returns a statement handle for repeated execution. The parse and compile work happens once; each call to `execute` only binds parameters and runs the cached plan. Use this for hot loops and point lookups.

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

# Re-bind on each call
stmt = db.prepare("select ?;")
assert stmt.execute([1]).fetchall() == [1]
assert stmt.execute([2]).fetchall() == [2]
```

A prepared statement becomes stale after schema changes that invalidate its plan (for example, `drop table`). Re-`prepare` after DDL.

## Table

`db.table("table")` returns a dict-like handle:

| Method              | Description                                             |
| ------------------- | ------------------------------------------------------- |
| `create(**columns)` | `create table` with key columns (`id=int`, `ts=int`, …) |
| `insert(rows)`      | Insert one dict or an iterable of dicts                 |
| `get(key, …)`       | Keyed lookup                                            |
| `delete(key, …)`    | Delete by key                                           |
| `table[key]`        | Same as `get`                                           |
| `del table[key]`    | Same as `delete`                                        |
| `len(table)`        | Row count                                               |
| `for row in table`  | Iterate all rows                                        |

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
