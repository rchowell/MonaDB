"""Error surfacing — monadb.Error for SQL failures, NotImplementedError for
deferred features."""

import pytest

import monadb


def test_syntax_error_raises_monadb_error():
    con = monadb.connect()
    with pytest.raises(monadb.Error):
        con.execute("create table")  # missing table name


def test_unknown_table_raises_monadb_error():
    con = monadb.connect()
    with pytest.raises(monadb.Error):
        con.execute("select * from nope;")


def test_missing_parameter_raises_monadb_error():
    con = monadb.connect()
    with pytest.raises(monadb.Error, match="missing parameter"):
        con.execute("select ?;")


def test_bad_parameters_container_raises_monadb_error():
    con = monadb.connect()
    with pytest.raises(monadb.Error, match="list, tuple, or dict"):
        con.execute("select 1;", "not a list")


def test_bad_parameter_value_raises_monadb_error():
    con = monadb.connect()
    with pytest.raises(monadb.Error, match="unsupported parameter value type"):
        con.execute("select ?;", [object()])
