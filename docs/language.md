# MonaDB Language Specification

MonaDB is an embedded, LMDB-backed database for semi-structured data. Its query language is stream-based operators over JSON-shaped values. This document defines the complete language.

## 1. Introduction

### 1.1 Query model

A query is a pipeline of clauses that transforms a stream of JSON objects:

| clause | action          |
|--------|-----------------|
| from   | iterate         |
| with   | rewrite         |
| where  | filter          |
| group  | reduce          |
| order  | sort            |
| limit  | slice           |
| select | project         |

Each clause produces a stream; `select` is the final projection. Set operations (`union`, `intersect`, `except`) combine two independent queries as multisets.

### 1.2 Lexical grammar

Identifiers start with a letter or `_` and continue with letters, digits, or `_`. Keywords are reserved and case-insensitive. Statements end with `;`. Line comments start with `--`.

```sql
-- this is a comment
select 1 + 1;
SELECT 1 + 1;          -- equivalent
```

**String literals** use `'...'` or `"..."`, interchangeable and JSON-style. Backslash escapes:

```
\"    \'    \\    \n    \t    \r    \0    \uXXXX
```

Unicode surrogates are paired JSON-style: `𐀀` encodes U+10000.

```sql
select 'hello';              -- string
select "hello";              -- equivalent
select 'it\'s';              -- escaped quote
select "it's";               -- opposite quote is literal
select 'a\nb';               -- newline
select 'A';             -- 'A' (U+0041)
```

**Keywords** (reserved, case-insensitive): `select`, `from`, `with`, `where`, `group`, `order`, `limit`, `union`, `intersect`, `except`, `as`, `asc`, `desc`, `and`, `or`, `not`, `cast`, `create`, `table`, `insert`, `into`, `delete`, `null`, `true`, `false`, `pivot`, `unpivot`.

---

## 2. Type System

### 2.1 Scalar types

Four scalar types: `null`, `bool`, `number`, `string`. The numeric type is IEEE-754 double-precision; integers up to 2^53 are exact. Cast functions `int` and `float` exist for conversions only and do not create distinct runtime types.

```sql
null
true
false
1
1.5
'hello'
```

### 2.2 Composite types

**Array**: ordered sequence of any values. **Object**: unordered map from string keys to any values. Both nest arbitrarily.

```sql
[1, 2, 3]
{"x": 1, "y": 2}
```

### 2.3 The `any` type and nullability

`any` is the top type. A field declared as `T` rejects `null`. Declare `T | null` to permit it. General unions are not supported.

```sql
any
number | null
{"x": number, "y": number | null}
```

### 2.4 Casting

Scalar type names are callable as conversion functions: `int(v)`, `float(v)`, `string(v)`, `bool(v)`, `number(v)`. `null` input yields `null`. A conversion that cannot succeed is a runtime error.

```sql
int(2.7)        -- 2
float('1.5')    -- 1.5
string(42)      -- "42"
bool('true')    -- true
number('1e3')   -- 1000.0
```

Rules:
- To `int`: floats and decimal strings truncate toward zero; `true`/`false` → `1`/`0`.
- To `float`: ints widen; numeric strings parse; `true`/`false` → `1.0`/`0.0`.
- To `string`: scalar's text form (strings unchanged, others JSON).
- To `bool`: number `0` is false, else true; `"true"`/`"false"` (case-insensitive) map to values; other strings or non-scalars error.
- To `number`: numbers keep their form; `true`/`false` → `1`/`0`; strings parse matching their literal form.

---

## 3. Expressions

### 3.1 Literals

```sql
null
true, false
1, 1.5
'string'
[1, 2, 3]
{"x": 1, "y": 2}
```

### 3.2 Variables

A bare identifier resolves to a name in the current binding tuple. Names are introduced by `from`, `with`, and `group`.

```sql
select t from T as t;
select x + 1 from T as t with t.x as x;
```

Unbound identifiers are static errors.

### 3.3 Path expressions

Navigate into objects and arrays:

```sql
t.user.name       -- object member access (null if missing)
t['user']         -- computed string key or integer index
t.items[0]        -- chained navigation
t.items[*]        -- array of all elements
t[1:5]            -- slice: elements at indices 1..5 (half-open)
t[0::2]           -- every other element starting at 0
t[:10]            -- first 10 elements
```

Rules:
- `.name` selects an object member; missing members yield `null`.
- `[expr]` selects by integer index (arrays) or string key (objects). Out-of-range yields `null`.
- `.*` and `[*]` produce an array of all values.
- Slices apply to arrays only: `start..end..step` (half-open). Defaults: start=0, end=length, step=1. Negative indices count from end.
- Recursive descent (`..`), filter selectors, and multi-selectors on values are out of scope.

**Keyed table access.** A subscript whose base is a bare table name (not bound in `from`) and whose subscript is a literal tuple is a key lookup, not path navigation. The key is matched positionally to the table's declared key columns:

- Arity == key-column count (full key): yields the one stored row (an object) or `null` on miss. `t[1]`, `c['x', 7]`.
- Arity < key-column count (partial key): yields the sub-sequence of matching rows as an array (empty on miss), in key order. `c['x']`.
- Arity 0, arity > key count, or keyless table: static error. Literal key type mismatch: schema error.

v1 restricts keys to literals; runtime-expression keys (`t[x.id]`) are deferred.

### 3.4 Object constructors

Build objects from named members and spreads.

```sql
{"x": 1, "y": 2}
{"x": 1}
{...t}            -- spread all members of t
{...t, "x": 2}    -- spread then override
```

Rules:
- Member keys must be string literals: `{"a": 1}`, not `{a: 1}`.
- Members are evaluated left to right. A later member with the same key overwrites it (last-wins) only when at least one arrives via spread. Two explicit members with the same key is a static error.
- `{...e}` requires `e` to evaluate to an object; otherwise runtime error.

### 3.5 Operators

Precedence (lowest first):

| Precedence | Operators                     | Kind            |
|------------|-------------------------------|-----------------|
| 1          | `or`                          | logical         |
| 2          | `and`                         | logical         |
| 3          | `not`                         | unary logical   |
| 4          | `=`, `!=`, `<`, `<=`, `>`, `>=` | comparison    |
| 5          | `\|\|`                        | string concat   |
| 6          | `+`, `-`                      | additive        |
| 7          | `*`, `/`, `%`                 | multiplicative  |
| 8          | unary `-`                     | numeric negate  |
| 9          | `.`, `[]`                     | postfix         |

Operators are left-associative; `not` is prefix.

```sql
a + b * c
not (x = 1) and y > 0
'hello' || ' ' || name
```

Rules:
- Arithmetic requires both operands to be `number`.
- Comparison: both operands must be the same scalar type. `null = null` is `true`; `null = x` (x ≠ null) is `false`. Ordering `null` against non-null is null (treated as not-true in `where`).
- `||` requires both operands to be `string`. Cast non-strings first.
- Logical operators require `bool` and short-circuit.
- No implicit coercion.

### 3.6 Function calls

Functions are invoked positionally. The built-in catalog is fixed.

```sql
upper(t.name)
len(t.items)
count(*)
```

**Built-in functions:**

| Function   | Signature                     | Notes                          |
|------------|-------------------------------|--------------------------------|
| `len`      | `(string \| array) → number`  | character or element count     |
| `upper`    | `(string) → string`           |                                |
| `lower`    | `(string) → string`           |                                |
| `count`    | `(any) → number`              | aggregate; `count(*)` valid    |
| `sum`      | `(number) → number`           | aggregate                      |
| `min`      | `(T) → T`                     | aggregate; T ordered           |
| `max`      | `(T) → T`                     | aggregate; T ordered           |
| `avg`      | `(number) → number`           | aggregate                      |

Aggregate functions are valid only in the `select` projection (ungrouped aggregation). They may not appear in `where`, `order by`, or `from`, nor nest inside another aggregate.

**Ungrouped aggregation** reduces the post-`where` input to exactly one output row even if input is empty:
- `count(*)` counts every row; `count(expr)`, `sum`, `min`, `max`, `avg` skip `null` arguments.
- Over empty or all-`null` input: `count` is `0`; `sum`/`min`/`max`/`avg` are `null`.
- `sum` over integers stays integer, promoting to float on overflow. `avg` is always float.
- `min`/`max` order by value; comparing incomparable types errors.

---

## 4. Queries

### 4.1 select

`select` is the final projection. Forms:

- `select .` — binding tuple as an object with one entry per binding.
- `select *` — flat spread of all bindings (equivalent to `select {...}`).
- `select expr` — single expression value.
- `select item, item, ...` — object with listed items as members.

```sql
select * from T as t;                           -- T's rows, spread
select . from T as t;                           -- [{"t": row}, ...]
select 1 + 1;                                   -- one scalar
select t.x, t.y from T as t;                    -- {"x": ..., "y": ...}
select t.x as a from T as t;                    -- {"a": ...}
```

Rules:
- `select expr` emits scalars, not objects.
- `select item, item, ...` is shorthand for `select {item, item, ...}`.
- An item `expr as name` introduces an output key. A path item (e.g., `t.x`) uses the last path segment as key.
- With no `from`, the query produces exactly one output.

### 4.1.1 pivot

`pivot value at name` replaces the `select` constructor. It folds the whole stream into a single object: each surviving tuple contributes one member `name: value`. Inverse of `unpivot`.

```sql
pivot p.price at p.sym from prices as p;
```

Rules:
- `pivot` requires a `from` clause; yields exactly one object.
- `name` must evaluate to `string`; non-string tuples contribute no member.
- Repeated `name`: last-wins.
- Empty stream yields `{}`.
- v1 restricts `pivot` to `from` and `where`; `order by` and `limit` are deferred.

### 4.2 from

`from` introduces bindings by iterating sources. Multiple sources separated by commas form a lateral cross product: each subsequent source is evaluated in the context of bindings from preceding sources.

```sql
select * from T as t;
select * from T as t, S as s;                -- cross product
select * from T as t, t.children as c;       -- lateral: c sees t
select * from T;                             -- alias defaults to T
```

Rules:
- Each source must have an alias; if omitted, the alias is the table name.
- Sources are evaluated left to right; the right side may reference bindings from the left (lateral).
- A source is a table name, path expression rooted at a bound name, parenthesized subquery, or `unpivot`.

**Unpivot** ranges over the attribute-value pairs of an object:

```sql
select sym, price
from unpivot {"amzn": 1900, "goog": 1120} as price at sym;
-- [{"sym": "amzn", "price": 1900}, {"sym": "goog", "price": 1120}]
```

Rules:
- `unpivot expr as value at name` binds the value under `as` alias and the attribute name under optional `at` alias.
- The value alias is required.
- Non-object `expr` yields no rows; the array elements are iterated as separate rows.

### 4.3 with

`with` rewrites the binding tuple, producing new bindings for clauses to its right. Its constructor takes the same forms as `select`.

```sql
select x + y from T as t with t.x as x, t.y as y;
select * from T as t with {...t, total: t.a + t.b};
```

Rules:
- `with` replaces the binding tuple. After `with`, only names from the with-constructor are in scope.
- `with *` is the flat spread of the current tuple (equivalent to `with {...}`).
- The with-constructor cannot reference output names from `select`.

### 4.4 where

`where` filters the binding-tuple stream by a boolean predicate. Tuples for which the predicate is not exactly `true` are dropped.

```sql
select * from T as t where t.x > 0;
select * from T as t where t.name = 'alice' and t.age > 18;
```

Rules:
- The predicate must evaluate to `bool`. A `null` result drops the tuple (treated as not-true).
- `where` runs after `from` and `with`, before `group`.

### 4.5 group

`group` reduces the stream into one tuple per distinct combination of group keys. Each item is either a key (any non-aggregate expression) or an aggregate.

```sql
select * from T as t group t.region as region, sum(t.amount) as total;
```

Rules:
- An item is an aggregate iff its expression contains an aggregate function call at top level.
- After `group`, only names from the group items are in scope.
- Items use `as` aliases; a path-only item without alias uses its last segment as the name.

**Status.** GROUP BY is implemented. Without `group`, no reduction occurs but aggregates in `select` work (ungrouped aggregation, 3.6).

### 4.6 order by

`order by` sorts the binding-tuple stream by one or more keys.

```sql
select * from T as t order by t.x;
select * from T as t order by t.x desc;
select * from T as t order by t.x, t.y desc;
```

Rules:
- Default direction is `asc`.
- `null` sorts last in `asc`, first in `desc`.
- Numbers order by numeric value; `1` and `1.0` compare equal.
- Sort is not guaranteed stable.

### 4.7 limit

`limit` slices the stream by row position using a range expression. The range follows Python-style half-open conventions.

```sql
select * from T as t limit 10;           -- first 10 rows
select * from T as t limit 0..10;        -- equivalent
select * from T as t limit 50..100;      -- skip 50, take 50
select * from T as t limit 50..;         -- skip 50, no upper bound
select * from T as t limit 0..100..2;    -- every other row
```

Rules:
- `limit n` is shorthand for `limit 0..n`.
- Range `start..end..step` is half-open: rows at indices `start, start+step, ...` strictly less than `end` are emitted.
- Omitted `start` defaults to 0; omitted `end` is unbounded; omitted `step` is 1.
- `start`, `end`, `step` must be non-negative integer literals.
- `step` must be >= 1.

### 4.8 Set operations

Two queries may be combined as multisets with `union`, `intersect`, or `except`. `union all` preserves duplicates.

```sql
select * from T union select * from S;
select * from T union all select * from S;
select * from T intersect select * from S;
select * from T except select * from S;
```

Rules:
- Both sides must produce values of the same JSON shape at every position.
- Equality for set ops is structural JSON equality.
- Set ops are left-associative; parenthesize to disambiguate.

### 4.9 Clause evaluation order

Regardless of source order: `from` → `with` → `where` → `group` → `order by` → `limit` → `select`. Set ops apply to the entire pipelined output of each side.

---

## 5. Schema and DDL

### 5.1 create table

`create table` declares a table. Declared columns, in declaration order, form the table's composite key.

```sql
create table points;

create table points ({
    "x": number,
    "y": number
});

create table points ({
    "x": number,
    "y": number,
    "z": number | null,
    ...
});

create table c ({"a": string, "b": number});   -- key is (a, b)
```

Rules:
- Without a schema, the table accepts any JSON object.
- With a closed schema (no `...`), inserts with extra keys error.
- Fields are non-null by default; declare `T | null` to permit `null`.
- Declared columns are the composite key; key columns must be `int` or `string`.
- Creating an existing table is a static error.

---

## 6. Mutations

### 6.1 insert

`insert` adds one or more values to a table. Sources are an explicit list of expressions or a query.

```sql
insert into points ({"x": 1, "y": 2});

insert into points (
    {"x": 1, "y": 2},
    {"x": 3, "y": 4}
);

insert into archive select * from events where ts < 1700000000;
```

Rules:
- Each value must satisfy the table's schema. Schema mismatch is an error.
- Duplicate full key replaces the row (LMDB `put` semantics, no NOOVERWRITE).

### 6.2 delete

`delete` removes rows from a table that match `where`.

```sql
delete from points where points.x < 0;
delete from users as u where u.banned = true;
delete from events;                       -- delete all rows
```

Rules:
- Without `where`, every row is deleted.
- The alias is optional; if omitted, the table name is the alias.

---

## 7. Scope and limits

- **Recursive descent** (`..` in paths), **filter selectors** (`?<expr>`), **multi-selectors** (`[a, b]` on values), **window functions**, **CTEs**, **runtime-expression keys**, **user-defined functions**, **transactions** are out of scope.
- **Integer vs float distinction** at runtime is out of scope; `int()` and `float()` are casts only.
- **Storage limits** (key length, value length, transaction concurrency) are properties of the storage layer; queries exceeding them produce runtime `storage` errors.
