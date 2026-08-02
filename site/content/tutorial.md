+++
title = "Tutorial"
description = "Build a small application with MonaDB, from opening a database to committing a transaction."
template = "docs/page.html"
weight = 2
+++

This walks through the whole API in one sitting. Everything here runs against an
in-memory database, so you can follow along in a shell.

## Opening

```python
import monadb

db = monadb.open()              # in-memory, discarded on close
db = monadb.open("app.db")      # file-backed
```

A `Database` is a mapping of collection names to collections, and a collection is
a mapping of keys to documents. So the whole store is a dict of dicts.

## Writing and reading

```python
db["users"]["alice"] = {"age": 30, "email": "alice@example.com"}
db["users"]["alice"]
# {'age': 30, 'email': 'alice@example.com'}
```

You never create a collection. `db["users"]` hands back a handle immediately; the
underlying table appears on the first write. Until then the collection reads as
empty and does not show up in `list(db)`.

```python
db2 = monadb.open()
db2["users"]                    # fine — a handle
list(db2)                       # []
len(db2["users"])               # 0
```

## The dict protocol

A collection is a `MutableMapping`, so everything you expect is present:

```python
users = db["users"]

"alice" in users                # True
len(users)                      # 1
users.get("bob", {})            # {}
users.setdefault("bob", {"age": 41})
users.update({"carol": {"age": 22}, "dan": {"age": 51}})
users.pop("dan")                # {'age': 51}
del users["bob"]

for key, doc in users.items():
    print(key, doc)
```

Missing keys raise `KeyError`, exactly as a dict does.

## Ordering

Iteration is in **key order**, not insertion order. That is the one difference
you will notice immediately, and it is the point: keys are stored in an
order-preserving encoding, so the b-tree's ordering is yours to use.

```python
events = db["events"]
for ts in [1699000300, 1699000100, 1699000200]:
    events[ts] = {"at": ts}

list(events)                    # [1699000100, 1699000200, 1699000300]
list(reversed(events))          # [1699000300, 1699000200, 1699000100]

events.first()                  # (1699000100, {'at': 1699000100})
events.last()                   # (1699000300, {'at': 1699000300})
```

Ranges are half-open, and either bound may be `None`:

```python
list(events.range(1699000100, 1699000300))   # the first two
list(events.range(None, 1699000200))         # everything below
list(events.range(1699000200, None))         # everything from there up
```

Prefix scans work on strings, bytes, and tuple keys:

```python
logs = db["logs"]
logs["2026-08-01:a"] = {}
logs["2026-08-02:b"] = {}
logs["2026-09-01:c"] = {}

[k for k, _ in logs.prefix("2026-08")]
# ['2026-08-01:a', '2026-08-02:b']
```

## Transactions

Outside a transaction each operation commits on its own. A `with` block groups
them into one commit, and an exception rolls the whole block back.

```python
with db.transaction() as tx:
    tx["users"]["erin"] = {"age": 28}
    del tx["users"]["carol"]
# both changes land together
```

```python
try:
    with db.transaction() as tx:
        tx["users"]["frank"] = {"age": 33}
        raise RuntimeError("something went wrong")
except RuntimeError:
    pass

"frank" in db["users"]          # False — nothing partial survives
```

Reads inside the block see your own uncommitted writes.

## Models

A collection is plain-dict by default. Bind it to a dataclass or pydantic model
and it will validate on write and rebuild instances on read.

```python
from dataclasses import dataclass

@dataclass
class User:
    age: int
    email: str

users = db.collection("users", User)
users["gwen"] = User(age=44, email="gwen@example.com")
users["gwen"]                   # User(age=44, email='gwen@example.com')
```

The binding lives on the handle, not in the file, so the same data is still
readable as dicts:

```python
db["users"]["gwen"]             # {'age': 44, 'email': 'gwen@example.com'}
```

## Closing

```python
db.close()
```

Or use the database as a context manager, which closes it on exit:

```python
with monadb.open("app.db") as db:
    db["users"]["alice"] = {"age": 30}
```

Closing aborts any transaction still open.

## Where to go next

- [Keys](@/guide/keys.md) — what can be a key, and how ordering works
- [Documents](@/guide/documents.md) — what can go in a value
- [Transactions](@/guide/transactions.md) — contention, timeouts, durability
- [Models](@/guide/models.md) — dataclasses and pydantic
- [Reference](@/reference.md) — the complete API
