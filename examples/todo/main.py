"""An interactive TODO CRUD REPL powered by the `monadb` Python package.

Run it with:

    uv run maturin develop          # once, to build the extension
    uv run python examples/todo/main.py [db_path]

Then type commands at the `todo>` prompt (try `help`).

What this demonstrates about driving monadb from Python:

  * `monadb.connect(path)` opens a persistent, file-backed database.
  * CRUD maps onto monadb's document SQL: keyed `create table`, object-literal
    `insert`, `select [where]`, and `delete [where]`.
  * monadb has no `UPDATE` statement — but inserting an object whose key already
    exists *overwrites* that row (upsert), so "toggle done" is a read-modify-write:
    read the row, flip `done`, re-insert it under the same `id`.
  * monadb has no bind parameters, so user text is interpolated into the SQL,
    delimited by a quote character the text doesn't contain (see `_quote`).
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


def _quote(text):
    """Render `text` as a monadb string literal.

    monadb ends a string at the first unescaped delimiter and its value decoder
    doesn't process escape sequences, so the robust trick is to delimit with a
    quote character the text doesn't contain. Apostrophes are common in todos,
    so prefer double quotes; fall back to single quotes, and as a last resort
    (text contains both) drop the rarer double quote to keep the SQL valid."""
    if '"' not in text:
        return '"' + text + '"'
    if "'" not in text:
        return "'" + text + "'"
    return '"' + text.replace('"', "") + '"'


def _insert_sql(tid, text, done):
    """An upsert: re-inserting an existing `id` overwrites that row."""
    done_sql = "true" if done else "false"
    return f"insert into todos ({{id: {tid}, text: {_quote(text)}, done: {done_sql}}});"


def _ensure_schema(con):
    """Create the keyed `todos` table, ignoring the error if it already exists
    (monadb has no `CREATE TABLE IF NOT EXISTS`)."""
    try:
        con.execute("create table todos (id int);")
    except monadb.Error:
        pass


def _id(row):
    """monadb represents numbers as f64, so a keyed `int` id reads back as e.g.
    `1.0`; present it as a plain int."""
    return int(row["id"])


def _all(con):
    """All todos, sorted by id (a keyed table already returns key order; we sort
    defensively so display order never surprises)."""
    rows = con.execute("select * from todos;").fetchall()
    return sorted(rows, key=_id)


def _find(con, tid):
    rows = con.execute(f"select * from todos where todos.id = {tid};").fetchall()
    return rows[0] if rows else None


def add(con, text):
    text = text.strip()
    if not text:
        return "usage: add <text>"
    tid = max((_id(r) for r in _all(con)), default=0) + 1  # no COUNT/MAX in monadb
    con.execute(_insert_sql(tid, text, done=False))
    return f"added #{tid}"


def toggle_done(con, tid):
    row = _find(con, tid)
    if row is None:
        return f"no todo #{tid}"
    new_done = not row["done"]
    con.execute(_insert_sql(tid, row["text"], done=new_done))  # upsert == update
    return f"#{tid} {'done' if new_done else 'todo'}"


def remove(con, tid):
    if _find(con, tid) is None:
        return f"no todo #{tid}"
    con.execute(f"delete from todos where todos.id = {tid};")
    return f"removed #{tid}"


def clear(con):
    con.execute("delete from todos;")
    return "cleared"


def render_list(con):
    rows = _all(con)
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


def dispatch(con, line):
    """Route one input line to an operation; returns text to print, or None."""
    line = line.strip()
    if not line:
        return None
    cmd, _, arg = line.partition(" ")
    cmd = cmd.lower()
    if cmd in ("list", "ls"):
        return render_list(con)
    if cmd == "add":
        return add(con, arg)
    if cmd == "done":
        return _with_id(arg, lambda tid: toggle_done(con, tid))
    if cmd in ("rm", "remove", "del"):
        return _with_id(arg, lambda tid: remove(con, tid))
    if cmd == "clear":
        return clear(con)
    if cmd == "help":
        return HELP
    return f"unknown command: {cmd!r} (type 'help')"


def main():
    db_path = sys.argv[1] if len(sys.argv) > 1 else "todos.db"
    con = monadb.connect(db_path)
    _ensure_schema(con)
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
            out = dispatch(con, line)
            if out is not None:
                print(out)
    finally:
        con.close()


if __name__ == "__main__":
    main()
