+++
title = "Limit"
description = "Slices the binding-tuple stream by row position (take / skip / half-open slice)."
weight = 10
+++

# Limit

Slices the binding-tuple stream by row position (take / skip / half-open slice).

<div class="example">

### Limit N

Limit N takes the first N rows in scan order.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2}, {x: 3}, {x: 4}, {x: 5});

select * from T limit 2;
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

### Limit 0

Limit 0 emits no rows.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2});

select * from T limit 0;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

### Limit Greater

Limit greater than the row count returns all rows.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2});

select * from T limit 10;
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

### Limit N..

Limit N.. skips the first N rows and keeps the rest.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2}, {x: 3}, {x: 4}, {x: 5});

select * from T limit 2..;
```

<p class="example-label">Result</p>

```json
[
  { "x": 3 },
  { "x": 4 },
  { "x": 5 }
]
```

</div>

<div class="example">

### Skipping Past

Skipping past the end yields no rows.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2}, {x: 3});

select * from T limit 5..;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

### Limit 0..

Limit 0.. skips nothing and returns all rows.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2}, {x: 3});

select * from T limit 0..;
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

### Limit N..M

Limit N..M is half-open over indices [N, M).

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2}, {x: 3}, {x: 4}, {x: 5});

select * from T limit 1..3;
```

<p class="example-label">Result</p>

```json
[
  { "x": 2 },
  { "x": 3 }
]
```

</div>

<div class="example">

### Limit Slice Empty

A slice with M == N emits nothing.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2}, {x: 3}, {x: 4}, {x: 5});

select * from T limit 3..3;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

### Last Row

A slice whose end runs past the data takes through the last row.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2}, {x: 3}, {x: 4});

select * from T limit 1..10;
```

<p class="example-label">Result</p>

```json
[
  { "x": 2 },
  { "x": 3 },
  { "x": 4 }
]
```

</div>

<div class="example">

### Limit Slices

Limit slices the post-where stream, not the raw scan.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2}, {x: 3}, {x: 4});

select * from T where T.x > 1 limit 2;
```

<p class="example-label">Result</p>

```json
[
  { "x": 2 },
  { "x": 3 }
]
```

</div>

<div class="example">

### Limit Applies

Limit applies to a scalar projection.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2}, {x: 3});

select T.x from T limit 2;
```

<p class="example-label">Result</p>

```json
[
  1,
  2
]
```

</div>