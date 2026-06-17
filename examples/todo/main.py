"""An interactive TODO CRUD REPL powered by the `monadb` Python package.

Run it with:

    uv run maturin develop          # once, to build the extension
    uv run python examples/todo/main.py [db_path]

Then type commands at the `todo>` prompt (try `help`).

What this demonstrates about driving monadb from Python:

  * ``monadb.connect(path)`` opens a persistent, file-backed database.
  * ``db.table("todos")`` returns a dict-like :class:`~monadb.table.Table` handle.
  * ``create`` / ``insert`` / ``get`` / ``delete`` / iteration map CRUD onto
    monadb's keyed document model without hand-built SQL.
  * monadb has no ``UPDATE`` statement — re-inserting an object whose key
    already exists overwrites that row (upsert), so "toggle done" is a
    read-modify-write via :meth:`~monadb.table.Table.insert`.
"""

import sys

import monadb

HELP = """commands:
  add <text>   add a todo
  list         list all todos
  done <id>    toggle a todo's done flag
  rm <id>      remove a todo
  clear        remove all todos
  help         show this help
  quit         exit"""


def _id(row):
    """monadb represents numbers as f64, so a keyed ``int`` id reads back as e.g.
    ``1.0``; present it as a plain int."""
    return int(row["id"])


def add(todos, text):
    text = text.strip()
    if not text:
        return "usage: add <text>"
    tid = max((_id(r) for r in todos), default=0) + 1
    todos.insert({"id": tid, "text": text, "done": False})
    return f"added #{tid}"


def toggle_done(todos, tid):
    row = todos.get(tid)
    if row is None:
        return f"no todo #{tid}"
    new_done = not row["done"]
    todos.insert({"id": tid, "text": row["text"], "done": new_done})
    return f"#{tid} {'done' if new_done else 'todo'}"


def remove(todos, tid):
    if todos.get(tid) is None:
        return f"no todo #{tid}"
    todos.delete(id=tid)
    return f"removed #{tid}"


def clear(db):
    db.execute("delete from todos;")
    return "cleared"


def render_list(todos):
    rows = sorted(todos, key=_id)
    if not rows:
        return "(no todos)"
    return "\n".join(
        f"  [{'x' if r['done'] else ' '}] {_id(r)}  {r['text']}" for r in rows
    )


def _with_id(arg, fn):
    try:
        tid = int(arg.strip())
    except ValueError:
        return f"expected a numeric id, got {arg.strip()!r}"
    return fn(tid)


def dispatch(db, todos, line):
    """Route one input line to an operation; returns text to print, or None."""
    line = line.strip()
    if not line:
        return None
    cmd, _, arg = line.partition(" ")
    cmd = cmd.lower()
    if cmd in ("list", "ls"):
        return render_list(todos)
    if cmd == "add":
        return add(todos, arg)
    if cmd == "done":
        return _with_id(arg, lambda tid: toggle_done(todos, tid))
    if cmd in ("rm", "remove", "del"):
        return _with_id(arg, lambda tid: remove(todos, tid))
    if cmd == "clear":
        return clear(db)
    if cmd == "help":
        return HELP
    return f"unknown command: {cmd!r} (type 'help')"


def main():
    db_path = sys.argv[1] if len(sys.argv) > 1 else "todos.db"
    db = monadb.connect(db_path)
    todos = db.table("todos")

    try:
        todos.create(id=int)
    except monadb.Error:
        pass

    print(f"todo — monadb @ {db_path}  (type 'help')")
    try:
        while True:
            try:
                line = input("todo> ")
            except (EOFError, KeyboardInterrupt):
                print()
                break
            if line.strip().lower() in ("quit", "exit"):
                break
            out = dispatch(db, todos, line)
            if out is not None:
                print(out)
    finally:
        db.close()


if __name__ == "__main__":
    main()
