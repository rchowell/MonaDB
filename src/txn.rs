use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, ThreadId};
use std::time::{Duration, Instant};

use pyo3::exceptions::PyKeyError;
use pyo3::prelude::*;

use crate::collection::Collection;
use crate::db::{DbInner, check_name};
use crate::error::{self, TransactionError};

/// Why the gate could not be acquired.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateError {
    /// Another thread held the gate past the deadline.
    Busy,
    /// The calling thread already holds the gate.
    Reentrant,
}

/// The write gate.
pub struct Gate {
    owner: Mutex<Option<ThreadId>>,
    cv: Condvar,
}

impl Default for Gate {
    fn default() -> Self {
        Self::new()
    }
}

impl Gate {
    pub fn new() -> Self {
        Gate {
            owner: Mutex::new(None),
            cv: Condvar::new(),
        }
    }

    /// Acquires the gate, waiting up to `timeout`.
    ///
    /// Callers must wrap this in `Python::detach` — waiting while holding the
    /// GIL would stall every other Python thread, including the one that would
    /// release the gate.
    pub fn acquire(&self, timeout: Duration) -> Result<(), GateError> {
        let me = thread::current().id();
        let deadline = Instant::now() + timeout;
        let mut owner = self.owner.lock().expect("gate poisoned");
        loop {
            match *owner {
                None => {
                    *owner = Some(me);
                    return Ok(());
                }
                // Waiting here could never succeed: this thread is the one that
                // would have to release.
                Some(t) if t == me => return Err(GateError::Reentrant),
                Some(_) => {
                    let now = Instant::now();
                    if now >= deadline {
                        return Err(GateError::Busy);
                    }
                    let (guard, _timed_out) = self
                        .cv
                        .wait_timeout(owner, deadline - now)
                        .expect("gate poisoned");
                    owner = guard;
                }
            }
        }
    }

    /// Releases the gate and wakes one waiter.
    pub fn release(&self) {
        *self.owner.lock().expect("gate poisoned") = None;
        self.cv.notify_one();
    }

    /// Returns whether the calling thread currently holds the gate.
    pub fn held_by_current_thread(&self) -> bool {
        *self.owner.lock().expect("gate poisoned") == Some(thread::current().id())
    }

    /// RAII acquire: the guard releases on drop, covering every error path.
    pub fn acquire_guard(&self, timeout: Duration) -> Result<GateGuard<'_>, GateError> {
        self.acquire(timeout)?;
        Ok(GateGuard(self))
    }
}

/// Releases the [`Gate`] on drop.
pub struct GateGuard<'a>(&'a Gate);

impl GateGuard<'_> {
    /// Holds the gate past this guard's scope.
    ///
    /// Used by explicit transactions, where release happens at commit, abort,
    /// or close rather than at the end of a call.
    pub fn keep(self) {
        // The guard borrows the gate and owns nothing, so skipping its drop
        // leaks no memory — it only skips the release.
        std::mem::forget(self);
    }
}

impl Drop for GateGuard<'_> {
    fn drop(&mut self) {
        self.0.release();
    }
}

/// An open explicit write transaction.
///
/// The underlying `WriteTransaction` lives in `DbInner::active`; commit and
/// abort take it out and release the gate. A write transaction therefore cannot
/// outlive the `with` block that created it — which is what makes the whole
/// class of retained-transaction deadlocks unrepresentable.
#[pyclass]
pub struct Txn {
    pub inner: Arc<DbInner>,
}

impl Txn {
    /// Takes the active transaction, or raises if it is already finished.
    fn take(&self) -> PyResult<redb::WriteTransaction> {
        self.inner
            .active
            .lock()
            .expect("txn poisoned")
            .take()
            .ok_or_else(|| TransactionError::new_err("transaction is not open"))
    }

    fn check_open(&self) -> PyResult<()> {
        if self.inner.active.lock().expect("txn poisoned").is_some() {
            Ok(())
        } else {
            Err(TransactionError::new_err("transaction is not open"))
        }
    }
}

#[pymethods]
impl Txn {
    fn commit(&self) -> PyResult<()> {
        let txn = self.take()?;
        let result = txn.commit().map_err(error::storage);
        self.inner.gate.release();
        result
    }

    fn abort(&self) -> PyResult<()> {
        let txn = self.take()?;
        let result = txn.abort().map_err(error::storage);
        self.inner.gate.release();
        result
    }

    fn names(&self) -> PyResult<Vec<String>> {
        self.check_open()?;
        self.inner.names()
    }

    fn has(&self, name: &str) -> PyResult<bool> {
        Ok(self.names()?.iter().any(|n| n == name))
    }

    /// Drops a collection inside this transaction; `KeyError` if absent.
    #[pyo3(name = "drop")]
    fn drop_(&self, name: &str) -> PyResult<()> {
        check_name(name)?;
        let active = self.inner.active.lock().expect("txn poisoned");
        let txn = active
            .as_ref()
            .ok_or_else(|| TransactionError::new_err("transaction is not open"))?;
        let def = redb::TableDefinition::<&[u8], &[u8]>::new(name);
        if txn.delete_table(def).map_err(error::storage)? {
            Ok(())
        } else {
            Err(PyKeyError::new_err(name.to_string()))
        }
    }

    fn collection(&self, name: String) -> PyResult<Collection> {
        check_name(&name)?;
        self.check_open()?;
        Ok(Collection::new(Arc::clone(&self.inner), name, true))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn reentrant_acquire_fails_immediately() {
        let g = Gate::new();
        g.acquire(Duration::from_secs(1)).unwrap();
        let t0 = Instant::now();
        assert!(matches!(
            g.acquire(Duration::from_secs(5)),
            Err(GateError::Reentrant)
        ));
        assert!(t0.elapsed() < Duration::from_millis(100), "must not wait");
        g.release();
    }

    #[test]
    fn contention_times_out_busy() {
        let g = Arc::new(Gate::new());
        g.acquire(Duration::from_secs(1)).unwrap();
        let g2 = Arc::clone(&g);
        let handle = std::thread::spawn(move || {
            let t0 = Instant::now();
            let r = g2.acquire(Duration::from_millis(100));
            (r, t0.elapsed())
        });
        let (r, elapsed) = handle.join().unwrap();
        assert!(matches!(r, Err(GateError::Busy)));
        assert!(elapsed >= Duration::from_millis(90));
        g.release();
    }

    #[test]
    fn release_wakes_waiter() {
        let g = Arc::new(Gate::new());
        g.acquire(Duration::from_secs(1)).unwrap();
        let g2 = Arc::clone(&g);
        let handle = std::thread::spawn(move || g2.acquire(Duration::from_secs(5)).is_ok());
        std::thread::sleep(Duration::from_millis(50));
        g.release();
        assert!(handle.join().unwrap());
    }

    #[test]
    fn guard_releases_on_drop() {
        let g = Gate::new();
        {
            let _guard = g.acquire_guard(Duration::from_secs(1)).unwrap();
        }
        g.acquire(Duration::from_secs(1)).unwrap(); // free again
        g.release();
    }

    #[test]
    fn kept_guard_does_not_release() {
        let g = Gate::new();
        g.acquire_guard(Duration::from_secs(1)).unwrap().keep();
        assert!(g.held_by_current_thread());
        assert!(matches!(
            g.acquire(Duration::from_millis(10)),
            Err(GateError::Reentrant)
        ));
        g.release();
    }
}
