# Grammar Extension Guide

You are helping extend the MonaDB RQL grammar. The task is: $ARGUMENTS

Work through the four layers below in order. Stop after step 4 — compiler and VM implementation is a separate follow-up task.

---

## Before You Start

Read these three files in full:

- `src/lexer.rs` — the `Token` enum (logos attributes)
- `src/parser.lalrpop` — the full grammar
- `src/ir.rs` — IR types and parser action functions

---

## Step 1 — Token (`src/lexer.rs`)

Only needed if the new construct introduces a keyword or symbol not already in `Token`.

- Add a variant to the `Token` enum with the appropriate `#[token("...")]` attribute
- Keywords go in the `// Keywords` section; symbols in `// Symbols`
- Then register it in the `extern` block of `src/parser.lalrpop` using the same string literal as the `#[token]` value

**Pattern** (keyword example, `lexer.rs:92`):
```rust
#[token("where")]
Where,
```

**Pattern** (extern block entry, `parser.lalrpop:51`):
```
"where" => Token::Where,
```

---

## Step 2 — Grammar Rule (`src/parser.lalrpop`)

Add the LALRPOP production for the new construct.

- Keep rule actions thin: call one `ir::` function and return its result, nothing else
- For comma-separated repetition use `List<T>` (defined at the bottom of the file)
- For optional clauses use `<Foo?>` — produces `Option<Foo>`
- For new operators on `Expr`, add alternatives with `#[precedence(level="N")]` and `#[assoc(side="left")]`
- Wire the new rule into the appropriate parent (`Statement`, `Expr`, `Type`, etc.)

**Pattern** (new statement wired into `Statement`, `parser.lalrpop:76`):
```
Statement: ir::Statement = {
    ...
    MyThing => ir::Statement::MyThing(<>),
};

MyThing: ir::MyThing = "my_keyword" <"ident"> => ir::my_thing(<>);
```

**Pattern** (optional clause, `parser.lalrpop:118`):
```
SelectBlock: ir::Select =
    <From>
    <Where?>
    <Fetch?> => ir::select_block(<>);
```

---

## Step 3 — IR Type (`src/ir.rs`)

Add the data type(s) and a parser action function.

- **New statement** → add a variant to `Statement` enum; add a struct if there are multiple fields
- **New expression variant** → add a variant to `Expr`; add a struct with `ExprRef` fields (never `Box<Expr>` directly)
- **New clause on `Select`** → add a field to `Select` struct, typically `Option<NewClause>`
- **New type variant** → add a variant to `Type` enum

Then write an `#[inline]` public action function that constructs and returns the IR node. It should do nothing except build the value.

**Pattern** (new expression type, `ir.rs:228`):
```rust
#[derive(Debug)]
pub struct Jpk {
    pub inp: ExprRef,
    pub key: String,
}

#[inline]
pub fn expr_jpk(inp: Expr, key: String) -> Expr {
    Expr::Jpk(Jpk { inp: Box::new(inp), key })
}
```

**Pattern** (new statement type, `ir.rs:18`):
```rust
#[derive(Debug)]
pub enum Create {
    Table(Table),
}
// action function:
pub fn table_definition(name: String, schema: Type) -> Table {
    Table { name, schema }
}
```

---

## Step 4 — Stubbed Compiler Method (`src/compiler.rs`)

Add a `cc_*` method that compiles cleanly but is not yet implemented.

- Add the method returning `unsupported!("...")` immediately
- Wire it into the appropriate `match` dispatch arm so the code compiles

**Pattern** (stub for a new statement, `compiler.rs:44`):
```rust
// in Compiler::compile():
Statement::MyThing(x) => self.cc_my_thing(x)?,

// new stub method:
fn cc_my_thing(&mut self, x: MyThing) -> Result<()> {
    unsupported!("my_thing not yet implemented")
}
```

**Pattern** (stub for a new expression variant, `compiler.rs:206`):
```rust
// in cc_expr():
Expr::MyVariant(x) => self.cc_expr_my_variant(x),

// new stub method:
fn cc_expr_my_variant(&mut self, x: MyVariant) -> Result<()> {
    unsupported!("my_variant not yet implemented")
}
```

---

## Verify

```sh
cargo build
```

The parser regenerates from `parser.lalrpop`. The new dispatch arm and stub method must type-check cleanly. The `unsupported!` stub will return an error at runtime, which is expected — full implementation follows separately.
