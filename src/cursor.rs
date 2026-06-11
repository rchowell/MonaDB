use heed::types::Bytes;
use heed::{RoIter, RoPrefix, RoRevIter, RoRevPrefix};

use crate::error::{Error, Result};
use crate::value::Value;
use crate::{storage::BTree, transaction::Transaction};

/// The error returned when a row is requested from a cursor that is not
/// positioned on one (no scan begun, or iteration exhausted).
fn unpositioned_cursor() -> Error {
    Error::InternalError("load on unpositioned cursor".to_string())
}

/// Cursor controls traversal of a source e.g. btree or value.
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

    /// Opens this cursor on the given btree (a table source, not yet scanned).
    pub fn open(&mut self, btree: BTree) {
        self.source = Some(Source::Btree { btree, scan: None });
    }

    /// Begins a forward scan; returns true if there's an available row.
    #[allow(unused)]
    pub fn scan(&mut self, txn: &Transaction, prefix: Option<&[u8]>) -> Result<bool> {
        let Some(Source::Btree { btree, scan }) = &mut self.source else {
            panic!("cursor must be open");
        };
        let btree = *btree;
        // SAFETY: ...
        let rtxn = txn.as_ro();
        let iter = btree.iter(rtxn)?;
        let iter: RoIter<'static, Bytes, Bytes> = unsafe { std::mem::transmute(iter) };
        let mut table = TableScan::new(TableIter::Fwd(iter));
        let available = table.next()?;
        *scan = Some(table);
        // Return true if there's an available row
        Ok(available)
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
            Some(Source::Btree { scan: Some(scan), .. }) => scan.next(),
            Some(Source::Value(value)) => Ok(value.next()),
            // Unreachable unless there's a compiler bug.
            _ => Err(Error::InternalError(
                "next called on a cursor with no scan state".to_string(),
            )),
        }
    }

    /// Returns the current (key,val) bytes; table-backed scans only.
    pub fn current(&self) -> Option<(&[u8], &[u8])> {
        match &self.source {
            Some(Source::Btree { scan: Some(scan), .. }) => scan.current(),
            // Value-backed (or unpositioned) cursors have no raw key/val bytes.
            _ => None,
        }
    }

    /// Returns the current row as a decoded Value.
    pub fn load(&self) -> Result<Value> {
        match &self.source {
            Some(Source::Btree { scan: Some(scan), .. }) => scan.load(),
            Some(Source::Value(value)) => value.load(),
            _ => Err(unpositioned_cursor()),
        }
    }

    /// The open btree handle; panics if the cursor is unopened or value-backed.
    /// Private — the handle never leaves the cursor.
    fn btree(&self) -> BTree {
        let Some(Source::Btree { btree, .. }) = &self.source else {
            panic!("cursor must be open on a btree");
        };
        *btree
    }

    /// Inserts the value at the given key
    pub fn insert(&self, txn: &mut Transaction, key: &[u8], val: &[u8]) -> Result<()> {
        self.btree().put(txn.as_rw()?, key, val)?;
        Ok(())
    }

    /// Applies a staged mutation log in two passes — every delete, then every
    /// put — and returns the number of rows affected (puts for an update,
    /// deletes for a delete). We first drop any live scan iterator so we never
    /// mutate the btree under a live read cursor (LMDB leaves a read cursor's
    /// position undefined across a mutation on the same table).
    ///
    /// Deletes precede puts so a key-swap among updated rows does not
    /// self-destruct (deleting a row's new key right after writing it). Once the
    /// deletes have landed, a put whose key still exists is landing on a row this
    /// statement never removed — a key collision — and is rejected rather than
    /// silently overwriting it.
    pub fn apply(&mut self, txn: &mut Transaction, log: &[Mutation]) -> Result<u64> {
        // Drops any live scan iterator, leaving the cursor open but unpositioned.
        if let Some(Source::Btree { scan, .. }) = &mut self.source {
            *scan = None;
        }
        let btree = self.btree();
        let mut dels = 0u64;
        let mut puts = 0u64;
        for m in log {
            if let Mutation::Del(key) = m {
                btree.delete(txn.as_rw()?, key)?;
                dels += 1;
            }
        }
        for m in log {
            if let Mutation::Put(key, val) = m {
                if btree.get(txn.as_ro(), key)?.is_some() {
                    return Err(Error::Schema(
                        "update collides with an existing row at the same key".to_string(),
                    ));
                }
                btree.put(txn.as_rw()?, key, val)?;
                puts += 1;
            }
        }
        Ok(puts.max(dels))
    }

    pub fn last(&self, txn: &Transaction) -> Result<Option<(Vec<u8>, Vec<u8>)>> {
        let res = self.btree().last(txn.as_ro())?;
        let res = res.map(|(k, v)| (k.to_vec(), v.to_vec()));
        Ok(res)
    }
}

/// A staged mutation against a cursor's table, drained by [`Cursor::apply`].
/// Keys and values are already encoded bytes.
pub enum Mutation {
    /// Remove the row at this key.
    Del(Vec<u8>),
    /// Write this value at this key.
    Put(Vec<u8>, Vec<u8>),
}

/// A cursor's bound source: a persistent btree table or an in-memory value.
enum Source {
    /// The btree handle persists across the open → (scan | insert/last)
    /// lifecycle; `scan` becomes `Some` only once a forward scan has begun.
    Btree { 
        btree: BTree,
        scan: Option<TableScan>,
    },
    /// Element-wise iteration over an in-memory array value.
    Value(ValueIter),
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

    /// Advance to the next element; true if one is available. A non-array
    /// source (or an out-of-range index) yields nothing. This only inspects
    /// the array length, so it never clones an element — `load` does that.
    fn next(&mut self) -> bool {
        let i = self.pos.map_or(0, |p| p + 1);
        self.pos = Some(i);
        self.source.len().is_some_and(|len| i < len)
    }

    /// Return the current element.
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

    fn next(&mut self) -> Result<bool> {
        self.curr = self.iter.next()?.map(|(k, v)| (k.to_vec(), v.to_vec()));
        Ok(self.curr.is_some())
    }

    fn load(&self) -> Result<Value> {
        let (_, val) = self.current().ok_or_else(unpositioned_cursor)?;
        Value::decode(val)
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
    use crate::storage::Storage;
    use tempfile::TempDir;

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
    #[ignore = "prefix arg currently unused in scan"]
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
    #[ignore = "prefix arg currently unused in scan"]
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
    #[ignore = "prefix arg currently unused in scan"]
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
