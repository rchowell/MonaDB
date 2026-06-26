from __future__ import annotations

from typing import Any, List

from monadb._monadb import _Statement


class Statement:
    """A SQL statement prepared for repeated execution."""

    def __init__(self, statement: _Statement):
        self._statement = statement

    def execute(self, parameters: Any = None) -> List[Any]:
        """Run the prepared statement and return its rows as a list.

        Args:
            parameters: Positional (list/tuple) or named (dict) bindings.

        Returns:
            The result rows as a list of dicts (or unwrapped scalars).
        """
        if parameters is None:
            return self._statement.execute()
        return self._statement.execute(parameters)

    @property
    def sql(self) -> str:
        """Return the original SQL text passed to ``prepare``."""
        return self._statement.sql
