use super::*;
use crate::error::Closed;
use std::sync::{Arc, Weak};

pub struct EventualWriter<T>
where
    T: Value,
{
    state: Weak<SharedState<T>>,
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
    pub(crate) fn new(state: &Arc<SharedState<T>>) -> Self {
        Self {
            state: Arc::downgrade(state),
        }
    }

    // This doesn't strictly need to take &mut self. Consider.
    // (Though if we want to use ArcSwap it certainly makes it easier)
    pub fn write(&mut self, value: T) -> Result<(), Closed> {
        self.write_private(Ok(value))
    }

    fn write_private(&mut self, value: Result<T, Closed>) -> Result<(), Closed> {
        let state = self.state.upgrade().ok_or_else(|| Closed)?;
        // See also b045e23a-f445-456f-a686-7e80de621cf2
        {
            let mut prev = state.last_write.lock().unwrap();
            *prev = Some(value);
        }
        state.notify_all();
        Ok(())
    }
}
