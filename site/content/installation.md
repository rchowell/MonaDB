+++
title = "Installation"
description = "Install MonaDB from PyPI, or build it from source."
template = "docs/page.html"
weight = 1
+++

MonaDB ships as a single Python package. There is no server to install and no
configuration to write.

## From PyPI

```sh
pip install monadb
```

Wheels are published for Linux (x86_64, aarch64), macOS (Apple silicon and
Intel), and Windows (x64). They are `abi3` wheels, so one wheel per platform
covers every supported Python.

## Requirements

Python 3.9 or newer. Nothing else — the Rust core is compiled into the wheel.

## Verifying

```python
import monadb

db = monadb.open()          # in-memory
db["t"]["k"] = {"ok": True}
assert db["t"]["k"] == {"ok": True}
```

## From source

Building from source needs Rust 1.85 or newer, for edition 2024.

```sh
git clone https://github.com/rchowell/MonaDB
cd MonaDB
pip install maturin
maturin develop
python -m pytest tests -q
```

## Earlier versions

MonaDB 0.1 was a different program: an embedded database with a SQL dialect,
built on LMDB. It is preserved under the `v0.1.0-sql` git tag and remains
installable:

```sh
pip install "monadb<0.2"
```

Databases written by 0.1 cannot be read by 0.2 — both the storage engine and the
document format changed.
