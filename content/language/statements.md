+++
title = "Statements"
description = "SELECT, INSERT, UPDATE, CREATE TABLE, DROP TABLE, and COPY."
weight = 2
+++

# Statements

A MonaDB program is a sequence of statements separated by semicolons. Each statement performs one operation: query, insert, update, create, drop, or export.

## select

Maps the current binding stream and applies a constructor — an object literal, a list of named expressions, or `*` to spread all bound variables.

```
select <constructor>
  [from <source>]
  [where <expr>]
  [group by <expr>]
  [order by <expr> [asc|desc]]
  [fetch <range>];
```

```
select 1 + 1;

select { x: p.x, y: p.y }
  from points as p;

select p.x as x, p.y as y
  from points as p
 where p.x > 0;

select * from t;          -- equivalent to { ...t }
select * from t, s;       -- { ...t, ...s }
```

## insert

`insert into <table>` followed by a parenthesised, comma-separated values list. A trailing comma is permitted.

```
insert into points ({ x: 1, y: 2 });

insert into points (
    { x: 1, y: 2 },
    { x: 3, y: 4 },
);

insert into numbers (1, 2, 3);
insert into tuples ([1, 2], [3, 4]);
```

## update

`update <table> set <col> = <expr>, ...` with an optional `where` clause. Only rows matching the predicate are updated. Column values may be an expression, `DEFAULT`, or `NULL`.

```
update points set x = 10 where x = 0;
update points set x = x + 1, y = x + 1 where y = 0;
```

## create table

A table is a collection with an optional type constraint and optional index declarations. Members are `NOT NULL` by default; append `|null` to permit null.

```
create table points;          -- no schema
create table points ();       -- equivalent

create table points ({
    x: number,
    y: number,
    z: number|null,           -- nullable
    ...                       -- open content
}, {
    hash: x,                  -- partition key
    sort: y,                  -- range key
});
```

The second block declares index keys. Both `hash` and `sort` are optional.

## drop table

Removes a table and all its contents.

```
drop table points;
```

## copy

Moves data between a table or query and a file. Format is inferred from the extension or set via the `format:` option.

```
copy items to 'items.jsonl';
copy items to 'items.csv';
copy items to 'items.tsv' { header: false };
```

Supported formats: `jsonl`, `csv`, `tsv`.
