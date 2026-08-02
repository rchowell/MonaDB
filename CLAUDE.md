# MonaDB

An embedded document store with Python dict semantics. Rust core behind pyo3,
storage on redb, documents as BSON. Ships only to PyPI.

## Architecture

```
  Python  monadb/
    open(path=None, *, durable=True) -> Database
    Database      Mapping[str, Collection]        db["users"]
    Collection    MutableMapping[Key, Doc]        users["alice"]
                          │  delegates every operation
  Rust    src/  (pyo3 extension module `_monadb`)
                          │
  redb 4.1   one table per collection, TableDefinition<&[u8], &[u8]>
```

Rust owns storage, key encoding, and BSON. Python owns the mapping protocol and
the model adapter. There is no catalog — redb's `list_tables()` is the catalog.

## Key Files

| File                 | Role                                                             |
|----------------------|------------------------------------------------------------------|
| `src/db.rs`          | `Connection` shared state, `Db` pyclass, `open()`                |
| `src/collection.rs`  | `Collection` pyclass, the one read and write path, `CollectionIter` |
| `src/keys.rs`        | Order-preserving key codec — the load-bearing property test      |
| `src/doc.rs`         | `PyObject` -> BSON on writes, `RawDocument` -> `PyDict` on reads |
| `src/error.rs`       | `Error`, and the mapping from redb faults onto it                |
| `monadb/collection.py` | `MutableMapping` glue, and the dataclass / pydantic adapter    |
| `monadb/db.py`       | `Database` as `Mapping[str, Collection]`                         |

## Build & Test

```sh
cargo test                          # Rust units: key codec, BSON
maturin develop                     # build the extension into the venv
python -m pytest tests -q           # the conformance suite
cargo clippy --all-targets          # pedantic, must stay clean
```

## Invariants

- `#![forbid(unsafe_code)]`. Exactly three dependencies: redb, bson, pyo3.
- `src/` is six files. The crate defines **no cargo features**:
  `pyo3/extension-module` comes from `[tool.maturin]`, and the `auto-initialize`
  dev-dependency is what lets `cargo test` link.
- The key codec's order-preservation property (`encode(a) < encode(b)` iff
  `a < b`) is what makes iteration order and every range bound correct. Change
  the encoding only with that test in front of you.
- There are no transactions in the API. Every operation is its own transaction,
  and no `WriteTransaction` outlives the call that opened it — which is what
  makes a retained write lock unrepresentable.
- `begin_write` and `commit` run inside `Python::detach`. redb serializes writers
  internally and waits without a deadline; holding the GIL through that wait, or
  through the fsync a commit does, stalls every other Python thread.
- `WriteTransaction::open_table` **creates** a missing table, so `delete` has to
  check that the collection exists before it opens the write, or a failed delete
  vivifies the collection. Reads open through a `ReadTransaction`, which cannot
  create one.
- `ReadOnlyTable` owns an `Arc` of its transaction guard, so a snapshot — and
  the `CollectionIter` built from it — outlives the `ReadTransaction` that opened it. That
  is what lets reads stream without borrowing anything.

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
