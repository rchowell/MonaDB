+++
title = "Functions"
description = "Builtin scalar functions — the standard library. Each case evaluates a function via a bare `select <expr>` (no from), which yields the result value directly as a single row. Covers happy paths, dynamic-on-value dispatch, null propagation, the null-aware exceptions, and the static/runtime errors. Note: MonaDB has no unary minus, so negatives are written `0 - n`."
weight = 14
+++

# Functions

Builtin scalar functions — the standard library. Each case evaluates a function via a bare `select <expr>` (no from), which yields the result value directly as a single row. Covers happy paths, dynamic-on-value dispatch, null propagation, the null-aware exceptions, and the static/runtime errors. Note: MonaDB has no unary minus, so negatives are written `0 - n`.

<div class="example">

### Typeof Reports

Typeof reports the value's runtime type name.

<p class="example-label">SQL</p>

```sql
select typeof(1);
```

<p class="example-label">Result</p>

```json
[
  "int"
]
```

<p class="example-label">SQL</p>

```sql
select typeof(1);

select typeof(1.5);
```

<p class="example-label">Result</p>

```json
[
  "float"
]
```

<p class="example-label">SQL</p>

```sql
select typeof(1);

select typeof(1.5);

select typeof('x');
```

<p class="example-label">Result</p>

```json
[
  "string"
]
```

<p class="example-label">SQL</p>

```sql
select typeof(1);

select typeof(1.5);

select typeof('x');

select typeof(true);
```

<p class="example-label">Result</p>

```json
[
  "bool"
]
```

<p class="example-label">SQL</p>

```sql
select typeof(1);

select typeof(1.5);

select typeof('x');

select typeof(true);

select typeof(null);
```

<p class="example-label">Result</p>

```json
[
  "null"
]
```

<p class="example-label">SQL</p>

```sql
select typeof(1);

select typeof(1.5);

select typeof('x');

select typeof(true);

select typeof(null);

select typeof([1, 2]);
```

<p class="example-label">Result</p>

```json
[
  "array"
]
```

<p class="example-label">SQL</p>

```sql
select typeof(1);

select typeof(1.5);

select typeof('x');

select typeof(true);

select typeof(null);

select typeof([1, 2]);

select typeof({a: 1});
```

<p class="example-label">Result</p>

```json
[
  "object"
]
```

</div>

<div class="example">

### Coalesce Returns

Coalesce returns the first non-null argument.

<p class="example-label">SQL</p>

```sql
select coalesce(null, null, 7);
```

<p class="example-label">Result</p>

```json
[
  7
]
```

<p class="example-label">SQL</p>

```sql
select coalesce(null, null, 7);

select coalesce(null, 'x');
```

<p class="example-label">Result</p>

```json
[
  "x"
]
```

<p class="example-label">SQL</p>

```sql
select coalesce(null, null, 7);

select coalesce(null, 'x');

select coalesce(1, 2);
```

<p class="example-label">Result</p>

```json
[
  1
]
```

<p class="example-label">SQL</p>

```sql
select coalesce(null, null, 7);

select coalesce(null, 'x');

select coalesce(1, 2);

select coalesce(null, null);
```

<p class="example-label">Result</p>

```json
[
  null
]
```

</div>

<div class="example">

### Nullif Yields

Nullif yields null when the two arguments are equal.

<p class="example-label">SQL</p>

```sql
select nullif(5, 5);
```

<p class="example-label">Result</p>

```json
[
  null
]
```

<p class="example-label">SQL</p>

```sql
select nullif(5, 5);

select nullif(5, 3);
```

<p class="example-label">Result</p>

```json
[
  5
]
```

</div>

<div class="example">

### Ifnull /

Ifnull / nvl substitute a default for null.

<p class="example-label">SQL</p>

```sql
select ifnull(null, 'd');
```

<p class="example-label">Result</p>

```json
[
  "d"
]
```

<p class="example-label">SQL</p>

```sql
select ifnull(null, 'd');

select ifnull('v', 'd');
```

<p class="example-label">Result</p>

```json
[
  "v"
]
```

<p class="example-label">SQL</p>

```sql
select ifnull(null, 'd');

select ifnull('v', 'd');

select nvl(null, 9);
```

<p class="example-label">Result</p>

```json
[
  9
]
```

</div>

<div class="example">

### Iif Selects

Iif selects a branch on the truthiness of the condition.

<p class="example-label">SQL</p>

```sql
select iif(true, 'y', 'n');
```

<p class="example-label">Result</p>

```json
[
  "y"
]
```

<p class="example-label">SQL</p>

```sql
select iif(true, 'y', 'n');

select iif(1 = 2, 'y', 'n');
```

<p class="example-label">Result</p>

```json
[
  "n"
]
```

<p class="example-label">SQL</p>

```sql
select iif(true, 'y', 'n');

select iif(1 = 2, 'y', 'n');

select iif(null, 'y', 'n');
```

<p class="example-label">Result</p>

```json
[
  "n"
]
```

</div>

<div class="example">

### Abs Of

Abs of ints and floats.

<p class="example-label">SQL</p>

```sql
select abs(0 - 5);
```

<p class="example-label">Result</p>

```json
[
  5
]
```

<p class="example-label">SQL</p>

```sql
select abs(0 - 5);

select abs(5);
```

<p class="example-label">Result</p>

```json
[
  5
]
```

<p class="example-label">SQL</p>

```sql
select abs(0 - 5);

select abs(5);

select abs(0.0 - 2.5);
```

<p class="example-label">Result</p>

```json
[
  2.5
]
```

</div>

<div class="example">

### Ceil, Floor,

Ceil, floor, and trunc round floats; ints pass through.

<p class="example-label">SQL</p>

```sql
select ceil(3.2);
```

<p class="example-label">Result</p>

```json
[
  4
]
```

<p class="example-label">SQL</p>

```sql
select ceil(3.2);

select ceiling(3.2);
```

<p class="example-label">Result</p>

```json
[
  4
]
```

<p class="example-label">SQL</p>

```sql
select ceil(3.2);

select ceiling(3.2);

select floor(3.8);
```

<p class="example-label">Result</p>

```json
[
  3
]
```

<p class="example-label">SQL</p>

```sql
select ceil(3.2);

select ceiling(3.2);

select floor(3.8);

select trunc(3.9);
```

<p class="example-label">Result</p>

```json
[
  3
]
```

<p class="example-label">SQL</p>

```sql
select ceil(3.2);

select ceiling(3.2);

select floor(3.8);

select trunc(3.9);

select ceil(7);
```

<p class="example-label">Result</p>

```json
[
  7
]
```

</div>

<div class="example">

### Round To

Round to nearest integer, and to n decimals.

<p class="example-label">SQL</p>

```sql
select round(3.7);
```

<p class="example-label">Result</p>

```json
[
  4
]
```

<p class="example-label">SQL</p>

```sql
select round(3.7);

select round(3.14159, 0);
```

<p class="example-label">Result</p>

```json
[
  3
]
```

<p class="example-label">SQL</p>

```sql
select round(3.7);

select round(3.14159, 0);

select round(5);
```

<p class="example-label">Result</p>

```json
[
  5
]
```

</div>

<div class="example">

### Sign Returns

Sign returns -1, 0, or 1.

<p class="example-label">SQL</p>

```sql
select sign(0 - 5);
```

<p class="example-label">Result</p>

```json
[
  -1
]
```

<p class="example-label">SQL</p>

```sql
select sign(0 - 5);

select sign(5);
```

<p class="example-label">Result</p>

```json
[
  1
]
```

<p class="example-label">SQL</p>

```sql
select sign(0 - 5);

select sign(5);

select sign(0);
```

<p class="example-label">Result</p>

```json
[
  0
]
```

</div>

<div class="example">

### Sqrt And

Sqrt and pow (integer powers stay integers).

<p class="example-label">SQL</p>

```sql
select sqrt(9);
```

<p class="example-label">Result</p>

```json
[
  3
]
```

<p class="example-label">SQL</p>

```sql
select sqrt(9);

select pow(2, 10);
```

<p class="example-label">Result</p>

```json
[
  1024
]
```

<p class="example-label">SQL</p>

```sql
select sqrt(9);

select pow(2, 10);

select power(3, 2);
```

<p class="example-label">Result</p>

```json
[
  9
]
```

</div>

<div class="example">

### Exp, Ln,

Exp, ln, and log10 at exact points.

<p class="example-label">SQL</p>

```sql
select exp(0);
```

<p class="example-label">Result</p>

```json
[
  1
]
```

<p class="example-label">SQL</p>

```sql
select exp(0);

select ln(1);
```

<p class="example-label">Result</p>

```json
[
  0
]
```

<p class="example-label">SQL</p>

```sql
select exp(0);

select ln(1);

select log10(1);
```

<p class="example-label">Result</p>

```json
[
  0
]
```

</div>

<div class="example">

### Mod Is

Mod is the integer remainder.

<p class="example-label">SQL</p>

```sql
select mod(7, 3);
```

<p class="example-label">Result</p>

```json
[
  1
]
```

<p class="example-label">SQL</p>

```sql
select mod(7, 3);

select mod(10, 5);
```

<p class="example-label">Result</p>

```json
[
  0
]
```

</div>

<div class="example">

### Greatest And

Greatest and least over numbers and strings.

<p class="example-label">SQL</p>

```sql
select greatest(1, 5, 3);
```

<p class="example-label">Result</p>

```json
[
  5
]
```

<p class="example-label">SQL</p>

```sql
select greatest(1, 5, 3);

select least(1, 5, 3);
```

<p class="example-label">Result</p>

```json
[
  1
]
```

<p class="example-label">SQL</p>

```sql
select greatest(1, 5, 3);

select least(1, 5, 3);

select greatest('a', 'c', 'b');
```

<p class="example-label">Result</p>

```json
[
  "c"
]
```

</div>

<div class="example">

### Length Dispatches

Length dispatches on value — chars, elements, or members.

<p class="example-label">SQL</p>

```sql
select length('hello');
```

<p class="example-label">Result</p>

```json
[
  5
]
```

<p class="example-label">SQL</p>

```sql
select length('hello');

select length([1, 2, 3]);
```

<p class="example-label">Result</p>

```json
[
  3
]
```

<p class="example-label">SQL</p>

```sql
select length('hello');

select length([1, 2, 3]);

select length({x: 1, y: 2});
```

<p class="example-label">Result</p>

```json
[
  2
]
```

<p class="example-label">SQL</p>

```sql
select length('hello');

select length([1, 2, 3]);

select length({x: 1, y: 2});

select length('café');
```

<p class="example-label">Result</p>

```json
[
  4
]
```

</div>

<div class="example">

### Upper And

Upper and lower case strings.

<p class="example-label">SQL</p>

```sql
select upper('hi');
```

<p class="example-label">Result</p>

```json
[
  "HI"
]
```

<p class="example-label">SQL</p>

```sql
select upper('hi');

select lower('HI');
```

<p class="example-label">Result</p>

```json
[
  "hi"
]
```

</div>

<div class="example">

### Trim, Ltrim,

Trim, ltrim, and rtrim strip surrounding whitespace.

<p class="example-label">SQL</p>

```sql
select trim('  hi  ');
```

<p class="example-label">Result</p>

```json
[
  "hi"
]
```

<p class="example-label">SQL</p>

```sql
select trim('  hi  ');

select ltrim('  hi');
```

<p class="example-label">Result</p>

```json
[
  "hi"
]
```

<p class="example-label">SQL</p>

```sql
select trim('  hi  ');

select ltrim('  hi');

select rtrim('hi  ');
```

<p class="example-label">Result</p>

```json
[
  "hi"
]
```

</div>

<div class="example">

### Substr Is

Substr is 1-based, with an optional length.

<p class="example-label">SQL</p>

```sql
select substr('hello', 2);
```

<p class="example-label">Result</p>

```json
[
  "ello"
]
```

<p class="example-label">SQL</p>

```sql
select substr('hello', 2);

select substr('hello', 2, 3);
```

<p class="example-label">Result</p>

```json
[
  "ell"
]
```

<p class="example-label">SQL</p>

```sql
select substr('hello', 2);

select substr('hello', 2, 3);

select substring('hello', 1, 1);
```

<p class="example-label">Result</p>

```json
[
  "h"
]
```

</div>

<div class="example">

### Replace Swaps

Replace swaps every occurrence of a substring.

<p class="example-label">SQL</p>

```sql
select replace('aXbXc', 'X', '-');
```

<p class="example-label">Result</p>

```json
[
  "a-b-c"
]
```

</div>

<div class="example">

### Concat Joins

Concat joins arguments, skipping nulls, stringifying scalars.

<p class="example-label">SQL</p>

```sql
select concat('a', 'b', 'c');
```

<p class="example-label">Result</p>

```json
[
  "abc"
]
```

<p class="example-label">SQL</p>

```sql
select concat('a', 'b', 'c');

select concat('a', null, 'b');
```

<p class="example-label">Result</p>

```json
[
  "ab"
]
```

<p class="example-label">SQL</p>

```sql
select concat('a', 'b', 'c');

select concat('a', null, 'b');

select concat('n', 42);
```

<p class="example-label">Result</p>

```json
[
  "n42"
]
```

</div>

<div class="example">

### Concat_ws Joins

Concat_ws joins with a separator, skipping nulls.

<p class="example-label">SQL</p>

```sql
select concat_ws('-', 'a', 'b', 'c');
```

<p class="example-label">Result</p>

```json
[
  "a-b-c"
]
```

<p class="example-label">SQL</p>

```sql
select concat_ws('-', 'a', 'b', 'c');

select concat_ws(',', 'a', null, 'b');
```

<p class="example-label">Result</p>

```json
[
  "a,b"
]
```

</div>

<div class="example">

### Repeat Duplicates

Repeat duplicates a string; reverse is dynamic on value.

<p class="example-label">SQL</p>

```sql
select repeat('ab', 3);
```

<p class="example-label">Result</p>

```json
[
  "ababab"
]
```

<p class="example-label">SQL</p>

```sql
select repeat('ab', 3);

select reverse('abc');
```

<p class="example-label">Result</p>

```json
[
  "cba"
]
```

<p class="example-label">SQL</p>

```sql
select repeat('ab', 3);

select reverse('abc');

select reverse([1, 2, 3]);
```

<p class="example-label">Result</p>

```json
[
  [ 3, 2, 1 ]
]
```

</div>

<div class="example">

### Lpad And

Lpad and rpad pad to a target width with a fill string.

<p class="example-label">SQL</p>

```sql
select lpad('5', 3, '0');
```

<p class="example-label">Result</p>

```json
[
  "005"
]
```

<p class="example-label">SQL</p>

```sql
select lpad('5', 3, '0');

select rpad('5', 3, '0');
```

<p class="example-label">Result</p>

```json
[
  "500"
]
```

<p class="example-label">SQL</p>

```sql
select lpad('5', 3, '0');

select rpad('5', 3, '0');

select lpad('x', 3);
```

<p class="example-label">Result</p>

```json
[
  "  x"
]
```

</div>

<div class="example">

### Repeat /

Repeat / lpad reject pathological sizes instead of exhausting memory.

<p class="example-label">SQL</p>

```sql
select repeat('x', 99999999999);
```

Expected error: `runtime`

<p class="example-label">SQL</p>

```sql
select repeat('x', 99999999999);

select lpad('x', 99999999999);
```

Expected error: `runtime`

</div>

<div class="example">

### Strpos /

Strpos / instr return a 1-based index, 0 if absent.

<p class="example-label">SQL</p>

```sql
select strpos('hello', 'l');
```

<p class="example-label">Result</p>

```json
[
  3
]
```

<p class="example-label">SQL</p>

```sql
select strpos('hello', 'l');

select instr('hello', 'l');
```

<p class="example-label">Result</p>

```json
[
  3
]
```

<p class="example-label">SQL</p>

```sql
select strpos('hello', 'l');

select instr('hello', 'l');

select strpos('hello', 'z');
```

<p class="example-label">Result</p>

```json
[
  0
]
```

</div>

<div class="example">

### Starts_with, Ends_with,

Starts_with, ends_with, contains (dynamic on value).

<p class="example-label">SQL</p>

```sql
select starts_with('hello', 'he');
```

<p class="example-label">Result</p>

```json
[
  true
]
```

<p class="example-label">SQL</p>

```sql
select starts_with('hello', 'he');

select ends_with('hello', 'lo');
```

<p class="example-label">Result</p>

```json
[
  true
]
```

<p class="example-label">SQL</p>

```sql
select starts_with('hello', 'he');

select ends_with('hello', 'lo');

select contains('hello', 'ell');
```

<p class="example-label">Result</p>

```json
[
  true
]
```

<p class="example-label">SQL</p>

```sql
select starts_with('hello', 'he');

select ends_with('hello', 'lo');

select contains('hello', 'ell');

select contains([1, 2, 3], 2);
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### Array_length, Array_contains, Array_position

Array_length, array_contains, array_position.

<p class="example-label">SQL</p>

```sql
select array_length([1, 2, 3]);
```

<p class="example-label">Result</p>

```json
[
  3
]
```

<p class="example-label">SQL</p>

```sql
select array_length([1, 2, 3]);

select array_contains([1, 2, 3], 2);
```

<p class="example-label">Result</p>

```json
[
  true
]
```

<p class="example-label">SQL</p>

```sql
select array_length([1, 2, 3]);

select array_contains([1, 2, 3], 2);

select array_position([10, 20, 30], 20);
```

<p class="example-label">Result</p>

```json
[
  2
]
```

<p class="example-label">SQL</p>

```sql
select array_length([1, 2, 3]);

select array_contains([1, 2, 3], 2);

select array_position([10, 20, 30], 20);

select array_position([10, 20], 99);
```

<p class="example-label">Result</p>

```json
[
  0
]
```

</div>

<div class="example">

### Array_append, Array_prepend, Array_concat

Array_append, array_prepend, array_concat.

<p class="example-label">SQL</p>

```sql
select array_append([1, 2], 3);
```

<p class="example-label">Result</p>

```json
[
  [ 1, 2, 3 ]
]
```

<p class="example-label">SQL</p>

```sql
select array_append([1, 2], 3);

select array_prepend(0, [1, 2]);
```

<p class="example-label">Result</p>

```json
[
  [ 0, 1, 2 ]
]
```

<p class="example-label">SQL</p>

```sql
select array_append([1, 2], 3);

select array_prepend(0, [1, 2]);

select array_concat([1, 2], [3, 4]);
```

<p class="example-label">Result</p>

```json
[
  [ 1, 2, 3, 4 ]
]
```

</div>

<div class="example">

### Array_reverse, Array_distinct,

Array_reverse, array_distinct, array_slice, array_to_string.

<p class="example-label">SQL</p>

```sql
select array_reverse([1, 2, 3]);
```

<p class="example-label">Result</p>

```json
[
  [ 3, 2, 1 ]
]
```

<p class="example-label">SQL</p>

```sql
select array_reverse([1, 2, 3]);

select array_distinct([1, 2, 2, 3, 1]);
```

<p class="example-label">Result</p>

```json
[
  [ 1, 2, 3 ]
]
```

<p class="example-label">SQL</p>

```sql
select array_reverse([1, 2, 3]);

select array_distinct([1, 2, 2, 3, 1]);

select array_slice([1, 2, 3, 4, 5], 2, 4);
```

<p class="example-label">Result</p>

```json
[
  [ 2, 3, 4 ]
]
```

<p class="example-label">SQL</p>

```sql
select array_reverse([1, 2, 3]);

select array_distinct([1, 2, 2, 3, 1]);

select array_slice([1, 2, 3, 4, 5], 2, 4);

select array_to_string([1, 2, 3], '-');
```

<p class="example-label">Result</p>

```json
[
  "1-2-3"
]
```

</div>

<div class="example">

### Object_keys, Object_values, Object_has_key

Object_keys, object_values, object_has_key.

<p class="example-label">SQL</p>

```sql
select object_keys({a: 1, b: 2});
```

<p class="example-label">Result</p>

```json
[
  [ "a", "b" ]
]
```

<p class="example-label">SQL</p>

```sql
select object_keys({a: 1, b: 2});

select object_values({a: 1, b: 2});
```

<p class="example-label">Result</p>

```json
[
  [ 1, 2 ]
]
```

<p class="example-label">SQL</p>

```sql
select object_keys({a: 1, b: 2});

select object_values({a: 1, b: 2});

select object_has_key({a: 1}, 'a');
```

<p class="example-label">Result</p>

```json
[
  true
]
```

<p class="example-label">SQL</p>

```sql
select object_keys({a: 1, b: 2});

select object_values({a: 1, b: 2});

select object_has_key({a: 1}, 'a');

select object_has_key({a: 1}, 'z');
```

<p class="example-label">Result</p>

```json
[
  false
]
```

</div>

<div class="example">

### Fn null Propagation

A null argument to a strict function yields null.

<p class="example-label">SQL</p>

```sql
select abs(null);
```

<p class="example-label">Result</p>

```json
[
  null
]
```

<p class="example-label">SQL</p>

```sql
select abs(null);

select upper(null);
```

<p class="example-label">Result</p>

```json
[
  null
]
```

<p class="example-label">SQL</p>

```sql
select abs(null);

select upper(null);

select length(null);
```

<p class="example-label">Result</p>

```json
[
  null
]
```

<p class="example-label">SQL</p>

```sql
select abs(null);

select upper(null);

select length(null);

select round(null, 2);
```

<p class="example-label">Result</p>

```json
[
  null
]
```

<p class="example-label">SQL</p>

```sql
select abs(null);

select upper(null);

select length(null);

select round(null, 2);

select array_length(null);
```

<p class="example-label">Result</p>

```json
[
  null
]
```

</div>

<div class="example">

### Wrong Argument

Wrong argument count is a static error.

<p class="example-label">SQL</p>

```sql
select abs(1, 2);
```

Expected error: `static`

<p class="example-label">SQL</p>

```sql
select abs(1, 2);

select upper();
```

Expected error: `static`

</div>

<div class="example">

### An Undefined

An undefined function name is a static error.

<p class="example-label">SQL</p>

```sql
select bogus(1);
```

Expected error: `static`

</div>

<div class="example">

### Fn Type Error

A wrong-typed argument is a runtime error.

<p class="example-label">SQL</p>

```sql
select abs('x');
```

Expected error: `runtime`

<p class="example-label">SQL</p>

```sql
select abs('x');

select upper(42);
```

Expected error: `runtime`

</div>