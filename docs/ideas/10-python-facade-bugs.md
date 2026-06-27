# Python facade correctness bugs (post "simplify python api")

**Status:** findings / to-fix · **Area:** Python facade (`monadb/`) + keyless-insert engine seam

## Context

A high-effort code review of the `feat: simplify python api` change (`d4578e2`)
surfaced a cluster of verified correctness regressions in the new `Table` facade,
which was rewritten around `collections.abc.MutableMapping`. These are **not** part
of the cast-unification work — they live in already-committed code — so they are
recorded here rather than fixed inline. References are `file:line` against the tree
at the time of review.

The two most serious are silent **data-loss / corruption** defects; the rest are
predicate-qualification failures and API regressions from the `MutableMapping`
rewrite, plus one engine seam (keyless surrogate ids inside a transaction).

---

## 1. `delete()` with no argument wipes the entire table  🔴 data loss

`monadb/table.py:134-153`. `predicate` now defaults to `None`, and the bare path
issues an unguarded full-table delete:

```python
def delete(self, predicate=None, params=None) -> int:
    table = encode_ident(self._name)
    if predicate is None:
        return self._conn._mutations(f"delete from {table};")   # ← wipes everything
```

The previous API raised `TypeError` on a bare `delete()` *specifically to avoid
accidents* (the removed `test_delete_requires_predicate` enforced it). A typo or a
stale `t.delete()` now silently empties the table — unrecoverable.

**Fix sketch:** require an explicit sentinel for "delete all" (e.g.
`delete(predicate=...)` mandatory, or a separate `clear()`), and restore the guard
test. Mirror whatever guard `update()` should also get (see #2).

## 2. `update()` orphans / duplicates rows when a key column changes  🟠 corruption

`monadb/table.py:99-132`, calling `_upsert` at `:130`; `_upsert` at `:286-288`:

```python
def _upsert(self, key: Any, row: dict) -> None:      # `key` is ignored
    table = encode_ident(self._name)
    self._conn.execute(f"insert into {table} ({encode(row)});")
```

`_upsert` is a plain INSERT that derives the destination key from the *new* row's
fields and never deletes the *old* key. So `update({"id": 2}, "id = ?", [1])`
inserts a fresh row at key 2 while leaving the original key-1 row in place —
duplicating data and inflating the row count instead of moving the row. (The unused
`key` parameter is dead — see Minor cleanups.)

**Fix sketch:** when the key changes, delete the old key then insert; or reject
key-column mutation. Engine has no UPDATE, so the facade must do delete-then-insert
transactionally.

## 3. Keyless insert returns a stale/duplicate surrogate id inside a transaction  🟠

`monadb/table.py:95` → `monadb/connection.py:101` → `src/lib.rs` `peek_keyless_row_id`
(~`:248-282`). The peek opens a **fresh read transaction**:

```rust
let txn = self.storage.read_txn()?;          // src/lib.rs ~:268
let btree = self.storage.open_btree(&txn, oid)?;
// ... cursor.last(&txn) → BigEndian::read_u32(&key) + 1
```

A fresh read-txn cannot see uncommitted inserts from an open session transaction.
Inside `begin; … commit;`, two consecutive keyless `table.insert()` calls both peek
the same committed `last` key and return the same id, even though the engine's
`NewOid` allocates distinct higher ids — so the caller gets a wrong/duplicate key
back.

**Fix sketch:** allocate and return the id from the *same* write path that performs
the insert (have the engine surface the allocated oid), rather than peeking from an
independent read-txn. Removes the read/write race entirely.

## 4. Predicates over non-key columns fail to bind  🟠

`monadb/table.py:317-329`. `_qualify_predicate` only prefixes columns it knows from
`self._schema` — which holds **key columns only**:

```python
for col in sorted(self._schema.keys(), key=len, reverse=True):
    ...  # only schema (key) columns get the alias prefix
```

So `t.select("status = ?", ["active"])` on a payload (non-key) column emits
`select * from t as r where status = ?`, and the binder rejects the bare name with
`BindError: unresolved variable: status`. Affects `select`/`count`/`delete`/`update`
alike.

**Fix sketch:** qualify *every* identifier the predicate references (or have the
binder resolve bare names against the single source alias), not just schema columns.

## 5. Predicate qualifier corrupts string literals  🟠

`monadb/table.py:322-328`. The qualifier rewrites column-name tokens with a regex
over the *whole* WHERE fragment, including inside string literals:

```python
qualified = re.sub(rf"(?<!\.)\b{re.escape(ident)}\b", f"{alias}.{ident}", qualified)
```

For a key column `name`, `items.select("name = 'name'")` becomes
`r.name = 'r.name'` — the literal value is corrupted, so the query matches the
wrong rows.

**Fix sketch:** parse/skip string literals when qualifying, or stop doing
token-rewriting entirely and qualify at the binder level (ties into #4).

## 6. Schema-less handle to a known table loses qualification & slice typing  🟡

`monadb/table.py:319` (and `:295`) read the **Table-local** `self._schema`, while
key ops read `self._conn.schema_columns()` (`:244-251`). `Connection.table()`
records the schema in `_schemas` for *any* caller but passes the per-call `schema`
into the handle (`monadb/connection.py:77,88`). So a second handle
`t = db.table('users')` (schema omitted — allowed because the name is already in
`_opened`) gets `self._schema = None`:

```python
def _qualify_predicate(self, predicate, alias):
    if not self._schema:
        return predicate          # ← unqualified, even though the table’s key cols are known
```

`t.select('id = 5')` then leaves the predicate bare and the binder raises, while
`db.table('users', {'id': int}).select('id = 5')` on the *same* logical table
succeeds. `_rows_for_slice` (`:295`) has the same divergence for slice type
validation.

**Fix sketch:** have the Table read its schema from `self._conn.schema_columns()`
(single source) rather than a per-handle copy.

## 7. `MutableMapping` rewrite breaks composite `get` / keyword keys  🟡

`monadb/table.py:27`. Subclassing `MutableMapping` pulls in its `.get(key, default)`
mixin, which **replaces** the deleted explicit `get(*keys, **named)` composite-key
lookup:

- `c.get('x', 7)` (previously the composite-key row `a='x', b=7`) now treats `7` as
  a dict-style *default* and does a partial-key lookup on `'x'` (or returns `7` on
  miss) — silently wrong.
- `c.get(a='x', b=7)` raises `TypeError` (unexpected keyword).

`__contains__` (`:224`) likewise only accepts a full key.

**Fix sketch:** keep the Mapping surface but re-expose an explicit composite-key
getter (a distinct method name, so it doesn't collide with the `.get` mixin
contract).

## 8. `insert()` dropped the bulk iterable-of-rows path  🟡

`monadb/table.py:85-86`:

```python
if not isinstance(doc, dict):
    raise TypeError("doc must be a dict")
```

The previous `insert()` accepted an iterable of dicts (`if isinstance(rows, dict):
rows = [rows]` then bulk-encoded), exercised by `test_create_insert_get_delete` /
`test_keyless_table`. `t.insert([{...}, {...}])` now raises `TypeError`.

**Fix sketch:** restore the iterable path (single dict → one-element list), or add an
explicit `insert_many`.

---

## Minor cleanups (verified by the `/simplify` pass, below the report cap)

- **Dead parameter:** `_upsert(self, key, row)` (`monadb/table.py:286`) never uses
  `key`. (Will change as part of fixing #2.)
- **Bare `except` + blind retry:** `_lookup_point` (`monadb/table.py:267-271`)
  catches `Exception`, drops the cached statement, and retries once — masking the
  real error if the retry also fails.
- **Duplicated param/qualify helpers** across `select`/`count`/`delete`/`update`
  (the `where = self._qualify_predicate(...)` + alias dance is repeated) — factor a
  single `_where(predicate, params)` helper. Ties into #4/#5.
- **Deleted module headers** — the rewrite dropped the module-level docstrings that
  the rest of the codebase keeps.

## Suggested order of attack

Data-loss first: **#1** (guard `delete()`), then **#2/#3** (the corrupt-write
pair). Then the predicate-qualification family **#4 → #5 → #6** (shared root cause —
best fixed together by qualifying at the binder or a single `_where` helper). Then
the Mapping-API regressions **#7/#8**. Restore the deleted tests
(`test_delete_requires_predicate`, the bulk-insert / keyless tests) as each is
fixed, per "keep failing tests visible".
