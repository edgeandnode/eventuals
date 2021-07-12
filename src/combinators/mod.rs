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
        while let Ok(v) = source.next().await {
            writer.write(f(v).await);
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
                    match a_value {
                        Ok(a_value) => {
                            if ab.0.replace(a_value).is_none() {
                                count += 1;
                            }
                        },
                        Err(_) => { return ; }
                    }
                }
                b_value = b.next() => {
                    match b_value {
                        Ok(b_value) => {
                            if ab.1.replace(b_value).is_none() {
                                count += 1;
                            }
                        },
                        Err(_) => {
                            return;
                        }
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
                    match a_value {
                        Ok(a_value) => { ab.0 = a_value; }
                        Err(_) => { return; }
                    }
                }
                b_value = b.next() => {
                    match b_value {
                        Ok(b_value) => { ab.1 = b_value; }
                        Err(_) => { return; }
                    }
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
        while let Ok(mut next) = read.next().await {
            let end = time::Instant::now() + duration;
            loop {
                // Allow replacing the value until the time is up. This
                // necessarily introduces latency but de-duplicates when there
                // are intermittent bursts. Not sure what is better. Matching
                // common-ts for now.
                select! {
                    n = read.next() => {
                        if let Ok(n) = n {
                            next = n;
                        } else {
                            return;
                        }
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

// TODO: Add pipe? The "GC" semantics make this unclear. The idea
// behind pipe is to produce some side effect, which is a desirable
// end goal for eventuals (eg: pipe this value into a UI, or log the latest)
// but the part that is not clear here is what to do when the UI goes out of
// scope. Should pipe provide an explicit handle that cancels on drop?
//
// TODO: Probably do not add filter or reduce, they don't make as much sense for eventuals.
