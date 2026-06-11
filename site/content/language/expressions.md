+++
title = "Expressions"
description = "Operators, precedence, object constructors, path traversal, and type coercion."
weight = 5
+++

# Expressions

## Operators

| Category | Operators |
|----------|-----------|
| Arithmetic | `+`  `-`  `*`  `/`  `%` |
| Comparison | `=`  `!=`  `<`  `>`  `<=`  `>=` |
| Logical | `and`  `or`  `not` |
| String | `\|\|` (concatenation) |

Operator precedence, highest to lowest:

1. Unary `-`, `not`
2. `*`  `/`  `%`
3. `+`  `-`  `||`
4. `=`  `!=`  `<`  `>`  `<=`  `>=`
5. `and`
6. `or`

All binary operators are left-associative at the same level.

## Object constructors

```
{ a: 1, b: 2 }
{ ...t, extra: true }    -- spread t, add extra
{ ...a, ...b }           -- merge (b wins on conflict)
{ x, y }                 -- shorthand for { x: x, y: y }
```

A trailing comma is permitted.

## Array constructors

```
[1, 2, 3]
[x, y, x + y]
```

## Path traversal

Path traversal is rooted at a table or variable with `$`. Single field access collapses to the value. Use in `from` to iterate a nested collection.

```
T$.address              -- the address field of T
T$.tags[0]              -- first element of tags
T$['key']               -- bracket notation, equivalent to T$.key
T$[x, y]                -- select fields x and y

from T$.items as item   -- iterate the items array of each row
```

## Cast and coercion

Three interchangeable forms:

```
cast(v as bool)
v::bool
bool(v)
```

| From | → bool | → number | → string |
|------|--------|----------|----------|
| `0` | `false` | — | `'0'` |
| `''` | `false` | — | — |
| `[]` | `false` | — | — |
| `true` | — | `1` | `'true'` |
| `false` | — | `0` | `'false'` |
| `'3.14'` | `true` | `3.14` | — |
| `null` | `false` | `0` | `'null'` |
