"""Table SQL-like surface — select, update, delete, count."""

import monadb


def test_select_with_predicate():
    items = monadb.connect().table("items", {"x": int})
    items.insert({"x": 1, "y": 0})
    items.insert({"x": 9, "y": 0})
    rows = items.select("x > ?", [3])
    assert rows == [{"x": 9, "y": 0}]


def test_update_dict_patch():
    items = monadb.connect().table("items", {"x": int})
    items.insert({"x": 1, "y": 0})
    items.insert({"x": 9, "y": 0})
    n = items.update({"y": 99}, "x > ?", [3])
    assert n == 1
    assert items[9]["y"] == 99
    assert items[1]["y"] == 0


def test_update_callable_patch():
    items = monadb.connect().table("items", {"x": int})
    items.insert({"x": 1, "y": 1})
    n = items.update(lambda doc: {**doc, "y": doc["y"] + 1}, "x = ?", [1])
    assert n == 1
    assert items[1]["y"] == 2


def test_update_whole_table_explicit_none():
    items = monadb.connect().table("items", {"x": int})
    items.insert({"x": 1, "y": 0})
    items.insert({"x": 2, "y": 0})
    n = items.update({"y": 1}, None)
    assert n == 2
    assert items[1]["y"] == 1
    assert items[2]["y"] == 1


def test_delete_with_predicate_returns_count():
    items = monadb.connect().table("items", {"x": int})
    items.insert({"x": 1})
    items.insert({"x": 9})
    assert items.delete("x < ?", [5]) == 1
    assert items.count() == 1


def test_count_with_predicate():
    items = monadb.connect().table("items", {"x": int})
    items.insert({"x": 1})
    items.insert({"x": 9})
    assert items.count("x > ?", [3]) == 1
