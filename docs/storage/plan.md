# MonaDB Storage Layer — Implementation Plan

Companion to `storage-reference.md`. That document explains *why*; this one is the execution roadmap. Read the reference first.

Branching is deferred and intentionally absent from this plan. See **Appendix A** for the forward-compatibility design.

---

## Goal

Replace `src/cask.rs` with an LMDB-backed storage layer that:

1. Lives in a single file.
2. Stores **schemaless rows**: the row body is opaque JSON. The only thing a table declares is its primary key, expressed as a list of typed key columns:

   ```sql
   create table points;                    -- no PK; storage assigns surrogate u64
   create table points (x int);            -- PK is (x); extracted from row's "x" field
   create table points (x int, y int);     -- composite PK (x, y)
   ```

3. Exposes a small façade to `src/vm.rs` so the VM never sees LMDB or key encoding.

---

## Phasing

| Phase | Scope | What changes user-visibly |
|-------|-------|---------------------------|
| **1** | LMDB swap. All tables use surrogate `u64` row ids. New CREATE TABLE syntax accepted but PK column lists are *recorded but unused* at storage time (still surrogate). | SQL grammar replaces type-body with key-column-list. `create + insert + select` work end-to-end. Database is durable across restarts. |
| **2** | Typed PK extraction: declared PK columns are pulled from row JSON and encoded into the data key. Planner uses leading-column equalities/ranges in `WHERE` to build prefix-range scans. | `where x = 1 and y = 2` becomes a prefix scan; full table scan otherwise. |

For the future branching milestone, see Appendix A.

---

## Storage façade (stable across phases)

The VM only ever sees this surface.

```rust
// src/storage/mod.rs
pub struct MonaDB { /* heed::Env + DB handles + catalog cache */ }

impl MonaDB {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self>;
    pub fn begin_read(&self) -> Result<ReadTxn<'_>>;
    pub fn begin_write(&self) -> Result<WriteTxn<'_>>;
}

pub struct ReadTxn<'env> { /* heed::RoTxn + catalog snapshot */ }
impl<'env> ReadTxn<'env> {
    pub fn open_cursor(&self, table: &str) -> Result<StorageCursor<'_>>;
    pub fn get_table(&self, table: &str) -> Result<TableSchema>;
}

pub struct WriteTxn<'env> { /* heed::RwTxn + staged writes */ }
impl<'env> WriteTxn<'env> {
    pub fn create_table(&mut self, schema: TableSchema) -> Result<()>;
    pub fn drop_table(&mut self, name: &str) -> Result<()>;
    pub fn put_row(&mut self, table: &str, value: Value) -> Result<()>;
    pub fn delete(&mut self, table: &str, key: &CompositeKey) -> Result<()>;
    pub fn clear(&mut self, table: &str) -> Result<()>;
    pub fn open_cursor(&self, table: &str) -> Result<StorageCursor<'_>>;
    pub fn commit(self) -> Result<()>;
    pub fn abort(self);
}

pub struct StorageCursor<'txn> { /* see reference §"Cursor state machine" */ }
impl StorageCursor<'_> {
    pub fn rewind(&mut self) -> bool;
    pub fn next(&mut self) -> bool;
    pub fn curr(&self) -> &Row;
    // P2 additions — SQLite-style positioning + termination split.
    // The cursor owns the keycodec; callers pass logical Values, never bytes.
    // `key.len() < pk_arity` is a partial-prefix seek and is encoded in prefix form
    // (terminator only, no trailing components).
    pub fn seek_ge(&mut self, key: &[Value]) -> bool;  // position; false if no row matches
    pub fn seek_gt(&mut self, key: &[Value]) -> bool;
    pub fn seek_le(&mut self, key: &[Value]) -> bool;
    pub fn seek_lt(&mut self, key: &[Value]) -> bool;
    pub fn idx_ge(&self, key: &[Value]) -> bool;       // current_key >= encoded(key)
    pub fn idx_gt(&self, key: &[Value]) -> bool;
    pub fn idx_le(&self, key: &[Value]) -> bool;
    pub fn idx_lt(&self, key: &[Value]) -> bool;
}
```

`Row { oid: u64, val: Value }` moves from `src/cursor.rs` to `src/storage/mod.rs`. `src/cursor.rs` is deleted. The VM holds `TxnHandle<'a>` (enum of `ReadTxn`/`WriteTxn`/`None`) plus `Vec<StorageCursor<'a>>`. `heed::Env` is internally `Arc`-cloneable, so `Connection` holds a clone and the VM creates a txn on `Vop::Init`.

---

## LMDB physical layout

Two named DBs in one env (`max_dbs = 8` for room to grow):

| DB        | Phase | Key                                | Value                       |
|-----------|-------|------------------------------------|-----------------------------|
| `meta`    | P1    | `b"schema/<name>"`, counters       | bincode `TableSchema`, u64  |
| `data`    | P1    | composite (below)                  | tagged value bytes          |

Data-key shape:

```
[ u32_be(table_id) ]                4 bytes
[ pk_component_1 ]                  var, terminated 00 00 (with 0x00→00 01 / 0x01→00 02 escape) for strings;
                                    raw 8-byte sign-biased BE for ints
... pk_component_N
[ 8 reserved bytes ]                always 0x00..00 in P1/P2; reserved for future versioning (Appendix A)
```

For tables without a declared PK, the single PK component is a surrogate `u64_be(row_id)` (8 bytes, fixed-width). Surrogates live in the same physical layout — they are a one-component composite PK whose component is system-allocated.

Value tag byte: first byte is `0x00` for live rows. `0x01` is reserved (Appendix A). Body is `serde_json::to_vec(&value)`.

---

## SQL grammar

Tables are schemaless. The only thing that follows the table name is an optional list of typed key columns:

```
CreateTable: ir::Create =
    "create" "table" <name:"ident"> <keys:KeyColumns?> => ir::create_table(name, keys);

KeyColumns: Vec<ir::KeyColumn> = "(" <List<KeyColumn>> ")";

KeyColumn: ir::KeyColumn = <name:"ident"> <typ:KeyType> => ir::key_column(name, typ);

KeyType: ir::KeyType = {
    "int"    => ir::KeyType::Int,
    "string" => ir::KeyType::String,
};
```

Lexer additions: `int` and `string` as type keywords for key columns. (If `string` already tokenizes for the existing type system, reuse it.)

Examples that must round-trip parse → IR → DDL string:

```sql
create table points;
create table points (x int);
create table points (x int, y int);
create table users (id string);
```

IR shape (replaces the previous `Table::schema: Type`):

```rust
pub struct Table {
    pub name: String,
    pub keys: Vec<KeyColumn>,    // empty = no PK; storage assigns surrogate u64
}
pub struct KeyColumn { pub name: String, pub typ: KeyType }
pub enum KeyType { Int, String }
```

The `Type` enum in `ir.rs` continues to exist for *expressions* and predicates; it just no longer participates in CREATE TABLE.

---

## Module layout

```
src/
  lib.rs                  re-export storage::MonaDB
  main.rs                 unchanged
  error.rs                + From<heed::Error>, KeyTooLong
  lexer.rs                + 'int','string' if not present
  parser.lalrpop          replace TableDefinition body with KeyColumns
  ir.rs                   replace Table::schema with Table::keys: Vec<KeyColumn>;
                          add KeyColumn, KeyType
  compiler.rs             cc_create takes the new Table; PK validation in P2
  vm.rs                   VM<'a> holds TxnHandle<'a>; cursors borrow from it.
                          Vops 'Open','Rewind','Next','Load' unchanged in shape.
                          P2 adds 'SeekGE','SeekGT','SeekLE','SeekLT' (positioning)
                          and 'IdxGE','IdxGT','IdxLE','IdxLT' (loop termination);
                          each carries (cursor: usize, ncols: usize, jmp: usize).
                          'Insert','InsertBatch','CreateTable','Drop','Clear','Commit',
                          'Transaction' get real impls.
  rows.rs                 unchanged
  value.rs                unchanged
  cursor.rs               DELETE (Row moves to storage/mod.rs)
  cask.rs                 DELETE at end of Phase 1
  connection.rs           REWRITE: thin adapter wrapping MonaDB
  storage/
    mod.rs                MonaDB, Row, public re-exports
    env.rs                heed env init, Dbs handle struct
    catalog.rs            TableSchema, table_id allocation, schema CRUD on `meta` DB
    keycodec.rs           P1: surrogate u64 BE; P2: typed string/int with 0x00 escape,
                          looped over N components
    value_codec.rs        TAG_LIVE prefix byte + serde_json round-trip
    cursor.rs             StorageCursor
    txn.rs                ReadTxn, WriteTxn, staged-buffer commit
    error.rs              StorageError, From<heed::Error>
```

---

## Phase 1 deliverables

End-to-end LMDB-backed `create + insert + select` with the new schemaless syntax. PK column lists parse and persist in the catalog but storage uses surrogate `u64` keys for now.

1. **Cargo.toml**: add `heed = "0.20"`, `byteorder = "1.5"`. Confirm `cargo build` regenerates the parser cleanly.
2. **src/storage/**: implement `mod.rs`, `env.rs`, `catalog.rs`, `keycodec.rs` (surrogate only), `value_codec.rs`, `cursor.rs`, `txn.rs`, `error.rs`.
   - `keycodec`: surrogate `u64_be(row_id)`. Counter lives at `meta[b"row_seq/<u32_be(table_id)>"]`.
   - `value_codec`: prepend `0x00` tag, then `serde_json::to_vec`.
   - `cursor`: prefix walk over `u32_be(table_id)`, ignore final 8 bytes (always zero), strip leading tag byte from value.
   - `txn::WriteTxn`: staged `BTreeMap<Vec<u8>, Vec<u8>>`, flush on commit with the 8-byte zero suffix appended to each key.
3. **src/parser.lalrpop, src/lexer.rs, src/ir.rs**: replace the type-body in `CreateTable` with the key-column list. Parser accepts the new syntax. IR records `Table::keys` (storage ignores it in P1).
4. **src/connection.rs**: rewrite as a thin façade on `MonaDB`. Keep public method names where possible so `vm.rs` changes are minimal.
5. **src/vm.rs**:
   - Add `txn: TxnHandle<'a>` field on `VM<'a>`.
   - `Vop::Init` opens a read or write txn (compiler annotates which based on whether the program contains any `Insert`/`CreateTable`/`Drop`/`Clear`).
   - `Vop::Open(table)` calls `self.txn.open_cursor(table)`.
   - `Vop::Insert` / `Vop::InsertBatch` call `self.txn.put_row(...)`.
   - `Vop::Commit` finalizes the txn.
   - `Vop::Rewind` / `Vop::Next` / `Vop::Load` unchanged in shape.
6. Move `Row` from `src/cursor.rs` to `src/storage/mod.rs`. **Delete `src/cursor.rs`**.
7. **Delete `src/cask.rs`**. Drop the `bytes` dep from `Cargo.toml` if unused after.
8. Update existing `Connection` tests — they use the old type-body syntax; rewrite to the new key-column syntax.
9. Add `tests/storage_phase1.rs` and `tests/vm_e2e.rs`.

## Phase 2 deliverables

Real PK extraction and prefix-range planning.

1. **Validation in `cc_create`**: at most one PK clause; column names unique; types are `int` or `string`.
2. **`keycodec`**: typed string (UTF-8 + 0x00→00 01 / 0x01→00 02 escape + 00 00 terminator) and int (i64 sign-biased BE) encoders. Composite encoder is a loop over `Table::keys` producing `enc(c_1) || ... || enc(c_N)`. Add `encode_partial(values, pk_schema)` for partial-prefix seeks (length < pk_arity) — emits prefix form, no trailing components.
3. **`WriteTxn::put_row`**: extract PK column values from the row JSON by top-level field name, type-check against the schema, encode in column declaration order, write under the typed key. Tables with no `keys` continue using surrogates.
4. **New Vops**: `SeekGE/GT/LE/LT(c, ncols, jmp)` for one-shot positioning at the top of a loop, `IdxGE/GT/LE/LT(c, ncols, jmp)` for per-iteration termination checks. Each pops `ncols` values from the stack, hands them to the cursor as a `&[Value]`, and either positions (Seek\*) or compares-and-conditionally-jumps (Idx\*). VM dispatch in `vm.rs::next()` mirrors the existing `Rewind`/`Next` shape.
5. **Planner in `cc_iter`**: take `where_: &Option<Expr>` as a new parameter. Walk the predicate to extract `PkBounds { equalities: Vec<Expr>, range: Option<RangeOp> }` against the leading PK columns; the remainder is residual. Compile shape:
    - No PK bounds → `Open` + `Rewind` (Phase 1 path, unchanged).
    - Bounds present → `Open` + push-lower-tuple + `SeekGE/GT` + (loop top) push-upper-tuple + `IdxGT/GE` + body + `Next` jumping to loop top.
    - Residual `where_` (the part of the predicate not absorbed by the PK bound) → keep the existing `IfNot` filter inside the loop after `Idx*`.
6. **`cc_iter` return type**: change from `usize` to `IterPatches { loop_top: usize, exits: Vec<Patch> }`. `cc_select`'s `to_patch` extends with `exits`, so the multi-exit patching mechanism (compiler.rs:133–135) is unchanged in shape — it just consumes a list instead of a single pc. Loop-back target shifts from `loop_ + 1` to `loop_top` because the upper-bound check sits at the top of the loop, not the bottom.

---

## Key design choices

- **`u32_be(table_id)`** as data-key prefix, *not* varint. Sort-stable across all values; trivial prefix scans.
- **Schemaless rows**, typed keys only. The body is opaque JSON; the catalog only knows what fields the storage layer needs to extract for indexing.
- **N-component composite primary keys**, *not* DynamoDB's two-slot (partition, sort) model. Arbitrary arity matches every comparable SQL system; the encoder loops over the column list.
- **0x00-terminator with byte-stuffing escape** between key components, *not* length-prefix. Preserves lex order across variable-length components; works uniformly for 1, 2, or N columns.
- **8 reserved trailing bytes** on every data key. Always zero in P1/P2. Reserved so the future versioning system (Appendix A) lands without migrating existing rows.
- **Single shared `data` DB** for all tables; `max_dbs` does not grow with table count.
- **One reserved tag byte** at the start of every value. `0x00` = live row in P1/P2.
- **Buffer-and-flush** for `WriteTxn`, not a merge cursor. Read-your-own-writes inside a write txn is not supported in v1; add it when a workload demands it.
- **Surrogate `u64` row id** for tables without a declared PK. Treated internally as a one-component composite PK.
- **`Seek*`/`Idx*` opcode split, not pre-bounded cursors.** Positioning is one-shot at the top of a loop; termination is a per-iteration check against an upper bound on the stack. SQLite has stress-tested this shape for 20 years. It composes with the future MVCC visibility walk (Appendix A) — the upper-bound check stays at the opcode level rather than getting buried in the version-skipping inner loop. It also leaves `Rewind`/`Next` semantically untouched, so the Phase 1 full-scan path keeps working unchanged.
- **`ncols` parameter on every `Seek*`/`Idx*` op.** A 1-component seek on a 3-component PK is a *prefix* seek — encoded as `enc(c_1) || 00 00`, with the next-prefix successor `enc(c_1) || 00 01` for the upper. The cursor handles the arity-vs-PK-arity gap via `keycodec::encode_partial`; the VM stays scalar.

---

## Verification

1. **`tests/storage_phase1.rs`**: open temp `MonaDB`, create tables (with and without PK syntax), insert rows, scan via cursor; assert order and content. Reopen the same path, scan again, assert durability.
2. **`tests/vm_e2e.rs`**: full SQL → VM → storage path: `create table foo (x int); insert into foo ({x: 1}); select * from foo;`.
3. **`tests/cursor_unit.rs`**: direct-against-storage tests for `StorageCursor` over canned `data` DB content (multi-table prefix isolation, empty table, deletes).
4. `cargo test` green; existing parser/compiler/value tests adapted to the new CREATE TABLE syntax.
5. `cargo run` REPL: `create table x; insert into x (1); insert into x (2); select * from x;` returns 2 rows.

Phase 2 verification: `create table events (tenant string, user string, ts int);` followed by inserts and `select * from events where tenant = "acme";` compiles to `Open` + `SeekGE(c=0, ncols=1)` + (loop top) `IdxGT(c=0, ncols=1)` + body + `Next`, verified by inspecting the emitted `Vop` program. A `where ts >= 100 and ts < 200` clause with no leading-PK constraint falls back to `Rewind`/`Next` plus a residual `IfNot` filter — exactly the Phase 1 path.

---

## Appendix A: Future Branching Design

Branching, refs, commits, and MVCC are deferred. The data layout reserves space so they land additively without migrating existing rows.

**Reservations.** Every data key carries 8 trailing zero bytes; every value carries a leading `0x00` tag byte. When branching ships:
- The 8 trailing bytes become `u64_be_inverted(commit_seq)` — newest commit sorts first within a row's cluster.
- Value tag `0x01` marks tombstones; the cursor's MVCC walk treats them as "row not visible at this commit."

**Two new DBs.** `refs` (`b"branch/<name>" → CommitId`) and `commits` (`u64_be(commit_id) → bincode { parent: Option<CommitId>, message, timestamp }`). Both added to `MonaDB::open` without touching `data`.

**Reads at a branch.** `MonaDB::begin_read(Branch("foo"))` resolves to a commit_id, walks parent pointers in `commits` to a `HashSet<u64>` ancestor set, and stashes it on the txn. Cursor visibility check becomes "iterate versions newest-first; first one whose seq is in the ancestor set is visible (or a tombstone, in which case the row is hidden)."

**Writes.** `MonaDB::begin_write(branch)` reads the branch's HEAD, allocates a fresh `commit_seq` from `meta::next_commit_id` at commit time, and stamps it into every staged key's trailing 8 bytes before flush. The branch's HEAD pointer in `refs` advances atomically inside the same LMDB txn.

**Branch creation.** O(1): copy the source branch's HEAD commit_id into the new branch entry. LMDB's page-level CoW means the new branch shares physical storage with its parent until either diverges.

**Surface.** `set branch <name>` for session state; `create branch <name> from <ref>`, `commit [<msg>]`, `rollback`, `list branches` as DDL statements.

**Migration.** None. The 8-byte zero suffix on every existing P1/P2 row is the implicit "root commit" (commit_seq = 0). `main` branch bootstraps to point at it.

The full theory — ancestor-set computation, tombstone GC trade-offs, the cursor MVCC walk in detail — lives in `storage-reference.md` Appendix A.
