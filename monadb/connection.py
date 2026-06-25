from __future__ import annotations

from typing import Any, Dict, List, Optional, Tuple

from monadb import _monadb
from monadb._monadb import Error, Statement as _Statement
from monadb.table import Table
from monadb.types import ConnectConfig


class Statement:
    """A SQL statement prepared for repeated execution."""

    def __init__(self, engine: _Statement):
        self._engine = engine

    def execute(self, parameters: Any = None) -> "Statement":
        """Run the prepared statement and return ``self`` for chaining."""
        if parameters is None:
            self._engine.execute()
        else:
            self._engine.execute(parameters)
        return self

    @property
    def sql(self) -> str:
        """Return the original SQL text passed to ``prepare``."""
        return self._engine.sql

    @property
    def description(self) -> Optional[list]:
        """DBAPI-style column metadata from the last result's first row."""
        desc = self._engine.description
        if desc is None:
            return None
        return list(desc)

    def fetchone(self) -> Optional[object]:
        """Return the next buffered row, or ``None`` when exhausted."""
        return self._engine.fetchone()

    def fetchmany(self, size: int = 1) -> List[object]:
        """Return up to ``size`` rows from the buffer."""
        return self._engine.fetchmany(size)

    def fetchall(self) -> List[object]:
        """Return all remaining buffered rows."""
        return self._engine.fetchall()


class Connection:
    """A connection to a local MonaDB database."""

    def __init__(
        self,
        database: Optional[str] = None,
        *,
        read_only: bool = False,
        config: ConnectConfig | None = None,
    ):
        self._engine = _monadb.connect(
            database, read_only=read_only, config=config
        )
        self._result: List[object] = []
        self._cursor = 0
        self._closed = False
        self._keys: Dict[str, Tuple[str, ...]] = {}

    def execute(self, sql: str, parameters: Any = None) -> "Connection":
        """Run ``sql`` (optionally with ``parameters``), buffer its rows, and
        return ``self`` for chaining. ``parameters`` is a list/tuple for
        positional (``?``, ``$N``) or a dict for named (``$name``) placeholders.
        """
        self._ensure_open()
        if parameters is None:
            self._result = self._engine.execute(sql).fetchall()
        else:
            self._result = self._engine.execute(sql, parameters).fetchall()
        self._cursor = 0
        return self

    def sql(self, query: str, parameters: Any = None) -> "Connection":
        """Alias of :meth:`execute`."""
        return self.execute(query, parameters)

    def prepare(self, sql: str) -> Statement:
        """Parse and cache ``sql`` for repeated execution."""
        self._ensure_open()
        return Statement(self._engine.prepare(sql))

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

        This is the only way to obtain a table handle on a connection.

        Pass ``keys`` when the table already exists and its key columns were not
        declared via :meth:`~monadb.table.Table.create` on this connection.
        """
        if keys is not None:
            self._keys[name] = (keys,) if isinstance(keys, str) else tuple(keys)
        return Table(self, name)

    def key_columns(self, name: str) -> Optional[Tuple[str, ...]]:
        """Return known key columns for ``name``, or ``None`` if unknown."""
        return self._keys.get(name)

    def _ensure_open(self) -> None:
        if self._closed:
            raise Error("connection is closed")
