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
        (EventualWriter::new(&state), Eventual { state })
    }

    pub fn spawn<F, Fut>(f: F) -> Self
    where
        F: 'static + Send + FnOnce(EventualWriter<T>) -> Fut,
        Fut: Future<Output = Result<(), Closed>> + Send + 'static,
    {
        let (writer, eventual) = Eventual::new();
        tokio::spawn(async move {
            // This would return an error if the readers
            // are dropped. Ok to ignore this.
            let _ignore = f(writer).await;
        }); // TODO: Return JoinHandle?
        eventual
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
