use super::change::ChangeReader;
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

    pub fn subscribe(&self) -> EventualReader<T> {
        EventualReader::new(self.state.clone())
    }

    pub fn value(&self) -> ValueFuture<T> {
        let change = self.state.clone().subscribe();
        ValueFuture {
            change: Some(change),
        }
    }

    #[cfg(feature = "trace")]
    pub fn subscriber_count(&self) -> usize {
        self.state.subscribers.lock().unwrap().len()
    }
}

pub struct ValueFuture<T> {
    change: Option<ChangeReader<T>>,
}

impl<T> Future for ValueFuture<T>
where
    T: Value,
{
    type Output = Result<T, Closed>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut swap = None;
        let prev = None;
        self.change
            .as_mut()
            .unwrap()
            .change
            .swap_or_wake(&mut swap, &prev, cx);
        match swap {
            None => Poll::Pending,
            Some(value) => {
                self.change = None;
                Poll::Ready(value)
            }
        }
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
