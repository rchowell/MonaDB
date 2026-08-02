+++
title = "Transaction"
description = "Begin opens an explicit transaction that spans statements until commit makes its changes durable or rollback discards them."
weight = 13
+++

Begin opens an explicit transaction that spans statements until commit makes its changes durable or rollback discards them. Outside an explicit transaction every statement runs in its own implicit transaction, committed when the statement finishes.

## Syntax

### Railroad

<div class="rr">
<div class="rr-track"><span class="rr-or"><span class="rr-branch"><span class="rr-t">begin</span></span><span class="rr-branch"><span class="rr-t">commit</span></span><span class="rr-branch"><span class="rr-t">rollback</span></span></span><span class="rr-join" aria-hidden="true"></span><span class="rr-t">;</span></div>
</div>

### BNF

```ebnf
transaction-stmt ::= ( "begin" | "commit" | "rollback" ) ";"
```

## Rules

1. Transaction control is a complete statement on its own; combining it with another statement in one submission, as in `commit; insert ...`, is an error rather than a partial run. *(phase: parse)*
2. There is one flavor of begin. It opens a write transaction immediately; there is no deferred, read-only, or exclusive variant, and transactions do not nest. *(phase: execute)*
3. Statements inside a transaction read their own uncommitted writes. *(phase: execute)*
4. A table created inside a transaction is visible to later statements in it. Rollback discards the table without invalidating statements prepared before it. *(phase: catalog)*
5. Begin while a transaction is active, or commit or rollback with none active, is a transaction error. *(phase: execute)*
6. A statement that fails part-way inside a transaction does not undo the rows it already wrote; the transaction stays open and a later commit persists them. Only rollback discards them. *(phase: execute)*

## Examples

### Minimal

<div class="example">

#### Own Writes

A select in a session sees the session's uncommitted insert.

<p class="example-label">SQL</p>

```sql
create table t;

begin;
```

<p class="example-label">Result</p>

```json
[]
```

<p class="example-label">SQL</p>

```sql
create table t;

begin;

insert into t ({"x": 1});

select * from t;
```

<p class="example-label">Result</p>

```json
[
  { "x": 1 }
]
```

<p class="example-label">SQL</p>

```sql
create table t;

begin;

insert into t ({"x": 1});

select * from t;

commit;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

### Compound

<div class="example">

#### DDL Created

DDL created mid-session is visible to a later statement in it.

<p class="example-label">SQL</p>

```sql
begin;

create table u;

insert into u ({"a": 7});

select * from u;
```

<p class="example-label">Result</p>

```json
[
  { "a": 7 }
]
```

</div>

<div class="example">

#### Commit Persists

A committed write is visible after the transaction closes.

<p class="example-label">SQL</p>

```sql
create table t;

begin;

insert into t ({"x": 42});

commit;

select * from t;
```

<p class="example-label">Result</p>

```json
[
  { "x": 42 }
]
```

</div>

<div class="example">

#### Rollback Discards Writes

A rolled-back insert leaves the table empty.

<p class="example-label">SQL</p>

```sql
create table t;

begin;

insert into t ({"x": 1});

rollback;

select * from t;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

### Error cases

<div class="example">

#### Rollback Discards Ddl

A table created then rolled back is unbound afterward.

<p class="example-label">SQL</p>

```sql
begin;

create table u;

rollback;

select * from u;
```

Expected error: `static`

</div>

<div class="example">

#### Opening A

Opening a transaction while one is active is an error.

<p class="example-label">SQL</p>

```sql
begin;

begin;
```

Expected error: `transaction`

</div>

<div class="example">

#### Committing With

Committing with no active transaction is an error.

<p class="example-label">SQL</p>

```sql
commit;
```

Expected error: `transaction`

</div>

## See also

- [Insert](@/language/statements/insert.md)
- [Delete](@/language/statements/delete.md)
- [Create Table](@/language/statements/create-table.md)
