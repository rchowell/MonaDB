+++
title = "Unpivot"
description = "UNPIVOT ranges over the attribute-value pairs of a tuple, binding the value with AS and the attribute name with AT."
weight = 17
+++

# Unpivot

UNPIVOT ranges over the attribute-value pairs of a tuple, binding the value with AS and the attribute name with AT.

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