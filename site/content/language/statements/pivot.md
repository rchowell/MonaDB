+++
title = "Pivot"
description = "Pivot replaces the select constructor and folds the entire binding stream into a single object."
weight = 4
+++

Pivot replaces the select constructor and folds the entire binding stream into a single object. Each surviving tuple contributes one member `name: value`. It is the dual of Unpivot.

## Syntax

### Railroad

<div class="rr">
<div class="rr-track"><span class="rr-t">pivot</span><span class="rr-join" aria-hidden="true"></span><span class="rr-n">value</span><span class="rr-join" aria-hidden="true"></span><span class="rr-t">at</span><span class="rr-join" aria-hidden="true"></span><span class="rr-n">name</span><span class="rr-join" aria-hidden="true"></span><span class="rr-t">from</span><span class="rr-join" aria-hidden="true"></span><span class="rr-n">source</span><span class="rr-join" aria-hidden="true"></span><span class="rr-rep"><span class="rr-rep-inner"><span class="rr-t">,</span><span class="rr-join" aria-hidden="true"></span><span class="rr-n">source</span></span></span><span class="rr-join" aria-hidden="true"></span><span class="rr-opt"><span class="rr-opt-inner"><span class="rr-t">where</span><span class="rr-join" aria-hidden="true"></span><span class="rr-n">expr</span></span></span><span class="rr-join" aria-hidden="true"></span><span class="rr-t">;</span></div>
</div>

### BNF

```ebnf
pivot-stmt ::= "pivot" expr "at" expr "from" source ( "," source )* [ where-clause ] ";"
```

## Rules

1. Pivot requires a `from` clause and yields exactly one object (one output row). *(phase: evaluate last — replaces **select**)*
2. `name` must evaluate to `string`; tuples whose name is not a string contribute no member. *(phase: evaluate last)*
3. Repeated names are last-wins across the folded stream. *(phase: evaluate last)*
4. An empty input stream yields `{}`. *(phase: evaluate last)*
5. v1 supports `from` and `where` only; `order by` and `limit` on pivot queries are deferred. *(phase: evaluate last)*

## Examples

### Minimal

<div class="example">

#### Pivot Builds

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

### Compound

<div class="example">

#### Pivot Inverts

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

#### Pivot Over

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

#### Wins Duplicate

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

#### String Name

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

#### Where Filters

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

### Error cases

<div class="example">

#### Pivot With

Pivot with order by is not supported in v1.

<p class="example-label">SQL</p>

```sql
create table T;

create table prices;

pivot p.price at p.sym from prices as p order by p.sym;
```

Expected error: `static`

</div>

## See also

- [Unpivot](@/language/statements/unpivot.md)
- [From](@/language/statements/from.md)
- [Where](@/language/statements/where.md)
