from __future__ import annotations

from typing import Any, Dict, List, Optional, Tuple

from monadb import _monadb
from monadb._monadb import Error
from monadb.table import Table


class Connection:
    """A connection to a local MonaDB database."""

    def __init__(self, database: Optional[str] = None, read_only: bool = False):
        self._engine = _monadb.connect(database, read_only)
        self._result: List[object] = []
        self._cursor = 0
        self._closed = False
        self._keys: Dict[str, Tuple[str, ...]] = {}

    def execute(self, sql: str, parameters: Any = None) -> "Connection":
        """Run ``sql``, buffer its rows, and return ``self`` for chaining."""
        if parameters is not None:
            raise NotImplementedError("parameterized queries are not supported yet")
        self._ensure_open()
        self._result = self._engine.execute(sql).fetchall()
        self._cursor = 0
        return self

    def sql(self, query: str, parameters: Any = None) -> "Connection":
        """Alias of :meth:`execute`."""
        return self.execute(query, parameters)

    def fetchone(self) -> Optional[object]:
        """Return the next buffered row, or ``None`` when exhausted."""
        self._ensure_open()
        if self._cursor < len(self._result):
            row = self._result[self._cursor]
            self._cursor += 1
            return row
        return None

    def fetchmany(self, size: int = 1) -> List[object]:
        """Return up to ``size`` rows from the buffer."""
        self._ensure_open()
        end = min(self._cursor + size, len(self._result))
        rows = self._result[self._cursor:end]
        self._cursor = end
        return rows

    def fetchall(self) -> List[object]:
        """Return all remaining buffered rows."""
        self._ensure_open()
        rows = self._result[self._cursor:]
        self._cursor = len(self._result)
        return rows

    @property
    def description(self) -> Optional[list]:
        """DBAPI-style column metadata from the last result's first row.

        ``[(name, None, None, None, None, None, None), ...]``, or ``None`` when
        the rows are not objects.
        """
        if not self._result:
            return None
        first = self._result[0]
        if not isinstance(first, dict):
            return None
        return [(name, None, None, None, None, None, None) for name in first]

    def close(self) -> None:
        """Close the connection; subsequent operations raise ``monadb.Error``."""
        if not self._closed:
            self._engine.close()
            self._closed = True

    def __enter__(self) -> "Connection":
        return self

    def __exit__(self, *_exc) -> bool:
        self.close()
        return False

    def table(self, name: str, keys: Any = None) -> Table:
        """Return a :class:`~monadb.table.Table` handle for ``name``.

        Pass ``keys`` when the table already exists and its key columns were not
        declared via :meth:`~monadb.table.Table.create` on this connection.
        """
        if keys is not None:
            self._keys[name] = (keys,) if isinstance(keys, str) else tuple(keys)
        return Table(self, name)

    def __getitem__(self, name: str) -> Table:
        return self.table(name)

    def key_columns(self, name: str) -> Optional[Tuple[str, ...]]:
        """Return known key columns for ``name``, or ``None`` if unknown."""
        return self._keys.get(name)

    def _ensure_open(self) -> None:
        if self._closed:
            raise Error("connection is closed")
