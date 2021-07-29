use crate::*;
use futures::future::select_all;
use never::Never;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{future::Future, time::Instant};
use tokio::{
    self, select,
    time::{sleep, sleep_until},
};

/// Applies an operation to each observed snapshot from the source. For example:
/// map([1, 2, 3, 4, 5], |v| v+1) may produce something like [2, 6] or [3, 4,
/// 6]. In this case, 6 is the only value guaranteed to be observed eventually.
pub fn map<E, I, O, F, Fut>(source: E, mut f: F) -> Eventual<O>
where
    E: IntoReader<Output = I>,
    F: 'static + Send + FnMut(I) -> Fut,
    I: Value,
    O: Value,
    Fut: Send + Future<Output = O>,
{
    let mut source = source.into_reader();

    Eventual::spawn(|mut writer| async move {
        loop {
            writer.write(f(source.next().await?).await);
        }
    })
}

/// Periodically writes a new value of the time elapsed. No guarantee is made
/// about frequency or the value written except that at least "interval" time
/// has passed since producing the last snapshot.
pub fn timer(interval: Duration) -> Eventual<Instant> {
    Eventual::spawn(move |mut writer| async move {
        loop {
            writer.write(Instant::now());
            sleep(interval).await;
        }
    })
}

/// Indicates the type can be used with the join method. Not intended to
/// be used directly.
pub trait Joinable {
    type Output;
    fn join(self) -> Eventual<Self::Output>;
}

macro_rules! impl_tuple {
    ($len:expr, $($T:ident, $t:ident),*) => {
        impl<T, $($T,)*> Selectable for ($($T,)*)
            where
            $($T: IntoReader<Output = T>,)*
            T: Value,
        {
            type Output = T;
            fn select(self) -> Eventual<Self::Output> {
                let ($($t),*) = self;
                $(let $t = $t.into_reader();)*
                #[allow(deprecated)]
                vec![$($t),*].select()
            }
        }

        impl<$($T,)*> Joinable for ($($T,)*)
            where
                $($T: IntoReader,)*
        {
            type Output = ($($T::Output),*);

            #[allow(non_snake_case)]
            fn join(self) -> Eventual<Self::Output> {
                let ($($T),*) = self;
                $(let mut $T = $T.into_reader();)*

                Eventual::spawn(move |mut writer| async move {
                    // In the first section we wait until all values are available
                    let mut count:usize = 0;
                    $(let mut $t = None;)*
                    let ($(mut $t,)*) = loop {
                        select! {
                            $(
                                next = $T.next() => {
                                    if $t.replace(next?).is_none() {
                                        count += 1;
                                    }
                                }
                            )*
                        }
                        if count == 2 {
                            break ($($t.unwrap()),*);
                        }
                    };
                    // Once all values are available, start writing but continue
                    // to update.
                    loop {
                        writer.write(($($t.clone(),)*));

                        select! {
                            $(
                                next = $T.next() => {
                                    $t = next?;
                                }
                            )*
                        }
                    }
                })
            }
        }
    };
}

// This macro exists to expand to the implementation for one tuple and
// call itself for the smaller tuple until running out of tuples.
macro_rules! impl_tuples {
    ($len:expr, $A:ident, $a:ident) => { };
    ($len:expr, $A:ident, $a:ident, $($T:ident, $t:ident),+) => {
        impl_tuple!($len, $A, $a, $($T, $t),+);
        impl_tuples!($len - 1, $($T, $t),+);
    }
}

impl_tuples!(12, A, a, B, b, C, c, D, d, E, e, F, f, G, g, H, h, I, i, J, j, K, k, L, l);

/// An eventual that will only progress once all inputs are available, and then
/// also progress with each change as they become available. For example,
/// join((["a", "b, "c"], [1, 2, 3])) may observe something like [("a", 1),
/// ("a", 2), ("c", 2), ("c", 3)] or [("c", 1), ("c", 3)]. The only snapshot
/// that is guaranteed to be observed is ("c", 3).
pub fn join<J>(joinable: J) -> Eventual<J::Output>
where
    J: Joinable,
{
    joinable.join()
}

pub trait Selectable {
    type Output;
    #[deprecated = "Not deterministic. This doesn't seem as harmful as filter, because it doesn't appear to miss updates."]
    fn select(self) -> Eventual<Self::Output>;
}

#[deprecated = "Not deterministic. This doesn't seem as harmful as filter, because it doesn't appear to miss updates."]
pub fn select<S>(selectable: S) -> Eventual<S::Output>
where
    S: Selectable,
{
    #[allow(deprecated)]
    selectable.select()
}

impl<R> Selectable for Vec<R>
where
    R: IntoReader,
{
    type Output = R::Output;
    fn select(self) -> Eventual<Self::Output> {
        // TODO: With specialization we can avoid what is essentially an
        // unnecessary clone when R is EventualReader
        let mut readers: Vec<_> = self.into_iter().map(|v| v.into_reader()).collect();
        Eventual::spawn(move |mut writer| async move {
            loop {
                if readers.len() == 0 {
                    return Err(Closed);
                }
                let read_futs: Vec<_> = readers.iter_mut().map(|r| r.next()).collect();

                let (output, index, remainder) = select_all(read_futs).await;

                // Ideally, we would want to re-use this list, but in most
                // cases we can't because it may have been shuffled.
                drop(remainder);

                match output {
                    Ok(value) => {
                        writer.write(value);
                    }
                    Err(Closed) => {
                        readers.remove(index);
                    }
                }
            }
        })
    }
}

/// Prevents observation of values more frequently than the provided duration.
/// The final value is guaranteed to be observed.
pub fn throttle<E>(read: E, duration: Duration) -> Eventual<E::Output>
where
    E: IntoReader,
{
    let mut read = read.into_reader();

    Eventual::spawn(move |mut writer| async move {
        loop {
            let mut next = read.next().await?;
            let end = tokio::time::Instant::now() + duration;
            loop {
                // Allow replacing the value until the time is up. This
                // necessarily introduces latency but de-duplicates when there
                // are intermittent bursts. Not sure what is better. Matching
                // common-ts for now.
                select! {
                    n = read.next() => {
                        next = n?;
                    }
                    _ = sleep_until(end) => {
                        break;
                    }
                }
            }
            writer.write(next);
        }
    })
}

/// Produce a side effect with the latest snapshots as they become available.
/// The caller must not drop the returned PipeHandle until it is no longer
/// desirable to produce the side effect.
pub fn pipe<E, F>(reader: E, mut f: F) -> PipeHandle
where
    E: IntoReader,
    F: 'static + Send + FnMut(E::Output),
{
    let mut reader = reader.into_reader();

    #[allow(unreachable_code)]
    PipeHandle::new(Eventual::spawn(|_writer| async move {
        loop {
            f(reader.next().await?);
        }
        // Prevent the writer from being dropped. Normally we would expect
        // _writer to not be dropped, but the async move creates a new lexical
        // scope for the Future. If _writer is not moved into the Future it
        // would be dropped right after the Future is created and before the
        // closure returns. Without this line, Pipe stops prematurely.
        drop(_writer);
    }))
}

/// Pipe ceases when this is dropped
#[must_use]
pub struct PipeHandle {
    inner: Eventual<Never>,
}

impl PipeHandle {
    fn new(eventual: Eventual<Never>) -> Self {
        Self { inner: eventual }
    }

    /// Prevent the pipe operation from ever stopping for as long
    /// as snapshots are observed.
    #[inline]
    pub fn forever(self) {
        let Self { inner } = self;
        tokio::task::spawn(async move {
            // Drops the reader when the writer is closed
            // This value is always Err(Closed) because inner is Eventual<Never>
            let _closed = inner.value().await;
        });
    }
}

#[deprecated = "Not deterministic. This is a special case of filter. Retry should be better"]
pub fn handle_errors<E, F, Ok, Err>(source: E, mut f: F) -> Eventual<Ok>
where
    E: IntoReader<Output = Result<Ok, Err>>,
    F: 'static + Send + FnMut(Err),
    Ok: Value,
    Err: Value,
{
    let mut reader = source.into_reader();

    Eventual::spawn(move |mut writer| async move {
        loop {
            match reader.next().await? {
                Ok(v) => writer.write(v),
                Err(e) => f(e),
            }
        }
    })
}

// TODO: Improve retry API. Some retry is needed because retry should be
// eventual aware in that it will only retry if there is no update available,
// instead preferring the update. It's a little tricky to write in a general
// sense because it is not clear _what_ is being retried. A retry can't force an
// upstream map to produce a value again. You could couple the map and retry
// API, but that's not great. The only thing I can think of is to have a
// function produce an eventual upon encountering an error. That seems like the
// right choice but need to let it simmer. With this API the retry "region" is
// configurable where the "region" could be an entire pipeline of eventuals.
//
// Below is an "interesting" first attempt.
//
// This is a retry that is maximally abstracted. It is somewhat experimental,
// but makes sense if you want to be able to not tie retry down to any
// particular other feature (like map). It's also BONKERS. See map_with_retry
// for usage.
pub fn retry<Ok, Err, F, Fut>(mut f: F) -> Eventual<Ok>
where
    Ok: Value,
    Err: Value,
    Fut: Send + Future<Output = Eventual<Result<Ok, Err>>>,
    F: 'static + Send + FnMut(Option<Err>) -> Fut,
{
    Eventual::spawn(move |mut writer| async move {
        loop {
            let mut e = f(None).await.subscribe();
            let mut next = e.next().await;

            loop {
                match next? {
                    Ok(v) => {
                        writer.write(v);
                        next = e.next().await;
                    }
                    Err(err) => {
                        select! {
                            e_temp = f(Some(err)) => {
                                e = e_temp.subscribe();
                                next = e.next().await;
                            }
                            n_temp = e.next() => {
                                next = n_temp;
                            }
                        }
                    }
                }
            }
        }
    })
}

/// Ensure that a fallible map operation will succeed eventually. For example
/// given map_with_retry(["url_1", "url_2"], fallibly_get_data, sleep) may
/// produce ["data_1", "data_2"] or just ["data_2"]. The difference between
/// map_with_retry and something like map(source, retry(fallibly_get_data,
/// on_err)) is that the former supports 'moving on' to "url_2" even if "url_1"
/// is in a retry state, whereas the latter would have to complete one item
/// fully before progressing. It is because of this distinction that
/// map_with_retry is allowed to retry forever instead of giving up after a set
/// number of attempts.
pub fn map_with_retry<Ok, Err, F, Fut, E, FutE, R>(source: R, f: F, on_err: E) -> Eventual<Ok>
where
    R: IntoReader,
    F: 'static + Send + FnMut(R::Output) -> Fut,
    E: 'static + Send + Sync + FnMut(Err) -> FutE,
    Ok: Value,
    Err: Value,
    Fut: Send + Future<Output = Result<Ok, Err>>,
    FutE: Send + Future<Output = ()>,
{
    let source = source.into_reader();

    // Wraping the FnMut values in Arc<Mutex<_>> allows us
    // to use FnMut instead of Fn, and not require Fn to impl
    // clone. This should make it easier to do things like
    // exponential backoff.
    let f = Arc::new(Mutex::new(f));
    let on_err = Arc::new(Mutex::new(on_err));

    retry(move |e| {
        let mut reader = source.clone();
        let f = f.clone();
        let on_err = on_err.clone();
        async move {
            if let Some(e) = e {
                let fut = {
                    let mut locked = on_err.lock().unwrap();
                    locked(e)
                };
                fut.await;
                // Without this line there is a very subtle problem.
                // One thing that map_with_retry needs to do is resume as
                // of the state of the source. We accomplish this with clone.
                // But, consider the following scenario: if the source had prev=A,
                // then [B, A] is observed, and A needs to retry. Without this line
                // the output of B could have been produced and the output of
                // map(A) would not have been produced. Interestingly, we also
                // know that this line does not force a double-read, because in order
                // to get here the reader must have had at least one observation.
                // Unless you count (Ok(A), Fail(B), Ok(A)) as a double read.
                //
                // There's one more subtle issue to consider, which is why force_dirty
                // is not public. force_dirty could cause the final value to be
                // double-read if the eventual is closed. However, we know that in this
                // case it was not ready to receive closed.
                //
                // This does raise a philisophical question about guaranteeing that
                // the last value is observed though. It could be that retry gets
                // stuck here on the last value forever. (Unless the readers are dropped)
                reader.force_dirty();
            }
            map(reader, move |value| {
                let fut = {
                    let mut locked = f.lock().unwrap();
                    locked(value)
                };
                fut
            })
        }
    })
}
