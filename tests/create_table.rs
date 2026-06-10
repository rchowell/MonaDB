use heed::types::Bytes;
use heed::{Database, Env, EnvFlags, EnvOpenOptions, WithoutTls};
use monadb::MonaDB;
use monadb::ir::{Create, Statement, TableDefinition, Type};
use serde_json::Value;
use std::path::Path;
use tempfile::TempDir;

type BTree = Database<Bytes, Bytes>;

fn open_env(path: &Path) -> Env<WithoutTls> {
    unsafe {
        EnvOpenOptions::new()
            .read_txn_without_tls()
            .map_size(10 * 1024 * 1024)
            .max_dbs(8)
            .flags(EnvFlags::NO_SUB_DIR)
            .open(path)
            .unwrap()
    }
}

fn run(db: &mut MonaDB, sql: &str) {
    db.execute(sql).unwrap();
}

#[test]
fn create_table_persists_catalog_and_btree() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.db");

    // Phase A: create table t
    {
        let mut db = MonaDB::open(&path).unwrap();
        run(&mut db, "create table t (id int);");
    }
    {
        let env = open_env(&path);
        let txn = env.read_txn().unwrap();

        // Check that the table's btree was created (oid 1; oid 0 is the catalog)
        let _: BTree = env
            .open_database(&txn, Some("00000001"))
            .unwrap()
            .expect("per-table btree 00000001 should exist");

        // Check the catalog insert (catalog system table is oid 0)
        let catalog: BTree = env
            .open_database(&txn, Some("00000000"))
            .unwrap()
            .expect("catalog sub-db should exist");
        let row = catalog
            .get(&txn, &1u32.to_be_bytes())
            .unwrap()
            .expect("catalog row at oid=1");
        let val: Value = serde_json::from_slice(row).unwrap();
        assert_eq!(val["name"], "t");
        assert_eq!(val["type"], "table");

        // The schema lives only in `sql`; it is not persisted as a separate field.
        assert!(val.get("members").is_none(), "members must not be stored separately");

        // Check the SQL is well-formatted and parseable
        let sql = val["sql"].as_str().unwrap();
        let statement = MonaDB::parse(sql).unwrap();

        // Assert we have a create table statement with the correct name and member
        match statement {
            Statement::Create(Create::Table(TableDefinition { name, members, .. })) => {
                assert_eq!(name, "t");
                assert_eq!(members.len(), 1);
                assert_eq!(members[0].name, "id");
                assert_eq!(members[0].ty, Type::Int);
            }
            other => panic!("Expected create table statement, got {other:?}"),
        }
    }

    // Phase B: create table u; oid increments; phase-A row persists across reopen
    {
        let mut db = MonaDB::open(&path).unwrap();
        run(&mut db, "create table u (id int);");
    }
    {
        let env = open_env(&path);
        let txn = env.read_txn().unwrap();
        let catalog: BTree = env
            .open_database(&txn, Some("00000000"))
            .unwrap()
            .expect("catalog sub-db should still exist");

        // Phase-A row is still present after reopen + second write.
        assert!(
            catalog.get(&txn, &1u32.to_be_bytes()).unwrap().is_some(),
            "oid=1 row from phase A should persist"
        );

        // Phase-B row.
        let row = catalog
            .get(&txn, &2u32.to_be_bytes())
            .unwrap()
            .expect("catalog row at oid=2");
        let val: Value = serde_json::from_slice(row).unwrap();
        assert_eq!(val["name"], "u");
        assert_eq!(val["type"], "table");

        let _u_btree: BTree = env
            .open_database(&txn, Some("00000002"))
            .unwrap()
            .expect("per-table btree 00000002 should exist");
    }
}
