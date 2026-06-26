from __future__ import annotations

from typing import TypedDict


class Config(TypedDict, total=False):
    """Open-time settings passed via :func:`monadb.connect`'s ``config`` argument."""

    nosync: bool
