+++
title = "Create Table"
description = "Create Table declares a table in the catalog."
weight = 10
+++

Create Table declares a table in the catalog. An optional schema lists key columns (in declaration order) that form the composite physical key. Key columns must be `int` or `string`. Keyless tables accept any object and preserve insertion order via surrogate ids.

## Syntax

### Railroad

<div class="rr">
<div class="rr-track"><span class="rr-t">create</span><span class="rr-join" aria-hidden="true"></span><span class="rr-t">table</span><span class="rr-join" aria-hidden="true"></span><span class="rr-n">name</span><span class="rr-join" aria-hidden="true"></span><span class="rr-opt"><span class="rr-opt-inner"><span class="rr-t">(</span><span class="rr-join" aria-hidden="true"></span><span class="rr-or"><span class="rr-branch"><span class="rr-t">)</span></span><span class="rr-branch"><span class="rr-n">key-column</span><span class="rr-join" aria-hidden="true"></span><span class="rr-rep"><span class="rr-rep-inner"><span class="rr-t">,</span><span class="rr-join" aria-hidden="true"></span><span class="rr-n">key-column</span></span></span><span class="rr-join" aria-hidden="true"></span><span class="rr-t">)</span></span></span></span></span><span class="rr-join" aria-hidden="true"></span><span class="rr-t">;</span></div>
</div>

### BNF

```ebnf
create-table-stmt ::= "create" "table" identifier [ "(" [ key-column ( "," key-column )* ] ")" ] ";"

key-column ::= identifier ( "int" | "string" )
```

## Rules

1. Without a schema, the table accepts any JSON object. *(phase: catalog)*
2. Declared columns form the composite key; key columns must be `int` or `string`. *(phase: catalog)*
3. Fields are non-null by default; declare `T | null` to permit null (general unions are not supported). *(phase: catalog)*
4. Creating a table that already exists is a static error (`IF NOT EXISTS` is not supported). *(phase: catalog)*
5. Inserts that violate the declared schema (missing keys, wrong types, extra keys on closed schemas) error at runtime. *(phase: execute on insert)*

## Examples

### Minimal

<div class="example">

#### Whole Objects

A keyless table stores and returns whole objects.

<p class="example-label">SQL</p>

```sql
create table t;

insert into t ({"x": 1, "y": 2, "z": 3});

select * from t;
```

<p class="example-label">Result</p>

```json
[
  { "x": 1, "y": 2, "z": 3 }
]
```

</div>

### Compound

<div class="example">

#### Ones X

A keyless table accepts any object, including ones with no x.

<p class="example-label">SQL</p>

```sql
create table t;

insert into t ({"y": 2, "z": 3});

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

#### Surrogate Ids

Surrogate ids increment, so rows come back in insertion order.

<p class="example-label">SQL</p>

```sql
create table t;

insert into t ({"x": 3}, {"x": 1}, {"x": 2});

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

#### Int Key

Int key with payload round-trips whole object.

<p class="example-label">SQL</p>

```sql
create table t (x int);

insert into t ({"x": 1, "z": 9});

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

#### Rows Inserted

Rows inserted out of order come back sorted by the int key.

<p class="example-label">SQL</p>

```sql
create table t (x int);

insert into t ({"x": 3}, {"x": 1}, {"x": 2});

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

#### Negative Ints

Negative ints sort before zero and positives (sign-flip encoding).

<p class="example-label">SQL</p>

```sql
create table t (x int);

insert into t ({"x": 1}, {"x": -5}, {"x": 0}, {"x": -1});

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

#### Re-inserting The

Re-inserting the same key overwrites (last write wins).

<p class="example-label">SQL</p>

```sql
create table t (x int);

insert into t ({"x": 1, "v": 100});

insert into t ({"x": 1, "v": 200});

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

#### Int Key

A float subscript coerces like cast(<float> as int) to find the int key.

<p class="example-label">SQL</p>

```sql
create table t (x int);

insert into t ({"x": 1, "z": 9});

select t[1.5];
```

<p class="example-label">Result</p>

```json
[
  { "x": 1, "z": 9 }
]
```

</div>

<div class="example">

#### String Key

String key with payload round-trips whole object.

<p class="example-label">SQL</p>

```sql
create table t (x string);

insert into t ({"x": "a", "z": 9});

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

#### Rows Come

Rows come back in lexicographic key order.

<p class="example-label">SQL</p>

```sql
create table t (x string);

insert into t ({"x": "c"}, {"x": "a"}, {"x": "b"});

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

#### Composite (int,

Composite (int, string) key round-trips whole object.

<p class="example-label">SQL</p>

```sql
create table t (a int, b string);

insert into t ({"a": 1, "b": "x", "z": 9});

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

#### Sort By

Sort by first component, tie-break on the second.

<p class="example-label">SQL</p>

```sql
create table t (a int, b string);

insert into t ({"a": 2, "b": "a"}, {"a": 1, "b": "y"}, {"a": 1, "b": "x"});

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

#### Composite (string,

Composite (string, int) key round-trips whole object.

<p class="example-label">SQL</p>

```sql
create table t (a string, b int);

insert into t ({"a": "x", "b": 1, "z": 9});

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

#### Sort By

Sort by string first, tie-break on the int.

<p class="example-label">SQL</p>

```sql
create table t (a string, b int);

insert into t ({"a": "b", "b": 1}, {"a": "a", "b": 2}, {"a": "a", "b": 1});

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

#### Composite (int,

Composite (int, int) key round-trips whole object.

<p class="example-label">SQL</p>

```sql
create table t (a int, b int);

insert into t ({"a": 1, "b": 2, "z": 9});

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

#### Sort By

Sort by first int, tie-break on the second int.

<p class="example-label">SQL</p>

```sql
create table t (a int, b int);

insert into t ({"a": 2, "b": 1}, {"a": 1, "b": 2}, {"a": 1, "b": 1});

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

#### Composite (string,

Composite (string, string) key round-trips whole object.

<p class="example-label">SQL</p>

```sql
create table t (a string, b string);

insert into t ({"a": "x", "b": "y", "z": 9});

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

#### Sort By

Sort by first string, tie-break on the second.

<p class="example-label">SQL</p>

```sql
create table t (a string, b string);

insert into t ({"a": "b", "b": "a"}, {"a": "a", "b": "b"}, {"a": "a", "b": "a"});

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

#### Before Ab

A shorter first component sorts before a longer one that shares its prefix, regardless of the second component — proves the string terminator. ("a","z") must sort before ("ab","a").

<p class="example-label">SQL</p>

```sql
create table t (a string, b string);

insert into t ({"a": "ab", "b": "a"}, {"a": "a", "b": "z"});

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

### Error cases

<div class="example">

#### Inserting Without

Inserting without the key field is a schema error.

<p class="example-label">SQL</p>

```sql
create table t (x int);

insert into t ({"z": 9});
```

Expected error: `schema`

</div>

<div class="example">

#### Wrong Type

A string where an int key is declared is a schema error.

<p class="example-label">SQL</p>

```sql
create table t (x int);

insert into t ({"x": "a"});
```

Expected error: `schema`

</div>

<div class="example">

#### Inserting Without

Inserting without the key field is a schema error.

<p class="example-label">SQL</p>

```sql
create table t (x string);

insert into t ({"z": 9});
```

Expected error: `schema`

</div>

<div class="example">

#### Wrong Type

A number where a string key is declared is a schema error.

<p class="example-label">SQL</p>

```sql
create table t (x string);

insert into t ({"x": 1});
```

Expected error: `schema`

</div>

<div class="example">

#### Missing The

Missing the first key field is a schema error.

<p class="example-label">SQL</p>

```sql
create table t (a int, b string);

insert into t ({"b": "x"});
```

Expected error: `schema`

</div>

<div class="example">

#### Missing The

Missing the second key field is a schema error.

<p class="example-label">SQL</p>

```sql
create table t (a int, b string);

insert into t ({"a": 1});
```

Expected error: `schema`

</div>

<div class="example">

#### Wrong Type

Wrong type for the first key field is a schema error.

<p class="example-label">SQL</p>

```sql
create table t (a int, b string);

insert into t ({"a": "q", "b": "x"});
```

Expected error: `schema`

</div>

<div class="example">

#### Wrong Type

Wrong type for the second key field is a schema error.

<p class="example-label">SQL</p>

```sql
create table t (a int, b string);

insert into t ({"a": 1, "b": 2});
```

Expected error: `schema`

</div>

<div class="example">

#### Missing The

Missing the int component is a schema error.

<p class="example-label">SQL</p>

```sql
create table t (a string, b int);

insert into t ({"a": "x"});
```

Expected error: `schema`

</div>

<div class="example">

#### Type Second

A string where the int component is declared is a schema error.

<p class="example-label">SQL</p>

```sql
create table t (a string, b int);

insert into t ({"a": "x", "b": "y"});
```

Expected error: `schema`

</div>

<div class="example">

#### Missing A

Missing a key component is a schema error.

<p class="example-label">SQL</p>

```sql
create table t (a int, b int);

insert into t ({"a": 1});
```

Expected error: `schema`

</div>

<div class="example">

#### Missing A

Missing a key component is a schema error.

<p class="example-label">SQL</p>

```sql
create table t (a string, b string);

insert into t ({"a": "x"});
```

Expected error: `schema`

</div>

<div class="example">

#### Float Key

A float key column is rejected at create.

<p class="example-label">SQL</p>

```sql
create table t (x float);
```

Expected error: `static`

</div>

<div class="example">

#### Bool Key

A bool key column is rejected at create.

<p class="example-label">SQL</p>

```sql
create table t (x bool);
```

Expected error: `static`

</div>

## See also

- [Insert](@/language/statements/insert.md)
- [Primary Keys](@/examples/keys.md) — keyed lookup examples
- [Schemas](@/language/schemas.md)
