+++
title = "Design"
description = "How MonaDB compiles query text to bytecode and executes it against LMDB."
+++

# Design

MonaDB is a compiler and a virtual machine. A query enters as text and exits as a result set. Between those two points it passes through six stages: lexer, parser, IR, compiler, VM, and storage. Each stage has a single job and hands off a well-typed value to the next.

## Lexer

The lexer is a [logos](https://github.com/maciejhirsz/logos) DFA over the raw query string. It produces a spanned token stream — each token carries the byte range of its source characters. Spans flow forward through every subsequent stage and surface in error messages, so a type error can point at the offending expression in the original text rather than a position in the bytecode.

The token set is small. Keywords (`select`, `from`, `where`, `insert`, `into`, `create`, `table`, `update`, `set`, `delete`, `drop`, `copy`, `as`, `and`, `or`, `not`, `null`, `true`, `false`, `limit`, `fetch`, `step`, `order`, `group`, `with`, `by`, `asc`, `desc`, `default`) are distinguished from identifiers at the lexer level. All other names are identifiers.

## Parser

The parser is a [LALRPOP](https://github.com/lalrpop/lalrpop) LR(1) grammar. Grammar rules stay thin — each action constructs an IR value by calling a function in `ir.rs`. No transformation or validation happens inside the grammar. The parser's only job is shape, not meaning.

Operator precedence and associativity are declared inline on the `Expr` production using LALRPOP's `#[precedence]` and `#[assoc]` annotations. This keeps the grammar readable while avoiding ambiguity.

## IR

The IR distinguishes two kinds of types.

**Enums** for sum types — when a value is one of several alternatives. `Statement`, `Expr`, `Type`, `Fetch`, `Constructor`, `Member`, `Source`, `Selector`, and `Segment` are all enums.

**Structs** for product types — when a value groups several named fields. `Select`, `Insert`, `Create`, `Update`, `Op`, `Jpk`, `Jpi`, `Jpe`, `Iter`, and `Table` are all structs.

Recursive types are always boxed through a type alias. `ExprRef = Box<Expr>` and `TypeRef = Box<Type>` appear throughout; `Box<Expr>` inline does not. This discipline keeps the IR consistent and refactoring tractable.

## Compiler

The compiler is a tree-walking pass over the IR that emits a flat sequence of `Vop` instructions. Dispatch methods carry the `cc_` prefix — `cc_select`, `cc_expr`, `cc_insert`. Append helpers carry `emit_` — `emit_push`, `emit_jpk`, `emit_rewind`.

Control-flow instructions — `Rewind`, `IfNot`, `Next`, `CntIfPos`, `CntIfZero` — are emitted with placeholder jump targets of `0`. After the loop body is fully emitted and its size is known, the compiler back-patches the target addresses via `patch(pc, dst)`. This avoids a two-pass approach while keeping the emitter linear.

Variables are tracked by index. `define(name)` appends a `Var` entry; a variable's index into `vars` is its address on the VM stack. `Load(idx)` and `Store(idx)` address by that index. `define_counter(n)` allocates a counter slot and emits a `CntSet` instruction.

## VM

The VM is a stack-based interpreter. It maintains a value stack and a counter array, then loops over the `Vop` program dispatching each instruction. The main loop is a `match` over `Vop` variants — one arm per opcode.

Variables live on the stack as absolute-indexed slots, not in a separate frame allocation. Counters drive loop bounds: `fetch 10` compiles to a `CntSet(10)` followed by `CntIfZero(→exit)` guards around the loop body.

Cursor operations follow the pattern `Open → Scan → [body] → Next → Halt`. The cursor is always valid inside the body; `Scan` positions it, `Next` advances it, and when the source is exhausted `Next` branches to the instruction after `Halt`.

## Storage

Execution reads and writes [LMDB](http://www.lmdb.tech/doc/), a memory-mapped B+tree store. There is no server process — the engine is a library that runs inside the host program. Each table is an LMDB named database. The database name is the table's OID encoded as a fixed-width big-endian hex string.

Keys are shredded from the record using order-preserving encoding. Values are stored verbatim — the format is schemaless. Deletions are deferred: during a scan, `(cursor, key)` pairs are buffered and applied at `Halt` after the read iterator is dropped, to avoid invalidating the cursor mid-scan.
