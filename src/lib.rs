use by_address::ByAddress;
use std::{
    future::Future,
    hash::Hash,
    mem,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};

mod subscribers;
use subscribers::Subscribers;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Closed;

pub struct Next<'a, T> {
    eventual: &'a mut EventualRead<T>,
}

impl<'a, T> Future for Next<'a, T>
where
    T: Eq + Clone,
{
    type Output = Result<T, Closed>;
    // TODO: This is currently checking for a pushed value, but that will require
    // eg: map() to run in separate tasks. It might be desireble to have this poll
    // the future that would produce values. But... that may be very complex. A
    // refactor may be necessary.
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // TODO: I think it should be possible to go full lockfree
        let mut swap = None;
        {
            let mut lock = self.eventual.change.inner.lock().unwrap();
            if let ChangeVal::Value(value) = &mut *lock {
                if value != &self.eventual.prev {
                    mem::swap(value, &mut swap);
                }
            }
            if swap.is_none() {
                *lock = ChangeVal::Waker(cx.waker().clone())
            }
        }
        match swap {
            None => Poll::Pending,
            Some(value) => {
                self.eventual.prev = Some(value.clone());
                Poll::Ready(value)
            }
        }
    }
}

enum ChangeVal<T> {
    Value(Option<Result<T, Closed>>),
    Waker(Waker),
}

struct Change<T> {
    inner: ByAddress<Arc<Mutex<ChangeVal<T>>>>,
}

impl<T> Change<T>
where
    T: Clone,
{
    pub fn new() -> Self {
        Self {
            inner: ByAddress(Arc::new(Mutex::new(ChangeVal::Value(None)))),
        }
    }
    pub fn set_value(&self, value: &Mutex<Option<Result<T, Closed>>>) {
        let prev = {
            // To avoid race conditions BOTH locks MUST be held. This insures
            // that if new values are pushed while subscribers are being
            // notified there cannot be a time that a subscriber is notified
            // with the old value. Instead, it might be notified with the new
            // value twice. Notice that the former is apocalyptic (missed
            // updates) and the later just drains some performance for an extra
            // equality check on the receiving end.
            // TODO: The value can use optimistic concurrency as long as
            // the read/write is synchronized with SeqCst AND the write lock
            // here is held. The value must be read from WITHIN the lock.
            let value = value.lock().unwrap();
            let mut inner = self.inner.lock().unwrap();
            let mut update = ChangeVal::Value(value.as_ref().map(|v| v.clone()));
            mem::swap(&mut *inner, &mut update);
            update

            // Drop locks before calling wake()
        };

        // Race conditions here are OK. The worst that can happen
        // is that Tasks are woken up unnecessarily. They would
        // just return Pending the second time.
        if let ChangeVal::Waker(waker) = prev {
            waker.wake();
        }
    }
}

impl<T> Clone for Change<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T> Eq for Change<T> {}
impl<T> PartialEq for Change<T> {
    fn eq(&self, other: &Self) -> bool {
        self.inner.eq(&other.inner)
    }
}

impl<T> Hash for Change<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.inner.hash(state)
    }
}

pub struct EventualRead<T> {
    change: Change<T>,
    prev: Option<Result<T, Closed>>,
    unsubscribe_from: Subscribers<T>,
}

// Remove from subscription on Drop
impl<T> Drop for EventualRead<T> {
    fn drop(&mut self) {
        self.unsubscribe_from.unsubscribe(&self.change)
    }
}

impl<T> EventualRead<T> {
    pub fn next(&mut self) -> Next<T> {
        Next { eventual: self }
    }
}

pub struct EventualWrite<T>
where
    T: Clone,
{
    prev: Mutex<Option<Result<T, Closed>>>,
    subscribers: Subscribers<T>,
}

impl<T> Drop for EventualWrite<T>
where
    T: Clone,
{
    fn drop(&mut self) {
        // See also b045e23a-f445-456f-a686-7e80de621cf2
        {
            let prev = self.prev.get_mut().unwrap();
            *prev = Some(Err(Closed));
        }
        self.notify_all()
    }
}

impl<T> EventualWrite<T>
where
    T: Clone,
{
    pub fn new() -> Self {
        Self {
            prev: Mutex::new(None),
            subscribers: Subscribers::new(),
        }
    }
    fn notify_all(&self) {
        for subscriber in self.subscribers.snapshot().iter() {
            self.notify_one(subscriber)
        }
    }

    fn notify_one(&self, subscriber: &Change<T>) {
        subscriber.set_value(&self.prev);
    }

    pub fn subscribe(&self) -> EventualRead<T> {
        let change: Change<T> = Change::new();

        self.subscribers.subscribe(&change);
        // Must notify AFTER it's in the subscriber list to avoid missing updates.
        self.notify_one(&change);

        EventualRead {
            change,
            prev: None,
            unsubscribe_from: self.subscribers.clone(),
        }
    }

    // TODO: From an API/permissions standpoint it would be nice to separate the subscribe
    // and the write. This would have the added benefit of allowing for write to be
    // able to take `&mut self`.
    pub fn write(&self, value: T) {
        // See also b045e23a-f445-456f-a686-7e80de621cf2
        {
            let mut prev = self.prev.lock().unwrap();
            *prev = Some(Ok(value));
        }
        self.notify_all()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::{join, test};

    // TODO: Much more sophisticated tests are needed.
    #[test]
    async fn it_works() {
        let eventual = EventualWrite::<u32>::new();
        let mut read_0 = eventual.subscribe();
        let mut read_1 = eventual.subscribe();
        let mut read_2 = eventual.subscribe();

        eventual.write(5);
        assert_eq!(read_0.next().await.unwrap(), 5);

        let r0 =
            tokio::spawn(
                async move { read_0.next().await.unwrap() + read_0.next().await.unwrap() },
            );

        let r1 = tokio::spawn(async move {
            eventual.write(10);
            tokio::time::sleep(Duration::from_millis(10)).await;
            let next = read_1.next();
            eventual.write(8);
            tokio::time::sleep(Duration::from_millis(10)).await;
            next.await.unwrap()
        });

        let (r0, r1) = join!(r0, r1);
        assert_eq!(r0.unwrap(), 18);
        assert_eq!(r1.unwrap(), 8);
        assert_eq!(read_2.next().await, Err(Closed));
    }
}
