use monadb::MonaDB;
use tempfile::TempDir;


fn run(db: &mut MonaDB, sql: &str) {
    let mut rows = db.exec(sql, true).unwrap();
    while let Some(row) = rows.next().unwrap() {
        println!("{row}");
    }
}


#[test]
fn select_from_catalog() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.db");

    let mut mona = MonaDB::open(&path).unwrap();
    // run(&mut mona, "create table t (id int);");
    // run(&mut mona, "select * from catalog;");
}
