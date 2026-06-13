+++
title = "Statements"
description = "SELECT, INSERT, DELETE, CREATE TABLE, DROP TABLE, and CLEAR."
weight = 2
+++

# Statements

## Select

The Select clause maps the current binding stream through a constructor: an expression, an object literal, a list of named expressions, `*` to spread bound variables, or `.` to wrap each binding under its alias.

```
select <constructor>
  [from <source> [, <source> …]]
  [where <expr>]
  [order by <expr> [asc|desc] [, …]]
  [limit <n> | limit <n>.. | limit <n>..<m>];
```

<div class="example">

### Envelope Object

Select . emits the binding tuple as an envelope object.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1});

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

### Bindings Flat

Select * spreads bindings flat.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1, y: 2});

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

### Per Row

Select <path-expr> emits a scalar per row.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2});

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

### Per Row

Select <literal-expr> emits the literal once per row.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2});

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

### Per Row

Select <object-expr> emits the object once per row.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1});

"select {a: t.x} from T as t;"
```

<p class="example-label">Result</p>

```json
[
  { "a": 1 }
]
```

</div>

<div class="example">

### Named Field

Select <expr> as <name> emits an object with the named field.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 10});

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

### Named Member

A list of items emits an object with each named member.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1, y: 2});

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

### List Items

List items may be arbitrary expressions, not only paths.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1});

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

### From <ident>

From <ident> uses the table name as the implicit alias.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1});

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

### From <ident>

From <ident> as <ident> binds the source under an explicit alias.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 7});

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

### From <ident>

From <ident> <ident> binds the source under an alias without 'as'.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 9});

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

### An Array

An array literal builds an array from its element expressions.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 7});

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

### Array Literals

Array literals may nest.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1});

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

### Single Row

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

### Nothing Spread

Select * requires a From clause (nothing to spread).

<p class="example-label">SQL</p>

```sql
create table T;

select *;
```

Expected error: `static`

</div>

<div class="example">

### Tuple Envelope

Select . requires a From clause (no binding tuple to envelope).

<p class="example-label">SQL</p>

```sql
create table T;

select .;
```

Expected error: `static`

</div>

## From

The From clause iterates sources — table scans, array literals, and lateral unnest paths — binding each row under an alias for use in later clauses.

<div class="example">

### Scanning An

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

### Insertion Order

A table scan returns all rows in insertion order.

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

### Omitting As

Omitting `as` uses the table name as the alias.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 42});

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

### An Explicit

An explicit `as` alias names the binding.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 10});

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

### Row Unwrapped

Select * over one source emits the row unwrapped.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1, y: 2});

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

### Its Alias

Select . wraps the binding under its alias.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1});

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

### Two Comma

Two comma sources form a Cartesian product, merged by select *.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into S ({b: 10}, {b: 20});

insert into T ({a: 1}, {a: 2});

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

### From Cross Projection

A projection list may reference both cross-joined bindings.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into S ({b: 10}, {b: 20});

insert into T ({a: 1}, {a: 2});

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

### Dot Envelope

Select . over two sources keys each binding by its alias.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into S ({b: 10});

insert into T ({a: 1});

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

### Both Bindings

A where predicate filters the product across both bindings.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into S ({b: 10}, {b: 20});

insert into T ({a: 1}, {a: 2});

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

### An Empty

An empty inner source makes the whole product empty.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into T ({a: 1}, {a: 2});

select * from T as t, S as s;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

### Referencing An

Referencing an undeclared table is a static error.

<p class="example-label">SQL</p>

```sql
create table T;

select * from Ghost;
```

Expected error: `static`

</div>

<div class="example">

### Referencing An

Referencing an alias not in scope is a static error.

<p class="example-label">SQL</p>

```sql
create table T;

select x.foo from T;
```

Expected error: `static`

</div>

<div class="example">

### Earlier Binding

A later source may unnest a collection path on an earlier binding.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({items: [1, 2, 3]});

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

### Star Scalar

Select * keeps a non-object (scalar) lateral binding under its alias.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({items: [1, 2, 3]});

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

### The Unnested

The unnested element binds under its alias in the envelope.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({items: [1, 2]});

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

### Unnest Flattens

Unnest flattens across every outer row in order.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({k: 1, items: [10, 11]}, {k: 2, items: [20]});

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

### An Empty

An empty collection contributes no rows for that outer binding.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({items: []});

select item as v from T as t, t.items as item;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

### Missing Path

A missing path is treated as empty (inner-join-like).

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1});

select item as v from T as t, t.items as item;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

### Non Array

A non-array source value contributes no rows.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({items: 5});

select item as v from T as t, t.items as item;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

### An Array

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

### Self Reference

A lateral source may not reference its own alias.

<p class="example-label">SQL</p>

```sql
create table T;

select * from T as t, item.x as item;
```

Expected error: `static`

</div>

<div class="example">

### Requires Alias

A lateral collection source requires an alias.

<p class="example-label">SQL</p>

```sql
create table T;

select * from T as t, t.items;
```

Expected error: `static`

</div>

<div class="example">

### An Empty

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

### Its Alias

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

### Into Them

A value source iterates object elements and may path into them.

<p class="example-label">SQL</p>

```sql
create table T;

select x.a as a from [{a: 1}, {a: 2}] as x;
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

### Non Array

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

### Table Row

A value source re-iterates for every outer table row.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({a: 1}, {a: 2});

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

### Unnest An

Unnest an array of objects and path into each element.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({s: [{x: 1}, {x: 2}]}, {s: [{x: 3}]});

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

## Unpivot

The Unpivot clause is a from-source that ranges over the attribute-value pairs of a tuple, binding each pair's value with `as` and its attribute name with `at` — the dual of Pivot.

```
from unpivot <expr> [as <value>] [at <name>]
```

<div class="example">

### Unpivot Yields

Unpivot yields one binding per attribute-value pair, the value bound by AS.

<p class="example-label">SQL</p>

```sql
create table T;

select price from unpivot {amzn: 1900, goog: 1120} as price;
```

<p class="example-label">Result</p>

```json
[
  1900,
  1120
]
```

</div>

<div class="example">

### AT Binds

AT binds the attribute name of each pair.

<p class="example-label">SQL</p>

```sql
create table T;

select sym as sym, price as price from unpivot {amzn: 1900, goog: 1120} as price at sym;
```

<p class="example-label">Result</p>

```json
[
  { "sym": "amzn", "price": 1900 },
  { "sym": "goog", "price": 1120 }
]
```

</div>

<div class="example">

### Pairs Are

Pairs are produced in object member order.

<p class="example-label">SQL</p>

```sql
create table T;

select sym as sym from unpivot {c: 3, a: 1, b: 2} as price at sym;
```

<p class="example-label">Result</p>

```json
[
  { "sym": "c" },
  { "sym": "a" },
  { "sym": "b" }
]
```

</div>

<div class="example">

### AT May

AT may be omitted, binding only the value.

<p class="example-label">SQL</p>

```sql
create table T;

select price from unpivot {a: 10, b: 20} as price;
```

<p class="example-label">Result</p>

```json
[
  10,
  20
]
```

</div>

<div class="example">

### Unpivot Dot Envelope

Select . envelopes the value and attribute bindings under their aliases.

<p class="example-label">SQL</p>

```sql
create table T;

select . from unpivot {a: 1, b: 2} as v at k;
```

<p class="example-label">Result</p>

```json
[
  { "v": 1, "k": "a" },
  { "v": 2, "k": "b" }
]
```

</div>

<div class="example">

### Their Aliases

Select * spreads the scalar bindings under their aliases.

<p class="example-label">SQL</p>

```sql
create table T;

select * from unpivot {a: 1, b: 2} as v at k;
```

<p class="example-label">Result</p>

```json
[
  { "v": 1, "k": "a" },
  { "v": 2, "k": "b" }
]
```

</div>

<div class="example">

### On Name

A where predicate may filter on the attribute name.

<p class="example-label">SQL</p>

```sql
create table T;

select price from unpivot {a: 1, b: 2, c: 3} as price at sym where sym != 'b';
```

<p class="example-label">Result</p>

```json
[
  1,
  3
]
```

</div>

<div class="example">

### Unpivot Of

Unpivot of a non-object value contributes no rows.

<p class="example-label">SQL</p>

```sql
create table T;

select price from unpivot 5 as price at sym;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

### Unpivot Of

Unpivot of an empty object contributes no rows.

<p class="example-label">SQL</p>

```sql
create table T;

select price from unpivot {} as price;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

### Unpivot A

Unpivot a table row's columns into (name, value) rows.

<p class="example-label">SQL</p>

```sql
create table T;

create table closing;

insert into closing ({date: "d1", amzn: 1900, goog: 1120});

select sym as sym, price as price from closing as c, unpivot c as price at sym where sym != 'date';
```

<p class="example-label">Result</p>

```json
[
  { "sym": "amzn", "price": 1900 },
  { "sym": "goog", "price": 1120 }
]
```

</div>

<div class="example">

### Unpivot Flattens

Unpivot flattens every outer row's members in order.

<p class="example-label">SQL</p>

```sql
create table T;

create table P;

insert into P ({a: 1, b: 2}, {a: 3, b: 4});

select sym as k, price as v from P as p, unpivot p as price at sym;
```

<p class="example-label">Result</p>

```json
[
  { "k": "a", "v": 1 },
  { "k": "b", "v": 2 },
  { "k": "a", "v": 3 },
  { "k": "b", "v": 4 }
]
```

</div>

<div class="example">

### Order By

Order by sorts the unpivoted pairs by the attribute name.

<p class="example-label">SQL</p>

```sql
create table T;

select price from unpivot {b: 2, a: 1, c: 3} as price at sym order by sym;
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

### The Unpivot

The unpivot value binding requires an alias.

<p class="example-label">SQL</p>

```sql
create table T;

select . from unpivot {a: 1};
```

Expected error: `static`

</div>

## Pivot

The Pivot clause replaces select, folding the whole binding stream into a single tuple: each row contributes one `name: value` member — the dual of Unpivot.

```
pivot <value> at <name> from <source> [where <expr>];
```

<div class="example">

### Pivot Builds

Pivot builds one object with an attribute per input row.

<p class="example-label">SQL</p>

```sql
create table T;

create table prices;

insert into prices ({sym: "amzn", price: 1900}, {sym: "goog", price: 1120});

pivot p.price at p.sym from prices as p;
```

<p class="example-label">Result</p>

```json
[
  { "amzn": 1900, "goog": 1120 }
]
```

</div>

<div class="example">

### Pivot Inverts

Pivot inverts unpivot over the same value.

<p class="example-label">SQL</p>

```sql
create table T;

pivot price at sym from unpivot {a: 1, b: 2, c: 3} as price at sym;
```

<p class="example-label">Result</p>

```json
[
  { "a": 1, "b": 2, "c": 3 }
]
```

</div>

<div class="example">

### Pivot Over

Pivot over an empty stream yields a single empty object.

<p class="example-label">SQL</p>

```sql
create table T;

create table empty_t;

pivot p.price at p.sym from empty_t as p;
```

<p class="example-label">Result</p>

```json
[
  {  }
]
```

</div>

<div class="example">

### Wins Duplicate

A repeated attribute name is last-wins.

<p class="example-label">SQL</p>

```sql
create table T;

create table d;

insert into d ({k: "x", v: 1}, {k: "x", v: 2});

pivot e.v at e.k from d as e;
```

<p class="example-label">Result</p>

```json
[
  { "x": 2 }
]
```

</div>

<div class="example">

### String Name

A row whose AT name is not a string contributes no attribute.

<p class="example-label">SQL</p>

```sql
create table T;

create table m;

insert into m ({k: "ok", v: 1}, {k: 5, v: 2});

pivot e.v at e.k from m as e;
```

<p class="example-label">Result</p>

```json
[
  { "ok": 1 }
]
```

</div>

<div class="example">

### Where Filters

Where filters which rows contribute attributes.

<p class="example-label">SQL</p>

```sql
create table T;

create table prices;

insert into prices ({sym: "a", price: 1}, {sym: "b", price: 2}, {sym: "c", price: 3});

pivot p.price at p.sym from prices as p where p.price > 1;
```

<p class="example-label">Result</p>

```json
[
  { "b": 2, "c": 3 }
]
```

</div>

<div class="example">

### Pivot With

Pivot with order by is not supported in v1.

<p class="example-label">SQL</p>

```sql
create table T;

create table prices;

pivot p.price at p.sym from prices as p order by p.sym;
```

Expected error: `static`

</div>

## Where

The Where clause filters binding tuples by a boolean predicate. Only rows for which the predicate is true pass through.

<div class="example">

### Constant True

Constant true keeps all rows.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2});

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

### Constant False

Constant false drops all rows.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1});

select * from T where false;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

### Null Predicate

Null predicate is not-true and drops all rows.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1});

select * from T where null;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

### Numeric Greater-than

Numeric greater-than filters by oid insertion order.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2}, {x: 3});

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

### Numeric Equality

Numeric equality matches a single row.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2}, {x: 3});

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

### Numeric Inequality

Numeric inequality excludes matching value.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2}, {x: 3});

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

### String Equality

String equality in where.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into S ({name: 'alice'}, {name: 'bob'});

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

### Boolean Equality

Boolean equality in where.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({flag: true}, {flag: false});

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

### Predicate May

Predicate may use an explicit from alias.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 0}, {x: 1});

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

### Null Member

Null member compares equal to null.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: null});

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

### Null Member

Null member fails inequality against non-null.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: null});

select * from T where T.x != 1;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

### Absent Field

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
  {  }
]
```

</div>

<div class="example">

### Absent Field

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

### Only Rows

Only rows with present matching field pass equality.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {});

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

### Scalar Projection

Scalar projection with where (moved from from-clause suite).

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2}, {x: 3});

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

## Order by

The Order by clause sorts the binding-tuple stream by one or more keys. Ascending is the default; nulls sort last in ascending order and first in descending order.

<div class="example">

### Order By

Order by sorts ascending by default.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 3}, {x: 1}, {x: 2});

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

### Explicit Asc

Explicit asc matches the default.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 3}, {x: 1}, {x: 2});

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

### Desc Sorts Descending

Desc sorts descending.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 3}, {x: 1}, {x: 2});

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

### Order By

Order by reorders whole rows under select *.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 3}, {x: 1}, {x: 2});

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

### Multiple Keys

Multiple keys sort left-to-right with per-key direction.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({a: 1, b: 2}, {a: 1, b: 1}, {a: 2, b: 5});

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

### Null Sorts

Null sorts after all values in ascending order.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 2}, {x: null}, {x: 1});

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

### Null Sorts

Null sorts before all values in descending order.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 2}, {x: null}, {x: 1});

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

### Ints And

Ints and floats interleave by numeric value.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 2}, {x: 1.5}, {x: 1});

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

### Strings Sort Lexicographically

Strings sort lexicographically.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({s: "banana"}, {s: "apple"}, {s: "cherry"});

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

### Order By

Order by then limit yields the top N.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 3}, {x: 1}, {x: 2}, {x: 5}, {x: 4});

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

### Order Sorts

Order sorts the post-where stream.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 3}, {x: 1}, {x: 2}, {x: 4});

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

### Order By

Order by sorts the cross product of two sources.

<p class="example-label">SQL</p>

```sql
create table T;

create table S;

insert into T ({x: 2}, {x: 1});

insert into S ({y: 9});

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

## Limit

The Limit clause slices the stream by row position: `limit n` takes the first n rows, `limit n..` skips the first n, and `limit n..m` selects the half-open index range [n, m).

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

## Insert

Insert adds one or more values to a table. The values list is parenthesised and comma-separated; a trailing comma is permitted.

```
insert into <table> (<value>, …);
```

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

## Delete

Delete removes rows from a table. Without a Where clause, every row is removed.

```
delete from <table> [as <alias>] [where <expr>];
```

<div class="example">

### Empties Table

Delete with no where removes every row.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2}, {x: 3});

delete from T;

select * from T;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

### Matching Rows

Delete with a predicate removes only matching rows.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2}, {x: 3});

delete from T where T.x > 1;

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

### An Explicit

An explicit `as` alias binds the predicate's references.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2}, {x: 3});

delete from T as r where r.x = 2;

select * from T;
```

<p class="example-label">Result</p>

```json
[
  { "x": 1 },
  { "x": 3 }
]
```

</div>

<div class="example">

### Table Unchanged

A predicate that matches nothing leaves the table unchanged.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2}, {x: 3});

delete from T where T.x > 100;

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

### Deleting From

Deleting from an empty table succeeds and yields nothing.

<p class="example-label">SQL</p>

```sql
create table T;

delete from T;

select * from T;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

### Reuses Table

A table is reusable after a delete-all.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2});

delete from T;

insert into T ({x: 9});

select * from T;
```

<p class="example-label">Result</p>

```json
[
  { "x": 9 }
]
```

</div>

<div class="example">

### Deleting From

Deleting from an undeclared table is a static error.

<p class="example-label">SQL</p>

```sql
create table T;

delete from Ghost;
```

Expected error: `static`

</div>

## Create Table

Create Table declares a table with an optional list of key columns. Key columns must be `int` or `string` and define the physical sort order; keyless tables keep surrogate ids and return rows in insertion order.

```
create table <name> [(<key> int|string, …)];
```

<div class="example">

### Whole Objects

A keyless table stores and returns whole objects.

<p class="example-label">SQL</p>

```sql
create table t;

insert into t ({x: 1, y: 2, z: 3});

select * from t;
```

<p class="example-label">Result</p>

```json
[
  { "x": 1, "y": 2, "z": 3 }
]
```

</div>

<div class="example">

### Ones X

A keyless table accepts any object, including ones with no x.

<p class="example-label">SQL</p>

```sql
create table t;

insert into t ({y: 2, z: 3});

select * from t;
```

<p class="example-label">Result</p>

```json
[
  { "y": 2, "z": 3 }
]
```

</div>

<div class="example">

### Surrogate Ids

Surrogate ids increment, so rows come back in insertion order.

<p class="example-label">SQL</p>

```sql
create table t;

insert into t ({x: 3}, {x: 1}, {x: 2});

select * from t;
```

<p class="example-label">Result</p>

```json
[
  { "x": 3 },
  { "x": 1 },
  { "x": 2 }
]
```

</div>

<div class="example">

### Int Key

Int key with payload round-trips whole object.

<p class="example-label">SQL</p>

```sql
create table t (x int);

insert into t ({x: 1, z: 9});

select * from t;
```

<p class="example-label">Result</p>

```json
[
  { "x": 1, "z": 9 }
]
```

</div>

<div class="example">

### Rows Inserted

Rows inserted out of order come back sorted by the int key.

<p class="example-label">SQL</p>

```sql
create table t (x int);

insert into t ({x: 3}, {x: 1}, {x: 2});

select * from t;
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

### Negative Ints

Negative ints sort before zero and positives (sign-flip encoding).

<p class="example-label">SQL</p>

```sql
create table t (x int);

insert into t ({x: 1}, {x: -5}, {x: 0}, {x: -1});

select * from t;
```

<p class="example-label">Result</p>

```json
[
  { "x": -5 },
  { "x": -1 },
  { "x": 0 },
  { "x": 1 }
]
```

</div>

<div class="example">

### Re-inserting The

Re-inserting the same key overwrites (last write wins).

<p class="example-label">SQL</p>

```sql
create table t (x int);

insert into t ({x: 1, v: 100});

insert into t ({x: 1, v: 200});

select * from t;
```

<p class="example-label">Result</p>

```json
[
  { "x": 1, "v": 200 }
]
```

</div>

<div class="example">

### Inserting Without

Inserting without the key field is a schema error.

<p class="example-label">SQL</p>

```sql
create table t (x int);

insert into t ({z: 9});
```

Expected error: `schema`

</div>

<div class="example">

### Wrong Type

A string where an int key is declared is a schema error.

<p class="example-label">SQL</p>

```sql
create table t (x int);

insert into t ({x: "a"});
```

Expected error: `schema`

</div>

<div class="example">

### Non Integral

A non-integral number for an int key is a schema error.

<p class="example-label">SQL</p>

```sql
create table t (x int);

insert into t ({x: 1.5});
```

Expected error: `schema`

</div>

<div class="example">

### String Key

String key with payload round-trips whole object.

<p class="example-label">SQL</p>

```sql
create table t (x string);

insert into t ({x: "a", z: 9});

select * from t;
```

<p class="example-label">Result</p>

```json
[
  { "x": "a", "z": 9 }
]
```

</div>

<div class="example">

### Rows Come

Rows come back in lexicographic key order.

<p class="example-label">SQL</p>

```sql
create table t (x string);

insert into t ({x: "c"}, {x: "a"}, {x: "b"});

select * from t;
```

<p class="example-label">Result</p>

```json
[
  { "x": "a" },
  { "x": "b" },
  { "x": "c" }
]
```

</div>

<div class="example">

### Inserting Without

Inserting without the key field is a schema error.

<p class="example-label">SQL</p>

```sql
create table t (x string);

insert into t ({z: 9});
```

Expected error: `schema`

</div>

<div class="example">

### Wrong Type

A number where a string key is declared is a schema error.

<p class="example-label">SQL</p>

```sql
create table t (x string);

insert into t ({x: 1});
```

Expected error: `schema`

</div>

<div class="example">

### Composite (int,

Composite (int, string) key round-trips whole object.

<p class="example-label">SQL</p>

```sql
create table t (a int, b string);

insert into t ({a: 1, b: "x", z: 9});

select * from t;
```

<p class="example-label">Result</p>

```json
[
  { "a": 1, "b": "x", "z": 9 }
]
```

</div>

<div class="example">

### Sort By

Sort by first component, tie-break on the second.

<p class="example-label">SQL</p>

```sql
create table t (a int, b string);

insert into t ({a: 2, b: "a"}, {a: 1, b: "y"}, {a: 1, b: "x"});

select * from t;
```

<p class="example-label">Result</p>

```json
[
  { "a": 1, "b": "x" },
  { "a": 1, "b": "y" },
  { "a": 2, "b": "a" }
]
```

</div>

<div class="example">

### Missing The

Missing the first key field is a schema error.

<p class="example-label">SQL</p>

```sql
create table t (a int, b string);

insert into t ({b: "x"});
```

Expected error: `schema`

</div>

<div class="example">

### Missing The

Missing the second key field is a schema error.

<p class="example-label">SQL</p>

```sql
create table t (a int, b string);

insert into t ({a: 1});
```

Expected error: `schema`

</div>

<div class="example">

### Wrong Type

Wrong type for the first key field is a schema error.

<p class="example-label">SQL</p>

```sql
create table t (a int, b string);

insert into t ({a: "q", b: "x"});
```

Expected error: `schema`

</div>

<div class="example">

### Wrong Type

Wrong type for the second key field is a schema error.

<p class="example-label">SQL</p>

```sql
create table t (a int, b string);

insert into t ({a: 1, b: 2});
```

Expected error: `schema`

</div>

<div class="example">

### Composite (string,

Composite (string, int) key round-trips whole object.

<p class="example-label">SQL</p>

```sql
create table t (a string, b int);

insert into t ({a: "x", b: 1, z: 9});

select * from t;
```

<p class="example-label">Result</p>

```json
[
  { "a": "x", "b": 1, "z": 9 }
]
```

</div>

<div class="example">

### Sort By

Sort by string first, tie-break on the int.

<p class="example-label">SQL</p>

```sql
create table t (a string, b int);

insert into t ({a: "b", b: 1}, {a: "a", b: 2}, {a: "a", b: 1});

select * from t;
```

<p class="example-label">Result</p>

```json
[
  { "a": "a", "b": 1 },
  { "a": "a", "b": 2 },
  { "a": "b", "b": 1 }
]
```

</div>

<div class="example">

### Missing The

Missing the int component is a schema error.

<p class="example-label">SQL</p>

```sql
create table t (a string, b int);

insert into t ({a: "x"});
```

Expected error: `schema`

</div>

<div class="example">

### Type Second

A string where the int component is declared is a schema error.

<p class="example-label">SQL</p>

```sql
create table t (a string, b int);

insert into t ({a: "x", b: "y"});
```

Expected error: `schema`

</div>

<div class="example">

### Composite (int,

Composite (int, int) key round-trips whole object.

<p class="example-label">SQL</p>

```sql
create table t (a int, b int);

insert into t ({a: 1, b: 2, z: 9});

select * from t;
```

<p class="example-label">Result</p>

```json
[
  { "a": 1, "b": 2, "z": 9 }
]
```

</div>

<div class="example">

### Sort By

Sort by first int, tie-break on the second int.

<p class="example-label">SQL</p>

```sql
create table t (a int, b int);

insert into t ({a: 2, b: 1}, {a: 1, b: 2}, {a: 1, b: 1});

select * from t;
```

<p class="example-label">Result</p>

```json
[
  { "a": 1, "b": 1 },
  { "a": 1, "b": 2 },
  { "a": 2, "b": 1 }
]
```

</div>

<div class="example">

### Missing A

Missing a key component is a schema error.

<p class="example-label">SQL</p>

```sql
create table t (a int, b int);

insert into t ({a: 1});
```

Expected error: `schema`

</div>

<div class="example">

### Composite (string,

Composite (string, string) key round-trips whole object.

<p class="example-label">SQL</p>

```sql
create table t (a string, b string);

insert into t ({a: "x", b: "y", z: 9});

select * from t;
```

<p class="example-label">Result</p>

```json
[
  { "a": "x", "b": "y", "z": 9 }
]
```

</div>

<div class="example">

### Sort By

Sort by first string, tie-break on the second.

<p class="example-label">SQL</p>

```sql
create table t (a string, b string);

insert into t ({a: "b", b: "a"}, {a: "a", b: "b"}, {a: "a", b: "a"});

select * from t;
```

<p class="example-label">Result</p>

```json
[
  { "a": "a", "b": "a" },
  { "a": "a", "b": "b" },
  { "a": "b", "b": "a" }
]
```

</div>

<div class="example">

### Before Ab

A shorter first component sorts before a longer one that shares its prefix, regardless of the second component — proves the string terminator. ("a","z") must sort before ("ab","a").

<p class="example-label">SQL</p>

```sql
create table t (a string, b string);

insert into t ({a: "ab", b: "a"}, {a: "a", b: "z"});

select * from t;
```

<p class="example-label">Result</p>

```json
[
  { "a": "a", "b": "z" },
  { "a": "ab", "b": "a" }
]
```

</div>

<div class="example">

### Missing A

Missing a key component is a schema error.

<p class="example-label">SQL</p>

```sql
create table t (a string, b string);

insert into t ({a: "x"});
```

Expected error: `schema`

</div>

<div class="example">

### Float Key

A float key column is rejected at create.

<p class="example-label">SQL</p>

```sql
create table t (x float);
```

Expected error: `static`

</div>

<div class="example">

### Bool Key

A bool key column is rejected at create.

<p class="example-label">SQL</p>

```sql
create table t (x bool);
```

Expected error: `static`

</div>

## Drop Table

Drop Table removes a table and all its contents from the catalog.

```
drop table <name>;
```

<div class="example">

### After Drop,

After drop, the table can no longer be selected from.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1});

drop table T;

select * from T;
```

Expected error: `static`

</div>

<div class="example">

### Is Empty

A table re-created after drop is fresh and empty.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2});

drop table T;

create table T;

select * from T;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

### Dropping An

Dropping an undeclared table is a static error.

<p class="example-label">SQL</p>

```sql
drop table Ghost;
```

Expected error: `static`

</div>

## Clear

Clear removes every row from a table but keeps the table definition.

```
clear table <name>;
```

<div class="example">

### Table Place

Clear removes every row but leaves the table in place.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2}, {x: 3});

clear table T;

select * from T;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

### New Rows

A cleared table is still resolvable and accepts new rows.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1});

clear table T;

insert into T ({x: 9});

select * from T;
```

<p class="example-label">Result</p>

```json
[
  { "x": 9 }
]
```

</div>

<div class="example">

### Clearing An

Clearing an undeclared table is a static error.

<p class="example-label">SQL</p>

```sql
clear table Ghost;
```

Expected error: `static`

</div>