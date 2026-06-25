//! Public prepare / statement API integration tests.

use std::collections::HashMap;

use monadb::{MonaDB, Params, Value};

#[test]
fn prepare_query_reuse() {
    let mut db = MonaDB::memory().unwrap();
    db.execute("create table t;").unwrap();
    db.execute(r#"insert into t ({"x": 1});"#).unwrap();

    let mut stmt = db.prepare("select * from t;").unwrap();
    assert_eq!(
        stmt.query(()).unwrap().next().unwrap().unwrap().jpk("x"),
        Some(Value::int(1))
    );
    assert_eq!(
        stmt.query(()).unwrap().next().unwrap().unwrap().jpk("x"),
        Some(Value::int(1))
    );
}

#[test]
fn prepare_execute_mutation() {
    let mut db = MonaDB::memory().unwrap();
    db.execute("create table t (id int);").unwrap();
    let mut stmt = db.prepare("insert into t ($1);").unwrap();
    for id in 1..=3 {
        let val = Value::from_json(serde_json::json!({"id": id}));
        stmt.execute([val]).unwrap();
    }
    let mut rows = db.query("select * from t;").unwrap();
    let mut n = 0;
    while rows.next().unwrap().is_some() {
        n += 1;
    }
    assert_eq!(n, 3);
}

#[test]
fn into_params_positional() {
    let mut db = MonaDB::memory().unwrap();
    let mut stmt = db.prepare("select [?, ?];").unwrap();
    let row = stmt
        .query((1i64, 2i64))
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    let Value::Array(items) = row else {
        panic!("expected array result");
    };
    assert_eq!(items[0], Value::int(1));
    assert_eq!(items[1], Value::int(2));
}

#[test]
fn into_params_named() {
    let mut db = MonaDB::memory().unwrap();
    let mut stmt = db.prepare("select $greeting;").unwrap();
    let mut named = HashMap::new();
    named.insert("greeting".into(), Value::String(std::rc::Rc::from("hi")));
    let row = stmt.query(named).unwrap().next().unwrap().unwrap();
    assert_eq!(row, Value::String(std::rc::Rc::from("hi")));
}

#[test]
fn prepare_matches_query_with() {
    let mut db = MonaDB::memory().unwrap();
    db.execute("create table t;").unwrap();
    db.execute(r#"insert into t ({"x": 1});"#).unwrap();

    let sql = "select * from t;";
    let direct = db
        .query_with(sql, Params::none())
        .unwrap()
        .next()
        .unwrap()
        .unwrap();

    let mut stmt = db.prepare(sql).unwrap();
    let prepared = stmt.query(()).unwrap().next().unwrap().unwrap();
    assert_eq!(direct, prepared);
}

#[test]
fn prepare_cached_reuses_plan() {
    let mut db = MonaDB::memory().unwrap();
    {
        let mut s1 = db.prepare_cached("select ?;").unwrap();
        assert_eq!(s1.query([1i64]).unwrap().next().unwrap().unwrap(), Value::int(1));
    }
    {
        let mut s2 = db.prepare_cached("select ?;").unwrap();
        assert_eq!(s2.query([2i64]).unwrap().next().unwrap().unwrap(), Value::int(2));
    }
}

#[test]
    fn stale_after_drop() {
        let mut db = MonaDB::memory().unwrap();
        db.execute("create table t;").unwrap();
    {
        db.prepare_cached("select * from t;")
            .unwrap()
            .query(())
            .unwrap();
    }
    db.execute("drop table t;").unwrap();
    let mut stmt = db.prepare_cached("select * from t;").unwrap();
    assert!(stmt.query(()).is_err());
    }

#[test]
fn missing_param() {
    let mut db = MonaDB::memory().unwrap();
    let mut stmt = db.prepare("select ?;").unwrap();
    assert!(stmt.query(()).is_err());
}

#[test]
fn parameter_count() {
    let mut db = MonaDB::memory().unwrap();
    let stmt = db.prepare("select [?, $2];").unwrap();
    assert_eq!(stmt.parameter_count(), 2);
}

#[test]
fn parameter_count_dedups_repeated_placeholder() {
    let mut db = MonaDB::memory().unwrap();
    let stmt = db.prepare("select [$1, $1];").unwrap();
    assert_eq!(stmt.parameter_count(), 1);
}

#[test]
fn keyed_lookup_via_prepare() {
    let mut db = MonaDB::memory().unwrap();
    db.execute("create table t (id int);").unwrap();
    db.execute(r#"insert into t ({"id": 1, "v": "a"});"#).unwrap();
    let mut stmt = db.prepare("select t[?];").unwrap();
    let row = stmt.query([1i64]).unwrap().next().unwrap().unwrap();
    assert_eq!(row.jpk("v"), Some(Value::String(std::rc::Rc::from("a"))));
}
