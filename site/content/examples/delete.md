+++
title = "Delete"
description = "delete from table — delete-all and predicate-filtered deletes."
weight = 4
+++

# Delete

delete from table — delete-all and predicate-filtered deletes.

<div class="example">

### Empties Table

Delete with no where removes every row.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2}, {x: 3});

delete from T;

select * from T;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

### Matching Rows

Delete with a predicate removes only matching rows.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2}, {x: 3});

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

### An Explicit

An explicit `as` alias binds the predicate's references.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2}, {x: 3});

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

### Table Unchanged

A predicate that matches nothing leaves the table unchanged.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2}, {x: 3});

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

### Deleting From

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

### Reuses Table

A table is reusable after a delete-all.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2});

delete from T;

insert into T ({x: 9});

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

### Deleting From

Deleting from an undeclared table is a static error.

<p class="example-label">SQL</p>

```sql
create table T;

delete from Ghost;
```

Expected error: `static`

</div>