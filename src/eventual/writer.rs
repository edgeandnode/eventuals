use futures::{channel::oneshot::Receiver, future::Shared};

use super::{change::ChangeValNoWake, *};
use crate::error::Closed;
use futures::FutureExt;
use std::{
    mem,
    ops::DerefMut,
    sync::{Arc, MutexGuard, Weak},
};

pub struct EventualWriter<T>
where
    T: Value,
{
    state: Weak<SharedState<T>>,
    closed: Shared<Receiver<()>>,
}

impl<T> Drop for EventualWriter<T>
where
    T: Value,
{
    fn drop(&mut self) {
        let _ignore = self.write_private(Err(Closed));
    }
}

impl<T> EventualWriter<T>
where
    T: Value,
{
    pub(crate) fn new(state: &Arc<SharedState<T>>, closed: Receiver<()>) -> Self {
        Self {
            state: Arc::downgrade(state),
            closed: closed.shared(),
        }
    }

    pub fn closed(&self) -> impl 'static + Future + Send + Unpin {
        self.closed.clone()
    }

    /// Get a snapshot of the current value of this Eventual, if any, without
    /// waiting.
    pub fn value_immediate(&self) -> Option<T> {
        let state = self.state.upgrade()?;
        let last_write = state.last_write.lock().unwrap();
        let value: Option<&T> = last_write.deref().into();
        value.cloned()
    }

    /// Write the next value of this Eventual.
    pub fn write(&mut self, next: T) {
        self.write_private(Ok(next));
    }

    /// Update the value of this Eventual, given the previous value if any.
    pub fn update<F: FnOnce(Option<&T>) -> T>(&mut self, f: F) -> Option<T> {
        let state = self.state.upgrade()?;
        let last_write = state.last_write.lock().unwrap();
        let next = f(last_write.deref().into());
        self.write_private_inner(&state, last_write, Ok(next))
    }

    fn write_private(&mut self, next: Result<T, Closed>) {
        let state = match self.state.upgrade() {
            Some(state) => state,
            None => return,
        };
        let last_write = state.last_write.lock().unwrap();
        self.write_private_inner(&state, last_write, next);
    }

    #[inline]
    fn write_private_inner(
        &mut self,
        state: &SharedState<T>,
        mut last_write: MutexGuard<ChangeValNoWake<T>>,
        value: Result<T, Closed>,
    ) -> Option<T> {
        let prev = if let Ok(value) = value {
            mem::replace(last_write.deref_mut(), ChangeValNoWake::Value(value))
        } else {
            let prev = mem::replace(last_write.deref_mut(), ChangeValNoWake::None);
            match &prev {
                ChangeValNoWake::None => {
                    *last_write = ChangeValNoWake::Finalized(None);
                }
                ChangeValNoWake::Value(value) => {
                    *last_write = ChangeValNoWake::Finalized(Some(value.clone()));
                }
                ChangeValNoWake::Finalized(_) => unreachable!(),
            };
            prev
        };
        drop(last_write);
        state.notify_all();
        prev.into()
    }
}
