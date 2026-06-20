"""connect() and connection lifecycle — mirrors duckdb.connect()."""

import pytest

import monadb


def test_connect_returns_connection():
    db = monadb.connect()
    assert isinstance(db, monadb.Connection)
    db.close()


def test_connect_defaults_to_in_memory():
    db = monadb.connect()
    db.execute("create table t;")
    db.execute("insert into t ({\"x\": 1});")
    assert db.execute("select * from t;").fetchall() == [{"x": 1}]
    db.close()


def test_connect_file_backed_persists(tmp_path):
    path = str(tmp_path / "x.db")
    db = monadb.connect(path)
    db.execute("create table t;")
    db.execute("insert into t ({\"x\": 1});")
    db.close()

    reopened = monadb.connect(path)
    assert reopened.execute("select * from t;").fetchall() == [{"x": 1}]
    reopened.close()


def test_context_manager_closes_connection():
    with monadb.connect() as db:
        db.execute("create table t;")
    with pytest.raises(monadb.Error):
        db.execute("select * from t;")


def test_read_only_not_supported():
    with pytest.raises(NotImplementedError):
        monadb.connect(read_only=True)


def test_module_level_execute_and_sql():
    monadb.execute("create table mt;")
    monadb.execute("insert into mt ({\"a\": 7});")
    assert monadb.sql("select * from mt;").fetchall() == [{"a": 7}]


def test_module_level_fetchone():
    monadb.execute("create table mo;")
    monadb.execute("insert into mo ({\"a\": 1});")
    monadb.execute("select * from mo;")
    assert monadb.fetchone() == {"a": 1}
    assert monadb.fetchone() is None