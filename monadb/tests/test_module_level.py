"""Module-level convenience functions operating on a shared default connection —
mirrors duckdb.execute()/duckdb.sql()/duckdb.fetchall()."""

import monadb


def test_module_level_execute_and_sql():
    monadb.execute("create table mt;")
    monadb.execute("insert into mt ({a: 7});")
    assert monadb.sql("select * from mt;").fetchall() == [{"a": 7}]


def test_module_level_fetchone():
    monadb.execute("create table mo;")
    monadb.execute("insert into mo ({a: 1});")
    monadb.execute("select * from mo;")
    assert monadb.fetchone() == {"a": 1}
    assert monadb.fetchone() is None
