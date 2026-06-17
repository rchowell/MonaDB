# TODO Application

A tiny interactive todo list backed by [MonaDB](../../README.md) through its Python
package. It's a worked example of CRUD via the dict-like table API.

## Run

```sh
# once: build the monadb extension into the venv
uv run maturin develop

# opens ./todos.db (pass a path to use another file)
uv run python examples/todo/main.py
```

## Example

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

## Reference

| Action    | Table API                                              |
| --------- | ------------------------------------------------------ |
| (startup) | `todos.create(id=int)`                                 |
| `add`     | `todos.insert({"id": N, "text": "...", "done": False})` |
| `list`    | `for row in todos: ...`                                |
| `done`    | `todos.get(id)` then `todos.insert(...)` with `done` flipped |
| `rm`      | `todos.delete(id=N)`                                   |
| `clear`   | `db.execute("delete from todos;")`                    |
