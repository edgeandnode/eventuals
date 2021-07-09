use crate::error::Closed;
use std::{
    collections::HashSet,
    ops::Deref,
    sync::{Arc, Mutex},
};

use super::change::Change;

pub struct SharedState<T> {
    // This takes a cue from the .NET event implementation
    // which bets that:
    //    Typically the number of subscribers is relatively small
    //    Modifying the subscriber list is less frequent than posting events
    // These taken together advocate for a snapshot-at-time style of concurrency
    // which makes snapshotting very cheap but updates expensive.
    pub subscribers: Mutex<Arc<HashSet<Change<T>>>>,
    pub last_write: Mutex<Option<Result<T, Closed>>>,
}

impl<T> SharedState<T>
where
    T: Clone + Eq,
{
    pub fn new() -> Self {
        Self {
            subscribers: Mutex::new(Arc::new(HashSet::new())),
            last_write: Mutex::new(None),
        }
    }

    pub fn notify_all(&self) {
        let snapshot = {
            let lock = self.subscribers.lock().unwrap();
            lock.deref().clone()
        };
        for subscriber in snapshot.iter() {
            self.notify_one(subscriber)
        }
    }

    pub fn notify_one(&self, subscriber: &Change<T>) {
        subscriber.set_value(&self.last_write);
    }
}
