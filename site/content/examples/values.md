+++
title = "Values & Functions"
description = "Literals, predicates, casts, and built-in functions."
weight = 4
+++

# Values & Functions

Expressions evaluated in isolation or inside larger queries.

<div class="example">

## Null

<p class="example-label">SQL</p>

```sql
select null;
```

<p class="example-label">Result</p>

```json
null
```

</div>

<div class="example">

## Boolean

<p class="example-label">SQL</p>

```sql
select true;
```

<p class="example-label">Result</p>

```json
true
```

<p class="example-label">SQL</p>

```sql
select false;
```

<p class="example-label">Result</p>

```json
false
```

</div>

<div class="example">

## Number

<p class="example-label">SQL</p>

```sql
select 1;
```

<p class="example-label">Result</p>

```json
1
```

<p class="example-label">SQL</p>

```sql
select 1.5;
```

<p class="example-label">Result</p>

```json
1.5
```

<p class="example-label">SQL</p>

```sql
select 9007199254740992;
```

<p class="example-label">Result</p>

```json
9007199254740992
```

</div>

<div class="example">

## String

<p class="example-label">SQL</p>

```sql
select 'hello';
```

<p class="example-label">Result</p>

```json
"hello"
```

<p class="example-label">SQL</p>

```sql
select '';
```

<p class="example-label">Result</p>

```json
""
```

<p class="example-label">SQL</p>

```sql
select 'café';
```

<p class="example-label">Result</p>

```json
"café"
```

</div>

<div class="example">

## Array

<p class="example-label">SQL</p>

```sql
select [];
```

<p class="example-label">Result</p>

```json
[]
```

<p class="example-label">SQL</p>

```sql
select [1, 2, 3];
```

<p class="example-label">Result</p>

```json
[ 1, 2, 3 ]
```

<p class="example-label">SQL</p>

```sql
select [1, 'a', null, true];
```

<p class="example-label">Result</p>

```json
[ 1, "a", null, true ]
```

<p class="example-label">SQL</p>

```sql
select [[1, 2], [3, 4]];
```

<p class="example-label">Result</p>

```json
[ [ 1, 2 ], [ 3, 4 ] ]
```

</div>

<div class="example">

## Object

<p class="example-label">SQL</p>

```sql
select {};
```

<p class="example-label">Result</p>

```json
{}
```

<p class="example-label">SQL</p>

```sql
select {x: 1, y: 2};
```

<p class="example-label">Result</p>

```json
{ "x": 1, "y": 2 }
```

<p class="example-label">SQL</p>

```sql
select {items: [1, 2], meta: {n: 2}};
```

<p class="example-label">Result</p>

```json
{ "items": [ 1, 2 ], "meta": { "n": 2 } }
```

</div>

## Predicates

<div class="example">

### Is null True

Shows the result of `is null true`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select null is null from T;
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### Is null False

Shows the result of `is null false`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select 1 is null from T;
```

<p class="example-label">Result</p>

```json
[
  false
]
```

</div>

<div class="example">

### Is Not null

Shows the result of `is not null on null`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select null is not null from T;
```

<p class="example-label">Result</p>

```json
[
  false
]
```

</div>

<div class="example">

### Is Not null

Shows the result of `is not null on value`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select 1 is not null from T;
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### Is null On

Shows the result of `is null on null column`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

create table S;

insert into S ({x: null});

select s.x is null from S as s;
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### Is null On

Shows the result of `is null on absent key`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

create table S;

insert into S ({});

select s.x is null from S as s;
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### Is Not null

Shows the result of `is not null on present key`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

create table S;

insert into S ({x: 1});

select s.x is not null from S as s;
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### True Is True

Shows the result of `true is true`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select true is true from T;
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### False Is True

Shows the result of `false is true`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select false is true from T;
```

<p class="example-label">Result</p>

```json
[
  false
]
```

</div>

<div class="example">

### null Is True

Shows the result of `null is true`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select null is true from T;
```

<p class="example-label">Result</p>

```json
[
  false
]
```

</div>

<div class="example">

### True Is False

Shows the result of `true is false`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select true is false from T;
```

<p class="example-label">Result</p>

```json
[
  false
]
```

</div>

<div class="example">

### False Is False

Shows the result of `false is false`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select false is false from T;
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### null Is False

Shows the result of `null is false`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select null is false from T;
```

<p class="example-label">Result</p>

```json
[
  false
]
```

</div>

<div class="example">

### True Is Unknown

Shows the result of `true is unknown`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select true is unknown from T;
```

<p class="example-label">Result</p>

```json
[
  false
]
```

</div>

<div class="example">

### False Is Unknown

Shows the result of `false is unknown`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select false is unknown from T;
```

<p class="example-label">Result</p>

```json
[
  false
]
```

</div>

<div class="example">

### null Is Unknown

Shows the result of `null is unknown`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select null is unknown from T;
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### Not True

Shows the result of `true is not true`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select true is not true from T;
```

<p class="example-label">Result</p>

```json
[
  false
]
```

</div>

<div class="example">

### Not True

Shows the result of `null is not true`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select null is not true from T;
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### Not False

Shows the result of `null is not false`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select null is not false from T;
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### Not Unknown

Shows the result of `null is not unknown`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select null is not unknown from T;
```

<p class="example-label">Result</p>

```json
[
  false
]
```

</div>

<div class="example">

### Not True

Shows the result of `not true`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select not true from T;
```

<p class="example-label">Result</p>

```json
[
  false
]
```

</div>

<div class="example">

### Not False

Shows the result of `not false`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select not false from T;
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### Not null

Shows the result of `not null`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select not null from T;
```

<p class="example-label">Result</p>

```json
[
  null
]
```

</div>

<div class="example">

### Double Not

Shows the result of `double not`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select not not true from T;
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### And T T

Shows the result of `and t t`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select true and true from T;
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### And T F

Shows the result of `and t f`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select true and false from T;
```

<p class="example-label">Result</p>

```json
[
  false
]
```

</div>

<div class="example">

### And T N

Shows the result of `and t n`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select true and null from T;
```

<p class="example-label">Result</p>

```json
[
  null
]
```

</div>

<div class="example">

### And F T

Shows the result of `and f t`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select false and true from T;
```

<p class="example-label">Result</p>

```json
[
  false
]
```

</div>

<div class="example">

### And F F

Shows the result of `and f f`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select false and false from T;
```

<p class="example-label">Result</p>

```json
[
  false
]
```

</div>

<div class="example">

### False-dominance, False

False-dominance, false AND null is false.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select false and null from T;
```

<p class="example-label">Result</p>

```json
[
  false
]
```

</div>

<div class="example">

### And N T

Shows the result of `and n t`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select null and true from T;
```

<p class="example-label">Result</p>

```json
[
  null
]
```

</div>

<div class="example">

### False-dominance, Null

False-dominance, null AND false is false.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select null and false from T;
```

<p class="example-label">Result</p>

```json
[
  false
]
```

</div>

<div class="example">

### And N N

Shows the result of `and n n`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select null and null from T;
```

<p class="example-label">Result</p>

```json
[
  null
]
```

</div>

<div class="example">

### Or T T

Shows the result of `or t t`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select true or true from T;
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### Or T F

Shows the result of `or t f`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select true or false from T;
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### True-dominance, True

True-dominance, true OR null is true.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select true or null from T;
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### Or F T

Shows the result of `or f t`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select false or true from T;
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### Or F F

Shows the result of `or f f`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select false or false from T;
```

<p class="example-label">Result</p>

```json
[
  false
]
```

</div>

<div class="example">

### Or F N

Shows the result of `or f n`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select false or null from T;
```

<p class="example-label">Result</p>

```json
[
  null
]
```

</div>

<div class="example">

### True-dominance, Null

True-dominance, null OR true is true.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select null or true from T;
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### Or N F

Shows the result of `or n f`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select null or false from T;
```

<p class="example-label">Result</p>

```json
[
  null
]
```

</div>

<div class="example">

### Or N N

Shows the result of `or n n`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select null or null from T;
```

<p class="example-label">Result</p>

```json
[
  null
]
```

</div>

<div class="example">

### Than Or

A OR b AND c parses as a OR (b AND c).

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select false or true and false from T;
```

<p class="example-label">Result</p>

```json
[
  false
]
```

</div>

<div class="example">

### NOT A

NOT a AND b parses as (NOT a) AND b.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select not false and true from T;
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### Parens Override Precedence

Shows the result of `parens override precedence`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select (false or true) and false from T;
```

<p class="example-label">Result</p>

```json
[
  false
]
```

</div>

<div class="example">

### Between Inside

Shows the result of `between inside`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select 5 between 1 and 10 from T;
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### Between Low Boundary

Shows the result of `between low boundary`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select 1 between 1 and 10 from T;
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### Between High Boundary

Shows the result of `between high boundary`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select 10 between 1 and 10 from T;
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### Between Below Range

Shows the result of `between below range`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select 0 between 1 and 10 from T;
```

<p class="example-label">Result</p>

```json
[
  false
]
```

</div>

<div class="example">

### Between Above Range

Shows the result of `between above range`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select 11 between 1 and 10 from T;
```

<p class="example-label">Result</p>

```json
[
  false
]
```

</div>

<div class="example">

### Asymmetric BETWEEN,

Asymmetric BETWEEN, a > b yields false.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select 5 between 10 and 1 from T;
```

<p class="example-label">Result</p>

```json
[
  false
]
```

</div>

<div class="example">

### Not Between Inside

Shows the result of `not between inside`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select 5 not between 1 and 10 from T;
```

<p class="example-label">Result</p>

```json
[
  false
]
```

</div>

<div class="example">

### Not Between Outside

Shows the result of `not between outside`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select 11 not between 1 and 10 from T;
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### Between On Column

Shows the result of `between on column`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

create table N;

insert into N ({x: 5});

select n.x between 1 and 10 from N as n;
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### Strings Order Lexicographically

Strings order lexicographically.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select 'a' < 'b' from T;
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### String Gt

Shows the result of `string gt`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select 'b' > 'a' from T;
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### String Ge Equal

Shows the result of `string ge equal`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select 'a' >= 'a' from T;
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### String Between Inside

Shows the result of `string between inside`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select 'b' between 'a' and 'c' from T;
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### String Between Outside

Shows the result of `string between outside`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select 'd' between 'a' and 'c' from T;
```

<p class="example-label">Result</p>

```json
[
  false
]
```

</div>

<div class="example">

### In Hit

Shows the result of `in hit`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select 2 in (1, 2, 3) from T;
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### In Miss

Shows the result of `in miss`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select 99 in (1, 2, 3) from T;
```

<p class="example-label">Result</p>

```json
[
  false
]
```

</div>

<div class="example">

### In Single Element

Shows the result of `in single element`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select 1 in (1) from T;
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### In String

Shows the result of `in string`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select 'bob' in ('alice', 'bob', 'carol') from T;
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### Not In Hit

Shows the result of `not in hit`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select 2 not in (1, 2, 3) from T;
```

<p class="example-label">Result</p>

```json
[
  false
]
```

</div>

<div class="example">

### Not In Miss

Shows the result of `not in miss`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select 99 not in (1, 2, 3) from T;
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### In On Column

Shows the result of `in on column`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

create table N;

insert into N ({x: 2});

select n.x in (1, 2, 3) from N as n;
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### Pin Existing

Pin existing semantics, null = null is true.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select null = null from T;
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### Pin Existing

Pin existing semantics, null = 1 is false.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select null = 1 from T;
```

<p class="example-label">Result</p>

```json
[
  false
]
```

</div>

<div class="example">

### Pin Existing

Pin existing semantics, null contaminates ne to false.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select null != 1 from T;
```

<p class="example-label">Result</p>

```json
[
  false
]
```

</div>

<div class="example">

### Pin Existing

Pin existing semantics, null != null is false.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select null != null from T;
```

<p class="example-label">Result</p>

```json
[
  false
]
```

</div>

<div class="example">

### Pin Existing

Pin existing semantics, ordering with null is false.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select null < 1 from T;
```

<p class="example-label">Result</p>

```json
[
  false
]
```

</div>

<div class="example">

### Where Boundary

A null predicate result excludes the row at the where boundary.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

create table N;

insert into N ({x: 1});

select n.x from N as n where null and n.x > 0;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

### Includes All

Shows the result of `where true includes all`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

create table N;

insert into N ({x: 1}, {x: 2});

select n.x from N as n where true or null;
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

### Where And Filters

Shows the result of `where and filters`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

create table N;

insert into N ({x: 2, y: 5});

select n.x from N as n where n.x > 1 and n.y > 0;
```

<p class="example-label">Result</p>

```json
[
  2
]
```

</div>

<div class="example">

### Where Or Filters

Shows the result of `where or filters`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

create table N;

insert into N ({x: 3});

select n.x from N as n where n.x = 1 or n.x = 3;
```

<p class="example-label">Result</p>

```json
[
  3
]
```

</div>

<div class="example">

### null Filters

Shows the result of `where is null filters`.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

create table N;

insert into N ({x: 1}, {x: null}, {x: 3});

select * from N where N.x is null;
```

<p class="example-label">Result</p>

```json
[
  { "x": null }
]
```

</div>

<div class="example">

### Two Identical

Two identical array literals are equal.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select [1] = [1] from T;
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### Two Distinct

Two distinct array literals are not equal.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select [1] = [2] from T;
```

<p class="example-label">Result</p>

```json
[
  false
]
```

</div>

<div class="example">

### Ne On

Ne on unequal arrays is true.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select [1] != [2] from T;
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### Ne On

Ne on equal arrays is false.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

select [1, 2] != [1, 2] from T;
```

<p class="example-label">Result</p>

```json
[
  false
]
```

</div>

<div class="example">

### In Where

A non-empty array is truthy in a Where clause.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({});

create table W;

insert into W ({v: 42});

select w.v from W as w where [1];
```

<p class="example-label">Result</p>

```json
[
  42
]
```

</div>

## Casts

<div class="example">

### Int(x) Over

Int(x) over int, float, string, and bool sources.

<p class="example-label">SQL</p>

```sql
select int(7);
```

<p class="example-label">Result</p>

```json
[
  7
]
```

<p class="example-label">SQL</p>

```sql
select int(7);

select int(2.7);
```

<p class="example-label">Result</p>

```json
[
  2
]
```

<p class="example-label">SQL</p>

```sql
select int(7);

select int(2.7);

select int(-2.7);
```

<p class="example-label">Result</p>

```json
[
  -2
]
```

<p class="example-label">SQL</p>

```sql
select int(7);

select int(2.7);

select int(-2.7);

select int(2.0);
```

<p class="example-label">Result</p>

```json
[
  2
]
```

<p class="example-label">SQL</p>

```sql
select int(7);

select int(2.7);

select int(-2.7);

select int(2.0);

select int('5');
```

<p class="example-label">Result</p>

```json
[
  5
]
```

<p class="example-label">SQL</p>

```sql
select int(7);

select int(2.7);

select int(-2.7);

select int(2.0);

select int('5');

select int(' 5 ');
```

<p class="example-label">Result</p>

```json
[
  5
]
```

<p class="example-label">SQL</p>

```sql
select int(7);

select int(2.7);

select int(-2.7);

select int(2.0);

select int('5');

select int(' 5 ');

select int('2.7');
```

<p class="example-label">Result</p>

```json
[
  2
]
```

<p class="example-label">SQL</p>

```sql
select int(7);

select int(2.7);

select int(-2.7);

select int(2.0);

select int('5');

select int(' 5 ');

select int('2.7');

select int(true);
```

<p class="example-label">Result</p>

```json
[
  1
]
```

<p class="example-label">SQL</p>

```sql
select int(7);

select int(2.7);

select int(-2.7);

select int(2.0);

select int('5');

select int(' 5 ');

select int('2.7');

select int(true);

select int(false);
```

<p class="example-label">Result</p>

```json
[
  0
]
```

<p class="example-label">SQL</p>

```sql
select int(7);

select int(2.7);

select int(-2.7);

select int(2.0);

select int('5');

select int(' 5 ');

select int('2.7');

select int(true);

select int(false);

select int('9007199254740993.0') = 9007199254740993;
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### Float(x) Over

Float(x) over int, float, string, and bool sources.

<p class="example-label">SQL</p>

```sql
select float(3);
```

<p class="example-label">Result</p>

```json
[
  3
]
```

<p class="example-label">SQL</p>

```sql
select float(3);

select float(2.5);
```

<p class="example-label">Result</p>

```json
[
  2.5
]
```

<p class="example-label">SQL</p>

```sql
select float(3);

select float(2.5);

select float('1.5');
```

<p class="example-label">Result</p>

```json
[
  1.5
]
```

<p class="example-label">SQL</p>

```sql
select float(3);

select float(2.5);

select float('1.5');

select float('4');
```

<p class="example-label">Result</p>

```json
[
  4
]
```

<p class="example-label">SQL</p>

```sql
select float(3);

select float(2.5);

select float('1.5');

select float('4');

select float(true);
```

<p class="example-label">Result</p>

```json
[
  1
]
```

<p class="example-label">SQL</p>

```sql
select float(3);

select float(2.5);

select float('1.5');

select float('4');

select float(true);

select float(false);
```

<p class="example-label">Result</p>

```json
[
  0
]
```

</div>

<div class="example">

### String(x) Renders

String(x) renders a scalar as text.

<p class="example-label">SQL</p>

```sql
select string(42);
```

<p class="example-label">Result</p>

```json
[
  "42"
]
```

<p class="example-label">SQL</p>

```sql
select string(42);

select string(1.5);
```

<p class="example-label">Result</p>

```json
[
  "1.5"
]
```

<p class="example-label">SQL</p>

```sql
select string(42);

select string(1.5);

select string(true);
```

<p class="example-label">Result</p>

```json
[
  "true"
]
```

<p class="example-label">SQL</p>

```sql
select string(42);

select string(1.5);

select string(true);

select string(false);
```

<p class="example-label">Result</p>

```json
[
  "false"
]
```

<p class="example-label">SQL</p>

```sql
select string(42);

select string(1.5);

select string(true);

select string(false);

select string('hi');
```

<p class="example-label">Result</p>

```json
[
  "hi"
]
```

</div>

<div class="example">

### Bool(x) Over

Bool(x) over numbers and the true/false strings.

<p class="example-label">SQL</p>

```sql
select bool(0);
```

<p class="example-label">Result</p>

```json
[
  false
]
```

<p class="example-label">SQL</p>

```sql
select bool(0);

select bool(1);
```

<p class="example-label">Result</p>

```json
[
  true
]
```

<p class="example-label">SQL</p>

```sql
select bool(0);

select bool(1);

select bool(5);
```

<p class="example-label">Result</p>

```json
[
  true
]
```

<p class="example-label">SQL</p>

```sql
select bool(0);

select bool(1);

select bool(5);

select bool(0.0);
```

<p class="example-label">Result</p>

```json
[
  false
]
```

<p class="example-label">SQL</p>

```sql
select bool(0);

select bool(1);

select bool(5);

select bool(0.0);

select bool(2.5);
```

<p class="example-label">Result</p>

```json
[
  true
]
```

<p class="example-label">SQL</p>

```sql
select bool(0);

select bool(1);

select bool(5);

select bool(0.0);

select bool(2.5);

select bool('true');
```

<p class="example-label">Result</p>

```json
[
  true
]
```

<p class="example-label">SQL</p>

```sql
select bool(0);

select bool(1);

select bool(5);

select bool(0.0);

select bool(2.5);

select bool('true');

select bool('false');
```

<p class="example-label">Result</p>

```json
[
  false
]
```

<p class="example-label">SQL</p>

```sql
select bool(0);

select bool(1);

select bool(5);

select bool(0.0);

select bool(2.5);

select bool('true');

select bool('false');

select bool('TRUE');
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### Number(x) Keeps

Number(x) keeps the int/float-ness of the value.

<p class="example-label">SQL</p>

```sql
select number(7);
```

<p class="example-label">Result</p>

```json
[
  7
]
```

<p class="example-label">SQL</p>

```sql
select number(7);

select number(2.5);
```

<p class="example-label">Result</p>

```json
[
  2.5
]
```

<p class="example-label">SQL</p>

```sql
select number(7);

select number(2.5);

select number('7');
```

<p class="example-label">Result</p>

```json
[
  7
]
```

<p class="example-label">SQL</p>

```sql
select number(7);

select number(2.5);

select number('7');

select number('1.5');
```

<p class="example-label">Result</p>

```json
[
  1.5
]
```

<p class="example-label">SQL</p>

```sql
select number(7);

select number(2.5);

select number('7');

select number('1.5');

select number(true);
```

<p class="example-label">Result</p>

```json
[
  1
]
```

<p class="example-label">SQL</p>

```sql
select number(7);

select number(2.5);

select number('7');

select number('1.5');

select number(true);

select number('5.0');
```

<p class="example-label">Result</p>

```json
[
  5
]
```

</div>

<div class="example">

### Cast Compose

A cast is a normal call — it nests and combines with operators.

<p class="example-label">SQL</p>

```sql
select int('5') + 1;
```

<p class="example-label">Result</p>

```json
[
  6
]
```

<p class="example-label">SQL</p>

```sql
select int('5') + 1;

select int(2.9) + 1;
```

<p class="example-label">Result</p>

```json
[
  3
]
```

<p class="example-label">SQL</p>

```sql
select int('5') + 1;

select int(2.9) + 1;

select int({a: 2.9}.a);
```

<p class="example-label">Result</p>

```json
[
  2
]
```

<p class="example-label">SQL</p>

```sql
select int('5') + 1;

select int(2.9) + 1;

select int({a: 2.9}.a);

select int(float('2.7'));
```

<p class="example-label">Result</p>

```json
[
  2
]
```

</div>

<div class="example">

### Cast null

A null argument short-circuits to null.

<p class="example-label">SQL</p>

```sql
select int(null);
```

<p class="example-label">Result</p>

```json
[
  null
]
```

<p class="example-label">SQL</p>

```sql
select int(null);

select float(null);
```

<p class="example-label">Result</p>

```json
[
  null
]
```

<p class="example-label">SQL</p>

```sql
select int(null);

select float(null);

select string(null);
```

<p class="example-label">Result</p>

```json
[
  null
]
```

<p class="example-label">SQL</p>

```sql
select int(null);

select float(null);

select string(null);

select bool(null);
```

<p class="example-label">Result</p>

```json
[
  null
]
```

<p class="example-label">SQL</p>

```sql
select int(null);

select float(null);

select string(null);

select bool(null);

select number(null);
```

<p class="example-label">Result</p>

```json
[
  null
]
```

</div>

<div class="example">

### Typeof(t(x)) Reports

Typeof(t(x)) reports the target type.

<p class="example-label">SQL</p>

```sql
select typeof(int(2.7));
```

<p class="example-label">Result</p>

```json
[
  "int"
]
```

<p class="example-label">SQL</p>

```sql
select typeof(int(2.7));

select typeof(float(5));
```

<p class="example-label">Result</p>

```json
[
  "float"
]
```

<p class="example-label">SQL</p>

```sql
select typeof(int(2.7));

select typeof(float(5));

select typeof(number(5));
```

<p class="example-label">Result</p>

```json
[
  "int"
]
```

<p class="example-label">SQL</p>

```sql
select typeof(int(2.7));

select typeof(float(5));

select typeof(number(5));

select typeof(number(2.5));
```

<p class="example-label">Result</p>

```json
[
  "float"
]
```

<p class="example-label">SQL</p>

```sql
select typeof(int(2.7));

select typeof(float(5));

select typeof(number(5));

select typeof(number(2.5));

select typeof(string(1));
```

<p class="example-label">Result</p>

```json
[
  "string"
]
```

<p class="example-label">SQL</p>

```sql
select typeof(int(2.7));

select typeof(float(5));

select typeof(number(5));

select typeof(number(2.5));

select typeof(string(1));

select typeof(bool(1));
```

<p class="example-label">Result</p>

```json
[
  "bool"
]
```

<p class="example-label">SQL</p>

```sql
select typeof(int(2.7));

select typeof(float(5));

select typeof(number(5));

select typeof(number(2.5));

select typeof(string(1));

select typeof(bool(1));

select typeof(float('4'));
```

<p class="example-label">Result</p>

```json
[
  "float"
]
```

<p class="example-label">SQL</p>

```sql
select typeof(int(2.7));

select typeof(float(5));

select typeof(number(5));

select typeof(number(2.5));

select typeof(string(1));

select typeof(bool(1));

select typeof(float('4'));

select typeof(number('7'));
```

<p class="example-label">Result</p>

```json
[
  "int"
]
```

<p class="example-label">SQL</p>

```sql
select typeof(int(2.7));

select typeof(float(5));

select typeof(number(5));

select typeof(number(2.5));

select typeof(string(1));

select typeof(bool(1));

select typeof(float('4'));

select typeof(number('7'));

select typeof(number('5.0'));
```

<p class="example-label">Result</p>

```json
[
  "float"
]
```

<p class="example-label">SQL</p>

```sql
select typeof(int(2.7));

select typeof(float(5));

select typeof(number(5));

select typeof(number(2.5));

select typeof(string(1));

select typeof(bool(1));

select typeof(float('4'));

select typeof(number('7'));

select typeof(number('5.0'));

select typeof(number(true));
```

<p class="example-label">Result</p>

```json
[
  "int"
]
```

</div>

<div class="example">

### Bad Conversions

Bad conversions fail at runtime.

<p class="example-label">SQL</p>

```sql
select int('abc');
```

Expected error: `runtime`

<p class="example-label">SQL</p>

```sql
select int('abc');

select int([1]);
```

Expected error: `runtime`

<p class="example-label">SQL</p>

```sql
select int('abc');

select int([1]);

select bool('x');
```

Expected error: `runtime`

<p class="example-label">SQL</p>

```sql
select int('abc');

select int([1]);

select bool('x');

select int(1e19);
```

Expected error: `runtime`

<p class="example-label">SQL</p>

```sql
select int('abc');

select int([1]);

select bool('x');

select int(1e19);

select float([1, 2]);
```

Expected error: `runtime`

<p class="example-label">SQL</p>

```sql
select int('abc');

select int([1]);

select bool('x');

select int(1e19);

select float([1, 2]);

select int('inf');
```

Expected error: `runtime`

<p class="example-label">SQL</p>

```sql
select int('abc');

select int([1]);

select bool('x');

select int(1e19);

select float([1, 2]);

select int('inf');

select float('nan');
```

Expected error: `runtime`

<p class="example-label">SQL</p>

```sql
select int('abc');

select int([1]);

select bool('x');

select int(1e19);

select float([1, 2]);

select int('inf');

select float('nan');

select float('inf');
```

Expected error: `runtime`

<p class="example-label">SQL</p>

```sql
select int('abc');

select int([1]);

select bool('x');

select int(1e19);

select float([1, 2]);

select int('inf');

select float('nan');

select float('inf');

select string([1, 2]);
```

Expected error: `runtime`

<p class="example-label">SQL</p>

```sql
select int('abc');

select int([1]);

select bool('x');

select int(1e19);

select float([1, 2]);

select int('inf');

select float('nan');

select float('inf');

select string([1, 2]);

select string({a: 1});
```

Expected error: `runtime`

</div>

<div class="example">

### Object/array/any Are

Object/array/any are not callable conversion functions.

<p class="example-label">SQL</p>

```sql
select object(1);
```

Expected error: `syntax`

<p class="example-label">SQL</p>

```sql
select object(1);

select array(1);
```

Expected error: `syntax`

<p class="example-label">SQL</p>

```sql
select object(1);

select array(1);

select any(1);
```

Expected error: `syntax`

</div>

## Functions

<div class="example">

### Typeof Reports

Typeof reports the value's runtime type name.

<p class="example-label">SQL</p>

```sql
select typeof(1);
```

<p class="example-label">Result</p>

```json
[
  "int"
]
```

<p class="example-label">SQL</p>

```sql
select typeof(1);

select typeof(1.5);
```

<p class="example-label">Result</p>

```json
[
  "float"
]
```

<p class="example-label">SQL</p>

```sql
select typeof(1);

select typeof(1.5);

select typeof('x');
```

<p class="example-label">Result</p>

```json
[
  "string"
]
```

<p class="example-label">SQL</p>

```sql
select typeof(1);

select typeof(1.5);

select typeof('x');

select typeof(true);
```

<p class="example-label">Result</p>

```json
[
  "bool"
]
```

<p class="example-label">SQL</p>

```sql
select typeof(1);

select typeof(1.5);

select typeof('x');

select typeof(true);

select typeof(null);
```

<p class="example-label">Result</p>

```json
[
  "null"
]
```

<p class="example-label">SQL</p>

```sql
select typeof(1);

select typeof(1.5);

select typeof('x');

select typeof(true);

select typeof(null);

select typeof([1, 2]);
```

<p class="example-label">Result</p>

```json
[
  "array"
]
```

<p class="example-label">SQL</p>

```sql
select typeof(1);

select typeof(1.5);

select typeof('x');

select typeof(true);

select typeof(null);

select typeof([1, 2]);

select typeof({a: 1});
```

<p class="example-label">Result</p>

```json
[
  "object"
]
```

</div>

<div class="example">

### Coalesce Returns

Coalesce returns the first non-null argument.

<p class="example-label">SQL</p>

```sql
select coalesce(null, null, 7);
```

<p class="example-label">Result</p>

```json
[
  7
]
```

<p class="example-label">SQL</p>

```sql
select coalesce(null, null, 7);

select coalesce(null, 'x');
```

<p class="example-label">Result</p>

```json
[
  "x"
]
```

<p class="example-label">SQL</p>

```sql
select coalesce(null, null, 7);

select coalesce(null, 'x');

select coalesce(1, 2);
```

<p class="example-label">Result</p>

```json
[
  1
]
```

<p class="example-label">SQL</p>

```sql
select coalesce(null, null, 7);

select coalesce(null, 'x');

select coalesce(1, 2);

select coalesce(null, null);
```

<p class="example-label">Result</p>

```json
[
  null
]
```

</div>

<div class="example">

### Nullif Yields

Nullif yields null when the two arguments are equal.

<p class="example-label">SQL</p>

```sql
select nullif(5, 5);
```

<p class="example-label">Result</p>

```json
[
  null
]
```

<p class="example-label">SQL</p>

```sql
select nullif(5, 5);

select nullif(5, 3);
```

<p class="example-label">Result</p>

```json
[
  5
]
```

</div>

<div class="example">

### Ifnull /

Ifnull / nvl substitute a default for null.

<p class="example-label">SQL</p>

```sql
select ifnull(null, 'd');
```

<p class="example-label">Result</p>

```json
[
  "d"
]
```

<p class="example-label">SQL</p>

```sql
select ifnull(null, 'd');

select ifnull('v', 'd');
```

<p class="example-label">Result</p>

```json
[
  "v"
]
```

<p class="example-label">SQL</p>

```sql
select ifnull(null, 'd');

select ifnull('v', 'd');

select nvl(null, 9);
```

<p class="example-label">Result</p>

```json
[
  9
]
```

</div>

<div class="example">

### Iif Selects

Iif selects a branch on the truthiness of the condition.

<p class="example-label">SQL</p>

```sql
select iif(true, 'y', 'n');
```

<p class="example-label">Result</p>

```json
[
  "y"
]
```

<p class="example-label">SQL</p>

```sql
select iif(true, 'y', 'n');

select iif(1 = 2, 'y', 'n');
```

<p class="example-label">Result</p>

```json
[
  "n"
]
```

<p class="example-label">SQL</p>

```sql
select iif(true, 'y', 'n');

select iif(1 = 2, 'y', 'n');

select iif(null, 'y', 'n');
```

<p class="example-label">Result</p>

```json
[
  "n"
]
```

</div>

<div class="example">

### Abs Of

Abs of ints and floats.

<p class="example-label">SQL</p>

```sql
select abs(0 - 5);
```

<p class="example-label">Result</p>

```json
[
  5
]
```

<p class="example-label">SQL</p>

```sql
select abs(0 - 5);

select abs(5);
```

<p class="example-label">Result</p>

```json
[
  5
]
```

<p class="example-label">SQL</p>

```sql
select abs(0 - 5);

select abs(5);

select abs(0.0 - 2.5);
```

<p class="example-label">Result</p>

```json
[
  2.5
]
```

</div>

<div class="example">

### Ceil, Floor,

Ceil, floor, and trunc round floats; ints pass through.

<p class="example-label">SQL</p>

```sql
select ceil(3.2);
```

<p class="example-label">Result</p>

```json
[
  4
]
```

<p class="example-label">SQL</p>

```sql
select ceil(3.2);

select ceiling(3.2);
```

<p class="example-label">Result</p>

```json
[
  4
]
```

<p class="example-label">SQL</p>

```sql
select ceil(3.2);

select ceiling(3.2);

select floor(3.8);
```

<p class="example-label">Result</p>

```json
[
  3
]
```

<p class="example-label">SQL</p>

```sql
select ceil(3.2);

select ceiling(3.2);

select floor(3.8);

select trunc(3.9);
```

<p class="example-label">Result</p>

```json
[
  3
]
```

<p class="example-label">SQL</p>

```sql
select ceil(3.2);

select ceiling(3.2);

select floor(3.8);

select trunc(3.9);

select ceil(7);
```

<p class="example-label">Result</p>

```json
[
  7
]
```

</div>

<div class="example">

### Round To

Round to nearest integer, and to n decimals.

<p class="example-label">SQL</p>

```sql
select round(3.7);
```

<p class="example-label">Result</p>

```json
[
  4
]
```

<p class="example-label">SQL</p>

```sql
select round(3.7);

select round(3.14159, 0);
```

<p class="example-label">Result</p>

```json
[
  3
]
```

<p class="example-label">SQL</p>

```sql
select round(3.7);

select round(3.14159, 0);

select round(5);
```

<p class="example-label">Result</p>

```json
[
  5
]
```

</div>

<div class="example">

### Sign Returns

Sign returns -1, 0, or 1.

<p class="example-label">SQL</p>

```sql
select sign(0 - 5);
```

<p class="example-label">Result</p>

```json
[
  -1
]
```

<p class="example-label">SQL</p>

```sql
select sign(0 - 5);

select sign(5);
```

<p class="example-label">Result</p>

```json
[
  1
]
```

<p class="example-label">SQL</p>

```sql
select sign(0 - 5);

select sign(5);

select sign(0);
```

<p class="example-label">Result</p>

```json
[
  0
]
```

</div>

<div class="example">

### Sqrt And

Sqrt and pow (integer powers stay integers).

<p class="example-label">SQL</p>

```sql
select sqrt(9);
```

<p class="example-label">Result</p>

```json
[
  3
]
```

<p class="example-label">SQL</p>

```sql
select sqrt(9);

select pow(2, 10);
```

<p class="example-label">Result</p>

```json
[
  1024
]
```

<p class="example-label">SQL</p>

```sql
select sqrt(9);

select pow(2, 10);

select power(3, 2);
```

<p class="example-label">Result</p>

```json
[
  9
]
```

</div>

<div class="example">

### Exp, Ln,

Exp, ln, and log10 at exact points.

<p class="example-label">SQL</p>

```sql
select exp(0);
```

<p class="example-label">Result</p>

```json
[
  1
]
```

<p class="example-label">SQL</p>

```sql
select exp(0);

select ln(1);
```

<p class="example-label">Result</p>

```json
[
  0
]
```

<p class="example-label">SQL</p>

```sql
select exp(0);

select ln(1);

select log10(1);
```

<p class="example-label">Result</p>

```json
[
  0
]
```

</div>

<div class="example">

### Mod Is

Mod is the integer remainder.

<p class="example-label">SQL</p>

```sql
select mod(7, 3);
```

<p class="example-label">Result</p>

```json
[
  1
]
```

<p class="example-label">SQL</p>

```sql
select mod(7, 3);

select mod(10, 5);
```

<p class="example-label">Result</p>

```json
[
  0
]
```

</div>

<div class="example">

### Greatest And

Greatest and least over numbers and strings.

<p class="example-label">SQL</p>

```sql
select greatest(1, 5, 3);
```

<p class="example-label">Result</p>

```json
[
  5
]
```

<p class="example-label">SQL</p>

```sql
select greatest(1, 5, 3);

select least(1, 5, 3);
```

<p class="example-label">Result</p>

```json
[
  1
]
```

<p class="example-label">SQL</p>

```sql
select greatest(1, 5, 3);

select least(1, 5, 3);

select greatest('a', 'c', 'b');
```

<p class="example-label">Result</p>

```json
[
  "c"
]
```

</div>

<div class="example">

### Length Dispatches

Length dispatches on value — chars, elements, or members.

<p class="example-label">SQL</p>

```sql
select length('hello');
```

<p class="example-label">Result</p>

```json
[
  5
]
```

<p class="example-label">SQL</p>

```sql
select length('hello');

select length([1, 2, 3]);
```

<p class="example-label">Result</p>

```json
[
  3
]
```

<p class="example-label">SQL</p>

```sql
select length('hello');

select length([1, 2, 3]);

select length({x: 1, y: 2});
```

<p class="example-label">Result</p>

```json
[
  2
]
```

<p class="example-label">SQL</p>

```sql
select length('hello');

select length([1, 2, 3]);

select length({x: 1, y: 2});

select length('café');
```

<p class="example-label">Result</p>

```json
[
  4
]
```

</div>

<div class="example">

### Upper And

Upper and lower case strings.

<p class="example-label">SQL</p>

```sql
select upper('hi');
```

<p class="example-label">Result</p>

```json
[
  "HI"
]
```

<p class="example-label">SQL</p>

```sql
select upper('hi');

select lower('HI');
```

<p class="example-label">Result</p>

```json
[
  "hi"
]
```

</div>

<div class="example">

### Trim, Ltrim,

Trim, ltrim, and rtrim strip surrounding whitespace.

<p class="example-label">SQL</p>

```sql
select trim('  hi  ');
```

<p class="example-label">Result</p>

```json
[
  "hi"
]
```

<p class="example-label">SQL</p>

```sql
select trim('  hi  ');

select ltrim('  hi');
```

<p class="example-label">Result</p>

```json
[
  "hi"
]
```

<p class="example-label">SQL</p>

```sql
select trim('  hi  ');

select ltrim('  hi');

select rtrim('hi  ');
```

<p class="example-label">Result</p>

```json
[
  "hi"
]
```

</div>

<div class="example">

### Substr Is

Substr is 1-based, with an optional length.

<p class="example-label">SQL</p>

```sql
select substr('hello', 2);
```

<p class="example-label">Result</p>

```json
[
  "ello"
]
```

<p class="example-label">SQL</p>

```sql
select substr('hello', 2);

select substr('hello', 2, 3);
```

<p class="example-label">Result</p>

```json
[
  "ell"
]
```

<p class="example-label">SQL</p>

```sql
select substr('hello', 2);

select substr('hello', 2, 3);

select substring('hello', 1, 1);
```

<p class="example-label">Result</p>

```json
[
  "h"
]
```

</div>

<div class="example">

### Replace Swaps

Replace swaps every occurrence of a substring.

<p class="example-label">SQL</p>

```sql
select replace('aXbXc', 'X', '-');
```

<p class="example-label">Result</p>

```json
[
  "a-b-c"
]
```

</div>

<div class="example">

### Concat Joins

Concat joins arguments, skipping nulls, stringifying scalars.

<p class="example-label">SQL</p>

```sql
select concat('a', 'b', 'c');
```

<p class="example-label">Result</p>

```json
[
  "abc"
]
```

<p class="example-label">SQL</p>

```sql
select concat('a', 'b', 'c');

select concat('a', null, 'b');
```

<p class="example-label">Result</p>

```json
[
  "ab"
]
```

<p class="example-label">SQL</p>

```sql
select concat('a', 'b', 'c');

select concat('a', null, 'b');

select concat('n', 42);
```

<p class="example-label">Result</p>

```json
[
  "n42"
]
```

</div>

<div class="example">

### Concat_ws Joins

Concat_ws joins with a separator, skipping nulls.

<p class="example-label">SQL</p>

```sql
select concat_ws('-', 'a', 'b', 'c');
```

<p class="example-label">Result</p>

```json
[
  "a-b-c"
]
```

<p class="example-label">SQL</p>

```sql
select concat_ws('-', 'a', 'b', 'c');

select concat_ws(',', 'a', null, 'b');
```

<p class="example-label">Result</p>

```json
[
  "a,b"
]
```

</div>

<div class="example">

### Repeat Duplicates

Repeat duplicates a string; reverse is dynamic on value.

<p class="example-label">SQL</p>

```sql
select repeat('ab', 3);
```

<p class="example-label">Result</p>

```json
[
  "ababab"
]
```

<p class="example-label">SQL</p>

```sql
select repeat('ab', 3);

select reverse('abc');
```

<p class="example-label">Result</p>

```json
[
  "cba"
]
```

<p class="example-label">SQL</p>

```sql
select repeat('ab', 3);

select reverse('abc');

select reverse([1, 2, 3]);
```

<p class="example-label">Result</p>

```json
[
  [ 3, 2, 1 ]
]
```

</div>

<div class="example">

### Lpad And

Lpad and rpad pad to a target width with a fill string.

<p class="example-label">SQL</p>

```sql
select lpad('5', 3, '0');
```

<p class="example-label">Result</p>

```json
[
  "005"
]
```

<p class="example-label">SQL</p>

```sql
select lpad('5', 3, '0');

select rpad('5', 3, '0');
```

<p class="example-label">Result</p>

```json
[
  "500"
]
```

<p class="example-label">SQL</p>

```sql
select lpad('5', 3, '0');

select rpad('5', 3, '0');

select lpad('x', 3);
```

<p class="example-label">Result</p>

```json
[
  "  x"
]
```

</div>

<div class="example">

### Repeat /

Repeat / lpad reject pathological sizes instead of exhausting memory.

<p class="example-label">SQL</p>

```sql
select repeat('x', 99999999999);
```

Expected error: `runtime`

<p class="example-label">SQL</p>

```sql
select repeat('x', 99999999999);

select lpad('x', 99999999999);
```

Expected error: `runtime`

</div>

<div class="example">

### Strpos /

Strpos / instr return a 1-based index, 0 if absent.

<p class="example-label">SQL</p>

```sql
select strpos('hello', 'l');
```

<p class="example-label">Result</p>

```json
[
  3
]
```

<p class="example-label">SQL</p>

```sql
select strpos('hello', 'l');

select instr('hello', 'l');
```

<p class="example-label">Result</p>

```json
[
  3
]
```

<p class="example-label">SQL</p>

```sql
select strpos('hello', 'l');

select instr('hello', 'l');

select strpos('hello', 'z');
```

<p class="example-label">Result</p>

```json
[
  0
]
```

</div>

<div class="example">

### Starts_with, Ends_with,

Starts_with, ends_with, contains (dynamic on value).

<p class="example-label">SQL</p>

```sql
select starts_with('hello', 'he');
```

<p class="example-label">Result</p>

```json
[
  true
]
```

<p class="example-label">SQL</p>

```sql
select starts_with('hello', 'he');

select ends_with('hello', 'lo');
```

<p class="example-label">Result</p>

```json
[
  true
]
```

<p class="example-label">SQL</p>

```sql
select starts_with('hello', 'he');

select ends_with('hello', 'lo');

select contains('hello', 'ell');
```

<p class="example-label">Result</p>

```json
[
  true
]
```

<p class="example-label">SQL</p>

```sql
select starts_with('hello', 'he');

select ends_with('hello', 'lo');

select contains('hello', 'ell');

select contains([1, 2, 3], 2);
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### Array_length, Array_contains, Array_position

Array_length, array_contains, array_position.

<p class="example-label">SQL</p>

```sql
select array_length([1, 2, 3]);
```

<p class="example-label">Result</p>

```json
[
  3
]
```

<p class="example-label">SQL</p>

```sql
select array_length([1, 2, 3]);

select array_contains([1, 2, 3], 2);
```

<p class="example-label">Result</p>

```json
[
  true
]
```

<p class="example-label">SQL</p>

```sql
select array_length([1, 2, 3]);

select array_contains([1, 2, 3], 2);

select array_position([10, 20, 30], 20);
```

<p class="example-label">Result</p>

```json
[
  2
]
```

<p class="example-label">SQL</p>

```sql
select array_length([1, 2, 3]);

select array_contains([1, 2, 3], 2);

select array_position([10, 20, 30], 20);

select array_position([10, 20], 99);
```

<p class="example-label">Result</p>

```json
[
  0
]
```

</div>

<div class="example">

### Array_append, Array_prepend, Array_concat

Array_append, array_prepend, array_concat.

<p class="example-label">SQL</p>

```sql
select array_append([1, 2], 3);
```

<p class="example-label">Result</p>

```json
[
  [ 1, 2, 3 ]
]
```

<p class="example-label">SQL</p>

```sql
select array_append([1, 2], 3);

select array_prepend(0, [1, 2]);
```

<p class="example-label">Result</p>

```json
[
  [ 0, 1, 2 ]
]
```

<p class="example-label">SQL</p>

```sql
select array_append([1, 2], 3);

select array_prepend(0, [1, 2]);

select array_concat([1, 2], [3, 4]);
```

<p class="example-label">Result</p>

```json
[
  [ 1, 2, 3, 4 ]
]
```

</div>

<div class="example">

### Array_reverse, Array_distinct,

Array_reverse, array_distinct, array_slice, array_to_string.

<p class="example-label">SQL</p>

```sql
select array_reverse([1, 2, 3]);
```

<p class="example-label">Result</p>

```json
[
  [ 3, 2, 1 ]
]
```

<p class="example-label">SQL</p>

```sql
select array_reverse([1, 2, 3]);

select array_distinct([1, 2, 2, 3, 1]);
```

<p class="example-label">Result</p>

```json
[
  [ 1, 2, 3 ]
]
```

<p class="example-label">SQL</p>

```sql
select array_reverse([1, 2, 3]);

select array_distinct([1, 2, 2, 3, 1]);

select array_slice([1, 2, 3, 4, 5], 2, 4);
```

<p class="example-label">Result</p>

```json
[
  [ 2, 3, 4 ]
]
```

<p class="example-label">SQL</p>

```sql
select array_reverse([1, 2, 3]);

select array_distinct([1, 2, 2, 3, 1]);

select array_slice([1, 2, 3, 4, 5], 2, 4);

select array_to_string([1, 2, 3], '-');
```

<p class="example-label">Result</p>

```json
[
  "1-2-3"
]
```

</div>

<div class="example">

### Object_keys, Object_values, Object_has_key

Object_keys, object_values, object_has_key.

<p class="example-label">SQL</p>

```sql
select object_keys({a: 1, b: 2});
```

<p class="example-label">Result</p>

```json
[
  [ "a", "b" ]
]
```

<p class="example-label">SQL</p>

```sql
select object_keys({a: 1, b: 2});

select object_values({a: 1, b: 2});
```

<p class="example-label">Result</p>

```json
[
  [ 1, 2 ]
]
```

<p class="example-label">SQL</p>

```sql
select object_keys({a: 1, b: 2});

select object_values({a: 1, b: 2});

select object_has_key({a: 1}, 'a');
```

<p class="example-label">Result</p>

```json
[
  true
]
```

<p class="example-label">SQL</p>

```sql
select object_keys({a: 1, b: 2});

select object_values({a: 1, b: 2});

select object_has_key({a: 1}, 'a');

select object_has_key({a: 1}, 'z');
```

<p class="example-label">Result</p>

```json
[
  false
]
```

</div>

<div class="example">

### Fn null Propagation

A null argument to a strict function yields null.

<p class="example-label">SQL</p>

```sql
select abs(null);
```

<p class="example-label">Result</p>

```json
[
  null
]
```

<p class="example-label">SQL</p>

```sql
select abs(null);

select upper(null);
```

<p class="example-label">Result</p>

```json
[
  null
]
```

<p class="example-label">SQL</p>

```sql
select abs(null);

select upper(null);

select length(null);
```

<p class="example-label">Result</p>

```json
[
  null
]
```

<p class="example-label">SQL</p>

```sql
select abs(null);

select upper(null);

select length(null);

select round(null, 2);
```

<p class="example-label">Result</p>

```json
[
  null
]
```

<p class="example-label">SQL</p>

```sql
select abs(null);

select upper(null);

select length(null);

select round(null, 2);

select array_length(null);
```

<p class="example-label">Result</p>

```json
[
  null
]
```

</div>

<div class="example">

### Wrong Argument

Wrong argument count is a static error.

<p class="example-label">SQL</p>

```sql
select abs(1, 2);
```

Expected error: `static`

<p class="example-label">SQL</p>

```sql
select abs(1, 2);

select upper();
```

Expected error: `static`

</div>

<div class="example">

### An Undefined

An undefined function name is a static error.

<p class="example-label">SQL</p>

```sql
select bogus(1);
```

Expected error: `static`

</div>

<div class="example">

### Fn Type Error

A wrong-typed argument is a runtime error.

<p class="example-label">SQL</p>

```sql
select abs('x');
```

Expected error: `runtime`

<p class="example-label">SQL</p>

```sql
select abs('x');

select upper(42);
```

Expected error: `runtime`

</div>
