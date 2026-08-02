# Changelog

## 0.2.0

A rewrite. MonaDB is now an embedded document store with Python dict semantics
and transactions, and nothing else.

### Added

- `monadb.open(path=None, *, timeout=5.0, durable=True)` — file-backed or
  in-memory.
- `Database` and `Transaction` as `Mapping[str, Collection]`; `Collection` as
  `MutableMapping`.
- Explicit transactions via `with db.transaction() as tx`, committing on clean
  exit and rolling back on exception.
- Ordered operations: `range`, `prefix`, `first`, `last`, `reversed`.
- Optional per-handle binding to a dataclass or pydantic model.
- `BusyError` when the write gate times out, and `TransactionError` for a nested
  or misused transaction — neither of which can hang.

### Removed

- The SQL dialect, its compiler, and its bytecode VM.
- The `monadb` CLI shell and the language-reference site.
- The Rust crate API; MonaDB is no longer published to crates.io.

### Changed

- Storage moved from LMDB to redb, and documents from a bespoke binary codec to
  BSON. **Databases written by 0.1 cannot be read by 0.2.**
- Dependencies went from 15 to 3: redb, bson, pyo3.
- The crate now builds under `#![forbid(unsafe_code)]`.

### Known limitation

Only one process may open a database for writing — redb takes an exclusive file
lock. LMDB permitted multi-process read-write access; this is the one capability
0.2 gives up.

The 0.1 SQL engine is preserved under the `v0.1.0-sql` git tag.
