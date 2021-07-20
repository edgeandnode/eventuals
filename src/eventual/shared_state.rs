use futures::channel::oneshot::Sender;
use std::{
    collections::HashSet,
    ops::Deref,
    sync::{Arc, Mutex},
};

use super::{
    change::{Change, ChangeReader, ChangeVal},
    *,
};

pub struct SharedState<T> {
    // This takes a cue from the .NET event implementation
    // which bets that:
    //    Typically the number of subscribers is relatively small
    //    Modifying the subscriber list is less frequent than posting events
    // These taken together advocate for a snapshot-at-time style of concurrency
    // which makes snapshotting very cheap but updates expensive.
    pub subscribers: Mutex<Arc<HashSet<Change<T>>>>,
    pub last_write: Mutex<ChangeVal<T>>,
    writer_notify: Option<Sender<()>>,
}

impl<T> Drop for SharedState<T> {
    fn drop(&mut self) {
        if let Some(notify) = self.writer_notify.take() {
            // Ignore the possible error because that means
            // the other side is dropped. The point of notify
            // is to drop.
            let _ignore = notify.send(());
        }
    }
}

impl<T> SharedState<T>
where
    T: Value,
{
    pub fn new(writer_notify: Sender<()>) -> Self {
        Self {
            subscribers: Mutex::new(Arc::new(HashSet::new())),
            last_write: Mutex::new(ChangeVal::None),
            writer_notify: Some(writer_notify),
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

    pub fn subscribe(self: Arc<Self>) -> ChangeReader<T> {
        let change: Change<T> = Change::new();
        {
            let mut lock = self.subscribers.lock().unwrap();
            let mut updated: HashSet<_> = lock.deref().deref().clone();
            updated.insert(change.clone());
            *lock = Arc::new(updated);
        }
        // Must notify AFTER it's in the subscriber list to avoid missing updates.
        self.notify_one(&change);
        ChangeReader {
            change,
            unsubscribe_from: self,
        }
    }
}
