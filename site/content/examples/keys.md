+++
title = "Keys & lookups"
description = "Key columns, ordering, and keyed get examples."
weight = 3
+++

# Keys & lookups

Declaring key columns and fetching rows by key.

## Keys

<div class="example">

### Whole Objects

A keyless table stores and returns whole objects.

<p class="example-label">SQL</p>

```sql
create table t;

insert into t ({x: 1, y: 2, z: 3});

select * from t;
```

<p class="example-label">Result</p>

```json
[
  { "x": 1, "y": 2, "z": 3 }
]
```

</div>

<div class="example">

### Ones X

A keyless table accepts any object, including ones with no x.

<p class="example-label">SQL</p>

```sql
create table t;

insert into t ({y: 2, z: 3});

select * from t;
```

<p class="example-label">Result</p>

```json
[
  { "y": 2, "z": 3 }
]
```

</div>

<div class="example">

### Surrogate Ids

Surrogate ids increment, so rows come back in insertion order.

<p class="example-label">SQL</p>

```sql
create table t;

insert into t ({x: 3}, {x: 1}, {x: 2});

select * from t;
```

<p class="example-label">Result</p>

```json
[
  { "x": 3 },
  { "x": 1 },
  { "x": 2 }
]
```

</div>

<div class="example">

### Int Key

Int key with payload round-trips whole object.

<p class="example-label">SQL</p>

```sql
create table t (x int);

insert into t ({x: 1, z: 9});

select * from t;
```

<p class="example-label">Result</p>

```json
[
  { "x": 1, "z": 9 }
]
```

</div>

<div class="example">

### Rows Inserted

Rows inserted out of order come back sorted by the int key.

<p class="example-label">SQL</p>

```sql
create table t (x int);

insert into t ({x: 3}, {x: 1}, {x: 2});

select * from t;
```

<p class="example-label">Result</p>

```json
[
  { "x": 1 },
  { "x": 2 },
  { "x": 3 }
]
```

</div>

<div class="example">

### Negative Ints

Negative ints sort before zero and positives (sign-flip encoding).

<p class="example-label">SQL</p>

```sql
create table t (x int);

insert into t ({x: 1}, {x: -5}, {x: 0}, {x: -1});

select * from t;
```

<p class="example-label">Result</p>

```json
[
  { "x": -5 },
  { "x": -1 },
  { "x": 0 },
  { "x": 1 }
]
```

</div>

<div class="example">

### Re-inserting The

Re-inserting the same key overwrites (last write wins).

<p class="example-label">SQL</p>

```sql
create table t (x int);

insert into t ({x: 1, v: 100});

insert into t ({x: 1, v: 200});

select * from t;
```

<p class="example-label">Result</p>

```json
[
  { "x": 1, "v": 200 }
]
```

</div>

<div class="example">

### Inserting Without

Inserting without the key field is a schema error.

<p class="example-label">SQL</p>

```sql
create table t (x int);

insert into t ({z: 9});
```

Expected error: `schema`

</div>

<div class="example">

### Wrong Type

A string where an int key is declared is a schema error.

<p class="example-label">SQL</p>

```sql
create table t (x int);

insert into t ({x: "a"});
```

Expected error: `schema`

</div>

<div class="example">

### Non Integral

A non-integral number for an int key is a schema error.

<p class="example-label">SQL</p>

```sql
create table t (x int);

insert into t ({x: 1.5});
```

Expected error: `schema`

</div>

<div class="example">

### String Key

String key with payload round-trips whole object.

<p class="example-label">SQL</p>

```sql
create table t (x string);

insert into t ({x: "a", z: 9});

select * from t;
```

<p class="example-label">Result</p>

```json
[
  { "x": "a", "z": 9 }
]
```

</div>

<div class="example">

### Rows Come

Rows come back in lexicographic key order.

<p class="example-label">SQL</p>

```sql
create table t (x string);

insert into t ({x: "c"}, {x: "a"}, {x: "b"});

select * from t;
```

<p class="example-label">Result</p>

```json
[
  { "x": "a" },
  { "x": "b" },
  { "x": "c" }
]
```

</div>

<div class="example">

### Inserting Without

Inserting without the key field is a schema error.

<p class="example-label">SQL</p>

```sql
create table t (x string);

insert into t ({z: 9});
```

Expected error: `schema`

</div>

<div class="example">

### Wrong Type

A number where a string key is declared is a schema error.

<p class="example-label">SQL</p>

```sql
create table t (x string);

insert into t ({x: 1});
```

Expected error: `schema`

</div>

<div class="example">

### Composite (int,

Composite (int, string) key round-trips whole object.

<p class="example-label">SQL</p>

```sql
create table t (a int, b string);

insert into t ({a: 1, b: "x", z: 9});

select * from t;
```

<p class="example-label">Result</p>

```json
[
  { "a": 1, "b": "x", "z": 9 }
]
```

</div>

<div class="example">

### Sort By

Sort by first component, tie-break on the second.

<p class="example-label">SQL</p>

```sql
create table t (a int, b string);

insert into t ({a: 2, b: "a"}, {a: 1, b: "y"}, {a: 1, b: "x"});

select * from t;
```

<p class="example-label">Result</p>

```json
[
  { "a": 1, "b": "x" },
  { "a": 1, "b": "y" },
  { "a": 2, "b": "a" }
]
```

</div>

<div class="example">

### Missing The

Missing the first key field is a schema error.

<p class="example-label">SQL</p>

```sql
create table t (a int, b string);

insert into t ({b: "x"});
```

Expected error: `schema`

</div>

<div class="example">

### Missing The

Missing the second key field is a schema error.

<p class="example-label">SQL</p>

```sql
create table t (a int, b string);

insert into t ({a: 1});
```

Expected error: `schema`

</div>

<div class="example">

### Wrong Type

Wrong type for the first key field is a schema error.

<p class="example-label">SQL</p>

```sql
create table t (a int, b string);

insert into t ({a: "q", b: "x"});
```

Expected error: `schema`

</div>

<div class="example">

### Wrong Type

Wrong type for the second key field is a schema error.

<p class="example-label">SQL</p>

```sql
create table t (a int, b string);

insert into t ({a: 1, b: 2});
```

Expected error: `schema`

</div>

<div class="example">

### Composite (string,

Composite (string, int) key round-trips whole object.

<p class="example-label">SQL</p>

```sql
create table t (a string, b int);

insert into t ({a: "x", b: 1, z: 9});

select * from t;
```

<p class="example-label">Result</p>

```json
[
  { "a": "x", "b": 1, "z": 9 }
]
```

</div>

<div class="example">

### Sort By

Sort by string first, tie-break on the int.

<p class="example-label">SQL</p>

```sql
create table t (a string, b int);

insert into t ({a: "b", b: 1}, {a: "a", b: 2}, {a: "a", b: 1});

select * from t;
```

<p class="example-label">Result</p>

```json
[
  { "a": "a", "b": 1 },
  { "a": "a", "b": 2 },
  { "a": "b", "b": 1 }
]
```

</div>

<div class="example">

### Missing The

Missing the int component is a schema error.

<p class="example-label">SQL</p>

```sql
create table t (a string, b int);

insert into t ({a: "x"});
```

Expected error: `schema`

</div>

<div class="example">

### Type Second

A string where the int component is declared is a schema error.

<p class="example-label">SQL</p>

```sql
create table t (a string, b int);

insert into t ({a: "x", b: "y"});
```

Expected error: `schema`

</div>

<div class="example">

### Composite (int,

Composite (int, int) key round-trips whole object.

<p class="example-label">SQL</p>

```sql
create table t (a int, b int);

insert into t ({a: 1, b: 2, z: 9});

select * from t;
```

<p class="example-label">Result</p>

```json
[
  { "a": 1, "b": 2, "z": 9 }
]
```

</div>

<div class="example">

### Sort By

Sort by first int, tie-break on the second int.

<p class="example-label">SQL</p>

```sql
create table t (a int, b int);

insert into t ({a: 2, b: 1}, {a: 1, b: 2}, {a: 1, b: 1});

select * from t;
```

<p class="example-label">Result</p>

```json
[
  { "a": 1, "b": 1 },
  { "a": 1, "b": 2 },
  { "a": 2, "b": 1 }
]
```

</div>

<div class="example">

### Missing A

Missing a key component is a schema error.

<p class="example-label">SQL</p>

```sql
create table t (a int, b int);

insert into t ({a: 1});
```

Expected error: `schema`

</div>

<div class="example">

### Composite (string,

Composite (string, string) key round-trips whole object.

<p class="example-label">SQL</p>

```sql
create table t (a string, b string);

insert into t ({a: "x", b: "y", z: 9});

select * from t;
```

<p class="example-label">Result</p>

```json
[
  { "a": "x", "b": "y", "z": 9 }
]
```

</div>

<div class="example">

### Sort By

Sort by first string, tie-break on the second.

<p class="example-label">SQL</p>

```sql
create table t (a string, b string);

insert into t ({a: "b", b: "a"}, {a: "a", b: "b"}, {a: "a", b: "a"});

select * from t;
```

<p class="example-label">Result</p>

```json
[
  { "a": "a", "b": "a" },
  { "a": "a", "b": "b" },
  { "a": "b", "b": "a" }
]
```

</div>

<div class="example">

### Before Ab

A shorter first component sorts before a longer one that shares its prefix, regardless of the second component — proves the string terminator. ("a","z") must sort before ("ab","a").

<p class="example-label">SQL</p>

```sql
create table t (a string, b string);

insert into t ({a: "ab", b: "a"}, {a: "a", b: "z"});

select * from t;
```

<p class="example-label">Result</p>

```json
[
  { "a": "a", "b": "z" },
  { "a": "ab", "b": "a" }
]
```

</div>

<div class="example">

### Missing A

Missing a key component is a schema error.

<p class="example-label">SQL</p>

```sql
create table t (a string, b string);

insert into t ({a: "x"});
```

Expected error: `schema`

</div>

<div class="example">

### Float Key

A float key column is rejected at create.

<p class="example-label">SQL</p>

```sql
create table t (x float);
```

Expected error: `static`

</div>

<div class="example">

### Bool Key

A bool key column is rejected at create.

<p class="example-label">SQL</p>

```sql
create table t (x bool);
```

Expected error: `static`

</div>

## Keyed lookup

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
