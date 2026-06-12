+++
title = "Clear table"
description = "clear table — empties the table's data but keeps it registered in the catalog."
weight = 3
+++

# Clear table

clear table — empties the table's data but keeps it registered in the catalog.

<div class="example">

### Table Place

Clear removes every row but leaves the table in place.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2}, {x: 3});

clear table T;

select * from T;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

### New Rows

A cleared table is still resolvable and accepts new rows.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1});

clear table T;

insert into T ({x: 9});

select * from T;
```

<p class="example-label">Result</p>

```json
[
  { "x": 9 }
]
```

</div>

<div class="example">

### Clearing An

Clearing an undeclared table is a static error.

<p class="example-label">SQL</p>

```sql
clear table Ghost;
```

Expected error: `static`

</div>