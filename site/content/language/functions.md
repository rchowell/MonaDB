+++
title = "Functions"
description = "Function call syntax, aggregate functions, string functions, and read()."
weight = 6
+++

# Functions

## Call syntax

Functions are called by name with positional arguments. Named arguments use `name: value` syntax and may follow positional arguments in any order.

```
upper('hello')
round(3.14159, 2)
read('examples/data.jsonl', format: 'jsonl')
```

## Aggregate functions

Aggregate functions reduce a group of rows to a single value. They are valid only inside a `select` with a `group by` clause, or as the outermost expression in a scalar `select`.

| Function | Description |
|----------|-------------|
| `count(*)` | Number of rows in the group |
| `count(expr)` | Number of non-null values |
| `sum(expr)` | Sum of numeric values |
| `avg(expr)` | Average of numeric values |
| `min(expr)` | Minimum value |
| `max(expr)` | Maximum value |

```
select { tag: tag, n: count(*) }
  from items
 group by tag;
```

## String functions

| Function | Description |
|----------|-------------|
| `upper(str)` | Uppercase |
| `lower(str)` | Lowercase |
| `length(str)` | Character count |
| `trim(str)` | Strip leading and trailing whitespace |
| `substr(str, start, len)` | Substring by character offset |
| `contains(str, substr)` | Returns `true` if str contains substr |

## read()

`read(path)` reads a file and returns its contents as a value or row sequence for use in `from`. Format is inferred from the file extension; override with the `format:` named argument.

```
select * from read('examples/data.jsonl') as row;
select * from read('records.csv', header: true) as row;
```

Supported formats: `jsonl`, `csv`, `tsv`.
