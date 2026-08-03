+++
title = "Tutorial"
description = "Build a small application with MonaDB, from opening a database to reading it back."
template = "docs/page.html"
weight = 2
+++

This tutorial walks through the entire MonaDB python API.

## Open Databasae

```python
import monadb

db = monadb.open()          # in-memory
db = monadb.open("app.db")  # file-backed
```

A `Database` is a mapping of collection names to collections, and a collection is
a mapping of keys to documents. You can think of collections as persistant
python dicts.

## Collections

Collections are automatically created on the first write.

```python
# Creates a 'users' collection
users = db["users"]

# Insert a document into the 'users' collection
users["alice"] = {"age": 30, "email": "alice@example.com"}

# Fetch a document by its key
users["alice"]  # {'age': 30, 'email': 'alice@example.com'}
```

## MutableMapping

Each collection is a `MutableMapping` and behaves like a Python dict.
Missing keys raise `KeyError`, exactly as a dict does.

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

# Updates given documents
users.update({"carol": {"age": 22}, "dan": {"age": 51}})

# Removes and returns the document at key "dan"
users.pop("dan")  # {'age': 51}

# Deletes the document at key "bob"
del users["bob"]

# Iterate all key, document pairs
for key, doc in users.items():
    print(key, doc)
```

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

## Models

A collection is plain-dict by default. You can bind a collection to a dataclass
or pydantic model and the collection will validate on write and rebuild
instances on read.

```python
from dataclasses import dataclass

@dataclass
class User:
    age: int
    email: str

# Collection is bound to the 'User' dataclass type
users = db.collection("users", User)

# Insert dataclass instances
users["gwen"] = User(age=44, email="gwen@example.com")

# Fetch a document, returning the data class instance
users["gwen"]  # User(age=44, email='gwen@example.com')
```

The model binding lives on the collection handle, so if you read
from an anonymous collection handle, then you get back a dict.

```python
db["users"]["gwen"]  # {'age': 44, 'email': 'gwen@example.com'}
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


## See also

- [Keys](@/guide/keys.md) — what can be a key, and how ordering works
- [Documents](@/guide/documents.md) — what can go in a value
- [Reference](@/reference.md) — the complete API

