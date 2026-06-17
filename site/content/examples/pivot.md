+++
title = "Pivot & Unpivot"
description = "Pivot and unpivot examples — reshape row streams into tuples and back."
weight = 5
+++

# Pivot & Unpivot

Pivot folds a binding stream into a single tuple; unpivot ranges over attribute-value pairs of a tuple — dual operations for reshaping data.

<div class="example-section">

## Pivot

</div>

<div class="example">

## Pivot Builds

Pivot builds one object with an attribute per input row.

<p class="example-label">SQL</p>

```sql
create table T;

create table prices;

insert into prices ({"sym": "amzn", "price": 1900}, {"sym": "goog", "price": 1120});

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

## Pivot Inverts

Pivot inverts unpivot over the same value.

<p class="example-label">SQL</p>

```sql
create table T;

pivot price at sym from unpivot {"a": 1, "b": 2, "c": 3} as price at sym;
```

<p class="example-label">Result</p>

```json
[
  { "a": 1, "b": 2, "c": 3 }
]
```

</div>

<div class="example">

## Pivot Over

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
  {}
]
```

</div>

<div class="example">

## Wins Duplicate

A repeated attribute name is last-wins.

<p class="example-label">SQL</p>

```sql
create table T;

create table d;

insert into d ({"k": "x", "v": 1}, {"k": "x", "v": 2});

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

## String Name

A row whose AT name is not a string contributes no attribute.

<p class="example-label">SQL</p>

```sql
create table T;

create table m;

insert into m ({"k": "ok", "v": 1}, {"k": 5, "v": 2});

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

## Where Filters

Where filters which rows contribute attributes.

<p class="example-label">SQL</p>

```sql
create table T;

create table prices;

insert into prices ({"sym": "a", "price": 1}, {"sym": "b", "price": 2}, {"sym": "c", "price": 3});

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

## Pivot With

Pivot with order by is not supported in v1.

<p class="example-label">SQL</p>

```sql
create table T;

create table prices;

pivot p.price at p.sym from prices as p order by p.sym;
```

Expected error: `static`

</div>

<div class="example-section">

## Unpivot

</div>

<div class="example">

## Unpivot Yields

Unpivot yields one binding per attribute-value pair, the value bound by AS.

<p class="example-label">SQL</p>

```sql
create table T;

select price from unpivot {"amzn": 1900, "goog": 1120} as price;
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

## AT Binds

AT binds the attribute name of each pair.

<p class="example-label">SQL</p>

```sql
create table T;

select sym as sym, price as price from unpivot {"amzn": 1900, "goog": 1120} as price at sym;
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

## Pairs Are

Pairs are produced in object member order.

<p class="example-label">SQL</p>

```sql
create table T;

select sym as sym from unpivot {"c": 3, "a": 1, "b": 2} as price at sym;
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

## AT May

AT may be omitted, binding only the value.

<p class="example-label">SQL</p>

```sql
create table T;

select price from unpivot {"a": 10, "b": 20} as price;
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

## Unpivot Dot Envelope

Select . envelopes the value and attribute bindings under their aliases.

<p class="example-label">SQL</p>

```sql
create table T;

select . from unpivot {"a": 1, "b": 2} as v at k;
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

## Their Aliases

Select * spreads the scalar bindings under their aliases.

<p class="example-label">SQL</p>

```sql
create table T;

select * from unpivot {"a": 1, "b": 2} as v at k;
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

## On Name

A where predicate may filter on the attribute name.

<p class="example-label">SQL</p>

```sql
create table T;

select price from unpivot {"a": 1, "b": 2, "c": 3} as price at sym where sym != 'b';
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

## Unpivot Of

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

## Unpivot Of

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

## Unpivot A

Unpivot a table row's columns into (name, value) rows.

<p class="example-label">SQL</p>

```sql
create table T;

create table closing;

insert into closing ({"date": "d1", "amzn": 1900, "goog": 1120});

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

## Unpivot Flattens

Unpivot flattens every outer row's members in order.

<p class="example-label">SQL</p>

```sql
create table T;

create table P;

insert into P ({"a": 1, "b": 2}, {"a": 3, "b": 4});

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

## Order By

Order by sorts the unpivoted pairs by the attribute name.

<p class="example-label">SQL</p>

```sql
create table T;

select price from unpivot {"b": 2, "a": 1, "c": 3} as price at sym order by sym;
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

## An Aggregate

An aggregate folds over the unpivoted value binding.

<p class="example-label">SQL</p>

```sql
create table T;

select sum(price) from unpivot {"a": 10, "b": 20, "c": 30} as price;
```

<p class="example-label">Result</p>

```json
[
  60
]
```

</div>

<div class="example">

## Count Over

Count over an unpivot source counts one row per pair.

<p class="example-label">SQL</p>

```sql
create table T;

select count(price) from unpivot {"a": 1, "b": 2, "c": 3} as price;
```

<p class="example-label">Result</p>

```json
[
  3
]
```

</div>

<div class="example">

## Group By

Group by over an unpivot source groups its value binding.

<p class="example-label">SQL</p>

```sql
create table T;

select { "v": price, "n": count(*) } from unpivot {"a": 5, "b": 5, "c": 9} as price group by price;
```

<p class="example-label">Result</p>

```json
[
  { "v": 5, "n": 2 },
  { "v": 9, "n": 1 }
]
```

</div>

<div class="example">

## The Unpivot

The unpivot value binding requires an alias.

<p class="example-label">SQL</p>

```sql
create table T;

select . from unpivot {"a": 1};
```

Expected error: `static`

</div>
