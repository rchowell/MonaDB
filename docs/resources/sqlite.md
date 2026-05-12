# SQLite VDBE Bytecode Reference

> See [sqlite.org/opcode.html](https://www.sqlite.org/opcode.html)

## Discussion

Some of my findings and discussions.

- Any statement that opens a cursor gets a Transaction opcode.

## Overview

SQLite compiles every SQL statement into bytecode for the **Virtual DataBase Engine (VDBE)**. `sqlite3_prepare_v2()` is the compiler; `sqlite3_step()` runs the bytecode. Use `EXPLAIN <statement>` to inspect the emitted program.

## Instruction Format

Each instruction has an opcode and five operands: **P1, P2, P3** (32-bit signed ints), **P4** (int, float, string, blob, pointer, etc.), **P5** (16-bit flags).


| Operand | Typical role                                       |
|---------|----------------------------------------------------|
| P1      | Cursor number (for cursor ops); source register    |
| P2      | Jump destination (for branch ops); second register |
| P3      | Destination register; third operand                |
| P4      | Constant value, KeyInfo, FuncDef, collation, etc.  |
| P5      | Flags (e.g. `SQLITE_NULLEQ`, `OPFLAG_NCHANGE`)     |


Execution starts at instruction 0 and halts at `Halt`, program end, or error.

---

## Registers

Fixed-count per prepared statement. Each register holds: NULL, int64, float64, string, blob, RowSet, or Frame. "Undefined" (no value) is distinct from NULL. Cleared on `reset`/`finalize`.

---

## Cursors

Identified by small integers (usually P1). Multiple cursors can point to the same table/index independently. Created by `OpenRead`/`OpenWrite`; auto-closed on reset/finalize. All DB I/O goes through cursors.

---

## Control Flow

Subroutines store return addresses in registers (not a stack) — not reentrant. Triggers use separate `Program` subprograms with fresh register sets. Coroutines implemented via `Yield`.

---

## Opcode Reference

### Control


| Opcode        | Summary                                                                                                           |
|---------------|-------------------------------------------------------------------------------------------------------------------|
| `Init`        | Always first opcode. Increments P1 (for `Once` checks); optionally jumps to P2; sets corrupt-error handler to P3. |
| `Halt`        | Exit immediately. P1 = result code (0 = OK). P2 = rollback behavior (Fail/Rollback/Abort).                        |
| `HaltIfNull`  | If reg[P3] is NULL, halt with P1/P2/P4 as error params.                                                           |
| `Goto`        | Unconditional jump to P2.                                                                                         |
| `Gosub`       | Store PC in reg[P1], jump to P2.                                                                                  |
| `Return`      | Jump to address in reg[P1]+1. P3=1 = no-op if reg[P1] is not int.                                                 |
| `BeginSubrtn` | Mark subroutine entry; loads NULL into P2 register so `Return` falls through if called inline.                    |
| `Once`        | Fall through on first encounter; jump to P2 on all subsequent encounters in this run.                             |
| `Noop`        | Do nothing.                                                                                                       |
| `Explain`     | No-op at runtime; holds query-plan text in P4 for `EXPLAIN QUERY PLAN` output.                                    |
| `Abortable`   | Debug-only: assert that an Abort here would not corrupt the DB.                                                   |


### Jumps & Comparisons


| Opcode          | Summary                                                                                                             |
|-----------------|---------------------------------------------------------------------------------------------------------------------|
| `Eq`            | Jump to P2 if reg[P3] == reg[P1]. Affinity in P5; collation in P4. `SQLITE_NULLEQ` in P5 makes NULLs equal.         |
| `Ne`            | Jump to P2 if reg[P3] != reg[P1].                                                                                   |
| `Lt`            | Jump to P2 if reg[P3] < reg[P1]. `SQLITE_JUMPIFNULL` in P5 controls NULL handling.                                  |
| `Le`            | Jump to P2 if reg[P3] <= reg[P1].                                                                                   |
| `Gt`            | Jump to P2 if reg[P3] > reg[P1].                                                                                    |
| `Ge`            | Jump to P2 if reg[P3] >= reg[P1].                                                                                   |
| `ElseEq`        | Must follow `Lt`/`Gt`. Jump to P2 if the same two operands would be equal.                                          |
| `Jump`          | Jump to P1, P2, or P3 based on whether the last `Compare` result was <, =, or >. Must follow `Compare`.             |
| `If`            | Jump to P2 if reg[P1] is non-zero (true). P3 controls NULL behavior.                                                |
| `IfNot`         | Jump to P2 if reg[P1] is zero (false). P3 controls NULL behavior.                                                   |
| `IsNull`        | Jump to P2 if reg[P1] is NULL.                                                                                      |
| `NotNull`       | Jump to P2 if reg[P1] is not NULL.                                                                                  |
| `IsTrue`        | Implements `IS TRUE/FALSE/NOT TRUE/NOT FALSE`. Stores 0/1 in reg[P2]; P3 = NULL substitute; P4 = invert flag.       |
| `IsType`        | Jump to P2 if the type of a column matches bits in P5 (0x01=INT, 0x02=FLOAT, 0x04=TEXT, 0x08=BLOB, 0x10=NULL).      |
| `Compare`       | Compare register vectors reg[P1..P1+P3-1] vs reg[P2..P2+P3-1]. Result saved for `Jump`. Must be followed by `Jump`. |
| `Permutation`   | Set permutation for next `Compare` (P4 = int array).                                                                |
| `DecrJumpZero`  | Decrement reg[P1]; jump to P2 if new value == 0.                                                                    |
| `IfPos`         | If reg[P1] >= 1, subtract P3 from reg[P1] and jump to P2.                                                           |
| `IfNotZero`     | If reg[P1] > 0, decrement reg[P1] and jump to P2.                                                                   |
| `IfNullRow`     | If cursor P1 is on a NULL row, set reg[P3]=NULL and jump to P2.                                                     |
| `IfNotOpen`     | If cursor P1 is not open or on a NULL row, jump to P2.                                                              |
| `IfSizeBetween` | Let X = 10·log2(N) for table at P1. Jump to P2 if X is in [P3, P4].                                                 |
| `IfEmpty`       | Jump to P2 if the b-tree at cursor P1 is empty.                                                                     |
| `MustBeInt`     | Force reg[P1] to integer. Jump to P2 (or raise error if P2=0) if conversion would lose data.                        |


### Arithmetic & Logic


| Opcode      | Summary                                                            |
|-------------|--------------------------------------------------------------------|
| `Add`       | reg[P3] = reg[P2] + reg[P1]. NULL if either input is NULL.         |
| `AddImm`    | reg[P1] += P2 (integer). Use `AddImm 0` to coerce to int.          |
| `Subtract`  | reg[P3] = reg[P2] - reg[P1].                                       |
| `Multiply`  | reg[P3] = reg[P1] * reg[P2].                                       |
| `Divide`    | reg[P3] = reg[P2] / reg[P1]. NULL if P1 is zero or either is NULL. |
| `Remainder` | reg[P3] = reg[P2] % reg[P1]. NULL if P1 is zero or either is NULL. |
| `BitAnd`    | reg[P3] = reg[P1] & reg[P2]. NULL if either is NULL.               |
| `BitOr`      | reg[P3] = reg[P1] | reg[P2]. NULL if either is NULL.                    |
| `BitNot`     | reg[P2] = ~reg[P1] (ones-complement). NULL if P1 is NULL.               |
| `ShiftLeft`  | reg[P3] = reg[P2] << reg[P1].                                           |
| `ShiftRight` | reg[P3] = reg[P2] >> reg[P1].                                           |
| `And`        | reg[P3] = reg[P1] AND reg[P2]. 0 if either is 0 (even with NULL).       |
| `Or`         | reg[P3] = reg[P1] OR reg[P2]. 1 if either is non-zero (even with NULL). |
| `Not`        | reg[P2] = boolean complement of reg[P1]. NULL → NULL.                   |
| `Concat`     | reg[P3] = reg[P2] || reg[P1]. NULL if either is NULL.                   |


### Register Operations


| Opcode         | Summary                                                                                  |
|----------------|------------------------------------------------------------------------------------------|
| `Integer`      | reg[P2] = P1 (32-bit int literal).                                                       |
| `Int64`        | reg[P2] = *P4 (64-bit int literal).                                                      |
| `Real`         | reg[P2] = *P4 (64-bit float literal).                                                    |
| `String`       | reg[P2] = P4 (string literal, P1 bytes, encoding P3).                                    |
| `String8`      | Like `String` but P4 is UTF-8; self-converts to `String` after first run.                |
| `Blob`         | reg[P2] = P4 blob of P1 bytes (or zero-filled if P4 is NULL).                            |
| `Null`         | reg[P2] = NULL. Also clears P2..P3 if P3 > P2.                                           |
| `NullRow`      | Move cursor P1 to a NULL row; all `Column` reads return NULL.                            |
| `Copy`         | Deep copy of reg[P1..P1+P3] into reg[P2..P2+P3].                                         |
| `SCopy`        | Shallow copy of reg[P1] into reg[P2] (pointer copy — source must outlive dest).          |
| `IntCopy`      | Like `SCopy` but only for integers (optimized).                                          |
| `Move`         | Move reg[P1..P1+P3-1] to reg[P2..P2+P3-1]; source registers become NULL.                 |
| `ReleaseReg`   | Mark P2 registers starting at P1 as releasable (debug/validation only).                  |
| `Cast`         | Coerce reg[P1] to type P2: A=BLOB, B=TEXT, C=NUMERIC, D=INTEGER, E=REAL. NULL unchanged. |
| `RealAffinity` | If reg[P1] is an integer, convert it to float (for REAL-affinity columns).               |
| `Affinity`     | Apply column affinities (P4 string) to P2 registers starting at P1.                      |
| `ClrSubtype`   | Clear the subtype field of reg[P1].                                                      |
| `GetSubtype`   | reg[P2] = subtype of reg[P1], or NULL if no subtype.                                     |


### Cursor — Open / Close


| Opcode          | Summary                                                                                                 |
|-----------------|---------------------------------------------------------------------------------------------------------|
| `OpenRead`      | Open cursor P1 read-only on table/index with root page P2 in database P3. P4 = column count or KeyInfo. |
| `OpenWrite`     | Open cursor P1 read/write on table/index. Same params as `OpenRead`.                                    |
| `ReopenIdx`     | Like `OpenRead` but no-ops if cursor P1 is already open on the same b-tree.                             |
| `OpenEphemeral` | Open cursor P1 on a new transient (auto-deleted) b-tree. P2 = columns; P4 = KeyInfo if index.           |
| `OpenAutoindex` | Same as `OpenEphemeral`; name signals use for auto-created join indices.                                |
| `OpenDup`       | Duplicate ephemeral cursor P2 into cursor P1 (for self-joins).                                          |
| `OpenPseudo`    | Open cursor P1 as alias for the MEM_Blob in reg[P2]. Used by the sorter.                                |
| `Close`         | Close cursor P1 (no-op if already closed).                                                              |


### Cursor — Positioning & Scanning


| Opcode         | Summary                                                                                               |
|----------------|-------------------------------------------------------------------------------------------------------|
| `Rewind`       | Position cursor P1 at the first row. Jump to P2 if table is empty. Enables `Next`.                    |
| `Last`         | Position cursor P1 at the last row. Jump to P2 if empty. Enables `Prev`.                              |
| `Next`         | Advance cursor P1 forward. Jump to P2 if successful; fall through if exhausted.                       |
| `Prev`         | Retreat cursor P1 backward. Jump to P2 if successful; fall through if exhausted.                      |
| `SeekGE`       | Seek cursor P1 to first row ≥ key (reg[P3] or P4 regs). Jump to P2 if none found. Enables `Next`.     |
| `SeekGT`       | Seek cursor P1 to first row > key. Jump to P2 if none. Enables `Next`.                                |
| `SeekLE`       | Seek cursor P1 to last row ≤ key. Jump to P2 if none. Enables `Prev`.                                 |
| `SeekLT`       | Seek cursor P1 to last row < key. Jump to P2 if none. Enables `Prev`.                                 |
| `SeekRowid`    | Seek table cursor P1 to rowid in reg[P3]. Jump to P2 if not found or reg[P3] not int.                 |
| `SeekEnd`      | Position cursor P1 at the end (for appending).                                                        |
| `SeekScan`     | Optimization prefix for `SeekGE`: try stepping forward up to P1 times instead of seeking.             |
| `SeekHit`      | Update cursor P1's seekHit value to be in [P2, P3]. Used by `IfNoHope` optimization.                  |
| `DeferredSeek` | Deferred seek: mark cursor P3 to seek to the row corresponding to index cursor P1 when actually read. |
| `FinishSeek`   | Complete a deferred seek on cursor P1 immediately if not yet done.                                    |
| `Found`        | Jump to P2 if the key from reg[P3] (or P4 regs) is a prefix of any entry in index cursor P1.          |
| `NotFound`     | Jump to P2 if the key from reg[P3] (or P4 regs) is not a prefix of any entry in index cursor P1.      |
| `NoConflict`   | Jump to P2 if key has any NULL field, or if no matching index entry exists.                           |
| `NotExists`    | Jump to P2 if table cursor P1 has no row with integer rowid in reg[P3].                               |
| `IfNoHope`     | Optimization: if seekHit < P4, run `NotFound`-like check; jump to P2 if no hope of match.             |


### Cursor — Read


| Opcode     | Summary                                                                                                |
|------------|--------------------------------------------------------------------------------------------------------|
| `Column`   | reg[P3] = column P2 from record at cursor P1. NULL if column missing.                                  |
| `Rowid`    | reg[P2] = integer rowid of current row at cursor P1.                                                   |
| `RowData`  | reg[P2] = raw bytes of the entire current row at cursor P1.                                            |
| `IdxRowid` | reg[P2] = rowid stored at the end of the index key at cursor P1.                                       |
| `Offset`   | reg[P3] = byte offset of the current record in the DB file (requires `SQLITE_ENABLE_OFFSET_SQL_FUNC`). |


### Cursor — Write


| Opcode       | Summary                                                                                             |
|--------------|-----------------------------------------------------------------------------------------------------|
| `Insert`     | Write reg[P2] (blob) with key reg[P3] (int rowid) into table cursor P1.                             |
| `IdxInsert`  | Write key reg[P2] (from `MakeRecord`) into index cursor P1.                                         |
| `RowCellCu`  | Copy current row of cursor P2 into cursor P1 (must follow with `Insert`/`IdxInsert PREFORMAT`).     |
| `Delete`     | Delete current row at cursor P1. P5 flags: `SAVEPOSITION` leaves cursor on next/prev row.           |
| `IdxDelete`  | Remove index entry (P2..P2+P3-1 = unpacked key) from index cursor P1.                               |
| `NewRowid`   | Generate a new unused rowid for table cursor P1; store in reg[P2]. P3 = optional max-seen register. |
| `MakeRecord` | Pack P2 registers starting at P1 into a record blob stored in reg[P3]. P4 = affinity string.        |
| `Count`      | reg[P2] = number of rows in table/index at cursor P1. P3≠0 = estimate from cursor position.         |


### Index Comparisons


| Opcode     | Summary                                                                                                                                              |
|------------|------------------------------------------------------------------------------------------------------------------------------------------------------|
| `IdxGE`    | Jump to P2 if index cursor P1's current key ≥ unpacked key from P3..P3+P4-1 (ignoring PK).                                                           |
| `IdxGT`    | Jump to P2 if index cursor P1's current key > unpacked key.                                                                                          |
| `IdxLE`    | Jump to P2 if index cursor P1's current key ≤ unpacked key.                                                                                          |
| `IdxLT`    | Jump to P2 if index cursor P1's current key < unpacked key.                                                                                          |
| `IFindKey` | Used by `integrity_check`: search for an index entry close to current position that matches within floating-point rounding; jump to P2 if not found. |


### Aggregate & Window Functions


| Opcode       | Summary                                                                                                      |
|--------------|--------------------------------------------------------------------------------------------------------------|
| `AggStep`    | Call xStep of aggregate (P4 = FuncDef) with P5 args from reg[P2..]. Accumulator = reg[P3].                   |
| `AggStep1`   | Like `AggStep` but self-modifies on first call to cache sqlite3_context (avoids per-call init).              |
| `AggInverse` | Call xInverse of window aggregate with P5 args from reg[P2..].                                               |
| `AggFinal`   | Call xFinal of aggregate; store result in reg[P1].                                                           |
| `AggValue`   | Call xValue (current window value); store in reg[P3].                                                        |
| `CollSeq`    | Set collation sequence (P4) for next built-in function call (min/max/nullif).                                |
| `Function`   | Call user function (P4 = sqlite3_context) with P5 args from reg[P2..]; result → reg[P3].                     |
| `PureFunc`   | Like `Function` but marks the call as non-deterministic. Throws error if used where determinism is required. |


### Sorting


| Opcode          | Summary                                                                                         |
|-----------------|-------------------------------------------------------------------------------------------------|
| `SorterOpen`    | Open cursor P1 on a sorter (external merge sort). P2 = columns; P3 = key columns; P4 = KeyInfo. |
| `SorterInsert`  | Insert reg[P2] as a key into sorter at cursor P1.                                               |
| `SorterSort`    | Sort entries in sorter at cursor P1. Jump to P2 if empty.                                       |
| `SorterData`    | reg[P2] = current data record from sorter cursor P1; leave sort key in reg[P3].                 |
| `SorterNext`    | Advance sorter cursor P1. Jump to P2 if row retrieved; fall through if done.                    |
| `SorterCompare` | Compare current sorter entry P1 against reg[P3..P3+P4-1]; jump to P2 if different.              |
| `ResetSorter`   | Clear all content from sorter/ephemeral table at cursor P1.                                     |


### Transactions & Schema


| Opcode         | Summary                                                                                             |
|----------------|-----------------------------------------------------------------------------------------------------|
| `Transaction`  | Begin a transaction on database P1. P2=0 read, P2=1 write. P3 = schema version to verify.           |
| `AutoCommit`   | Set auto-commit to P1. If P2=true, roll back active transactions. Halts the VM.                     |
| `Savepoint`    | Open (P1=0), release (P1=1), or rollback (P1=2) savepoint named P4.                                 |
| `TableLock`    | Assert table-level lock on table P2 in database P1. P3=0 read lock, P3=1 write lock.                |
| `ReadCookie`   | reg[P2] = schema cookie P3 from database P1 (e.g. P3=1 → schema version).                           |
| `SetCookie`    | Set schema cookie P3 in database P1 to value reg[P2].                                               |
| `Expire`       | Expire all (P1=0) or current (P1=1) prepared statements.                                            |
| `ParseSchema`  | Re-parse schema entries matching P4 (WHERE clause) from database P1.                                |
| `LoadAnalysis` | Load `sqlite_stat1` for database P1 into the query planner.                                         |
| `DropTable`    | Remove in-memory schema entry for table P4 in database P1.                                          |
| `DropIndex`    | Remove in-memory schema entry for index P4 in database P1.                                          |
| `DropTrigger`  | Remove in-memory schema entry for trigger P4 in database P1.                                        |
| `IntegrityCk`  | Run integrity check on database. Errors stored in reg[P1+1]. P4 = root page array; P3 = max errors. |


### B-Tree Management


| Opcode        | Summary                                                                                                   |
|---------------|-----------------------------------------------------------------------------------------------------------|
| `CreateBtree` | Allocate a new b-tree in database P1 (0=main, 1=temp). P3=1 rowid table, P3=2 index. Root page → reg[P2]. |
| `Clear`       | Delete all rows from table/index at root page P1. Does not drop the table itself.                         |
| `Destroy`     | Drop the entire table/index at root page P1. Frees pages.                                                 |
| `VDestroy`    | Drop virtual table P4.                                                                                    |
| `VCreate`     | Create virtual table P4.                                                                                  |
| `IncrVacuum`  | One step of incremental vacuum on database P1. Jump to P2 when done.                                      |
| `Vacuum`      | Run full vacuum on database P1.                                                                           |
| `Pagecount`   | reg[P2] = current page count of database P1.                                                              |
| `MaxPgcnt`    | Set max page count for database P1 to max(current, P3). reg[P2] = new max.                                |


### Bloom Filter


| Opcode      | Summary                                                                                                          |
|-------------|------------------------------------------------------------------------------------------------------------------|
| `Filter`    | Hash P4 registers from reg[P3]; if hash not in bloom filter reg[P1], maybe jump to P2 (avoids expensive lookup). |
| `FilterAdd` | Hash P4 registers from reg[P3] and add to bloom filter in reg[P1].                                               |


### RowSet (Deduplication)


| Opcode       | Summary                                                                                          |
|--------------|--------------------------------------------------------------------------------------------------|
| `RowSetAdd`  | Insert integer reg[P2] into RowSet object in reg[P1].                                            |
| `RowSetRead` | Extract smallest value from RowSet reg[P1] into reg[P3]. Jump to P2 if empty.                    |
| `RowSetTest` | If RowSet reg[P1] contains integer reg[P3], jump to P2. Otherwise insert it. P4 = set phase tag. |


### Subroutines / Coroutines / Subprograms


| Opcode          | Summary                                                                                          |
|-----------------|--------------------------------------------------------------------------------------------------|
| `InitCoroutine` | Set up reg[P1] to yield to coroutine at P3. Jump to P2 to skip coroutine body.                   |
| `Yield`         | Swap PC with integer in reg[P1]. Implements coroutines.                                          |
| `EndCoroutine`  | Jump to the P2 of the `Yield` at reg[P1]'s address. Terminates coroutine.                        |
| `Program`       | Execute trigger subprogram P4. P1 = first arg register; P2 = IGNORE jump; P3 = scratch register. |
| `Param`         | Copy a value from the parent frame's register into reg[P2]. Used inside trigger subprograms.     |


### LIMIT / OFFSET


| Opcode        | Summary                                                                                                |
|---------------|--------------------------------------------------------------------------------------------------------|
| `OffsetLimit` | Compute combined LIMIT+OFFSET: reg[P2] = reg[P1] + reg[P3] (or -1 if no LIMIT, or LIMIT if no OFFSET). |


### Output


| Opcode      | Summary                                                                                              |
|-------------|------------------------------------------------------------------------------------------------------|
| `ResultRow` | Emit registers reg[P1..P1+P2-1] as one result row. Pauses VM; `sqlite3_step()` returns `SQLITE_ROW`. |


### WAL / Pager


| Opcode        | Summary                                                                                            |
|---------------|----------------------------------------------------------------------------------------------------|
| `Checkpoint`  | WAL checkpoint on database P1. P2 = mode (PASSIVE/FULL/RESTART/TRUNCATE). Results → reg[P3..P3+2]. |
| `JournalMode` | Change journal mode of database P1 to P3. Result string → reg[P2].                                 |


### Memory


| Opcode   | Summary                                                   |
|----------|-----------------------------------------------------------|
| `MemMax` | reg[P1] = max(reg[P1], reg[P2]). P1 is in the root frame. |


### Virtual Tables


| Opcode    | Summary                                                                                   |
|-----------|-------------------------------------------------------------------------------------------|
| `VOpen`   | Open virtual table P4; cursor → P1.                                                       |
| `VCheck`  | Run change constraints for virtual table.                                                 |
| `VColumn` | Read column P2 from virtual table cursor P1; result → reg[P3].                            |
| `VFilter` | Begin scan of virtual table cursor P1 using index P3 with query string P4.                |
| `VNext`   | Advance virtual table cursor P1. Jump to P2 if row retrieved.                             |
| `VRename` | Rename virtual table P4 to the string in reg[P1].                                         |
| `VUpdate` | Insert/update/delete on virtual table P4. P3 regs starting at P2; result rowid → reg[P1]. |
| `VInitIn` | Set up IN-constraint iteration for virtual table.                                         |


---

## EXPLAIN Output Format

```
addr  opcode         p1    p2    p3    p4             p5  comment
----  -------------  ----  ----  ----  -------------  --  -------
0     Init           0     12    0                    00  Start at 12
...
```

Run `EXPLAIN <sql>` in the SQLite shell to see this for any statement.

## Key Notes

- Opcodes are **not stable** across SQLite versions.
- `SQLITE_DEBUG` build unlocks `PRAGMA vdbe_trace`, `vdbe_listing`, etc.
- Long-running `EXPLAIN QUERY PLAN` uses the `Explain` opcode (not `EXPLAIN`).
- Source of truth: `[vdbe.c](https://sqlite.org/src/file/src/vdbe.c)` comments.

## Appendix

Some of my QA with Claude.

Q: Why does sqlite init jump to the transaction code, just to jump back to the top?
A:

This is a genuinely elegant design that falls out from how `Init` and `Once` interact. Let me walk through the reasoning.

## The Pattern

A typical compiled statement looks roughly like:

```
addr 0:  Init       → jump to addr N
addr 1:  [Once-guarded init: set up ephemeral tables, constant subexprs, etc.]
addr 2:  OpenRead   cursor 0
addr 3:  Rewind     → jump to addr M if empty
...      [query body]
addr M:  Halt
addr N:  Transaction (begin txn, verify schema cookie)
addr N+1:Goto 1
```

## Why Not `Transaction` First?

The reference says `Init` is **"always the first opcode"** — not by convention, but because it does two things everything else depends on:

1. It increments an internal **generation counter** that `Once` checks.
2. It installs the **corrupt-error handler** (P3).

`Once` works by remembering what the generation counter was when it last fell through. If the current counter matches, it's a re-run — jump past the one-time setup. If it doesn't match, it's a fresh execution — fall through and run the init code.

This means `Init` *must* fire before any `Once` block is evaluated. If `Transaction` came first, you'd have opcodes running before `Once`'s counter is initialized, which breaks re-execution semantics entirely.

## The Actual Reason for the Jump-to-End Shape

The `Transaction` opcode does something subtle: **it verifies the schema cookie** (P3). If the schema has changed since the statement was prepared, the statement is expired and must be re-prepared. This check needs to happen *before any cursor opens or data access* — but *after* `Init` has set up the generation counter.

The jump-to-end layout threads this needle:

1. `Init` fires at addr 0 → generation counter incremented, error handler set → **jump to `Transaction` at addr N**
2. `Transaction` checks schema cookie, begins the transaction → **Goto addr 1**
3. `Once`-guarded blocks at addr 1+ now execute correctly — `Init` already ran, so the counter is valid

The `Once` blocks live between addr 1 and the first real cursor open. They initialize things like constant expression caches, ephemeral sort tables, and bloom filters — things that should be set up once per `sqlite3_step` call, not re-initialized on each row. The Init→jump-over-Once-blocks→Transaction→jump-back structure means: **when execution first reaches the `Once` blocks, `Init` has already run but the `Once` blocks haven't yet fired this generation — so they fall through and initialize correctly.**

## The Deeper Elegance

The layout creates a clean separation of *three* lifetimes:

| Code location          | Lifetime                     | What lives here                           |
|------------------------|------------------------------|-------------------------------------------|
| Addr 0 (`Init`)        | Per-execution setup          | Generation counter, error handler         |
| Addr N (`Transaction`) | Per-statement-validity check | Schema cookie verification, txn begin     |
| Addr 1..M-1            | Per-row logic                | Cursors, loops, Once-guarded init, output |

If `Transaction` were at addr 1 and `Init` at addr 0, you'd collapse the first two lifetimes together and lose the ability to re-check schema validity independently of re-initializing `Once` state. The jump-to-end shape keeps them orthogonal — `Transaction` can fail and expire the statement without having touched any `Once` state at all.

It's a small architectural decision, but it's the kind that only makes sense once you understand that SQLite's prepared statements are designed to be **reset and re-executed many times**, with schema-change detection as a first-class concern at every step call.

