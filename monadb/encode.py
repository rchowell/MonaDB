"""Encode Python values into MonaDB literal SQL text.

The high-level façade (``Connection``/``Table``) builds SQL by hand because the
engine has no parameter binding yet, so every Python value must be rendered as a
MonaDB literal. The rules below are pinned to the actual lexer/parser
(``src/lexer.rs``, ``src/parser.lalrpop``) and to the string decoder
(``parse_string_literal`` in ``src/value.rs``), verified by round-trip tests.

Two engine limitations are surfaced honestly as errors rather than papered over
with silent corruption:

* The string decoder un-doubles quotes and passes backslashes through literally,
  so a literal cannot faithfully carry a ``"`` or ``\\``.

Object keys are quoted string literals (``{"a": 1}``), matching the engine's
string-only key grammar.
"""

from __future__ import annotations

import math
import re
from typing import Any

# Mirrors the `ident` token in src/lexer.rs: [a-zA-Z_][a-zA-Z0-9_]*
_IDENT = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")

# Python key-column type → MonaDB type keyword. Declared columns are the
# composite key, and MonaDB keys are int/string only.
_TYPE_NAMES = {int: "int", str: "string"}


def encode(value: Any) -> str:
    """Render a Python value as a MonaDB literal.

    Supports ``None``, ``bool``, ``int``, finite ``float``, ``str`` (without
    ``"`` or ``\\``), ``list``/``tuple``, and ``dict`` (string keys). Raises
    ``TypeError`` for unsupported types and ``ValueError`` for values the engine
    cannot represent faithfully.
    """
    # bool must precede int: bool is a subclass of int in Python.
    if value is None:
        return "null"
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, int):
        return str(value)
    if isinstance(value, float):
        if not math.isfinite(value):
            raise ValueError(f"cannot encode non-finite float: {value!r}")
        return repr(value)
    if isinstance(value, str):
        return encode_str(value)
    if isinstance(value, (list, tuple)):
        return "[" + ", ".join(encode(v) for v in value) + "]"
    if isinstance(value, dict):
        # Object keys are quoted string literals (the engine's key grammar).
        members = ", ".join(f"{encode_str(str(k))}: {encode(v)}" for k, v in value.items())
        return "{" + members + "}"
    if isinstance(value, (bytes, bytearray)):
        raise TypeError("MonaDB has no byte-string literal; cannot encode bytes")
    raise TypeError(f"cannot encode value of type {type(value).__name__}: {value!r}")


def encode_str(s: str) -> str:
    """Render a Python string as a double-quoted MonaDB string literal.

    The engine's string decoder passes backslashes through literally and has no
    working escape for an embedded double-quote, so strings containing ``"`` or
    ``\\`` are rejected rather than silently corrupted. Newlines, tabs, single
    quotes, and non-ASCII characters round-trip fine and are emitted verbatim.
    """
    if '"' in s or "\\" in s:
        raise ValueError(
            'MonaDB string literals cannot contain a double-quote (") or '
            f"backslash (\\); got {s!r}"
        )
    return '"' + s + '"'


def encode_ident(name: Any) -> str:
    """Render a bare MonaDB identifier — an object key, table, or column name.

    The single choke point for identifier rendering, mirroring :func:`encode`
    for values. Quoted-string object keys are stored with their quotes embedded
    by the engine, so only identifier-shaped names are supported.
    """
    if not isinstance(name, str):
        raise TypeError(f"identifiers must be strings, got {type(name).__name__}: {name!r}")
    if not _IDENT.fullmatch(name):
        raise ValueError(
            f"{name!r} is not a valid MonaDB identifier ([A-Za-z_][A-Za-z0-9_]*)"
        )
    return name


def type_name(typ: Any) -> str:
    """Map a key-column type spec to its MonaDB keyword (``int`` or ``string``)."""
    if isinstance(typ, str):
        return typ
    if typ in _TYPE_NAMES:
        return _TYPE_NAMES[typ]
    raise TypeError(f"unsupported key column type {typ!r}; use int or str")
