use super::*;
use crate::{error::Closed, IntoReader};
use std::collections::HashSet;

pub struct EventualReader<T> {
    change: Change<T>,
    prev: Option<Result<T, Closed>>,
    unsubscribe_from: Arc<SharedState<T>>,
}

impl<T> IntoReader for EventualReader<T> {
    type Output = T;
    fn into_reader(self) -> EventualReader<Self::Output> {
        self
    }
}

pub struct Next<'a, T> {
    eventual: &'a mut EventualReader<T>,
}

impl<'a, T> Future for Next<'a, T>
where
    T: Value,
{
    type Output = Result<T, Closed>;
    // TODO: This is currently checking for a pushed value, but that will require
    // eg: map() to run in separate tasks. It might be desireble to have this poll
    // the future that would produce values. But... that may be very complex. A
    // refactor may be necessary.
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut swap = None;
        self.eventual
            .change
            .swap_or_wake(&mut swap, &self.eventual.prev, cx);
        match swap {
            None => Poll::Pending,
            Some(value) => {
                self.eventual.prev = Some(value.clone());
                Poll::Ready(value)
            }
        }
    }
}

// Remove from subscription on Drop
impl<T> Drop for EventualReader<T> {
    fn drop(&mut self) {
        let mut lock = self.unsubscribe_from.subscribers.lock().unwrap();
        let mut updated: HashSet<_> = lock.deref().deref().clone();
        if updated.remove(&self.change) {
            *lock = Arc::new(updated);
        }
    }
}

impl<T> EventualReader<T>
where
    T: Value,
{
    pub fn next(&mut self) -> Next<T> {
        Next { eventual: self }
    }

    pub(crate) fn new(state: Arc<SharedState<T>>) -> Self {
        let change: Change<T> = Change::new();
        {
            // TODO: This can use optimistic concurrency with a CAS loop.
            let mut lock = state.subscribers.lock().unwrap();
            let mut updated: HashSet<_> = lock.deref().deref().clone();
            updated.insert(change.clone());
            *lock = Arc::new(updated);
        }
        // Must notify AFTER it's in the subscriber list to avoid missing updates.
        state.notify_one(&change);

        EventualReader {
            change,
            prev: None,
            unsubscribe_from: state,
        }
    }
}
