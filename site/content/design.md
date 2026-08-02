+++
title = "Design"
description = "How MonaDB is put together, and why."
template = "docs/page.html"
weight = 6
+++

MonaDB is about 1,500 lines of Rust and 230 lines of Python. The small size is
the design goal, not a side effect.

## Layering

```
  Python  monadb/
    Database      Mapping[str, Collection]
    Transaction   Mapping[str, Collection]
    Collection    MutableMapping[Key, Doc]
    models        dataclass / pydantic adapter
                        │  delegates every operation
  Rust    _monadb
    db · txn · collection · keys · doc · error
                        │
  redb 4.1              one table per collection
```

The seam sits at the type boundary. Rust owns storage, transactions, key
encoding, and BSON. Python owns the mapping protocol and the model adapter.

That split is deliberate on both sides. BSON encoding belongs in Rust because
doing it in Python would mean a pymongo dependency. Model normalization belongs
in Python because calling `model_dump` from Rust would be the most fragile code
in the project — it would break every time pydantic moved.

Registering `Collection` as a `MutableMapping` means the Rust layer implements
seven dunders and the ABC supplies `get`, `pop`, `popitem`, `setdefault`,
`update`, and `clear` for free.

## No catalog

A collection is a redb table, named directly. There is no catalog table, no
object-id indirection, and no schema record: redb's own `list_tables()` answers
"what collections exist", which is the only catalog question MonaDB asks.

## Key encoding

Keys use a tuple encoding in the style of FoundationDB's: a tag byte per
component, integers as sign-flipped big-endian, strings and bytes terminated
with `0x00` and escaped as `0x00 0xFF`.

```
  tag   payload
  0x01  8 bytes   int    i64 big-endian, sign bit flipped
  0x02  var       str    UTF-8, 0x00-terminated
  0x03  var       bytes  raw, same escaping
```

Two properties fall out. The tag byte makes the encoding self-describing, so a
stored key decodes back to the exact Python type written. And because redb
compares byte strings lexicographically, encoded order *is* iteration order —
which is why range and prefix scans need no machinery beyond a range call.

This is the load-bearing part of the system. It is defended by a property test
asserting `encode(a) < encode(b)` if and only if `a < b`, checked over every pair
in a corpus, against Rust's derived `Ord` as the model.

## Reads avoid a copy

Writes build an owned BSON `Document`. Reads take a shorter path: the stored
bytes are borrowed as a `RawDocument` and walked straight into a `PyDict`,
without ever materializing an owned `Document`.

## The write gate

redb allows one write transaction at a time, and its `begin_write()` blocks with
no timeout. MonaDB puts a gate in front of it: a mutex and condition variable
with a deadline, which supplies the timeout redb lacks. An in-process gate is
enough precisely because redb's exclusive file lock guarantees this process is
the only writer.

Two details matter more than they look:

**The wait releases the GIL.** A thread waiting while holding the GIL would stall
the very threads that could release the gate — the deadlock would be total.

**Re-entry is detected, not waited out.** A thread that already holds the gate
gets an immediate `TransactionError`, because waiting could never succeed.

## Transactions cannot outlive their block

A write transaction lives in shared state; commit and abort take it out and
release the gate. There is no way to hold one across an API boundary.

That is a structural fix for a real bug in MonaDB 0.1, where a lazily executed
statement could retain a write transaction after an error, and the next write
would hang forever. The class of bug is not patched here — it is unrepresentable.

## Where iteration streams, and where it cannot

Database-scoped iteration streams. redb's `ReadOnlyTable::range` returns a range
that owns its transaction guard, so the iterator keeps its own snapshot alive
with nothing borrowed.

Transaction-scoped iteration materializes instead. A table opened on a write
transaction borrows that transaction and cannot be handed across the FFI boundary
beside it without a self-reference — and the crate forbids `unsafe`. Read-your-writes
is preserved; the cost is memory proportional to the result.

## What was removed

MonaDB 0.1 was a different program: a SQL dialect compiled to bytecode and run on
a stack VM over LMDB, about 16,000 lines of Rust with 15 dependencies. Version
0.2 deletes the lexer, parser, binder, compiler, VM, and function library, and
replaces LMDB with redb.

What remains is three dependencies — redb, bson, pyo3 — and a crate that compiles
with `#![forbid(unsafe_code)]`.

The 0.1 engine is preserved under the `v0.1.0-sql` git tag.
