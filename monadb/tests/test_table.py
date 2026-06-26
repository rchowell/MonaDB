"""Table handles — MutableMapping dict surface and SQL-like surface."""

import pytest

import monadb
from monadb import DuplicateKeyError


def test_table_schema_creates_on_open():
    db = monadb.connect()
    items = db.table("items", {"x": int})
    items.insert({"x": 1, "y": "a"})
    assert items[1] == {"x": 1, "y": "a"}


def test_insert_returns_key():
    t = monadb.connect().table("t", {"k": int})
    assert t.insert({"k": 7, "v": "a"}) == 7


def test_insert_duplicate_raises():
    t = monadb.connect().table("t", {"k": int})
    t.insert({"k": 1})
    with pytest.raises(DuplicateKeyError):
        t.insert({"k": 1, "v": "again"})


def test_setitem_upserts_distinct_from_insert():
    t = monadb.connect().table("t", {"k": int})
    t[1] = {"v": "first"}
    assert t[1]["v"] == "first"
    t[1] = {"v": "second"}
    assert t[1]["v"] == "second"


def test_getitem_missing_raises_key_error():
    t = monadb.connect().table("t", {"k": int})
    with pytest.raises(KeyError):
        _ = t[99]


def test_slice_range_scan():
    t = monadb.connect().table("t", {"k": int})
    for i in (1, 2, 3, 5):
        t.insert({"k": i})
    rows = t[1:4]
    assert sorted(r["k"] for r in rows) == [1, 2, 3]


def test_slice_reverse_scan():
    t = monadb.connect().table("t", {"k": int})
    for i in (1, 2, 3):
        t.insert({"k": i})
    keys = [r["k"] for r in t[3:0:-1]]
    assert keys == [3, 2, 1]


def test_slice_bad_step_raises():
    t = monadb.connect().table("t", {"k": int})
    t.insert({"k": 1})
    with pytest.raises(NotImplementedError):
        _ = t[0:10:2]


def test_iter_yields_keys():
    t = monadb.connect().table("t", {"k": int})
    t.insert({"k": 3})
    t.insert({"k": 1})
    t.insert({"k": 2})
    assert list(t) == [1, 2, 3]


def test_contains_point_lookup():
    t = monadb.connect().table("t", {"k": int})
    t.insert({"k": 1})
    assert 1 in t
    assert 99 not in t


def test_values_and_items_single_scan():
    t = monadb.connect().table("t", {"k": int})
    t.insert({"k": 1, "v": "a"})
    t.insert({"k": 2, "v": "b"})
    assert list(t.values()) == [{"k": 1, "v": "a"}, {"k": 2, "v": "b"}]
    assert list(t.items()) == [(1, {"k": 1, "v": "a"}), (2, {"k": 2, "v": "b"})]


def test_len_and_count():
    t = monadb.connect().table("t", {"k": int})
    t.insert({"k": 1})
    t.insert({"k": 2})
    assert len(t) == 2
    assert t.count("k > ?", [1]) == 1


def test_delete_requires_explicit_none_for_all():
    t = monadb.connect().table("t", {"k": int})
    t.insert({"k": 1})
    t.insert({"k": 2})
    assert t.delete(None) == 2
    assert len(t) == 0


def test_delitem():
    t = monadb.connect().table("t", {"k": int})
    t.insert({"k": 1})
    t.insert({"k": 2})
    del t[1]
    assert 1 not in t
    assert 2 in t


def test_existing_table_with_schema():
    db = monadb.connect()
    db.execute("create table pre (k int);")
    pre = db.table("pre", {"k": int})
    pre.insert({"k": 5})
    del pre[5]
    assert 5 not in pre


def test_composite_key():
    c = monadb.connect().table("c", {"a": str, "b": int})
    c.insert({"a": "x", "b": 7})
    assert c["x", 7] == {"a": "x", "b": 7}


def test_keyless_insert_returns_surrogate_id():
    db = monadb.connect()
    log = db.table("log", {})
    k1 = log.insert({"msg": "a"})
    k2 = log.insert({"msg": "b"})
    assert k1 == 0
    assert k2 == 1


def test_connection_not_subscriptable():
    db = monadb.connect()
    with pytest.raises(TypeError):
        _ = db["foo"]
