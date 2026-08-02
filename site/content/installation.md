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

Python 3.9 or newer.

## Verifying

```python
import monadb

db = monadb.open()          # in-memory

# Create a collection
collection = db["t"]

# Insert a document
collection["k"] = {"ok": True}

# Fetch a document
assert collection["k"] == {"ok": True}
```
