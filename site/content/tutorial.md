+++
title = "Tutorial"
description = "A first look at MonaDB — connect, create a table, insert rows, and query them."
template = "docs/page.html"
weight = 2
+++

This tutorial walks through the basics in a Python REPL. MonaDB runs in-process — no server, no config file.

## Connect

```python
import monadb

db = monadb.connect("demo.db")  # omit the path for :memory:
```

## Create a table

Tables are created with SQL. Key columns are optional; here we declare an integer `id`:

```python
db.execute("create table todos (id int);")
```

You can also use the dict-like table API:

```python
todos = db.table("todos")
todos.create(id=int)
```

## Insert rows

```python
db.execute("""
    insert into todos ({id: 1, text: "Buy milk", done: false});
    insert into todos ({id: 2, text: "Walk the dog", done: false});
""")
```

Or via the table handle:

```python
todos.insert({"id": 1, "text": "Buy milk", "done": False})
```

Re-inserting a row with the same key overwrites it — MonaDB has no separate `UPDATE` statement.

## Query

```python
db.execute("select * from todos where done = false order by id;")
db.fetchall()
# [{'id': 1, 'text': 'Buy milk', 'done': False}, ...]
```

`select` maps each binding to an output value. `from` iterates rows (or collection elements). `where` filters, `order by` sorts, and `limit` slices the stream.

## Next steps

- Read the [Introduction](@/language/introduction.md) for how clauses compose
- Browse runnable [Queries](@/examples/queries.md) examples from the conformance suite
- See the [Python](@/python.md) API reference for connection and table methods
