+++
title = "Documents"
description = "What can go into a value, how it is stored, and where BSON shows through."
template = "docs/page.html"
weight = 2
+++

A value must be a mapping — a `dict`, a dataclass instance, or a model. BSON has
no other top-level form, and neither does MonaDB.

```python
c["k"] = {"n": 1}               # fine
c["k"] = [1, 2, 3]              # TypeError: document must be a mapping
c["k"] = "not a mapping"        # TypeError
```

Lists and scalars are welcome *inside* a document, just not as the whole of one.

## Types

| Python | BSON | Notes |
|--------|------|-------|
| `None` | Null | |
| `bool` | Boolean | |
| `int` | Int32 / Int64 | narrowed automatically; beyond Int64 raises `ValueError` |
| `float` | Double | |
| `str` | String | |
| `bytes` | Binary | generic subtype |
| `datetime` | UTC datetime | millisecond precision — see below |
| `list` | Array | |
| `dict` | Document | nests to any depth |

Anything else raises `TypeError`, and the message names where in the document
the offending value sits:

```python
c["k"] = {"a": {"b": [1, {1, 2}]}}
# TypeError: unsupported type set at $.a.b[1]
```

## Nesting

Documents nest freely, and round-trip exactly:

```python
c["k"] = {
    "user": {"name": "alice", "tags": ["a", "b"]},
    "counts": [1, 2, [3, {"deep": True}]],
    "blob": b"\x00\xff",
}
c["k"] == ...                   # identical structure back
```

## Datetimes

Two BSON properties show through, and it is better to know them than to discover
them:

**Millisecond precision.** Microseconds are truncated.

```python
from datetime import datetime, timezone

c["k"] = {"at": datetime(2026, 8, 2, 12, 0, 0, 123456, tzinfo=timezone.utc)}
c["k"]["at"].microsecond        # 123000, not 123456
```

**Naive datetimes are treated as UTC.** Reads always return tz-aware UTC.

```python
c["k"] = {"at": datetime(2026, 8, 2, 12, 0, 0)}     # naive
c["k"]["at"]
# datetime.datetime(2026, 8, 2, 12, 0, tzinfo=datetime.timezone.utc)
```

If you care about a local wall-clock time, attach the zone yourself before
storing it.

## Integers

Integers are stored as Int32 when they fit and Int64 otherwise. That is a
storage detail — you always read back a Python `int`. Beyond 64 bits it is an
error, not a silent truncation:

```python
c["k"] = {"n": 2**63}           # ValueError: int out of 64-bit range at $.n
```

## Why BSON

BSON keeps `datetime` and `bytes` as first-class types, which JSON cannot, and
its files are readable from any language with a BSON library. The alternative
considered was msgpack, which is smaller but weaker on exactly those two types.
