from collections.abc import Mapping

from .collection import Collection


class Transaction(Mapping):
    """An open write transaction and a mapping of collection names."""

    def __init__(self, connection):
        """Initialize with a Rust transaction connection."""
        self._conn = connection

    def __getitem__(self, name):
        """Return a Collection handle for the given collection name."""
        return Collection(self._conn.collection(name))

    def __iter__(self):
        """Iterate over collection names in the transaction."""
        return iter(self._conn.names())

    def __len__(self):
        """Return the number of collections in the transaction."""
        return len(self._conn.names())

    def __contains__(self, name):
        """Check if a collection with the given name exists."""
        return isinstance(name, str) and self._conn.has(name)

    def __delitem__(self, name):
        """Drop (delete) a collection by name."""
        self._conn.drop(name)

    def collection(self, name, model=None):
        """Return a Collection handle, optionally bound to a dataclass or pydantic model."""
        return Collection(self._conn.collection(name), model)

    def commit(self):
        """Commit the transaction."""
        self._conn.commit()

    def abort(self):
        """Abort (roll back) the transaction."""
        self._conn.abort()

    def __enter__(self):
        """Enter context manager, returning self."""
        return self

    def __exit__(self, exc_type, exc, tb):
        """Exit context manager. Commit if no exception, else abort."""
        if exc_type is None:
            self._conn.commit()
        else:
            self._conn.abort()
        return False
