use monadb::MonaDB;
use tempfile::TempDir;

fn open() -> (TempDir, MonaDB) {
    let dir = TempDir::new().unwrap();
    let db = MonaDB::open(dir.path().join("test.db")).unwrap();
    (dir, db)
}

fn run(db: &mut MonaDB, sql: &str) {
    db.execute(sql).unwrap();
}

fn collect_names(db: &mut MonaDB, sql: &str) -> Vec<String> {
    let mut rows = db.query(sql, false).unwrap();
    let mut out = vec![];
    while let Some(row) = rows.next().unwrap() {
        let name = row.jpk("name").unwrap();
        out.push(name.as_str().unwrap().to_string());
    }
    out
}

#[test]
fn select_from_empty_catalog_yields_zero_rows() {
    let (_dir, mut db) = open();
    let mut rows = db.query("select * from catalog;", false).unwrap();
    assert!(rows.next().unwrap().is_none());
}

#[test]
fn select_after_create_table_yields_one_row() {
    let (_dir, mut db) = open();
    run(&mut db, "create table t (id int);");
    let names = collect_names(&mut db, "select * from catalog;");
    assert_eq!(names, vec!["t".to_string()]);
}

#[test]
fn select_after_two_creates_yields_two_rows_in_oid_order() {
    let (_dir, mut db) = open();
    run(&mut db, "create table a (id int);");
    run(&mut db, "create table b (id int);");
    let names = collect_names(&mut db, "select * from catalog;");
    assert_eq!(names, vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn select_row_has_table_metadata() {
    let (_dir, mut db) = open();
    run(&mut db, "create table t (id int);");
    let mut rows = db.query("select * from catalog;", false).unwrap();
    let row = rows.next().unwrap().unwrap();
    assert_eq!(row.jpk("name").unwrap().as_str(), Some("t"));
    assert_eq!(row.jpk("type").unwrap().as_str(), Some("table"));
    assert!(row.jpk("sql").unwrap().as_str().is_some());
    assert!(rows.next().unwrap().is_none());
}

#[test]
fn rows_yields_one_at_a_time() {
    let (_dir, mut db) = open();
    run(&mut db, "create table a (id int);");
    run(&mut db, "create table b (id int);");
    run(&mut db, "create table c (id int);");
    let mut rows = db.query("select * from catalog;", false).unwrap();
    assert_eq!(
        rows.next().unwrap().unwrap().jpk("name").unwrap().as_str(),
        Some("a")
    );
    assert_eq!(
        rows.next().unwrap().unwrap().jpk("name").unwrap().as_str(),
        Some("b")
    );
    assert_eq!(
        rows.next().unwrap().unwrap().jpk("name").unwrap().as_str(),
        Some("c")
    );
    assert!(rows.next().unwrap().is_none());
}

#[test]
fn commit_visibility_across_handles() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.db");
    {
        let mut db = MonaDB::open(&path).unwrap();
        run(&mut db, "create table persisted (id int);");
    }
    let mut db = MonaDB::open(&path).unwrap();
    let names = collect_names(&mut db, "select * from catalog;");
    assert_eq!(names, vec!["persisted".to_string()]);
}
