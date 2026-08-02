"""Database lifecycle, durability, and concurrency."""

import threading

import pytest

import monadb


def test_close(tmp_path):
    path = str(tmp_path / "x.db")
    db = monadb.open(path)
    db["u"]["a"] = {"n": 1}
    db.close()
    with pytest.raises(monadb.Error):
        db["u"]["b"] = {"n": 2}
    with pytest.raises(monadb.Error):
        list(db)
    db2 = monadb.open(path)
    try:
        assert db2["u"]["a"] == {"n": 1}          # the commit before close survived
    finally:
        db2.close()


def test_database_context_manager(tmp_path):
    with monadb.open(str(tmp_path / "cm.db")) as db:
        db["u"]["a"] = {}
    with pytest.raises(monadb.Error):
        db["u"]["b"] = {}                        # closed on exit


def test_durable_false_smoke(tmp_path):
    db = monadb.open(str(tmp_path / "d.db"), durable=False)
    try:
        db["u"]["a"] = {"n": 1}
        assert db["u"]["a"] == {"n": 1}
    finally:
        db.close()


def test_reader_survives_concurrent_commit(db):
    """A reader holds its own snapshot: redb is MVCC, so readers never block."""
    c = db["r"]
    for i in range(4):
        c[i] = {"i": i}
    it = iter(c.items())
    assert next(it)[0] == 0
    c[99] = {"i": 99}
    del c[3]
    assert [k for k, _ in it] == [1, 2, 3]        # snapshot intact


def test_concurrent_writers_serialize(tmp_path):
    """Two threads writing at once both complete; redb serializes them."""
    db = monadb.open(str(tmp_path / "w.db"), durable=False)
    errors = []

    def writer(tag):
        try:
            for i in range(50):
                db["w"][(tag, i)] = {"i": i}
        except Exception as exc:                 # noqa: BLE001 - reported below
            errors.append(exc)

    threads = [threading.Thread(target=writer, args=(t,)) for t in ("a", "b")]
    try:
        for t in threads:
            t.start()
        for t in threads:
            t.join(60)
        assert not any(t.is_alive() for t in threads), "a writer hung"
        assert errors == []
        assert len(db["w"]) == 100
    finally:
        db.close()


def test_exception_hierarchy():
    assert issubclass(monadb.Error, Exception)


def test_no_transaction_api():
    """There is no transaction surface left to hold open."""
    assert not hasattr(monadb.Database, "transaction")
    assert not hasattr(monadb, "Transaction")
    assert not hasattr(monadb, "BusyError")
    assert not hasattr(monadb, "TransactionError")
