+++
title = "Select"
description = "The Select clause is the final projection of a query."
weight = 1
+++

The Select clause is the final projection of a query. It runs once per binding tuple in the current stream and emits one output value per tuple — a scalar, a constructed object, a flat spread of bindings, or an envelope object keyed by source aliases.

## Syntax

### Railroad

<div class="rr">
<div class="rr-track"><span class="rr-t">select</span><span class="rr-join" aria-hidden="true"></span><span class="rr-or"><span class="rr-branch"><span class="rr-t">.</span></span><span class="rr-branch"><span class="rr-t">*</span></span><span class="rr-branch"><span class="rr-n">expr</span></span><span class="rr-branch"><span class="rr-n">select-list</span></span></span><span class="rr-join" aria-hidden="true"></span><span class="rr-opt"><span class="rr-opt-inner"><span class="rr-t">from</span><span class="rr-join" aria-hidden="true"></span><span class="rr-n">source</span><span class="rr-join" aria-hidden="true"></span><span class="rr-rep"><span class="rr-rep-inner"><span class="rr-t">,</span><span class="rr-join" aria-hidden="true"></span><span class="rr-n">source</span></span></span></span></span><span class="rr-join" aria-hidden="true"></span><span class="rr-opt"><span class="rr-opt-inner"><span class="rr-t">where</span><span class="rr-join" aria-hidden="true"></span><span class="rr-n">expr</span></span></span><span class="rr-join" aria-hidden="true"></span><span class="rr-opt"><span class="rr-opt-inner"><span class="rr-t">order</span><span class="rr-join" aria-hidden="true"></span><span class="rr-t">by</span><span class="rr-join" aria-hidden="true"></span><span class="rr-n">order-key</span><span class="rr-join" aria-hidden="true"></span><span class="rr-rep"><span class="rr-rep-inner"><span class="rr-t">,</span><span class="rr-join" aria-hidden="true"></span><span class="rr-n">order-key</span></span></span></span></span><span class="rr-join" aria-hidden="true"></span><span class="rr-opt"><span class="rr-opt-inner"><span class="rr-t">limit</span><span class="rr-join" aria-hidden="true"></span><span class="rr-n">range</span></span></span><span class="rr-join" aria-hidden="true"></span><span class="rr-t">;</span></div>
</div>

### BNF

```ebnf
select-stmt ::= "select" select-ctor query-body-opt ";"

select-ctor ::= "."
              | "*"
              | expr
              | select-list

select-list ::= select-item ( "," select-item )*

select-item ::= expr [ "as" identifier ]

query-body-opt ::= ε | query-body

query-body ::= from-clause [ where-clause ] [ order-clause ] [ limit-clause ]
```

## Rules

1. `select expr` emits a scalar per row, not an object. *(phase: evaluate last — after **from**, **where**, **order by**, and **limit**)*
2. `select item, item, …` is shorthand for `select {item, item, …}`; an item `expr as name` introduces an output key, and a path item such as `t.x` uses the last segment as the key.
3. `select *` spreads all bindings flat; `select .` wraps each binding under its source alias.
4. With no `from` clause, the query produces exactly one output row.
5. Aggregate functions are valid only in the select projection (ungrouped aggregation reduces the post-where stream to one row). *(phase: evaluate last)*

## Examples

### Minimal

<div class="example">

#### Envelope Object

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

#### Bindings Flat

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

#### Per Row

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

#### Per Row

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

#### Per Row

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

#### Named Field

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

#### List Items

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

#### From <ident>

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

#### From <ident>

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

#### From <ident>

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

#### An Array

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

#### Single Row

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

### Compound

<div class="example">

#### Named Member

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

#### Array Literals

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

### Error cases

<div class="example">

#### Nothing Spread

Select * requires a From clause (nothing to spread).

<p class="example-label">SQL</p>

```sql
create table T;

select *;
```

Expected error: `static`

</div>

<div class="example">

#### Tuple Envelope

Select . requires a From clause (no binding tuple to envelope).

<p class="example-label">SQL</p>

```sql
create table T;

select .;
```

Expected error: `static`

</div>

## See also

- [From](@/language/statements/from.md) — introduces bindings consumed by select
- [Where](@/language/statements/where.md) — filters rows before projection
- [Expressions](@/language/expressions.md) — constructor and projection expressions
