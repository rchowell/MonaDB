# Storage Layer — Cleanup State & Component Design

Companion to [`plan.md`](./plan.md). The forward-looking design lives in [`../storage/plan.md`](../storage/plan.md) and [`../storage/reference.md`](../storage/reference.md). This document is **the current state of the storage component as of this branch** and what specifically needs to land to ship Phase 2 of the cleanup.

---

## 1. Module Inventory (Current)

```
src/
├── storage/
│   ├── mod.rs           Storage, StorageInner, Row — public façade + LMDB env
│   ├── keycodec.rs      surrogate key encoding [u32_be(table_id) | u64_be(row_id) | 8 zero suffix]
│   └── value_codec.rs   tagged-byte value encoding [tag:u8 | json_body]
├── catalog.rs           ColumnType, ColumnSchema, TableSchema + meta-DB CRUD
├── cursor.rs            Cursor — forward iterator over one table's rows
├── transaction.rs       ReadTxn, WriteTxn — currently DISABLED in lib.rs
└── schema.rs            stub: just a `// TODO` comment
```

**lib.rs status:**
```rust
// pub mod storage;       ← commented out, but `use storage::Storage` still active
// pub mod transaction;
// mod catalog;
```

This is the immediate compile blocker. Phase 1 of [`plan.md`](./plan.md) re-enables `storage` and `catalog`; Phase 2 re-enables `transaction`.

---

## 2. Component Boundaries

The storage layer's contract with the rest of the system:

```
┌─────────────────────────────────────────────────────────┐
│ vm.rs  ──>  Storage, ReadTxn/WriteTxn, Cursor, Row      │  public surface
├─────────────────────────────────────────────────────────┤
│ transaction.rs  ──>  catalog::*, keycodec, value_codec  │  internal wiring
│ cursor.rs       ──>  keycodec, value_codec              │
│ catalog.rs      ──>  StorageInner.meta DB               │
├─────────────────────────────────────────────────────────┤
│ storage/keycodec.rs, storage/value_codec.rs             │  pure byte layout
│ storage/mod.rs::StorageInner   (heed::Env + Databases)  │  LMDB primitive
└─────────────────────────────────────────────────────────┘
```

**Encapsulation invariant:** nothing outside this layer should ever import `heed::*`, `keycodec::*`, or `value_codec::*`. The VM gets `Storage`, transactions, cursors, `Row`, `TableSchema` — that is the entire surface.

---

## 3. Public API (Target)

```rust
// src/storage/mod.rs
pub struct Storage { /* Arc<StorageInner> */ }
pub struct Row     { pub oid: u64, pub val: Value }

impl Storage {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self>;
    pub fn memory()             -> Result<Self>;            // ← add for tests
    pub fn begin_read(&self)    -> Result<ReadTxn<'_>>;
    pub fn begin_write(&self)   -> Result<WriteTxn<'_>>;
}

// src/transaction.rs
pub struct ReadTxn<'env> { /* &StorageInner + heed::RoTxn */ }
impl<'env> ReadTxn<'env> {
    pub fn get_table(&self, name: &str) -> Result<TableSchema>;
    pub fn open_cursor(&self, table: &str) -> Result<Cursor<'_>>;
}

pub struct WriteTxn<'env> { /* &StorageInner + heed::RwTxn + staged BTreeMap */ }
impl<'env> WriteTxn<'env> {
    pub fn get_table(&self, name: &str) -> Result<TableSchema>;
    pub fn create_table(&mut self, name: &str, columns: Vec<ColumnSchema>) -> Result<TableSchema>;
    pub fn put_row(&mut self, table: &str, value: Value) -> Result<()>;
    pub fn open_cursor(&self, table: &str) -> Result<Cursor<'_>>;   // committed-state only
    pub fn commit(self) -> Result<()>;
    pub fn abort(self);
}

// src/catalog.rs
pub struct TableSchema { pub name: String, pub table_id: u32, pub columns: Vec<ColumnSchema> }
pub struct ColumnSchema { pub name: String, pub typ: ColumnType }
pub enum   ColumnType  { Int, String }

// src/cursor.rs
pub struct Cursor<'a> { /* heed::RoRange<'a> + table_id + state */ }
impl<'a> Cursor<'a> {
    pub fn table_id(&self) -> u32;
    pub fn rewind(&mut self) -> Result<bool>;
    pub fn next(&mut self)   -> Result<bool>;
    pub fn curr(&self)       -> &Row;
}
```

**Visibility cleanup needed:**
- `storage/keycodec.rs`, `storage/value_codec.rs`: keep `mod` (private), expose only what `transaction.rs` and `cursor.rs` need via `pub(super)` or `pub(crate)`
- `StorageInner`: stays `pub(super)` — only `Storage`, `ReadTxn`, `WriteTxn` reach into it
- `catalog::ColumnType`, `ColumnSchema`, `TableSchema`: `pub` (the VM needs these)

---

## 4. Physical Layout (Phase 1, As-Implemented)

Two named LMDB databases inside one env (`max_dbs = 8` for future growth).

### 4.1 The `meta` DB — catalog and counters

Hand-rolled binary keys and values. No serde dependency at runtime.

| Key bytes                          | Value                          | Purpose                       |
|------------------------------------|--------------------------------|-------------------------------|
| `b"schema/<table_name>"`           | encoded `TableSchema` (binary) | per-table schema record       |
| `b"table_id/<u32_be>"`             | table name bytes               | reverse map for diagnostics   |
| `b"next_table_id"`                 | `u32_le`                       | monotonic table-id allocator  |
| `b"row_seq/<u32_be(table_id)>"`    | `u64_le`                       | per-table surrogate counter   |

`TableSchema` binary layout (in `catalog.rs::encode_schema`):
```
[ u32_le table_id ]
[ u16_be name_len ][ name bytes ]
[ u16_be col_count ]
foreach column:
  [ u8 type_tag ]                  0 = Int, 1 = String
  [ u16_be name_len ][ name bytes ]
```

### 4.2 The `data` DB — rows

| Key (20 bytes)                                                        | Value                                |
|-----------------------------------------------------------------------|--------------------------------------|
| `[ u32_be table_id ][ u64_be row_id ][ 8 reserved zero bytes ]`       | `[ u8 tag ][ serde_json::to_vec ]`   |

- `table_id` prefix: contiguous per-table B+ tree clusters → cheap prefix scans
- `row_id`: surrogate `u64`, allocated by `catalog::next_row_id`
- `8 trailing zero bytes`: reserved for future `commit_seq` versioning (see `docs/storage/reference.md` Appendix A). Do not use these for anything in Phase 1.
- value tag `0x00`: live row. Tags `0x01..0xFF` reserved (tombstones, large-blob pointers).

---

## 5. Cursor State Machine (cursor.rs)

```
   ┌────────────┐    rewind()      ┌────────────┐
   │ BeforeStart │  ───────────>   │ Positioned │ ◄──┐
   └────────────┘                  └────────────┘    │ next() (still inside prefix)
         │                                │           │
         │ next() before rewind()         │ next() (off end of prefix)
         │ ──> no-op (returns false)      ▼
         │                          ┌────────────┐
         │                          │ Exhausted  │
         │                          └────────────┘
         └─── rewind() ─────────────────^
```

Existing tests in `storage/mod.rs` cover all three transitions (see `rewind_after_exhaustion_reads_again`).

**Phase 2 will add:** `seek_ge`, `seek_gt`, `seek_le`, `seek_lt` for one-shot positioning, and `idx_ge`, etc. for per-iteration termination checks. Out of scope for cleanup.

---

## 6. Transaction Semantics (Current)

### ReadTxn
- Wraps `heed::RoTxn<'env>` — a snapshot of the DB at `begin_read` time
- MVCC via LMDB: never blocks writers, never blocked by them
- Multiple concurrent readers fine (no fan-out yet, but the type allows it)

### WriteTxn
- Wraps `heed::RwTxn<'env>` — exclusive (LMDB serializes writers)
- Staged writes go into a `BTreeMap<Vec<u8>, Vec<u8>>` keyed by *partial* data key (no trailing 8-byte suffix)
- `commit()` appends `ZERO_SUFFIX` to each staged key and flushes to LMDB inside the same `heed` txn — atomic
- `abort()` drops the staged map and the heed txn

**Read-your-own-writes is NOT supported.** Cursors opened on a `WriteTxn` see only the *committed* state, not the staged buffer. This is documented in the file header. Add a merge cursor when a real workload needs it.

### Why staged writes
- Lets the layer choose key encoding *after* knowing the full set of writes (irrelevant in P1, becomes relevant in P2 when typed PK extraction may want batching)
- Separates "logical write" from "physical key" — handy for Phase 2 PK extraction
- Matches the existing forward-looking design in `docs/storage/plan.md`

---

## 7. Test Coverage (Current)

In `src/storage/mod.rs::test`:

| Test                                  | Covers                                                |
|---------------------------------------|-------------------------------------------------------|
| `create_then_get_table`               | catalog round-trip                                    |
| `create_table_assigns_distinct_ids`   | table_id allocator                                    |
| `put_row_and_scan_in_insertion_order` | full WriteTxn → ReadTxn → Cursor path                 |
| `cursor_isolates_tables_by_prefix`    | table_id prefix isolation                             |
| `empty_table_scans_clean`             | empty-table edge case                                 |
| `unknown_table_errors`                | UnknownTable error path                               |
| `duplicate_create_errors`             | catalog duplicate detection                           |
| `aborted_write_does_not_persist`      | WriteTxn::abort                                       |
| `durability_across_reopen`            | LMDB persistence                                      |
| `rewind_after_exhaustion_reads_again` | cursor state machine                                  |

**Gaps to fill in Phase 2 of [`plan.md`](./plan.md):**
- WriteTxn-internal: open cursor on `WriteTxn` (not just `ReadTxn`) and verify it sees committed-but-not-staged state
- Multi-table interleaved writes inside one `WriteTxn` then commit, verify all tables present
- Catalog: `next_row_id` is monotonic across separate `WriteTxn`s
- `keycodec` unit tests (currently only exercised through integration)

---

## 8. Outstanding Cleanup Tasks (Phase 2 Surface)

In rough dependency order:

### 8.1 Re-enable `mod transaction` in lib.rs
- Uncomment `pub mod transaction` (and `mod catalog`)
- Verify `transaction.rs` compiles — its imports reference `super::catalog`, `super::cursor`, `super::StorageInner`, `super::keycodec`, `super::value_codec`. The `super::` is wrong now that `catalog` and `cursor` are top-level modules — these should be `crate::catalog`, `crate::cursor`, etc., **or** `transaction.rs` should be moved into `src/storage/`.

  **Decision needed:** is `transaction.rs` a top-level module or part of `storage/`? The existing forward-looking plan (`docs/storage/plan.md` §Module layout) puts it inside `storage/` as `storage/txn.rs`. Recommend: move `transaction.rs`, `catalog.rs`, and `cursor.rs` all under `src/storage/` to match. This removes the awkward `super::` / `crate::` confusion entirely and reinforces the encapsulation boundary.

### 8.2 Resolve `src/schema.rs`
The file is a one-line TODO comment. Two options:

| Option | Action | Trade-off |
|---|---|---|
| **A — delete** | `rm src/schema.rs`. Storage's `ColumnType`/`ColumnSchema` stay in `catalog.rs`. | Simplest; removes ambiguity; matches what's actually written. |
| **B — fill in** | Move `ColumnType`/`ColumnSchema` from `catalog.rs` into `schema.rs`; `catalog.rs` re-exports them. | Splits "schema types" from "schema persistence" — cleaner conceptually, but adds a file for a 30-line struct. |

**Recommend A** unless there's a planned use we're not seeing. Easier to add the file later than to delete code that's wired in.

### 8.3 Visibility audit
Walk the storage modules and tighten:
- `storage/keycodec.rs`: every `pub` → `pub(super)` unless `transaction.rs`/`cursor.rs` actually need it
- `storage/value_codec.rs`: same
- `StorageInner` fields (`heed`, `meta`, `data`): currently `pub`, should be `pub(super)` — only the storage module accesses them directly

### 8.4 Test additions
The four gaps listed in §7.

### 8.5 Add `Storage::memory()`
Currently every test does the `TempDir` dance manually. Add a helper:
```rust
impl Storage {
    pub fn memory() -> Result<Self> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("monadb.mdb");
        // NOTE: TempDir must outlive Storage; either leak it (dev-only) or hold it inside.
        // ...
    }
}
```
The `TempDir` ownership question is real — `MonaDB::memory()` already solves it (see `src/lib.rs`). Mirror that pattern: `Storage` carries an `Option<TempDir>` that drops with the engine.

---

## 9. Non-Goals for Cleanup

These belong to future phases, not the cleanup pass:

- **Typed PK extraction** — Phase 2 of `docs/storage/plan.md`. Surrogate `u64` keys for everything.
- **`Seek*` / `Idx*` opcodes** — composes with typed PKs; out of scope.
- **MVCC / branching** — Appendix A of `docs/storage/reference.md`. The 8-byte zero suffix on every key reserves space for it.
- **Read-your-own-writes inside WriteTxn** — needs a merge cursor; defer until a workload demands it.
- **Secondary indexes** — not in any plan yet.

---

## 10. Verification

End-of-Phase-2 acceptance:

```sh
cargo build               # warning-clean
cargo test storage        # all tests in storage/mod.rs + transaction.rs pass
cargo test --doc          # if doctests get added
grep -r "use heed" src/   # only matches inside src/storage/, src/catalog.rs, src/transaction.rs
```

Manual smoke test (after Phase 5 wires the VM):

```sh
cargo run
> create table x;
> insert into x ({a: 1});
> insert into x ({a: 2});
> select * from x;
> .exit
$ cargo run                   # reopen
> select * from x;            # should still return both rows (durability)
```
