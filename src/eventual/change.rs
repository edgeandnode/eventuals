use super::*;

use crate::Ptr;
use std::{
    collections::HashSet,
    hash::{Hash, Hasher},
    mem,
    ops::DerefMut,
    ptr,
    sync::{Arc, Mutex},
    task::Waker,
};

#[cfg(feature = "trace")]
mod busy {
    use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
    use tokio::sync::Notify;

    pub static BUSY_COUNT: AtomicUsize = AtomicUsize::new(0);
    pub static BUSY_WAKER: Notify = Notify::const_new();

    pub fn set_busy() {
        BUSY_COUNT.fetch_add(1, SeqCst);
    }
    pub fn clear_busy() {
        let prev = BUSY_COUNT.fetch_sub(1, SeqCst);
        debug_assert!(prev != 0);
        if prev == 1 {
            BUSY_WAKER.notify_waiters();
        }
    }

    /// Ready once all eventuals readers are waiting on a new value. _Generally_
    /// speaking, it is possible to ensure that a change has propagated through
    /// an eventuals pipeline using this method. However, there is no guarantee
    /// that this will complete in a timely fashion if ever. A sufficiently
    /// layered pipeline that is always moving values through may never be idle.
    /// So, this is only useful in isolated tests.
    pub async fn idle() {
        loop {
            let notified = BUSY_WAKER.notified();
            if BUSY_COUNT.load(SeqCst) == 0 {
                return;
            }
            notified.await
        }
    }
}

#[cfg(feature = "trace")]
pub use busy::idle;

struct Busy<T>(T);

impl<T> Busy<T> {
    fn new(value: T) -> Self {
        #[cfg(feature = "trace")]
        busy::set_busy();
        Self(value)
    }

    fn unbusy(mut self) -> T {
        
            let inner = unsafe {ptr::read(&mut self.0)};
            mem::forget(self);
            #[cfg(feature = "trace")]
            busy::clear_busy();

            inner
        
    }
}

impl<T> Deref for Busy<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> Drop for Busy<T> {
    fn drop(&mut self) {
        #[cfg(feature = "trace")]
        busy::clear_busy();
    }
}

enum ChangeVal<T> {
    None(Busy<()>),
    Value(Busy<T>),
    Finalized(Busy<Option<T>>),
    Waker(Waker),
}

pub enum ChangeValNoWake<T> {
    None,
    Value(T),
    Finalized(Option<T>),
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
            inner: Ptr::new(Mutex::new(ChangeVal::None(Busy::new(())))),
        }
    }

    pub fn poll(
        &self,
        cmp: &Option<Result<T, Closed>>,
        cx: &mut Context,
    ) -> Option<Result<T, Closed>> {
        let mut lock = self.inner.lock().unwrap();

        // Move the value out pre-emptively to keep things sane for the borrow checker.
        // Depending on the branch ahead we'll swap in different values.
        let value = mem::replace(lock.deref_mut(), ChangeVal::None(Busy::new(())));

        match value {
            // If there is a new value and it is different than our previously
            // observed value return it. Otherwise fall back to waking later.
            ChangeVal::Value(value) => {
                let value = value.unbusy();
                let value = Some(Ok(value));
                if cmp != &value {
                    return value;
                }
            }
            // If the eventual is finalized from the writer end make sure that the final value
            // (if any) is returned once as though it were a normal value. Then (possibly on
            // a subsequent poll) return the Err.
            ChangeVal::Finalized(value) => {
                if let Some(value) = value.unbusy() {
                    let value = Some(Ok(value));
                    if cmp != &value {
                        *lock = ChangeVal::Finalized(Busy::new(None));
                        return value;
                    }
                }
                return Some(Err(Closed));
            }
            // There is no update. The waker may need to be re-scheduled.
            ChangeVal::None(_) | ChangeVal::Waker(_) => {}
        }
        *lock = ChangeVal::Waker(cx.waker().clone());
        None
    }

    pub fn set_value(&self, value: &Mutex<ChangeValNoWake<T>>) {
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

            // Move out of inner early for borrow checker.
            let prev = mem::replace(inner.deref_mut(), ChangeVal::None(Busy::new(())));

            match value.deref() {
                ChangeValNoWake::None => {
                    // Prev must be None. The only time set_value is called when
                    // value is None is when the value has never before been set
                    // (therefore prev is not ChangeValue::Finalized or
                    // ChangeVal::Value) and the ChangeVal has no waker because
                    // we are now adding it to the subscriber list.
                    // If this assert fails, we would want `*inner = prev;`.
                    debug_assert!(matches!(prev, ChangeVal::None(_)));

                    // Since we know this is None, there is no need to check
                    // for the waker (below)
                    return;
                }
                // There is an update.
                ChangeValNoWake::Value(value) => {
                    // The previous value must not have been finalized.
                    // It is not possible to move from a finalized state to
                    // then have updates.
                    debug_assert!(!matches!(prev, ChangeVal::Finalized(_)));
                    // Set the value.
                    *inner = ChangeVal::Value(Busy::new(value.clone()));
                }
                // If closing, this is more tricky because we want to preserve
                // the last update (if any) so that the final value propagates
                // all the way through.
                ChangeValNoWake::Finalized(finalized) => {
                    // Verify that it's not copying the final value over again
                    // because in racey situations it may have been copied once
                    // then had the value consumed. It wouldn't be the end of the
                    // world to reset the finalized state, but would result in
                    // some unnecessary work.
                    if !matches!(prev, ChangeVal::Finalized(_)) {
                        *inner = ChangeVal::Finalized(Busy::new(finalized.clone()));
                    }
                }
            };

            prev

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
