use crate::*;
use futures::Future;
use std::time::Duration;

/// Fluent style API extensions for any Eventual reader.
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

    #[inline]
    fn pipe<F>(self, f: F) -> PipeHandle
    where
        F: 'static + Send + FnMut(Self::Output),
    {
        pipe(self, f)
    }
}

impl<E> EventualExt for E where E: IntoReader {}

pub trait TryEventualExt<Ok, Err>: Sized + IntoReader<Output = Result<Ok, Err>>
where
    Ok: Value,
    Err: Value,
{
    #[inline]
    #[deprecated = "Not deterministic. This is a special case of filter. Retry should be better"]
    fn handle_errors<F>(self, f: F) -> Eventual<Ok>
    where
        F: 'static + Send + FnMut(Err),
    {
        #[allow(deprecated)]
        handle_errors(self, f)
    }
}

impl<E, Ok, Err> TryEventualExt<Ok, Err> for E
where
    E: IntoReader<Output = Result<Ok, Err>>,
    Ok: Value,
    Err: Value,
{
}
