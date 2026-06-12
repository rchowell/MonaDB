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


def test_parameters_not_supported():
    con = monadb.connect()
    with pytest.raises(NotImplementedError):
        con.execute("select 1;", [1])
