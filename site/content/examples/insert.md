+++
title = "Insert"
description = "insert into table — single and multi-value expr lists."
weight = 2
+++

# Insert

insert into table — single and multi-value expr lists.

<div class="example">

### One Object

One object in the values list produces one row.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1});

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

### Two Objects

Two objects in one statement produce two rows.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2});

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

### Five Objects

Five objects in one statement are all persisted.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2}, {x: 3}, {x: 4}, {x: 5});

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

### One Multi-value

One multi-value insert persists all rows.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2}, {x: 3});

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

### Three Single-value

Three single-value inserts produce the same table as one multi-value insert.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1});

insert into T ({x: 2});

insert into T ({x: 3});

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

### Values In

Values in one insert may differ in shape.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1, y: 2}, {x: 3});

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

### Object Rejected

A stored row must be an object; a scalar value is rejected.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T (1, 2, 3);
```

Expected error: `schema`

</div>

<div class="example">

### Values List

Values list may span multiple lines.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T (
    {x: 1},
    {x: 2},
    {x: 3}
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

### Inserting Into

Inserting into an undeclared table is a static error.

<p class="example-label">SQL</p>

```sql
create table T;

insert into Ghost ({x: 1});
```

Expected error: `static`

</div>

<div class="example">

### Empty Values

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

### An Array

An array value as a row is rejected with a schema error (not a panic).

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ([1, 2, 3]);
```

Expected error: `schema`

</div>