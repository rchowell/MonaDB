+++
title = "Types"
description = "Runtime value types and DDL type keywords for key columns."
weight = 4
+++

# Types

MonaDB uses a JSON-like type system at runtime. Values stored in tables and produced by expressions are one of null, boolean, integer, float, string, array, or object.

```
null                    -- distinct from missing object keys
true    false           -- booleans
1       -5              -- exact 64-bit signed integers
1.5     3.14            -- floats; non-finite values error on keyed insert
'hello'                 -- UTF-8 text
[1, 2, 3]               -- ordered sequence
{ x: 1, y: 2 }          -- insertion-ordered map of named fields
```

`typeof(value)` reports runtime type names: `null`, `bool`, `int`, `float`, `string`, `array`, or `object`.

## Key Columns

`create table` accepts an optional parenthesised list of **key columns**. Each column is a name followed by a type keyword. Only `int` (integer key component) and `string` (string key component) are accepted:

```
create table users (id int, name string);
create table pages (slug string);
```

The parser recognises additional type keywords (`bool`, `float`, `number`, `object`, `array`, `any`), but using any of them in a key-column position is rejected at compile time.

## Keyless Tables

A table declared without key columns accepts any JSON object (or scalar) on insert. No field-level schema checking is performed.

```
create table t;
create table t ();     -- equivalent
```

Rows in a keyless table keep surrogate ids and return in insertion order. Rows in a keyed table are sorted by their encoded key.

## Key Validation

When key columns are declared, each insert must supply every key field with the correct type. Missing or mistyped keys are schema errors. Extra payload fields beyond the declared keys are allowed.

```
create table t (x int);
insert into t ({ x: 1, note: 'ok' });   -- ok
insert into t ({ note: 'no key' });   -- schema error
insert into t ({ x: 'a' });           -- schema error
```

Explicit cast syntax is not implemented. Values retain their runtime types.
