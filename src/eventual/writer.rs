use super::*;
use crate::error::Closed;
use std::sync::Arc;

pub struct EventualWriter<T>
where
    T: Value,
{
    state: Arc<SharedState<T>>,
}

impl<T> Drop for EventualWriter<T>
where
    T: Value,
{
    fn drop(&mut self) {
        self.write_private(Err(Closed))
    }
}

impl<T> EventualWriter<T>
where
    T: Value,
{
    pub(crate) fn new(state: Arc<SharedState<T>>) -> Self {
        Self { state }
    }

    // This doesn't strictly need to take &mut self. Consider.
    pub fn write(&mut self, value: T) {
        self.write_private(Ok(value))
    }

    fn write_private(&mut self, value: Result<T, Closed>) {
        // See also b045e23a-f445-456f-a686-7e80de621cf2
        {
            let mut prev = self.state.last_write.lock().unwrap();
            *prev = Some(value);
        }
        self.state.notify_all()
    }
}
