from __future__ import annotations

from typing import Any, Iterator, Optional, Tuple

from .encode import encode, encode_ident, type_name


class Table:
    """Dict-like handle to one table on a :class:`~monadb.connection.Connection`.

    Example:
        >>> import monadb
        >>> con = monadb.connect()
        >>> users = con["users"]
        >>> users.create(id=int)
        >>> users.insert({"id": 1, "name": "Ada"})
        >>> users[1]
        {'id': 1, 'name': 'Ada'}
    """

    def __init__(self, connection: "object", name: str):
        self._conn = connection
        self._name = name

    @property
    def name(self) -> str:
        """Return this table's name.

        Example:
            >>> import monadb
            >>> monadb.connect()["items"].name
            'items'
        """
        return self._name

    def __repr__(self) -> str:
        return f"Table({self._name!r})"

    def create(self, **columns: Any) -> "Table":
        """Create this table and declare its key columns.

        Keyword arguments name the composite key in declaration order. Omit them
        for a keyless table (append-only log).

        Args:
            **columns: ``column_name=int`` or ``column_name=str`` for each key
                column.

        Returns:
            This table handle, for chaining.

        Example:
            >>> import monadb
            >>> con = monadb.connect()
            >>> con["events"].create(ts=int, kind=str)
            >>> con["log"].create()  # no key columns
        """
        table = encode_ident(self._name)
        if columns:
            cols = ", ".join(
                f"{encode_ident(col)} {type_name(typ)}" for col, typ in columns.items()
            )
            sql = f"create table {table} ({cols});"
        else:
            sql = f"create table {table};"
        self._conn.execute(sql)
        self._conn._keys[self._name] = tuple(columns.keys())
        return self

    def insert(self, rows: Any) -> "Table":
        """Insert one row or many rows into this table.

        Args:
            rows: A single row dict, or an iterable of row dicts. Extra fields
                beyond the declared key columns are stored as payload.

        Returns:
            This table handle, for chaining.

        Example:
            >>> import monadb
            >>> t = monadb.connect()["scores"]
            >>> t.create(player=str)
            >>> t.insert({"player": "ada", "score": 100})
            >>> t.insert([{"player": "linus", "score": 95},
            ...           {"player": "grace", "score": 99}])
        """
        if isinstance(rows, dict):
            rows = [rows]
        objs = ", ".join(encode(row) for row in rows)
        self._conn.execute(f"insert into {encode_ident(self._name)} ({objs});")
        return self

    def get(self, *keys: Any, **named: Any) -> Any:
        """Look up rows by key value(s).

        A full key returns the matching row, or ``None`` if absent. A partial
        key (prefix of the composite key) returns a list of all matching rows.

        Args:
            *keys: Key values in column order.
            **named: Same values by column name (``get(a="x", b=7)``).

        Returns:
            A row dict, a list of row dicts, or ``None``.

        Example:
            >>> import monadb
            >>> c = monadb.connect()["c"]
            >>> c.create(a=str, b=int)
            >>> c.insert([{"a": "x", "b": 7}, {"a": "x", "b": 8}])
            >>> c.get("x", 7)          # full key → row
            {'a': 'x', 'b': 7}
            >>> c.get(a="x")           # partial key → list
            [{'a': 'x', 'b': 7}, {'a': 'x', 'b': 8}]
            >>> c.get("x", 99) is None
            True
        """
        key_vals = list(keys) + list(named.values())
        if not key_vals:
            raise TypeError("get() requires at least one key value")
        subscript = ", ".join(encode(k) for k in key_vals)
        rows = self._conn.execute(
            f"select {encode_ident(self._name)}[{subscript}];"
        ).fetchall()
        return rows[0] if rows else None

    def delete(self, *, where: Optional[str] = None, **eq: Any) -> "Table":
        """Delete rows matching key predicates or a raw ``where`` clause.

        To delete every row, use ``connection.execute("delete from …")`` —
        bare ``delete()`` is rejected to avoid accidents.

        Args:
            where: Raw SQL predicate (``where="score > 90"``).
            **eq: Key-column equality, e.g. ``delete(x=1)`` or
                ``delete(a="x", b=7)``.

        Returns:
            This table handle, for chaining.

        Raises:
            TypeError: When called with no predicates.

        Example:
            >>> import monadb
            >>> t = monadb.connect()["t"]
            >>> t.create(k=int)
            >>> t.insert([{"k": 1}, {"k": 2}])
            >>> t.delete(k=1)
            >>> t.get(1) is None
            True
        """
        table = encode_ident(self._name)
        if where is not None:
            self._conn.execute(f"delete from {table} where {where};")
        elif eq:
            preds = " and ".join(
                f"{table}.{encode_ident(col)} = {encode(v)}" for col, v in eq.items()
            )
            self._conn.execute(f"delete from {table} where {preds};")
        else:
            raise TypeError(
                "delete() needs key predicates or where=; "
                "use execute('delete from <table>') to clear all rows"
            )
        return self

    def __getitem__(self, key: Any) -> Any:
        """Dict-style lookup by key.

        A scalar or tuple subscript is a key lookup. A full key returns the row
        and raises ``KeyError`` when absent; a partial key returns a list.

        Args:
            key: One key value, or a tuple of values for a composite key.

        Returns:
            A row dict or a list of row dicts.

        Raises:
            KeyError: When a full key does not match any row.

        Example:
            >>> import monadb
            >>> c = monadb.connect()["c"]
            >>> c.create(a=str, b=int)
            >>> c.insert({"a": "x", "b": 7})
            >>> c["x", 7]              # composite full key
            {'a': 'x', 'b': 7}
            >>> c["x"]                 # partial prefix
            [{'a': 'x', 'b': 7}]
        """
        rows = self.get(*_as_tuple(key))
        if rows is None:
            raise KeyError(key)
        return rows

    def __contains__(self, key: Any) -> bool:
        """Return whether a full or partial key matches at least one row.

        Example:
            >>> import monadb
            >>> t = monadb.connect()["t"]
            >>> t.create(k=int)
            >>> t.insert({"k": 1})
            >>> 1 in t
            True
            >>> 99 in t
            False
        """
        return bool(self.get(*_as_tuple(key)))

    def __iter__(self) -> Iterator[Any]:
        """Iterate over every row in the table.

        Example:
            >>> import monadb
            >>> t = monadb.connect()["t"]
            >>> t.create(k=int)
            >>> t.insert([{"k": 1}, {"k": 2}])
            >>> sorted(row["k"] for row in t)
            [1, 2]
        """
        rows = self._conn.execute(
            f"select * from {encode_ident(self._name)};"
        ).fetchall()
        return iter(rows)

    def __len__(self) -> int:
        """Return the number of rows in the table.

        Example:
            >>> import monadb
            >>> t = monadb.connect()["t"]
            >>> t.create(k=int)
            >>> t.insert([{"k": 1}, {"k": 2}, {"k": 3}])
            >>> len(t)
            3
        """
        rows = self._conn.execute(
            f"select count(*) from {encode_ident(self._name)};"
        ).fetchall()
        return int(rows[0]) if rows else 0

    def __delitem__(self, key: Any) -> None:
        """Delete the row at a full key.

        Requires the table's key columns to be known (via :meth:`create` or
        ``connection.table(name, keys=…)``).

        Args:
            key: The full composite key as a scalar or tuple.

        Raises:
            KeyError: When ``key`` is not a full key.
            TypeError: When key columns are unknown.

        Example:
            >>> import monadb
            >>> t = monadb.connect()["t"]
            >>> t.create(k=int)
            >>> t.insert([{"k": 1}, {"k": 2}])
            >>> del t[1]
            >>> 1 in t
            False
        """
        cols = self._key_columns()
        vals = _as_tuple(key)
        if len(vals) != len(cols):
            raise KeyError(f"deleting a row requires the full key {cols}, got {key!r}")
        self.delete(**dict(zip(cols, vals)))

    def __setitem__(self, key: Any, value: Any) -> None:
        """Replace (or insert) the row at a full key.

        The subscript key wins over duplicate key fields in ``value``. Requires
        known key columns (see :meth:`__delitem__`).

        Args:
            key: The full composite key as a scalar or tuple.
            value: Row payload as a dict.

        Raises:
            KeyError: When ``key`` is not a full key.
            TypeError: When ``value`` is not a dict or key columns are unknown.

        Example:
            >>> import monadb
            >>> t = monadb.connect()["t"]
            >>> t.create(k=int)
            >>> t.insert({"k": 2, "v": "old"})
            >>> t[2] = {"v": "new"}
            >>> t[2]["v"]
            'new'
        """
        cols = self._key_columns()
        vals = _as_tuple(key)
        if len(vals) != len(cols):
            raise KeyError(f"setting a row requires the full key {cols}, got {key!r}")
        if not isinstance(value, dict):
            raise TypeError("row value must be a dict")
        row = {**value, **dict(zip(cols, vals))}
        self.insert([row])

    def _key_columns(self) -> Tuple[str, ...]:
        cols = self._conn.key_columns(self._name)
        if not cols:
            raise TypeError(
                f"key columns for {self._name!r} are unknown; create the table "
                "via create() or pass keys= to connection.table()"
            )
        return cols


def _as_tuple(key: Any) -> Tuple[Any, ...]:
    """Normalize a subscript key to a tuple of key-column values."""
    return key if isinstance(key, tuple) else (key,)
