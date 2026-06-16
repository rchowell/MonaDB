# MonaDB

An embedded document database with a small SQL dialect, compiled to stack-based bytecode and stored in LMDB.

No server, no config — use it from Python, Rust, or the interactive `mona` shell.

## Install

### Python (PyPI)

```sh
pip install monadb
```

```python
import monadb

con = monadb.connect()          # in-memory
con = monadb.connect("app.db")  # file-backed
con.execute("create table todos (id int);")
```

### CLI shell

**Homebrew** (installs the `mona` command):

```sh
brew tap rchowell/tap
brew install monadb
mona
```

**cargo** (builds from source):

```sh
cargo install monadb --features cli
mona
```

**GitHub Releases** — download a prebuilt `mona` binary for your platform from
[Releases](https://github.com/rchowell/MonaDB/releases).

### Rust library (crates.io)

```sh
cargo add monadb
```

```rust
use monadb::MonaDB;

let mut db = MonaDB::open("app.db")?;
db.execute("create table t (id int);")?;
```

Install the REPL via Cargo:

```sh
cargo install monadb --features cli
```

## `mona` REPL

```sh
mona              # in-memory database
mona ./data.db    # open or create a file
```

Dot-commands inside the shell:

| Command | Action |
|---------|--------|
| `.info` | List catalog tables |
| `.debug` | Toggle bytecode trace |
| `.exit` | Quit |

SQL statements end with `;`. The prompt supports syntax highlighting and multiline input.

## Requirements

- **Python**: 3.9+
- **Rust** (from source): 1.85+ (edition 2024)
- **CLI**: terminal with readline support

## Documentation

- [Installation](https://github.com/rchowell/MonaDB/blob/main/site/content/installation.md)
- [Language reference](https://github.com/rchowell/MonaDB/tree/main/site/content/language)
- [Examples](https://github.com/rchowell/MonaDB/tree/main/site/content/examples)

## License

Apache-2.0 — see [LICENSE](LICENSE).
