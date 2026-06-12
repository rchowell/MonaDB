+++
title = "Drop table"
description = "drop table — removes the table from the catalog and clears its data."
weight = 5
+++

# Drop table

drop table — removes the table from the catalog and clears its data.

<div class="example">

### After Drop,

After drop, the table can no longer be selected from.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1});

drop table T;

select * from T;
```

Expected error: `static`

</div>

<div class="example">

### Is Empty

A table re-created after drop is fresh and empty.

<p class="example-label">SQL</p>

```sql
create table T;

insert into T ({x: 1}, {x: 2});

drop table T;

create table T;

select * from T;
```

<p class="example-label">Result</p>

```json
[]
```

</div>

<div class="example">

### Dropping An

Dropping an undeclared table is a static error.

<p class="example-label">SQL</p>

```sql
drop table Ghost;
```

Expected error: `static`

</div>