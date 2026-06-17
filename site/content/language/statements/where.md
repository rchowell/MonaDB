+++
title = "Where"
description = "The Where clause filters the binding-tuple stream by a boolean predicate."
weight = 5
+++

The Where clause filters the binding-tuple stream by a boolean predicate. Only tuples for which the predicate evaluates to exactly `true` pass through; `false` and `null` both drop the row.

## Syntax

### Railroad

<div class="rr">
<div class="rr-track"><span class="rr-t">where</span><span class="rr-join" aria-hidden="true"></span><span class="rr-n">expr</span></div>
</div>

### BNF

```ebnf
where-clause ::= "where" expr
```

## Rules

1. The predicate must evaluate to `bool`. A `null` result is treated as not-true and drops the tuple. *(phase: after **from** / **with**, before **group**, **order by**, **limit**, and **select**)*
2. Comparison with `null` follows SQL three-valued logic: only `null = null` is `true`; `null` compared to a non-null value is `false`; ordering `null` against non-null yields `null` (drops the row in where). *(phase: after **from**)*
3. Aggregate functions may not appear in the where predicate. *(phase: after **from**)*

## Examples

### Minimal

<div class="example">

#### Constant True

Constant true keeps all rows.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1}, {"x": 2});

select * from T where true;
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

#### Constant False

Constant false drops all rows.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1});

select * from T where false;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

#### Null Predicate

Null predicate is not-true and drops all rows.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1});

select * from T where null;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

#### Numeric Greater-than

Numeric greater-than filters by oid insertion order.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1}, {"x": 2}, {"x": 3});

select * from T where T.x > 1;
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

#### Numeric Equality

Numeric equality matches a single row.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1}, {"x": 2}, {"x": 3});

select * from T where T.x = 2;
```

<p class="example-label">Result</p>

```json
[
  { "x": 2 }
]
```

</div>

<div class="example">

#### Numeric Inequality

Numeric inequality excludes matching value.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1}, {"x": 2}, {"x": 3});

select * from T where T.x != 1;
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

#### String Equality

String equality in where.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into S ({"name": 'alice'}, {"name": 'bob'});

select * from S where S.name = 'bob';
```

<p class="example-label">Result</p>

```json
[
  { "name": "bob" }
]
```

</div>

<div class="example">

#### Boolean Equality

Boolean equality in where.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"flag": true}, {"flag": false});

select * from T where T.flag = true;
```

<p class="example-label">Result</p>

```json
[
  { "flag": true }
]
```

</div>

<div class="example">

#### Predicate May

Predicate may use an explicit from alias.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 0}, {"x": 1});

select * from T as t where t.x > 0;
```

<p class="example-label">Result</p>

```json
[
  { "x": 1 }
]
```

</div>

<div class="example">

#### Null Member

Null member compares equal to null.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": null});

select * from T where T.x = null;
```

<p class="example-label">Result</p>

```json
[
  { "x": null }
]
```

</div>

<div class="example">

#### Null Member

Null member fails inequality against non-null.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": null});

select * from T where T.x != 1;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

#### Absent Field

Absent field reads as null and matches null.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select * from T where T.x = null;
```

<p class="example-label">Result</p>

```json
[
  {}
]
```

</div>

<div class="example">

#### Only Rows

Only rows with present matching field pass equality.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1}, {});

select * from T where T.x = 1;
```

<p class="example-label">Result</p>

```json
[
  { "x": 1 }
]
```

</div>

<div class="example">

#### Scalar Projection

Scalar projection with where (moved from from-clause suite).

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1}, {"x": 2}, {"x": 3});

select T.x from T where T.x > 1;
```

<p class="example-label">Result</p>

```json
[
  2,
  3
]
```

</div>

### Compound

<div class="example">

#### Absent Field

Absent field reads as null and fails ordering comparison.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select * from T where T.x > 0;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

## See also

- [From](@/language/statements/from.md)
- [Expressions](@/language/expressions.md) — predicates and operators
- [Select](@/language/statements/select.md)
