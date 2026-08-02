import os

from ._monadb import Error
from ._monadb import open as _open
from .collection import Collection
from .db import Database

__all__ = [
    "Collection",
    "Database",
    "Error",
    "open",
]


def open(path=None, *, durable=True):
    """Open a MonaDB database.

    Args:
        path (str or os.PathLike or None): Filesystem path to the database file.
            If None, the database is opened in memory.
        durable (bool): If True (default), each commit is durable (synced to disk).
            If False, commits may be buffered for speed.

    Returns:
        Database: A Database object.
    """
    p = os.fspath(path) if path is not None else None
    return Database(_open(p, durable=durable))
