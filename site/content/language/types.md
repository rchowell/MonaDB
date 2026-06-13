+++
title = "Types"
description = "Runtime value types."
weight = 3
+++

## Scalars

Scalar types are the JSON scalar types with distinction between int and float.

| Type    | Example         | Notes                                 |
|---------|-----------------|---------------------------------------|
| null    | `null`          | Distinct from a missing object key    |
| boolean | `true`, `false` |                                       |
| integer | `1`, `-5`       | Exact 64-bit signed integers          |
| float   | `1.5`, `3.14`   | Non-finite values error on insert     |
| string  | `'hello'`       | UTF-8 text                            |

## Complex

Complex types are the JSON array type and object type.

| Type   | Example       | Notes                                     |
|--------|---------------|-------------------------------------------|
| array  | `[1, 2, 3]`   | Ordered sequence                          |
| object | `{ x: 1 }`    | Insertion-ordered map of named fields     |

## Cast

Scalar type names are callable as conversion functions: `int(x)`, `float(x)`, `string(x)`, `bool(x)`, and `number(x)`. A cast is a normal function call — it nests and combines with operators. `null` propagates (`int(null)` is `null`); conversions that cannot succeed are runtime errors; `object`, `array`, and `any` are not callable and produce syntax errors.

| Function    | Description                                                                 |
|-------------|-----------------------------------------------------------------------------|
| `int(x)`    | To integer: floats truncate toward zero; strings parse leniently (trimmed whitespace, float syntax accepted for int targets); bools → `1`/`0` |
| `float(x)`  | To float: ints widen, strings parse, bools → `1.0`/`0.0`                    |
| `string(x)` | To string: the value's text form (`42`, `1.5`, `true`)                      |
| `bool(x)`   | To bool: `0`/`0.0` → `false`, nonzero → `true`; the strings `true`/`false` (case-insensitive) |
| `number(x)` | To number: keeps int/float-ness; strings parse to the narrowest numeric variant |

```
int(2.7)           -- 2
int('2.7')         -- 2
string(42)         -- "42"
float(3)           -- 3.0
int({a: 2.9}.a)    -- 2
typeof(int(2.7))   -- "int"
```

See the [cast examples](/examples/cast/) for full conversion matrices and error cases.
