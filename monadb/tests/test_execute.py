"""execute()/fetchone()/fetchmany()/fetchall() — DuckDB cursor semantics,
returning monadb's JSON-native rows (dicts for objects, scalars unwrapped)."""

import monadb


def populated():
    db = monadb.connect()
    db.execute("create table t;")
    db.execute("insert into t ({\"x\": 1});")
    db.execute("insert into t ({\"x\": 2});")
    return db


def test_fetchall_returns_list_of_dicts():
    db = populated()
    assert db.execute("select * from t;").fetchall() == [{"x": 1}, {"x": 2}]


def test_fetchone_walks_rows_then_returns_none():
    db = populated()
    db.execute("select * from t;")
    assert db.fetchone() == {"x": 1}
    assert db.fetchone() == {"x": 2}
    assert db.fetchone() is None


def test_fetchmany_clamps_to_remaining():
    db = populated()
    db.execute("select * from t;")
    assert db.fetchmany(1) == [{"x": 1}]
    assert db.fetchmany(5) == [{"x": 2}]
    assert db.fetchmany(5) == []


def test_empty_result_is_empty_list():
    db = monadb.connect()
    db.execute("create table t;")
    assert db.execute("select * from t;").fetchall() == []


def test_scalar_row_is_unwrapped():
    db = monadb.connect()
    assert db.execute("select 1;").fetchall() == [1]


def test_execute_returns_connection_for_chaining():
    db = monadb.connect()
    db.execute("create table t;")
    assert db.execute('insert into t ({"x": 1});') is db


def test_description_exposes_column_names():
    db = populated()
    db.execute("select * from t;")
    assert db.description is not None
    assert [col[0] for col in db.description] == ["x"]
