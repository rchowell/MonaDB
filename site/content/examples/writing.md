+++
title = "Writing data"
description = "Insert, delete, clear, and drop examples."
weight = 2
+++

# Writing data

Creating tables, inserting rows, and removing data.

<div class="example-section">

## Insert

</div>

<div class="example">

## One Object

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

## Two Objects

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

## Five Objects

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

## One Multi-value

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

## Three Single-value

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

## Values In

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

## Object Rejected

A stored row must be an object; a scalar value is rejected.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T (1, 2, 3);
```

Expected error: `schema`

</div>

<div class="example">

## Values List

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

<div class="example">

## Inserting Into

Inserting into an undeclared table is a static error.

<p class="example-label">SQL</p>

```sql
create table T;

insert into Ghost ({"x": 1});
```

Expected error: `static`

</div>

<div class="example">

## Empty Values

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

<div class="example">

## An Array

An array value as a row is rejected with a schema error (not a panic).

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ([1, 2, 3]);
```

Expected error: `schema`

</div>

<div class="example-section">

## Delete

</div>

<div class="example">

## Empties Table

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

## Matching Rows

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

## An Explicit

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

## Table Unchanged

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

## Deleting From

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

<div class="example">

## Reuses Table

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

<div class="example">

## Deleting From

Deleting from an undeclared table is a static error.

<p class="example-label">SQL</p>

```sql
create table T;

delete from Ghost;
```

Expected error: `static`

</div>

<div class="example-section">

## Clear table

</div>

<div class="example">

## Table Place

Clear removes every row but leaves the table in place.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1}, {"x": 2}, {"x": 3});

clear table T;

select * from T;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

## New Rows

A cleared table is still resolvable and accepts new rows.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1});

clear table T;

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

<div class="example">

## Clearing An

Clearing an undeclared table is a static error.

<p class="example-label">SQL</p>

```sql
clear table Ghost;
```

Expected error: `static`

</div>

<div class="example-section">

## Drop table

</div>

<div class="example">

## After Drop,

After drop, the table can no longer be selected from.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1});

drop table T;

select * from T;
```

Expected error: `static`

</div>

<div class="example">

## Is Empty

A table re-created after drop is fresh and empty.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1}, {"x": 2});

drop table T;

create table T;

select * from T;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

## Dropping An

Dropping an undeclared table is a static error.

<p class="example-label">SQL</p>

```sql
drop table Ghost;
```

Expected error: `static`

</div>
