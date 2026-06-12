# Handoff — `select foo['a']` / `select foo['a','b']` (table key-index access)

Audience: the next agent implementing keyed table subscript access ("get"). This
extends the just-completed `Value` redesign, which left `Cursor::get` in place as a
tested seam. Read it end-to-end before touching code; **Phase 0 is a design task and is
gated on human sign-off** (the user asked to *reconcile the spec first*, not override it).

---

## 1. What we're building

A keyed table behaves like a big ordered dict. A subscript on a **table receiver**
resolves against the btree by key:

```sql
create table t (id int);
select t[1];            -- full key  -> the one stored row, or null on miss
create table c (a int, b int);
select c['x', 7];       -- full composite key -> the one row, or null
select c['x'];          -- PARTIAL key -> the sub-sequence of rows whose a = 'x'
```

**Key-completeness decides element vs. sub-sequence** (the dict/array-view merge from the
`Value` redesign design notes):

- subscript arity **==** the table's key-column count → **point lookup** → one row or `null`.
- subscript arity **<** the *leading* key columns → **prefix scan** → a **sub-sequence**
  (an iterable sequence value of the matching rows).
- arity **>** key count, or a keyless table, or a non-table unbound name → **static error**.

### Decisions locked with the user (do not relitigate)

| # | Decision | Consequence |
|---|----------|-------------|
| 1 | **Literal keys only** for v1 (`foo[1]`, `foo['a','b']`). | The compiler encodes the key at **compile time** and pushes a `Value::Bytes` literal — no runtime key-encoder opcode. Runtime-expression keys (`foo[x.id]`) are a clean follow-up. |
| 2 | **Partial key → prefix-scan sub-sequence now** (not deferred). | Requires making `Cursor::scan` honor its (currently ignored) prefix arg, and representing the sub-sequence as a composable sequence `Value`. This is the hard part. |
| 3 | **Unbound-name → table** disambiguation. | `foo[k]` is a table access **iff** `foo` is a bare identifier that does **not** resolve to a FROM binding; a binding of the same name **shadows** the table (so `t[k]` inside `... from u as t` stays value path-navigation). |
| 4 | **Reconcile `docs/language.md` first.** | The formal spec (below) currently contradicts this feature. Phase 0 settles the spec and gets sign-off **before** implementation. |

---

## 2. Current state (ground truth, verified)

### Already in place
- **`Cursor::get(&self, txn, key) -> Result<Value>`** — `src/cursor.rs` (full-key point
  read → decoded row, or `Value::Null` on miss). Tested (`cursor::tests::get_*`).
  Marked `#[allow(dead_code)]` — **remove that attribute** once wired.
- **`Value`** is the new Rc-backed tagged enum (Int/Float/Str/Array/Object/…); `Value::Array`
  is `Rc<Vec<Value>>`. A sub-sequence result is naturally an `Array` value (or a lazy
  cursor-backed sequence — see §5.5).
- **`Cursor::scan(&mut self, txn, prefix: Option<&[u8]>)`** exists but **ignores `prefix`**
  — the 3 `#[ignore = "prefix arg currently unused in scan"]` tests in `src/cursor.rs`
  (`scan_with_prefix_*`) are the red spec for honoring it. `TableIter::FwdPre(RoPrefix…)`
  is already stubbed in the enum.
- **Catalog**: `Catalog::get_table(&self, txn, name) -> Result<TableDefinition>`
  (`src/catalog.rs`) returns `{ oid: Some(u32), name, keys: Vec<Key> }`, re-parsing the
  stored `create` SQL to recover `keys` (each `Key { name, ty: Type }`, Int/String only).
- **Key encoding primitives**: `schema::encode_int(i64) -> [u8;8]`, `schema::encode_str(&str)
  -> Vec<u8>` (order-preserving, self-delimiting). `schema::encode_key(val: &Value, keys)`
  exists but is **object-based** (pulls fields by name via `jpk`) — **not** reusable as-is
  for a positional tuple; see §5.3.

### Not yet present
- **No `Vop::Get` opcode.**
- **No multi-element subscript in the grammar.** `src/parser.lalrpop:251`
  `<Expr> "[" <Expr> "]" => ir::expr_jpe(<>)` accepts a **single** index only.
  `foo['a','b']` does **not** parse today.
- **No table-receiver resolution in the binder.** `select t[1]` parses to
  `Expr::Jpe { inp: Var("t"), exp: Lit(Int(1)) }` and dies at bind with
  `BindError("unresolved variable: t")` (`src/binder.rs` `visit_expr_mut`).
- **No positional key encoder** (only the object-based `encode_key`).

### `14-get.yaml` status (the conformance spec)
Auto-registered by `build.rs`'s suite glob (one `#[test]` per case). Of its 11 cases:
- **8 fail today** (the positive lookups + `get-wrong-type`) — they expect a result/`schema`
  error but hit `BindError` instead.
- **3 pass coincidentally** (`get-keyless`, `get-composite-arity`, `get-unknown-table`) —
  they expect `static` and `BindError` *is* `static`. **These will need real logic to keep
  passing for the right reason** once binding succeeds for table receivers.
- It only covers **single-key** positive cases + the negative composite-arity case. **No
  composite-key or partial-key positive cases exist yet** — add them in Phase 1.

### Baseline / verification gate (IMPORTANT — main is NOT green)
`main` has pre-existing, unrelated failures. The gate is **"no regressions vs. this set,"**
not "all green":
- conformance: **14 fail** — 8 `get__*` (this feature), 3 `literals__*`, 3 `select_clause__*`
  (bare `select <expr> from T` WIP).
- lib unit tests: green.

Record the set first (`cargo test --no-fail-fast`), then ensure your work only *removes*
`get__*` failures and adds green tests — no new reds elsewhere.

### Build gotcha (will bite you)
Editing `src/parser.lalrpop` does **not** reliably regenerate the parser in-session
(`build.rs`'s `lalrpop::process_root()` skips on an unchanged fingerprint). Force it:
```sh
rm -rf target/debug/build/monadb-*
cargo build
```
`touch` alone does not work. Symptom of a stale parser: type errors pointing at
`target/.../out/parser.rs`. Also: the `"number"` token feeds **four** grammar rules
(literal `Value`, `Selector::Index`, and all three `limit` rules) — keep that in mind if
you touch numeric subscripts.

---

## 3. Phase 0 — spec reconciliation (DESIGN; get sign-off before coding)

`docs/language.md §3.3` currently says:
- `t['user']` is **value path-navigation** (`[expr]` selects by index/computed key into the
  value bound to `t`), and
- **multi-selectors `[a, b]` are not in v1** (rule 6, deferred to Appendix A).

This feature introduces a **second meaning** for `t[...]` (table key access) and needs
multi-element subscripts for composite keys. Resolve and write down, in `docs/language.md`
(and reconcile the `.adoc` fragments in `docs/sql/path.adoc`, `select.adoc`, `model.adoc`,
`from.adoc`):

1. **The overload rule** (decision #3): subscript on a **table receiver** (bare unbound name
   resolving to a catalog table) = key access; subscript on a **value** (a binding, or any
   non-table expression) = the existing path-navigation. A binding shadows a table.
2. **Multi-element subscript** `[a, b]`: for a *table receiver* this is a **key tuple**
   (matched positionally to the declared key columns), **not** the deferred path multi-selector.
   Decide whether path-receiver multi-selectors stay deferred (recommend yes — keep them
   orthogonal).
3. **Key-completeness semantics**: full key → element-or-null; partial key (a **leading**
   subset of the key columns, in declared order) → sub-sequence. Define what "partial" means
   against the **implemented** schema model — note `docs/language.md §6` describes
   `{key}` / `{hash, sort}` table options, but the implementation uses **"declared columns
   ARE the composite key, in order"** (`docs/sql/create.adoc`, `schema.rs`). Reconcile the doc
   to the implemented model (or flag if the user wants the doc's model built instead).
4. **Result shape**: `select t[full]` yields **one row** per outer row (the object) or `null`.
   `select t[partial]` yields **one value** that is the **sub-sequence** of matching rows
   (a sequence/array). Confirm whether the sub-sequence is itself a first-class value that
   composes (`t['a'][0]`, `from t['a']`) — this drives §5.5.
5. **Errors**: keyless-table access and arity > key-count are `static`; a literal key whose
   type mismatches the column (`t["a"]` on an int key) is **`schema`** (see §5.4 — must be
   `Error::Schema`, not `BindError`).

Deliverable: an updated `docs/language.md` get/section + a short design note, reviewed by the
user. Do not start Phase 2 until this is signed off.

---

## 4. Phase 1 — tests first (extend `14-get.yaml`)

Per the project's tests-first-for-new-surface practice, write the conformance cases before
the IR/parser/VM work. Keep every existing case; **add**:
- composite **full-key** positives: `create table c (a string, b int); insert …; select c['x', 7];`
  → the row.
- **partial-key** sub-sequence positives: `select c['x'];` → the array of rows with `a = 'x'`,
  in key order; partial-miss → empty sequence.
- composite **miss** → `null`.
- ensure `get-keyless` / `get-composite-arity` (now meaning **arity > keys**) /
  `get-unknown-table` still assert `static`, and `get-wrong-type` asserts `schema`.

Also plan to **un-ignore** the 3 `cursor::tests::scan_with_prefix_*` tests in Phase 5 (they
are the unit-level red spec for prefix scan). Watch all new tests fail for the right reason
before implementing.

---

## 5. Phases 2–5 — implementation map (files, functions, sequence)

### 5.1 Grammar + IR (Phase 2)
- `src/parser.lalrpop`: add a table-subscript production accepting a **list** of index
  exprs, e.g. `<Expr> "[" <List<Expr>> "]"`. Single-arg subscript currently lowers to
  `expr_jpe`; keep path-nav working. Cleanest: emit a **uniform** subscript IR node
  (`Expr::Subscript { base: ExprRef, args: Vec<Expr> }`) and let the **binder** lower it to
  either path-nav (`Jpe`, single arg, value base) or table-get (any arity, table base). This
  avoids committing the parser to a meaning it can't know. (Alternative: keep `Jpe` for the
  single-arg value case and add `Expr::Get` for the table case — but the parser can't
  distinguish base kind, so prefer lowering in the binder.)
- `src/ir.rs`: add the new `Expr` variant(s) + parser-action fn(s) in the `ir.rs`
  action-function style. Remember `List<T>` macro lives at the bottom of the grammar.

### 5.2 Binder (Phase 3) — `src/binder.rs`
- In `visit_expr_mut`, when you reach a subscript whose `base` is an `Expr::Var` that
  `scope.resolve(name)` returns `None` for: try `self.get_table(name)` (already wraps
  `Catalog::get_table` and pushes the right error). If it resolves to a table:
  - allocate a cursor slot (`self.next_cursor()`), attach `oid` + `keys` to the lowered
    Get node (mirror how `visit_from_mut`/`visit_insert_mut` attach `csr`/`oid`/`keys`).
  - classify by arity vs `keys.len()`: full → point-get; `0 < args < keys.len()` (leading
    prefix) → prefix-get; `args == 0` or `args > keys.len()` or keyless table → **static**
    error; non-table unbound name → existing `BindError` (`static`).
  - **do not** recurse into the base as a normal Var (that would re-raise "unresolved
    variable"); handle the table base specially.
- If the base **does** resolve to a binding → leave it as path-nav (`Jpe`) — single-arg only;
  a multi-arg subscript on a value is a static error (path multi-selectors are deferred).

### 5.3 Compile-time key encoding (Phase 4) — `src/schema.rs`
- Add a **positional** encoder, e.g. `encode_key_tuple(vals: &[Value], keys: &[Key]) ->
  Result<Vec<u8>>`: zip `vals` with `keys` (for a prefix, with the **leading** `vals.len()`
  keys), encode each via `encode_int`/`encode_str` by column type, concatenating. A type
  mismatch → **`Error::Schema`** (category `schema`, matches `get-wrong-type`). The literal
  values come from the bound Get node's `args` (all `Expr::Lit` in v1), so this runs at
  **compile time** — no runtime opcode needed for the key.
- The order-preserving `encode_str` is prefix-safe (`"a" < "ab"`), so a leading-column prefix
  is a correct byte prefix for the range scan. Good.

### 5.4 Full-key point lookup (Phase 4) — compiler + VM
- `src/compiler.rs`: add `cc_expr_get` (dispatch from `cc_expr`). Emit:
  `Open { csr, tbl: oid }` → `emit_push(Value::Bytes(encoded_key))` → `Get { csr }`.
  Reuse `emit_open` / `emit_push`. (Read txn: the surrounding statement must be a read txn;
  `ensure_txn(TransactionMode::Read)` — check how `cc_select` sets the mode.)
- `src/vm.rs`: add `Vop::Get { csr }`:
  ```rust
  Vop::Get { csr } => {
      let key = pop_key(self.pop())?;
      let txn = self.txn.as_ref().expect("Get before Transaction");
      let val = self.cursors[*csr].get(txn, &key)?;   // Cursor::get already exists
      self.push(val);
  }
  ```
  Remove the `#[allow(dead_code)]` from `Cursor::get`. (`pop_key` already turns a
  `Value::Bytes` back into `Vec<u8>`.)
- This alone turns the 8 single-key `get__*` cases green and keeps the 3 negatives green for
  the right reason. Commit here as a milestone before the harder partial-key work.

### 5.5 Partial-key sub-sequence (Phase 5) — the hard part
- **Make `Cursor::scan` honor `prefix`**: when `Some(prefix)`, build a prefix iterator
  (`TableIter::FwdPre` over `btree.prefix_iter` — heed `RoPrefix`) instead of the full
  forward iter. Un-ignore and pass `cursor::tests::scan_with_prefix_*`.
- **Represent the sub-sequence as a composable `Value`.** Two options — pick during Phase 0
  design:
  1. **Materialize** the matching rows into a `Value::Array` (simplest; correct; eager).
     `select t['a']` yields that array; it composes with indexing/iteration for free.
  2. **Lazy cursor-backed sequence** — the `Value`-redesign design's "a table is a `Value`
     whose iteration is backed by an LMDB cursor." This is more faithful and avoids
     materialization, but needs a `Value` variant or wrapper that the cursor's `Source::Value`
     / `ValueIter` path can iterate, plus lifetime care. **Recommend materialize (option 1)
     for this pass** and leave lazy as a follow-up unless Phase 0 says otherwise.
- **Compiler/VM**: for a partial-get, emit `Open` + push prefix `Value::Bytes` + a
  prefix-`Scan` that drains into an `Array` (or the lazy sequence). If materializing, a small
  `Vop::GetRange { csr }` that prefix-scans and collects rows into a `Value::Array` is the
  most direct; reuse `Cursor::scan(Some(prefix))` + `load()` + `next()` internally.
- **Compose**: confirm `select t['a']` (array result), `t['a'][0]` (index into it), and
  `from t['a']` (scan it as a value source — the existing `Source::Value`/`Iter` path) all
  work; add cases.

---

## 6. Suggested sequence & gates

0. **Phase 0** spec reconciliation → user sign-off.
1. **Phase 1** extend `14-get.yaml` (+ plan to un-ignore prefix tests). Watch reds.
2. **Phase 2** grammar + IR (`rm -rf target/debug/build/monadb-*` after grammar edits).
3. **Phase 3** binder table-receiver lowering + arity/type/keyless classification.
4. **Phase 4** positional encoder + `cc_expr_get` + `Vop::Get`. **Gate:** single-key + composite
   full-key `get__*` green; negatives still green; no regressions vs. the recorded baseline.
5. **Phase 5** prefix scan in `Cursor::scan` (un-ignore prefix unit tests) + sub-sequence value
   + partial-key cases. **Gate:** partial-key cases green; prefix unit tests green; no regressions.

Each phase: `cargo test --no-fail-fast`, diff the conformance failure set against the
recorded baseline, confirm only `get__*` (and the prefix unit tests) move to green.

## 7. Watch-outs

- **Error categories**: keyless / arity-too-long / unknown-name → `Error::BindError` (or
  `Unsupported`) = `static`; literal-key type mismatch → `Error::Schema` = `schema`. Getting
  the variant wrong silently flips the conformance category.
- **Disambiguation precedence**: resolve scope bindings *before* the catalog, so a binding
  shadows a table (decision #3). `select t[1];` (no FROM) has no binding → table.
- **`select <expr>` WIP**: bare `select <expr> from T` has 3 pre-existing failures
  (`select_clause__select_expr_*`). `select t[k];` (no FROM) already parses, so v1 get does
  not depend on that fix — but `select t[k] from u …` forms may surface the same WIP gap.
  Keep get cases in the bare-`select <expr>;` (no-FROM) form like `14-get.yaml` does.
- **`get-wrong-type` is currently `schema`** in the suite — keep it that way (route through
  `encode_key_tuple` → `Error::Schema`).
- Do **not** "fix" the 3 `literals` / 3 `select_clause` failures — they are out of scope and
  pre-existing (keep failing tests visible).

## 8. Key file index

| File | Role for this feature |
|------|-----------------------|
| `src/parser.lalrpop` | add multi-element subscript production (line ~251) |
| `src/ir.rs` | new subscript/Get IR node + action fn |
| `src/binder.rs` | table-receiver detection & lowering (`visit_expr_mut`, `get_table`) |
| `src/catalog.rs` | `get_table` → oid + `keys` (reuse) |
| `src/schema.rs` | add positional `encode_key_tuple`; reuse `encode_int`/`encode_str` |
| `src/compiler.rs` | `cc_expr_get`; `emit_open`/`emit_push`; prefix-scan emission |
| `src/vm.rs` | new `Vop::Get` (+ `GetRange` if materializing); reuse `pop_key` |
| `src/cursor.rs` | un-`#[allow(dead_code)]` `get`; make `scan` honor `prefix`; un-ignore prefix tests |
| `tests/suites/14-get.yaml` | extend with composite + partial cases (Phase 1) |
| `docs/language.md`, `docs/sql/*.adoc` | Phase 0 reconciliation |
