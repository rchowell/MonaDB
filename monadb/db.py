"""Database and Transaction: Mapping[str, Collection] views over Rust handles."""

from collections.abc import Mapping

from .collection import Collection


class _Namespace(Mapping):
    """Collection-namespace behavior shared by databases and transactions.

    ``self._h`` is a Rust ``Db`` or ``Txn``; both expose the same
    ``names``/``has``/``drop``/``collection`` surface.
    """

    def __getitem__(self, name):
        # Auto-vivifying: this returns a handle without creating anything. The
        # underlying table appears on the first write.
        return Collection(self._h.collection(name))

    def __iter__(self):
        return iter(self._h.names())

    def __len__(self):
        return len(self._h.names())

    def __contains__(self, name):
        return isinstance(name, str) and self._h.has(name)

    def __delitem__(self, name):
        self._h.drop(name)

    def collection(self, name, model=None):
        """A handle, optionally bound to a dataclass or pydantic model."""
        return Collection(self._h.collection(name), model)


class Transaction(_Namespace):
    """An open write transaction, and a Mapping of collection names."""

    def __init__(self, handle):
        self._h = handle

    def commit(self):
        self._h.commit()

    def abort(self):
        self._h.abort()

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc, tb):
        if exc_type is None:
            self._h.commit()
        else:
            self._h.abort()
        return False


class Database(_Namespace):
    """An open database: a Mapping of collection names to collections."""

    def __init__(self, handle):
        self._h = handle

    def transaction(self):
        """Begin a write transaction, acquiring the write gate now."""
        return Transaction(self._h.begin())

    def close(self):
        """Close the database, aborting any open transaction."""
        self._h.close()

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc, tb):
        self.close()
        return False
