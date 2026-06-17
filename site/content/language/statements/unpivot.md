+++
title = "Unpivot"
description = "Unpivot is a from-source that ranges over the attribute–value pairs of an object."
weight = 3
+++

Unpivot is a from-source that ranges over the attribute–value pairs of an object. Each pair binds its value under the required `as` alias and its attribute name under the optional `at` alias. It is the dual of Pivot.

## Syntax

### Railroad

<div class="rr">
<div class="rr-track"><span class="rr-t">unpivot</span><span class="rr-join" aria-hidden="true"></span><span class="rr-n">expr</span><span class="rr-join" aria-hidden="true"></span><span class="rr-t">as</span><span class="rr-join" aria-hidden="true"></span><span class="rr-n">value</span><span class="rr-join" aria-hidden="true"></span><span class="rr-opt"><span class="rr-opt-inner"><span class="rr-t">at</span><span class="rr-join" aria-hidden="true"></span><span class="rr-n">name</span></span></span></div>
</div>

### BNF

```ebnf
unpivot-source ::= "unpivot" expr "as" identifier [ "at" identifier ]
```

## Rules

1. The value alias (`as`) is required; the attribute-name alias (`at`) is optional. *(phase: evaluate as part of **from**)*
2. Each object member produces one output row with the value and name bindings. *(phase: evaluate as part of **from**)*
3. When `expr` is not an object, unpivot yields no rows (inner-join semantics). *(phase: evaluate as part of **from**)*
4. Unpivot may appear inline in a comma-separated from list and may reference lateral bindings from preceding sources. *(phase: evaluate as part of **from**)*

## Examples

### Minimal

<div class="example">

#### Unpivot Yields

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

### Compound

<div class="example">

#### AT Binds

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

#### Pairs Are

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

#### AT May

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

#### Unpivot Dot Envelope

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

#### Their Aliases

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

#### On Name

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

#### Unpivot Of

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

#### Unpivot Of

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

#### Unpivot A

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

#### Unpivot Flattens

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

#### Order By

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

#### An Aggregate

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

#### Count Over

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

#### Group By

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

### Error cases

<div class="example">

#### The Unpivot

The unpivot value binding requires an alias.

<p class="example-label">SQL</p>

```sql
create table T;

select . from unpivot {"a": 1};
```

Expected error: `static`

</div>

## See also

- [Pivot](@/language/statements/pivot.md) — inverse fold into one object
- [From](@/language/statements/from.md)
