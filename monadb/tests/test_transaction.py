"""Explicit transactions over the Python API — begin/commit/rollback (#5).

Regression coverage for docs/plans/10-transactions.md: transaction-control
statements must be intercepted on the `query_with` path that `Connection.run`
uses, and a session must read its own uncommitted writes.
"""

import monadb


def test_session_reads_own_writes_then_commits():
    db = monadb.connect()
    db.execute("create table t;")
    db.execute("begin;")
    db.execute('insert into t ({"x": 1});')
    # The select runs on the session's write txn and sees the uncommitted row.
    assert db.execute("select * from t;").fetchall() == [{"x": 1}]
    db.execute("commit;")
    assert db.execute("select * from t;").fetchall() == [{"x": 1}]
    db.close()


def test_rollback_discards_session_writes():
    db = monadb.connect()
    db.execute("create table t;")
    db.execute("begin;")
    db.execute('insert into t ({"x": 1});')
    db.execute("rollback;")
    assert db.execute("select * from t;").fetchall() == []
    db.close()


def test_commit_with_no_transaction_errors():
    db = monadb.connect()
    try:
        with_error = False
        try:
            db.execute("commit;")
        except monadb.Error:
            with_error = True
        assert with_error, "commit with no active transaction must error"
    finally:
        db.close()
