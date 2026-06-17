+++
title = "Queries"
description = "Select, from, where, order, limit, group, aggregate, and subquery examples."
weight = 1
+++

# Queries

Reading and transforming data — select shapes output, from iterates sources, and subsequent clauses filter, sort, group, or nest results.

<div class="example-section">

## Select

</div>

<div class="example">

## Envelope Object

Select . emits the binding tuple as an envelope object.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1});

select . from T as t;
```

<p class="example-label">Result</p>

```json
[
  { "t": { "x": 1 } }
]
```

</div>

<div class="example">

## Bindings Flat

Select * spreads bindings flat.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1, "y": 2});

select * from T as t;
```

<p class="example-label">Result</p>

```json
[
  { "x": 1, "y": 2 }
]
```

</div>

<div class="example">

## Per Row

Select <path-expr> emits a scalar per row.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1}, {"x": 2});

select t.x from T as t order by t.x;
```

<p class="example-label">Result</p>

```json
[
  1,
  2
]
```

</div>

<div class="example">

## Per Row

Select <literal-expr> emits the literal once per row.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1}, {"x": 2});

select 7 from T as t order by t.x;
```

<p class="example-label">Result</p>

```json
[
  7,
  7
]
```

</div>

<div class="example">

## Per Row

Select <object-expr> emits the object once per row.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1});

select {"a": t.x} from T as t;
```

<p class="example-label">Result</p>

```json
[
  { "a": 1 }
]
```

</div>

<div class="example">

## Named Field

Select <expr> as <name> emits an object with the named field.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 10});

select t.x as a from T as t;
```

<p class="example-label">Result</p>

```json
[
  { "a": 10 }
]
```

</div>

<div class="example">

## Named Member

A list of items emits an object with each named member.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1, "y": 2});

select t.x as a, t.y as b from T as t;
```

<p class="example-label">Result</p>

```json
[
  { "a": 1, "b": 2 }
]
```

</div>

<div class="example">

## List Items

List items may be arbitrary expressions, not only paths.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1});

select 1 as a, 'hi' as b from T as t;
```

<p class="example-label">Result</p>

```json
[
  { "a": 1, "b": "hi" }
]
```

</div>

<div class="example">

## From <ident>

From <ident> uses the table name as the implicit alias.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1});

select T.x from T;
```

<p class="example-label">Result</p>

```json
[
  1
]
```

</div>

<div class="example">

## From <ident>

From <ident> as <ident> binds the source under an explicit alias.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 7});

select t.x from T as t;
```

<p class="example-label">Result</p>

```json
[
  7
]
```

</div>

<div class="example">

## From <ident>

From <ident> <ident> binds the source under an alias without 'as'.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 9});

select t.x from T t;
```

<p class="example-label">Result</p>

```json
[
  9
]
```

</div>

<div class="example">

## An Array

An array literal builds an array from its element expressions.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 7});

select [1, 2, t.x] as a from T as t;
```

<p class="example-label">Result</p>

```json
[
  { "a": [ 1, 2, 7 ] }
]
```

</div>

<div class="example">

## Array Literals

Array literals may nest.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1});

select [[1, 2], [3]] as a from T as t;
```

<p class="example-label">Result</p>

```json
[
  { "a": [ [ 1, 2 ], [ 3 ] ] }
]
```

</div>

<div class="example">

## Single Row

Select <expr> with no From clause yields the value as a single row.

<p class="example-label">SQL</p>

```sql
create table T;

select 1;
```

<p class="example-label">Result</p>

```json
[
  1
]
```

</div>

<div class="example">

## Nothing Spread

Select * requires a From clause (nothing to spread).

<p class="example-label">SQL</p>

```sql
create table T;

select *;
```

Expected error: `static`

</div>

<div class="example">

## Tuple Envelope

Select . requires a From clause (no binding tuple to envelope).

<p class="example-label">SQL</p>

```sql
create table T;

select .;
```

Expected error: `static`

</div>

<div class="example-section">

## From

</div>

<div class="example">

## Scanning An

Scanning an empty table yields no rows.

<p class="example-label">SQL</p>

```sql
create table T;

select * from T;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

## Insertion Order

A table scan returns all rows in insertion order.

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

## Omitting As

Omitting `as` uses the table name as the alias.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 42});

select T.x from T;
```

<p class="example-label">Result</p>

```json
[
  42
]
```

</div>

<div class="example">

## An Explicit

An explicit `as` alias names the binding.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 10});

select t.x from T as t;
```

<p class="example-label">Result</p>

```json
[
  10
]
```

</div>

<div class="example">

## Row Unwrapped

Select * over one source emits the row unwrapped.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1, "y": 2});

select * from T as t;
```

<p class="example-label">Result</p>

```json
[
  { "x": 1, "y": 2 }
]
```

</div>

<div class="example">

## Its Alias

Select . wraps the binding under its alias.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1});

select . from T as t;
```

<p class="example-label">Result</p>

```json
[
  { "t": { "x": 1 } }
]
```

</div>

<div class="example">

## Two Comma

Two comma sources form a Cartesian product, merged by select *.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into S ({"b": 10}, {"b": 20});

insert into T ({"a": 1}, {"a": 2});

select * from T as t, S as s;
```

<p class="example-label">Result</p>

```json
[
  { "a": 1, "b": 10 },
  { "a": 1, "b": 20 },
  { "a": 2, "b": 10 },
  { "a": 2, "b": 20 }
]
```

</div>

<div class="example">

## From Cross Projection

A projection list may reference both cross-joined bindings.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into S ({"b": 10}, {"b": 20});

insert into T ({"a": 1}, {"a": 2});

select t.a as a, s.b as b from T as t, S as s;
```

<p class="example-label">Result</p>

```json
[
  { "a": 1, "b": 10 },
  { "a": 1, "b": 20 },
  { "a": 2, "b": 10 },
  { "a": 2, "b": 20 }
]
```

</div>

<div class="example">

## Dot Envelope

Select . over two sources keys each binding by its alias.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into S ({"b": 10});

insert into T ({"a": 1});

select . from T as t, S as s;
```

<p class="example-label">Result</p>

```json
[
  { "t": { "a": 1 }, "s": { "b": 10 } }
]
```

</div>

<div class="example">

## Both Bindings

A where predicate filters the product across both bindings.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into S ({"b": 10}, {"b": 20});

insert into T ({"a": 1}, {"a": 2});

select t.a as a, s.b as b from T as t, S as s where t.a = 1;
```

<p class="example-label">Result</p>

```json
[
  { "a": 1, "b": 10 },
  { "a": 1, "b": 20 }
]
```

</div>

<div class="example">

## An Empty

An empty inner source makes the whole product empty.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into T ({"a": 1}, {"a": 2});

select * from T as t, S as s;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

## Referencing An

Referencing an undeclared table is a static error.

<p class="example-label">SQL</p>

```sql
create table T;

select * from Ghost;
```

Expected error: `static`

</div>

<div class="example">

## Referencing An

Referencing an alias not in scope is a static error.

<p class="example-label">SQL</p>

```sql
create table T;

select x.foo from T;
```

Expected error: `static`

</div>

<div class="example">

## Earlier Binding

A later source may unnest a collection path on an earlier binding.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"items": [1, 2, 3]});

select t.items as items, item as item from T as t, t.items as item;
```

<p class="example-label">Result</p>

```json
[
  { "items": [ 1, 2, 3 ], "item": 1 },
  { "items": [ 1, 2, 3 ], "item": 2 },
  { "items": [ 1, 2, 3 ], "item": 3 }
]
```

</div>

<div class="example">

## Star Scalar

Select * keeps a non-object (scalar) lateral binding under its alias.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"items": [1, 2, 3]});

select * from T as t, t.items as item;
```

<p class="example-label">Result</p>

```json
[
  { "items": [ 1, 2, 3 ], "item": 1 },
  { "items": [ 1, 2, 3 ], "item": 2 },
  { "items": [ 1, 2, 3 ], "item": 3 }
]
```

</div>

<div class="example">

## The Unnested

The unnested element binds under its alias in the envelope.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"items": [1, 2]});

select . from T as t, t.items as item;
```

<p class="example-label">Result</p>

```json
[
  { "t": { "items": [ 1, 2 ] }, "item": 1 },
  { "t": { "items": [ 1, 2 ] }, "item": 2 }
]
```

</div>

<div class="example">

## Unnest Flattens

Unnest flattens across every outer row in order.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"k": 1, "items": [10, 11]}, {"k": 2, "items": [20]});

select t.k as k, item as v from T as t, t.items as item;
```

<p class="example-label">Result</p>

```json
[
  { "k": 1, "v": 10 },
  { "k": 1, "v": 11 },
  { "k": 2, "v": 20 }
]
```

</div>

<div class="example">

## An Empty

An empty collection contributes no rows for that outer binding.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"items": []});

select item as v from T as t, t.items as item;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

## Missing Path

A missing path is treated as empty (inner-join-like).

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1});

select item as v from T as t, t.items as item;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

## Non Array

A non-array source value contributes no rows.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"items": 5});

select item as v from T as t, t.items as item;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

## An Array

An array literal may serve as a From source, scanned element-wise.

<p class="example-label">SQL</p>

```sql
create table T;

select x from [1, 2, 3] as x;
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

## Self Reference

A lateral source may not reference its own alias.

<p class="example-label">SQL</p>

```sql
create table T;

select * from T as t, item.x as item;
```

Expected error: `static`

</div>

<div class="example">

## Requires Alias

A lateral collection source requires an alias.

<p class="example-label">SQL</p>

```sql
create table T;

select * from T as t, t.items;
```

Expected error: `static`

</div>

<div class="example">

## An Empty

An empty array literal source yields no rows.

<p class="example-label">SQL</p>

```sql
create table T;

select x from [] as x;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

## Its Alias

Select . wraps each scanned element under its alias.

<p class="example-label">SQL</p>

```sql
create table T;

select . from [1, 2] as x;
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

## Into Them

A value source iterates object elements and may path into them.

<p class="example-label">SQL</p>

```sql
create table T;

select x.a as a from [{"a": 1}, {"a": 2}] as x;
```

<p class="example-label">Result</p>

```json
[
  { "a": 1 },
  { "a": 2 }
]
```

</div>

<div class="example">

## Literal Csv

A file-path string literal desugars to read_csv.

<p class="example-label">SQL</p>

```sql
create table T;

select * from 'tests/fixtures/people.csv' order by people.name;
```

<p class="example-label">Result</p>

```json
[
  { "name": "alice", "age": 30 },
  { "name": "bob", "age": 25 }
]
```

</div>

<div class="example">

## Read_csv In

Read_csv in FROM with an explicit alias.

<p class="example-label">SQL</p>

```sql
create table T;

select r.name as name from read_csv('tests/fixtures/people.csv') as r order by r.name;
```

<p class="example-label">Result</p>

```json
[
  { "name": "alice" },
  { "name": "bob" }
]
```

</div>

<div class="example">

## Non Array

A non-array value source contributes no rows.

<p class="example-label">SQL</p>

```sql
create table T;

select x from 5 as x;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

## Table Row

A value source re-iterates for every outer table row.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"a": 1}, {"a": 2});

select . from T as t, [10, 20] as n;
```

<p class="example-label">Result</p>

```json
[
  { "t": { "a": 1 }, "n": 10 },
  { "t": { "a": 1 }, "n": 20 },
  { "t": { "a": 2 }, "n": 10 },
  { "t": { "a": 2 }, "n": 20 }
]
```

</div>

<div class="example">

## Unnest An

Unnest an array of objects and path into each element.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"s": [{"x": 1}, {"x": 2}]}, {"s": [{"x": 3}]});

select s.x as x from T as t, t.s as s;
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

<div class="example-section">

## Where

</div>

<div class="example">

## Constant True

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

## Constant False

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

## Null Predicate

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

## Numeric Greater-than

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

## Numeric Equality

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

## Numeric Inequality

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

## String Equality

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

## Boolean Equality

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

## Predicate May

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

## Null Member

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

## Null Member

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

## Absent Field

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

## Absent Field

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

<div class="example">

## Only Rows

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

## Scalar Projection

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

<div class="example-section">

## Order by

</div>

<div class="example">

## Order By

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

## Explicit Asc

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

## Desc Sorts Descending

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

## Order By

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

## Multiple Keys

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

## Null Sorts

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

## Null Sorts

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

## Ints And

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

## Strings Sort Lexicographically

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

## Order By

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

## Order Sorts

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

<div class="example">

## Order By

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

<div class="example-section">

## Limit

</div>

<div class="example">

## Limit N

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

## Limit 0

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

## Limit Greater

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

## Limit N..

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

## Skipping Past

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

## Limit 0..

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

## Limit N..M

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

## Limit Slice Empty

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

## Last Row

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

## Limit Slices

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

## Limit Applies

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

<div class="example-section">

## Aggregate

</div>

<div class="example">

## Count(*) Counts

Count(*) counts every row.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1}, {"x": 2}, {"x": 3});

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

## Count(expr) Counts

Count(expr) counts only rows where expr is non-null.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1}, {"x": 2}, {"x": null});

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

## Count(*) Includes

Count(*) includes rows with a null column, unlike count(expr).

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1}, {"x": null});

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

## Sum Of Integers

Sum of integers.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1}, {"x": 2}, {"x": 3});

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

## Sum Of

Sum of floats (and mixed int/float) is a float.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1.5}, {"x": 2}, {"x": 0.25});

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

## Sum Skips

Sum skips null inputs.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 10}, {"x": null}, {"x": 5});

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

## An Integer

An integer sum that overflows i64 promotes to a float (sqlite-faithful).

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 9223372036854775807}, {"x": 9223372036854775807});

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

## Overflow Errors

A float sum that overflows to non-finite is a runtime error, not a silent null.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1e308}, {"x": 1e308});

select sum(t.x) from T as t;
```

Expected error: `runtime`

</div>

<div class="example">

## Min Of Integers

Min of integers.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 3}, {"x": 1}, {"x": 2});

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

## Max Of Integers

Max of integers.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 3}, {"x": 1}, {"x": 2});

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

## Min Of

Min of strings is lexicographic.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"s": "banana"}, {"s": "apple"}, {"s": "cherry"});

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

## Max Of

Max of strings is lexicographic.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"s": "banana"}, {"s": "apple"}, {"s": "cherry"});

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

## Min/max Skip

Min/max skip null inputs.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 2}, {"x": null}, {"x": 5});

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

## Avg Returns

Avg returns a float mean.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1}, {"x": 2});

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

## Avg Divides

Avg divides the sum by the non-null count only.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 2}, {"x": null}, {"x": 4});

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

## Count(*) Over

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

## Sum Over

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

## Min Over

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

## Max Over

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

## Avg Over

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

## Aggregates Over

Aggregates over an all-null column — count(expr) 0, others null.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": null}, {"x": null});

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

insert into T ({"x": null}, {"x": null});

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

insert into T ({"x": null}, {"x": null});

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

insert into T ({"x": null}, {"x": null});

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

## Aggregation Runs

Aggregation runs over the post-where stream.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1}, {"x": 2}, {"x": 3});

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

## Several Aggregates

Several aggregates project into one object row.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1}, {"x": 2}, {"x": 3});

select { "c": count(*), "s": sum(t.x), "m": max(t.x) } from T as t;
```

<p class="example-label">Result</p>

```json
[
  { "c": 3, "s": 6, "m": 3 }
]
```

</div>

<div class="example">

## Arithmetic Over

Arithmetic over an aggregate is allowed (the agg is folded, then combined).

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1}, {"x": 2}, {"x": 3});

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

## Limit Applies

Limit applies to the single aggregate row.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1}, {"x": 2});

select count(*) from T as t limit 0;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

## An Aggregate

An aggregate in WHERE is a static (bind) error.

<p class="example-label">SQL</p>

```sql
create table T;

select count(*) from T as t where count(*) > 0;
```

Expected error: `static`

</div>

<div class="example">

## An Aggregate

An aggregate nested inside another is a static (bind) error.

<p class="example-label">SQL</p>

```sql
create table T;

select sum(count(t.x)) from T as t;
```

Expected error: `static`

</div>

<div class="example">

## Only Count

Only count accepts the star form; sum(*) is a static error.

<p class="example-label">SQL</p>

```sql
create table T;

select sum(*) from T as t;
```

Expected error: `static`

</div>

<div class="example">

## Column Rejected

A bare column reference alongside an aggregate is unsupported (no GROUP BY).

<p class="example-label">SQL</p>

```sql
create table T;

select { "c": count(*), "x": t.x } from T as t;
```

Expected error: `static`

</div>

<div class="example">

## Min Over

Min over incomparable types (int vs string) is a runtime error.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"v": 1}, {"v": "a"});

select min(t.v) from T as t;
```

Expected error: `runtime`

</div>

<div class="example-section">

## group

</div>

<div class="example">

## Count(*) Per

Count(*) per group, one row per distinct key in key order.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"g": "a"}, {"g": "b"}, {"g": "a"}, {"g": "a"}, {"g": "b"});

select { "g": t.g, "n": count(*) } from T as t group by t.g;
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

## The Expr

The `expr as name` list projection form works under grouping.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"g": "a"}, {"g": "b"}, {"g": "a"});

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

## Integer Group

Integer group keys come out in ascending key order.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"k": 3}, {"k": 1}, {"k": 2}, {"k": 1}, {"k": 3});

select { "k": t.k, "n": count(*) } from T as t group by t.k;
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

## Every Aggregate

Every aggregate folds independently within each group.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"g": "a", "v": 10}, {"g": "a", "v": 20}, {"g": "b", "v": 5});

select { "g": t.g, "s": sum(t.v), "a": avg(t.v), "mn": min(t.v), "mx": max(t.v) } from T as t group by t.g;
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

## Count(expr) Skips

Count(expr) skips nulls inside each group.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"g": "a", "v": 1}, {"g": "a", "v": null}, {"g": "b", "v": 2});

select { "g": t.g, "n": count(t.v) } from T as t group by t.g;
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

## Grouping On

Grouping on two keys orders by the first then the second.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"a": 1, "b": 1}, {"a": 1, "b": 2}, {"a": 1, "b": 1}, {"a": 2, "b": 1});

select { "a": t.a, "b": t.b, "n": count(*) } from T as t group by t.a, t.b;
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

## An Arbitrary

An arbitrary key expression groups, and the same expression projects it.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1, "y": 1}, {"x": 2, "y": 0}, {"x": 0, "y": 3});

select { "s": t.x + t.y, "n": count(*) } from T as t group by t.x + t.y;
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

## Grouping With

Grouping with no aggregate behaves like DISTINCT over the key.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"c": "x"}, {"c": "y"}, {"c": "x"}, {"c": "x"});

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

## Rows Whose

Rows whose key is null form a single group, sorted last.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"g": "a", "v": 1}, {"g": "a", "v": 2}, {"v": 3});

select { "g": t.g, "n": count(*) } from T as t group by t.g;
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

## An Empty

An empty input yields zero groups (contrast ungrouped count -> one row).

<p class="example-label">SQL</p>

```sql
create table T;

select { "g": t.g, "n": count(*) } from T as t group by t.g;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

## WHERE Filters

WHERE filters rows before they are grouped.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"g": "a", "v": 1}, {"g": "a", "v": 5}, {"g": "b", "v": 2});

select { "g": t.g, "n": count(*) } from T as t where t.v > 1 group by t.g;
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

## HAVING Filters

HAVING filters whole groups by an aggregate predicate.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"g": "a"}, {"g": "a"}, {"g": "b"});

select { "g": t.g, "n": count(*) } from T as t group by t.g having count(*) > 1;
```

<p class="example-label">Result</p>

```json
[
  { "g": "a", "n": 2 }
]
```

</div>

<div class="example">

## HAVING May

HAVING may reference a group key (read as the group's value).

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"g": "a"}, {"g": "b"});

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

## LIMIT Takes

LIMIT takes the first N groups in key order.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"g": "a"}, {"g": "b"}, {"g": "c"});

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

## LIMIT N..M

LIMIT N..M skips then takes over the grouped stream.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"g": "a"}, {"g": "b"}, {"g": "c"});

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

## LIMIT Counts

LIMIT counts only groups that survived HAVING.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"g": "a"}, {"g": "a"}, {"g": "b"}, {"g": "c"}, {"g": "c"});

select { "g": t.g, "n": count(*) } from T as t group by t.g having count(*) > 1 limit 1;
```

<p class="example-label">Result</p>

```json
[
  { "g": "a", "n": 2 }
]
```

</div>

<div class="example">

## Projection Rejected

A projected column that is neither grouped nor aggregated is a static error.

<p class="example-label">SQL</p>

```sql
create table T;

select { "g": t.g, "v": t.v } from T as t group by t.g;
```

Expected error: `static`

</div>

<div class="example">

## Group Rejected

Select * has no defined columns under grouping — static error.

<p class="example-label">SQL</p>

```sql
create table T;

select * from T as t group by t.g;
```

Expected error: `static`

</div>

<div class="example">

## Group Rejected

Select . (the binding tuple) is undefined under grouping — static error.

<p class="example-label">SQL</p>

```sql
create table T;

select . from T as t group by t.g;
```

Expected error: `static`

</div>

<div class="example">

## An Aggregate

An aggregate in a GROUP BY key is a static error.

<p class="example-label">SQL</p>

```sql
create table T;

select count(*) from T as t group by count(*);
```

Expected error: `static`

</div>

<div class="example">

## ORDER BY

ORDER BY over a grouped query is not supported yet — static error.

<p class="example-label">SQL</p>

```sql
create table T;

select t.g from T as t group by t.g order by t.g;
```

Expected error: `static`

</div>

<div class="example">

## Having Rejected

A bare non-grouped column in HAVING is a static error.

<p class="example-label">SQL</p>

```sql
create table T;

select t.g from T as t group by t.g having t.v > 0;
```

Expected error: `static`

</div>

<div class="example-section">

## subquery

</div>

<div class="example">

## Scalar Uncorrelated Count

A scalar subquery in projection with no outer from.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into T ({"x": 1}, {"x": 2}, {"x": 3});

select (select count(*) from T as t);
```

<p class="example-label">Result</p>

```json
[
  3
]
```

</div>

<div class="example">

## Scalar Aliased Member

A scalar subquery aliased into a projection member.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into T ({"x": 1}, {"x": 2}, {"x": 3});

select (select count(*) from T as t) as n;
```

<p class="example-label">Result</p>

```json
[
  { "n": 3 }
]
```

</div>

<div class="example">

## Is null

A scalar subquery over an empty bag coerces to null.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

select (select t.x from T as t);
```

<p class="example-label">Result</p>

```json
[
  null
]
```

</div>

<div class="example">

## An Aggregate

An aggregate scalar subquery over an empty table is null.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

select (select max(t.x) from T as t);
```

<p class="example-label">Result</p>

```json
[
  null
]
```

</div>

<div class="example">

## Many Rows

A scalar subquery returning more than one row is a runtime error.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into T ({"x": 1}, {"x": 2});

select (select t.x from T as t);
```

Expected error: `runtime`

</div>

<div class="example">

## Scalar In Where

A scalar subquery as a where comparand.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into T ({"x": 1}, {"x": 2}, {"x": 3});

insert into S ({"v": 2}, {"v": 3});

select t.x from T as t where t.x = (select max(s.v) from S as s);
```

<p class="example-label">Result</p>

```json
[
  3
]
```

</div>

<div class="example">

## Scalar Correlated

A correlated scalar subquery re-runs per outer row.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into T ({"x": 1}, {"x": 2});

insert into S ({"g": 1}, {"g": 1}, {"g": 2});

select t.x as x, (select count(*) from S as s where s.g = t.x) as n from T as t;
```

<p class="example-label">Result</p>

```json
[
  { "x": 1, "n": 2 },
  { "x": 2, "n": 1 }
]
```

</div>

<div class="example">

## Scalar Single Row

A non-aggregate scalar subquery over exactly one row is that value.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into T ({"x": 7});

select (select t.x from T as t);
```

<p class="example-label">Result</p>

```json
[
  7
]
```

</div>

<div class="example">

## Scalar In Expr

A scalar subquery composes as an operand of an arithmetic expression.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into T ({"x": 10}, {"x": 20});

insert into S ({"v": 5});

select t.x + (select max(s.v) from S as s) as y from T as t;
```

<p class="example-label">Result</p>

```json
[
  { "y": 15 },
  { "y": 25 }
]
```

</div>

<div class="example">

## Outer Query

A derived table filters then projects in the outer query.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into T ({"x": 1}, {"x": 2}, {"x": 3});

select d.x from (select t.x as x from T as t where t.x > 1) as d;
```

<p class="example-label">Result</p>

```json
[
  2,
  3
]
```

</div>

<div class="example">

## Derived Star

Select * over a derived table.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into T ({"x": 1, "y": 2});

select * from (select t.x as x, t.y as y from T as t) as d;
```

<p class="example-label">Result</p>

```json
[
  { "x": 1, "y": 2 }
]
```

</div>

<div class="example">

## Derived Correlated Lateral

A lateral derived table references an earlier from binding.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into T ({"x": 1}, {"x": 2});

insert into S ({"g": 1, "v": 10}, {"g": 1, "v": 11}, {"g": 2, "v": 20});

select t.x as x, d.n as n from T as t, (select s.v as n from S as s where s.g = t.x) as d;
```

<p class="example-label">Result</p>

```json
[
  { "x": 1, "n": 10 },
  { "x": 1, "n": 11 },
  { "x": 2, "n": 20 }
]
```

</div>

<div class="example">

## Derived Group By

A derived table containing group by (sink through cc_group).

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into T ({"g": "a"}, {"g": "b"}, {"g": "a"});

select d.g as g, d.n as n from (select t.g as g, count(*) as n from T as t group by t.g) as d;
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

## Derived Order Limit

A derived table containing order by + limit (sink through cc_order).

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into T ({"x": 1}, {"x": 2}, {"x": 3});

select d.x from (select t.x as x from T as t order by t.x desc limit 2) as d;
```

<p class="example-label">Result</p>

```json
[
  3,
  2
]
```

</div>

<div class="example">

## Derived Requires Alias

A derived-table source requires an alias.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

select x from (select t.x as x from T as t);
```

Expected error: `static`

</div>

<div class="example">

## An Uncorrelated

An uncorrelated derived table cross-joined with a base table.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into T ({"x": 1}, {"x": 2});

insert into S ({"v": 2}, {"v": 3});

select t.x as x, d.v as v from T as t, (select s.v as v from S as s where s.v = 2) as d;
```

<p class="example-label">Result</p>

```json
[
  { "x": 1, "v": 2 },
  { "x": 2, "v": 2 }
]
```

</div>

<div class="example">

## Nested Derived

A derived table whose source is itself a derived table.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into T ({"x": 1}, {"x": 2}, {"x": 3});

select dd.x from (select d.x as x from (select t.x as x from T as t where t.x > 1) as d) as dd;
```

<p class="example-label">Result</p>

```json
[
  2,
  3
]
```

</div>

<div class="example">

## Membership Against

Membership against a subquery bag.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into T ({"x": 1}, {"x": 2}, {"x": 3});

insert into S ({"v": 2}, {"v": 3});

select t.x from T as t where t.x in (select s.v from S as s);
```

<p class="example-label">Result</p>

```json
[
  2,
  3
]
```

</div>

<div class="example">

## Non-membership Against

Non-membership against a subquery bag.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into T ({"x": 1}, {"x": 2}, {"x": 3});

insert into S ({"v": 2}, {"v": 3});

select t.x from T as t where t.x not in (select s.v from S as s);
```

<p class="example-label">Result</p>

```json
[
  1
]
```

</div>

<div class="example">

## In Over

In over an empty bag matches nothing.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into T ({"x": 1}, {"x": 2});

select t.x from T as t where t.x in (select s.v from S as s);
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

## Not In

Not in over an empty bag matches everything.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into T ({"x": 1}, {"x": 2});

select t.x from T as t where t.x not in (select s.v from S as s);
```

<p class="example-label">Result</p>

```json
[
  1,
  2
]
```

</div>

<div class="example">

## Not In

Not in with a null element is unknown (3VL), excluding the row.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into T ({"x": 1}, {"x": 2});

insert into S ({"v": 2}, {"v": null});

select t.x from T as t where t.x not in (select s.v from S as s);
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

## In null Operand

A null left operand of in is unknown (3VL), excluding the row.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into T ({"x": null}, {"x": 2});

insert into S ({"v": 2}, {"v": 3});

select t.x from T as t where t.x in (select s.v from S as s);
```

<p class="example-label">Result</p>

```json
[
  2
]
```

</div>

<div class="example">

## Exists Over

Exists over a correlated subquery.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into T ({"x": 1}, {"x": 2});

insert into S ({"g": 1});

select t.x from T as t where exists (select s.g from S as s where s.g = t.x);
```

<p class="example-label">Result</p>

```json
[
  1
]
```

</div>

<div class="example">

## Not Exists

Not exists over a correlated subquery.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into T ({"x": 1}, {"x": 2});

insert into S ({"g": 1});

select t.x from T as t where not exists (select s.g from S as s where s.g = t.x);
```

<p class="example-label">Result</p>

```json
[
  2
]
```

</div>

<div class="example">

## Exists Over

Exists over a non-empty uncorrelated subquery is true for all rows.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into T ({"x": 1}, {"x": 2});

insert into S ({"g": 9});

select t.x from T as t where exists (select s.g from S as s);
```

<p class="example-label">Result</p>

```json
[
  1,
  2
]
```

</div>

<div class="example">

## Exists Over

Exists over an empty subquery is false.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into T ({"x": 1}, {"x": 2});

select t.x from T as t where exists (select s.g from S as s);
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

## X >

X > any (bag) is true when x exceeds the minimum.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into T ({"x": 1}, {"x": 5}, {"x": 10});

insert into S ({"v": 3}, {"v": 8});

select t.x from T as t where t.x > any (select s.v from S as s);
```

<p class="example-label">Result</p>

```json
[
  5,
  10
]
```

</div>

<div class="example">

## X >

X > all (bag) is true when x exceeds the maximum.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into T ({"x": 1}, {"x": 5}, {"x": 10});

insert into S ({"v": 3}, {"v": 8});

select t.x from T as t where t.x > all (select s.v from S as s);
```

<p class="example-label">Result</p>

```json
[
  10
]
```

</div>

<div class="example">

## X =

X = any (bag) is equivalent to in.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into T ({"x": 1}, {"x": 2}, {"x": 3});

insert into S ({"v": 2}, {"v": 3});

select t.x from T as t where t.x = any (select s.v from S as s);
```

<p class="example-label">Result</p>

```json
[
  2,
  3
]
```

</div>

<div class="example">

## Any Over

Any over an empty bag is false.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into T ({"x": 1}, {"x": 2});

select t.x from T as t where t.x > any (select s.v from S as s);
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

## All Over

All over an empty bag is true.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into T ({"x": 1}, {"x": 2});

select t.x from T as t where t.x > all (select s.v from S as s);
```

<p class="example-label">Result</p>

```json
[
  1,
  2
]
```

</div>

<div class="example">

## All With

All with a null element is unknown (3VL), excluding the row.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into T ({"x": 1}, {"x": 10});

insert into S ({"v": 3}, {"v": null});

select t.x from T as t where t.x > all (select s.v from S as s);
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

## X <

X < all (bag) is true when x is below the minimum.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into T ({"x": 1}, {"x": 5}, {"x": 10});

insert into S ({"v": 3}, {"v": 8});

select t.x from T as t where t.x < all (select s.v from S as s);
```

<p class="example-label">Result</p>

```json
[
  1
]
```

</div>

<div class="example">

## X !=

X != all (bag) is equivalent to not in.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into T ({"x": 1}, {"x": 2}, {"x": 3});

insert into S ({"v": 2}, {"v": 3});

select t.x from T as t where t.x != all (select s.v from S as s);
```

<p class="example-label">Result</p>

```json
[
  1
]
```

</div>

<div class="example">

## Match Wins

A real match makes = any true even when the bag holds a null.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into T ({"x": 1}, {"x": 2}, {"x": 3});

insert into S ({"v": 2}, {"v": null});

select t.x from T as t where t.x = any (select s.v from S as s);
```

<p class="example-label">Result</p>

```json
[
  2
]
```

</div>

<div class="example">

## Order Limit

A subquery with order by + limit on the right of in.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into T ({"x": 1}, {"x": 2}, {"x": 3});

insert into S ({"v": 3}, {"v": 2}, {"v": 1});

select t.x from T as t where t.x in (select s.v from S as s order by s.v limit 1);
```

<p class="example-label">Result</p>

```json
[
  1
]
```

</div>

<div class="example">

## Nested Subquery

A subquery whose predicate contains another subquery.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

create table U;

insert into T ({"x": 1});

insert into S ({"g": 5});

insert into U ({"w": 5});

select t.x from T as t where exists (select s.g from S as s where s.g in (select u.w from U as u));
```

<p class="example-label">Result</p>

```json
[
  1
]
```

</div>

<div class="example">

## Param In Subquery

A query parameter resolves inside a subquery.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into T ({"x": 2});

insert into S ({"v": 1}, {"v": 2});

select t.x from T as t where t.x in (select s.v from S as s where s.v > ?);
```

<p class="example-label">Result</p>

```json
[
  2
]
```

</div>

<div class="example">

## An Outer

An outer aggregate and a scalar subquery with its own aggregate stay at separate query levels — the inner count is not folded into the outer.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into T ({"x": 1}, {"x": 2});

insert into S ({"v": 1}, {"v": 2}, {"v": 3});

select count(*) as c, (select count(*) from S as s) as d from T as t;
```

<p class="example-label">Result</p>

```json
[
  { "c": 2, "d": 3 }
]
```

</div>

<div class="example">

## Grouped Projection

A scalar subquery as a member of a grouped projection (known gap).

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into T ({"g": "a"}, {"g": "a"}, {"g": "b"});

insert into S ({"v": 1}, {"v": 2});

select t.g as g, count(*) as n, (select count(*) from S as s) as d from T as t group by t.g;
```

<p class="example-label">Result</p>

```json
[
  { "g": "a", "n": 2, "d": 2 },
  { "g": "b", "n": 1, "d": 2 }
]
```

</div>

<div class="example">

## Subquery In Having

A scalar subquery as a having comparand (known gap).

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into T ({"g": "a"}, {"g": "a"}, {"g": "b"});

insert into S ({"v": 1}, {"v": 2});

select t.g as g, count(*) as n from T as t group by t.g having count(*) >= (select count(*) from S as s);
```

<p class="example-label">Result</p>

```json
[
  { "g": "a", "n": 2 }
]
```

</div>

<div class="example">

## An Inner

An inner subquery correlates to the binding of a middle subquery.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

create table U;

insert into T ({"x": 1}, {"x": 2});

insert into S ({"g": 1});

insert into U ({"w": 1});

select t.x from T as t where exists (select s.g from S as s where s.g = t.x and exists (select u.w from U as u where u.w = s.g));
```

<p class="example-label">Result</p>

```json
[
  1
]
```

</div>
