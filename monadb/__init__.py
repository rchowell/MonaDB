"""MonaDB — an embedded, dict-like SQL engine with a Python API.

Open a database with :func:`connect` (``None`` / ``":memory:"`` for in-memory,
or a filesystem path). Each connection exposes:

* A SQL surface (``execute`` / ``sql`` / ``prepare``) where reads return plain
  Python lists.
* Dict-like :class:`~monadb.table.Table` handles with a complementary SQL-like
  surface.

The compiled extension (``monadb._monadb``) is the in-process engine; the
high-level façade is pure Python layered over it.
"""

from typing import Any, List, Optional

from ._monadb import DuplicateKeyError, Error
from .types import Config
from .statement import Statement
from .table import Table
from .connection import Connection

__all__ = [
    "connect",
    "Connection",
    "Statement",
    "Table",
    "Config",
    "Error",
    "DuplicateKeyError",
    "execute",
    "sql",
]


def connect(
    path: "str | None" = None,
    *,
    config: Config | None = None,
) -> Connection:
    """Opens a connection to a MonaDB database at the given path.

    Args:
        path: Filesystem path, or ``None`` / ``":memory:"`` for in-memory.
        config: Open-time settings (see :class:`~monadb.types.Config`).

    Returns:
        A :class:`~monadb.connection.Connection` handle.
    """
    return Connection(path, config=config)


_conn: "Connection | None" = None


def _connection() -> Connection:
    """Return the shared default in-memory connection, created on first use."""
    global _conn
    if _conn is None:
        _conn = connect()
    return _conn


def execute(query: str, parameters: Any = None) -> List[Any]:
    """Run ``query`` on the default connection and return its rows as a list.

    Args:
        query: SQL statement text.
        parameters: Optional parameter bindings.

    Returns:
        The result rows as a list.
    """
    return _connection().execute(query, parameters)


def sql(query: str, parameters: Any = None) -> List[Any]:
    """Alias of :func:`execute` on the default connection."""
    return _connection().sql(query, parameters)
