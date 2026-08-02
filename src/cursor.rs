//! Row traversal over a btree table, a file, or an in-memory value.
//!
//! A [`Cursor`] is the VM's iteration register: scans, point/range gets,
//! inserts, and deletes all run through one. It unifies a persistent btree scan,
//! a streaming file reader, and an in-memory array behind the same
//! `scan`/`next`/`load` interface.

use heed::types::Bytes;
use heed::{RoIter, RoPrefix, RoRevIter, RoRevPrefix};

use crate::error::{Error, Result};
use crate::read::{self, FileSource, RowReader};
use crate::value::Value;
use crate::{storage::BTree, transaction::Transaction};

/// The error returned when a row is requested from a cursor that is not
/// positioned on one (no scan begun, or iteration exhausted).
fn unpositioned_cursor() -> Error {
    Error::InternalError("load on unpositioned cursor".to_string())
}

/// Controls traversal of one source — a btree table or an in-memory value.
///
/// A table-backed cursor moves through a fixed lifecycle. `close` drops the read
/// iterator but leaves the cursor open, so it can still be written through:
///
///   new ─▶ open ─▶ scanning ─▶ exhausted
///           ▲                      │
///           └──────── close ───────┘
#[derive(Default)]
pub struct Cursor {
    /// The bound source, if any.
    source: Option<Source>,
}

impl Cursor {
    /// Creates a new unopened and unpositioned cursor.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true if this cursor is already open on a btree.
    pub fn is_open(&self) -> bool {
        matches!(&self.source, Some(Source::Btree { .. }))
    }

    /// Opens this cursor on the given btree (a table source, not yet scanned).
    pub fn open(&mut self, btree: BTree) {
        self.source = Some(Source::Btree { btree, scan: None });
    }

    /// Begins a forward scan, optionally restricted to keys sharing `prefix`;
    /// returns true if there's an available row. A non-empty prefix scans only
    /// the contiguous run of matching keys (a leading-column range); an absent
    /// or empty prefix scans the whole table.
    pub fn scan(&mut self, txn: &Transaction, prefix: Option<&[u8]>) -> Result<bool> {
        let Some(Source::Btree { btree, scan }) = &mut self.source else {
            panic!("cursor must be open");
        };
        let btree = *btree;
        let rtxn = txn.as_ro();
        // SAFETY: the iterator borrows the storage env, which outlives the
        // cursor (yolk keep-alive); the lifetime is erased to 'static for
        // self-reference. A non-empty prefix uses LMDB's prefix iterator — the
        // order-preserving, self-delimiting key encoding makes a leading-column
        // byte prefix an exact match at the column boundary.
        let iter = match prefix {
            Some(p) if !p.is_empty() => {
                let iter = btree.prefix_iter(rtxn, p)?;
                let iter: RoPrefix<'static, Bytes, Bytes> = unsafe { std::mem::transmute(iter) };
                TableIter::FwdPre(iter)
            }
            _ => {
                let iter = btree.iter(rtxn)?;
                let iter: RoIter<'static, Bytes, Bytes> = unsafe { std::mem::transmute(iter) };
                TableIter::Fwd(iter)
            }
        };
        let mut table = TableScan::new(iter);
        let available = table.next()?;
        *scan = Some(table);
        // Return true if there's an available row
        Ok(available)
    }

    /// Opens `spec`'s file and positions on its first row; returns true if
    /// there's an available row.
    ///
    /// Restartable exactly like [`Cursor::scan`]: each call builds a fresh
    /// reader and replaces the bound source, so a file source re-entered by a
    /// nested loop reopens the file from the top. The reader owns its `File`,
    /// so — unlike a btree scan, whose heed iterator borrows the storage env —
    /// no lifetime erasure is involved here.
    pub fn open_file(&mut self, spec: &FileSource) -> Result<bool> {
        let reader = read::open_rows(&spec.path, spec.format, &spec.options)?;
        let mut scan = FileScan { reader, curr: None };
        let available = scan.next()?;
        self.source = Some(Source::File(scan));
        Ok(available)
    }

    /// Binds this cursor to a single in-memory value, so `load` returns it.
    /// Used to re-seed a from-binding from a materialized payload after a sort
    /// (ORDER BY), keeping projection compilation identical to a live scan.
    pub fn set(&mut self, value: Value) {
        self.source = Some(Source::Single(value));
    }

    /// Begins iterating an array value; returns true if there's an available row.
    #[allow(clippy::unnecessary_wraps, clippy::iter_not_returning_iterator)]
    pub fn iter(&mut self, source: Value) -> Result<bool> {
        let mut value = ValueIter::new(source);
        let available = value.next();
        self.source = Some(Source::Value(value));
        Ok(available)
    }

    /// Advances the scan; returns true if there's an available row
    pub fn next(&mut self) -> Result<bool> {
        match &mut self.source {
            Some(Source::Btree {
                scan: Some(scan), ..
            }) => scan.next(),
            Some(Source::Value(value)) => Ok(value.next()),
            Some(Source::File(scan)) => scan.next(),
            // Unreachable unless there's a compiler bug.
            _ => Err(Error::InternalError(
                "next called on a cursor with no scan state".to_string(),
            )),
        }
    }

    /// Returns the current (key,val) bytes; table-backed scans only.
    pub fn current(&self) -> Option<(&[u8], &[u8])> {
        match &self.source {
            Some(Source::Btree {
                scan: Some(scan), ..
            }) => scan.current(),
            // Value-backed (or unpositioned) cursors have no raw key/val bytes.
            _ => None,
        }
    }

    /// Returns the current row as a decoded Value.
    pub fn load(&self) -> Result<Value> {
        match &self.source {
            Some(Source::Btree {
                scan: Some(scan), ..
            }) => scan.load(),
            Some(Source::Value(value)) => value.load(),
            Some(Source::File(scan)) => scan.load(),
            Some(Source::Single(value)) => Ok(value.clone()),
            _ => Err(unpositioned_cursor()),
        }
    }

    /// The open btree handle; panics if the cursor is unopened or value-backed.
    fn btree(&self) -> BTree {
        let Some(Source::Btree { btree, .. }) = &self.source else {
            panic!("cursor must be open on a btree");
        };
        *btree
    }

    /// Inserts the value at the given key.
    pub fn insert(&self, txn: &mut Transaction, key: &[u8], val: &[u8]) -> Result<()> {
        self.btree().put(txn.as_rw()?, key, val)?;
        Ok(())
    }

    /// Deletes the row at the given key.
    pub fn delete(&self, txn: &mut Transaction, key: &[u8]) -> Result<()> {
        self.btree().delete(txn.as_rw()?, key)?;
        Ok(())
    }

    /// Returns the value at the given key or null.
    pub fn get(&self, txn: &Transaction, key: &[u8]) -> Result<Value> {
        match self.btree().get(txn.as_ro(), key)? {
            Some(bytes) => Value::from_storage(bytes),
            None => Ok(Value::Null),
        }
    }

    /// Ends an active scan and releases the read iterator.
    ///
    /// A btree keeps its handle so the cursor can still be written through; a
    /// file has no such handle, so the whole source is dropped and the file
    /// descriptor released.
    pub fn close(&mut self) {
        match &mut self.source {
            Some(Source::Btree { scan, .. }) => *scan = None,
            Some(Source::File(_)) => self.source = None,
            _ => {}
        }
    }

    /// Returns the last (key, val) in the btree, or none if it is empty.
    pub fn last(&self, txn: &Transaction) -> Result<Option<(Vec<u8>, Vec<u8>)>> {
        let res = self.btree().last(txn.as_ro())?;
        let res = res.map(|(k, v)| (k.to_vec(), v.to_vec()));
        Ok(res)
    }
}

/// A cursor's bound source: a persistent btree table, a file, or an in-memory
/// value.
///
/// Not to be confused with [`crate::ast::Source`], the from-clause AST node —
/// this one is the runtime binding, that one is the compile-time description.
enum Source {
    /// The btree handle persists across the open → (scan | insert/last)
    /// lifecycle; `scan` becomes `Some` only once a forward scan has begun.
    Btree {
        btree: BTree,
        scan: Option<TableScan>,
    },
    /// Element-wise iteration over an in-memory array value.
    Value(ValueIter),
    /// Row-at-a-time iteration over an open file reader.
    File(FileScan),
    /// A single in-memory value (re-seeded via `set`); `load` returns it.
    Single(Value),
}

/// File-backed scan state.
///
/// The parallel of [`TableScan`]: it owns the open reader and caches the row it
/// is positioned on, so `load` never re-reads.
struct FileScan {
    /// The open reader; dropped when the cursor is closed or rebound.
    reader: RowReader,
    /// The row the cursor is positioned on, or none once exhausted.
    curr: Option<Value>,
}

impl FileScan {
    /// Advances to the next row, caching it; true if one exists.
    fn next(&mut self) -> Result<bool> {
        self.curr = self.reader.next_row()?;
        Ok(self.curr.is_some())
    }

    fn load(&self) -> Result<Value> {
        self.curr.clone().ok_or_else(unpositioned_cursor)
    }
}

/// Value-backed iteration over a collection's elements.
///
/// Holds the source value and a position, indexing lazily via `Value::jpi`.
/// The source is copied in today; this shape also fits a future shared
/// reference (`Rc<Value>` / borrow) with no change to `next`/`load`.
struct ValueIter {
    source: Value,
    pos: Option<usize>,
}

impl ValueIter {
    fn new(source: Value) -> Self {
        Self { source, pos: None }
    }

    /// Advances to the next element; true if one is available. A non-array
    /// source (or an out-of-range index) yields nothing. This only inspects
    /// the array length, so it never clones an element — `load` does that.
    fn next(&mut self) -> bool {
        let i = self.pos.map_or(0, |p| p + 1);
        self.pos = Some(i);
        self.source.len().is_some_and(|len| i < len)
    }

    /// Returns the current element.
    fn load(&self) -> Result<Value> {
        self.pos
            .and_then(|i| self.source.jpi(i))
            .ok_or_else(unpositioned_cursor)
    }
}

/// Table-backed scan state.
struct TableScan {
    /// Underlying table iterator.
    iter: TableIter,
    /// Current (key,val) owned bytes.
    curr: Option<(Vec<u8>, Vec<u8>)>,
}

impl TableScan {
    fn new(iter: TableIter) -> Self {
        Self { iter, curr: None }
    }

    /// Advances to the next row, caching its owned bytes; true if one exists.
    fn next(&mut self) -> Result<bool> {
        self.curr = self.iter.next()?.map(|(k, v)| (k.to_vec(), v.to_vec()));
        Ok(self.curr.is_some())
    }

    fn load(&self) -> Result<Value> {
        let (_, val) = self.current().ok_or_else(unpositioned_cursor)?;
        Value::from_storage(val)
    }

    fn current(&self) -> Option<(&[u8], &[u8])> {
        self.curr
            .as_ref()
            .map(|(k, v)| (k.as_slice(), v.as_slice()))
    }
}

/// The inner table iterator; dispatches on type.
#[allow(unused)]
enum TableIter {
    /// Forward iteration.
    Fwd(RoIter<'static, Bytes, Bytes>),
    /// Forward iteration with prefix.
    FwdPre(RoPrefix<'static, Bytes, Bytes>),
    /// Reverse iteration.
    Rev(RoRevIter<'static, Bytes, Bytes>),
    /// Reverse iteration with prefix.
    RevPre(RoRevPrefix<'static, Bytes, Bytes>),
}

impl TableIter {
    /// Returns the next (key,val) reference or none.
    fn next(&mut self) -> Result<Option<(&[u8], &[u8])>> {
        let res = match self {
            TableIter::Fwd(it) => it.next(),
            TableIter::FwdPre(it) => it.next(),
            TableIter::Rev(it) => it.next(),
            TableIter::RevPre(it) => it.next(),
        };
        Ok(res.transpose()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::read::{FileFormat, ReadOptions};
    use crate::storage::Storage;
    use std::io::Write;
    use tempfile::{NamedTempFile, TempDir};

    /// Writes `text` to a temp file and returns it with a `FileSource` over it.
    /// The returned handle must outlive the scope — dropping it unlinks the file.
    fn file_fixture(text: &str, format: FileFormat) -> (NamedTempFile, FileSource) {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "{text}").unwrap();
        f.flush().unwrap();
        let spec = FileSource {
            path: f.path().to_str().unwrap().to_string(),
            format,
            options: ReadOptions::default(),
        };
        (f, spec)
    }

    #[test]
    fn file_scan_positions_on_first_row() {
        let (_f, spec) = file_fixture("{\"x\":1}\n{\"x\":2}\n", FileFormat::Jsonl);
        let mut cursor = Cursor::new();
        assert!(cursor.open_file(&spec).unwrap());
        assert_eq!(cursor.load().unwrap().jpk("x"), Some(Value::Int(1)));
        assert!(cursor.next().unwrap());
        assert_eq!(cursor.load().unwrap().jpk("x"), Some(Value::Int(2)));
        assert!(!cursor.next().unwrap());
    }

    #[test]
    fn file_scan_empty_file_returns_false() {
        let (_f, spec) = file_fixture("", FileFormat::Jsonl);
        let mut cursor = Cursor::new();
        assert!(!cursor.open_file(&spec).unwrap());
        assert!(cursor.load().is_err());
    }

    /// The mirror of [`scan_restart_repositions_on_first_row`]. A file source
    /// nested inside a join is re-entered once per outer row, so `open_file`
    /// must reopen from the top rather than continue where it left off.
    #[test]
    fn file_scan_restart_repositions_on_first_row() {
        let (_f, spec) = file_fixture("{\"x\":1}\n{\"x\":2}\n", FileFormat::Jsonl);
        let mut cursor = Cursor::new();
        assert!(cursor.open_file(&spec).unwrap());
        assert!(cursor.next().unwrap());
        assert_eq!(cursor.load().unwrap().jpk("x"), Some(Value::Int(2)));

        assert!(cursor.open_file(&spec).unwrap());
        assert_eq!(cursor.load().unwrap().jpk("x"), Some(Value::Int(1)));
    }

    /// A file has no raw key/val bytes, so it never reports a current entry —
    /// the same contract as a value-backed cursor.
    #[test]
    fn file_scan_current_is_none() {
        let (_f, spec) = file_fixture("{\"x\":1}\n", FileFormat::Jsonl);
        let mut cursor = Cursor::new();
        assert!(cursor.open_file(&spec).unwrap());
        assert!(cursor.current().is_none());
    }

    /// Unlike a btree, a file keeps no handle worth preserving, so `close`
    /// unbinds the source entirely and releases the descriptor.
    #[test]
    fn file_close_releases_reader() {
        let (_f, spec) = file_fixture("{\"x\":1}\n", FileFormat::Jsonl);
        let mut cursor = Cursor::new();
        assert!(cursor.open_file(&spec).unwrap());
        cursor.close();
        assert!(cursor.load().is_err());
        assert!(cursor.next().is_err());
        // Still reusable.
        assert!(cursor.open_file(&spec).unwrap());
    }

    #[test]
    fn file_scan_reads_csv_rows() {
        let (_f, spec) = file_fixture("a,b\n1,2\n3,4\n", FileFormat::Csv);
        let mut cursor = Cursor::new();
        assert!(cursor.open_file(&spec).unwrap());
        assert_eq!(cursor.load().unwrap().jpk("a"), Some(Value::Int(1)));
        assert!(cursor.next().unwrap());
        assert_eq!(cursor.load().unwrap().jpk("b"), Some(Value::Int(4)));
        assert!(!cursor.next().unwrap());
    }

    /// A malformed row surfaces as an error only when the cursor advances onto
    /// it — the cursor-level statement of streaming.
    #[test]
    fn file_scan_defers_parse_error_until_reached() {
        let (_f, spec) = file_fixture("{\"x\":1}\n{\"x\":2\n", FileFormat::Jsonl);
        let mut cursor = Cursor::new();
        assert!(cursor.open_file(&spec).unwrap());
        assert_eq!(cursor.load().unwrap().jpk("x"), Some(Value::Int(1)));
        assert!(cursor.next().is_err());
    }

    /// Builds a fresh on-disk env containing a btree named "test" populated
    /// with the given rows. The returned tempdir must outlive the test scope.
    fn fixture(rows: &[(&[u8], &[u8])]) -> (TempDir, Storage) {
        let dir = TempDir::new().unwrap();
        let storage = Storage::open(dir.path().join("test.db")).unwrap();
        let mut txn = Transaction::write(&storage).unwrap();
        let btree = storage.create_btree(&mut txn, 1).unwrap();
        {
            let wtxn = txn.as_rw().unwrap();
            for (k, v) in rows {
                btree.put(wtxn, k, v).unwrap();
            }
        }
        txn.commit().unwrap();
        (dir, storage)
    }

    /// Opens a fresh read txn against the "test" btree.
    fn open_read(storage: &Storage) -> (Transaction, BTree) {
        let txn = Transaction::read(storage).unwrap();
        let btree = storage.open_btree(&txn, 1).unwrap();
        (txn, btree)
    }

    /// Walks a positioned cursor to exhaustion, returning all owned rows.
    fn drain(cursor: &mut Cursor) -> Vec<(Vec<u8>, Vec<u8>)> {
        let mut rows = Vec::new();
        while let Some((k, v)) = cursor.current() {
            rows.push((k.to_vec(), v.to_vec()));
            cursor.next().unwrap();
        }
        rows
    }

    #[test]
    fn scan_empty_btree_returns_false() {
        let (_dir, storage) = fixture(&[]);
        let (txn, btree) = open_read(&storage);
        let mut cursor = Cursor::new();
        cursor.open(btree);
        assert!(!cursor.scan(&txn, None).unwrap());
        assert_eq!(cursor.current(), None);
    }

    #[test]
    fn scan_single_row_positions_on_it() {
        let (_dir, storage) = fixture(&[(b"a", b"1")]);
        let (txn, btree) = open_read(&storage);
        let mut cursor = Cursor::new();
        cursor.open(btree);
        assert!(cursor.scan(&txn, None).unwrap());
        assert_eq!(cursor.current(), Some((b"a".as_slice(), b"1".as_slice())));
        assert!(!cursor.next().unwrap());
        assert_eq!(cursor.current(), None);
    }

    #[test]
    fn scan_walks_keys_in_lexicographic_order() {
        let rows: &[(&[u8], &[u8])] = &[(b"c", b"3"), (b"a", b"1"), (b"b", b"2")];
        let (_dir, storage) = fixture(rows);
        let (txn, btree) = open_read(&storage);
        let mut cursor = Cursor::new();
        cursor.open(btree);
        assert!(cursor.scan(&txn, None).unwrap());
        let collected = drain(&mut cursor);
        let keys: Vec<&[u8]> = collected.iter().map(|(k, _)| k.as_slice()).collect();
        assert_eq!(
            keys,
            vec![b"a".as_slice(), b"b".as_slice(), b"c".as_slice()]
        );
    }

    #[test]
    fn scan_yields_correct_values() {
        let rows: &[(&[u8], &[u8])] = &[(b"c", b"three"), (b"a", b"one"), (b"b", b"two")];
        let (_dir, storage) = fixture(rows);
        let (txn, btree) = open_read(&storage);
        let mut cursor = Cursor::new();
        cursor.open(btree);
        assert!(cursor.scan(&txn, None).unwrap());
        let collected = drain(&mut cursor);
        assert_eq!(
            collected,
            vec![
                (b"a".to_vec(), b"one".to_vec()),
                (b"b".to_vec(), b"two".to_vec()),
                (b"c".to_vec(), b"three".to_vec()),
            ]
        );
    }

    #[test]
    fn next_after_exhaustion_is_idempotent() {
        let (_dir, storage) = fixture(&[(b"a", b"1")]);
        let (txn, btree) = open_read(&storage);
        let mut cursor = Cursor::new();
        cursor.open(btree);
        assert!(cursor.scan(&txn, None).unwrap());
        assert!(!cursor.next().unwrap());
        assert!(!cursor.next().unwrap());
        assert!(!cursor.next().unwrap());
        assert_eq!(cursor.current(), None);
    }

    #[test]
    fn get_existing_key_returns_row() {
        // Rows are stored in the flat layout, so encode the value before storing.
        let row = Value::decode(br#"{"id":1,"v":"a"}"#).unwrap();
        let bytes = row.encode().unwrap();
        let (_dir, storage) = fixture(&[(b"k1", bytes.as_slice())]);
        let (txn, btree) = open_read(&storage);
        let mut cursor = Cursor::new();
        cursor.open(btree);
        let got = cursor.get(&txn, b"k1").unwrap();
        assert!(got == row);
    }

    #[test]
    fn get_missing_key_returns_null() {
        let (_dir, storage) = fixture(&[(b"k1", br#"{"id":1}"#)]);
        let (txn, btree) = open_read(&storage);
        let mut cursor = Cursor::new();
        cursor.open(btree);
        assert!(cursor.get(&txn, b"absent").unwrap().is_null());
    }

    #[test]
    fn get_among_many_picks_one() {
        // Rows are stored in the flat layout, so encode each value before storing.
        let enc = |json: &[u8]| Value::decode(json).unwrap().encode().unwrap();
        let (v1, v2, v3) = (
            enc(br#"{"id":1}"#),
            enc(br#"{"id":2}"#),
            enc(br#"{"id":3}"#),
        );
        let rows: &[(&[u8], &[u8])] = &[(b"k1", &v1), (b"k2", &v2), (b"k3", &v3)];
        let (_dir, storage) = fixture(rows);
        let (txn, btree) = open_read(&storage);
        let mut cursor = Cursor::new();
        cursor.open(btree);
        let got = cursor.get(&txn, b"k2").unwrap();
        assert_eq!(got.jpk("id"), Some(Value::Int(2)));
    }

    #[test]
    fn current_is_none_before_first_scan() {
        let (_dir, storage) = fixture(&[(b"a", b"1")]);
        let (_txn, btree) = open_read(&storage);
        let mut cursor = Cursor::new();
        cursor.open(btree);
        assert_eq!(cursor.current(), None);
    }

    #[test]
    fn next_without_scan_returns_error() {
        let (_dir, storage) = fixture(&[(b"a", b"1")]);
        let (_txn, btree) = open_read(&storage);
        let mut cursor = Cursor::new();
        cursor.open(btree);
        let err = cursor.next().unwrap_err();
        assert!(matches!(err, Error::InternalError(_)));
    }

    #[test]
    fn scan_restart_repositions_on_first_row() {
        let rows: &[(&[u8], &[u8])] = &[(b"a", b"1"), (b"b", b"2"), (b"c", b"3")];
        let (_dir, storage) = fixture(rows);
        let (txn, btree) = open_read(&storage);
        let mut cursor = Cursor::new();
        cursor.open(btree);

        // First scan: walk to exhaustion.
        assert!(cursor.scan(&txn, None).unwrap());
        let first = drain(&mut cursor);
        assert_eq!(first.len(), 3);
        assert_eq!(cursor.current(), None);

        // Restart: same cursor, same txn, expect identical traversal.
        assert!(cursor.scan(&txn, None).unwrap());
        assert_eq!(cursor.current(), Some((b"a".as_slice(), b"1".as_slice())));
        let second = drain(&mut cursor);
        assert_eq!(second, first);
    }

    #[test]
    fn multiple_cursors_share_btree_independently() {
        let rows: &[(&[u8], &[u8])] = &[(b"a", b"1"), (b"b", b"2"), (b"c", b"3")];
        let (_dir, storage) = fixture(rows);
        let (txn, btree) = open_read(&storage);

        let mut a = Cursor::new();
        a.open(btree);
        let mut b = Cursor::new();
        b.open(btree);

        assert!(a.scan(&txn, None).unwrap());
        assert!(b.scan(&txn, None).unwrap());
        assert_eq!(a.current(), Some((b"a".as_slice(), b"1".as_slice())));
        assert_eq!(b.current(), Some((b"a".as_slice(), b"1".as_slice())));

        // Advance only `a`; `b` must remain at the start.
        assert!(a.next().unwrap());
        assert_eq!(a.current(), Some((b"b".as_slice(), b"2".as_slice())));
        assert_eq!(b.current(), Some((b"a".as_slice(), b"1".as_slice())));

        let a_rest = drain(&mut a);
        let b_rest = drain(&mut b);
        assert_eq!(a_rest.len(), 2);
        assert_eq!(b_rest.len(), 3);
    }

    #[test]
    fn current_bytes_match_inserted_payload() {
        let key: Vec<u8> = (0u8..200).collect();
        let val: Vec<u8> = (0u8..=255).collect();
        let rows: &[(&[u8], &[u8])] = &[(key.as_slice(), val.as_slice())];
        let (_dir, storage) = fixture(rows);
        let (txn, btree) = open_read(&storage);
        let mut cursor = Cursor::new();
        cursor.open(btree);
        assert!(cursor.scan(&txn, None).unwrap());
        let (k, v) = cursor.current().unwrap();
        assert_eq!(k, key.as_slice());
        assert_eq!(v, val.as_slice());
    }

    #[test]
    fn scan_sees_committed_writes_only() {
        // The fixture commits two rows; a fresh read txn must observe both.
        let rows: &[(&[u8], &[u8])] = &[(b"a", b"1"), (b"b", b"2")];
        let (_dir, storage) = fixture(rows);
        let (txn, btree) = open_read(&storage);
        let mut cursor = Cursor::new();
        cursor.open(btree);
        assert!(cursor.scan(&txn, None).unwrap());
        let collected = drain(&mut cursor);
        assert_eq!(collected.len(), 2);
    }

    #[test]
    fn scan_with_prefix_yields_only_matching_keys() {
        let rows: &[(&[u8], &[u8])] = &[(b"aa", b"1"), (b"ab", b"2"), (b"ba", b"3"), (b"bb", b"4")];
        let (_dir, storage) = fixture(rows);
        let (txn, btree) = open_read(&storage);
        let mut cursor = Cursor::new();
        cursor.open(btree);
        assert!(cursor.scan(&txn, Some(b"a")).unwrap());
        let collected = drain(&mut cursor);
        let keys: Vec<&[u8]> = collected.iter().map(|(k, _)| k.as_slice()).collect();
        assert_eq!(keys, vec![b"aa".as_slice(), b"ab".as_slice()]);
    }

    #[test]
    fn scan_with_empty_prefix_matches_everything() {
        let rows: &[(&[u8], &[u8])] = &[(b"a", b"1"), (b"b", b"2")];
        let (_dir, storage) = fixture(rows);
        let (txn, btree) = open_read(&storage);
        let mut cursor = Cursor::new();
        cursor.open(btree);
        assert!(cursor.scan(&txn, Some(b"")).unwrap());
        let collected = drain(&mut cursor);
        assert_eq!(collected.len(), 2);
    }

    #[test]
    fn scan_with_prefix_on_empty_btree_returns_false() {
        let (_dir, storage) = fixture(&[]);
        let (txn, btree) = open_read(&storage);
        let mut cursor = Cursor::new();
        cursor.open(btree);
        assert!(!cursor.scan(&txn, Some(b"x")).unwrap());
        assert_eq!(cursor.current(), None);
    }

    #[test]
    fn insert_visible_via_scan_in_same_txn() {
        let (_dir, storage) = fixture(&[]);
        let mut txn = Transaction::write(&storage).unwrap();
        let btree = storage.open_btree(&txn, 1).unwrap();
        let mut cursor = Cursor::new();
        cursor.open(btree);
        cursor.insert(&mut txn, b"k", b"v").unwrap();
        assert!(cursor.scan(&txn, None).unwrap());
        assert_eq!(cursor.current(), Some((b"k".as_slice(), b"v".as_slice())));
    }

    #[test]
    fn insert_fails_on_read_only_txn() {
        let (_dir, storage) = fixture(&[]);
        let mut ro = Transaction::read(&storage).unwrap();
        let btree = storage.open_btree(&ro, 1).unwrap();
        let mut cursor = Cursor::new();
        cursor.open(btree);
        let err = cursor.insert(&mut ro, b"k", b"v").unwrap_err();
        assert!(matches!(err, Error::InternalError(_)));
    }

    #[test]
    fn last_returns_none_on_empty() {
        let (_dir, storage) = fixture(&[]);
        let (txn, btree) = open_read(&storage);
        let mut cursor = Cursor::new();
        cursor.open(btree);
        assert_eq!(cursor.last(&txn).unwrap(), None);
    }

    #[test]
    fn last_returns_owned_max_key() {
        let (_dir, storage) = fixture(&[(b"a", b"1"), (b"c", b"3"), (b"b", b"2")]);
        let (txn, btree) = open_read(&storage);
        let mut cursor = Cursor::new();
        cursor.open(btree);
        let (k, v) = cursor.last(&txn).unwrap().unwrap();
        assert_eq!(k, b"c");
        assert_eq!(v, b"3");
    }

    #[test]
    fn is_open_reflects_cursor_state() {
        let (_dir, storage) = fixture(&[(b"a", b"1")]);
        let (_txn, btree) = open_read(&storage);
        let mut cursor = Cursor::new();
        assert!(!cursor.is_open(), "new cursor should not be open");
        cursor.open(btree);
        assert!(cursor.is_open(), "cursor should be open after open()");
    }

    #[test]
    fn drop_cursor_before_commit_is_safe() {
        let (_dir, storage) = fixture(&[(b"a", b"1"), (b"b", b"2")]);
        let txn = Transaction::read(&storage).unwrap();
        let btree = storage.open_btree(&txn, 1).unwrap();
        {
            let mut cursor = Cursor::new();
            cursor.open(btree);
            assert!(cursor.scan(&txn, None).unwrap());
        }
        txn.commit().unwrap();
    }

    #[test]
    fn write_txn_commits_after_cursor_state() {
        let (_dir, storage) = fixture(&[]);
        let mut txn = Transaction::write(&storage).unwrap();
        let btree = storage.open_btree(&txn, 1).unwrap();
        {
            let mut cursor = Cursor::new();
            cursor.open(btree);
            cursor.insert(&mut txn, b"k", b"v").unwrap();
        }
        txn.commit().unwrap();

        let (txn, btree) = open_read(&storage);
        let mut cursor = Cursor::new();
        cursor.open(btree);
        let (k, v) = cursor.last(&txn).unwrap().unwrap();
        assert_eq!(k, b"k");
        assert_eq!(v, b"v");
    }

    #[test]
    fn delete_removes_row() {
        let (_dir, storage) = fixture(&[(b"a", b"1"), (b"b", b"2")]);
        let mut txn = Transaction::write(&storage).unwrap();
        let btree = storage.open_btree(&txn, 1).unwrap();
        let mut cursor = Cursor::new();
        cursor.open(btree);
        cursor.delete(&mut txn, b"a").unwrap();
        // 'a' is gone; 'b' remains, visible to a scan in the same txn.
        assert!(cursor.scan(&txn, None).unwrap());
        assert_eq!(cursor.current(), Some((b"b".as_slice(), b"2".as_slice())));
        assert!(!cursor.next().unwrap());
    }

    #[test]
    fn close_scan_releases_iterator() {
        let (_dir, storage) = fixture(&[(b"a", b"1")]);
        let (txn, btree) = open_read(&storage);
        let mut cursor = Cursor::new();
        cursor.open(btree);
        assert!(cursor.scan(&txn, None).unwrap());
        assert!(cursor.current().is_some());
        cursor.close();
        // After close, the cursor is unpositioned but still open (re-scannable).
        assert_eq!(cursor.current(), None);
        assert!(cursor.scan(&txn, None).unwrap());
    }

    #[test]
    fn delete_after_close_scan_on_same_cursor() {
        // Pins the contract cc_delete relies on: scan a table, capture a key,
        // release the read iterator with close_scan, then delete through the
        // same cursor — all in one write txn.
        let (_dir, storage) = fixture(&[(b"a", b"1"), (b"b", b"2")]);
        let mut txn = Transaction::write(&storage).unwrap();
        let btree = storage.open_btree(&txn, 1).unwrap();
        let mut cursor = Cursor::new();
        cursor.open(btree);

        // Phase 1: scan and capture the first key.
        assert!(cursor.scan(&txn, None).unwrap());
        let key = cursor.current().unwrap().0.to_vec();

        // Phase 2: release the read iterator, then delete the captured key.
        cursor.close();
        cursor.delete(&mut txn, &key).unwrap();

        // The captured row is gone; the other remains.
        assert!(cursor.scan(&txn, None).unwrap());
        assert_eq!(cursor.current(), Some((b"b".as_slice(), b"2".as_slice())));
        assert!(!cursor.next().unwrap());
    }

    // These cannot compile until `Cursor::scan_rev` is supported
    //
    // #[test]
    // fn scan_rev_walks_in_reverse_order() {
    //     let rows: &[(&[u8], &[u8])] = &[(b"a", b"1"), (b"b", b"2"), (b"c", b"3")];
    //     let (_dir, storage) = fixture(rows);
    //     let (txn, btree) = open_read(&storage);
    //     let mut cursor = Cursor::new();
    //     cursor.open(btree);
    //     assert!(cursor.scan_rev(&txn, None).unwrap());
    //     let collected = drain(&mut cursor);
    //     let keys: Vec<&[u8]> = collected.iter().map(|(k, _)| k.as_slice()).collect();
    //     assert_eq!(keys, vec![b"c".as_slice(), b"b".as_slice(), b"a".as_slice()]);
    // }
    //
    // #[test]
    // fn scan_rev_with_prefix() {
    //     let rows: &[(&[u8], &[u8])] = &[(b"aa", b"1"), (b"ab", b"2"), (b"ba", b"3")];
    //     let (_dir, storage) = fixture(rows);
    //     let (txn, btree) = open_read(&storage);
    //     let mut cursor = Cursor::new();
    //     cursor.open(btree);
    //     assert!(cursor.scan_rev(&txn, Some(b"a")).unwrap());
    //     let collected = drain(&mut cursor);
    //     let keys: Vec<&[u8]> = collected.iter().map(|(k, _)| k.as_slice()).collect();
    //     assert_eq!(keys, vec![b"ab".as_slice(), b"aa".as_slice()]);
    // }
    //
    // #[test]
    // fn scan_rev_on_empty_btree() {
    //     let (_dir, storage) = fixture(&[]);
    //     let (txn, btree) = open_read(&storage);
    //     let mut cursor = Cursor::new();
    //     cursor.open(btree);
    //     assert!(!cursor.scan_rev(&txn, None).unwrap());
    // }
}
