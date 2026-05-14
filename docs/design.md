# MonaDB Design

I will try to capture design decisions here for future reference.

## DDL

All DDL is stored in the single system table called 'catalog' which
is like sqlite's 'sqlite_master' table. All objects in the catalog
have the following schema and are key'd on a derived oid (u32).

```json
{
  "type": string,   // "table"
  "name": string,   // "points"
  "sql": string     // "create table points (x int);"
}
```

The oid key is u32 big-endian and the corresponding LMDB database
name is a fixed-width hex string of the big-endian bytes. This is
elegant for several reasons based upon postgres and sqlite learnings.

The oid is not stored in the value, so we can avoid awkward u32 with
json conversions. The catalog entry value is also known statically so
we don't need object (record) building. This is different than sqlite
which requires record building because sqlite must add the btree location
whereas our btree location (LMDB database name) is derived from the oid.

Finally, a catalog entry name is not coupled to anything in the storage
layer (like postgres) so renames are just a single column update with
no modifications to storage.

## Inserts

MonaDB is schemaless, but its create table statement allows you to specify a
composite primary key. Upon insertion, I shred the key values from the record
and create an order-preserving encoding e.g. big-endian integers so that we can
efficiently do range scans with LMDB. The value itself is inserted as-is into
the BTree, with no assertions on what it contains. This means the only schema
enforcement is that any declared key MUST be present and a non-null value.

```sql
-- (A) no key
create table points;

-- (B) single key (x)
create table points (x int);

-- (C) composite key (x, y)
create table points (x int, y int);

-- valid for ALL tables
insert into points ({ "x": 1, "y": 2, "z": 3 });

-- invalid for (B) and (C) because missing x.
insert into points ({ "y": 2, "z": 3 });
```

## Transactions

MonaDB uses LMDB transactions which use MVCC for multiple readers
and a single writer. We create a transaction handles whenever
we interact with cursors for reading or writing tables, and the
transaction is committed when the VM halts. The transaction bytecode
is based on sqlite's bytecode which supports once initialization
and statement validation. I do not yet support these; however, I am
happy to learn from sqlite on how to architect for these abilities.

To compile a statement, we first emit an init instruction with
a placeholder jump address. If any operation requires a read or write
transaction, we set the transaction mode accordingly. From this mode,
we create a transaction instruction then patch the init jump.

```
addr 0:      Init           -> jumps to addr N (patched)
addr 1:      [body]
...
addr M:      Halt           -> commits the transaction, halts execution
addr N:      Transaction    -> opens the transaction, falls through
addr N+1:    Jump 1         -> jumps to body start
```

During execution, we create the appropriate transaction handle and
hold it as VM state. This transaction handle is then used by all other
instructions in the program that require it, and we have appropriate
typing and assertions on the transaction mode.


## Display

This could be a whole blog post, but I basically just ported my work
from partiql kotlin to rust which is wadler-inspired.
https://github.com/partiql/partiql-lang-kotlin/tree/main/partiql-ast/src/main/java/org/partiql/ast/sql
