use super::*;

use crate::Ptr;
use std::{
    collections::HashSet,
    hash::{Hash, Hasher},
    mem,
    sync::{Arc, Mutex},
    task::Waker,
};

pub enum ChangeVal<T> {
    Value(Option<Result<T, Closed>>),
    Waker(Waker),
}

pub struct Change<T> {
    inner: Ptr<Mutex<ChangeVal<T>>>,
}

pub struct ChangeReader<T> {
    pub change: Change<T>,
    pub unsubscribe_from: Arc<SharedState<T>>,
}

impl<T> Drop for ChangeReader<T> {
    fn drop(&mut self) {
        let mut lock = self.unsubscribe_from.subscribers.lock().unwrap();
        let mut updated: HashSet<_> = lock.deref().deref().clone();
        if updated.remove(&self.change) {
            *lock = Arc::new(updated);
        }
    }
}

impl<T> Change<T>
where
    T: Value,
{
    pub fn new() -> Self {
        Self {
            inner: Ptr::new(Mutex::new(ChangeVal::Value(None))),
        }
    }

    pub fn swap_or_wake(
        &self,
        swap: &mut Option<Result<T, Closed>>,
        cmp: &Option<Result<T, Closed>>,
        cx: &mut Context,
    ) {
        let mut lock = self.inner.lock().unwrap();
        if let ChangeVal::Value(value) = &mut *lock {
            if value != cmp {
                mem::swap(value, swap);
            }
        }
        if swap.is_none() {
            *lock = ChangeVal::Waker(cx.waker().clone())
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
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.hash(state)
    }
}
