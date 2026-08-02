# MonaDB

An embedded document store with Python dict semantics and transactions. Rust
core behind pyo3, storage on redb, documents as BSON. Ships only to PyPI.

## Architecture

```
  Python  monadb/
    open(path=None, *, timeout=5.0, durable=True) -> Database
    Database      Mapping[str, Collection]        db["users"]
    Transaction   Mapping[str, Collection]        with db.transaction() as tx
    Collection    MutableMapping[Key, Doc]        users["alice"]
                          │  delegates every operation
  Rust    src/  (pyo3 extension module `_monadb`)
                          │
  redb 4.1   one table per collection, TableDefinition<&[u8], &[u8]>
```

Rust owns storage, transactions, key encoding, and BSON. Python owns the
mapping protocol and the model adapter. There is no catalog — redb's
`list_tables()` is the catalog.

## Key Files

| File                 | Role                                                             |
|----------------------|------------------------------------------------------------------|
| `src/db.rs`          | `DbInner` shared state, `Db` pyclass, `open()`                   |
| `src/txn.rs`         | The write `Gate` (timeout + re-entry guard), `Txn` pyclass       |
| `src/collection.rs`  | `Collection` pyclass, the `Readable` bridge, `DocIter`           |
| `src/keys.rs`        | Order-preserving key codec — the load-bearing property test      |
| `src/doc.rs`         | `PyObject` -> BSON on writes, `RawDocument` -> `PyDict` on reads |
| `src/error.rs`       | `Error` / `BusyError` / `TransactionError` and redb mapping      |
| `monadb/collection.py` | `MutableMapping` glue over the Rust connection                 |
| `monadb/db.py`       | `Database` as `Mapping[str, Collection]`                         |
| `monadb/txn.py`      | `Transaction` as `Mapping[str, Collection]`                      |
| `monadb/models.py`   | dataclass / pydantic adapter, by duck-typing                     |

## Build & Test

```sh
cargo test                          # Rust units: key codec, BSON, gate
maturin develop                     # build the extension into the venv
python -m pytest tests -q           # the conformance suite
cargo clippy --all-targets          # pedantic, must stay clean
```

## Invariants

- `#![forbid(unsafe_code)]`. Exactly three dependencies: redb, bson, pyo3.
- `src/` is seven files. The crate defines **no cargo features**:
  `pyo3/extension-module` comes from `[tool.maturin]`, and the `auto-initialize`
  dev-dependency is what lets `cargo test` link.
- The key codec's order-preservation property (`encode(a) < encode(b)` iff
  `a < b`) is what makes iteration order and every range bound correct. Change
  the encoding only with that test in front of you.
- The write gate must be acquired inside `Python::detach`. Waiting while holding
  the GIL stalls the threads that would release it.
- A write transaction cannot outlive the call or `with` block that created it.
  That is what makes retained-transaction deadlocks unrepresentable.
- `WriteTransaction::open_table` **creates** a missing table, so
  transaction-scoped reads must check `list_tables()` first or they silently
  vivify a collection.

## Conventions

### Comment Style

- Every public item opens with a one-line `///` summary: imperative, present
  tense, ending with a period.
- When one line isn't enough: summary, blank `///` line, then prose or an
  indented ASCII diagram. Align the art.
- Each file opens with a `//!` module header stating its role.
- Explain *why*, not *what* — especially where a redb or pyo3 constraint forced
  the shape of the code.
- `//` stays for in-body step narration.

### Naming

- Rust methods exposed to Python that collide with Rust keywords or traits take
  a trailing underscore plus `#[pyo3(name = "...")]` — `drop_` is exposed as
  `drop`.
- Iteration modes are `0 = keys`, `1 = values`, `2 = items`.
