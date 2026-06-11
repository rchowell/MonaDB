+++
title = "Types"
description = "Scalar types, collection types, aliases, and nullability."
weight = 4
+++

# Types

RQL has five base types. All have a canonical name and a short alias. Types appear in `create table` schema declarations and in cast expressions.

| Type | Alias | Description |
|------|-------|-------------|
| `boolean` | `bool` | `true` or `false` |
| `number` | `num` | 64-bit floating-point |
| `string` | `str` | UTF-8 text |
| `array` | `arr` | Ordered sequence of values |
| `object` | `obj` | Unordered map of named fields |

## Nullability

Schema members are `NOT NULL` by default. Append `|null` to a type to allow null.

```
create table readings ({
    sensor: string,
    value:  number,
    label:  string|null,   -- nullable
});
```

A nullable field accepts either a typed value or `null`. A non-nullable field rejects `null` at insert time.

## Open content

Append `...` as the last member of an object schema to allow extra fields. Without `...`, inserting an object with undeclared fields is a type error.

```
create table events ({
    id:   number,
    name: string,
    ...               -- any extra fields allowed
});
```

## No schema

A table declared as `create table t;` accepts any value. Type checking is skipped entirely.
