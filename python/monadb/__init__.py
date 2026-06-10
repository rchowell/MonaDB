"""MonaDB — a minimal, DuckDB-style Python interface.

The compiled extension (`monadb._monadb`) provides the `connect()` constructor,
the `Connection` cursor class, and the `Error` exception. This module re-exports
them and adds DuckDB-style module-level convenience functions that operate on a
lazily-created default in-memory connection.
"""

from ._monadb import Connection, Error, connect

__all__ = [
    "connect",
    "Connection",
    "Error",
    "execute",
    "sql",
    "fetchone",
    "fetchmany",
    "fetchall",
]

_default: "Connection | None" = None


def _default_connection() -> "Connection":
    """The shared default in-memory connection, created on first use."""
    global _default
    if _default is None:
        _default = connect()
    return _default


def execute(query: str, parameters=None) -> "Connection":
    """Run ``query`` on the default connection; returns the connection (cursor)."""
    return _default_connection().execute(query, parameters)


def sql(query: str, parameters=None) -> "Connection":
    """Alias of :func:`execute` on the default connection."""
    return _default_connection().sql(query, parameters)


def fetchone():
    """Fetch the next row from the default connection's last result."""
    return _default_connection().fetchone()


def fetchmany(size: int = 1):
    """Fetch up to ``size`` rows from the default connection's last result."""
    return _default_connection().fetchmany(size)


def fetchall():
    """Fetch all remaining rows from the default connection's last result."""
    return _default_connection().fetchall()
