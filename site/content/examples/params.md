+++
title = "Parameters"
description = "Query parameter binding examples."
weight = 6
+++

# Parameters

Placeholders (`?`, `$N`, `$name`) stand in for values supplied alongside the SQL text and are substituted before compilation.

<div class="example">

## Positional Single

A lone `?` resolves to the first list element.

<p class="example-label">SQL</p>

```sql
select ?;
```

<p class="example-label">Result</p>

```json
[
  42
]
```

</div>

<div class="example">

## Each ?

Each `?` consumes the next list slot in source order.

<p class="example-label">SQL</p>

```sql
select [?, ?, ?];
```

<p class="example-label">Result</p>

```json
[
  [ 1, 2, 3 ]
]
```

</div>

<div class="example">

## Positional String

A string parameter.

<p class="example-label">SQL</p>

```sql
select ?;
```

<p class="example-label">Result</p>

```json
[
  "hello"
]
```

</div>

<div class="example">

## Positional null

A null parameter.

<p class="example-label">SQL</p>

```sql
select ?;
```

<p class="example-label">Result</p>

```json
[
  null
]
```

</div>

<div class="example">

## Parameters Compose

Parameters compose inside an expression.

<p class="example-label">SQL</p>

```sql
select ? + ?;
```

<p class="example-label">Result</p>

```json
[
  30
]
```

</div>

<div class="example">

## $1 Is

$1 is the first (1-based) list element.

<p class="example-label">SQL</p>

```sql
select $1;
```

<p class="example-label">Result</p>

```json
[
  7
]
```

</div>

<div class="example">

## The Same

The same index may be referenced more than once.

<p class="example-label">SQL</p>

```sql
select [$1, $1, $2];
```

<p class="example-label">Result</p>

```json
[
  [ 9, 9, 8 ]
]
```

</div>

<div class="example">

## Numbered References

Numbered references pick by index, not by appearance.

<p class="example-label">SQL</p>

```sql
select [$2, $1];
```

<p class="example-label">Result</p>

```json
[
  [ "b", "a" ]
]
```

</div>

<div class="example">

## The First

The first `?` and `$1` both resolve to list[0].

<p class="example-label">SQL</p>

```sql
select [?, $1];
```

<p class="example-label">Result</p>

```json
[
  [ 5, 5 ]
]
```

</div>

<div class="example">

## Named Single

A named parameter resolves from the map.

<p class="example-label">SQL</p>

```sql
select $foo;
```

<p class="example-label">Result</p>

```json
[
  99
]
```

</div>

<div class="example">

## Several Named Parameters

Several named parameters.

<p class="example-label">SQL</p>

```sql
select [$a, $b];
```

<p class="example-label">Result</p>

```json
[
  [ 1, 2 ]
]
```

</div>

<div class="example">

## Where Positional

A parameter as a where-clause operand.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1}, {"x": 2}, {"x": 3});

select * from T where T.x > ?;
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

## Where Named

A named parameter in a Where clause.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1}, {"x": 2});

select * from T where T.x = $val;
```

<p class="example-label">Result</p>

```json
[
  { "x": 2 }
]
```

</div>

<div class="example">

## Keyed Get Positional

A parameter as a keyed-table index (substituted to a literal key).

<p class="example-label">SQL</p>

```sql
create table t (id int);

insert into t ({"id": 1, "v": "a"}, {"id": 2, "v": "b"});

select t[?];
```

<p class="example-label">Result</p>

```json
[
  { "id": 2, "v": "b" }
]
```

</div>

<div class="example">

## ? With

`?` with too few list elements is a bind error.

<p class="example-label">SQL</p>

```sql
select [?, ?];
```

Expected error: `static`

</div>

<div class="example">

## $2 With

$2 with a one-element list is a bind error.

<p class="example-label">SQL</p>

```sql
select $2;
```

Expected error: `static`

</div>

<div class="example">

## An Absent

An absent named key is a bind error.

<p class="example-label">SQL</p>

```sql
select $foo;
```

Expected error: `static`

</div>

<div class="example">

## $0 Is

$0 is out of range (numbering is 1-based).

<p class="example-label">SQL</p>

```sql
select $0;
```

Expected error: `static`

</div>

<div class="example">

## Numbered Overflow

A numbered index past u32 is a static bind error, not an opaque lexer error.

<p class="example-label">SQL</p>

```sql
select $4294967296;
```

Expected error: `static`

</div>

<div class="example">

## No Parameters Supplied

A placeholder with no parameters at all is a bind error.

<p class="example-label">SQL</p>

```sql
select ?;
```

Expected error: `static`

</div>
