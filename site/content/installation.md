+++
title = "Installation"
description = "Install MonaDB for Python or the mona CLI shell."
template = "docs/page.html"
weight = 1
+++

MonaDB ships as a Python package, a Rust library, and an interactive CLI (`mona`).
There is no separate server to install or configure.

## Python (PyPI)

```sh
pip install monadb
```

```python
>>> import monadb
>>> monadb.connect()
```

Requires Python 3.9 or later.

## CLI shell (`mona`)

The Homebrew formula is named **monadb**; the command it installs is **mona**.

### Homebrew

```sh
brew tap rchowell/tap
brew install monadb
mona
```

### cargo

```sh
cargo install monadb --features cli
mona
```

### GitHub Releases

Download a prebuilt `mona` tarball for your platform from
[GitHub Releases](https://github.com/rchowell/MonaDB/releases), extract, and run `./mona`.

```sh
mona              # in-memory database
mona ./data.db    # open or create a file
```

Inside the REPL, dot-commands start with `.` (`.info`, `.debug`, `.exit`).
SQL statements end with `;`.

## Rust library (crates.io)

```sh
cargo add monadb
```

## From source

Clone the repository and build with [maturin](https://www.maturin.rs/) (Python) or Cargo (CLI):

```sh
git clone https://github.com/rchowell/MonaDB.git
cd MonaDB

# Python extension
uv run maturin develop

# CLI shell
cargo run --features cli --bin mona
```

## Requirements

- Python 3.9+ (for `pip install monadb`)
- Rust 1.85+ (when building from source)
- A filesystem path for persistence, or omit the path for an in-memory database

Next: walk through a first query in the [Tutorial](@/tutorial.md).
