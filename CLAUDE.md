# MonaDB

Embedded database with a custom SQL query language compiled to stack-based bytecode.

## Architecture

```
SQL text → Lexer (logos) → Parser (lalrpop) → IR → Compiler → Vop bytecode → VM → LMDB
```

## Key Files

| File                 | Role                                                               |
|----------------------|--------------------------------------------------------------------|
| `src/lexer.rs`       | Token definitions — logos DFA lexer, produces spanned token stream |
| `src/parser.lalrpop` | Grammar — LALRPOP LR(1) parser, calls action functions in `ir.rs`  |
| `src/ir.rs`          | AST/IR types + parser action functions (called from grammar rules) |
| `src/compiler.rs`    | IR → Vop bytecode (`cc_*` methods, `emit_*` helpers)               |
| `src/vm.rs`          | Stack-based bytecode interpreter — `next()` loop over `Vop`        |
| `src/functions.rs`   | Builtin scalar standard library — flat `fn(&[Value])` registry      |
| `src/error.rs`       | Error enum + `error!` / `unsupported!` macros                      |

## Build & Test

```sh
cargo build   # regenerates src/parser.rs from src/parser.lalrpop via build.rs
cargo test
cargo run --features cli --bin monadb   # starts the REPL
```

## Release

See [RELEASING.md](RELEASING.md). Summary:

- Version lives in `Cargo.toml`; PyPI reads it via maturin `dynamic = ["version"]`.
- Tag `vX.Y.Z` triggers `.github/workflows/release.yml` (crates.io, PyPI, GitHub CLI assets).
- Homebrew: bump [`Formula/monadb.rb`](https://github.com/rchowell/homebrew-tap/blob/main/Formula/monadb.rb) in `rchowell/homebrew-tap`.

```sh
cargo publish --dry-run --no-default-features
maturin build --release --features python
```

## Task Management

Use the 'bd' command for task management of work items.

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --status in_progress  # Claim work
bd close <id>         # Complete work
bd sync               # Sync with git
```

## Conventions

### Comment Style
- Every public item (and non-obvious private one) opens with a one-line `///` summary: imperative, present tense, ends with a period — `/// Opens a btree with the given transaction mode.`
- When one line isn't enough: the summary, a blank `///` line, then prose and/or an indented ASCII illustration. Use backticks for identifiers and byte literals, and intra-doc links (`` [`encode_key`] ``) to related items.
- Illustrate layouts, stack effects, and bytecode addresses with ASCII diagrams *inside* the doc comment — byte layouts (`schema.rs`), `stack: … a b → … (a+b)` effects (`vm.rs`), address maps (`compiler.rs`), lifecycles (`cursor.rs`). Aesthetics matter: align the art.
- Keep `//` for in-body step narration, `// SAFETY:` blocks, and field annotations; keep `//-----` section dividers. Don't promote these to `///`.
- Each file opens with a concise `//!` module header stating its role in the pipeline.
- Mechanical/forwarding items stay bare — the `visitor.rs` `visit_*`/`fold_*` forwarders, raw token variants, and one-to-one `emit_*` wrappers. Uniformity shouldn't add noise.

```rust
/// Returns the cursor's current key, or null if unpositioned.
pub fn current_key(&self) -> Value { ... }

/// Adds the top two stack values.
///
///   stack:  … a b  ─▶  … (a + b)
Add,
```

### Naming
- Compiler dispatch methods: `cc_` prefix — `cc_select`, `cc_expr`, `cc_expr_op`, ...
- Compiler emit helpers: `emit_` prefix — `emit_push`, `emit_jpk`, `emit_rewind`, ...
- Type aliases: `ExprRef = Box<Expr>`, `TypeRef = Box<Type>`, `Program = Vec<Vop>`, `Patch = (usize, usize)`, `Obj = Vec<Member>`
- Reserved-word struct fields: trailing underscore — `where_`, `typ_`

### Error Handling
- `error!(...)` — returns `Err(Error::InternalError(...))` early from the current function
- `unsupported!(...)` — returns `Err(Error::Unsupported(...))` for unimplemented features
- Implement `From<T> for Error` for each external error type (see `error.rs`)
- Never use `unwrap()` on user-controlled paths; reserve it for invariants that must hold

### AST / IR Shape
- **Enum** for sum types (alternatives): `Statement`, `Expr`, `Type`, `Fetch`, `Constructor`, `Member`, `Source`, `Selector`, `Segment`
- **Struct** for product types (grouped fields): `Select`, `Insert`, `Create`, `Op`, `Jpk`, `Jpi`, `Jpe`, `Iter`, `Table`
- Recursive types always boxed through a type alias — `ExprRef`, `TypeRef` — never `Box<Expr>` inline
- Parser action functions live in `ir.rs` and are `#[inline]` public functions (not methods)

### Grammar
- External lexer declared in the `extern` block at the top of `parser.lalrpop`
- Grammar rule actions stay thin — construct an IR value via an `ir.rs` function, nothing else
- Operator precedence: `#[precedence(level="N")]` and `#[assoc(side="left")]` annotations on `Expr` alternatives
- Comma-separated repetition: use the `List<T>` macro (defined at the bottom of the grammar file)

### Compiler
- `Compiler` holds `code: Program`, `vars: Vec<Var>`, `counters: usize`
- `cc_*` methods take ownership of their IR node and call `emit_*` helpers to append `Vop` instructions
- Control-flow instructions (`Rewind`, `IfNot`, `Next`, `CntIfPos`, `CntIfZero`) are emitted with placeholder `0` jump targets and back-patched via `patch(pc, dst)` after the loop body is known
- `define(name)` appends a `Var` and its stack depth is its index — `Load(idx)` addresses by index
- `define_counter(n)` allocates a counter slot and emits `CntSet`

## Grammar Extension

Use the `/grammar` skill for a step-by-step guide to adding new grammar constructs.
