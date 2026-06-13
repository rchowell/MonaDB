+++
title = "Functions"
description = "Built-in scalar function reference."
weight = 7
+++

## Call Syntax

Functions are called by name with positional arguments only.

```
upper('hello')
round(3.14159, 2)
concat('a', 'b', 'c')
```

Most scalar functions are _strict_: a `null` argument yields a `null` result without invoking the function. The null-aware exceptions handle nulls themselves — `typeof`, `coalesce`, `nullif`, `ifnull`/`nvl`, `iif`, `concat`, and `concat_ws`.

Aggregate functions (`count`, `sum`, `min`, `max`, `avg`) are supported in the `select` projection as ungrouped aggregation — the whole input reduces to one row. Named arguments, grouped aggregation (the `group` clause), and `read()` are not implemented.

Built-in scalar functions grouped by domain. Each returns a single value per row.

## Type and Conditional Functions

Inspect a value's type and select among values. These are null-aware rather than null-propagating.

| Function                    | Description                                                 |
|-----------------------------|-------------------------------------------------------------|
| `typeof(value)`             | Runtime type name as a string (`int`, `string`, `array`, …) |
| `coalesce(a, b, …)`         | First non-null argument, or null if all are null            |
| `nullif(a, b)`              | Null when the arguments are equal, else `a`                 |
| `ifnull(a, b)`, `nvl(a, b)` | `b` when `a` is null, else `a`                              |
| `iif(cond, a, b)`           | `a` when `cond` is truthy, else `b`                         |

## Math Functions

Numeric operations. Integer arguments stay integers where the result is exact; non-finite results error rather than yield NaN or infinity.

| Function                             | Description                                                 |
|--------------------------------------|-------------------------------------------------------------|
| `abs(x)`                             | Absolute value (integers stay integers)                     |
| `ceil(x)`, `ceiling(x)`              | Smallest integer ≥ `x`                                      |
| `floor(x)`                           | Largest integer ≤ `x`                                       |
| `trunc(x)`                           | Truncate toward zero                                        |
| `round(x)`, `round(x, n)`            | Round to the nearest integer, or to `n` decimal places      |
| `sign(x)`                            | Sign of `x` as -1, 0, or 1                                  |
| `sqrt(x)`                            | Square root (negative argument errors)                      |
| `pow(base, exp)`, `power(base, exp)` | `base` raised to `exp` (exact integer powers stay integers) |
| `exp(x)`                             | e raised to `x`                                             |
| `ln(x)`                              | Natural logarithm (non-positive argument errors)            |
| `log10(x)`                           | Base-10 logarithm (non-positive argument errors)            |
| `mod(a, b)`                          | Remainder of `a` divided by `b` (zero divisor errors)       |
| `greatest(a, b, …)`                  | Largest argument by value order                             |
| `least(a, b, …)`                     | Smallest argument by value order                            |

## String Functions

Text manipulation. `length`, `reverse`, and `contains` also accept arrays, and `length` accepts objects.

| Function                                    | Description                                                            |
|---------------------------------------------|------------------------------------------------------------------------|
| `length(value)`                             | Characters of a string, elements of an array, or members of an object  |
| `upper(str)`                                | Uppercase                                                              |
| `lower(str)`                                | Lowercase                                                              |
| `trim(str)`                                 | Strip leading and trailing whitespace                                  |
| `ltrim(str)`                                | Strip leading whitespace                                               |
| `rtrim(str)`                                | Strip trailing whitespace                                              |
| `substr(str, start[, len])`, `substring(…)` | Substring from a 1-based start, with an optional character length      |
| `replace(str, from, to)`                    | Replace every occurrence of `from` with `to`                           |
| `concat(a, b, …)`                           | Concatenate the arguments, skipping nulls                              |
| `concat_ws(sep, a, b, …)`                   | Join the arguments with `sep`, skipping nulls (null `sep` → null)      |
| `repeat(str, n)`                            | Repeat `str` `n` times                                                 |
| `reverse(value)`                            | Reverse a string's characters or an array's elements                   |
| `lpad(str, width[, fill])`                  | Left-pad to `width` characters (truncates if already longer)           |
| `rpad(str, width[, fill])`                  | Right-pad to `width` characters (truncates if already longer)          |
| `strpos(str, substr)`, `instr(…)`           | 1-based index of the first occurrence of `substr`, or 0                |
| `starts_with(str, prefix)`                  | Whether `str` begins with `prefix`                                     |
| `ends_with(str, suffix)`                    | Whether `str` ends with `suffix`                                       |
| `contains(value, item)`                     | Whether a string contains a substring, or an array contains an element |

## Array Functions

Operate on arrays by value; each returns a new array or a derived scalar without mutating its input.

| Function                       | Description                                                  |
|--------------------------------|--------------------------------------------------------------|
| `array_length(arr)`            | Number of elements                                           |
| `array_contains(arr, item)`    | Whether `arr` contains `item` (by value equality)            |
| `array_position(arr, item)`    | 1-based index of `item`, or 0 if absent                      |
| `array_append(arr, item)`      | `arr` with `item` appended                                   |
| `array_prepend(item, arr)`     | `arr` with `item` prepended                                  |
| `array_concat(a, b)`           | Concatenate two arrays                                       |
| `array_reverse(arr)`           | `arr` with its elements reversed                             |
| `array_distinct(arr)`          | `arr` with duplicates removed, keeping the first occurrence  |
| `array_to_string(arr, sep)`    | Join non-null elements into a string with `sep`              |
| `array_slice(arr, start, end)` | Sub-array between 1-based inclusive bounds, clamped to range |

## Object Functions

Inspect an object's members in insertion order.

| Function                   | Description                                |
|----------------------------|--------------------------------------------|
| `object_keys(obj)`         | Keys as a string array, in insertion order |
| `object_values(obj)`       | Values as an array, in insertion order     |
| `object_has_key(obj, key)` | Whether `obj` has a member under `key`     |

## Type Conversion Functions

The scalar type names are callable as conversion functions. A `null` argument
yields `null`; a conversion that can't succeed — an unparseable string, an
out-of-range integer, or a non-convertible type such as an array — is a runtime
error. Only scalar type names convert; `object`, `array`, and `any` are not callable.

| Function    | Description                                                                    |
|-------------|--------------------------------------------------------------------------------|
| `int(x)`    | To integer: floats truncate toward zero, strings parse leniently, bools → `1`/`0` |
| `float(x)`  | To float: ints widen, strings parse, bools → `1.0`/`0.0`                        |
| `string(x)` | To string: the value's text form (`42`, `1.5`, `true`)                          |
| `bool(x)`   | To bool: `0`/`0.0` → `false`, nonzero → `true`; the strings `true`/`false`       |
| `number(x)` | To number: keeps int/float-ness; a string parses as the matching literal (`'7'` → int, `'5.0'` → float) |

```
int(2.7)        -- 2
int('2.7')      -- 2
string(42)      -- "42"
float(3)        -- 3.0
```
