use crate::*;
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

//pub fn retry(

// TODO: Retry
// TODO: HandleErrors
// TODO: Consider re-exporting ByAddress<Arc<T>>. One nice thing about
// having a local version is that it would allow this lib to impl things
// like Error if ByAddress isn't already.
// TODO: Add pipe? The "GC" semantics make this unclear. The idea
// behind pipe is to produce some side effect, which is a desirable
// end goal for eventuals (eg: pipe this value into a UI, or log the latest)
// but the part that is not clear here is what to do when the UI goes out of
// scope. Should pipe provide an explicit handle that cancels on drop?
//
// TODO: Eventual.value
