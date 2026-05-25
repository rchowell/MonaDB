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
addr	instruction	    comment
----	-----------	    -------
0     Init	          jumps to addr N (patched)
1	    [body]	        main body of program
...	  ...	            ...
M	    Halt	          commits the transaction, halts execution
N	    Transaction	    opens the transaction, falls through
N+1   Jump 1	        jumps to body start (addr 1)
```

During execution, we create the appropriate transaction handle and
hold it as VM state. This transaction handle is then used by all other
instructions in the program that require it, and we have appropriate
typing and assertions on the transaction mode.


## Cursors

Cursors are iterators over rows in table. Our tables are backed by
an LMDB B+ tree, and we can iterate forwards or backwards as well as
use byte-ordered key prefixes. We only need three cursor instructions.

```rs
Open { csr, tbl }
Scan { csr, jmp } // also ScanRev
Next { csr, jmp }
Load { csr }
```

The `Open` instruction will open the underlyign heed btree, but does
not create any state or position the cursor. It simply binds a cursor
slot to an open btree based on the tbl argument. The btree handle is
stable for the process lifetime and across transactions.

The `Scan` instruction creates the internal cursor state by initializing a
forward iterator and positioning it to the first value. If there is no value
then jump, otherwise fallthrough and the loop body begins with the cursor
properly positioned. The `Scan` instruction unconditionally pops a prefix value
from the stack before positioning the cursor. I think this is an elegant design
because we can support compiled prefixes (optimization), runtime prefixes (nested
loop joins), and full table scans with a uniform instruction pattern just by
using our existing 'push' instruction. Scans are also easily restarted with
different runtime prefixes (nested loop join) by simply pushing the new key
prefix and calling `Scan` again. The existing iterator is dropped, that new
key is popped, and the new iterator is created with proper positioning.

The `Next` instruction advances the scan and updates the iternal cursor state.
If there's a next row, then we jump back to the top of the loop with the cursor
properly positioned once again. Otherwise fallthrough because we are done looping.

The `Load` instruction is used to put the current value at the cursor onto
the stack, and it is read only. It does not change the cursor's state.

The `Scan->[body]->Next` structure upholds the invariant that, whenever we are
in a loop body, the cursor points to a valid row. All other instructions can
safely read the current row, but cannot modify the cursor position. Scans are easily
restarted and don't require any re-openning. Finally, you will see erased
lifetimes in the code. This is for self-reference and the lifetime invariants are
upheld where storage lives longer than a transaction, which lives longer than all
cursors. This keep-alive pattern is yolk-inspired.

## Limit

The `limit` clause slices the row stream by position using python-style
half-open ranges: `limit N` takes the first N, `limit N..` skips N, and
`limit N..M` is the half-open `[N, M)` skip N then take `M - N`. The bounds
are integer literals, so I resolve them at compile time; there is no runtime
limit expression like sqlite (yet?).

I slice with two count-down counters (skip and take) — held in a
dedicated `counters` array on the VM rather than on the value stack. This
mirrors how cursors get their own slots: counters are addressed by index and
live beside the stack, not on it. Three instructions manage them:

```rs
CntSet(cnt, val)     // counters[cnt] = val
CntIfPos(cnt, jmp)   // if counters[cnt] > 0 { decrement; jump }       -- skip
CntIfZero(cnt, jmp)  // if counters[cnt] == 0 { jump } else decrement  -- take
```

`CntSet` runs once before the loop. The two checks sit inside the loop body,
after the where filter and before we load and yield the row, so the slice
applies to the post-filter stream just as sqlite's offset counts only
qualifying rows. A spent skip jumps to `Next` (drop this row, advance); an
exhausted take jumps to `Halt` (stop the scan entirely).

```
addr	instruction	      comment
----	-----------	      -------
...	  Scan	            
      [where]	          residual filter, IfNot  -> Next
      CntIfPos(skip)	  skip > 0: drop row,     -> Next
      CntIfZero(take)	  take == 0: done,        -> Halt
      Load
      [select]
      Yield
      Next	            jmp -> top of loop body
      Halt
```

sqlite checks its limit *after* emitting a row with a decrement-then-test
opcode (`DecrJumpZero`). I check *before* emitting with a test-then-decrement
opcode, which is why the take check must precede the body — a post-yield check
with these semantics emits one row too many. Both shapes yield exactly the
requested count; they are duals. I also don't fold skip and take into a single
`limit + offset` counter the way sqlite's `OffsetLimit` does. That would be nice
for bounding a top-N sort, and I have no `order` yet.

## Display

This could be a whole blog post, but I basically just ported my work
from partiql kotlin to rust which is wadler-inspired.
https://github.com/partiql/partiql-lang-kotlin/tree/main/partiql-ast/src/main/java/org/partiql/ast/sql


## Compilation

From Postgres, "The reason for separating raw parsing from semantic analysis is that system catalog lookups can only be done within a transaction, and we do not wish to start a transaction immediately upon receiving a query string.".

This is exactly true for MonaDB too.

1. Parse
2. Bind
3. Emit


**Variable Binding**

Well, I also just realized that I will need to have a cursor backed
by an iterable/container value! 
