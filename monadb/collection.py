"""Collection: a MutableMapping over a Rust collection handle."""

from collections.abc import MutableMapping

from .models import from_doc, to_doc

_KEYS, _VALUES, _ITEMS = 0, 1, 2


class Collection(MutableMapping):
    """An ordered mapping of keys to documents.

    Differs from ``dict`` in six documented ways: iteration is key order rather
    than insertion order; keys are ``str | int | bytes | tuple`` of those;
    values must be mappings; ``update()`` is atomic only inside a transaction;
    ``keys()``/``values()``/``items()`` are snapshot iterators rather than live
    views; and ``"a"`` and ``("a",)`` are the same key.
    """

    def __init__(self, handle, model=None):
        self._h = handle
        self._model = model

    def __getitem__(self, key):
        return from_doc(self._model, self._h.get(key))

    def __setitem__(self, key, value):
        self._h.put(key, to_doc(value))

    def __delitem__(self, key):
        self._h.delete(key)

    def __contains__(self, key):
        return self._h.contains(key)

    def __len__(self):
        return self._h.len()

    def __iter__(self):
        return iter(self._h.iter_(_KEYS, False))

    def __reversed__(self):
        return iter(self._h.iter_(_KEYS, True))

    # Each of the following opens one read snapshot, owned by the iterator it
    # returns. They are iterators, not dict-style live views.

    def keys(self):
        return iter(self._h.iter_(_KEYS, False))

    def values(self):
        return (from_doc(self._model, d) for d in self._h.iter_(_VALUES, False))

    def items(self):
        return ((k, from_doc(self._model, d)) for k, d in self._h.iter_(_ITEMS, False))

    def range(self, start, stop):
        """Entries with ``start <= key < stop``; ``None`` means unbounded."""
        if start is not None and stop is not None and type(start) is not type(stop):
            raise TypeError("range bounds must be the same key type")
        pairs = self._h.range_(start, stop, _ITEMS)
        return ((k, from_doc(self._model, d)) for k, d in pairs)

    def prefix(self, p):
        """Entries whose key begins with ``p``.

        A ``str`` or ``bytes`` prefix matches keys of that type extending it; a
        tuple matches keys whose leading components equal it.
        """
        pairs = self._h.prefix_(p, _ITEMS)
        return ((k, from_doc(self._model, d)) for k, d in pairs)

    def first(self):
        """The smallest ``(key, document)``, or ``None`` if empty."""
        pair = self._h.first()
        return None if pair is None else (pair[0], from_doc(self._model, pair[1]))

    def last(self):
        """The largest ``(key, document)``, or ``None`` if empty."""
        pair = self._h.last()
        return None if pair is None else (pair[0], from_doc(self._model, pair[1]))
