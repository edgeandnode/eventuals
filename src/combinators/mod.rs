use crate::*;
use futures::never::Never;
use std::time::Duration;
use std::{future::Future, time::Instant};
use tokio::{self, select, time};

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

pub fn timer(interval: Duration) -> Eventual<Instant> {
    Eventual::spawn(move |mut writer| async move {
        loop {
            writer.write(Instant::now());
            time::sleep(interval).await;
        }
    })
}

// TODO: Put this in a macro to de-duplicate A/B and support more arguments.
pub fn join<A, Ar, B, Br>(a: Ar, b: Br) -> Eventual<(A, B)>
where
    A: Value,
    B: Value,
    Ar: IntoReader<Output = A>,
    Br: IntoReader<Output = B>,
{
    let mut a = a.into_reader();
    let mut b = b.into_reader();

    Eventual::spawn(move |mut writer| async move {
        let mut count = 0;
        let mut ab = (None, None);

        let mut ab = loop {
            select! {
                a_value = a.next() => {
                    if ab.0.replace(a_value?).is_none() {
                        count += 1;
                    }
                }
                b_value = b.next() => {
                    if ab.1.replace(b_value?).is_none() {
                        count += 1;
                    }
                }
            }
            if count == 2 {
                break (ab.0.unwrap(), ab.1.unwrap());
            }
        };
        loop {
            writer.write(ab.clone());

            select! {
                a_value = a.next() => {
                    ab.0 = a_value?;
                }
                b_value = b.next() => {
                    ab.1 = b_value?;
                }
            }
        }
    })
}

pub fn throttle<E>(read: E, duration: Duration) -> Eventual<E::Output>
where
    E: IntoReader,
{
    let mut read = read.into_reader();

    Eventual::spawn(move |mut writer| async move {
        loop {
            let mut next = read.next().await?;
            let end = time::Instant::now() + duration;
            loop {
                // Allow replacing the value until the time is up. This
                // necessarily introduces latency but de-duplicates when there
                // are intermittent bursts. Not sure what is better. Matching
                // common-ts for now.
                select! {
                    n = read.next() => {
                        next = n?;
                    }
                    _ = time::sleep_until(end) => {
                        break;
                    }
                }
            }
            writer.write(next);
        }
    })
}

/// Produce a side effect with the latest values of an eventual
pub fn pipe<E, F>(reader: E, mut f: F) -> PipeHandle
where
    E: IntoReader,
    F: 'static + Send + FnMut(E::Output),
{
    let mut reader = reader.into_reader();

    PipeHandle::new(Eventual::spawn(
        move |_writer: EventualWriter<Never>| async move {
            loop {
                f(reader.next().await?);
            }
        },
    ))
}

/// Pipe ceases when this is dropped
pub struct PipeHandle {
    _inner: Eventual<Never>,
}

impl PipeHandle {
    fn new(eventual: Eventual<Never>) -> Self {
        Self { _inner: eventual }
    }
}

pub fn handle_errors<E, F, Ok, Err>(source: E, f: F) -> Eventual<Ok>
where
    E: IntoReader<Output = Result<Ok, Err>>,
    F: 'static + Send + Fn(Err),
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

// TODO: Retry. This is needed to be supported because retry should be eventual
// aware in that it will only retry if there is no update available, instead
// preferring the update. It's a little tricky to write in a general sense because
// it is not clear _what_ is being retried. A retry can't force an upstream map
// to produce a value again. You could couple the map and retry API, but that's
// not great. The only thing I can think of is to have a function produce an eventual
// upon encountering an error. That seems like the right choice but need to let it simmer.
//
// Below is an "interesting" first attempt.
//
// This is a retry that is maximimally abstracted.
// It is somewhat experimental, but makes sense if you
// want to be able to not tie retry down to any particular
// other feature (like map). It's also BONKERS. See map_with_rety
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
                match next {
                    Ok(Ok(v)) => {
                        writer.write(v);
                        next = e.next().await;
                    }
                    // TODO: Is this how we want to handle closed?
                    Err(e) => {
                        return Err(e);
                    }
                    Ok(Err(err)) => {
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

pub fn map_with_retry<I, Ok, Err, F, Fut>(source: Eventual<I>, f: F) -> Eventual<Ok>
where
    F: 'static + Clone + Send + Fn(I) -> Fut,
    I: Value,
    Ok: Value,
    Err: Value,
    Fut: Send + Future<Output = Result<Ok, Err>>,
{
    retry(move |e| {
        let reader = source.subscribe();
        let f = f.clone();
        async move {
            if e.is_some() {
                // TODO: Configurable time via on_error, which will
                // also allow eg: loggging.
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
            map(reader, f)
        }
    })
}

// TODO: Consider re-exporting ByAddress<Arc<T>>. One nice thing about
// having a local version is that it would allow this lib to impl things
// like Error if ByAddress isn't already.
//
