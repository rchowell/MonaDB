# Ideas

Hi, this is actually Conner (not Claude). This directoey has Claude-assisted
findings that I want to save but not implement (yet). Claude is not allowed
to touch this file.

I am constantly measuring MonaDB against SQLite (which is very fast) to
identify where I can improve on MonaDB's performance. Claude is excellent
at working backwards (#amzn) from benchmarks to identify where to improve.


## Snapshots & Branching

I want to support snapshots and branching. This pattern appears across databases
like neon and turbopuffer, and table formats like iceberg. It is highly feasible
with my storage layer, and read snapshots would be a perf improvement for bursts
of point lookups.

Today's point lookups each open their own read transaction which acquires a slot
in LMDB's reader lock table. These slots are NOT thread-cached because I use the
without-tls mode. The acquired slot is free'd when the read transaction is
committed.

The impact is that every point lookup has redundant transaction maintanence
overhead. I can amortize this with snapshots, which will help me close the gap
on sqlite. Once again, sqlite is so good. My connection will maintain an open
read transaction, the snapshot, and use this across point lookups. Writes
invalidate the snapshot, and the next read opens the next snapshot.

Building this should (1) improve performance on bursts of point lookups and (2)
create snapshotting mechanisms that would become a customer feature.


## Python API

The python api was one-shotted from the [python.md](../references/python.md)
document, which was great to *feel* the API, but is not acceptable to release.
Some personal code review notes which I can capture in a python skill at a later
time:

- Remove the fetch* apis from connection, use the new row type
- Disable init, use internal constructors with __new__
- Strict usage of type hints and proper type hint enforcement (ruff,mypy)


## Value Representation & Storage

The prototype value representation (value.rs) was `serde_json::Value` which was
easy for me to implement at the time, but performed poorly due to (1) copying
the btree value's bytes and (2) allocating an expensive value tree for every
value. I unfortunately need to collect historical metrics to attach a number to
this. Finally, all values *were* stored as json bytes rather than bson or some
other specialized format.

The current value representation now uses a flat `[u8]` jsonb-inspired
representation that avoids allocating a value tree, but still copies the btree
value's bytes. I want to use borrowed values from the mmap, which is possible
because the borrow is valid for the lifetime of the read transaction.

The stored representation is ok, but needs to be benchmarked against alternatives
such as bson, cbor, sqlite's json, postgres jsonb (exact), and possibly others.
This will be a fun topic to research and write about. I will benchmark on
encode, decode, and path navigation. This is very important to lock down for
compatibility across MonaDB versions.

Today the `Value` type needs a lot of cleanup. I want to remove the 'raw' verbiage
with 'reference' since these are references into the mmap which we can work with.
The cast/to_t/as_s has too many sources of truth and extra methods; these will be
reduced to:

- as_bool()
- as_int()
- as_float()
- as_string()
- as_array()
- as_object()

All casts and coercions will use these methods. I will also do my best to remove
any non-json types like oid and bytes which interestingly are not value types
but are *key* types. However, the vm still needs to work with these types on the
stack because of key encoding. This will require some deliberation.


## Benchmarks

The current benchmarks are not standardized on a process, and I recently noticed
thermal throttling skewing results. I should add a 60s cooldown between passes.
I will need to research benchmarking best practices on a macbook, like how to
pin a core and stop/pause as much background work as possible. I can also see
how the system responds using these samplers. Benchmarks also need to (1) use
short passes, (2) swap suite order and (3) record min times.

```bash
# Show thermal activity every 1s
sudo powermetrics --samplers thermal -i 1000 | grep "Current pressure level:" 

# Show cpu frequencies every 1s
sudo powermetrics --samplers cpu_power -i 1000 | grep -E "CPU \d frequency:"    

# Show if any thermal or perf warnings have been logged
pmset -g thermlog
```
