# MonaDB Language Specification

> Status: draft, normative. This is the contract; the implementation is expected to converge on it.

MonaDB is an embedded, LMDB-backed database for semi-structured data. Its query language (RQL) is a small set of stream operators over JSON-shaped values. This document defines the complete language. Everything not described here is out of scope.

Each operator and statement is documented in four blocks: **Description**, **Examples**, **Rules**, **Syntax** (mckeeman grammar).

---

## 1. Introduction

### 1.1 Binding-tuple model

A query is a pipeline of clauses that transforms a stream of **binding tuples**. A binding tuple is a JSON object whose keys are names introduced by `from` or `with`, and whose values are the rows or expressions bound to those names.

```
| clause | action          |
|--------|-----------------|
| from   | iterate         |
| with   | rewrite         |
| where  | filter          |
| group  | reduce          |
| order  | sort            |
| limit  | slice           |
| select | project (final) |
```

Every clause produces a stream of binding tuples; `select` is the final projection that turns each tuple into the output value. Two queries combined with `union`, `intersect`, or `except` are evaluated independently and combined as multisets.

### 1.2 Lexical grammar

**Description.** The lexer is whitespace-insensitive outside of string literals. Identifiers start with a letter or `_` and continue with letters, digits, or `_`. Keywords are reserved and case-insensitive. Statements are terminated by `;`. Line comments start with `--` and run to end of line.

**Examples.**
```rql
-- this is a comment
select 1 + 1;
SELECT 1 + 1;          -- equivalent
```

**Rules.**
1. Keywords are reserved: `select`, `from`, `with`, `where`, `group`, `order`, `limit`, `union`, `intersect`, `except`, `as`, `asc`, `desc`, `and`, `or`, `not`, `cast`, `create`, `table`, `insert`, `into`, `update`, `set`, `delete`, `null`, `true`, `false`.
2. Keywords are case-insensitive; identifiers are case-sensitive.
3. String literals use single quotes; embedded single quotes are escaped by doubling: `'it''s'`.
4. Number literals are decimal; no octal or hex.
5. Every statement ends with `;`.

**Syntax.**
```mckeeman
identifier
    letter
    letter ident_tail

ident_tail
    letter
    digit
    "_"
    letter ident_tail
    digit ident_tail
    "_" ident_tail

string_lit
    "'" string_chars "'"

number_lit
    int_part
    int_part "." digits
    int_part "." digits exponent

bool_lit
    "true"
    "false"

null_lit
    "null"
```

---

## 2. Type System

### 2.1 Scalar types

**Description.** Four scalar types: `null`, `bool`, `number`, `string`. There is exactly one numeric type, IEEE-754 double-precision float. The names `int` and `float` exist only as cast helpers (§2.4) and as schema refinements (§5.1); they are not distinct runtime types.

**Examples.**
```rql
null
true
false
1
1.5
'hello'
```

**Rules.**
1. `null` is a value, a type, and the only inhabitant of its type.
2. `number` is IEEE-754 double; integers up to 2^53 are exact.
3. `string` is a sequence of Unicode code points; encoding is UTF-8.

**Syntax.**
```mckeeman
t_scalar
    "null"
    "bool"
    "number"
    "string"
```

### 2.2 Composite types

**Description.** Two composite types: `array` (ordered sequence of any values) and `object` (unordered map from string keys to any values). Object types may be **closed** (exact keys) or **open** (additional keys allowed).

**Examples.**
```rql
[1, 2, 3]
{ x: 1, y: 2 }
{ x: number, y: number }            -- closed object type
{ x: number, y: number, ... }       -- open object type
[number]                             -- array of numbers
```

**Rules.**
1. An array type is `[T]` where `T` is any type.
2. A closed object type lists every permitted key; unknown keys are an error at write time.
3. An open object type ends with `...` and permits additional keys.
4. Arrays and objects may nest.

**Syntax.**
```mckeeman
t_array
    "array"
    "[" type "]"

t_object
    "object"
    "{" "}"
    "{" t_members "}"
    "{" t_members "," "..." "}"

t_members
    t_member
    t_member "," t_members

t_member
    identifier ":" type
```

### 2.3 The `any` type and nullability

**Description.** `any` is the top type; every value inhabits it. A type may be made nullable by union with `null`: `T | null`. By default, schema fields are non-null.

**Examples.**
```rql
any
number | null
{ x: number, y: number | null }
```

**Rules.**
1. A field declared as `T` rejects `null`; declare `T | null` to permit it.
2. `any` is the type of unconstrained values and is always nullable.
3. The only union form is `T | null`. General unions are not supported.

**Syntax.**
```mckeeman
type
    "any"
    t_scalar
    t_array
    t_object
    type "|" "null"
```

### 2.4 Casting and coercion

**Description.** Three equivalent forms convert values: `cast(v as T)`, `v::T`, and `T(v)`. Conversions are explicit; there is no implicit coercion between types except where this section names one.

**Examples.**
```rql
cast('3.14' as number)
'3.14'::number
number('3.14')
```

**Rules.**
1. To **bool**: `null → false`, `0 → false`, any other number → `true`, `'' → false`, any other string → `true`, `[] → false`, any other array → `true`, `{} → false`, any other object → `true`.
2. To **number**: `false → 0`, `true → 1`, numeric string → its parsed value, `null` → error, array/object → error.
3. To **string**: every value has a canonical JSON representation; cast produces it.
4. To **null**: only `null` casts to `null`; all others are an error.
5. `int(v)` is `cast(v as number)` truncated toward zero. `float(v)` is `cast(v as number)`.
6. Casting to a structured type (`array`, `object`, `[T]`, `{...}`) requires the value already match the shape; otherwise it is an error.

**Syntax.**
```mckeeman
expr_cast
    "cast" "(" expr "as" type ")"
    expr "::" type
    type "(" expr ")"
```

---

## 3. Expressions

### 3.1 Literals

```mckeeman
expr_lit
    null_lit
    bool_lit
    number_lit
    string_lit
    array_lit
    object_lit

array_lit
    "[" "]"
    "[" expr_list "]"

expr_list
    expr
    expr "," expr_list
```

### 3.2 Variables and bindings

**Description.** A bare identifier in an expression resolves to a name in the current binding tuple. Names are introduced by `from` (§4.2), `with` (§4.3), and `group` (§4.5).

**Examples.**
```rql
select t from T as t;
select x + 1 from T as t with t.x as x;
```

**Rules.**
1. An unbound identifier is a static error.
2. Bindings are resolved in lexical order: clauses to the left are visible to clauses to the right.

**Syntax.**
```mckeeman
expr_var
    identifier
```

### 3.3 Path expressions

**Description.** Path expressions navigate into objects and arrays. Five segment forms are supported: `.name`, `[expr]`, `.*`, `[start:end]`, and `[start:end:step]`.

**Examples.**
```rql
t.user.name
t['user']
t.items[0]
t.items[*]
t.items[1:5]
t.items[0::2]      -- every other element
```

**Rules.**
1. `.name` selects an object member; missing members yield `null`.
2. `[expr]` selects by integer index (arrays) or by computed string key (objects). Out-of-range index yields `null`.
3. `.*` and `[*]` produce an array of all values: object member values or array elements.
4. Slices apply to arrays only; `start` defaults to 0, `end` to the array length, `step` to 1.
5. Negative indices count from the end.
6. Recursive descent (`..`), filter selectors (`?<expr>`), and multi-selectors (`[a, b]`) are not in v1 (Appendix A).

**Syntax.**
```mckeeman
path_segment
    "." identifier
    "." "*"
    "[" expr "]"
    "[" "*" "]"
    "[" slice "]"

slice
    expr_opt ":" expr_opt
    expr_opt ":" expr_opt ":" expr_opt

expr_opt
    expr
    ""
```

### 3.4 Object constructors

**Description.** Object constructors build objects from named members, shorthand bindings, and spreads.

**Examples.**
```rql
{ x: 1, y: 2 }
{ x, y }                  -- shorthand: { x: x, y: y }
{ t.x }                   -- shorthand: { x: t.x }    (last path segment is the key)
{ ...t }                  -- spread all members of t
{ ...t, x: t.x + 1 }      -- spread then override
```

**Rules.**
1. `{ a }` is shorthand for `{ a: a }`; `a` must be a bound name.
2. `{ p.x.y }` is shorthand for `{ y: p.x.y }`: the last segment of the path supplies the key. The path's final segment must be an identifier (not `[expr]`, `[*]`, or a slice).
3. `{ ...e }` requires `e` to evaluate to an object; otherwise it is a runtime error.
4. Members are evaluated left to right. A later member with the same key as an earlier one overwrites it (last-wins) — but **only when at least one of the two arrives via spread**. Two explicit members with the same key in one literal is a static error.
5. Member keys may be identifiers or string literals; `{ 'a-b': 1 }` is permitted.

**Syntax.**
```mckeeman
expr_obj
    "{" "}"
    "{" obj_members "}"

obj_members
    obj_member
    obj_member "," obj_members

obj_member
    identifier
    path_shorthand
    member_key ":" expr
    "..." expr

path_shorthand
    expr_var path_segments_id

path_segments_id
    "." identifier
    "." identifier path_segments_id

member_key
    identifier
    string_lit
```

### 3.5 Operators

**Description.** Operators are grouped by precedence, lowest first. All operators are left-associative except `not` (prefix) and `::` (postfix).

| Precedence | Operators                          | Kind            |
|------------|------------------------------------|-----------------|
| 1 (lowest) | `or`                               | logical         |
| 2          | `and`                              | logical         |
| 3          | `not`                              | unary logical   |
| 4          | `=`, `!=`, `<`, `<=`, `>`, `>=`    | comparison      |
| 5          | `\|\|`                             | string concat   |
| 6          | `+`, `-`                           | additive        |
| 7          | `*`, `/`, `%`                      | multiplicative  |
| 8          | unary `-`                          | numeric negate  |
| 9          | `::`, `.`, `[]`                    | postfix         |

**Examples.**
```rql
a + b * c
not (x = 1) and y > 0
'hello' || ' ' || name
-x
```

**Rules.**
1. Arithmetic operators require both operands to be `number`; otherwise error.
2. Comparison operators require both operands to be the same scalar type. `null = null` is `true`; `null = x` is `false` for any `x ≠ null`. `null` ordering against non-null is an error.
3. `||` requires both operands to be `string`. To concatenate non-strings, cast first.
4. Logical operators require `bool`; they short-circuit.
5. There is no implicit coercion. `1 + '1'` is an error.

**Syntax.**
```mckeeman
expr_op
    expr binop expr
    unop expr

binop
    "or" | "and" | "=" | "!=" | "<" | "<=" | ">" | ">="
    "||" | "+" | "-" | "*" | "/" | "%"

unop
    "not" | "-"
```

### 3.6 Function calls

**Description.** Functions are invoked positionally. The built-in catalog is small and fixed for v1.

**Examples.**
```rql
upper(t.name)
len(t.items)
count(*)
```

**Built-in catalog (v1).**

| Function   | Signature                       | Notes                              |
|------------|---------------------------------|------------------------------------|
| `len`      | `(string \| array) → number`    | character count or element count   |
| `upper`    | `(string) → string`             |                                    |
| `lower`    | `(string) → string`             |                                    |
| `count`    | `(any) → number`                | aggregate (§4.5); `count(*)` valid |
| `sum`      | `(number) → number`             | aggregate                          |
| `min`      | `(T) → T`                       | aggregate; `T` ordered             |
| `max`      | `(T) → T`                       | aggregate; `T` ordered             |
| `avg`      | `(number) → number`             | aggregate                          |

**Rules.**
1. Calling an unknown function is a static error.
2. Aggregate functions are valid only inside `group` (§4.5).

**Syntax.**
```mckeeman
expr_call
    identifier "(" ")"
    identifier "(" expr_list ")"
    "count" "(" "*" ")"
```

---

## 4. Data Query Language (DQL)

A query is a `select` followed by zero or more clauses. The clauses must appear in this order: `from`, `with`, `where`, `group`, `order`, `limit`. A query may be combined with another query via `union`, `intersect`, or `except`.

The grammar root for the entire language is:

```mckeeman
statement
    query ";"
    create_stmt ";"
    insert_stmt ";"
    update_stmt ";"
    delete_stmt ";"
```

### 4.1 select

**Description.** `select` is the final projection. It runs once per binding tuple after all other clauses and produces one output value per tuple.

The constructor takes one of four forms:
- `.` — emit the binding tuple itself, as an object with one entry per binding.
- `*` — emit the **flat spread** of all bindings: equivalent to `{ ...b1, ...b2, ... }` in binding order.
- `expr` — emit a single expression value.
- `item, item, ...` — emit an object whose members are the listed items.

**Examples.**
```rql
select * from T as t;                       -- T's rows, flattened
select . from T as t;                       -- [{ t: <row> }, ...]
select 1 + 1;                               -- one scalar; no from
select t.x, t.y from T as t;                -- { x: t.x, y: t.y } per row
select { x: t.x, y: t.y } from T as t;      -- equivalent
select t.x as a from T as t;                -- { a: t.x }
```

**Rules.**
1. `select *` and `select { ... }` are equivalent.
2. `select expr` (a single bare expression) emits scalars, not objects.
3. `select <ident>, <ident>, ...` is shorthand for `select { <ident>, <ident>, ... }` (object with shorthand members).
4. An item `expr as name` introduces an output key. An item `path` (e.g., `t.x`) uses the last path segment as the key.
5. With no `from` clause, the query produces exactly one output (the constructor evaluated once with an empty binding tuple).

**Syntax.**
```mckeeman
select_stmt
    "select" select_ctor query_body_opt

select_ctor
    "."
    "*"
    expr
    select_list

select_list
    select_item
    select_item "," select_list

select_item
    expr
    expr "as" identifier

query_body_opt
    ""
    query_body

query_body
    from_clause with_clause_opt where_clause_opt group_clause_opt order_clause_opt limit_clause_opt
```

### 4.2 from

**Description.** `from` introduces bindings by iterating one or more sources. Multiple sources separated by commas form a **lateral cross product**: each subsequent source is evaluated in the context of the bindings produced by the preceding sources, and all combinations are emitted.

**Examples.**
```rql
select * from T as t;
select * from T as t, S as s;             -- cross product
select * from T as t, t.children as c;    -- lateral: c sees t
select * from T;                          -- alias defaults to T
```

**Rules.**
1. Each source must have an alias; if omitted, the alias is the table name.
2. Sources are evaluated left to right; the right side may reference bindings from the left (lateral).
3. A source is a table name, a path expression rooted at a previously bound name, or a parenthesized subquery.
4. After `from`, the binding tuple has one key per source, with the value being the current row of that source.

**Syntax.**
```mckeeman
from_clause
    "from" from_sources

from_sources
    from_source
    from_source "," from_sources

from_source
    table_name from_alias_opt
    expr from_alias_opt
    "(" select_stmt ")" from_alias_opt

from_alias_opt
    ""
    "as" identifier
```

### 4.3 with

**Description.** `with` rewrites the binding tuple. Its constructor takes the same forms as `select` (§4.1), but instead of producing the output it produces the new bindings used by clauses to its right (`where`, `group`, `order`, `limit`, `select`).

**Examples.**
```rql
select x + y from T as t with t.x as x, t.y as y;
select * from T as t with { ...t, total: t.a + t.b };
select . from T as t with *;              -- rebinds to the flat spread of t
```

**Rules.**
1. `with` **replaces** the binding tuple. After `with`, only the names introduced by the with-constructor are in scope.
2. `with *` is the flat spread of the current tuple, equivalent to `with { ... }`.
3. The with-constructor cannot reference output names from `select`.
4. To spread a single binding, write `with { ...t }` or `with t.x as x, t.y as y`. There is no `t.*` spread shorthand; `.*` in any expression is the path wildcard from §3.3.

**Syntax.**
```mckeeman
with_clause_opt
    ""
    "with" select_ctor
```

### 4.4 where

**Description.** `where` filters the binding-tuple stream by a boolean predicate. Tuples for which the predicate is not exactly `true` are dropped.

**Examples.**
```rql
select * from T as t where t.x > 0;
select * from T as t where t.name = 'alice' and t.age > 18;
```

**Rules.**
1. The predicate must evaluate to `bool`. A `null` result drops the tuple (treated as not-true).
2. `where` runs after `from` and `with`, before `group`.

**Syntax.**
```mckeeman
where_clause_opt
    ""
    "where" expr
```

### 4.5 group

**Description.** `group` reduces the stream into one tuple per distinct combination of group keys. Each item in the group clause is either a **key** (any non-aggregate expression) or an **aggregate** (a call to `count`/`sum`/`min`/`max`/`avg`). After `group`, the binding tuple has one key per item.

**Examples.**
```rql
select * from T as t group t.region as region, sum(t.amount) as total;
select region, total from T as t
    group t.region as region, sum(t.amount) as total
    where total > 1000;
```

**Rules.**
1. Items use `as` aliases just like `select` (§4.1).
2. A path-only item (e.g., `t.region`) without an alias takes its last segment as the name.
3. An item is an aggregate if and only if its expression contains an aggregate function call at the top level. Mixed aggregate/non-aggregate inside one expression is a static error.
4. After `group`, only the names introduced by the group items are in scope. The original `from` bindings are no longer visible.
5. `group` is optional. Without it, no reduction occurs.

**Syntax.**
```mckeeman
group_clause_opt
    ""
    "group" group_items

group_items
    group_item
    group_item "," group_items

group_item
    expr
    expr "as" identifier
```

### 4.6 order

**Description.** `order` sorts the binding-tuple stream by one or more keys.

**Examples.**
```rql
select * from T as t order t.x;
select * from T as t order t.x desc;
select * from T as t order t.x, t.y desc;
```

**Rules.**
1. Default direction is `asc`.
2. `null` sorts last in `asc`, first in `desc`.
3. Heterogeneous types in the same key are an error (compare strings with strings, numbers with numbers).
4. The sort is **not** guaranteed to be stable.

**Syntax.**
```mckeeman
order_clause_opt
    ""
    "order" order_items

order_items
    order_item
    order_item "," order_items

order_item
    expr
    expr "asc"
    expr "desc"
```

### 4.7 limit

**Description.** `limit` slices the stream by row position using a single range expression. The range follows Python-style half-open conventions.

**Examples.**
```rql
select * from T as t limit 10;          -- first 10 rows
select * from T as t limit 0..10;       -- equivalent
select * from T as t limit 50..100;     -- skip 50, take 50
select * from T as t limit 50..;        -- skip 50, no upper bound
select * from T as t limit ..100;       -- equivalent to limit 100
select * from T as t limit 0..100..2;   -- every other row in the first 100
```

**Rules.**
1. `limit n` is shorthand for `limit 0..n`.
2. The range `start..end..step` is half-open: rows at indices `start, start+step, start+2*step, ...` strictly less than `end` are emitted.
3. Omitted `start` defaults to 0; omitted `end` is unbounded; omitted `step` is 1.
4. `start`, `end`, and `step` are non-negative integer literals. Negative or non-integer is a static error.
5. `step` must be `≥ 1`.

**Syntax.**
```mckeeman
limit_clause_opt
    ""
    "limit" limit_range

limit_range
    number_lit
    range

range
    int_opt ".." int_opt
    int_opt ".." int_opt ".." int_opt

int_opt
    ""
    number_lit
```

### 4.8 Set operations

**Description.** Two queries may be combined as multisets with `union` (concatenate and deduplicate), `intersect`, or `except`. `union all` preserves duplicates.

**Examples.**
```rql
select * from T union select * from S;
select * from T union all select * from S;
select * from T intersect select * from S;
select * from T except select * from S;
```

**Rules.**
1. Both sides must produce values of the same shape: same JSON type at every position. Heterogeneous shapes are a static error.
2. Equality for set ops is structural JSON equality.
3. Set ops are left-associative; precedence is below all clause-level operators (parenthesize subqueries to disambiguate).

**Syntax.**
```mckeeman
query
    select_stmt
    query set_op select_stmt

set_op
    "union"
    "union" "all"
    "intersect"
    "except"
```

### 4.9 Clause evaluation order

Clauses execute in this order regardless of source order: `from` → `with` → `where` → `group` → `order` → `limit` → `select`. Set ops apply to the entire pipelined output of each side.

---

## 5. Data Definition Language (DDL)

### 5.1 create table

**Description.** `create table` declares a table. The schema may be omitted (schema-less, accepts any object), declared inline, or declared open. Table options specify the partition key and sort key for the underlying storage.

**Examples.**
```rql
-- schema-less
create table points;

-- closed schema
create table points ({
    x: number,
    y: number,
});

-- open schema with nullable field
create table points ({
    x: number,
    y: number,
    z: number | null,
    ...
});

-- with partition/sort keys
create table events (
    { user_id: string, ts: number, payload: object, ... },
    { hash: user_id, sort: ts }
);

-- single-column primary key
create table users ({ id: string, name: string }, { key: id });
```

**Rules.**
1. Without a schema, the table accepts any JSON object.
2. With a closed schema (no trailing `...`), inserts with extra keys are a static error.
3. Fields are non-null by default; declare `T | null` to permit `null`.
4. Table options are an object with at most three keys: `key`, `hash`, `sort`. `key` is shorthand for a single-column primary key; `hash` and `sort` together define a composite (partition, sort) key.
5. `key` and (`hash` + `sort`) are mutually exclusive.
6. If no key option is provided, the table has an implicit auto-assigned row identifier.
7. Creating a table that already exists is a static error.

**Syntax.**
```mckeeman
create_stmt
    "create" "table" identifier table_body_opt

table_body_opt
    ""
    "(" ")"
    "(" t_object ")"
    "(" t_object "," table_options ")"

table_options
    "{" table_option_list "}"

table_option_list
    table_option
    table_option "," table_option_list

table_option
    "key" ":" identifier
    "hash" ":" identifier
    "sort" ":" identifier
```

---

## 6. Data Manipulation Language (DML)

### 6.1 insert

**Description.** `insert` adds one or more values to a table. Sources are an explicit list of expressions or a query.

**Examples.**
```rql
insert into points ({ x: 1, y: 2 });

insert into points (
    { x: 1, y: 2 },
    { x: 3, y: 4 }
);

insert into archive select * from events where ts < 1700000000;
```

**Rules.**
1. Each value must satisfy the table's schema (§5.1). Schema mismatch is an error.
2. Values violating the table's key uniqueness are an error.
3. The query form must produce values of the table's shape.

**Syntax.**
```mckeeman
insert_stmt
    "insert" "into" identifier "(" expr_list ")"
    "insert" "into" identifier select_stmt
```

### 6.2 update

**Description.** `update` rewrites every binding tuple in a table that matches `where` by replacing it with the value of an expression. The expression typically uses spread to retain unchanged fields.

**Examples.**
```rql
update points as p set { ...p, x: p.x + 1 };

update users as u
    set { ...u, last_seen: now() }
    where u.id = 'alice';
```

**Rules.**
1. The `set` expression must evaluate to an object that satisfies the table's schema.
2. Without a `where` clause, every row is updated.
3. The alias is required; the expression must reference it.
4. Updates may not change the value of a `key` or `hash` column. To change them, delete and re-insert.

**Syntax.**
```mckeeman
update_stmt
    "update" identifier "as" identifier "set" expr where_clause_opt
```

### 6.3 delete

**Description.** `delete` removes binding tuples from a table that match `where`.

**Examples.**
```rql
delete from points where points.x < 0;
delete from users as u where u.banned = true;
delete from events;                       -- delete all rows
```

**Rules.**
1. Without a `where` clause, every row is deleted.
2. The alias is optional; if omitted, the table name is the alias.

**Syntax.**
```mckeeman
delete_stmt
    "delete" "from" identifier from_alias_opt where_clause_opt
```

---

## 7. Storage Model (informative)

This section is non-normative. MonaDB is implemented on LMDB; see [resources/lmdb.adoc](resources/lmdb.adoc) for the full design. Each table maps to one primary B+ tree keyed by the `(hash, sort)` composite if specified, or by `key` if specified, or by an implicit row identifier otherwise. Sort-order-preserving byte encoding of keys is the responsibility of the storage layer; the language is unaware of the encoding.

Storage limits (key length, value length, transaction concurrency) are properties of the storage layer and are not surfaced as language constructs. A query that exceeds a storage limit produces a runtime error of category `storage`.

---

## Appendix A — Out of scope for v1

Each item below is a deliberate omission. Add only with explicit language-design discussion.

| Feature                                       | Rationale                                                         |
|-----------------------------------------------|-------------------------------------------------------------------|
| `copy ... to <file>`                          | File I/O is orthogonal to the language; belongs in a tools layer. |
| `create type`, `create view`, `create index`  | Minimal DDL; types live inline, indexes are an implementation concern. |
| Transactions (`create transaction`, `commit`, `abort`) | Not yet specified; will be added when concurrency story stabilizes. |
| User-defined functions                        | The built-in catalog (§3.6) is the entire function surface for v1. |
| JSONPath filter selectors `?<expr>`           | Filter inside paths duplicates `where`; pick one.                 |
| Recursive descent `..` in paths               | Power-tool; can be added later without breaking changes.          |
| Multi-selectors `[a, b]` in paths             | Same rationale.                                                   |
| Single-record `T@<id>` syntax                 | Pure sugar over `where ... = <id>`; can be added later.           |
| `inner` / `left` / `outer` join keywords      | Lateral cross + `where` covers all join shapes.                   |
| Window functions, CTEs                        | `with` is reserved for binding rewrite, not CTEs.                 |
| Three-valued logic in `where`                 | `null` is treated as not-true; comparisons are strict (§3.5).      |
| Integer vs float distinction at runtime       | One numeric type (§2.1); `int()`/`float()` are casts only.         |

---

## Appendix B — Open semantic decisions (resolved here)

For each, this spec adopts the listed position. Future revisions may reopen these.

1. **`select *` ≡ `select { ... }`** — flat spread of all bindings. `select .` returns the binding tuple as an envelope object. (§4.1)
2. **`from T, S`** is a left-to-right **lateral** cross product. (§4.2)
3. **`with`** **replaces** the binding tuple. Names from `from` are not in scope after `with`. (§4.3)
4. **`group`** uses **implicit** keying: items without aggregate calls are keys; items with aggregate calls are reductions. No separate `by` clause. (§4.5)
5. **`order`** is **not stable** by default. (§4.6)
6. **Set ops** require **strict shape match**. (§4.8)
7. **`update`** uses an **object-uniform** form: `set <expr>`, not `set col = val`. (§6.2)
8. **`delete`** uses `delete from T where ...` as the general form; `@<id>` sugar is deferred to Appendix A.
9. **Object duplicates**: explicit duplicate keys in one literal are a **static error**; spread merges are **last-wins**. (§3.4)
10. **`null` comparison** is **strict**: `null = null` is `true`, `null = x` (x ≠ null) is `false`, ordering against `null` is `null` and treated as not-true in `where`. (§3.5, §4.4)
11. **No `t.*` spread shorthand.** `.*` is exclusively the path wildcard (§3.3). To spread a binding, write `{ ...t }` or list members explicitly. The earlier draft in [docs/sql/select.adoc](sql/select.adoc) used `select t.* from T as t` as a synonym for `select { ...t } from T as t`; this synonym is dropped to keep `.*` unambiguous.
