+++
title = "Keyed lookup"
description = "Key-index point lookup. A keyed table behaves like a big dict: `table[key]` resolves the receiver to a table (not a value) and does an order-preserving btree point lookup, returning the stored row — or null when the key is absent (Redis GET → nil). It is an ordinary expression, so it composes and rides the bare `select <expr>` form. v1 covers single-column AND composite full-key literal lookups, plus partial-key (leading-prefix) lookups that return the sub-sequence of matching rows as an array, in key order (empty on no match)."
weight = 12
+++

# Keyed lookup

Key-index point lookup. A keyed table behaves like a big dict: `table[key]` resolves the receiver to a table (not a value) and does an order-preserving btree point lookup, returning the stored row — or null when the key is absent (Redis GET → nil). It is an ordinary expression, so it composes and rides the bare `select <expr>` form. v1 covers single-column AND composite full-key literal lookups, plus partial-key (leading-prefix) lookups that return the sub-sequence of matching rows as an array, in key order (empty on no match).

<div class="example">

### Indexing By

Indexing by an existing int key returns the whole row.

<p class="example-label">SQL</p>

```sql
create table t (id int);

insert into t ({id: 1, v: "a"});

select t[1];
```

<p class="example-label">Result</p>

```json
[
  { "id": 1, "v": "a" }
]
```

</div>

<div class="example">

### Nil Row

A missing key yields null (dict-get → nil), one row.

<p class="example-label">SQL</p>

```sql
create table t (id int);

insert into t ({id: 1, v: "a"});

select t[99];
```

<p class="example-label">Result</p>

```json
[
  null
]
```

</div>

<div class="example">

### Indexing An

Indexing an empty table yields null.

<p class="example-label">SQL</p>

```sql
create table t (id int);

select t[1];
```

<p class="example-label">Result</p>

```json
[
  null
]
```

</div>

<div class="example">

### Get String Key

A string-keyed table indexes by a string literal.

<p class="example-label">SQL</p>

```sql
create table t (id int);

create table s (id string);

insert into s ({id: "x", v: 9});

select s["x"];
```

<p class="example-label">Result</p>

```json
[
  { "id": "x", "v": 9 }
]
```

</div>

<div class="example">

### Re-inserting A

Re-inserting a key overwrites; the lookup sees the latest value.

<p class="example-label">SQL</p>

```sql
create table t (id int);

insert into t ({id: 1, v: 100});

insert into t ({id: 1, v: 200});

select t[1];
```

<p class="example-label">Result</p>

```json
[
  { "id": 1, "v": 200 }
]
```

</div>

<div class="example">

### The Looked-up

The looked-up row is a value and can be indexed further.

<p class="example-label">SQL</p>

```sql
create table t (id int);

insert into t ({id: 1, v: "a"});

select t[1].v;
```

<p class="example-label">Result</p>

```json
[
  "a"
]
```

</div>

<div class="example">

### The Lookup

The lookup picks exactly one row out of many, ignoring the rest.

<p class="example-label">SQL</p>

```sql
create table t (id int);

insert into t ({id: 1, v: "a"}, {id: 2, v: "b"}, {id: 3, v: "c"});

select t[2];
```

<p class="example-label">Result</p>

```json
[
  { "id": 2, "v": "b" }
]
```

</div>

<div class="example">

### Get Wrong Type

A string key against an int-keyed table is a schema error.

<p class="example-label">SQL</p>

```sql
create table t (id int);

select t["a"];
```

Expected error: `schema`

</div>

<div class="example">

### Get Keyless

A keyless table cannot be indexed by key.

<p class="example-label">SQL</p>

```sql
create table t (id int);

create table k;

select k[1];
```

Expected error: `static`

</div>

<div class="example">

### Get Composite Arity

A key tuple longer than the key-column count is a static error.

<p class="example-label">SQL</p>

```sql
create table t (id int);

create table c (a int, b int);

select c[1, 2, 3];
```

Expected error: `static`

</div>

<div class="example">

### Matching Row

A full composite key returns the one matching row.

<p class="example-label">SQL</p>

```sql
create table t (id int);

create table c (a string, b int);

insert into c ({a: "x", b: 7, v: "hit"});

select c["x", 7];
```

<p class="example-label">Result</p>

```json
[
  { "a": "x", "b": 7, "v": "hit" }
]
```

</div>

<div class="example">

### Key Order

A leading-prefix key returns the matching rows as an array in key order.

<p class="example-label">SQL</p>

```sql
create table t (id int);

create table c (a string, b int);

insert into c ({a: "x", b: 2, v: "q"}, {a: "x", b: 1, v: "p"}, {a: "y", b: 9, v: "r"});

select c["x"];
```

<p class="example-label">Result</p>

```json
[
  [ { "a": "x", "b": 1, "v": "p" }, { "a": "x", "b": 2, "v": "q" } ]
]
```

</div>

<div class="example">

### Empty Array

A partial key matching no rows yields an empty array.

<p class="example-label">SQL</p>

```sql
create table t (id int);

create table c (a string, b int);

insert into c ({a: "x", b: 1, v: "p"});

select c["z"];
```

<p class="example-label">Result</p>

```json
[
  []
]
```

</div>

<div class="example">

### Matching Prefix

A two-column prefix of a three-column key scans only the matching prefix.

<p class="example-label">SQL</p>

```sql
create table t (id int);

create table k (a string, b int, c int);

insert into k ({a: "x", b: 7, c: 2, v: "n"}, {a: "x", b: 7, c: 1, v: "m"}, {a: "x", b: 8, c: 9, v: "o"});

select k["x", 7];
```

<p class="example-label">Result</p>

```json
[
  [ { "a": "x", "b": 7, "c": 1, "v": "m" }, { "a": "x", "b": 7, "c": 2, "v": "n" } ]
]
```

</div>

<div class="example">

### The Sub-sequence

The sub-sequence array composes — index into it.

<p class="example-label">SQL</p>

```sql
create table t (id int);

create table c (a string, b int);

insert into c ({a: "x", b: 1, v: "p"}, {a: "x", b: 2, v: "q"});

select c["x"][0];
```

<p class="example-label">Result</p>

```json
[
  { "a": "x", "b": 1, "v": "p" }
]
```

</div>

<div class="example">

### The Sub-sequence

The sub-sequence array composes — scan it as a value source.

<p class="example-label">SQL</p>

```sql
create table t (id int);

create table c (a string, b int);

insert into c ({a: "x", b: 1, v: "p"}, {a: "x", b: 2, v: "q"}, {a: "y", b: 9, v: "r"});

select r from c["x"] as r;
```

<p class="example-label">Result</p>

```json
[
  { "a": "x", "b": 1, "v": "p" },
  { "a": "x", "b": 2, "v": "q" }
]
```

</div>

<div class="example">

### Get Composite Miss

A full composite key with no matching row yields null.

<p class="example-label">SQL</p>

```sql
create table t (id int);

create table c (a string, b int);

insert into c ({a: "x", b: 7});

select c["x", 8];
```

<p class="example-label">Result</p>

```json
[
  null
]
```

</div>

<div class="example">

### Indexing A

Indexing a name that is neither a binding nor a table is unbound.

<p class="example-label">SQL</p>

```sql
create table t (id int);

select ghost[1];
```

Expected error: `static`

</div>