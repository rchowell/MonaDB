import dataclasses
from collections.abc import MutableMapping

_KEYS, _VALUES, _ITEMS = 0, 1, 2


class Collection(MutableMapping):
    """An ordered mapping of keys to documents."""

    def __init__(self, collection, model=None):
        """Initialize with a Rust collection connection and optional model."""
        self._collection = collection
        self._model = model

    def __getitem__(self, key):
        """Return the document for the given key, as dict or model."""
        return from_doc(self._model, self._collection.get(key))

    def __setitem__(self, key, value):
        """Set the value for the given key, storing dict or model as document."""
        self._collection.put(key, to_doc(value))

    def __delitem__(self, key):
        """Delete the document with the given key."""
        self._collection.delete(key)

    def update(self, other=(), /, **kwargs):
        """Store many documents in one commit; nothing is written if any item is bad."""
        items = other.items() if hasattr(other, "keys") else other
        pairs = [(k, to_doc(v)) for k, v in items]
        pairs += [(k, to_doc(v)) for k, v in kwargs.items()]
        self._collection.put_many(pairs)

    def __contains__(self, key):
        """Return True if the collection contains key."""
        return self._collection.contains(key)

    def __len__(self):
        """Return the number of documents in the collection."""
        return self._collection.len()

    def __iter__(self):
        """Iterate over the collection's keys in order."""
        return iter(self._collection.iter_(_KEYS, False))

    def __reversed__(self):
        """Iterate over the collection's keys in reverse order."""
        return iter(self._collection.iter_(_KEYS, True))

    def keys(self):
        """Iterate over the collection's keys in order."""
        return iter(self._collection.iter_(_KEYS, False))

    def values(self):
        """Iterate over the collection's documents (dict or model) in order."""
        return (from_doc(self._model, d) for d in self._collection.iter_(_VALUES, False))

    def items(self):
        """Iterate over (key, document) pairs in key order."""
        return ((k, from_doc(self._model, d)) for k, d in self._collection.iter_(_ITEMS, False))

    def range(self, start, stop):
        """Iterate (key, document) pairs with start <= key < stop."""
        if start is not None and stop is not None and type(start) is not type(stop):
            raise TypeError("range bounds must be the same key type")
        pairs = self._collection.range_(start, stop, _ITEMS)
        return ((k, from_doc(self._model, d)) for k, d in pairs)

    def prefix(self, p):
        """Iterate (key, document) pairs with key beginning with prefix p."""
        pairs = self._collection.prefix_(p, _ITEMS)
        return ((k, from_doc(self._model, d)) for k, d in pairs)

    def first(self):
        """Return the smallest (key, document) tuple, or None if empty."""
        pair = self._collection.first()
        return None if pair is None else (pair[0], from_doc(self._model, pair[1]))

    def last(self):
        """Return the largest (key, document) tuple, or None if empty."""
        pair = self._collection.last()
        return None if pair is None else (pair[0], from_doc(self._model, pair[1]))


def to_doc(value):
    """Converts a python value into a document for storage."""
    if isinstance(value, dict):
        return value
    if hasattr(type(value), "model_dump"):  # pydantic, by duck-typing
        return value.model_dump()
    if dataclasses.is_dataclass(value) and not isinstance(value, type):
        return dataclasses.asdict(value)
    return value


def from_doc(model, doc):
    """Converts a document from storage into a pyhton value."""
    if model is None:
        return doc
    if hasattr(model, "model_validate"):  # pydantic, by duck-typing
        return model.model_validate(doc)
    if dataclasses.is_dataclass(model):
        return model(**doc)
    raise TypeError(f"unsupported model type: {model!r}")