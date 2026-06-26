"""prepare() / Statement.execute() — cached parse and compile."""

import pytest

import monadb


def test_prepare_select_executes_twice():
    db = monadb.connect()
    db.execute("create table t;")
    db.execute("insert into t ({\"x\": 1});")

    stmt = db.prepare("select * from t;")
    assert stmt.execute() == [{"x": 1}]
    assert stmt.execute() == [{"x": 1}]


def test_prepare_parameterized_binds_each_execute():
    stmt = monadb.connect().prepare("select ?;")
    assert stmt.execute([1]) == [1]
    assert stmt.execute([2]) == [2]


def test_prepare_matches_execute():
    db = monadb.connect()
    db.execute("create table t;")
    db.execute("insert into t ({\"x\": 1});")

    direct = db.execute("select * from t;")
    prepared = db.prepare("select * from t;").execute()
    assert prepared == direct


def test_prepare_stale_after_drop():
    db = monadb.connect()
    db.execute("create table t;")
    stmt = db.prepare("select * from t;")
    db.execute("drop table t;")
    with pytest.raises(monadb.Error):
        stmt.execute()


def test_prepare_named_params():
    stmt = monadb.connect().prepare("select $greeting;")
    assert stmt.execute({"greeting": "hi"}) == ["hi"]


def test_prepare_missing_param():
    stmt = monadb.connect().prepare("select ?;")
    with pytest.raises(monadb.Error):
        stmt.execute()


def test_prepare_insert_reuse():
    db = monadb.connect()
    db.execute("create table t (id int);")
    stmt = db.prepare("insert into t ($1);")
    for i in range(1, 4):
        stmt.execute([{"id": i}])
    assert len(db.execute("select * from t;")) == 3


def test_prepare_keyed_lookup():
    db = monadb.connect()
    db.execute("create table t (id int);")
    db.execute('insert into t ({"id": 1, "v": "a"});')
    stmt = db.prepare("select t[?];")
    assert stmt.execute([1]) == [{"id": 1, "v": "a"}]


def test_prepare_sql_property():
    sql = "select 1;"
    stmt = monadb.connect().prepare(sql)
    assert stmt.sql == sql
