import pytest

import monadb
from conftest import rank

# Strictly increasing under the codec's order (int < str < bytes; tuples componentwise).
ORDERED = [
    -(2**63), -1, 0, 1, 2**63 - 1,
    "", "a", "a\x00b", "ab", "b", "é",
    b"", b"\x00", b"a", b"\xff",
]


def test_key_roundtrip_and_order(db):
    c = db["k"]
    for i, k in enumerate(ORDERED):
        c[k] = {"i": i}
    assert list(c) == ORDERED
    assert ORDERED == sorted(ORDERED, key=rank)   # the reference agrees
    for i, k in enumerate(ORDERED):
        assert c[k] == {"i": i}


def test_tuple_keys(db):
    c = db["t"]
    keys = [(1, "a"), (1, "a", 0), (1, "b"), (2,), ("a", b"b"), ("a", b"b", -1)]
    for k in keys:
        c[k] = {}
    # Ordering is componentwise; a 1-tuple reads back as its scalar, since a
    # scalar *is* a 1-component tuple (see test_scalar_is_one_tuple).
    expect = [k[0] if len(k) == 1 else k for k in sorted(keys, key=rank)]
    assert list(c) == expect
    assert () not in c
    c[()] = {"root": True}                 # empty tuple: empty (minimal) key
    assert list(c)[0] == ()


def test_scalar_is_one_tuple(db):
    """Divergence 6 — spec: scalar = a 1-component tuple. ("a",) aliases "a"."""
    c = db["alias"]
    c[("a",)] = {"n": 1}
    assert c["a"] == {"n": 1}
    assert list(c) == ["a"]                # decodes to the scalar
    c["a"] = {"n": 2}
    assert c[("a",)] == {"n": 2} and len(c) == 1


def test_key_rejections(db):
    c = db["rej"]
    for bad in [1.5, True, False, None, (1, (2, 3)), (None,), object(), [1]]:
        with pytest.raises(TypeError):
            c[bad] = {}
    for bad in [2**63, -(2**63) - 1, (0, 2**63)]:
        with pytest.raises(ValueError):
            c[bad] = {}
    with pytest.raises(ValueError):
        db[""]["k"] = {}                   # empty collection name
