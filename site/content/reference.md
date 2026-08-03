+++
title = "Reference"
description = "The complete MonaDB API surface."
template = "docs/page.html"
weight = 5
+++

## MonaDB

```python
monadb.open(path=None, *, durable=True) -> Database
```

Opens a database. `path` of `None` gives an in-memory database; otherwise the
file is created if it does not exist.

| Argument  | Meaning                                           |
| --------- | ------------------------------------------------- |
| `path`    | file path, or `None` for in-memory                |
| `durable` | `False` relaxes commit durability, for bulk loads |

## Database

A `Mapping[str, Collection]`, and a context manager that closes on exit.

| Operation                         | Meaning                                           |
| --------------------------------- | ------------------------------------------------- |
| `db[name]`                        | a collection handle; auto-vivifying, never raises |
| `name in db`                      | whether the collection exists on disk             |
| `list(db)`, `iter(db)`            | collection names, sorted                          |
| `len(db)`                         | number of collections                             |
| `del db[name]`                    | drops the collection; `KeyError` if absent        |
| `db.collection(name, model=None)` | a handle, optionally bound to a type              |
| `db.close()`                      | closes the database                               |

`db[name]` returns a handle without creating anything — the table appears on the
first write. So a collection that has never been written reads as empty and does
not appear in `list(db)`.

## Collection

A `MutableMapping`. Every method below the divider comes from the ABC.

| Operation                    | Meaning                                                          |
| ---------------------------- | ---------------------------------------------------------------- |
| `c[key]`                     | the document; `KeyError` if absent                               |
| `c[key] = doc`               | upsert — assignment overwrites                                   |
| `del c[key]`                 | delete; `KeyError` if absent                                     |
| `key in c`                   | membership                                                       |
| `len(c)`                     | number of documents                                              |
| `iter(c)`                    | keys, in key order                                               |
| `reversed(c)`                | keys, descending                                                 |
| `c.keys()`                   | keys — a snapshot iterator                                       |
| `c.values()`                 | documents — a snapshot iterator                                  |
| `c.items()`                  | `(key, document)` — a snapshot iterator                          |
| `c.range(start, stop)`       | `(key, document)` for `start <= key < stop`; `None` is unbounded |
| `c.prefix(p)`                | `(key, document)` for keys beginning with `p`                    |
| `c.first()`                  | smallest `(key, document)`, or `None`                            |
| `c.last()`                   | largest `(key, document)`, or `None`                             |
| —                            |                                                                  |
| `c.get(key, default)`        |                                                                  |
| `c.pop(key[, default])`      |                                                                  |
| `c.popitem()`                |                                                                  |
| `c.setdefault(key, default)` |                                                                  |
| `c.update(mapping)`          | one commit for the whole mapping                                 |
| `c.clear()`                  |                                                                  |

## Keys

Keys are of type: `str`, `int`, `bytes`, or a flat tuple of those. Ordering is
int < str < bytes, componentwise for tuples. `"a"` and `("a",)` are the same
key.

Using `float`, `bool`, `None`, or nested tuples raise `TypeError`; an `int` outside 64
bits will raise a `ValueError`.

## Documents

Documents are mappings whose values are `None`, `bool`, `int`, `float`, `str`, `bytes`,
`datetime`, `list`, or a nested mapping. Datetimes store at millisecond
precision, and naive ones are treated as UTC.

## Exceptions

| Exception                 | Raised when                                                              |
| ------------------------- | ------------------------------------------------------------------------ |
| `monadb.Error`            | any storage fault, or use of a closed database                           |
| `KeyError`                | missing key, or dropping a missing collection                            |
| `TypeError`               | bad key type, non-mapping document, unsupported value type               |
| `ValueError`              | int out of range, or an empty collection name                            |

## Differences from Dict

1. Iteration is key order, not insertion order.
2. Keys are restricted to `str | int | bytes | tuple`.
3. Values must be mappings.
4. `keys()`, `values()`, `items()` are snapshot iterators, not live views.
5. `"a"` and `("a",)` are the same key.
