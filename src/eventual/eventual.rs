use super::shared_state::SharedState;
use super::*;
use crate::IntoReader;
use futures::channel::oneshot;
use futures::never::Never;
use tokio::select;

pub struct Eventual<T> {
    state: Arc<SharedState<T>>,
}

impl<T> Eventual<T>
where
    T: Value,
{
    pub fn new() -> (EventualWriter<T>, Self) {
        let (sender, receiver) = oneshot::channel();
        let state = Arc::new(SharedState::new(sender));
        (EventualWriter::new(&state, receiver), Eventual { state })
    }

    pub fn spawn<F, Fut>(f: F) -> Self
    where
        F: 'static + Send + FnOnce(EventualWriter<T>) -> Fut,
        Fut: Future<Output = Result<Never, Closed>> + Send,
    {
        let (writer, eventual) = Eventual::new();
        tokio::spawn(async move {
            select!(
                _ = writer.closed() => {}
                _ = async { f(writer).await }  => {}
            );
        });
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
    #[inline]
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
    #[inline]
    fn into_reader(self) -> EventualReader<Self::Output> {
        self.subscribe()
    }
}

impl<T> IntoReader for Eventual<T>
where
    T: Value,
{
    type Output = T;
    #[inline]
    fn into_reader(self) -> EventualReader<Self::Output> {
        self.subscribe()
    }
}
