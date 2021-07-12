use crate::*;
use futures::Future;
use std::time::Duration;

pub trait EventualExt: Sized + IntoReader {
    #[inline]
    fn map<F, O, Fut>(self, f: F) -> Eventual<O>
    where
        F: 'static + Send + FnMut(Self::Output) -> Fut,
        O: Value,
        Fut: Send + Future<Output = O>,
    {
        map(self, f)
    }

    #[inline]
    fn throttle(self, duration: Duration) -> Eventual<Self::Output> {
        throttle(self, duration)
    }
}

impl<E> EventualExt for E where E: IntoReader {}
