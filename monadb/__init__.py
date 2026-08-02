"""MonaDB: an embedded document store with Python dict semantics.

    import monadb

    db = monadb.open("app.db")
    db["users"]["alice"] = {"age": 30}

    with db.transaction() as tx:
        tx["users"]["bob"] = {"age": 41}
        del tx["users"]["alice"]

Only one process may open a database for writing: redb takes an exclusive file
lock.
"""

import os

from ._monadb import BusyError, Error, TransactionError
from ._monadb import open as _open_raw
from .collection import Collection
from .db import Database, Transaction

__all__ = [
    "open",
    "Database",
    "Transaction",
    "Collection",
    "Error",
    "BusyError",
    "TransactionError",
]


def open(path=None, *, timeout=5.0, durable=True):
    """Open a database: in-memory when ``path`` is ``None``, else file-backed.

    ``timeout`` bounds the wait for the write gate, in seconds; exceeding it
    raises :class:`BusyError`. ``durable=False`` trades commit durability for
    speed, which suits bulk loads.
    """
    p = os.fspath(path) if path is not None else None
    return Database(_open_raw(p, timeout=timeout, durable=durable))
