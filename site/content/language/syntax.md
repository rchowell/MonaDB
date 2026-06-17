+++
title = "Syntax"
description = "Identifiers, reserved words, literals, and comments."
weight = 2
+++

## Identifiers

An identifier is a name matching `[a-zA-Z_][a-zA-Z0-9_]*` that is not a reserved word. Identifiers are case-sensitive. Keywords must be written in lowercase. Quoted identifiers are not supported.

```
points          -- valid
myTable         -- valid
select          -- reserved; cannot be used as a bare identifier
```

## Literals

```
42              -- integer
3.14            -- float
-1              -- negative number
'hello'         -- single-quoted string
"hello"         -- double-quoted string (equivalent)
'it\'s'         -- escaped single quote
"say \"hi\""    -- escaped double quote
'a\nb'          -- newline escape
'a\\b'          -- escaped backslash
'\u263A'        -- unicode escape (☺)
true            -- boolean
false           -- boolean
null            -- null
{"x": 1, "y": 2}  -- object with string keys
{...t, "z": 3}  -- object with spread
[1, 2, 3]       -- array
[1, 2, ]        -- array with trailing comma
```

String literals use either `'...'` or `"..."` (interchangeable, JSON-style). Backslash escapes:

- `\"` — double quote
- `\'` — single quote
- `\\` — backslash
- `\n` — newline
- `\t` — tab
- `\r` — carriage return
- `\0` — null byte
- `\uXXXX` — unicode scalar (4 hex digits; surrogates paired JSON-style)

The opposite delimiter needs no escape inside a string.

## Comments

Single-line comments begin with `--` and run to end of line. Block comments are not supported.

```
select x from t;   -- this is a comment
```

## Reserved Words

The following words are reserved and may not appear as bare identifiers:

| Keyword | Role |
|---------|------|
| `all` | Subquery quantifier |
| `and` | Logical conjunction |
| `any` | Type name |
| `array` | Type name |
| `as` | Source alias in `from` |
| `asc` | Ascending sort order |
| `at` | Attribute name in `pivot` / `unpivot` |
| `between` | Range predicate |
| `bool` | Type name |
| `by` | `order by` / `group by` clause |
| `clear` | `clear` statement |
| `copy` | `copy` statement |
| `create` | `create table` statement |
| `delete` | `delete` statement |
| `desc` | Descending sort order |
| `drop` | `drop table` statement |
| `exists` | Subquery existence test |
| `false` | Boolean literal |
| `float` | Type name |
| `from` | `from` clause |
| `group` | `group by` clause |
| `having` | `having` clause |
| `in` | Membership predicate |
| `insert` | `insert` statement |
| `into` | `insert into` target |
| `int` | Integer key-column type |
| `is` | Null and truth tests |
| `limit` | `limit` clause |
| `not` | Logical negation |
| `null` | Null literal |
| `number` | Type name |
| `object` | Type name |
| `or` | Logical disjunction |
| `order` | `order by` clause |
| `pivot` | `pivot` clause |
| `select` | `select` clause |
| `string` | String key-column type |
| `table` | `create table` / `drop table` / `clear table` |
| `to` | `copy … to` target path |
| `true` | Boolean literal |
| `unknown` | `is unknown` test |
| `unpivot` | `unpivot` source |
| `where` | `where` clause |
