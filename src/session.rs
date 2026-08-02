//! Connection state shared between the handle, the binder, and a running VM.
//!
//! One [`ConnState`] per connection, held behind an `Rc` so a lazy [`crate::Rows`]
//! can outlive the `&mut MonaDB` that produced it. It owns the explicit-session
//! transaction and the catalog generation counters — everything a statement may
//! read or mutate while it runs.
//!
//! The session is a three-state machine, so "open" and "who holds the txn" are
//! one fact rather than two fields that must agree:
//!
//!   Closed ──begin──▶ Open(txn) ──Vop::Transaction──▶ Lent
//!      ▲                  ▲                             │
//!      └─commit/rollback──┘         Drop for VM ────────┘

use std::cell::{Cell, RefCell};

use crate::ast::TableDefinition;
use crate::catalog::Catalog;
use crate::error::{Error, Result};
use crate::storage::Storage;
use crate::transaction::Transaction;

/// The message for a statement issued while a prior result still holds the txn.
const IN_PROGRESS: &str =
    "a previous statement is still in progress; consume or drop its result before continuing";

/// How an explicit transaction ends.
#[derive(Debug, Clone, Copy)]
pub enum End {
    /// Make the session's writes durable.
    Commit,
    /// Discard them.
    Rollback,
}

/// The explicit-session transaction and who currently holds it.
enum Session {
    /// No explicit transaction (autocommit).
    Closed,
    /// An explicit transaction, idle between statements.
    Open(Transaction),
    /// An explicit transaction, lent to a running statement's VM. Returned by
    /// `Drop for VM`, so this state is transient but observable: a caller holding
    /// an unconsumed result keeps the session here.
    Lent,
}

/// Per-connection state a running statement may read or mutate.
pub struct ConnState {
    /// The explicit-session transaction, if any.
    session: RefCell<Session>,
    /// Table metadata and its generation-gated cache.
    catalog: Catalog,
    /// Bumped by any committed CREATE/DROP; prepared plans capture it to detect
    /// staleness.
    version: Cell<u64>,
    /// Set when an in-session statement mutates the catalog. `commit` consumes it
    /// to bump [`Self::version`] exactly once; `rollback` clears it without
    /// bumping, so a rolled-back DDL leaves earlier prepared statements valid.
    dirty: Cell<bool>,
}

impl ConnState {
    /// Builds the state for a freshly opened connection.
    pub fn new(catalog: Catalog) -> Self {
        Self {
            session: RefCell::new(Session::Closed),
            catalog,
            version: Cell::new(0),
            dirty: Cell::new(false),
        }
    }

    /// Returns the connection's catalog.
    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    /// Returns the current catalog generation.
    pub fn version(&self) -> u64 {
        self.version.get()
    }

    /// Advances the catalog generation, invalidating prepared plans.
    pub fn bump_version(&self) {
        self.version.set(self.version.get() + 1);
    }

    /// Records that an in-session statement changed catalog membership.
    pub fn mark_dirty(&self) {
        self.dirty.set(true);
    }

    /// Returns whether an explicit transaction is open, whether or not a
    /// statement currently holds it.
    pub fn in_transaction(&self) -> bool {
        !matches!(&*self.session.borrow(), Session::Closed)
    }

    /// Errors if a running statement still holds the session transaction.
    ///
    /// Checked wherever the session txn is about to be used or a second txn
    /// opened — otherwise the caller would mis-bind against a fresh transaction
    /// or block forever on LMDB's writer lock.
    pub fn guard_idle(&self) -> Result<()> {
        if matches!(&*self.session.borrow(), Session::Lent) {
            return Err(Error::Transaction(IN_PROGRESS.into()));
        }
        Ok(())
    }

    /// Borrows the session transaction for a statement, moving it to `Lent`.
    ///
    /// The three outcomes must stay distinct — collapsing the last two into
    /// `None` would let a second statement quietly open its *own* transaction
    /// while a sibling result holds the session's:
    ///
    /// - `Ok(Some(txn))` — an explicit session handed its transaction over.
    /// - `Ok(None)` — no explicit session; the caller opens its own.
    /// - `Err(..)` — a session is open but another statement still holds it.
    ///
    /// [`Self::guard_idle`] rejects that last case earlier, at compile and
    /// execute time; this catches the window it cannot see, because a lazy
    /// [`crate::Rows`] runs `Vop::Transaction` long after `execute_plan`
    /// returned.
    pub fn lend(&self) -> Result<Option<Transaction>> {
        let mut slot = self.session.borrow_mut();
        match std::mem::replace(&mut *slot, Session::Lent) {
            Session::Open(txn) => Ok(Some(txn)),
            Session::Closed => {
                *slot = Session::Closed;
                Ok(None)
            }
            Session::Lent => {
                *slot = Session::Lent;
                Err(Error::Transaction(IN_PROGRESS.into()))
            }
        }
    }

    /// Returns a lent transaction to the session. Called by `Drop for VM` on
    /// every exit path — completion, error, or an abandoned result.
    pub fn restore(&self, txn: Transaction) {
        *self.session.borrow_mut() = Session::Open(txn);
    }

    /// Runs a closure against the session transaction, if one is idle in the slot.
    ///
    /// Used by the binder and by prepare-time handle resolution so an in-session
    /// CREATE is visible to a later statement in the same session.
    pub fn with_txn<T>(&self, f: impl FnOnce(&Transaction) -> T) -> Option<T> {
        match &*self.session.borrow() {
            Session::Open(txn) => Some(f(txn)),
            _ => None,
        }
    }

    /// Resolves a table definition through `txn`, applying the session's caching
    /// rule. See [`Catalog::resolve`].
    pub fn resolve_table(&self, txn: &Transaction, name: &str) -> Result<TableDefinition> {
        self.catalog
            .resolve(txn, name, self.version(), self.in_transaction())
    }

    /// Opens an explicit transaction (`begin`).
    pub fn begin(&self, storage: &Storage) -> Result<()> {
        let mut slot = self.session.borrow_mut();
        match &*slot {
            Session::Closed => {}
            Session::Lent => return Err(Error::Transaction(IN_PROGRESS.into())),
            Session::Open(_) => {
                return Err(Error::Transaction("transaction already active".into()));
            }
        }
        *slot = Session::Open(Transaction::write(storage)?);
        Ok(())
    }

    /// Resolves the explicit transaction (`commit` / `rollback`).
    ///
    /// A commit publishes any in-session DDL by bumping the generation exactly
    /// once. A rollback does not bump — earlier prepared statements stay valid —
    /// but flushes the catalog cache, since entries learned through the aborted
    /// transaction must not linger.
    pub fn end(&self, how: End) -> Result<()> {
        let mut slot = self.session.borrow_mut();
        let txn = match std::mem::replace(&mut *slot, Session::Closed) {
            Session::Open(txn) => txn,
            prev => {
                let msg = match &prev {
                    Session::Lent => IN_PROGRESS,
                    _ => "no active transaction",
                };
                *slot = prev;
                return Err(Error::Transaction(msg.into()));
            }
        };
        drop(slot);

        match how {
            End::Commit => {
                txn.commit()?;
                if self.dirty.replace(false) {
                    self.bump_version();
                }
            }
            End::Rollback => {
                txn.abort();
                self.dirty.set(false);
                self.catalog.flush();
            }
        }
        Ok(())
    }

    /// Discards any open session without erroring; used when the connection is
    /// dropped so uncommitted writes are never flushed.
    ///
    /// A rollback *is* the discard, and [`Self::end`] already reports `Closed`
    /// and `Lent` as errors rather than panicking — exactly the no-op this wants.
    pub fn abort_if_open(&self) {
        let _ = self.end(End::Rollback);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::BTree;
    use tempfile::TempDir;

    fn open() -> (TempDir, Storage, ConnState) {
        let dir = TempDir::new().unwrap();
        let storage = Storage::open(dir.path().join("test.db")).unwrap();
        let catalog = Catalog::load(&storage).unwrap();
        let state = ConnState::new(catalog);
        (dir, storage, state)
    }

    fn make_btree(storage: &Storage, oid: u32) -> BTree {
        let mut txn = Transaction::write(storage).unwrap();
        let btree = storage.create_btree(&mut txn, oid).unwrap();
        txn.commit().unwrap();
        btree
    }

    #[test]
    fn closed_is_not_in_transaction() {
        let (_d, _s, state) = open();
        assert!(!state.in_transaction());
        assert!(state.guard_idle().is_ok());
        assert!(
            matches!(state.lend(), Ok(None)),
            "no session means the statement opens its own txn"
        );
    }

    #[test]
    fn begin_opens_and_double_begin_errors() {
        let (_d, storage, state) = open();
        state.begin(&storage).unwrap();
        assert!(state.in_transaction());
        assert!(matches!(
            state.begin(&storage),
            Err(Error::Transaction(_))
        ));
    }

    #[test]
    fn end_without_begin_errors() {
        let (_d, _s, state) = open();
        assert!(matches!(state.end(End::Commit), Err(Error::Transaction(_))));
        assert!(matches!(
            state.end(End::Rollback),
            Err(Error::Transaction(_))
        ));
    }

    #[test]
    fn lend_then_restore_round_trips() {
        let (_d, storage, state) = open();
        state.begin(&storage).unwrap();

        let txn = state.lend().unwrap().expect("session txn is lendable");
        // While lent the session is still active, but nothing may use it.
        assert!(state.in_transaction());
        assert!(state.guard_idle().is_err());
        // Critically an *error*, not `Ok(None)`: `Ok(None)` would tell the VM
        // "no session, open your own", silently giving a second statement its
        // own transaction while this one still holds the session's.
        assert!(
            matches!(state.lend(), Err(Error::Transaction(_))),
            "a lent txn cannot be lent twice, and must not read as `no session`"
        );
        assert!(state.end(End::Commit).is_err());
        assert!(state.with_txn(|_| ()).is_none());

        state.restore(txn);
        assert!(state.guard_idle().is_ok());
        state.end(End::Commit).unwrap();
        assert!(!state.in_transaction());
    }

    #[test]
    fn rollback_discards_writes_and_commit_keeps_them() {
        let (_d, storage, state) = open();
        let btree = make_btree(&storage, 1);

        state.begin(&storage).unwrap();
        let mut txn = state.lend().unwrap().expect("session txn is lendable");
        btree.put(txn.as_rw().unwrap(), b"k", b"v").unwrap();
        state.restore(txn);
        state.end(End::Rollback).unwrap();

        let ro = Transaction::read(&storage).unwrap();
        assert_eq!(btree.get(ro.as_ro(), b"k").unwrap(), None);
        drop(ro);

        state.begin(&storage).unwrap();
        let mut txn = state.lend().unwrap().expect("session txn is lendable");
        btree.put(txn.as_rw().unwrap(), b"k", b"v").unwrap();
        state.restore(txn);
        state.end(End::Commit).unwrap();

        let ro = Transaction::read(&storage).unwrap();
        assert_eq!(btree.get(ro.as_ro(), b"k").unwrap(), Some(b"v".as_slice()));
    }

    #[test]
    fn commit_bumps_version_only_when_dirty() {
        let (_d, storage, state) = open();

        state.begin(&storage).unwrap();
        state.end(End::Commit).unwrap();
        assert_eq!(state.version(), 0, "a clean session must not bump");

        state.begin(&storage).unwrap();
        state.mark_dirty();
        state.end(End::Commit).unwrap();
        assert_eq!(state.version(), 1);
    }

    #[test]
    fn rollback_never_bumps_version() {
        let (_d, storage, state) = open();
        state.begin(&storage).unwrap();
        state.mark_dirty();
        state.end(End::Rollback).unwrap();
        assert_eq!(
            state.version(),
            0,
            "a rolled-back DDL must not invalidate prepared statements"
        );
    }

    #[test]
    fn abort_if_open_discards_and_is_idempotent() {
        let (_d, storage, state) = open();
        let btree = make_btree(&storage, 1);

        state.begin(&storage).unwrap();
        let mut txn = state.lend().unwrap().expect("session txn is lendable");
        btree.put(txn.as_rw().unwrap(), b"k", b"v").unwrap();
        state.restore(txn);

        state.abort_if_open();
        assert!(!state.in_transaction());
        state.abort_if_open(); // no-op the second time

        let ro = Transaction::read(&storage).unwrap();
        assert_eq!(btree.get(ro.as_ro(), b"k").unwrap(), None);
    }
}
