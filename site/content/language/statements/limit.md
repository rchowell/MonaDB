+++
title = "Limit"
description = "The Limit clause slices the stream by row position using Python-style half-open range syntax."
weight = 7
+++

The Limit clause slices the stream by row position using Python-style half-open range syntax. `limit n` takes the first *n* rows; `limit n..` skips the first *n*; `limit n..m` selects the half-open index range [*n*, *m*).

## Syntax

### Railroad

<div class="rr">
<div class="rr-track"><span class="rr-t">limit</span><span class="rr-join" aria-hidden="true"></span><span class="rr-or"><span class="rr-branch"><span class="rr-n">integer</span></span><span class="rr-branch"><span class="rr-n">integer</span><span class="rr-join" aria-hidden="true"></span><span class="rr-t">..</span></span><span class="rr-branch"><span class="rr-n">integer</span><span class="rr-join" aria-hidden="true"></span><span class="rr-t">..</span><span class="rr-join" aria-hidden="true"></span><span class="rr-n">integer</span></span><span class="rr-branch"><span class="rr-n">integer</span><span class="rr-join" aria-hidden="true"></span><span class="rr-t">..</span><span class="rr-join" aria-hidden="true"></span><span class="rr-n">integer</span><span class="rr-join" aria-hidden="true"></span><span class="rr-t">..</span><span class="rr-join" aria-hidden="true"></span><span class="rr-n">integer</span></span></span></div>
</div>

### BNF

```ebnf
limit-clause ::= "limit" limit-range

limit-range ::= integer
              | integer ".." [ integer ] [ ".." integer ]
```

## Rules

1. `limit n` is shorthand for `limit 0..n`. *(phase: after **order by**, before **select**)*
2. Range `start..end..step` is half-open: indices `start, start+step, …` strictly less than `end` are emitted. *(phase: before **select**)*
3. Omitted `start` defaults to `0`; omitted `end` is unbounded; omitted `step` is `1`. *(phase: before **select**)*
4. `start`, `end`, and `step` must be non-negative integer literals; `step` must be ≥ 1. *(phase: parse / static)*

## Examples

### Minimal

<div class="example">

#### Limit N

Limit N takes the first N rows in scan order.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1}, {"x": 2}, {"x": 3}, {"x": 4}, {"x": 5});

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

#### Limit 0

Limit 0 emits no rows.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1}, {"x": 2});

select * from T limit 0;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

#### Limit Greater

Limit greater than the row count returns all rows.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1}, {"x": 2});

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

#### Limit N..

Limit N.. skips the first N rows and keeps the rest.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1}, {"x": 2}, {"x": 3}, {"x": 4}, {"x": 5});

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

#### Skipping Past

Skipping past the end yields no rows.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1}, {"x": 2}, {"x": 3});

select * from T limit 5..;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

#### Limit 0..

Limit 0.. skips nothing and returns all rows.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1}, {"x": 2}, {"x": 3});

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

#### Limit N..M

Limit N..M is half-open over indices [N, M).

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1}, {"x": 2}, {"x": 3}, {"x": 4}, {"x": 5});

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

#### Limit Slice Empty

A slice with M == N emits nothing.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1}, {"x": 2}, {"x": 3}, {"x": 4}, {"x": 5});

select * from T limit 3..3;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

#### Last Row

A slice whose end runs past the data takes through the last row.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1}, {"x": 2}, {"x": 3}, {"x": 4});

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

#### Limit Slices

Limit slices the post-where stream, not the raw scan.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1}, {"x": 2}, {"x": 3}, {"x": 4});

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

#### Limit Applies

Limit applies to a scalar projection.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1}, {"x": 2}, {"x": 3});

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

## See also

- [Order by](@/language/statements/order-by.md)
- [Select](@/language/statements/select.md)
