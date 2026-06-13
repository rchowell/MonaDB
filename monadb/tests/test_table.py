"""Table handles and the Mapping protocol — exercised end-to-end against the engine."""

import pytest

import monadb


def test_create_insert_get_delete():
    con = monadb.connect()
    foo = con["foo"]
    foo.create(x=int, y=str)
    foo.insert([{"x": 1, "y": "a"}, {"x": 2, "y": "b"}, {"x": 3, "y": "c"}])

    assert foo.get(x=1) == [{"x": 1, "y": "a"}]

    foo.delete(x=1)
    assert con.execute("select * from foo;").fetchall() == [
        {"x": 2, "y": "b"},
        {"x": 3, "y": "c"},
    ]


def test_insert_single_dict():
    t = monadb.connect()["t"]
    t.create(k=int)
    t.insert({"k": 1, "v": "solo"})
    assert t.get(1) == {"k": 1, "v": "solo"}


def test_get_full_vs_partial_key():
    c = monadb.connect()["c"]
    c.create(a=str, b=int)
    c.insert([{"a": "x", "b": 7}, {"a": "x", "b": 8}, {"a": "z", "b": 1}])

    assert c.get("x", 7) == {"a": "x", "b": 7}
    assert c.get(a="x", b=7) == {"a": "x", "b": 7}
    assert c.get("x") == [{"a": "x", "b": 7}, {"a": "x", "b": 8}]
    assert c.get("x", 99) is None


def test_delete_requires_predicate():
    t = monadb.connect()["t"]
    t.create(k=int)
    with pytest.raises(TypeError):
        t.delete()


def test_keyless_table():
    con = monadb.connect()
    log = con["log"]
    log.create()
    log.insert([{"msg": "a"}, {"msg": "b"}])
    assert con.execute("select * from log;").fetchall() == [{"msg": "a"}, {"msg": "b"}]


def test_table_handle_verbs():
    con = monadb.connect()
    foo = con.table("foo")
    foo.create(k=int)
    foo.insert([{"k": 1, "v": "a"}])
    assert foo.get(k=1) == {"k": 1, "v": "a"}
    assert con["foo"].name == "foo"


def test_handle_getitem_point_and_prefix():
    c = monadb.connect()["c"]
    c.create(a=str, b=int)
    c.insert([{"a": "x", "b": 7}, {"a": "x", "b": 8}])
    assert c["x", 7] == {"a": "x", "b": 7}
    assert c["x"] == [{"a": "x", "b": 7}, {"a": "x", "b": 8}]
    with pytest.raises(KeyError):
        _ = c["x", 99]


def test_handle_contains_iter_len():
    t = monadb.connect()["t"]
    t.create(k=int)
    t.insert([{"k": 1}, {"k": 2}, {"k": 3}])
    assert len(t) == 3
    assert 1 in t
    assert 99 not in t
    assert sorted(row["k"] for row in t) == [1, 2, 3]


def test_handle_delitem_and_setitem():
    t = monadb.connect()["t"]
    t.create(k=int)
    t.insert([{"k": 1, "v": "a"}, {"k": 2, "v": "b"}])

    del t[1]
    assert t.get(2) == {"k": 2, "v": "b"}
    assert 1 not in t

    t[2] = {"v": "REPLACED"}
    assert t[2] == {"k": 2, "v": "REPLACED"}


def test_dunders_need_known_keys():
    con = monadb.connect()
    con.execute("create table pre (k int);")
    pre = con.table("pre")
    with pytest.raises(TypeError):
        del pre[1]
    pre2 = con.table("pre", keys="k")
    pre2.insert({"k": 5})
    del pre2[5]
    assert 5 not in pre2
