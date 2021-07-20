use std::ops::DerefMut;

use super::change::{ChangeReader, ChangeVal};
use super::shared_state::SharedState;
use super::*;
use crate::IntoReader;
use futures::channel::oneshot;
use futures::never::Never;
use tokio::select;

/// The entry point for getting the latest snapshots of values
/// written by an EventualWriter. It supports multiple styles
/// of observation.
pub struct Eventual<T> {
    state: Arc<SharedState<T>>,
}

impl<T> Eventual<T>
where
    T: Value,
{
    /// Create a reader/writer pair.
    pub fn new() -> (EventualWriter<T>, Self) {
        let (sender, receiver) = oneshot::channel();
        let state = Arc::new(SharedState::new(sender));
        (EventualWriter::new(&state, receiver), Eventual { state })
    }

    /// Create an eventual having a final value. This is useful for creating
    /// "mock" eventuals to pass into consumers.
    pub fn from_value(value: T) -> Self {
        let (mut writer, eventual) = Eventual::new();
        writer.write(value);
        eventual
    }

    /// A helper for spawning a task which writes to an eventual.
    /// These are used extensively within the library for eventuals
    /// which update continuously over time.
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

    /// Subscribe to present and future snapshots of the value in this Eventual.
    /// Generally speaking the observations of snapshots take into account the
    /// state of the reader such that:
    ///   * The same observation is not made redundantly (same value twice in a
    ///     row)
    ///   * The observations always move forward in time
    ///   * The final observation will always be eventually consistent with the
    ///     final write.
    pub fn subscribe(&self) -> EventualReader<T> {
        EventualReader::new(self.state.clone())
    }

    /// Get a future that resolves with a snapshot of the present value of the
    /// Eventual, if any, or a future snapshot if none is available. Which
    /// snapshot is returned depends on when the Future is polled (as opposed
    /// to when the Future is created)
    pub fn value(&self) -> ValueFuture<T> {
        let change = self.state.clone().subscribe();
        ValueFuture {
            change: Some(change),
        }
    }

    /// Get a snapshot of the current value of this Eventual, if any,
    /// without waiting.
    pub fn value_immediate(&self) -> Option<T> {
        match self.state.last_write.lock().unwrap().deref_mut() {
            ChangeVal::None => None,
            ChangeVal::Value(t) => Some(t.clone()),
            ChangeVal::Finalized(t) => t.clone(),
            ChangeVal::Waker(_) => {
                unreachable!();
            }
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
        let update = self.change.as_mut().unwrap().change.poll(&None, cx);
        match update {
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
