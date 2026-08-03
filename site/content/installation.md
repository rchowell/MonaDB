+++
title = "Installation"
description = "Install MonaDB from PyPI, or build it from source."
template = "docs/page.html"
weight = 1
+++

MonaDB ships as a single Python package. There is no server to install and no
configuration to write. You can install from PyPi like so:

## From PyPI

**pip**

```sh
pip install monadb
```

**uv**

```sh
uv add monadb
```

## Requirements

Python 3.9+.

## Verifying

```python
import monadb

db = monadb.open()

items = db["items"]
items["test"] = {"ok": True}

assert items["test"] == {"ok": True}
```
