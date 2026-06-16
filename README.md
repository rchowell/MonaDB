# MonaDB

An embedded document database with a small SQL dialect. Use it from Python, Rust, or the interactive `monadb` shell.

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

### CLI shell (`monadb`)

**Homebrew**

```sh
brew tap rchowell/tap
brew install monadb
monadb
```

**cargo**

```sh
cargo install monadb --features cli
monadb
```

**GitHub Releases** — download a prebuilt `monadb` binary for your platform from
[Releases](https://github.com/rchowell/MonaDB/releases).

### Rust (crates.io)

```sh
cargo add monadb
```

```rust
use monadb::MonaDB;

let mut db = MonaDB::open("app.db")?;
db.execute("create table t (id int);")?;
```

## `monadb` REPL

```sh
monadb              # in-memory database
monadb ./data.db    # open or create a file
```

Dot-commands inside the shell:

| Command  | Action                |
|----------|-----------------------|
| `.info`  | List catalog tables   |
| `.debug` | Toggle bytecode trace |
| `.exit`  | Quit                  |

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
