+++
title = "Documents"
description = "What can go into a value, how it is stored, and where BSON shows through."
template = "docs/page.html"
weight = 2
+++

All documents are mappings such as python dicts, dataclasses, or pyndantic models.

```python
c["k"] = {"n": 1}               # fine
c["k"] = [1, 2, 3]              # TypeError: document must be a mapping
c["k"] = "not a mapping"        # TypeError
```

## Types

| Python     | BSON          | Notes                                                    |
| ---------- | ------------- | -------------------------------------------------------- |
| `None`     | Null          |                                                          |
| `bool`     | Boolean       |                                                          |
| `int`      | Int32 / Int64 | narrowed automatically; beyond Int64 raises `ValueError` |
| `float`    | Double        |                                                          |
| `str`      | String        |                                                          |
| `bytes`    | Binary        | generic subtype                                          |
| `datetime` | UTC datetime  | millisecond precision (see below)                        |
| `list`     | Array         |                                                          |
| `dict`     | Document      | nests to any depth                                       |

Anything else raises `TypeError`, and the message names where in the document
the offending value sits. This example has a python `set` which is not supported.

```python
c["k"] = {"a": {"b": [1, {1, 2}]}}
# TypeError: unsupported type set at $.a.b[1]
```

## Datetimes

**Millisecond precision.** Microseconds are truncated.

```python
from datetime import datetime, timezone

# Insert a time with microsecond precision
c["times"] = {"at": datetime(2026, 8, 2, 12, 0, 0, 123456, tzinfo=timezone.utc)}

# Returns a time with millisecond precision: 123000, not 123456
c["times"]["at"].microsecond 

# Naive datetimes are treated as UTC
c["naive"] = {"at": datetime(2026, 8, 2, 12, 0, 0)}

# Returns datetime.datetime(2026, 8, 2, 12, 0, tzinfo=datetime.timezone.utc)
c["naive"]["at"]
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
