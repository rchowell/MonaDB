# RQL Conformance Test Suite — Format Specification

> Version 1. This document defines the authoritative schema for all files in `tests/conformance/`.

---

## Overview

Tests are organized into **suite files** — one per language-spec section. Suite files are YAML for readability: multiline queries use `|` block scalars, and result values use YAML flow mappings (`{x: 1}`) instead of escaped JSON.

Each test is a sequence of **steps**. A step executes one RQL statement and optionally asserts its output (`result`) or expected error category (`error`). Steps without either field are fire-and-forget (executed, must succeed).

```
manifest.yaml
└── suites/
    ├── 01-literals.yaml    §3.1
    ├── 09-from.yaml        §4.2
    └── ...
```

---

## Execution model

1. Fresh in-memory DB per test case (no state leaks between tests).
2. Suite-level `setup` statements run before each test's steps.
3. Test-level `setup` statements run after suite-level setup.
4. Steps execute in order.
5. Test-level `teardown` runs after steps.
6. Suite-level `teardown` runs last.
7. DB is discarded. Because the DB is fresh per test, teardown is rarely needed.

---

## `manifest.yaml` schema

```yaml
version: "1"
suites:
  - file: suites/01-literals.yaml    # path relative to manifest
    title: Literals
    spec_ref: "§3.1"                 # optional, references language.md section
```

---

## Suite file schema

```yaml
suite: from-clause                   # short machine-readable name
spec_ref: "§4.2"                     # optional
description: One-line description.   # optional

setup:                               # optional; run before each test
  - create table T;

teardown: []                         # optional; run after each test

tests:
  - id: my-test-id                   # kebab-case, unique within suite
    description: What this tests.    # optional
    setup: []                        # test-local setup (after suite setup)
    teardown: []                     # test-local teardown (before suite teardown)
    tags: []                         # optional: "slow", "optional", "wip"
    steps:
      - rql: select 1;               # the statement
        result: [1]                  # expected output rows (positive test)
      - rql: select * from Ghost;
        error: static                # expected error category (negative test)
      - rql: insert into T ({x: 1}); # no result/error = fire-and-forget
```

---

## Step fields

| Field | Required | Description |
|-------|----------|-------------|
| `rql` | yes | The RQL statement to execute |
| `result` | one of | Ordered array of expected output values |
| `error` | one of | Expected error category (see taxonomy) |

`result` and `error` are mutually exclusive. Omitting both asserts only that the statement succeeds.

**Result ordering.** `result` is an ordered sequence. For queries without `order`, use `order` in the RQL to pin the output order.

**Numeric comparison.** MonaDB has one numeric type (IEEE-754 double). The harness compares numbers as `f64`, so `1` and `1.0` in expected values are treated identically.

---

## Error taxonomy

| Category | Meaning |
|----------|---------|
| `syntax` | Parse or lex error |
| `static` | Semantic error before execution: unbound name, duplicate key literal, aggregate in wrong position, etc. |
| `runtime` | Error during execution: type mismatch, cast failure, division by zero |
| `schema` | Schema violation: insert with wrong type, extra keys in closed schema |
| `constraint` | Key uniqueness violation |
| `storage` | Storage-layer error: exceeds size limits |

Conformance requires matching the **category**, not the error message.

---

## YAML quoting conventions

RQL statements may contain characters that YAML parses specially (`{`, `}`, `[`, `]`, `:`, `#`). Use these conventions:

- **Simple statements** (no JSON-like syntax): unquoted is fine.
  ```yaml
  rql: select null;
  ```
- **Statements with `{...}` or `[...]`**: use `|` (literal block scalar) or double-quote the value.
  ```yaml
  rql: |
    insert into T ({x: 1, y: 2});
  ```
  ```yaml
  rql: "select {x: 1};"
  ```
- **Multiline statements**: always use `|`.
  ```yaml
  rql: |
    select t.x, t.y
    from T as t
    where t.x > 0
    order t.x;
  ```

---

## How to add tests

1. Find the suite file matching the spec section (or create one and add it to `manifest.yaml`).
2. Add a test case with a unique `id` in kebab-case.
3. For positive tests: use `result` with an ordered list of expected output values.
4. For negative tests: use `error` with the appropriate category.
5. Use `order` in queries to ensure deterministic output when testing multi-row results.
6. Run `cargo test conformance` to verify (requires `Connection::memory()` to be implemented).
