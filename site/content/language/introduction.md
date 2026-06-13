+++
title = "Introduction"
description = "What the SQL dialect is and how to read this reference."
weight = 1
+++

SQL is the query language for embedded document storage. It keeps a familiar clause vocabulary — `select`, `from`, `where`, `order`, `limit` — and treats objects, arrays, and path navigation as core constructs rather than extensions.

Every query compiles to a sequence of stack-based bytecode instructions and runs inside the host process. There is no server, no network, no configuration file. The database is a library.

## The Clause Model

Each clause in a query is a transform over a stream of bindings. The clauses compose left-to-right: From produces bindings; each subsequent clause transforms them; Select maps them to output values.

**From** iterates — one binding per row or collection element:

```
select t.x from T as t;
select x from [1, 2, 3] as x;
```

**Select** maps — construct the output value:

```
select { x: t.a, y: t.b } from T as t;
select * from T;
```

**Where** filters — drop bindings that fail the predicate:

```
select * from T where T.x > 0;
```

**Order by** sorts — reorder the binding stream:

```
select * from T order by T.x desc;
```

**Limit** takes, skips, or slices the stream:

```
select * from T limit 10;
select * from T limit 2..;
```

## Documents

SQL treats objects and arrays as first-class values. Object literals use `{ key: value }` syntax. Arrays use `[value, value]`. Navigate into values with dot and bracket notation on a binding alias.

```
{ x: 1, y: 2 }           -- object literal
[1, 2, 3]                 -- array literal
t.address.city            -- path into binding t
from T as t, t.items as item   -- unnest an array field
```

Tables may declare key columns or omit them entirely. See [Schemas](@/language/schemas.md) for how keys affect storage, ordering, and lookups.

## How to Read This Reference

The remaining sections cover the language in bottom-up order: [Syntax](@/language/syntax.md) and identifiers first, then [Types](@/language/types.md) and [Schemas](@/language/schemas.md), then [Statements](@/language/statements.md) that compose them, then [Expressions](@/language/expressions.md). [Functions](@/language/functions.md) covers the built-in scalar library. Start with [Statements](@/language/statements.md) if you want to write queries immediately.
