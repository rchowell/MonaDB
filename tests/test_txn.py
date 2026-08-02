import threading
import time

import pytest

import monadb


def test_commit_visible(db):
    with db.transaction() as tx:
        tx["users"]["bob"] = {"age": 41}
        assert tx["users"]["bob"] == {"age": 41}     # read-your-writes
        assert "bob" not in db                       # db maps collections, not keys
    assert db["users"]["bob"] == {"age": 41}


def test_exception_rolls_back(db):
    db["users"]["alice"] = {"age": 30}
    with pytest.raises(RuntimeError):
        with db.transaction() as tx:
            tx["users"]["bob"] = {"age": 41}
            del tx["users"]["alice"]
            raise RuntimeError("boom")
    assert "bob" not in db["users"]
    assert db["users"]["alice"] == {"age": 30}       # no partial state


def test_txn_mapping_and_ddl(db):
    db["a"]["k"] = {}
    with db.transaction() as tx:
        tx["b"]["k"] = {}
        assert "b" in tx and list(tx) == ["a", "b"]
        del tx["a"]
        assert "a" not in tx
    assert list(db) == ["b"]


def test_nested_transaction_raises(db):
    with db.transaction():
        with pytest.raises(monadb.TransactionError):
            db.transaction()


def test_implicit_write_inside_txn_raises(db):
    """Same thread, same db handle, gate already held -> immediate error, no hang."""
    with db.transaction() as tx:
        tx["u"]["a"] = {}
        with pytest.raises(monadb.TransactionError):
            db["u"]["b"] = {}


def test_contention_busy_error(tmp_path):
    db = monadb.open(str(tmp_path / "c.db"), timeout=0.3)
    try:
        result = {}
        with db.transaction() as tx:
            tx["u"]["a"] = {}

            def attempt():
                t0 = time.monotonic()
                try:
                    with db.transaction():
                        pass
                    result["err"] = None
                except monadb.BusyError:
                    result["err"] = "busy"
                result["elapsed"] = time.monotonic() - t0

            th = threading.Thread(target=attempt)
            th.start()
            th.join(10)
            assert not th.is_alive(), "second writer hung"
        assert result["err"] == "busy"
        assert 0.25 <= result["elapsed"] < 5.0
    finally:
        db.close()


def test_reader_survives_concurrent_commit(db):
    c = db["r"]
    for i in range(4):
        c[i] = {"i": i}
    it = iter(c.items())
    assert next(it)[0] == 0
    with db.transaction() as tx:
        tx["r"][99] = {"i": 99}
        del tx["r"][3]
    assert [k for k, _ in it] == [1, 2, 3]           # snapshot intact


def test_close_aborts_open_txn(tmp_path):
    path = str(tmp_path / "x.db")
    db = monadb.open(path)
    tx = db.transaction()
    tx.__enter__()
    tx["u"]["a"] = {"n": 1}
    db.close()
    with pytest.raises(monadb.TransactionError):
        tx["u"]["b"] = {"n": 2}
    db2 = monadb.open(path)
    try:
        assert "u" not in db2                        # nothing committed
    finally:
        db2.close()


def test_database_context_manager(tmp_path):
    with monadb.open(str(tmp_path / "cm.db")) as db:
        db["u"]["a"] = {}
    with pytest.raises(monadb.TransactionError):
        db["u"]["b"] = {}                            # closed


def test_durable_false_smoke(tmp_path):
    db = monadb.open(str(tmp_path / "d.db"), durable=False)
    try:
        db["u"]["a"] = {"n": 1}
        assert db["u"]["a"] == {"n": 1}
    finally:
        db.close()


def test_exception_hierarchy():
    assert issubclass(monadb.BusyError, monadb.Error)
    assert issubclass(monadb.TransactionError, monadb.Error)
