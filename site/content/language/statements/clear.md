+++
title = "Clear"
description = "Clear removes every row from a table but keeps the table definition in the catalog."
weight = 12
+++

Clear removes every row from a table but keeps the table definition in the catalog. It is equivalent to `delete from` without a where clause, but expressed as a dedicated statement.

## Syntax

### Railroad

<div class="rr">
<div class="rr-track"><span class="rr-t">clear</span><span class="rr-join" aria-hidden="true"></span><span class="rr-t">table</span><span class="rr-join" aria-hidden="true"></span><span class="rr-n">name</span><span class="rr-join" aria-hidden="true"></span><span class="rr-t">;</span></div>
</div>

### BNF

```ebnf
clear-stmt ::= "clear" "table" identifier ";"
```

## Rules

1. Clears all rows; the table schema and catalog entry remain. *(phase: execute)*
2. Clearing a non-existent table is a static error. *(phase: catalog)*

## Examples

### Minimal

<div class="example">

#### Table Place

Clear removes every row but leaves the table in place.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1}, {"x": 2}, {"x": 3});

clear table T;

select * from T;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

### Compound

<div class="example">

#### New Rows

A cleared table is still resolvable and accepts new rows.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({"x": 1});

clear table T;

insert into T ({"x": 9});

select * from T;
```

<p class="example-label">Result</p>

```json
[
  { "x": 9 }
]
```

</div>

### Error cases

<div class="example">

#### Clearing An

Clearing an undeclared table is a static error.

<p class="example-label">SQL</p>

```sql
clear table Ghost;
```

Expected error: `static`

</div>

## See also

- [Delete](@/language/statements/delete.md)
- [Drop Table](@/language/statements/drop-table.md)
