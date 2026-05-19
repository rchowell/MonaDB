use heed::{RoIter, RoPrefix, RoRevIter, RoRevPrefix};
use heed::types::Bytes;

use crate::error::{Error, Result};
use crate::{storage::BTree, transaction::Transaction};

/// Cursor controls btree traversal.
pub struct Cursor {
    /// Inner btree handle for creating scan states.
    btree: BTree,
    /// Inner scan state upon which we call next.
    scan: Option<Scan>,
    /// Current (key,val) owned bytes
    curr: Option<(Vec<u8>, Vec<u8>)>,
}

impl Cursor {
    /// Creates a new cursor for the given btree with empty scan state.
    pub fn new(btree: BTree) -> Self {
        Self {
            btree,
            scan: None,
            curr: None,
        }
    }

    /// Begins a forward scan; returns true if there's an available row.
    pub fn scan(&mut self, txn: &Transaction, prefix: Option<&[u8]>) -> Result<bool> {
        // SAFETY: ...
        let rtxn = txn.as_ro();
        let iter = self.btree.iter(rtxn)?;
        let iter: RoIter<'static, Bytes, Bytes> = unsafe { std::mem::transmute(iter) };
        let mut scan: Scan = Scan::Fwd(iter);
        // Position on the next available item
        self.curr = scan.next()?.map(|(k, v)| (k.to_vec(), v.to_vec()));
        self.scan = Some(scan);
        // Return true if there's an available row
        Ok(self.curr.is_some())
    }

    /// Advances the scan; returns true if there's an available row
    pub fn next(&mut self) -> Result<bool> {
        let scan = self.scan.as_mut().ok_or_else(|| {
            // Unreachable unless there's a compiler bug.
            Error::InternalError("next called on a cursor with no scan state".to_string())
        })?;
        self.curr = scan.next()?.map(|(k, v)| (k.to_vec(), v.to_vec()));
        Ok(self.curr.is_some())
    }

    /// Returns the current (key,val) bytes.
    pub fn current(&self) -> Option<(&[u8], &[u8])> {
        self.curr.as_ref().map(|(k,v)| (k.as_slice(), v.as_slice()))
    }

    /// Inserts the value at the given key
    pub fn insert(&self, txn: &mut Transaction, key: &[u8], val: &[u8]) -> Result<()> {
        let txn = txn.as_rw()?;
        self.btree.put(txn, key, val)?;
        Ok(())
    }

    pub fn last(&self, txn: &Transaction) -> Result<Option<(Vec<u8>, Vec<u8>)>> {
        let txn = txn.as_ro();
        let res = self.btree.last(txn)?;
        let res = res.map(|(k ,v)| (k.to_vec(), v.to_vec()));
        Ok(res)
    }
}


/// The inner scan state; dispatches on type.
enum Scan {
    /// Forward iteration.
    Fwd(RoIter<'static, Bytes, Bytes>),
    /// Forward iteration with prefix.
    FwdPre(RoPrefix<'static, Bytes, Bytes>),
    /// Reverse iteration.
    Rev(RoRevIter<'static, Bytes, Bytes>),
    /// Reverse iteration with prefix.
    RevPre(RoRevPrefix<'static, Bytes, Bytes>),
}

impl Scan {
    /// Returns the next (key,val) reference or none.
    fn next(&mut self) -> Result<Option<(&[u8], &[u8])>> {
        let res = match self {
            Scan::Fwd(it) => it.next(),
            Scan::FwdPre(it) => it.next(),
            Scan::Rev(it) => it.next(),
            Scan::RevPre(it) => it.next(),
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
        let mut cursor = Cursor::new(btree);
        assert!(!cursor.scan(&txn, None).unwrap());
        assert_eq!(cursor.current(), None);
    }

    #[test]
    fn scan_single_row_positions_on_it() {
        let (_dir, storage) = fixture(&[(b"a", b"1")]);
        let (txn, btree) = open_read(&storage);
        let mut cursor = Cursor::new(btree);
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
        let mut cursor = Cursor::new(btree);
        assert!(cursor.scan(&txn, None).unwrap());
        let collected = drain(&mut cursor);
        let keys: Vec<&[u8]> = collected.iter().map(|(k, _)| k.as_slice()).collect();
        assert_eq!(keys, vec![b"a".as_slice(), b"b".as_slice(), b"c".as_slice()]);
    }

    #[test]
    fn scan_yields_correct_values() {
        let rows: &[(&[u8], &[u8])] = &[(b"c", b"three"), (b"a", b"one"), (b"b", b"two")];
        let (_dir, storage) = fixture(rows);
        let (txn, btree) = open_read(&storage);
        let mut cursor = Cursor::new(btree);
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
        let mut cursor = Cursor::new(btree);
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
        let cursor = Cursor::new(btree);
        assert_eq!(cursor.current(), None);
    }

    #[test]
    fn next_without_scan_returns_error() {
        let (_dir, storage) = fixture(&[(b"a", b"1")]);
        let (_txn, btree) = open_read(&storage);
        let mut cursor = Cursor::new(btree);
        let err = cursor.next().unwrap_err();
        assert!(matches!(err, Error::InternalError(_)));
    }

    #[test]
    fn scan_restart_repositions_on_first_row() {
        let rows: &[(&[u8], &[u8])] = &[(b"a", b"1"), (b"b", b"2"), (b"c", b"3")];
        let (_dir, storage) = fixture(rows);
        let (txn, btree) = open_read(&storage);
        let mut cursor = Cursor::new(btree);

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

        let mut a = Cursor::new(btree);
        let mut b = Cursor::new(btree);

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
        let mut cursor = Cursor::new(btree);
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
        let mut cursor = Cursor::new(btree);
        assert!(cursor.scan(&txn, None).unwrap());
        let collected = drain(&mut cursor);
        assert_eq!(collected.len(), 2);
    }

    #[test]
    #[ignore = "prefix arg currently unused in scan"]
    fn scan_with_prefix_yields_only_matching_keys() {
        let rows: &[(&[u8], &[u8])] =
            &[(b"aa", b"1"), (b"ab", b"2"), (b"ba", b"3"), (b"bb", b"4")];
        let (_dir, storage) = fixture(rows);
        let (txn, btree) = open_read(&storage);
        let mut cursor = Cursor::new(btree);
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
        let mut cursor = Cursor::new(btree);
        assert!(cursor.scan(&txn, Some(b"")).unwrap());
        let collected = drain(&mut cursor);
        assert_eq!(collected.len(), 2);
    }

    #[test]
    #[ignore = "prefix arg currently unused in scan"]
    fn scan_with_prefix_on_empty_btree_returns_false() {
        let (_dir, storage) = fixture(&[]);
        let (txn, btree) = open_read(&storage);
        let mut cursor = Cursor::new(btree);
        assert!(!cursor.scan(&txn, Some(b"x")).unwrap());
        assert_eq!(cursor.current(), None);
    }

    #[test]
    fn insert_visible_via_scan_in_same_txn() {
        let (_dir, storage) = fixture(&[]);
        let mut txn = Transaction::write(&storage).unwrap();
        let btree = storage.open_btree(&txn, 1).unwrap();
        let mut cursor = Cursor::new(btree);
        cursor.insert(&mut txn, b"k", b"v").unwrap();
        assert!(cursor.scan(&txn, None).unwrap());
        assert_eq!(cursor.current(), Some((b"k".as_slice(), b"v".as_slice())));
    }

    #[test]
    fn insert_fails_on_read_only_txn() {
        let (_dir, storage) = fixture(&[]);
        let mut ro = Transaction::read(&storage).unwrap();
        let btree = storage.open_btree(&ro, 1).unwrap();
        let cursor = Cursor::new(btree);
        let err = cursor.insert(&mut ro, b"k", b"v").unwrap_err();
        assert!(matches!(err, Error::InternalError(_)));
    }

    #[test]
    fn last_returns_none_on_empty() {
        let (_dir, storage) = fixture(&[]);
        let (txn, btree) = open_read(&storage);
        let cursor = Cursor::new(btree);
        assert_eq!(cursor.last(&txn).unwrap(), None);
    }

    #[test]
    fn last_returns_owned_max_key() {
        let (_dir, storage) = fixture(&[(b"a", b"1"), (b"c", b"3"), (b"b", b"2")]);
        let (txn, btree) = open_read(&storage);
        let cursor = Cursor::new(btree);
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
            let mut cursor = Cursor::new(btree);
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
            let cursor = Cursor::new(btree);
            cursor.insert(&mut txn, b"k", b"v").unwrap();
        }
        txn.commit().unwrap();

        let (txn, btree) = open_read(&storage);
        let cursor = Cursor::new(btree);
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
    //     let mut cursor = Cursor::new(btree);
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
    //     let mut cursor = Cursor::new(btree);
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
    //     let mut cursor = Cursor::new(btree);
    //     assert!(!cursor.scan_rev(&txn, None).unwrap());
    // }
}
