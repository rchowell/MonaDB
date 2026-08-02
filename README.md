# MonaDB

An embedded document store with Python dict semantics and transactions, built on
[redb](https://github.com/cberner/redb). Documents are stored as BSON.

```sh
pip install monadb
```

```python
import monadb

db = monadb.open("app.db")        # or monadb.open() for in-memory

db["users"]["alice"] = {"age": 30}
db["users"]["alice"]              # {"age": 30}
"alice" in db["users"]
len(db["users"])
del db["users"]["alice"]
```

## Transactions

A `with` block is one commit. An exception rolls the whole block back.

```python
with db.transaction() as tx:
    tx["users"]["bob"] = {"age": 41}
    del tx["users"]["alice"]
```

Outside a transaction, every operation commits on its own.

## Ordered operations

Keys use an order-preserving encoding, so the b-tree's ordering is directly
available:

```python
events.range(start, stop)   # half-open [start, stop); None means unbounded
events.prefix("2026-08")    # str/bytes prefix, or a tuple of leading components
events.first(); events.last()
reversed(events)
```

## Models

A collection is plain-dict by default and can be bound to a type. Binding is a
property of the handle — nothing about the type is stored, so the same data is
always readable as dicts.

```python
from dataclasses import dataclass

@dataclass
class User:
    name: str
    age: int

users = db.collection("users", User)
users["alice"] = User(name="alice", age=30)
users["alice"]                    # User(name='alice', age=30)
db["users"]["alice"]              # {"name": "alice", "age": 30}
```

pydantic models work the same way, recognized by shape — MonaDB never imports
pydantic.

## How it differs from `dict`

1. **Iteration is key order**, not insertion order.
2. **Keys** are `str | int | bytes | tuple` of those. `float`, `bool`, and
   `None` raise `TypeError`; an `int` outside 64 bits raises `ValueError`.
3. **Values must be mappings** — a dict, dataclass instance, or model.
4. **`update()` is atomic only inside a transaction.** Outside one it is a
   sequence of independent commits.
5. **`keys()`, `values()`, `items()` are snapshot iterators**, not live views.
6. **`"a"` and `("a",)` are the same key** — a scalar is a 1-component tuple.

## Concurrency

Readers never block: redb is MVCC. Writers serialize, and a write that cannot
start within `timeout` raises `BusyError` rather than waiting forever:

```python
db = monadb.open("app.db", timeout=5.0)
```

Opening a transaction while one is already open on the same thread raises
`TransactionError` immediately, instead of deadlocking against itself.

**Only one process may open a database for writing.** redb takes an exclusive
file lock; several processes may open the same file read-only. This is the one
capability 0.2 gives up relative to 0.1, which used LMDB.

## Durability

`durable=False` trades commit durability for speed, which suits bulk loads:

```python
db = monadb.open("app.db", durable=False)
```

## Exceptions

`monadb.Error` is the base; `BusyError` and `TransactionError` derive from it.
Everything else is a Python builtin — `KeyError`, `TypeError`, `ValueError`.

## Requirements

Python 3.9+. Building from source needs Rust 1.85+ (edition 2024).

## Versions

0.2 is a rewrite. The SQL engine MonaDB shipped through 0.1 is preserved under
the `v0.1.0-sql` git tag, and `pip install "monadb<0.2"` still installs it.
Databases written by 0.1 cannot be read by 0.2.

## License

Apache-2.0 — see [LICENSE](LICENSE).
