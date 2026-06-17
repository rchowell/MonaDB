+++
title = "Delete"
description = "Delete removes rows from a table."
weight = 9
+++

Delete removes rows from a table. An optional `where` predicate restricts which rows are removed; without it, every row in the table is deleted.

## Syntax

### Railroad

<div class="rr">
<div class="rr-track"><span class="rr-t">delete</span><span class="rr-join" aria-hidden="true"></span><span class="rr-t">from</span><span class="rr-join" aria-hidden="true"></span><span class="rr-n">table</span><span class="rr-join" aria-hidden="true"></span><span class="rr-opt"><span class="rr-opt-inner"><span class="rr-t">as</span><span class="rr-join" aria-hidden="true"></span><span class="rr-n">alias</span></span></span><span class="rr-join" aria-hidden="true"></span><span class="rr-opt"><span class="rr-opt-inner"><span class="rr-t">where</span><span class="rr-join" aria-hidden="true"></span><span class="rr-n">expr</span></span></span><span class="rr-join" aria-hidden="true"></span><span class="rr-t">;</span></div>
</div>

### BNF

```ebnf
delete-stmt ::= "delete" "from" identifier [ "as" identifier ] [ where-clause ] ";"
```

## Rules

1. Without `where`, every row in the table is removed. *(phase: execute)*
2. The table alias is optional; when omitted, the table name is the binding name in the predicate. *(phase: execute)*
3. The where predicate follows the same boolean semantics as query `where` (null is not-true). *(phase: execute)*

## Examples

### Minimal

<div class="example">

#### Deleting From

Deleting from an empty table succeeds and yields nothing.

<p class="example-label">SQL</p>

```sql
create table T;

delete from T;

select * from T;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

### Compound

<div class="example">

#### Empties Table

Delete with no where removes every row.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1}, {"x": 2}, {"x": 3});

delete from T;

select * from T;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

#### Matching Rows

Delete with a predicate removes only matching rows.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1}, {"x": 2}, {"x": 3});

delete from T where T.x > 1;

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

#### An Explicit

An explicit `as` alias binds the predicate's references.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1}, {"x": 2}, {"x": 3});

delete from T as r where r.x = 2;

select * from T;
```

<p class="example-label">Result</p>

```json
[
  { "x": 1 },
  { "x": 3 }
]
```

</div>

<div class="example">

#### Table Unchanged

A predicate that matches nothing leaves the table unchanged.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1}, {"x": 2}, {"x": 3});

delete from T where T.x > 100;

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

#### Reuses Table

A table is reusable after a delete-all.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1}, {"x": 2});

delete from T;

insert into T ({"x": 9});

select * from T;
```

<p class="example-label">Result</p>

```json
[
  { "x": 9 }
]
```

</div>

### Error cases

<div class="example">

#### Deleting From

Deleting from an undeclared table is a static error.

<p class="example-label">SQL</p>

```sql
create table T;

delete from Ghost;
```

Expected error: `static`

</div>

## See also

- [Where](@/language/statements/where.md)
- [Clear](@/language/statements/clear.md) — remove all rows without a predicate
- [Writing data](@/examples/writing.md)
