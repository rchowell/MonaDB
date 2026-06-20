"""MonaDB — an embedded, dict-like SQL engine with a Python API.

Open a database with :func:`connect` (``None`` / ``":memory:"`` for in-memory,
or a filesystem path). Each connection is a DuckDB-style SQL cursor
(``execute``/``sql`` + ``fetch*``) and a namespace of dict-like table handles
(``db.table("table")``).

The compiled extension (``monadb._monadb``) is the in-process engine; the
high-level façade is pure Python layered over it.
"""

from ._monadb import Error
from .connection import Connection, PreparedStatement
from .table import Table

__all__ = [
    "connect",
    "Connection",
    "PreparedStatement",
    "Table",
    "Error",
    "execute",
    "sql",
    "fetchone",
    "fetchmany",
    "fetchall",
]


def connect(database: "str | None" = None, read_only: bool = False) -> Connection:
    return Connection(database, read_only)


_conn: "Connection | None" = None


def _connection() -> Connection:
    """The shared default in-memory connection, created on first use."""
    global _conn
    if _conn is None:
        _conn = connect()
    return _conn


def execute(query: str, parameters=None) -> Connection:
    """Run ``query`` on the default connection; returns the connection (cursor)."""
    return _connection().execute(query, parameters)


def sql(query: str, parameters=None) -> Connection:
    """Alias of :func:`execute` on the default connection."""
    return _connection().sql(query, parameters)


def fetchone():
    """Fetch the next row from the default connection's last result."""
    return _connection().fetchone()


def fetchmany(size: int = 1):
    """Fetch up to ``size`` rows from the default connection's last result."""
    return _connection().fetchmany(size)


def fetchall():
    """Fetch all remaining rows from the default connection's last result."""
    return _connection().fetchall()
