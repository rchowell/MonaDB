+++
title = "Casts"
description = "Type conversions — the scalar type names are callable as conversion functions: int(x), float(x), string(x), bool(x), number(x). They desugar to per-type builtins, so null propagates (a null cast is null), bad conversions are runtime errors, and a non-scalar target name (object, array, any) is a syntax error. Float→int truncates toward zero; string→number parsing is lenient (trims whitespace, accepts float syntax for int targets). Note: MonaDB has no unary minus, so negatives are written as literals."
weight = 15
+++

# Casts

Type conversions — the scalar type names are callable as conversion functions: int(x), float(x), string(x), bool(x), number(x). They desugar to per-type builtins, so null propagates (a null cast is null), bad conversions are runtime errors, and a non-scalar target name (object, array, any) is a syntax error. Float→int truncates toward zero; string→number parsing is lenient (trims whitespace, accepts float syntax for int targets). Note: MonaDB has no unary minus, so negatives are written as literals.

<div class="example">

### Int(x) Over

Int(x) over int, float, string, and bool sources.

<p class="example-label">SQL</p>

```sql
select int(7);
```

<p class="example-label">Result</p>

```json
[
  7
]
```

<p class="example-label">SQL</p>

```sql
select int(7);

select int(2.7);
```

<p class="example-label">Result</p>

```json
[
  2
]
```

<p class="example-label">SQL</p>

```sql
select int(7);

select int(2.7);

select int(-2.7);
```

<p class="example-label">Result</p>

```json
[
  -2
]
```

<p class="example-label">SQL</p>

```sql
select int(7);

select int(2.7);

select int(-2.7);

select int(2.0);
```

<p class="example-label">Result</p>

```json
[
  2
]
```

<p class="example-label">SQL</p>

```sql
select int(7);

select int(2.7);

select int(-2.7);

select int(2.0);

select int('5');
```

<p class="example-label">Result</p>

```json
[
  5
]
```

<p class="example-label">SQL</p>

```sql
select int(7);

select int(2.7);

select int(-2.7);

select int(2.0);

select int('5');

select int(' 5 ');
```

<p class="example-label">Result</p>

```json
[
  5
]
```

<p class="example-label">SQL</p>

```sql
select int(7);

select int(2.7);

select int(-2.7);

select int(2.0);

select int('5');

select int(' 5 ');

select int('2.7');
```

<p class="example-label">Result</p>

```json
[
  2
]
```

<p class="example-label">SQL</p>

```sql
select int(7);

select int(2.7);

select int(-2.7);

select int(2.0);

select int('5');

select int(' 5 ');

select int('2.7');

select int(true);
```

<p class="example-label">Result</p>

```json
[
  1
]
```

<p class="example-label">SQL</p>

```sql
select int(7);

select int(2.7);

select int(-2.7);

select int(2.0);

select int('5');

select int(' 5 ');

select int('2.7');

select int(true);

select int(false);
```

<p class="example-label">Result</p>

```json
[
  0
]
```

<p class="example-label">SQL</p>

```sql
select int(7);

select int(2.7);

select int(-2.7);

select int(2.0);

select int('5');

select int(' 5 ');

select int('2.7');

select int(true);

select int(false);

select int('9007199254740993.0') = 9007199254740993;
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### Float(x) Over

Float(x) over int, float, string, and bool sources.

<p class="example-label">SQL</p>

```sql
select float(3);
```

<p class="example-label">Result</p>

```json
[
  3
]
```

<p class="example-label">SQL</p>

```sql
select float(3);

select float(2.5);
```

<p class="example-label">Result</p>

```json
[
  2.5
]
```

<p class="example-label">SQL</p>

```sql
select float(3);

select float(2.5);

select float('1.5');
```

<p class="example-label">Result</p>

```json
[
  1.5
]
```

<p class="example-label">SQL</p>

```sql
select float(3);

select float(2.5);

select float('1.5');

select float('4');
```

<p class="example-label">Result</p>

```json
[
  4
]
```

<p class="example-label">SQL</p>

```sql
select float(3);

select float(2.5);

select float('1.5');

select float('4');

select float(true);
```

<p class="example-label">Result</p>

```json
[
  1
]
```

<p class="example-label">SQL</p>

```sql
select float(3);

select float(2.5);

select float('1.5');

select float('4');

select float(true);

select float(false);
```

<p class="example-label">Result</p>

```json
[
  0
]
```

</div>

<div class="example">

### String(x) Renders

String(x) renders a scalar as text.

<p class="example-label">SQL</p>

```sql
select string(42);
```

<p class="example-label">Result</p>

```json
[
  "42"
]
```

<p class="example-label">SQL</p>

```sql
select string(42);

select string(1.5);
```

<p class="example-label">Result</p>

```json
[
  "1.5"
]
```

<p class="example-label">SQL</p>

```sql
select string(42);

select string(1.5);

select string(true);
```

<p class="example-label">Result</p>

```json
[
  "true"
]
```

<p class="example-label">SQL</p>

```sql
select string(42);

select string(1.5);

select string(true);

select string(false);
```

<p class="example-label">Result</p>

```json
[
  "false"
]
```

<p class="example-label">SQL</p>

```sql
select string(42);

select string(1.5);

select string(true);

select string(false);

select string('hi');
```

<p class="example-label">Result</p>

```json
[
  "hi"
]
```

</div>

<div class="example">

### Bool(x) Over

Bool(x) over numbers and the true/false strings.

<p class="example-label">SQL</p>

```sql
select bool(0);
```

<p class="example-label">Result</p>

```json
[
  false
]
```

<p class="example-label">SQL</p>

```sql
select bool(0);

select bool(1);
```

<p class="example-label">Result</p>

```json
[
  true
]
```

<p class="example-label">SQL</p>

```sql
select bool(0);

select bool(1);

select bool(5);
```

<p class="example-label">Result</p>

```json
[
  true
]
```

<p class="example-label">SQL</p>

```sql
select bool(0);

select bool(1);

select bool(5);

select bool(0.0);
```

<p class="example-label">Result</p>

```json
[
  false
]
```

<p class="example-label">SQL</p>

```sql
select bool(0);

select bool(1);

select bool(5);

select bool(0.0);

select bool(2.5);
```

<p class="example-label">Result</p>

```json
[
  true
]
```

<p class="example-label">SQL</p>

```sql
select bool(0);

select bool(1);

select bool(5);

select bool(0.0);

select bool(2.5);

select bool('true');
```

<p class="example-label">Result</p>

```json
[
  true
]
```

<p class="example-label">SQL</p>

```sql
select bool(0);

select bool(1);

select bool(5);

select bool(0.0);

select bool(2.5);

select bool('true');

select bool('false');
```

<p class="example-label">Result</p>

```json
[
  false
]
```

<p class="example-label">SQL</p>

```sql
select bool(0);

select bool(1);

select bool(5);

select bool(0.0);

select bool(2.5);

select bool('true');

select bool('false');

select bool('TRUE');
```

<p class="example-label">Result</p>

```json
[
  true
]
```

</div>

<div class="example">

### Number(x) Keeps

Number(x) keeps the int/float-ness of the value.

<p class="example-label">SQL</p>

```sql
select number(7);
```

<p class="example-label">Result</p>

```json
[
  7
]
```

<p class="example-label">SQL</p>

```sql
select number(7);

select number(2.5);
```

<p class="example-label">Result</p>

```json
[
  2.5
]
```

<p class="example-label">SQL</p>

```sql
select number(7);

select number(2.5);

select number('7');
```

<p class="example-label">Result</p>

```json
[
  7
]
```

<p class="example-label">SQL</p>

```sql
select number(7);

select number(2.5);

select number('7');

select number('1.5');
```

<p class="example-label">Result</p>

```json
[
  1.5
]
```

<p class="example-label">SQL</p>

```sql
select number(7);

select number(2.5);

select number('7');

select number('1.5');

select number(true);
```

<p class="example-label">Result</p>

```json
[
  1
]
```

<p class="example-label">SQL</p>

```sql
select number(7);

select number(2.5);

select number('7');

select number('1.5');

select number(true);

select number('5.0');
```

<p class="example-label">Result</p>

```json
[
  5
]
```

</div>

<div class="example">

### Cast Compose

A cast is a normal call — it nests and combines with operators.

<p class="example-label">SQL</p>

```sql
select int('5') + 1;
```

<p class="example-label">Result</p>

```json
[
  6
]
```

<p class="example-label">SQL</p>

```sql
select int('5') + 1;

select int(2.9) + 1;
```

<p class="example-label">Result</p>

```json
[
  3
]
```

<p class="example-label">SQL</p>

```sql
select int('5') + 1;

select int(2.9) + 1;

select int({a: 2.9}.a);
```

<p class="example-label">Result</p>

```json
[
  2
]
```

<p class="example-label">SQL</p>

```sql
select int('5') + 1;

select int(2.9) + 1;

select int({a: 2.9}.a);

select int(float('2.7'));
```

<p class="example-label">Result</p>

```json
[
  2
]
```

</div>

<div class="example">

### Cast null

A null argument short-circuits to null.

<p class="example-label">SQL</p>

```sql
select int(null);
```

<p class="example-label">Result</p>

```json
[
  null
]
```

<p class="example-label">SQL</p>

```sql
select int(null);

select float(null);
```

<p class="example-label">Result</p>

```json
[
  null
]
```

<p class="example-label">SQL</p>

```sql
select int(null);

select float(null);

select string(null);
```

<p class="example-label">Result</p>

```json
[
  null
]
```

<p class="example-label">SQL</p>

```sql
select int(null);

select float(null);

select string(null);

select bool(null);
```

<p class="example-label">Result</p>

```json
[
  null
]
```

<p class="example-label">SQL</p>

```sql
select int(null);

select float(null);

select string(null);

select bool(null);

select number(null);
```

<p class="example-label">Result</p>

```json
[
  null
]
```

</div>

<div class="example">

### Typeof(t(x)) Reports

Typeof(t(x)) reports the target type.

<p class="example-label">SQL</p>

```sql
select typeof(int(2.7));
```

<p class="example-label">Result</p>

```json
[
  "int"
]
```

<p class="example-label">SQL</p>

```sql
select typeof(int(2.7));

select typeof(float(5));
```

<p class="example-label">Result</p>

```json
[
  "float"
]
```

<p class="example-label">SQL</p>

```sql
select typeof(int(2.7));

select typeof(float(5));

select typeof(number(5));
```

<p class="example-label">Result</p>

```json
[
  "int"
]
```

<p class="example-label">SQL</p>

```sql
select typeof(int(2.7));

select typeof(float(5));

select typeof(number(5));

select typeof(number(2.5));
```

<p class="example-label">Result</p>

```json
[
  "float"
]
```

<p class="example-label">SQL</p>

```sql
select typeof(int(2.7));

select typeof(float(5));

select typeof(number(5));

select typeof(number(2.5));

select typeof(string(1));
```

<p class="example-label">Result</p>

```json
[
  "string"
]
```

<p class="example-label">SQL</p>

```sql
select typeof(int(2.7));

select typeof(float(5));

select typeof(number(5));

select typeof(number(2.5));

select typeof(string(1));

select typeof(bool(1));
```

<p class="example-label">Result</p>

```json
[
  "bool"
]
```

<p class="example-label">SQL</p>

```sql
select typeof(int(2.7));

select typeof(float(5));

select typeof(number(5));

select typeof(number(2.5));

select typeof(string(1));

select typeof(bool(1));

select typeof(float('4'));
```

<p class="example-label">Result</p>

```json
[
  "float"
]
```

<p class="example-label">SQL</p>

```sql
select typeof(int(2.7));

select typeof(float(5));

select typeof(number(5));

select typeof(number(2.5));

select typeof(string(1));

select typeof(bool(1));

select typeof(float('4'));

select typeof(number('7'));
```

<p class="example-label">Result</p>

```json
[
  "int"
]
```

<p class="example-label">SQL</p>

```sql
select typeof(int(2.7));

select typeof(float(5));

select typeof(number(5));

select typeof(number(2.5));

select typeof(string(1));

select typeof(bool(1));

select typeof(float('4'));

select typeof(number('7'));

select typeof(number('5.0'));
```

<p class="example-label">Result</p>

```json
[
  "float"
]
```

<p class="example-label">SQL</p>

```sql
select typeof(int(2.7));

select typeof(float(5));

select typeof(number(5));

select typeof(number(2.5));

select typeof(string(1));

select typeof(bool(1));

select typeof(float('4'));

select typeof(number('7'));

select typeof(number('5.0'));

select typeof(number(true));
```

<p class="example-label">Result</p>

```json
[
  "int"
]
```

</div>

<div class="example">

### Bad Conversions

Bad conversions fail at runtime.

<p class="example-label">SQL</p>

```sql
select int('abc');
```

Expected error: `runtime`

<p class="example-label">SQL</p>

```sql
select int('abc');

select int([1]);
```

Expected error: `runtime`

<p class="example-label">SQL</p>

```sql
select int('abc');

select int([1]);

select bool('x');
```

Expected error: `runtime`

<p class="example-label">SQL</p>

```sql
select int('abc');

select int([1]);

select bool('x');

select int(1e19);
```

Expected error: `runtime`

<p class="example-label">SQL</p>

```sql
select int('abc');

select int([1]);

select bool('x');

select int(1e19);

select float([1, 2]);
```

Expected error: `runtime`

<p class="example-label">SQL</p>

```sql
select int('abc');

select int([1]);

select bool('x');

select int(1e19);

select float([1, 2]);

select int('inf');
```

Expected error: `runtime`

<p class="example-label">SQL</p>

```sql
select int('abc');

select int([1]);

select bool('x');

select int(1e19);

select float([1, 2]);

select int('inf');

select float('nan');
```

Expected error: `runtime`

<p class="example-label">SQL</p>

```sql
select int('abc');

select int([1]);

select bool('x');

select int(1e19);

select float([1, 2]);

select int('inf');

select float('nan');

select float('inf');
```

Expected error: `runtime`

<p class="example-label">SQL</p>

```sql
select int('abc');

select int([1]);

select bool('x');

select int(1e19);

select float([1, 2]);

select int('inf');

select float('nan');

select float('inf');

select string([1, 2]);
```

Expected error: `runtime`

<p class="example-label">SQL</p>

```sql
select int('abc');

select int([1]);

select bool('x');

select int(1e19);

select float([1, 2]);

select int('inf');

select float('nan');

select float('inf');

select string([1, 2]);

select string({a: 1});
```

Expected error: `runtime`

</div>

<div class="example">

### Object/array/any Are

Object/array/any are not callable conversion functions.

<p class="example-label">SQL</p>

```sql
select object(1);
```

Expected error: `syntax`

<p class="example-label">SQL</p>

```sql
select object(1);

select array(1);
```

Expected error: `syntax`

<p class="example-label">SQL</p>

```sql
select object(1);

select array(1);

select any(1);
```

Expected error: `syntax`

</div>