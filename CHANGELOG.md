# Changelog

## 0.2.0

A rewrite. MonaDB is now an embedded document store with Python dict semantics,
and nothing else.

### Added

- `monadb.open(path=None, *, durable=True)` — file-backed or in-memory.
- `Database` as `Mapping[str, Collection]`; `Collection` as `MutableMapping`.
- Every operation is its own commit, and `update()` is a single commit for the
  whole mapping — a bad item writes nothing.
- Ordered operations: `range`, `prefix`, `first`, `last`, `reversed`.
- Optional per-handle binding to a dataclass or pydantic model.
- `monadb.Error` for storage faults and use of a closed database. Everything
  else raises a Python builtin.

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
