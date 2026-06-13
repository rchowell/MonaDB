+++
title = "Literals"
description = "Literal expressions — null, bool, number, string, array, object — evaluated via select with no from clause."
weight = 1
+++

# Literals

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