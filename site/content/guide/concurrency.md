+++
title = "Concurrency"
description = "Commits, readers, writers, durability, and the one-process limit."
template = "docs/page.html"
weight = 3
+++

Every operation is its own commit. There is no transaction to open, hold, or
forget to close.

```python
db["users"]["bob"] = {"age": 41}     # one commit
del db["users"]["alice"]             # another
```

## `update()` is one commit

A multi-key write goes through `update()`, which commits the whole mapping at
once. Every key and document is encoded before the write opens, so an item that
cannot be stored raises without anything having been written:

```python
c.update({"a": {"n": 1}, "b": 42, "c": {"n": 3}})   # 42 is not a mapping
# TypeError — and nothing was written, not even "a"

"a" in c                        # False
```

It takes the same arguments `dict.update` does — a mapping, an iterable of
pairs, or keyword arguments.

## Readers never block

redb is MVCC, so a read snapshot is unaffected by concurrent writes — including
a long iteration:

```python
it = iter(c.items())
next(it)
c[99] = {"n": 99}               # concurrent write
list(it)                        # the original snapshot, without 99
```

An iterator holds its own snapshot until it is exhausted, so nothing it returns
can change underneath it.

## Writers serialize

One write runs at a time. A second writer waits for the first to commit, and
that wait happens without the GIL — other Python threads, readers included, keep
running while a write is in flight.

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
