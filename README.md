# MonaDB

An embedded document store with Python dict semantics, built on
[redb](https://github.com/cberner/redb).

```sh
pip install monadb
```

```python
import monadb

# Open a database, no file opens in-memory
db = monadb.open("app.db")

# Create a collection
users = db["users"]

# Insert a document
users["alice"] = {"age": 30}

# Fetch a document
users["alice"]  # {"age": 30}

# Check document existence
"alice" in users  # True

# Return collection length
len(users)

# Delete a document
del users["alice"]
```

Every operation is its own commit. `update()` is one commit for the whole
mapping.

```python
# Update all documents
users.update({"bob": {"age": 41}, "cy": {"age": 9}})
```

## Range Reads

Keys use an order-preserving encoding, so the b-tree's ordering is directly
available:

```python
# Iterate events in iterval [start, stop)
events.range(start, stop) 

# Iterate by key prefix
events.prefix("2026-08")

# Return first and last events
events.first(); events.last()

# Iterate in reverse key order
reversed(events)
```

## Models

A collection is plain-dict by default and can be bound to a type. Binding is a
property of the handle.

```python
from dataclasses import dataclass

@dataclass
class User:
    name: str
    age: int

# Collection bound to type 'User'
users = db.collection("users", User)

# Collection returns a User
users["alice"] = User(name="alice", age=30)
users["alice"]                    # User(name='alice', age=30)

# Raw access still returns a dict
db["users"]["alice"]              # {"name": "alice", "age": 30}
```

MonaDB also supports pydantic models.

## Concurrency

- Readers never block
- Writers serialize; a second writer waits for the first to commit.
- Only one process may open a database for writing.

## Durability

The option `durable=False` trades commit durability for speed, which suits bulk loads:

```python
db = monadb.open("app.db", durable=False)
```

## License

Apache-2.0 — see [LICENSE](LICENSE).
