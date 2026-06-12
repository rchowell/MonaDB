+++
title = "Syntax"
description = "Identifiers, reserved words, literals, and comments."
weight = 3
+++

# Syntax

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
"hello"         -- double-quoted string
'it''s fine'    -- embedded single quote
"say ""hi"""    -- embedded double quote
'\n'            -- two-character string, not a newline
true            -- boolean
false           -- boolean
null            -- null
{ x: 1, y: 2 }  -- object
{ ...t, z: 3 }  -- object with spread
[1, 2, 3]       -- array
[1, 2, ]        -- array with trailing comma
```

## Comments

Single-line comments begin with `--` and run to end of line. Block comments are not supported.

```
select x from t;   -- this is a comment
```

## Reserved Words

The following words are reserved and may not appear as bare identifiers:

| Keyword | Role |
|---------|------|
| `and` | Logical conjunction |
| `as` | Source alias in `from` |
| `asc` | Ascending sort order |
| `between` | Range predicate |
| `by` | `order by` clause |
| `clear` | `clear` statement |
| `create` | `create table` statement |
| `delete` | `delete` statement |
| `desc` | Descending sort order |
| `drop` | `drop table` statement |
| `false` | Boolean literal |
| `from` | `from` clause |
| `in` | Membership predicate |
| `insert` | `insert` statement |
| `into` | `insert into` target |
| `int` | Integer key-column type |
| `is` | Null and truth tests |
| `limit` | `limit` clause |
| `not` | Logical negation |
| `null` | Null literal |
| `or` | Logical disjunction |
| `order` | `order by` clause |
| `select` | `select` clause |
| `string` | String key-column type |
| `table` | `create table` / `drop table` |
| `true` | Boolean literal |
| `unknown` | `is unknown` test |
| `where` | `where` clause |
