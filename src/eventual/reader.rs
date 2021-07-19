use super::{change::ChangeReader, *};
use crate::{error::Closed, IntoReader};

// It's tempting here to provide some API that treats the Eventual like a
// Stream. That would be bad though, because it would expose all the APIs that
// come with stream. For example, someone could call `.map` on a Stream, but
// that would be bad because `.map` on Stream and `.map` on Eventual have very
// different semantics. In general, Stream has very different semantics. It even
// has a size_hint - a Stream is a progressively available Vec (distinct
// values), but an Eventual is an eventually consistent and updating "latest"
// value which infers no sequence and may drop intermediate values.
pub struct EventualReader<T> {
    change: ChangeReader<T>,
    prev: Option<Result<T, Closed>>,
}

impl<T> IntoReader for EventualReader<T>
where
    T: Value,
{
    type Output = T;
    fn into_reader(self) -> EventualReader<Self::Output> {
        self
    }
}

pub struct Next<'a, T> {
    eventual: &'a mut EventualReader<T>,
}

impl<'a, T> Future for Next<'a, T>
where
    T: Value,
{
    type Output = Result<T, Closed>;
    // TODO: This is currently checking for a pushed value, but that will require
    // eg: map() to run in separate tasks. It might be desireble to have this poll
    // the future that would produce values. But... that may be very complex. A
    // refactor may be necessary.
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut swap = None;
        self.eventual
            .change
            .change
            .swap_or_wake(&mut swap, &self.eventual.prev, cx);
        match swap {
            None => Poll::Pending,
            Some(value) => {
                self.eventual.prev = Some(value.clone());
                Poll::Ready(value)
            }
        }
    }
}

impl<T> EventualReader<T>
where
    T: Value,
{
    pub fn next(&mut self) -> Next<T> {
        Next { eventual: self }
    }

    pub(crate) fn new(state: Arc<SharedState<T>>) -> Self {
        let change = state.subscribe();

        EventualReader { change, prev: None }
    }
}
