"""execute()/fetchone()/fetchmany()/fetchall() — DuckDB cursor semantics,
returning monadb's JSON-native rows (dicts for objects, scalars unwrapped)."""

import monadb


def populated():
    con = monadb.connect()
    con.execute("create table t;")
    con.execute("insert into t ({x: 1});")
    con.execute("insert into t ({x: 2});")
    return con


def test_fetchall_returns_list_of_dicts():
    con = populated()
    assert con.execute("select * from t;").fetchall() == [{"x": 1}, {"x": 2}]


def test_fetchone_walks_rows_then_returns_none():
    con = populated()
    con.execute("select * from t;")
    assert con.fetchone() == {"x": 1}
    assert con.fetchone() == {"x": 2}
    assert con.fetchone() is None


def test_fetchmany_clamps_to_remaining():
    con = populated()
    con.execute("select * from t;")
    assert con.fetchmany(1) == [{"x": 1}]
    assert con.fetchmany(5) == [{"x": 2}]
    assert con.fetchmany(5) == []


def test_empty_result_is_empty_list():
    con = monadb.connect()
    con.execute("create table t;")
    assert con.execute("select * from t;").fetchall() == []


def test_scalar_row_is_unwrapped():
    con = monadb.connect()
    assert con.execute("select 1;").fetchall() == [1]


def test_execute_returns_connection_for_chaining():
    con = monadb.connect()
    con.execute("create table t;")
    assert con.execute("insert into t ({x: 1});") is con


def test_description_exposes_column_names():
    con = populated()
    con.execute("select * from t;")
    assert con.description is not None
    assert [col[0] for col in con.description] == ["x"]
