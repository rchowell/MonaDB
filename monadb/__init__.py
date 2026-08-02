import os

from ._monadb import BusyError, Error, TransactionError
from ._monadb import open as _open
from .collection import Collection
from .db import Database
from .txn import Transaction

__all__ = [
    "BusyError",
    "Collection",
    "Database",
    "Error",
    "Transaction",
    "TransactionError",
    "open",
]


def open(path=None, *, timeout=5.0, durable=True):
    """Open a database: in-memory when ``path`` is ``None``, else file-backed.

    ``timeout`` bounds the wait for the write gate, in seconds; exceeding it
    raises :class:`BusyError`. ``durable=False`` trades commit durability for
    speed, which suits bulk loads.
    """
    p = os.fspath(path) if path is not None else None
    return Database(_open(p, timeout=timeout, durable=durable))
