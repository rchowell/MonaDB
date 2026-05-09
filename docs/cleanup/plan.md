# MonaDB Cleanup — Phased Delivery Plan

## Context

Several features have been written in parallel and the tree no longer compiles cleanly. The goal is to land them as small, independently testable PRs with clear component boundaries — not as one big merge.

**In-flight work (per `git status`):**
- A new LMDB-backed storage layer (`src/storage/`, `src/catalog.rs`) replacing the old `Cask` engine
- A disabled transaction wrapper (`src/transaction.rs`)
- A `schema.rs` stub
- Lexer/parser changes that don't yet match the conformance suite
- A missing `Rows` iterator type referenced by `lib.rs` and `main.rs`
- VM updates that depend on storage modules currently commented out of `lib.rs`

This document sequences the cleanup. Each phase is one PR. Companion: [`storage.md`](./storage.md) — the detailed storage component design.

## Component Boundaries (Reference)

| Layer | Modules | Knows About | Doesn't Know About |
|---|---|---|---|
| **Storage** | `storage/`, `catalog`, `transaction`, `cursor` | LMDB (`heed`), `Value`, `Error` | SQL, IR, bytecode |
| **Query language** | `lexer`, `parser` (gen), `ir` | Tokens, AST | Storage, bytecode |
| **Compilation** | `compiler` | IR → `Vop` | Storage internals |
| **Execution** | `vm` | `Vop`, `Storage` API | IR, parser |
| **Public API** | `lib`, `main` | All layers — orchestrator | — |

**Encapsulation rule:** the layer below never depends on the layer above. `keycodec` and `value_codec` are storage-internal — the VM should only see `Storage`, `ReadTxn`, `WriteTxn`, `Cursor`, and `Row`.

---

## Phase 1: Compilation Baseline

**Goal:** `cargo build` and `cargo test` are green before any other work lands.

**Problem:** `lib.rs` has `mod storage`, `mod transaction`, and `mod catalog` commented out, but still does `use storage::Storage`. `Rows` is used by `lib.rs` and `main.rs` but defined nowhere in the tree.

**Changes:**
- `src/lib.rs`: re-enable `pub mod storage`, `pub mod catalog` (keep `mod transaction` disabled until Phase 2 — the file has an intentional gate)
- `src/vm.rs`: define `pub struct Rows<'vm>` wrapping `VM<'vm>` with `impl Iterator<Item = Result<Value>>`
- Verify `cargo build` is warning-clean and existing storage unit tests pass

**Files touched:** `src/lib.rs`, `src/vm.rs`

**Out of scope:** any new functionality. This phase exists only to unblock the next four.

**Done when:** `cargo build && cargo test` green; no `todo!()` or `unimplemented!()` reachable from a passing test.

---

## Phase 2: Storage Layer

**Goal:** Storage is a fully encapsulated, fully unit-tested component with one public surface.

See [`storage.md`](./storage.md) for the detailed design.

**Changes:**
- Re-enable `mod transaction` in `lib.rs`; resolve any compile errors in `transaction.rs`
- Resolve `src/schema.rs`: either delete it or move `ColumnType`/`ColumnSchema` definitions there and re-export from `catalog.rs`
- Lock down visibility:
  - Public: `Storage`, `ReadTxn`, `WriteTxn`, `Cursor`, `Row`, `TableSchema`, `ColumnSchema`, `ColumnType`
  - `pub(crate)`: `keycodec`, `value_codec`, internal catalog helpers
- Add `WriteTxn` round-trip and abort tests
- Add a `Storage::memory()` constructor for tests (uses `TempDir` like `MonaDB::memory`)

**Files touched:** `src/storage/`, `src/catalog.rs`, `src/transaction.rs`, `src/schema.rs`, `src/lib.rs`

**Tests:**
- Existing 9 tests in `storage/mod.rs` pass
- New: `transaction.rs` write+commit+read round-trip
- New: `transaction.rs` abort discards staged writes
- New: multi-table isolation through `WriteTxn`

**Done when:** the storage layer compiles, tests pass, and nothing outside `src/storage/` (and `src/catalog.rs`, `src/transaction.rs`) imports `heed::*` or `keycodec::*`.

---

## Phase 3: Lexer Hardening

**Goal:** All tokenization edge cases referenced by the conformance suite work, with unit-test coverage.

**Gaps (from suite 01-literals.yaml):**
1. **Case-insensitive keywords** — `SELECT`, `Select`, `select` all currently fail to tokenize uniformly. Replace `#[token("select")]` with `#[token("select", ignore(ascii_case))]` for every keyword.
2. **SQL-standard string escaping** — `'it''s'` should produce `it's`. Update the string regex and post-process doubled single quotes inside the slice callback.
3. **`fetch` keyword** — `ir.rs` tests use `fetch`, the parser uses `limit`. Pick one; per the existing storage docs the canonical form is `limit`. Either add `fetch` as an alias or update the IR tests.

**Files touched:** `src/lexer.rs`, possibly `src/ir.rs` test strings

**Tests (in `src/lexer.rs`):**
- All keywords tokenize regardless of case
- `'it''s'` → `String("it's")`
- `'\n'` → `String("\n")` (existing backslash escapes preserved)
- `LIMIT 5` and `limit 5` both tokenize

**Done when:** every lexer-level case in `01-literals.yaml` passes the lexer, even if the parser hasn't caught up yet.

---

## Phase 4: Parser Completions

**Goal:** No `todo!()` panics in the parser. Every grammar production has at least one unit test.

**Gaps:**
1. **`SELECT <expr>` without `FROM`** — currently `todo!("select without block")`. Replace with a real production emitting `Statement::Select { from: None, ... }` (or equivalent shape). Every literal test in `01-literals.yaml` depends on this.
2. **Array literal syntax** — `[1, 2, 3]` has no production. Add `"[" <List<Expr>> "]"` and a corresponding IR variant.
3. **Field naming consistency** — if Phase 3 chose `limit`, rename `Select.fetch` to `Select.limit` in `ir.rs` and update tests.
4. **`Constructor::None` semantics** — the `.` envelope form (`select . from T as t`) currently emits nothing. Decide: emit the binding object, or remove the production. Per `09-from.yaml::from-select-dot-envelope` it should emit `{t: <row>}`.

**Files touched:** `src/parser.lalrpop`, `src/ir.rs`, `src/compiler.rs` (for any new IR variants)

**Tests:**
- `select 1;`, `select null;`, `select 'hello';`, `select {};`
- `select [1, 2, 3];`
- `select * from T limit 5;`
- Object construction round-trip

**Done when:** no `todo!()` reachable from the conformance suite.

---

## Phase 5: VM ↔ Storage Integration

**Goal:** End-to-end SQL execution via the new storage layer; conformance suite enabled for implemented features.

**Changes:**
- Wire `VM` to `Storage` through `ReadTxn` / `WriteTxn` (the `txn: TxnHandle<'a>` field already exists in `vm.rs`)
- Implement remaining `Vop`s currently returning `unsupported!`: `Clear`, `Drop`
- Fix `Vop::Init` to open the right txn mode based on compiler annotation
- Implement `Constructor::None` (envelope) properly in the compiler
- Remove `#[ignore]` from conformance tests that should now pass

**Files touched:** `src/vm.rs`, `src/compiler.rs`, `tests/conformance/`

**Tests — enable in `09-from.yaml`:**
- `from-empty-table`, `from-basic-scan` (without ORDER BY clause variants), `from-where-filter`, `from-explicit-alias`, `from-implicit-alias`, `from-unbound-table`, `from-unbound-alias`

**Tests — keep ignored (with reason string):**
- `from-cross-product` — multiple FROM sources not implemented
- `from-lateral-path-source` — path sources in FROM not implemented
- `from-subquery-source` — subqueries not in grammar
- `from-multi-step-insert-count` — `count(*)` and `GROUP BY` not implemented

**Done when:** `cargo test` runs the full conformance suite and the non-ignored tests pass.

---

## Deferred (Future Phases, Out of Scope)

Tracked for visibility but not part of this cleanup pass:

- `ORDER BY` clause
- `GROUP BY` / aggregates / `count(*)`
- Multiple `FROM` sources (joins, cross products)
- Lateral / path sources in `FROM`
- Subqueries in `FROM`
- Function calls (currently `unimplemented!()`)
- `DELETE FROM` / `DROP TABLE` VM implementation
- `.info` REPL command wired to the real catalog
- Stdin pipe mode in `main.rs` (currently echoes lines instead of executing)
- Branching / MVCC (see existing `docs/storage/reference.md` Appendix A)

---

## Per-Phase Verification

Every phase must pass:

```sh
cargo build          # warning-clean
cargo test           # all non-ignored tests pass
cargo clippy         # no new lints
```

Phase 5 additionally:

```sh
cargo test conformance
cargo run            # REPL: create table x; insert into x ({a:1}); select * from x;
```
