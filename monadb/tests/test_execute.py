"""execute() — eager; returns a list of monadb's JSON-native rows (dicts for
objects, scalars unwrapped). Writes and DDL return an empty list."""

import monadb


def populated():
    db = monadb.connect()
    db.execute("create table t;")
    db.execute("insert into t ({\"x\": 1});")
    db.execute("insert into t ({\"x\": 2});")
    return db


def test_select_returns_list_of_dicts():
    db = populated()
    assert db.execute("select * from t;") == [{"x": 1}, {"x": 2}]


def test_empty_result_is_empty_list():
    db = monadb.connect()
    db.execute("create table t;")
    assert db.execute("select * from t;") == []


def test_write_returns_empty_list():
    db = monadb.connect()
    db.execute("create table t;")
    assert db.execute('insert into t ({"x": 1});') == []


def test_scalar_row_is_unwrapped():
    db = monadb.connect()
    assert db.execute("select 1;") == [1]
