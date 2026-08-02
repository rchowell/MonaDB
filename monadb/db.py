from collections.abc import Mapping

from .collection import Collection


class Database(Mapping):
    """An open database mapping collection names to collections."""

    def __init__(self, db):
        """Initialize with a Rust database connection."""
        self._db = db

    def __getitem__(self, name):
        """Return a Collection handle for the given collection name."""
        return Collection(self._db.collection(name))

    def __iter__(self):
        """Iterate over collection names in the database."""
        return iter(self._db.names())

    def __len__(self):
        """Return the number of collections in the database."""
        return len(self._db.names())

    def __contains__(self, name):
        """Check if a collection with the given name exists."""
        return isinstance(name, str) and self._db.has(name)

    def __delitem__(self, name):
        """Drop (delete) a collection by name."""
        self._db.drop(name)

    def collection(self, name, model=None):
        """Return a Collection handle, optionally bound to a dataclass or pydantic model."""
        return Collection(self._db.collection(name), model)

    def close(self):
        """Close the database connection."""
        self._db.close()

    def __enter__(self):
        """Enter context manager, returning self."""
        return self

    def __exit__(self, exc_type, exc, tb):
        """Exit context manager, closing the database connection."""
        self.close()
        return False
