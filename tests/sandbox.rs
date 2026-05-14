use heed::Database;
use heed::Env;
use heed::EnvFlags;
use heed::EnvOpenOptions;
use heed::byteorder::BigEndian;
use heed::byteorder::ByteOrder;
use heed::types::{Bytes};
use serde_json::Value;
use serde_json::json;

type Cursor = Database<Bytes, Bytes>;

/// Opens the database.
fn open() -> Result<Env, Box<dyn std::error::Error>> {
    // Opens a new LMDB single-file database.
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("temp.db");
    let env = unsafe {
        EnvOpenOptions::new()
            .map_size(10 * 1024 * 1024)
            .max_dbs(3000)
            .flags(EnvFlags::NO_SUB_DIR)
            .open(path)?
    };
    // Initialize system tables, this is idempotent
    let mut txn = env.write_txn()?;
    let _: Database<Bytes, Bytes> = env.create_database(&mut txn, Some("_tables"))?;
    txn.commit()?;
    // TODO insert schema for _tables
    Ok(env)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let env = open()?;

    // BEGIN TRANSACTION 1
    // 
    // ```
    // create table foo;
    // ```
    //
    // 01: BEGIN WRITE TRANSACTION
    let mut txn = env.write_txn()?;
    //
    // 02: ALLOCATE THE NEW LMDB BTREE (IDEMPOTENT)
    let _: Cursor = env.create_database(&mut txn, Some("foo"))?;
    //
    // 03: OPEN CURSOR c=0, table='catalog'
    let c0 = env.open_database(&txn, Some("catalog"))?;
    let c0: Cursor = c0.expect("Missing system table: 'catalog'");
    //
    // 04: CREATE THE KEY
    let key = match c0.last(&txn)? {
        Some((key, _)) => BigEndian::read_u32(key) + 1,
        None => 0,
    };
    //
    // 05: CREATE THE VALUE
    let val = json!({
        "oid": key,
        "name": "foo",
        "type": "table",
        "sql": "create table foo;",
    });
    //
    // 06: INSERT (KEY, VALUE)
    let key_enc = key.to_be_bytes();
    let val_enc = serde_json::to_vec(&val)?;
    c0.put(&mut txn, &key_enc, &val_enc)?;
    //
    // 07: COMMIT
    txn.commit()?;
    //
    // END TRANSACTION 1


    // BEGIN TRANSACTION 2
    // 
    // ```
    // insert into foo ({ x: 1, y: 2 });
    // ```
    //
    // 01: BEGIN WRITE TRANSACTION
    let mut txn = env.write_txn()?;
    // 
    // 02: OPEN CURSOR c=0, table='foo'
    let c0 = env.open_database(&txn, Some("foo"))?;
    let c0: Cursor = c0.expect("Table 'foo' does not exist.");
    //
    // 03: CREATE THE KEY
    let key = match c0.last(&txn)? {
        Some((key, _)) => BigEndian::read_u32(key) + 1,
        None => 0,
    };
    //
    // 04: CREATE THE VALUE
    let val = json!({
        "x": 1,
        "y": 2,
    });
    //
    // 05: INSERT (KEY, VALUE)
    let key_enc = key.to_be_bytes();
    let val_enc = serde_json::to_vec(&val)?;
    c0.put(&mut txn, &key_enc, &val_enc)?;
    //
    // 06: COMMIT
    txn.commit()?;
    //
    // END TRANSACTION 1

    //
    // VERIFICATION
    //

    // select * from foo;
    let txn = env.read_txn()?;
    let cur: Cursor = env.open_database(&txn, Some("foo"))?.unwrap();
    for res in cur.iter(&txn)? {
        let item = res?;
        let key = BigEndian::read_u32(item.0);
        let val: Value = serde_json::from_slice(item.1)?;
        dbg!((key, val));
    }
    txn.commit()?;

    Ok(())
}

    // env -> transaction -> db(s)

    // you can iterate over ranges too!!!
    // let range = 35..=42;
    // let iter = db.range(&wtxn, &range)?;

    // for row in iter {
    //     let val = row?;
    //     dbg!(val);
    // }

    // let rets: Result<_, _> = db.range(&wtxn, &range)?.collect();
    // let rets: Vec<(i64, _)> = rets?;

    // let expected = vec![(35, "c"), (42, "d")];
    // assert_eq!(rets, expected);
    // dbg!(expected);

    // // even delete a range of keys
    // let range = 35..=42;
    // let deleted: usize = db.delete_range(&mut wtxn, &range)?;
    // dbg!(deleted);

    // let rets: Result<_, _> = db.iter(&wtxn)?.collect();
    // let rets: Vec<(i64, _)> = rets?;
    // let expected = vec![(0, "a"), (68, "b")];

    // assert_eq!(deleted, 2);
    // assert_eq!(rets, expected);