# MonaDB Design

I will try to capture design decisions here for future reference.

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
