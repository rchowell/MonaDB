+++
title = "Order by"
description = "The Order by clause sorts the binding-tuple stream by one or more keys."
weight = 6
+++

The Order by clause sorts the binding-tuple stream by one or more keys. Ascending is the default. Nulls sort last in ascending order and first in descending order.

## Syntax

### Railroad

<div class="rr">
<div class="rr-track"><span class="rr-t">order</span><span class="rr-join" aria-hidden="true"></span><span class="rr-t">by</span><span class="rr-join" aria-hidden="true"></span><span class="rr-n">order-key</span><span class="rr-join" aria-hidden="true"></span><span class="rr-rep"><span class="rr-rep-inner"><span class="rr-t">,</span><span class="rr-join" aria-hidden="true"></span><span class="rr-n">order-key</span></span></span></div>
</div>

### BNF

```ebnf
order-clause ::= "order" "by" order-key ( "," order-key )*

order-key ::= expr [ ( "asc" | "desc" ) ]
```

## Rules

1. Default sort direction is `asc`. *(phase: after **where** / **group**, before **limit** and **select**)*
2. `null` sorts last in `asc` and first in `desc`. *(phase: before **limit**)*
3. Numbers order by numeric value; `1` and `1.0` compare equal. *(phase: before **limit**)*
4. Sort stability is not guaranteed. *(phase: before **limit**)*

## Examples

### Minimal

<div class="example">

#### Order By

Order by sorts ascending by default.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 3}, {"x": 1}, {"x": 2});

select t.x from T as t order by t.x;
```

<p class="example-label">Result</p>

```json
[
  1,
  2,
  3
]
```

</div>

<div class="example">

#### Explicit Asc

Explicit asc matches the default.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 3}, {"x": 1}, {"x": 2});

select t.x from T as t order by t.x asc;
```

<p class="example-label">Result</p>

```json
[
  1,
  2,
  3
]
```

</div>

<div class="example">

#### Order By

Order by reorders whole rows under select *.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 3}, {"x": 1}, {"x": 2});

select * from T as t order by t.x;
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

#### Null Sorts

Null sorts after all values in ascending order.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 2}, {"x": null}, {"x": 1});

select t.x from T as t order by t.x;
```

<p class="example-label">Result</p>

```json
[
  1,
  2,
  null
]
```

</div>

<div class="example">

#### Ints And

Ints and floats interleave by numeric value.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 2}, {"x": 1.5}, {"x": 1});

select t.x from T as t order by t.x;
```

<p class="example-label">Result</p>

```json
[
  1,
  1.5,
  2
]
```

</div>

<div class="example">

#### Strings Sort Lexicographically

Strings sort lexicographically.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"s": "banana"}, {"s": "apple"}, {"s": "cherry"});

select t.s from T as t order by t.s;
```

<p class="example-label">Result</p>

```json
[
  "apple",
  "banana",
  "cherry"
]
```

</div>

<div class="example">

#### Order By

Order by then limit yields the top N.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 3}, {"x": 1}, {"x": 2}, {"x": 5}, {"x": 4});

select t.x from T as t order by t.x desc limit 2;
```

<p class="example-label">Result</p>

```json
[
  5,
  4
]
```

</div>

<div class="example">

#### Order Sorts

Order sorts the post-where stream.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 3}, {"x": 1}, {"x": 2}, {"x": 4});

select t.x from T as t where t.x > 1 order by t.x;
```

<p class="example-label">Result</p>

```json
[
  2,
  3,
  4
]
```

</div>

### Compound

<div class="example">

#### Desc Sorts Descending

Desc sorts descending.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 3}, {"x": 1}, {"x": 2});

select t.x from T as t order by t.x desc;
```

<p class="example-label">Result</p>

```json
[
  3,
  2,
  1
]
```

</div>

<div class="example">

#### Multiple Keys

Multiple keys sort left-to-right with per-key direction.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"a": 1, "b": 2}, {"a": 1, "b": 1}, {"a": 2, "b": 5});

select * from T as t order by t.a, t.b desc;
```

<p class="example-label">Result</p>

```json
[
  { "a": 1, "b": 2 },
  { "a": 1, "b": 1 },
  { "a": 2, "b": 5 }
]
```

</div>

<div class="example">

#### Null Sorts

Null sorts before all values in descending order.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 2}, {"x": null}, {"x": 1});

select t.x from T as t order by t.x desc;
```

<p class="example-label">Result</p>

```json
[
  null,
  2,
  1
]
```

</div>

<div class="example">

#### Mixed Types

Mixed types sort by the same total order as the comparison operators (bool < number < string < null).

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": "s"}, {"x": null}, {"x": 1}, {"x": true});

select t.x from T as t order by t.x;
```

<p class="example-label">Result</p>

```json
[
  true,
  1,
  "s",
  null
]
```

</div>

<div class="example">

#### Order By

Order by sorts the cross product of two sources.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into T ({"x": 2}, {"x": 1});

insert into S ({"y": 9});

select * from T as t, S as s order by t.x;
```

<p class="example-label">Result</p>

```json
[
  { "x": 1, "y": 9 },
  { "x": 2, "y": 9 }
]
```

</div>

## See also

- [Limit](@/language/statements/limit.md) — slice after sorting
- [Select](@/language/statements/select.md)
