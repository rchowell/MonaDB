# MonaDB

Embedded database with a custom SQL-like query language (RQL) compiled to stack-based bytecode.

## Architecture

```
RQL text → Lexer (logos) → Parser (lalrpop) → IR → Compiler → Vop bytecode → VM → Cask
```

## Key Files

| File | Role |
|------|------|
| `src/lexer.rs` | Token definitions — logos DFA lexer, produces spanned token stream |
| `src/parser.lalrpop` | Grammar — LALRPOP LR(1) parser, calls action functions in `ir.rs` |
| `src/ir.rs` | AST/IR types + parser action functions (called from grammar rules) |
| `src/compiler.rs` | IR → Vop bytecode (`cc_*` methods, `emit_*` helpers) |
| `src/vm.rs` | Stack-based bytecode interpreter — `next()` loop over `Vop` |
| `src/value.rs` | JSON value wrapper; operator overloads for arithmetic/comparison |
| `src/cask.rs` | Log-structured persistent key-value store |
| `src/error.rs` | Error enum + `error!` / `unsupported!` macros |
| `src/connection.rs` | Table metadata and cursor management |
| `src/rows.rs` | Query result iterator |

## Build & Test

```sh
cargo build   # regenerates src/parser.rs from src/parser.lalrpop via build.rs
cargo test
cargo run     # starts the REPL
```

## Conventions

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

Use `/grammar` for a step-by-step guide to adding new grammar constructs.
