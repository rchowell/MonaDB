"""prepare() / PreparedStatement.execute() — cached parse and compile."""

import monadb


def test_prepare_select_executes_twice():
    db = monadb.connect()
    db.execute("create table t;")
    db.execute("insert into t ({\"x\": 1});")

    stmt = db.prepare("select * from t;")
    assert stmt.execute().fetchall() == [{"x": 1}]
    assert stmt.execute().fetchall() == [{"x": 1}]


def test_prepare_parameterized_binds_each_execute():
    stmt = monadb.connect().prepare("select ?;")
    assert stmt.execute([1]).fetchall() == [1]
    assert stmt.execute([2]).fetchall() == [2]


def test_prepare_matches_execute():
    db = monadb.connect()
    db.execute("create table t;")
    db.execute("insert into t ({\"x\": 1});")

    direct = db.execute("select * from t;").fetchall()
    prepared = db.prepare("select * from t;").execute().fetchall()
    assert prepared == direct
