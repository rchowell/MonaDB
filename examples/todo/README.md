# TODO Application

A tiny interactive todo list backed by [MonaDB](../../README.md) through its python
package. It's a worked example of doing CRUD against monadb from python.

## Run

```sh
# once: build the monadb extension into the venv
uv run maturin develop

# opens ./todos.db (pass a path to use another file)
uv run python examples/todo/main.py
```

## Session

```
todo — monadb @ todos.db  (type 'help')
todo> add Buy milk
added #1
todo> add Walk the dog
added #2
todo> list
  [ ] 1  Buy milk
  [ ] 2  Walk the dog
todo> done 1
#1 done
todo> list
  [x] 1  Buy milk
  [ ] 2  Walk the dog
todo> rm 2
removed #2
todo> quit
```

Data persists in the database file, so todos survive between sessions.

## How it maps to monadb

| command      | monadb SQL                                                        |
|--------------|------------------------------------------------------------------|
| (startup)    | `create table todos (id int);`                                   |
| `add`        | `insert into todos ({id: N, text: '...', done: false});`         |
| `list`       | `select * from todos;`                                            |
| `done`       | read the row, then re-`insert` it with `done` flipped            |
| `rm`         | `delete from todos where todos.id = N;`                           |
| `clear`      | `delete from todos;`                                              |

monadb has no `UPDATE` statement, so `done` relies on **upsert**: inserting an
object whose key (`id`) already exists overwrites that row. And because monadb has
no bind parameters, the example quotes user text by delimiting it with a quote
character the text doesn't contain (so apostrophes like `O'Brien` just work).
