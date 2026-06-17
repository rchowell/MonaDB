+++
title = "Drop Table"
description = "Drop Table removes a table and all of its rows from the catalog."
weight = 11
+++

Drop Table removes a table and all of its rows from the catalog. The table name must exist; dropping a missing table is a static error.

## Syntax

### Railroad

<div class="rr">
<div class="rr-track"><span class="rr-t">drop</span><span class="rr-join" aria-hidden="true"></span><span class="rr-t">table</span><span class="rr-join" aria-hidden="true"></span><span class="rr-n">name</span><span class="rr-join" aria-hidden="true"></span><span class="rr-t">;</span></div>
</div>

### BNF

```ebnf
drop-table-stmt ::= "drop" "table" identifier ";"
```

## Rules

1. Dropping a non-existent table is a static error. *(phase: catalog)*
2. All rows and the table definition are removed. *(phase: catalog)*

## Examples

### Minimal

<div class="example">

#### Is Empty

A table re-created after drop is fresh and empty.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1}, {"x": 2});

drop table T;

create table T;

select * from T;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

### Error cases

<div class="example">

#### After Drop,

After drop, the table can no longer be selected from.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1});

drop table T;

select * from T;
```

Expected error: `static`

</div>

<div class="example">

#### Dropping An

Dropping an undeclared table is a static error.

<p class="example-label">SQL</p>

```sql
drop table Ghost;
```

Expected error: `static`

</div>

## See also

- [Create Table](@/language/statements/create-table.md)
- [Clear](@/language/statements/clear.md) — empty a table but keep its definition
