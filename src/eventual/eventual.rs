use crate::IntoReader;

use super::shared_state::SharedState;
use super::*;

pub struct Eventual<T> {
    state: Arc<SharedState<T>>,
}

impl<T> Eventual<T>
where
    T: Value,
{
    pub fn new() -> (EventualWriter<T>, Self) {
        let state = Arc::new(SharedState::new());
        (EventualWriter::new(state.clone()), Eventual { state })
    }
}

impl<T> Eventual<T>
where
    T: Value,
{
    pub fn subscribe(&self) -> EventualReader<T> {
        EventualReader::new(self.state.clone())
    }
}

impl<T> Clone for Eventual<T> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
        }
    }
}

impl<T> IntoReader for &'_ Eventual<T>
where
    T: Value,
{
    type Output = T;
    fn into_reader(self) -> EventualReader<Self::Output> {
        self.subscribe()
    }
}
