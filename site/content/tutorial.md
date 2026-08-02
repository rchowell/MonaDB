+++
title = "Tutorial"
description = "Build a small application with MonaDB, from opening a database to committing a transaction."
template = "docs/page.html"
weight = 2
+++

This tutorial walks through the entire MonaDB python API.

## Open Databasae

```python
import monadb

db = monadb.open()              # in-memory, discarded on close
db = monadb.open("app.db")      # file-backed
```

A `Database` is a mapping of collection names to collections, and a collection is
a mapping of keys to documents. You can think of collections as persistant
python dicts.

## Writing and reading

```python
# Creates a 'users' collection
users = db["users"]

# Insert a document into the 'users' collection
users["alice"] = {"age": 30, "email": "alice@example.com"}

# Fetch a document by its key
users["alice"]
# out:{'age': 30, 'email': 'alice@example.com'}
```

Collections are automatically created on the first write.

## The dict protocol

Each collection is a `MutableMapping` and behaves like a Python dict.

```python
# Returns the 'users' collection
users = db["users"]

# Checks if a document at the given key exists
"alice" in users

# Returns the length of the collection
len(users)

# Returns the document or default value
users.get("bob", {})

# Returns document for "bob" if it exists, otherwise sets it.
users.setdefault("bob", {"age": 41})

# 
users.update({"carol": {"age": 22}, "dan": {"age": 51}})

users.pop("dan")                # {'age': 51}

# Deletes the document at key "bob"
del users["bob"]

# Iterate of key, document pairs
for key, doc in users.items():
    print(key, doc)
```

Missing keys raise `KeyError`, exactly as a dict does.

## Ordering

Iteration is in **key order**, not insertion order.

```python
events = db["events"]

for ts in [3, 1, 2]:
    events[ts] = {"at": ts}

list(events)                    # [1, 2, 3]
list(reversed(events))          # [3, 2, 1]

events.first()                  # (1, {'at': 1})
events.last()                   # (3, {'at': 3})
```

Ranges are half-open, and either bound may be `None`:

```python
list(events.range(1, 3))        # the first two: [(1, {'at': 1}), (2, {'at': 2})]
list(events.range(None, 2))     # everything below: [(1, {'at': 1})]
list(events.range(2, None))     # everything from there up: [(2, {'at': 2}), (3, {'at': 3})]
```

Prefix scans work on strings, bytes, and tuple keys:

```python
# Create a "logs" collection
logs = db["logs"]

# Insert documents
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
