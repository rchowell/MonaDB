"""dict protocol conformance + differential replay against a real dict."""

import random
from datetime import datetime, timezone

import pytest

import monadb
from conftest import rank

KEY_POOL = [0, 1, -5, 2**40, "a", "b", "user:1", "", b"\x00", b"raw",
            (1, "a"), ("a", 1), ("x", b"y", 3)]
DOC_POOL = [{}, {"n": 1}, {"n": 2, "s": "x"}, {"nested": {"a": [1, 2.5, None, True]}},
            {"b": b"bin"}, {"big": 2**40}]


def test_smoke_roundtrip(db):
    db["users"]["alice"] = {"age": 30}
    assert db["users"]["alice"] == {"age": 30}


def test_differential_replay(db):
    rng = random.Random(1234)
    model = {}
    coll = db["replay"]
    for _ in range(500):
        op = rng.choice(["set", "set", "set", "del", "get", "contains", "len"])
        k = rng.choice(KEY_POOL)
        if op == "set":
            v = rng.choice(DOC_POOL)
            model[k] = v
            coll[k] = v
        elif op == "del":
            if k in model:
                del model[k]
                del coll[k]
            else:
                with pytest.raises(KeyError):
                    del coll[k]
        elif op == "get":
            if k in model:
                assert coll[k] == model[k]
            else:
                with pytest.raises(KeyError):
                    coll[k]
        elif op == "contains":
            assert (k in coll) == (k in model)
        else:
            assert len(coll) == len(model)
    # Divergence 1, asserted not excused: key order, not insertion order.
    assert list(coll) == sorted(model, key=rank)
    assert dict(coll.items()) == model


def test_mutablemapping_derived_methods(db):
    c = db["m"]
    assert c.get("missing") is None
    assert c.get("missing", {"d": 1}) == {"d": 1}
    assert c.setdefault("a", {"n": 1}) == {"n": 1}
    assert c.setdefault("a", {"n": 2}) == {"n": 1}
    c.update({"b": {"n": 2}, "c": {"n": 3}})
    assert c.pop("b") == {"n": 2}
    with pytest.raises(KeyError):
        c.pop("b")
    assert c.pop("b", None) is None
    k, v = c.popitem()
    assert k in ("a", "c")
    c.clear()
    assert len(c) == 0
    assert list(c.keys()) == list(c.values()) == list(c.items()) == []


def test_autovivify(db):
    users = db["users"]          # no table created yet
    assert "users" not in db
    assert list(db) == []
    assert len(users) == 0
    assert list(users) == []
    with pytest.raises(KeyError):
        users["nope"]
    users["a"] = {"n": 1}        # first write creates the table
    assert "users" in db
    assert list(db) == ["users"]
    with pytest.raises(KeyError):
        del db["posts"]          # dropping a missing collection
    del db["users"]
    assert "users" not in db


def test_database_mapping(db):
    db["b"]["k"] = {}
    db["a"]["k"] = {}
    assert list(db) == ["a", "b"]        # sorted
    assert len(db) == 2
    assert "a" in db and "zz" not in db


def test_ordered_ops(db):
    c = db["ord"]
    for k in ["a", "ab", "b", ("t", 1), ("t", 2), 5, -1]:
        c[k] = {"k": True}
    expect = sorted([-1, 5, "a", "ab", "b", ("t", 1), ("t", 2)], key=rank)
    assert list(c) == expect
    assert list(reversed(c)) == expect[::-1]
    assert c.first() == (expect[0], {"k": True})
    assert c.last() == (expect[-1], {"k": True})
    assert [k for k, _ in c.range("a", "b")] == ["a", "ab"]      # half-open
    assert [k for k, _ in c.range(None, "a")] == [-1, 5]
    assert [k for k, _ in c.range("b", None)] == ["b", ("t", 1), ("t", 2)]
    assert [k for k, _ in c.prefix("a")] == ["a", "ab"]
    assert [k for k, _ in c.prefix(("t",))] == [("t", 1), ("t", 2)]
    with pytest.raises(TypeError):
        c.range(1, "z")                    # bounds must be the same key type
    assert db["empty"].first() is None and db["empty"].last() is None


def test_value_fidelity(db):
    c = db["vals"]
    doc = {"none": None, "t": True, "f": False, "i32": 7, "i64": 2**40,
           "f64": 1.5, "s": "héllo", "b": b"\x00\xff",
           "list": [1, "two", None, [3]], "nested": {"deep": {"x": 1}}}
    c["k"] = doc
    assert c["k"] == doc


def test_datetime_ms_truncation(db):
    c = db["dt"]
    aware = datetime(2026, 8, 2, 12, 0, 0, 123456, tzinfo=timezone.utc)
    c["k"] = {"at": aware}
    out = c["k"]["at"]
    assert out.tzinfo is not None
    assert out == aware.replace(microsecond=123000)   # BSON is ms-precision


def test_naive_datetime_is_utc(db):
    c = db["dt2"]
    naive = datetime(2026, 8, 2, 12, 0, 0)
    c["k"] = {"at": naive}
    out = c["k"]["at"]
    assert out == naive.replace(tzinfo=timezone.utc)


def test_value_rejections(db):
    c = db["bad"]
    with pytest.raises(TypeError):
        c["k"] = [1, 2, 3]                 # divergence 3: values must be mappings
    with pytest.raises(TypeError):
        c["k"] = "not a mapping"
    with pytest.raises(TypeError):
        c["k"] = {"x": {1, 2}}             # unsupported type inside the doc
    with pytest.raises(ValueError):
        c["k"] = {"x": 2**63}              # beyond Int64


def test_update_not_atomic_outside_txn(db):
    """Divergence 4: update() outside a transaction is independent commits."""
    c = db["upd"]
    bad = {"a": {"n": 1}, "b": 42, "c": {"n": 3}}    # 42 fails mid-way
    with pytest.raises(TypeError):
        c.update(bad)
    assert "a" in c and "c" not in c                  # first write stuck
    del c["a"]
    with pytest.raises(TypeError):
        with db.transaction() as tx:
            tx["upd"].update(bad)
    assert "a" not in c                               # atomic inside a txn


def test_iterators_stream_snapshot(db):
    """Divergence 5: views are snapshot iterators, not live dict views."""
    c = db["snap"]
    for i in range(5):
        c[i] = {"i": i}
    it = iter(c.items())
    next(it)
    c[99] = {"i": 99}                     # write while iterator is open
    assert [k for k, _ in it] == [1, 2, 3, 4]   # snapshot: no 99
