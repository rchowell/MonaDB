+++
title = "Literals"
description = "Literal expressions — null, bool, number, string, array, object — evaluated via select with no from clause."
weight = 1
+++

# Literals

Literal expressions — null, bool, number, string, array, object — evaluated via select with no from clause.

<div class="example">

### Null Literal

Null literal evaluates to null.

<p class="example-label">SQL</p>

```sql
select null;
```

<p class="example-label">Result</p>

```json
[
  null
]
```

</div>

<div class="example">

### True Boolean Literal

True boolean literal.

<p class="example-label">SQL</p>

```sql
select true;
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### False Boolean Literal

False boolean literal.

<p class="example-label">SQL</p>

```sql
select false;
```

<p class="example-label">Result</p>

```json
[
  false
]
```

</div>

<div class="example">

### Integer Number Literal

Integer number literal.

<p class="example-label">SQL</p>

```sql
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

### Floating-point Number Literal

Floating-point number literal.

<p class="example-label">SQL</p>

```sql
select 1.5;
```

<p class="example-label">Result</p>

```json
[
  1.5
]
```

</div>

<div class="example">

### Integer At

Integer at the edge of exact float representation (2^53).

<p class="example-label">SQL</p>

```sql
select 9007199254740992;
```

<p class="example-label">Result</p>

```json
[
  9007199254740992
]
```

</div>

<div class="example">

### Single-quoted String Literal

Single-quoted string literal.

<p class="example-label">SQL</p>

```sql
select 'hello';
```

<p class="example-label">Result</p>

```json
[
  "hello"
]
```

</div>

<div class="example">

### Empty String Literal

Empty string literal.

<p class="example-label">SQL</p>

```sql
select '';
```

<p class="example-label">Result</p>

```json
[
  ""
]
```

</div>

<div class="example">

### Embedded Single

Embedded single quote is escaped by doubling.

<p class="example-label">SQL</p>

```sql
select 'it''s';
```

<p class="example-label">Result</p>

```json
[
  "it's"
]
```

</div>

<div class="example">

### String With

String with Unicode content.

<p class="example-label">SQL</p>

```sql
select 'café';
```

<p class="example-label">Result</p>

```json
[
  "café"
]
```

</div>

<div class="example">

### Empty Array Literal

Empty array literal.

<p class="example-label">SQL</p>

```sql
select [];
```

<p class="example-label">Result</p>

```json
[
  []
]
```

</div>

<div class="example">

### Array Literal

Array literal with two elements yields a single array value.

<p class="example-label">SQL</p>

```sql
select [1, 2];
```

<p class="example-label">Result</p>

```json
[
  [ 1, 2 ]
]
```

</div>

<div class="example">

### Array Literal

Array literal of numbers.

<p class="example-label">SQL</p>

```sql
select [1, 2, 3];
```

<p class="example-label">Result</p>

```json
[
  [ 1, 2, 3 ]
]
```

</div>

<div class="example">

### Array With

Array with mixed types.

<p class="example-label">SQL</p>

```sql
select [1, 'a', null, true];
```

<p class="example-label">Result</p>

```json
[
  [ 1, "a", null, true ]
]
```

</div>

<div class="example">

### Nested Array Literal

Nested array literal.

<p class="example-label">SQL</p>

```sql
select [[1, 2], [3, 4]];
```

<p class="example-label">Result</p>

```json
[
  [ [ 1, 2 ], [ 3, 4 ] ]
]
```

</div>

<div class="example">

### Empty Object Literal

Empty object literal.

<p class="example-label">SQL</p>

```sql
select {};
```

<p class="example-label">Result</p>

```json
[
  {  }
]
```

</div>

<div class="example">

### Object Literal

Object literal with two members.

<p class="example-label">SQL</p>

```sql
select {x: 1, y: 2};
```

<p class="example-label">Result</p>

```json
[
  { "x": 1, "y": 2 }
]
```

</div>

<div class="example">

### Object Key

Object key as string literal.

<p class="example-label">SQL</p>

```sql
select {'a-b': 1};
```

<p class="example-label">Result</p>

```json
[
  { "a-b": 1 }
]
```

</div>

<div class="example">

### Nested Object

Nested object and array in a literal.

<p class="example-label">SQL</p>

```sql
select {items: [1, 2], meta: {n: 2}};
```

<p class="example-label">Result</p>

```json
[
  { "items": [ 1, 2 ], "meta": { "n": 2 } }
]
```

</div>

<div class="example">

### Keywords Are Case-insensitive

Keywords are case-insensitive.

<p class="example-label">SQL</p>

```sql
SELECT null;
```

<p class="example-label">Result</p>

```json
[
  null
]
```

<p class="example-label">SQL</p>

```sql
SELECT null;

Select True;
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>