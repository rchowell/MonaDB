+++
title = "group"
description = "GROUP BY — a two-pass sort then a streaming pass that resets one shared set of accumulators at each group boundary. One row per group, emitted in group-key order; an empty input yields zero rows (unlike ungrouped aggregation). Covers WHERE pre-filtering, HAVING, LIMIT, composite and expression keys, NULL grouping, and the static errors."
weight = 18
+++

# group

GROUP BY — a two-pass sort then a streaming pass that resets one shared set of accumulators at each group boundary. One row per group, emitted in group-key order; an empty input yields zero rows (unlike ungrouped aggregation). Covers WHERE pre-filtering, HAVING, LIMIT, composite and expression keys, NULL grouping, and the static errors.

<div class="example">

### Count(*) Per

Count(*) per group, one row per distinct key in key order.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({g: "a"}, {g: "b"}, {g: "a"}, {g: "a"}, {g: "b"});

select { g: t.g, n: count(*) } from T as t group by t.g;
```

<p class="example-label">Result</p>

```json
[
  { "g": "a", "n": 3 },
  { "g": "b", "n": 2 }
]
```

</div>

<div class="example">

### The Expr

The `expr as name` list projection form works under grouping.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({g: "a"}, {g: "b"}, {g: "a"});

select t.g as g, count(*) as n from T as t group by t.g;
```

<p class="example-label">Result</p>

```json
[
  { "g": "a", "n": 2 },
  { "g": "b", "n": 1 }
]
```

</div>

<div class="example">

### Integer Group

Integer group keys come out in ascending key order.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({k: 3}, {k: 1}, {k: 2}, {k: 1}, {k: 3});

select { k: t.k, n: count(*) } from T as t group by t.k;
```

<p class="example-label">Result</p>

```json
[
  { "k": 1, "n": 2 },
  { "k": 2, "n": 1 },
  { "k": 3, "n": 2 }
]
```

</div>

<div class="example">

### Every Aggregate

Every aggregate folds independently within each group.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({g: "a", v: 10}, {g: "a", v: 20}, {g: "b", v: 5});

select { g: t.g, s: sum(t.v), a: avg(t.v), mn: min(t.v), mx: max(t.v) } from T as t group by t.g;
```

<p class="example-label">Result</p>

```json
[
  { "g": "a", "s": 30, "a": 15, "mn": 10, "mx": 20 },
  { "g": "b", "s": 5, "a": 5, "mn": 5, "mx": 5 }
]
```

</div>

<div class="example">

### Count(expr) Skips

Count(expr) skips nulls inside each group.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({g: "a", v: 1}, {g: "a", v: null}, {g: "b", v: 2});

select { g: t.g, n: count(t.v) } from T as t group by t.g;
```

<p class="example-label">Result</p>

```json
[
  { "g": "a", "n": 1 },
  { "g": "b", "n": 1 }
]
```

</div>

<div class="example">

### Grouping On

Grouping on two keys orders by the first then the second.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({a: 1, b: 1}, {a: 1, b: 2}, {a: 1, b: 1}, {a: 2, b: 1});

select { a: t.a, b: t.b, n: count(*) } from T as t group by t.a, t.b;
```

<p class="example-label">Result</p>

```json
[
  { "a": 1, "b": 1, "n": 2 },
  { "a": 1, "b": 2, "n": 1 },
  { "a": 2, "b": 1, "n": 1 }
]
```

</div>

<div class="example">

### An Arbitrary

An arbitrary key expression groups, and the same expression projects it.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1, y: 1}, {x: 2, y: 0}, {x: 0, y: 3});

select { s: t.x + t.y, n: count(*) } from T as t group by t.x + t.y;
```

<p class="example-label">Result</p>

```json
[
  { "s": 2, "n": 2 },
  { "s": 3, "n": 1 }
]
```

</div>

<div class="example">

### Grouping With

Grouping with no aggregate behaves like DISTINCT over the key.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({c: "x"}, {c: "y"}, {c: "x"}, {c: "x"});

select t.c from T as t group by t.c;
```

<p class="example-label">Result</p>

```json
[
  "x",
  "y"
]
```

</div>

<div class="example">

### Rows Whose

Rows whose key is null form a single group, sorted last.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({g: "a", v: 1}, {g: "a", v: 2}, {v: 3});

select { g: t.g, n: count(*) } from T as t group by t.g;
```

<p class="example-label">Result</p>

```json
[
  { "g": "a", "n": 2 },
  { "g": null, "n": 1 }
]
```

</div>

<div class="example">

### An Empty

An empty input yields zero groups (contrast ungrouped count -> one row).

<p class="example-label">SQL</p>

```sql
create table T;

select { g: t.g, n: count(*) } from T as t group by t.g;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

### WHERE Filters

WHERE filters rows before they are grouped.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({g: "a", v: 1}, {g: "a", v: 5}, {g: "b", v: 2});

select { g: t.g, n: count(*) } from T as t where t.v > 1 group by t.g;
```

<p class="example-label">Result</p>

```json
[
  { "g": "a", "n": 1 },
  { "g": "b", "n": 1 }
]
```

</div>

<div class="example">

### HAVING Filters

HAVING filters whole groups by an aggregate predicate.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({g: "a"}, {g: "a"}, {g: "b"});

select { g: t.g, n: count(*) } from T as t group by t.g having count(*) > 1;
```

<p class="example-label">Result</p>

```json
[
  { "g": "a", "n": 2 }
]
```

</div>

<div class="example">

### HAVING May

HAVING may reference a group key (read as the group's value).

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({g: "a"}, {g: "b"});

select t.g from T as t group by t.g having t.g = "b";
```

<p class="example-label">Result</p>

```json
[
  "b"
]
```

</div>

<div class="example">

### LIMIT Takes

LIMIT takes the first N groups in key order.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({g: "a"}, {g: "b"}, {g: "c"});

select t.g from T as t group by t.g limit 2;
```

<p class="example-label">Result</p>

```json
[
  "a",
  "b"
]
```

</div>

<div class="example">

### LIMIT N..M

LIMIT N..M skips then takes over the grouped stream.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({g: "a"}, {g: "b"}, {g: "c"});

select t.g from T as t group by t.g limit 1..3;
```

<p class="example-label">Result</p>

```json
[
  "b",
  "c"
]
```

</div>

<div class="example">

### LIMIT Counts

LIMIT counts only groups that survived HAVING.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({g: "a"}, {g: "a"}, {g: "b"}, {g: "c"}, {g: "c"});

select { g: t.g, n: count(*) } from T as t group by t.g having count(*) > 1 limit 1;
```

<p class="example-label">Result</p>

```json
[
  { "g": "a", "n": 2 }
]
```

</div>

<div class="example">

### Projection Rejected

A projected column that is neither grouped nor aggregated is a static error.

<p class="example-label">SQL</p>

```sql
create table T;

select { g: t.g, v: t.v } from T as t group by t.g;
```

Expected error: `static`

</div>

<div class="example">

### Group Rejected

Select * has no defined columns under grouping — static error.

<p class="example-label">SQL</p>

```sql
create table T;

select * from T as t group by t.g;
```

Expected error: `static`

</div>

<div class="example">

### Group Rejected

Select . (the binding tuple) is undefined under grouping — static error.

<p class="example-label">SQL</p>

```sql
create table T;

select . from T as t group by t.g;
```

Expected error: `static`

</div>

<div class="example">

### An Aggregate

An aggregate in a GROUP BY key is a static error.

<p class="example-label">SQL</p>

```sql
create table T;

select count(*) from T as t group by count(*);
```

Expected error: `static`

</div>

<div class="example">

### ORDER BY

ORDER BY over a grouped query is not supported yet — static error.

<p class="example-label">SQL</p>

```sql
create table T;

select t.g from T as t group by t.g order by t.g;
```

Expected error: `static`

</div>

<div class="example">

### Having Rejected

A bare non-grouped column in HAVING is a static error.

<p class="example-label">SQL</p>

```sql
create table T;

select t.g from T as t group by t.g having t.v > 0;
```

Expected error: `static`

</div>