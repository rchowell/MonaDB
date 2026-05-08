# MonaDB Storage Layer — Reference

A study guide. Read this before writing code. Companion to `storage-plan.md`.

The goal of this document is to give you a complete mental model of how the storage layer works — physical layout, key encoding theory, MVCC, branching, cursor mechanics — so that the implementation phases land without surprises. Every design decision below is justified, and the trickier ones are demonstrated with byte-level examples.

---

## Table of contents

1. [Why LMDB](#1-why-lmdb)
2. [LMDB in one page](#2-lmdb-in-one-page)
3. [Heed: the Rust binding](#3-heed-the-rust-binding)
4. [Key encoding theory](#4-key-encoding-theory)
5. [Value encoding](#5-value-encoding)
6. [MonaDB's physical layout](#6-monadbs-physical-layout)
7. [The data key, byte by byte](#7-the-data-key-byte-by-byte)
8. [The cursor state machine](#8-the-cursor-state-machine)
9. [Read-your-own-writes](#9-read-your-own-writes)
10. [The storage façade](#10-the-storage-façade)
11. [LMDB gotchas](#11-lmdb-gotchas)
12. [Worked example: a full request](#12-worked-example-a-full-request)
13. [Glossary](#13-glossary)
14. [Appendix A: Future versioning and branching](#appendix-a-future-versioning-and-branching)

---

## 1. Why LMDB

LMDB (Lightning Memory-Mapped Database) is a memory-mapped B+ tree that maps `[u8] → [u8]` with full ACID transactions and MVCC. It is the right foundation for MonaDB because:

- **Single-file durability.** The whole DB is one file (`data.mdb`). Backup is `cp data.mdb`. There is no WAL to replay; recovery is instantaneous because writes use copy-on-write pages and a final atomic root-pointer swing.
- **Crash safety is free.** Either the root pointer swings on commit or it doesn't. Partially-written pages are unreachable garbage.
- **MVCC out of the box.** Read transactions see a consistent snapshot of the DB; readers never block writers and writers never block readers. We piggyback on this for our git-like branching.
- **Zero opinions about schema.** LMDB stores opaque bytes. The layer above owns key shape, type system, indexes, and branching semantics. That's exactly the separation we want.
- **Ordered cursors.** Keys are stored in a B+ tree sorted lexicographically over raw bytes. We exploit this for partition scans and for MVCC version walks.

What LMDB is *not*: a full database. There is no SQL, no query planner, no schema, no secondary indexes (until you build them), no replication. We build all of that on top.

---

## 2. LMDB in one page

### Three operations

`put(key, value)`, `get(key)`, `delete(key)`, plus ordered iteration via cursors. That's nearly the whole API.

### Pages and copy-on-write

The DB file is divided into pages (default 4 KB). The B+ tree's nodes are pages. When a write transaction modifies a leaf page, LMDB does *not* mutate the page in place — it allocates a new page, writes the modified version there, and propagates the change up the tree (also via new pages) all the way to a new root. Only when the txn commits is a single 8-byte root pointer in the meta page atomically swung to the new root. Until that swing, the old tree is fully intact and all readers continue to see it.

This is why crash recovery is trivial: a torn root-pointer write is impossible (it's atomic on every modern OS for 8-byte aligned writes), so on crash the DB is either pre-commit (old tree visible) or post-commit (new tree visible).

### MVCC

Because old pages aren't reclaimed until no reader references them, every reader gets a frozen snapshot of the DB at the moment its transaction opened. Readers walk the tree starting from whatever root pointer was current at open time. They block nothing and are blocked by nothing.

Old pages live on a freelist; LMDB reclaims them when no live reader needs them. This means **a long-lived reader pins old pages and grows the file**. We accept this and document it; in our embedded usage, read transactions are short.

### Single writer

There is at most one concurrent write transaction. LMDB serializes them with a mutex. For an embedded JSON DB this is fine.

### Named databases

A single LMDB env can hold multiple "named databases," which are just multiple B+ trees inside the same file sharing one transaction system. You declare the maximum number at env-open time (`max_dbs`). All named DBs commit atomically together — this is why we can update the data DB and the refs DB and the commits DB in one transaction without ever leaving them inconsistent.

### Map size

`map_size` is the maximum the file can grow to. It's a **virtual address space reservation**, not a physical disk allocation. On a 64-bit system you can set it to 1 TB without paying any cost up front. We default to 1 GiB and grow.

---

## 3. Heed: the Rust binding

`heed` wraps the LMDB C library with a typed, safe Rust API. Key things to know:

- `EnvOpenOptions::new().map_size(...).max_dbs(...).open(path)` returns `heed::Env`. Cheap to clone (`Arc` internally).
- `env.read_txn() → RoTxn<'env>` — a read transaction. Drop closes it.
- `env.write_txn() → RwTxn<'env>` — a write transaction. Call `.commit()` to make it durable; drop without committing aborts.
- `Database<KC, VC>` — a typed handle to a named DB. We use `Database<Bytes, Bytes>` for `data` and most others; the type parameters let heed enforce serialization at compile time. Since we own the byte layout, `Bytes` is right.
- `db.put(&mut txn, &key, &value)`, `db.get(&txn, &key) → Option<&[u8]>`, `db.delete(&mut txn, &key)`.
- `db.range(&txn, &range) → RoIter`, `db.iter(&txn) → RoIter`, `db.rev_range(...)` etc. for ordered scans.
- Cursors: `db.iter` returns an iterator; for finer control (positioning to a specific key, then walking forward) use `RoCursor` via `db.iter(...)` or `db.range(...)`. heed 0.20 exposes `move_on_key_greater_than_or_equal_to` and friends.

### Lifetimes

`RoTxn<'env>` borrows the env. `RoCursor<'txn>` borrows the txn. Values returned from `db.get` borrow from the txn (zero-copy from the mmap). This lifetime chain matters for our cursor design: a `StorageCursor<'txn>` cannot outlive its `ReadTxn`, which cannot outlive its `MonaDB`.

---

## 4. Key encoding theory

**This is the most important section.** LMDB sorts keys lexicographically over raw bytes — byte 1 first, then byte 2, etc., shorter prefixes sort before their extensions. Every property we want from the storage layer (partition contiguity, sorted scans, MVCC version order) flows from how we map logical key values to byte strings.

### 4.1 Lexicographic order — what it is

For two byte strings `a` and `b`:

```
a < b  iff  there exists i such that
              a[..i] == b[..i] AND
              (i == a.len() AND i < b.len())          -- a is a strict prefix of b
              OR
              i < a.len() AND i < b.len() AND a[i] < b[i]
```

In words: scan left to right; the first byte where they differ decides; if one runs out first, the shorter one wins.

### 4.2 Sort-order preservation

A logical type `T` has an encoding `enc: T → [u8]` that is **sort-order preserving** if, for all `x, y: T`, `x < y` (in T's logical order) iff `enc(x) < enc(y)` (in lex order). Most type encodings are *not* sort-order preserving by default. We have to design them.

- **Strings (UTF-8).** Already sort-order preserving by codepoint (which is what byte order gives you for UTF-8). "alice" < "bob" both logically and byte-wise.
- **Unsigned integers.** Must be encoded **big-endian**. In little-endian, `1 = 01 00 00 00` and `256 = 00 01 00 00` — the smaller number sorts after the larger because the low byte comes first. In big-endian, `1 = 00 00 00 01` and `256 = 00 00 01 00`, which sort correctly.
- **Signed integers.** Big-endian alone isn't enough: `-1` in two's complement is `FF FF FF FF FF FF FF FF` and `1` is `00 00 00 00 00 00 00 01`. Lex order puts `-1` *after* `1`, which is wrong. Fix: XOR the high bit (`0x8000_0000_0000_0000`). This flips the sign bit so negatives (now leading `0x00..0x7F`) sort before positives (now leading `0x80..0xFF`).
- **Floats.** IEEE 754: positive floats are already sort-order preserving by their bit pattern (sign bit 0, exponent BE, mantissa BE). Negatives are inverted (larger magnitude has smaller bits, opposite of what we want) and sort *after* positives because their sign bit is 1. Fix: if sign bit is 0, flip the sign bit; if sign bit is 1, invert the entire 64 bits. We don't need this in v1 (only string and int are valid PK types).
- **Descending sort.** Bitwise-invert the entire encoded value; lex order then runs backwards.

### 4.3 Composite keys: the terminator vs length-prefix question

A **composite key** is a tuple `(c_1, c_2, ..., c_N)` of N ≥ 1 components encoded as a single byte string in such a way that:

1. `(c_1, c_2, ...)` < `(c_1', c_2', ...)` iff the tuple is logically less in dictionary (column-by-column) order.
2. We can do prefix scans: "all rows with `c_1 == 'alice'`" must form a contiguous range. Same for `c_1 == X AND c_2 == Y`, and so on.

The naive approach `enc(c_1) || enc(c_2) || ...` (raw concatenation) breaks rule 1 when components are variable-length. Example:

- `("alic", "e")` → `alic` `e` → `61 6c 69 63 65`
- `("alice", "")` → `alice` `` → `61 6c 69 63 65`

Same bytes, different tuples — collision. Or:

- `("alic", "z")` → `alicz` → `61 6c 69 63 7a`
- `("alice", "")` → `alice` → `61 6c 69 63 65`

Lex order says `alice` < `alicz`, but `("alic", "z")` should sort *before* `("alice", "")` because `"alic" < "alice"`. Contradiction.

There are two fixes. **We use the first.**

#### Option A: terminator with byte-stuffing escape

Reserve a byte (we use `0x00`) as a terminator between components. Inside any component, escape any literal `0x00` byte as `0x00 0x01`, and to keep the escape unambiguous, escape `0x01` as `0x00 0x02`. Now no component-internal byte can equal the two-byte terminator `0x00 0x00`.

```
enc("alic", "e")    → 61 6c 69 63   00 00   65   00 00
enc("alice", "")    → 61 6c 69 63 65 00 00         00 00
```

Compare them byte-wise: the first 4 bytes match (`61 6c 69 63`); the 5th byte differs (`00` vs `65`). `00 < 65`, so `("alic", "e") < ("alice", "")`. Correct.

The same scheme generalizes to any N. For a 3-component key `("acme", "alice", "evt-1")`:

```
61 63 6d 65 00 00 | 61 6c 69 63 65 00 00 | 65 76 74 2d 31 00 00
[c_1 = "acme"   ] | [c_2 = "alice"     ] | [c_3 = "evt-1"     ]
```

Prefix scan for "all rows where c_1 = 'acme'": range from `[61 63 6d 65 00 00]` to `[61 63 6d 65 00 01]`. Prefix scan for "c_1 = 'acme' AND c_2 = 'alice'": range from `[61 63 6d 65 00 00 61 6c 69 63 65 00 00]` to `[61 63 6d 65 00 00 61 6c 69 63 65 00 01]`. Each leading-column subset gives a single contiguous range. This is the property that makes composite primary keys cheap to query and is why a planner can recognize them with a simple prefix match against `WHERE`.

This is the [BigTable / FoundationDB tuple layer](https://apple.github.io/foundationdb/data-modeling.html#tuples) trick. It preserves lex order across variable-length components and supports prefix scans cleanly. The cost is at most 2× expansion for keys that are all `0x00` — vanishingly rare in real data.

#### Option B: length-prefix every component

Prefix each component with its length. To preserve lex order across components of different lengths you need a length encoding that is itself sort-order-preserving — which means BE fixed width. Then every component has a fixed cost overhead (e.g. 2 bytes for u16 length). This works but is fiddly and harder to debug. We don't use it.

### 4.4 Why N-component lex composite, not (partition, sort) or Z-order

We chose **N-component lexicographic composite primary keys** as the only physical key model. Other shapes are deferred to opt-in secondary indexes:

- **(Partition, sort) à la DynamoDB** is the same physical layout as a 2-component composite, but the vocabulary imports distributed-systems framing — privileged single-column partition, exactly two key levels, eventual sharding — that doesn't apply to an embedded engine and confuses users coming from SQL. A 2-slot model is also strictly less expressive: it can't natively represent `(tenant, user, created_at)` without overloading one slot.
- **Z-order / Morton encoding** specializes for spatial bounding-box queries, requires fixed-width components, and breaks prefix-scan semantics. It belongs as an opt-in spatial index, not as a primary-key encoding.
- **R-tree** is an entirely different index structure for multi-dimensional range queries; orthogonal to primary-key storage.

The lex composite encoder also generalizes for free: the loop that handles a 1-column PK is the same loop that handles N columns. The asymmetry between "support N from day one" and "retrofit N onto a 2-slot model later" is enormous (every existing row has to be re-keyed); we pay the small upfront cost to avoid it.

### 4.5 Reserved suffix bytes

Every data key carries a fixed 8-byte suffix at the end. In the current design these bytes are always zero — they are **reserved** for a future versioning system that will use them to encode a per-row version stamp without disturbing the rest of the layout (see Appendix A). Because the suffix is fixed-width and always last, it doesn't interact with prefix scan boundaries: range bounds are computed by varying the bytes *before* the suffix.

Forward-compat reservations are cheap insurance. Adding the bytes later would mean re-keying every existing row; pre-paying 8 bytes per key now is essentially free.

### 4.6 Worked byte examples

Suppose a table `events (tenant string, user string, ts int)`. Insert a row whose JSON contains `tenant = "acme"`, `user = "alice"`, `ts = 1234`, into `table_id = 7`. The data key is built by concatenating five pieces:

```
u32_be(7)                = 00 00 00 07
enc("acme")              = 61 63 6d 65 00 00
enc("alice")             = 61 6c 69 63 65 00 00
i64 1234 sign-biased BE  = (1234 as u64) ^ 0x8000_0000_0000_0000
                         = 80 00 00 00 00 00 04 D2
                         (no terminator — fixed-width components don't need one
                          when no further variable-length component follows.)
reserved suffix          = 00 00 00 00 00 00 00 00
                         (always zero in the current design;
                          reserved for the future versioning system, see Appendix A)

Full key (5 segments):
00 00 00 07 | 61 63 6d 65 00 00 | 61 6c 69 63 65 00 00 | 80 00 00 00 00 00 04 D2 | 00 00 00 00 00 00 00 00
[table_id=7]| [c_1="acme"      ]| [c_2="alice"        ]| [c_3=1234 sign-biased BE          ]| [reserved                   ]
```

Total: 4 + 6 + 7 + 8 + 8 = 33 bytes. The B+ tree stores this key alongside all other rows in table 7 with `tenant="acme"` (contiguous), then within that subset all rows with `user="alice"` (contiguous), then within that subset all rows with `ts=1234` (one row, since the suffix is constant in P1/P2).

Range scans fall out for free at every PK prefix:

| Predicate | Range start | Range end |
|---|---|---|
| All rows in table 7 | `00 00 00 07` | `00 00 00 08` |
| `tenant = "acme"` | `00 00 00 07 \| 61 63 6d 65 00 00` | `00 00 00 07 \| 61 63 6d 65 00 01` |
| `tenant = "acme" AND user = "alice"` | `00 00 00 07 \| 61 63 6d 65 00 00 \| 61 6c 69 63 65 00 00` | `00 00 00 07 \| 61 63 6d 65 00 00 \| 61 6c 69 63 65 00 01` |
| `tenant = "acme" AND user = "alice" AND ts >= 1000` | start = the above start ++ enc(1000) | end = the above end |

Every prefix subset of the PK columns gives one contiguous range in the B+ tree. That's the planner's job in Phase 2: walk the `WHERE` clause, match equalities/range-comparisons against leading PK columns, build start/end byte strings, hand them to the cursor.

### 4.7 LMDB key length limit

LMDB enforces a maximum key length, set at compile time (default 511 bytes). Heed exposes this via `db.put` returning an error. Our key overhead is 4 (table_id) + 2 per string component (terminator) + 8 (seq) = 12 bytes for a single-string PK, plus 2 more per additional string component. We enforce `total_key_len ≤ 511` in `put_row` and surface a clean `KeyTooLong` error.

If we ever need bigger keys, we can patch lmdb-master-sys with `-DMDB_MAXKEYSIZE=4096`. Not for v1.

---

## 5. Value encoding

We store the row's `Value` (a `serde_json::Value` wrapper) as JSON bytes prefixed by a one-byte tag:

```
[tag : u8] [body : variable]

tag = 0x00 → live row;       body = serde_json::to_vec(&value)
tag = 0x01..0xFF → reserved
```

Why a tag byte:
- It's a forward-compatibility hook. Future value variants (tombstones, large-blob pointers, alternate codecs) get distinct tag values without disturbing the row format. Appendix A documents the first reserved use.
- The tag is stable across codec changes. If we swap JSON for CBOR later, the tag byte stays at offset 0 and only the body changes.

Why JSON in v1, not bincode/CBOR/MessagePack:
- We already depend on serde_json.
- Debugging with `mdb_dump` and `xxd` is dramatically easier with JSON values.
- JSON is the *only* lossless wire format for our Value type, which is itself a serde_json::Value. We can swap to CBOR (`ciborium`) later for compactness without changing the storage interface.

---

## 6. MonaDB's physical layout

One LMDB env, two named DBs, fixed `max_dbs = 8` (room to grow without reopening for future features — Appendix A).

| DB        | Key                              | Value                       | Purpose |
|-----------|----------------------------------|-----------------------------|---------|
| `meta`    | `b"schema/<name>"` etc.          | bincode `TableSchema`       | Catalog: table schemas, table_id assignments, counters |
| `data`    | composite (see §7)               | tagged value bytes          | All rows of all tables |

### Why a single shared `data` DB

It would be tempting to create one named DB per user table. We don't, for two reasons:

1. **`max_dbs` is fixed at env open.** It's intended for a small, stable set of logical buckets, not unbounded user tables.
2. **Single-DB writes commit atomically across all tables.** With one shared `data` DB, that's automatic. With per-table DBs, it's still automatic (heed groups them in one txn) but there's no cost to consolidating.

Sharding by `u32_be(table_id)` prefix achieves perfect isolation: a scan of one table never sees another's bytes.

### Why `meta` is separate

It's accessed for *every* operation (look up table_id, fetch schema). Keeping it small and hot — distinct from the data DB whose pages we want full of user rows — keeps the meta pages in cache.

### What `meta` contains

```
b"schema/<table_name>"      → bincode(TableSchema { name, table_id, keys })
b"table_id/<u32_be>"        → table_name (reverse map; useful for diagnostics)
b"next_table_id"            → u32 LE
b"row_seq/<u32_be table>"   → u64 LE   (surrogate row id counter, per-table)
```

---

## 7. The data key, byte by byte

```
Layout (composite PK with N components):
[ u32_be(table_id) ] [ enc(c_1) ] [ enc(c_2) ] ... [ enc(c_N) ] [ reserved 0x00..00 ]

Sizes:
   4 bytes              variable     variable        variable     8 bytes
```

For tables with no declared PK, N = 1 and the single component is a system-allocated `u64_be(row_id)` — fixed-width 8 bytes, no terminator. For declared PKs, each variable-length component (strings) carries a trailing `0x00 0x00` terminator with byte-stuffing escape inside; fixed-width components (ints) are emitted raw.

Properties (you should be able to derive each from §4):

1. **All rows of table T are contiguous.** Range `[u32_be(T)..]` to `[u32_be(T+1)..]` covers them.
2. **Every leading-column subset of the PK gives a contiguous range.** Rows with `c_1 = X` are contiguous; rows with `c_1 = X AND c_2 = Y` are contiguous; and so on for any prefix of the column list. This is the property the planner exploits.
3. **Each row maps to exactly one key.** The 8-byte suffix is constant zero, so within a given `[u32_be(T), enc(c_1), ..., enc(c_N)]` prefix there is at most one entry. Inserting another row with the same PK overwrites it.
4. **Empty trailing components are unambiguous.** A component encoded as just its terminator `00 00` is a different byte string from a component with content. Two rows with primary keys `(a="alic", b="e")` and `(a="alice", b="")` have distinguishable encoded keys.
5. **The trailing 8 bytes are reserved** for forward compatibility (Appendix A). They never change in the current design and can be ignored by every consumer of the data DB except whatever future component activates them.

### What's stored in the value

Just the row's `Value`, tagged:

```
[ 0x00 ] [ serde_json::to_vec(value) ]
   ^         ^
   live      JSON bytes
```

Deletes physically remove the row from `data` via `db.delete(&mut rw, &key)`. There are no tombstones in the current design; the `0x01` tag value is reserved for the future versioning system that will need them (Appendix A).

---

## 8. The cursor state machine

The VM's existing contract:

```rust
fn rewind(&mut self) -> bool;   // true if there's any row
fn next(&mut self) -> bool;     // true if positioned on a new row
fn curr(&self) -> &Row;         // panic if not positioned
```

Our `StorageCursor<'txn>` implements this contract over an LMDB cursor.

### 8.1 State

```rust
pub struct StorageCursor<'txn> {
    inner: heed::RoCursor<'txn>,        // wraps the heed cursor over `data` DB
    table_id: u32,
    table_prefix: [u8; 4],              // u32_be(table_id)
    pk_schema: Arc<Vec<KeyColumn>>,     // for encoding seek tuples (P2)
    curr_row: Option<Row>,              // populated by rewind/next/seek
    state: CursorState,
}
enum CursorState { BeforeStart, Positioned, Exhausted }
```

The cursor holds **no per-scan bound state**. It knows the table's PK schema so it can encode `&[Value]` tuples handed to `seek_*` and `idx_*`, but the upper bound of a range scan is *not* cursor state — it's an operand of an `Idx*` opcode evaluated once per iteration. Everything else (decoding individual PK components from the stored key, building range plans from `WHERE`) happens above the cursor in the keycodec and the planner.

### 8.2 `rewind()`

```text
inner.move_on_key_greater_than_or_equal_to(table_prefix)
state = BeforeStart
curr_row = None
return advance()
```

### 8.3 `next()`

```text
return advance()
```

### 8.4 `advance()`

```text
let (k, v) = match inner.current() {
    Some(kv) => kv,
    None => { state = Exhausted; curr_row = None; return false; }
};

// Past our table prefix?
if !k.starts_with(&table_prefix) {
    state = Exhausted; curr_row = None; return false;
}
// Note: no upper-bound check here. Range termination is the caller's job
// via Idx* opcodes, not the cursor's. See §8.5.

// Tag dispatch on the value.
match v[0] {
    TAG_LIVE => {
        curr_row = Some(decode_row(&v[1..])?);
        state = Positioned;
        inner.move_on_next();  // step past so future next() looks at next entry
        return true;
    }
    _ => {
        // Reserved tag — future versioning system uses these (Appendix A).
        // Phase 1/2 should never see them; surface as decode error if we do.
        return Err(Error::CorruptValueTag);
    }
}
```

That's the entire cursor in P1/P2: walk `data` DB entries within the table prefix, decode each, yield. No version walking, no visibility check, no dedup. The structure of the data DB makes this trivial — exactly one entry per row.

### 8.5 Bounded scans via Seek*/Idx* (P2)

The cursor itself stays an unbounded scanner over the table prefix. Bounded scans are expressed in *bytecode* via two new opcode families that mirror SQLite's `Seek*`/`Idx*` split:

- **`SeekGE/GT/LE/LT(c, ncols, jmp)`** — pop `ncols` values, hand them to `cursor[c].seek_*` as a `&[Value]`. The cursor encodes the tuple via `keycodec`, positions the inner LMDB cursor, and returns `false` if no row in the table matches; the opcode then jumps to `jmp`. One-shot: emitted once at the top of a loop.
- **`IdxGE/GT/LE/LT(c, ncols, jmp)`** — pop `ncols` values, encode them, compare against the cursor's *current* key bytes; jump to `jmp` if the comparison holds. Per-iteration: emitted at the top of every loop iteration as the termination guard.

A range query `select * from events where tenant = 'acme' AND user >= 'b' AND user < 'd'` compiles to:

```
Init(read)
Open("events")                    ; cursor 0
Push("acme"); Push("b")           ; lower-bound tuple (eq prefix + range start)
SeekGE c=0 ncols=2, jmp=end       ; position; if nothing matches, exit
loop_top:
  Push("acme"); Push("d")         ; upper-bound tuple
  IdxGE c=0 ncols=2, jmp=end      ; if (tenant, user) ≥ ("acme", "d"), exit
  Load(0)
  <project>
  Return(0)
  Next c=0, jmp=loop_top
end:
Exit
```

Three subtleties worth internalizing:

1. **The eq prefix is pushed for both Seek and Idx.** Each end of the range carries the full prefix tuple. SQLite does the same. Don't try to "save" the bound in cursor state — it makes the bytecode less composable and obscures what's an operand vs. what's pinned at construction.
2. **Bytewise compare in `idx_*` is correct only because the keycodec is sort-order preserving** (§4). The cursor encodes the bound tuple once per call and does a `[u8]` compare against the LMDB key at the cursor's current position. A comment in `cursor.rs` should spell this out so a future implementer doesn't accidentally swap to a typed compare.
3. **Partial-prefix seeks use prefix-form encoding.** A 1-component seek on a 3-component PK builds `enc(c_1) || 00 00` for `seek_ge` and `enc(c_1) || 00 01` (the next-prefix successor) for an exclusive upper. `keycodec::encode_partial(values, pk_schema)` produces the right form when `values.len() < pk_schema.len()`. A bug here yields malformed full-arity keys that either over- or under-match silently.

For pure equality on the full PK (`where x = 1 and y = 2` on `(x int, y int)`), the planner emits `SeekGE(ncols=2)` + `IdxGT(ncols=2)`. A dedicated `SeekEQ` fast-path is a possible future optimization (SQLite has `SeekRowid`); defer until measurement justifies it.

Phase 1 ships only the unbounded `Rewind`/`Next` path. Phase 2 adds the `Seek*`/`Idx*` opcodes and a `cc_iter` planner that picks the bounded path when the `WHERE` clause constrains a leading-PK prefix; otherwise it falls back to the Phase 1 path with a residual `IfNot` filter inside the loop.

---

## 9. Read-your-own-writes

When you `INSERT` then `SELECT` inside the same transaction, the SELECT must see the INSERT. Two common designs:

### 10.1 Merge cursor

A cursor that overlays two sorted streams: the in-memory staging map and the LMDB cursor. On each `next()`, peek both, yield the smaller, advance. Dedup tombstones cross-layer.

Correct, but adds significant complexity to the cursor state machine — which is already non-trivial because of MVCC. And our VM doesn't currently emit programs that read-after-write inside a single statement.

### 10.2 Buffer-and-flush (chosen)

`WriteTxn` accumulates writes in an in-memory `BTreeMap<Vec<u8>, Vec<u8>>` keyed by `u32_be(table_id) || pk || sk` (no seq suffix). Cursors opened on a `WriteTxn` see *only committed entries* from LMDB — staged writes are invisible until commit. On `commit()`, we walk the staged map, append `u64_be_inv(seq)` to each key, write through to the `data` DB, and update `refs`/`commits` (P3) — all inside one `RwTxn`. Atomic.

Why this is fine for v1:
- The VM's `Vop::Open` followed by `Vop::Insert` in the same program isn't a current pattern.
- INSERT/UPDATE/DELETE statements don't internally need to read the staging buffer.
- When a real workload demands read-your-own-writes (e.g. a multi-statement transaction with reads after writes), we add a merge cursor; the current cursor state machine has the right shape for it.

---

## 10. The storage façade

### 10.1 Goal: keep the VM dumb

The VM should know nothing about LMDB types, key encoding, or transactions-as-LMDB-objects. It just calls `txn.open_cursor("users")` and gets something with `rewind/next/curr`.

The current `Connection` is the right boundary, but its API needs to grow:

```rust
// before (current)
conn.open_cursor(table) -> Cursor              // Cursor backed by Vec<Row>

// after
conn.begin_read() / conn.begin_write()         // returns ReadTxn / WriteTxn
txn.open_cursor(table) -> StorageCursor        // borrows from txn
txn.put_row(table, value) / txn.commit() / ...
```

### 10.2 Lifetime story

The VM owns its txn for its lifetime:

```rust
pub struct VM<'a> {
    txn: TxnHandle<'a>,             // Read | Write | None
    cursors: Vec<StorageCursor<'a>>, // borrows from txn
    // ...
}

enum TxnHandle<'a> {
    None,
    Read(ReadTxn<'a>),
    Write(WriteTxn<'a>),
}
```

`Vop::Init` opens the txn; `Vop::Commit` finalizes it; `Vop::Open` registers a cursor against the active txn. `'a` is the lifetime of the underlying `heed::Env`, which is held by the `Connection` (and is `Arc`-cloneable so the borrow is cheap).

### 10.3 Read vs write txn — who decides?

At program-compile time, the compiler classifies the program as read-only (only `Open`/`Rewind`/`Next`/`Load`/`Return`/`Seek*`/`Idx*` etc.) or write (any `Insert`/`InsertBatch`/`CreateTable`/`Drop`/`Clear`). It emits the right txn flavor on `Vop::Init`. This decision is local to `cc_program` and adds one bit to the program prelude. The Phase 2 `Seek*`/`Idx*` opcodes are read-only — they don't flip the program to write mode.

### 10.4 Implicit single-statement transactions

Most RQL programs today are a single statement (one CREATE, one INSERT, one SELECT). Wrapping each statement in an automatic txn — `Init`-as-`begin`, `Exit`/`Return`-as-`commit` — is what the existing Vop set already implies. We preserve that. Explicit multi-statement txns (with a real `transaction;` ... `commit;` block) come later, gated on grammar work.

---

## 11. LMDB gotchas

A list of things that bite people, mapped to our mitigations.

| Gotcha | Mitigation |
|---|---|
| `map_size` is a hard cap; exceeding it returns `MDB_MAP_FULL`. | Default 1 GiB, configurable, document. Future: auto-grow on full by reopening with larger. |
| `max_dbs` is fixed at env open. | Set to 8 (room to grow); we never create per-table DBs, so 8 is plenty. |
| Default key length cap is 511 bytes. | Enforce `pk_len + sk_len + 16 ≤ 511` in `put_row`; surface `KeyTooLong`. |
| Long-lived read txns block page reuse, file grows. | Document; v1 has only short txns. Future: snapshot-staleness timeout. |
| Single concurrent writer. | Document. Acceptable for embedded use. |
| Returned slices borrow from the txn — using them after the txn drops is UB. | Heed's lifetimes prevent this at compile time. We pay the borrow-check tax willingly. |
| Tombstones never auto-GC. | Defer to manual `compact()` in a later milestone. Reads stay correct. |
| `write_txn().commit()` failure (out-of-memory mapping, etc.) is propagated as `heed::Error`. | Map to our own `Error::Storage(...)` via `From<heed::Error>`; never `unwrap()`. |
| LMDB on macOS by default uses a directory layout (`data.mdb` + `lock.mdb`). | Use `EnvFlags::NO_SUB_DIR` so the user's `path` is a single file. |

---

## 12. Worked example: a full request

Let's trace `create table points; insert into points (1); select * from points;` on Phase 1.

### 12.1 `create table points;`

1. Parser produces `ir::Statement::Create(Create::Table(Table { name: "points", keys: vec![] }))`.
2. Compiler emits `[Init(write), CreateTable { table }, Commit, Exit]`.
3. VM:
   - `Init(write)` → `conn.begin_write()` opens an LMDB `RwTxn`, stashes it on the VM as `TxnHandle::Write`.
   - `CreateTable` → `txn.create_table(schema)` allocates a new `table_id` from `meta::next_table_id` (say 7), writes `meta[b"schema/points"] = bincode(TableSchema { name: "points", table_id: 7, keys: vec![] })`, increments the counter.
   - `Commit` → `txn.commit()` flushes (no staged data writes; just the meta updates) and wraps `RwTxn.commit()`.
4. Returns to the REPL: 0 rows.

### 12.2 `insert into points (1);`

1. Parser: `Insert { target: "points", source: [Expr::Lit(1)] }`.
2. Compiler: `[Init(write), Push(1), Insert("points"), Commit, Exit]`.
3. VM:
   - `Init(write)` → opens RwTxn.
   - `Push(1)` → stack is `[1]`.
   - `Insert("points")` → `pop()` gets `1`; calls `txn.put_row("points", Value::number(1))`:
     - Look up table_id (7) and schema in the catalog snapshot.
     - PK extraction: schema has `keys = []` → allocate surrogate row_id from `meta[b"row_seq/" || u32_be(7)]`. Say `row_id = 1`.
     - Build partial key: `u32_be(7) || u64_be(1)` → `00 00 00 07 00 00 00 00 00 00 00 01`.
     - Encode value: `[0x00] || serde_json::to_vec(&Value::number(1))` → `00 31` (tag byte, then ASCII `1`).
     - Stage in `WriteTxn::staged`.
   - `Commit` → for each staged entry, append the 8-byte zero suffix to the key, write to `data` DB. Wrap `RwTxn.commit()`. The file's root pointer atomically swings to include the new entries.
4. Returns: 0 rows (INSERT yields no rows).

### 12.3 `select * from points;`

1. Parser: a Select with `from points as points`, no where, no fetch.
2. Compiler: roughly `[Init(read), Open("points"), Rewind(c=0, jmp=after_loop), Load(c=0), <project *>, Return(0), Next(c=0, jmp=loop_top), <after_loop>, Exit]`.
3. VM:
   - `Init(read)` → `conn.begin_read()` opens `RoTxn`.
   - `Open("points")` → `txn.open_cursor("points")` builds a `StorageCursor`:
     - Look up table_id = 7 in the catalog snapshot.
     - Construct `inner = data_db.iter(&ro)` positioned at `u32_be(7)`.
     - `state = BeforeStart`.
     - Pushed onto `vm.cursors[0]`.
   - `Rewind(0, jmp)`:
     - Calls `cursor.rewind()` which positions at `[00 00 00 07 ...]` and runs `advance()`.
     - First entry: key `00 00 00 07 00 00 00 00 00 00 00 01 00 00 00 00 00 00 00 00`, value `[00] [31]`.
     - `k.starts_with(table_prefix)` → true. `v[0] == TAG_LIVE`. Decode `Value` from `[31]` → `1`. Set `curr_row = Row { oid: 1, val: 1 }`. Step inner forward. Return true.
     - `Rewind` returns true → don't jump.
   - `Load(0)` → push `cursor.curr().val.clone()` = `1` onto stack.
   - `Return(0)` → emit row to caller; yield `1`.
   - `Next(0, jmp)`:
     - Calls `cursor.next()` which calls `advance()`.
     - inner cursor was advanced past the first entry. Now sees end-of-table (next key not under `00 00 00 07`) → returns false.
     - `Next` falls through (no jump back to loop top).
   - `<after_loop>` → `Exit`.
4. REPL sees one row: `1`. Done.

### 12.4 What changes in Phase 2

The table is declared with key columns: `create table points (x int, y int);`. `put_row(value)` extracts `x` and `y` from the row JSON, type-checks them, encodes them via `keycodec::encode_int` in declaration order, and uses the result as the PK components. The full key becomes `u32_be(7) || enc(x) || enc(y) || 00 00 00 00 00 00 00 00`.

The planner reads the `WHERE` clause and emits the `Seek*`/`Idx*` pair (§8.5):

- `where x = 1 and y = 2` → `Open` + `Push(1); Push(2); SeekGE(c=0, ncols=2)` + (loop top) `Push(1); Push(2); IdxGT(c=0, ncols=2)` + body.
- `where x = 1` (single leading column, no constraint on `y`) → same shape with `ncols=1`. The cursor encodes the partial tuple in prefix form (`enc(1) || 00 00` for the `SeekGE` lower, `enc(1) || 00 01` for the `IdxGE`-style upper successor).
- `where y = 2` (no constraint on the leading column) — *cannot* be expressed as a contiguous range in this PK order, so it compiles to `Rewind`/`Next` with a residual `IfNot` filter inside the loop, exactly like Phase 1.
- `where x >= 1 and x < 5 and other_field = 'foo'` — the `x` range becomes the `SeekGE`/`IdxGE` pair; `other_field = 'foo'` is the residual, evaluated as an `IfNot` after `IdxGE` and before `Load`/`Return`.

Anything that doesn't pin a leading PK column compiles to `Rewind`/`Next` with the predicate as a residual filter — exactly the Phase 1 path, untouched.

---

## 13. Glossary

- **B+ tree**: balanced search tree where all data lives in leaf pages, internal pages hold routing keys; supports ordered range scans efficiently.
- **mmap**: a kernel facility that maps a file's bytes into the process's virtual address space; reads/writes touch the file via page faults rather than syscalls.
- **MVCC** (multi-version concurrency control): readers see a snapshot, writers create new versions; readers and writers don't block each other.
- **Copy-on-write (CoW)**: never overwrite a page; allocate a new one. Old pages stay live as long as something refers to them.
- **Lexicographic order**: byte-by-byte left-to-right comparison.
- **Sort-order preserving**: an encoding where logical order matches byte order.
- **Composite key**: a tuple of values encoded into a single byte string, with structure (terminator or length-prefix) so the tuple boundaries are recoverable and order is preserved.
- **Reserved suffix**: the 8 trailing bytes on every data key, currently zero, set aside for the future versioning system (Appendix A).
- **Read txn / write txn**: LMDB transactions; reads are snapshot-isolated; one write txn at a time.
- **Named DB**: one of multiple B+ trees inside a single LMDB env, sharing one transaction system.
- **Surrogate key**: a system-generated u64 used as the primary key when the user didn't declare one.
- **Heed**: the Rust LMDB binding we use; provides typed Database, RoTxn, RwTxn, and cursor APIs over LMDB's C library.

---

## Further reading

- LMDB design paper: Howard Chu, "LMDB: A Memory-Mapped Database and Backend for OpenLDAP" — read for the page CoW and meta-page swing details.
- FoundationDB tuple layer documentation — the canonical reference for the 0x00-terminator-with-byte-stuffing key encoding we use.
- BigTable paper (Chang et al., 2006) — origin of the keyed B+ tree as a database substrate.
- The `heed` crate docs on docs.rs — small, well-documented; spend an hour reading top to bottom before implementing.

---

## Appendix A: Future versioning and branching

This appendix documents the future versioning system that the current data layout reserves space for. None of it is implemented; none of it is required to ship Phases 1 and 2. It is here so that when you are ready to build it, the design is already chosen and the existing data does not have to migrate.

### A.1 The model

- A **commit** is identified by a monotonically-allocated `u64` called `commit_id`. We use `commit_id` and `commit_seq` interchangeably.
- A **branch** is a named pointer to a commit. `b"branch/main" → 7` means the HEAD of `main` is commit 7.
- The **`commits` DB** stores `commit_id → bincode { parent: Option<CommitId>, message, timestamp, seq }`.
- A **commit's ancestry** is the transitive set of commits reachable by walking `parent` pointers from it. Linear history → `{1, 2, ..., commit_id}`. Branched history → a subset of that range.

### A.2 How branching works

- Creating a branch from another branch copies the source's HEAD commit_id into the new branch entry. **O(1).**
- Committing on a branch allocates `next_commit_id`, writes a `CommitMeta` with `parent = current_HEAD`, and sets the branch's `current_HEAD = new_commit_id`. The commit's *only* contents are the writes the txn made, encoded into `data` with the new commit_id stamped into the trailing 8 bytes (formerly all zero).
- LMDB's page-level copy-on-write means branching does not duplicate row data. Old rows from the parent branch remain visible to the new branch via shared physical storage. Each branch has its own *logical view* (defined by its ancestry), not its own physical copy.

### A.3 Activating the reserved bytes

The trailing 8 bytes of every data key, currently zero, become `u64_be_inverted(commit_seq)` — newest commit sorts first within a row's cluster. The leading byte of every value, currently `0x00` for live rows, gets a new value `0x01` for tombstones.

Existing P1/P2 rows carry trailing zero bytes, which decode as `commit_seq = !0u64 = 0xFFFF...FF`? No — they decode as `!0 = u64::MAX`. To make pre-existing rows visible at the bootstrap commit, the migration is one of:

1. **Bootstrap commit gets seq = `u64::MAX`.** The implicit "root commit" is allocated this seq value; existing data already encodes it. Real commits start at `commit_id = 1`, but their stamped seq is `u64::MAX - commit_id` so they sort newer than the root. Slightly weird arithmetic but no migration.
2. **Migration pass.** A one-time read-and-rewrite walks every row, peels off the zero suffix, and re-writes with a real seq. Avoidable; option 1 is cleaner.

Pick option 1 when the time comes.

### A.4 Reading at a branch

`MonaDB::begin_read(Branch("foo"))`:

1. Resolve `foo` → commit_id `H`.
2. Walk parent pointers in `commits` DB starting from `H`, collecting all visited commit_ids into a `HashSet<u64>` — the **ancestor set**.
3. Stash the ancestor set on the read txn.

A row's cluster (the run of keys sharing the entire `[u32_be(table_id), enc(c_1), ..., enc(c_N)]` prefix) may now have many versions in `data`. The cursor's visibility check is: **iterate versions newest-first; the first one whose commit_seq is in the ancestor set is the visible version (or a tombstone, in which case the row is hidden).**

Why "first" and not "any": commit_seqs are monotonically allocated globally, but two branches can write to the same row interleaved. Within an ancestor set, the newest visible write wins. Versions with seq in the ancestor set form a totally-ordered chain (since the ancestor set is itself totally ordered by parent walk in the absence of merges).

### A.5 The cursor MVCC walk

**Layering rule, before any code.** The MVCC version walk lives *under* the `Seek*`/`Idx*` opcode surface (§8.5). `seek_ge` positions the inner LMDB cursor on a key in raw byte order, *then* the cursor's MVCC walk advances forward inside that position skipping older versions, tombstones, and out-of-ancestor entries until it lands on a visible row. The opcode-level upper-bound check (`IdxGT` etc.) runs against the *visible* row's key — i.e. the key the cursor reports via `curr()` — not whatever raw LMDB key the inner cursor happens to be parked on between visibility hops. Pushing the bound check down into the version-skipping inner loop is the classic trap; don't.

The non-versioned `advance()` from §8.4 grows two extra concerns: cluster dedup (skip older versions of the row we just yielded) and visibility check (find the newest version in our ancestry).

```text
loop {
    let (k, v) = inner.current()? else { exhausted; return false };

    if !k.starts_with(&table_prefix) { exhausted; return false }
    // No upper-bound check here — that's an Idx* opcode's job (§8.5, A.5 layering rule).

    let body = &k[4 .. k.len()-8];
    let seq  = !u64::from_be_bytes(k[k.len()-8..].try_into().unwrap());

    if body == last_key_body.as_slice() {
        // Older version of the row we just yielded. Skip rest of cluster.
        let next_seek = byte_successor(table_prefix.iter().chain(body).chain(b"\x00\x01"));
        inner.move_on_key_greater_than_or_equal_to(&next_seek);
        continue;
    }

    last_key_body = body.to_vec();

    if !ancestors.contains(&seq) {
        // Newest version invisible; walk forward inside cluster looking for an older visible one.
        if let Some(vv) = find_visible_version_in_cluster(body)? {
            if vv[0] == TAG_TOMB { advance_past_cluster()?; continue; }
            curr_row = Some(decode_row(&vv[1..])?);
            state = Positioned;
            inner.move_on_next();
            return true;
        }
        advance_past_cluster()?;
        continue;
    }

    if v[0] == TAG_TOMB { advance_past_cluster()?; continue; }

    curr_row = Some(decode_row(&v[1..])?);
    state = Positioned;
    inner.move_on_next();
    return true;
}
```

The dedup key is the entire `last_key_body`, not just the leading PK — two rows whose PKs share their first N-1 components but differ in the Nth are distinct logical rows.

### A.6 Writes

`MonaDB::begin_write(branch)` reads the branch's HEAD into `parent_commit`, computes the parent's ancestor set (same logic as a read txn), and stages writes in-memory keyed by `[table_prefix, enc(c_1), ..., enc(c_N)]` — no seq suffix yet.

`commit(message)`:

1. Inside the wrapped `RwTxn`, allocate `commit_id = next_commit_id` (read+write `meta::next_commit_id`).
2. For each `(partial_key, tagged_value)` in `staged`, append `u64_be_inv(commit_id)` and `data.put(full_key, value)`.
3. Write `commits.put(commit_id, CommitMeta { parent: Some(parent_commit), message, ... })`.
4. Update `refs.put(b"branch/" + branch_name, commit_id)` (and `b"HEAD"` if the session points at this branch).
5. `rw.commit()`.

Atomic across all five writes by virtue of being one LMDB txn.

### A.7 Tombstone semantics across branches

A tombstone is just another write at a specific commit_seq. If branch A deletes row R at commit 10, branch B forked from commit 5 still sees R (commit 10 is not in B's ancestry). If C is a child of A, C sees the deletion (commit 10 is in C's ancestry). The tombstone tag tells the cursor "this row's most recent visible state is deleted; skip it."

### A.8 Garbage collection

Tombstones and old versions accumulate. Reads stay correct (the cursor walks through them) but storage and read latency grow. A future `MonaDB::compact()`:

1. Compute the union of ancestor sets across all current branch HEADs (the "live" commit set).
2. For each row cluster, walk versions and drop any whose seq is not in the live set, except keep the newest visible per branch HEAD.
3. Run as a single stop-the-world write txn.

Correctness does not require it; performance eventually does.

### A.9 Surface

DDL statements added to RQL when this lands:

```
create branch <name> from <branch_name | commit <id>>;
list branches;
commit [<message_string>];
rollback;
```

Plus a generic session-state command `set branch <name>` that mutates `Connection::current_branch`. Subsequent reads and writes target that branch.

### A.10 Glossary additions

When this appendix activates, add to the main glossary:

- **MVCC** (multi-version concurrency control): readers see a snapshot, writers create new versions; readers and writers don't block each other.
- **Tombstone**: a marker that says "this row is deleted at this commit"; needed because we can't simply remove rows when older readers might still need to see them.
- **Commit seq**: a monotonically allocated `u64` per write; appears (inverted) in the trailing 8 bytes of every data key.
- **Ancestor set**: the set of commit seqs reachable by walking parent pointers from a given commit; defines what a reader at that commit can see.
