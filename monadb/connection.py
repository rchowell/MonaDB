from __future__ import annotations

from typing import Any, Dict, List, Optional, Tuple

from monadb._monadb import Error, _Connection, connect as _connect
from monadb.encode import create_table_sql
from monadb.statement import Statement
from monadb.table import Table
from monadb.types import Config


class Connection:
    """A connection to a MonaDB database.

    Reads return plain Python lists (a list of row dicts/scalars); there is no
    cursor or ``fetch*`` buffer. Writes and DDL return an empty list.
    """

    _conn: _Connection

    def __init__(
        self,
        path: Optional[str] = None,
        *,
        config: Config | None = None,
    ):
        self._conn = _connect(path, config=config)
        self._schemas: Dict[str, Tuple[str, ...]] = {}
        self._opened: set[str] = set()
        self._closed = False

    def execute(self, sql: str, parameters: Any = None) -> List[Any]:
        """Run ``sql`` and return its rows as a list (empty for writes/DDL).

        Args:
            sql: SQL statement text.
            parameters: Positional (list/tuple) or named (dict) bindings.

        Returns:
            The result rows as a list of dicts (or unwrapped scalars).
        """
        self._ensure_open()
        if parameters is None:
            return self._conn.execute(sql)
        return self._conn.execute(sql, parameters)

    def sql(self, query: str, parameters: Any = None) -> List[Any]:
        """Alias of :meth:`execute`."""
        return self.execute(query, parameters)

    def prepare(self, sql: str) -> Statement:
        """Parse and cache ``sql`` for repeated execution.

        Args:
            sql: SQL statement text.

        Returns:
            A prepared statement handle.
        """
        self._ensure_open()
        return Statement(self._conn.prepare(sql))

    def table(self, name: str, schema: Optional[Dict[str, Any]] = None) -> Table:
        """Return a :class:`~monadb.table.Table` handle for ``name``.

        Args:
            name: Table name.
            schema: Open-schema type hint — declared fields are key columns
                (``int`` or ``str``). When the table does not yet exist, it is
                created from this schema.

        Returns:
            A table handle exposing dict-like and SQL-like surfaces.
        """
        self._ensure_open()
        if schema is not None:
            self._schemas[name] = tuple(schema.keys())
        if name not in self._opened:
            rows = self.execute("select catalog.name from catalog;")
            names = {row["name"] for row in rows if isinstance(row, dict)}
            if name not in names:
                if schema is None:
                    raise Error(
                        f"table {name!r} does not exist and no schema was provided"
                    )
                self.execute(create_table_sql(name, schema))
            self._opened.add(name)
        return Table(self, name, schema)

    def schema_columns(self, name: str) -> Optional[Tuple[str, ...]]:
        """Return the recorded key columns for ``name``, or ``None`` if unknown."""
        return self._schemas.get(name)

    def _mutations(self, sql: str, params: Optional[List[Any]] = None) -> int:
        """Run a mutating statement and return the number of rows changed."""
        self._ensure_open()
        if params is None:
            return int(self._conn.execute_mutations(sql))
        return int(self._conn.execute_mutations(sql, params))

    def peek_keyless_row_id(self, table: str) -> int:
        """Return the surrogate id the next keyless insert would allocate."""
        self._ensure_open()
        return int(self._conn.peek_keyless_row_id(table))

    def close(self) -> None:
        """Close the connection."""
        if not self._closed:
            self._conn.close()
            self._closed = True
            self._conn = None  # type: ignore[assignment]

    def __enter__(self) -> "Connection":
        return self

    def __exit__(self, *_exc) -> bool:
        self.close()
        return False

    def _ensure_open(self) -> None:
        if self._closed:
            raise Error("connection is closed")
