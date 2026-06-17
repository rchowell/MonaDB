+++
title = "From"
description = "The From clause introduces bindings by iterating data sources."
weight = 2
+++

The From clause introduces bindings by iterating data sources. Each source binds rows under an alias for use in later clauses. Multiple comma-separated sources form a lateral cross product: each subsequent source is evaluated in the context of bindings from preceding sources.

## Syntax

### Railroad

<div class="rr">
<div class="rr-track"><span class="rr-t">from</span><span class="rr-join" aria-hidden="true"></span><span class="rr-n">source</span><span class="rr-join" aria-hidden="true"></span><span class="rr-rep"><span class="rr-rep-inner"><span class="rr-t">,</span><span class="rr-join" aria-hidden="true"></span><span class="rr-n">source</span></span></span></div>
</div>

### BNF

```ebnf
from-clause ::= "from" source ( "," source )*

source ::= table-ref
         | path-expr
         | "(" select-stmt ")"
         | unpivot-source

table-ref ::= identifier [ "as" identifier ]

unpivot-source ::= "unpivot" expr "as" identifier [ "at" identifier ]
```

## Rules

1. Each source must have an alias; when `as` is omitted, the alias defaults to the table name. *(phase: evaluate first in the query pipeline)*
2. Sources are evaluated left to right; the right-hand source may reference bindings from the left (lateral evaluation). *(phase: evaluate first)*
3. A source may be a table name, a path expression rooted at a bound name, a parenthesized subquery, or `unpivot`.
4. Scanning an empty table yields no rows; insertion order is preserved for keyless tables. *(phase: evaluate first)*

## Examples

### Minimal

<div class="example">

#### Scanning An

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

#### Insertion Order

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

#### Omitting As

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

#### An Explicit

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

#### Row Unwrapped

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

#### Its Alias

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

#### An Array

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

#### An Empty

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

#### Its Alias

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

#### Into Them

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

#### Literal Csv

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

#### Read_csv In

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

#### Non Array

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

### Compound

<div class="example">

#### Two Comma

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

#### From Cross Projection

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

#### Dot Envelope

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

#### Both Bindings

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

#### An Empty

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

#### Earlier Binding

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

#### Star Scalar

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

#### The Unnested

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

#### Unnest Flattens

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

#### An Empty

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

#### Missing Path

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

#### Non Array

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

#### Table Row

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

#### Unnest An

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

### Error cases

<div class="example">

#### Referencing An

Referencing an undeclared table is a static error.

<p class="example-label">SQL</p>

```sql
create table T;

select * from Ghost;
```

Expected error: `static`

</div>

<div class="example">

#### Referencing An

Referencing an alias not in scope is a static error.

<p class="example-label">SQL</p>

```sql
create table T;

select x.foo from T;
```

Expected error: `static`

</div>

<div class="example">

#### Self Reference

A lateral source may not reference its own alias.

<p class="example-label">SQL</p>

```sql
create table T;

select * from T as t, item.x as item;
```

Expected error: `static`

</div>

<div class="example">

#### Requires Alias

A lateral collection source requires an alias.

<p class="example-label">SQL</p>

```sql
create table T;

select * from T as t, t.items;
```

Expected error: `static`

</div>

## See also

- [Select](@/language/statements/select.md)
- [Unpivot](@/language/statements/unpivot.md) — from-source over object members
- [Where](@/language/statements/where.md) — filters the binding stream after from
