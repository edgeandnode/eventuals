use futures::{channel::oneshot::Receiver, future::Shared};

use super::{change::ChangeValNoWake, *};
use crate::error::Closed;
use futures::FutureExt;
use std::{
    mem,
    ops::DerefMut,
    sync::{Arc, Weak},
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

    pub fn write(&mut self, value: T) {
        self.write_private(Ok(value))
    }

    fn write_private(&mut self, value: Result<T, Closed>) {
        if let Some(state) = self.state.upgrade() {
            // See also b045e23a-f445-456f-a686-7e80de621cf2
            {
                let mut prev = state.last_write.lock().unwrap();

                if let Ok(value) = value {
                    *prev = ChangeValNoWake::Value(value);
                } else {
                    match mem::replace(prev.deref_mut(), ChangeValNoWake::None) {
                        ChangeValNoWake::None => {
                            *prev = ChangeValNoWake::Finalized(None);
                        }
                        ChangeValNoWake::Value(value) => {
                            *prev = ChangeValNoWake::Finalized(Some(value));
                        }
                        ChangeValNoWake::Finalized(_) => unreachable!(),
                    }
                }
            }
            state.notify_all();
        }
    }
}
