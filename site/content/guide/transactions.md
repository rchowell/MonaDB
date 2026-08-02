+++
title = "Transactions"
description = "Commits, rollback, contention, timeouts, and the concurrency model."
template = "docs/page.html"
weight = 3
+++

Outside a transaction, every operation is its own commit. A `with` block makes
many operations one commit.

```python
with db.transaction() as tx:
    tx["users"]["bob"] = {"age": 41}
    del tx["users"]["alice"]
```

The block commits on clean exit. Any exception aborts it and propagates:

```python
with pytest.raises(RuntimeError):
    with db.transaction() as tx:
        tx["users"]["bob"] = {"age": 41}
        raise RuntimeError("boom")

"bob" in db["users"]            # False
```

## Scope is the handle you hold

A collection from `db` runs each operation in its own transaction. A collection
from `tx` participates in that transaction. There is no ambient state — the
object you are holding tells you the scope you are in.

```python
users = db["users"]             # its own commit per operation

with db.transaction() as tx:
    tusers = tx["users"]        # part of the transaction
```

Reads through a transaction handle see the transaction's own uncommitted writes.

## Atomicity of `update()`

This is a real difference from `dict`, and worth stating plainly. Outside a
transaction, `update()` is a sequence of independent commits, so a failure part
way through leaves the earlier writes in place:

```python
c.update({"a": {"n": 1}, "b": 42, "c": {"n": 3}})   # 42 is not a mapping
# TypeError — and "a" is already stored

"a" in c                        # True
"c" in c                        # False
```

Inside a transaction the same call is all-or-nothing:

```python
with db.transaction() as tx:
    tx["t"].update({"a": {"n": 1}, "b": 42, "c": {"n": 3}})
# TypeError, and nothing was written
```

## Concurrency

**Readers never block.** redb is MVCC, so a read snapshot is unaffected by
concurrent writes — including a long iteration:

```python
it = iter(c.items())
next(it)
c[99] = {"n": 99}               # concurrent write
list(it)                        # the original snapshot, without 99
```

**Writers serialize.** One write transaction runs at a time. A writer that
cannot start within `timeout` raises `BusyError` rather than waiting forever:

```python
db = monadb.open("app.db", timeout=5.0)     # seconds; the default
```

```python
try:
    with db.transaction() as tx:
        ...
except monadb.BusyError:
    ...                          # another writer held the lock too long
```

## Re-entry raises immediately

Opening a transaction while one is already open **on the same thread** raises
`TransactionError` at once, rather than waiting out a timeout that could never
clear:

```python
with db.transaction():
    db.transaction()             # TransactionError
```

The same applies to an implicit write from inside a block:

```python
with db.transaction() as tx:
    db["users"]["x"] = {}        # TransactionError, not a hang
```

Both cases would otherwise be a thread waiting on itself. Neither can hang.

## Durability

Commits are durable by default. For bulk loads, where you would rather redo the
work than pay for every commit:

```python
db = monadb.open("app.db", durable=False)
```

## One writing process

redb takes an exclusive file lock, so **only one process may open a database for
writing**. Several processes may open the same file read-only.

This is the one capability MonaDB 0.2 gives up relative to 0.1, which used LMDB
and permitted multi-process read-write access. If you need several processes
writing the same data, MonaDB is not the right tool.
