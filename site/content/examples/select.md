+++
title = "Select"
description = "Branches of the select constructor (., *, expr, item-list) paired with a single from source."
weight = 6
+++

# Select

Branches of the select constructor (., *, expr, item-list) paired with a single from source.

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