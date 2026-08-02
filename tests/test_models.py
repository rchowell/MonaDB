from dataclasses import dataclass

import pydantic          # test-only dev dependency; the package never imports it
import pytest

import monadb


@dataclass
class DUser:
    name: str
    age: int


class PUser(pydantic.BaseModel):
    name: str
    age: int


def test_dataclass_roundtrip(db):
    users = db.collection("users", DUser)
    users["a"] = DUser(name="alice", age=30)
    out = users["a"]
    assert isinstance(out, DUser) and out == DUser(name="alice", age=30)


def test_pydantic_roundtrip(db):
    users = db.collection("pusers", PUser)
    users["a"] = PUser(name="alice", age=30)
    out = users["a"]
    assert isinstance(out, PUser) and out.age == 30


def test_model_binding_is_handle_property(db):
    users = db.collection("users", DUser)
    users["a"] = DUser(name="alice", age=30)
    assert db["users"]["a"] == {"name": "alice", "age": 30}   # plain-dict view


def test_model_write_accepted_anywhere(db):
    db["users"]["b"] = DUser(name="bob", age=41)              # values may be dataclasses
    assert db["users"]["b"] == {"name": "bob", "age": 41}


def test_model_iteration(db):
    users = db.collection("users", DUser)
    users["a"] = DUser(name="alice", age=30)
    users["b"] = DUser(name="bob", age=41)
    assert [u.name for _, u in users.items()] == ["alice", "bob"]
    assert users.first() == ("a", DUser(name="alice", age=30))
