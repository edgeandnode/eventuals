use crate::*;
use futures::Future;

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
}

impl<E> EventualExt for E where E: IntoReader {}
