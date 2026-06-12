# Design note — `t[...]` keyed table subscript access ("get")

**Status:** Signed off and implemented — the spec edits in §7 are applied (`docs/language.md`), and
both the full-key point lookup (`cc_expr_get` + `Vop::Get`) **and** the partial-key prefix
sub-sequence (handoff §5.5) are built. Partial keys lower to the same `Get`; the compiler reads
`args.len() < keys.len()` and emits `Vop::GetRange`, which prefix-scans (`Cursor::scan` now honors
its prefix arg) and materializes the matching rows into a `Value::Array` in key order.

---

## 1. Feature

A subscript on a **table receiver** resolves against the btree by key:

```sql
create table t (id int);
select t[1];            -- full key       -> the one stored row, or null on miss
create table c (a string, b int);
select c['x', 7];       -- full composite  -> the one row, or null
select c['x'];          -- PARTIAL (leading prefix) -> sub-sequence of rows with a = 'x'
```

Key-completeness decides element vs. sub-sequence:

| subscript arity vs. key columns | result | status |
|---|---|---|
| `== key count` (full key) | the one row (object) or `null` | **done** |
| `0 < arity < key count` (leading prefix) | a sub-sequence (array) of matching rows, in key order | **done** |
| `arity == 0`, `arity > key count`, or keyless table | **static** error | done |
| base is a non-table unbound name | **static** error (unresolved) | done |

---

## 2. The overload rule (decision #3 — locked)

`foo[...]` has two meanings disambiguated by the **base**:

- **Table key access** iff `foo` is a bare identifier that does **not** resolve to a FROM/scope
  binding *and* names a catalog table.
- **Value path-navigation** (the existing `[expr]` segment) otherwise — i.e. when `foo` resolves
  to a binding, or is any non-identifier expression.

A binding **shadows** a table: inside `... from u as t`, `t[k]` stays value path-navigation.
`select t[1];` with no FROM has no binding, so `t` resolves to the table.

The parser cannot know the base kind, so it emits a **uniform** subscript IR node and the **binder**
lowers it to either path-nav (`Jpe`) or table-get (`Get`). (Handoff §5.1.)

## 3. Multi-element subscript (decision #2 — locked)

`[a, b]` on a **table receiver** is a **key tuple**, matched positionally to the declared key
columns. This is **not** the deferred path multi-selector. Path-receiver multi-selectors
(`value[a, b]`) **stay deferred** (Appendix A) and remain a **static** error — the two stay
orthogonal. A multi-arg subscript on a value base is therefore a static error.

## 4. Key-completeness against the **implemented** schema model — RECONCILIATION

This is the substantive spec fork (handoff Phase 0 #3).

- **`docs/language.md §5.1`** currently documents table options `{ key: c }` (single-column PK)
  and `{ hash: c, sort: d }` (partition+sort), mutually exclusive, with an implicit row id when
  omitted. **None of this options syntax is implemented.**
- **The implementation** (`schema.rs`, `catalog.get_table`, memory `project_keyed_storage`) uses:
  **the declared columns, in declaration order, ARE the composite key** (int/string only); there is
  a single order-preserving `EncodeKey` and no `{key}`/`{hash,sort}` options, no implicit row id.

**Proposed reconciliation (recommended):** make the **doc match the implementation**. Document the
"declared columns are the composite key, in order" model as normative for v1, and demote the
`{key}` / `{hash, sort}` options to a clearly-marked *future* note (or Appendix A). "Partial key"
then has a precise meaning: a **leading** subset of the declared columns, in order.

This is the cleanest basis for `t[...]`: the key columns are unambiguous and ordered, byte-encoding
is order-preserving, and a leading-column subset is a correct byte-prefix for the (future) range scan.

## 5. Result shape

- `select t[full]` → **one row** (the stored object) per evaluation, or `null` on miss.
- `select t[partial]` → **one value** that is the **sub-sequence** (array) of matching rows, in key
  order; partial-miss → empty array. It is a first-class value that composes — `t['a'][0]` indexes
  it and `from t['a'] as r` scans it as a value source (both covered in `14-get.yaml`). Implemented
  by materialize-to-`Array` (`Vop::GetRange`); a lazy cursor-backed sequence remains a future option.

## 6. Error categories (handoff §7 — get them exactly right)

| condition | error | conformance category |
|---|---|---|
| keyless table, `arity == 0`, `arity > key count` | `Error::BindError` (or `Unsupported`) | `static` |
| unbound non-table name | `Error::BindError` (unresolved) | `static` |
| literal key whose type mismatches the column (`t["a"]` on int key) | **`Error::Schema`** | `schema` |

Routing a literal type-mismatch through `encode_key_tuple → Error::Schema` is what keeps
`get-wrong-type` in the `schema` category.

## 7. Spec edits (applied)

**`docs/language.md §3.3`** — add a rule distinguishing the two subscript meanings, and replace the
blanket "multi-selectors `[a, b]` are not in v1" with "multi-selectors on a **path/value** receiver
are not in v1; a multi-element subscript on a **table** receiver is a composite-key tuple (§ get)."

**`docs/language.md §5.x`** — add a short "Keyed table access (`get`)" subsection stating §1–§6 above,
with full-key implemented and partial-key marked *planned*.

**`docs/language.md §5.1`** — reconcile the table-key model to "declared columns are the composite
key, in order"; mark `{key}`/`{hash,sort}` as future.

**`docs/sql/*.adoc`** — these are draft/historical (path.adoc, select.adoc, model.adoc, from.adoc,
create.adoc); add a one-line pointer to the normative `language.md` get section. Low priority.

## 8. Deferred (still not built)

- Runtime-expression keys (`foo[x.id]`) — v1 is **literal keys only**, encoded at compile time.
  This applies to both full and partial keys.
- Reverse-order range scans (`scan_rev` with a prefix) — there is no descending-order surface yet.
- A lazy, cursor-backed sub-sequence value (avoiding materialization) — `Vop::GetRange` eagerly
  builds a `Value::Array` for now.
- `{key}` / `{hash, sort}` options syntax.
