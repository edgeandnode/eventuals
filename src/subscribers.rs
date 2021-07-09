use super::Change;
use std::{
    collections::HashSet,
    ops::Deref,
    sync::{Arc, Mutex},
};

// This takes a cue from the .NET event implementation
// which bets that:
//    Typically the number of subscribers is relatively small
//    Modifying the subscriber list is less frequent than posting events
// These taken together advocate for a snapshot-at-time style of concurrency
// which makes snapshotting very cheap but updates expensive.
pub(crate) struct Subscribers<T> {
    inner: Arc<Mutex<Arc<HashSet<Change<T>>>>>,
}

impl<T> Subscribers<T> {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Arc::new(HashSet::new()))),
        }
    }

    // TODO: Would prefer to make the type opaque using some sort of IntoIterator
    // but not sure about the syntax for `impl Deref<impl IntoIterator...`
    pub fn snapshot(&self) -> Arc<HashSet<Change<T>>> {
        let lock = self.inner.lock().unwrap();
        lock.deref().clone()
    }

    pub fn subscribe(&self, change: &Change<T>) {
        // TODO: This can use optimistic concurrency with a CAS loop.
        let mut lock = self.inner.lock().unwrap();
        let mut updated: HashSet<_> = lock.deref().deref().clone();
        updated.insert(change.clone());
        *lock = Arc::new(updated);
    }

    pub fn unsubscribe(&self, change: &Change<T>) {
        let mut lock = self.inner.lock().unwrap();
        let mut updated: HashSet<_> = lock.deref().deref().clone();
        updated.remove(change);
        *lock = Arc::new(updated);
    }
}

impl<T> Clone for Subscribers<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}
