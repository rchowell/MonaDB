from __future__ import annotations

from collections.abc import MutableMapping
import re
from typing import (
    TYPE_CHECKING,
    Any,
    Callable,
    Dict,
    Iterator,
    List,
    Mapping,
    Optional,
    Tuple,
    Union,
)

from monadb._monadb import DuplicateKeyError
from monadb.encode import encode, encode_ident

if TYPE_CHECKING:
    from monadb.connection import Connection

Patch = Union[Mapping[str, Any], Callable[[dict], dict]]


class Table(MutableMapping[Any, dict]):
    def __init__(
        self,
        conn: Connection,
        name: str,
        schema: Optional[Dict[str, Any]] = None,
    ):
        self._conn = conn
        self._name = name
        self._schema = dict(schema) if schema is not None else None
        self._statements: dict[int, Any] = {}

    @property
    def name(self) -> str:
        return self._name

    def __repr__(self) -> str:
        return f"Table({self._name!r})"

    def select(
        self,
        predicate: Optional[str] = None,
        params: Optional[List[Any]] = None,
    ) -> List[dict]:
        """Return rows matching an optional predicate as a list.

        Args:
            predicate: SQL ``WHERE`` fragment without the ``where`` keyword.
                ``None`` selects all rows.
            params: Positional ``?`` bindings for ``predicate``.

        Returns:
            The matching rows as a list of dicts.
        """
        table = self._name
        if predicate is None:
            sql = f"select * from {table};"
        else:
            alias = "r"
            where = self._qualify_predicate(predicate, alias)
            sql = f"select * from {table} as {alias} where {where};"
        return self._conn.execute(sql, params)

    def insert(self, doc: dict) -> Any:
        """Insert a row with a generated key and return that key.

        Args:
            doc: Row payload as a dict.

        Returns:
            The generated key — a scalar for a single-column key, otherwise a
            tuple of key-column values. For keyless tables, the surrogate row
            id.

        Raises:
            TypeError: When ``doc`` is not a dict.
            DuplicateKeyError: When a keyed row with the same key already exists.
        """
        if not isinstance(doc, dict):
            raise TypeError("doc must be a dict")
        cols = self._key_columns()
        table = encode_ident(self._name)
        if cols:
            key = self._key_from_doc(doc)
            if self._lookup_point(_as_tuple(key)) is not None:
                raise DuplicateKeyError(f"duplicate key {key!r}")
            self._conn.execute(f"insert into {table} ({encode(doc)});")
            return key
        row_id = self._conn.peek_keyless_row_id(self._name)
        self._conn.execute(f"insert into {table} ({encode(doc)});")
        return row_id

    def update(
        self,
        patch: Patch,
        predicate: Optional[str] = None,
        params: Optional[List[Any]] = None,
    ) -> int:
        """Shallow-merge or transform rows matching an optional predicate.

        Args:
            patch: Either a dict shallow-merged into each matching document, or
                a callable ``doc -> doc`` applied to each match.
            predicate: SQL ``WHERE`` fragment. Pass ``None`` explicitly to
                update every row.
            params: Positional ``?`` bindings for ``predicate``.

        Returns:
            The number of rows updated.
        """
        if predicate is not None and params is None:
            params = []
        rows = self.select(predicate, params)
        cols = self._key_columns()
        updated = 0
        for row in rows:
            if callable(patch):
                new_row = patch(row)
            else:
                new_row = {**row, **patch}
            if not isinstance(new_row, dict):
                raise TypeError("patch callable must return a dict")
            key = self._key_from_row(row, cols)
            self._upsert(key, new_row)
            updated += 1
        return updated

    def delete(
        self,
        predicate: Optional[str] = None,
        params: Optional[List[Any]] = None,
    ) -> int:
        """Delete rows matching an optional predicate.

        Args:
            predicate: SQL ``WHERE`` fragment. Pass ``None`` explicitly to delete
                every row.
            params: Positional ``?`` bindings for ``predicate``.

        Returns:
            The number of rows deleted.
        """
        table = encode_ident(self._name)
        if predicate is None:
            return self._conn._mutations(f"delete from {table};")
        where = self._qualify_predicate(predicate, "r")
        return self._conn._mutations(f"delete from {table} as r where {where};", params)

    def count(
        self,
        predicate: Optional[str] = None,
        params: Optional[List[Any]] = None,
    ) -> int:
        """Return the number of rows matching an optional predicate.

        Args:
            predicate: SQL ``WHERE`` fragment. ``None`` counts all rows.
            params: Positional ``?`` bindings for ``predicate``.

        Returns:
            The matching row count.
        """
        table = encode_ident(self._name)
        if predicate is None:
            sql = f"select count(*) from {table};"
        else:
            where = self._qualify_predicate(predicate, "r")
            sql = f"select count(*) from {table} as r where {where};"
        rows = self._conn.execute(sql, params)
        return int(rows[0]) if rows else 0

    # ----- MutableMapping core -----

    def __getitem__(self, key: Any) -> Any:
        """Point-lookup by key, or return a range scan as a list of rows."""
        if isinstance(key, slice):
            return self._rows_for_slice(key)
        row = self._lookup_point(_as_tuple(key))
        if row is None:
            raise KeyError(key)
        return row

    def __setitem__(self, key: Any, value: dict) -> None:
        """Upsert the row at a full caller-supplied key."""
        cols = self._key_columns()
        vals = _as_tuple(key)
        if len(vals) != len(cols):
            raise KeyError(f"setting a row requires the full key {cols}, got {key!r}")
        if not isinstance(value, dict):
            raise TypeError("row value must be a dict")
        row = {**value, **dict(zip(cols, vals))}
        self._upsert(key, row)

    def __delitem__(self, key: Any) -> None:
        """Delete the row at a full key."""
        cols = self._key_columns()
        vals = _as_tuple(key)
        if len(vals) != len(cols):
            raise KeyError(f"deleting a row requires the full key {cols}, got {key!r}")
        if self._lookup_point(vals) is None:
            raise KeyError(key)
        preds = " and ".join(
            f"{encode_ident(self._name)}.{encode_ident(col)} = {encode(v)}"
            for col, v in zip(cols, vals)
        )
        self._conn._mutations(f"delete from {encode_ident(self._name)} where {preds};")

    def __iter__(self) -> Iterator[Any]:
        """Yield primary-key values in storage order."""
        cols = self._key_columns()
        for row in self.select():
            yield self._key_from_row(row, cols)

    def __len__(self) -> int:
        """Return the total row count."""
        return self.count()

    def __contains__(self, key: object) -> bool:
        """Return whether a full key is present."""
        cols = self._key_columns()
        vals = _as_tuple(key)
        if len(vals) != len(cols):
            return False
        return self._lookup_point(vals) is not None

    def values(self) -> Iterator[dict]:
        """Yield row dicts in a single cursor scan."""
        yield from self.select()

    def items(self) -> Iterator[Tuple[Any, dict]]:
        """Yield ``(key, row)`` pairs in a single cursor scan."""
        cols = self._key_columns()
        for row in self.select():
            yield self._key_from_row(row, cols), row

    # ----- internals -----

    def _key_columns(self) -> Tuple[str, ...]:
        cols = self._conn.schema_columns(self._name)
        if cols is None:
            raise TypeError(
                f"key columns for {self._name!r} are unknown; pass schema= to "
                "connection.table()"
            )
        return cols

    def _key_from_doc(self, doc: dict) -> Any:
        cols = self._key_columns()
        missing = [col for col in cols if col not in doc]
        if missing:
            raise KeyError(f"document missing key field(s): {missing}")
        values = tuple(doc[col] for col in cols)
        return values[0] if len(values) == 1 else values

    def _key_from_row(self, row: dict, cols: Tuple[str, ...]) -> Any:
        values = tuple(row[col] for col in cols)
        return values[0] if len(values) == 1 else values

    def _lookup_point(self, key_vals: Tuple[Any, ...]) -> Optional[dict]:
        arity = len(key_vals)
        try:
            rows = self._get_stmt(arity).execute(list(key_vals))
        except Exception:
            self._statements.pop(arity, None)
            rows = self._get_stmt(arity).execute(list(key_vals))
        if not rows or rows[0] is None:
            return None
        return rows[0]

    def _get_stmt(self, arity: int) -> Any:
        stmt = self._statements.get(arity)
        if stmt is None:
            placeholders = ", ".join(["?"] * arity)
            stmt = self._conn.prepare(
                f"select {encode_ident(self._name)}[{placeholders}];"
            )
            self._statements[arity] = stmt
        return stmt

    def _upsert(self, key: Any, row: dict) -> None:
        table = encode_ident(self._name)
        self._conn.execute(f"insert into {table} ({encode(row)});")

    def _rows_for_slice(self, s: slice) -> List[dict]:
        cols = self._key_columns()
        if len(cols) != 1:
            raise TypeError("slice ranges require a single-column primary key")
        col = cols[0]
        if self._schema is not None and self._schema.get(col) not in (int, "int"):
            raise TypeError("slice ranges require an int primary-key column")
        if s.step not in (None, 1, -1):
            raise NotImplementedError("slice step values other than 1 or -1")
        if s.start is None or s.stop is None:
            raise TypeError("slice bounds must be explicit for range scans")
        table = encode_ident(self._name)
        alias = "r"
        col_ref = f"{alias}.{encode_ident(col)}"
        if s.step == -1:
            lo = s.stop + 1
            hi = s.start
            predicate = f"{col_ref} >= ? and {col_ref} <= ?"
            params: List[Any] = [lo, hi]
            order = f" order by {col_ref} desc"
        else:
            predicate = f"{col_ref} >= ? and {col_ref} < ?"
            params = [s.start, s.stop]
            order = ""
        sql = f"select * from {table} as {alias} where {predicate}{order};"
        return self._conn.execute(sql, params)

    def _qualify_predicate(self, predicate: str, alias: str) -> str:
        """Prefix bare schema column names with ``alias`` for the binder."""
        if not self._schema:
            return predicate
        qualified = predicate
        for col in sorted(self._schema.keys(), key=len, reverse=True):
            ident = encode_ident(col)
            qualified = re.sub(
                rf"(?<!\.)\b{re.escape(ident)}\b",
                f"{alias}.{ident}",
                qualified,
            )
        return qualified


def _as_tuple(key: Any) -> Tuple[Any, ...]:
    """Normalize a subscript key to a tuple of key-column values."""
    return key if isinstance(key, tuple) else (key,)
