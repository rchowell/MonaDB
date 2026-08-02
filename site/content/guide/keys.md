+++
title = "Keys"
description = "Key types, ordering, and the range and prefix scans that ordering buys."
template = "docs/page.html"
weight = 1
+++

A key is a `str`, an `int`, a `bytes`, or a tuple of those. Keys are stored in an
order-preserving encoding, which is what makes iteration sorted and range scans
possible.

## Types

```python
users["alice"] = {...}          # str
events[1699000000] = {...}      # int, full 64-bit range
blobs[b"\x01\x02"] = {...}      # bytes
edges[("a", "b")] = {...}       # tuple of the above
```

### What is rejected

| Key | Result | Why |
|-----|--------|-----|
| `1.5` | `TypeError` | float ordering has NaN and `-0.0` hazards |
| `True` | `TypeError` | `bool` is an `int` subclass; it would alias `1` |
| `None` | `TypeError` | no meaningful position in the ordering |
| `(1, (2, 3))` | `TypeError` | tuples are flat, not nested |
| `2**63` | `ValueError` | outside the 64-bit range |

Rejection happens at write time, so a bad key never reaches storage.

## Ordering

Components sort by type first, then by value: **int < str < bytes**. Strings
compare by their UTF-8 bytes, and tuples compare componentwise.

```python
c = db["c"]
for k in [b"q", "z", 5, -1, ("t", 1)]:
    c[k] = {}

list(c)
# [-1, 5, ('t', 1), 'z', b'q']
```

`('t', 1)` precedes `'z'` because the comparison reaches the first component
before anything else: `'t' < 'z'`. Tuple keys are not a separate category that
sorts after scalars — they interleave, component by component.

A shorter key sorts before any key extending it:

```python
[("a",), ("a", 1), ("a", 2), ("b",)]    # already in order
```

## A scalar is a 1-tuple

`"a"` and `("a",)` are the **same key**. The encoding treats a scalar as a
one-component tuple, so there is a single code path and no ambiguity:

```python
c[("a",)] = {"n": 1}
c["a"]                          # {'n': 1}
list(c)                         # ['a'] — decodes to the scalar
```

## Ranges

`range(start, stop)` is half-open — `start` included, `stop` excluded — and
either bound may be `None` for unbounded:

```python
c.range("a", "m")               # 'a' <= key < 'm'
c.range(None, "m")              # everything below 'm'
c.range("a", None)              # everything from 'a' up
```

Both bounds must be the same key type; mixing them raises `TypeError`.

## Prefixes

`prefix(p)` means two different things depending on what you pass, and both are
useful:

```python
logs.prefix("2026-08")          # str: keys of that type beginning with it
blobs.prefix(b"\x01")           # bytes: likewise
edges.prefix(("a",))            # tuple: keys whose leading components match
```

A string or bytes prefix matches on characters. A tuple prefix matches on whole
components — `("a",)` matches `("a", 1)` and `("a", "b")`, but never `("ab",)`.

## Iteration order is not insertion order

This is the sharpest departure from `dict`. If you need insertion order, store a
sequence number in the key:

```python
events[(seq, "created")] = {...}
```
