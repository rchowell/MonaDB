+++
title = "Insert"
description = "Insert adds one or more values to a table."
weight = 8
+++

Insert adds one or more values to a table. The values list is parenthesised and comma-separated; a trailing comma is permitted. Values may also come from a nested `select` query.

## Syntax

### Railroad

<div class="rr">
<div class="rr-track"><span class="rr-t">insert</span><span class="rr-join" aria-hidden="true"></span><span class="rr-t">into</span><span class="rr-join" aria-hidden="true"></span><span class="rr-n">table</span><span class="rr-join" aria-hidden="true"></span><span class="rr-t">(</span><span class="rr-join" aria-hidden="true"></span><span class="rr-or"><span class="rr-branch"><span class="rr-n">expr-list</span></span><span class="rr-branch"><span class="rr-n">select-stmt</span></span></span><span class="rr-join" aria-hidden="true"></span><span class="rr-t">)</span><span class="rr-join" aria-hidden="true"></span><span class="rr-t">;</span></div>
</div>

### BNF

```ebnf
insert-stmt ::= "insert" "into" identifier "(" expr-list ")" ";"
              | "insert" "into" identifier select-stmt

expr-list ::= expr ( "," expr )*
```

## Rules

1. Each inserted value must be an object that satisfies the table schema; schema mismatch is a runtime error. *(phase: execute)*
2. Duplicate full key replaces the existing row (LMDB put semantics; no `NOOVERWRITE`). *(phase: execute)*
3. Scalar or non-object values in the values list are rejected. *(phase: execute)*
4. A trailing comma after the last value in the list is permitted. *(phase: parse)*

## Examples

### Minimal

<div class="example">

#### One Object

One object in the values list produces one row.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1});

select * from T;
```

<p class="example-label">Result</p>

```json
[
  { "x": 1 }
]
```

</div>

<div class="example">

#### Empty Values

Empty values list is a no-op.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ();

select * from T;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

### Compound

<div class="example">

#### Two Objects

Two objects in one statement produce two rows.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1}, {"x": 2});

select * from T;
```

<p class="example-label">Result</p>

```json
[
  { "x": 1 },
  { "x": 2 }
]
```

</div>

<div class="example">

#### Five Objects

Five objects in one statement are all persisted.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1}, {"x": 2}, {"x": 3}, {"x": 4}, {"x": 5});

select * from T;
```

<p class="example-label">Result</p>

```json
[
  { "x": 1 },
  { "x": 2 },
  { "x": 3 },
  { "x": 4 },
  { "x": 5 }
]
```

</div>

<div class="example">

#### One Multi-value

One multi-value insert persists all rows.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1}, {"x": 2}, {"x": 3});

select * from T;
```

<p class="example-label">Result</p>

```json
[
  { "x": 1 },
  { "x": 2 },
  { "x": 3 }
]
```

</div>

<div class="example">

#### Three Single-value

Three single-value inserts produce the same table as one multi-value insert.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1});

insert into T ({"x": 2});

insert into T ({"x": 3});

select * from T;
```

<p class="example-label">Result</p>

```json
[
  { "x": 1 },
  { "x": 2 },
  { "x": 3 }
]
```

</div>

<div class="example">

#### Values In

Values in one insert may differ in shape.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1, "y": 2}, {"x": 3});

select * from T;
```

<p class="example-label">Result</p>

```json
[
  { "x": 1, "y": 2 },
  { "x": 3 }
]
```

</div>

<div class="example">

#### Values List

Values list may span multiple lines.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T (
    {"x": 1},
    {"x": 2},
    {"x": 3}
);

select * from T;
```

<p class="example-label">Result</p>

```json
[
  { "x": 1 },
  { "x": 2 },
  { "x": 3 }
]
```

</div>

### Error cases

<div class="example">

#### Object Rejected

A stored row must be an object; a scalar value is rejected.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T (1, 2, 3);
```

Expected error: `schema`

</div>

<div class="example">

#### Inserting Into

Inserting into an undeclared table is a static error.

<p class="example-label">SQL</p>

```sql
create table T;

insert into Ghost ({"x": 1});
```

Expected error: `static`

</div>

<div class="example">

#### An Array

An array value as a row is rejected with a schema error (not a panic).

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ([1, 2, 3]);
```

Expected error: `schema`

</div>

## See also

- [Create Table](@/language/statements/create-table.md)
- [Writing data](@/examples/writing.md) — more insert examples
