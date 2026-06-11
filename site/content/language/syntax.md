+++
title = "Syntax"
description = "Identifiers, reserved words, literals, comments, and semicolons."
weight = 3
+++

# Syntax

## Identifiers

An identifier is any UTF-8 name that is not a reserved word. Identifiers are case-sensitive. A name that collides with a reserved word may be quoted with backticks.

```
points          -- valid
myTable         -- valid
`select`        -- quoted reserved word, valid
```

## Reserved words

The following words are reserved and may not appear as bare identifiers:

`and` `as` `asc` `by` `copy` `create` `default` `delete` `desc` `drop` `false` `fetch` `from` `group` `insert` `into` `limit` `not` `null` `or` `order` `select` `set` `step` `table` `true` `update` `where` `with`

## Literals

**Numbers** are decimal integers or floating-point values.

```
42      3.14      -1
```

**Strings** are single-quoted. Escape sequences: `\'` `\\` `\n` `\t`.

```
'hello'      'it\'s fine'
```

**Booleans**: `true` and `false`.

**Null**: `null`.

**Objects**: `{ key: value, ... }`. Keys are bare identifiers. A trailing comma is permitted.

**Arrays**: `[value, ...]`. A trailing comma is permitted.

## Comments

Single-line comments begin with `--` and run to end of line. Block comments are not supported.

```
select x from t;   -- this is a comment
```

## Semicolons

Statements are separated by semicolons. A trailing semicolon after the last statement is optional in the REPL and required in multi-statement programs.
