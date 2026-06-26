+++
title = "Tutorial"
description = "A first look at MonaDB — connect, open a table, insert rows, and query them."
template = "docs/page.html"
weight = 2
+++

This tutorial walks through the basics in a Python REPL. MonaDB runs in-process — no server, no config file.

## Connect

```python
import monadb

db = monadb.connect("demo.db")  # omit the path for :memory:
```

## Open a table

Declare an open schema when you open the handle. Key columns are optional; here we declare an integer `id`:

```python
todos = db.table("todos", {"id": int})
```

You can also create tables with raw SQL when you prefer:

```python
db.execute("create table todos (id int);")
todos = db.table("todos", {"id": int})
```

## Insert rows

Generated-key insert (returns the key; raises on duplicate):

```python
tid = todos.insert({"id": 1, "text": "Buy milk", "done": False})
```

Upsert at a caller-supplied key:

```python
todos[1] = {"text": "Buy milk", "done": False}
```

Or via SQL:

```python
db.execute("""
    insert into todos ({id: 1, text: "Buy milk", done: false});
""")
```

## Query

Dict-style point lookup:

```python
todos[1]
```

Predicate-driven reads return a lazy `Rows` handle:

```python
for row in todos.select("done = false"):
    print(row)

open_todos = todos.select("done = false").fetchall()
```

Raw SQL still works on the connection cursor:

```python
db.execute("select * from todos where done = false order by id;")
db.fetchall()
```

Patch matching rows (whole-table updates require an explicit `None` predicate):

```python
todos.update({"done": True}, "id = ?", [1])
```

## Next steps

- Read the [Introduction](@/language/introduction.md) for how clauses compose
- Browse runnable [Queries](@/examples/queries.md) examples from the conformance suite
- See the [Python](@/python.md) API reference for the full `Table` / `Rows` surface
