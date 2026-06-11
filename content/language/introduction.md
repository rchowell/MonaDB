+++
title = "Introduction"
description = "What RQL is, how it relates to SQL, and how to read this reference."
weight = 1
+++

# Introduction

RQL is a SQL-flavored query language for embedded document storage. It keeps the clause vocabulary of standard SQL — `select`, `from`, `where`, `group`, `order`, `limit` — and extends it to treat objects, arrays, and path traversal as core constructs rather than extensions.

Every query compiles to a sequence of stack-based bytecode instructions and runs inside the host process. There is no server, no network, no configuration file. The database is a library.

## The clause model

Each clause in a query is a transform over a stream of bindings.

| Clause | Operation |
|--------|-----------|
| `from` | Iterate — produce one binding per row |
| `with` | Map — extend each binding |
| `select` | Map — construct the output value |
| `where` | Filter — drop bindings that fail the predicate |
| `group` | Reduce — collapse bindings by key |
| `order` | Sort — reorder the binding stream |
| `limit` / `fetch` | Limit — take at most N bindings, with optional offset |

The clauses compose left-to-right. `from` produces bindings; each subsequent clause transforms them; `select` maps them to output values.

## Documents

RQL treats objects and arrays as first-class values. Object literals use `{ key: value }` syntax. Arrays use `[value, value]`. Path traversal uses `$` notation rooted at a table or variable.

```
{ x: 1, y: 2 }           -- object literal
[1, 2, 3]                 -- array literal
T$.address.city           -- path into T
```

Schemas are optional. A table declared without a schema accepts any value.

## How to read this reference

The remaining sections cover the language in bottom-up order: [Syntax](@/language/syntax.md) and identifiers first, then [Types](@/language/types.md), then [Expressions](@/language/expressions.md), then [Statements](@/language/statements.md) that compose them. [Functions](@/language/functions.md) covers built-in functions and `read()`. Start with [Statements](@/language/statements.md) if you want to write queries immediately.
