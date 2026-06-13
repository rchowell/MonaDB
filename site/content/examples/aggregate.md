+++
title = "Aggregate"
description = "Ungrouped aggregation — count/min/max/sum/avg over the whole from/where stream, always collapsing to exactly one result row (even over an empty table). No GROUP BY / HAVING / ORDER BY of aggregates."
weight = 14
+++

# Aggregate

Ungrouped aggregation — count/min/max/sum/avg over the whole from/where stream, always collapsing to exactly one result row (even over an empty table). No GROUP BY / HAVING / ORDER BY of aggregates.

<div class="example">

### Count(*) Counts

Count(*) counts every row.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2}, {x: 3});

select count(*) from T as t;
```

<p class="example-label">Result</p>

```json
[
  3
]
```

</div>

<div class="example">

### Count(expr) Counts

Count(expr) counts only rows where expr is non-null.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2}, {x: null});

select count(t.x) from T as t;
```

<p class="example-label">Result</p>

```json
[
  2
]
```

</div>

<div class="example">

### Count(*) Includes

Count(*) includes rows with a null column, unlike count(expr).

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: null});

select count(*) from T as t;
```

<p class="example-label">Result</p>

```json
[
  2
]
```

</div>

<div class="example">

### Sum Of Integers

Sum of integers.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2}, {x: 3});

select sum(t.x) from T as t;
```

<p class="example-label">Result</p>

```json
[
  6
]
```

</div>

<div class="example">

### Sum Of

Sum of floats (and mixed int/float) is a float.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1.5}, {x: 2}, {x: 0.25});

select sum(t.x) from T as t;
```

<p class="example-label">Result</p>

```json
[
  3.75
]
```

</div>

<div class="example">

### Sum Skips

Sum skips null inputs.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 10}, {x: null}, {x: 5});

select sum(t.x) from T as t;
```

<p class="example-label">Result</p>

```json
[
  15
]
```

</div>

<div class="example">

### An Integer

An integer sum that overflows i64 promotes to a float (sqlite-faithful).

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 9223372036854775807}, {x: 9223372036854775807});

select sum(t.x) from T as t;
```

<p class="example-label">Result</p>

```json
[
  18446744073709552000
]
```

</div>

<div class="example">

### Overflow Errors

A float sum that overflows to non-finite is a runtime error, not a silent null.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1e308}, {x: 1e308});

select sum(t.x) from T as t;
```

Expected error: `runtime`

</div>

<div class="example">

### Min Of Integers

Min of integers.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 3}, {x: 1}, {x: 2});

select min(t.x) from T as t;
```

<p class="example-label">Result</p>

```json
[
  1
]
```

</div>

<div class="example">

### Max Of Integers

Max of integers.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 3}, {x: 1}, {x: 2});

select max(t.x) from T as t;
```

<p class="example-label">Result</p>

```json
[
  3
]
```

</div>

<div class="example">

### Min Of

Min of strings is lexicographic.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({s: "banana"}, {s: "apple"}, {s: "cherry"});

select min(t.s) from T as t;
```

<p class="example-label">Result</p>

```json
[
  "apple"
]
```

</div>

<div class="example">

### Max Of

Max of strings is lexicographic.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({s: "banana"}, {s: "apple"}, {s: "cherry"});

select max(t.s) from T as t;
```

<p class="example-label">Result</p>

```json
[
  "cherry"
]
```

</div>

<div class="example">

### Min/max Skip

Min/max skip null inputs.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 2}, {x: null}, {x: 5});

select min(t.x) from T as t;
```

<p class="example-label">Result</p>

```json
[
  2
]
```

</div>

<div class="example">

### Avg Returns

Avg returns a float mean.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2});

select avg(t.x) from T as t;
```

<p class="example-label">Result</p>

```json
[
  1.5
]
```

</div>

<div class="example">

### Avg Divides

Avg divides the sum by the non-null count only.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 2}, {x: null}, {x: 4});

select avg(t.x) from T as t;
```

<p class="example-label">Result</p>

```json
[
  3
]
```

</div>

<div class="example">

### Count(*) Over

Count(*) over an empty table is 0 (one row).

<p class="example-label">SQL</p>

```sql
create table T;

select count(*) from T as t;
```

<p class="example-label">Result</p>

```json
[
  0
]
```

</div>

<div class="example">

### Sum Over

Sum over an empty table is null (one row).

<p class="example-label">SQL</p>

```sql
create table T;

select sum(t.x) from T as t;
```

<p class="example-label">Result</p>

```json
[
  null
]
```

</div>

<div class="example">

### Min Over

Min over an empty table is null (one row).

<p class="example-label">SQL</p>

```sql
create table T;

select min(t.x) from T as t;
```

<p class="example-label">Result</p>

```json
[
  null
]
```

</div>

<div class="example">

### Max Over

Max over an empty table is null (one row).

<p class="example-label">SQL</p>

```sql
create table T;

select max(t.x) from T as t;
```

<p class="example-label">Result</p>

```json
[
  null
]
```

</div>

<div class="example">

### Avg Over

Avg over an empty table is null (one row).

<p class="example-label">SQL</p>

```sql
create table T;

select avg(t.x) from T as t;
```

<p class="example-label">Result</p>

```json
[
  null
]
```

</div>

<div class="example">

### Aggregates Over

Aggregates over an all-null column — count(expr) 0, others null.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: null}, {x: null});

select count(t.x) from T as t;
```

<p class="example-label">Result</p>

```json
[
  0
]
```

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: null}, {x: null});

select count(t.x) from T as t;

select sum(t.x) from T as t;
```

<p class="example-label">Result</p>

```json
[
  null
]
```

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: null}, {x: null});

select count(t.x) from T as t;

select sum(t.x) from T as t;

select max(t.x) from T as t;
```

<p class="example-label">Result</p>

```json
[
  null
]
```

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: null}, {x: null});

select count(t.x) from T as t;

select sum(t.x) from T as t;

select max(t.x) from T as t;

select avg(t.x) from T as t;
```

<p class="example-label">Result</p>

```json
[
  null
]
```

</div>

<div class="example">

### Aggregation Runs

Aggregation runs over the post-where stream.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2}, {x: 3});

select count(*) from T as t where t.x > 1;
```

<p class="example-label">Result</p>

```json
[
  2
]
```

</div>

<div class="example">

### Several Aggregates

Several aggregates project into one object row.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2}, {x: 3});

select { c: count(*), s: sum(t.x), m: max(t.x) } from T as t;
```

<p class="example-label">Result</p>

```json
[
  { "c": 3, "s": 6, "m": 3 }
]
```

</div>

<div class="example">

### Arithmetic Over

Arithmetic over an aggregate is allowed (the agg is folded, then combined).

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2}, {x: 3});

select sum(t.x) + 1 from T as t;
```

<p class="example-label">Result</p>

```json
[
  7
]
```

</div>

<div class="example">

### Limit Applies

Limit applies to the single aggregate row.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2});

select count(*) from T as t limit 0;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

### An Aggregate

An aggregate in WHERE is a static (bind) error.

<p class="example-label">SQL</p>

```sql
create table T;

select count(*) from T as t where count(*) > 0;
```

Expected error: `static`

</div>

<div class="example">

### An Aggregate

An aggregate nested inside another is a static (bind) error.

<p class="example-label">SQL</p>

```sql
create table T;

select sum(count(t.x)) from T as t;
```

Expected error: `static`

</div>

<div class="example">

### Only Count

Only count accepts the star form; sum(*) is a static error.

<p class="example-label">SQL</p>

```sql
create table T;

select sum(*) from T as t;
```

Expected error: `static`

</div>

<div class="example">

### Column Rejected

A bare column reference alongside an aggregate is unsupported (no GROUP BY).

<p class="example-label">SQL</p>

```sql
create table T;

select { c: count(*), x: t.x } from T as t;
```

Expected error: `static`

</div>

<div class="example">

### Min Over

Min over incomparable types (int vs string) is a runtime error.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({v: 1}, {v: "a"});

select min(t.v) from T as t;
```

Expected error: `runtime`

</div>