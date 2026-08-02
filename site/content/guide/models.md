+++
title = "Models"
description = "Binding a collection to a dataclass or a pydantic model."
template = "docs/page.html"
weight = 4
+++

A collection is plain-dict by default. Bind it to a type and it converts on the
way down and rebuilds instances on the way up.

```python
from dataclasses import dataclass

@dataclass
class User:
    name: str
    age: int

users = db.collection("users", User)
users["alice"] = User(name="alice", age=30)
users["alice"]                  # User(name='alice', age=30)
```

pydantic works identically:

```python
import pydantic

class User(pydantic.BaseModel):
    name: str
    age: int

users = db.collection("users", User)
users["alice"] = User(name="alice", age=30)
users["alice"].age              # 30
```

## Binding lives on the handle

Nothing about the type is written to storage. The binding is a property of the
handle you are holding, so the same data is readable as dicts at any time:

```python
db["users"]["alice"]            # {'name': 'alice', 'age': 30}
```

That has a practical consequence worth planning around: renaming or moving a
class cannot break stored data, because the class name was never stored. It also
means nothing validates documents that were written through an unbound handle.

## Models are accepted anywhere

You do not need a bound handle to *write* a model. Any collection accepts one and
flattens it:

```python
db["users"]["bob"] = User(name="bob", age=41)
db["users"]["bob"]              # {'name': 'bob', 'age': 41} — dict, unbound
```

Binding is what you need to get instances *back*.

## Iteration

Bound handles rebuild instances everywhere a document is returned — `values()`,
`items()`, `first()`, `last()`, `range()`, and `prefix()`:

```python
[u.name for _, u in users.items()]
users.first()                   # ('alice', User(name='alice', age=30))
```

## How detection works

MonaDB **never imports pydantic**. It recognizes both shapes by duck-typing:

| Shape | Written with | Read with |
|-------|--------------|-----------|
| has `model_dump` / `model_validate` | `model_dump()` | `Model.model_validate(doc)` |
| `dataclasses.is_dataclass` | `dataclasses.asdict()` | `Model(**doc)` |

Anything else raises `TypeError` when you try to bind it. Since pydantic is
matched structurally, an attrs class or any other type exposing the same two
methods works too.
