+++
title = "Predicates"
description = "Primitive predicates and boolean connectives. Tests project the predicate through SELECT so the truth value is shown directly. A small section at the end pins the WHERE-boundary rule that a null predicate excludes the row. Scope: comparisons with null, IS [NOT] NULL, IS [NOT] {TRUE|FALSE|UNKNOWN}, AND/OR/NOT, BETWEEN, IN-list."
weight = 9
+++

# Predicates

Primitive predicates and boolean connectives. Tests project the predicate through SELECT so the truth value is shown directly. A small section at the end pins the WHERE-boundary rule that a null predicate excludes the row. Scope: comparisons with null, IS [NOT] NULL, IS [NOT] {TRUE|FALSE|UNKNOWN}, AND/OR/NOT, BETWEEN, IN-list.

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