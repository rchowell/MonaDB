+++
title = "Expressions"
description = "Operators, precedence, object and array constructors, and path navigation."
weight = 5
+++

# Expressions

## Operators

| Category | Operators |
|----------|-----------|
| Member access | `.`  `[expr]` |
| Arithmetic | `+`  `-`  `*`  `/`  `%` |
| Comparison | `=`  `!=`  `<`  `>`  `<=`  `>=` |
| Null tests | `is null`, `is not null`, `is true`, `is false`, `is unknown`, and their `is not …` forms |
| Range / membership | `between … and …`, `not between … and …`, `in (…)`, `not in (…)` |
| Logical | `and`  `or`  `not` |

There is no string-concatenation operator; use the `concat()` function.

Operator precedence, highest to lowest:

1. Member access: `.field`, `[index]`, `[key]`
2. `*`  `/`  `%`
3. Binary `+`  `-`
4. `=`  `!=`  `<`  `>`  `<=`  `>=`
5. `is …`, `between … and …`, `in (…)`
6. `not`
7. `and`
8. `or`

All binary operators at the same level are left-associative.

Negative numeric **literals** are written with a leading minus: `-1`, `-3.14`. There is no unary minus on expressions; write `0 - x` instead.

Parentheses override precedence. `and` binds tighter than `or`; `not` binds tighter than `and` but looser than comparisons (`not 1 = 1` is `not (1 = 1)`).

## Object Constructors

```
{ a: 1, b: 2 }
{ 'a-b': 1 }             -- string key when the name is not a bare identifier
{ ...t, extra: true }    -- spread t, add extra
{ ...a, ...b }           -- merge (later spreads win on conflict)
```

Keys are bare identifiers or string literals. A trailing comma is permitted. Shorthand `{ x, y }` is not supported — write `{ x: x, y: y }`.

## Array Constructors

```
[1, 2, 3]
[x, y, x + y]
```

## Path Navigation

Navigate into bound values with dot and bracket notation on an alias or expression.

```
t.address              -- field access
t.address.city         -- nested field
t['key']               -- bracket key (equivalent to t.key when key is an identifier)
t.tags[0]              -- array index (0-based)
```

### Unnesting in `from`

To iterate an array field, add a lateral source that paths into an earlier binding:

```
from T as t, t.items as item
```

Each outer row is paired with one element of `t.items`. A missing path, a non-array value, or an empty array contributes no rows for that outer binding.

Value sources — array literals, object arrays — also work in `from`:

```
select x from [1, 2, 3] as x;
select x.a as a from [{a: 1}, {a: 2}] as x;
```

### Keyed table lookup

When the receiver of `[…]` resolves to a catalog table with declared key columns, the subscript is a **point lookup**, not ordinary path navigation:

```
create table t (id int);
select t[1];              -- whole row, or null if absent
select t[1].v;           -- field of the looked-up row

create table c (a string, b int);
select c["x", 7];         -- composite full key
select c["x"];            -- partial key: matching rows as an array, in key order
```

On a row binding (not a table name), multi-element subscripts are not supported.
